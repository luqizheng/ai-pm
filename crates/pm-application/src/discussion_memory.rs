//! Discussion Memory 的 Platform 侧：引用校验（Platform 对象）、按 Task 计算检索主题、
//! Agent Context 注入（`RunContract.discussion_memory`）。
//!
//! 分工（检查单 A-1 / §5.4.4 #8）：讨论原文（L2 Deep Memory）是 Platform 的既有载体——
//! `ai.conversation_message`（0060，属主私有）与 `delivery.task_interaction`（0046）；
//! KB 只持有 L1 沉淀（`kb.memory*`）与指向原文的四元组。故 Platform 对象的存在性 /
//! 归属校验在本模块（读 Platform 表），KB 对象（`knowledge_base/source`）由
//! [`crate::kb_service`] 在写事务内自校验；本模块不读 `kb.*` 表，只经
//! [`crate::kb_service::retrieval`] 读面取 CURRENT 版本。
//!
//! 注入裁决（用户 2026-11-08 = B，差异说明 D-KP3-7）：`pm_get_run_context` 契约新增
//! 可选字段 `discussion_memory[]`（摘要级）；开关 `AI_PM_DISCUSSION_MEMORY_INJECTION`
//! **默认 off**，off 或无命中时字段不序列化，契约输出与改动前逐字节一致。
//!
//! Platform 对象四元组沿 Phase 1-3 偏差⑧暂挂 `requirement` 命名空间，以 `target_type`
//! 区分（差异说明 D-KP3-6）。

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::kb_service::memory::{RefRole, ResolvedRef};
use crate::kb_service::retrieval::{current_memories_for_subjects, SubjectKey};
use crate::kb_service::{KbServiceError, TYPE_SOURCE};

/// Platform 命名空间（偏差⑧）。
pub const PLATFORM_DOMAIN: &str = "requirement";
/// Platform 对象类型。
pub const TYPE_TASK: &str = "task";
pub const TYPE_REQUIREMENT: &str = "requirement";
pub const TYPE_CONVERSATION_MESSAGE: &str = "conversation_message";
pub const TYPE_TASK_INTERACTION: &str = "task_interaction";

/// 无 Revision 机制对象的空缺原因（检查单 R-4）。
const NO_REVISION_MECHANISM: &str = "no_revision_mechanism";
/// 需求实体级引用（讨论针对需求本身，不限于某个修订）。
const ENTITY_LEVEL: &str = "entity_level_reference";

/// 注入开关（`AI_PM_DISCUSSION_MEMORY_INJECTION=off|on`），默认 off——参照
/// `AI_PM_CONVERSATION_REPLY` 先例：新的契约输出面不随升级静默打开。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscussionMemoryInjectionMode {
    Off,
    On,
}

impl DiscussionMemoryInjectionMode {
    pub fn parse(value: Option<&str>) -> Self {
        match value {
            Some("on") => Self::On,
            Some("off") | None => Self::Off,
            // 拼写错误不得静默打开契约新输出面：告警 + off。
            Some(other) => {
                tracing::warn!(
                    value = other,
                    "unknown AI_PM_DISCUSSION_MEMORY_INJECTION value (expected off|on); staying off"
                );
                Self::Off
            }
        }
    }

    pub fn from_env() -> Self {
        Self::parse(
            std::env::var("AI_PM_DISCUSSION_MEMORY_INJECTION")
                .ok()
                .as_deref(),
        )
    }

    pub fn enabled(self) -> bool {
        matches!(self, Self::On)
    }
}

/// REST 入参：一条引用（四元组 + 角色 + 可选片段定位）。
#[derive(Debug, Clone, Deserialize)]
pub struct MemoryRefRequest {
    pub ref_role: String,
    pub target_domain: String,
    pub target_type: String,
    pub target_id: String,
    #[serde(default)]
    pub target_revision_id: Option<String>,
    #[serde(default)]
    pub locator: Option<serde_json::Value>,
}

fn invalid(msg: impl Into<String>) -> KbServiceError {
    KbServiceError::Invalid(msg.into())
}

