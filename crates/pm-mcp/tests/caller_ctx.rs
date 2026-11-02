//! T-088 MCP 调用人上下文验收（C173 / C176 / C178）。
//!
//! 直调工具函数层（HTTP 侧的中间件行为已在 `apps/pm-api/tests/pm_devices.rs`
//! 覆盖；C175 的端到端注入证据在 `tests/tools.rs` 的 smoke 测试）。
//!
//! - **C173**：成员校验覆盖工具家族——非成员 → `ToolError::NotFound`（防 ID
//!   探测），成员 → 通过授权门（工具按业务语义继续）；越权请求零副作用；
//! - **C176**：`DeviceClaim` 有效 → `task_run.executor_id` 落设备 id（与审计
//!   `claimed_device_id` 同源同值）；忽略类 → 不落且不影响执行；
//! - **C178**：真人成员校验先行；执行主体缺失 → 报错**不回退真人**；
//!   非成员带有效 lease 也过不了成员门。

use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use pm_application::caller::{CallerContext, DeviceClaim, DeviceClaimIgnoredReason, HumanActor};
use pm_domain::SYSTEM_TENANT;
use pm_mcp::tools::{
    ToolError, claim_task, get_run_context, get_submission_status, get_task_context,
    report_progress, submit_technical_design,
};
use pm_mcp::tools::{
    ClaimTaskArgs, ReportProgressArgs, RunContextArgs, SubmissionStatusArgs,
    SubmitTechnicalDesignArgs, TaskContextArgs,
};
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    /// 项目 owner（成员）。
    member: Uuid,
    member_identity: Uuid,
    /// 同租户但非任何项目成员。
    outsider: Uuid,
    outsider_identity: Uuid,
    project_a: Uuid,
    project_b: Uuid,
    task_a: Uuid,
}

async fn seed_fixture(pool: &PgPool) -> Fx {
    let a = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: "CC-A".into(),
            project_name: "CC-A".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let b = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: "CC-B".into(),
            project_name: "CC-B".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let outsider: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) \
         VALUES ('human', 'outsider', true) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let outsider_identity = seed_identity(pool, SYSTEM_TENANT, "cc-outsider", outsider).await;
    let member_identity = seed_identity(pool, SYSTEM_TENANT, "cc-member", a.human_id).await;
    // 设备 owner 的组合 FK 指向 tenant_member（bootstrap 不建该行，生产由登录链建）
    sqlx::query(
        "INSERT INTO core.tenant_member (tenant_id, principal_id, role)          VALUES ($1, $2, 'member') ON CONFLICT (tenant_id, principal_id) DO NOTHING",
    )
    .bind(SYSTEM_TENANT)
    .bind(a.human_id)
    .execute(pool)
    .await
    .unwrap();
    let task_a = seed_claimable_task(pool, a.project_id).await;
    Fx {
        member: a.human_id,
        member_identity,
        outsider,
        outsider_identity,
        project_a: a.project_id,
        project_b: b.project_id,
        task_a,
    }
}

