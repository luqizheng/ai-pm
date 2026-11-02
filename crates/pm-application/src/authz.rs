//! 授权判定（T-088）：项目成员资格 + 项目执行主体解析。
//!
//! 两件事，都是服务端强制、不可由客户端自称：
//!
//! 1. [`require_project_member`]——**真人**是否是目标项目的成员。口径照 pm-api
//!    既有过滤（`pm_lists.rs:99`：`p.tenant_id = $5 AND EXISTS(project_member)`）。
//!    非成员 → `NotMember`（上层映射为 **404**，与 T-022「不可见即不存在」一致，
//!    防项目/任务 ID 探测）。
//! 2. [`resolve_project_executor`]——该项目的**执行主体**（dev_agent / ai_pm）。
//!    **找不到即报错**，不得回退真人 Principal（会破 ADR-0001 D2 的执行主体口径）。

use sqlx::PgPool;
use uuid::Uuid;

/// 授权判定失败。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthzError {
    /// 调用人不是该项目的成员（**上层映射 404**，不泄露存在性）。
    #[error("caller is not a member of the project")]
    NotMember,
    /// 该项目没有对应的执行主体（dev_agent / ai_pm）。
    #[error("project has no executor principal of the required kind")]
    ExecutorMissing,
    /// 数据库错误。
    #[error("database error: {0}")]
    Db(String),
}

/// 校验真人是否为目标项目的成员。
///
/// **必须传真人 principal_id**——`dev_agent` 本身就是 `core.project_member` 的一行，
/// 用它校验恒真，等于没校验。
pub async fn require_project_member(
    pool: &PgPool,
    caller_principal_id: Uuid,
    caller_tenant_id: Uuid,
    project_id: Uuid,
) -> Result<(), AuthzError> {
    let ok: Option<bool> = sqlx::query_scalar(
        "SELECT EXISTS ( \
            SELECT 1 FROM core.project_member m \
            JOIN core.project p ON p.id = m.project_id AND p.tenant_id = m.tenant_id \
            WHERE m.project_id = $1 AND m.principal_id = $2 AND m.tenant_id = $3 \
         )",
    )
    .bind(project_id)
    .bind(caller_principal_id)
    .bind(caller_tenant_id)
    .fetch_one(pool)
    .await
    .map_err(|e| AuthzError::Db(e.to_string()))?;
    if ok.unwrap_or(false) {
        Ok(())
    } else {
        Err(AuthzError::NotMember)
    }
}

/// 项目执行主体的种类（维持各工具现状口径，不统一改口径）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorKind {
    /// 任务 / 运行类工具的执行主体。
    DevAgent,
    /// 文档 / 知识类工具的执行主体。
    AiPm,
}

impl ExecutorKind {
    /// `core.principal.kind` 的取值。
    pub const fn principal_kind(self) -> &'static str {
        match self {
            Self::DevAgent => "dev_agent",
            Self::AiPm => "ai_pm",
        }
    }
}

/// 取该项目的执行主体 principal。
///
/// **找不到即 `ExecutorMissing`**——不静默回退真人 Principal。
pub async fn resolve_project_executor(
    pool: &PgPool,
    project_id: Uuid,
    kind: ExecutorKind,
) -> Result<Uuid, AuthzError> {
    let principal_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT pm.principal_id FROM core.project_member pm \
         JOIN core.principal p ON p.id = pm.principal_id \
         WHERE pm.project_id = $1 AND p.kind = $2 \
         ORDER BY pm.principal_id LIMIT 1",
    )
    .bind(project_id)
    .bind(kind.principal_kind())
    .fetch_optional(pool)
    .await
    .map_err(|e| AuthzError::Db(e.to_string()))?;
    principal_id.ok_or(AuthzError::ExecutorMissing)
}
