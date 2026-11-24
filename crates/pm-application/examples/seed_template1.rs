//! 把全量迁移预装进 `template1`（测试集群专用，一次性）。
//!
//! 原理：`sqlx::test` 的 `CREATE DATABASE "<name>"` 不带 `TEMPLATE`，PostgreSQL
//! 默认从 template1 克隆（sqlx-postgres/migrate.rs:49）。template1 预装到位后，
//! 每用例的 `Migrator::run` 只剩一次 `_sqlx_migrations` 校验查询，不再重放
//! 17 个迁移（363 张表）。
//!
//! **必须在测试容器上跑**（默认写死 aipm-pg-gate 语义由 DATABASE_URL 决定）：
//!   DATABASE_URL='postgres://aipm:aipm@127.0.0.1:16433/aipm' \
//!     cargo run -p pm-application --example seed_template1
//!
//! 约束：
//! - 迁移文件内容不可变（本仓 AGENTS 规则 1 的同一前提）；新增迁移后重跑本程序
//!   即可让新库带上最新 schema。
//! - 其它连到同一集群的库也会继承该 schema——测试集群无此顾虑，**禁止**对任何
//!   生产/共享实例执行。
//! - 撤销：`DROP SCHEMA <schema> CASCADE` 逐 schema 清理，或重建模板库。

use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    if !base.contains("127.0.0.1") && !base.contains("192.168.") {
        return Err("refusing to run against a non-local database".into());
    }
    let template1 = replace_db(&base, "template1");

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&template1)
        .await?;

    // template1 默认允许连接（template0 不允许）；库必须干净，否则迁移可能撞已存在对象。
    let tables: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.tables \
         WHERE table_schema NOT IN ('pg_catalog', 'information_schema')",
    )
    .fetch_one(&pool)
    .await?;
    if tables > 0 {
        eprintln!(
            "template1 已有 {tables} 张非系统表——可能已播种或集群共享。\
             如需重建：DROP DATABASE template1 后再 CREATE DATABASE template1 TEMPLATE template0"
        );
    }

    let started = Instant::now();
    sqlx::migrate!("../../migrations").run(&pool).await?;
    let elapsed = started.elapsed();

    let applied: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations WHERE success")
        .fetch_one(&pool)
        .await?;
    let tables: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.tables \
         WHERE table_schema NOT IN ('pg_catalog', 'information_schema')",
    )
    .fetch_one(&pool)
    .await?;

    println!("template1 播种完成：{elapsed:?}（{applied} 个迁移，{tables} 张表）");
    println!("后续每个测试库 CREATE DATABASE 会克隆此模板；迁移 run 降为校验查询。");
    Ok(())
}

fn replace_db(url: &str, db: &str) -> String {
    let (prefix, _) = url.rsplit_once('/').unwrap_or((url, ""));
    format!("{prefix}/{db}")
}
