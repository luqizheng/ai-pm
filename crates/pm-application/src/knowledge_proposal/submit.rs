use super::*;
use serde::Serialize;use sqlx::{PgPool, Postgres, Row, Transaction};use uuid::Uuid;use crate::knowledge::{self, KnowledgeError, KnowledgeRoot};use crate::knowledge_quality::{self, GateFailure, GateInput, QualityGateResult};

pub(crate) async fn current_doc(
    pool: &PgPool,
    project_id: Uuid,
    doc_key: &str,
) -> Result<Option<(CurrentDoc, String)>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT d.id AS document_id, d.doc_type, r.id AS revision_id, r.revision_no \
           FROM ai.project_document d \
           LEFT JOIN LATERAL ( \
               SELECT id, revision_no FROM ai.document_revision \
                WHERE document_id = d.id AND status = 'published' \
                ORDER BY revision_no DESC LIMIT 1 \
           ) r ON true \
          WHERE d.project_id = $1 AND d.doc_key = $2",
    )
    .bind(project_id)
    .bind(doc_key)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| {
        (
            CurrentDoc {
                document_id: Some(row.get("document_id")),
                revision_id: row.get("revision_id"),
                revision_no: row.get::<Option<i32>, _>("revision_no").unwrap_or(0),
            },
            row.get::<String, _>("doc_type"),
        )
    }))
}


