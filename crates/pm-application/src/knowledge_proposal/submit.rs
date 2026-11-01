use super::*;
use serde::Serialize;use sqlx::{PgPool, Postgres, Row, Transaction};use uuid::Uuid;use crate::knowledge::{self, KnowledgeError, KnowledgeRoot};use crate::knowledge_quality::{self, GateFailure, GateInput, QualityGateResult};

pub(crate) async fn current_doc(
    pool: &PgPool,
    project_id: Uuid,
    doc_key: &str,
) -> Result<Option<(CurrentDoc, String)>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT d.id AS document_id, d.doc_type, r.id AS revision_id, \
                r.revision_no, r.content_hash \
           FROM ai.project_document d \
           LEFT JOIN LATERAL ( \
               SELECT id, revision_no, content_hash FROM ai.document_revision \
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
                content_hash: row.get("content_hash"),
            },
            row.get::<String, _>("doc_type"),
        )
    }))
}


/// 提交一次提案：质量门决定「自动提交 / 红线挂起 / 拒绝」。
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

    let current = current_doc(pool, req.project_id, &req.doc_key).await?;
    if let Some((_, existing_type)) = &current {
        if existing_type != &req.doc_type {
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
    let current_content_hash = current.as_ref().and_then(|doc| doc.content_hash.clone());
    let hash = knowledge::content_hash(&req.markdown);

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
            current_content_hash: current_content_hash.as_deref(),
            base_content_hash: req.base_content_hash.as_deref(),
        },
    )
    .await?;

    if gate.duplicate {
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
    let proposal_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.document_proposal \
         (project_id, document_id, doc_key, doc_type, canonical_path, base_revision_id, \
          base_content_hash, proposed_markdown, proposed_content_hash, change_summary, \
          source_event_id, analysis_run_id, status, quality_gate_result, redline_kind, \
          rejection_reason, created_by, tenant_id) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(req.project_id)
    .bind(current_document_id)
    .bind(&req.doc_key)
    .bind(&req.doc_type)
    .bind(&path)
    .bind(current_revision_id)
    .bind(&current_content_hash)
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
    .fetch_one(pool)
    .await?;

    if !gate.passed {
        return Ok(ProposalOutcome::Rejected {
            proposal_id,
            failures: gate.failures,
        });
    }
    if gate.redline_kind.is_some() {
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

    finalize_committed(
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
    .await
}


/// 人类确认红线提案：确认后写 Git 并落修订。
pub async fn confirm_redline_proposal(
    pool: &PgPool,
    root: &KnowledgeRoot,
    proposal_id: Uuid,
    project_id: Uuid,
    confirmed_by: Uuid,
) -> Result<ProposalOutcome, KnowledgeError> {
    require_human(pool, confirmed_by).await?;

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

    sqlx::query(
        "UPDATE ai.document_proposal SET redline_confirmed_by = $2, \
            redline_confirmed_at = now(), updated_at = now() WHERE id = $1",
    )
    .bind(proposal_id)
    .bind(confirmed_by)
    .execute(pool)
    .await?;

    root.ensure_ready()?;
    let (commit_sha, hash) = knowledge::write_revision(
        root,
        &canonical_path,
        &markdown,
        &format!("docs({doc_type}): {doc_key} (redline confirmed, rev next)"),
    )?;

    finalize_committed(
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
    .await
}


