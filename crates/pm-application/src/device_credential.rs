//! V-05-1 Client Registry · 设备凭据签发/撤销/解析（0049）。
//!
//! 设计：`docs/project-ai-team-v2/ai-pm-v05-client-registry-design-v1.md` V1.0
//! （联合裁决 2026-11-07 通过 + 工程约束 A/B/C）。
//!
//! **约束 A（安全底线）**：凭据校验不得退化为仅查 Hash 存在——[`resolve`]
//! 同时验证 ① key_hash 命中 ② device.status='active' ③ tenant 一致（owner
//! principal 同租户且 active）④ credential_revoked_at IS NULL；owner 的项目级
//! 成员资格由工具层 `ensure_member` 继续承担（V-6 边界不变）。
//! **cpk 认证失败一律失败（无 fallback）**——调用方（pm-api 鉴权层）不得回退
//! 真人 JWT 路径。
//!
//! **约束 C**：本模块不做任何缓存；每次解析直查库（撤销即时生效），撤销后
//! 不得轮询/续领/执行命令/经既有连接绕过。
//!
//! 凭据形式（Q-A = 方案 B）：`cpk_` + 64 hex（32 字节随机，256bit 熵）；
//! 落库仅 sha256 小写 hex；明文只在签发响应返回一次。

use sqlx::{PgPool, Row, Transaction};
use uuid::Uuid;

use crate::actor::AuditActor;

/// 凭据前缀（鉴权层按此前缀路由到 Client 通道；与 JWT 形态天然可区分）。
pub const CREDENTIAL_PREFIX: &str = "cpk_";

/// V1 已知权限码全集（granted_permissions 裁剪值域；lmms 字典落地后对齐）。
pub const KNOWN_PERMISSIONS: [&str; 3] = ["PM_MCP_READ", "PM_MCP_AGENT_REPORT", "PM_MCP_COMMAND"];

#[derive(Debug, thiserror::Error)]
pub enum DeviceCredentialError {
    #[error("database error: {0}")]
    Db(String),
    #[error("device not found")]
    NotFound,
    /// 已有未撤销凭据（先 revoke 再 issue；轮换两步制）。
    #[error("an active credential already exists (revoke it first)")]
    ActiveCredentialExists,
    #[error("{0}")]
    Validation(String),
}

/// 解析结果（鉴权层据此构造 AuthedPrincipal + ClientChannel）。
#[derive(Debug, Clone)]
pub struct ResolvedClientCredential {
    pub device_id: Uuid,
    pub tenant_id: Uuid,
    pub owner_principal_id: Uuid,
    pub permissions: Vec<String>,
}

/// 签发结果：明文**只在此处出现一次**；落库仅 sha256。
#[derive(Debug)]
pub struct IssuedCredential {
    pub plaintext: String,
    pub device_id: Uuid,
    pub issued_at: chrono::DateTime<chrono::Utc>,
}

/// 生成 `cpk_` + 64 hex（复用 `generate_lease_token` 的 getrandom 32 字节管线；
/// 256bit 熵；不引入新依赖）。
fn generate_plaintext() -> Result<String, DeviceCredentialError> {
    let token = crate::run_context::generate_lease_token()
        .map_err(|e| DeviceCredentialError::Db(e.to_string()))?;
    Ok(format!("{CREDENTIAL_PREFIX}{token}"))
}

fn hash_key(plaintext: &str) -> String {
    crate::run_context::hash_lease_token(plaintext)
}

/// 校验 granted_permissions 裁剪值（非空 + 全部在已知码域内）。
pub fn validate_permissions(perms: &[String]) -> Result<(), DeviceCredentialError> {
    if perms.is_empty() {
        return Err(DeviceCredentialError::Validation(
            "granted_permissions must not be empty".into(),
        ));
    }
    for p in perms {
        if !KNOWN_PERMISSIONS.contains(&p.as_str()) {
            return Err(DeviceCredentialError::Validation(format!(
                "unknown permission code `{p}` (expected one of {KNOWN_PERMISSIONS:?})"
            )));
        }
    }
    Ok(())
}

