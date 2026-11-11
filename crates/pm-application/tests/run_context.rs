//! Slice D2 契约与租约的真实 PostgreSQL 集成测试
//! （`docs/ai-pm-slice-d2-mcp-context-v1.md` §七：C1–C28 中属 pm-application 层的部分）。
//!
//! 断言的是 **pm-application 层 `LeaseError` / `RunDispatchError` 变体**，不是
//! pm-mcp 的 `ToolError`（R-G3：依赖方向 pm-mcp → pm-application）。

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::pm_write::{
    update_task, AcceptanceCriterionInput, DeliverableInput, KnowledgeRefInput, UpdateTaskRequest,
};
use pm_application::run_context::{
    get_run_context, hash_lease_token, renew_lease_by_token, report_progress, LeaseError,
    ProgressReport,
};
use pm_application::run_dispatch::{
    claim_once, claim_once_with_policy, claim_task_run, DispatchMode, ExecutionPolicy,
    RunDispatchConfig, RunDispatchError,
};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    project_id: Uuid,
    task_id: Uuid,
    run_id: Uuid,
    agent_id: Uuid,
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

async fn add_repo(pool: &PgPool, project_id: Uuid, code: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO integration.project_repository (project_id, name, url, status, head_commit_sha, tenant_id) \
         VALUES ($1, 'world-view', $2, 'verified', 'headsha', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(format!("git@host:{code}"))
    .fetch_one(pool)
    .await
    .unwrap()
}

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
         VALUES ($1, '确定地形渲染架构', 'ready', 1, $2, 'architecture', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(outcome.project_id)
    .bind(agent_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let repo_id = add_repo(pool, outcome.project_id, code).await;
    sqlx::query(
        "INSERT INTO delivery.task_repository (task_id, project_id, repository_id, write_access, base_ref, path_scope, tenant_id) \
         VALUES ($1, $2, $3, true, 'main', ARRAY['crates/terrain/**'], \
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
        agent_id,
    }
}

/// 通过 **pm_write 契约写路径**（待裁 #11 已并入 D2）写入完整契约，不依赖 SQL 手动种子。
async fn write_full_contract(pool: &PgPool, fx: &Fx) -> Uuid {
    let document_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.project_document (project_id, doc_key, doc_type, canonical_path, tenant_id) \
         VALUES ($1, 'architecture.terrain', 'architecture', 'architecture/terrain.md', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(fx.project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let author: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('ai_pm', 'doc-author') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    for revision_no in [1, 2] {
        sqlx::query(
            "INSERT INTO ai.document_revision \
             (project_id, document_id, revision_no, git_commit_sha, content_hash, status, created_by, tenant_id) \
             VALUES ($1, $2, $3, $4, $5, 'published', $6, \
                     (SELECT tenant_id FROM core.project WHERE id = $1))",
        )
        .bind(fx.project_id)
        .bind(document_id)
        .bind(revision_no)
        .bind(format!("sha-{revision_no}"))
        .bind(format!("hash-{revision_no}"))
        .bind(author)
        .execute(pool)
        .await
        .unwrap();
    }

    update_task(
        pool,
        fx.task_id,
        UpdateTaskRequest {
            revision: Some(1),
            title: Some("确定地形渲染架构".to_string()),
            description: Some("比较 GPU Shader 与 CPU 预计算".to_string()),
            status: Some("ready".to_string()),
            priority: Some("normal".to_string()),
            assignee_id: Some(fx.agent_id),
            goal: Some(Some("确定 Bevy 2D 地形高度晕渲方案".to_string())),
            constraints: Some(serde_json::json!({
                "language": "Rust", "engine": "Bevy 0.19", "map_scale": "regional"
            })),
            deliverables: Some(vec![DeliverableInput {
                id: Some("ADR-TERRAIN".to_string()),
                path: "docs/adr/001-terrain-rendering.md".to_string(),
            }]),
            // 乱序插入：读取时必须按 ordinal 升序（验收 C25）。
            acceptance_criteria: Some(vec![
                AcceptanceCriterionInput {
                    ordinal: Some(20),
                    text: "cargo test 全绿".to_string(),
                    verification_kind: Some("automated_test".to_string()),
                },
                AcceptanceCriterionInput {
                    ordinal: Some(10),
                    text: "比较 GPU Shader 和 CPU 预计算".to_string(),
                    verification_kind: Some("review".to_string()),
                },
            ]),
            knowledge_refs: Some(vec![KnowledgeRefInput {
                document_id,
                is_required: true,
                pinned_revision_no: None,
            }]),
        },
        &pm_application::actor::AuditActor::dispatch(),
    )
    .await
    .unwrap();

    document_id
}

#[sqlx::test(migrations = "../../migrations")]
async fn c1_c27_claim_signs_a_one_time_token_and_records_the_client(pool: PgPool) {
    let fx = seed(&pool, "D2-C1").await;
    let claimed = claim_task_run(
        &pool,
        fx.project_id,
        fx.task_id,
        "codex",
        Some("1.2.0"),
        &cfg(),
    )
    .await
    .unwrap();

    assert_eq!(
        claimed.lease_token.len(),
        64,
        "32 random bytes as lowercase hex"
    );
    assert!(
        claimed
            .lease_token
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "the plaintext token is lowercase hex"
    );

    let (stored_hash, client_name, client_version, lease_owner): (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT lease_token_hash, client_name, client_version, lease_owner \
           FROM delivery.task_run WHERE id = $1",
    )
    .bind(claimed.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let stored_hash = stored_hash.expect("a Model A claim stores the hash");
    assert_ne!(
        stored_hash, claimed.lease_token,
        "the plaintext token is never stored"
    );
    assert_eq!(stored_hash, hash_lease_token(&claimed.lease_token));
    assert_eq!(client_name.as_deref(), Some("codex"));
    assert_eq!(client_version.as_deref(), Some("1.2.0"));
    assert_eq!(
        lease_owner.as_deref(),
        Some(format!("mcp:codex:{}", &claimed.lease_token[..8]).as_str()),
        "lease_owner is mcp:<client>:<token prefix>, never the credential itself"
    );
    assert_eq!(
        claimed.git_branch,
        format!("task/{}", &fx.task_id.to_string()[..8])
    );
    let task_status: String = sqlx::query_scalar("SELECT status FROM delivery.task WHERE id = $1")
        .bind(fx.task_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(task_status, "claimed");
}

#[sqlx::test(migrations = "../../migrations")]
async fn c2_c26_claim_preconditions(pool: PgPool) {
    let fx = seed(&pool, "D2-C2").await;
    // 跨项目：不泄露存在性。
    let other = seed(&pool, "D2-C2-OTHER").await;
    let err = claim_task_run(&pool, other.project_id, fx.task_id, "codex", None, &cfg())
        .await
        .unwrap_err();
    assert!(matches!(err, RunDispatchError::TaskNotFound));

    let claimed = claim_task_run(&pool, fx.project_id, fx.task_id, "codex", None, &cfg())
        .await
        .unwrap();
    let err = claim_task_run(&pool, fx.project_id, fx.task_id, "other", None, &cfg())
        .await
        .unwrap_err();
    assert!(
        matches!(err, RunDispatchError::RunAlreadyClaimed),
        "a second claim of the same task must be refused, got {err:?}"
    );
    let active: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_run WHERE task_id = $1 AND status IN ('queued','claimed','running')",
    )
    .bind(fx.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        active, 1,
        "the unique active-run index keeps a single active run"
    );

    // 无 Run：TaskReadyHandler 尚未创建。
    let no_run_task: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, assignee_id, tenant_id) \
         VALUES ($1, 'no-run', 'ready', $2, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(fx.project_id)
    .bind(fx.agent_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_repository (task_id, project_id, repository_id, write_access, base_ref, tenant_id) \
         SELECT $1, $2, repository_id, true, 'main', \
                (SELECT tenant_id FROM core.project WHERE id = $2) \
         FROM delivery.task_repository WHERE task_id = $3",
    )
    .bind(no_run_task)
    .bind(fx.project_id)
    .bind(fx.task_id)
    .execute(&pool)
    .await
    .unwrap();
    let err = claim_task_run(&pool, fx.project_id, no_run_task, "codex", None, &cfg())
        .await
        .unwrap_err();
    assert!(matches!(err, RunDispatchError::NoRunnableRun));

    // 非 ready：附当前状态。
    let not_ready_task = no_run_task;
    sqlx::query(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 1, 'queued', (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(fx.project_id)
    .bind(not_ready_task)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE delivery.task SET status = 'draft' WHERE id = $1")
        .bind(not_ready_task)
        .execute(&pool)
        .await
        .unwrap();
    let err = claim_task_run(&pool, fx.project_id, not_ready_task, "codex", None, &cfg())
        .await
        .unwrap_err();
    match err {
        RunDispatchError::TaskNotReady(state) => assert_eq!(state, "draft"),
        other => panic!("expected TaskNotReady, got {other:?}"),
    }
    assert!(claimed.run_id != Uuid::nil());
}

#[sqlx::test(migrations = "../../migrations")]
async fn c3_c24_c25_run_context_maps_every_contract_field(pool: PgPool) {
    let fx = seed(&pool, "D2-C3").await;
    let document_id = write_full_contract(&pool, &fx).await;
    let claimed = claim_task_run(&pool, fx.project_id, fx.task_id, "codex", None, &cfg())
        .await
        .unwrap();

    let contract = get_run_context(
        &pool,
        fx.project_id,
        claimed.run_id,
        &claimed.lease_token,
        claimed.lease_generation,
    )
    .await
    .unwrap();
    assert_eq!(contract.contract_version, 1);
    assert_eq!(contract.run.run_id, claimed.run_id);
    assert_eq!(contract.run.attempt_no, 1);
    assert_eq!(contract.run.timeout_seconds, Some(1800));
    assert_eq!(contract.run.lease.heartbeat_interval_secs, 60);

    assert_eq!(contract.task.title, "确定地形渲染架构");
    assert_eq!(contract.task.task_type.as_deref(), Some("architecture"));
    assert_eq!(contract.task.status, "claimed");
    assert_eq!(
        contract.task.goal.as_deref(),
        Some("确定 Bevy 2D 地形高度晕渲方案")
    );
    assert_eq!(contract.task.constraints["engine"], "Bevy 0.19");
    assert_eq!(contract.task.deliverables.len(), 1);
    assert_eq!(contract.task.deliverables[0].id, "ADR-TERRAIN");
    assert_eq!(
        contract.task.deliverables[0].path,
        "docs/adr/001-terrain-rendering.md"
    );
    assert_eq!(
        contract.task.git_branch.as_deref(),
        Some(format!("task/{}", &fx.task_id.to_string()[..8]).as_str())
    );

    // C25：按 ordinal 升序，允许间隙。
    let ordinals: Vec<i32> = contract
        .acceptance_criteria
        .iter()
        .map(|criterion| criterion.ordinal)
        .collect();
    assert_eq!(ordinals, vec![10, 20]);
    assert_eq!(contract.acceptance_criteria[0].verification_kind, "review");

    let snapshot_rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.task_run_repository WHERE run_id = $1")
            .bind(claimed.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(contract.repositories.len() as i64, snapshot_rows);
    assert_eq!(contract.repositories[0].name, "world-view");
    assert_eq!(contract.repositories[0].base_ref, "main");
    assert_eq!(contract.repositories[0].base_commit, "headsha");
    assert_eq!(
        contract.repositories[0].workspace_path,
        pm_application::run_dispatch::workspace_path_for(
            contract.repositories[0].repository_id,
            "world-view"
        )
    );
    assert_eq!(
        contract.repositories[0].path_scope.as_deref(),
        Some(&["crates/terrain/**".to_string()][..])
    );

    // C24：live 解析取最新 revision。
    assert_eq!(contract.knowledge_refs.len(), 1);
    assert_eq!(contract.knowledge_refs[0].document_id, document_id);
    assert_eq!(contract.knowledge_refs[0].revision_no, Some(2));
    assert!(contract.knowledge_refs[0].is_required);

    assert!(contract.policy.execution_enabled);
    assert_eq!(contract.policy.snapshot_scope, "composed-v2");
}

#[sqlx::test(migrations = "../../migrations")]
async fn c5_c6_c7_c17_c28_lease_rejections(pool: PgPool) {
    let fx = seed(&pool, "D2-C6").await;
    let claimed = claim_task_run(&pool, fx.project_id, fx.task_id, "codex", None, &cfg())
        .await
        .unwrap();
    let other = seed(&pool, "D2-C6-OTHER").await;

    let err = get_run_context(
        &pool,
        other.project_id,
        claimed.run_id,
        &claimed.lease_token,
        claimed.lease_generation,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, LeaseError::NotFound),
        "cross-project reads as not found"
    );

    let err = get_run_context(
        &pool,
        fx.project_id,
        claimed.run_id,
        "deadbeef",
        claimed.lease_generation,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, LeaseError::Forbidden));

    let err = get_run_context(
        &pool,
        fx.project_id,
        claimed.run_id,
        "",
        claimed.lease_generation,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, LeaseError::Forbidden));

    // C28：大写 hex 变体必须拒绝（严格匹配小写 hex 的 sha256）。
    let uppercase = hash_lease_token(&claimed.lease_token).to_uppercase();
    sqlx::query("UPDATE delivery.task_run SET lease_token_hash = $2 WHERE id = $1")
        .bind(claimed.run_id)
        .bind(&uppercase)
        .execute(&pool)
        .await
        .unwrap();
    let err = get_run_context(
        &pool,
        fx.project_id,
        claimed.run_id,
        &claimed.lease_token,
        claimed.lease_generation,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, LeaseError::Forbidden));

    sqlx::query("UPDATE delivery.task_run SET lease_token_hash = $2 WHERE id = $1")
        .bind(claimed.run_id)
        .bind(hash_lease_token(&claimed.lease_token))
        .execute(&pool)
        .await
        .unwrap();

    // C7：过期后正确 token 也拒绝。
    sqlx::query(
        "UPDATE delivery.task_run SET lease_expires_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(claimed.run_id)
    .execute(&pool)
    .await
    .unwrap();
    let err = get_run_context(
        &pool,
        fx.project_id,
        claimed.run_id,
        &claimed.lease_token,
        claimed.lease_generation,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, LeaseError::Expired));

    // C17：Model B 领取的 Run 没有 hash → 任意 token 都被拒绝，不 panic。
    sqlx::query("UPDATE delivery.task_run SET status = 'succeeded' WHERE id = $1")
        .bind(claimed.run_id)
        .execute(&pool)
        .await
        .unwrap();
    let fx_b = seed(&pool, "D2-C17").await;
    let worker_claim = claim_once(&pool, "worker-1", &cfg(), DispatchMode::On)
        .await
        .unwrap()
        .unwrap();
    // T-032（ADR-0002 L1）：Model B 也统一签发 lease_token（执行端 MCP 凭据）。
    assert!(
        worker_claim.lease_token.is_some(),
        "Model B also issues a lease token"
    );
    assert_eq!(
        worker_claim.lease_generation, 1,
        "first valid claim is generation 1"
    );
    let hash: Option<String> =
        sqlx::query_scalar("SELECT lease_token_hash FROM delivery.task_run WHERE id = $1")
            .bind(worker_claim.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        hash.is_some(),
        "T-032: Model B stores a lease token hash too"
    );
    // claim_once 取最早的候选 Run，可能来自本测试先前任一项目；按 Run 自身的
    // project 校验，再用另一个项目验证「跨项目不可见」。
    let worker_project: Uuid =
        sqlx::query_scalar("SELECT project_id FROM delivery.task_run WHERE id = $1")
            .bind(worker_claim.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let err = get_run_context(
        &pool,
        worker_project,
        worker_claim.run_id,
        "anything",
        worker_claim.lease_generation,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, LeaseError::Forbidden));
    let foreign_project = if worker_project == fx.project_id {
        fx_b.project_id
    } else {
        fx.project_id
    };
    let err = get_run_context(
        &pool,
        foreign_project,
        worker_claim.run_id,
        "anything",
        worker_claim.lease_generation,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, LeaseError::NotFound),
        "a run of another project is invisible"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn c8_c9_c22_report_progress_renews_and_advances_once(pool: PgPool) {
    let fx = seed(&pool, "D2-C8").await;
    let claimed = claim_task_run(
        &pool,
        fx.project_id,
        fx.task_id,
        "codex",
        Some("1.2.0"),
        &cfg(),
    )
    .await
    .unwrap();
    let before: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT lease_expires_at FROM delivery.task_run WHERE id = $1")
            .bind(claimed.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    let reported = report_progress(
        &pool,
        fx.project_id,
        claimed.run_id,
        &claimed.lease_token,
        ProgressReport {
            stage: "started",
            note: Some("located crates/terrain/src/render.rs"),
            blocked: false,
        },
        &cfg(),
        claimed.lease_generation,
    )
    .await
    .unwrap();
    assert!(
        reported.lease_expires_at > before,
        "the lease moves strictly forward"
    );
    assert_eq!(reported.task_status, "running");
    assert_eq!(reported.status, "running");
    assert_eq!(reported.stage.as_deref(), Some("started"));

    let (run_status, executor_kind, executor_session, task_status): (
        String,
        Option<String>,
        Option<String>,
        String,
    ) = sqlx::query_as(
        "SELECT r.status, r.executor_kind, r.executor_session, t.status \
           FROM delivery.task_run r JOIN delivery.task t ON t.id = r.task_id WHERE r.id = $1",
    )
    .bind(claimed.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(run_status, "running");
    assert_eq!(executor_kind.as_deref(), Some("mcp"));
    assert_eq!(executor_session.as_deref(), Some("mcp:codex:1.2.0"));
    assert_eq!(task_status, "running");

    let started_audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log WHERE target_id = $1 AND action = 'run_stage_started'",
    )
    .bind(claimed.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(started_audits, 1);

    // C22：运行态二次上报仍可续租，且严格前移。
    let first_expiry = reported.lease_expires_at;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let second = report_progress(
        &pool,
        fx.project_id,
        claimed.run_id,
        &claimed.lease_token,
        ProgressReport {
            stage: "started",
            note: Some("still running"),
            blocked: false,
        },
        &cfg(),
        claimed.lease_generation,
    )
    .await
    .unwrap();
    assert!(second.lease_expires_at > first_expiry);
    let renew_audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log WHERE target_id = $1 AND action = 'renew_run_lease'",
    )
    .bind(claimed.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(renew_audits, 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn c10_c15_renewal_rejects_wrong_and_expired_tokens(pool: PgPool) {
    let fx = seed(&pool, "D2-C10").await;
    let claimed = claim_task_run(&pool, fx.project_id, fx.task_id, "codex", None, &cfg())
        .await
        .unwrap();
    let (owner_before, hash_before): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT lease_owner, lease_token_hash FROM delivery.task_run WHERE id = $1")
            .bind(claimed.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    let err = renew_lease_by_token(
        &pool,
        fx.project_id,
        claimed.run_id,
        "wrong",
        &cfg(),
        claimed.lease_generation,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, LeaseError::Forbidden));
    let (owner_after, hash_after): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT lease_owner, lease_token_hash FROM delivery.task_run WHERE id = $1")
            .bind(claimed.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (owner_before, hash_before),
        (owner_after, hash_after),
        "a rejected renewal has no side effects"
    );

    let expiry_before: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT lease_expires_at FROM delivery.task_run WHERE id = $1")
            .bind(claimed.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(
        "UPDATE delivery.task_run SET lease_expires_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(claimed.run_id)
    .execute(&pool)
    .await
    .unwrap();
    let err = renew_lease_by_token(
        &pool,
        fx.project_id,
        claimed.run_id,
        &claimed.lease_token,
        &cfg(),
        claimed.lease_generation,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, LeaseError::Expired),
        "an expired lease must not be revived even with the right token"
    );
    let expiry_after: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT lease_expires_at FROM delivery.task_run WHERE id = $1")
            .bind(claimed.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        expiry_after < expiry_before,
        "the expired lease was not extended"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn c23_concurrent_renewals_are_monotonic(pool: PgPool) {
    let fx = seed(&pool, "D2-C23").await;
    let claimed = claim_task_run(&pool, fx.project_id, fx.task_id, "codex", None, &cfg())
        .await
        .unwrap();
    let cfg = cfg();
    let (a, b) = tokio::join!(
        renew_lease_by_token(
            &pool,
            fx.project_id,
            claimed.run_id,
            &claimed.lease_token,
            &cfg,
            claimed.lease_generation
        ),
        renew_lease_by_token(
            &pool,
            fx.project_id,
            claimed.run_id,
            &claimed.lease_token,
            &cfg,
            claimed.lease_generation
        ),
    );
    let first = a.expect("concurrent renewal a");
    let second = b.expect("concurrent renewal b");
    let max = first.lease_expires_at.max(second.lease_expires_at);
    let stored: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT lease_expires_at FROM delivery.task_run WHERE id = $1")
            .bind(claimed.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored, max, "the lease never moves backwards");

    let wrong = renew_lease_by_token(
        &pool,
        fx.project_id,
        claimed.run_id,
        "wrong",
        &cfg,
        claimed.lease_generation,
    )
    .await
    .unwrap_err();
    assert!(matches!(wrong, LeaseError::Forbidden));
    let stored_after: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT lease_expires_at FROM delivery.task_run WHERE id = $1")
            .bind(claimed.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored_after, max, "the failing path has no side effects");
}

#[sqlx::test(migrations = "../../migrations")]
async fn c16_dual_entry_concurrency_keeps_one_active_run(pool: PgPool) {
    let fx = seed(&pool, "D2-C16").await;
    let cfg = cfg();
    let (a, b) = tokio::join!(
        claim_task_run(&pool, fx.project_id, fx.task_id, "codex", None, &cfg),
        claim_once(&pool, "worker-1", &cfg, DispatchMode::On),
    );
    let successes = usize::from(a.is_ok()) + usize::from(
        b.as_ref()
            .map(|outcome| matches!(outcome, Some(o) if o.kind == pm_application::run_dispatch::ClaimKind::Claimed))
            .unwrap_or(false),
    );
    assert_eq!(
        successes, 1,
        "exactly one entry point may claim the same task"
    );
    let active: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_run WHERE task_id = $1 AND status IN ('queued','claimed','running')",
    )
    .bind(fx.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        active <= 1,
        "the active-run unique index holds under a dual-entry race"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn c19_reclaim_invalidates_the_old_token(pool: PgPool) {
    let fx = seed(&pool, "D2-C19").await;
    let first = claim_task_run(&pool, fx.project_id, fx.task_id, "codex", None, &cfg())
        .await
        .unwrap();
    // 造过期：Model A 重领走 B3-1（同一 Run 恢复）并签发新 token。
    sqlx::query(
        "UPDATE delivery.task_run SET lease_expires_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(first.run_id)
    .execute(&pool)
    .await
    .unwrap();
    let err = get_run_context(
        &pool,
        fx.project_id,
        first.run_id,
        &first.lease_token,
        first.lease_generation,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, LeaseError::Expired));

    let second = claim_task_run(&pool, fx.project_id, fx.task_id, "codex", None, &cfg())
        .await
        .unwrap();
    assert_eq!(second.run_id, first.run_id, "B3-1 reuses the same run");
    assert_ne!(second.lease_token, first.lease_token);
    // T-032 T2：重领 = 成功获取执行权 → generation 递增；旧 generation 永不授权。
    assert_eq!(
        second.lease_generation,
        first.lease_generation + 1,
        "B3-1 reclaim bumps the lease generation"
    );
    let old = get_run_context(
        &pool,
        fx.project_id,
        first.run_id,
        &first.lease_token,
        first.lease_generation,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(old, LeaseError::Forbidden),
        "the old token is invalidated"
    );
    // 旧 token + 新 generation 同样拒绝（generation 与 token 双验证）。
    let old_gen = get_run_context(
        &pool,
        fx.project_id,
        first.run_id,
        &second.lease_token,
        first.lease_generation,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(old_gen, LeaseError::Forbidden),
        "a stale generation never regains authorization, even with the fresh token"
    );
    get_run_context(
        &pool,
        fx.project_id,
        second.run_id,
        &second.lease_token,
        second.lease_generation,
    )
    .await
    .expect("the new token works");
}

#[sqlx::test(migrations = "../../migrations")]
async fn c12_policy_failure_refuses_the_claim_with_an_audit_snapshot(pool: PgPool) {
    let fx = seed(&pool, "D2-C12").await;
    struct FailingPolicy;
    impl ExecutionPolicy for FailingPolicy {
        fn execution_enabled(&self) -> Result<bool, String> {
            Err("policy backend down".to_string())
        }
    }
    let outcome =
        claim_once_with_policy(&pool, "worker-1", &cfg(), DispatchMode::On, &FailingPolicy)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(
        outcome.kind,
        pm_application::run_dispatch::ClaimKind::Skipped,
        "a failing policy read must refuse the claim, not default to enabled"
    );
    let after: serde_json::Value = sqlx::query_scalar(
        "SELECT after_data FROM governance.audit_log \
          WHERE action = 'skip_task_run' ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after["release_reason"], "policy_unavailable");
    assert_eq!(after["policy_snapshot"]["error"], "policy backend down");
    let run_snapshot: Option<serde_json::Value> =
        sqlx::query_scalar("SELECT policy_snapshot FROM delivery.task_run WHERE id = $1")
            .bind(fx.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        run_snapshot.is_none(),
        "the refusal path must not pollute the run's policy_snapshot"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn c13_project_without_human_members_claims_and_audits_as_service(pool: PgPool) {
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ('no-human', $1) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .fetch_one(&pool)
    .await
    .unwrap();
    let project_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, 'D2-C13', 'no-human', $2) RETURNING id",
    )
    .bind(customer)
    .bind(SYSTEM_TENANT)
    .fetch_one(&pool)
    .await
    .unwrap();
    // 项目没有任何 human 成员行。
    let agent: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('dev_agent', 'executor') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, assignee_id, tenant_id) \
         VALUES ($1, 'no-human', 'ready', $2, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(agent)
    .fetch_one(&pool)
    .await
    .unwrap();
    let repo_id = add_repo(&pool, project_id, "D2-C13").await;
    sqlx::query(
        "INSERT INTO delivery.task_repository (task_id, project_id, repository_id, write_access, base_ref, tenant_id) \
         VALUES ($1, $2, $3, true, 'main', (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(task_id)
    .bind(project_id)
    .bind(repo_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 1, 'queued', (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(task_id)
    .execute(&pool)
    .await
    .unwrap();

    let claimed = claim_task_run(&pool, project_id, task_id, "codex", None, &cfg())
        .await
        .unwrap();
    let actor: Uuid = sqlx::query_scalar(
        "SELECT actor_id FROM governance.audit_log WHERE target_id = $1 AND action = 'claim_task_run'",
    )
    .bind(claimed.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        actor,
        pm_application::run_dispatch::DISPATCH_SERVICE_PRINCIPAL,
        "G6: the audit falls back to the platform service principal"
    );

    // 显式生成的 Run 与 task_run_repository 快照都在，未被跳过。
    let snapshots: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.task_run_repository WHERE run_id = $1")
            .bind(claimed.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(snapshots, 1);
}
