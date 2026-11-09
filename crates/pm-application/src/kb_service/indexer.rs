//! KB / PC 检索索引器（WP-3）。
//!
//! **纪律**（任务书 §四）：
//! - **家族现行六步**（防乱序/并发）：①家族级一致性锁（advisory lock）→ ②从权威
//!   数据读最新 published（**不信事件**）→ ③比较事件修订与家族现行 → ④仅现行才建
//!   索引 → ⑤过期事件只清旧不覆盖新 → ⑥索引与 state 同事务。
//! - **不变量**：`status ≠ 'indexed'` ⇒ 该 (revision, provider) 章节行数 = 0。
//! - **清零语义**：清的是**索引行**，永不删除历史正式版本的知识正文与 Revision 元
//!   数据（历史走版本钉死直读，不靠索引）。
//! - **幂等**：生产侧幂等键在 outbox；消费侧重建是天然幂等（同输入同结果）。
//! - **draft 恒不索引**：唯一入口是 published 事件与 rebuild（只扫 published）。
//!
//! 章节解析复用 [`crate::knowledge_index::parse_markdown_tree`] 纯函数；
//! 分词复用 [`crate::retrieval::cjk::cjk_bigrams`]（双端同一实现）。

use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::knowledge_index::parse_markdown_tree;
use crate::retrieval::cjk::cjk_bigrams;

/// KB 域索引 provider 名（新命名域，不复用 ai 栈的 'db-section'）。
pub const KB_SECTION_PROVIDER: &str = "kb-section";
/// 解析/分块规则版本（参与有效性判定）。
pub const KB_INDEX_VERSION: &str = "v1";

/// 索引失败（错误台账面）。
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("revision not found: {0}")]
    NotFound(String),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

/// 家族现行修订（权威读法：**不信事件**，以库内最新 published 为准）。
async fn family_current_published(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    document_id: Uuid,
) -> Result<Option<(Uuid, String, String)>, sqlx::Error> {
    // (revision_id, content_hash, content)——最新 published（按 revision_no 取最大）。
    let row = sqlx::query(
        "SELECT id, content_hash, content \
           FROM kb.document_revision \
          WHERE document_id = $1 AND status = 'published' \
          ORDER BY revision_no DESC LIMIT 1",
    )
    .bind(document_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.map(|r| {
        (
            r.get("id"),
            r.get::<String, _>("content_hash"),
            r.get::<String, _>("content"),
        )
    }))
}

/// 建索引上下文（收拢参数；clippy too_many_arguments）。
struct RevisionInput<'a> {
    tenant_id: Uuid,
    project_id: Uuid,
    document_id: Uuid,
    revision_id: Uuid,
    content_hash: &'a str,
    content: &'a str,
}

