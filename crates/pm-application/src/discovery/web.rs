use super::*;

#[derive(Debug, Clone, Serialize)]
pub struct StartWebSession {
    pub session: DiscoverySessionView,
    pub questions_added: usize,
    pub linked_import_id: Option<Uuid>,
}

/// Web 入口创建 `source=WEB` 的认知会话；可选挂接既有 repo_import（Q1 单向 FK）
/// 并把其 `draft.unknowns` 映射为 `discovery_question`（设计稿 §三）。
pub async fn start_session_from_import(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    created_by: Uuid,
    import_id: Option<Uuid>,
) -> Result<StartWebSession, DiscoveryError> {
    let session =
        create_session(pool, tenant_id, project_id, "WEB", "INITIAL_ONBOARDING", created_by).await?;
    let Some(import_id) = import_id else {
        return Ok(StartWebSession {
            session,
            questions_added: 0,
            linked_import_id: None,
        });
    };
    let draft_value: Option<(serde_json::Value,)> = sqlx::query_as(
        "SELECT draft FROM integration.repo_import \
         WHERE tenant_id = $1 AND project_id = $2 AND id = $3",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(import_id)
    .fetch_optional(pool)
    .await?;
    let Some((draft_value,)) = draft_value else {
        return Err(DiscoveryError::ImportNotFound);
    };
    sqlx::query(
        "UPDATE integration.repo_import SET discovery_session_id = $3 \
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(import_id)
    .bind(session.id)
    .execute(pool)
    .await?;
    let draft: pm_domain::import::ProjectDraft = serde_json::from_value(draft_value)
        .map_err(|e| DiscoveryError::Validation(format!("repo_import draft is invalid: {e}")))?;
    let mut questions_added = 0usize;
    for unknown in draft.unknowns {
        add_question(
            pool,
            tenant_id,
            session.id,
            &unknown.question,
            Some(unknown.why_blocking.as_str()),
            None,
        )
        .await?;
        questions_added += 1;
    }
    Ok(StartWebSession {
        session,
        questions_added,
        linked_import_id: Some(import_id),
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct FindingVerificationOutcome {
    pub finding: DiscoveryFindingView,
    pub question: Option<DiscoveryQuestionView>,
    pub answer: Option<DiscoveryAnswerView>,
    pub confirmed_finding: Option<DiscoveryFindingView>,
}

/// 发现卡片确认（设计稿 §五）：用户选择写 `discovery_answer`，并把
/// `verification_status` 从 `PENDING` 推进；选项对应「已通过其他方式解决」时
/// 同时产生一条 `user_confirmed` finding。每轮确认落一对 question + answer。
pub async fn record_finding_verification(
    pool: &PgPool,
    tenant_id: Uuid,
    session_id: Uuid,
    finding_id: Uuid,
    status: &str,
    answer: &str,
    answered_by: Uuid,
    confirmed_finding_content: Option<&str>,
) -> Result<FindingVerificationOutcome, DiscoveryError> {
    let finding = set_finding_verification(pool, tenant_id, finding_id, status).await?;
    if finding.session_id != session_id {
        return Err(DiscoveryError::FindingNotFound);
    }
    let existing: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT id, status FROM discovery.discovery_question \
         WHERE tenant_id = $1 AND finding_id = $2 \
         ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .bind(tenant_id)
    .bind(finding_id)
    .fetch_optional(pool)
    .await?;
    let (question, answer) = match existing {
        Some((question_id, question_status)) if question_status == QUESTION_STATUS_OPEN => {
            let answered = answer_question(pool, tenant_id, question_id, answer, answered_by).await?;
            let question = get_question(pool, tenant_id, question_id).await?;
            (question, Some(answered))
        }
        _ => {
            let question = add_question(
                pool,
                tenant_id,
                session_id,
                &finding.content,
                None,
                Some(finding.id),
            )
            .await?;
            let answered = answer_question(pool, tenant_id, question.id, answer, answered_by).await?;
            let question = get_question(pool, tenant_id, question.id).await?;
            (question, Some(answered))
        }
    };
    let confirmed_finding = match confirmed_finding_content {
        Some(content) if !content.trim().is_empty() => Some(
            add_finding(
                pool,
                tenant_id,
                session_id,
                NewFinding {
                    kind: "user_confirmed".to_string(),
                    content: content.to_string(),
                    confidence: None,
                    evidence: serde_json::json!([]),
                    repository_ids: vec![],
                    verification_status: None,
                    actor_role: None,
                },
            )
            .await?,
        ),
        _ => None,
    };
    Ok(FindingVerificationOutcome {
        finding,
        question: Some(question),
        answer,
        confirmed_finding,
    })
}