//! S02-I5 · Acceptance（需求级验收）统一服务。
//!
//! 契约 = S02-04 V1.2 §11（Q-11~Q-14 / S02D-09~12）：
//! - 载体 `delivery.requirement_acceptance_criterion` 锚 requirement_id
//!   （0056；验收语义属需求级治理内容，不绑修订；任务级验收链 0015 不动）；
//! - 增删改 = 独立受控命令：真人主体（§7.2 A*）+ 治理授权（Lead OR 显式
//!   Approver，复用 0040 `ensure_governance_authorized`）+ 同事务审计；
//! - verification_kind 扩枚举 + params JSONB 承载（Q-13；形状应用层校验，
//!   V1 仅要求 object——族内 required keys 待消费方定义后收紧）；
//! - 需求级验收裁决入口 `record_criterion_verdict`（Q-14：不挂 Task 链，
//!   即 S02 向 S04/S05 声明的接口）；
//! - `criteria_drift_since`：确认时一致性提示的数据面（Q-12——验收项在
//!   待确认修订提交之后又被修改 = 漂移，审批人应复核）。
//!
//! 成员可见性闸由 REST 适配层 `require_requirement_visible` 把门（I1 分层）。

use sqlx::PgPool;
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::pm_write::{append_audit, PmWriteError};
use crate::requirement::require_human;

/// verification_kind 词表（Q-13 扩枚举：前四值对齐 0015 任务级既有枚举，
/// performance/security 为 I5 扩充验证族；扩值须同步 0056 CHECK 约束）。
pub const VERIFICATION_KINDS: &[&str] = &[
    "manual",
    "automated_test",
    "artifact_exists",
    "review",
    "performance",
    "security",
];

const MAX_STATEMENT_LEN: usize = 2000;
const MAX_PARAMS_BYTES: usize = 16 * 1024;

// ─────────────────────────────────────────────────────────────── 视图

/// 验收标准条目（列表/裁决回显共用）。
#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct CriterionView {
    pub id: Uuid,
    pub requirement_id: Uuid,
    pub position: i32,
    pub statement: String,
    pub verification_kind: String,
    pub params: serde_json::Value,
    pub established_in_revision_id: Option<Uuid>,
    pub verdict: String,
    pub verdict_note: Option<String>,
    pub verdict_by: Option<Uuid>,
    pub verdict_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

// ─────────────────────────────────────────────────────────────── 请求

