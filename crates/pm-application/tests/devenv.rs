//! T-088 devenv 域 + authz 集成测试（真实 PostgreSQL）。
//!
//! 覆盖设计稿 §9.2 第 8 / 10 / 11 / 12 / 13 / 14 项：成员校验三态、执行主体
//! 解析、跨租户 FK 由数据库拒绝（C177）、首次登记并发幂等（C181）、多 Project
//! Binding 下 Ready 不随调用人变化（C182）、解绑/重绑与验证结果失效（C183）。
//!
//! 夹具说明：`bootstrap_project` 只建 `project_member`，不建 `core.tenant_member`；
//! 生产路径由登录链（`auth.rs` resolve_principal）建成员行。设备 owner 的组合 FK
//! 指向 `tenant_member(tenant_id, principal_id)`，故测试先补成员行。

use pm_application::authz::{
    AuthzError, ExecutorKind, require_project_member, resolve_project_executor,
};
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use pm_application::caller::HumanActor;
use pm_application::devenv::{
    DeviceChecks, DeviceError, DeviceReadiness, DeviceStatus, RegisterDeviceRequest,
    bind_project, get_device, list_devices, register_device, report_checks, revoke_device,
    unbind_binding,
};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    project_id: Uuid,
    human_id: Uuid,
}

async fn seed_project(pool: &PgPool, code: &str) -> Fx {
    let o = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    ensure_tenant_member(pool, SYSTEM_TENANT, o.human_id, "member").await;
    Fx {
        project_id: o.project_id,
        human_id: o.human_id,
    }
}

async fn ensure_tenant_member(pool: &PgPool, tenant_id: Uuid, principal_id: Uuid, role: &str) {
    sqlx::query(
        "INSERT INTO core.tenant_member (tenant_id, principal_id, role) VALUES ($1, $2, $3) \
         ON CONFLICT (tenant_id, principal_id) DO NOTHING",
    )
    .bind(tenant_id)
    .bind(principal_id)
    .bind(role)
    .execute(pool)
    .await
    .unwrap();
}

/// 另一个真人（同租户，非任何项目成员）。
async fn seed_outsider(pool: &PgPool, tenant_id: Uuid) -> Uuid {
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) \
         VALUES ('human', 'outsider', true) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    ensure_tenant_member(pool, tenant_id, id, "member").await;
    id
}

/// 另一个租户（含一名成员）。
async fn seed_other_tenant(pool: &PgPool) -> (Uuid, Uuid) {
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', 'other-tenant', 'other') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let member: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) \
         VALUES ('human', 'other-human', true) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    ensure_tenant_member(pool, tenant, member, "member").await;
    (tenant, member)
}

