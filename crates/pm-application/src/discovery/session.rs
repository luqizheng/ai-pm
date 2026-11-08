use super::*;

const SESSION_COLS: &str =
    "id, project_id, source, kind, status, created_by, created_at, updated_at";
type SessionRow = (
    Uuid,
    Uuid,
    String,
    String,
    String,
    Uuid,
    DateTime<Utc>,
    DateTime<Utc>,
);

fn to_session(row: SessionRow) -> DiscoverySessionView {
    DiscoverySessionView {
        id: row.0,
        project_id: row.1,
        source: row.2,
        kind: row.3,
        status: row.4,
        created_by: row.5,
        created_at: row.6,
        updated_at: row.7,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoverySessionView {
    pub id: Uuid,
    pub project_id: Uuid,
    pub source: String,
    pub kind: String,
    pub status: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoverySessionDetail {
    pub session: DiscoverySessionView,
    pub messages: Vec<DiscoveryMessageView>,
    pub findings: Vec<DiscoveryFindingView>,
    pub questions: Vec<DiscoveryQuestionView>,
    pub proposals: Vec<DiscoveryProposalView>,
}

pub async fn create_session(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    source: &str,
    kind: &str,
    created_by: Uuid,
) -> Result<DiscoverySessionView, DiscoveryError> {
    validate_one_of(source, &SESSION_SOURCES, "source")?;
    validate_one_of(kind, &SESSION_KINDS, "kind")?;
    let exists: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM core.project WHERE tenant_id = $1 AND id = $2")
            .bind(tenant_id)
            .bind(project_id)
            .fetch_optional(pool)
            .await?;
    if exists.is_none() {
        return Err(DiscoveryError::ProjectNotFound);
    }
    let row: SessionRow = sqlx::query_as(&format!(
        "INSERT INTO discovery.discovery_session \
         (tenant_id, project_id, source, kind, status, created_by) \
         VALUES ($1, $2, $3, $4, 'IN_PROGRESS', $5) RETURNING {SESSION_COLS}"
    ))
    .bind(tenant_id)
    .bind(project_id)
    .bind(source)
    .bind(kind)
    .bind(created_by)
    .fetch_one(pool)
    .await?;
    Ok(to_session(row))
}

pub async fn get_session(
    pool: &PgPool,
    tenant_id: Uuid,
    session_id: Uuid,
) -> Result<DiscoverySessionView, DiscoveryError> {
    let row: Option<SessionRow> = sqlx::query_as(&format!(
        "SELECT {SESSION_COLS} FROM discovery.discovery_session \
         WHERE tenant_id = $1 AND id = $2"
    ))
    .bind(tenant_id)
    .bind(session_id)
    .fetch_optional(pool)
    .await?;
    row.map(to_session).ok_or(DiscoveryError::SessionNotFound)
}

pub async fn list_sessions(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<Vec<DiscoverySessionView>, DiscoveryError> {
    let rows: Vec<SessionRow> = sqlx::query_as(&format!(
        "SELECT {SESSION_COLS} FROM discovery.discovery_session \
         WHERE tenant_id = $1 AND project_id = $2 ORDER BY created_at DESC, id"
    ))
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(to_session).collect())
}

pub async fn get_session_detail(
    pool: &PgPool,
    tenant_id: Uuid,
    session_id: Uuid,
) -> Result<DiscoverySessionDetail, DiscoveryError> {
    let session = get_session(pool, tenant_id, session_id).await?;
    let messages = list_messages(pool, tenant_id, session_id).await?;
    let findings = list_findings(pool, tenant_id, session_id).await?;
    let questions = list_questions(pool, tenant_id, session_id).await?;
    let proposals = list_proposals(pool, tenant_id, session_id).await?;
    Ok(DiscoverySessionDetail {
        session,
        messages,
        findings,
        questions,
        proposals,
    })
}

pub async fn transition_session(
    pool: &PgPool,
    tenant_id: Uuid,
    session_id: Uuid,
    target: &str,
) -> Result<DiscoverySessionView, DiscoveryError> {
    validate_one_of(target, &SESSION_STATUSES, "status")?;
    let mut tx = pool.begin().await?;
    let current: Option<String> = sqlx::query_scalar(
        "SELECT status FROM discovery.discovery_session \
         WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(session_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(from) = current else {
        return Err(DiscoveryError::SessionNotFound);
    };
    if !session_transition_allowed(&from, target) {
        return Err(DiscoveryError::InvalidTransition {
            from,
            to: target.to_string(),
        });
    }
    let row: SessionRow = sqlx::query_as(&format!(
        "UPDATE discovery.discovery_session SET status = $3, updated_at = now() \
         WHERE tenant_id = $1 AND id = $2 RETURNING {SESSION_COLS}"
    ))
    .bind(tenant_id)
    .bind(session_id)
    .bind(target)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(to_session(row))
}

fn session_transition_allowed(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        ("IN_PROGRESS", "NEEDS_USER_INPUT")
            | ("IN_PROGRESS", "REVIEW_REQUIRED")
            | ("IN_PROGRESS", "PAUSED")
            | ("IN_PROGRESS", "COMPLETED")
            | ("NEEDS_USER_INPUT", "IN_PROGRESS")
            | ("NEEDS_USER_INPUT", "COMPLETED")
            | ("REVIEW_REQUIRED", "IN_PROGRESS")
            | ("REVIEW_REQUIRED", "COMPLETED")
            | ("PAUSED", "IN_PROGRESS")
            | ("PAUSED", "COMPLETED")
    )
}