/// 种一个**可领取**的任务：ready + 指派给项目 dev_agent + 可写仓库契约
/// （与 `tests/contract.rs` 的种子同构——claim 政策的判定 4 / 至少一可写仓）。
async fn seed_claimable_task(pool: &PgPool, project_id: Uuid) -> Uuid {
    let agent_id: Uuid = sqlx::query_scalar(
        "SELECT pm.principal_id FROM core.project_member pm            JOIN core.principal p ON p.id = pm.principal_id           WHERE pm.project_id = $1 AND p.kind = 'dev_agent' LIMIT 1",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, assignee_id, tenant_id)          VALUES ($1, 't', 'ready', 1, $2, (SELECT tenant_id FROM core.project WHERE id = $1))          RETURNING id",
    )
    .bind(project_id)
    .bind(agent_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let repo_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.project_repository          (project_id, name, url, status, head_commit_sha, tenant_id)          VALUES ($1, 'r', 'git@host:cc', 'verified', 'headsha',                  (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_repository          (task_id, project_id, repository_id, write_access, base_ref, path_scope, tenant_id)          VALUES ($1, $2, $3, true, 'main', ARRAY['**'],                  (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(task_id)
    .bind(project_id)
    .bind(repo_id)
    .execute(pool)
    .await
    .unwrap();
    task_id
}

/// 种一条 `queued` Run（TaskReady 的产物；直领用它）。
async fn seed_run(pool: &PgPool, project_id: Uuid, task_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id)          VALUES ($1, $2, 1, 'queued', (SELECT tenant_id FROM core.project WHERE id = $1))          RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 种一条真实的 `core.principal_identity`（audit_log.actor_identity_id 有 FK）。
async fn seed_identity(pool: &PgPool, tenant_id: Uuid, subject: &str, principal_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.principal_identity (provider, tenant_id, subject, principal_id) \
         VALUES ('lmms', $1, $2, $3) \
         ON CONFLICT (provider, tenant_id, subject) DO UPDATE SET principal_id = $3 \
         RETURNING id"
    )
    .bind(tenant_id)
    .bind(subject)
    .bind(principal_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn caller(principal_id: Uuid, identity_id: Uuid, device: DeviceClaim) -> CallerContext {
    CallerContext::new(
        HumanActor {
            principal_id,
            tenant_id: SYSTEM_TENANT,
            identity_id: Some(identity_id),
            client: Some("codex".into()),
            auth_mode: "api_key".into(),
        },
        device,
    )
}

fn member_caller(human_id: Uuid, identity_id: Uuid) -> CallerContext {
    caller(human_id, identity_id, DeviceClaim::absent())
}

// ── C173：成员校验覆盖 + 零副作用 ───────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn c173_non_member_is_not_found_across_tool_families(pool: PgPool) {
    let fx = seed_fixture(&pool).await;
    let intruder = caller(fx.outsider, fx.outsider_identity, DeviceClaim::absent());
    let member = member_caller(fx.member, fx.member_identity);

    // 任务上下文（读）
    let err = get_task_context(
        &pool,
        TaskContextArgs {
            project_id: fx.project_a,
            task_ref: fx.task_a.to_string(),
        },
        &intruder,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::NotFound(_)), "got {err:?}");

    // 技术设计提交（写）——非成员 404 且不产生事件
    let err = submit_technical_design(
        &pool,
        SubmitTechnicalDesignArgs {
            project_id: fx.project_a,
            task_ref: fx.task_a.to_string(),
            idempotency_key: "cc-1".into(),
            fixture: "cc".into(),
            summary: "design".into(),
        },
        &intruder,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::NotFound(_)), "got {err:?}");
    let events: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.inbound_event WHERE project_id = $1",
    )
    .bind(fx.project_a)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(events, 0, "越权请求不产生业务副作用");

    // 领取（T-051 关闭点）
    let err = claim_task(
        &pool,
        ClaimTaskArgs {
            project_id: fx.project_a,
            task_id: fx.task_a,
            client_name: "codex".into(),
            client_version: None,
        },
        &intruder,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::NotFound(_)), "got {err:?}");
    let runs: i64 = sqlx::query_scalar("SELECT count(*) FROM delivery.task_run")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(runs, 0, "越权领取不产生 Run");

    // 成员 → 通过授权门（读工具正常返回）
    let ok = get_task_context(
        &pool,
        TaskContextArgs {
            project_id: fx.project_a,
            task_ref: fx.task_a.to_string(),
        },
        &member,
    )
    .await
    .unwrap();
    assert_eq!(
        ok["task_id"],
        serde_json::Value::String(fx.task_a.to_string())
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn c173_get_submission_status_resolves_project_from_event(pool: PgPool) {
    let fx = seed_fixture(&pool).await;
    let member = member_caller(fx.member, fx.member_identity);
    let intruder = caller(fx.outsider, fx.outsider_identity, DeviceClaim::absent());

    let out = submit_technical_design(
        &pool,
        SubmitTechnicalDesignArgs {
            project_id: fx.project_a,
            task_ref: fx.task_a.to_string(),
            idempotency_key: "cc-evt".into(),
            fixture: "cc".into(),
            summary: "design".into(),
        },
        &member,
    )
    .await
    .unwrap();
    let event_id: Uuid = out["event_id"].as_str().unwrap().parse().unwrap();

    // 成员读自己项目的事件 → 正常
    let ok = get_submission_status(&pool, SubmissionStatusArgs { event_id }, &member)
        .await
        .unwrap();
    assert!(ok.is_object());

    // 非成员读他项目事件 → 404（由 inbound_event 反查项目后校验）
    let err = get_submission_status(&pool, SubmissionStatusArgs { event_id }, &intruder)
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::NotFound(_)), "got {err:?}");
}

// ── C176：设备声明（有效落 executor_id；忽略类不落且不影响执行）────────

