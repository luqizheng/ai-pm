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
    start_session_from_import_as(
        pool,
        tenant_id,
        project_id,
        created_by,
        import_id,
        "WEB",
    )
    .await
}

/// 同上，但**来源可指定**：Worker 在导入草案就绪后自动开 `source=SYSTEM` 会话
/// （Discovery Worker 分析作业，台账 G-20）；Web 向导仍用 `WEB`。
pub async fn start_session_from_import_as(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    created_by: Uuid,
    import_id: Option<Uuid>,
    source: &str,
) -> Result<StartWebSession, DiscoveryError> {
    let session =
        create_session(pool, tenant_id, project_id, source, "INITIAL_ONBOARDING", created_by).await?;
    let Some(import_id) = import_id else {
        return Ok(StartWebSession {
            session,
            questions_added: 0,
            linked_import_id: None,
        });
    };
    let draft_value: Option<(serde_json::Value,)> = sqlx::query_as(
        "SELECT dv.draft FROM integration.repo_import i \
         LEFT JOIN integration.repo_import_draft_version dv \
                ON dv.tenant_id = i.tenant_id AND dv.import_id = i.id \
               AND dv.revision = i.draft_revision \
         WHERE i.tenant_id = $1 AND i.project_id = $2 AND i.id = $3",
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

/// 发现卡片确认（设计稿 §五）——**I2-C 统一人工决定服务**。
///
/// 单事务完成（I2-036：Finding 更新 + question/answer + 审计**同一事务**，
/// 审计失败整体回滚）：
///
/// 1. 行锁读 Finding，**先**校验 Session 归属（I2-029：路径 Session 与 Finding
///    实际 Session 不一致 → 404，零副作用）；
/// 2. 调用人必须是真人（I2-025：`core.principal.kind='human'`）；
/// 3. 条件更新 `verification_status`（I2-020：同值重复决定是 no-op，不覆盖）；
/// 4. 写 question/answer；选项「已通过其他方式解决」时补一条 `user_confirmed`
///    finding（HumanDirect 通道，创建即 CONFIRMED）；
/// 5. 审计行 `finding.decided`（带 before/after 与 answer 摘要）。
///
/// `operation_id` 幂等（I2-C，沿用 N-4 先例）：透传到
/// `integration.inbound_event UNIQUE (project_id, source, idempotency_key)`，
/// 重放返回当前状态、零业务写入。
#[allow(clippy::too_many_arguments)]
pub async fn record_finding_verification(
    pool: &PgPool,
    tenant_id: Uuid,
    session_id: Uuid,
    finding_id: Uuid,
    status: &str,
    answer: &str,
    answered_by: Uuid,
    confirmed_finding_content: Option<&str>,
    operation_id: Option<&str>,
) -> Result<FindingVerificationOutcome, DiscoveryError> {
    // REVISION_REQUESTED（要求补充）不是 verification_status——它**不改状态**、
    // 只落设计稿 §8.2 指定的审计 action；其余按既有四态校验。
    let is_revision_request = status == DECISION_REVISION_REQUESTED;
    if !is_revision_request {
        validate_one_of(status, &VERIFICATION_STATUSES, "verification_status")?;
    }
    if answer.trim().is_empty() {
        return Err(DiscoveryError::Validation(
            "answer must not be blank".to_string(),
        ));
    }

    let mut tx = pool.begin().await?;

    // Session → Project（审计与幂等键的锚点）；**先锁 Project 行**——
    // 归档与决定的竞争由行锁串行化（I2 §15.3 场景 3；锁序 project → session →
    // finding 单向，与 I1 的 project → run → task 同向不成环）。
    let session_row: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT s.project_id, p.status \
         FROM discovery.discovery_session s \
         JOIN core.project p ON p.tenant_id = s.tenant_id AND p.id = s.project_id \
         WHERE s.tenant_id = $1 AND s.id = $2 \
         FOR UPDATE OF s",
    )
    .bind(tenant_id)
    .bind(session_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((project_id, project_status)) = session_row else {
        return Err(DiscoveryError::SessionNotFound);
    };
    if project_status == "archived" {
        // 归档项目只读（I2-C §4「Project 非 archived」）：零写入。
        return Err(DiscoveryError::Validation(
            "project is archived; findings are read-only".to_string(),
        ));
    }

    // operation_id 幂等：重放返回当前状态、零业务写入（N-4 先例）。
    if let Some(key) = operation_id.map(str::trim).filter(|k| !k.is_empty()) {
        let payload = serde_json::json!({ "finding_id": finding_id, "status": status });
        let inserted: Option<Uuid> = sqlx::query_scalar(
            "INSERT INTO integration.inbound_event \
             (project_id, actor_id, source, event_type, idempotency_key, payload, payload_hash, tenant_id) \
             VALUES ($1, $2, 'finding_decision', 'finding_decided', $3, $4, $5, $6) \
             ON CONFLICT (project_id, source, idempotency_key) DO NOTHING RETURNING id",
        )
        .bind(project_id)
        .bind(answered_by)
        .bind(key)
        .bind(&payload)
        .bind(crate::knowledge::content_hash(&payload.to_string()))
        .bind(tenant_id)
        .fetch_optional(&mut *tx)
        .await?;
        if inserted.is_none() {
            let finding = get_finding_tx(&mut tx, tenant_id, finding_id).await?;
            let (question, answer_row) = latest_qa_tx(&mut tx, tenant_id, finding_id).await?;
            tx.commit().await?;
            return Ok(FindingVerificationOutcome {
                finding,
                question,
                answer: answer_row,
                confirmed_finding: None,
            });
        }
    }

    // I2-025：调用人必须是真人（服务端二次校验，不信任入口）。
    let caller_kind: Option<String> =
        sqlx::query_scalar("SELECT kind FROM core.principal WHERE id = $1")
            .bind(answered_by)
            .fetch_optional(&mut *tx)
            .await?;
    if caller_kind.as_deref() != Some("human") {
        return Err(DiscoveryError::Validation(
            "only human principals can decide findings".to_string(),
        ));
    }

    // 行锁读 + Session 归属校验（I2-029）——**先校验后写**，不一致时零副作用。
    let row: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT session_id, verification_status FROM discovery.discovery_finding \
         WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(finding_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((finding_session_id, before)) = row else {
        return Err(DiscoveryError::FindingNotFound);
    };
    if finding_session_id != session_id {
        // I2-030：归属检查失败 → 不更新、不写成功决定审计。
        return Err(DiscoveryError::FindingNotFound);
    }

    // 条件更新（I2-020）：同值重复决定是 no-op。
    // REVISION_REQUESTED（要求补充）：状态不变，只落设计稿 §8.2 指定的审计。
    let (changed, audit_action) = if is_revision_request {
        (before.clone(), "discovery_finding_revision_requested")
    } else {
        (
            set_finding_verification_tx(&mut tx, tenant_id, finding_id, status).await?,
            "finding.decided",
        )
    };

    // question/answer（同事务）：优先回答既有 open 问题，否则按状态新建一对。
    let existing: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT id, status FROM discovery.discovery_question \
         WHERE tenant_id = $1 AND finding_id = $2 \
         ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .bind(tenant_id)
    .bind(finding_id)
    .fetch_optional(&mut *tx)
    .await?;
    let answer_view_text = answer.to_string();
    let (question, answer) = match existing {
        Some((question_id, question_status)) if question_status == QUESTION_STATUS_OPEN => {
            let answered = answer_question_tx(&mut tx, tenant_id, question_id, answer, answered_by).await?;
            let question = get_question_tx(&mut tx, tenant_id, question_id).await?;
            (question, Some(answered))
        }
        _ => {
            let question = add_question_tx(
                &mut tx,
                tenant_id,
                project_id,
                session_id,
                finding_id,
                status,
            )
            .await?;
            let answered = answer_question_tx(&mut tx, tenant_id, question.id, answer, answered_by).await?;
            (question, Some(answered))
        }
    };

    // 「已通过其他方式解决」→ 补一条 user_confirmed finding（同事务，HumanDirect）。
    let confirmed_finding = match confirmed_finding_content {
        Some(content) if !content.trim().is_empty() => Some(
            add_finding_in_tx(
                &mut tx,
                tenant_id,
                session_id,
                NewFinding {
                    kind: "user_confirmed".to_string(),
                    content: content.to_string(),
                    confidence: None,
                    evidence: serde_json::json!([]),
                    repository_ids: vec![],
                    actor_role: None,
                },
                FindingCreator::HumanDirect,
            )
            .await?,
        ),
        _ => None,
    };

    // 审计（同事务；失败即整体回滚）。action 按决定类型：
    // 确认/拒绝 → `finding.decided`；要求补充 → `discovery_finding_revision_requested`
    //（I2 V1.1 §8.2 指定名，读取侧按它推导「待补充」）。
    let answer_digest: String = answer_view_text.clone();
    crate::pm_write::append_audit_with_after(
        &mut *tx,
        project_id,
        &crate::actor::AuditActor::principal(answered_by),
        audit_action,
        "discovery_finding",
        Some(finding_id),
        serde_json::json!({
            "decision": status,
            "before": before,
            "after": changed,
            "answer": answer_digest,
        }),
    )
    .await?;

    let finding = get_finding_tx(&mut tx, tenant_id, finding_id).await?;
    tx.commit().await?;
    Ok(FindingVerificationOutcome {
        finding,
        question: Some(question),
        answer,
        confirmed_finding,
    })
}

/// 行锁读单个 finding（事务版，含证据聚合与仓库关联）。
async fn get_finding_tx(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    tenant_id: Uuid,
    finding_id: Uuid,
) -> Result<DiscoveryFindingView, DiscoveryError> {
    let row: Option<(Uuid, Uuid, String, String, Option<f64>, String, Uuid, Option<String>, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(&format!(
            "SELECT id, session_id, kind, content, confidence, verification_status, \
                    project_id, actor_role, created_at \
             FROM discovery.discovery_finding WHERE tenant_id = $1 AND id = $2"
        ))
        .bind(tenant_id)
        .bind(finding_id)
        .fetch_optional(&mut **tx)
        .await?;
    let Some(row) = row else {
        return Err(DiscoveryError::FindingNotFound);
    };
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT repository_id FROM discovery.discovery_finding_repository \
         WHERE tenant_id = $1 AND finding_id = $2 ORDER BY repository_id",
    )
    .bind(tenant_id)
    .bind(finding_id)
    .fetch_all(&mut **tx)
    .await?;
    let rows: Vec<(String, Option<String>, Option<String>, Option<Uuid>, Option<String>, Option<i32>)> =
        sqlx::query_as(
            "SELECT evidence_type, description, file_path, repository_id, commit_sha, attempt_no \
             FROM discovery.discovery_finding_evidence \
             WHERE tenant_id = $1 AND finding_id = $2 ORDER BY created_at, id",
        )
        .bind(tenant_id)
        .bind(finding_id)
        .fetch_all(&mut **tx)
        .await?;
    let evidence = serde_json::Value::Array(
        rows.into_iter()
            .map(
                |(evidence_type, description, file_path, repository_id, commit_sha, attempt_no)| {
                    serde_json::json!({
                        "evidence_type": evidence_type,
                        "description": description,
                        "file": file_path,
                        "file_path": file_path,
                        "repository": repository_id.map(|id| id.to_string()),
                        "repository_id": repository_id,
                        "commit_sha": commit_sha,
                        "attempt_no": attempt_no,
                    })
                },
            )
            .collect(),
    );
    // I2-033：事务内推导展示态（最近一次有效人工决定审计）。
    let decisions = latest_decision_actions_tx(&mut *tx, row.6, &[finding_id]).await?;
    let latest = decisions.get(&finding_id).map(String::as_str);
    Ok(DiscoveryFindingView {
        id: row.0,
        session_id: row.1,
        kind: row.2,
        content: row.3,
        confidence: row.4,
        verification_status: row.5.clone(),
        evidence,
        actor_role: row.7,
        repository_ids: ids,
        created_at: row.8,
        display_state: pm_domain_display(row.5.as_str(), latest),
    })
}

/// 事务版批量取最近人工决定审计（与 `finding::latest_decision_actions` 同口径）。
async fn latest_decision_actions_tx(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    finding_ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, String>, DiscoveryError> {
    let mut map = std::collections::HashMap::new();
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT DISTINCT ON (target_id) target_id, action \
         FROM governance.audit_log \
         WHERE project_id = $1 AND target_type = 'discovery_finding' \
           AND target_id = ANY($2) AND action = ANY($3) \
         ORDER BY target_id, created_at DESC, id DESC",
    )
    .bind(project_id)
    .bind(finding_ids)
    .bind(&DECISION_AUDIT_ACTIONS)
    .fetch_all(&mut **tx)
    .await?;
    for (finding_id, action) in rows {
        map.insert(finding_id, action);
    }
    Ok(map)
}

use crate::discovery::finding::derive_display_state as pm_domain_display;

/// 该 finding 最近一次 question（事务版）。
async fn latest_qa_tx(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    tenant_id: Uuid,
    finding_id: Uuid,
) -> Result<(Option<DiscoveryQuestionView>, Option<DiscoveryAnswerView>), DiscoveryError> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM discovery.discovery_question \
         WHERE tenant_id = $1 AND finding_id = $2 \
         ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .bind(tenant_id)
    .bind(finding_id)
    .fetch_optional(&mut **tx)
    .await?;
    match row {
        Some((question_id,)) => {
            let question = get_question_tx(tx, tenant_id, question_id).await?;
            Ok((Some(question), None))
        }
        None => Ok((None, None)),
    }
}

/// 新建决定质询（事务版）：问题正文取该轮决定的状态。
async fn add_question_tx(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    tenant_id: Uuid,
    project_id: Uuid,
    session_id: Uuid,
    finding_id: Uuid,
    question: &str,
) -> Result<DiscoveryQuestionView, DiscoveryError> {
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO discovery.discovery_question \
         (tenant_id, project_id, session_id, finding_id, question, why_blocking) \
         VALUES ($1, $2, $3, $4, $5, NULL) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(session_id)
    .bind(finding_id)
    .bind(question)
    .fetch_one(&mut **tx)
    .await?;
    get_question_tx(tx, tenant_id, id).await
}

/// 回答一个 open 问题（事务版，与 `question::answer_question` 同口径）。
async fn answer_question_tx(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    tenant_id: Uuid,
    question_id: Uuid,
    answer: &str,
    answered_by: Uuid,
) -> Result<DiscoveryAnswerView, DiscoveryError> {
    let status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM discovery.discovery_question \
         WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(question_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(status) = status else {
        return Err(DiscoveryError::QuestionNotFound);
    };
    if status != QUESTION_STATUS_OPEN {
        return Err(DiscoveryError::Validation(format!(
            "question is {status}, only open questions can be answered"
        )));
    }
    let row: (Uuid, Uuid, String, Uuid, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
        "INSERT INTO discovery.discovery_answer (tenant_id, question_id, answer, answered_by) \
         VALUES ($1, $2, $3, $4) RETURNING id, question_id, answer, answered_by, created_at",
    )
    .bind(tenant_id)
    .bind(question_id)
    .bind(answer)
    .bind(answered_by)
    .fetch_one(&mut **tx)
    .await?;
    sqlx::query(
        "UPDATE discovery.discovery_question SET status = 'answered' \
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(question_id)
    .execute(&mut **tx)
    .await?;
    Ok(DiscoveryAnswerView {
        id: row.0,
        question_id: row.1,
        answer: row.2,
        answered_by: row.3,
        created_at: row.4,
    })
}

/// 读单个 question（事务版，含回答列表）。
async fn get_question_tx(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    tenant_id: Uuid,
    question_id: Uuid,
) -> Result<DiscoveryQuestionView, DiscoveryError> {
    let row: Option<(Uuid, Uuid, Option<Uuid>, String, Option<String>, String, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            "SELECT id, session_id, finding_id, question, why_blocking, status, created_at \
             FROM discovery.discovery_question WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(question_id)
        .fetch_optional(&mut **tx)
        .await?;
    let Some(row) = row else {
        return Err(DiscoveryError::QuestionNotFound);
    };
    let answer_rows: Vec<(Uuid, Uuid, String, Uuid, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            "SELECT id, question_id, answer, answered_by, created_at \
             FROM discovery.discovery_answer WHERE tenant_id = $1 AND question_id = $2 \
             ORDER BY created_at, id",
        )
        .bind(tenant_id)
        .bind(question_id)
        .fetch_all(&mut **tx)
        .await?;
    let answers = answer_rows
        .into_iter()
        .map(|row| DiscoveryAnswerView {
            id: row.0,
            question_id: row.1,
            answer: row.2,
            answered_by: row.3,
            created_at: row.4,
        })
        .collect();
    Ok(DiscoveryQuestionView {
        id: row.0,
        session_id: row.1,
        finding_id: row.2,
        question: row.3,
        why_blocking: row.4,
        status: row.5,
        created_at: row.6,
        answers,
    })
}
