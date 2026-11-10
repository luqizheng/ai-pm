//! S04 · 任务域读面集成测试（单任务详情 / 项目依赖图 / 执行准备度 / 排期）。
//!
//! 覆盖点：
//! - 单任务详情：本体 + 需求追溯 + 依赖方向（上游/下游）+ 验收标准 + 审计；
//! - 项目依赖图：节点/边、分层、关键路径、并行组、环检测；
//! - 执行准备度：A–F 六段检查的逐项判定与三态汇总（硬阻断 → blocked）；
//! - 排期写：起止与工时落库 + 审计留痕 + 区间校验。

use pm_application::actor::AuditActor;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::task_read::{self, TaskScheduleRequest};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    project_id: Uuid,
    human: AuditActor,
    human_id: Uuid,
}

async fn fixture(pool: &PgPool, code: &str) -> Fx {
    let boot = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: format!("s04-{code}"),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    Fx {
        project_id: boot.project_id,
        human: AuditActor::principal(boot.human_id),
        human_id: boot.human_id,
    }
}

/// 种子任务（含目标/范围/交付）。
async fn seed_task(
    pool: &PgPool,
    project_id: Uuid,
    title: &str,
    status: &str,
    with_content: bool,
) -> Uuid {
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, priority, revision, tenant_id, \
                goal, description, deliverables) \
         VALUES ($1, $2, $3, 'normal', 1, (SELECT tenant_id FROM core.project WHERE id = $1), \
                $4, $5, $6) RETURNING id",
    )
    .bind(project_id)
    .bind(title)
    .bind(status)
    .bind(if with_content {
        Some(format!("{title} 的目标"))
    } else {
        None
    })
    .bind(if with_content {
        Some(format!("{title} 的范围"))
    } else {
        None
    })
    .bind(if with_content {
        serde_json::json!(["代码变更", "单元测试"])
    } else {
        serde_json::json!([])
    })
    .fetch_one(pool)
    .await
    .unwrap();
    id
}

