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
    let mut conn = pool.acquire().await?;
    create_session_in(&mut conn, tenant_id, project_id, source, kind, created_by).await
}

/// [`create_session`] 的连接/事务内变体（DK-A 单向升级：建会话与线程链接同事务）。
pub async fn create_session_in(
    conn: &mut sqlx::PgConnection,
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
            .fetch_optional(&mut *conn)
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
    .fetch_one(&mut *conn)
    .await?;
    Ok(to_session(row))
}

pub async fn get_session(
    pool: &PgPool,
    tenant_id: Uuid,
    session_id: Uuid,
) -> Result<DiscoverySessionView, DiscoveryError> {
    let mut conn = pool.acquire().await?;
    get_session_in(&mut conn, tenant_id, session_id).await
}

/// [`get_session`] 的连接/事务内变体。
pub async fn get_session_in(
    conn: &mut sqlx::PgConnection,
    tenant_id: Uuid,
    session_id: Uuid,
) -> Result<DiscoverySessionView, DiscoveryError> {
    let row: Option<SessionRow> = sqlx::query_as(&format!(
        "SELECT {SESSION_COLS} FROM discovery.discovery_session \
         WHERE tenant_id = $1 AND id = $2"
    ))
    .bind(tenant_id)
    .bind(session_id)
    .fetch_optional(&mut *conn)
    .await?;
    row.map(to_session).ok_or(DiscoveryError::SessionNotFound)
}

/// 事务内对会话行取 `FOR UPDATE` 并返回当前视图——会话级追加互斥点（C7，与 DK-B
/// `91024f1` 池版 `append_message` 同一把锁），调用 [`append_message_in_tx`] 之前取。
/// 锁序：调用方若另持 `ai.conversation` 行锁，须先 conversation 后 session。
pub async fn lock_session_in(
    conn: &mut sqlx::PgConnection,
    tenant_id: Uuid,
    session_id: Uuid,
) -> Result<DiscoverySessionView, DiscoveryError> {
    let row: Option<SessionRow> = sqlx::query_as(&format!(
        "SELECT {SESSION_COLS} FROM discovery.discovery_session \
         WHERE tenant_id = $1 AND id = $2 FOR UPDATE"
    ))
    .bind(tenant_id)
    .bind(session_id)
    .fetch_optional(&mut *conn)
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
    let mut tx = pool.begin().await?;
    let view = transition_session_in(&mut tx, tenant_id, session_id, target).await?;
    tx.commit().await?;
    Ok(view)
}

/// [`transition_session`] 的事务内变体（DK-A 撤销升级：置 PAUSED 与回退线程
/// 链接同事务）。对会话行加 `FOR UPDATE`；提交由调用方负责。
pub async fn transition_session_in(
    conn: &mut sqlx::PgConnection,
    tenant_id: Uuid,
    session_id: Uuid,
    target: &str,
) -> Result<DiscoverySessionView, DiscoveryError> {
    validate_one_of(target, &SESSION_STATUSES, "status")?;
    let current: Option<String> = sqlx::query_scalar(
        "SELECT status FROM discovery.discovery_session \
         WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(session_id)
    .fetch_optional(&mut *conn)
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
    .fetch_one(&mut *conn)
    .await?;
    Ok(to_session(row))
}

/// 合法状态边（唯一来源：`session_transition_allowed` 与编排器的多跳路径共用）。
///
/// **设计勘误（DK-B）**：设计稿 F2 声称含 `NEEDS_USER_INPUT → REVIEW_REQUIRED`，
/// 代码实际**没有**这条边（本表即事实）。编排器保持零迁移：走
/// `NEEDS_USER_INPUT → IN_PROGRESS → REVIEW_REQUIRED` 两跳，终态一致。
pub(crate) const SESSION_EDGES: [(&str, &str); 10] = [
    ("IN_PROGRESS", "NEEDS_USER_INPUT"),
    ("IN_PROGRESS", "REVIEW_REQUIRED"),
    ("IN_PROGRESS", "PAUSED"),
    ("IN_PROGRESS", "COMPLETED"),
    ("NEEDS_USER_INPUT", "IN_PROGRESS"),
    ("NEEDS_USER_INPUT", "COMPLETED"),
    ("REVIEW_REQUIRED", "IN_PROGRESS"),
    ("REVIEW_REQUIRED", "COMPLETED"),
    ("PAUSED", "IN_PROGRESS"),
    ("PAUSED", "COMPLETED"),
];

fn session_transition_allowed(from: &str, to: &str) -> bool {
    SESSION_EDGES.contains(&(from, to))
}

/// 从 `from` 到 `to` 的合法路径（0/1/2 跳）；无路径返回 `None`。
///
/// 编排器只用来落「本轮结束后的状态」：单跳优先，必要时走两跳
/// （见 [`SESSION_EDGES`] 的勘误说明）。路径每一跳都必须在既有边上，
/// 不新造转移。
pub(crate) fn session_transition_path(from: &str, to: &str) -> Option<Vec<String>> {
    if from == to {
        return Some(Vec::new());
    }
    if session_transition_allowed(from, to) {
        return Some(vec![to.to_string()]);
    }
    for (a, mid) in SESSION_EDGES {
        if a != from {
            continue;
        }
        if session_transition_allowed(mid, to) {
            return Some(vec![mid.to_string(), to.to_string()]);
        }
    }
    None
}
