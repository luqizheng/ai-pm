//! Team LLM Provider 统一应用服务（设计稿
//! `docs/ui/smark-pm-team-llm-provider-management-v1.md`，DEC-LLM-001~004）。
//!
//! 资源归属（DEC-LLM-001）：Provider 是 Team-owned Resource，Team ≡ `core.tenant`；
//! 同 Team 可配置多个同类型实例。所有写路径统一走本服务：单事务 + 乐观锁
//! （`revision`，409 冲突）+ `governance.tenant_audit_log` 审计（四列身份口径，
//! 清单见 `audit_actor_coverage`）。
//!
//! 凭证安全（§7）：API Key 明文永不落库、永不回显——AES-256-GCM 密文存
//! `ai.team_llm_credential`（主密钥由 `AI_PM_SECRET_KEY` 派生；KMS/Vault 挂账，
//! 见核验报告 Q-4）；替换 = 新 credential 行 + `credential_id` 重指向（轮换留痕，
//! Team Owner 也不能回读完整密钥）。
//!
//! 故障处理（DEC-LLM-004）：不自动切换 Provider/Model——连通性探测显式返回
//! ok/失败；`anthropic` 适配器未实现时显式报错，绝不用 OpenAI 兼容面硬调。
//! 生命周期（§6.3）：`not_configured → configured → connected|failed → enabled ⇄
//! disabled → archived`；测试成功≠自动启用，enable 必须显式动作且要求已有凭证。

use std::time::{Duration, Instant};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::actor::AuditActor;

/// GCM nonce 长度（96 bit，NIST SP 800-38D 推荐值）。
const NONCE_LEN: usize = 12;

/// 初始 Provider 类型（设计稿 §6.1）。类型清单校验在服务层——扩展类型不再需要
/// DB CHECK / 常量 / 前端联合类型三处同步（核验报告 R-LLM-08）。
pub const PROVIDER_TYPES: [&str; 5] = [
    "openai",
    "anthropic",
    "gemini",
    "deepseek",
    "openai_compatible",
];

/// V1 适配器覆盖面：OpenAI 兼容 `/chat/completions` 面覆盖 openai / deepseek /
/// gemini（其 OpenAI 兼容端点）/ openai_compatible；`anthropic` 的 `/v1/messages`
/// 面未实现——显式报错（DEC-LLM-004），不静默换面硬调。
fn adapter_available(provider_type: &str) -> bool {
    !matches!(provider_type, "anthropic")
}

/// 连接测试用途的调用记录 purpose 值（`ai.llm_call_record.purpose`）。
const PURPOSE_CONNECTION_TEST: &str = "connection_test";

// ─────────────────────────────────────────────────────────── 行与请求

