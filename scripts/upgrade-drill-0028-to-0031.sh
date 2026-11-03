#!/usr/bin/env bash
# S01 基线 §4.4 / §11：迁移的「升级路径与环境一致性」演练。
#
# 演练内容：
#   1. 旧环境：空库按序执行 0001–0028（模拟存量部署）。
#   2. 升级：追加执行 0028 之后的**全部**增量迁移（0030、0031、0032、0033）。
#   3. 一致性：与「空库直接迁到最新」的 Schema 逐对象比对
#      （表集合 / 列集合 / 约束集合 / 索引集合必须一致）。
#
# 用法：bash scripts/upgrade-drill-0028-to-0031.sh
# 依赖：本机 5435 端口的 aipm PG 容器（docker-compose.dev.yml）。
#
# 文件名保留「0028-to-0031」（基线冻结时的编号），实际覆盖到**当前最新**迁移；
# 脚本按文件名排序动态执行 0030 之后的全部迁移，新增迁移无需改脚本。
set -euo pipefail

HOST_PORT="${HOST_PORT:-5435}"
DB_UP="s01_drill_upgrade"
DB_FRESH="s01_drill_fresh"
PSQL="docker exec -i infrastructure-aipm-pg-1 psql -U aipm -v ON_ERROR_STOP=1"

