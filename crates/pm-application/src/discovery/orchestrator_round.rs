//! DK-B：访谈编排器「一轮」的**单点写入服务**（V-6）。
//!
//! 编排器（`pm-ai` 侧）只负责「问模型 + 校验模型输出」，落到库里的每一笔
//! 都经本函数一次事务完成：
//!
//! ```text
//! BEGIN
//!   1. 会话行 FOR UPDATE（与 REST 追加消息同一把锁 → C7 跨进程串行）
//!   2. 整轮幂等：最新消息 id 必须仍是本轮看到的那条，否则 StaleRound 中止
//!   3. findings 落库（按归一化正文去重；槽位标签写 actor_role=slot:<id>）
//!   4. questions 落库（按归一化正文对 open 问题去重）
//!   5. 消费用户这一轮回答的问题（仍 open 才作答，良性竞争静默跳过）
//!   6. 状态跃迁（只允许 AI 可达的三态；按既有边，必要时两跳）
//!   7. ai 消息追加（seq 在同一把行锁下计算）
//! COMMIT
//! ```
//!
//! 单事务 = 设计 §4.5「数据库写失败 → 整轮回滚，会话状态不变」；所有写入
//! 都复用既有 discovery 服务内核（`add_finding_in_tx` 等），不新造旁路。
//!
//! **AI 无确认权的服务层硬闸**：`next_status` 只接受
//! `IN_PROGRESS / NEEDS_USER_INPUT / REVIEW_REQUIRED`——`COMPLETED` 与
//! `PAUSED` 是人工动作，编排器给就会被 [`DiscoveryError::Validation`] 拒掉，
//! 模型说什么都不影响这一层。

use super::*;
use crate::discovery::finding::add_finding_in_tx;
use crate::discovery::message::append_message_in_tx;
use crate::discovery::session::session_transition_path;
use crate::discovery::slots::{is_valid_slot, slot_tag, slots_for_kind};
use std::collections::HashSet;

/// 编排器（AI 主体）可以落到的会话状态；`COMPLETED`/`PAUSED` 是人工专有。
pub const AI_ALLOWED_SESSION_TARGETS: [&str; 3] =
    ["IN_PROGRESS", "NEEDS_USER_INPUT", "REVIEW_REQUIRED"];

/// ai 消息正文的防御性上限（模型抽风时不能把一行撑爆）。
pub const MAX_ANSWER_CHARS: usize = 20_000;

/// 问题指向哪条 finding：本 session 既有行，或本轮 findings 数组的下标。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingRef {
    Existing(Uuid),
    /// 本轮第 i 条 finding（若该条因去重未落库，引用解析为 `None`）。
    Round(usize),
}