/// 列表/回显行——**永不包含密文字段**（§7.3：只回 `has_credential` 与末 4 位 hint）。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct TeamLlmProviderRow {
    pub id: Uuid,
    pub name: String,
    pub provider_type: String,
    pub endpoint: String,
    pub configuration: Value,
    pub status: String,
    pub timeout_secs: i32,
    pub revision: i64,
    pub has_credential: bool,
    pub credential_hint: Option<String>,
    pub last_tested_at: Option<DateTime<Utc>>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTeamProviderRequest {
    pub name: String,
    pub provider_type: String,
    pub endpoint: String,
    #[serde(default)]
    pub configuration: Option<Value>,
    /// 缺省/空 = 先建实例不配凭证（not_configured）；有值 = 直接 configured。
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub timeout_secs: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateTeamProviderRequest {
    #[serde(default)]
    pub expected_revision: i64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub provider_type: Option<String>,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub configuration: Option<Value>,
    #[serde(default)]
    pub timeout_secs: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReplaceCredentialRequest {
    #[serde(default)]
    pub expected_revision: i64,
    pub api_key: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SetStatusRequest {
    #[serde(default)]
    pub expected_revision: i64,
    /// enable | disable | archive（§6.3 生命周期；enable 必须显式动作）。
    pub action: String,
}

#[derive(Debug, Serialize)]
pub struct TestResult {
    pub ok: bool,
    pub latency_ms: u64,
    pub message: String,
}

// ─────────────────────────────────────────────────────────── 错误

#[derive(Debug, thiserror::Error)]
pub enum TeamLlmError {
    #[error("provider not found")]
    NotFound,
    #[error("{0}")]
    Validation(&'static str),
    #[error("provider type `{0}` has no adapter yet")]
    AdapterUnavailable(String),
    #[error("{0}")]
    StatusTransition(&'static str),
    #[error("provider is referenced by {0} and cannot be deleted")]
    ReferencedBy(&'static str),
    #[error("revision conflict: expected {expected}, actual {actual}")]
    RevisionConflict { expected: i64, actual: i64 },
    #[error("AI_PM_SECRET_KEY is not set")]
    SecretKeyMissing,
    #[error("api key could not be encrypted or decrypted")]
    Crypto,
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

// ─────────────────────────────────────────────────────────── 凭证加密
// （与 0007 时代 `llm_config` 同格式：nonce(12) || ciphertext，hint 只留末 4 位；
// 0007 表退役后这里是唯一实现。）

fn cipher() -> Result<Aes256Gcm, TeamLlmError> {
    let raw = std::env::var("AI_PM_SECRET_KEY").map_err(|_| TeamLlmError::SecretKeyMissing)?;
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(TeamLlmError::SecretKeyMissing);
    }
    // 32-byte key derived from the operator secret (any length input).
    let key: [u8; 32] = Sha256::digest(raw.as_bytes()).into();
    Aes256Gcm::new_from_slice(&key).map_err(|_| TeamLlmError::Crypto)
}

fn hint_of(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    let start = chars.len().saturating_sub(4);
    let tail: String = chars[start..].iter().collect();
    format!("****{tail}")
}

/// AES-256-GCM 加密，返回 `nonce || ciphertext` 与脱敏 hint。
pub fn encrypt_api_key(key: &str) -> Result<(Vec<u8>, String), TeamLlmError> {
    let cipher = cipher()?;
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).map_err(|_| TeamLlmError::Crypto)?;
    let encrypted = cipher
        .encrypt(Nonce::from_slice(&nonce), key.as_bytes())
        .map_err(|_| TeamLlmError::Crypto)?;
    let mut payload = Vec::with_capacity(NONCE_LEN + encrypted.len());
    payload.extend_from_slice(&nonce);
    payload.extend_from_slice(&encrypted);
    Ok((payload, hint_of(key)))
}

pub(crate) fn decrypt_api_key_bytes(payload: &[u8]) -> Result<String, TeamLlmError> {
    if payload.len() <= NONCE_LEN {
        return Err(TeamLlmError::Crypto);
    }
    let (nonce, ciphertext) = payload.split_at(NONCE_LEN);
    let plain = cipher()?
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| TeamLlmError::Crypto)?;
    String::from_utf8(plain).map_err(|_| TeamLlmError::Crypto)
}

// ─────────────────────────────────────────────────────────── 校验

fn validate_endpoint(endpoint: &str) -> Result<String, TeamLlmError> {
    let endpoint = endpoint.trim().trim_end_matches('/').to_string();
    let parsed = url::Url::parse(&endpoint)
        .map_err(|_| TeamLlmError::Validation("endpoint is not a valid URL"))?;
    if !(parsed.scheme() == "http" || parsed.scheme() == "https") {
        return Err(TeamLlmError::Validation(
            "endpoint must start with http:// or https://",
        ));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| TeamLlmError::Validation("endpoint must include a host"))?;
    if !is_public_host(host) {
        // §17 SSRF 防护（2026-11-11 D-3）：仅放行公网地址。字面校验
        // （localhost / 私网 / 回环 / 链路本地 / ULA / 未指定）；DNS 解析后
        // 校验（防 rebinding）随统一 Gateway 挂账。企业内网网关如需豁免
        // 须经独立产品裁决。
        return Err(TeamLlmError::Validation(
            "endpoint host must be a public address; localhost, private, loopback \
             and link-local hosts are blocked",
        ));
    }
    Ok(endpoint)
}

/// Host 字面校验：域名只拦 localhost 及其子域；IP 用地址段判定。
/// url crate 对 IPv6 host 序列化保留方括号（"[::1]"），解析前先剥掉。
fn is_public_host(host: &str) -> bool {
    let lower = host.to_ascii_lowercase();
    if lower == "localhost" || lower.ends_with(".localhost") {
        return false;
    }
    let bare = lower
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(&lower);
    match bare.parse::<std::net::IpAddr>() {
        Ok(ip) => !is_private_ip(ip),
        Err(_) => true,
    }
}

fn is_private_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            // 未指定 / 回环 / 私网(10/8,172.16/12,192.168/16) / 链路本地 /
            // CGNAT(100.64/10)
            v4.is_unspecified()
                || v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || (v4.octets()[0] == 100 && (64..=127).contains(&v4.octets()[1]))
        }
        std::net::IpAddr::V6(v6) => {
            // IPv4-mapped 私网地址换回 v4 判定（::ffff:10.0.0.1 不得绕过）
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_private_ip(std::net::IpAddr::V4(v4));
            }
            let s = v6.segments();
            v6.is_unspecified()
                || v6.is_loopback()
                || (s[0] & 0xfe00) == 0xfc00 // fc00::/7 ULA
                || (s[0] & 0xffc0) == 0xfe80 // fe80::/10 链路本地
        }
    }
}

fn validate_provider_type(provider_type: &str) -> Result<(), TeamLlmError> {
    if PROVIDER_TYPES.contains(&provider_type) {
        Ok(())
    } else {
        Err(TeamLlmError::Validation("unknown provider_type"))
    }
}

fn validate_timeout(timeout_secs: i32) -> Result<(), TeamLlmError> {
    if (1..=600).contains(&timeout_secs) {
        Ok(())
    } else {
        Err(TeamLlmError::Validation(
            "timeout_secs must be between 1 and 600",
        ))
    }
}