#[sqlx::test(migrations = "../../migrations")]
async fn c176_device_claim_valid_lands_executor_id_and_audit(pool: PgPool) {
    let fx = seed_fixture(&pool).await;
    // 先真实登记设备（claimed_device_id 有 FK——生产上 attach_device_claim
    // 只放行已登记设备，这里同构）。
    let me = fx_member_human(&fx);
    let registered = pm_application::devenv::register_device(
        &pool,
        &me,
        &pm_application::devenv::RegisterDeviceRequest {
            device_id: None,
            name: "PC".into(),
            platform: "windows".into(),
            os_version: None,
            executor_kind: Some("codex".into()),
            executor_version: None,
        },
    )
    .await
    .unwrap();
    let device_id = registered.device.id;
    let valid = caller(fx.member, fx.member_identity, DeviceClaim::valid(device_id));
    let _run = seed_run(&pool, fx.project_a, fx.task_a).await;

    let claimed = claim_task(
        &pool,
        ClaimTaskArgs {
            project_id: fx.project_a,
            task_id: fx.task_a,
            client_name: "codex".into(),
            client_version: None,
        },
        &valid,
    )
    .await
    .unwrap();
    let run_id: Uuid = claimed["run_id"].as_str().unwrap().parse().unwrap();

    // task_run.executor_id = 已校验设备 id（同源同值）
    let executor_id: Option<Uuid> =
        sqlx::query_scalar("SELECT executor_id FROM delivery.task_run WHERE id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        executor_id,
        Some(device_id),
        "有效声明落 executor_id（ADR-0001 D6 前向列，仅归因）"
    );

    // claim 审计行的 claimed_device_id 同值；真人三列齐备
    let (audit_device, identity_id, client, auth_mode): (Option<Uuid>, Option<Uuid>, Option<String>, Option<String>) =
        sqlx::query_as(
            "SELECT claimed_device_id, actor_identity_id, actor_client, actor_auth_mode \
             FROM governance.audit_log WHERE action = 'claim_task_run' AND target_id = $1",
        )
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(audit_device, Some(device_id), "审计 claimed_device_id 同源同值");
    assert_eq!(identity_id, member_identity_of(&valid), "真人 identity 列");
    assert_eq!(client.as_deref(), Some("codex"));
    assert_eq!(auth_mode.as_deref(), Some("api_key"));

    // actor_id = 当前 Project 的 dev_agent（C178 任务类口径）
    let dev_agent: Uuid = pm_application::authz::resolve_project_executor(
        &pool,
        fx.project_a,
        pm_application::authz::ExecutorKind::DevAgent,
    )
    .await
    .unwrap();
    let audit_actor: Uuid = sqlx::query_scalar(
        "SELECT actor_id FROM governance.audit_log \
          WHERE action = 'claim_task_run' AND target_id = $1",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit_actor, dev_agent, "执行主体 = 当前 Project 的 dev_agent");
}

fn member_identity_of(caller: &CallerContext) -> Option<Uuid> {
    caller.human.identity_id
}

