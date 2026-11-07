//! WP-C：Task ↔ Repository 绑定生产写入口（六约束逐条钉住）。
//!
//! 约束 → 测试映射：
//! 1 关联只来自本项目有效仓（verified） → `bind_requires_verified_repository`
//! 2 创建时校验既有授权（仓库管理面 verified） → 同上 + 角色域
//! 3 不允许跨 Project 写入 → `bind_rejects_cross_project_repository`
//! 4 重复写入幂等 → `bind_is_idempotent_converging_upsert` / `unbind_is_idempotent`
//! 5 Claim 与 execution_allows 同一关联事实 → `claim_snapshot_and_execution_allows_consume_binding`
//! 6 禁 MCP 参数伪造归属 → 同上（未绑定仓库 deny）+ `bind_rejects_unknown_repository`

use pm_application::actor::AuditActor;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use pm_application::repo_access::{build_task_repo_access_tx, execution_allows};
use pm_application::task_repository_binding::{
    self, BindRepositoryEntry, TaskRepoBindingError,
};
use pm_domain::{RepoAccess, SYSTEM_TENANT};
use sqlx::PgPool;
use uuid::Uuid;

async fn seed_project_task(pool: &PgPool, code: &str) -> (Uuid, Uuid, Uuid) {
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
        "INSERT INTO delivery.task (project_id, title, status, revision, tenant_id) \
         VALUES ($1, 't', 'ready', 1, (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(outcome.project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    (outcome.project_id, task_id, outcome.human_id)
}

