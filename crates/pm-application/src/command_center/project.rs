//! 项目概要与初始化进度两块（§4.1/§4.2）。

use super::*;

// ---------------------------------------------------------------------------
// §4.1 实现
// ---------------------------------------------------------------------------

pub(super) async fn load_project_summary(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<ProjectSummary, sqlx::Error> {
    let (code, name, lifecycle, customer_name): (String, String, String, Option<String>) =
        sqlx::query_as(
            "SELECT p.code, p.name, p.status, c.name \
             FROM core.project p LEFT JOIN core.customer c ON c.id = p.customer_id \
             WHERE p.id = $1 AND p.tenant_id = $2",
        )
        .bind(project_id)
        .bind(tenant_id)
        .fetch_one(pool)
        .await?;

    let goal_row: Option<(i32, Option<String>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT r.revision_no, r.change_summary, r.created_at \
         FROM ai.project_document d \
         JOIN LATERAL ( \
              SELECT revision_no, change_summary, created_at \
                FROM ai.document_revision \
               WHERE document_id = d.id AND status = 'published' \
               ORDER BY revision_no DESC LIMIT 1 \
         ) r ON true \
         WHERE d.project_id = $1 AND d.tenant_id = $2 AND d.doc_key = 'project-goal'",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await?;

    let goal_pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.document_proposal \
         WHERE project_id = $1 AND tenant_id = $2 AND doc_key = 'project-goal' \
           AND status = ANY($3)",
    )
    .bind(project_id)
    .bind(tenant_id)
    .bind(ACTIVE_PROPOSAL_STATUSES)
    .fetch_one(pool)
    .await?;

    let (repository_count, verified_repository_count): (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE status = 'verified') \
         FROM integration.project_repository \
         WHERE project_id = $1 AND tenant_id = $2",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;

    let member_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.project_member WHERE project_id = $1 AND tenant_id = $2",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;

    let last_activity_at: Option<DateTime<Utc>> = sqlx::query_scalar(
        "SELECT max(created_at) FROM governance.audit_log \
         WHERE tenant_id = $1 AND project_id = $2 AND action = ANY($3)",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(activity_audit_actions())
    .fetch_one(pool)
    .await?;

    Ok(ProjectSummary {
        project_id,
        code,
        name,
        closed: lifecycle == "closed",
        lifecycle,
        customer_name,
        goal: GoalSummary {
            present: goal_row.is_some(),
            revision_no: goal_row.as_ref().map(|r| r.0),
            published_at: goal_row.as_ref().map(|r| r.2),
            change_summary: goal_row.as_ref().and_then(|r| r.1.clone()),
            pending_proposal: goal_pending > 0,
        },
        repository_count,
        verified_repository_count,
        member_count,
        last_activity_at,
    })
}

// ---------------------------------------------------------------------------
// §4.2 实现
// ---------------------------------------------------------------------------

pub(super) async fn load_initialization_progress(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<InitializationProgress, sqlx::Error> {
    // 仓库接入
    let repo_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status, count(*) FROM integration.project_repository \
         WHERE project_id = $1 AND tenant_id = $2 GROUP BY status",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    let repo_total: i64 = repo_rows.iter().map(|(_, n)| n).sum();
    let repo_verified: i64 = repo_rows
        .iter()
        .find(|(s, _)| s == "verified")
        .map(|(_, n)| *n)
        .unwrap_or(0);
    let repo_unreachable: i64 = repo_rows
        .iter()
        .find(|(s, _)| s == "unreachable")
        .map(|(_, n)| *n)
        .unwrap_or(0);
    let repo_pending: i64 = repo_rows
        .iter()
        .find(|(s, _)| s == "pending")
        .map(|(_, n)| *n)
        .unwrap_or(0);

    // 代码分析（导入作业）
    let import_rows: Vec<(String, Option<String>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT status, error_code, created_at FROM integration.repo_import \
         WHERE project_id = $1 AND tenant_id = $2 ORDER BY created_at DESC, id DESC LIMIT 20",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    let latest_import = import_rows.first();

    // 项目认知（Discovery 会话）
    let session_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status, count(*) FROM discovery.discovery_session \
         WHERE project_id = $1 AND tenant_id = $2 GROUP BY status",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    let session_total: i64 = session_rows.iter().map(|(_, n)| n).sum();
    // 开/待人审会话的口径收敛在 init_judgement::cognition_status（G-01 七态）。

    // 开发环境（绑定设备的就绪快照）
    let device_rows: Vec<(Option<String>, i64)> = sqlx::query_as(
        "SELECT d.readiness, count(*) \
         FROM devenv.device_project_binding b \
         JOIN devenv.device d ON d.id = b.device_id \
         WHERE b.project_id = $1 AND b.tenant_id = $2 AND b.unbound_at IS NULL \
         GROUP BY d.readiness",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    let device_total: i64 = device_rows.iter().map(|(_, n)| n).sum();
    let device_ready: i64 = device_rows
        .iter()
        .find(|(r, _)| r.as_deref() == Some("ready"))
        .map(|(_, n)| *n)
        .unwrap_or(0);
    let device_degraded: i64 = device_rows
        .iter()
        .filter(|(r, _)| matches!(r.as_deref(), Some("degraded") | Some("failed")))
        .map(|(_, n)| n)
        .sum();
    let device_unchecked: i64 = device_rows
        .iter()
        .find(|(r, _)| r.is_none())
        .map(|(_, n)| *n)
        .unwrap_or(0);

    // 人工处理待办数（用于「人工处理」事项与展开判定）
    let action_total = pending_action_total(pool, tenant_id, project_id).await?;

    // Goal
    let goal_present: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.project_document d \
         WHERE d.project_id = $1 AND d.tenant_id = $2 AND d.doc_key = 'project-goal' \
           AND EXISTS (SELECT 1 FROM ai.document_revision r \
                        WHERE r.document_id = d.id AND r.status = 'published')",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;
    // G-01 七态：goal 的活跃提案按 status 拆分——只有 `redline_pending`
    // 是「待人工红线确认」（系统已交出球）；draft / commit_pending 等由系统推进。
    // （`quality_gates_passed` / `confirmed` 现无写入路径，不计。）
    let goal_proposal_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status, count(*) FROM ai.document_proposal \
         WHERE project_id = $1 AND tenant_id = $2 AND doc_key = 'project-goal' \
           AND status = ANY($3) GROUP BY status",
    )
    .bind(project_id)
    .bind(tenant_id)
    .bind(ACTIVE_PROPOSAL_STATUSES)
    .fetch_all(pool)
    .await?;
    let goal_human_pending: i64 = goal_proposal_rows
        .iter()
        .filter(|(status, _)| status == "redline_pending")
        .map(|(_, n)| n)
        .sum();
    let goal_pending: i64 = goal_proposal_rows.iter().map(|(_, n)| n).sum();

    // 认知：待人工确认的发现数——finding 的核实决定者只能是人类
    // （set_finding_verification 强制可信人类身份），故 PENDING 即人类待办。
    let finding_pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM discovery.discovery_finding \
         WHERE project_id = $1 AND tenant_id = $2 AND verification_status = 'PENDING'",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;

    let mut items: Vec<InitItem> = Vec::new();
    items.push(InitItem {
        key: "project_basics".to_string(),
        title: "项目基本信息".to_string(),
        status: init_status::COMPLETED.to_string(),
        required: true,
        source_state: Some("planning".to_string()),
        detail: None,
    });
    items.push(InitItem {
        key: "project_goal".to_string(),
        title: "项目目标".to_string(),
        status: init_judgement::goal_status(
            goal_present > 0,
            goal_human_pending > 0,
            goal_pending > 0,
        )
        .to_string(),
        required: true,
        source_state: None,
        detail: None,
    });
    items.push(InitItem {
        key: "repository_access".to_string(),
        title: "仓库接入".to_string(),
        status: init_judgement::repository_access_status(
            repo_total,
            repo_unreachable,
            repo_pending,
        )
        .to_string(),
        required: false,
        source_state: None,
        detail: Some(format!(
            "已关联 {repo_total} 个仓库，其中 {repo_verified} 个已验证"
        )),
    });
    items.push(InitItem {
        key: "code_analysis".to_string(),
        title: "代码分析".to_string(),
        status: init_judgement::code_analysis_status(
            repo_total,
            latest_import.map(|(s, _, _)| s.as_str()),
        )
        .to_string(),
        required: false,
        source_state: latest_import.map(|(s, _, _)| s.clone()),
        detail: latest_import.and_then(|(_, code, _)| code.clone()),
    });
    items.push(InitItem {
        key: "project_cognition".to_string(),
        title: "项目认知".to_string(),
        status: init_judgement::cognition_status(&session_rows, finding_pending).to_string(),
        required: false,
        source_state: None,
        detail: Some(format!("共 {session_total} 个发现会话")),
    });
    items.push(InitItem {
        key: "dev_environment".to_string(),
        title: "开发环境".to_string(),
        status: init_judgement::environment_status(
            device_total,
            device_ready,
            device_degraded,
            device_unchecked,
        )
        .to_string(),
        required: false,
        source_state: None,
        detail: Some(format!("已绑定 {device_total} 台设备")),
    });
    items.push(InitItem {
        key: "human_actions".to_string(),
        title: "人工处理".to_string(),
        status: init_judgement::human_actions_status(action_total).to_string(),
        required: true,
        source_state: None,
        detail: Some(format!("{action_total} 项待人工处理")),
    });

    let required_items = items
        .iter()
        .filter(|item| item.required)
        .map(|item| item.key.clone())
        .collect::<Vec<_>>();
    // 收缩规则（设计稿 §4.6）：进行中/未开始的必需事项或任一阻塞 → 展开。
    // G-01 七态：waiting_for_human 走**自有展开分支**（无条件展开）——
    // 球在人手里，收缩不得隐藏人工待办（I5 §4.6「收缩不能隐藏新的阻塞或
    // 人工处理事项」）；刻意不并入 in_progress 的 required 限定分支，
    // 两态展示语义不同（系统在工作 vs 等人行动）。
    let expanded = items.iter().any(|item| {
        item.status == init_status::BLOCKED
            || item.status == init_status::WAITING_FOR_HUMAN
            || (item.required
                && matches!(
                    item.status.as_str(),
                    init_status::IN_PROGRESS | init_status::NOT_STARTED
                ))
    });

    Ok(InitializationProgress {
        items,
        required_items,
        // 当前没有项目类型/初始化要求配置 → 不得宣称「初始化已完成」。
        completion: "not_determinable".to_string(),
        expanded,
    })
}
