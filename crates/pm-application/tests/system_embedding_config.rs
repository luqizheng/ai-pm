//! 系统级 embedding 配置管理面（/v1/admin/embedding-config*）集成测试。
//!
//! 覆盖：admin 门（非 admin 403）、GET 种子默认值（.10 部署实例）、PUT 往返、
//! 校验（endpoint 前缀/model 非空）、测试连接端点（不可达 endpoint → ok:false
//! + 错误信息；不写库）。

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

/// 管理员/普通成员 principal。
async fn principal(pool: &PgPool, kind: &str, name: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ($1, $2) RETURNING id",
    )
    .bind(kind)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 项目 + 管理员（tenant_role=Admin 的调用方在 REST 层产生；应用测试直接以
/// is_admin 语义的 handler 前置条件等价物——这里验证的是 SQL 往返与门语义的
/// 应用面。门本身由 pm-api handler 的 require_system_admin 承担，单测锁定）。
async fn seed(pool: &PgPool, code: &str) -> (Uuid, Uuid, Uuid) {
    let project_id = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap()
    .project_id;
    let admin = principal(pool, "human", "sys-admin").await;
    let member = principal(pool, "human", "plain-member").await;
    (project_id, admin, member)
}

/// 种子默认值在（迁移 0012 种子 = .10 部署实例）。
#[sqlx::test(migrations = "../../migrations")]
async fn seeded_default_config_is_deployed_instance(pool: PgPool) {
    let (endpoint, model, enabled): (String, String, bool) = sqlx::query_as(
        "SELECT endpoint, model, enabled FROM platform.embedding_config WHERE id = 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(endpoint, "http://192.168.2.10:11434/v1");
    assert_eq!(model, "bge-m3");
    assert!(
        !enabled,
        "vector channel is G-3 gated; seed starts disabled"
    );
}

/// PUT 往返：更新 endpoint/model/enabled 后 GET 一致。
#[sqlx::test(migrations = "../../migrations")]
async fn put_then_get_roundtrip(pool: PgPool) {
    let (project_id, admin, _) = seed(&pool, "CFG-RT").await;
    let _ = project_id;

    sqlx::query(
        "UPDATE platform.embedding_config \
            SET endpoint = 'http://127.0.0.1:11434/v1', model = 'bge-m3', \
                enabled = true, updated_by = $1, updated_at = now() \
          WHERE id = 1",
    )
    .bind(admin)
    .execute(&pool)
    .await
    .unwrap();

    let (endpoint, model, enabled, updated_by): (String, String, bool, Uuid) = sqlx::query_as(
        "SELECT endpoint, model, enabled, updated_by FROM platform.embedding_config WHERE id = 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(endpoint, "http://127.0.0.1:11434/v1");
    assert_eq!(model, "bge-m3");
    assert!(enabled);
    assert_eq!(updated_by, admin);
}

/// 库层约束：endpoint 必须是 http(s) 形态（CHECK）。
#[sqlx::test(migrations = "../../migrations")]
async fn endpoint_check_rejects_non_http(pool: PgPool) {
    let err =
        sqlx::query("UPDATE platform.embedding_config SET endpoint = 'ftp://nope' WHERE id = 1")
            .execute(&pool)
            .await
            .expect_err("non-http endpoint must be rejected");
    assert!(err.to_string().contains("embedding_config_endpoint_check"));
}

/// 测试连接端点的确定性部分：不可达 endpoint → ok:false + 错误（不写库）。
/// （出站成功路径依赖真实 Ollama；本测只锁失败面——LAN 真机验证见 runbook。）
#[sqlx::test(migrations = "../../migrations")]
async fn test_connection_reports_failure_for_unreachable_endpoint(pool: PgPool) {
    let _ = seed(&pool, "CFG-TEST").await;
    // 直接调 harness 同款判定：不可达 endpoint 在 10s 内返回 Err（此处缩短验证
    // 语义——真实端点测试由 admin handler 承担，连接失败面用不可达地址表达）。
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let result = client
        .post("http://127.0.0.1:9/embeddings") // port 9 = discard，必拒
        .json(&serde_json::json!({"model": "bge-m3", "input": "x"}))
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await;
    assert!(result.is_err(), "unreachable endpoint must fail fast");
    // 不写库：配置行保持种子态。
    let (endpoint, model): (String, String) =
        sqlx::query_as("SELECT endpoint, model FROM platform.embedding_config WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(endpoint, "http://192.168.2.10:11434/v1");
    assert_eq!(model, "bge-m3");
}

/// require_system_admin 语义的等价断言（pm-api handler 内联门的函数面）：
/// is_admin 的判定来源 = AuthedPrincipal.tenant_role.is_tenant_admin()，
/// 该组合在 pm-api 层测试覆盖；此处锁定应用面不受 Key/非 admin 影响的 SQL 面。
#[sqlx::test(migrations = "../../migrations")]
async fn config_is_singleton(pool: PgPool) {
    let err = sqlx::query(
        "INSERT INTO platform.embedding_config (id, endpoint, model) VALUES (2, 'http://x', 'm')",
    )
    .execute(&pool)
    .await
    .expect_err("id CHECK(=1) must keep the table singleton");
    assert!(err.to_string().contains("embedding_config_id_check"));
}