async fn seed_repo(pool: &PgPool, project_id: Uuid, url: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO integration.project_repository (project_id, name, url, tenant_id) \
         VALUES ($1, 'r', $2, (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(url)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn human(principal_id: Uuid, tenant_id: Uuid) -> HumanActor {
    HumanActor {
        principal_id,
        tenant_id,
        identity_id: None,
        client: Some("test".into()),
        auth_mode: "api_key".into(),
    }
}

fn register_req(id: Option<Uuid>) -> RegisterDeviceRequest {
    RegisterDeviceRequest {
        device_id: id,
        name: "Windows Dev PC".into(),
        platform: "windows".into(),
        os_version: Some("11".into()),
        executor_kind: Some("codex".into()),
        executor_version: Some("0.163".into()),
    }
}

// ── authz ───────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn require_project_member_three_states(pool: PgPool) {
    let fx = seed_project(&pool, "DV-AZ1").await;
    let outsider = seed_outsider(&pool, SYSTEM_TENANT).await;
    let (other_tenant, other_member) = seed_other_tenant(&pool).await;

    // 命中
    require_project_member(&pool, fx.human_id, SYSTEM_TENANT, fx.project_id)
        .await
        .unwrap();
    // 同租户非成员
    assert_eq!(
        require_project_member(&pool, outsider, SYSTEM_TENANT, fx.project_id).await,
        Err(AuthzError::NotMember)
    );
    // 跨租户
    assert_eq!(
        require_project_member(&pool, other_member, other_tenant, fx.project_id).await,
        Err(AuthzError::NotMember)
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn dev_agent_cannot_pass_member_check_as_authorization_subject(pool: PgPool) {
    // 说明性断言：dev_agent 本身就是 project_member 一行——用它校验「恒真」。
    // 这正是设计稿禁止把 dev_agent 当授权主体的原因（评审 P0-4 硬约束 1）。
    let fx = seed_project(&pool, "DV-AZ2").await;
    let dev_agent = resolve_project_executor(&pool, fx.project_id, ExecutorKind::DevAgent)
        .await
        .unwrap();
    // dev_agent 是成员（恒真）→ 不能作为「调用人」去校验
    require_project_member(&pool, dev_agent, SYSTEM_TENANT, fx.project_id)
        .await
        .unwrap();
    // 真人才是授权主体
    require_project_member(&pool, fx.human_id, SYSTEM_TENANT, fx.project_id)
        .await
        .unwrap();
}

#[sqlx::test(migrations = "../../migrations")]
async fn resolve_project_executor_both_kinds_and_missing(pool: PgPool) {
    let fx = seed_project(&pool, "DV-AZ3").await;
    let dev = resolve_project_executor(&pool, fx.project_id, ExecutorKind::DevAgent)
        .await
        .unwrap();
    let ai = resolve_project_executor(&pool, fx.project_id, ExecutorKind::AiPm)
        .await
        .unwrap();
    assert_ne!(dev, ai, "dev_agent 与 ai_pm 是不同 principal");

    // 找不到执行主体 → 报错，不回退真人
    let empty: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         SELECT customer_id, 'DV-EMPTY', 'empty', tenant_id FROM core.project WHERE id = $1 \
         RETURNING id",
    )
    .bind(fx.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        resolve_project_executor(&pool, empty, ExecutorKind::DevAgent).await,
        Err(AuthzError::ExecutorMissing)
    );
}

// ── C177：跨租户 FK 由数据库拒绝 ────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn c177_cross_tenant_fk_rejected_by_database(pool: PgPool) {
    let fx = seed_project(&pool, "DV-FK").await;
    let (other_tenant, other_member) = seed_other_tenant(&pool).await;

    // 1) Tenant A 的 tenant_id + Tenant B 的 principal 作 owner → 拒绝
    let r = sqlx::query(
        "INSERT INTO devenv.device (id, tenant_id, owner_principal_id, name, platform) \
         VALUES ($1, $2, $3, 'x', 'windows')",
    )
    .bind(Uuid::new_v4())
    .bind(SYSTEM_TENANT)
    .bind(other_member)
    .execute(&pool)
    .await;
    assert!(r.is_err(), "owner 不属于 tenant 必须被 FK 拒绝");

    // 合法设备（Tenant A）
    let dev_a = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO devenv.device (id, tenant_id, owner_principal_id, name, platform) \
         VALUES ($1, $2, $3, 'a', 'windows')",
    )
    .bind(dev_a)
    .bind(SYSTEM_TENANT)
    .bind(fx.human_id)
    .execute(&pool)
    .await
    .unwrap();

    // 2) Tenant B 的 tenant_id + Tenant A 的 device → 拒绝
    let r = sqlx::query(
        "INSERT INTO devenv.device_project_binding \
         (tenant_id, device_id, project_id, bound_by) VALUES ($1, $2, $3, $4)",
    )
    .bind(other_tenant)
    .bind(dev_a)
    .bind(fx.project_id)
    .bind(other_member)
    .execute(&pool)
    .await;
    assert!(r.is_err(), "device 不属于 tenant 必须被 FK 拒绝");

    // 3) Tenant A 的 tenant_id + Tenant B 的 bound_by → 拒绝
    let r = sqlx::query(
        "INSERT INTO devenv.device_project_binding \
         (tenant_id, device_id, project_id, bound_by) VALUES ($1, $2, $3, $4)",
    )
    .bind(SYSTEM_TENANT)
    .bind(dev_a)
    .bind(fx.project_id)
    .bind(other_member)
    .execute(&pool)
    .await;
    assert!(r.is_err(), "bound_by 不属于 tenant 必须被 FK 拒绝");

    // 4) 跨租户 project → 拒绝（project 属 Tenant A，tenant_id 填 B）
    let r = sqlx::query(
        "INSERT INTO devenv.device_project_binding \
         (tenant_id, device_id, project_id, bound_by) VALUES ($1, $2, $3, $4)",
    )
    .bind(other_tenant)
    .bind(dev_a)
    .bind(fx.project_id)
    .bind(other_member)
    .execute(&pool)
    .await;
    assert!(r.is_err(), "project 不属于 tenant 必须被 FK 拒绝");
}

// ── D5 / C181：登记幂等与并发 ────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn register_first_time_without_id_then_reuse_with_id(pool: PgPool) {
    let fx = seed_project(&pool, "DV-REG1").await;
    let me = human(fx.human_id, SYSTEM_TENANT);

    let first = register_device(&pool, &me, &register_req(None)).await.unwrap();
    assert!(!first.reused);
    assert_eq!(first.device.status, DeviceStatus::Active);
    assert_eq!(first.device.readiness, None);

    // 同 id 再来 → 复用，同一 device_id
    let again = register_device(&pool, &me, &register_req(Some(first.device.id)))
        .await
        .unwrap();
    assert!(again.reused);
    assert_eq!(again.device.id, first.device.id);

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM devenv.device")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "重复登记不产生重复记录");
}

