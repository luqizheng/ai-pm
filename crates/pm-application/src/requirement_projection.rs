//! S02-I2 · Requirement 投影协议（Markdown 权威正文，S02-04 V1.2 §6 落地）。
//!
//! 裁决（I2 实施裁决，随本模块成文）：
//! - **O-3 doc_key** = `req-{requirement_id}`（0006 doc_key CHECK
//!   `^[A-Za-z0-9._-]+$` 合规；一个 Requirement 对应一份投影文档，1:1 §6.4）；
//!   doc_type = `'requirement'`（0006 既有四值，零迁移）。
//! - **O-4 markdown_version_ref** = `ai.document_revision.git_commit_sha`
//!   （内容寻址唯一；修订序号由 document_revision 承载）。
//! - **绑定点 = submit_revision（唯一绑定点，§5.4 Submit 冻结）**：导入产生的
//!   draft 不绑定——投影文档随首次 submit 诞生（§6.6 V1 口径；「Agent 写
//!   Markdown」由 submit 路径统一承担）。
//! - **content_hash 单点收敛（S02D-16）**：DB 侧与 Git 侧共用
//!   `knowledge::content_hash`（sha256(markdown) 纯正文）；绑定时刻刷新
//!   revision 的 content_hash 为 Git 侧哈希（与正文严格同源）。
//!
//! 写入职责分层（§6.2）：V1 的「Agent 执行层」= 本模块（受控进程内写入：
//! knowledge_write_coordinator 项目级写锁 + knowledge_write_operation 幂等账本
//! + Git 先行提交）；外部 Agent 编排接管归后续切片。
//!
//! 跨存储一致性（§6.7）：Git 提交先于 PG 事务（create_document 同款）。崩溃
//! 窗口的孤儿提交可容忍——`operation_id = "req-submit:{revision_id}"` 幂等：
//! 重放路径①账本行带 revision_id → 返回该修订 sha；②账本行存在但绑定缺失
//! （事务曾回滚）→ 继续补齐绑定并 `complete_operation` 回填；③revision 已绑
//! → 直接返回绑定 ref。submit 本身对 pending-未绑定状态可重入（见
//! `requirement::submit_revision`）。
//!
//! 读侧（§6.1）：任务上下文 read.rs 按钉定 revision 读 content 投影列（「读
//! 投影」形态，合规）；本模块提供 [`read_at_ref`]（按 ref 读 Git）与
//! [`rebuild_projection`]（Git→content 反向投影，投影损坏的可恢复机制，
//! §6.5 清单「反向投影工具」）。

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::knowledge::{self, KnowledgeError, KnowledgeRoot};
use crate::knowledge_write_coordinator::{
    self, OperationClaim, WriteCommand, DEFAULT_WRITE_LOCK_TIMEOUT_MS,
};
use crate::pm_write::PmWriteError;

pub const DOC_TYPE: &str = "requirement";

/// O-3：需求投影文档的 doc_key（0006 CHECK `^[A-Za-z0-9._-]+$` 合规）。
pub fn doc_key(requirement_id: Uuid) -> String {
    format!("req-{requirement_id}")
}

