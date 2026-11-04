use super::*;
use serde::Serialize;use sqlx::{PgPool, Postgres, Row, Transaction};use uuid::Uuid;use crate::knowledge::{self, KnowledgeError, KnowledgeRoot};use crate::knowledge_quality::{self, GateFailure, GateInput, QualityGateResult};

/// 否决窗口：对已发布修订做 Git revert + 状态回退 + 索引重跟进 + 审计。
///
/// T-060：veto 是独立写入命令（`veto_revision`），走 project 写锁与幂等账本。
/// 索引处理（A-1）：被否决修订 → `stale` 且**清空产物**；生效修订 → 重建。
pub async fn veto_revision(
    pool: &PgPool,
    root: &KnowledgeRoot,
    project_id: Uuid,
    revision_id: Uuid,
    vetoed_by: Uuid,
    reason: &str,
    operation_id: String,
) -> Result<RevisionVetoOutcome, KnowledgeError> {
    require_human(pool, vetoed_by).await?;

    let guard = crate::knowledge_write_coordinator::acquire(
        pool,
        project_id,
        std::time::Duration::from_millis(
            crate::knowledge_write_coordinator::DEFAULT_WRITE_LOCK_TIMEOUT_MS,
        ),
    )
    .await?;

    // 幂等重放：veto 作用于显式指定的修订，不按 head 比较。
    if let Some((_, existing)) =
        crate::knowledge_write_coordinator::find_operation(pool, project_id, &operation_id).await?
    {
        if let Some(revision_id) = existing {
            let outcome = load_veto_outcome(pool, project_id, revision_id).await?;
            guard.release().await;
            return Ok(outcome);
        }
    }

    let row = sqlx::query(
        "SELECT r.status, r.git_commit_sha, r.revision_no, r.document_id, r.tenant_id, \
                d.canonical_path, \
                extract(epoch from (now() - coalesce(r.published_at, r.created_at)))::float8 AS age_secs \
           FROM ai.document_revision r \
           JOIN ai.project_document d ON d.id = r.document_id \
          WHERE r.id = $1 AND r.project_id = $2",
    )
    .bind(revision_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .ok_or(KnowledgeError::NotFound)?;
    let status: String = row.get("status");
    if status != "published" {
        guard.release().await;
        return Err(KnowledgeError::Conflict(format!(
            "revision {revision_id} is {status}, only published revisions can be vetoed"
        )));
    }
    let window = std::env::var("AI_PM_KNOWLEDGE_VETO_WINDOW_SECS")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(DEFAULT_VETO_WINDOW_SECS);
    let age_secs: f64 = row.get("age_secs");
    if age_secs > window as f64 {
        guard.release().await;
        return Err(KnowledgeError::Conflict(format!(
            "veto window expired ({age_secs:.0}s > {window}s)"
        )));
    }
    let sha: String = row.get("git_commit_sha");
    let revision_no: i32 = row.get("revision_no");
    let document_id: Uuid = row.get("document_id");
    let tenant_id: Uuid = row.get("tenant_id");

    let new_sha = knowledge::revert_commit(
        root,
        &sha,
        &format!("revert: veto document revision {revision_id}: {reason}"),
    )?;

    let mut tx = pool.begin().await?;
    let claim = crate::knowledge_write_coordinator::claim_operation(
        &mut tx,
        project_id,
        &operation_id,
        crate::knowledge_write_coordinator::WriteCommand::VetoRevision,
    )
    .await?;
    if let crate::knowledge_write_coordinator::OperationClaim::Replayed { revision_id, .. } = claim {
        tx.rollback().await?;
        if let Some(revision_id) = revision_id {
            let outcome = load_veto_outcome(pool, project_id, revision_id).await?;
            guard.release().await;
            return Ok(outcome);
        }
        guard.release().await;
        return Err(KnowledgeError::Conflict(format!(
            "write operation {operation_id} is already recorded"
        )));
    }

    sqlx::query("UPDATE ai.document_revision SET status = 'rolled_back' WHERE id = $1")
        .bind(revision_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE ai.document_proposal SET status = 'rolled_back', updated_at = now() \
          WHERE resulting_revision_id = $1",
    )
    .bind(revision_id)
    .execute(&mut *tx)
    .await?;

    // 指针回退：查询层按 status='published' 取最新修订，rolled_back 自动失效。
    let restored: Option<(Uuid, i32)> = sqlx::query_as(
        "SELECT id, revision_no FROM ai.document_revision \
          WHERE document_id = $1 AND status = 'published' AND revision_no < $2 \
          ORDER BY revision_no DESC LIMIT 1",
    )
    .bind(document_id)
    .bind(revision_no)
    .fetch_optional(&mut *tx)
    .await?;

    audit(
        &mut tx,
        project_id,
        &crate::actor::AuditActor::principal(vetoed_by),
        "veto_document_revision",
        "document_revision",
        revision_id,
        serde_json::json!({
            "reason": reason,
            "reverted_commit": sha,
            "new_commit": new_sha,
            "restored_revision_id": restored.map(|(id, _)| id),
        }),
    )
    .await?;
    crate::knowledge_write_coordinator::complete_operation(
        &mut tx,
        project_id,
        &operation_id,
        None,
        Some(revision_id),
    )
    .await?;
    tx.commit().await?;

    // 索引跟进（A-1）：被否决修订清空产物；生效修订重建。
    let _ = crate::knowledge_indexer::mark_stale(
        pool,
        root,
        tenant_id,
        project_id,
        revision_id,
        "revision was vetoed",
    )
    .await;
    if let Some((restored_id, _)) = restored {
        crate::knowledge_indexer::rebuild_quietly(pool, root, project_id, restored_id).await;
    }

    guard.release().await;

    Ok(RevisionVetoOutcome {
        new_commit_sha: new_sha,
        rolled_back_revision_id: revision_id,
        restored_revision_id: restored.map(|(id, _)| id),
    })
}

/// 读回一次已完成的否决结果（幂等重放用）。
async fn load_veto_outcome(
    pool: &PgPool,
    project_id: Uuid,
    revision_id: Uuid,
) -> Result<RevisionVetoOutcome, KnowledgeError> {
    let row = sqlx::query(
        "SELECT r.git_commit_sha, r.document_id, r.revision_no, \
                (SELECT id FROM ai.document_revision \
                  WHERE document_id = r.document_id AND status = 'published' \
                    AND revision_no < r.revision_no \
                  ORDER BY revision_no DESC LIMIT 1) AS restored_revision_id \
           FROM ai.document_revision r WHERE r.id = $1 AND r.project_id = $2",
    )
    .bind(revision_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .ok_or(KnowledgeError::NotFound)?;
    let document_id: Uuid = row.get("document_id");
    let revision_no: i32 = row.get("revision_no");
    let restored_revision_id: Option<Uuid> = row.get("restored_revision_id");
    let _ = (document_id, revision_no);
    Ok(RevisionVetoOutcome {
        new_commit_sha: String::new(),
        rolled_back_revision_id: revision_id,
        restored_revision_id,
    })
}


#[allow(clippy::too_many_arguments)]
pub(crate) async fn finalize_committed(
    pool: &PgPool,
    proposal_id: Uuid,
    project_id: Uuid,
    doc_key: &str,
    doc_type: &str,
    canonical_path: &str,
    _markdown: &str,
    content_hash: &str,
    commit_sha: &str,
    change_summary: Option<&str>,
    source_event_id: Option<Uuid>,
    analysis_run_id: Option<Uuid>,
    created_by: Uuid,
    redline_confirmed_by: Option<Uuid>,
) -> Result<ProposalOutcome, KnowledgeError> {
    let mut tx = pool.begin().await?;

    // I4-F 归档守卫：**正式提交入口**（发布 published 修订）必须确认 Project 仍
    // active——归档后不得出现新的正式版本（I4 §9.1/§9.3）。与归档事务共享
    // `core.project` 行锁 → 提交与归档线性化（I4 §11.3）。
    crate::knowledge::ensure_project_active_for_write(&mut tx, project_id).await?;

    let document_id: Uuid = match sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM ai.project_document WHERE project_id = $1 AND doc_key = $2",
    )
    .bind(project_id)
    .bind(doc_key)
    .fetch_optional(&mut *tx)
    .await?
    {
        Some(id) => {
            sqlx::query("UPDATE ai.project_document SET updated_at = now() WHERE id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            id
        }
        None => sqlx::query_scalar(
            "INSERT INTO ai.project_document (project_id, doc_key, doc_type, canonical_path, tenant_id) \
             VALUES ($1, $2, $3, $4, \
                     (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
        )
        .bind(project_id)
        .bind(doc_key)
        .bind(doc_type)
        .bind(canonical_path)
        .fetch_one(&mut *tx)
        .await?,
    };

    let revision_no: i32 = sqlx::query_scalar(
        "SELECT coalesce(max(revision_no), 0) + 1 FROM ai.document_revision \
          WHERE document_id = $1",
    )
    .bind(document_id)
    .fetch_one(&mut *tx)
    .await?;

    let revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.document_revision \
         (project_id, document_id, revision_no, git_commit_sha, content_hash, status, \
          change_summary, created_by, source_event_id, analysis_run_id, \
          redline_confirmed_by, published_at, tenant_id) \
         VALUES ($1,$2,$3,$4,$5,'published',$6,$7,$8,$9,$10,now(), \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(document_id)
    .bind(revision_no)
    .bind(commit_sha)
    .bind(content_hash)
    .bind(change_summary)
    .bind(created_by)
    .bind(source_event_id)
    .bind(analysis_run_id)
    .bind(redline_confirmed_by)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE ai.document_proposal SET status = 'committed', document_id = $2, \
            resulting_revision_id = $3, updated_at = now() WHERE id = $1",
    )
    .bind(proposal_id)
    .bind(document_id)
    .bind(revision_id)
    .execute(&mut *tx)
    .await?;

    audit(
        &mut tx,
        project_id,
        &crate::actor::AuditActor::principal(redline_confirmed_by.unwrap_or(created_by)),
        "commit_document_revision",
        "document_revision",
        revision_id,
        serde_json::json!({
            "document_id": document_id,
            "revision_no": revision_no,
            "git_commit_sha": commit_sha,
            "content_hash": content_hash,
            "redline_confirmed_by": redline_confirmed_by,
        }),
    )
    .await?;
    tx.commit().await?;

    Ok(ProposalOutcome::Committed {
        proposal_id,
        document_id,
        revision_id,
        revision_no,
        git_commit_sha: commit_sha.to_string(),
        content_hash: content_hash.to_string(),
        canonical_path: canonical_path.to_string(),
    })
}

