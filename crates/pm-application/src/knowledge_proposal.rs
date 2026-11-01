//! 知识空间 V2 提案流：质量门 → 自动提交 / 红线事前确认 / 事后否决回滚。
//!
//! 状态机（设计稿 §八）：`quality_gates_passed → commit_pending → committed`；
//! 红线走 `redline_pending → confirmed → committed`；失败 `rejected`；否决
//! `rolled_back`（并触发 Git revert）。正文权威仍是 Git，提案正文只作为候选。

use serde::Serialize;
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::knowledge::{self, KnowledgeError, KnowledgeRoot};
use crate::knowledge_quality::{self, GateFailure, GateInput, QualityGateResult};

/// 否决窗口默认 24 小时（可用 `AI_PM_KNOWLEDGE_VETO_WINDOW_SECS` 覆盖）。
pub const DEFAULT_VETO_WINDOW_SECS: i64 = 86_400;

#[derive(Debug, Clone)]
pub struct ProposalRequest {
    pub project_id: Uuid,
    pub doc_key: String,
    pub doc_type: String,
    pub markdown: String,
    pub change_summary: Option<String>,
    pub source_event_id: Option<Uuid>,
    pub analysis_run_id: Option<Uuid>,
    /// 显式来源标记（例如需求迁移 `requirement_revision:<id>`）。
    pub source_ref: Option<String>,
    pub redline_kind: Option<String>,
    pub base_content_hash: Option<String>,
    pub created_by: Uuid,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ProposalOutcome {
    /// 同一文档、同一内容：幂等无操作。
    Duplicate {
        document_id: Uuid,
        revision_id: Uuid,
        revision_no: i32,
    },
    /// 红线类：已落库，等待人类事前确认，未写 Git。
    RedlinePending { proposal_id: Uuid },
    /// 质量门拒绝：未写 Git。
    Rejected {
        proposal_id: Uuid,
        failures: Vec<GateFailure>,
    },
    /// 已提交 Git 并落修订。
    Committed {
        proposal_id: Uuid,
        document_id: Uuid,
        revision_id: Uuid,
        revision_no: i32,
        git_commit_sha: String,
        content_hash: String,
        canonical_path: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct ProposalRow {
    pub id: Uuid,
    pub doc_key: String,
    pub doc_type: String,
    pub status: String,
    pub quality_gate_result: serde_json::Value,
    pub redline_kind: Option<String>,
    pub change_summary: Option<String>,
    pub resulting_revision_id: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RevisionVetoOutcome {
    pub new_commit_sha: String,
    pub rolled_back_revision_id: Uuid,
    pub restored_revision_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
struct CurrentDoc {
    document_id: Option<Uuid>,
    revision_id: Option<Uuid>,
    revision_no: i32,
    content_hash: Option<String>,
}

async fn current_doc(
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
          rejection_reason, created_by) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17) RETURNING id",
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

/// 否决窗口：对已发布修订做 Git revert + 状态回退 + 索引重跟进 + 审计。
pub async fn veto_revision(
    pool: &PgPool,
    root: &KnowledgeRoot,
    project_id: Uuid,
    revision_id: Uuid,
    vetoed_by: Uuid,
    reason: &str,
) -> Result<RevisionVetoOutcome, KnowledgeError> {
    require_human(pool, vetoed_by).await?;

    let row = sqlx::query(
        "SELECT r.status, r.git_commit_sha, r.revision_no, r.document_id, d.canonical_path, \
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
        return Err(KnowledgeError::Conflict(format!(
            "veto window expired ({age_secs:.0}s > {window}s)"
        )));
    }
    let sha: String = row.get("git_commit_sha");
    let revision_no: i32 = row.get("revision_no");
    let document_id: Uuid = row.get("document_id");

    let new_sha = knowledge::revert_commit(
        root,
        &sha,
        &format!("revert: veto document revision {revision_id}: {reason}"),
    )?;

    let mut tx = pool.begin().await?;
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
    sqlx::query(
        "UPDATE ai.document_index_state SET status = 'stale', updated_at = now() \
          WHERE document_revision_id = $1",
    )
    .bind(revision_id)
    .execute(&mut *tx)
    .await?;

    // 指针回退：查询层按 status='published' 取最新修订，rolled_back 自动失效；
    // 这里把上一修订的索引状态重新置为 pending，触发索引重跟进。
    let restored: Option<(Uuid, i32)> = sqlx::query_as(
        "SELECT id, revision_no FROM ai.document_revision \
          WHERE document_id = $1 AND status = 'published' AND revision_no < $2 \
          ORDER BY revision_no DESC LIMIT 1",
    )
    .bind(document_id)
    .bind(revision_no)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some((restored_id, _)) = restored {
        sqlx::query(
            "INSERT INTO ai.document_index_state \
             (project_id, document_revision_id, provider, status) \
             VALUES ($1, $2, 'markdown-tree', 'pending') \
             ON CONFLICT (document_revision_id, provider) \
             DO UPDATE SET status = 'pending', updated_at = now()",
        )
        .bind(project_id)
        .bind(restored_id)
        .execute(&mut *tx)
        .await?;
    }

    audit(
        &mut tx,
        project_id,
        vetoed_by,
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
    tx.commit().await?;

    Ok(RevisionVetoOutcome {
        new_commit_sha: new_sha,
        rolled_back_revision_id: revision_id,
        restored_revision_id: restored.map(|(id, _)| id),
    })
}

/// 列出项目（或某文档）的提案，最新在前。
pub async fn list_proposals(
    pool: &PgPool,
    project_id: Uuid,
    document_id: Option<Uuid>,
) -> Result<Vec<ProposalRow>, KnowledgeError> {
    let rows = sqlx::query(
        "SELECT id, doc_key, doc_type, status, quality_gate_result, redline_kind, \
                change_summary, resulting_revision_id, created_at \
           FROM ai.document_proposal \
          WHERE project_id = $1 AND ($2::uuid IS NULL OR document_id = $2) \
          ORDER BY created_at DESC",
    )
    .bind(project_id)
    .bind(document_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| ProposalRow {
            id: row.get("id"),
            doc_key: row.get("doc_key"),
            doc_type: row.get("doc_type"),
            status: row.get("status"),
            quality_gate_result: row.get("quality_gate_result"),
            redline_kind: row.get("redline_kind"),
            change_summary: row.get("change_summary"),
            resulting_revision_id: row.get("resulting_revision_id"),
            created_at: row.get("created_at"),
        })
        .collect())
}

#[allow(clippy::too_many_arguments)]
async fn finalize_committed(
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
            "INSERT INTO ai.project_document (project_id, doc_key, doc_type, canonical_path) \
             VALUES ($1, $2, $3, $4) RETURNING id",
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
          redline_confirmed_by, published_at) \
         VALUES ($1,$2,$3,$4,$5,'published',$6,$7,$8,$9,$10,now()) RETURNING id",
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

    sqlx::query(
        "INSERT INTO ai.document_index_state \
         (project_id, document_revision_id, provider, status) \
         VALUES ($1, $2, 'markdown-tree', 'pending') \
         ON CONFLICT (document_revision_id, provider) DO NOTHING",
    )
    .bind(project_id)
    .bind(revision_id)
    .execute(&mut *tx)
    .await?;

    audit(
        &mut tx,
        project_id,
        redline_confirmed_by.unwrap_or(created_by),
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

async fn require_human(pool: &PgPool, principal_id: Uuid) -> Result<(), KnowledgeError> {
    let kind: Option<String> =
        sqlx::query_scalar("SELECT kind FROM core.principal WHERE id = $1")
            .bind(principal_id)
            .fetch_optional(pool)
            .await?;
    match kind.as_deref() {
        Some("human") => Ok(()),
        Some(other) => Err(KnowledgeError::Conflict(format!(
            "principal {principal_id} is `{other}`, a human confirmation is required"
        ))),
        None => Err(KnowledgeError::NotFound),
    }
}

async fn audit(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    actor_id: Uuid,
    action: &str,
    target_type: &str,
    target_id: Uuid,
    after_data: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (project_id, actor_id, action, target_type, target_id, after_data, source_kind) \
         VALUES ($1, $2, $3, $4, $5, $6, 'knowledge_v2')",
    )
    .bind(project_id)
    .bind(actor_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(after_data)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 供索引层复用的质量门结果序列化。
pub fn gates_as_json(result: &QualityGateResult) -> serde_json::Value {
    serde_json::to_value(result).unwrap_or_default()
}