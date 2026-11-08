use super::*;

const ANSWER_COLS: &str = "id, question_id, answer, answered_by, created_at";
type AnswerRow = (Uuid, Uuid, String, Uuid, DateTime<Utc>);

fn to_answer(row: AnswerRow) -> DiscoveryAnswerView {
    DiscoveryAnswerView {
        id: row.0,
        question_id: row.1,
        answer: row.2,
        answered_by: row.3,
        created_at: row.4,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveryAnswerView {
    pub id: Uuid,
    pub question_id: Uuid,
    pub answer: String,
    pub answered_by: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveryQuestionView {
    pub id: Uuid,
    pub session_id: Uuid,
    pub finding_id: Option<Uuid>,
    pub question: String,
    pub why_blocking: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub answers: Vec<DiscoveryAnswerView>,
}

pub async fn add_question(
    pool: &PgPool,
    tenant_id: Uuid,
    session_id: Uuid,
    question: &str,
    why_blocking: Option<&str>,
    finding_id: Option<Uuid>,
) -> Result<DiscoveryQuestionView, DiscoveryError> {
    if question.trim().is_empty() {
        return Err(DiscoveryError::Validation(
            "question must not be blank".to_string(),
        ));
    }
    let session = get_session(pool, tenant_id, session_id).await?;
    let project_id = session.project_id;
    if let Some(finding_id) = finding_id {
        let ok: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM discovery.discovery_finding \
             WHERE tenant_id = $1 AND id = $2 AND session_id = $3",
        )
        .bind(tenant_id)
        .bind(finding_id)
        .bind(session_id)
        .fetch_optional(pool)
        .await?;
        if ok.is_none() {
            return Err(DiscoveryError::FindingNotFound);
        }
    }
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO discovery.discovery_question \
         (tenant_id, project_id, session_id, finding_id, question, why_blocking) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(session_id)
    .bind(finding_id)
    .bind(question)
    .bind(why_blocking)
    .fetch_one(pool)
    .await?;
    get_question(pool, tenant_id, id).await
}

pub async fn get_question(
    pool: &PgPool,
    tenant_id: Uuid,
    question_id: Uuid,
) -> Result<DiscoveryQuestionView, DiscoveryError> {
    let row: Option<(
        Uuid,
        Uuid,
        Option<Uuid>,
        String,
        Option<String>,
        String,
        DateTime<Utc>,
    )> = sqlx::query_as(
        "SELECT id, session_id, finding_id, question, why_blocking, status, created_at \
             FROM discovery.discovery_question WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(question_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Err(DiscoveryError::QuestionNotFound);
    };
    let answers: Vec<AnswerRow> = sqlx::query_as(&format!(
        "SELECT {ANSWER_COLS} FROM discovery.discovery_answer \
         WHERE tenant_id = $1 AND question_id = $2 ORDER BY created_at, id"
    ))
    .bind(tenant_id)
    .bind(row.0)
    .fetch_all(pool)
    .await?;
    Ok(DiscoveryQuestionView {
        id: row.0,
        session_id: row.1,
        finding_id: row.2,
        question: row.3,
        why_blocking: row.4,
        status: row.5,
        created_at: row.6,
        answers: answers.into_iter().map(to_answer).collect(),
    })
}

pub async fn list_questions(
    pool: &PgPool,
    tenant_id: Uuid,
    session_id: Uuid,
) -> Result<Vec<DiscoveryQuestionView>, DiscoveryError> {
    let rows: Vec<(
        Uuid,
        Uuid,
        Option<Uuid>,
        String,
        Option<String>,
        String,
        DateTime<Utc>,
    )> = sqlx::query_as(
        "SELECT id, session_id, finding_id, question, why_blocking, status, created_at \
             FROM discovery.discovery_question \
             WHERE tenant_id = $1 AND session_id = $2 ORDER BY created_at, id",
    )
    .bind(tenant_id)
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    let answers: Vec<AnswerRow> = sqlx::query_as(
        "SELECT a.id, a.question_id, a.answer, a.answered_by, a.created_at \
         FROM discovery.discovery_answer a \
         JOIN discovery.discovery_question q \
           ON q.tenant_id = a.tenant_id AND q.id = a.question_id \
         WHERE a.tenant_id = $1 AND q.session_id = $2 ORDER BY a.created_at, a.id",
    )
    .bind(tenant_id)
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    let mut by_question: std::collections::HashMap<Uuid, Vec<DiscoveryAnswerView>> =
        std::collections::HashMap::new();
    for row in answers {
        let view = to_answer(row);
        by_question.entry(view.question_id).or_default().push(view);
    }
    Ok(rows
        .into_iter()
        .map(|row| DiscoveryQuestionView {
            id: row.0,
            session_id: row.1,
            finding_id: row.2,
            question: row.3,
            why_blocking: row.4,
            status: row.5,
            created_at: row.6,
            answers: by_question.remove(&row.0).unwrap_or_default(),
        })
        .collect())
}

pub async fn answer_question(
    pool: &PgPool,
    tenant_id: Uuid,
    question_id: Uuid,
    answer: &str,
    answered_by: Uuid,
) -> Result<DiscoveryAnswerView, DiscoveryError> {
    if answer.trim().is_empty() {
        return Err(DiscoveryError::Validation(
            "answer must not be blank".to_string(),
        ));
    }
    let mut tx = pool.begin().await?;
    let status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM discovery.discovery_question \
         WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(question_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(status) = status else {
        return Err(DiscoveryError::QuestionNotFound);
    };
    if status != QUESTION_STATUS_OPEN {
        return Err(DiscoveryError::Validation(format!(
            "question is {status}, only open questions can be answered"
        )));
    }
    let row: AnswerRow = sqlx::query_as(&format!(
        "INSERT INTO discovery.discovery_answer (tenant_id, question_id, answer, answered_by) \
         VALUES ($1, $2, $3, $4) RETURNING {ANSWER_COLS}"
    ))
    .bind(tenant_id)
    .bind(question_id)
    .bind(answer)
    .bind(answered_by)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE discovery.discovery_question SET status = 'answered' \
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(question_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(to_answer(row))
}

pub async fn defer_question(
    pool: &PgPool,
    tenant_id: Uuid,
    question_id: Uuid,
) -> Result<DiscoveryQuestionView, DiscoveryError> {
    let updated = sqlx::query(
        "UPDATE discovery.discovery_question SET status = 'deferred' \
         WHERE tenant_id = $1 AND id = $2 AND status = 'open'",
    )
    .bind(tenant_id)
    .bind(question_id)
    .execute(pool)
    .await?
    .rows_affected();
    if updated == 0 {
        let exists: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM discovery.discovery_question WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(question_id)
        .fetch_optional(pool)
        .await?;
        return match exists {
            None => Err(DiscoveryError::QuestionNotFound),
            Some(_) => Err(DiscoveryError::Validation(
                "question is not open".to_string(),
            )),
        };
    }
    get_question(pool, tenant_id, question_id).await
}
