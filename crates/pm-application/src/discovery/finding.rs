use super::*;

const FINDING_COLS: &str =
    "id, session_id, kind, content, confidence, verification_status, evidence, actor_role, created_at";
type FindingRow = (
    Uuid,
    Uuid,
    String,
    String,
    Option<f64>,
    String,
    serde_json::Value,
    Option<String>,
    DateTime<Utc>,
);

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveryFindingView {
    pub id: Uuid,
    pub session_id: Uuid,
    pub kind: String,
    pub content: String,
    pub confidence: Option<f64>,
    pub verification_status: String,
    pub evidence: serde_json::Value,
    pub actor_role: Option<String>,
    pub repository_ids: Vec<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewFinding {
    pub kind: String,
    pub content: String,
    pub confidence: Option<f64>,
    #[serde(default)]
    pub evidence: serde_json::Value,
    #[serde(default)]
    pub repository_ids: Vec<Uuid>,
    pub verification_status: Option<String>,
    pub actor_role: Option<String>,
}

fn to_finding(row: FindingRow, repository_ids: Vec<Uuid>) -> DiscoveryFindingView {
    DiscoveryFindingView {
        id: row.0,
        session_id: row.1,
        kind: row.2,
        content: row.3,
        confidence: row.4,
        verification_status: row.5,
        evidence: row.6,
        actor_role: row.7,
        repository_ids,
        created_at: row.8,
    }
}

async fn repository_ids_by_session(
    pool: &PgPool,
    tenant_id: Uuid,
    session_id: Uuid,
) -> Result<std::collections::HashMap<Uuid, Vec<Uuid>>, DiscoveryError> {
    let links: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT fr.finding_id, fr.repository_id \
         FROM discovery.discovery_finding_repository fr \
         JOIN discovery.discovery_finding f \
           ON f.tenant_id = fr.tenant_id AND f.id = fr.finding_id \
         WHERE fr.tenant_id = $1 AND f.session_id = $2",
    )
    .bind(tenant_id)
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    let mut map: std::collections::HashMap<Uuid, Vec<Uuid>> = std::collections::HashMap::new();
    for (finding_id, repository_id) in links {
        map.entry(finding_id).or_default().push(repository_id);
    }
    Ok(map)
}

pub async fn add_finding(
    pool: &PgPool,
    tenant_id: Uuid,
    session_id: Uuid,
    new_finding: NewFinding,
) -> Result<DiscoveryFindingView, DiscoveryError> {
    validate_one_of(&new_finding.kind, &FINDING_KINDS, "kind")?;
    validate_confidence(new_finding.confidence)?;
    let verification_status = new_finding.verification_status.as_deref().unwrap_or("PENDING");
    validate_one_of(verification_status, &VERIFICATION_STATUSES, "verification_status")?;
    if new_finding.content.trim().is_empty() {
        return Err(DiscoveryError::Validation(
            "content must not be blank".to_string(),
        ));
    }
    get_session(pool, tenant_id, session_id).await?;
    let evidence = empty_array_if_null(new_finding.evidence);
    let repository_ids = new_finding.repository_ids;
    if !repository_ids.is_empty() {
        let found: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM integration.project_repository \
             WHERE tenant_id = $1 AND id = ANY($2)",
        )
        .bind(tenant_id)
        .bind(&repository_ids)
        .fetch_one(pool)
        .await?;
        if found != repository_ids.len() as i64 {
            return Err(DiscoveryError::Validation(
                "repository_ids must all belong to the tenant".to_string(),
            ));
        }
    }
    let mut tx = pool.begin().await?;
    let row: FindingRow = sqlx::query_as(&format!(
        "INSERT INTO discovery.discovery_finding \
         (tenant_id, session_id, kind, content, confidence, verification_status, evidence, actor_role) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING {FINDING_COLS}"
    ))
    .bind(tenant_id)
    .bind(session_id)
    .bind(&new_finding.kind)
    .bind(&new_finding.content)
    .bind(new_finding.confidence)
    .bind(verification_status)
    .bind(&evidence)
    .bind(&new_finding.actor_role)
    .fetch_one(&mut *tx)
    .await?;
    for repository_id in &repository_ids {
        sqlx::query(
            "INSERT INTO discovery.discovery_finding_repository \
             (tenant_id, finding_id, repository_id) VALUES ($1, $2, $3)",
        )
        .bind(tenant_id)
        .bind(row.0)
        .bind(repository_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(to_finding(row, repository_ids))
}

pub async fn list_findings(
    pool: &PgPool,
    tenant_id: Uuid,
    session_id: Uuid,
) -> Result<Vec<DiscoveryFindingView>, DiscoveryError> {
    let rows: Vec<FindingRow> = sqlx::query_as(&format!(
        "SELECT {FINDING_COLS} FROM discovery.discovery_finding \
         WHERE tenant_id = $1 AND session_id = $2 ORDER BY created_at, id"
    ))
    .bind(tenant_id)
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    let links = repository_ids_by_session(pool, tenant_id, session_id).await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let ids = links.get(&row.0).cloned().unwrap_or_default();
            to_finding(row, ids)
        })
        .collect())
}

pub async fn get_finding(
    pool: &PgPool,
    tenant_id: Uuid,
    finding_id: Uuid,
) -> Result<DiscoveryFindingView, DiscoveryError> {
    let row: Option<FindingRow> = sqlx::query_as(&format!(
        "SELECT {FINDING_COLS} FROM discovery.discovery_finding WHERE tenant_id = $1 AND id = $2"
    ))
    .bind(tenant_id)
    .bind(finding_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Err(DiscoveryError::FindingNotFound);
    };
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT repository_id FROM discovery.discovery_finding_repository \
         WHERE tenant_id = $1 AND finding_id = $2 ORDER BY repository_id",
    )
    .bind(tenant_id)
    .bind(row.0)
    .fetch_all(pool)
    .await?;
    Ok(to_finding(row, ids))
}

pub async fn set_finding_verification(
    pool: &PgPool,
    tenant_id: Uuid,
    finding_id: Uuid,
    status: &str,
) -> Result<DiscoveryFindingView, DiscoveryError> {
    validate_one_of(status, &VERIFICATION_STATUSES, "verification_status")?;
    let updated = sqlx::query(
        "UPDATE discovery.discovery_finding SET verification_status = $3 \
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(finding_id)
    .bind(status)
    .execute(pool)
    .await?
    .rows_affected();
    if updated == 0 {
        return Err(DiscoveryError::FindingNotFound);
    }
    get_finding(pool, tenant_id, finding_id).await
}