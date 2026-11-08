//! 测试模板库预建：把 `migrations/` 的全部迁移预先跑进 Postgres 的模板库
//! （默认 `template1`），由 `scripts/cargo-test.sh` 在跑测试前调用一次。
//!
//! 为什么存在（实测依据见 README「测试提速」段）：
//!   `#[sqlx::test]` 每个用例都 `CREATE DATABASE` 再从零跑一遍 `migrations/`
//!   （2026-11-09 基线合并后 = 单文件 0001_baseline.sql ≈3.6s；此前旧链
//!   64 文件 ≈4.06s/次，最早 20 文件时代 ≈1.9s，298 用例吃掉约 1500s 串行 CPU）。
//!   sqlx 0.8 的 `test_support` 建库语句是裸 `create database "<name>"`，
//!   Postgres 对它的默认模板就是 `template1`；所以只要 `template1` 里已经
//!   有「全部迁移 + `_sqlx_migrations` 记账行」，每个用例的建库就退化成
//!   文件拷贝，随后 sqlx 的 `run_direct` 查到 `_sqlx_migrations` 全部 applied
//!   且 checksum 一致，直接跳过全部 DDL。实测建库 ~0.08s、DROP ~0.09s，
//!   单个 schema 用例从 5.15s 降到 0.33s。
//!
//! 因此本工具**不改任何测试代码**：`#[sqlx::test(migrations = "../../migrations")]`
//! 原样保留；模板没预建时走原来的慢路径，结果一样，只是慢。
//!
//! 指纹：模板库里存一份「迁移版本 + checksum」指纹；迁移新增/改动时指纹变化，
//! 下次预建自动重建，避免 sqlx 报 "migration was previously applied but has
//! been modified"。指纹一致则直接跳过，不付重建成本。
//!
//! 安全边界：会 DROP 模板库里除系统 schema 外的一切对象，并且会改变该实例上
//! **所有** 后续 `CREATE DATABASE` 的初始内容 —— 只允许本地开发/测试库。
//! 非 localhost 的 `DATABASE_URL` 默认拒绝执行（`AIPM_TEST_TEMPLATE_FORCE=1` 可覆盖）。
//!
//! 用法：
//!   DATABASE_URL=postgres://aipm:aipm@127.0.0.1:5433/aipm \
//!     cargo run -q -p pm-infrastructure --example seed_test_template
//!
//! 环境变量：
//!   DATABASE_URL             必填；连接参数来源，库名会被替换成模板库名
//!   AIPM_TEST_TEMPLATE_DB    模板库名，默认 template1
//!   AIPM_TEST_TEMPLATE_FORCE 非空时跳过 localhost 守卫

use std::str::FromStr;
use std::time::Instant;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Executor, PgConnection};

/// `CREATE DATABASE` 的隐式模板。
const DEFAULT_TEMPLATE_DB: &str = "template1";
const TEMPLATE_DB_ENV: &str = "AIPM_TEST_TEMPLATE_DB";
const FORCE_ENV: &str = "AIPM_TEST_TEMPLATE_FORCE";

/// 指纹所在 schema / 表。建在模板库里，随克隆进入每个测试库（无断言依赖它）。
const FINGERPRINT_SCHEMA: &str = "_aipm_test";
const FINGERPRINT_TABLE: &str = "template_fingerprint";

