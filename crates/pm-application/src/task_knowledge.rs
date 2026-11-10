//! S05-02 执行上下文注入：任务 → 知识文档的**显式锚点**与章节内容解析。
//!
//! 用户裁决（2026-11-10）：知识来源 = **显式锚点 + 人工确认**，不做自由检索。
//! 理由（蓝图 S05-02）：Agent 必须使用「正确的执行依据，而不是随意读取过时
//! 信息」——自由检索无法保证这一点。
//!
//! 三层事实：
//! - 锚点表 `delivery.task_knowledge_ref` 早已存在（指向项目文档身份表），
//!   但**生产链路从未接上**：唯一写路径是 `update_task` 的全量替换，而生产
//!   代码不调用它 → 表恒空。本模块补上生产侧写入。
//! - **ADR-06a 约束**：文档身份表与文档修订表是跨领域冻结的共享容器（守卫
//!   `kb-pc-contracts/tests/guard_ai_container_users.rs`）——**本模块不直接
//!   访问它们**：文档身份由调用方（既有白名单文件）解析后传入，章节正文取自
//!   章节索引表（不在冻结集合内）。
//! - 需求文档身份规则：`doc_key = "req-{requirement_id}"`（投影唯一绑定点
//!   `requirement_projection::bind_projection`）→ 任务绑定需求时可据此定位
//!   该需求的知识文档。
//! - 正文在库：`ai.document_section_index`（provider=`db-section`）按修订存
//!   section 内容 → 执行上下文可注入**内容**而非仅锚点。
//!
//! 版本语义：`pinned_revision_no` 为 NULL → 取该文档最新 published 修订
//! （默认，不落后）；非空 → 人工确认时钉定的版本（可复现）。

use uuid::Uuid;

/// `ai.document_section_index` 的 provider 名（与 `knowledge_indexer` 同源）。
pub const SECTION_PROVIDER: &str = "db-section";

/// 每篇文档最多注入的 section 数（超出部分丢弃，保契约体积可控）。
pub const MAX_SECTIONS_PER_DOC: i64 = 6;
/// 单 section 内容最大字符数（Rust 侧截断，超出以省略号收尾）。
pub const MAX_SECTION_CHARS: usize = 800;
/// 全文最多注入的 section 总数（按 is_required DESC 优先）。
pub const MAX_TOTAL_SECTIONS: i64 = 12;

/// 需求文档的 `doc_key` 规则（与 `requirement_projection::doc_key` 同源）。
pub fn requirement_doc_key(requirement_id: Uuid) -> String {
    format!("req-{requirement_id}")
}

/// 知识注入开关（`AI_PM_KNOWLEDGE_SECTION_INJECTION=off|on`），默认 off。
///
/// 与 `AI_PM_DISCUSSION_MEMORY_INJECTION` 同款纪律：新的契约输出面不随升级
/// 静默打开；拼写错误一律按 off（告警但不打开）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnowledgeInjectionMode {
    Off,
    On,
}

impl KnowledgeInjectionMode {
    pub fn parse(value: Option<&str>) -> Self {
        match value {
            Some("on") => Self::On,
            Some("off") | None => Self::Off,
            Some(other) => {
                tracing::warn!(
                    value = other,
                    "unknown AI_PM_KNOWLEDGE_SECTION_INJECTION value (expected off|on); staying off"
                );
                Self::Off
            }
        }
    }

    pub fn from_env() -> Self {
        Self::parse(
            std::env::var("AI_PM_KNOWLEDGE_SECTION_INJECTION")
                .ok()
                .as_deref(),
        )
    }

    pub fn enabled(self) -> bool {
        matches!(self, Self::On)
    }
}

/// 一条知识锚点（任务 → 文档）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct AnchorView {
    pub document_id: Uuid,
    pub doc_key: String,
    pub doc_type: String,
    pub canonical_path: String,
    /// 生效修订号（钉版优先，否则最新 published）。
    pub revision_no: Option<i32>,
    pub is_required: bool,
    /// 人工钉版修订号（NULL = 未钉版，取最新 published）。
    pub pinned_revision_no: Option<i32>,
}