fn validate_configuration(configuration: &Value) -> Result<(), TeamLlmError> {
    if !configuration.is_object() {
        return Err(TeamLlmError::Validation("configuration must be an object"));
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────── 审计

/// 租户审计写入口（单 INSERT；四列身份口径见 `audit_actor_coverage` 清单）。
/// Provider 等 Team 资源事件的目标不是 principal，走 0041 新增的
/// `target_type`/`target_id` 通用列；审计失败即整体回滚。
async fn insert_tenant_audit(
    tx: &mut Transaction<'static, Postgres>,
    tenant_id: Uuid,
    actor: &AuditActor,
    action: &str,
    target_id: Uuid,
    before: Option<Value>,
    after: Option<Value>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO governance.tenant_audit_log \
         (tenant_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
          action, target_type, target_id, target_principal_id, before_data, after_data) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'llm_provider', $8, NULL, $9, $10)",
    )
    .bind(tenant_id)
    .bind(actor.principal_id)
    .bind(actor.identity_id)
    .bind(&actor.client)
    .bind(&actor.auth_mode)
    .bind(actor.claimed_device_id)
    .bind(action)
    .bind(target_id)
    .bind(before)
    .bind(after)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

// ─────────────────────────────────────────────────────────── 读取

const PROVIDER_SELECT: &str = "SELECT p.id, p.name, p.provider_type, p.endpoint, p.configuration, \
     p.status, p.timeout_secs, p.revision, (p.credential_id IS NOT NULL) AS has_credential, \
     c.api_key_hint AS credential_hint, p.last_tested_at, p.created_by, p.created_at, p.updated_at \
     FROM ai.team_llm_provider p \
     LEFT JOIN ai.team_llm_credential c ON c.id = p.credential_id";

/// 读单个 Provider（租户作用域）。
async fn get_team_provider(
    executor: impl sqlx::PgExecutor<'_>,
    tenant_id: Uuid,
    provider_id: Uuid,
) -> Result<Option<TeamLlmProviderRow>, sqlx::Error> {
    let sql = format!("{PROVIDER_SELECT} WHERE p.tenant_id = $1 AND p.id = $2");
    sqlx::query_as::<_, TeamLlmProviderRow>(&sql)
        .bind(tenant_id)
        .bind(provider_id)
        .fetch_optional(executor)
        .await
}

/// 列出 Team 的全部 Provider 实例（创建序）。
pub async fn list_team_providers(
    executor: impl sqlx::PgExecutor<'_>,
    tenant_id: Uuid,
) -> Result<Vec<TeamLlmProviderRow>, sqlx::Error> {
    let sql = format!("{PROVIDER_SELECT} WHERE p.tenant_id = $1 ORDER BY p.created_at, p.id");
    sqlx::query_as::<_, TeamLlmProviderRow>(&sql)
        .bind(tenant_id)
        .fetch_all(executor)
        .await
}

fn audit_snapshot(row: &TeamLlmProviderRow) -> Value {
    serde_json::to_value(row).unwrap_or(Value::Null)
}

/// 区分「行不存在」（404）与「revision 过期」（409）。
async fn not_found_or_conflict(
    executor: impl sqlx::PgExecutor<'_>,
    tenant_id: Uuid,
    provider_id: Uuid,
    expected_revision: i64,
) -> TeamLlmError {
    let actual: Option<i64> = sqlx::query_scalar(
        "SELECT revision FROM ai.team_llm_provider WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(provider_id)
    .fetch_optional(executor)
    .await
    .ok()
    .flatten();
    match actual {
        None => TeamLlmError::NotFound,
        Some(actual) => TeamLlmError::RevisionConflict {
            expected: expected_revision,
            actual,
        },
    }
}

// ─────────────────────────────────────────────────────────── 创建

/// 创建 Provider 实例（§6.2）。带 `api_key` 直接 configured，否则 not_configured。
pub async fn create_team_provider(
    pool: &PgPool,
    actor: &AuditActor,
    tenant_id: Uuid,
    req: CreateTeamProviderRequest,
) -> Result<TeamLlmProviderRow, TeamLlmError> {
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return Err(TeamLlmError::Validation("name is required"));
    }
    validate_provider_type(&req.provider_type)?;
    let endpoint = validate_endpoint(&req.endpoint)?;
    let configuration = req.configuration.unwrap_or_else(|| json!({}));
    validate_configuration(&configuration)?;
    let timeout_secs = req.timeout_secs.unwrap_or(120);
    validate_timeout(timeout_secs)?;
    let api_key = req
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string);
    let status = if api_key.is_some() {
        "configured"
    } else {
        "not_configured"
    };

    let mut tx = pool.begin().await?;

    // 先 provider（credential_id 暂空），有 key 再插 credential 并回指——
    // credential.provider_id 有组合 FK，必须 provider 行先落库。
    let provider_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.team_llm_provider \
         (tenant_id, name, provider_type, endpoint, configuration, status, timeout_secs, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
    )
    .bind(tenant_id)
    .bind(&name)
    .bind(&req.provider_type)
    .bind(&endpoint)
    .bind(&configuration)
    .bind(status)
    .bind(timeout_secs)
    .bind(actor.principal_id)
    .fetch_one(&mut *tx)
    .await?;

    if let Some(key) = &api_key {
        attach_credential(&mut tx, tenant_id, provider_id, actor, key, None).await?;
    }

    let row = get_team_provider(&mut *tx, tenant_id, provider_id)
        .await?
        .ok_or(TeamLlmError::NotFound)?;

    insert_tenant_audit(
        &mut tx,
        tenant_id,
        actor,
        "llm_provider.created",
        provider_id,
        None,
        Some(audit_snapshot(&row)),
    )
    .await?;

    tx.commit().await?;
    Ok(row)
}

