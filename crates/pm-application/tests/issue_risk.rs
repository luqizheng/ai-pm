//! Issue / Risk / Blocker 领域验收（PA-05-01 收尾；0052）。
//!
//! 覆盖：创建即注册统一事项+created 活动 · resolve/cancel 终态（语义映射
//! open→done / cancelled）· 风险线性状态机（禁止跳跃回退）· blocker 清除
//! （open→blocked→done 语义）· 校验（空标题/非法 severity/cancel 缺理由/
//! blocked_task 跨项目拒绝）。

use pm_application::actor::AuditActor;
use pm_application::issue_risk::{
    clear_blocker, create_blocker, create_issue, create_risk, list_blockers, list_issues,
    transition_issue, transition_risk, WorkDomainError,
};
use sqlx::PgPool;
use uuid::Uuid;

async fn seed_world(pool: &PgPool) -> (Uuid, Uuid, Uuid) {
    let tenant_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO core.tenant (id, provider, external_key, display_name) \
         VALUES ($1, 'test', $2, 'IRB Tenant')",
    )
    .bind(tenant_id)
    .bind(tenant_id.to_string())
    .execute(pool)
    .await
    .unwrap();
    let customer_id = Uuid::new_v4();
    sqlx::query("INSERT INTO core.customer (id, tenant_id, name) VALUES ($1, $2, 'Acme')")
        .bind(customer_id)
        .bind(tenant_id)
        .execute(pool)
        .await
        .unwrap();
    let project_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO core.project (id, customer_id, code, name, tenant_id) \
         VALUES ($1, $2, 'IRB', 'Issue Risk Blocker', $3)",
    )
    .bind(project_id)
    .bind(customer_id)
    .bind(tenant_id)
    .execute(pool)
    .await
    .unwrap();
    let principal_id = Uuid::new_v4();
    sqlx::query("INSERT INTO core.principal (id, kind, display_name) VALUES ($1, 'human', 'Maya')")
        .bind(principal_id)
        .execute(pool)
        .await
        .unwrap();
    (tenant_id, project_id, principal_id)
}

fn actor(principal_id: Uuid) -> AuditActor {
    AuditActor {
        principal_id,
        identity_id: None,
        client: None,
        auth_mode: None,
        claimed_device_id: None,
    }
}

async fn work_item_status(pool: &PgPool, target_id: Uuid) -> Option<String> {
    sqlx::query_scalar("SELECT status FROM core.work_item WHERE target_id = $1")
        .bind(target_id)
        .fetch_optional(pool)
        .await
        .unwrap()
}

