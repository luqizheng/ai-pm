//! 全局 LLM 供应商配置（OpenAI 兼容面：deepseek / openai / glm / minimax / ollama）。
//!
//! 密钥策略（docs/ai-pm-architecture-principles.md §密钥管理）：明文永不落库。
//! 写入时用 `AI_PM_SECRET_KEY` 派生 AES-256-GCM 密钥加密（nonce 前缀随密文
//! 一起存 BYTEA），读取时按需解密；界面只拿到 `has_api_key` 与末 4 位提示。

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

/// GCM nonce 长度（96 bit，NIST SP 800-38D 推荐值）。
const NONCE_LEN: usize = 12;

pub const PROVIDERS: [&str; 5] = ["deepseek", "openai", "glm", "minimax", "ollama"];

/// `GET /v1/pm/llm/providers` 的行（字段源：`frontend/src/api/types.ts`）。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct LlmProviderRow {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub has_api_key: bool,
    pub api_key_hint: Option<String>,
    pub enabled: bool,
    pub timeout_secs: i32,
    pub updated_at: DateTime<Utc>,
}

/// `PUT /v1/pm/llm/providers/{provider}` 的请求体。
///
/// `api_key` 缺省或空串表示保持原密钥；`clear_api_key = true` 表示删除密钥
/// （Ollama 等无需鉴权的本地模型用）。
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateLlmProviderRequest {
    pub base_url: String,
    pub model: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub clear_api_key: bool,
    pub enabled: bool,
    pub timeout_secs: i32,
}

#[derive(Debug, thiserror::Error)]
pub enum LlmConfigError {
    #[error("unknown provider: {0}")]
    UnknownProvider(String),
    #[error("{0}")]
    Validation(&'static str),
    #[error("AI_PM_SECRET_KEY is not set")]
    SecretKeyMissing,
    #[error("api key could not be encrypted or decrypted")]
    Crypto,
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

fn cipher() -> Result<Aes256Gcm, LlmConfigError> {
    let raw = std::env::var("AI_PM_SECRET_KEY").map_err(|_| LlmConfigError::SecretKeyMissing)?;
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(LlmConfigError::SecretKeyMissing);
    }
    // 32-byte key derived from the operator secret (any length input).
    let key: [u8; 32] = Sha256::digest(raw.as_bytes()).into();
    Aes256Gcm::new_from_slice(&key).map_err(|_| LlmConfigError::Crypto)
}

fn hint_of(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    let start = chars.len().saturating_sub(4);
    let tail: String = chars[start..].iter().collect();
    format!("****{tail}")
}

/// AES-256-GCM 加密，返回 `nonce || ciphertext`。
pub fn encrypt_api_key(key: &str) -> Result<(Vec<u8>, String), LlmConfigError> {
    let cipher = cipher()?;
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).map_err(|_| LlmConfigError::Crypto)?;
    let encrypted = cipher
        .encrypt(Nonce::from_slice(&nonce), key.as_bytes())
        .map_err(|_| LlmConfigError::Crypto)?;
    let mut payload = Vec::with_capacity(NONCE_LEN + encrypted.len());
    payload.extend_from_slice(&nonce);
    payload.extend_from_slice(&encrypted);
    Ok((payload, hint_of(key)))
}

fn decrypt_api_key_bytes(payload: &[u8]) -> Result<String, LlmConfigError> {
    if payload.len() <= NONCE_LEN {
        return Err(LlmConfigError::Crypto);
    }
    let (nonce, ciphertext) = payload.split_at(NONCE_LEN);
    let plain = cipher()?
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| LlmConfigError::Crypto)?;
    String::from_utf8(plain).map_err(|_| LlmConfigError::Crypto)
}

fn ensure_known(provider: &str) -> Result<(), LlmConfigError> {
    if PROVIDERS.contains(&provider) {
        Ok(())
    } else {
        Err(LlmConfigError::UnknownProvider(provider.to_string()))
    }
}