/// 插入新凭证行并重指 `provider.credential_id`、状态推到 `configured`、
/// revision+1（替换制轮换：旧行留痕）。archived 实例拒绝改凭证。
/// `expected_revision`：Some 时 CAS（替换路径），None 用于创建路径（新行）。
async fn attach_credential(
    tx: &mut Transaction<'static, Postgres>,
    tenant_id: Uuid,
    provider_id: Uuid,
    actor: &AuditActor,
    api_key: &str,
    expected_revision: Option<i64>,
) -> Result<(), TeamLlmError> {
    let (bytes, hint) = encrypt_api_key(api_key)?;
    let credential_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.team_llm_credential \
         (tenant_id, provider_id, api_key_ciphertext, api_key_hint, created_by) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(tenant_id)
    .bind(provider_id)
    .bind(&bytes)
    .bind(&hint)
    .bind(actor.principal_id)
    .fetch_one(&mut **tx)
    .await?;

    // archived 一律拒绝（生命周期终态）；其余状态统一回 configured——
    // 新凭证未经连通性验证，重新走 test → enable 显式链。
    let updated = match expected_revision {
        Some(expected) => {
            let updated = sqlx::query(
                "UPDATE ai.team_llm_provider \
                 SET credential_id = $3, status = 'configured', revision = revision + 1, updated_at = now() \
                 WHERE tenant_id = $1 AND id = $2 AND revision = $4 AND status <> 'archived'",
            )
            .bind(tenant_id)
            .bind(provider_id)
            .bind(credential_id)
            .bind(expected)
            .execute(&mut **tx)
            .await?;
            if updated.rows_affected() == 0 {
                return Err(
                    not_found_or_conflict(&mut **tx, tenant_id, provider_id, expected).await,
                );
            }
            updated
        }
        None => {
            // 创建路径：provider 行刚插入，直接回指（无 CAS 对手）。
            sqlx::query(
                "UPDATE ai.team_llm_provider \
                 SET credential_id = $3, status = 'configured', revision = revision + 1, updated_at = now() \
                 WHERE tenant_id = $1 AND id = $2",
            )
            .bind(tenant_id)
            .bind(provider_id)
            .bind(credential_id)
            .execute(&mut **tx)
            .await?
        }
    };
    let _ = updated;
    Ok(())
}

// ─────────────────────────────────────────────────────────── 更新

/// 编辑实例元数据（§12：Team Owner/Admin）。CAS：`expected_revision` 不符即 409。
pub async fn update_team_provider(
    pool: &PgPool,
    actor: &AuditActor,
    tenant_id: Uuid,
    provider_id: Uuid,
    req: UpdateTeamProviderRequest,
) -> Result<TeamLlmProviderRow, TeamLlmError> {
    if let Some(name) = req.name.as_deref() {
        if name.trim().is_empty() {
            return Err(TeamLlmError::Validation("name is required"));
        }
    }
    if let Some(provider_type) = &req.provider_type {
        validate_provider_type(provider_type)?;
    }
    let endpoint = match req.endpoint.as_deref() {
        Some(endpoint) => Some(validate_endpoint(endpoint)?),
        None => None,
    };
    if let Some(configuration) = &req.configuration {
        validate_configuration(configuration)?;
    }
    if let Some(timeout_secs) = req.timeout_secs {
        validate_timeout(timeout_secs)?;
    }

    let mut tx = pool.begin().await?;

    let updated = sqlx::query(
        "UPDATE ai.team_llm_provider SET \
             name = COALESCE($3, name), \
             provider_type = COALESCE($4, provider_type), \
             endpoint = COALESCE($5, endpoint), \
             configuration = COALESCE($6, configuration), \
             timeout_secs = COALESCE($7, timeout_secs), \
             revision = revision + 1, updated_at = now() \
         WHERE tenant_id = $1 AND id = $2 AND revision = $8 AND status <> 'archived'",
    )
    .bind(tenant_id)
    .bind(provider_id)
    .bind(req.name.as_deref().map(str::trim))
    .bind(&req.provider_type)
    .bind(&endpoint)
    .bind(&req.configuration)
    .bind(req.timeout_secs)
    .bind(req.expected_revision)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(
            not_found_or_conflict(&mut *tx, tenant_id, provider_id, req.expected_revision).await,
        );
    }

    let row = get_team_provider(&mut *tx, tenant_id, provider_id)
        .await?
        .ok_or(TeamLlmError::NotFound)?;
    insert_tenant_audit(
        &mut tx,
        tenant_id,
        actor,
        "llm_provider.updated",
        provider_id,
        None,
        Some(audit_snapshot(&row)),
    )
    .await?;

    tx.commit().await?;
    Ok(row)
}

// ─────────────────────────────────────────────────────────── 凭证替换

