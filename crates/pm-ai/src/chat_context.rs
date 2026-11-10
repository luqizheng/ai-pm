//! chat 段知识注入（整合计划 Batch 3A；D-IC-2 = 最小两节）。
//!
//! **为什么 chat 需要注入**：原 chat 回复链的模型上下文只有「固定语言镜像
//! system prompt + 最近 20 轮纯文本」（F-5），AI 不知道当前对着哪个项目/
//! 仓库说话——UI 承诺的「只读上下文」是空头承诺（DK-0 F20 已如实登记）。
//! 本模块给 chat 补上**最小两节**注入，使「注入面 = 承诺面」。
//!
//! ## 与 DK-C（访谈注入）的关系
//!
//! 复用 [`crate::discovery_context`] 的**分节与记账结构**，但**独立预算**：
//! 访谈 36,000 字符（[`discovery_context::TOTAL_BUDGET_CHARS`]）不动，
//! chat 用 8,000（[`CHAT_TOTAL_BUDGET_CHARS`]）——访谈是长程结构化任务，
//! chat 是一问一答，二者不该共享额度。
//!
//! ## 注入面（两节，D-IC-2）
//!
//! ```text
//! project_profile  (S1) ← core.project（名称/描述/状态）
//! repository_facts (S2) ← integration.project_repository（仓库/分支/角色）
//! ```
//!
//! 注入源 = **线程锚点**（Batch 2 的 `ai.conversation.context_anchor`）：
//! 锚点声明「这个线程对着哪个仓库」，服务端按锚点**重新投影**实时事实。
//! 锚点缺失（存量线程）→ **零注入**，行为与注入前逐字节一致（向后兼容）。
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
pub const CHAT_TOTAL_BUDGET_CHARS: usize = 8_000;
/// 单节上限：项目画像 / 仓库事实各半。
pub const CHAT_CAP_PROJECT_PROFILE: usize = 2_000;
pub const CHAT_CAP_REPOSITORY_FACTS: usize = 4_000;

/// 分节键。
pub const CHAT_KEY_PROJECT_PROFILE: &str = "project_profile";
pub const CHAT_KEY_REPOSITORY_FACTS: &str = "repository_facts";

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
    }

    // S2 仓库事实（按锚点定位实时仓库）。
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
pub fn chat_section_order() -> [&'static str; 2] {
    CHAT_SECTION_ORDER
}
