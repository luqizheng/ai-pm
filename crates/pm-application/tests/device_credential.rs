//! V-05-1 Client Registry（0049）集成测试：
//! ACP-460 签发纪律（明文一次性/哈希落库/重复 409）· ACP-461 claim 稳定身份 ·
//! ACP-462 跨重启路由稳定 · ACP-463 回填分类 · ACP-464 权限最小化 ·
//! ACP-465 cpk 失败不回退（解析层） · ACP-466 复合外键拒跨租户 ·
//! ACP-467 撤销后下一请求失效（约束 A/B/C 落地证据）。

use pm_application::actor::AuditActor;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::device_credential::{self, DeviceCredentialError, KNOWN_PERMISSIONS};
use pm_application::repo_access::build_task_repo_access_tx;
use pm_application::run_dispatch::{claim_task_run_as_client, RunDispatchConfig};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

async fn seed(pool: &PgPool, code: &str) -> (Uuid, Uuid, Uuid) {
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
        "INSERT INTO delivery.task (project_id, title, status, revision, tenant_id, assignee_id) \
         VALUES ($1, 't', 'ready', 1, (SELECT tenant_id FROM core.project WHERE id = $1), $2) \
         RETURNING id",
    )
    .bind(outcome.project_id)
    .bind(outcome.dev_agent_id)
    .fetch_one(pool)
    .await
    .unwrap();
    // 策略判定 5：至少一个可写仓库（claim 前置，policy.rs 判定 4/5）。
    let repo_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.project_repository (project_id, name, url, status, head_commit_sha, tenant_id) \
         VALUES ($1, 'web', 'git@host:repo', 'verified', 'headsha', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(outcome.project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_repository (task_id, project_id, repository_id, write_access, base_ref, tenant_id) \
         VALUES ($1, $2, $3, true, 'main', (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(task_id)
    .bind(outcome.project_id)
    .bind(repo_id)
    .execute(pool)
    .await
    .unwrap();
    (outcome.project_id, task_id, outcome.human_id)
}

/// 造一台已登记设备（V-05-1 语义：active + owner）。
async fn seed_device(pool: &PgPool, tenant_id: Uuid, owner: Uuid, name: &str) -> Uuid {
    // fk_device_tenant_owner：owner 必须是租户成员（0027 约束）。
    sqlx::query(
        "INSERT INTO core.tenant_member (tenant_id, principal_id, role) \
         VALUES ($1, $2, 'owner') ON CONFLICT (tenant_id, principal_id) DO NOTHING",
    )
    .bind(tenant_id)
    .bind(owner)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO devenv.device (id, tenant_id, owner_principal_id, name, platform, status) \
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

fn actor(principal_id: Uuid) -> AuditActor {
    AuditActor::principal(principal_id)
}

/// ACP-460：明文一次性（哈希落库、不回显）+ 重复签发 409 + 撤销后可重签。
#[sqlx::test(migrations = "../../migrations")]
async fn acp460_issue_once_revoke_then_rotate(pool: PgPool) {
    let (_, _, human_id) = seed(&pool, "CR-460").await;
    let device_id = seed_device(&pool, SYSTEM_TENANT, human_id, "runner-1").await;
    let a = actor(human_id);

    let issued = device_credential::issue_device_credential(&pool, SYSTEM_TENANT, device_id, &a)
        .await
        .unwrap();
    assert!(issued.plaintext.starts_with("cpk_"));
    assert_eq!(issued.plaintext.len(), 4 + 64, "32-byte random as hex");

    // 落库仅哈希：无明文列；hash 与解析一致性由 resolve 用例覆盖。
    let stored: Option<String> =
        sqlx::query_scalar("SELECT credential_key_hash FROM devenv.device WHERE id = $1")
            .bind(device_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(stored.is_some());
    assert!(
        !stored.as_deref().unwrap().contains("cpk_"),
        "only the hash is stored"
    );

    // 已有未撤销凭据 → 409。
    let again = device_credential::issue_device_credential(&pool, SYSTEM_TENANT, device_id, &a)
        .await
        .unwrap_err();
    assert!(matches!(
        again,
        DeviceCredentialError::ActiveCredentialExists
    ));

    // 撤销（幂等）→ 可重签（轮换两步制）。
    assert!(
        device_credential::revoke_device_credential(&pool, SYSTEM_TENANT, device_id, &a,)
            .await
            .unwrap()
    );
    assert!(
        !device_credential::revoke_device_credential(&pool, SYSTEM_TENANT, device_id, &a,)
            .await
            .unwrap()
    );
    device_credential::issue_device_credential(&pool, SYSTEM_TENANT, device_id, &a)
        .await
        .unwrap();
}

/// 约束 A 四项验证链：hash 不命中/已撤销/device 非 active/owner inactive → 全 None。
#[sqlx::test(migrations = "../../migrations")]
async fn acp465_resolve_validates_all_four_conditions(pool: PgPool) {
    let (_, _, human_id) = seed(&pool, "CR-465").await;
    let device_id = seed_device(&pool, SYSTEM_TENANT, human_id, "runner-a").await;
    let a = actor(human_id);

    let issued = device_credential::issue_device_credential(&pool, SYSTEM_TENANT, device_id, &a)
        .await
        .unwrap();

    // ① 正常解析（granted_permissions 默认三码）。
    let ok = device_credential::resolve(&pool, &issued.plaintext)
        .await
        .unwrap()
        .expect("valid credential must resolve");
    assert_eq!(ok.device_id, device_id);
    assert_eq!(ok.permissions.len(), 3);
    assert!(ok
        .permissions
        .iter()
        .all(|p| KNOWN_PERMISSIONS.contains(&p.as_str())));

    // ② 错 key → None（无 fallback 语义：调用方一律 401）。
    assert!(device_credential::resolve(&pool, "cpk_deadb00f")
        .await
        .unwrap()
        .is_none());
    // 非 cpk 前缀 → None。
    assert!(device_credential::resolve(&pool, "eyJhbGciOi...")
        .await
        .unwrap()
        .is_none());

    // ③ 已撤销 → None（约束 C：撤销即时）。
    device_credential::revoke_device_credential(&pool, SYSTEM_TENANT, device_id, &a)
        .await
        .unwrap();
    assert!(device_credential::resolve(&pool, &issued.plaintext)
        .await
        .unwrap()
        .is_none());

    // ④ device 非 active（revoked 设备）→ None。
    let device2 = seed_device(&pool, SYSTEM_TENANT, human_id, "runner-b").await;
    let issued2 = device_credential::issue_device_credential(&pool, SYSTEM_TENANT, device2, &a)
        .await
        .unwrap();
    sqlx::query("UPDATE devenv.device SET status = 'revoked' WHERE id = $1")
        .bind(device2)
        .execute(&pool)
        .await
        .unwrap();
    assert!(device_credential::resolve(&pool, &issued2.plaintext)
        .await
        .unwrap()
        .is_none());

    // ⑤ owner inactive → None。
    let device3 = seed_device(&pool, SYSTEM_TENANT, human_id, "runner-c").await;
    let issued3 = device_credential::issue_device_credential(&pool, SYSTEM_TENANT, device3, &a)
        .await
        .unwrap();
    sqlx::query("UPDATE core.principal SET is_active = false WHERE id = $1")
        .bind(human_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(device_credential::resolve(&pool, &issued3.plaintext)
        .await
        .unwrap()
        .is_none());
}

/// ACP-461/462：凭据通道 claim → 稳定身份（lease_client_id + client:{id} +
/// execution_mode=spawn）；命令路由跨 poll 稳定。
#[sqlx::test(migrations = "../../migrations")]
async fn acp461_462_client_channel_claim_stable_identity(pool: PgPool) {
    let (project_id, task_id, human_id) = seed(&pool, "CR-461").await;
    let device_id = seed_device(&pool, SYSTEM_TENANT, human_id, "runner-x").await;
    let a = actor(human_id);

    // seed 不建 Run（acp 系列惯例：Run 由用例自建）。
    sqlx::query(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) VALUES ($1, $2, 1, 'queued', (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(task_id)
    .execute(&pool)
    .await
    .unwrap();

    let claimed = claim_task_run_as_client(
        &pool,
        project_id,
        task_id,
        device_id,
        "runner-x",
        None,
        &RunDispatchConfig::from_env(),
        &a,
    )
    .await
    .unwrap();

    // 响应快照 = client:{device_id}。
    assert_eq!(claimed.lease_owner, format!("client:{device_id}"));

    // 库内：lease_client_id + execution_mode=spawn。
    let row: (Uuid, String, String) = sqlx::query_as(
        "SELECT lease_client_id, lease_owner, execution_mode FROM delivery.task_run WHERE id = $1",
    )
    .bind(claimed.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, device_id);
    assert_eq!(row.1, format!("client:{device_id}"));
    assert_eq!(row.2, "spawn");

    // ACP-462：路由稳定——凭据通道可上报介入（spawn 准入），terminate 命令
    // 路由到 client:{id}；同一 worker_id（响应快照）能 poll 到它，无需本地
    // 重建 token8（旧形态的死锁窗口不可再现）。
    pm_application::interaction::report_interaction(
        &pool,
        &pm_application::interaction::ReportInteractionInput {
            project_id,
            task_id,
            run_id: claimed.run_id,
            event_id: "evt-c1".into(),
            reason: "needs_decision".into(),
            title: "t".into(),
            question: "q".into(),
            options: None,
            required_response: false,
            checkpoint_path: None,
        },
        &a,
    )
    .await
    .unwrap();
    let polled = pm_application::worker_command::poll_commands(
        &pool,
        project_id,
        claimed.run_id,
        &claimed.lease_owner,
    )
    .await
    .unwrap();
    assert_eq!(polled.len(), 1);
    assert_eq!(polled[0].command_type, "agent.process.terminate");
}

/// 约束 B：跨租户 lease_client_id 绑定被**复合外键**在库层拒绝（ACP-466）。
#[sqlx::test(migrations = "../../migrations")]
async fn acp466_composite_fk_rejects_cross_tenant_binding(pool: PgPool) {
    let (project_id, task_id, human_id) = seed(&pool, "CR-466A").await;
    let other = seed(&pool, "CR-466B").await;
    // 他租户设备（bootstrap_project 落 SYSTEM_TENANT——同租户；再造一台
    // 独立租户设备须独立租户行：用 system 租户即可——这里直接造第二个项目
    // 的设备，然后试把 A 项目的 Run 指到「另一项目语境」的设备：复合外键
    // 匹配 (tenant_id, id)；同租户下不同项目设备仍可绑（租户级身份语义），
    // 跨租户才拒。构造真正跨租户：手工插一个非 system 租户设备。
    let tenant2: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('test', 't2', 't2') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let foreign_device = seed_device(&pool, tenant2, human_id, "foreign").await;

    // Run 行（system 租户）。
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 1, 'queued', (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let _ = other;

    // 跨租户绑定 → 外键违例（库层拒绝，非应用层软校验）。
    let err = sqlx::query("UPDATE delivery.task_run SET lease_client_id = $1 WHERE id = $2")
        .bind(foreign_device)
        .bind(run_id)
        .execute(&pool)
        .await;
    assert!(
        err.is_err(),
        "composite FK must reject cross-tenant binding"
    );
    let msg = err.unwrap_err().to_string();
    assert!(msg.contains("fk_task_run_client_tenant"), "{msg}");
}

/// ACP-464：granted_permissions 裁剪（值域校验 + 生效）。
#[sqlx::test(migrations = "../../migrations")]
async fn acp464_granted_permissions_can_be_narrowed(pool: PgPool) {
    let (_, _, human_id) = seed(&pool, "CR-464").await;
    let device_id = seed_device(&pool, SYSTEM_TENANT, human_id, "runner-p").await;
    let a = actor(human_id);

    let err = device_credential::set_granted_permissions(
        &pool,
        SYSTEM_TENANT,
        device_id,
        &["PM_MCP_READ".into(), "PM_MCP_BOGUS".into()],
        &a,
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("unknown permission code"));

    let narrowed = device_credential::set_granted_permissions(
        &pool,
        SYSTEM_TENANT,
        device_id,
        &["PM_MCP_READ".into()],
        &a,
    )
    .await
    .unwrap();
    assert_eq!(narrowed, vec!["PM_MCP_READ".to_string()]);

    let resolved = device_credential::resolve(
        &pool,
        &device_credential::issue_device_credential(&pool, SYSTEM_TENANT, device_id, &a)
            .await
            .unwrap()
            .plaintext,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(resolved.permissions, vec!["PM_MCP_READ".to_string()]);
}

/// ACP-463：回填三分类——client:{uuid}（可靠对应）自动回填；mcp:{name}:{token8}
/// 自报形态保持 NULL（legacy 标记谓词）。
#[sqlx::test(migrations = "../../migrations")]
async fn acp463_backfill_classification_predicate(pool: PgPool) {
    let (project_id, task_id, human_id) = seed(&pool, "CR-463").await;
    let device_id = seed_device(&pool, SYSTEM_TENANT, human_id, "runner-bf").await;

    // 新格式（V-05-1 后产生）→ 回填规则命中。
    let run_new: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id, lease_owner) \
         VALUES ($1, $2, 1, 'queued', (SELECT tenant_id FROM core.project WHERE id = $1), $3) \
         RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(format!("client:{device_id}"))
    .fetch_one(&pool)
    .await
    .unwrap();
    // 手动执行迁移内的回填语句（迁移只对既有行跑一次；测试对种子行重放同 SQL）。
    sqlx::query(
        "UPDATE delivery.task_run tr \
            SET lease_client_id = (substring(tr.lease_owner from 8))::uuid \
          WHERE tr.lease_client_id IS NULL \
            AND tr.lease_owner LIKE 'client:%' \
            AND EXISTS (SELECT 1 FROM devenv.device d \
                         WHERE d.id = (substring(tr.lease_owner from 8))::uuid \
                           AND d.tenant_id = tr.tenant_id)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let backfilled: Option<Uuid> =
        sqlx::query_scalar("SELECT lease_client_id FROM delivery.task_run WHERE id = $1")
            .bind(run_new)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(backfilled, Some(device_id));

    // 自报形态 → 不认定（NULL 即 legacy 标记）。非活跃态（uq_task_run_active：
    // 一任务至多一条活跃 Run）。
    let run_legacy: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id, lease_owner) \
         VALUES ($1, $2, 2, 'timed_out', (SELECT tenant_id FROM core.project WHERE id = $1), 'mcp:runner-bf:abcd1234') \
         RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE delivery.task_run tr \
            SET lease_client_id = (substring(tr.lease_owner from 8))::uuid \
          WHERE tr.lease_client_id IS NULL \
            AND tr.lease_owner LIKE 'client:%'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let legacy: Option<Uuid> =
        sqlx::query_scalar("SELECT lease_client_id FROM delivery.task_run WHERE id = $1")
            .bind(run_legacy)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        legacy.is_none(),
        "self-reported client_name must NOT be auto-promoted"
    );

    // 快照读方不受影响（绑定事实与 claim/execution_allows 同表——约束 5 延续）。
    let mut tx = pool.begin().await.unwrap();
    let snaps = build_task_repo_access_tx(&mut tx, SYSTEM_TENANT, project_id, task_id)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let _ = snaps;
}
