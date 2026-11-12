#!/usr/bin/env bash
# Visualization 零业务写路径门禁（Visualization V0-1.8）
#
# 依据 ADR-VIZ-005 / AC-16：Visualization 是共享基础设施，**不得写入任何业务
# 领域权威对象**（Requirement / Design / ADR / Task / 认知 / 知识）。
# 它只能写自己的基础设施数据（viz.artifact / viz.approval / viz.render_output /
# viz.slot_config）。
#
# 本脚本扫描可视化模块源码的 use 语句，命中业务写路径即失败。
# 判定依据是**模块名**（写路径模块的命名在仓内是稳定的），而不是逐个函数——
# 架构约束要防的是「引入写能力」，不是「调用了某个只读函数」。
#
# 用法：
#   bash scripts/viz-no-write-path.sh            # 校验，违规退出码 1
#   bash scripts/viz-no-write-path.sh --report   # 只报告，不以非零码退出
#
# 退出码：0 通过；1 存在违规；2 运行错误。

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

REPORT_ONLY=0
for arg in "$@"; do
  case "$arg" in
    --report) REPORT_ONLY=1 ;;
    *) ;;
  esac
done

# 可视化模块所在目录（相对仓库根）
VIZ_DIRS=(
  "crates/pm-domain/src/visualization"
  "crates/pm-application/src/visualization"
)

# 业务写路径模块（禁止被 viz 模块 import）。
# 命名来自仓内既有写路径模块；命中即视为可视化获得了业务写能力。
FORBIDDEN_PATTERN='pm_write|command_executor|command_center|pc::(publish|proposal)|design::|requirement::|task_transition|approval_decide|interaction_decide'

VIOLATIONS=0

for dir in "${VIZ_DIRS[@]}"; do
  if [ ! -d "$dir" ]; then
    continue
  fi
  while IFS= read -r file; do
    hits=$(grep -nE "^[[:space:]]*(pub )?use .*(${FORBIDDEN_PATTERN})" "$file" 2>/dev/null || true)
    # 排除注释行
    hits=$(printf '%s\n' "$hits" | grep -vE "^\s*[0-9]+:\s*//" || true)
    # 豁免：pm_write::audit 是横切基础设施（governance.audit_log 写入助手，
    # 全仓统一可观测性要求），不是业务领域写路径——否则可视化无法落审计。
    hits=$(printf '%s\n' "$hits" | grep -v "pm_write::audit" || true)
    if [ -n "$hits" ]; then
      while IFS= read -r line; do
        [ -z "$line" ] && continue
        echo "[viz-no-write-path] $file:$line"
        echo "    ↳ 可视化模块不得引入业务写路径（ADR-VIZ-005 / AC-16）"
        VIOLATIONS=$((VIOLATIONS + 1))
      done <<< "$hits"
    fi
  done < <(find "$dir" -type f -name '*.rs' | sort)
done

# 附加检查：SQL 写语句只允许落在 viz.*（自身基础设施数据，ADR V1.0.1 §2.1），
# 触及任何**业务 schema**（delivery/design/pc/kb/ai/plan/core/governance）即越界。
# 注意：仅禁"写"（INSERT/UPDATE/DELETE），SELECT 不受限——投影层天然只读业务表。
for dir in "${VIZ_DIRS[@]}"; do
  [ -d "$dir" ] || continue
  while IFS= read -r file; do
    hits=$(grep -nEi '\b(INSERT[[:space:]]+INTO[[:space:]]+(delivery|design|pc|kb|ai|plan|core|governance)\.|UPDATE[[:space:]]+(delivery|design|pc|kb|ai|plan|core|governance)\.|DELETE[[:space:]]+FROM[[:space:]]+(delivery|design|pc|kb|ai|plan|core|governance)\.)' "$file" 2>/dev/null || true)
    if [ -n "$hits" ]; then
      while IFS= read -r line; do
        [ -z "$line" ] && continue
        echo "[viz-no-write-path] $file:$line"
        echo "    ↳ 可视化模块写业务 schema——零业务领域写路径（AC-16）"
        VIOLATIONS=$((VIOLATIONS + 1))
      done <<< "$hits"
    fi
  done < <(find "$dir" -type f -name '*.rs' | sort)
done

if [ "$VIOLATIONS" -eq 0 ]; then
  echo "[viz-no-write-path] ✓ 可视化模块零业务写路径检查通过"
  exit 0
fi

echo "[viz-no-write-path] ✗ $VIOLATIONS 项违规"
if [ "$REPORT_ONLY" -eq 1 ]; then
  exit 0
fi
exit 1
