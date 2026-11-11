//! 测试基础设施成本探针（不属于产品代码，`cargo run --example` 用）。
//!
//! 回答一个问题：`#[sqlx::test(migrations = "../../migrations")]` 每个用例的
//! **固定开销**（建库 + 全量迁移 + 连接池）到底占多少秒，以及其中迁移占多少。
//!
//! 用法：
//!   DATABASE_URL='postgres://aipm:aipm@127.0.0.1:16433/aipm' \
//!     cargo run -p pm-application --example testdb_bench

use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set (postgres://aipm:aipm@127.0.0.1:16433/aipm)");
    // 维护连接：优先 postgres 库；不可用时退回业务库（CREATE DATABASE 也能跑）
    let maintenance = with_db(&base_url, "postgres");
    let admin_pool = match sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&maintenance)
        .await
    {
        Ok(pool) => pool,
        Err(e) => {
            eprintln!("connect to /postgres failed ({e}); falling back to base url");
            sqlx::postgres::PgPoolOptions::new()
                .max_connections(1)
                .connect(&base_url)
                .await?
        }
    };

    println!("base: {}", redact(&base_url));

    for round in 1..=3 {
        let db_name = format!("_bench_testdb_{}_{}", std::process::id(), round);

        // ① CREATE DATABASE
        let t0 = Instant::now();
        sqlx::query(&format!("CREATE DATABASE {db_name}"))
            .execute(&admin_pool)
            .await?;
        let create_ms = t0.elapsed().as_millis();

        // ② 连接（sqlx::test 每用例建池）
        let t1 = Instant::now();
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(&with_db(&base_url, &db_name))
            .await?;
        let connect_ms = t1.elapsed().as_millis();

        // ③ 全量迁移
        let t2 = Instant::now();
        sqlx::migrate!("../../migrations").run(&pool).await?;
        let migrate_ms = t2.elapsed().as_millis();

        // ④ 一次典型业务查询（模拟用例体量级）
        let t3 = Instant::now();
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM information_schema.tables")
            .fetch_one(&pool)
            .await?;
        let query_us = t3.elapsed().as_micros();

        // ⑤ 收尾（sqlx::test 会 DROP DATABASE）
        pool.close().await;
        let t4 = Instant::now();
        sqlx::query(&format!("DROP DATABASE {db_name}"))
            .execute(&admin_pool)
            .await?;
        let drop_ms = t4.elapsed().as_millis();

        let total = create_ms + connect_ms + migrate_ms + drop_ms;
        println!(
            "round {round}: 总 {total:>5}ms = 建库 {create_ms:>4} + 连接 {connect_ms:>4} + \
             迁移 {migrate_ms:>5} + 查询 {query_us:>6}us + 删库 {drop_ms:>4}  (表数 {n})"
        );
    }
    Ok(())
}

fn redact(url: &str) -> String {
    // 只打 host/db，避免密码进日志
    url.rsplit('@').next().unwrap_or(url).to_string()
}

fn with_db(base: &str, db: &str) -> String {
    let (prefix, _) = base.rsplit_once('/').unwrap_or((base, ""));
    format!("{prefix}/{db}")
}
