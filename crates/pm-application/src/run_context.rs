//! Slice D2：Run 级执行契约、租约凭据与进度上报
//! （`docs/ai-pm-slice-d2-mcp-context-v1.md` §四 §六）。
//!
//! 三方分工不变：本模块只做**读取/凭证校验/续租**，不启动进程（D3）、不写
//! 交付清单（D4）、不裁决任务状态（D5）。契约里的每个字段都能回到一张库表
//! （§4.2 映射表），不拼装、不带工具/prompt/model 字段。

use crate::project_goal::load_official_goal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::command_executor::{apply_task_transition, system_context, TransitionFacts};
use crate::run_dispatch::{
    audit_run, audit_run_opt, run_stage, run_status, RunDispatchConfig, RunDispatchError,
};
use pm_domain::TaskStatus;

/// 契约版本（形状冻结，D4 拥有定稿权）。
pub const CONTRACT_VERSION: i32 = 1;

/// 租约契约的明文编码（§五 R-G7 钉死）：32 字节随机数的小写 hex，共 64 字符；
/// 库内 `lease_token_hash` = sha256(明文字符串) 的小写 hex，严格相等比较。
pub const LEASE_TOKEN_HEX_LEN: usize = 64;

#[derive(Debug, thiserror::Error)]
pub enum LeaseError {
    /// Run 不存在或不在调用方项目内（不泄露跨项目存在性）。
    #[error("run not found in this project")]
    NotFound,
    /// 无 token、错 token、hash 为空（Model B / 存量 Run）、非活跃状态。
    #[error("lease credential rejected")]
    Forbidden,
    /// 租约已过期——即使 token 正确也必须拒绝（防僵尸 Run 复活）。
    #[error("lease expired")]
    Expired,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("lease token generation failed")]
    Random,
}

impl From<RunDispatchError> for LeaseError {
    fn from(err: RunDispatchError) -> Self {
        match err {
            RunDispatchError::Db(e) => LeaseError::Db(e),
            RunDispatchError::TaskNotFound | RunDispatchError::NoRunnableRun => {
                LeaseError::NotFound
            }
            RunDispatchError::RunAlreadyClaimed | RunDispatchError::TaskNotReady(_) => {
                LeaseError::Conflict(err.to_string())
            }
            other => LeaseError::Conflict(other.to_string()),
        }
    }
}

/// 生成 32 字节随机数的**小写 hex**（64 字符）。
pub fn generate_lease_token() -> Result<String, RunDispatchError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| RunDispatchError::Random)?;
    let mut out = String::with_capacity(LEASE_TOKEN_HEX_LEN);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    Ok(out)
}

/// sha256(明文 token 字符串) 的**小写 hex**；大写变体自然不相等（验收 C28）。
pub fn hash_lease_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// 一次有效租约的库内事实。
#[derive(Debug, Clone)]
pub struct LeaseState {
    pub run_id: Uuid,
    pub task_id: Uuid,
    pub status: String,
    pub stage: Option<String>,
    pub attempt_no: i32,
    pub lease_expires_at: DateTime<Utc>,
    pub client_name: Option<String>,
    pub client_version: Option<String>,
    /// T-032 T2：库内当前执行权世代（执行端请求携带值必须与此一致）。
    pub lease_generation: i32,
    /// 既有 `executor_session`：**非 None 表示平台已写入启动意图 / spawn 身份**
    /// （Model B）——执行端首报 `started` 不得覆盖它（T-031 实施裁定 2）。
    pub executor_session: Option<String>,
}

const LEASE_SELECT: &str = "SELECT r.id, r.task_id, r.status, r.stage, r.attempt_no, \
        r.lease_expires_at, r.lease_token_hash, r.client_name, r.client_version, \
        r.lease_generation, r.deadline_at, r.executor_session, \
        (r.lease_expires_at IS NOT NULL AND r.lease_expires_at > now()) AS lease_live \
   FROM delivery.task_run r \
  WHERE r.id = $1 AND r.project_id = $2";