async fn activity_count(pool: &PgPool, target_id: Uuid, activity_type: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM core.work_item_activity a \
          JOIN core.work_item w ON w.id = a.item_id \
          WHERE w.target_id = $1 AND a.activity_type = $2",
    )
    .bind(target_id)
    .bind(activity_type)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn issue_lifecycle_registers_and_transitions(pool: PgPool) {
    let (tenant_id, project_id, principal_id) = seed_world(&pool).await;
    let actor = actor(principal_id);
    let issue_id = create_issue(
        &pool,
        tenant_id,
        project_id,
        "Sign-in broken on mobile",
        Some("OAuth redirect fails"),
        Some("high"),
        &actor,
    )
    .await
    .expect("create issue");
    assert_eq!(work_item_status(&pool, issue_id).await.as_deref(), Some("open"));
    assert_eq!(activity_count(&pool, issue_id, "created").await, 1);

    // 非法 severity / 空标题 / cancel 缺理由拒绝。
    assert!(matches!(
        create_issue(&pool, tenant_id, project_id, "x", None, Some("urgent"), &actor).await,
        Err(WorkDomainError::Validation(_))
    ));
    assert!(matches!(
        create_issue(&pool, tenant_id, project_id, "  ", None, None, &actor).await,
        Err(WorkDomainError::Validation(_))
    ));
    assert!(matches!(
        transition_issue(&pool, tenant_id, issue_id, "cancelled", None, &actor).await,
        Err(WorkDomainError::Validation(_))
    ));

    // resolve → done 语义同步 + status_changed 活动。
    transition_issue(&pool, tenant_id, issue_id, "resolved", None, &actor)
        .await
        .expect("resolve");
    assert_eq!(
        work_item_status(&pool, issue_id).await.as_deref(),
        Some("done")
    );
    assert_eq!(activity_count(&pool, issue_id, "status_changed").await, 1);
    // 终态后再迁移拒绝。
    assert!(matches!(
        transition_issue(&pool, tenant_id, issue_id, "cancelled", Some("late"), &actor).await,
        Err(WorkDomainError::Validation(_))
    ));

    // cancel 必填理由路径（新 issue）。
    let issue2 = create_issue(&pool, tenant_id, project_id, "Duplicate report", None, None, &actor)
        .await
        .expect("create issue 2");
    transition_issue(&pool, tenant_id, issue2, "cancelled", Some("duplicate"), &actor)
        .await
        .expect("cancel");
    assert_eq!(
        work_item_status(&pool, issue2).await.as_deref(),
        Some("cancelled")
    );
    let rows = list_issues(&pool, tenant_id, project_id, Some("cancelled"))
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn risk_linear_state_machine(pool: PgPool) {
    let (tenant_id, project_id, principal_id) = seed_world(&pool).await;
    let actor = actor(principal_id);
    let risk_id = create_risk(
        &pool,
        tenant_id,
        project_id,
        "Vendor API deprecation",
        None,
        Some("high"),
        Some("low"),
        &actor,
    )
    .await
    .expect("create risk");
    assert_eq!(
        work_item_status(&pool, risk_id).await.as_deref(),
        Some("open")
    );

    // identified → closed 允许（V1 线性闭集含直关）；mitigating → 回退拒绝。
    transition_risk(&pool, tenant_id, risk_id, "mitigating", &actor)
        .await
        .expect("mitigate");
    assert_eq!(
        work_item_status(&pool, risk_id).await.as_deref(),
        Some("in_progress")
    );
    assert!(matches!(
        transition_risk(&pool, tenant_id, risk_id, "mitigating", &actor).await,
        Err(WorkDomainError::Validation(_))
    ));
    transition_risk(&pool, tenant_id, risk_id, "closed", &actor)
        .await
        .expect("close");
    assert_eq!(
        work_item_status(&pool, risk_id).await.as_deref(),
        Some("done")
    );
    // closed 终态。
    assert!(matches!(
        transition_risk(&pool, tenant_id, risk_id, "mitigating", &actor).await,
        Err(WorkDomainError::Validation(_))
    ));
    assert_eq!(activity_count(&pool, risk_id, "status_changed").await, 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn blocker_references_task_and_clears(pool: PgPool) {
    let (tenant_id, project_id, principal_id) = seed_world(&pool).await;
    let actor = actor(principal_id);
    let task_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO delivery.task (id, project_id, title, status, revision, tenant_id) \
         VALUES ($1, $2, 'Blocked task', 'blocked', 1, $3)",
    )
    .bind(task_id)
    .bind(project_id)
    .bind(tenant_id)
    .execute(&pool)
    .await
    .unwrap();

    // blocked_task 跨项目拒绝。
    let other_project = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO core.project (id, customer_id, code, name, tenant_id) \
         VALUES ($1, (SELECT customer_id FROM core.project WHERE id = $2), 'OTH', 'Other', \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(other_project)
    .bind(project_id)
    .execute(&pool)
    .await
    .unwrap();
    assert!(matches!(
        create_blocker(
            &pool,
            tenant_id,
            other_project,
            "Cross",
            None,
            Some(task_id),
            &actor
        )
        .await,
        Err(WorkDomainError::Validation(_))
    ));

    let blocker_id = create_blocker(
        &pool,
        tenant_id,
        project_id,
        "Waiting on vendor credentials",
        None,
        Some(task_id),
        &actor,
    )
    .await
    .expect("create blocker");
    assert_eq!(
        work_item_status(&pool, blocker_id).await.as_deref(),
        Some("blocked")
    );

    clear_blocker(&pool, tenant_id, blocker_id, &actor)
        .await
        .expect("clear");
    assert_eq!(
        work_item_status(&pool, blocker_id).await.as_deref(),
        Some("done")
    );
    // 重复清除拒绝。
    assert!(matches!(
        clear_blocker(&pool, tenant_id, blocker_id, &actor).await,
        Err(WorkDomainError::Validation(_))
    ));
    let rows = list_blockers(&pool, tenant_id, project_id, Some("cleared"))
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
}

/// PA06-IMP-001 A2（2026-11-21）：REST 人类面显式执法——agent 主体
/// （dev_agent/ai_pm）创建/迁移执行性事项一律 Forbidden；PA-06-02 授权
/// Agent 的自主管理通道归 MCP 工具面（批次 C2），REST 面不共用。
#[sqlx::test(migrations = "../../migrations")]
async fn agent_principal_is_rejected_on_human_rest_face(pool: PgPool) {
    let (tenant_id, project_id, human_id) = seed_world(&pool).await;
    let agent_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name) \
         VALUES ($1, 'dev_agent', 'Dev Agent')",
    )
    .bind(agent_id)
    .execute(&pool)
    .await
    .unwrap();
    let agent = actor(agent_id);

    assert!(matches!(
        create_issue(&pool, tenant_id, project_id, "agent issue", None, None, &agent).await,
        Err(WorkDomainError::Forbidden(_))
    ));
    assert!(matches!(
        create_risk(&pool, tenant_id, project_id, "agent risk", None, None, None, &agent).await,
        Err(WorkDomainError::Forbidden(_))
    ));
    assert!(matches!(
        create_blocker(&pool, tenant_id, project_id, "agent blocker", None, None, &agent).await,
        Err(WorkDomainError::Forbidden(_))
    ));

    // 人类创建后，agent 也不得迁移（resolve/clear 同为人类面动作）。
    let actor = actor(human_id);
    let issue_id = create_issue(&pool, tenant_id, project_id, "human issue", None, None, &actor)
        .await
        .unwrap();
    assert!(matches!(
        transition_issue(&pool, tenant_id, issue_id, "resolved", Some("done"), &agent).await,
        Err(WorkDomainError::Forbidden(_))
    ));
}
