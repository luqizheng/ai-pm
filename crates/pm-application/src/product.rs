//! Product Membership 与生命周期统一应用服务（P4-ORG Phase 3；PA 基线 §2/§3/§4/§5；
//! DR-PA-01/02/03，2026-11-20）。
//!
//! 域语义：
//! - **Product Membership** 由 Product 内部管理；Team Member 不自动成为 Product
//!   成员（DR-ORG-03①②废止）。Product 双身份：Owner（生命周期决策）/ Member
//!   （全部日常管理与决策，平权，含添加/移除普通成员）。
//! - **Owner 保护**：多 Owner 并存；不可直接移除最后一位 Owner；更换末位 Owner
//!   先指定继任者（appoint）再变更；Owner 离开 Team 不自动撤销；无 Owner 态由
//!   Team Owner 恢复性任命（唯一显式 Owner 授予通道，第一轮；Member 任命/降级
//!   Owner 归 PA §8-3 OPEN，不实现）。
//! - **生命周期** {active, abandoned}：Owner 决定 abandon/restart；abandoned 后
//!   [`assert_product_active`] 拦截业务写入（fail-closed），历史读取不拦；对关联
//!   Project 与在飞 Agent Run 零触碰。
//! - 审计：`product_member.added/removed/role_changed` + `product.lifecycle_changed`
//!   （governance.tenant_audit_log 管线）。
//!
//! OPEN（PA §8，不得实现）：Member 任命/降级 Owner；Owner 离开 Team 后普通
//! Membership 去留（当前=不自动撤销）；成员添加范围（第一轮限本 Tenant
//! principal）；Abandoned 在途收尾。

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::actor::AuditActor;

/// 邀请类操作的默认有效期（当前 Product 域暂无 token 链路，占位对齐
/// team_onboarding 的 TTL 惯例）。
pub const DEFAULT_TTL: Duration = Duration::from_secs(7 * 24 * 3600);

// ─────────────────────────────────────────────────────────── 错误