fn fx_member_human(fx: &Fx) -> HumanActor {
    HumanActor {
        principal_id: fx.member,
        tenant_id: SYSTEM_TENANT,
        identity_id: Some(fx.member_identity),
        client: Some("codex".into()),
        auth_mode: "api_key".into(),
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn c176_ignored_claim_still_executes_without_attribution(pool: PgPool) {
    let fx = seed_fixture(&pool).await;
    // not_found 声明（pm-api 校验后的结果形态）
    let ignored = caller(fx.member, fx.member_identity, DeviceClaim::ignored(DeviceClaimIgnoredReason::NotFound));
    let _run = seed_run(&pool, fx.project_a, fx.task_a).await;

    let claimed = claim_task(
        &pool,
        ClaimTaskArgs {
            project_id: fx.project_a,
            task_id: fx.task_a,
            client_name: "codex".into(),
            client_version: None,
        },
        &ignored,
    )
    .await
    .unwrap();
    let run_id: Uuid = claimed["run_id"].as_str().unwrap().parse().unwrap();

    let executor_id: Option<Uuid> =
        sqlx::query_scalar("SELECT executor_id FROM delivery.task_run WHERE id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(executor_id, None, "忽略声明不落设备归因");
    let audit_device: Option<Uuid> = sqlx::query_scalar(
        "SELECT claimed_device_id FROM governance.audit_log \
          WHERE action = 'claim_task_run' AND target_id = $1",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit_device, None);
}

// ── C178 补充：执行主体缺失即报错；Run 工具也过成员门 ───────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn c178_missing_executor_errors_instead_of_falling_back(pool: PgPool) {
    // 建一个没有任何成员/执行主体的空项目
    let fx = seed_fixture(&pool).await;
    let customer: Uuid = sqlx::query_scalar(
        "SELECT customer_id FROM core.project WHERE id = $1",
    )
    .bind(fx.project_a)
    .fetch_one(&pool)
    .await
    .unwrap();
    let empty_project: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name, status) \
         SELECT tenant_id, customer_id, 'CC-EMPTY', 'empty', 'planning' \
         FROM core.project WHERE id = $1 RETURNING id",
    )
    .bind(fx.project_a)
    .fetch_one(&pool)
    .await
    .unwrap();
    // 把 owner human 加为 empty_project 的成员（通过成员门，卡在执行主体缺失）
    sqlx::query(
        "INSERT INTO core.project_member (project_id, principal_id, role, tenant_id) \
         SELECT id, $2, 'owner', tenant_id FROM core.project WHERE id = $1",
    )
    .bind(empty_project)
    .bind(fx.member)
    .execute(&pool)
    .await
    .unwrap();

    let member = member_caller(fx.member, fx.member_identity);
    let err = get_task_context(
        &pool,
        TaskContextArgs {
            project_id: empty_project,
            task_ref: Uuid::new_v4().to_string(),
        },
        &member,
    )
    .await;
    // get_task_context 是只读工具（ensure_member，不取执行主体）——通过门后按
    // 任务不存在报 NotFound。真正断言「不回退」的是取执行主体的写工具：
    let err = submit_technical_design(
        &pool,
        SubmitTechnicalDesignArgs {
            project_id: empty_project,
            task_ref: Uuid::new_v4().to_string(),
            idempotency_key: "cc-empty".into(),
            fixture: "cc".into(),
            summary: "design".into(),
        },
        &member,
    )
    .await
    .unwrap_err();
    let text = err.to_string();
    assert!(
        text.contains("no dev_agent principal") || text.contains("InvalidRequest"),
        "执行主体缺失应报错而非回退真人：{text}"
    );
    let _ = customer;
}

#[sqlx::test(migrations = "../../migrations")]
async fn run_tools_still_gate_on_membership(pool: PgPool) {
    let fx = seed_fixture(&pool).await;
    let member = member_caller(fx.member, fx.member_identity);

    // 成员领取
    let _run = seed_run(&pool, fx.project_a, fx.task_a).await;
    let claimed = claim_task(
        &pool,
        ClaimTaskArgs {
            project_id: fx.project_a,
            task_id: fx.task_a,
            client_name: "codex".into(),
            client_version: None,
        },
        &member,
    )
    .await
    .unwrap();
    let run_id: Uuid = claimed["run_id"].as_str().unwrap().parse().unwrap();
    let lease_token: String = claimed["lease_token"].as_str().unwrap().into();
    let generation: i32 = claimed["lease_generation"].as_i64().unwrap() as i32;

    // 成员续租正常（经 report_progress_as 落真人审计）
    let renewed = report_progress(
        &pool,
        ReportProgressArgs {
            project_id: fx.project_a,
            run_id,
            lease_token: lease_token.clone(),
            lease_generation: generation,
            stage: "started".into(),
            note: None,
            blocked: false,
        },
        &member,
    )
    .await
    .unwrap();
    assert_eq!(
        renewed["run_id"],
        serde_json::Value::String(run_id.to_string())
    );

    // 非成员带同 token 读 run 上下文 → 404（成员资格先于 lease 校验）
    let intruder = caller(fx.outsider, fx.outsider_identity, DeviceClaim::absent());
    let err = get_run_context(
        &pool,
        RunContextArgs {
            project_id: fx.project_a,
            run_id,
            lease_token,
            lease_generation: generation,
        },
        &intruder,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::NotFound(_)), "got {err:?}");

    // report_progress 的审计行：actor_id = dev_agent、claimed_device_id = NULL（缺席）
    let (actor_id, audit_device): (Uuid, Option<Uuid>) = sqlx::query_as(
        "SELECT actor_id, claimed_device_id FROM governance.audit_log \
          WHERE action = 'renew_run_lease' AND target_id = $1 \
          ORDER BY created_at DESC LIMIT 1",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let dev_agent: Uuid = pm_application::authz::resolve_project_executor(
        &pool,
        fx.project_a,
        pm_application::authz::ExecutorKind::DevAgent,
    )
    .await
    .unwrap();
    assert_eq!(actor_id, dev_agent);
    assert_eq!(audit_device, None);
}