/// 签发（或轮换第一步）：生成明文 + hash 落库。已有未撤销凭据 → 409。
pub async fn issue_device_credential(
    pool: &PgPool,
    tenant_id: Uuid,
    device_id: Uuid,
    actor: &AuditActor,
) -> Result<IssuedCredential, DeviceCredentialError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| DeviceCredentialError::Db(e.to_string()))?;
    let row = sqlx::query(
        "SELECT tenant_id, credential_key_hash, credential_revoked_at \
           FROM devenv.device WHERE id = $1 FOR UPDATE",
    )
    .bind(device_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DeviceCredentialError::Db(e.to_string()))?
    .ok_or(DeviceCredentialError::NotFound)?;
    let device_tenant: Uuid = row.get("tenant_id");
    if device_tenant != tenant_id {
        return Err(DeviceCredentialError::NotFound);
    }
    let existing_hash: Option<String> = row.get("credential_key_hash");
    let revoked_at: Option<chrono::DateTime<chrono::Utc>> = row.get("credential_revoked_at");
    if existing_hash.is_some() && revoked_at.is_none() {
        return Err(DeviceCredentialError::ActiveCredentialExists);
    }

    let plaintext = generate_plaintext()?;
    let issued = sqlx::query(
        "UPDATE devenv.device \
            SET credential_key_hash = $2, credential_issued_at = now(), \
                credential_revoked_at = NULL, updated_at = now() \
          WHERE id = $1 \
         RETURNING credential_issued_at",
    )
    .bind(device_id)
    .bind(hash_key(&plaintext))
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DeviceCredentialError::Db(e.to_string()))?;
    let issued_at: chrono::DateTime<chrono::Utc> = issued.get("credential_issued_at");

    // 设备是租户级实体（无 project 归属）——审计走 governance.tenant_audit_log
    // （team_llm 同款载体；四身份列全写，C180/C174 盘点登记本文件 3 处 INSERT）。
    insert_tenant_audit(
        &mut tx,
        tenant_id,
        actor,
        "device.credential.issued",
        device_id,
        serde_json::json!({
            "device_id": device_id,
            "key_hash_prefix": hash_key(&plaintext).get(..12).unwrap_or("").to_string(),
        }),
    )
    .await?;
    tx.commit()
        .await
        .map_err(|e| DeviceCredentialError::Db(e.to_string()))?;
    Ok(IssuedCredential {
        plaintext,
        device_id,
        issued_at,
    })
}

/// 租户级审计（governance.tenant_audit_log；四身份列全写——C174 守卫）。
async fn insert_tenant_audit(
    tx: &mut Transaction<'static, sqlx::Postgres>,
    tenant_id: Uuid,
    actor: &AuditActor,
    action: &str,
    target_id: Uuid,
    after: serde_json::Value,
) -> Result<(), DeviceCredentialError> {
    sqlx::query(
        "INSERT INTO governance.tenant_audit_log \
         (tenant_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
          action, target_type, target_id, target_principal_id, before_data, after_data) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'device', $8, NULL, NULL, $9)",
    )
    .bind(tenant_id)
    .bind(actor.principal_id)
    .bind(actor.identity_id)
    .bind(&actor.client)
    .bind(&actor.auth_mode)
    .bind(actor.claimed_device_id)
    .bind(action)
    .bind(target_id)
    .bind(after)
    .execute(&mut **tx)
    .await
    .map_err(|e| DeviceCredentialError::Db(e.to_string()))?;
    Ok(())
}