/// 替换凭证（§7.4 替换机制 / Credential Rotated）。新凭证未经验证，状态回
/// `configured`，需要重新 test → enable。
pub async fn replace_team_credential(
    pool: &PgPool,
    actor: &AuditActor,
    tenant_id: Uuid,
    provider_id: Uuid,
    req: ReplaceCredentialRequest,
) -> Result<TeamLlmProviderRow, TeamLlmError> {
    let api_key = req.api_key.trim().to_string();
    if api_key.is_empty() {
        return Err(TeamLlmError::Validation("api_key is required"));
    }

    let mut tx = pool.begin().await?;

    // 先做 CAS 预检（防止对不存在/已过期的行做凭证插入），复用 revision 条件。
    let current = get_team_provider(&mut *tx, tenant_id, provider_id)
        .await?
        .ok_or(TeamLlmError::NotFound)?;
    if current.revision != req.expected_revision {
        return Err(TeamLlmError::RevisionConflict {
            expected: req.expected_revision,
            actual: current.revision,
        });
    }
    if current.status == "archived" {
        return Err(TeamLlmError::StatusTransition(
            "archived provider cannot change credential",
        ));
    }

    attach_credential(
        &mut tx,
        tenant_id,
        provider_id,
        actor,
        &api_key,
        Some(req.expected_revision),
    )
    .await?;

    let row = get_team_provider(&mut *tx, tenant_id, provider_id)
        .await?
        .ok_or(TeamLlmError::NotFound)?;
    insert_tenant_audit(
        &mut tx,
        tenant_id,
        actor,
        "llm_provider.credential_rotated",
        provider_id,
        None,
        Some(audit_snapshot(&row)),
    )
    .await?;

    tx.commit().await?;
    Ok(row)
}

// ─────────────────────────────────────────────────────────── 连接测试

/// 连接测试（§16 test 端点）：服务端发起 `/chat/completions` 探活（"ping"，
/// max_tokens 16，超时 min(timeout, 30s)），返回 `{ok, latency_ms, message}`，
/// 不回显任何密钥信息。结果落 `ai.llm_call_record`（purpose=connection_test）并
/// 更新 `last_tested_at`；pre-enable 状态下同步推进 `connected|failed`——
/// **不自动启用**（§6.3）。archived 拒测；无凭证/无模型显式报错（§11
/// Configuration Error），anthropic 显式 AdapterUnavailable。
pub async fn test_team_provider(
    pool: &PgPool,
    actor: &AuditActor,
    tenant_id: Uuid,
    provider_id: Uuid,
) -> Result<TestResult, TeamLlmError> {
    let mut tx = pool.begin().await?;

    let row = get_team_provider(&mut *tx, tenant_id, provider_id)
        .await?
        .ok_or(TeamLlmError::NotFound)?;
    if row.status == "archived" {
        return Err(TeamLlmError::StatusTransition(
            "archived provider cannot be tested",
        ));
    }
    if !adapter_available(&row.provider_type) {
        return Err(TeamLlmError::AdapterUnavailable(row.provider_type.clone()));
    }
    let model = row
        .configuration
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .ok_or(TeamLlmError::Validation("model is required before testing"))?;

    // 取凭证（可选）：`openai_compatible`（Ollama 等本地端点）允许匿名探测；
    // 其余类型无凭证 = Configuration Error（显式，不静默）。
    let ciphertext: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT api_key_ciphertext FROM ai.team_llm_credential WHERE id = \
         (SELECT credential_id FROM ai.team_llm_provider WHERE tenant_id = $1 AND id = $2)",
    )
    .bind(tenant_id)
    .bind(provider_id)
    .fetch_optional(&mut *tx)
    .await?
    .flatten();
    let api_key = match ciphertext {
        Some(bytes) => Some(decrypt_api_key_bytes(&bytes)?),
        None if row.provider_type == "openai_compatible" => None,
        None => {
            return Err(TeamLlmError::Validation(
                "credential is required before testing",
            ));
        }
    };

    let result =
        probe_chat_completions(&row.endpoint, model, api_key.as_deref(), row.timeout_secs).await;

    // 落调用记录（ok/failed 都记——DEC-LLM-004.7 失败必须保留执行记录）。
    let (status, error_kind) = if result.ok {
        ("ok", None)
    } else {
        ("failed", Some(classify_probe_message(&result.message)))
    };
    sqlx::query(
        "INSERT INTO ai.llm_call_record \
         (tenant_id, provider_id, model, purpose, status, error_kind, error_message, latency_ms) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(tenant_id)
    .bind(provider_id)
    .bind(model)
    .bind(PURPOSE_CONNECTION_TEST)
    .bind(status)
    .bind(error_kind)
    .bind(if result.ok {
        None
    } else {
        Some(result.message.clone())
    })
    .bind(i64::try_from(result.latency_ms).unwrap_or(i64::MAX))
    .execute(&mut *tx)
    .await?;

    // pre-enable 状态（not_configured/configured/connected/failed）随测试推进；
    // enabled/disabled 保持运营状态，仅记 last_tested_at。
    sqlx::query(
        "UPDATE ai.team_llm_provider SET \
             last_tested_at = now(), \
             status = CASE \
                 WHEN status IN ('not_configured', 'configured', 'connected', 'failed') \
                     THEN $3 ELSE status END, \
             updated_at = now() \
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(provider_id)
    .bind(if result.ok { "connected" } else { "failed" })
    .execute(&mut *tx)
    .await?;

    let after = json!({
        "ok": result.ok,
        "latency_ms": result.latency_ms,
        "message": result.message,
    });
    insert_tenant_audit(
        &mut tx,
        tenant_id,
        actor,
        "llm_provider.tested",
        provider_id,
        None,
        Some(after),
    )
    .await?;

    tx.commit().await?;
    Ok(result)
}

/// `/chat/completions` 探活（原 pm-api llm_configs 实现收敛入服务层——统一
/// Gateway 前的第三处重复请求构造归一，见核验报告 R-LLM-03）。
async fn probe_chat_completions(
    endpoint: &str,
    model: &str,
    api_key: Option<&str>,
    timeout_secs: i32,
) -> TestResult {
    // Bound the probe: a misconfigured endpoint must not hold the request open
    // for the full production timeout.
    let timeout = Duration::from_secs(timeout_secs.min(30) as u64);
    let client = match reqwest::Client::builder()
        .no_proxy()
        .timeout(timeout)
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            tracing::error!(error = %err, "llm test client build failed");
            return TestResult {
                ok: false,
                latency_ms: 0,
                message: "internal error".to_string(),
            };
        }
    };

    let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));
    let payload = json!({
        "model": model,
        "messages": [{ "role": "user", "content": "ping" }],
        "max_tokens": 16,
        "stream": false
    });
    let mut request = client.post(&url).json(&payload);
    if let Some(key) = api_key.filter(|key| !key.is_empty()) {
        request = request.bearer_auth(key);
    }

    let started = Instant::now();
    match request.send().await {
        Ok(response) => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            let latency = started.elapsed().as_millis() as u64;
            if status.is_success() {
                TestResult {
                    ok: true,
                    latency_ms: latency,
                    message: format!("HTTP {}", status.as_u16()),
                }
            } else {
                TestResult {
                    ok: false,
                    latency_ms: latency,
                    message: format!("HTTP {}: {}", status.as_u16(), brief(&body)),
                }
            }
        }
        Err(err) => TestResult {
            ok: false,
            latency_ms: started.elapsed().as_millis() as u64,
            message: if err.is_timeout() {
                "request timed out".to_string()
            } else {
                brief(&err.to_string())
            },
        },
    }
}

