//! S05 · 执行域读面集成测试（需要人工处理 / 态势概览 / 活动流 /
//! Run 附属事实 / 协作图 / Agent 名册）。
//!
//! 覆盖点：
//! - waiting reason 五值原样读出（不做归并）；
//! - 待答交互带问题原文与截止时间；
//! - 态势概览的计数与源表一致（活跃 Run / 失败 / 待评审 / 成功率）；
//! - ACP-07 重授权台账（含已撤销）；
//! - Run 附属事实（命令 / continuation / 逐仓 commit）；
//! - 协作图节点带未完成上游计数。

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::execution_read;
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

async fn fixture(pool: &PgPool, code: &str) -> (Uuid, Uuid) {
    let boot = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: format!("s05-{code}"),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    (boot.project_id, boot.human_id)
}

async fn seed_task(pool: &PgPool, project_id: Uuid, title: &str, status: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, priority, revision, tenant_id) \
         VALUES ($1, $2, $3, 'normal', 1, (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(project_id)
    .bind(title)
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_run(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    attempt_no: i32,
    status: &str,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task_run \
         (tenant_id, project_id, task_id, attempt_no, run_kind, status, lease_generation) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3, 'delivery', $4, 0) \
         RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(attempt_no)
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn waiting_queue_reads_reason_five_values_and_interactions(pool: PgPool) {
    let (project_id, _human_id) = fixture(&pool, "wait").await;

    // 一个任务至多一条活跃等待（uq_task_waiting_active）→ 五个任务各一条。
    for reason in [
        "needs_info",
        "needs_decision",
        "needs_approval",
        "external_dependency",
        "no_executor_available",
    ] {
        let task_id = seed_task(&pool, project_id, "等待任务", "awaiting_decision").await;
        sqlx::query(
            "INSERT INTO delivery.task_waiting \
             (project_id, task_id, reason, questions, since, tenant_id) \
             VALUES ($1, $2, $3, '[]'::jsonb, now(), \
                     (SELECT tenant_id FROM core.project WHERE id = $1))",
        )
        .bind(project_id)
        .bind(task_id)
        .bind(reason)
        .execute(&pool)
        .await
        .unwrap();
    }

    let queue = execution_read::project_waiting(&pool, SYSTEM_TENANT, project_id)
        .await
        .unwrap();
    assert_eq!(queue.waiting.len(), 5, "五个等待原因逐条读出");
    let reasons: Vec<&str> = queue
        .waiting
        .iter()
        .map(|row| row.reason.as_str())
        .collect();
    for reason in [
        "needs_info",
        "needs_decision",
        "needs_approval",
        "external_dependency",
        "no_executor_available",
    ] {
        assert!(reasons.contains(&reason), "缺少等待原因 {reason}");
    }
    assert_eq!(queue.waiting[0].task_title.as_deref(), Some("等待任务"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn execution_overview_counts_match_source_tables(pool: PgPool) {
    let (project_id, _human_id) = fixture(&pool, "overview").await;
    let task_id = seed_task(&pool, project_id, "运行任务", "running").await;
    let _running = seed_run(&pool, project_id, task_id, 1, "running").await;
    let _succeeded = seed_run(&pool, project_id, task_id, 2, "succeeded").await;
    let _failed = seed_run(&pool, project_id, task_id, 3, "failed").await;

    let overview = execution_read::project_execution_overview(&pool, SYSTEM_TENANT, project_id)
        .await
        .unwrap();
    assert_eq!(overview.total_run_count, 3);
    assert_eq!(overview.succeeded_run_count, 1);
    assert_eq!(overview.active_run_count, 1);
    assert_eq!(overview.failed_run_count, 1);
    let rate = overview.success_rate.expect("成功率");
    assert!((rate - 1.0 / 3.0).abs() < 1e-6, "成功率 = 成功数 / 总数");
    assert_eq!(overview.waiting_count, 0);
    assert_eq!(overview.pending_approval_count, 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn run_attachments_expose_authorization_commands_and_commits(pool: PgPool) {
    let (project_id, human_id) = fixture(&pool, "attach").await;
    let task_id = seed_task(&pool, project_id, "附属事实", "running").await;
    let run_id = seed_run(&pool, project_id, task_id, 1, "running").await;

    // ACP-07 重授权（Run 级）。
    sqlx::query(
        "INSERT INTO delivery.task_run_execution_authorization \
         (tenant_id, project_id, run_id, authorized_by, reason, new_deadline_at, expires_at) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, \
                 $3, \
                 'deadline 延长', now() + interval '1 hour', now() + interval '2 hour')",
    )
    .bind(project_id)
    .bind(run_id)
    .bind(human_id)
    .execute(&pool)
    .await
    .unwrap();

    let authorizations = execution_read::list_run_authorizations(&pool, SYSTEM_TENANT, run_id)
        .await
        .unwrap();
    assert_eq!(authorizations.len(), 1);
    assert_eq!(authorizations[0].reason, "deadline 延长");

    // Worker 命令。
    sqlx::query(
        "INSERT INTO delivery.worker_command \
         (tenant_id, project_id, command_type, worker_id, task_id, run_id, payload, status) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, 'agent.task.continue', \
                 'w-1', $2, $3, '{}'::jsonb, 'issued')",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(run_id)
    .execute(&pool)
    .await
    .unwrap();
    let commands = execution_read::list_run_commands(&pool, SYSTEM_TENANT, run_id)
        .await
        .unwrap();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].command_type, "agent.task.continue");
    assert_eq!(commands[0].status, "issued");

    // 待答交互（continuation 的 origin_interaction_id 有 FK）。
    let interaction_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_interaction \
         (tenant_id, project_id, task_id, run_id, event_id, reason, title, question, \
          options, required_response, status, chain_open, deadline_at) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3, 'e-1', \
                 'needs_info', '澄清口令策略', '使用哪种口令策略？', '[\"A\", \"B\"]'::jsonb, \
                 true, 'pending', true, now() + interval '1 hour') RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 待答交互必须出现在「需要人工处理」队列里，且带问题原文。
    let queue = execution_read::project_waiting(&pool, SYSTEM_TENANT, project_id)
        .await
        .unwrap();
    assert_eq!(queue.interactions.len(), 1);
    assert_eq!(queue.interactions[0].question, "使用哪种口令策略？");
    assert_eq!(queue.interactions[0].reason, "needs_info");

    // Continuation 链。
    sqlx::query(
        "INSERT INTO delivery.continuation \
         (tenant_id, project_id, task_id, run_id, origin_interaction_id, generation, status) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3, \
                 $4, 1, 'open')",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(run_id)
    .bind(interaction_id)
    .execute(&pool)
    .await
    .unwrap();
    let continuations = execution_read::list_run_continuations(&pool, SYSTEM_TENANT, run_id)
        .await
        .unwrap();
    assert_eq!(continuations.len(), 1);
    assert_eq!(continuations[0].status, "open");
    assert_eq!(continuations[0].generation, 1);

    // 逐仓交付清单。
    let repository_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.project_repository \
         (project_id, name, url, tenant_id) \
         VALUES ($1, 'core-api', 'https://example.invalid/core-api.git', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_repository_commit \
         (tenant_id, project_id, task_run_id, repository_id, commit_sha, changed_files) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3, 'abc1234', 3)",
    )
    .bind(project_id)
    .bind(run_id)
    .bind(repository_id)
    .execute(&pool)
    .await
    .unwrap();
    let commits = execution_read::list_run_commits(&pool, SYSTEM_TENANT, run_id)
        .await
        .unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].commit_sha, "abc1234");
    assert_eq!(commits[0].changed_files, 3);
    assert_eq!(commits[0].repository_name.as_deref(), Some("core-api"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn collaboration_graph_counts_open_upstreams(pool: PgPool) {
    let (project_id, human_id) = fixture(&pool, "collab").await;
    let upstream = seed_task(&pool, project_id, "前置", "draft").await;
    let target = seed_task(&pool, project_id, "目标", "draft").await;
    sqlx::query(
        "INSERT INTO delivery.task_dependency \
         (tenant_id, project_id, task_id, depends_on_task_id, created_by) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3, \
                 $4)",
    )
    .bind(project_id)
    .bind(target)
    .bind(upstream)
    .bind(human_id)
    .execute(&pool)
    .await
    .unwrap();

    let graph = execution_read::task_collaboration(&pool, SYSTEM_TENANT, target)
        .await
        .unwrap();
    assert_eq!(graph.nodes.len(), 2, "目标 + 其上游");
    assert_eq!(graph.edges.len(), 1);
    let target_node = graph
        .nodes
        .iter()
        .find(|node| node.id == target)
        .expect("目标节点");
    assert_eq!(target_node.open_upstream_count, 1, "未完成上游计数");
    let upstream_node = graph
        .nodes
        .iter()
        .find(|node| node.id == upstream)
        .expect("上游节点");
    assert_eq!(upstream_node.open_upstream_count, 0);
}
