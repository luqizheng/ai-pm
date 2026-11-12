//! 平台级 embedding 配置仓储（`PgPlatformOpsRepo`）读写回归。
//!
//! 背景（2026-11-12 生产缺陷）：`update_embedding_config` 的 SQL 参数序号
//! 错位（`$2..$5` 配 4 个 bind，`enabled=$4` 收到 updated_by 的 uuid）——
//! pm-application 侧的 put_then_get_roundtrip 用例手写 UPDATE，没走仓储，
//! 拦不住本类缺陷。本文件**直接驱动真实仓储 port**，锁「bind 数 = 参数数、
//! 类型逐位对齐」。
use uuid::Uuid;

use pm_application::repo::platform_ops::PlatformOpsRepo;
use pm_infrastructure::repo_platform_ops::PgPlatformOpsRepo;

/// 种子单行存在（0012）且 load 返回种子值。
#[sqlx::test(migrations = "../../migrations")]
async fn load_returns_seed_row(pool: sqlx::PgPool) {
    let repo = PgPlatformOpsRepo::new(pool.clone());
    let (endpoint, model, enabled, _updated_at) = repo.load_embedding_config().await.unwrap();
    assert_eq!(endpoint, "http://192.168.2.10:11434/v1");
    assert_eq!(model, "bge-m3");
    assert!(!enabled, "vector channel is G-3 gated; seed starts disabled");
}

/// **核心回归**：update 经真实仓储往返，四列逐位一致（含 updated_by 落对列）。
#[sqlx::test(migrations = "../../migrations")]
async fn update_roundtrip_writes_all_columns(pool: sqlx::PgPool) {
    let repo = PgPlatformOpsRepo::new(pool.clone());
    let who = Uuid::new_v4();
    let (endpoint, model, enabled, _updated_at) = repo
        .update_embedding_config("https://embed.example.internal/v1", "bge-m3", true, who)
        .await
        .unwrap();

    assert_eq!(endpoint, "https://embed.example.internal/v1");
    assert_eq!(model, "bge-m3");
    assert!(enabled);

    // 回读：值与 updated_by（uuid 落对列，不再串位到 enabled）
    let (db_endpoint, db_model, db_enabled, db_updated_by): (
        String,
        String,
        bool,
        Option<Uuid>,
    ) = sqlx::query_as(
        "SELECT endpoint, model, enabled, updated_by FROM platform.embedding_config WHERE id = 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(db_endpoint, "https://embed.example.internal/v1");
    assert_eq!(db_model, "bge-m3");
    assert!(db_enabled);
    assert_eq!(db_updated_by, Some(who));
}

/// https 端点须被 CHECK 放行（应用层与库层同口径；此前只放行 http://）。
#[sqlx::test(migrations = "../../migrations")]
async fn https_endpoint_passes_check(pool: sqlx::PgPool) {
    let repo = PgPlatformOpsRepo::new(pool.clone());
    let result = repo
        .update_embedding_config("https://192.168.2.10:11434/v1", "bge-m3", false, Uuid::new_v4())
        .await;
    assert!(
        result.is_ok(),
        "https endpoint must satisfy the endpoint CHECK: {result:?}"
    );
}