fn lease_row(row: &sqlx::postgres::PgRow) -> Result<LeaseState, LeaseError> {
    let lease_expires_at: Option<DateTime<Utc>> = row.get("lease_expires_at");
    Ok(LeaseState {
        run_id: row.get("id"),
        task_id: row.get("task_id"),
        status: row.get("status"),
        stage: row.get("stage"),
        attempt_no: row.get("attempt_no"),
        lease_expires_at: lease_expires_at.ok_or(LeaseError::Expired)?,
        client_name: row.get("client_name"),
        client_version: row.get("client_version"),
        lease_generation: row.get("lease_generation"),
        executor_session: row.get("executor_session"),
    })
}

fn verify_row(
    row: &sqlx::postgres::PgRow,
    token: &str,
    generation: i32,
) -> Result<LeaseState, LeaseError> {
    let stored: Option<String> = row.get("lease_token_hash");
    // hash 为 NULL：无凭据 Run，一律拒绝，不 panic、不泄露（验收 C17）。
    let stored = stored.ok_or(LeaseError::Forbidden)?;
    if stored != hash_lease_token(token) {
        return Err(LeaseError::Forbidden);
    }
    // T-032 T2 终裁：MCP 必须同时验证 token 与 generation——**旧 generation
    // 永远不能重新获得授权**（租约所有权已递增到下一代）。
    if row.get::<i32, _>("lease_generation") != generation {
        return Err(LeaseError::Forbidden);
    }
    let status: String = row.get("status");
    if status != run_status::CLAIMED && status != run_status::RUNNING {
        return Err(LeaseError::Forbidden);
    }
    let live: bool = row.get("lease_live");
    if !live {
        return Err(LeaseError::Expired);
    }
    // T-032 T3 终裁：绝对执行截止优先于租约续期——deadline 已过的请求即使
    // token/generation/租约全部有效也必须拒绝。
    let deadline_at: Option<DateTime<Utc>> = row.get("deadline_at");
    if let Some(deadline) = deadline_at {
        if deadline <= Utc::now() {
            return Err(LeaseError::Expired);
        }
    }
    lease_row(row)
}

/// 只读校验（`get_run_context` 不续租、不加行锁）。
pub async fn verify_lease(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
    token: &str,
    generation: i32,
) -> Result<LeaseState, LeaseError> {
    let row = sqlx::query(LEASE_SELECT)
        .bind(run_id)
        .bind(project_id)
        .fetch_optional(pool)
        .await?
        .ok_or(LeaseError::NotFound)?;
    verify_row(&row, token, generation)
}

/// 事务内加行锁校验（续租/进度上报）。
///
/// D4 的提交管线复用本函数做 **fencing**——在事务内持 Run 行锁重验 token /
/// generation / 状态 / 过期，防止迟到结果污染已被回收的 Run。
pub(crate) async fn lock_lease(
    tx: &mut Transaction<'static, Postgres>,
    project_id: Uuid,
    run_id: Uuid,
    token: &str,
    generation: i32,
) -> Result<LeaseState, LeaseError> {
    let sql = format!("{LEASE_SELECT} FOR UPDATE OF r");
    let row = sqlx::query(&sql)
        .bind(run_id)
        .bind(project_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(LeaseError::NotFound)?;
    verify_row(&row, token, generation)
}

// ---------------------------------------------------------------------------
// 契约读模型（§4.1 / §4.2）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunContract {
    pub contract_version: i32,
    pub run: RunInfo,
    pub task: TaskInfo,
    /// 项目正式 Goal（I4-E）：最新 published 的 `project-goal` 修订；
    /// 建立前为 `null`（Agent 必须把 Goal 缺失当作显式信号，不得臆测目标）。
    #[serde(default)]
    pub goal: Option<GoalSnapshot>,
    /// I3 V1.1 简化裁决：不再有「能力」字段——`repositories` 即本次 Run 的
    /// 仓库范围，访问语义恒为「可读可写」（不在范围内的仓库不可访问）。
    pub acceptance_criteria: Vec<AcceptanceCriterionView>,
    pub repositories: Vec<RepositoryView>,
    pub knowledge_refs: Vec<KnowledgeRefView>,
    /// KP-3 Discussion Memory 注入（用户裁决 B，2026-11-08；差异说明 D-KP3-7）：按 Task
    /// 检索的 L1 记忆 CURRENT 版本摘要（task 本身 + 关联需求为主题，`kb.memory_ref`
    /// subject ⋈ `kb.memory_revision` CURRENT）。开关 `AI_PM_DISCUSSION_MEMORY_INJECTION`
    /// 默认 off；off 或无命中时**不序列化**——契约输出与 KP-3 前逐字节一致
    /// （additive，`CONTRACT_VERSION` 不跳）。空集合只表示「当前浅层记忆中没有找到相关
    /// 记录」，不代表过去从未讨论。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub discussion_memory: Vec<crate::discussion_memory::DiscussionMemoryView>,
    /// S05-02 知识注入（用户裁决 2026-11-10：显式锚点 + 人工确认）：任务**锚定**
    /// 的知识文档章节内容（`ai.document_section_index` 正文，非仅锚点）。
    /// 开关 `AI_PM_KNOWLEDGE_SECTION_INJECTION` 默认 off；off 或无锚点时**不
    /// 序列化**——契约输出与本字段引入前逐字节一致（additive，`CONTRACT_VERSION`
    /// 不跳）。空集合只表示「本任务未锚定可注入的知识文档」，不代表项目无知识。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub knowledge_sections: Vec<crate::task_knowledge::SectionView>,
    pub policy: PolicyView,
}

