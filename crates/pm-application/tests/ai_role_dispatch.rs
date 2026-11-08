//! D-2=A：`resolve_role_assignee` 承接解析的集成测试
//! （`docs/project-ai-team-v2/ai-pm-ai-team-kickoff-adjudication-pack-v1.md` §三）。
//!
//! 覆盖五条裁决语义：
//! ① 有合格 dev_agent 成员 → Some；
//! ② 只有 human 成员 → None；
//! ③ role 行 enabled=false → None；
//! ④ requirement_analyst（V1 无执行体，过渡映射无映射项）→ None；
//! ⑤ 空闲优先（活跃 delivery run 归因 = task.assignee_id），都忙时取最小
//!    principal_id（确定性）。

use pm_application::ai_roles::resolve_role_assignee;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

/// bootstrap 后的 developer role 版本 id（bootstrap 种子：5 角色全 enabled + v1.0）。
async fn role_version_id(pool: &PgPool, project_id: Uuid, role_key: &str) -> Uuid {
    sqlx::query_scalar(
        "SELECT v.id \
           FROM ai.ai_role_version v \
           JOIN ai.project_ai_role pr ON pr.id = v.project_role_id \
           JOIN ai.ai_role_template t ON t.id = pr.role_id \
          WHERE pr.project_id = $1 AND t.role_key = $2 AND v.version = '1.0'",
    )
    .bind(project_id)
    .bind(role_key)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 再授权一个 dev_agent principal 进项目（bootstrap 只给每角色一个实例；
/// 选取规则的「空闲优先 / 平局取小」需要 ≥2 候选才能区分）。
async fn add_dev_agent_member(pool: &PgPool, project_id: Uuid, role: &str) -> Uuid {
    let tenant_id: Uuid =
        sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
            .bind(project_id)
            .fetch_one(pool)
            .await
            .unwrap();
    let principal: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) \
         VALUES ('dev_agent', $1) RETURNING id",
    )
    .bind(format!("{role}-extra-{project_id}"))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(principal)
    .bind(role)
    .execute(pool)
    .await
    .unwrap();
    principal
}

/// 把某个 principal 标记为「忙」：task.assignee_id + 一条活跃 delivery run。
async fn make_busy(pool: &PgPool, project_id: Uuid, assignee: Uuid, code: &str) {
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, assignee_id, tenant_id) \
         VALUES ($1, $2, 'ready', 1, $3, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(code.to_string())
    .bind(assignee)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, run_kind, tenant_id) \
         VALUES ($1, $2, 1, 'claimed', 'delivery', \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(task_id)
    .execute(pool)
    .await
    .unwrap();
}

/// 手工造一个只有 human 成员的项目 + developer role 版本（不走 bootstrap：
/// 懒生效路径，§② 只有人、没有合格 dev_agent 的场景）。
async fn seed_human_only_project(pool: &PgPool, external_key: &str, code: &str) -> (Uuid, Uuid) {
    let tenant_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', $1, $1) RETURNING id",
    )
    .bind(external_key)
    .fetch_one(pool)
    .await
    .unwrap();
    let customer_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, name) VALUES ($1, 'c') RETURNING id",
    )
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let project_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name, status) \
         VALUES ($1, $2, $3, $3, 'planning') RETURNING id",
    )
    .bind(tenant_id)
    .bind(customer_id)
    .bind(code)
    .fetch_one(pool)
    .await
    .unwrap();
    let human: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) \
         VALUES ('human', 'dev-in-name-only', true) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ($1, $2, $3, 'developer')",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(human)
    .execute(pool)
    .await
    .unwrap();

    // developer role：懒生效 = 项目行 + v1.0（与 set_project_role_enabled 同构）。
    let version_id: Uuid = sqlx::query_scalar(
        "WITH row AS ( \
             INSERT INTO ai.project_ai_role (tenant_id, project_id, role_id, enabled) \
             SELECT $1, $2, t.id, TRUE FROM ai.ai_role_template t \
              WHERE t.role_key = 'developer' RETURNING id \
         ) \
         INSERT INTO ai.ai_role_version (project_role_id, version, definition) \
         SELECT row.id, '1.0', t.default_definition \
           FROM row, ai.ai_role_template t WHERE t.role_key = 'developer' \
         RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    (project_id, version_id)
}