/// 截断上游/传输层错误，避免把整段响应体（可能含不可信模型输出）回显给前端。
fn brief(text: &str) -> String {
    text.chars().take(200).collect()
}

/// 探测结果 → §11 错误分类（仅用于 llm_call_record.error_kind）。
fn classify_probe_message(message: &str) -> &'static str {
    if message == "request timed out" {
        return "timeout";
    }
    if let Some(rest) = message.strip_prefix("HTTP ") {
        let code: u16 = rest
            .split(':')
            .next()
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        return match code {
            401 | 403 => "authentication",
            429 => "rate_limit",
            404 => "model_unavailable",
            _ => "provider_error",
        };
    }
    "connection"
}

// ─────────────────────────────────────────────────────────── 生命周期

/// 生命周期动作（§6.3；enable 必须显式动作且要求已有凭证——测试成功≠自动启用）。
pub async fn set_team_provider_status(
    pool: &PgPool,
    actor: &AuditActor,
    tenant_id: Uuid,
    provider_id: Uuid,
    req: SetStatusRequest,
) -> Result<TeamLlmProviderRow, TeamLlmError> {
    let action = req.action.as_str();
    if !matches!(action, "enable" | "disable" | "archive") {
        return Err(TeamLlmError::Validation(
            "action must be enable|disable|archive",
        ));
    }

    let mut tx = pool.begin().await?;

    let current = get_team_provider(&mut *tx, tenant_id, provider_id)
        .await?
        .ok_or(TeamLlmError::NotFound)?;
    if current.revision != req.expected_revision {
        return Err(TeamLlmError::RevisionConflict {
            expected: req.expected_revision,
            actual: current.revision,
        });
    }
    if current.status == "archived" {
        return Err(TeamLlmError::StatusTransition(
            "archived provider cannot change status",
        ));
    }

    match action {
        "enable" => {
            if current.status == "enabled" {
                return Ok(current); // 幂等
            }
            // 凭证门禁：`openai_compatible`（本地端点，无鉴权）豁免。
            if !current.has_credential && current.provider_type != "openai_compatible" {
                return Err(TeamLlmError::Validation(
                    "credential is required before enabling",
                ));
            }
        }
        "disable" => {
            if current.status == "disabled" {
                return Ok(current); // 幂等
            }
            if current.status != "enabled" {
                return Err(TeamLlmError::StatusTransition(
                    "only an enabled provider can be disabled",
                ));
            }
        }
        _ => {} // archive：任何非终态都可归档（归档后解析/claim 都不再选中）
    }

    let next = match action {
        "enable" => "enabled",
        "disable" => "disabled",
        _ => "archived",
    };
    let updated = sqlx::query(
        "UPDATE ai.team_llm_provider SET status = $3, revision = revision + 1, updated_at = now() \
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(provider_id)
    .bind(next)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(TeamLlmError::NotFound);
    }

    let row = get_team_provider(&mut *tx, tenant_id, provider_id)
        .await?
        .ok_or(TeamLlmError::NotFound)?;
    let audit_action = format!("llm_provider.{action}d"); // enabled / disabled / archived
    insert_tenant_audit(
        &mut tx,
        tenant_id,
        actor,
        &audit_action,
        provider_id,
        None,
        Some(audit_snapshot(&row)),
    )
    .await?;

    tx.commit().await?;
    Ok(row)
}