/// 提交时的投影绑定（唯一绑定点）：写 Git → 幂等账本 → document_revision 行 →
/// 绑定 UPDATE → 返回 `markdown_version_ref`（= git_commit_sha）。
pub async fn bind_projection(
    pool: &PgPool,
    root: &KnowledgeRoot,
    project_id: Uuid,
    requirement_id: Uuid,
    revision_id: Uuid,
    markdown: &str,
    created_by: Uuid,
) -> Result<String, PmWriteError> {
    // 已绑定 → 幂等返回（submit 重入 / 重复绑定请求）。
    let bound: Option<String> = sqlx::query_scalar(
        "SELECT markdown_version_ref FROM delivery.requirement_revision \
         WHERE id = $1 AND requirement_id = $2",
    )
    .bind(revision_id)
    .bind(requirement_id)
    .fetch_optional(pool)
    .await
    .map_err(PmWriteError::Db)?
    .flatten();
    if let Some(sha) = bound {
        return Ok(sha);
    }

    let operation_id = format!("req-submit:{revision_id}");
    let doc_key = doc_key(requirement_id);

    let guard = knowledge_write_coordinator::acquire(
        pool,
        project_id,
        std::time::Duration::from_millis(DEFAULT_WRITE_LOCK_TIMEOUT_MS),
    )
    .await
    .map_err(knowledge_err)?;

    // 账本早查：上一 attempt 已完整提交 → 直接取既有 sha，不写 Git、不新增修订。
    if let Some((_, Some(doc_rev_id))) =
        knowledge_write_coordinator::find_operation(pool, project_id, &operation_id)
            .await
            .map_err(knowledge_err)?
    {
        let sha: Option<String> = sqlx::query_scalar(
            "SELECT git_commit_sha FROM ai.document_revision WHERE id = $1",
        )
        .bind(doc_rev_id)
        .fetch_optional(pool)
        .await
        .map_err(PmWriteError::Db)?;
        guard.release().await;
        return sha.ok_or(PmWriteError::Conflict(
            "bound ledger operation has no document revision",
        ));
    }

    let project_code: String =
        sqlx::query_scalar("SELECT code FROM core.project WHERE id = $1")
            .bind(project_id)
            .fetch_optional(pool)
            .await
            .map_err(PmWriteError::Db)?
            .ok_or(PmWriteError::NotFound("unknown project"))?;

    // 文档现状：既有 → 沿用存储路径 + head 序号；新建 → 计算路径，head=0。
    let existing: Option<(Uuid, String, Option<i32>)> = sqlx::query_as(
        "SELECT d.id, d.canonical_path, \
                (SELECT max(revision_no) FROM ai.document_revision r WHERE r.document_id = d.id) \
         FROM ai.project_document d \
         WHERE d.project_id = $1 AND d.doc_key = $2",
    )
    .bind(project_id)
    .bind(&doc_key)
    .fetch_optional(pool)
    .await
    .map_err(PmWriteError::Db)?;

    let path = match &existing {
        Some((_, path, _)) => path.clone(),
        None => knowledge::canonical_path(&project_code, DOC_TYPE, &doc_key)
            .map_err(knowledge_err)?,
    };
    let next_no = existing
        .as_ref()
        .and_then(|(_, _, head)| head.map(|h| h + 1))
        .unwrap_or(1);

    root.ensure_ready().map_err(knowledge_err)?;
    // Git 先行（§6.7）：提交成功后再进 PG 事务；崩溃窗口的孤儿提交由幂等账本坍缩。
    let (sha, hash) = knowledge::write_revision(
        root,
        &path,
        markdown,
        &format!("docs(requirement): {doc_key} r{next_no}"),
    )
    .map_err(knowledge_err)?;

    let mut tx = pool.begin().await.map_err(PmWriteError::Db)?;
    // I4-F 归档守卫（与 create_document 同款）：归档项目知识写入已关闭。
    knowledge::ensure_project_active_for_write(&mut tx, project_id)
        .await
        .map_err(knowledge_err)?;
    let claim = knowledge_write_coordinator::claim_operation(
        &mut tx,
        project_id,
        &operation_id,
        WriteCommand::BindRequirementProjection,
    )
    .await
    .map_err(knowledge_err)?;
    if let OperationClaim::Replayed {
        revision_id: Some(doc_rev_id),
        ..
    } = &claim
    {
        // 上一 attempt 完整提交（账本带绑定）→ 取既有 sha 返回。
        let sha: Option<String> = sqlx::query_scalar(
            "SELECT git_commit_sha FROM ai.document_revision WHERE id = $1",
        )
        .bind(doc_rev_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(PmWriteError::Db)?;
        tx.rollback().await.map_err(PmWriteError::Db)?;
        guard.release().await;
        return sha.ok_or(PmWriteError::Conflict(
            "bound ledger operation has no document revision",
        ));
    }
    // Replayed { revision_id: None } = 上一 attempt 在 Git 提交后、事务提交前
    // 崩溃 → 继续在本事务补齐绑定，末尾 complete_operation 回填账本。

    let document_id = match &existing {
        Some((id, _, _)) => *id,
        None => {
            sqlx::query_scalar(
                "INSERT INTO ai.project_document \
                 (project_id, doc_key, doc_type, canonical_path, tenant_id) \
                 VALUES ($1, $2, $3, $4, (SELECT tenant_id FROM core.project WHERE id = $1)) \
                 RETURNING id",
            )
            .bind(project_id)
            .bind(&doc_key)
            .bind(DOC_TYPE)
            .bind(&path)
            .fetch_one(&mut *tx)
            .await
            .map_err(PmWriteError::Db)?
        }
    };

    let doc_revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.document_revision \
         (project_id, document_id, revision_no, git_commit_sha, content_hash, status, \
          change_summary, created_by, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, 'published', $6, $7, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(document_id)
    .bind(next_no)
    .bind(&sha)
    .bind(&hash)
    .bind("requirement revision submit")
    .bind(created_by)
    .fetch_one(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;

    // 绑定 + S02D-16 哈希刷新（Git 侧哈希与正文严格同源）。
    sqlx::query(
        "UPDATE delivery.requirement_revision \
         SET markdown_version_ref = $2, content_hash = $3 WHERE id = $1",
    )
    .bind(revision_id)
    .bind(&sha)
    .bind(&hash)
    .execute(&mut *tx)
    .await
    .map_err(PmWriteError::Db)?;

    knowledge_write_coordinator::complete_operation(
        &mut tx,
        project_id,
        &operation_id,
        None,
        Some(doc_revision_id),
    )
    .await
    .map_err(knowledge_err)?;

    tx.commit().await.map_err(PmWriteError::Db)?;
    guard.release().await;
    Ok(sha)
}

/// 按绑定 ref 读 Git（§6.1「按 ref 读 Git」读形态）。
pub async fn read_at_ref(
    pool: &PgPool,
    root: &KnowledgeRoot,
    project_id: Uuid,
    requirement_id: Uuid,
    sha: &str,
) -> Result<String, PmWriteError> {
    let project_code: String =
        sqlx::query_scalar("SELECT code FROM core.project WHERE id = $1")
            .bind(project_id)
            .fetch_optional(pool)
            .await
            .map_err(PmWriteError::Db)?
            .ok_or(PmWriteError::NotFound("unknown project"))?;
    let path =
        knowledge::canonical_path(&project_code, DOC_TYPE, &doc_key(requirement_id))
            .map_err(knowledge_err)?;
    knowledge::git_read_at(root.path(), &path, sha).map_err(knowledge_err)
}

/// 反向投影工具（§6.5 Markdown→DB）：按绑定 ref 读 Git 正文 → 校验哈希 →
/// 重建 content 投影列（summary 穿透保留）。投影损坏的可恢复机制；Git 正文
/// 与账面哈希不符 → 显式拒绝（A=B 不变量，§6.1——漂移必须可观测，不静默吞）。
pub async fn rebuild_projection(
    pool: &PgPool,
    root: &KnowledgeRoot,
    requirement_id: Uuid,
    revision_id: Uuid,
) -> Result<serde_json::Value, PmWriteError> {
    let row: Option<(Uuid, Option<String>, String, serde_json::Value)> = sqlx::query_as(
        "SELECT r.project_id, r.markdown_version_ref, r.content_hash, r.content \
         FROM delivery.requirement_revision r WHERE r.id = $1 AND r.requirement_id = $2",
    )
    .bind(revision_id)
    .bind(requirement_id)
    .fetch_optional(pool)
    .await
    .map_err(PmWriteError::Db)?;
    let (project_id, sha, recorded_hash, current) =
        row.ok_or(PmWriteError::NotFound("revision not found"))?;
    let sha = sha.ok_or(PmWriteError::Conflict(
        "revision is not bound to a markdown version — nothing to rebuild from",
    ))?;

    let markdown = read_at_ref(pool, root, project_id, requirement_id, &sha).await?;
    if knowledge::content_hash(&markdown) != recorded_hash {
        return Err(PmWriteError::Conflict(
            "bound markdown does not match the recorded content_hash — projection drift",
        ));
    }
    let mut content = json!({ "markdown": markdown });
    if let Some(summary) = current.get("summary") {
        content["summary"] = summary.clone();
    }
    sqlx::query("UPDATE delivery.requirement_revision SET content = $2 WHERE id = $1")
        .bind(revision_id)
        .bind(&content)
        .execute(pool)
        .await
        .map_err(PmWriteError::Db)?;
    Ok(content)
}

fn knowledge_err(e: KnowledgeError) -> PmWriteError {
    match e {
        KnowledgeError::NotFound => PmWriteError::NotFound("knowledge document not found"),
        KnowledgeError::Stale => {
            PmWriteError::Conflict("knowledge projection is stale")
        }
        KnowledgeError::ProjectWriteBusy(_) => PmWriteError::Conflict(
            "knowledge write lock is busy — retry shortly",
        ),
        KnowledgeError::RevisionConflict(msg) => PmWriteError::Conflict(
            Box::leak(msg.into_boxed_str()),
        ),
        other => PmWriteError::Db(sqlx::Error::Protocol(other.to_string())),
    }
}

fn knowledge_db(e: sqlx::Error) -> PmWriteError {
    PmWriteError::Db(e)
}
