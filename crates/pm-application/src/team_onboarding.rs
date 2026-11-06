//! Team Onboarding 统一应用服务（设计稿 `docs/team-onboarding/`，产品裁决 2026-11-06：
//! Q1 首登必须建团或入团 / Q2 创建者自动 Owner / Q3 邮箱邀请+邀请链接）。
//!
//! 实体映射（2026-11-06 现实扫描裁决，设计稿 §14「Extend 不重建」）：**Team ≡
//! `core.tenant`**（DEC-LLM-001 口径）——团队档案 = tenant 行（0044 补
//! slug/description），成员关系 = `core.tenant_member`（owner/admin/member 现成）；
//! project 保持 tenant_id 组合 FK，不新增 team_id。owner 转移/移除/角色升降沿用
//! `tenant_members` 既有端点，本服务只补 onboarding 缺口：
//!
//! 1. [`create_team`]：**认领**当前 token 租户。前提 = 该租户零 ACTIVE 成员
//!    （provision 自动落 member 行但 principal 未激活，即「首登无团队」态）；
//!    创建者同事务自升 owner + 激活（Q2）。租户已被认领（有 ACTIVE 成员）→ 409。
//! 2. 邮箱邀请（§3.3）：角色仅 admin|member（OWNER 不可经邀请产生）；
//!    同团队同邮箱仅一张 PENDING（部分唯一索引）；接受 = 把本租户 inactive
//!    principal 激活为被邀角色——lmms 账号邀请负责建号/设密码，本链负责
//!    团队角色预授与激活。
//! 3. 邀请链接（§3.4/§3.5）：default_role 仅 'member'；token 只存 SHA-256；
//!    余量在链接行 FOR UPDATE 事务内消耗（并发争最后一个名额只赢一个）；
//!    UNIQUE(link_id, principal_id) 保证同用户幂等。
//!
//! 现实约束（V1 裁决，见 0044 头注释）：a) principal 无 email 列、JWT 无 email
//! claim → 接受时的 email 比对按客户端提交（iam UserInfo）尽力执行；b) 无
//! SMTP → 邀请只返回接受路径供复制分享；c) lmms 单租户用户 → 接受只可能
//! 发生在 token 所在租户（tid 绑定），跨租户邀请显式 mismatch 报错。
//!
//! 审计：`governance.tenant_audit_log`（四列身份口径 + 0041 的
//! target_type/target_id 通用列），审计失败即整体回滚。角色唯一事实源是库内
//! `tenant_member.role`，与全仓口径一致。

use std::time::Duration;

use chrono::{DateTime, Utc};
use getrandom::fill as fill_random;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::actor::AuditActor;

/// 邀请/链接默认有效期（设计稿 §3.3 建议 7 天起步；产品化配置留后续）。
pub const DEFAULT_TTL: Duration = Duration::from_secs(7 * 24 * 3600);

/// 邀请接受路径（前端路由；token 以查询串携带，仅 V1 复制分享语义）。
pub const INVITATION_ACCEPT_PATH: &str = "/invitation/accept";
pub const LINK_ACCEPT_PATH: &str = "/invitation/accept";

// ─────────────────────────────────────────────────────────── 错误