/// 种子验收标准。
async fn seed_criterion(pool: &PgPool, project_id: Uuid, task_id: Uuid, text: &str) {
    sqlx::query(
        "INSERT INTO delivery.task_acceptance_criterion \
         (project_id, task_id, ordinal, text, verification_kind, tenant_id) \
         VALUES ($1, $2, 1, $3, 'automated_test', \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(text)
    .execute(pool)
    .await
    .unwrap();
}

/// 种子依赖边（task 依赖 depends_on）。
async fn seed_dependency(
    pool: &PgPool,
    project_id: Uuid,
    created_by: Uuid,
    task_id: Uuid,
    depends_on: Uuid,
) {
    sqlx::query(
        "INSERT INTO delivery.task_dependency \
         (tenant_id, project_id, task_id, depends_on_task_id, created_by) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3, $4)",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(depends_on)
    .bind(created_by)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "../../migrations")]
async fn task_detail_reads_basis_dependencies_and_criteria(pool: PgPool) {
    let fx = fixture(&pool, "detail").await;
    let upstream = seed_task(&pool, fx.project_id, "前置任务", "completed", true).await;
    let downstream = seed_task(&pool, fx.project_id, "后继任务", "draft", true).await;
    let target = seed_task(&pool, fx.project_id, "目标任务", "draft", true).await;

    seed_dependency(&pool, fx.project_id, fx.human_id, target, upstream).await;
    seed_dependency(&pool, fx.project_id, fx.human_id, downstream, target).await;
    seed_criterion(&pool, fx.project_id, target, "登录返回 200").await;

    let detail = task_read::task_detail(&pool, SYSTEM_TENANT, target)
        .await
        .unwrap();

    assert_eq!(detail.task.id, target);
    assert_eq!(detail.task.title, "目标任务");
    assert_eq!(detail.task.goal.as_deref(), Some("目标任务 的目标"));

    // 依赖方向：上游 = 本任务依赖的；下游 = 依赖本任务的。
    let upstream_row = detail
        .dependencies
        .iter()
        .find(|row| row.id == upstream)
        .expect("上游依赖");
    assert_eq!(upstream_row.direction, "upstream");
    let downstream_row = detail
        .dependencies
        .iter()
        .find(|row| row.id == downstream)
        .expect("下游依赖");
    assert_eq!(downstream_row.direction, "downstream");

    assert_eq!(detail.criteria.len(), 1);
    assert_eq!(detail.criteria[0].text, "登录返回 200");
    assert_eq!(detail.criteria[0].verification_kind, "automated_test");
}

#[sqlx::test(migrations = "../../migrations")]
async fn dependency_graph_computes_layers_critical_path_and_cycles(pool: PgPool) {
    let fx = fixture(&pool, "graph").await;
    // a → b → c（链）；d 与 b 无关（并行候选）。
    let a = seed_task(&pool, fx.project_id, "A", "draft", true).await;
    let b = seed_task(&pool, fx.project_id, "B", "draft", true).await;
    let c = seed_task(&pool, fx.project_id, "C", "draft", true).await;
    let d = seed_task(&pool, fx.project_id, "D", "draft", true).await;
    seed_dependency(&pool, fx.project_id, fx.human_id, b, a).await;
    seed_dependency(&pool, fx.project_id, fx.human_id, c, b).await;

    let graph = task_read::project_dependency_graph(&pool, SYSTEM_TENANT, fx.project_id)
        .await
        .unwrap();
    assert_eq!(graph.nodes.len(), 4);
    assert_eq!(graph.edges.len(), 2);
    assert!(graph.cycles.is_empty(), "无环图不得报环");
    assert_eq!(graph.layers.len(), 3, "a→b→c 应分三层");
    assert_eq!(graph.layers[0], vec![a, d]);
    assert_eq!(graph.layers[1], vec![b]);
    assert_eq!(graph.layers[2], vec![c]);
    assert_eq!(graph.critical_path, vec![a, b, c], "关键路径 = 最长下游链");
    assert!(
        graph
            .parallel_groups
            .iter()
            .any(|group| group.contains(&a) && group.contains(&d)),
        "同层且无依赖边的节点应成并行组"
    );

    // 造环：a → b → c → a。
    seed_dependency(&pool, fx.project_id, fx.human_id, a, c).await;
    let graph = task_read::project_dependency_graph(&pool, SYSTEM_TENANT, fx.project_id)
        .await
        .unwrap();
    assert!(!graph.cycles.is_empty(), "成环必须被检出");
}

#[sqlx::test(migrations = "../../migrations")]
async fn execution_readiness_marks_blocked_and_ready(pool: PgPool) {
    let fx = fixture(&pool, "ready").await;
    // 无内容、无依赖、无标准、无仓库 → 至少 C/D 段不过 → blocked。
    let bare = seed_task(&pool, fx.project_id, "裸任务", "draft", false).await;
    let readiness = task_read::project_execution_readiness(&pool, SYSTEM_TENANT, fx.project_id)
        .await
        .unwrap();
    assert_eq!(readiness.tasks.len(), 1);
    assert_eq!(readiness.tasks[0].task_id, bare);
    assert_eq!(readiness.tasks[0].summary.status, "blocked");
    assert_eq!(readiness.summary.status, "blocked");
    // 六段检查齐全。
    assert_eq!(readiness.tasks[0].checks.len(), 6);
    let sections: Vec<&str> = readiness.tasks[0]
        .checks
        .iter()
        .map(|check| check.section.as_str())
        .collect();
    assert_eq!(sections, vec!["A", "B", "C", "D", "E", "F"]);
    // C/D 未过 → blocked_count > 0。
    assert!(readiness.tasks[0].summary.blocked_count > 0);
    // A 段：缺目标与范围与交付 → 不过。
    let check_a = readiness.tasks[0]
        .checks
        .iter()
        .find(|check| check.section == "A")
        .unwrap();
    assert!(!check_a.passed);
    assert!(!check_a.issues.is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn task_schedule_writes_range_estimate_and_audit(pool: PgPool) {
    let fx = fixture(&pool, "schedule").await;
    let task = seed_task(&pool, fx.project_id, "排期任务", "draft", true).await;

    let start = chrono::DateTime::parse_from_rfc3339("2026-11-20T09:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let end = chrono::DateTime::parse_from_rfc3339("2026-11-22T18:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);

    let updated = task_read::set_task_schedule(
        &pool,
        SYSTEM_TENANT,
        task,
        TaskScheduleRequest {
            planned_start_at: Some(start),
            planned_end_at: Some(end),
            estimate_hours: Some(16.0),
        },
        &fx.human,
    )
    .await
    .unwrap();
    assert_eq!(updated.planned_start_at, Some(start));
    assert_eq!(updated.planned_end_at, Some(end));
    assert_eq!(updated.estimate_hours, Some(16.0));

    // 审计留痕。
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
         WHERE tenant_id = $1 AND target_type = 'task' AND target_id = $2 \
           AND action = 'task.schedule.updated'",
    )
    .bind(SYSTEM_TENANT)
    .bind(task)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 1, "排期变更必须留审计");

    // 非法区间被拒。
    let err = task_read::set_task_schedule(
        &pool,
        SYSTEM_TENANT,
        task,
        TaskScheduleRequest {
            planned_start_at: Some(end),
            planned_end_at: Some(start),
            estimate_hours: None,
        },
        &fx.human,
    )
    .await
    .unwrap_err();
    assert!(matches!(
        err,
        pm_application::pm_write::PmWriteError::Validation(_)
    ));

    // 非法工时被拒。
    let err = task_read::set_task_schedule(
        &pool,
        SYSTEM_TENANT,
        task,
        TaskScheduleRequest {
            planned_start_at: None,
            planned_end_at: None,
            estimate_hours: Some(-1.0),
        },
        &fx.human,
    )
    .await
    .unwrap_err();
    assert!(matches!(
        err,
        pm_application::pm_write::PmWriteError::Validation(_)
    ));
}
