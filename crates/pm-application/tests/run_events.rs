//! S05 执行监控最小事件面（迁移 0013）集成测试：
//! - `record_run_event` 幂等（UNIQUE + ON CONFLICT）与追加式守卫（禁 UPDATE）；
//! - generation 语义：同 tag 新 generation 成新行；tenant/project/task/client
//!   一律快照自 run 行（不信任调用点）；
//! - 接线：claim → `run_claimed`；`report_progress` 首报 started → `run_started`、
//!   阶段推进 → `run_stage_changed`（同阶段重报不重复）；`cancel_run` →
//!   `run_finished`（outcome/reason 落 detail）。

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::run_context::{report_progress, ProgressReport};
use pm_application::run_dispatch::{claim_task_run, NoopExecutor, RunDispatchConfig};
use pm_application::run_events::{event_type, record_run_event, RunEvent};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    project_id: Uuid,
    task_id: Uuid,
    run_id: Uuid,
}

fn cfg() -> RunDispatchConfig {
    RunDispatchConfig {
        lease_secs: 300,
        backoff_cap_secs: 300,
        max_run_attempts: 3,
        max_infrastructure_attempts: 5,
        timeout_secs: 1800,
        deadline_default_secs: 3600,
        agent_heartbeat_stale_secs: 900,
        progress_stale_secs: 3600,
        exclude_projects: vec![],
    }
}