#[sqlx::test(migrations = "../../migrations")]
async fn resolves_qualified_dev_agent_for_developer_role(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "D2A-1".into(),
            project_name: "d".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let version = role_version_id(&pool, boot.project_id, "developer").await;

    let resolved = resolve_role_assignee(&pool, SYSTEM_TENANT, boot.project_id, version)
        .await
        .unwrap();
    assert_eq!(
        resolved,
        Some(boot.developer_id),
        "the qualified dev_agent member must be resolved as assignee"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn returns_none_when_project_has_only_human_members(pool: PgPool) {
    let (project_id, version) = seed_human_only_project(&pool, "d2a-h1", "D2A-2").await;

    let resolved = resolve_role_assignee(&pool, SYSTEM_TENANT, project_id, version)
        .await
        .unwrap();
    assert_eq!(
        resolved, None,
        "a human member in the mapped role is NOT a qualified executor"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn returns_none_when_role_row_is_disabled(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "D2A-3".into(),
            project_name: "d".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let version = role_version_id(&pool, boot.project_id, "developer").await;
    sqlx::query(
        "UPDATE ai.project_ai_role SET enabled = FALSE \
          WHERE project_id = $1 \
            AND role_id = (SELECT id FROM ai.ai_role_template WHERE role_key = 'developer')",
    )
    .bind(boot.project_id)
    .execute(&pool)
    .await
    .unwrap();

    let resolved = resolve_role_assignee(&pool, SYSTEM_TENANT, boot.project_id, version)
        .await
        .unwrap();
    assert_eq!(
        resolved, None,
        "a disabled project role row must yield no executor"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn returns_none_for_requirement_analyst_role(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "D2A-4".into(),
            project_name: "d".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let version = role_version_id(&pool, boot.project_id, "requirement_analyst").await;

    let resolved = resolve_role_assignee(&pool, SYSTEM_TENANT, boot.project_id, version)
        .await
        .unwrap();
    assert_eq!(
        resolved, None,
        "requirement_analyst has no executor mapping in V1 (P2-13)"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn prefers_idle_candidate_then_smallest_principal_id(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "D2A-5".into(),
            project_name: "d".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let version = role_version_id(&pool, boot.project_id, "developer").await;
    let extra = add_dev_agent_member(&pool, boot.project_id, "developer").await;
    let (busy, idle) = (boot.developer_id, extra);

    // ① busy 有活跃 delivery run、idle 全闲 → 无论 uuid 序，必选 idle。
    make_busy(&pool, boot.project_id, busy, "D2A-5-busy-A").await;
    let resolved = resolve_role_assignee(&pool, SYSTEM_TENANT, boot.project_id, version)
        .await
        .unwrap();
    assert_eq!(resolved, Some(idle), "the idle candidate must win");

    // ② 两个都忙 → 平局取最小 principal_id（确定性）。
    make_busy(&pool, boot.project_id, idle, "D2A-5-busy-B").await;
    let resolved = resolve_role_assignee(&pool, SYSTEM_TENANT, boot.project_id, version)
        .await
        .unwrap();
    assert_eq!(
        resolved,
        Some(busy.min(idle)),
        "all-busy tie must deterministically pick the smallest principal_id"
    );

    // ③ run 落终态（脱离活跃集合）后，busy 判据随之解除 → 该候选重新可选。
    sqlx::query(
        "UPDATE delivery.task_run SET status = 'succeeded' \
          WHERE project_id = $1 AND task_id = ( \
              SELECT id FROM delivery.task WHERE project_id = $1 AND assignee_id = $2)",
    )
    .bind(boot.project_id)
    .bind(busy)
    .execute(&pool)
    .await
    .unwrap();
    let resolved = resolve_role_assignee(&pool, SYSTEM_TENANT, boot.project_id, version)
        .await
        .unwrap();
    // busy 已空闲、另一候选仍忙 → 空闲者重新当选（判据随活跃 run 起落）。
    assert_eq!(resolved, Some(busy), "terminal runs free the candidate");
}

/// 兜底：跨项目引用（版本挂在别的项目）→ None（不泄露存在性）。
#[sqlx::test(migrations = "../../migrations")]
async fn returns_none_for_version_anchored_to_another_project(pool: PgPool) {
    let boot_a = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "D2A-6A".into(),
            project_name: "d".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let boot_b = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "D2A-6B".into(),
            project_name: "d".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let version_a = role_version_id(&pool, boot_a.project_id, "developer").await;

    let resolved = resolve_role_assignee(&pool, SYSTEM_TENANT, boot_b.project_id, version_a)
        .await
        .unwrap();
    assert_eq!(
        resolved, None,
        "a version from another project must never resolve in this project"
    );
}
