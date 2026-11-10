//! S04 · AI 任务拆解编排（S4 提案 V1 §1.2/§3.1/§3.2 落地）。
//!
//! 与 DK-F `DiscoveryWriteBack` 同层的编排：把「已批准的 S03 成果」转化为
//! 「任务计划草案」。
//! - **准入校验（§1.2）**：来源基线必须 frozen 且 Readiness Gate 通过——
//!   拆解只读已冻结的实施依据，不复制正文进任务描述（追溯链不断）；
//! - **LLM 调用**：复用 [`LlmWriteBackModel::decompose_tasks`]（三级
//!   Provider 解析 + 结构化 JSON 输出 + `ai.llm_call_record` 留痕在模型侧
//!   完成——DK-F 同款链路）；无 Provider 时显式失败，不静默造数据；
//! - **确定性校验**：`task_plan::validate_plan_tasks`（键唯一/验收非空/
//!   依赖前向图）——系统拥有骨架，模型只填内容；
//! - **重拆（Q2）**：永远产生新 Revision 草案 diff（add/update/remove），
//!   保留人工修改由「拒绝采纳」表达；已批准计划的旧修订不被覆盖。
//!
//! 治理红线：拆解产物一律 `draft`（AI 无批准权，R3）；LLM 输出只落
//! 计划草案，不得直接写 `delivery.task`。

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use pm_application::actor::AuditActor;
use pm_application::pm_write::PmWriteError;
use pm_application::task_plan::{self, PlanTaskDraft};

use crate::discovery_writeback::{
    ModelCallFailure, ModelReply, TaskDecomposeContext, WriteBackModel,
};
use crate::writeback_model::LlmWriteBackModel;

/// 设计正文段的字符预算（防 prompt 超长；DK-F 同款纪律）。
const MAX_DESIGN_CHARS: usize = 24_000;
const MAX_BASELINE_CHARS: usize = 8_000;

/// 拆解结果视图。
#[derive(Debug, Serialize)]
pub struct DecompositionOutcome {
    pub plan_id: Uuid,
    pub revision_id: Uuid,
    pub version: i32,
    pub task_count: usize,
    /// 重拆 diff 摘要（add/update/remove；首拆提示无基线）。
    pub warnings: Vec<String>,
}

/// 拆解请求（§3.1）。
#[derive(Debug, serde::Deserialize)]
pub struct DecomposeRequest {
    pub plan_id: Uuid,
    /// 缺省 = 计划的 source_baseline_id（创建时钉定）。
    #[serde(default)]
    pub baseline_id: Option<Uuid>,
    /// 重拆时的人工修改意图说明（进入 prompt 与修订 instructions 留痕）。
    #[serde(default)]
    pub instructions: Option<String>,
}

/// 拆解准入校验：基线 frozen + Readiness Gate 通过。
///
/// 返回基线内容快照（供 prompt 构建）。
async fn decomposition_admission(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    baseline_id: Uuid,
) -> Result<serde_json::Value, PmWriteError> {
    let (b_project, b_status): (Uuid, String) = sqlx::query_as(
        "SELECT project_id, status FROM design.implementation_baseline \
         WHERE id = $1 AND tenant_id = $2",
    )
    .bind(baseline_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(PmWriteError::Db)?
    .ok_or(PmWriteError::NotFound("baseline not found"))?;
    if b_project != project_id {
        return Err(PmWriteError::Validation(
            "baseline does not belong to this project",
        ));
    }
    if b_status != "frozen" {
        return Err(PmWriteError::Conflict(
            "baseline is not frozen; decomposition requires a frozen implementation baseline",
        ));
    }
    let content: serde_json::Value = sqlx::query_scalar(
        "SELECT content FROM design.implementation_baseline WHERE id = $1 AND tenant_id = $2",
    )
    .bind(baseline_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let revisions: Vec<Uuid> = content["design_revision_ids"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().and_then(|s| Uuid::parse_str(s).ok()))
                .collect()
        })
        .unwrap_or_default();
    if revisions.is_empty() {
        return Err(PmWriteError::Validation("baseline has no design revisions"));
    }
    // Readiness Gate 复判（基线冻结后设计可能被 supersede——引用已死决策须拦截）。
    let gate =
        pm_application::design::evaluate_readiness(pool, tenant_id, project_id, &revisions).await?;
    if !gate.ready {
        return Err(PmWriteError::Conflict(
            "readiness gate no longer passes for the frozen baseline's revisions",
        ));
    }
    Ok(content)
}