cleanup() {
  $PSQL -d postgres -c "DROP DATABASE IF EXISTS $DB_UP;" >/dev/null 2>&1 || true
  $PSQL -d postgres -c "DROP DATABASE IF EXISTS $DB_FRESH;" >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "── 1. 旧环境：0001–0028 ──"
$PSQL -d postgres -c "DROP DATABASE IF EXISTS $DB_UP;" >/dev/null
$PSQL -d postgres -c "CREATE DATABASE $DB_UP;" >/dev/null
for f in migrations/0*.sql; do
  base=$(basename "$f")
  [ "$base" \> "0028_run_execution_mode.sql" ] && break
  $PSQL -d "$DB_UP" -q -f - < "$f" >/dev/null
  echo "  applied $base"
done

echo "── 2. 升级：0028 之后的全部增量迁移（0030、0031、0032、0033）──"
for f in migrations/0*.sql; do
  base=$(basename "$f")
  [ "$base" \< "0030_project_member_role.sql" ] && continue
  $PSQL -d "$DB_UP" -q -f - < "$f" >/dev/null
  echo "  applied $base"
done

echo "── 3. 新环境：全序 0001–0033 ──"
$PSQL -d postgres -c "DROP DATABASE IF EXISTS $DB_FRESH;" >/dev/null
$PSQL -d postgres -c "CREATE DATABASE $DB_FRESH;" >/dev/null
for f in migrations/0*.sql; do
  $PSQL -d "$DB_FRESH" -q -f - < "$f" >/dev/null
done
echo "  applied all"

echo "── 4. 环境一致性比对 ──"
# 以「库名 + 对象指纹」比对：表 / 列 / 约束 / 索引 四类对象集合必须一致。
dump_objects() {
  local db="$1"
  $PSQL -d "$db" -At -c "
    SELECT 'T|' || table_schema || '.' || table_name
      FROM information_schema.tables
     WHERE table_schema IN ('core','delivery','integration','ai','governance','discovery','devenv')
    UNION ALL
    SELECT 'C|' || table_schema || '.' || table_name || '.' || column_name || ':' || data_type
      FROM information_schema.columns
     WHERE table_schema IN ('core','delivery','integration','ai','governance','discovery','devenv')
    UNION ALL
    SELECT 'K|' || n.nspname || '.' || c.relname || '.' || con.conname || ':' || con.contype::text
      FROM pg_constraint con
      JOIN pg_class c ON c.oid = con.conrelid
      JOIN pg_namespace n ON n.oid = c.relnamespace
     WHERE n.nspname IN ('core','delivery','integration','ai','governance','discovery','devenv')
    UNION ALL
    SELECT 'I|' || schemaname || '.' || tablename || '.' || indexname
      FROM pg_indexes
     WHERE schemaname IN ('core','delivery','integration','ai','governance','discovery','devenv')
    ORDER BY 1;"
}
dump_objects "$DB_UP" > .workbuddy/drill_upgrade.txt
dump_objects "$DB_FRESH" > .workbuddy/drill_fresh.txt

if diff -u .workbuddy/drill_fresh.txt .workbuddy/drill_upgrade.txt > .workbuddy/drill_diff.txt; then
  echo "  ✅ 升级环境与新环境 Schema 完全一致（表 / 列 / 约束 / 索引）"
else
  echo "  ❌ Schema 不一致："
  cat .workbuddy/drill_diff.txt
  exit 1
fi

echo "── 5. 关键对象抽查（升级库）──"
check() {
  local desc="$1" sql="$2" expect="$3"
  local got
  got=$($PSQL -d "$DB_UP" -At -c "$sql")
  if [ "$got" = "$expect" ]; then
    echo "  ✅ $desc = $got"
  else
    echo "  ❌ $desc 期望 $expect 实际 $got"
    exit 1
  fi
}
check "新表 project_initialization_operation" \
  "SELECT count(*) FROM information_schema.tables WHERE table_schema='core' AND table_name='project_initialization_operation'" 1
check "新表 repo_import_draft_version" \
  "SELECT count(*) FROM information_schema.tables WHERE table_schema='integration' AND table_name='repo_import_draft_version'" 1
check "新表 repo_import_source_attempt" \
  "SELECT count(*) FROM information_schema.tables WHERE table_schema='integration' AND table_name='repo_import_source_attempt'" 1
check "新表 discovery_finding_evidence" \
  "SELECT count(*) FROM information_schema.tables WHERE table_schema='discovery' AND table_name='discovery_finding_evidence'" 1
check "旧单列 draft 已删除" \
  "SELECT count(*) FROM information_schema.columns WHERE table_schema='integration' AND table_name='repo_import' AND column_name='draft'" 0
check "旧名额索引已删除" \
  "SELECT count(*) FROM pg_indexes WHERE schemaname='integration' AND indexname='repo_import_one_in_progress'" 0
check "新名额索引已建立" \
  "SELECT count(*) FROM pg_indexes WHERE schemaname='integration' AND indexname='repo_import_one_active_per_project'" 1
check "Evidence CHECK 判别器存在（类型枚举 + 链路判别器共 2 条）" \
  "SELECT count(*) FROM pg_constraint con JOIN pg_class c ON c.oid=con.conrelid JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='discovery' AND c.relname='discovery_finding_evidence' AND con.contype='c'" 2
check "task_repository 仓库 FK 为 RESTRICT（非 CASCADE）" \
  "SELECT count(*) FROM pg_constraint con JOIN pg_class c ON c.oid=con.conrelid JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='delivery' AND c.relname='task_repository' AND con.contype='f' AND con.confdeltype='r'" 1
check "discovery 域不得引用 repo_import（Q1 单向不变量保持）" \
  "SELECT count(*) FROM pg_constraint con JOIN pg_class c ON c.oid=con.conrelid JOIN pg_namespace n ON n.oid=c.relnamespace JOIN pg_class t ON t.oid=con.confrelid JOIN pg_namespace tn ON tn.oid=t.relnamespace WHERE n.nspname='discovery' AND tn.nspname='integration' AND t.relname='repo_import' AND con.contype='f'" 0
# 0032/0033（I3 V1.1 简化裁决）：快照锚点列保留、授权表已撤下。
check "task_run.policy_snapshot_hash 列存在（I3-D 锚点）" \
  "SELECT count(*) FROM information_schema.columns WHERE table_schema='delivery' AND table_name='task_run' AND column_name='policy_snapshot_hash'" 1
check "Git 能力授权表已撤下（0033）" \
  "SELECT count(*) FROM information_schema.tables WHERE table_schema='integration' AND table_name='git_capability_grant'" 0
check "授权表索引已随之撤下" \
  "SELECT count(*) FROM pg_indexes WHERE schemaname='integration' AND indexname='idx_git_capability_grant_principal'" 0

rm -f .workbuddy/drill_upgrade.txt .workbuddy/drill_fresh.txt .workbuddy/drill_diff.txt
echo ""
echo "演练通过：0028 → 0033 升级路径与环境一致性 ✅"