#[derive(Debug, thiserror::Error)]
pub enum ProductError {
    #[error("product not found")]
    NotFound,
    #[error("{0}")]
    Validation(&'static str),
    #[error("{0}")]
    Conflict(&'static str),
    /// 稳定 machine message：product_owner_required / not_a_product_member /
    /// team_owner_required / last_owner 等。
    #[error("{0}")]
    Forbidden(&'static str),
    /// 成员变更规则拒绝（锁内裁决）：`(code, message)` 由 HTTP 层按 code
    /// 分流 403/404/409，整体回滚。
    #[error("product rule rejected: {0}: {1}")]
    RuleRejected(String, String),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

// ─────────────────────────────────────────────────────────── 行与载荷

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ProductProfile {
    pub id: Uuid,
    pub team_id: Uuid,
    /// 稳定业务标识（PA-07 §4.8）：全局唯一、创建时自动生成、创建后不可变；
    /// 仅用于 UI/文档/AI-PM/MCP 引用，一切关联仍走 id（key 不作外键）。
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ProductMemberRow {
    pub principal_id: Uuid,
    pub display_name: String,
    pub role: String,
    pub is_active: bool,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ProductLinkRow {
    pub id: Uuid,
    pub product_id: Uuid,
    pub project_id: Uuid,
    pub created_at: chrono::DateTime<Utc>,
}

fn role_of(raw: Option<&str>) -> Option<&'static str> {
    match raw {
        Some("owner") => Some("owner"),
        Some("member") => Some("member"),
        _ => None,
    }
}

/// 审计（governance.tenant_audit_log 管线；target_type="product"/"product_member"）。
#[allow(clippy::too_many_arguments)] // 审计行字段齐写
async fn audit(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    tenant_id: Uuid,
    actor: &AuditActor,
    action: &str,
    target_type: &str,
    target_id: Uuid,
    before: Option<serde_json::Value>,
    after: Option<serde_json::Value>,
) -> Result<(), ProductError> {
    let mut map = serde_json::Map::new();
    if let Some(serde_json::Value::Object(b)) = &before {
        for (k, v) in b {
            map.insert(format!("before_{k}"), v.clone());
        }
    }
    if let Some(serde_json::Value::Object(a)) = &after {
        for (k, v) in a {
            map.insert(k.clone(), v.clone());
        }
    }
    let extra = if map.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(map))
    };
    sqlx::query(
        "INSERT INTO governance.tenant_audit_log \
         (tenant_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
          action, target_type, target_id, target_principal_id, before_data, after_data) \
         VALUES ($1, $2, $3, $4, $5, NULL, $6, $7, $8, NULL, $9, $10)",
    )
    .bind(tenant_id)
    .bind(actor.principal_id)
    .bind(actor.identity_id)
    .bind(&actor.client)
    .bind(actor.auth_mode.clone())
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(before)
    .bind(extra)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

// ─────────────────────────────────────────────────────────── 创建

/// 创建 Product（Team Owner 独占；PA §2：Team Owner 指定首位 Product Owner，
/// 创建者/Team Owner 均不因组织身份自动成为 Product Owner）。
/// 同事务：product 行 + product_member owner 行 + 审计。
/// Product Key 基名派生（PA-07 §4.8）：name 中非 `[A-Za-z0-9]` 连续序列折叠
/// 为单个 `-`，去首尾 `-` 后大写，截断 64。返回基名可能不足 2 字符（如 name
/// 不含字母数字），由调用方以 id 兜底。
fn derive_product_key_base(name: &str) -> String {
    let collapsed: String = name
        .to_uppercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let base = collapsed
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    base.chars().take(64).collect()
}

/// 生成创建用 Product Key：与既有 key 大小写不敏感冲突时追加 `-2`、`-3`…
/// （茎干截断 55 留足后缀空间，保持格式合法）。基名（含 `P-` + id 兜底形）
/// 由调用方保证 ≥2 字符；`lower(key)` 唯一索引兜住并发竞态，INSERT 层映射
/// 为 `Conflict`。
async fn allocate_product_key(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    base: &str,
) -> Result<String, ProductError> {
    let mut candidate = base.to_string();
    let mut suffix: u32 = 1;
    loop {
        let taken: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM core.product WHERE lower(key) = lower($1))",
        )
        .bind(&candidate)
        .fetch_one(&mut **tx)
        .await?;
        if !taken {
            return Ok(candidate);
        }
        suffix += 1;
        if suffix > 1000 {
            return Err(ProductError::Conflict("product key unavailable"));
        }
        let stem = base
            .trim_end_matches('-')
            .chars()
            .take(55)
            .collect::<String>()
            .trim_end_matches('-')
            .to_string();
        if stem.chars().count() < 2 {
            return Err(ProductError::Conflict("product key unavailable"));
        }
        candidate = format!("{stem}-{suffix}");
    }
}

pub async fn create_product(
    pool: &PgPool,
    actor: &AuditActor,
    actor_principal_id: Uuid,
    team_id: Uuid,
    name: &str,
    description: Option<&str>,
    first_owner_principal_id: Uuid,
) -> Result<ProductProfile, ProductError> {
    let name = name.trim();
    if name.is_empty() || name.len() > 120 {
        return Err(ProductError::Validation("invalid product name"));
    }
    let mut tx = pool.begin().await?;

    // Team 锚与调用者身份（Team Owner 独占）。
    let team: Option<(Uuid,)> = sqlx::query_as(
        "SELECT tenant_id FROM core.team WHERE id = $1 FOR UPDATE",
    )
    .bind(team_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((tenant_id,)) = team else {
        return Err(ProductError::NotFound);
    };
    let caller_role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM core.team_member WHERE team_id = $1 AND principal_id = $2",
    )
    .bind(team_id)
    .bind(actor_principal_id)
    .fetch_optional(&mut *tx)
    .await?;
    if caller_role.as_deref() != Some("owner") {
        return Err(ProductError::Forbidden("team owner required"));
    }

    // 首位 Owner 必须是本 Tenant 的 active principal（OPEN-4：范围第一轮限本 Tenant）。
    let owner_ok: Option<bool> = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM core.principal \
          WHERE id = $1 AND is_active AND EXISTS (\
               SELECT 1 FROM core.tenant_member tm WHERE tm.tenant_id = $2 AND tm.principal_id = core.principal.id))",
    )
    .bind(first_owner_principal_id)
    .bind(tenant_id)
    .fetch_one(&mut *tx)
    .await?;
    if owner_ok != Some(true) {
        return Err(ProductError::Validation(
            "first owner must be an active principal of this tenant",
        ));
    }

    // Product Key（PA-07 §4.8）：服务层从 name 派生自动生成，创建后不可变；
    // 基名不足 2 字符（name 无字母数字）时以 `P-` + id 前 8 位十六进制兜底。
    let product_id = Uuid::new_v4();
    let mut base = derive_product_key_base(name);
    if base.chars().count() < 2 {
        base = format!("P-{}", &product_id.to_string()[..8].to_uppercase());
    }
    let product_key = allocate_product_key(&mut tx, &base).await?;

    let inserted: (Uuid, DateTime<Utc>) = sqlx::query_as(
        "INSERT INTO core.product (id, team_id, tenant_id, key, name, description) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id, updated_at",
    )
    .bind(product_id)
    .bind(team_id)
    .bind(tenant_id)
    .bind(&product_key)
    .bind(name)
    .bind(description.map(str::trim).filter(|s| !s.is_empty()))
    .fetch_one(&mut *tx)
    .await?;
    debug_assert_eq!(inserted.0, product_id);
    let updated_at = inserted.1;

    sqlx::query(
        "INSERT INTO core.product_member (product_id, principal_id, tenant_id, role, granted_by) \
         VALUES ($1, $2, $3, 'owner', $4)",
    )
    .bind(product_id)
    .bind(first_owner_principal_id)
    .bind(tenant_id)
    .bind(actor_principal_id)
    .execute(&mut *tx)
    .await?;

    audit(
        &mut tx,
        tenant_id,
        actor,
        "product_member.added",
        "product_member",
        first_owner_principal_id,
        None,
        Some(json!({ "role": "owner", "via": "product_created", "product_id": product_id })),
    )
    .await?;
    audit(
        &mut tx,
        tenant_id,
        actor,
        "product.created",
        "product",
        product_id,
        None,
        Some(json!({
            "name": name,
            "key": product_key,
            "team_id": team_id,
            "first_owner": first_owner_principal_id
        })),
    )
    .await?;

    tx.commit().await?;
    Ok(ProductProfile {
        id: product_id,
        team_id,
        key: product_key,
        name: name.to_string(),
        description: description.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string),
        status: "normal".to_string(),
        updated_at,
    })
}