/// 提交一次提案：质量门决定「自动提交 / 红线挂起 / 拒绝」。
///
/// T-060：整条路径经 `KnowledgeWriteCoordinator`——project 级写锁 → 幂等预占
/// → `base_revision` 校验 → 业务事务提交 → 提交后同步重建索引。
pub async fn submit_proposal(
    pool: &PgPool,
    root: &KnowledgeRoot,
    req: ProposalRequest,
) -> Result<ProposalOutcome, KnowledgeError> {
    let project_code: String = sqlx::query_scalar("SELECT code FROM core.project WHERE id = $1")
        .bind(req.project_id)
        .fetch_optional(pool)
        .await?
        .ok_or(KnowledgeError::NotFound)?;
    let path = knowledge::canonical_path(&project_code, &req.doc_type, &req.doc_key)?;

    // 写锁：同 Project 的知识写入串行（跨实例有效）。
    let guard = crate::knowledge_write_coordinator::acquire(
        pool,
        req.project_id,
        std::time::Duration::from_millis(
            crate::knowledge_write_coordinator::DEFAULT_WRITE_LOCK_TIMEOUT_MS,
        ),
    )
    .await?;

    // 幂等重放：命中既有账本行 → 返回既有结果，不写 Git、不产生第二条修订。
    if let Some((proposal_id, revision_id)) =
        crate::knowledge_write_coordinator::find_operation(pool, req.project_id, &req.operation_id)
            .await?
    {
        if let Some(revision_id) = revision_id {
            let outcome = load_committed_outcome(pool, req.project_id, revision_id).await?;
            guard.release().await;
            return Ok(outcome);
        }
        if let Some(proposal_id) = proposal_id {
            let outcome = load_proposal_outcome(pool, req.project_id, proposal_id).await?;
            guard.release().await;
            return Ok(outcome);
        }
    }

    let current = current_doc(pool, req.project_id, &req.doc_key).await?;
    if let Some((_, existing_type)) = &current {
        if existing_type != &req.doc_type {
            guard.release().await;
            return Err(KnowledgeError::Conflict(format!(
                "document {} already exists as doc_type {existing_type}",
                req.doc_key
            )));
        }
    }
    let current = current.map(|(doc, _)| doc);
    let current_document_id = current.as_ref().and_then(|doc| doc.document_id);
    let current_revision_id = current.as_ref().and_then(|doc| doc.revision_id);
    let current_revision_no = current.as_ref().map(|doc| doc.revision_no).unwrap_or(0);
    let hash = knowledge::content_hash(&req.markdown);

    // `base_revision` 校验：与当前 head 不一致 → 409，不产生修订。
    if let Err(err) =
        crate::knowledge_write_coordinator::check_base_revision(req.base_revision_id, current_revision_id)
    {
        guard.release().await;
        return Err(err);
    }

    let gate = knowledge_quality::evaluate(
        pool,
        &GateInput {
            project_id: req.project_id,
            doc_type: &req.doc_type,
            markdown: &req.markdown,
            content_hash: &hash,
            change_summary: req.change_summary.as_deref(),
            source_event_id: req.source_event_id,
            analysis_run_id: req.analysis_run_id,
            source_ref: req.source_ref.as_deref(),
            declared_redline: req.redline_kind.as_deref(),
            current_document_id,
            current_revision_id,
            base_revision_id: req.base_revision_id,
        },
    )
    .await?;

    if gate.duplicate {
        guard.release().await;
        return Ok(ProposalOutcome::Duplicate {
            document_id: current_document_id.ok_or(KnowledgeError::NotFound)?,
            revision_id: current_revision_id.ok_or(KnowledgeError::NotFound)?,
            revision_no: current_revision_no,
        });
    }

    let status = if !gate.passed {
        "rejected"
    } else if gate.redline_kind.is_some() {
        "redline_pending"
    } else {
        "commit_pending"
    };
    let gate_json = serde_json::to_value(&gate).unwrap_or_default();
    let rejection_reason = if gate.passed {
        None
    } else {
        Some(
            gate.failures
                .iter()
                .map(|failure| format!("{}:{}", failure.gate, failure.code))
                .collect::<Vec<_>>()
                .join(","),
        )
    };

    let mut tx = pool.begin().await?;
    // 幂等预占在业务事务内。
    let claim = crate::knowledge_write_coordinator::claim_operation(
        &mut tx,
        req.project_id,
        &req.operation_id,
        crate::knowledge_write_coordinator::WriteCommand::SubmitProposal,
    )
    .await?;
    if let crate::knowledge_write_coordinator::OperationClaim::Replayed { proposal_id, revision_id } =
        claim
    {
        tx.rollback().await?;
        if let Some(revision_id) = revision_id {
            let outcome = load_committed_outcome(pool, req.project_id, revision_id).await?;
            guard.release().await;
            return Ok(outcome);
        }
        if let Some(proposal_id) = proposal_id {
            let outcome = load_proposal_outcome(pool, req.project_id, proposal_id).await?;
            guard.release().await;
            return Ok(outcome);
        }
        guard.release().await;
        return Err(KnowledgeError::Conflict(format!(
            "write operation {} is already recorded",
            req.operation_id
        )));
    }

    let proposal_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.document_proposal \
         (project_id, document_id, doc_key, doc_type, canonical_path, base_revision_id, \
          proposed_markdown, proposed_content_hash, change_summary, \
          source_event_id, analysis_run_id, status, quality_gate_result, redline_kind, \
          rejection_reason, created_by, tenant_id) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(req.project_id)
    .bind(current_document_id)
    .bind(&req.doc_key)
    .bind(&req.doc_type)
    .bind(&path)
    .bind(current_revision_id)
    .bind(&req.markdown)
    .bind(&hash)
    .bind(&req.change_summary)
    .bind(req.source_event_id)
    .bind(req.analysis_run_id)
    .bind(status)
    .bind(&gate_json)
    .bind(&gate.redline_kind)
    .bind(&rejection_reason)
    .bind(req.created_by)
    .fetch_one(&mut *tx)
    .await?;

    crate::knowledge_write_coordinator::complete_operation(
        &mut tx,
        req.project_id,
        &req.operation_id,
        Some(proposal_id),
        None,
    )
    .await?;
    tx.commit().await?;

    if !gate.passed {
        guard.release().await;
        return Ok(ProposalOutcome::Rejected {
            proposal_id,
            failures: gate.failures,
        });
    }
    if gate.redline_kind.is_some() {
        guard.release().await;
        return Ok(ProposalOutcome::RedlinePending { proposal_id });
    }

    let revision_no = current_revision_no + 1;
    root.ensure_ready()?;
    let (commit_sha, content_hash) = knowledge::write_revision(
        root,
        &path,
        &req.markdown,
        &format!("docs({}): {} (rev {revision_no})", req.doc_type, req.doc_key),
    )?;

    let outcome = finalize_committed(
        pool,
        proposal_id,
        req.project_id,
        &req.doc_key,
        &req.doc_type,
        &path,
        &req.markdown,
        &content_hash,
        &commit_sha,
        req.change_summary.as_deref(),
        req.source_event_id,
        req.analysis_run_id,
        req.created_by,
        None,
    )
    .await?;

    // 提交后同步重建索引。
    if let ProposalOutcome::Committed { revision_id, .. } = &outcome {
        crate::knowledge_indexer::rebuild_quietly(pool, root, req.project_id, *revision_id).await;
    }
    guard.release().await;
    Ok(outcome)
}