/// 五家供应商按固定顺序返回（迁移里预置，接口保证顺序稳定便于前端卡片排列）。
pub async fn list_providers(pool: &PgPool) -> Result<Vec<LlmProviderRow>, LlmConfigError> {
    let rows = sqlx::query_as::<_, LlmProviderRow>(
        "SELECT provider, base_url, model, \
                (api_key_ciphertext IS NOT NULL) AS has_api_key, \
                api_key_hint, enabled, timeout_secs, updated_at \
         FROM ai.llm_provider \
         ORDER BY CASE provider \
             WHEN 'deepseek' THEN 1 WHEN 'openai' THEN 2 WHEN 'glm' THEN 3 \
             WHEN 'minimax' THEN 4 WHEN 'ollama' THEN 5 ELSE 99 END",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn update_provider(
    pool: &PgPool,
    provider: &str,
    req: UpdateLlmProviderRequest,
) -> Result<LlmProviderRow, LlmConfigError> {
    ensure_known(provider)?;

    let base_url = req.base_url.trim().trim_end_matches('/').to_string();
    if !(base_url.starts_with("http://") || base_url.starts_with("https://")) {
        return Err(LlmConfigError::Validation(
            "base_url must start with http:// or https://",
        ));
    }
    let model = req.model.trim().to_string();
    if model.is_empty() {
        return Err(LlmConfigError::Validation("model is required"));
    }
    if !(1..=600).contains(&req.timeout_secs) {
        return Err(LlmConfigError::Validation(
            "timeout_secs must be between 1 and 600",
        ));
    }

    let current: Option<(Option<Vec<u8>>, Option<String>)> = sqlx::query_as(
        "SELECT api_key_ciphertext, api_key_hint FROM ai.llm_provider WHERE provider = $1",
    )
    .bind(provider)
    .fetch_optional(pool)
    .await?;
    let Some((current_cipher, current_hint)) = current else {
        return Err(LlmConfigError::UnknownProvider(provider.to_string()));
    };

    let (ciphertext, hint) = if req.clear_api_key {
        (None, None)
    } else {
        match req
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            Some(key) => {
                let (bytes, hint) = encrypt_api_key(key)?;
                (Some(bytes), Some(hint))
            }
            None => (current_cipher, current_hint),
        }
    };

    let row = sqlx::query_as::<_, LlmProviderRow>(
        "UPDATE ai.llm_provider SET \
             base_url = $2, model = $3, api_key_ciphertext = $4, api_key_hint = $5, \
             enabled = $6, timeout_secs = $7, updated_at = now() \
         WHERE provider = $1 \
         RETURNING provider, base_url, model, \
                   (api_key_ciphertext IS NOT NULL) AS has_api_key, \
                   api_key_hint, enabled, timeout_secs, updated_at",
    )
    .bind(provider)
    .bind(&base_url)
    .bind(&model)
    .bind(ciphertext.as_deref())
    .bind(hint.as_deref())
    .bind(req.enabled)
    .bind(req.timeout_secs)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| LlmConfigError::UnknownProvider(provider.to_string()))?;
    Ok(row)
}

/// 读取某供应商配置（含解密后的 API Key），供连通性测试与 AI worker 构造模型。
pub async fn load_provider_credentials(
    pool: &PgPool,
    provider: &str,
) -> Result<(LlmProviderRow, Option<String>), LlmConfigError> {
    ensure_known(provider)?;
    let (base_url, model, ciphertext, enabled, timeout_secs, updated_at): (
        String,
        String,
        Option<Vec<u8>>,
        bool,
        i32,
        DateTime<Utc>,
    ) = sqlx::query_as(
        "SELECT base_url, model, api_key_ciphertext, enabled, timeout_secs, updated_at \
         FROM ai.llm_provider WHERE provider = $1",
    )
    .bind(provider)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| LlmConfigError::UnknownProvider(provider.to_string()))?;

    let api_key = match ciphertext {
        Some(bytes) => Some(decrypt_api_key_bytes(&bytes)?),
        None => None,
    };
    Ok((
        LlmProviderRow {
            provider: provider.to_string(),
            base_url,
            model,
            has_api_key: api_key.is_some(),
            api_key_hint: None,
            enabled,
            timeout_secs,
            updated_at,
        },
        api_key,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hint_keeps_only_last_four_chars() {
        assert_eq!(hint_of("sk-1234567890abcd"), "****abcd");
        assert_eq!(hint_of("abc"), "****abc");
    }
}
