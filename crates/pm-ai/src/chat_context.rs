//! chat 段知识注入（整合计划 Batch 3A + R2/B09-01 项目记录节）。
//!
//! **为什么 chat 需要注入**：原 chat 回复链的模型上下文只有「固定语言镜像
//! system prompt + 最近 20 轮纯文本」（F-5），AI 不知道当前对着哪个项目/
//! 仓库说话——UI 承诺的「只读上下文」是空头承诺（DK-0 F20 已如实登记）。
//!
//! ## 与 DK-C（访谈注入）的关系
//!
//! 复用 [`crate::discovery_context`] 的**分节与记账结构**，但**独立预算**：
//! 访谈 36,000 字符（[`discovery_context::TOTAL_BUDGET_CHARS`]）不动，
//! chat 用 16,000（[`CHAT_TOTAL_BUDGET_CHARS`]）——访谈是长程结构化任务，
//! chat 是一问一答，二者不该共享额度。
//!
//! ## 注入面（三节）
//!
//! ```text
//! project_profile  (S1) ← core.project（名称/描述/状态）
//! project_records  (S2) ← delivery/design/discovery/governance（正式记录投影，
//!                         R2/B09-01：需求/设计/ADR/任务/风险/待人工/审计，
//!                         由 project_records::build_project_records_body 构建）
//! repository_facts (S3) ← integration.project_repository（仓库/分支/角色）
//! ```
//!
//! 注入源 = 线程所属项目的**实时投影** + 线程锚点（仓库节）：锚点声明
//! 「这个线程对着哪个仓库」，服务端按锚点**重新投影**实时事实。记录节
//! 每次回复前实时查询（无缓存）——记录变更后下一轮即取最新版本。
//!
//! ## 安全三闸（沿 DK-C §5.5）
//!
//! 1. **注入隔离**：`<context>` 内一律按数据对待，标题即来源标注；
//! 2. **不注入凭据**：投影不触碰任何 provider/token/secret 列（D1 扫描断言）；
//! 3. **锚点不即事实**：锚点是**定位**不是**内容**——仓库名/分支从库里重读，
//!    锚点只用于「找哪个仓库」，伪造的锚点查不到即零注入。

use sqlx::Row;
use uuid::Uuid;

use crate::discovery_context::{assemble_chat, ContextSection, SectionDraft, CHAT_SECTION_ORDER};

/// chat 注入总量硬预算（字符）。与访谈的 36,000 独立。
///
/// R2（B09-01）从 8,000 提到 16,000：新增项目正式记录节（上限 6,000）——
/// 「AI 项目负责人能读到项目事实」是 1.0 的核心承诺，预算不足以承载记录
/// 投影时宁可压缩对象节/仓库节，不能没有记录节。
pub const CHAT_TOTAL_BUDGET_CHARS: usize = 16_000;
/// 单节上限：项目画像 / 项目记录 / 仓库事实。
pub const CHAT_CAP_PROJECT_PROFILE: usize = 2_000;
pub const CHAT_CAP_PROJECT_RECORDS: usize = 6_000;
pub const CHAT_CAP_REPOSITORY_FACTS: usize = 4_000;

/// 对象节（S3+）总预算。
pub const CHAT_CAP_OBJECT_CONTEXT: usize = 2_000;

/// 分节键。
pub const CHAT_KEY_PROJECT_PROFILE: &str = "project_profile";
/// 项目正式记录投影（R2/B09-01）。
pub const CHAT_KEY_PROJECT_RECORDS: &str = "project_records";
pub const CHAT_KEY_REPOSITORY_FACTS: &str = "repository_facts";
/// 对象节键前缀（B3 / CX2：按对象分节，键 = `object:<type>:<object_id>`）。
pub const CHAT_KEY_OBJECT_PREFIX: &str = "object:";

/// 注入载荷（含记账）。
#[derive(Debug, Clone)]
pub struct ChatContext {
    pub sections: Vec<ContextSection>,
    pub sections_dropped: Vec<String>,
    pub chars_total: usize,
}