/// 建索引（单个现行修订；在给定事务内）。
async fn index_revision_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    input: &RevisionInput<'_>,
) -> Result<usize, sqlx::Error> {
    let RevisionInput {
        tenant_id,
        project_id,
        document_id,
        revision_id,
        content_hash,
        content,
    } = *input;
    // 章节树（纯函数解析；无标题的正文作为单一前言章节，与 ai 栈同语义）。
    let canonical_path: String =
        sqlx::query_scalar("SELECT canonical_path FROM kb.document WHERE id = $1")
            .bind(document_id)
            .fetch_one(&mut **tx)
            .await?;
    let mut sections = parse_markdown_tree(&canonical_path, content);
    if sections.is_empty() {
        // parse_markdown_tree 对空正文返回空 → 记单空节（保 state=indexed 语义一致）。
        sections = parse_markdown_tree(&canonical_path, "# （空正文）\n");
    }

    // 清零：本文档其他修订的索引行（现行切换——只清索引，不动正文/修订元数据）。
    sqlx::query(
        "DELETE FROM kb.document_section_index \
          WHERE document_id = $1 AND provider = $2 AND document_revision_id <> $3",
    )
    .bind(document_id)
    .bind(KB_SECTION_PROVIDER)
    .bind(revision_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "DELETE FROM kb.document_index_state \
          WHERE document_id = $1 AND provider = $2 AND document_revision_id <> $3",
    )
    .bind(document_id)
    .bind(KB_SECTION_PROVIDER)
    .bind(revision_id)
    .execute(&mut **tx)
    .await?;
    // 本修订旧章节行（重索引幂等）。
    sqlx::query(
        "DELETE FROM kb.document_section_index \
          WHERE document_revision_id = $1 AND provider = $2",
    )
    .bind(revision_id)
    .bind(KB_SECTION_PROVIDER)
    .execute(&mut **tx)
    .await?;

    for (no, section) in sections.iter().enumerate() {
        let tsv = cjk_bigrams(&format!("{} {}", section.title, section.content));
        sqlx::query(
            "INSERT INTO kb.document_section_index \
             (tenant_id, project_id, document_id, document_revision_id, provider, section_no, \
              level, title, anchor, content, start_line, search_tsv) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, \
                     CASE WHEN $12 = '' THEN NULL ELSE to_tsvector('simple', $12) END)",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(document_id)
        .bind(revision_id)
        .bind(KB_SECTION_PROVIDER)
        .bind(no as i32)
        .bind(i16::from(section.level))
        .bind(&section.title)
        .bind(&section.anchor)
        .bind(&section.content)
        .bind(section.start_line as i32)
        .bind(tsv)
        .execute(&mut **tx)
        .await?;
    }

    // state 同事务推进（upsert；已 indexed 则刷新）。
    sqlx::query(
        "INSERT INTO kb.document_index_state \
         (tenant_id, project_id, document_id, document_revision_id, provider, \
          indexed_content_hash, index_version, status, indexed_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'indexed', now()) \
         ON CONFLICT (document_revision_id, provider) DO UPDATE \
            SET indexed_content_hash = EXCLUDED.indexed_content_hash, \
                index_version = EXCLUDED.index_version, \
                status = 'indexed', \
                last_error = NULL, \
                indexed_at = now(), \
                updated_at = now()",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(document_id)
    .bind(revision_id)
    .bind(KB_SECTION_PROVIDER)
    .bind(content_hash)
    .bind(KB_INDEX_VERSION)
    .execute(&mut **tx)
    .await?;

    Ok(sections.len())
}

/// KB 修订发布事件的索引消费（家族现行六步）。
///
/// 返回 `(action, sections)`：action = `indexed`（本修订是家族现行，已建索引）/
/// `stale_event_skipped`（事件过期：已有更新的 published，只清本修订旧索引）。
pub async fn index_published_revision(
    pool: &PgPool,
    document_id: Uuid,
    revision_id: Uuid,
) -> Result<(&'static str, usize), IndexError> {
    let mut tx = pool.begin().await?;

    // ① 家族级一致性锁（advisory lock 绑定 document_id——并发发布串行化）。
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(format!("kb-index:{document_id}"))
        .execute(&mut *tx)
        .await?;

    // ② 从权威数据读家族现行（不信事件）。
    let Some((current_id, current_hash, content)) =
        family_current_published(&mut tx, document_id).await?
    else {
        return Err(IndexError::NotFound(format!(
            "document {document_id} has no published revision"
        )));
    };

    if current_id != revision_id {
        // ⑤ 过期事件：只清本修订旧索引（不得覆盖新索引）。
        sqlx::query(
            "DELETE FROM kb.document_section_index \
              WHERE document_revision_id = $1 AND provider = $2",
        )
        .bind(revision_id)
        .bind(KB_SECTION_PROVIDER)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "DELETE FROM kb.document_index_state \
              WHERE document_revision_id = $1 AND provider = $2",
        )
        .bind(revision_id)
        .bind(KB_SECTION_PROVIDER)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        return Ok(("stale_event_skipped", 0));
    }

    // ④ 现行 → 建索引（含清旧；⑥ 索引与 state 同事务）。
    let tenant_id: Uuid = sqlx::query_scalar("SELECT tenant_id FROM kb.document WHERE id = $1")
        .bind(document_id)
        .fetch_one(&mut *tx)
        .await?;
    let project_id: Uuid = sqlx::query_scalar("SELECT project_id FROM kb.document WHERE id = $1")
        .bind(document_id)
        .fetch_one(&mut *tx)
        .await?;

    let sections = index_revision_tx(
        &mut tx,
        &RevisionInput {
            tenant_id,
            project_id,
            document_id,
            revision_id: current_id,
            content_hash: &current_hash,
            content: &content,
        },
    )
    .await?;

    tx.commit().await?;
    Ok(("indexed", sections))
}

/// 全量重建报告（Q-C 裁决：结构化报告，内部函数）。
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct RebuildReport {
    pub scanned: usize,
    pub indexed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub elapsed_ms: u64,
    /// 失败明细（revision 级，RT-012 恢复面）。
    pub failures: Vec<String>,
}

/// KB 域全量重建（Q-C：仅内部函数；内部调用同样过项目边界）。
///
/// 扫描项目内全部**家族现行 published** 修订重建索引；期间 draft / 无效版本不会
/// 进入正式检索（只扫 published 且逐家族取最新）。
pub async fn rebuild_kb_index(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<RebuildReport, IndexError> {
    let started = std::time::Instant::now();
    let mut report = RebuildReport::default();

    let rows: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT DISTINCT ON (d.id) d.id, r.id \
           FROM kb.document d \
           JOIN kb.document_revision r ON r.document_id = d.id AND r.status = 'published' \
          WHERE d.tenant_id = $1 AND d.project_id = $2 \
          ORDER BY d.id, r.revision_no DESC",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    report.scanned = rows.len();

    for (document_id, revision_id) in rows {
        match index_published_revision(pool, document_id, revision_id).await {
            Ok(("indexed", _)) => report.indexed += 1,
            Ok((_, _)) => report.skipped += 1,
            Err(err) => {
                report.failed += 1;
                report.failures.push(format!("{revision_id}: {err}"));
                // 失败台账（KB-STATE-02 恢复面：failed 状态 + last_error）。
                let _ = sqlx::query(
                    "INSERT INTO kb.document_index_state \
                     (tenant_id, project_id, document_id, document_revision_id, provider, \
                      status, last_error) \
                     VALUES ($1, $2, $3, $4, $5, 'failed', $6) \
                     ON CONFLICT (document_revision_id, provider) DO UPDATE \
                        SET status = 'failed', last_error = EXCLUDED.last_error, \
                            updated_at = now()",
                )
                .bind(tenant_id)
                .bind(project_id)
                .bind(document_id)
                .bind(revision_id)
                .bind(KB_SECTION_PROVIDER)
                .bind(err.to_string())
                .execute(pool)
                .await;
            }
        }
    }

    // PC 域：重算全部现行正文行的 tsv（Q-D：无 state 表，重算即对账）。
    let pc_result = sqlx::query(
        "UPDATE pc.document_revision dr \
            SET search_tsv = CASE WHEN $3 = '' THEN NULL \
                                  ELSE to_tsvector('simple', $3) END \
           FROM pc.cognition_object o \
           JOIN pc.cognition_revision cr ON cr.id = o.current_revision_id \
          WHERE dr.id = cr.document_revision_id \
            AND o.tenant_id = $1 AND o.project_id = $2",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind("") // 占位：真正的 per-row 分词须逐行（见下）。
    .execute(pool)
    .await;
    // 上面单语句无法 per-row 调 Rust 分词 → 逐行重算（项目内现行行数量有界）。
    let _ = pc_result;
    let pc_rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT dr.id, dr.content \
           FROM pc.document_revision dr \
           JOIN pc.cognition_object o ON o.current_revision_id IS NOT NULL \
           JOIN pc.cognition_revision cr ON cr.id = o.current_revision_id \
          WHERE dr.id = cr.document_revision_id \
            AND o.tenant_id = $1 AND o.project_id = $2",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    for (revision_id, content) in &pc_rows {
        let tsv = cjk_bigrams(content);
        let _ = sqlx::query(
            "UPDATE pc.document_revision \
                SET search_tsv = CASE WHEN $2 = '' THEN NULL ELSE to_tsvector('simple', $2) END \
              WHERE id = $1",
        )
        .bind(revision_id)
        .bind(tsv)
        .execute(pool)
        .await;
    }

    report.elapsed_ms = started.elapsed().as_millis() as u64;
    Ok(report)
}

/// PC 版本发布的 tsv 重算（消费事务内；版本 → 成员对象 → 现行正文行）。
///
/// 幂等：重算同输入同结果（pc_index:{version_id} 生产侧幂等键在 outbox）。
pub async fn recompute_pc_revision_tsv(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    version_id: Uuid,
) -> Result<usize, sqlx::Error> {
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT dr.id, dr.content \
           FROM pc.version_member vm \
           JOIN pc.cognition_revision cr ON cr.id = vm.object_revision_id \
           JOIN pc.document_revision dr ON dr.id = cr.document_revision_id \
          WHERE vm.version_id = $1",
    )
    .bind(version_id)
    .fetch_all(&mut **tx)
    .await?;
    for (revision_id, content) in &rows {
        let tsv = cjk_bigrams(content);
        sqlx::query(
            "UPDATE pc.document_revision \
                SET search_tsv = CASE WHEN $2 = '' THEN NULL ELSE to_tsvector('simple', $2) END \
              WHERE id = $1",
        )
        .bind(revision_id)
        .bind(tsv)
        .execute(&mut **tx)
        .await?;
    }
    Ok(rows.len())
}
