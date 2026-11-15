//! 人工行动面：待办计数与行动摘要（§4.4）。

use super::*;

// ---------------------------------------------------------------------------
// §4.4 实现
// ---------------------------------------------------------------------------

pub(super) async fn pending_action_total(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<i64, sqlx::Error> {
    let findings: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM discovery.discovery_finding \
         WHERE project_id = $1 AND tenant_id = $2 AND verification_status = 'PENDING'",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;
    let proposals: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.document_proposal \
         WHERE project_id = $1 AND tenant_id = $2 AND status = ANY($3)",
    )
    .bind(project_id)
    .bind(tenant_id)
    .bind(ACTIVE_PROPOSAL_STATUSES)
    .fetch_one(pool)
    .await?;
    let deliveries: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task \
         WHERE project_id = $1 AND tenant_id = $2 AND status IN ('submitted','reviewing')",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;
    // D-5：未关闭的分析报告（analyzed 待审理）也是人工决策项（报告粒度，
    // 点击直达知识审阅对应报告；条目级决策仍在知识审阅，单一决策源）。
    let reports: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.repo_analysis_report \
         WHERE project_id = $1 AND tenant_id = $2 AND status = 'analyzed'",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;
    Ok(findings + proposals + deliveries + reports)
}

pub(super) async fn load_human_action_summary(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<HumanActionSummary, sqlx::Error> {
    let mut items: Vec<ActionItem> = Vec::new();

    // 待确认 / 待补充 Finding（I2 权威推导：display_state）。
    // link 同时携带 session_id（R06）：决策端点
    // POST .../sessions/{session_id}/findings/{finding_id}/verification 是
    // 两段主键，前端不得反查或猜测 session_id。
    let finding_rows: Vec<(Uuid, Uuid, String, String, String, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, session_id, kind, content, verification_status, created_at \
         FROM discovery.discovery_finding \
         WHERE project_id = $1 AND tenant_id = $2 AND verification_status = 'PENDING' \
         ORDER BY created_at DESC, id DESC LIMIT 50",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    let finding_ids: Vec<Uuid> = finding_rows.iter().map(|r| r.0).collect();
    let decisions =
        crate::discovery::latest_decision_actions(pool, project_id, &finding_ids).await?;
    for (id, session_id, kind, content, status, created_at) in finding_rows {
        let display_state =
            crate::discovery::derive_display_state(&status, decisions.get(&id).map(|s| s.as_str()));
        let (action_kind, hint) = if display_state == "REVISION_REQUESTED" {
            ("finding_revision_requested", "补充信息后重新提交")
        } else {
            ("finding_pending", "确认或拒绝该发现")
        };
        items.push(ActionItem {
            kind: action_kind.to_string(),
            domain: "discovery".to_string(),
            id,
            project_id,
            title: format!("发现待处理（{kind}）"),
            detail: Some(truncate(&content, 160)),
            source_state: display_state,
            updated_at: Some(created_at),
            action_hint: hint.to_string(),
            link: ActionLink {
                kind: "discovery_finding".to_string(),
                params: BTreeMap::from([
                    ("finding_id".to_string(), id.to_string()),
                    ("session_id".to_string(), session_id.to_string()),
                ]),
            },
        });
    }

    // 待审批知识提案
    type Row = (Uuid, String, Option<String>, String, DateTime<Utc>);
    let proposal_rows: Vec<Row> = sqlx::query_as(
        "SELECT id, doc_key, change_summary, status, created_at \
         FROM ai.document_proposal \
         WHERE project_id = $1 AND tenant_id = $2 AND status = ANY($3) \
         ORDER BY created_at DESC, id DESC LIMIT 50",
    )
    .bind(project_id)
    .bind(tenant_id)
    .bind(ACTIVE_PROPOSAL_STATUSES)
    .fetch_all(pool)
    .await?;
    for (id, doc_key, change_summary, status, created_at) in proposal_rows {
        items.push(ActionItem {
            kind: "proposal_pending".to_string(),
            domain: "knowledge".to_string(),
            id,
            project_id,
            title: format!("文档提案待确认（{doc_key}）"),
            detail: change_summary.map(|s| truncate(&s, 160)),
            source_state: status,
            updated_at: Some(created_at),
            action_hint: "确认或否决该提案".to_string(),
            link: ActionLink {
                kind: "document_proposal".to_string(),
                params: BTreeMap::from([("proposal_id".to_string(), id.to_string())]),
            },
        });
    }

    // 待验收 Delivery
    let delivery_rows: Vec<(Uuid, String, String, Option<DateTime<Utc>>)> = sqlx::query_as(
        "SELECT id, title, status, updated_at FROM delivery.task \
         WHERE project_id = $1 AND tenant_id = $2 AND status IN ('submitted','reviewing') \
         ORDER BY updated_at DESC NULLS LAST, id DESC LIMIT 50",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    for (id, title, status, updated_at) in delivery_rows {
        items.push(ActionItem {
            kind: "delivery_pending".to_string(),
            domain: "delivery".to_string(),
            id,
            project_id,
            title: format!("交付待验收（{title}）"),
            detail: None,
            source_state: status,
            updated_at,
            action_hint: "验收交付证据".to_string(),
            link: ActionLink {
                kind: "task_delivery".to_string(),
                params: BTreeMap::from([("task_id".to_string(), id.to_string())]),
            },
        });
    }

    // D-5：未关闭的分析报告（粒度=报告；逐条审阅在知识审阅页处理）。
    let report_rows: Vec<(Uuid, String, String, Option<String>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT r.id, p.name, r.commit_sha, r.stage_detail, r.updated_at \
           FROM integration.repo_analysis_report r \
           JOIN integration.project_repository p ON p.id = r.repository_id \
          WHERE r.project_id = $1 AND r.tenant_id = $2 AND r.status = 'analyzed' \
          ORDER BY r.updated_at DESC, r.id DESC LIMIT 50",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    for (id, repository_name, commit_sha, stage_detail, updated_at) in report_rows {
        let short: String = commit_sha.chars().take(12).collect();
        items.push(ActionItem {
            kind: "report_pending".to_string(),
            domain: "knowledge".to_string(),
            id,
            project_id,
            title: format!("分析报告待审阅（{repository_name} @ {short}）"),
            detail: stage_detail.map(|s| truncate(&s, 160)),
            source_state: "analyzed".to_string(),
            updated_at: Some(updated_at),
            action_hint: "审阅报告内容并逐条处置后关闭".to_string(),
            link: ActionLink {
                kind: "knowledge_report".to_string(),
                params: BTreeMap::from([
                    ("report_id".to_string(), id.to_string()),
                    ("project_id".to_string(), project_id.to_string()),
                ]),
            },
        });
    }

    // 排序：kind 优先级 → 更新时间 → id（唯一 tiebreaker，保证稳定）。
    items.sort_by(|a, b| {
        action_kind_rank(&a.kind)
            .cmp(&action_kind_rank(&b.kind))
            .then_with(|| b.updated_at.cmp(&a.updated_at))
            .then_with(|| a.id.cmp(&b.id))
    });
    let total = items.len() as i64;
    Ok(HumanActionSummary { total, items })
}

fn action_kind_rank(kind: &str) -> u8 {
    match kind {
        "finding_pending" => 0,
        "finding_revision_requested" => 1,
        "report_pending" => 2,
        "proposal_pending" => 3,
        "delivery_pending" => 4,
        _ => 9,
    }
}