impl ChatContext {
    /// 记账载荷（并入 `llm_call_record.metadata`）。
    pub fn metadata(&self) -> serde_json::Value {
        serde_json::json!({
            "sections_included": self.sections.iter().map(|s| s.key.clone()).collect::<Vec<_>>(),
            "sections_dropped": self.sections_dropped.clone(),
            "chars_total": self.chars_total,
        })
    }

    /// 拼进模型上下文的正文（空 → 空串，调用方据此跳过注入）。
    pub fn render(&self) -> String {
        self.sections
            .iter()
            .map(|section| format!("## {}\n{}", section.title, section.body))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// 是否为空注入（无任何小节 → 调用方不注入，保持旧行为）。
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }
}

/// 只读投影 SQL（单条 SELECT；无 INSERT/UPDATE/DELETE）。
///
/// D1 静态断言对象：不得出现 provider/凭据列。
const SQL_PROJECT_PROFILE: &str = "SELECT p.name, COALESCE(p.description, '') AS description, \
            p.status \
     FROM core.project p WHERE p.tenant_id = $1 AND p.id = $2";

const SQL_REPOSITORY_FACTS: &str =
    "SELECT r.name, COALESCE(r.default_branch, '') AS default_branch, \
            r.role, r.status, COALESCE(r.head_commit_sha, '') AS head_commit_sha \
     FROM integration.project_repository r \
     WHERE r.tenant_id = $1 AND r.project_id = $2 AND r.name = $3 \
     LIMIT 1";

/// 构造 chat 注入。
///
/// - `anchor_repository`：线程锚点里的仓库名；None → 无仓库节（只注入项目画像）；
/// - 锚点指向的仓库查不到 → 仓库节缺席（**不报错、不伪造**）。
/// - 项目查不到 → 零注入（返回空 `ChatContext`）。
pub async fn build_chat_context(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    anchor_repository: Option<&str>,
) -> Result<ChatContext, sqlx::Error> {
    let mut drafts: Vec<SectionDraft> = Vec::new();

    // S1 项目画像。
    let profile = sqlx::query(SQL_PROJECT_PROFILE)
        .bind(tenant_id)
        .bind(project_id)
        .fetch_optional(pool)
        .await?;
    if let Some(row) = profile {
        let name: String = row.get("name");
        let description: String = row.get("description");
        let status: String = row.get("status");
        let mut body = format!("项目名称：{name}\n项目状态：{status}");
        if !description.trim().is_empty() {
            body.push_str(&format!("\n项目说明：{}", description.trim()));
        }
        drafts.push(SectionDraft::new(
            CHAT_KEY_PROJECT_PROFILE,
            "项目画像",
            body,
        ));

        // S2 项目正式记录投影（R2/B09-01：需求/设计/ADR/任务/风险/待人工/审计）。
        // 只对**存在的项目**投影（缺失项目 → 零注入，防探测语义与画像一致）；
        // 实时查询、预算内整行裁剪；查询失败 → 整节缺席（与仓库节同语义：
        // 不报错、不伪造），由外层 WARN 记账。
        match crate::project_records::build_project_records_body(pool, tenant_id, project_id).await
        {
            Ok(records_body) => drafts.push(SectionDraft::new(
                CHAT_KEY_PROJECT_RECORDS,
                "项目正式记录",
                records_body,
            )),
            Err(err) => {
                tracing::warn!(
                    %project_id,
                    error = %err,
                    "chat context: project records projection failed; section omitted"
                );
            }
        }
    }

    // S3 仓库事实（按锚点定位实时仓库）。
    if let Some(repo_name) = anchor_repository.map(str::trim).filter(|n| !n.is_empty()) {
        let repo = sqlx::query(SQL_REPOSITORY_FACTS)
            .bind(tenant_id)
            .bind(project_id)
            .bind(repo_name)
            .fetch_optional(pool)
            .await?;
        if let Some(row) = repo {
            let name: String = row.get("name");
            let branch: String = row.get("default_branch");
            let role: String = row.get("role");
            let status: String = row.get("status");
            let head: String = row.get("head_commit_sha");
            let mut body = format!(
                "仓库：{name}\n默认分支：{}\n仓库角色：{role}\n仓库状态：{status}",
                if branch.trim().is_empty() {
                    "（未设置）"
                } else {
                    branch.trim()
                }
            );
            if !head.trim().is_empty() {
                body.push_str(&format!("\nHEAD 提交：{}", head.trim()));
            }
            drafts.push(SectionDraft::new(
                CHAT_KEY_REPOSITORY_FACTS,
                "仓库事实",
                body,
            ));
        }
    }

    Ok(assemble_chat(drafts))
}