// ─────────────────────────────────────────────────────────── 读

/// Product 列表：Team Owner 全量；普通 Team Member 仅自己参与的（PA §6
/// 信息路由思想——Membership 决定关注范围）。
pub async fn list_products(
    pool: &PgPool,
    team_id: Uuid,
    principal_id: Uuid,
) -> Result<Vec<ProductProfile>, ProductError> {
    let caller_role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM core.team_member WHERE team_id = $1 AND principal_id = $2",
    )
    .bind(team_id)
    .bind(principal_id)
    .fetch_optional(pool)
    .await?;
    let Some(role) = caller_role.as_deref() else {
        return Err(ProductError::NotFound);
    };
    if role == "owner" {
        let rows: Vec<ProductProfile> = sqlx::query_as(
            "SELECT id, team_id, key, name, description, status, updated_at FROM core.product \
              WHERE team_id = $1 ORDER BY created_at",
        )
        .bind(team_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    } else {
        let rows: Vec<ProductProfile> = sqlx::query_as(
            "SELECT p.id, p.team_id, p.key, p.name, p.description, p.status, p.updated_at \
               FROM core.product p \
               JOIN core.product_member m ON m.product_id = p.id \
              WHERE p.team_id = $1 AND m.principal_id = $2 \
              ORDER BY p.created_at",
        )
        .bind(team_id)
        .bind(principal_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
}

/// Product 档案（Product Member 可读）。
pub async fn product_profile(
    pool: &PgPool,
    product_id: Uuid,
    principal_id: Uuid,
) -> Result<ProductProfile, ProductError> {
    let row: Option<ProductProfile> = sqlx::query_as(
        "SELECT p.id, p.team_id, p.key, p.name, p.description, p.status, p.updated_at \
           FROM core.product p \
          WHERE p.id = $1 AND EXISTS (\
               SELECT 1 FROM core.product_member m \
                WHERE m.product_id = p.id AND m.principal_id = $2)",
    )
    .bind(product_id)
    .bind(principal_id)
    .fetch_optional(pool)
    .await?;
    row.ok_or(ProductError::NotFound)
}

/// 名册（Product Member 可读）。
pub async fn list_product_members(
    pool: &PgPool,
    product_id: Uuid,
    principal_id: Uuid,
) -> Result<Vec<ProductMemberRow>, ProductError> {
    let is_member: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM core.product_member \
          WHERE product_id = $1 AND principal_id = $2)",
    )
    .bind(product_id)
    .bind(principal_id)
    .fetch_one(pool)
    .await?;
    if !is_member {
        return Err(ProductError::NotFound);
    }
    let rows: Vec<ProductMemberRow> = sqlx::query_as(
        "SELECT m.principal_id, p.display_name, m.role, p.is_active, m.created_at \
           FROM core.product_member m \
           JOIN core.principal p ON p.id = m.principal_id \
          WHERE m.product_id = $1 \
          ORDER BY CASE m.role WHEN 'owner' THEN 0 ELSE 1 END, m.created_at",
    )
    .bind(product_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ─────────────────────────────────────────────────────────── 成员变更

// ─────────────────────────────────────────────────────────── 成员变更

/// 成员变更 DML（锁内裁决产出）。
enum ProductWrite {
    Add { role: &'static str, granted_by: Uuid },
    SetRole { role: &'static str },
    Remove,
}

struct ProductAuditLine {
    action: &'static str,
    before_role: Option<&'static str>,
    after: Option<AfterRole>,
}

struct AfterRole {
    role: &'static str,
    via: &'static str,
}

/// 锁内快照（apply_member_change 第 1-2 步读出）。
struct ChangeSnapshot {
    actor_role: Option<&'static str>,
    target_role: Option<&'static str>,
    owner_count: i64,
    /// 调用者是否为该 Product 所属 Team 的 Team Owner（锁内预取）。
    actor_is_team_owner: bool,
    tenant_id: Uuid,
}

/// 规则拒绝：稳定 machine message 承载在 Forbidden（HTTP 层按 code 分流 403/409）。
fn rule(code: &str, message: &str) -> (String, String) {
    (code.to_string(), message.to_string())
}

/// 锁内裁决回调：快照 → (DML, 审计行)；Err = (错误码, 稳定消息) 整体回滚。
type PlanFn<'a> = &'a (dyn Fn(&ChangeSnapshot)
    -> Result<(Vec<ProductWrite>, Vec<ProductAuditLine>), (String, String)>
    + Send
    + Sync);

/// 变更事务管线（对齐 Team 面锁序）：锁 product 行（同 Product 变更串行化）
/// → 锁调用者/目标 product_member 行 + owner 计数 + Team Owner 标志 → `plan`
/// 锁内裁决 → DML + 审计。`plan` Err(code, message) 整体回滚。
async fn apply_member_change(
    pool: &PgPool,
    actor: &AuditActor,
    actor_principal_id: Uuid,
    product_id: Uuid,
    target_id: Uuid,
    plan: PlanFn<'_>,
) -> Result<(), ProductError> {
    let mut tx = pool.begin().await?;

    // 1) 锁 product 行（顺带取 tenant 锚与存在性）。
    let locked: Option<(Uuid,)> =
        sqlx::query_as("SELECT tenant_id FROM core.product WHERE id = $1 FOR UPDATE")
            .bind(product_id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some((tenant_id,)) = locked else {
        return Err(ProductError::NotFound);
    };

    // 2) 调用者/目标 role + owner 计数 + Team Owner 标志（锁内）。
    let actor_role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM core.product_member \
          WHERE product_id = $1 AND principal_id = $2 FOR UPDATE",
    )
    .bind(product_id)
    .bind(actor_principal_id)
    .fetch_optional(&mut *tx)
    .await?;
    let target_role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM core.product_member \
          WHERE product_id = $1 AND principal_id = $2 FOR UPDATE",
    )
    .bind(product_id)
    .bind(target_id)
    .fetch_optional(&mut *tx)
    .await?;
    let owner_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.product_member WHERE product_id = $1 AND role = 'owner'",
    )
    .bind(product_id)
    .fetch_one(&mut *tx)
    .await?;
    let actor_is_team_owner: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM core.team_member tm \
           JOIN core.team t ON t.id = tm.team_id \
          WHERE t.id = (SELECT team_id FROM core.product WHERE id = $1) \
            AND tm.principal_id = $2 AND tm.role = 'owner')",
    )
    .bind(product_id)
    .bind(actor_principal_id)
    .fetch_one(&mut *tx)
    .await?;

    let snapshot = ChangeSnapshot {
        actor_role: role_of(actor_role.as_deref()),
        target_role: role_of(target_role.as_deref()),
        owner_count,
        actor_is_team_owner,
        tenant_id,
    };

    let (writes, audits) =
        plan(&snapshot).map_err(|(c, m)| rule_rejected(c, m))?;

    // 3) DML。
    for w in writes {
        match w {
            ProductWrite::Add { role, granted_by } => {
                sqlx::query(
                    "INSERT INTO core.product_member \
                     (product_id, principal_id, tenant_id, role, granted_by) \
                     VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(product_id)
                .bind(target_id)
                .bind(tenant_id)
                .bind(role)
                .bind(granted_by)
                .execute(&mut *tx)
                .await?;
            }
            ProductWrite::SetRole { role } => {
                sqlx::query(
                    "UPDATE core.product_member SET role = $3, updated_at = now() \
                      WHERE product_id = $1 AND principal_id = $2",
                )
                .bind(product_id)
                .bind(target_id)
                .bind(role)
                .execute(&mut *tx)
                .await?;
            }
            ProductWrite::Remove => {
                sqlx::query(
                    "DELETE FROM core.product_member WHERE product_id = $1 AND principal_id = $2",
                )
                .bind(product_id)
                .bind(target_id)
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    // 4) 审计。
    for line in &audits {
        audit(
            &mut tx,
            snapshot.tenant_id,
            actor,
            line.action,
            "product_member",
            target_id,
            line.before_role.map(|r| json!({ "role": r })),
            line.after
                .as_ref()
                .map(|a| json!({ "role": a.role, "via": a.via })),
        )
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// 规则拒绝 → 域错误（HTTP 层按 code 前缀分流状态码）。
fn rule_rejected(code: String, message: String) -> ProductError {
    ProductError::RuleRejected(code, message)
}

/// 添加成员（PA §3：所有 Product Member 可添加普通成员；PA §8-4 OPEN：范围
/// 第一轮限本 Tenant active principal）。无 Owner 态下 Team Owner 可恢复性
/// 任命 Owner（PA §4）。
pub async fn add_product_member(
    pool: &PgPool,
    actor: &AuditActor,
    actor_principal_id: Uuid,
    product_id: Uuid,
    principal_id: Uuid,
) -> Result<(), ProductError> {
    apply_member_change(pool, actor, actor_principal_id, product_id, principal_id, &|s| {
        if s.actor_role.is_none() && !s.actor_is_team_owner {
            return Err(rule("not_a_product_member", "not a product member"));
        }
        if s.target_role.is_some() {
            return Err(rule(
                "already_product_member",
                "principal is already a product member",
            ));
        }
        // 恢复性任命（PA §4）：无 Owner 态 + Team Owner → 直接落 Owner 行。
        if s.owner_count == 0 && s.actor_is_team_owner {
            return Ok((
                vec![ProductWrite::Add {
                    role: "owner",
                    granted_by: actor_principal_id,
                }],
                vec![ProductAuditLine {
                    action: "product_member.role_changed",
                    before_role: None,
                    after: Some(AfterRole { role: "owner", via: "team_owner_restore" }),
                }],
            ));
        }
        Ok((
            vec![ProductWrite::Add {
                role: "member",
                granted_by: actor_principal_id,
            }],
            vec![ProductAuditLine {
                action: "product_member.added",
                before_role: None,
                after: Some(AfterRole { role: "member", via: "member_added" }),
            }],
        ))
    })
    .await
}

/// 移除成员（PA §3：所有 Product Member 可移除普通成员；PA §4：不可直接移除
/// 最后一位 Owner；移除 Owner 归 OPEN-3——第一轮仅 Team Owner 可）。
pub async fn remove_product_member(
    pool: &PgPool,
    actor: &AuditActor,
    actor_principal_id: Uuid,
    product_id: Uuid,
    target_id: Uuid,
) -> Result<(), ProductError> {
    apply_member_change(pool, actor, actor_principal_id, product_id, target_id, &|s| {
        let Some(target) = s.target_role else {
            return Err(rule("product_member_not_found", "product member not found"));
        };
        if target == "owner" {
            // OPEN-3：Owner 移除第一轮仅 Team Owner。
            if !s.actor_is_team_owner {
                return Err(rule(
                    "team_owner_required",
                    "removing a product owner requires the team owner",
                ));
            }
            if s.owner_count <= 1 {
                return Err(rule("last_owner", "the last owner cannot be removed"));
            }
        }
        Ok((
            vec![ProductWrite::Remove],
            vec![ProductAuditLine {
                action: "product_member.removed",
                before_role: Some(target),
                after: None,
            }],
        ))
    })
    .await
}

/// 任命 Product Owner（继任指定，PA §4：更换末位 Owner 先指定继任者）。
/// 任一有效 Product Owner 可任命 Member 为 Owner（多 Owner 并存）；无 Owner
/// 态由 Team Owner 恢复性任命。OPEN-3：Member 任命/降级 Owner 不实现。
pub async fn appoint_product_owner(
    pool: &PgPool,
    actor: &AuditActor,
    actor_principal_id: Uuid,
    product_id: Uuid,
    target_id: Uuid,
) -> Result<(), ProductError> {
    apply_member_change(pool, actor, actor_principal_id, product_id, target_id, &|s| {
        let authorized = s.actor_role == Some("owner")
            || (s.owner_count == 0 && s.actor_is_team_owner);
        if !authorized {
            return Err(rule(
                "product_owner_required",
                "appointing an owner requires a product owner (or team owner to restore)",
            ));
        }
        let Some("member") = s.target_role else {
            if s.target_role == Some("owner") {
                return Err(rule("already_owner", "target is already an owner"));
            }
            return Err(rule("product_member_not_found", "product member not found"));
        };
        if target_id == actor_principal_id && s.actor_role == Some("owner") {
            return Err(rule("already_owner", "caller is already an owner"));
        }
        Ok((
            vec![ProductWrite::SetRole { role: "owner" }],
            vec![ProductAuditLine {
                action: "product_member.role_changed",
                before_role: Some("member"),
                after: Some(AfterRole { role: "owner", via: "succession_appointment" }),
            }],
        ))
    })
    .await
}

/// 降级 Product Owner（第一轮仅 Team Owner；末位 Owner 不可降级）。OPEN-3。
pub async fn demote_product_owner(
    pool: &PgPool,
    actor: &AuditActor,
    actor_principal_id: Uuid,
    product_id: Uuid,
    target_id: Uuid,
) -> Result<(), ProductError> {
    apply_member_change(pool, actor, actor_principal_id, product_id, target_id, &|s| {
        // 权限检查先行（不给无权者泄露目标状态）。
        if !s.actor_is_team_owner {
            return Err(rule(
                "team_owner_required",
                "demoting a product owner requires the team owner",
            ));
        }
        let Some("owner") = s.target_role else {
            if s.target_role.is_some() {
                return Err(rule("already_member", "target is already a member"));
            }
            return Err(rule("product_member_not_found", "product member not found"));
        };
        if s.owner_count <= 1 {
            return Err(rule("last_owner", "the last owner cannot be demoted"));
        }
        Ok((
            vec![ProductWrite::SetRole { role: "member" }],
            vec![ProductAuditLine {
                action: "product_member.role_changed",
                before_role: Some("owner"),
                after: Some(AfterRole { role: "member", via: "team_owner_demote" }),
            }],
        ))
    })
    .await
}

// ─────────────────────────────────────────────────────────── 生命周期

/// Product 生命周期转换（PA §5；DR-PA-03）：任意有效 Product Owner 可执行。
/// abandon 仅 active→abandoned；restart 仅 abandoned→active；不触碰关联
/// Project 与在飞 Agent Run（生命周期独立）。
pub async fn set_product_lifecycle(
    pool: &PgPool,
    actor: &AuditActor,
    actor_principal_id: Uuid,
    product_id: Uuid,
    action: &str,
) -> Result<ProductProfile, ProductError> {
    // PA07-IMP-004（0044 四值化）：abandon→archived 终态（PA-07 §12 不提供
    // 常规恢复）；suspend/resume/complete_resume 承接 §11 的暂停-评估-恢复
    // 三段；restart 语义收敛为 suspended/resuming → normal 的再激活。
    let target_status = match action {
        "abandon" => "archived",
        "suspend" => "suspended",
        "resume" => "resuming",
        "restart" | "complete_resume" => "normal",
        _ => {
            return Err(ProductError::Validation(
                "action must be abandon, suspend, resume, complete_resume or restart",
            ))
        }
    };
    let mut tx = pool.begin().await?;

    let locked: Option<(Uuid, String, String, Option<String>, Uuid, String, DateTime<Utc>)> =
        sqlx::query_as(
            "SELECT tenant_id, name, status, description, team_id, key, updated_at \
               FROM core.product WHERE id = $1 FOR UPDATE",
        )
        .bind(product_id)
        .fetch_optional(&mut *tx)
        .await?;
    let Some((tenant_id, name, status, description, team_id, product_key, updated_at)) = locked
    else {
        return Err(ProductError::NotFound);
    };
    let is_owner: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM core.product_member \
          WHERE product_id = $1 AND principal_id = $2 AND role = 'owner')",
    )
    .bind(product_id)
    .bind(actor_principal_id)
    .fetch_one(&mut *tx)
    .await?;
    if !is_owner {
        return Err(ProductError::Forbidden("product_owner_required"));
    }
    if status == target_status {
        return Err(ProductError::Conflict("product_lifecycle_noop"));
    }

    sqlx::query("UPDATE core.product SET status = $2, updated_at = now() WHERE id = $1")
        .bind(product_id)
        .bind(target_status)
        .execute(&mut *tx)
        .await?;

    audit(
        &mut tx,
        tenant_id,
        actor,
        "product.lifecycle_changed",
        "product",
        product_id,
        Some(json!({ "status": status })),
        Some(json!({ "status": target_status, "action": action })),
    )
    .await?;

    tx.commit().await?;
    Ok(ProductProfile {
        id: product_id,
        team_id,
        key: product_key,
        name,
        description,
        status: target_status.to_string(),
        updated_at,
    })
}

// ─────────────────────────────────────────────────────────── 写入门禁

/// Product 业务写入闸（DR-PA-03④，对齐 project_gate 模式）：业务不活跃时
/// fail-closed。第一轮消费点 = Product-Project Link 建立与 Product 域业务
/// 写入。PA07-IMP-004 四值化后业务活跃 = `normal`（resuming 逐步开放、
/// suspended/archived 一律拒新关联）；对外错误码沿用 `product_abandoned`
/// （含义泛化为「Product 不在可承接业务活动的状态」）。
pub async fn assert_product_active(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    product_id: Uuid,
) -> Result<(), ProductError> {
    let status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM core.product WHERE id = $1 FOR UPDATE",
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?;
    match status.as_deref() {
        Some("normal") => Ok(()),
        Some(_) => Err(ProductError::Conflict("product_abandoned")),
        None => Err(ProductError::NotFound),
    }
}

/// 事务外只读版（无锁）。
pub async fn product_is_active(pool: &PgPool, product_id: Uuid) -> Result<bool, ProductError> {
    let status: Option<String> =
        sqlx::query_scalar("SELECT status FROM core.product WHERE id = $1")
            .bind(product_id)
            .fetch_optional(pool)
            .await?;
    Ok(status.as_deref() == Some("normal"))
}

// ─────────────────────────────────────────────────────────── Product↔Project Link

/// 建立 Link（DR-PA-03④ 第一消费点：product abandoned → 拒绝）。
/// 写权（第一轮）：Team Owner ∪ Product Owner。
pub async fn create_product_link(
    pool: &PgPool,
    actor: &AuditActor,
    actor_principal_id: Uuid,
    product_id: Uuid,
    project_id: Uuid,
) -> Result<Uuid, ProductError> {
    let mut tx = pool.begin().await?;

    let product: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT tenant_id, status FROM core.product WHERE id = $1 FOR UPDATE",
    )
    .bind(product_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((tenant_id, status)) = product else {
        return Err(ProductError::NotFound);
    };
    // PA07-IMP-004 四值化：新 Link 只在 normal 态可建（与 assert_product_active
    // 同口径；错误码沿用 product_abandoned）。
    if status != "normal" {
        return Err(ProductError::Conflict("product_abandoned"));
    }

    // project 存在性（同租户）。
    let project_ok: Option<bool> = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM core.project WHERE id = $1 AND tenant_id = $2)",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_one(&mut *tx)
    .await?;
    if project_ok != Some(true) {
        return Err(ProductError::NotFound);
    }

    // 写权：Team Owner ∪ Product Owner。
    let team_owner: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM core.team_member tm \
           JOIN core.team t ON t.id = tm.team_id \
          WHERE t.id = (SELECT team_id FROM core.product WHERE id = $1) \
            AND tm.principal_id = $2 AND tm.role = 'owner')",
    )
    .bind(product_id)
    .bind(actor_principal_id)
    .fetch_one(&mut *tx)
    .await?;
    let product_owner: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM core.product_member \
          WHERE product_id = $1 AND principal_id = $2 AND role = 'owner')",
    )
    .bind(product_id)
    .bind(actor_principal_id)
    .fetch_one(&mut *tx)
    .await?;
    if !team_owner && !product_owner {
        return Err(ProductError::Forbidden("product_owner_required"));
    }

    let link_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.product_project_link (product_id, project_id, tenant_id, created_by) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (product_id, project_id) WHERE withdrawn_at IS NULL          DO NOTHING RETURNING id",
    )
    .bind(product_id)
    .bind(project_id)
    .bind(tenant_id)
    .bind(actor_principal_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ProductError::Conflict("link_already_exists"))?;

    audit(
        &mut tx,
        tenant_id,
        actor,
        "product_link.created",
        "product_project_link",
        link_id,
        None,
        Some(json!({ "product_id": product_id, "project_id": project_id })),
    )
    .await?;

    tx.commit().await?;
    Ok(link_id)
}

