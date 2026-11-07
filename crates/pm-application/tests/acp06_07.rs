//! ACP-06/07（0054）集成测试：
//! ACP-478 Checkpoint 登记幂等/不存正文 · ACP-479 无 Checkpoint 不阻止介入 ·
//! ACP-480 worker_receipt 通道 · ACP-481 过期+有效授权放行续领 ·
//! ACP-482 过期无授权拒绝+REST 授权语义（服务层）+撤销 · ACP-483 Workspace
//! 互斥操作化（情形 6 语义既有，此处登记策略验证）。

use pm_application::actor::AuditActor;
use pm_application::acp06_07;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use pm_application::interaction::{
    record_response_persisted, report_interaction, ReportInteractionInput,
};
use pm_application::recovery;
use pm_application::run_dispatch::RunDispatchConfig;
use pm_domain::SYSTEM_TENANT;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

async fn seed(pool: &PgPool, code: &str) -> (Uuid, Uuid, Uuid, Uuid) {
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
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, tenant_id, assignee_id) 
         VALUES ($1, 't', 'claimed', 1, (SELECT tenant_id FROM core.project WHERE id = $1), $2) 
         RETURNING id",
    )
    .bind(outcome.project_id)
    .bind(outcome.dev_agent_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let repo_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.project_repository (project_id, name, url, status, head_commit_sha, tenant_id) 
         VALUES ($1, 'web', 'git@host:repo', 'verified', 'headsha', 
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(outcome.project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_repository (task_id, project_id, repository_id, write_access, base_ref, tenant_id) 
         VALUES ($1, $2, $3, true, 'main', (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(task_id)
    .bind(outcome.project_id)
    .bind(repo_id)
    .execute(pool)
    .await
    .unwrap();
    (outcome.project_id, task_id, outcome.human_id, outcome.dev_agent_id)
}

async fn seed_device(pool: &PgPool, tenant_id: Uuid, owner: Uuid, name: &str) -> Uuid {
    sqlx::query(
        "INSERT INTO core.tenant_member (tenant_id, principal_id, role) 
         VALUES ($1, $2, 'owner') ON CONFLICT (tenant_id, principal_id) DO NOTHING",
    )
    .bind(tenant_id)
    .bind(owner)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO devenv.device (id, tenant_id, owner_principal_id, name, platform, status) 
         VALUES ($1, $2, $3, $4, 'linux', 'active') RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(tenant_id)
    .bind(owner)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// ACP-478：登记幂等（同 run+hash 返回原 id）；Platform 只存元数据不存正文。
#[sqlx::test(migrations = "../../migrations")]
async fn acp478_checkpoint_registration_idempotent(pool: PgPool) {
    let (project_id, task_id, human_id, _) = seed(&pool, "A78").await;
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) 
         VALUES ($1, $2, 1, 'queued', (SELECT tenant_id FROM core.project WHERE id = $1)) 
         RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let a = AuditActor::dispatch();

    let id1 = acp06_07::register_checkpoint(
        &pool, SYSTEM_TENANT, project_id, task_id, run_id, None,
        ".aipm/checkpoint.json", "abc123hash", 1024, Some("sess-1"),
        "executor_report", &a,
    )
    .await
    .unwrap();
    let id2 = acp06_07::register_checkpoint(
        &pool, SYSTEM_TENANT, project_id, task_id, run_id, None,
        ".aipm/checkpoint.json", "abc123hash", 1024, None,
        "executor_report", &a,
    )
    .await
    .unwrap();
    assert_eq!(id1, id2, "idempotent per (run, content_hash)");

    // 元数据齐备；无正文列。
    let row: (String, i64, String) = sqlx::query_as(
        "SELECT file_path, file_size, registered_via FROM delivery.task_checkpoint WHERE id = $1",
    )
    .bind(id1)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, ".aipm/checkpoint.json");
    assert_eq!(row.1, 1024);
    assert_eq!(row.2, "executor_report");
    let body_col: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.columns \
          WHERE table_name = 'task_checkpoint' AND column_name IN ('body','content')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(body_col, 0, "platform never stores the checkpoint body");
}

/// ACP-479：无 Checkpoint 不阻止介入/继续（interaction_report 全链零依赖）。
#[sqlx::test(migrations = "../../migrations")]
async fn acp479_no_checkpoint_never_blocks(pool: PgPool) {
    let (project_id, task_id, human_id, _) = seed(&pool, "A479").await;
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, execution_mode, stage, lease_owner, lease_token_hash, lease_generation, lease_expires_at, deadline_at, tenant_id) 
         VALUES ($1, $2, 1, 'claimed', 'spawn', 'started', 'client:dbg', 'h', 1, now() + interval '300 seconds', now() + interval '3600 seconds', (SELECT tenant_id FROM core.project WHERE id = $1)) 
         RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    // 全库无 checkpoint 行 → 介入照常成立。
    let reported = report_interaction(
        &pool,
        &ReportInteractionInput {
            project_id,
            task_id,
            run_id,
            event_id: "evt-1".into(),
            reason: "needs_decision".into(),
            title: "t".into(),
            question: "q".into(),
            options: None,
            required_response: false,
            checkpoint_path: Some(".aipm/checkpoint.json".into()),
        },
        &AuditActor::dispatch(),
    )
    .await
    .unwrap();
    assert!(!reported.replayed);
    // continue 下发的 payload checkpoint=null（无登记时）。
    record_response_persisted(
        &pool, project_id, reported.interaction_id, json!({ "response_id": "r" }),
        &AuditActor::dispatch(),
    )
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let c = pm_application::worker_command::issue_continue_command(
        &mut tx, project_id, task_id, run_id, reported.interaction_id, None, "client:dbg", 1,
        json!({ "decision": "approve" }),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let payload: serde_json::Value =
        sqlx::query_scalar("SELECT payload FROM delivery.worker_command WHERE id = $1")
            .bind(c)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(payload["checkpoint"].is_null(), "no registered checkpoint => null");
    // 有登记 → payload 携元数据引用。
    acp06_07::register_checkpoint(
        &pool, SYSTEM_TENANT, project_id, task_id, run_id, None,
        ".aipm/checkpoint.json", "hash-x", 10, None, "executor_report", &AuditActor::dispatch(),
    )
    .await
    .unwrap();
    // 登记后 latest_for_run 提供引用（payload 端由 issue 时读取该表实现）。
    let latest = pm_application::acp06_07::latest_for_run(&pool, run_id)
        .await
        .unwrap()
        .expect("checkpoint registered");
    assert_eq!(latest.0, ".aipm/checkpoint.json");
    assert_eq!(latest.1, "hash-x");
}

/// ACP-480：worker_receipt 通道登记（registered_via=worker_receipt，不判真实性）。
#[sqlx::test(migrations = "../../migrations")]
async fn acp480_worker_receipt_channel_registers(pool: PgPool) {
    let (project_id, task_id, human_id, _) = seed(&pool, "A480").await;
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) 
         VALUES ($1, $2, 1, 'queued', (SELECT tenant_id FROM core.project WHERE id = $1)) 
         RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let id = acp06_07::register_checkpoint(
        &pool, SYSTEM_TENANT, project_id, task_id, run_id, None,
        ".aipm/checkpoint.json", "receipt-hash", 5, None,
        "worker_receipt", &AuditActor::dispatch(),
    )
    .await
    .unwrap();
    let via: String =
        sqlx::query_scalar("SELECT registered_via FROM delivery.task_checkpoint WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(via, "worker_receipt");
}

/// ACP-481/482：Deadline 过期 → 无授权拒绝续领；授予后放行；撤销后复拒。
/// 授权期间 run.deadline_at 与交互有效期零改动（T3/独立有效期）。
#[sqlx::test(migrations = "../../migrations")]
async fn acp481_482_authorization_gates_reacquire(pool: PgPool) {
    let (project_id, task_id, human_id, _) = seed(&pool, "A481").await;
    let device_id = seed_device(&pool, SYSTEM_TENANT, human_id, "runner-auth").await;
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, execution_mode, stage, lease_owner, lease_token_hash, lease_generation, lease_expires_at, deadline_at, lease_client_id, tenant_id) 
         VALUES ($1, $2, 1, 'claimed', 'spawn', 'started', 'client:dbg', 'h', 1, \
                 now() - interval '5 seconds', now() - interval '10 seconds', $3, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) 
         RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(device_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    // shield=start_wait + responded interaction（续领前置）。
    sqlx::query("UPDATE delivery.task_run SET scheduling_shield = 'start_wait' WHERE id = $1")
        .bind(run_id)
        .execute(&pool)
        .await
        .unwrap();
    let interaction_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_interaction 
           (id, tenant_id, project_id, task_id, run_id, event_id, reason, title, question, status, chain_open, deadline_at) 
         VALUES ($1, (SELECT tenant_id FROM core.project WHERE id = $2), $2, $3, $4, 'e1', 'needs_decision', 't', 'q', 'responded', TRUE, now()) 
         RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(task_id)
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let _ = interaction_id;
    let a = AuditActor::principal(human_id);

    // 过期 + 无授权 → 拒绝（ACP-482 前半）。
    let err = recovery::reacquire_lease(
        &pool, SYSTEM_TENANT, project_id, run_id, device_id,
        RunDispatchConfig::from_env().lease_secs, &a,
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("ACP-07"), "{err}");

    // 授予（Lead 语义；独立有效期）→ 放行（ACP-481）。
    let new_dl = chrono::Utc::now() + chrono::Duration::hours(1);
    let auth_id = acp06_07::grant_execution_authorization(
        &pool, SYSTEM_TENANT, project_id, run_id, None, human_id,
        "expired_deadline", new_dl, &a,
    )
    .await
    .unwrap();
    let lease = recovery::reacquire_lease(
        &pool, SYSTEM_TENANT, project_id, run_id, device_id,
        RunDispatchConfig::from_env().lease_secs, &a,
    )
    .await
    .unwrap();
    assert_eq!(lease.lease_generation, 2);
    // run.deadline_at 零改动（独立有效期在授权行）。
    let run_dl: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT deadline_at FROM delivery.task_run WHERE id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        run_dl <= chrono::Utc::now(),
        "run.deadline_at must remain untouched (T3)"
    );

    // 撤销 → 再次续领（需第二次过期模拟）拒绝（ACP-482 撤销面）。
    assert!(acp06_07::revoke_execution_authorization(
        &pool, SYSTEM_TENANT, project_id, run_id, auth_id, &a,
    )
    .await
    .unwrap());
    sqlx::query("UPDATE delivery.task_run SET lease_expires_at = now() - interval '5 seconds' WHERE id = $1")
        .bind(run_id)
        .execute(&pool)
        .await
        .unwrap();
    let err = recovery::reacquire_lease(
        &pool, SYSTEM_TENANT, project_id, run_id, device_id,
        RunDispatchConfig::from_env().lease_secs, &a,
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("ACP-07"), "{err}");
}

/// ACP-483（操作化验证）：情形 6 语义 = Workspace 互斥（未确认禁启）既有；
/// 此处验证续领不改 shield、不新建 Run（run 隔离目录单写者不变）。
#[sqlx::test(migrations = "../../migrations")]
async fn acp483_workspace_mutual_exclusion_operationalization(pool: PgPool) {
    let (project_id, task_id, human_id, _) = seed(&pool, "A483").await;
    let device_id = seed_device(&pool, SYSTEM_TENANT, human_id, "runner-w").await;
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, execution_mode, stage, lease_owner, lease_token_hash, lease_generation, lease_expires_at, deadline_at, lease_client_id, scheduling_shield, tenant_id) 
         VALUES ($1, $2, 1, 'claimed', 'spawn', 'started', 'client:dbg', 'h', 1, \
                 now() + interval '300 seconds', now() + interval '3600 seconds', $3, 'start_wait', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) 
         RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(device_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    // 续领前置：存在 responded Interaction（回复已持久化）。
    sqlx::query(
        "INSERT INTO delivery.task_interaction 
           (id, tenant_id, project_id, task_id, run_id, event_id, reason, title, question, status, chain_open, deadline_at) 
         VALUES ($1, (SELECT tenant_id FROM core.project WHERE id = $2), $2, $3, $4, 'e1', 'needs_decision', 't', 'q', 'responded', TRUE, now()) 
         RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(task_id)
    .bind(run_id)
    .execute(&pool)
    .await
    .unwrap();
    // 续领（恢复）不改 shield、不新建 Run（attempt 1 保持）——run 隔离目录单写者。
    let before: (String, i32) = sqlx::query_as(
        "SELECT scheduling_shield, attempt_no FROM delivery.task_run WHERE id = $1",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    recovery::reacquire_lease(
        &pool, SYSTEM_TENANT, project_id, run_id, device_id,
        RunDispatchConfig::from_env().lease_secs, &AuditActor::principal(human_id),
    )
    .await
    .unwrap();
    let after: (String, i32) = sqlx::query_as(
        "SELECT scheduling_shield, attempt_no FROM delivery.task_run WHERE id = $1",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(before, after, "reacquire must not create a run or change the shield");
}
