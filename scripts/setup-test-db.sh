#!/usr/bin/env bash
# 测试库容器调优（一次性；可重复执行，幂等）。
#
# 为什么需要：
#   `#[sqlx::test]` 每个用例都 CREATE DATABASE + 跑全部 17 个迁移（363 张表）。
#   在默认 durability 设置下这段固定开销 ≈ 380ms/用例 × 近 1400 个用例，
#   且 20 路并行时 DDL 争用把单例耗时再放大 2 倍。
#
# 实测（aipm-pg-gate，2026-11-11）：
#   fsync/synchronous_commit 默认 : 迁移 380ms，建库总成本 525ms/用例
#   fsync=off + full_page_writes=off : 迁移 65ms，建库总成本 130ms/用例（4x）
#
# 安全性：仅用于**测试集群**。关掉 fsync 意味着崩溃可能丢数据——测试库每次
# 用例都新建、结束即删，没有需要保护的数据。**禁止**对任何持久化实例执行。
#
# 用法：
#   bash scripts/setup-test-db.sh            # 应用设置（必要时重启容器）
#   bash scripts/setup-test-db.sh --check    # 只检查当前值

set -euo pipefail

CONTAINER="${TEST_PG_CONTAINER:-aipm-pg-gate}"
DB_USER="${TEST_PG_USER:-aipm}"
PORT="${TEST_PG_PORT:-16433}"

if ! docker ps --format '{{.Names}}' | grep -qx "$CONTAINER"; then
  echo "==> 启动容器 $CONTAINER"
  docker start "$CONTAINER" >/dev/null
  for _ in $(seq 1 30); do
    docker exec "$CONTAINER" pg_isready -U "$DB_USER" -q 2>/dev/null && break
    sleep 1
  done
fi

CURRENT=$(docker exec "$CONTAINER" psql -U "$DB_USER" -d postgres -t -A -F' ' \
  -c "SELECT current_setting('fsync'), current_setting('full_page_writes'), current_setting('synchronous_commit');")
echo "==> 当前 fsync/full_page_writes/synchronous_commit = $CURRENT"

if [ "${1:-}" = "--check" ]; then
  case "$CURRENT" in
    "off off off") echo "OK 测试库已是性能配置"; exit 0 ;;
    *) echo "!! 仍是默认 durability 配置，跑 bash scripts/setup-test-db.sh 可提速约 4x"; exit 1 ;;
  esac
fi

case "$CURRENT" in
  "off off off") echo "OK 已是性能配置，跳过重启"; exit 0 ;;
esac

echo "==> 应用测试库设置（postgresql.auto.conf）"
docker exec "$CONTAINER" psql -U "$DB_USER" -d postgres -q \
  -c "ALTER SYSTEM SET fsync = 'off';" \
  -c "ALTER SYSTEM SET full_page_writes = 'off';" \
  -c "ALTER SYSTEM SET synchronous_commit = 'off';" \
  -c "ALTER SYSTEM SET max_wal_size = '4GB';" \
  -c "ALTER SYSTEM SET checkpoint_timeout = '30min';"

echo "==> 重启容器使 fsync 生效（postmaster 级参数）"
docker restart "$CONTAINER" >/dev/null
for _ in $(seq 1 60); do
  docker exec "$CONTAINER" pg_isready -U "$DB_USER" -q 2>/dev/null && break
  sleep 1
done

CURRENT=$(docker exec "$CONTAINER" psql -U "$DB_USER" -d postgres -t -A -F' ' \
  -c "SELECT current_setting('fsync'), current_setting('full_page_writes'), current_setting('synchronous_commit');")
echo "==> 生效值 = $CURRENT"
echo "==> 下一步：bash scripts/test-timing.sh -p pm-application --test docproc_pipeline"
echo "    （DATABASE_URL 必须用 127.0.0.1:${PORT}，见 AGENTS.md 规则 3）"