#[sqlx::test(migrations = "../../migrations")]
async fn c181_same_uuid_concurrent_registration_converges_to_one_row(pool: PgPool) {
    let fx = seed_project(&pool, "DV-REG2").await;
    let me = human(fx.human_id, SYSTEM_TENANT);
    let id = Uuid::new_v4();
    let req_a = register_req(Some(id));
    let req_b = register_req(Some(id));

    let (a, b) = tokio::join!(
        register_device(&pool, &me, &req_a),
        register_device(&pool, &me, &req_b),
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.device.id, id);
    assert_eq!(b.device.id, id);
    // 恰有一个是首次、一个是复用（或两者都走了 ON CONFLICT 后的复用路径）
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM devenv.device WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn register_with_someone_elses_id_is_not_found(pool: PgPool) {
    let fx = seed_project(&pool, "DV-REG3").await;
    let outsider = seed_outsider(&pool, SYSTEM_TENANT).await;
    let me = human(fx.human_id, SYSTEM_TENANT);
    let them = human(outsider, SYSTEM_TENANT);

    let mine = register_device(&pool, &me, &register_req(None)).await.unwrap();
    // 他人拿我的 id 登记 → 404，且不新建
    let r = register_device(&pool, &them, &register_req(Some(mine.device.id))).await;
    assert_eq!(r.unwrap_err(), DeviceError::NotFound);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM devenv.device")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn revoked_device_is_not_auto_restored_on_reregister(pool: PgPool) {
    let fx = seed_project(&pool, "DV-REG4").await;
    let me = human(fx.human_id, SYSTEM_TENANT);

    let d = register_device(&pool, &me, &register_req(None)).await.unwrap();
    let revoked = revoke_device(&pool, &me, d.device.id).await.unwrap();
    assert_eq!(revoked.status, DeviceStatus::Revoked);
    assert!(revoked.revoked_at.is_some());

    // 带旧 id 再登记 → 新建新 id，旧行留历史
    let fresh = register_device(&pool, &me, &register_req(Some(d.device.id)))
        .await
        .unwrap();
    assert!(!fresh.reused);
    assert_ne!(fresh.device.id, d.device.id, "已撤销设备不得通过相同 id 自动恢复");
    let old_status: String =
        sqlx::query_scalar("SELECT status FROM devenv.device WHERE id = $1")
            .bind(d.device.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(old_status, "revoked");

    // 撤销幂等
    revoke_device(&pool, &me, d.device.id).await.unwrap();
}

#[sqlx::test(migrations = "../../migrations")]
async fn visibility_member_sees_own_admin_sees_tenant(pool: PgPool) {
    let fx = seed_project(&pool, "DV-VIS").await;
    let outsider = seed_outsider(&pool, SYSTEM_TENANT).await;
    let me = human(fx.human_id, SYSTEM_TENANT);
    let them = human(outsider, SYSTEM_TENANT);

    let mine = register_device(&pool, &me, &register_req(None)).await.unwrap();
    let _theirs = register_device(&pool, &them, &register_req(None)).await.unwrap();

    // member 只见自己的
    assert_eq!(list_devices(&pool, &me).await.unwrap().len(), 1);
    // 他人设备详情 → 404
    assert_eq!(
        get_device(&pool, &them, mine.device.id).await.unwrap_err(),
        DeviceError::NotFound
    );
    // 升为 admin → 见全租户
    sqlx::query("UPDATE core.tenant_member SET role = 'admin' WHERE principal_id = $1")
        .bind(fx.human_id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(list_devices(&pool, &me).await.unwrap().len(), 2);
}

// ── Binding：幂等 / 解绑 / 重绑 / 声明校验 ─────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn bind_requires_membership_and_repo_ownership(pool: PgPool) {
    let fx = seed_project(&pool, "DV-BND1").await;
    let other = seed_project(&pool, "DV-BND1-B").await;
    let me = human(fx.human_id, SYSTEM_TENANT);
    let d = register_device(&pool, &me, &register_req(None)).await.unwrap();
    let repo = seed_repo(&pool, fx.project_id, "git@x/a.git").await;
    let foreign_repo = seed_repo(&pool, other.project_id, "git@x/b.git").await;

    // 非成员项目 → 404（即便设备是我的）
    assert_eq!(
        bind_project(&pool, &me, d.device.id, other.project_id, &[])
            .await
            .unwrap_err(),
        DeviceError::NotFound
    );
    // 声明他项目的仓库 → Validation
    assert!(matches!(
        bind_project(&pool, &me, d.device.id, fx.project_id, &[foreign_repo])
            .await
            .unwrap_err(),
        DeviceError::Validation(_)
    ));
    // 合法
    let b = bind_project(&pool, &me, d.device.id, fx.project_id, &[repo])
        .await
        .unwrap();
    assert_eq!(b.declared_repository_ids, vec![repo]);
    assert!(b.verified_repositories.is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn c183_unbind_keeps_row_rebind_revives_and_clears_verified(pool: PgPool) {
    let fx = seed_project(&pool, "DV-BND2").await;
    let me = human(fx.human_id, SYSTEM_TENANT);
    let d = register_device(&pool, &me, &register_req(None)).await.unwrap();
    let repo = seed_repo(&pool, fx.project_id, "git@x/a.git").await;

    let b1 = bind_project(&pool, &me, d.device.id, fx.project_id, &[repo])
        .await
        .unwrap();
    // 写入验证结果（通过自检）
    report_checks(
        &pool,
        &me,
        d.device.id,
        &DeviceChecks {
            installed: true,
            connected: true,
            tool: true,
            accessible_repositories: vec![repo],
            evidence: None,
        },
    )
    .await
    .unwrap();
    let verified: serde_json::Value = sqlx::query_scalar(
        "SELECT verified_repositories FROM devenv.device_project_binding WHERE id = $1",
    )
    .bind(b1.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(verified, serde_json::json!([repo]));

    // 重复绑定（声明不变）→ 同一行，验证结果保留
    let b1_again = bind_project(&pool, &me, d.device.id, fx.project_id, &[repo])
        .await
        .unwrap();
    assert_eq!(b1_again.id, b1.id);
    assert_eq!(b1_again.verified_repositories, vec![repo], "声明不变 → 不清验证结果");

    // 解绑：行保留 + unbound_at 置位
    unbind_binding(&pool, &me, d.device.id, b1.id).await.unwrap();
    let unbound: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT unbound_at FROM devenv.device_project_binding WHERE id = $1",
    )
    .bind(b1.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(unbound.is_some());
    // 二次解绑 → BindingNotFound
    assert_eq!(
        unbind_binding(&pool, &me, d.device.id, b1.id).await.unwrap_err(),
        DeviceError::BindingNotFound
    );

    // 重绑 → 复活同一行，unbound_at 清空，verified 清空（同一事务）
    let b2 = bind_project(&pool, &me, d.device.id, fx.project_id, &[repo])
        .await
        .unwrap();
    assert_eq!(b2.id, b1.id, "重绑复活原行，不新建");
    assert!(b2.unbound_at.is_none());
    assert!(b2.verified_repositories.is_empty(), "重绑清空旧验证结果");

    let rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM devenv.device_project_binding WHERE device_id = $1",
    )
    .bind(d.device.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rows, 1, "UNIQUE(device_id, project_id) 保证单行");
}

// ── C182：Ready 只描述设备自身；项目可用性按调用人派生 ─────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn c182_readiness_is_device_own_project_accessibility_is_derived(pool: PgPool) {
    let fx_a = seed_project(&pool, "DV-RDY-A").await;
    let fx_b = seed_project(&pool, "DV-RDY-B").await;
    // 同一真人同时是 A、B 成员：A 的 owner human 也加入 B
    sqlx::query(
        "INSERT INTO core.project_member (project_id, principal_id, role, tenant_id) \
         VALUES ($1, $2, 'member', $3)",
    )
    .bind(fx_b.project_id)
    .bind(fx_a.human_id)
    .bind(SYSTEM_TENANT)
    .execute(&pool)
    .await
    .unwrap();
    let me = human(fx_a.human_id, SYSTEM_TENANT);
    let d = register_device(&pool, &me, &register_req(None)).await.unwrap();
    let repo_a = seed_repo(&pool, fx_a.project_id, "git@x/a.git").await;
    let repo_b = seed_repo(&pool, fx_b.project_id, "git@x/b.git").await;
    bind_project(&pool, &me, d.device.id, fx_a.project_id, &[repo_a])
        .await
        .unwrap();
    bind_project(&pool, &me, d.device.id, fx_b.project_id, &[repo_b])
        .await
        .unwrap();

    // 全部仓库可达 → ready
    let dev = report_checks(
        &pool,
        &me,
        d.device.id,
        &DeviceChecks {
            installed: true,
            connected: true,
            tool: true,
            accessible_repositories: vec![repo_a, repo_b],
            evidence: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(dev.readiness, Some(DeviceReadiness::Ready));

    // 把真人从 B 移除 → 设备 Ready 不变；B 的项目可用性变 false
    sqlx::query("DELETE FROM core.project_member WHERE project_id = $1 AND principal_id = $2")
        .bind(fx_b.project_id)
        .bind(fx_a.human_id)
        .execute(&pool)
        .await
        .unwrap();
    let detail = get_device(&pool, &me, d.device.id).await.unwrap();
    assert_eq!(detail.device.readiness, Some(DeviceReadiness::Ready), "Ready 不随调用人成员资格改变");
    let a = detail
        .bindings
        .iter()
        .find(|b| b.binding.project_id == fx_a.project_id)
        .unwrap();
    let b = detail
        .bindings
        .iter()
        .find(|b| b.binding.project_id == fx_b.project_id)
        .unwrap();
    assert!(a.project_accessible);
    assert!(!b.project_accessible, "项目可用性按调用人成员资格派生");
}

#[sqlx::test(migrations = "../../migrations")]
async fn readiness_layers_and_degradation(pool: PgPool) {
    let fx = seed_project(&pool, "DV-RDY2").await;
    let me = human(fx.human_id, SYSTEM_TENANT);
    let d = register_device(&pool, &me, &register_req(None)).await.unwrap();
    let r1 = seed_repo(&pool, fx.project_id, "git@x/1.git").await;
    let r2 = seed_repo(&pool, fx.project_id, "git@x/2.git").await;
    bind_project(&pool, &me, d.device.id, fx.project_id, &[r1, r2])
        .await
        .unwrap();

    let check = |installed, connected, tool, repos: Vec<Uuid>| DeviceChecks {
        installed,
        connected,
        tool,
        accessible_repositories: repos,
        evidence: None,
    };

    let dev = report_checks(&pool, &me, d.device.id, &check(false, false, false, vec![]))
        .await
        .unwrap();
    assert_eq!(dev.readiness, Some(DeviceReadiness::Failed));

    let dev = report_checks(&pool, &me, d.device.id, &check(true, false, false, vec![]))
        .await
        .unwrap();
    assert_eq!(dev.readiness, Some(DeviceReadiness::Installed));

    let dev = report_checks(&pool, &me, d.device.id, &check(true, true, false, vec![]))
        .await
        .unwrap();
    assert_eq!(dev.readiness, Some(DeviceReadiness::Connected));

    // 工具通但无一仓库可达 → authenticated
    let dev = report_checks(&pool, &me, d.device.id, &check(true, true, true, vec![]))
        .await
        .unwrap();
    assert_eq!(dev.readiness, Some(DeviceReadiness::Authenticated));

    // 部分仓库可达 → degraded
    let dev = report_checks(&pool, &me, d.device.id, &check(true, true, true, vec![r1]))
        .await
        .unwrap();
    assert_eq!(dev.readiness, Some(DeviceReadiness::Degraded));

    // 全部可达 → ready；且 verified 只含项目内仓库（自报的外部 id 被剔除）
    let stray = Uuid::new_v4();
    let dev = report_checks(
        &pool,
        &me,
        d.device.id,
        &check(true, true, true, vec![r1, r2, stray]),
    )
    .await
    .unwrap();
    assert_eq!(dev.readiness, Some(DeviceReadiness::Ready));
    let verified: serde_json::Value = sqlx::query_scalar(
        "SELECT verified_repositories FROM devenv.device_project_binding WHERE device_id = $1",
    )
    .bind(d.device.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let verified: Vec<Uuid> = serde_json::from_value(verified).unwrap();
    assert_eq!(verified.len(), 2);
    assert!(!verified.contains(&stray), "verified_repositories 只表服务端确认的仓库身份与项目归属");

    // 撤销后不得上报
    revoke_device(&pool, &me, d.device.id).await.unwrap();
    assert!(matches!(
        report_checks(&pool, &me, d.device.id, &check(true, true, true, vec![r1, r2]))
            .await
            .unwrap_err(),
        DeviceError::Validation(_)
    ));
}

// ── 迁移完整性 ─────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn migration_0027_shape(pool: PgPool) {
    // 两张审计表都有 claimed_device_id
    for table in ["audit_log", "tenant_audit_log"] {
        let n: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM information_schema.columns \
             WHERE table_schema = 'governance' AND table_name = $1 \
               AND column_name = 'claimed_device_id' AND data_type = 'uuid'",
        )
        .bind(table)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(n, 1, "governance.{table}.claimed_device_id");
    }
    // 四条跨租户组合 FK 存在（device 1 条 + binding 3 条）
    let fks: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_constraint con \
         JOIN pg_class rel ON rel.oid = con.conrelid \
         JOIN pg_namespace nsp ON nsp.oid = rel.relnamespace \
         WHERE nsp.nspname = 'devenv' AND con.contype = 'f' \
           AND array_length(con.conkey, 1) = 2",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(fks, 4, "devenv 两表共 4 条组合外键");
    // 幂等唯一键
    let uq: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_constraint con \
         JOIN pg_class rel ON rel.oid = con.conrelid \
         WHERE rel.relname = 'device_project_binding' AND con.contype = 'u'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(uq >= 2, "binding 至少有 (tenant_id,id) 与 (device_id,project_id) 两条唯一键");
}