fn parse_uuid(raw: &str, what: &str) -> Result<Uuid, KbServiceError> {
    Uuid::parse_str(raw.trim()).map_err(|_| invalid(format!("{what} id must be a UUID")))
}

/// 校验并解析引用：Platform 对象在此校验存在性与项目 / 属主归属并补齐 revision 语义；
/// KB 对象只做形状校验后透传（KB 写事务内自校验）。
///
/// 角色矩阵（V0.5 最小面）：subject ∈ {task, requirement}（按 Task 检索的锚）；
/// evidence ∈ {conversation_message（必须是提案人自己的会话——会话属主私有，跨属主按
/// 不存在处理）, task_interaction, knowledge_base/source}。
pub async fn resolve_refs(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    proposer_principal_id: Uuid,
    inputs: Vec<MemoryRefRequest>,
) -> Result<Vec<ResolvedRef>, KbServiceError> {
    let mut out = Vec::with_capacity(inputs.len());
    for input in inputs {
        let role = RefRole::parse(&input.ref_role)
            .ok_or_else(|| invalid("ref_role must be evidence | subject"))?;
        let domain = input.target_domain.as_str();
        let ty = input.target_type.as_str();
        let (target_id, revision_id, reason) = match (role, domain, ty) {
            (RefRole::Subject, PLATFORM_DOMAIN, TYPE_TASK) => {
                let id = parse_uuid(&input.target_id, "task")?;
                let ok: bool = sqlx::query_scalar(
                    "SELECT EXISTS (SELECT 1 FROM delivery.task \
                      WHERE id = $1 AND project_id = $2 AND tenant_id = $3)",
                )
                .bind(id)
                .bind(project_id)
                .bind(tenant_id)
                .fetch_one(pool)
                .await?;
                if !ok {
                    return Err(invalid("referenced task not found in this project"));
                }
                (id.to_string(), None, Some(NO_REVISION_MECHANISM))
            }
            (RefRole::Subject, PLATFORM_DOMAIN, TYPE_REQUIREMENT) => {
                let id = parse_uuid(&input.target_id, "requirement")?;
                let ok: bool = sqlx::query_scalar(
                    "SELECT EXISTS (SELECT 1 FROM delivery.requirement \
                      WHERE id = $1 AND project_id = $2)",
                )
                .bind(id)
                .bind(project_id)
                .fetch_one(pool)
                .await?;
                if !ok {
                    return Err(invalid("referenced requirement not found in this project"));
                }
                match input.target_revision_id.as_deref() {
                    Some(raw) => {
                        let rev = parse_uuid(raw, "requirement revision")?;
                        let ok: bool = sqlx::query_scalar(
                            "SELECT EXISTS (SELECT 1 FROM delivery.requirement_revision \
                              WHERE id = $1 AND requirement_id = $2)",
                        )
                        .bind(rev)
                        .bind(id)
                        .fetch_one(pool)
                        .await?;
                        if !ok {
                            return Err(invalid(
                                "referenced requirement revision does not belong to the requirement",
                            ));
                        }
                        (id.to_string(), Some(rev.to_string()), None)
                    }
                    None => (id.to_string(), None, Some(ENTITY_LEVEL)),
                }
            }
            (RefRole::Evidence, PLATFORM_DOMAIN, TYPE_CONVERSATION_MESSAGE) => {
                let id = parse_uuid(&input.target_id, "conversation message")?;
                let ok: bool = sqlx::query_scalar(
                    "SELECT EXISTS (SELECT 1 FROM ai.conversation_message m \
                       JOIN ai.conversation c ON c.id = m.conversation_id \
                      WHERE m.id = $1 AND c.project_id = $2 AND c.tenant_id = $3 \
                        AND c.principal_id = $4)",
                )
                .bind(id)
                .bind(project_id)
                .bind(tenant_id)
                .bind(proposer_principal_id)
                .fetch_one(pool)
                .await?;
                if !ok {
                    return Err(invalid("referenced conversation message not found"));
                }
                (id.to_string(), None, Some(NO_REVISION_MECHANISM))
            }
            (RefRole::Evidence, PLATFORM_DOMAIN, TYPE_TASK_INTERACTION) => {
                let id = parse_uuid(&input.target_id, "task interaction")?;
                let ok: bool = sqlx::query_scalar(
                    "SELECT EXISTS (SELECT 1 FROM delivery.task_interaction \
                      WHERE id = $1 AND project_id = $2 AND tenant_id = $3)",
                )
                .bind(id)
                .bind(project_id)
                .bind(tenant_id)
                .fetch_one(pool)
                .await?;
                if !ok {
                    return Err(invalid(
                        "referenced task interaction not found in this project",
                    ));
                }
                (id.to_string(), None, Some(NO_REVISION_MECHANISM))
            }
            (RefRole::Evidence, "knowledge_base", TYPE_SOURCE) => {
                // 原始资料不可改写（0065 触发器）：content_hash 已钉内容，无 Revision 机制。
                let id = parse_uuid(&input.target_id, "source")?;
                (id.to_string(), None, Some(NO_REVISION_MECHANISM))
            }
            _ => {
                return Err(invalid(format!(
                    "unsupported reference {}:{}/{} (subject: requirement/task|requirement; \
                     evidence: requirement/conversation_message|task_interaction, \
                     knowledge_base/source)",
                    input.ref_role, input.target_domain, input.target_type
                )))
            }
        };
        out.push(ResolvedRef {
            role,
            target_domain: input.target_domain,
            target_type: input.target_type,
            target_id,
            target_revision_id: revision_id,
            revision_missing_reason: reason.map(str::to_string),
            locator: input.locator,
        });
    }
    Ok(out)
}

