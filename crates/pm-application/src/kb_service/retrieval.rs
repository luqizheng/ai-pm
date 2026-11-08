//! 按主题检索 L1 记忆（Discussion Memory 注入的 KB 侧读面）。
//!
//! 只读 `kb.*`：主题集合（task 本身 + 其关联需求等）由 Platform 侧计算后传入
//! （检查单 A-1：KB 不读 Platform 表）。只返回 **CURRENT** 版本——未审核提案、
//! 被替代 / 撤回的版本一律不进入 Agent Context（定义稿 §9.1：未经审核的结果不得进入
//! 正式浅层记忆）。结果按审核通过时间新者在前，截断至 [`MAX_RETRIEVED`]。
//!
//! 未命中时调用方只能表述为「当前浅层记忆中没有找到相关记录」，不能断言过去从未
//! 讨论（定义稿 §5.3）——本函数返回空集合，不做任何推断。

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// 单次注入上限（摘要级条目；防 Agent Context 膨胀）。
pub const MAX_RETRIEVED: i64 = 20;

/// 检索主题键（四元组去掉 revision：记忆讨论的是对象本身，不限于某个修订）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubjectKey {
    pub target_domain: String,
    pub target_type: String,
    pub target_id: String,
}

impl SubjectKey {
    pub fn new(
        target_domain: impl Into<String>,
        target_type: impl Into<String>,
        target_id: impl Into<String>,
    ) -> Self {
        Self {
            target_domain: target_domain.into(),
            target_type: target_type.into(),
            target_id: target_id.into(),
        }
    }
}

/// 命中的 CURRENT 记忆版本（摘要级，不带 body）。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RetrievedMemory {
    pub memory_id: Uuid,
    pub revision_id: Uuid,
    pub version_no: i32,
    pub memory_type: String,
    pub title: String,
    pub summary: String,
    pub approved_at: DateTime<Utc>,
    /// 命中的主题（`<target_type>:<target_id>`，按字典序）。
    pub matched_subjects: Vec<String>,
}

/// 项目内、主题命中任一键的 CURRENT 记忆版本。
pub async fn current_memories_for_subjects(
    pool: &PgPool,
    project_id: Uuid,
    subjects: &[SubjectKey],
) -> Result<Vec<RetrievedMemory>, sqlx::Error> {
    if subjects.is_empty() {
        return Ok(Vec::new());
    }
    let domains: Vec<&str> = subjects.iter().map(|s| s.target_domain.as_str()).collect();
    let types: Vec<&str> = subjects.iter().map(|s| s.target_type.as_str()).collect();
    let ids: Vec<&str> = subjects.iter().map(|s| s.target_id.as_str()).collect();
    let rows = sqlx::query(
        "SELECT r.memory_id, r.id AS revision_id, r.version_no, r.memory_type, r.title, \
                r.summary, r.reviewed_at, \
                array_agg(DISTINCT mr.target_type || ':' || mr.target_id \
                          ORDER BY mr.target_type || ':' || mr.target_id) AS matched \
           FROM kb.memory_ref mr \
           JOIN kb.memory_revision r ON r.id = mr.revision_id \
           JOIN UNNEST($2::text[], $3::text[], $4::text[]) AS s(d, t, i) \
             ON mr.target_domain = s.d AND mr.target_type = s.t AND mr.target_id = s.i \
          WHERE mr.project_id = $1 AND mr.ref_role = 'subject' \
            AND r.version_status = 'CURRENT' \
          GROUP BY r.memory_id, r.id, r.version_no, r.memory_type, r.title, r.summary, \
                   r.reviewed_at \
          ORDER BY r.reviewed_at DESC, r.id \
          LIMIT $5",
    )
    .bind(project_id)
    .bind(&domains)
    .bind(&types)
    .bind(&ids)
    .bind(MAX_RETRIEVED)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| RetrievedMemory {
            memory_id: r.get("memory_id"),
            revision_id: r.get("revision_id"),
            version_no: r.get("version_no"),
            memory_type: r.get("memory_type"),
            title: r.get("title"),
            summary: r.get("summary"),
            approved_at: r.get("reviewed_at"),
            matched_subjects: r.get("matched"),
        })
        .collect())
}