/// 解除 Link（写权同建立）。
pub async fn end_product_link(
    pool: &PgPool,
    actor: &AuditActor,
    actor_principal_id: Uuid,
    link_id: Uuid,
) -> Result<(), ProductError> {
    let mut tx = pool.begin().await?;
    let row: Option<(Uuid, Uuid, Uuid)> = sqlx::query_as(
        "SELECT l.product_id, l.tenant_id, \
                (SELECT team_id FROM core.product WHERE id = l.product_id) \
           FROM core.product_project_link l WHERE l.id = $1 FOR UPDATE",
    )
    .bind(link_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((product_id, tenant_id, team_id)) = row else {
        return Err(ProductError::NotFound);
    };
    let team_owner: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM core.team_member \
          WHERE team_id = $1 AND principal_id = $2 AND role = 'owner')",
    )
    .bind(team_id)
    .bind(actor_principal_id)
    .fetch_one(&mut *tx)
    .await?;
    let product_owner: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM core.product_member \
          WHERE product_id = $1 AND principal_id = $2 AND role = 'owner')",
    )
    .bind(product_id)
    .bind(actor_principal_id)
    .fetch_one(&mut *tx)
    .await?;
    if !team_owner && !product_owner {
        return Err(ProductError::Forbidden("product_owner_required"));
    }
    // PA07-IMP-007：解除 Link 改软删（历史可追溯，与 task_product 同模式）；
    // 已撤回的行再 end → NotFound（读面不再展示）。
    let withdrawn = sqlx::query(
        "UPDATE core.product_project_link             SET withdrawn_at = now(), withdrawn_by = $2           WHERE id = $1 AND withdrawn_at IS NULL",
    )
    .bind(link_id)
    .bind(actor_principal_id)
    .execute(&mut *tx)
    .await?;
    if withdrawn.rows_affected() == 0 {
        return Err(ProductError::NotFound);
    }
    audit(
        &mut tx,
        tenant_id,
        actor,
        "product_link.ended",
        "product_project_link",
        link_id,
        None,
        Some(json!({ "product_id": product_id })),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Product 的 Link 列表（Product Member 可读）。
pub async fn list_product_links(
    pool: &PgPool,
    product_id: Uuid,
    principal_id: Uuid,
) -> Result<Vec<ProductLinkRow>, ProductError> {
    let is_member: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM core.product_member \
          WHERE product_id = $1 AND principal_id = $2)",
    )
    .bind(product_id)
    .bind(principal_id)
    .fetch_one(pool)
    .await?;
    if !is_member {
        return Err(ProductError::NotFound);
    }
    let rows: Vec<ProductLinkRow> = sqlx::query_as(
        "SELECT id, product_id, project_id, created_at \
           FROM core.product_project_link \
          WHERE product_id = $1 AND withdrawn_at IS NULL ORDER BY created_at",
    )
    .bind(product_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ───────────────────────────── PA-07 项目级产品读面（Figma 179:3603 E 系列）

/// 项目关联产品行（`GET /v1/pm/projects/{project_id}/products`）：
/// core.product_project_link 有效行 → 产品 profile + link 元数据 + 关联任务计数。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ProjectProductRow {
    pub link_id: Uuid,
    pub linked_at: chrono::DateTime<Utc>,
    #[sqlx(flatten)]
    pub product: ProductProfile,
    /// 该产品在本项目内的活跃关联任务数（task_product 反查，withdrawn 不含）。
    pub task_count: i64,
}