/// 正式 Goal 快照（只给身份与版本锚点，正文经知识读取路径获取）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalSnapshot {
    pub document_id: Uuid,
    pub revision_id: Uuid,
    pub revision_no: i32,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunInfo {
    pub run_id: Uuid,
    pub attempt_no: i32,
    pub stage: Option<String>,
    pub timeout_seconds: Option<i32>,
    pub lease: LeaseInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseInfo {
    pub expires_at: DateTime<Utc>,
    pub heartbeat_interval_secs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInfo {
    pub task_id: Uuid,
    pub title: String,
    pub task_type: Option<String>,
    pub status: String,
    pub revision: i64,
    pub priority: String,
    pub goal: Option<String>,
    pub description: Option<String>,
    pub constraints: serde_json::Value,
    pub deliverables: Vec<DeliverableView>,
    pub git_branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliverableView {
    pub id: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceCriterionView {
    pub id: Uuid,
    pub ordinal: i32,
    pub text: String,
    pub verification_kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryView {
    pub repository_id: Uuid,
    pub name: String,
    pub base_ref: String,
    pub base_commit: String,
    pub write_access: bool,
    pub workspace_path: String,
    pub path_scope: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeRefView {
    pub document_id: Uuid,
    pub doc_key: String,
    pub doc_type: String,
    pub canonical_path: String,
    /// live 解析：每次读取都取 `document_revision` 的最新版本（§4.2 R-F11）；
    /// 若锚点被人工**钉版**（`pinned_revision_no`），则以钉版为准（可复现）。
    pub revision_no: Option<i32>,
    pub is_required: bool,
    /// 人工钉版修订号（NULL = 未钉版，取最新 published）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_revision_no: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyView {
    pub approval_required: bool,
    pub approval_status: String,
    pub execution_enabled: bool,
    pub snapshot_scope: String,
}

/// I3 V1.1 简化裁决：本次 Run 的每仓**访问**定格快照
/// （`policy_snapshot.repo_access`）。
///
/// 执行端只读这份快照；**服务端**在执行期做实时校验时按「快照（上界）∩ 当前
/// 现状（仓库仍 verified ∧ 项目未归档）」取交集——状态变化即时生效，
/// 但已定格的快照不被改写。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoAccessView {
    pub repository_id: Uuid,
    /// 访问档位（当前唯一档位 `read_write`）。
    pub access: String,
}

/// 读取本 Run 的完整执行契约（只读、不续租）。
///
/// T-032 T2：`generation` 为执行端领取时拿到的执行权世代，服务端双验证。
/// Discussion Memory 注入面按环境开关 `AI_PM_DISCUSSION_MEMORY_INJECTION` 判定
/// （默认 off，见 [`get_run_context_with`]）。
pub async fn get_run_context(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
    lease_token: &str,
    generation: i32,
) -> Result<RunContract, LeaseError> {
    get_run_context_with(
        pool,
        project_id,
        run_id,
        lease_token,
        generation,
        crate::discussion_memory::DiscussionMemoryInjectionMode::from_env(),
    )
    .await
}

/// [`get_run_context`] 的显式注入开关形态（测试与需要固定口径的调用方使用，避免
/// 依赖进程级环境变量）。
pub async fn get_run_context_with(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
    lease_token: &str,
    generation: i32,
    injection: crate::discussion_memory::DiscussionMemoryInjectionMode,
) -> Result<RunContract, LeaseError> {
    get_run_context_with_injection(
        pool,
        project_id,
        run_id,
        lease_token,
        generation,
        injection,
        crate::task_knowledge::KnowledgeInjectionMode::from_env(),
    )
    .await
}

/// [`get_run_context_with`] 的双开关形态：Discussion Memory 注入 + **S05-02
/// 知识章节注入**。两个开关独立，互不影响（各自默认 off）。
pub async fn get_run_context_with_injection(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
    lease_token: &str,
    generation: i32,
    injection: crate::discussion_memory::DiscussionMemoryInjectionMode,
    knowledge_injection: crate::task_knowledge::KnowledgeInjectionMode,
) -> Result<RunContract, LeaseError> {
    let lease = verify_lease(pool, project_id, run_id, lease_token, generation).await?;

    let run = sqlx::query(
        "SELECT attempt_no, stage, timeout_seconds, lease_expires_at, policy_snapshot \
           FROM delivery.task_run WHERE id = $1 AND project_id = $2",
    )
    .bind(run_id)
    .bind(project_id)
    .fetch_one(pool)
    .await?;
    let policy_snapshot: Option<serde_json::Value> = run.get("policy_snapshot");
    let policy = policy_from_snapshot(policy_snapshot.as_ref());
    // I3 V1.1 简化裁决：访问档位不再随契约下发——执行端可见的仓库清单就是
    // `repositories`（Task 范围快照，含 write_access），访问语义恒为「可读可写」。

    let task = sqlx::query(
        "SELECT id, title, task_type, status, revision, priority, goal, description, \
                constraints, deliverables, git_branch \
           FROM delivery.task WHERE id = $1 AND project_id = $2",
    )
    .bind(lease.task_id)
    .bind(project_id)
    .fetch_one(pool)
    .await?;
    let deliverables: serde_json::Value = task.get("deliverables");
    let deliverables: Vec<DeliverableView> =
        serde_json::from_value(deliverables).unwrap_or_default();

    let criteria = sqlx::query(
        "SELECT id, ordinal, text, verification_kind FROM delivery.task_acceptance_criterion \
          WHERE task_id = $1 ORDER BY ordinal",
    )
    .bind(lease.task_id)
    .fetch_all(pool)
    .await?;
    let acceptance_criteria = criteria
        .into_iter()
        .map(|row| AcceptanceCriterionView {
            id: row.get("id"),
            ordinal: row.get("ordinal"),
            text: row.get("text"),
            verification_kind: row.get("verification_kind"),
        })
        .collect();

    let repositories = sqlx::query(
        "SELECT trr.repository_id, pr.name, trr.base_ref, trr.base_commit, trr.write_access, \
                trr.workspace_path, trr.path_scope \
           FROM delivery.task_run_repository trr \
           JOIN integration.project_repository pr \
             ON pr.id = trr.repository_id AND pr.project_id = trr.project_id \
          WHERE trr.run_id = $1 AND trr.project_id = $2 \
          ORDER BY trr.repository_id",
    )
    .bind(run_id)
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    let repositories = repositories
        .into_iter()
        .map(|row| RepositoryView {
            repository_id: row.get("repository_id"),
            name: row.get("name"),
            base_ref: row.get("base_ref"),
            base_commit: row.get("base_commit"),
            write_access: row.get("write_access"),
            workspace_path: row.get("workspace_path"),
            path_scope: row.get("path_scope"),
        })
        .collect();

    // S05-02：知识锚点（钉版优先，否则最新 published——§4.2 R-F11 的 live 解析
    // 在「人工钉版」时让位于钉版，因为钉版才是被确认的执行依据）。
    // ADR-06a：文档身份/修订表是冻结的共享容器——本文件在既有使用方白名单内，
    // 锚点身份与修订的解析只落在这里；章节正文取自章节索引表。
    let knowledge_refs = sqlx::query(
        "SELECT kr.document_id, pd.doc_key, pd.doc_type, pd.canonical_path, \
                COALESCE(kr.pinned_revision_no, latest.revision_no) AS revision_no, \
                kr.is_required, kr.pinned_revision_no, \
                COALESCE(pinned.id, latest.id) AS revision_id \
           FROM delivery.task_knowledge_ref kr \
           JOIN ai.project_document pd \
             ON pd.id = kr.document_id AND pd.project_id = kr.project_id \
           LEFT JOIN LATERAL ( \
               SELECT dr.id, dr.revision_no FROM ai.document_revision dr \
                WHERE dr.document_id = pd.id AND dr.status = 'published' \
                ORDER BY dr.revision_no DESC LIMIT 1 \
           ) latest ON TRUE \
           LEFT JOIN LATERAL ( \
               SELECT dr.id FROM ai.document_revision dr \
                WHERE dr.document_id = pd.id AND dr.revision_no = kr.pinned_revision_no \
                  AND dr.status = 'published' \
                ORDER BY dr.revision_no DESC LIMIT 1 \
           ) pinned ON TRUE \
          WHERE kr.task_id = $1 AND kr.project_id = $2 \
          ORDER BY kr.is_required DESC, pd.doc_key",
    )
    .bind(lease.task_id)
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    let mut anchor_meta: Vec<AnchorMeta> = Vec::new();
    let mut revision_ids: Vec<Uuid> = Vec::new();
    let knowledge_refs = knowledge_refs
        .into_iter()
        .map(|row| {
            let document_id: Uuid = row.get("document_id");
            let doc_key: String = row.get("doc_key");
            let is_required: bool = row.get("is_required");
            let revision_no: Option<i32> = row.get("revision_no");
            let revision_id: Option<Uuid> = row.get("revision_id");
            if let Some(revision_id) = revision_id {
                revision_ids.push(revision_id);
            }
            anchor_meta.push(AnchorMeta {
                revision_id,
                document_id,
                doc_key: doc_key.clone(),
                is_required,
                revision_no,
            });
            KnowledgeRefView {
                document_id,
                doc_key,
                doc_type: row.get("doc_type"),
                canonical_path: row.get("canonical_path"),
                revision_no,
                is_required,
                pinned_revision_no: row.get("pinned_revision_no"),
            }
        })
        .collect();

    // S05-02：锚定文档的**章节正文**注入（开关 off = 不查询，契约逐字节不变）。
    let knowledge_sections = if knowledge_injection.enabled() && !revision_ids.is_empty() {
        let raw = crate::task_knowledge::resolve_sections(pool, &revision_ids).await?;
        let mut sections: Vec<crate::task_knowledge::SectionView> = Vec::new();
        for item in raw {
            let Some(meta) = anchor_meta
                .iter()
                .find(|meta| meta.revision_id == Some(item.revision_id))
            else {
                continue;
            };
            let Some(revision_no) = meta.revision_no else {
                continue;
            };
            sections.push(crate::task_knowledge::SectionView {
                document_id: meta.document_id,
                doc_key: meta.doc_key.clone(),
                revision_no,
                section_no: item.section_no,
                title: item.title,
                anchor: item.anchor,
                content: item.content,
                is_required: meta.is_required,
            });
        }
        // 每文档 ≤ MAX_SECTIONS_PER_DOC 节；全文 ≤ MAX_TOTAL_SECTIONS 节；
        // is_required 优先。
        let mut per_doc: std::collections::HashMap<Uuid, i32> = std::collections::HashMap::new();
        let mut kept: Vec<crate::task_knowledge::SectionView> = Vec::new();
        for section in sections {
            let count = per_doc.entry(section.document_id).or_insert(0);
            if *count as i64 >= crate::task_knowledge::MAX_SECTIONS_PER_DOC {
                continue;
            }
            *count += 1;
            kept.push(section);
        }
        kept.sort_by(|a, b| {
            b.is_required
                .cmp(&a.is_required)
                .then_with(|| a.doc_key.cmp(&b.doc_key))
                .then_with(|| a.section_no.cmp(&b.section_no))
        });
        kept.truncate(crate::task_knowledge::MAX_TOTAL_SECTIONS as usize);
        kept
    } else {
        Vec::new()
    };

    // I4-E：正式 Goal 快照（缺 Goal 是显式信号，不是错误）。
    let goal = load_official_goal(pool, project_id)
        .await?
        .map(|goal| GoalSnapshot {
            document_id: goal.document_id,
            revision_id: goal.revision_id,
            revision_no: goal.revision_no,
            content_hash: goal.content_hash,
        });

    // KP-3：Discussion Memory 按 Task 检索注入（只取 CURRENT 版本；开关 off = 不查询）。
    let discussion_memory = if injection.enabled() {
        crate::discussion_memory::discussion_memory_for_task(pool, project_id, lease.task_id)
            .await?
    } else {
        Vec::new()
    };

    Ok(RunContract {
        contract_version: CONTRACT_VERSION,
        goal,
        run: RunInfo {
            run_id,
            attempt_no: run.get("attempt_no"),
            stage: run.get("stage"),
            timeout_seconds: run.get("timeout_seconds"),
            lease: LeaseInfo {
                expires_at: run.get("lease_expires_at"),
                heartbeat_interval_secs: RunDispatchConfig::heartbeat_interval_secs(),
            },
        },
        task: TaskInfo {
            task_id: lease.task_id,
            title: task.get("title"),
            task_type: task.get("task_type"),
            status: task.get("status"),
            revision: task.get("revision"),
            priority: task.get("priority"),
            goal: task.get("goal"),
            description: task.get("description"),
            constraints: task.get("constraints"),
            deliverables,
            git_branch: task.get("git_branch"),
        },
        acceptance_criteria,
        repositories,
        knowledge_refs,
        discussion_memory,
        knowledge_sections,
        policy,
    })
}

fn policy_from_snapshot(snapshot: Option<&serde_json::Value>) -> PolicyView {
    let empty = serde_json::json!({});
    let snapshot = snapshot.unwrap_or(&empty);
    PolicyView {
        approval_required: snapshot
            .get("approval_required")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        approval_status: snapshot
            .get("approval_status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
        execution_enabled: snapshot
            .get("execution_enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        snapshot_scope: snapshot
            .get("snapshot_scope")
            .and_then(|v| v.as_str())
            .unwrap_or("d1-minimal")
            .to_string(),
    }
}

// ---------------------------------------------------------------------------
// 续租 / 进度上报（§4.3）
// ---------------------------------------------------------------------------

/// 续租后的租约事实。
#[derive(Debug, Clone)]
pub struct RenewedLease {
    pub run_id: Uuid,
    pub stage: Option<String>,
    pub status: String,
    pub lease_expires_at: DateTime<Utc>,
    pub task_status: String,
}

/// 对外续租通道（D2）：凭 token、含过期检查、单事务、带审计。
///
/// 与既有 `run_dispatch::renew_lease` 的差别是四项都补齐：token 凭证、过期拒绝、
/// 同事务、审计留痕。
///
/// T-032：`generation` 双验证（T2）；本通道是**执行端 Agent 心跳**（T4）——
/// 刷新 `last_agent_heartbeat_at`；deadline 已过一律拒绝（T3，verify_row 内）。
pub async fn renew_lease_by_token(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
    lease_token: &str,
    cfg: &RunDispatchConfig,
    generation: i32,
) -> Result<RenewedLease, LeaseError> {
    let mut tx = pool.begin().await?;
    let renewed =
        renew_lease_in_tx(&mut tx, project_id, run_id, lease_token, cfg, generation).await?;
    tx.commit().await?;
    Ok(renewed)
}

async fn renew_lease_in_tx(
    tx: &mut Transaction<'static, Postgres>,
    project_id: Uuid,
    run_id: Uuid,
    lease_token: &str,
    cfg: &RunDispatchConfig,
    generation: i32,
) -> Result<RenewedLease, LeaseError> {
    let lease = lock_lease(tx, project_id, run_id, lease_token, generation).await?;
    let expires_at: DateTime<Utc> = sqlx::query_scalar(
        "UPDATE delivery.task_run \
            SET lease_expires_at = now() + make_interval(secs => $2::float8), heartbeat_at = now(), \
                last_agent_heartbeat_at = now() \
          WHERE id = $1 RETURNING lease_expires_at",
    )
    .bind(run_id)
    .bind(cfg.lease_secs)
    .fetch_one(&mut **tx)
    .await?;
    audit_run(
        tx,
        project_id,
        "renew_run_lease",
        run_id,
        serde_json::json!({ "lease_expires_at": expires_at, "stage": lease.stage }),
    )
    .await?;
    let task_status: String = sqlx::query_scalar("SELECT status FROM delivery.task WHERE id = $1")
        .bind(lease.task_id)
        .fetch_one(&mut **tx)
        .await?;
    Ok(RenewedLease {
        run_id,
        stage: lease.stage,
        status: lease.status,
        lease_expires_at: expires_at,
        task_status,
    })
}

/// `pm_report_progress` 的入参语义（§4.3）。
#[derive(Debug, Clone, Copy)]
pub struct ProgressReport<'a> {
    pub stage: &'a str,
    pub note: Option<&'a str>,
    pub blocked: bool,
}

/// 进度上报 = 续租 +（首次 `started` 且平台尚未写入启动意图）阶段推进 + 审计，
/// 全部**单事务**。
///
/// `claimed → running` 由平台经 `apply_task_transition` 写（R1 红线：不允许
/// 第二条绕过状态机的写入路径）；Run 自身的 `status/stage` 与执行会话同事务写。
///
/// T-032：`generation` 双验证（T2）；本通道刷新 **Agent 心跳**（T4）——
/// `last_agent_heartbeat_at = now()`；**仅 stage 变化才刷 `last_progress_at`**
/// （心跳不能自动视为执行进展）；首报 `started` 分支只在平台尚未写入
/// `executor_session`（无启动意图）时才写 mcp session——平台 spawn（Model B）
/// 的 session 由编排器前置写入，执行端**不得覆盖**。
pub async fn report_progress(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
    lease_token: &str,
    report: ProgressReport<'_>,
    cfg: &RunDispatchConfig,
    generation: i32,
) -> Result<RenewedLease, LeaseError> {
    report_progress_impl(
        pool,
        project_id,
        run_id,
        lease_token,
        report,
        cfg,
        generation,
        None,
    )
    .await
}

/// 同 [`report_progress`]，但由调用方**显式**给出完整审计身份（T-088 MCP 路径）。
#[allow(clippy::too_many_arguments)]
pub async fn report_progress_as(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
    lease_token: &str,
    report: ProgressReport<'_>,
    cfg: &RunDispatchConfig,
    generation: i32,
    actor: &crate::actor::AuditActor,
) -> Result<RenewedLease, LeaseError> {
    report_progress_impl(
        pool,
        project_id,
        run_id,
        lease_token,
        report,
        cfg,
        generation,
        Some(actor),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn report_progress_impl(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
    lease_token: &str,
    report: ProgressReport<'_>,
    cfg: &RunDispatchConfig,
    generation: i32,
    actor: Option<&crate::actor::AuditActor>,
) -> Result<RenewedLease, LeaseError> {
    let mut tx = pool.begin().await?;
    let lease = lock_lease(&mut tx, project_id, run_id, lease_token, generation).await?;

    let stage_changed = lease.stage.as_deref() != Some(report.stage);
    let expires_at: DateTime<Utc> = sqlx::query_scalar(
        "UPDATE delivery.task_run \
            SET lease_expires_at = now() + make_interval(secs => $2::float8), heartbeat_at = now(), \
                last_agent_heartbeat_at = now(), \
                last_progress_at = CASE WHEN $3::text IS DISTINCT FROM stage \
                                        THEN now() ELSE last_progress_at END \
          WHERE id = $1 RETURNING lease_expires_at",
    )
    .bind(run_id)
    .bind(cfg.lease_secs)
    .bind(report.stage)
    .fetch_one(&mut *tx)
    .await?;
    // 进展判定已在 SQL CASE 内完成（与 stage 读取同事务一致）；该布尔供
    // 事件面（0013）判定「阶段推进」落账。
    audit_run_opt(
        &mut tx,
        project_id,
        actor,
        "renew_run_lease",
        run_id,
        serde_json::json!({ "lease_expires_at": expires_at, "stage": report.stage }),
    )
    .await?;

    let mut run_status_after = lease.status.clone();
    let mut stage_after = lease.stage.clone();
    let mut task_status: String =
        sqlx::query_scalar("SELECT status FROM delivery.task WHERE id = $1")
            .bind(lease.task_id)
            .fetch_one(&mut *tx)
            .await?;

    // T-032：平台已写入启动意图（executor_session 非空 = Model B spawn）→
    // 首报不写 mcp session、claimed→running 由编排器推进（T-031 实施裁定 2 +
    // T1 终裁：执行端不得覆盖平台 session）。
    let mut first_started = false;
    if report.stage == run_stage::STARTED
        && lease.status == run_status::CLAIMED
        && lease.executor_session.is_none()
    {
        first_started = true;
        // 首报 started：任务状态经唯一受守卫通道推进。
        let ctx = system_context(&mut tx, project_id)
            .await
            .map_err(|e| LeaseError::Conflict(e.to_string()))?;
        apply_task_transition(
            &mut tx,
            lease.task_id,
            None,
            TaskStatus::Running,
            &TransitionFacts::for_dispatch(),
            &ctx,
        )
        .await
        .map_err(|e| LeaseError::Conflict(e.to_string()))?;
        task_status = TaskStatus::Running.as_str().to_string();

        let executor_session = match (&lease.client_name, &lease.client_version) {
            (Some(name), Some(version)) => format!("mcp:{name}:{version}"),
            (Some(name), None) => format!("mcp:{name}"),
            (None, _) => "mcp:unknown".to_string(),
        };
        sqlx::query(
            "UPDATE delivery.task_run \
                SET status = 'running', stage = 'started', started_at = now(), \
                    executor_kind = 'mcp', executor_session = $2 \
              WHERE id = $1",
        )
        .bind(run_id)
        .bind(&executor_session)
        .execute(&mut *tx)
        .await?;
        run_status_after = run_status::RUNNING.to_string();
        stage_after = Some(run_stage::STARTED.to_string());
        audit_run_opt(
            &mut tx,
            project_id,
            actor,
            "run_stage_started",
            run_id,
            serde_json::json!({
                "stage": run_stage::STARTED,
                "executor_kind": "mcp",
                "executor_session": executor_session,
            }),
        )
        .await?;
    }

    if report.blocked {
        audit_run_opt(
            &mut tx,
            project_id,
            actor,
            "run_blocked",
            run_id,
            serde_json::json!({ "note": report.note, "stage": report.stage }),
        )
        .await?;
    } else if let Some(note) = report.note {
        audit_run_opt(
            &mut tx,
            project_id,
            actor,
            "run_progress_reported",
            run_id,
            serde_json::json!({ "note": note, "stage": report.stage }),
        )
        .await?;
    }

    // S05 监控事件面（0013）：MCP 通道启动/阶段推进落账（同事务；幂等键
    // generation+stage——同阶段重报不重复，续领（generation+1）后同阶段可再记）。
    if first_started {
        crate::run_events::record_run_event(
            &mut tx,
            run_id,
            crate::run_events::RunEvent {
                event_type: crate::run_events::event_type::RUN_STARTED,
                generation: Some(generation),
                dedupe_tag: "",
                detail: Some(serde_json::json!({ "executor_kind": "mcp" })),
            },
        )
        .await?;
    } else if stage_changed {
        crate::run_events::record_run_event(
            &mut tx,
            run_id,
            crate::run_events::RunEvent {
                event_type: crate::run_events::event_type::RUN_STAGE_CHANGED,
                generation: Some(generation),
                dedupe_tag: report.stage,
                detail: report.note.map(|n| serde_json::json!({ "note": n })),
            },
        )
        .await?;
    }

    tx.commit().await?;
    Ok(RenewedLease {
        run_id,
        stage: stage_after,
        status: run_status_after,
        lease_expires_at: expires_at,
        task_status,
    })
}


/// 知识锚点装配元数据（章节合并时的锚点侧信息）。
#[derive(Debug, Clone)]
struct AnchorMeta {
    revision_id: Option<Uuid>,
    document_id: Uuid,
    doc_key: String,
    is_required: bool,
    revision_no: Option<i32>,
}
