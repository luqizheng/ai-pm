#!/usr/bin/env bash
# Visualization 门禁统一入口（仓级）
#
# 仓内**无 CI workflow**（见 docs/ai-pm-test-performance-toolchain 记录的纪律：
# 门禁一律落 scripts/ 下的本地脚本），因此本文件是可视化相关的唯一门禁入口。
#
# 依次执行：
#   1. viz-registry-lint.js   —— Relation Registry 结构与治理不变量（15 条规则）
#   2. viz-spec-lint.js       —— VizSpec 字段纯净性（禁渲染器专属字段与未知字段）
#   3. viz-no-write-path.sh   —— 零业务写路径（架构约束，AC-16）
#
# 用法：
#   bash scripts/check-viz.sh            # 校验，任一失败退出码 1
#   bash scripts/check-viz.sh --report   # 只报告，不以非零码退出
#
# 退出码：0 全部通过；1 存在违规；2 运行错误。

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

REPORT_FLAG=()
if [ "${1:-}" = "--report" ]; then
  REPORT_FLAG=(--report)
fi

STATUS=0

echo "==> [1/3] Relation Registry lint"
node scripts/viz-registry-lint.js "${REPORT_FLAG[@]}" || STATUS=1

echo "==> [2/3] VizSpec 字段纯净性 lint"
node scripts/viz-spec-lint.js "${REPORT_FLAG[@]}" || STATUS=1

echo "==> [3/3] 零业务写路径 lint"
bash scripts/viz-no-write-path.sh "${REPORT_FLAG[@]}" || STATUS=1

if [ "$STATUS" -eq 0 ]; then
  echo "==> ✓ 可视化门禁全部通过"
else
  echo "==> ✗ 可视化门禁存在违规"
fi
exit "$STATUS"
