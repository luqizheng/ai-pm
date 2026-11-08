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

/// 追加一条访谈消息。
///
/// **C7 双入口串行（DK-B / D-DK-9 附注）**：`seq` 由 `MAX(seq)+1` 生成，
/// 而 REST（pm-api 进程）与访谈编排器（ai-pm-worker 进程）会并发追加同一
/// session 的消息——进程内锁覆盖不了跨进程竞争。因此本函数在**同一事务内**
/// 先对 `discovery_session` 行取 `FOR UPDATE`，再计算 `seq`：两个入口都走
/// 这一个函数，行锁即成为跨进程的会话级互斥，`UNIQUE(tenant_id,
/// session_id, seq)` 不可能被撞（单点服务内的加锁收紧，不属绕过 V-6）。
///
/// 锁序：session → message（本函数只取 session 锁；`record_finding_verification`
/// 的 project → session → finding 锁序与之同向，不成环）。
pub async fn append_message(
    pool: &PgPool,
    tenant_id: Uuid,
    session_id: Uuid,
    speaker: &str,
    source: &str,
    content: &str,
) -> Result<DiscoveryMessageView, DiscoveryError> {
    let mut tx = pool.begin().await?;
    // 会话行锁 = 本 session 的追加互斥点（跨进程有效）。顺带完成存在性校验。
    let locked: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM discovery.discovery_session \
         WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(session_id)
    .fetch_optional(&mut *tx)
    .await?;
    if locked.is_none() {
        return Err(DiscoveryError::SessionNotFound);
    }
    let message =
        append_message_in_tx(&mut tx, tenant_id, session_id, speaker, source, content).await?;
    tx.commit().await?;
    Ok(message)
}

/// 事务内追加（DK-A 与 DK-B 合并后同一把锁的唯一写点）。
///
/// 调用方**必须**已在同事务内持有该 session 的行锁——[`lock_session_in`]
/// （DK-A：升级交接摘要与 interview 落点用户消息同线程链接同事务）、
/// [`append_message`] 池版（先加锁后调本函数）、`apply_orchestrator_round`
/// （DK-B：一轮事务里同时写 finding/question/状态/消息）三个入口共用，
/// 行锁即跨进程的会话级追加互斥点（C7）；`seq` 由 `MAX(seq)+1` 生成，
/// `UNIQUE(tenant_id, session_id, seq)` 不可能被撞。
pub(crate) async fn append_message_in_tx(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
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
    .fetch_one(&mut **tx)
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