/// Task 的检索主题：task 本身 + 其关联需求（`delivery.task_requirement`，实体级）。
pub async fn task_subjects(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
) -> Result<Vec<SubjectKey>, sqlx::Error> {
    let requirement_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT requirement_id FROM delivery.task_requirement \
          WHERE task_id = $1 AND project_id = $2 ORDER BY requirement_id",
    )
    .bind(task_id)
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    let mut subjects = vec![SubjectKey::new(
        PLATFORM_DOMAIN,
        TYPE_TASK,
        task_id.to_string(),
    )];
    subjects.extend(
        requirement_ids
            .into_iter()
            .map(|id| SubjectKey::new(PLATFORM_DOMAIN, TYPE_REQUIREMENT, id.to_string())),
    );
    Ok(subjects)
}

/// `RunContract.discussion_memory[]` 元素（摘要级；正文 body 不进 Agent Context）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscussionMemoryView {
    pub memory_id: Uuid,
    /// 被批准的修订 id（版本身份；K4 / 详情面同一 id）。
    pub version_id: Uuid,
    pub version_no: i32,
    pub memory_type: String,
    pub title: String,
    pub summary: String,
    /// 命中的检索主题（`task:<id>` / `requirement:<id>`）。
    pub matched_by: Vec<String>,
}

/// 按 Task 检索 CURRENT L1 记忆（注入面读模型；开关由调用方判定）。
pub async fn discussion_memory_for_task(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
) -> Result<Vec<DiscussionMemoryView>, sqlx::Error> {
    let subjects = task_subjects(pool, project_id, task_id).await?;
    Ok(current_memories_for_subjects(pool, project_id, &subjects)
        .await?
        .into_iter()
        .map(|m| DiscussionMemoryView {
            memory_id: m.memory_id,
            version_id: m.revision_id,
            version_no: m.version_no,
            memory_type: m.memory_type,
            title: m.title,
            summary: m.summary,
            matched_by: m.matched_subjects,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injection_switch_defaults_off_and_typo_stays_off() {
        assert_eq!(
            DiscussionMemoryInjectionMode::parse(None),
            DiscussionMemoryInjectionMode::Off
        );
        assert_eq!(
            DiscussionMemoryInjectionMode::parse(Some("off")),
            DiscussionMemoryInjectionMode::Off
        );
        assert_eq!(
            DiscussionMemoryInjectionMode::parse(Some("ON")),
            DiscussionMemoryInjectionMode::Off
        );
        assert_eq!(
            DiscussionMemoryInjectionMode::parse(Some("true")),
            DiscussionMemoryInjectionMode::Off
        );
        assert!(DiscussionMemoryInjectionMode::parse(Some("on")).enabled());
    }
}