/// 删除实例（§14.4 删除保护）：必须先 archive（终态确认），且被
/// `ai.llm_call_record` 引用（含连接测试记录）即拒绝——执行记录永久保留
/// （DEC-LLM-004.7）。删除后凭证行级联清除。
pub async fn delete_team_provider(
    pool: &PgPool,
    actor: &AuditActor,
    tenant_id: Uuid,
    provider_id: Uuid,
) -> Result<(), TeamLlmError> {
    let mut tx = pool.begin().await?;

    let current = get_team_provider(&mut *tx, tenant_id, provider_id)
        .await?
        .ok_or(TeamLlmError::NotFound)?;
    if current.status != "archived" {
        return Err(TeamLlmError::StatusTransition(
            "provider must be archived before deletion",
        ));
    }
    let references: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ai.llm_call_record WHERE provider_id = $1")
            .bind(provider_id)
            .fetch_one(&mut *tx)
            .await?;
    if references > 0 {
        return Err(TeamLlmError::ReferencedBy("llm_call_record"));
    }

    insert_tenant_audit(
        &mut tx,
        tenant_id,
        actor,
        "llm_provider.deleted",
        provider_id,
        Some(audit_snapshot(&current)),
        None,
    )
    .await?;

    sqlx::query("DELETE FROM ai.team_llm_provider WHERE tenant_id = $1 AND id = $2")
        .bind(tenant_id)
        .bind(provider_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

// ─────────────────────────────────────────────────────────── worker 消费

// ─────────────────────────────────────────────────────────── 项目策略（§10.2）

/// 项目 AI 模型策略行（V1：项目 → 指定一个 Team Provider 实例）。
#[derive(Debug, Clone, Serialize)]
pub struct ProjectAiPolicyRow {
    pub project_id: Uuid,
    pub provider_id: Option<Uuid>,
    pub provider_name: Option<String>,
    pub provider_status: Option<String>,
    pub revision: i64,
    pub updated_at: Option<DateTime<Utc>>,
}

/// 读项目策略（无指定时 provider 字段为 NULL）。
pub async fn get_project_ai_policy(
    executor: impl sqlx::PgExecutor<'_>,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<ProjectAiPolicyRow, TeamLlmError> {
    type Row = Option<(
        Option<Uuid>,
        Option<String>,
        Option<String>,
        i64,
        Option<DateTime<Utc>>,
    )>;
    let row: Row =
        sqlx::query_as(
            "SELECT pol.provider_id, p.name, p.status, COALESCE(pol.revision, 1) AS revision, pol.updated_at \
             FROM (SELECT $1::uuid AS tenant_id, $2::uuid AS project_id) base \
             LEFT JOIN ai.project_ai_model_policy pol \
               ON pol.tenant_id = base.tenant_id AND pol.project_id = base.project_id \
             LEFT JOIN ai.team_llm_provider p ON p.id = pol.provider_id",
        )
        .bind(tenant_id)
        .bind(project_id)
        .fetch_optional(executor)
        .await?;
    // base 恒一行，LEFT JOIN 保行；无策略行时 revision 由 COALESCE 落到虚值 1
    //（与 set_project_ai_policy 的 CAS「无行 = 1」同一口径）。
    let (provider_id, provider_name, provider_status, revision, updated_at) =
        row.unwrap_or((None, None, None, 1, None));
    Ok(ProjectAiPolicyRow {
        project_id,
        provider_id,
        provider_name,
        provider_status,
        revision,
        updated_at,
    })
}

/// 设置/更新项目策略（§12：Team Owner/Admin 或 Project Admin=project owner；
/// 归属与权限校验在 REST 层）。校验：Provider 必须同租户且存在；允许保存
/// 非 enabled 的指定——解析时严格不回退（§10.3）。
pub async fn set_project_ai_policy(
    pool: &PgPool,
    actor: &AuditActor,
    tenant_id: Uuid,
    project_id: Uuid,
    provider_id: Option<Uuid>,
    expected_revision: i64,
) -> Result<ProjectAiPolicyRow, TeamLlmError> {
    let mut tx = pool.begin().await?;

    // 项目必须属于本租户（组合 FK 兜底，这里给可读错误）。
    let project_tenant: Option<Uuid> =
        sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
            .bind(project_id)
            .fetch_optional(&mut *tx)
            .await?;
    if project_tenant != Some(tenant_id) {
        return Err(TeamLlmError::NotFound);
    }

    // CAS：当前 revision（无行 = 1）
    let current: Option<(i64,)> = sqlx::query_as(
        "SELECT revision FROM ai.project_ai_model_policy \
         WHERE tenant_id = $1 AND project_id = $2",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await?;
    let current_rev = current.map(|r| r.0).unwrap_or(1);
    if current_rev != expected_revision {
        return Err(TeamLlmError::RevisionConflict {
            expected: expected_revision,
            actual: current_rev,
        });
    }

    if let Some(pid) = provider_id {
        let ok: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM ai.team_llm_provider WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(pid)
        .fetch_optional(&mut *tx)
        .await?;
        if ok.is_none() {
            return Err(TeamLlmError::NotFound);
        }
    }

    match provider_id {
        Some(pid) => {
            sqlx::query(
                "INSERT INTO ai.project_ai_model_policy \
                 (tenant_id, project_id, provider_id, revision, updated_by) \
                 VALUES ($1, $2, $3, 2, $4) \
                 ON CONFLICT (tenant_id, project_id) DO UPDATE SET \
                   provider_id = EXCLUDED.provider_id, \
                   revision = ai.project_ai_model_policy.revision + 1, \
                   updated_by = EXCLUDED.updated_by, updated_at = now()",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(pid)
            .bind(actor.principal_id)
            .execute(&mut *tx)
            .await?;
        }
        None => {
            // 恢复继承 Team Default = 删除指定行
            sqlx::query(
                "DELETE FROM ai.project_ai_model_policy \
                 WHERE tenant_id = $1 AND project_id = $2",
            )
            .bind(tenant_id)
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
        }
    }

    let row = get_project_ai_policy(&mut *tx, tenant_id, project_id).await?;

    insert_tenant_audit(
        &mut tx,
        tenant_id,
        actor,
        "llm_provider.project_policy_changed",
        provider_id.unwrap_or(Uuid::nil()),
        None,
        Some(json!({
            "project_id": project_id,
            "provider_id": provider_id,
        })),
    )
    .await?;

    tx.commit().await?;
    Ok(row)
}

/// 解析后的调用配置（worker/分析链路径构造模型用；明文 Key 只在
/// 后端内存流转，绝不回显）。
#[derive(Debug, Clone)]
pub struct ResolvedProvider {
    pub provider_id: Uuid,
    pub endpoint: String,
    pub model: String,
    pub api_key: String,
    pub timeout_secs: i32,
    /// 解析出的目录模型行（Phase 3B；记账归因 ai.llm_call_record.model_ref
    /// 接线在 3C）。过渡解析链产出的实例为 None。
    pub model_ref: Option<Uuid>,
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hint_keeps_only_last_four_chars() {
        assert_eq!(hint_of("sk-1234567890abcd"), "****abcd");
        assert_eq!(hint_of("abc"), "****abc");
    }

    /// §17 SSRF 防护：公网放行，localhost/私网/回环/链路本地/ULA 拒绝。
    #[test]
    fn endpoint_blocks_private_hosts() {
        assert_eq!(
            validate_endpoint("https://api.openai.com/v1").unwrap(),
            "https://api.openai.com/v1"
        );
        assert_eq!(
            validate_endpoint("http://203.0.113.1:9/v1").unwrap(),
            "http://203.0.113.1:9/v1"
        );
        for bad in [
            "http://localhost:11434/v1",
            "http://foo.localhost/v1",
            "http://127.0.0.1:9/v1",
            "http://10.1.2.3/v1",
            "http://192.168.1.1/v1",
            "http://172.16.0.9/v1",
            "http://169.254.10.10/v1",
            "http://0.0.0.0/v1",
            "http://100.64.0.1/v1",
            "http://[::1]:8080/v1",
            "http://[fe80::1]/v1",
            "http://[fd00::1]/v1",
            "http://[::ffff:10.0.0.1]/v1",
            "ftp://api.openai.com",
            "not-a-url",
        ] {
            assert!(validate_endpoint(bad).is_err(), "must block {bad}");
        }
    }

    #[test]
    fn known_provider_types_match_design_doc() {
        assert_eq!(
            PROVIDER_TYPES,
            [
                "openai",
                "anthropic",
                "gemini",
                "deepseek",
                "openai_compatible"
            ]
        );
        // anthropic 适配器未实现必须显式暴露（DEC-LLM-004）。
        assert!(!adapter_available("anthropic"));
        assert!(adapter_available("openai"));
        assert!(adapter_available("gemini"));
        assert!(adapter_available("deepseek"));
        assert!(adapter_available("openai_compatible"));
    }

    #[test]
    fn probe_error_classification() {
        assert_eq!(classify_probe_message("request timed out"), "timeout");
        assert_eq!(
            classify_probe_message("HTTP 401: unauthorized"),
            "authentication"
        );
        assert_eq!(classify_probe_message("HTTP 429: slow down"), "rate_limit");
        assert_eq!(
            classify_probe_message("HTTP 404: no model"),
            "model_unavailable"
        );
        assert_eq!(classify_probe_message("HTTP 500: boom"), "provider_error");
        assert_eq!(
            classify_probe_message("error sending request"),
            "connection"
        );
    }
}