#[derive(Debug, thiserror::Error)]
pub enum TeamOnboardingError {
    #[error("team not found")]
    NotFound,
    #[error("{0}")]
    Validation(&'static str),
    #[error("{0}")]
    Conflict(&'static str),
    /// 稳定 machine message（前端据其渲染结果态）：email_mismatch /
    /// invitation_tenant_mismatch / team_suspended。
    #[error("{0}")]
    Forbidden(&'static str),
    #[error("invitation_expired")]
    Expired,
    #[error("invitation_revoked")]
    Revoked,
    #[error("invitation_already_accepted")]
    AlreadyAccepted,
    #[error("invitation_link_exhausted")]
    Exhausted,
    #[error("invitation_not_found")]
    InvitationNotFound,
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

// ─────────────────────────────────────────────────────────── 行与载荷

/// 团队档案（tenant 行投影）。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct TeamProfile {
    pub id: Uuid,
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct InvitationRow {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    /// 计算态：PENDING 且过期显示 EXPIRED（惰性转移只发生在 accept 命中时）。
    pub status: String,
    pub invited_by: Uuid,
    pub invited_by_name: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct InvitationLinkRow {
    pub id: Uuid,
    pub default_role: String,
    pub status: String,
    pub max_uses: Option<i32>,
    pub used_count: i32,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// 未登录不可见的预览面只暴露团队名/邀请人/角色/有效期——token 本身即凭证。
#[derive(Debug, Clone, Serialize)]
pub struct InvitationPreview {
    pub team_name: String,
    pub team_slug: Option<String>,
    pub inviter_name: String,
    pub role: String,
    pub expires_at: DateTime<Utc>,
    pub status: String,
    /// 邀请目标邮箱（规范化），供接受页做 email 比对提示。
    pub email: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LinkPreview {
    pub team_name: String,
    pub default_role: String,
    pub expires_at: DateTime<Utc>,
    pub status: String,
    pub remaining_uses: Option<i64>,
}

/// 创建邀请/链接的一次性返回：明文 token 只在此刻出现（库内仅哈希）。
#[derive(Debug, Clone, Serialize)]
pub struct InvitationCreated {
    pub id: Uuid,
    pub token: String,
    pub accept_path: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LinkCreated {
    pub id: Uuid,
    pub token: String,
    pub accept_path: String,
    pub expires_at: DateTime<Utc>,
}

/// 接受结果：幂等语义——已是 ACTIVE 成员时不重复建关系（设计稿 §7.2/§7.3）。
#[derive(Debug, Clone, Serialize)]
pub struct AcceptOutcome {
    pub outcome: &'static str, // "joined" | "already_member"
    pub team_id: Uuid,
    pub team_name: String,
    pub role: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CreateTeamRequest {
    pub name: String,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct UpdateTeamRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CreateInvitationRequest {
    pub email: String,
    pub role: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AcceptInvitationRequest {
    pub token: String,
    /// 客户端持有的 iam UserInfo email（JWT 无 email claim，V1 尽力比对）。
    #[serde(default)]
    pub email: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AcceptLinkRequest {
    pub token: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CreateLinkRequest {
    #[serde(default)]
    pub default_role: Option<String>,
    /// ISO8601；缺省 = now + 7d。
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    /// NULL/缺省 = 不限次数。
    #[serde(default)]
    pub max_uses: Option<i32>,
}

// ─────────────────────────────────────────────────────────── 基础件

fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}

fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

/// 宽松邮箱校验（本地@域名非空、总长 ≤320）：权威校验在上游 iam 注册面。
fn valid_email(email: &str) -> bool {
    let e = email.trim();
    if e.len() > 320 || e.contains(char::is_whitespace) {
        return false;
    }
    match e.split_once('@') {
        Some((local, domain)) => !local.is_empty() && !domain.is_empty() && domain.contains('.'),
        None => false,
    }
}

/// slug 规则：`^[a-z0-9][a-z0-9-]{0,98}$`（设计稿 §3.1，全局唯一落 DB 索引）。
fn valid_slug(slug: &str) -> bool {
    let bytes = slug.as_bytes();
    if bytes.is_empty() || bytes.len() > 100 {
        return false;
    }
    let ok = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-';
    (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit()) && bytes.iter().all(|&b| ok(b))
}

/// 256-bit 随机 token（hex 64 字符）；库内只存 SHA-256（设计稿 §11）。
fn generate_token() -> Result<String, TeamOnboardingError> {
    let mut bytes = [0u8; 32];
    fill_random(&mut bytes).map_err(|_| TeamOnboardingError::Validation("random unavailable"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn validate_team_name(name: &str) -> Result<(), TeamOnboardingError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 120 {
        return Err(TeamOnboardingError::Validation("team name must be 1..=120 chars"));
    }
    Ok(())
}

/// 团队档案列的写入体（认领与改档案共用）。
fn tenant_display_name(name: &str) -> String {
    name.trim().to_string()
}

async fn insert_tenant_audit(
    tx: &mut Transaction<'static, Postgres>,
    tenant_id: Uuid,
    actor: &AuditActor,
    action: &str,
    target_type: &str,
    target_id: Uuid,
    before: Option<Value>,
    after: Option<Value>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO governance.tenant_audit_log \
         (tenant_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
          action, target_type, target_id, target_principal_id, before_data, after_data) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NULL, $10, $11)",
    )
    .bind(tenant_id)
    .bind(actor.principal_id)
    .bind(actor.identity_id)
    .bind(&actor.client)
    .bind(&actor.auth_mode)
    .bind(actor.claimed_device_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(before)
    .bind(after)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// slug 唯一冲突（23505 on `uq_tenant_slug`）映射为稳定 409。
fn map_slug_conflict(err: sqlx::Error) -> TeamOnboardingError {
    if let sqlx::Error::Database(db) = &err {
        if db.constraint().is_some_and(|c| c.contains("uq_tenant_slug")) {
            return TeamOnboardingError::Conflict("slug_already_taken");
        }
    }
    TeamOnboardingError::Db(err)
}

/// 同团队同邮箱 PENDING 唯一冲突映射。
fn map_pending_conflict(err: sqlx::Error) -> TeamOnboardingError {
    if let sqlx::Error::Database(db) = &err {
        if db
            .constraint()
            .is_some_and(|c| c.contains("uq_tenant_invitation_pending_email"))
        {
            return TeamOnboardingError::Conflict("pending_invitation_exists");
        }
    }
    TeamOnboardingError::Db(err)
}

// ─────────────────────────────────────────────────────────── 团队档案

/// 团队档案读（成员可见；跨租户由 handler 的 guard_tenant 收口）。
pub async fn team_profile(
    pool: &PgPool,
    tenant_id: Uuid,
) -> Result<Option<TeamProfile>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, display_name AS name, slug, description \
           FROM core.tenant WHERE id = $1",
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
}

/// 认领建团（Q1/Q2）：锁 tenant 行 → 校验零 ACTIVE 成员 → 写档案 + 自升 owner
/// + 激活，单事务。并发认领由行锁串行化，输家 409。
pub async fn create_team(
    pool: &PgPool,
    actor: &AuditActor,
    principal_id: Uuid,
    tenant_id: Uuid,
    req: CreateTeamRequest,
) -> Result<TeamProfile, TeamOnboardingError> {
    let name = req.name.trim();
    validate_team_name(name)?;
    if let Some(slug) = req.slug.as_deref() {
        if !slug.is_empty() && !valid_slug(slug) {
            return Err(TeamOnboardingError::Validation("invalid slug"));
        }
    }

    let mut tx = pool.begin().await?;
    // 行锁串行化并发认领（设计稿 §3.2「以 Team 行为锁定对象的事务方案」）。
    let locked: Option<(String,)> =
        sqlx::query_as("SELECT display_name FROM core.tenant WHERE id = $1 FOR UPDATE")
            .bind(tenant_id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some(_) = locked else {
        return Err(TeamOnboardingError::NotFound);
    };

    // 认领前提：租户内没有任何 ACTIVE 成员（provision 落的行 principal 均
    // inactive，不计入）。系统租户/已认领租户在此被拒绝。
    let active_members: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.tenant_member tm \
          JOIN core.principal p ON p.id = tm.principal_id \
          WHERE tm.tenant_id = $1 AND p.is_active",
    )
    .bind(tenant_id)
    .fetch_one(&mut *tx)
    .await?;
    if active_members > 0 {
        return Err(TeamOnboardingError::Conflict("team_already_exists"));
    }

    let slug = req
        .slug
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let description = req
        .description
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let display_name = tenant_display_name(name);

    sqlx::query(
        "UPDATE core.tenant \
            SET display_name = $2, slug = $3, description = $4, is_active = true, updated_at = now() \
          WHERE id = $1",
    )
    .bind(tenant_id)
    .bind(&display_name)
    .bind(&slug)
    .bind(&description)
    .execute(&mut *tx)
    .await
    .map_err(map_slug_conflict)?;

    // 创建者自升 OWNER（Q2）：provision 已落 member 行 → 覆盖为 owner；
    // 无行（极端：成员行被清）→ 插入。
    sqlx::query(
        "INSERT INTO core.tenant_member (tenant_id, principal_id, role, granted_by) \
         VALUES ($1, $2, 'owner', $2) \
         ON CONFLICT (tenant_id, principal_id) \
         DO UPDATE SET role = 'owner', granted_by = $2, updated_at = now()",
    )
    .bind(tenant_id)
    .bind(principal_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE core.principal SET is_active = true WHERE id = $1")
        .bind(principal_id)
        .execute(&mut *tx)
        .await?;

    insert_tenant_audit(
        &mut tx,
        tenant_id,
        actor,
        "team.created",
        "tenant",
        tenant_id,
        None,
        Some(json!({ "name": display_name, "slug": slug, "owner": principal_id })),
    )
    .await?;

    tx.commit().await?;
    Ok(TeamProfile {
        id: tenant_id,
        name: display_name,
        slug,
        description,
    })
}

/// 修改团队资料（owner/admin；角色矩阵「修改团队资料」）。
pub async fn update_team_profile(
    pool: &PgPool,
    actor: &AuditActor,
    tenant_id: Uuid,
    req: UpdateTeamRequest,
) -> Result<TeamProfile, TeamOnboardingError> {
    if let Some(name) = req.name.as_deref() {
        validate_team_name(name)?;
    }
    if let Some(slug) = req.slug.as_deref() {
        if !slug.is_empty() && !valid_slug(slug) {
            return Err(TeamOnboardingError::Validation("invalid slug"));
        }
    }

    let mut tx = pool.begin().await?;
    let current: Option<TeamProfile> = sqlx::query_as(
        "SELECT id, display_name AS name, slug, description \
           FROM core.tenant WHERE id = $1 FOR UPDATE",
    )
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(current) = current else {
        return Err(TeamOnboardingError::NotFound);
    };

    // 审计 before 快照先行（下方 unwrap_or 会移走 current 的字段）。
    let before = json!({
        "name": current.name,
        "slug": current.slug,
        "description": current.description
    });

    let name = req
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .unwrap_or(current.name);
    let slug = match req.slug.as_deref() {
        Some(s) if !s.trim().is_empty() => Some(s.trim().to_owned()),
        Some(_) => None, // 显式空串 = 清除 slug
        None => current.slug,
    };
    let description = match req.description.as_deref() {
        Some(s) if !s.trim().is_empty() => Some(s.trim().to_owned()),
        Some(_) => None,
        None => current.description,
    };

    sqlx::query(
        "UPDATE core.tenant \
            SET display_name = $2, slug = $3, description = $4, updated_at = now() \
          WHERE id = $1",
    )
    .bind(tenant_id)
    .bind(&name)
    .bind(&slug)
    .bind(&description)
    .execute(&mut *tx)
    .await
    .map_err(map_slug_conflict)?;

    insert_tenant_audit(
        &mut tx,
        tenant_id,
        actor,
        "team.updated",
        "tenant",
        tenant_id,
        Some(before),
        Some(json!({ "name": name, "slug": slug, "description": description })),
    )
    .await?;

    tx.commit().await?;
    Ok(TeamProfile { id: tenant_id, name, slug, description })
}

// ─────────────────────────────────────────────────────────── 邮箱邀请

/// 创建邮箱邀请（owner/admin；角色仅 admin|member）。返回一次性明文 token。
pub async fn create_invitation(
    pool: &PgPool,
    actor: &AuditActor,
    tenant_id: Uuid,
    req: CreateInvitationRequest,
) -> Result<InvitationCreated, TeamOnboardingError> {
    let email = req.email.trim().to_string();
    if !valid_email(&email) {
        return Err(TeamOnboardingError::Validation("invalid email"));
    }
    let role = req.role.trim().to_lowercase();
    if role != "admin" && role != "member" {
        // OWNER 不可经邀请产生（spec §5.1 / 权限矩阵）。
        return Err(TeamOnboardingError::Validation("invitation role must be admin or member"));
    }
    let email_normalized = normalize_email(&email);
    let expires_at = now() + chrono::Duration::from_std(DEFAULT_TTL).expect("static ttl");
    let token = generate_token()?;

    let mut tx = pool.begin().await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant_invitation \
             (tenant_id, email, email_normalized, role, token_hash, invited_by, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         RETURNING id",
    )
    .bind(tenant_id)
    .bind(&email)
    .bind(&email_normalized)
    .bind(&role)
    .bind(hash_token(&token))
    .bind(actor.principal_id)
    .bind(expires_at)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_pending_conflict)?;

    insert_tenant_audit(
        &mut tx,
        tenant_id,
        actor,
        "team_invitation.created",
        "tenant_invitation",
        id,
        None,
        Some(json!({ "email": email_normalized, "role": role, "expires_at": expires_at })),
    )
    .await?;
    tx.commit().await?;

    let accept_path = format!("{INVITATION_ACCEPT_PATH}?token={token}&type=invite");
    Ok(InvitationCreated { id, token, accept_path, expires_at })
}

/// 邀请列表（owner/admin）。PENDING 过期显示 EXPIRED（计算态，惰性翻转
/// 只发生在 accept 命中时——列表是读面，不因查看而写）。
pub async fn list_invitations(
    pool: &PgPool,
    tenant_id: Uuid,
) -> Result<Vec<InvitationRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT i.id, i.email, i.role, \
                CASE WHEN i.status = 'PENDING' AND i.expires_at < now() \
                     THEN 'EXPIRED' ELSE i.status END AS status, \
                i.invited_by, COALESCE(p.display_name, i.invited_by::text) AS invited_by_name, \
                i.expires_at, i.created_at, i.accepted_at \
           FROM core.tenant_invitation i \
           LEFT JOIN core.principal p ON p.id = i.invited_by \
          WHERE i.tenant_id = $1 \
          ORDER BY i.created_at DESC",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
}

/// 撤销邀请（owner/admin；仅 PENDING 可撤销，设计稿 §7 异常表）。
pub async fn revoke_invitation(
    pool: &PgPool,
    actor: &AuditActor,
    tenant_id: Uuid,
    invitation_id: Uuid,
) -> Result<(), TeamOnboardingError> {
    let mut tx = pool.begin().await?;
    let locked: Option<(String,)> = sqlx::query_as(
        "SELECT status FROM core.tenant_invitation \
          WHERE id = $1 AND tenant_id = $2 FOR UPDATE",
    )
    .bind(invitation_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((status,)) = locked else {
        return Err(TeamOnboardingError::NotFound);
    };
    if status != "PENDING" {
        return Err(TeamOnboardingError::Conflict("invitation_not_pending"));
    }
    sqlx::query(
        "UPDATE core.tenant_invitation SET status = 'REVOKED', revoked_at = now() \
          WHERE id = $1",
    )
    .bind(invitation_id)
    .execute(&mut *tx)
    .await?;
    insert_tenant_audit(
        &mut tx,
        tenant_id,
        actor,
        "team_invitation.revoked",
        "tenant_invitation",
        invitation_id,
        None,
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// 邀请预览（按 token 哈希查；团队名/邀请人/角色/有效期，无任何密钥回显）。
/// 跨租户 caller 也可预览——token 本身即凭证，接受动作才做租户匹配。
pub async fn preview_invitation(
    pool: &PgPool,
    token: &str,
) -> Result<Option<InvitationPreview>, sqlx::Error> {
    let row: Option<(String, Option<String>, String, Option<String>, String, DateTime<Utc>, String, String)> =
        sqlx::query_as(
            "SELECT t.display_name, t.slug, i.role, p.display_name, \
                    CASE WHEN i.status = 'PENDING' AND i.expires_at < now() \
                         THEN 'EXPIRED' ELSE i.status END AS status, \
                    i.expires_at, i.email_normalized, i.status \
               FROM core.tenant_invitation i \
               JOIN core.tenant t ON t.id = i.tenant_id \
               LEFT JOIN core.principal p ON p.id = i.invited_by \
              WHERE i.token_hash = $1",
        )
        .bind(hash_token(token))
        .fetch_optional(pool)
        .await?;
    let Some((team_name, team_slug, role, inviter_name, status, expires_at, email, raw_status)) =
        row
    else {
        return Ok(None);
    };
    let _ = raw_status;
    Ok(Some(InvitationPreview {
        team_name,
        team_slug,
        inviter_name: inviter_name.unwrap_or_else(|| "unknown".to_string()),
        role,
        expires_at,
        status,
        email,
    }))
}

/// 接受邮箱邀请（设计稿 §7.2）。幂等语义：已是 ACTIVE 成员 → 终结邀请并返回
/// already_member；重复接受同一张已 ACCEPTED 邀请 → AlreadyAccepted。
pub async fn accept_invitation(
    pool: &PgPool,
    actor: &AuditActor,
    principal_id: Uuid,
    caller_tenant_id: Uuid,
    req: AcceptInvitationRequest,
) -> Result<AcceptOutcome, TeamOnboardingError> {
    let mut tx = pool.begin().await?;
    // 锁定即取全决策所需列：id/租户/角色/状态/有效期/规范化邮箱/邀请人。
    let locked: Option<(Uuid, Uuid, String, String, DateTime<Utc>, String, Uuid)> =
        sqlx::query_as(
            "SELECT id, tenant_id, role, status, expires_at, email_normalized, invited_by \
               FROM core.tenant_invitation WHERE token_hash = $1 FOR UPDATE",
        )
        .bind(hash_token(&req.token))
        .fetch_optional(&mut *tx)
        .await?;
    let Some((invitation_id, tenant_id, role, status, expires_at, email_normalized, invited_by)) =
        locked
    else {
        return Err(TeamOnboardingError::InvitationNotFound);
    };

    // 状态闸：REVOKED / ACCEPTED / PENDING 但过期（惰性翻转到 EXPIRED）。
    match status.as_str() {
        "REVOKED" => return Err(TeamOnboardingError::Revoked),
        "ACCEPTED" | "EXPIRED" => return Err(TeamOnboardingError::AlreadyAccepted),
        _ => {}
    }
    if expires_at <= now() {
        sqlx::query(
            "UPDATE core.tenant_invitation SET status = 'EXPIRED' \
              WHERE id = $1 AND status = 'PENDING'",
        )
        .bind(invitation_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        return Err(TeamOnboardingError::Expired);
    }

    // tid 绑定：接受只可能发生在 token 所在租户（V1 单团队现实，见模块头）。
    if tenant_id != caller_tenant_id {
        return Err(TeamOnboardingError::Forbidden("invitation_tenant_mismatch"));
    }
    let team_active: Option<bool> =
        sqlx::query_scalar("SELECT is_active FROM core.tenant WHERE id = $1")
            .bind(tenant_id)
            .fetch_optional(&mut *tx)
            .await?;
    if team_active != Some(true) {
        return Err(TeamOnboardingError::Forbidden("team_suspended"));
    }

    // email 尽力比对：客户端提交了才比对（JWT 无 email claim，V1 裁决）。
    if let Some(email) = req.email.as_deref() {
        let email = email.trim();
        if !email.is_empty() && normalize_email(email) != email_normalized {
            return Err(TeamOnboardingError::Forbidden("email_mismatch"));
        }
    }

    // 成员现状：ACTIVE 成员 → 幂等终结；否则激活 + 落被邀角色。
    let membership: Option<(String,)> = sqlx::query_as(
        "SELECT tm.role FROM core.tenant_member tm \
          JOIN core.principal p ON p.id = tm.principal_id \
          WHERE tm.tenant_id = $1 AND tm.principal_id = $2 AND p.is_active",
    )
    .bind(tenant_id)
    .bind(principal_id)
    .fetch_optional(&mut *tx)
    .await?;

    let team_name: String =
        sqlx::query_scalar("SELECT display_name FROM core.tenant WHERE id = $1")
            .bind(tenant_id)
            .fetch_one(&mut *tx)
            .await?;

    let outcome = if membership.is_some() {
        AcceptOutcome { outcome: "already_member", team_id: tenant_id, team_name, role }
    } else {
        sqlx::query(
            "INSERT INTO core.tenant_member (tenant_id, principal_id, role, granted_by) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (tenant_id, principal_id) \
             DO UPDATE SET role = $3, granted_by = $4, updated_at = now()",
        )
        .bind(tenant_id)
        .bind(principal_id)
        .bind(&role)
        // granted_by = 邀请人（迁移 0019 列语义）。
        .bind(invited_by)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE core.principal SET is_active = true WHERE id = $1")
            .bind(principal_id)
            .execute(&mut *tx)
            .await?;
        insert_tenant_audit(
            &mut tx,
            tenant_id,
            actor,
            "team_member.activated",
            "principal",
            principal_id,
            None,
            Some(json!({ "role": role, "via": "invitation" })),
        )
        .await?;
        AcceptOutcome { outcome: "joined", team_id: tenant_id, team_name, role }
    };

    // 邀请单终结（含 already_member 分支——设计稿 §7.2「明确的终结处理」）。
    sqlx::query(
        "UPDATE core.tenant_invitation \
            SET status = 'ACCEPTED', accepted_principal_id = $2, accepted_at = now() \
          WHERE id = $1 AND status = 'PENDING'",
    )
    .bind(invitation_id)
    .bind(principal_id)
    .execute(&mut *tx)
    .await?;

    insert_tenant_audit(
        &mut tx,
        tenant_id,
        actor,
        "team_invitation.accepted",
        "tenant_invitation",
        invitation_id,
        None,
        Some(json!({ "principal_id": principal_id, "outcome": outcome.outcome, "role": outcome.role })),
    )
    .await?;

    tx.commit().await?;
    Ok(outcome)
}


// ─────────────────────────────────────────────────────────── 邀请链接

/// 创建邀请链接（owner/admin）。V1 链接只发 MEMBER（default_role 仅 'member'，
/// 禁止经链接产生 ADMIN/OWNER）；token 只存哈希，明文一次性返回。
pub async fn create_invitation_link(
    pool: &PgPool,
    actor: &AuditActor,
    tenant_id: Uuid,
    req: CreateLinkRequest,
) -> Result<LinkCreated, TeamOnboardingError> {
    let role = req.default_role.as_deref().unwrap_or("member").trim().to_lowercase();
    if role != "member" {
        return Err(TeamOnboardingError::Validation("link default role must be member"));
    }
    if let Some(max_uses) = req.max_uses {
        if max_uses <= 0 {
            return Err(TeamOnboardingError::Validation("max_uses must be positive"));
        }
    }
    let expires_at = req.expires_at.unwrap_or_else(|| now() + chrono::Duration::from_std(DEFAULT_TTL).expect("static ttl"));
    if expires_at <= now() {
        return Err(TeamOnboardingError::Validation("expires_at must be in the future"));
    }
    let token = generate_token()?;

    let mut tx = pool.begin().await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant_invitation_link \
             (tenant_id, token_hash, default_role, max_uses, expires_at, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(tenant_id)
    .bind(hash_token(&token))
    .bind(&role)
    .bind(req.max_uses)
    .bind(expires_at)
    .bind(actor.principal_id)
    .fetch_one(&mut *tx)
    .await?;

    insert_tenant_audit(
        &mut tx,
        tenant_id,
        actor,
        "team_invitation_link.created",
        "tenant_invitation_link",
        id,
        None,
        Some(json!({ "default_role": role, "max_uses": req.max_uses, "expires_at": expires_at })),
    )
    .await?;
    tx.commit().await?;

    let accept_path = format!("{LINK_ACCEPT_PATH}?token={token}&type=link");
    Ok(LinkCreated { id, token, accept_path, expires_at })
}

/// 链接列表（owner/admin）；ACTIVE 行按过期/余量给计算态。
pub async fn list_invitation_links(
    pool: &PgPool,
    tenant_id: Uuid,
) -> Result<Vec<InvitationLinkRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT l.id, l.default_role, \
                CASE WHEN l.status = 'ACTIVE' AND l.expires_at < now() THEN 'EXPIRED' \
                     WHEN l.status = 'ACTIVE' AND l.max_uses IS NOT NULL \
                          AND l.used_count >= l.max_uses THEN 'EXHAUSTED' \
                     ELSE l.status END AS status, \
                l.max_uses, l.used_count, l.expires_at, l.created_at \
           FROM core.tenant_invitation_link l \
          WHERE l.tenant_id = $1 \
          ORDER BY l.created_at DESC",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
}

/// 撤销链接（owner/admin；仅 ACTIVE 可撤销）。
pub async fn revoke_invitation_link(
    pool: &PgPool,
    actor: &AuditActor,
    tenant_id: Uuid,
    link_id: Uuid,
) -> Result<(), TeamOnboardingError> {
    let mut tx = pool.begin().await?;
    let locked: Option<(String,)> = sqlx::query_as(
        "SELECT status FROM core.tenant_invitation_link \
          WHERE id = $1 AND tenant_id = $2 FOR UPDATE",
    )
    .bind(link_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((status,)) = locked else {
        return Err(TeamOnboardingError::NotFound);
    };
    if status != "ACTIVE" {
        return Err(TeamOnboardingError::Conflict("link_not_active"));
    }
    sqlx::query(
        "UPDATE core.tenant_invitation_link SET status = 'REVOKED', revoked_at = now() \
          WHERE id = $1",
    )
    .bind(link_id)
    .execute(&mut *tx)
    .await?;
    insert_tenant_audit(
        &mut tx,
        tenant_id,
        actor,
        "team_invitation_link.revoked",
        "tenant_invitation_link",
        link_id,
        None,
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// 链接预览（token 即凭证；含剩余次数，供接受页展示用量）。
pub async fn preview_invitation_link(
    pool: &PgPool,
    token: &str,
) -> Result<Option<LinkPreview>, sqlx::Error> {
    let row: Option<(String, String, Option<i32>, i32, DateTime<Utc>, String)> = sqlx::query_as(
        "SELECT t.display_name, l.default_role, l.max_uses, l.used_count, l.expires_at, l.status \
           FROM core.tenant_invitation_link l \
           JOIN core.tenant t ON t.id = l.tenant_id \
          WHERE l.token_hash = $1",
    )
    .bind(hash_token(token))
    .fetch_optional(pool)
    .await?;
    let Some((team_name, default_role, max_uses, used_count, expires_at, status)) = row else {
        return Ok(None);
    };
    let status = if status == "ACTIVE" && expires_at <= now() {
        "EXPIRED".to_string()
    } else if status == "ACTIVE" && max_uses.is_some_and(|m| used_count >= m) {
        "EXHAUSTED".to_string()
    } else {
        status
    };
    let remaining_uses = max_uses.map(|m| (m - used_count).max(0) as i64);
    Ok(Some(LinkPreview { team_name, default_role, expires_at, status, remaining_uses }))
}

/// 接受邀请链接（设计稿 §7.3）。幂等：同用户重复提交不重复计数
/// （UNIQUE(link_id, principal_id) + 已是成员直接 already_member）；
/// 并发争最后一个名额由链接行 FOR UPDATE 串行化，只赢一个。
pub async fn accept_invitation_link(
    pool: &PgPool,
    actor: &AuditActor,
    principal_id: Uuid,
    caller_tenant_id: Uuid,
    token: &str,
) -> Result<AcceptOutcome, TeamOnboardingError> {
    let mut tx = pool.begin().await?;
    let locked: Option<(Uuid, Uuid, String, String, Option<i32>, i32, DateTime<Utc>)> =
        sqlx::query_as(
            "SELECT id, tenant_id, default_role, status, max_uses, used_count, expires_at \
               FROM core.tenant_invitation_link WHERE token_hash = $1 FOR UPDATE",
        )
        .bind(hash_token(token))
        .fetch_optional(&mut *tx)
        .await?;
    let Some((link_id, tenant_id, default_role, status, max_uses, used_count, expires_at)) =
        locked
    else {
        return Err(TeamOnboardingError::InvitationNotFound);
    };

    // 状态闸（死 token 先拒）：REVOKED / 过期（惰性翻转）。EXHAUSTED 的判定
    // 延后到成员检查之后——已是成员的重复提交是幂等 already_member，不因
    // 名额耗尽而变 error（UNIQUE(link_id, principal_id) 已有领用记录）。
    match status.as_str() {
        "REVOKED" => return Err(TeamOnboardingError::Revoked),
        _ => {}
    }
    if expires_at <= now() {
        sqlx::query(
            "UPDATE core.tenant_invitation_link SET status = 'EXPIRED' \
              WHERE id = $1 AND status = 'ACTIVE'",
        )
        .bind(link_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        return Err(TeamOnboardingError::Expired);
    }

    // tid 绑定与租户活性（与邮箱邀请同一口径）。
    if tenant_id != caller_tenant_id {
        return Err(TeamOnboardingError::Forbidden("invitation_tenant_mismatch"));
    }
    let team_active: Option<bool> =
        sqlx::query_scalar("SELECT is_active FROM core.tenant WHERE id = $1")
            .bind(tenant_id)
            .fetch_optional(&mut *tx)
            .await?;
    if team_active != Some(true) {
        return Err(TeamOnboardingError::Forbidden("team_suspended"));
    }

    let membership: Option<(String,)> = sqlx::query_as(
        "SELECT tm.role FROM core.tenant_member tm \
          JOIN core.principal p ON p.id = tm.principal_id \
          WHERE tm.tenant_id = $1 AND tm.principal_id = $2 AND p.is_active",
    )
    .bind(tenant_id)
    .bind(principal_id)
    .fetch_optional(&mut *tx)
    .await?;

    let team_name: String =
        sqlx::query_scalar("SELECT display_name FROM core.tenant WHERE id = $1")
            .bind(tenant_id)
            .fetch_one(&mut *tx)
            .await?;

    let outcome = if membership.is_some() {
        // 已加入：记录领用（幂等）但不消耗名额。
        sqlx::query(
            "INSERT INTO core.tenant_invitation_link_redemption \
                 (tenant_id, link_id, principal_id) VALUES ($1, $2, $3) \
             ON CONFLICT (link_id, principal_id) DO NOTHING",
        )
        .bind(tenant_id)
        .bind(link_id)
        .bind(principal_id)
        .execute(&mut *tx)
        .await?;
        AcceptOutcome { outcome: "already_member", team_id: tenant_id, team_name, role: default_role }
    } else {
        // 名额闸：仅对尚未加入的用户生效（行锁内判定 + 触顶翻 EXHAUSTED）。
        if status == "EXHAUSTED" || max_uses.is_some_and(|m| used_count >= m) {
            sqlx::query(
                "UPDATE core.tenant_invitation_link SET status = 'EXHAUSTED' \
                  WHERE id = $1 AND status = 'ACTIVE'",
            )
            .bind(link_id)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            return Err(TeamOnboardingError::Exhausted);
        }
        sqlx::query(
            "INSERT INTO core.tenant_member (tenant_id, principal_id, role, granted_by) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (tenant_id, principal_id) \
             DO UPDATE SET role = $3, granted_by = $4, updated_at = now()",
        )
        .bind(tenant_id)
        .bind(principal_id)
        .bind(&default_role)
        .bind(actor.principal_id) // 链接无邀请人主体，granted_by 记领用人
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE core.principal SET is_active = true WHERE id = $1")
            .bind(principal_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO core.tenant_invitation_link_redemption \
                 (tenant_id, link_id, principal_id) VALUES ($1, $2, $3)",
        )
        .bind(tenant_id)
        .bind(link_id)
        .bind(principal_id)
        .execute(&mut *tx)
        .await?;
        // 余量消耗：行锁内 +1，触顶翻 EXHAUSTED。
        let new_count = used_count + 1;
        let new_status =
            if max_uses.is_some_and(|m| new_count >= m) { "EXHAUSTED" } else { "ACTIVE" };
        sqlx::query(
            "UPDATE core.tenant_invitation_link SET used_count = $2, status = $3 WHERE id = $1",
        )
        .bind(link_id)
        .bind(new_count)
        .bind(new_status)
        .execute(&mut *tx)
        .await?;
        insert_tenant_audit(
            &mut tx,
            tenant_id,
            actor,
            "team_member.activated",
            "principal",
            principal_id,
            None,
            Some(json!({ "role": default_role, "via": "invitation_link" })),
        )
        .await?;
        AcceptOutcome { outcome: "joined", team_id: tenant_id, team_name, role: default_role }
    };

    insert_tenant_audit(
        &mut tx,
        tenant_id,
        actor,
        "team_invitation_link.redeemed",
        "tenant_invitation_link",
        link_id,
        None,
        Some(json!({ "principal_id": principal_id, "outcome": outcome.outcome })),
    )
    .await?;

    tx.commit().await?;
    Ok(outcome)
}

// ─────────────────────────────────────────────────────────── 解散团队

/// 解散结果（审计与回显用）。
#[derive(Debug, Clone, Serialize)]
pub struct DissolveOutcome {
    pub projects_deleted: i64,
    pub members_removed: i64,
}

/// 解散团队（产品裁决 2026-11-06：**Owner 独占**；admin/member 均不可）。
///
/// 级联语义——「projects 全部删除」在事务内落地为**终态软删**：
/// `core.project.status → 'deleted'` + 项目成员行清空（两道闸都使项目从
/// 列表与可见性检查中消失）。硬删不可行：`governance.audit_log.project_id`
/// NOT NULL FK 锚定审计链，且数十张业务表无级联——审计优先口径下删除历史
/// 不可接受。团队面同步清算：成员关系全撤（含 Owner 本人）、邀请/链接/
/// 领用记录清空、租户休眠（is_active=false + 档案列清空，slug 释放）。
///
/// 后续语义：休眠租户在 `/me` 呈现 team=null → onboarding_required=true，
/// 原 member（principal 仍 active）经认领路径可**重建**团队（create_team
/// 会翻回 is_active）——解散后同 lmms 租户的人不会被永久锁死。不可逆性：
/// 软删项目无恢复端点（V1），租户重建后旧项目不可见。
pub async fn dissolve_team(
    pool: &PgPool,
    actor: &AuditActor,
    principal_id: Uuid,
    tenant_id: Uuid,
) -> Result<DissolveOutcome, TeamOnboardingError> {
    let mut tx = pool.begin().await?;

    let locked: Option<(bool,)> =
        sqlx::query_as("SELECT is_active FROM core.tenant WHERE id = $1 FOR UPDATE")
            .bind(tenant_id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some((is_active,)) = locked else {
        return Err(TeamOnboardingError::NotFound);
    };
    if !is_active {
        return Err(TeamOnboardingError::Conflict("team_already_dissolved"));
    }

    // Owner 独占：admin/member 一律拒绝（实时查库，token 不参与授权）。
    let role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM core.tenant_member \
          WHERE tenant_id = $1 AND principal_id = $2 AND principal_id IN \
                (SELECT id FROM core.principal WHERE is_active)",
    )
    .bind(tenant_id)
    .bind(principal_id)
    .fetch_optional(&mut *tx)
    .await?;
    if role.as_deref() != Some("owner") {
        return Err(TeamOnboardingError::Forbidden("team owner required"));
    }

    // 终态软删项目 + 清项目成员行（两道可见性闸同时失效）。
    let projects_deleted: i64 = sqlx::query_scalar(
        "WITH moved AS ( \
             UPDATE core.project SET status = 'deleted', updated_at = now() \
              WHERE tenant_id = $1 AND status <> 'deleted' \
              RETURNING 1) \
         SELECT count(*) FROM moved",
    )
    .bind(tenant_id)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM core.project_member \
          WHERE project_id IN (SELECT id FROM core.project WHERE tenant_id = $1)",
    )
    .bind(tenant_id)
    .execute(&mut *tx)
    .await?;

    // 团队面清算：邀请/链接/领用清空，成员关系全撤（含 Owner 本人）。
    sqlx::query("DELETE FROM core.tenant_invitation_link_redemption WHERE tenant_id = $1")
        .bind(tenant_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM core.tenant_invitation WHERE tenant_id = $1")
        .bind(tenant_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM core.tenant_invitation_link WHERE tenant_id = $1")
        .bind(tenant_id)
        .execute(&mut *tx)
        .await?;
    let members_removed: i64 = sqlx::query_scalar(
        "WITH gone AS ( \
             DELETE FROM core.tenant_member WHERE tenant_id = $1 RETURNING 1) \
         SELECT count(*) FROM gone",
    )
    .bind(tenant_id)
    .fetch_one(&mut *tx)
    .await?;

    // 租户休眠 + 档案清空（slug 释放，重建团队可复用）。
    sqlx::query(
        "UPDATE core.tenant \
            SET is_active = false, slug = NULL, description = NULL, updated_at = now() \
          WHERE id = $1",
    )
    .bind(tenant_id)
    .execute(&mut *tx)
    .await?;

    insert_tenant_audit(
        &mut tx,
        tenant_id,
        actor,
        "team.dissolved",
        "tenant",
        tenant_id,
        None,
        Some(json!({
            "projects_deleted": projects_deleted,
            "members_removed": members_removed,
            "by": principal_id
        })),
    )
    .await?;

    tx.commit().await?;
    Ok(DissolveOutcome { projects_deleted, members_removed })
}