/// 项目 → 产品方向读面（此前只有 product→project 方向）。
/// 可见性：项目成员可见（产品经由 link/任务对项目成员天然可见；
/// Team Owner / Product Member 管理面走既有 team/products 端点，不在此列）。
pub async fn list_project_products(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
) -> Result<Vec<ProjectProductRow>, ProductError> {
    let visible: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM core.project WHERE id = $1 AND tenant_id = $2 \
          AND EXISTS (SELECT 1 FROM core.project_member m \
                       WHERE m.project_id = core.project.id AND m.principal_id = $3)",
    )
    .bind(project_id)
    .bind(tenant_id)
    .bind(principal_id)
    .fetch_optional(pool)
    .await?;
    if visible.is_none() {
        return Err(ProductError::NotFound);
    }
    let rows: Vec<ProjectProductRow> = sqlx::query_as(
        "SELECT l.id AS link_id, l.created_at AS linked_at, \
                p.id, p.team_id, p.key, p.name, p.description, p.status, p.updated_at, \
                (SELECT count(*) FROM delivery.task_product tp \
                   JOIN delivery.task tt ON tt.id = tp.task_id \
                  WHERE tp.product_id = p.id AND tp.withdrawn_at IS NULL \
                    AND tt.project_id = $1) AS task_count \
           FROM core.product_project_link l \
           JOIN core.product p ON p.id = l.product_id \
          WHERE l.project_id = $1 AND l.withdrawn_at IS NULL \
          ORDER BY p.name, p.id",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 产品关联任务行（`GET /v1/pm/products/{product_id}/tasks`，E 页 Tasks tab /
/// Impact Summary 数据源）：task_product 活跃关联 → 任务 + PA-07 调度原料。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ProductTaskRow {
    pub id: Uuid,
    pub code: String,
    pub project_id: Uuid,
    pub title: String,
    pub status: String,
    pub priority: String,
    pub planned_end_at: Option<chrono::DateTime<Utc>>,
    pub deps_total: i64,
    pub deps_met: i64,
    pub created_at: chrono::DateTime<Utc>,
}

/// product → tasks 反查。可见性：Product Member（与 product_profile 同口径）。
pub async fn list_product_tasks(
    pool: &PgPool,
    product_id: Uuid,
    principal_id: Uuid,
) -> Result<Vec<ProductTaskRow>, ProductError> {
    let is_member: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM core.product_member \
          WHERE product_id = $1 AND principal_id = $2)",
    )
    .bind(product_id)
    .bind(principal_id)
    .fetch_one(pool)
    .await?;
    if !is_member {
        return Err(ProductError::NotFound);
    }
    let rows: Vec<ProductTaskRow> = sqlx::query_as(&format!(
        "SELECT t.id, t.code, t.project_id, t.title, t.status, t.priority, \
                t.planned_end_at, {}, t.created_at \
           FROM delivery.task_product tp \
           JOIN delivery.task t ON t.id = tp.task_id \
          WHERE tp.product_id = $1 AND tp.withdrawn_at IS NULL \
          ORDER BY CASE t.priority WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 \
                                   WHEN 'normal' THEN 2 ELSE 3 END, \
                   t.created_at DESC",
        crate::rows::TASK_AGGREGATE_COLUMNS
    ))
    .bind(product_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
