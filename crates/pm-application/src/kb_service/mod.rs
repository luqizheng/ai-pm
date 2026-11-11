//! KB-Service 领域本体（KP-3：KB 三层最小面）。
//!
//! 三层落点（迁移 0065；定义稿 §5 / 架构稿 V1.1 §7.1）：
//! - **Original Sources**（[`source`]）：团队级来源池 `kb.source`（内容不可改写，同内容
//!   同身份）+ 项目关联 `kb.source_project`（一源多项目，访问权继承自关联项目成员关系）。
//! - **Extracted Content**：既有 `kb.document*`（0053 digest 半层，本模块不碰）。
//! - **Structured Memory / L1 Shallow Memory**（[`memory`]）：`kb.memory` +
//!   `kb.memory_revision` + `kb.memory_ref`。任何内容变化先进待审核区，**只有真人审核
//!   通过才产生版本**；版本内容不可变、不可物理删除（库层触发器与 CHECK 兜底）。
//! - **按 Task 检索**（[`retrieval`]）：只读 CURRENT 版本的 subject 引用；主题集合由
//!   Platform 侧给出（本模块不读 Platform 表——检查单 A-1）。
//!
//! 端口：[`port::KbServiceLive`] 实现 `KbServicePort` 的 K4（审核转达，CAS）与 K5
//! （来源只读查询）；K1/K2/K3/K6 维持未接线语义（差异说明 D-KP3-5：§12.2 未列的 V0.5
//! 管理面由本模块应用命令直接承接，端口形状不变）。
//!
//! 写入纪律：KB 是 `kb.*` 唯一写入方；项目活跃性与真人校验经 Platform 鉴权函数
//! （`project_gate` / `requirement::require_human`，V1 进程内形态）；审计写
//! `governance.audit_log`，`source_kind='kb_service'`，四身份列全写（C174/C180），
//! 不落正文（检查单 A-4）；跨域事件只走 outbox（`kb.memory.approved`）。

pub mod document;
pub mod indexer;
pub mod memory;
pub mod port;
pub mod retrieval;
pub mod source;

use uuid::Uuid;

use crate::actor::AuditActor;

pub use port::{KbCaller, KbServiceLive};

/// `knowledge_base` 域内对象类型（四元组 `target_type` 取值）。
pub const TYPE_SOURCE: &str = "source";
pub const TYPE_MEMORY: &str = "memory";
pub const TYPE_MEMORY_REVISION: &str = "memory_revision";
/// `kb.document` 域内对象类型（四元组 `target_type` 取值）。
pub const TYPE_DOCUMENT: &str = "document";

/// 审计来源标签（`governance.audit_log.source_kind`）。
pub const AUDIT_SOURCE_KIND: &str = "kb_service";

/// KB 应用命令错误。
#[derive(Debug, thiserror::Error)]
pub enum KbServiceError {
    #[error("not found: {0}")]
    NotFound(&'static str),
    /// 有身份但无权限（AI 主体审核等）。
    #[error("forbidden: {0}")]
    Forbidden(&'static str),
    #[error("invalid request: {0}")]
    Invalid(String),
    /// CAS / 状态冲突（项目已归档、在途提案、过期审核基线等）。
    #[error("conflict: {0}")]
    Conflict(String),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

impl From<KbServiceError> for kb_pc_contracts::PortError {
    fn from(e: KbServiceError) -> Self {
        use kb_pc_contracts::PortError;
        match e {
            KbServiceError::NotFound(m) => PortError::NotFound(m.to_string()),
            KbServiceError::Forbidden(m) => PortError::Forbidden(m.to_string()),
            KbServiceError::Invalid(m) => PortError::Invalid(m),
            KbServiceError::Conflict(m) => PortError::Conflict(m),
            KbServiceError::Db(err) => {
                tracing::error!(error = %err, "kb_service database error");
                PortError::Internal("database error".to_string())
            }
        }
    }
}

/// 写入口统一门：在 project 行锁下确认项目仍活跃（I4-F 同款锁序：project → kb 行）。
pub(crate) async fn ensure_project_writable(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
) -> Result<(), KbServiceError> {
    use crate::project_gate::{lock_project_activity_tx, GateError, ProjectActivity};
    match lock_project_activity_tx(tx, project_id).await {
        Ok(ProjectActivity::Active) => Ok(()),
        Ok(ProjectActivity::Archived) => Err(KbServiceError::Conflict(
            "project is archived: knowledge base writes are closed".to_string(),
        )),
        Err(GateError::NotFound) => Err(KbServiceError::NotFound("project not found")),
        Err(err) => Err(KbServiceError::Db(sqlx::Error::Protocol(err.to_string()))),
    }
}

/// KB 域审计行（四身份列全写——C174/C180；`after_data` 只放元数据，不落正文）。
/// tenant 由 project 行内派生（knowledge_proposal 同款；调用点已持 project 锁）。
pub(crate) async fn audit(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
    actor: &AuditActor,
    action: &str,
    target_type: &str,
    target_id: Uuid,
    after_data: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (project_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
          action, target_type, target_id, after_data, source_kind, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(actor.principal_id)
    .bind(actor.identity_id)
    .bind(&actor.client)
    .bind(&actor.auth_mode)
    .bind(actor.claimed_device_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(after_data)
    .bind(AUDIT_SOURCE_KIND)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 文本字段长度校验（按字符计，与库层 `length()` 同口径）。
pub(crate) fn check_len(
    field: &str,
    value: &str,
    min: usize,
    max: usize,
) -> Result<(), KbServiceError> {
    let n = value.trim().chars().count();
    if n < min || n > max {
        return Err(KbServiceError::Invalid(format!(
            "{field} must be {min}..={max} characters (got {n})"
        )));
    }
    Ok(())
}

/// 唯一约束冲突 → 业务冲突（并发提案撞单 pending 索引等）。
/// 单一定义点：`pm_write::is_unique_violation`（批 A1 归并）。
pub(crate) use crate::pm_write::is_unique_violation;
