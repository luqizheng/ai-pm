#!/usr/bin/env bash
# scripts/guard-pm-api-sql.sh —— pm-api 零 SQL 门禁（任务书批 B，D3 裁决）。
#
# pm-api 的 handler 不再直接写 SQL：数据访问收口到仓储（port 在
# pm-application/src/repo/，实现在 pm-infrastructure/src/repo_*）。本脚本
# 固化该纪律，防回潮。
#
# 豁免：support.rs 的 project_visible EXISTS——全站 require_project_visible
# 的底层依赖，判定保留（任务书 §批 B 执行记录）。
#
# 用法：bash scripts/guard-pm-api-sql.sh   # 违规非零退出
set -uo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

ALLOWED=1   # support.rs 判定保留的唯一豁免
total=0
violations=()

while IFS=$'\t' read -r count file; do
    total=$((total + count))
    if [ "$file" = "apps/pm-api/src/support.rs" ]; then
        if [ "$count" -gt "$ALLOWED" ]; then
            violations+=("apps/pm-api/src/support.rs: $count (豁免上限 $ALLOWED)")
        fi
    else
        violations+=("$file: $count")
    fi
done < <(grep -rc "sqlx::query" apps/pm-api/src --include="*.rs" | awk -F: '$2 > 0 {print $2"\t"$1}' | sort -rn)

if [ "${#violations[@]}" -gt 0 ]; then
    echo "[pm-api-sql 门禁] 违规（handler 直写 SQL；请下沉到 pm-application 仓储 port + pm-infrastructure 实现）："
    printf '  - %s\n' "${violations[@]}"
    exit 1
fi

echo "[pm-api-sql 门禁] PASS（豁免内 support.rs=${ALLOWED}）"
