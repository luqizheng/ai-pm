use super::*;

const MESSAGE_COLS: &str = "id, session_id, speaker, source, content, seq, created_at";
type MessageRow = (Uuid, Uuid, String, String, String, i64, DateTime<Utc>);

fn to_message(row: MessageRow) -> DiscoveryMessageView {
    DiscoveryMessageView {
        id: row.0,
        session_id: row.1,
        speaker: row.2,
        source: row.3,
        content: row.4,
        seq: row.5,
        created_at: row.6,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveryMessageView {
    pub id: Uuid,
    pub session_id: Uuid,
    pub speaker: String,
    pub source: String,
    pub content: String,
    pub seq: i64,
    pub created_at: DateTime<Utc>,
}

pub async fn append_message(
    pool: &PgPool,
    tenant_id: Uuid,
    session_id: Uuid,
    speaker: &str,
    source: &str,
    content: &str,
) -> Result<DiscoveryMessageView, DiscoveryError> {
    validate_one_of(speaker, &["ai", "user"], "speaker")?;
    validate_one_of(source, &SESSION_SOURCES, "source")?;
    if content.trim().is_empty() {
        return Err(DiscoveryError::Validation(
            "content must not be blank".to_string(),
        ));
    }
    get_session(pool, tenant_id, session_id).await?;
    let row: MessageRow = sqlx::query_as(&format!(
        "INSERT INTO discovery.discovery_message \
         (tenant_id, session_id, speaker, source, content, seq) \
         VALUES ($1, $2, $3, $4, $5, \
                 (SELECT COALESCE(MAX(seq), 0) + 1 FROM discovery.discovery_message \
                  WHERE tenant_id = $1 AND session_id = $2)) \
         RETURNING {MESSAGE_COLS}"
    ))
    .bind(tenant_id)
    .bind(session_id)
    .bind(speaker)
    .bind(source)
    .bind(content)
    .fetch_one(pool)
    .await?;
    Ok(to_message(row))
}

pub async fn list_messages(
    pool: &PgPool,
    tenant_id: Uuid,
    session_id: Uuid,
) -> Result<Vec<DiscoveryMessageView>, DiscoveryError> {
    let rows: Vec<MessageRow> = sqlx::query_as(&format!(
        "SELECT {MESSAGE_COLS} FROM discovery.discovery_message \
         WHERE tenant_id = $1 AND session_id = $2 ORDER BY seq"
    ))
    .bind(tenant_id)
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(to_message).collect())
}