/// 项目 + ready 任务 + 已绑定仓库 + queued run（与 run_context.rs 测试同款种子）。
async fn seed(pool: &PgPool, code: &str) -> Fx {
    let outcome = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let agent_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('dev_agent', 'executor') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, assignee_id, task_type, tenant_id) \
         VALUES ($1, '事件面种子任务', 'ready', 1, $2, 'architecture', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(outcome.project_id)
    .bind(agent_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let repo_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.project_repository (project_id, name, url, status, head_commit_sha, tenant_id) \
         VALUES ($1, 'world-view', 'git@host:ev', 'verified', 'headsha', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(outcome.project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_repository (task_id, project_id, repository_id, write_access, base_ref, path_scope, tenant_id) \
         VALUES ($1, $2, $3, true, 'main', ARRAY['crates/**'], \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(task_id)
    .bind(outcome.project_id)
    .bind(repo_id)
    .execute(pool)
    .await
    .unwrap();
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 1, 'queued', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(outcome.project_id)
    .bind(task_id)
    .fetch_one(pool)
    .await
    .unwrap();
    Fx {
        project_id: outcome.project_id,
        task_id,
        run_id,
    }
}

async fn count_type(pool: &PgPool, run_id: Uuid, t: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_run_event WHERE run_id = $1 AND event_type = $2",
    )
    .bind(run_id)
    .bind(t)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn record(
    pool: &PgPool,
    run_id: Uuid,
    ty: &'static str,
    generation: Option<i32>,
    tag: &'static str,
) -> u64 {
    let mut conn = pool.acquire().await.unwrap();
    record_run_event(
        &mut conn,
        run_id,
        RunEvent {
            event_type: ty,
            generation,
            dedupe_tag: tag,
            detail: None,
        },
    )
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn event_log_is_idempotent_append_only_and_generation_scoped(pool: PgPool) {
    let fx = seed(&pool, "EV-01").await;

    // ① 幂等：同 (run, type, generation, tag) 重插 = 0 行。
    assert_eq!(
        record(
            &pool,
            fx.run_id,
            event_type::RUN_STAGE_CHANGED,
            Some(1),
            "testing"
        )
        .await,
        1
    );
    assert_eq!(
        record(
            &pool,
            fx.run_id,
            event_type::RUN_STAGE_CHANGED,
            Some(1),
            "testing"
        )
        .await,
        0
    );
    // ② 新 generation 同 tag → 新行（续领后同阶段可再记）。
    assert_eq!(
        record(
            &pool,
            fx.run_id,
            event_type::RUN_STAGE_CHANGED,
            Some(2),
            "testing"
        )
        .await,
        1
    );
    assert_eq!(
        count_type(&pool, fx.run_id, event_type::RUN_STAGE_CHANGED).await,
        2
    );

    // ③ 追加式守卫：UPDATE 被 trigger 拒绝。
    let err = sqlx::query(
        "UPDATE delivery.task_run_event SET detail = '{\"x\":1}'::jsonb WHERE run_id = $1",
    )
    .bind(fx.run_id)
    .execute(&pool)
    .await;
    assert!(err.is_err(), "append-only guard must reject UPDATE");

    // ④ 维度快照自 run 行，不信任调用点。
    let (tenant_id, project_id, task_id): (Uuid, Uuid, Uuid) = sqlx::query_as(
        "SELECT tenant_id, project_id, task_id FROM delivery.task_run_event WHERE run_id = $1",
    )
    .bind(fx.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let run_row: (Uuid, Uuid, Uuid) = sqlx::query_as(
        "SELECT tenant_id, project_id, task_id FROM delivery.task_run WHERE id = $1",
    )
    .bind(fx.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((tenant_id, project_id, task_id), run_row);
}

#[sqlx::test(migrations = "../../migrations")]
async fn claim_and_progress_paths_emit_events(pool: PgPool) {
    let fx = seed(&pool, "EV-02").await;

    let claimed = claim_task_run(
        &pool,
        fx.project_id,
        fx.task_id,
        "codex",
        Some("1.0.0"),
        &cfg(),
    )
    .await
    .unwrap();
    assert_eq!(
        count_type(&pool, claimed.run_id, event_type::RUN_CLAIMED).await,
        1
    );

    // 首报 started → run_started（claim 后 stage='dispatch'，started 分支优先于
    // 阶段推进——不重复落 stage 事件）。
    report_progress(
        &pool,
        fx.project_id,
        claimed.run_id,
        &claimed.lease_token,
        ProgressReport {
            stage: "started",
            note: None,
            blocked: false,
        },
        &cfg(),
        claimed.lease_generation,
    )
    .await
    .unwrap();
    assert_eq!(
        count_type(&pool, claimed.run_id, event_type::RUN_STARTED).await,
        1
    );
    assert_eq!(
        count_type(&pool, claimed.run_id, event_type::RUN_STAGE_CHANGED).await,
        0
    );

    // 阶段推进 coding → run_stage_changed(tag=coding)。
    report_progress(
        &pool,
        fx.project_id,
        claimed.run_id,
        &claimed.lease_token,
        ProgressReport {
            stage: "coding",
            note: Some("impl entry"),
            blocked: false,
        },
        &cfg(),
        claimed.lease_generation,
    )
    .await
    .unwrap();
    assert_eq!(
        count_type(&pool, claimed.run_id, event_type::RUN_STAGE_CHANGED).await,
        1
    );

    // 同阶段重报（心跳续租）不重复落账。
    report_progress(
        &pool,
        fx.project_id,
        claimed.run_id,
        &claimed.lease_token,
        ProgressReport {
            stage: "coding",
            note: Some("still coding"),
            blocked: false,
        },
        &cfg(),
        claimed.lease_generation,
    )
    .await
    .unwrap();
    assert_eq!(
        count_type(&pool, claimed.run_id, event_type::RUN_STAGE_CHANGED).await,
        1
    );

    // 换阶段 testing → +1；tag=stage 保证阶段事件与 run 一一对应。
    report_progress(
        &pool,
        fx.project_id,
        claimed.run_id,
        &claimed.lease_token,
        ProgressReport {
            stage: "testing",
            note: None,
            blocked: false,
        },
        &cfg(),
        claimed.lease_generation,
    )
    .await
    .unwrap();
    assert_eq!(
        count_type(&pool, claimed.run_id, event_type::RUN_STAGE_CHANGED).await,
        2
    );
    let tags: Vec<String> = sqlx::query_scalar(
        "SELECT dedupe_tag FROM delivery.task_run_event \
          WHERE run_id = $1 AND event_type = 'run_stage_changed' ORDER BY occurred_at",
    )
    .bind(claimed.run_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(tags, vec!["coding".to_string(), "testing".to_string()]);
}

/// 建一条 `claimed` 态 delivery run（终态路径前置；executor_kind 不能是
/// 'platform'——0026 库层隔离 review Run）。
async fn seed_claimed_run(pool: &PgPool, project_id: Uuid, task_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task_run \
         (project_id, task_id, attempt_no, status, run_kind, executor_kind, tenant_id) \
         VALUES ($1, $2, 1, 'claimed', 'delivery', 'codex', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn cancel_run_emits_finished_event(pool: PgPool) {
    let outcome = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "EV-03".to_string(),
            project_name: "EV-03".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, tenant_id) \
         VALUES ($1, '取消终态种子', 'running', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(outcome.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let run_id = seed_claimed_run(&pool, outcome.project_id, task_id).await;

    let cancelled =
        pm_application::run_executor::cancel_run(&pool, &NoopExecutor, run_id, "user_cancel")
            .await
            .unwrap();
    assert!(cancelled);

    let (event_type, detail): (String, serde_json::Value) = sqlx::query_as(
        "SELECT event_type, detail FROM delivery.task_run_event \
          WHERE run_id = $1 AND event_type = 'run_finished'",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event_type, "run_finished");
    assert_eq!(detail["outcome"], "cancelled");
    assert_eq!(detail["reason"], "user_cancel");
}