/// 在指定项目下造一个仓库（status 可指定；对齐 0008/0009 形状）。
async fn seed_repository(pool: &PgPool, project_id: Uuid, name: &str, status: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO integration.project_repository \
           (project_id, name, url, status, head_commit_sha, tenant_id) \
         VALUES ($1, $2, $3, $4, 'headsha', (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(project_id)
    .bind(name)
    .bind(format!("git@host:{name}"))
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn entry(repository_id: Uuid) -> BindRepositoryEntry {
    BindRepositoryEntry {
        repository_id,
        write_access: None,
        base_ref: None,
        role: None,
        path_scope: None,
    }
}

async fn audit_count(pool: &PgPool, project_id: Uuid, action: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM governance.audit_log WHERE project_id = $1 AND action = $2")
        .bind(project_id)
        .bind(action)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// 约束 4：重复写入幂等——同 (task, repo) 收敛更新，不产重复行；
/// 未提供字段保留现值；审计逐次留痕。
#[sqlx::test(migrations = "../../migrations")]
async fn bind_is_idempotent_converging_upsert(pool: PgPool) {
    let (project_id, task_id, human_id) = seed_project_task(&pool, "TRB-IDEM").await;
    let repo_a = seed_repository(&pool, project_id, "web", "verified").await;
    let actor = AuditActor::principal(human_id);

    // 首绑：默认值（write_access=true / base_ref='main' / role='primary'）。
    let bound = task_repository_binding::bind_task_repositories(
        &pool, SYSTEM_TENANT, project_id, task_id, &[entry(repo_a)], &actor,
    )
    .await
    .unwrap();
    assert_eq!(bound.len(), 1);
    assert_eq!(bound[0].repository_name, "web");
    assert!(bound[0].write_access);
    assert_eq!(bound[0].base_ref, "main");
    assert_eq!(bound[0].role, "primary");
    assert!(bound[0].repository_verified);

    // 重复绑定同仓库（部分字段）：收敛更新而非新行；未提供字段保留现值。
    let rebound = task_repository_binding::bind_task_repositories(
        &pool,
        SYSTEM_TENANT,
        project_id,
        task_id,
        &[BindRepositoryEntry {
            repository_id: repo_a,
            write_access: Some(false),
            base_ref: None,     // 保留 'main'
            role: Some("secondary".into()),
            path_scope: None,
        }],
        &actor,
    )
    .await
    .unwrap();
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.task_repository WHERE task_id = $1")
            .bind(task_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1, "idempotent upsert must not duplicate rows");
    assert!(!rebound[0].write_access, "provided field must converge");
    assert_eq!(rebound[0].base_ref, "main", "omitted field must keep current value");
    assert_eq!(rebound[0].role, "secondary");

    // 多仓一次绑：两个独立行。
    let repo_b = seed_repository(&pool, project_id, "docs", "verified").await;
    let both = task_repository_binding::bind_task_repositories(
        &pool,
        SYSTEM_TENANT,
        project_id,
        task_id,
        &[entry(repo_a), entry(repo_b)],
        &actor,
    )
    .await
    .unwrap();
    assert_eq!(both.len(), 2);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.task_repository WHERE task_id = $1")
            .bind(task_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 2);

    assert!(audit_count(&pool, project_id, "task.repository.bound").await >= 4);
}

/// 约束 1/2：只有 verified 仓库可绑定；角色域受 CHECK 语义约束。
#[sqlx::test(migrations = "../../migrations")]
async fn bind_requires_verified_repository(pool: PgPool) {
    let (project_id, task_id, human_id) = seed_project_task(&pool, "TRB-VER").await;
    let pending = seed_repository(&pool, project_id, "pending-repo", "pending").await;
    let actor = AuditActor::principal(human_id);

    let err = task_repository_binding::bind_task_repositories(
        &pool, SYSTEM_TENANT, project_id, task_id, &[entry(pending)], &actor,
    )
    .await
    .unwrap_err();
    assert!(matches!(
        err,
        TaskRepoBindingError::RepositoryNotVerified(ref s) if s == "pending"
    ));
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.task_repository WHERE task_id = $1")
            .bind(task_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0);

    // 角色域：CHECK 三值之外的 role 在写入前被拒绝（约束 2）。
    let ok_repo = seed_repository(&pool, project_id, "main-repo", "verified").await;
    let bad_role = task_repository_binding::bind_task_repositories(
        &pool,
        SYSTEM_TENANT,
        project_id,
        task_id,
        &[BindRepositoryEntry {
            repository_id: ok_repo,
            write_access: None,
            base_ref: None,
            role: Some("owner".into()),
            path_scope: None,
        }],
        &actor,
    )
    .await
    .unwrap_err();
    assert!(matches!(bad_role, TaskRepoBindingError::InvalidRole(_)));
}

/// 约束 3：仓库存在但属其他项目 → REPOSITORY_NOT_IN_PROJECT；未知仓 → NotFound；
/// 未知任务 → TaskNotFound（不泄露存在性）。
#[sqlx::test(migrations = "../../migrations")]
async fn bind_rejects_cross_project_repository(pool: PgPool) {
    let (project_id, task_id, human_id) = seed_project_task(&pool, "TRB-XA").await;
    let (other_project, _, _) = seed_project_task(&pool, "TRB-XB").await;
    let foreign_repo = seed_repository(&pool, other_project, "foreign", "verified").await;
    let actor = AuditActor::principal(human_id);

    let err = task_repository_binding::bind_task_repositories(
        &pool, SYSTEM_TENANT, project_id, task_id, &[entry(foreign_repo)], &actor,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, TaskRepoBindingError::RepositoryNotInProject));

    let unknown_repo = Uuid::new_v4();
    let err = task_repository_binding::bind_task_repositories(
        &pool, SYSTEM_TENANT, project_id, task_id, &[entry(unknown_repo)], &actor,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, TaskRepoBindingError::RepositoryNotFound));

    let unknown_task = Uuid::new_v4();
    let err = task_repository_binding::bind_task_repositories(
        &pool,
        SYSTEM_TENANT,
        project_id,
        unknown_task,
        &[entry(seed_repository(&pool, project_id, "ok", "verified").await)],
        &actor,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, TaskRepoBindingError::TaskNotFound));
}

/// 约束 5/6：claim 快照与 execution_allows 消费同一绑定事实——
/// 已绑定（verified）→ 允许；同项目 verified 但**未绑定** → deny（伪造归属无效）。
#[sqlx::test(migrations = "../../migrations")]
async fn claim_snapshot_and_execution_allows_consume_binding(pool: PgPool) {
    let (project_id, task_id, human_id) = seed_project_task(&pool, "TRB-FACT").await;
    let bound_repo = seed_repository(&pool, project_id, "bound", "verified").await;
    let unbound_repo = seed_repository(&pool, project_id, "unbound", "verified").await;
    let actor = AuditActor::principal(human_id);

    // 绑定前：快照空（这就是 P0 空洞本来的样子）。
    let mut tx = pool.begin().await.unwrap();
    let before = build_task_repo_access_tx(&mut *tx, SYSTEM_TENANT, project_id, task_id)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(before.is_empty());

    // 绑定后：同一事实出现在 claim 快照。
    task_repository_binding::bind_task_repositories(
        &pool, SYSTEM_TENANT, project_id, task_id, &[entry(bound_repo)], &actor,
    )
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let snapshot = build_task_repo_access_tx(&mut *tx, SYSTEM_TENANT, project_id, task_id)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].repository_id, bound_repo);
    assert_eq!(snapshot[0].access, RepoAccess::ReadWrite.as_str());

    // 执行期：快照内 + 现状 verified → 允许。
    // 快照形状对齐 claim 写入的 policy_snapshot（包裹态 {"repo_access": [...]}）。
    let snapshot_json = serde_json::json!({
        "repo_access": pm_application::repo_access::access_snapshot_json(&snapshot),
    });
    let allowed = execution_allows(
        &pool,
        SYSTEM_TENANT,
        project_id,
        bound_repo,
        Some(&snapshot_json),
        RepoAccess::ReadWrite,
    )
    .await
    .unwrap();
    assert!(allowed);

    // 约束 6：同项目 verified 但未绑定 → deny（MCP 即使带参数也伪造不出归属）。
    let forged = execution_allows(
        &pool,
        SYSTEM_TENANT,
        project_id,
        unbound_repo,
        Some(&snapshot_json),
        RepoAccess::ReadWrite,
    )
    .await
    .unwrap();
    assert!(!forged, "unbound repository must be denied even if same-project verified");
}