/// 一段注入用章节内容（契约形状；`doc_key` / `is_required` 由锚点合并而来）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SectionView {
    pub document_id: Uuid,
    pub doc_key: String,
    /// 实际取用的修订号（审计口径：这次执行用的是哪一版）。
    pub revision_no: i32,
    pub section_no: i32,
    pub title: String,
    pub anchor: String,
    pub content: String,
    pub is_required: bool,
}

/// 章节原始行（未合并锚点元数据；`document_id` 由调用方按修订映射）。
#[derive(Debug, Clone)]
pub struct RawSection {
    pub revision_id: Uuid,
    pub section_no: i32,
    pub title: String,
    pub anchor: String,
    pub content: String,
}

/// 解析任务锚定文档的章节内容（执行上下文注入的数据源）。
///
/// 规则：钉版优先，否则最新 published；每篇文档最多
/// [`MAX_SECTIONS_PER_DOC`] 节，全文最多 [`MAX_TOTAL_SECTIONS`] 节；
/// `is_required` 优先。**无锚点/无 published 修订/无索引 → 返回空**（不是错误：
/// 知识注入是增强，不是执行前提）。
pub async fn resolve_sections(
    pool: &sqlx::PgPool,
    revision_ids: &[Uuid],
) -> Result<Vec<RawSection>, sqlx::Error> {
    if revision_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query_as::<_, (Uuid, i32, String, String, String)>(
        "SELECT s.document_revision_id, s.section_no, s.title, s.anchor, s.content            FROM ai.document_section_index s           WHERE s.document_revision_id = ANY($1) AND s.provider = $2           ORDER BY s.document_revision_id, s.section_no",
    )
    .bind(revision_ids)
    .bind(SECTION_PROVIDER)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(revision_id, section_no, title, anchor, content)| RawSection {
                revision_id,
                section_no,
                title,
                anchor,
                content: truncate(&content, MAX_SECTION_CHARS),
            },
        )
        .collect())
}

/// 绑定任务 → 文档的知识锚点（`is_required=true`，未钉版）。
///
/// `document_id` 由调用方解析——文档身份表是 ADR-06a 冻结的共享容器，新代码
/// 不得直接访问（守卫：`kb-pc-contracts/tests/guard_ai_container_users.rs`），
/// 因此文档身份解析留在既有白名单文件内。
pub async fn bind_document_anchor(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    document_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO delivery.task_knowledge_ref          (project_id, task_id, document_id, is_required, tenant_id)          VALUES ($1, $2, $3, true,                  (SELECT tenant_id FROM core.project WHERE id = $1))          ON CONFLICT (task_id, document_id) DO NOTHING",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(document_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 回填：文档投影完成后，为该文档**已关联的所有任务**补上锚点。
///
/// 覆盖时序：先绑任务后提交需求（投影晚于绑定）→ 本函数补齐；
/// 先提交需求后绑任务 → 绑定路径直接命中。
pub async fn backfill_document_anchor(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
    requirement_id: Uuid,
    document_id: Uuid,
) -> Result<u64, sqlx::Error> {
    let affected = sqlx::query(
        "INSERT INTO delivery.task_knowledge_ref          (project_id, task_id, document_id, is_required, tenant_id)          SELECT DISTINCT tr.project_id, tr.task_id, $3, true, tr.tenant_id            FROM delivery.task_requirement tr           WHERE tr.project_id = $1 AND tr.requirement_id = $2          ON CONFLICT (task_id, document_id) DO NOTHING",
    )
    .bind(project_id)
    .bind(requirement_id)
    .bind(document_id)
    .execute(&mut **tx)
    .await?;
    Ok(affected.rows_affected())
}

/// 字符截断（超长以省略号收尾，保证契约体积可控）。
fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let head: String = text.chars().take(max).collect();
    format!("{head}…")
}