/// 人类确认红线提案：确认后写 Git 并落修订。
///
/// T-060：红线确认是**独立写入命令**（`confirm_redline`），有自己的
/// `operation_id` 与 project 写锁。
pub async fn confirm_redline_proposal(
    pool: &PgPool,
    root: &KnowledgeRoot,
    proposal_id: Uuid,
    project_id: Uuid,
    confirmed_by: Uuid,
    operation_id: String,
) -> Result<ProposalOutcome, KnowledgeError> {
    require_human(pool, confirmed_by).await?;

    let guard = crate::knowledge_write_coordinator::acquire(
        pool,
        project_id,
        std::time::Duration::from_millis(
            crate::knowledge_write_coordinator::DEFAULT_WRITE_LOCK_TIMEOUT_MS,
        ),
    )
    .await?;

    // 幂等重放。
    if let Some((_, Some(revision_id))) =
        crate::knowledge_write_coordinator::find_operation(pool, project_id, &operation_id).await?
    {
        let outcome = load_committed_outcome(pool, project_id, revision_id).await?;
        guard.release().await;
        return Ok(outcome);
    }

    let row = sqlx::query(
        "SELECT doc_key, doc_type, canonical_path, proposed_markdown, proposed_content_hash, \
                change_summary, source_event_id, analysis_run_id, created_by, status \
           FROM ai.document_proposal WHERE id = $1 AND project_id = $2",
    )
    .bind(proposal_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .ok_or(KnowledgeError::NotFound)?;
    let status: String = row.get("status");
    if status != "redline_pending" {
        guard.release().await;
        return Err(KnowledgeError::Conflict(format!(
            "proposal {proposal_id} is {status}, not redline_pending"
        )));
    }
    let doc_key: String = row.get("doc_key");
    let doc_type: String = row.get("doc_type");
    let canonical_path: String = row.get("canonical_path");
    let markdown: String = row.get("proposed_markdown");

    let change_summary: Option<String> = row.get("change_summary");
    let source_event_id: Option<Uuid> = row.get("source_event_id");
    let analysis_run_id: Option<Uuid> = row.get("analysis_run_id");
    let created_by: Uuid = row.get("created_by");

    let mut tx = pool.begin().await?;
    let claim = crate::knowledge_write_coordinator::claim_operation(
        &mut tx,
        project_id,
        &operation_id,
        crate::knowledge_write_coordinator::WriteCommand::ConfirmRedline,
    )
    .await?;
    if let crate::knowledge_write_coordinator::OperationClaim::Replayed { revision_id, .. } = claim {
        tx.rollback().await?;
        if let Some(revision_id) = revision_id {
            let outcome = load_committed_outcome(pool, project_id, revision_id).await?;
            guard.release().await;
            return Ok(outcome);
        }
        guard.release().await;
        return Err(KnowledgeError::Conflict(format!(
            "write operation {operation_id} is already recorded"
        )));
    }

    sqlx::query(
        "UPDATE ai.document_proposal SET redline_confirmed_by = $2, \
            redline_confirmed_at = now(), updated_at = now() WHERE id = $1",
    )
    .bind(proposal_id)
    .bind(confirmed_by)
    .execute(&mut *tx)
    .await?;

    crate::knowledge_write_coordinator::complete_operation(
        &mut tx,
        project_id,
        &operation_id,
        Some(proposal_id),
        None,
    )
    .await?;
    tx.commit().await?;

    root.ensure_ready()?;
    let (commit_sha, hash) = knowledge::write_revision(
        root,
        &canonical_path,
        &markdown,
        &format!("docs({doc_type}): {doc_key} (redline confirmed, rev next)"),
    )?;

    let outcome = finalize_committed(
        pool,
        proposal_id,
        project_id,
        &doc_key,
        &doc_type,
        &canonical_path,
        &markdown,
        &hash,
        &commit_sha,
        change_summary.as_deref(),
        source_event_id,
        analysis_run_id,
        created_by,
        Some(confirmed_by),
    )
    .await?;

    if let ProposalOutcome::Committed { revision_id, .. } = &outcome {
        crate::knowledge_indexer::rebuild_quietly(pool, root, project_id, *revision_id).await;
    }
    guard.release().await;
    Ok(outcome)
}

/// 读回一次已提交的提案结果（幂等重放用）。
pub(crate) async fn load_committed_outcome(
    pool: &PgPool,
    project_id: Uuid,
    revision_id: Uuid,
) -> Result<ProposalOutcome, KnowledgeError> {
    let row = sqlx::query(
        "SELECT r.id, r.document_id, r.revision_no, r.git_commit_sha, r.content_hash, \
                d.canonical_path, p.id AS proposal_id \
           FROM ai.document_revision r \
           JOIN ai.project_document d ON d.id = r.document_id \
           LEFT JOIN ai.document_proposal p ON p.resulting_revision_id = r.id \
          WHERE r.id = $1 AND r.project_id = $2",
    )
    .bind(revision_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .ok_or(KnowledgeError::NotFound)?;
    Ok(ProposalOutcome::Committed {
        proposal_id: row.get("proposal_id"),
        document_id: row.get("document_id"),
        revision_id,
        revision_no: row.get("revision_no"),
        git_commit_sha: row.get("git_commit_sha"),
        content_hash: row.get("content_hash"),
        canonical_path: row.get("canonical_path"),
    })
}

/// 读回一次提案的当前状态（幂等重放：提案已落但尚未提交）。
pub(crate) async fn load_proposal_outcome(
    pool: &PgPool,
    project_id: Uuid,
    proposal_id: Uuid,
) -> Result<ProposalOutcome, KnowledgeError> {
    let row = sqlx::query(
        "SELECT status, document_id, resulting_revision_id FROM ai.document_proposal \
          WHERE id = $1 AND project_id = $2",
    )
    .bind(proposal_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .ok_or(KnowledgeError::NotFound)?;
    let status: String = row.get("status");
    match status.as_str() {
        "rejected" => Ok(ProposalOutcome::Rejected {
            proposal_id,
            failures: Vec::new(),
        }),
        "redline_pending" => Ok(ProposalOutcome::RedlinePending { proposal_id }),
        "committed" => {
            let revision_id: Option<Uuid> = row.get("resulting_revision_id");
            match revision_id {
                Some(revision_id) => load_committed_outcome(pool, project_id, revision_id).await,
                None => Err(KnowledgeError::Conflict(format!(
                    "proposal {proposal_id} is committed but has no resulting revision"
                ))),
            }
        }
        other => Err(KnowledgeError::Conflict(format!(
            "proposal {proposal_id} is {other}, not replayable"
        ))),
    }
}