/// 约束 4（逆操作）：解绑幂等；解绑后快照同步收敛。
#[sqlx::test(migrations = "../../migrations")]
async fn unbind_is_idempotent(pool: PgPool) {
    let (project_id, task_id, human_id) = seed_project_task(&pool, "TRB-UB").await;
    let repo = seed_repository(&pool, project_id, "web", "verified").await;
    let actor = AuditActor::principal(human_id);
    task_repository_binding::bind_task_repositories(
        &pool, SYSTEM_TENANT, project_id, task_id, &[entry(repo)], &actor,
    )
    .await
    .unwrap();

    assert!(task_repository_binding::unbind_task_repository(
        &pool, SYSTEM_TENANT, project_id, task_id, repo, &actor,
    )
    .await
    .unwrap());
    // 幂等：再解绑返回 false 而非报错。
    assert!(!task_repository_binding::unbind_task_repository(
        &pool, SYSTEM_TENANT, project_id, task_id, repo, &actor,
    )
    .await
    .unwrap());

    let rows = task_repository_binding::list_task_repositories(
        &pool, SYSTEM_TENANT, project_id, task_id,
    )
    .await
    .unwrap();
    assert!(rows.is_empty());
    assert!(audit_count(&pool, project_id, "task.repository.unbound").await == 1);

    // 未知任务的解绑 → TaskNotFound。
    let err = task_repository_binding::unbind_task_repository(
        &pool,
        SYSTEM_TENANT,
        project_id,
        Uuid::new_v4(),
        repo,
        &actor,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, TaskRepoBindingError::TaskNotFound));
}
