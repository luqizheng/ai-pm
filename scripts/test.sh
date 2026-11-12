#!/usr/bin/env bash
# 本地测试统一入口（把「快路径」固化成默认路径）。
#
#   bash scripts/test.sh                 # 内环：fast profile（排除真远端 SSH 套件）
#   bash scripts/test.sh --full          # 全套：含 e2e_flow 真远端（需要网络 + SSH key）
#   bash scripts/test.sh -p pm-application --test docproc_pipeline
#   bash scripts/test.sh --timing        # 附带耗时画像（调 scripts/test-timing.sh）
#
# 做的事（顺序有意义）：
#   1. 确保测试库容器在跑且已按测试场景调优（fsync=off 等，见 setup-test-db.sh）
#   2. 用 nextest 跑（进程级并行 + per-test 计时）
#   3. 有失败时打印失败用例清单
#
# 不用 `cargo test`：libtest 的 --report-time 是 nightly-only，且 cargo test 按
# 二进制串行（本仓 173 个测试二进制）。详见 docs/ai-pm-test-performance-v1.md。

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

: "${DATABASE_URL:=postgres://aipm:aipm@127.0.0.1:16433/aipm}"
export DATABASE_URL
export CARGO_HTTP_PROXY=""
export NO_PROXY="${NO_PROXY:-192.168.2.12}"

PROFILE="fast"
TIMING=0
ARGS=()
for arg in "$@"; do
  case "$arg" in
    --full) PROFILE="default" ;;
    --timing) TIMING=1 ;;
    *) ARGS+=("$arg") ;;
  esac
done

# 1. 测试库就绪 + 调优（缺配置只警告：慢但能跑）
if ! bash "$REPO_ROOT/scripts/setup-test-db.sh" --check >/dev/null 2>&1; then
  echo "==> 测试库未调优（每用例迁移 ≈380ms）。修：bash scripts/setup-test-db.sh"
fi

# 2. 计时画像走专用脚本（它自带编译/执行分段与汇总）
if [ "$TIMING" = "1" ]; then
  exec bash "$REPO_ROOT/scripts/test-timing.sh" "${ARGS[@]+"${ARGS[@]}"}"
fi

# 3. 源码级门禁（快、零编译；违规即失败，详情见各自脚本头注释）
bash "$REPO_ROOT/scripts/guard-pm-api-sql.sh" || exit 1

LOG="$(mktemp)"
echo "==> nextest --profile $PROFILE  (DATABASE_URL=$DATABASE_URL)"
cargo nextest run --no-fail-fast --profile "$PROFILE" "${ARGS[@]+"${ARGS[@]}"}" 2>&1 | tee "$LOG"
STATUS=${PIPESTATUS[0]}

if [ "$STATUS" -ne 0 ]; then
  echo
  # 模板库版本钉死导致的失败：给出可自助的恢复命令（见 scripts/seed-test-template.sh）
  if grep -q "VersionMissing\|VersionMismatch" "$LOG"; then
    echo "==> 检测到迁移版本不匹配：template1 模板库钉在别的分支的迁移集上。"
    echo "    修：bash scripts/seed-test-template.sh --reset  （或按当前分支重新播种）"
  else
    echo "==> 失败用例见上方逐条 FAIL 行"
  fi
fi
rm -f "$LOG"
exit "$STATUS"