/// 从线程锚点 JSON 读出仓库名（锚点缺失 → None）。
pub fn anchor_repository(anchor: Option<&serde_json::Value>) -> Option<String> {
    anchor
        .and_then(|value| value.get("repository_name"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

/// chat 分节顺序（供测试断言；与 `assemble_chat` 的固定顺序同源）。
pub fn chat_section_order() -> [&'static str; 3] {
    CHAT_SECTION_ORDER
}

/// 构造带对象节（S3+）的 chat 注入（B3 / CX2）。
///
/// 在 S1（项目画像）/ S2（仓库事实）之外，把会话上下文条目投影为**按对象
/// 分节**的 S3+（键 `object:<type>:<object_id>`，每对象独立记账）：
///
/// - **对象预算 2,000**（CX2 终裁：原 8,000 的余量，不提总预算）；
/// - 条目顺序沿用 `load_injection_sources` 的 `updated_at DESC, id ASC`
///   （稳定次排序，超预算裁剪结果**可重复**）；
/// - 逐对象装配：`meta` 行必带，`body`（已批准正文摘要）在预算内才带；
///   **放不下的对象整节裁掉**，键进 `sections_dropped`（逐对象记账，
///   不是只记总字符数）；
/// - 对象节与 S1/S2 互不挤占：S1/S2 走 [`build_chat_context`] 的既有预算，
///   对象节独立 2,000——总注入量仍 ≤ 8,000。
///
/// 条目由调用方（conversation_reply）经
/// `pm_application::ai_context_items::load_injection_sources` 取得——
/// 本函数纯装配，无对象 IO（正文已在投影层重读，SQL-07）。
pub async fn build_chat_context_with_objects(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    anchor_repository: Option<&str>,
    objects: &[pm_application::ai_context_items::InjectionSource],
) -> Result<ChatContext, sqlx::Error> {
    let mut context = build_chat_context(pool, tenant_id, project_id, anchor_repository).await?;

    let mut remaining = CHAT_CAP_OBJECT_CONTEXT;
    let mut object_sections: Vec<ContextSection> = Vec::new();
    for item in objects {
        // 元数据行必带（标题 + meta）；正文仅在预算内追加。
        let meta_line = format!("{} · {}", item.title, item.meta);
        let mut body_chars = meta_line.chars().count();
        let mut body = meta_line;
        if let Some(text) = &item.body {
            let room = remaining.saturating_sub(body_chars);
            if room > 0 {
                let taken: String = text.chars().take(room).collect();
                body.push_str(
                    "
",
                );
                body.push_str(&taken);
                body_chars += taken.chars().count() + 1;
            }
        }
        if body_chars > remaining {
            // 放不下整节 → 整节裁掉，键进 dropped（逐对象记账）。
            context.sections_dropped.push(item.section_key.clone());
            continue;
        }
        remaining -= body_chars;
        object_sections.push(ContextSection {
            key: item.section_key.clone(),
            title: item.title.clone(),
            body,
            chars: body_chars,
            truncated: false,
        });
    }
    context.sections.extend(object_sections);
    context.chars_total = context.sections.iter().map(|s| s.chars).sum();
    Ok(context)
}