#[derive(Debug, Clone)]
pub struct RoundFinding {
    pub kind: String,
    pub content: String,
    pub confidence: Option<f64>,
    /// 槽位 id（必须在会话 kind 的槽位表内）；`None` = 不计入槽位覆盖。
    pub slot: Option<String>,
    /// 证据引文（已由调用方对照注入内容白名单过滤）。
    pub evidence_quotes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RoundQuestion {
    pub question: String,
    pub why_blocking: Option<String>,
    pub finding_ref: Option<FindingRef>,
    pub slot: String,
}

/// 一条成立的知识更正（DK-E §8.2 场景 2）。
///
/// 落库形态（F8↔F23 断链走 (b) 路径，见设计稿 §12.2-4）：
/// - 旧 finding **只读不写**（F1），另落一条 `contradicted` 指向它；
/// - 更正事实落一条 `user_confirmed`，创建通道仍是 AgentRelayed →
///   `verification_status=PENDING`（F7：人工裁决面不可绕）；
/// - 落一条 `[更正待知识应用]` 的 `deferred` 问题作结构化标记（不阻塞跃迁，
///   复核面必须可见）；KB 应用挂账 KP-3、PC 侧挂账 KP-2。
///
/// `target_document` 只是模型给出的定位提示（已对照注入白名单过滤），
/// 不是 `discovery_proposal` 的 FK 目标——本批零提案行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundCorrection {
    /// 被否定的既有 finding（调用方已定位到本 session）。
    pub target_finding_id: Uuid,
    /// 用户更正后的完整事实。
    pub corrected_fact: String,
    /// 用户更正原话（可选；作为 `user_statement` 证据）。
    pub user_quote: Option<String>,
    /// 更正指向的知识文档提示（可选；仅用于标记正文）。
    pub target_document: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RoundDraft {
    /// 本轮看到的最后一条消息 id（整轮幂等前置比较，§4.6）。
    pub expected_last_message_id: Uuid,
    pub answer_markdown: String,
    pub findings: Vec<RoundFinding>,
    pub questions: Vec<RoundQuestion>,
    /// 用户这一轮消息所回答的问题（仍 `open` 才作答）。
    pub consume_question: Option<Uuid>,
    /// 用户这一轮的原话（作为 answer 落库，归属 session 创建者）。
    pub answer_text: String,
    /// 本轮结束后的会话状态（AI 可达三态之一）。
    pub next_status: String,
    /// 本轮成立的更正（DK-E；`None` = 普通问答）。
    pub correction: Option<RoundCorrection>,
}

#[derive(Debug, Clone)]
pub struct RoundOutcome {
    pub message: DiscoveryMessageView,
    pub finding_ids: Vec<Uuid>,
    pub findings_added: usize,
    pub findings_deduped: usize,
    pub questions_added: usize,
    pub questions_deduped: usize,
    pub answers_consumed: usize,
    /// 本轮成立的更正数（0/1）与落下的 `[更正待知识应用]` 标记数（0/1）。
    pub corrections_applied: usize,
    pub correction_markers: usize,
    pub status: String,
    pub transitions: u8,
}

/// 问题/发现正文归一化：折叠空白 + ASCII 小写 + 去尾部标点。
/// 「同一问题不重复插」与「同一事实不重复插」都用它（§4.6）。
pub fn normalize_text(text: &str) -> String {
    let lowered = text.trim().to_lowercase();
    let collapsed = lowered.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed
        .trim_end_matches(|c: char| {
            matches!(
                c,
                '。' | '？' | '！' | '，' | '.' | '?' | '!' | ',' | ';' | '；'
            )
        })
        .to_string()
}

/// 应用一轮编排产出（见模块文档的七步）。
#[allow(clippy::too_many_arguments)]
pub async fn apply_orchestrator_round(
    pool: &PgPool,
    tenant_id: Uuid,
    session_id: Uuid,
    draft: RoundDraft,
) -> Result<RoundOutcome, DiscoveryError> {
    // 0) 入口校验（AI 无确认权硬闸 + 契约非空）。
    validate_one_of(
        &draft.next_status,
        &AI_ALLOWED_SESSION_TARGETS,
        "next_status",
    )?;
    if draft.answer_markdown.trim().is_empty() {
        return Err(DiscoveryError::Validation(
            "answer_markdown must not be blank".to_string(),
        ));
    }

    let mut tx = pool.begin().await?;

    // 1) 会话行锁（与 REST append_message 同一把锁）。
    let session: Option<(String, String, Uuid, Uuid)> = sqlx::query_as(
        "SELECT status, kind, project_id, created_by FROM discovery.discovery_session \
         WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
    )
    .bind(tenant_id)
    .bind(session_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((mut status, kind, project_id, created_by)) = session else {
        return Err(DiscoveryError::SessionNotFound);
    };
    if slots_for_kind(&kind).is_empty() {
        return Err(DiscoveryError::Validation(format!(
            "session kind {kind:?} has no slot table; orchestrator cannot run"
        )));
    }

    // 2) 整轮幂等：最新消息必须仍是本轮看到的那条（§4.6）。
    let latest: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM discovery.discovery_message \
         WHERE tenant_id = $1 AND session_id = $2 \
         ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .bind(tenant_id)
    .bind(session_id)
    .fetch_optional(&mut *tx)
    .await?;
    if latest != Some(draft.expected_last_message_id) {
        return Err(DiscoveryError::StaleRound {
            expected: draft.expected_last_message_id,
            actual: latest,
        });
    }

    // 3) findings。
    let mut finding_ids: Vec<Uuid> = Vec::with_capacity(draft.findings.len());
    let mut findings_added = 0usize;
    let mut findings_deduped = 0usize;
    for finding in &draft.findings {
        validate_one_of(&finding.kind, &FINDING_KINDS, "kind")?;
        if finding.content.trim().is_empty() {
            return Err(DiscoveryError::Validation(
                "finding content must not be blank".to_string(),
            ));
        }
        if let Some(slot) = finding.slot.as_deref() {
            if !is_valid_slot(&kind, slot) {
                return Err(DiscoveryError::Validation(format!(
                    "slot {slot:?} is not in the {kind} slot table"
                )));
            }
        }
        let duplicate: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM discovery.discovery_finding \
             WHERE tenant_id = $1 AND session_id = $2 \
               AND lower(btrim(content)) = lower(btrim($3)) LIMIT 1",
        )
        .bind(tenant_id)
        .bind(session_id)
        .bind(&finding.content)
        .fetch_optional(&mut *tx)
        .await?;
        if duplicate.is_some() {
            findings_deduped += 1;
            continue;
        }
        let evidence: Vec<serde_json::Value> = finding
            .evidence_quotes
            .iter()
            .map(|quote| {
                serde_json::json!({
                    "evidence_type": "system_observation",
                    "description": quote,
                })
            })
            .collect();
        let view = add_finding_in_tx(
            &mut tx,
            tenant_id,
            session_id,
            NewFinding {
                kind: finding.kind.clone(),
                content: finding.content.clone(),
                confidence: finding.confidence,
                evidence: serde_json::Value::Array(evidence),
                repository_ids: Vec::new(),
                actor_role: finding.slot.as_deref().map(slot_tag),
            },
            FindingCreator::AgentRelayed,
        )
        .await?;
        finding_ids.push(view.id);
        findings_added += 1;
    }

    // 3b) 更正分支（DK-E §8.2 场景 2；F8↔F23 断链走 (b) 路径，设计稿 §12.2-4）。
    //     旧行只读（F1）；更正事实的创建通道仍是 AgentRelayed → PENDING（F7）；
    //     知识应用本批不做，落一条 deferred 标记把待办显式留在复核面。
    let mut corrections_applied = 0usize;
    let mut correction_markers = 0usize;
    if let Some(correction) = &draft.correction {
        if correction.corrected_fact.trim().is_empty() {
            return Err(DiscoveryError::Validation(
                "corrected_fact must not be blank".to_string(),
            ));
        }
        let target: Option<(String,)> = sqlx::query_as(
            "SELECT content FROM discovery.discovery_finding \
             WHERE tenant_id = $1 AND session_id = $2 AND id = $3 FOR UPDATE",
        )
        .bind(tenant_id)
        .bind(session_id)
        .bind(correction.target_finding_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((old_content,)) = target else {
            return Err(DiscoveryError::FindingNotFound);
        };
        let old_short: String = old_content.chars().take(200).collect();
        let new_short: String = correction.corrected_fact.chars().take(200).collect();
        let quote = correction
            .user_quote
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let mut contradicted_evidence = vec![serde_json::json!({
            "evidence_type": "system_observation",
            "description": format!("contradicts:{}", correction.target_finding_id),
        })];
        if let Some(quote) = quote {
            contradicted_evidence.push(serde_json::json!({
                "evidence_type": "user_statement",
                "description": quote,
            }));
        }
        let contradicted = add_finding_in_tx(
            &mut tx,
            tenant_id,
            session_id,
            NewFinding {
                kind: "contradicted".to_string(),
                content: format!("用户更正：此前记录的事实「{old_short}」不再成立"),
                confidence: None,
                evidence: serde_json::Value::Array(contradicted_evidence),
                repository_ids: Vec::new(),
                actor_role: None,
            },
            FindingCreator::AgentRelayed,
        )
        .await?;

        // 新事实：正文与既有 finding 相同则不重复落（幂等），标记照落。
        let duplicate: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM discovery.discovery_finding \
             WHERE tenant_id = $1 AND session_id = $2 \
               AND lower(btrim(content)) = lower(btrim($3)) LIMIT 1",
        )
        .bind(tenant_id)
        .bind(session_id)
        .bind(correction.corrected_fact.trim())
        .fetch_optional(&mut *tx)
        .await?;
        if duplicate.is_some() {
            findings_deduped += 1;
        } else {
            let mut confirmed_evidence = Vec::new();
            if let Some(quote) = quote {
                confirmed_evidence.push(serde_json::json!({
                    "evidence_type": "user_statement",
                    "description": quote,
                }));
            }
            add_finding_in_tx(
                &mut tx,
                tenant_id,
                session_id,
                NewFinding {
                    kind: "user_confirmed".to_string(),
                    content: correction.corrected_fact.trim().to_string(),
                    confidence: None,
                    evidence: serde_json::Value::Array(confirmed_evidence),
                    repository_ids: Vec::new(),
                    actor_role: None,
                },
                FindingCreator::AgentRelayed,
            )
            .await?;
            findings_added += 1;
        }

        // 结构化标记：`deferred` 不阻塞跃迁（§4.3 补充规则），但必须出现在
        // 复核面；正文前缀是机器可判的待办锚（KP-3 / KP-2 承接）。
        let marker = format!(
            "[更正待知识应用] {target}：被否定事实「{old_short}」→ 更正后事实「{new_short}」；\
             本会话不写 KB/PC——KB 侧由 KP-3 评审链接收、PC 侧由 KP-2 接收，人工决定后再应用。",
            target = correction
                .target_document
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("项目知识（KB digest / PC 对象）"),
        );
        let marker_exists: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM discovery.discovery_question \
             WHERE tenant_id = $1 AND session_id = $2 \
               AND lower(btrim(question)) = lower(btrim($3)) LIMIT 1",
        )
        .bind(tenant_id)
        .bind(session_id)
        .bind(&marker)
        .fetch_optional(&mut *tx)
        .await?;
        if marker_exists.is_none() {
            sqlx::query(
                "INSERT INTO discovery.discovery_question \
                 (tenant_id, project_id, session_id, finding_id, question, why_blocking, status) \
                 VALUES ($1, $2, $3, $4, $5, NULL, 'deferred')",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(session_id)
            .bind(contradicted.id)
            .bind(&marker)
            .execute(&mut *tx)
            .await?;
            correction_markers += 1;
        }
        corrections_applied = 1;
    }

    // 4) questions（对既有 open 与本轮新增都去重）。
    let existing_open: Vec<String> = sqlx::query_scalar(
        "SELECT question FROM discovery.discovery_question \
         WHERE tenant_id = $1 AND session_id = $2 AND status = 'open'",
    )
    .bind(tenant_id)
    .bind(session_id)
    .fetch_all(&mut *tx)
    .await?;
    let mut seen: HashSet<String> = existing_open.iter().map(|q| normalize_text(q)).collect();
    let mut questions_added = 0usize;
    let mut questions_deduped = 0usize;
    for question in &draft.questions {
        if question.question.trim().is_empty() {
            return Err(DiscoveryError::Validation(
                "question must not be blank".to_string(),
            ));
        }
        if !is_valid_slot(&kind, &question.slot) {
            return Err(DiscoveryError::Validation(format!(
                "slot {:?} is not in the {kind} slot table",
                question.slot
            )));
        }
        let normalized = normalize_text(&question.question);
        if !seen.insert(normalized) {
            questions_deduped += 1;
            continue;
        }
        let finding_id = match question.finding_ref {
            Some(FindingRef::Existing(id)) => {
                let ok: Option<Uuid> = sqlx::query_scalar(
                    "SELECT id FROM discovery.discovery_finding \
                     WHERE tenant_id = $1 AND session_id = $2 AND id = $3",
                )
                .bind(tenant_id)
                .bind(session_id)
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
                if ok.is_none() {
                    return Err(DiscoveryError::FindingNotFound);
                }
                Some(id)
            }
            Some(FindingRef::Round(index)) => finding_ids.get(index).copied(),
            None => None,
        };
        let why_blocking = question
            .why_blocking
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        sqlx::query(
            "INSERT INTO discovery.discovery_question \
             (tenant_id, project_id, session_id, finding_id, question, why_blocking) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(session_id)
        .bind(finding_id)
        .bind(&question.question)
        .bind(why_blocking)
        .execute(&mut *tx)
        .await?;
        questions_added += 1;
    }

    // 5) 消费用户这一轮回答的问题（已被 REST 答过 → 静默跳过，良性竞争）。
    let mut answers_consumed = 0usize;
    if let Some(question_id) = draft.consume_question {
        let question_status: Option<String> = sqlx::query_scalar(
            "SELECT status FROM discovery.discovery_question \
             WHERE tenant_id = $1 AND session_id = $2 AND id = $3 FOR UPDATE",
        )
        .bind(tenant_id)
        .bind(session_id)
        .bind(question_id)
        .fetch_optional(&mut *tx)
        .await?;
        if question_status.as_deref() == Some(QUESTION_STATUS_OPEN) {
            let answer = draft.answer_text.trim();
            if !answer.is_empty() {
                sqlx::query(
                    "INSERT INTO discovery.discovery_answer \
                     (tenant_id, question_id, answer, answered_by) VALUES ($1, $2, $3, $4)",
                )
                .bind(tenant_id)
                .bind(question_id)
                .bind(answer)
                .bind(created_by)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "UPDATE discovery.discovery_question SET status = 'answered' \
                     WHERE tenant_id = $1 AND id = $2",
                )
                .bind(tenant_id)
                .bind(question_id)
                .execute(&mut *tx)
                .await?;
                answers_consumed = 1;
            }
        }
    }

    // 6) 状态跃迁（既有边；必要时两跳，见 `SESSION_EDGES` 勘误）。
    let mut transitions = 0u8;
    let path = session_transition_path(&status, &draft.next_status).ok_or_else(|| {
        DiscoveryError::InvalidTransition {
            from: status.clone(),
            to: draft.next_status.clone(),
        }
    })?;
    for step in &path {
        sqlx::query(
            "UPDATE discovery.discovery_session SET status = $3, updated_at = now() \
             WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(session_id)
        .bind(step)
        .execute(&mut *tx)
        .await?;
        status = step.clone();
        transitions += 1;
    }

    // 7) ai 消息（seq 在同一把行锁下计算）。
    let answer: String = draft
        .answer_markdown
        .chars()
        .take(MAX_ANSWER_CHARS)
        .collect();
    let message =
        append_message_in_tx(&mut tx, tenant_id, session_id, "ai", "SYSTEM", &answer).await?;

    tx.commit().await?;

    Ok(RoundOutcome {
        message,
        finding_ids,
        findings_added,
        findings_deduped,
        questions_added,
        questions_deduped,
        answers_consumed,
        corrections_applied,
        correction_markers,
        status,
        transitions,
    })
}
