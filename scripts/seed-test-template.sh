#!/usr/bin/env bash
# 测试模板库播种（可选加速，默认不启用）。
#
# 作用：`#[sqlx::test]` 的 `CREATE DATABASE` 不带 TEMPLATE，默认克隆 template1。
# 把全量迁移预装进 template1 后，每个测试库直接带着 schema 出生，迁移 run 从
# 「重放 17 个迁移（362 表，≈300ms）」降为「校验 _sqlx_migrations（≈5ms）」
# —— 实测单例建库成本 340ms → 62ms。
#
# 实测（1491 用例，fast profile，2026-11-11）：
#   干净 template1：319s 墙钟 / 4761s CPU
#   播种 template1：226s 墙钟 / 2429s CPU（-29%）
#
# ⚠️ 代价与风险（务必读）
#   1. template1 是**集群级**共享资源：同集群所有新库都继承这份 schema。
#   2. schema 被钉在**当前分支的迁移集**上。切到迁移更少的分支（例如 master 有
#      0017、特性分支停在 0016）时，sqlx 会报 `VersionMissing(17)`——必须先
#      `--reset`。切分支前养成 `--reset` 的习惯，或只在单一分支上长跑时启用。
#   3. 新增迁移后重跑本脚本即可（增量应用）。
#
# 用法：
#   bash scripts/seed-test-template.sh          # 播种（当前分支的迁移集）
#   bash scripts/seed-test-template.sh --reset   # 还原为干净 template1
#   bash scripts/seed-test-template.sh --check   # 查看是否已播种
#
# 仅限本地测试集群（脚本会拒绝非本地 DATABASE_URL）。

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

: "${DATABASE_URL:=postgres://aipm:aipm@127.0.0.1:16433/aipm}"
export DATABASE_URL
export CARGO_HTTP_PROXY=""
export NO_PROXY="${NO_PROXY:-192.168.2.12}"

CONTAINER="${TEST_PG_CONTAINER:-aipm-pg-gate}"
DB_USER="${TEST_PG_USER:-aipm}"

template_tables() {
  docker exec "$CONTAINER" psql -U "$DB_USER" -d template1 -t -A -c \
    "SELECT count(*) FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace \
      WHERE c.relkind='r' AND n.nspname NOT IN ('pg_catalog','information_schema');" 2>/dev/null
}

case "${1:-}" in
  --check)
    n=$(template_tables)
    if [ "${n:-0}" -gt 0 ]; then
      echo "已播种：template1 有 $n 张表（测试库免迁移重放）"
    else
      echo "干净：template1 无业务表（每个测试库都要重放 17 个迁移，≈300ms/用例）"
    fi
    exit 0
    ;;
  --reset)
    echo "==> 还原 template1（DROP + 从 template0 重建）"
    docker exec "$CONTAINER" psql -U "$DB_USER" -d postgres -q \
      -c "UPDATE pg_database SET datistemplate=false WHERE datname='template1';" \
      -c "DROP DATABASE template1;" \
      -c "CREATE DATABASE template1 TEMPLATE template0;" \
      -c "UPDATE pg_database SET datistemplate=true WHERE datname='template1';"
    echo "==> 完成，template1 表数 = $(template_tables)"
    exit 0
    ;;
  "") ;;
  *) echo "未知参数 $1（可用：--reset / --check）" >&2; exit 2 ;;
esac

echo "==> 播种 template1（当前分支的迁移集：$(ls migrations/*.sql | wc -l) 个文件）"
echo "    注意：schema 将钉在本分支；切到迁移更少的分支前请先 --reset"
cargo run -q -p pm-application --example seed_template1
echo "==> template1 表数 = $(template_tables)"
echo "==> 现在跑 bash scripts/test.sh 即可享受加速"