#[derive(Debug, serde::Deserialize)]
pub struct AddCriterionRequest {
    pub statement: String,
    #[serde(default)]
    pub verification_kind: Option<String>,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
    /// 追加位置（缺省 = 尾部 max(position)+1）。
    #[serde(default)]
    pub position: Option<i32>,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateCriterionRequest {
    #[serde(default)]
    pub statement: Option<String>,
    #[serde(default)]
    pub verification_kind: Option<String>,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
    #[serde(default)]
    pub position: Option<i32>,
}

/// 需求级验收裁决（Q-14；S04/S05 声明接口的请求面）。
#[derive(Debug, serde::Deserialize)]
pub struct CriterionVerdictRequest {
    pub verdict: String,
    #[serde(default)]
    pub note: Option<String>,
}

// ─────────────────────────────────────────────────────── 内部助手

/// 治理命令公共前置：真人（§7.2）+ 需求存在且 active（FOR UPDATE 锁）。
/// 返回 (project_id)。成员可见性闸由 REST 层把门；治理授权由各命令在
/// 事务内调 `ensure_governance_authorized`（与 confirm/reject 同型）。
async fn lock_active_requirement(
    tx: &mut sqlx::PgConnection,
    tenant_id: Uuid,
    requirement_id: Uuid,
) -> Result<Uuid, PmWriteError> {
    let row: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT project_id, lifecycle_status FROM delivery.requirement \
         WHERE id = $1 AND tenant_id = $2 FOR UPDATE",
    )
    .bind(requirement_id)
    .bind(tenant_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    let (project_id, lifecycle) = row.ok_or(PmWriteError::NotFound("requirement not found"))?;
    if lifecycle != "active" {
        return Err(PmWriteError::Conflict(
            "requirement is not active (withdrawn/archived)",
        ));
    }
    Ok(project_id)
}

/// 校验并归一 verification_kind（缺省 manual，Q-13）。
fn validate_kind(kind: Option<&str>) -> Result<String, PmWriteError> {
    match kind {
        None => Ok("manual".to_string()),
        Some(raw) => {
            let kind = raw.trim();
            if !VERIFICATION_KINDS.contains(&kind) {
                return Err(PmWriteError::Validation("invalid verification_kind"));
            }
            Ok(kind.to_string())
        }
    }
}

/// params 形状校验（Q-13：应用层按形状校验；V1 = 必须 object 且有界）。
fn validate_params(params: Option<&serde_json::Value>) -> Result<serde_json::Value, PmWriteError> {
    match params {
        None => Ok(serde_json::json!({})),
        Some(value) => {
            if !value.is_object() {
                return Err(PmWriteError::Validation("params must be a JSON object"));
            }
            let serialized = serde_json::to_string(value)
                .map_err(|_| PmWriteError::Validation("params is not valid JSON"))?;
            if serialized.len() > MAX_PARAMS_BYTES {
                return Err(PmWriteError::Validation("params exceeds size limit"));
            }
            Ok(value.clone())
        }
    }
}

fn validate_statement(statement: Option<&str>) -> Result<String, PmWriteError> {
    let statement = statement
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or(PmWriteError::Validation("statement is required"))?;
    if statement.len() > MAX_STATEMENT_LEN {
        return Err(PmWriteError::Validation("statement exceeds size limit"));
    }
    Ok(statement.to_string())
}

/// 尾部位置（追加用）。
async fn next_position(
    tx: &mut sqlx::PgConnection,
    requirement_id: Uuid,
) -> Result<i32, PmWriteError> {
    let position: i32 = sqlx::query_scalar(
        "SELECT COALESCE(max(position), 0) + 1 FROM delivery.requirement_acceptance_criterion \
         WHERE requirement_id = $1",
    )
    .bind(requirement_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    Ok(position)
}

/// 校验 position 非负。
fn validate_position(position: Option<i32>) -> Result<Option<i32>, PmWriteError> {
    match position {
        Some(p) if p < 0 => Err(PmWriteError::Validation("position must be >= 0")),
        other => Ok(other),
    }
}

// ─────────────────────────────────────────────────────────────── 命令

/// 新增验收标准（受控命令；established_in_revision_id 留痕当前正式版本）。
pub async fn add_criterion(
    pool: &PgPool,
    tenant_id: Uuid,
    requirement_id: Uuid,
    req: AddCriterionRequest,
    actor: &AuditActor,
) -> Result<CriterionView, PmWriteError> {
    require_human(pool, actor).await?;
    let statement = validate_statement(Some(&req.statement))?;
    let kind = validate_kind(req.verification_kind.as_deref())?;
    let params = validate_params(req.params.as_ref())?;
    let position = validate_position(req.position)?;

    let mut tx = pool.begin().await?;
    let project_id = lock_active_requirement(&mut *tx, tenant_id, requirement_id).await?;
    crate::approver::ensure_governance_authorized(pool, project_id, actor.principal_id).await?;

    let position = match position {
        Some(p) => p,
        None => next_position(&mut *tx, requirement_id).await?,
    };
    // established_in_revision_id 留痕：确立时需求所在正式版本（Q-11，可为 NULL）
    let established: Option<Uuid> =
        sqlx::query_scalar("SELECT current_revision_id FROM delivery.requirement WHERE id = $1")
            .bind(requirement_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(PmWriteError::Db)?;

    let criterion_id = Uuid::new_v4();
    let view = sqlx::query_as::<_, CriterionView>(
        "INSERT INTO delivery.requirement_acceptance_criterion \
         (id, tenant_id, project_id, requirement_id, position, statement, \
          verification_kind, params, established_in_revision_id, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
         RETURNING id, requirement_id, position, statement, verification_kind, params, \
                   established_in_revision_id, verdict, verdict_note, verdict_by, verdict_at, \
                   created_at, updated_at",
    )
    .bind(criterion_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(requirement_id)
    .bind(position)
    .bind(&statement)
    .bind(&kind)
    .bind(&params)
    .bind(established)
    .bind(actor.principal_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;

    append_audit(
        &mut tx,
        project_id,
        actor,
        "requirement.criterion_added",
        "requirement",
        Some(requirement_id),
    )
    .await?;
    tx.commit().await?;
    Ok(view)
}

/// 修改验收标准（Q-12：独立受控命令，不产生新修订版本）。
pub async fn update_criterion(
    pool: &PgPool,
    tenant_id: Uuid,
    requirement_id: Uuid,
    criterion_id: Uuid,
    req: UpdateCriterionRequest,
    actor: &AuditActor,
) -> Result<CriterionView, PmWriteError> {
    require_human(pool, actor).await?;
    let statement = match req.statement.as_deref() {
        None => None,
        Some(_) => Some(validate_statement(req.statement.as_deref())?),
    };
    let kind = match req.verification_kind.as_deref() {
        None => None,
        Some(_) => Some(validate_kind(req.verification_kind.as_deref())?),
    };
    let params = match req.params.as_ref() {
        None => None,
        Some(_) => Some(validate_params(req.params.as_ref())?),
    };
    let position = validate_position(req.position)?;

    let mut tx = pool.begin().await?;
    let project_id = lock_active_requirement(&mut *tx, tenant_id, requirement_id).await?;
    crate::approver::ensure_governance_authorized(pool, project_id, actor.principal_id).await?;

    let updated = sqlx::query_scalar::<_, i64>(
        "WITH touched AS ( \
           UPDATE delivery.requirement_acceptance_criterion SET \
             statement = COALESCE($3, statement), \
             verification_kind = COALESCE($4, verification_kind), \
             params = COALESCE($5, params), \
             position = COALESCE($6, position), \
             updated_at = now() \
           WHERE id = $1 AND requirement_id = $2 \
           RETURNING 1) \
         SELECT count(*) FROM touched",
    )
    .bind(criterion_id)
    .bind(requirement_id)
    .bind(&statement)
    .bind(&kind)
    .bind(&params)
    .bind(position)
    .fetch_one(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    if updated == 0 {
        return Err(PmWriteError::NotFound("criterion not found"));
    }

    let view = fetch_criterion(&mut *tx, requirement_id, criterion_id).await?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "requirement.criterion_updated",
        "requirement",
        Some(requirement_id),
    )
    .await?;
    tx.commit().await?;
    Ok(view)
}

/// 移除验收标准（治理命令；审计留痕，不软删——Q-12 无版本机，历史在 audit）。
pub async fn remove_criterion(
    pool: &PgPool,
    tenant_id: Uuid,
    requirement_id: Uuid,
    criterion_id: Uuid,
    actor: &AuditActor,
) -> Result<(), PmWriteError> {
    require_human(pool, actor).await?;
    let mut tx = pool.begin().await?;
    let project_id = lock_active_requirement(&mut *tx, tenant_id, requirement_id).await?;
    crate::approver::ensure_governance_authorized(pool, project_id, actor.principal_id).await?;

    let removed = sqlx::query(
        "DELETE FROM delivery.requirement_acceptance_criterion \
         WHERE id = $1 AND requirement_id = $2",
    )
    .bind(criterion_id)
    .bind(requirement_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    if removed.rows_affected() == 0 {
        return Err(PmWriteError::NotFound("criterion not found"));
    }

    append_audit(
        &mut tx,
        project_id,
        actor,
        "requirement.criterion_removed",
        "requirement",
        Some(requirement_id),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// 需求级验收裁决（Q-14：零 Task NFR 的裁决入口，S04/S05 声明接口）。
/// verdict ∈ passed/failed；re-verdict 覆盖更新（审计留痕）。
pub async fn record_criterion_verdict(
    pool: &PgPool,
    tenant_id: Uuid,
    requirement_id: Uuid,
    criterion_id: Uuid,
    req: CriterionVerdictRequest,
    actor: &AuditActor,
) -> Result<CriterionView, PmWriteError> {
    require_human(pool, actor).await?;
    let verdict = req.verdict.trim();
    if !matches!(verdict, "passed" | "failed") {
        return Err(PmWriteError::Validation(
            "verdict must be 'passed' or 'failed'",
        ));
    }

    let mut tx = pool.begin().await?;
    let project_id = lock_active_requirement(&mut *tx, tenant_id, requirement_id).await?;
    crate::approver::ensure_governance_authorized(pool, project_id, actor.principal_id).await?;

    let updated = sqlx::query(
        "UPDATE delivery.requirement_acceptance_criterion SET \
           verdict = $3, verdict_note = $4, verdict_by = $5, verdict_at = now(), \
           updated_at = now() \
         WHERE id = $1 AND requirement_id = $2",
    )
    .bind(criterion_id)
    .bind(requirement_id)
    .bind(verdict)
    .bind(req.note.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;
    if updated.rows_affected() == 0 {
        return Err(PmWriteError::NotFound("criterion not found"));
    }

    let view = fetch_criterion(&mut *tx, requirement_id, criterion_id).await?;
    append_audit(
        &mut tx,
        project_id,
        actor,
        "requirement.criterion_verdict",
        "requirement",
        Some(requirement_id),
    )
    .await?;
    tx.commit().await?;
    Ok(view)
}

// ─────────────────────────────────────────────────────────────── 读取

/// 验收标准列表（按 position, id 稳定排序）。
pub async fn list_criteria(
    pool: &PgPool,
    tenant_id: Uuid,
    requirement_id: Uuid,
) -> Result<serde_json::Value, PmWriteError> {
    let items = sqlx::query_as::<_, CriterionView>(
        "SELECT id, requirement_id, position, statement, verification_kind, params, \
                established_in_revision_id, verdict, verdict_note, verdict_by, verdict_at, \
                created_at, updated_at \
         FROM delivery.requirement_acceptance_criterion \
         WHERE requirement_id = $1 \
           AND project_id = (SELECT project_id FROM delivery.requirement \
                             WHERE id = $1 AND tenant_id = $2) \
         ORDER BY position, id",
    )
    .bind(requirement_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;
    Ok(serde_json::json!({ "items": items }))
}

/// 确认时一致性提示的数据面（Q-12）：`since` 之后是否有验收项被修改。
/// REST 层在 confirm 成功后以 `entry.latest_submitted_at` 调用并透出
/// `criteria_drift` 字段——审批人核准的修订提交后验收项又被改过 = 应复核。
pub async fn criteria_drift_since(
    pool: &PgPool,
    tenant_id: Uuid,
    requirement_id: Uuid,
    since: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<bool, PmWriteError> {
    let since = match since {
        Some(value) => value,
        None => return Ok(false),
    };
    let drifted: bool = sqlx::query_scalar(
        "SELECT EXISTS ( \
           SELECT 1 FROM delivery.requirement_acceptance_criterion c \
           JOIN delivery.requirement r ON r.id = c.requirement_id \
           WHERE c.requirement_id = $1 AND r.tenant_id = $2 \
             AND c.updated_at > $3)",
    )
    .bind(requirement_id)
    .bind(tenant_id)
    .bind(since)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)?;
    Ok(drifted)
}

// ─────────────────────────────────────────────────────────────── 内部

async fn fetch_criterion(
    conn: &mut sqlx::PgConnection,
    requirement_id: Uuid,
    criterion_id: Uuid,
) -> Result<CriterionView, PmWriteError> {
    sqlx::query_as::<_, CriterionView>(
        "SELECT id, requirement_id, position, statement, verification_kind, params, \
                established_in_revision_id, verdict, verdict_note, verdict_by, verdict_at, \
                created_at, updated_at \
         FROM delivery.requirement_acceptance_criterion \
         WHERE id = $1 AND requirement_id = $2",
    )
    .bind(criterion_id)
    .bind(requirement_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("criterion not found"))
}
