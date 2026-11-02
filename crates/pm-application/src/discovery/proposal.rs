use super::*;

const PROPOSAL_COLS: &str =
    "id, session_id, action, target_document, document_id, base_revision_id, proposed_content, evidence, status, document_proposal_id, created_at, updated_at";
type ProposalRow = (
    Uuid,
    Uuid,
    String,
    String,
    Option<Uuid>,
    Option<Uuid>,
    String,
    serde_json::Value,
    String,
    Option<Uuid>,
    DateTime<Utc>,
    DateTime<Utc>,
);

fn to_proposal(row: ProposalRow) -> DiscoveryProposalView {
    DiscoveryProposalView {
        id: row.0,
        session_id: row.1,
        action: row.2,
        target_document: row.3,
        document_id: row.4,
        base_revision_id: row.5,
        proposed_content: row.6,
        evidence: row.7,
        status: row.8,
        document_proposal_id: row.9,
        created_at: row.10,
        updated_at: row.11,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveryProposalView {
    pub id: Uuid,
    pub session_id: Uuid,
    pub action: String,
    pub target_document: String,
    pub document_id: Option<Uuid>,
    pub base_revision_id: Option<Uuid>,
    pub proposed_content: String,
    pub evidence: serde_json::Value,
    pub status: String,
    pub document_proposal_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewProposal {
    pub action: String,
    pub target_document: String,
    pub document_id: Option<Uuid>,
    pub base_revision_id: Option<Uuid>,
    pub proposed_content: String,
    #[serde(default)]
    pub evidence: serde_json::Value,
}

pub async fn add_proposal(
    pool: &PgPool,
    tenant_id: Uuid,
    session_id: Uuid,
    new_proposal: NewProposal,
) -> Result<DiscoveryProposalView, DiscoveryError> {
    validate_one_of(&new_proposal.action, &[PROPOSAL_ACTION_ADD, PROPOSAL_ACTION_MODIFY], "action")?;
    if new_proposal.target_document.trim().is_empty() {
        return Err(DiscoveryError::Validation(
            "target_document must not be blank".to_string(),
        ));
    }
    if new_proposal.proposed_content.trim().is_empty() {
        return Err(DiscoveryError::Validation(
            "proposed_content must not be blank".to_string(),
        ));
    }
    get_session(pool, tenant_id, session_id).await?;
    let document_id = new_proposal.document_id;
    let base_revision_id = new_proposal.base_revision_id;
    match new_proposal.action.as_str() {
        PROPOSAL_ACTION_MODIFY => {
            let Some(document_id) = document_id else {
                return Err(DiscoveryError::Validation(
                    "modify proposals require document_id".to_string(),
                ));
            };
            let Some(base_revision_id) = base_revision_id else {
                return Err(DiscoveryError::Validation(
                    "modify proposals require base_revision_id (Q7)".to_string(),
                ));
            };
            let ok: Option<Uuid> = sqlx::query_scalar(
                "SELECT r.id FROM ai.document_revision r \
                 JOIN ai.project_document d \
                   ON d.tenant_id = r.tenant_id AND d.id = r.document_id \
                 WHERE r.tenant_id = $1 AND r.id = $2 AND r.document_id = $3",
            )
            .bind(tenant_id)
            .bind(base_revision_id)
            .bind(document_id)
            .fetch_optional(pool)
            .await?;
            if ok.is_none() {
                return Err(DiscoveryError::Validation(
                    "base_revision_id must reference a revision of document_id".to_string(),
                ));
            }
        }
        PROPOSAL_ACTION_ADD => {
            if let Some(document_id) = document_id {
                let ok: Option<Uuid> = sqlx::query_scalar(
                    "SELECT id FROM ai.project_document WHERE tenant_id = $1 AND id = $2",
                )
                .bind(tenant_id)
                .bind(document_id)
                .fetch_optional(pool)
                .await?;
                if ok.is_none() {
                    return Err(DiscoveryError::Validation(
                        "document_id must belong to the tenant".to_string(),
                    ));
                }
            }
        }
        _ => unreachable!(),
    }
    let evidence = empty_array_if_null(new_proposal.evidence);
    let row: ProposalRow = sqlx::query_as(&format!(
        "INSERT INTO discovery.discovery_proposal \
         (tenant_id, session_id, action, target_document, document_id, base_revision_id, proposed_content, evidence) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING {PROPOSAL_COLS}"
    ))
    .bind(tenant_id)
    .bind(session_id)
    .bind(&new_proposal.action)
    .bind(&new_proposal.target_document)
    .bind(document_id)
    .bind(base_revision_id)
    .bind(&new_proposal.proposed_content)
    .bind(&evidence)
    .fetch_one(pool)
    .await?;
    Ok(to_proposal(row))
}

pub async fn list_proposals(
    pool: &PgPool,
    tenant_id: Uuid,
    session_id: Uuid,
) -> Result<Vec<DiscoveryProposalView>, DiscoveryError> {
    let rows: Vec<ProposalRow> = sqlx::query_as(&format!(
        "SELECT {PROPOSAL_COLS} FROM discovery.discovery_proposal \
         WHERE tenant_id = $1 AND session_id = $2 ORDER BY created_at, id"
    ))
    .bind(tenant_id)
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(to_proposal).collect())
}

/// 知识提案链落库后回填 `ai.document_proposal` 关联（§八 submit_proposal）。
pub async fn link_proposal_document(
    pool: &PgPool,
    tenant_id: Uuid,
    proposal_id: Uuid,
    document_proposal_id: Uuid,
) -> Result<DiscoveryProposalView, DiscoveryError> {
    let updated = sqlx::query(
        "UPDATE discovery.discovery_proposal \
         SET document_proposal_id = $3, updated_at = now() \
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(proposal_id)
    .bind(document_proposal_id)
    .execute(pool)
    .await?
    .rows_affected();
    if updated == 0 {
        return Err(DiscoveryError::ProposalNotFound);
    }
    let row: Option<ProposalRow> = sqlx::query_as(&format!(
        "SELECT {PROPOSAL_COLS} FROM discovery.discovery_proposal \
         WHERE tenant_id = $1 AND id = $2"
    ))
    .bind(tenant_id)
    .bind(proposal_id)
    .fetch_optional(pool)
    .await?;
    row.map(to_proposal).ok_or(DiscoveryError::ProposalNotFound)
}

pub async fn set_proposal_status(
    pool: &PgPool,
    tenant_id: Uuid,
    proposal_id: Uuid,
    status: &str,
    document_proposal_id: Option<Uuid>,
) -> Result<DiscoveryProposalView, DiscoveryError> {
    validate_one_of(
        status,
        &[
            PROPOSAL_STATUS_APPROVED,
            PROPOSAL_STATUS_REJECTED,
            PROPOSAL_STATUS_CONFLICT,
        ],
        "status",
    )?;
    let mut tx = pool.begin().await?;
    let current: Option<String> = sqlx::query_scalar(
        "SELECT status FROM discovery.discovery_proposal \
         WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(proposal_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(current) = current else {
        return Err(DiscoveryError::ProposalNotFound);
    };
    if current != PROPOSAL_STATUS_PENDING {
        return Err(DiscoveryError::Validation(format!(
            "proposal is {current}, only pending proposals can be decided"
        )));
    }
    let row: ProposalRow = sqlx::query_as(&format!(
        "UPDATE discovery.discovery_proposal \
         SET status = $3, document_proposal_id = COALESCE($4, document_proposal_id), updated_at = now() \
         WHERE tenant_id = $1 AND id = $2 RETURNING {PROPOSAL_COLS}"
    ))
    .bind(tenant_id)
    .bind(proposal_id)
    .bind(status)
    .bind(document_proposal_id)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(to_proposal(row))
}