/// 撤销（幂等）：`credential_revoked_at = now()`；下一请求起全通道 401
///（约束 C：每请求直查库，无缓存窗口——轮询/续领/执行/既有连接全部关闭）。
pub async fn revoke_device_credential(
    pool: &PgPool,
    tenant_id: Uuid,
    device_id: Uuid,
    actor: &AuditActor,
) -> Result<bool, DeviceCredentialError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| DeviceCredentialError::Db(e.to_string()))?;
    let result = sqlx::query(
        "UPDATE devenv.device \
            SET credential_revoked_at = now(), updated_at = now() \
          WHERE id = $1 AND tenant_id = $2 \
            AND credential_key_hash IS NOT NULL \
            AND credential_revoked_at IS NULL \
         RETURNING credential_revoked_at",
    )
    .bind(device_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DeviceCredentialError::Db(e.to_string()))?;
    let Some(row) = result else {
        tx.commit()
            .await
            .map_err(|e| DeviceCredentialError::Db(e.to_string()))?;
        return Ok(false);
    };
    let revoked_at: chrono::DateTime<chrono::Utc> = row.get("credential_revoked_at");
    insert_tenant_audit(
        &mut tx,
        tenant_id,
        actor,
        "device.credential.revoked",
        device_id,
        serde_json::json!({ "device_id": device_id, "revoked_at": revoked_at }),
    )
    .await?;
    tx.commit()
        .await
        .map_err(|e| DeviceCredentialError::Db(e.to_string()))?;
    Ok(true)
}

/// 更新 granted_permissions 裁剪值（Q-C；签发后可收紧）。
pub async fn set_granted_permissions(
    pool: &PgPool,
    tenant_id: Uuid,
    device_id: Uuid,
    permissions: &[String],
    actor: &AuditActor,
) -> Result<Vec<String>, DeviceCredentialError> {
    validate_permissions(permissions)?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| DeviceCredentialError::Db(e.to_string()))?;
    let result = sqlx::query(
        "UPDATE devenv.device \
            SET granted_permissions = $3, updated_at = now() \
          WHERE id = $1 AND tenant_id = $2 \
         RETURNING granted_permissions",
    )
    .bind(device_id)
    .bind(tenant_id)
    .bind(permissions)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DeviceCredentialError::Db(e.to_string()))?
    .ok_or(DeviceCredentialError::NotFound)?;
    let granted: Vec<String> = result.get("granted_permissions");
    insert_tenant_audit(
        &mut tx,
        tenant_id,
        actor,
        "device.permissions.changed",
        device_id,
        serde_json::json!({ "device_id": device_id, "granted_permissions": granted }),
    )
    .await?;
    tx.commit()
        .await
        .map_err(|e| DeviceCredentialError::Db(e.to_string()))?;
    Ok(granted)
}

/// 解析（约束 A 四项验证链 + owner principal active；无缓存，每请求直查）。
/// 任何一项不满足 → None（调用方一律 401，不得回退真人 JWT 路径）。
pub async fn resolve(
    pool: &PgPool,
    plaintext: &str,
) -> Result<Option<ResolvedClientCredential>, DeviceCredentialError> {
    if !plaintext.starts_with(CREDENTIAL_PREFIX) {
        return Ok(None);
    }
    let hash = hash_key(plaintext);
    let row = sqlx::query(
        "SELECT d.id AS device_id, d.tenant_id, d.owner_principal_id, \
                d.granted_permissions, d.status, d.credential_revoked_at, \
                p.is_active AS owner_active \
           FROM devenv.device d \
           JOIN core.principal p ON p.id = d.owner_principal_id \
          WHERE d.credential_key_hash = $1",
    )
    .bind(hash)
    .fetch_optional(pool)
    .await
    .map_err(|e| DeviceCredentialError::Db(e.to_string()))?;
    let Some(row) = row else {
        return Ok(None);
    };
    // 约束 A 四项 + owner active（Tenant 由 device 行携带，owner 归属同表约束）。
    let status: String = row.get("status");
    let revoked_at: Option<chrono::DateTime<chrono::Utc>> = row.get("credential_revoked_at");
    let owner_active: bool = row.get("owner_active");
    if status != "active" || revoked_at.is_some() || !owner_active {
        return Ok(None);
    }
    Ok(Some(ResolvedClientCredential {
        device_id: row.get("device_id"),
        tenant_id: row.get("tenant_id"),
        owner_principal_id: row.get("owner_principal_id"),
        permissions: row.get("granted_permissions"),
    }))
}
