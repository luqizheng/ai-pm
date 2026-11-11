#!/usr/bin/env bash
# 测试耗时画像（编译 + 执行两阶段，per-test 计时）。
#
# 为什么用 nextest 而不是 cargo test：
#   - nextest 是进程级并行，且每条测试的输出行自带耗时（`PASS [ 2.61s] ...`）；
#   - libtest 的 --report-time 仍是 nightly-only，本仓 stable 工具链不可用。
#
# 用法：
#   bash scripts/test-timing.sh                          # 全 workspace
#   bash scripts/test-timing.sh -p pm-application        # 单包
#   bash scripts/test-timing.sh -p pm-application --test docproc_pipeline
#   SKIP_BUILD=1 bash scripts/test-timing.sh -p pm-api   # 跳过编译计时（已编好）
#
# 产出：
#   target/test-timing/nextest.log   原始输出（含每测试耗时）
#   target/test-timing/summary.txt   汇总：套件 / 慢用例 / 分布 / 编译-执行占比
#
# 数据库：必须 127.0.0.1（AGENTS.md 规则 3——localhost 走 ::1 黑洞，每用例
# 建连 4~5 次会耗尽 OS 超时）。

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="$REPO_ROOT/target/test-timing"
LOG="$OUT_DIR/nextest.log"
SUMMARY="$OUT_DIR/summary.txt"

: "${DATABASE_URL:=postgres://aipm:aipm@127.0.0.1:16433/aipm}"
export DATABASE_URL
export CARGO_HTTP_PROXY=""
export NO_PROXY="${NO_PROXY:-192.168.2.12}"

mkdir -p "$OUT_DIR"

# 测试库 durability 配置检查（fsync/full_page_writes/synchronous_commit=off）。
# 未调优时每用例迁移 ≈380ms（近 1400 个 sqlx 用例 ⇒ 全量多花 ~4 分钟）；
# 调优后 ≈65ms。只警告不阻断（调优是可选收益）。
if ! bash "$REPO_ROOT/scripts/setup-test-db.sh" --check >/dev/null 2>&1; then
  echo "!! 测试库仍是默认 durability 配置——跑 bash scripts/setup-test-db.sh 可提速约 4x" >&2
fi

echo "==> DATABASE_URL=$DATABASE_URL"
echo "==> 输出: $OUT_DIR"

BUILD_SECONDS=0
if [ "${SKIP_BUILD:-0}" != "1" ]; then
  echo "==> [1/2] 编译测试二进制（cargo build --tests）"
  BUILD_START=$(date +%s)
  cargo build --tests "$@" 2>&1 | tail -2
  BUILD_SECONDS=$(( $(date +%s) - BUILD_START ))
  echo "==> 编译阶段 ${BUILD_SECONDS}s"
fi

echo "==> [2/2] 执行（nextest；per-test 计时来自输出行）"
RUN_START=$(date +%s)
cargo nextest run --no-fail-fast --status-level all --final-status-level fail "$@" \
  >"$LOG" 2>&1
RUN_STATUS=$?
RUN_SECONDS=$(( $(date +%s) - RUN_START ))

tail -4 "$LOG"
echo "==> 编译 ${BUILD_SECONDS}s  执行 ${RUN_SECONDS}s  合计 $(( BUILD_SECONDS + RUN_SECONDS ))s"

python - "$LOG" "$SUMMARY" "$BUILD_SECONDS" "$RUN_SECONDS" <<'PY'
import re, sys
from collections import defaultdict

log_path, summary_path, build_s, run_s = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
# `        PASS [   2.614s] ( 3/13) pm-application::docproc_pipeline cancel_pending_job_and_tenant_isolation`
line_re = re.compile(
    r"^\s*(PASS|FAIL|SLOW|LEAK|TIMEOUT|SKIP|IGNORE)\s+\[\s*([0-9.]+)s\]\s+\(\s*\d+/\d+\)\s+(\S+)\s+(\S+)\s*$"
)

rows = []
binaries = set()
with open(log_path, encoding="utf-8", errors="replace") as f:
    for line in f:
        m = line_re.match(line)
        if not m:
            continue
        status, t, suite, name = m.group(1), float(m.group(2)), m.group(3), m.group(4)
        rows.append((t, suite, name, status))
        binaries.add(suite)

per_suite = defaultdict(lambda: [0.0, 0])
for t, suite, _n, _s in rows:
    per_suite[suite][0] += t
    per_suite[suite][1] += 1

rows.sort(reverse=True)
total_case = sum(r[0] for r in rows)
failed = [r for r in rows if r[3] == "FAIL"]

L = []
w = L.append
w("")
w("=" * 80)
w("测试耗时画像")
w("=" * 80)
w(f"编译阶段              : {build_s}s")
w(f"执行阶段（墙钟）      : {run_s}s")
w(f"用例 CPU 时间之和     : {total_case:.1f}s   （并行度 ≈ {total_case / run_s:.1f}x）" if run_s else "")
w(f"用例总数 / 失败       : {len(rows)} / {len(failed)}")
w("")
w("-- 按测试二进制（CPU 时间之和 / 用例数 / 均值）" + "-" * 32)
for suite, (t, n) in sorted(per_suite.items(), key=lambda kv: -kv[1][0]):
    w(f"{t:9.2f}s  n={n:3d}  avg={t / n:6.3f}s  {suite}")
w("")
w("-- 最慢 30 个用例" + "-" * 61)
for t, suite, name, status in rows[:30]:
    flag = "" if status == "PASS" else f" [{status}]"
    w(f"{t:9.3f}s  {suite}  {name}{flag}")
w("")
w("-- 耗时分布" + "-" * 68)
for lo, hi in [(0, 0.05), (0.05, 0.2), (0.2, 0.5), (0.5, 1.0), (1.0, 3.0), (3.0, 10.0), (10.0, 1e9)]:
    sel = [r for r in rows if lo <= r[0] < hi]
    if sel:
        label = f"{lo:g}–{hi:g}s" if hi < 1e9 else f">={lo:g}s"
        w(f"{label:>12}  {len(sel):4d} 例   累计 {sum(r[0] for r in sel):7.1f}s")
w("")
text = "\n".join(x for x in L if x is not None and x != "")
print(text)
with open(summary_path, "w", encoding="utf-8") as f:
    f.write(text + "\n")
print(f"汇总写入 {summary_path}")
PY

exit "$RUN_STATUS"