/// 并发预建互斥（两路测试同时启动时序列化重建）。常量任取，本仓专用。
const SEED_LOCK_KEY: i64 = 0x4150_4d5f_5445_5354;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL 未设置：本工具需要它来定位测试库（默认 127.0.0.1:5433/aipm）")?;
    let template_db =
        std::env::var(TEMPLATE_DB_ENV).unwrap_or_else(|_| DEFAULT_TEMPLATE_DB.to_string());

    let base = PgConnectOptions::from_str(&url)?;
    let forced = std::env::var(FORCE_ENV)
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    if !forced && !is_local(&base) {
        return Err(format!(
            "拒绝预建模板库：DATABASE_URL 的主机 {:?} 不是本机。\
             模板库会被改写，进而影响该实例上所有 CREATE DATABASE。\
             确需执行请设 {FORCE_ENV}=1",
            base.get_host()
        )
        .into());
    }

    let migrator = sqlx::migrate!("../../migrations");
    let want = fingerprint(&migrator);
    let total = migrator.iter().count();

    let opts = base.database(&template_db);
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await?;
    let mut conn = pool.acquire().await?;

    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(SEED_LOCK_KEY)
        .execute(&mut *conn)
        .await?;

    let current = read_fingerprint(&mut conn).await;
    if current.as_deref() == Some(want.as_str()) {
        unlock(&mut conn).await?;
        drop(conn);
        pool.close().await;
        println!("[test-template] {template_db} 已是最新（{total} 个迁移），跳过重建");
        return Ok(());
    }

    let started = Instant::now();
    reset(&mut conn).await?;
    migrator.run_direct(&mut *conn).await?;

    let applied: i64 =
        sqlx::query_scalar("SELECT count(*)::int8 FROM _sqlx_migrations WHERE success")
            .fetch_one(&mut *conn)
            .await?;
    if applied as usize != total {
        return Err(
            format!("_sqlx_migrations 记账行 {applied} 与迁移文件数 {total} 不一致").into(),
        );
    }

    write_fingerprint(&mut conn, &want, applied).await?;
    unlock(&mut conn).await?;
    drop(conn);
    pool.close().await;

    println!(
        "[test-template] {template_db} 重建完成：{applied} 个迁移，耗时 {:.2}s",
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

fn is_local(opts: &PgConnectOptions) -> bool {
    matches!(opts.get_host(), "localhost" | "127.0.0.1" | "::1")
}

async fn unlock(conn: &mut PgConnection) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(SEED_LOCK_KEY)
        .execute(conn)
        .await?;
    Ok(())
}

/// 迁移指纹：`version:sha384-checksum;` 逐条拼接。与 sqlx 记账用的 checksum 同源，
/// 所以「指纹一致」等价于「sqlx 看到的已应用迁移与当前文件一一对应」。
fn fingerprint(migrator: &sqlx::migrate::Migrator) -> String {
    let mut out = String::new();
    for migration in migrator.iter() {
        out.push_str(&migration.version.to_string());
        out.push(':');
        for byte in migration.checksum.iter() {
            out.push_str(&format!("{byte:02x}"));
        }
        out.push(';');
    }
    out
}

/// 读取已有指纹；表不存在（首次运行或上次被清空）视为没有。
async fn read_fingerprint(conn: &mut PgConnection) -> Option<String> {
    sqlx::query_scalar(&format!(
        "SELECT fingerprint FROM {FINGERPRINT_SCHEMA}.{FINGERPRINT_TABLE} LIMIT 1"
    ))
    .fetch_optional(conn)
    .await
    .ok()
    .flatten()
}

/// 清空模板库：删掉所有非系统 schema（表/索引/序列随之消失），重建 `public`。
async fn reset(conn: &mut PgConnection) -> Result<(), sqlx::Error> {
    let schemas: Vec<String> = sqlx::query_scalar(
        "SELECT nspname::text FROM pg_namespace \
          WHERE nspname NOT IN ('pg_catalog', 'information_schema') \
            AND nspname NOT LIKE 'pg_toast%' \
            AND nspname NOT LIKE 'pg_temp%'",
    )
    .fetch_all(&mut *conn)
    .await?;

    for schema in schemas {
        let quoted = format!("\"{}\"", schema.replace('"', "\"\""));
        conn.execute(format!("DROP SCHEMA {quoted} CASCADE").as_str())
            .await?;
    }

    // sqlx 的 `_sqlx_migrations` 落在 search_path 首个 schema，迁移里的
    // `CREATE EXTENSION pgcrypto` 也默认落 public —— 必须把它补回来。
    conn.execute("CREATE SCHEMA public").await?;
    conn.execute("DROP EXTENSION IF EXISTS pgcrypto").await?;
    Ok(())
}

async fn write_fingerprint(
    conn: &mut PgConnection,
    fp: &str,
    applied: i64,
) -> Result<(), sqlx::Error> {
    conn.execute(format!("CREATE SCHEMA {FINGERPRINT_SCHEMA}").as_str())
        .await?;
    conn.execute(
        format!(
            "CREATE TABLE {FINGERPRINT_SCHEMA}.{FINGERPRINT_TABLE} (\
               fingerprint text NOT NULL, \
               migration_count bigint NOT NULL, \
               built_at timestamptz NOT NULL DEFAULT now())"
        )
        .as_str(),
    )
    .await?;
    sqlx::query(&format!(
        "INSERT INTO {FINGERPRINT_SCHEMA}.{FINGERPRINT_TABLE} (fingerprint, migration_count) \
         VALUES ($1, $2)"
    ))
    .bind(fp)
    .bind(applied)
    .execute(&mut *conn)
    .await?;
    Ok(())
}