/// 组装拆解 payload（§3.1 输入：基线快照 + 设计正文 + 范围 + 验收 + 现有任务清单）。
async fn build_decompose_payload(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    baseline_content: &serde_json::Value,
    instructions: Option<&str>,
    existing_tasks: &[(String, String, String)],
) -> Result<String, PmWriteError> {
    let revisions: Vec<Uuid> = baseline_content["design_revision_ids"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().and_then(|s| Uuid::parse_str(s).ok()))
                .collect()
        })
        .unwrap_or_default();

    let mut design_sections = String::new();
    let mut scope_included: Vec<String> = Vec::new();
    let mut acceptance: Vec<String> = Vec::new();
    for rid in &revisions {
        let row: Option<(String, String, serde_json::Value)> = sqlx::query_as(
            "SELECT d.code, d.title, rv.content \
             FROM design.design_revision rv \
             JOIN design.design d ON d.id = rv.design_id \
             WHERE rv.id = $1 AND rv.tenant_id = $2 AND rv.project_id = $3",
        )
        .bind(rid)
        .bind(tenant_id)
        .bind(project_id)
        .fetch_optional(pool)
        .await
        .map_err(PmWriteError::Db)?;
        let Some((code, title, content)) = row else {
            continue;
        };
        let markdown = content
            .get("markdown")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        design_sections.push_str(&format!(
            "\n<design code=\"{code}\" title=\"{title}\">\n{markdown}\n</design>\n"
        ));
        if let Some(arr) = content
            .get("implementation_scope")
            .and_then(|s| s.get("included"))
            .and_then(serde_json::Value::as_array)
        {
            for item in arr {
                if let Some(s) = item.as_str() {
                    scope_included.push(s.to_string());
                }
            }
        }
        if let Some(arr) = content
            .get("acceptance_criteria")
            .and_then(serde_json::Value::as_array)
        {
            for item in arr {
                if let Some(s) = item.get("statement").and_then(serde_json::Value::as_str) {
                    acceptance.push(s.to_string());
                }
            }
        }
    }

    let baseline_summary = serde_json::to_string_pretty(baseline_content).unwrap_or_default();
    let plan_marker = instructions
        .map(|s| format!("\n<human_instructions>\n{s}\n</human_instructions>\n"))
        .unwrap_or_default();
    let existing = if existing_tasks.is_empty() {
        "（无）".to_string()
    } else {
        existing_tasks
            .iter()
            .map(|(key, title, status)| format!("- {key}: {title} ({status})"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    Ok(format!(
        "<implementation_baseline>\n{baseline}\n</implementation_baseline>\n\
         <designs>\n{designs}\n</designs>\n\
         <implementation_scope>\n{scope}\n</implementation_scope>\n\
         <acceptance_criteria>\n{acceptance}\n</acceptance_criteria>\n\
         <existing_tasks>\n{existing}\n</existing_tasks>{plan_marker}",
        baseline = cap(&baseline_summary, MAX_BASELINE_CHARS),
        designs = cap(&design_sections, MAX_DESIGN_CHARS),
        scope = if scope_included.is_empty() {
            "（无）".to_string()
        } else {
            scope_included.join("\n")
        },
        acceptance = if acceptance.is_empty() {
            "（无）".to_string()
        } else {
            acceptance.join("\n")
        },
    ))
}

fn cap(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect::<String>() + "\n…（截断）"
    }
}

/// LLM 原始批 → 计划草案（prerequisite + main 合一；依赖键原样保留）。
fn draft_tasks_from_batch(batch: &crate::discovery_writeback::RawTaskBatch) -> Vec<PlanTaskDraft> {
    let mut tasks = Vec::new();
    for section in [&batch.prerequisite, &batch.main] {
        for t in section {
            tasks.push(PlanTaskDraft {
                key: t.key.trim().to_string(),
                title: t.title.trim().to_string(),
                description: t.description.clone(),
                acceptance_criteria: t.acceptance_criteria.clone(),
                depends_on: t.depends_on.clone(),
                repository_ids: Vec::new(),
                priority: None,
                change: None,
            });
        }
    }
    tasks
}

/// 发起拆解/重拆：准入校验 → LLM → 确定性校验 → 新 Revision 草案落库。
pub async fn decompose(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    req: DecomposeRequest,
    actor: &AuditActor,
) -> Result<DecompositionOutcome, DecomposeError> {
    let plan: Option<(Uuid, Option<Uuid>, String)> = sqlx::query_as(
        "SELECT project_id, source_baseline_id, status FROM plan.task_plan \
         WHERE id = $1 AND tenant_id = $2",
    )
    .bind(req.plan_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let Some((plan_project, source_baseline, plan_status)) = plan else {
        return Err(DecomposeError::Pm(PmWriteError::NotFound(
            "task plan not found",
        )));
    };
    if plan_project != project_id {
        return Err(DecomposeError::Pm(PmWriteError::Validation(
            "plan does not belong to this project",
        )));
    }
    // Q2：重拆只允许 draft 计划（已批准计划的变更走退回或新计划）。
    if plan_status != "draft" {
        return Err(DecomposeError::Pm(PmWriteError::Conflict(
            "plan must be in draft for decomposition; return it first if approved",
        )));
    }
    let baseline_id = req
        .baseline_id
        .or(source_baseline)
        .ok_or(DecomposeError::Pm(PmWriteError::Validation(
            "no frozen baseline bound to this plan; pass baseline_id",
        )))?;
    let baseline_content =
        decomposition_admission(pool, tenant_id, project_id, baseline_id).await?;

    // 现有任务清单（重拆上下文；防重复拆解出平行任务）。
    let existing: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT COALESCE(plan_task_key, id::text), title, status FROM delivery.task \
         WHERE tenant_id = $1 AND project_id = $2 AND status NOT IN ('cancelled', 'superseded') \
         ORDER BY created_at LIMIT 100",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let payload = build_decompose_payload(
        pool,
        tenant_id,
        project_id,
        &baseline_content,
        req.instructions.as_deref(),
        &existing,
    )
    .await?;

    let ctx = TaskDecomposeContext {
        project_id,
        // 复用现有分解器 wire 契约：requirement_* 字段承载「实施依据」语义。
        requirement_id: baseline_id,
        requirement_code: "BASELINE".to_string(),
        requirement_title: "S03 Implementation Baseline".to_string(),
        markdown: payload,
        acceptance_criteria: Vec::new(),
        capability: Default::default(),
        roles: Vec::new(),
    };

    // LLM 拆解（复用 DK-F 生产模型链；无 Provider 显式失败）。
    let model = LlmWriteBackModel::new(pool.clone());
    let reply: ModelReply<crate::discovery_writeback::RawTaskBatch> = model
        .decompose_tasks(&ctx)
        .await
        .map_err(DecomposeError::Model)?;
    let tasks = draft_tasks_from_batch(&reply.value);

    // 确定性校验（系统拥有骨架；模型越界整条拒绝）。
    if let Err(message) = task_plan::validate_plan_tasks(&tasks) {
        return Err(DecomposeError::InvalidBatch(message));
    }

    // 重拆 diff 摘要（Q2）。
    let warnings = diff_against_current(pool, tenant_id, req.plan_id, &tasks).await?;

    let outcome = task_plan::create_plan_revision(
        pool,
        tenant_id,
        req.plan_id,
        task_plan::DraftPlanRevisionRequest {
            tasks,
            instructions: req.instructions.clone(),
        },
        actor,
    )
    .await?;
    let task_count = outcome
        .content
        .get("tasks")
        .and_then(|t| t.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    Ok(DecompositionOutcome {
        plan_id: req.plan_id,
        revision_id: outcome.id,
        version: outcome.version,
        task_count,
        warnings,
    })
}

/// 与计划最新修订逐键对比，产出 add/update/remove 摘要（供前端 diff 视图）。
async fn diff_against_current(
    pool: &PgPool,
    tenant_id: Uuid,
    plan_id: Uuid,
    new_tasks: &[PlanTaskDraft],
) -> Result<Vec<String>, PmWriteError> {
    let current: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT content FROM plan.task_plan_revision \
         WHERE plan_id = $1 AND tenant_id = $2 AND state IN ('draft', 'submitted', 'approved') \
         ORDER BY version DESC LIMIT 1",
    )
    .bind(plan_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let Some(current) = current else {
        return Ok(vec!["first revision; no diff baseline".to_string()]);
    };
    let old: Vec<PlanTaskDraft> = current
        .get("tasks")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    let old_map: std::collections::HashMap<&str, &PlanTaskDraft> =
        old.iter().map(|t| (t.key.as_str(), t)).collect();
    let new_keys: std::collections::HashSet<&str> =
        new_tasks.iter().map(|t| t.key.as_str()).collect();
    let mut warnings = Vec::new();
    let added: Vec<&str> = new_tasks
        .iter()
        .map(|t| t.key.as_str())
        .filter(|k| !old_map.contains_key(k))
        .collect();
    if !added.is_empty() {
        warnings.push(format!("added keys: {}", added.join(", ")));
    }
    let removed: Vec<&str> = old_map
        .keys()
        .copied()
        .filter(|k| !new_keys.contains(k))
        .collect();
    if !removed.is_empty() {
        warnings.push(format!(
            "removed keys (their materialized tasks will NOT be touched): {}",
            removed.join(", ")
        ));
    }
    let changed: Vec<&str> = new_tasks
        .iter()
        .filter(|t| {
            old_map
                .get(t.key.as_str())
                .map(|o| {
                    o.title != t.title
                        || o.description != t.description
                        || o.acceptance_criteria != t.acceptance_criteria
                        || o.depends_on != t.depends_on
                })
                .unwrap_or(false)
        })
        .map(|t| t.key.as_str())
        .collect();
    if !changed.is_empty() {
        warnings.push(format!("updated keys: {}", changed.join(", ")));
    }
    Ok(warnings)
}

/// 拆解错误（模型失败原样透传供上层留痕）。
#[derive(Debug)]
pub enum DecomposeError {
    Pm(PmWriteError),
    Model(ModelCallFailure),
    /// 模型输出违反系统侧骨架契约（键唯一/验收非空/依赖前向）。
    InvalidBatch(String),
}

impl From<PmWriteError> for DecomposeError {
    fn from(err: PmWriteError) -> Self {
        DecomposeError::Pm(err)
    }
}

impl From<DecomposeError> for PmWriteError {
    fn from(e: DecomposeError) -> Self {
        match e {
            DecomposeError::Pm(err) => err,
            DecomposeError::Model(failure) => {
                PmWriteError::ValidationMsg(format!("model call failed: {}", failure.error))
            }
            DecomposeError::InvalidBatch(message) => {
                PmWriteError::ValidationMsg(format!("model batch rejected: {message}"))
            }
        }
    }
}

/// 拆解历史（审计读模型：本计划的 AI 修订）。
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DecompositionRecord {
    pub revision_id: Uuid,
    pub version: i32,
    pub source: String,
    pub instructions: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn list_decompositions(
    pool: &PgPool,
    tenant_id: Uuid,
    plan_id: Uuid,
) -> Result<Vec<DecompositionRecord>, PmWriteError> {
    sqlx::query_as(
        "SELECT id, version, source, instructions, created_at \
         FROM plan.task_plan_revision \
         WHERE tenant_id = $1 AND plan_id = $2 AND source = 'ai' \
         ORDER BY version DESC",
    )
    .bind(tenant_id)
    .bind(plan_id)
    .fetch_all(pool)
    .await
    .map_err(PmWriteError::Db)
}
