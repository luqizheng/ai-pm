#!/usr/bin/env bash
# E2E 全流程 · UI 套件一键入口（UIFLOW-01~08，test-case/e2e-test-plan-v1.md §4.2）
#
# 载体：Playwright（MS Edge headless 1440×900 zh-CN）+ 真 vite(:8860, VITE_USE_MOCK=0)
#        + 真 pm-api(:18495, jwks) + 一次性库(aipm_e2e_flow_ui)。
# 与 L2 服务级套件（run-e2e-flow.sh，端口 18495 系）共用 aipm-pg-gate:16433，库不同互不干扰。
#
# 用法：
#   bash test-case/evidence/e2e-flow/run-e2e-flow-ui.sh              # 全量
#   FLOW_E2E_KEEP_DB=1 bash ... run-e2e-flow-ui.sh                   # 保留一次性库（调试）
#   FLOW_E2E_SKIP_SSH=1 bash ... run-e2e-flow-ui.sh                  # 跳过真实仓库用例（UIFLOW-06 标 skip）
#
# 退出码：0 = 全部通过（skip 不算失败）；非 0 = 有失败（现场见 evidence 目录）。
set -uo pipefail

EV="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$EV/../../.." && pwd)"
STAMP="$(date +%Y%m%d-%H%M%S)"
LOG="$EV/run-ui-$STAMP.log"

say() { echo "[$(date +%H:%M:%S)] $*" | tee -a "$LOG"; }

# ── QA PG（127.0.0.1，禁 localhost——本机 IPv6 回环是黑洞） ──────────────
PG_CONTAINER="${FLOW_E2E_PG_CONTAINER:-aipm-pg-gate}"
if ! docker ps --format '{{.Names}}' | grep -qx "$PG_CONTAINER"; then
  if docker ps -a --format '{{.Names}}' | grep -qx "$PG_CONTAINER"; then
    say "start QA postgres container $PG_CONTAINER"
    docker start "$PG_CONTAINER" >/dev/null
  else
    say "PRECHECK FAIL: QA postgres container $PG_CONTAINER not found"
    exit 1
  fi
  for _ in $(seq 1 30); do
    docker exec "$PG_CONTAINER" pg_isready -U aipm -q && break
    sleep 1
  done
fi
export NO_PROXY="127.0.0.1,localhost${NO_PROXY:+,$NO_PROXY}"
export no_proxy="$NO_PROXY"

# ── SSH 前置（UIFLOW-06 用真实远端；缺失时该用例自动 skip，不算失败） ────
SSH_KEY="${AI_PM_E2E_SSH_KEY:-$HOME/.ssh/id_rsa}"
if [ -r "$SSH_KEY" ] && [ "${FLOW_E2E_SKIP_SSH:-0}" != "1" ]; then
  say "precheck: ssh key at $SSH_KEY readable — UIFLOW-06 enabled"
else
  say "precheck: ssh key unavailable or skipped — UIFLOW-06 will skip"
  export FLOW_E2E_SKIP_SSH=1
fi

# ── 构建 pm-api + 私钥封装助手（flow/stack.ts 的 sealPrivateKey 依赖后者） ──
say "build: pm-api + pm-application example seal_ssh_key"
cd "$REPO" || exit 1
if ! cargo build -q -p pm-api --bin pm-api 2>&1 | tail -3 | tee -a "$LOG"; then
  say "BUILD FAIL: pm-api"; exit 1
fi
if ! cargo build -q -p pm-application --example seal_ssh_key 2>&1 | tail -3 | tee -a "$LOG"; then
  say "BUILD FAIL: seal_ssh_key"; exit 1
fi
BIN="$REPO/target/debug/pm-api.exe"
[ -x "$BIN" ] || BIN="$REPO/target/debug/pm-api"
export FLOW_E2E_PM_API_BIN="$BIN"
export FLOW_E2E_LOG="$EV/pm-api-ui-$STAMP.log"
export AI_PM_SECRET_KEY="${AI_PM_SECRET_KEY:-e2e-flow-ui-secret}"

# ── 执行 ───────────────────────────────────────────────────────────────
say "run: playwright flow suite (vite :8860 / pm-api :18495 / jwks :18496)"
cd "$REPO/ai-frontend" || exit 1
pnpm exec playwright test -c playwright.flow.config.ts 2>&1 \
  | tee "$EV/e2e-ui-$STAMP.log" \
  | grep -E '✓|✘|passed|failed|skipped|flaky|Error' | tee -a "$LOG"
STATUS="${PIPESTATUS[0]}"

say "evidence: $EV/e2e-ui-$STAMP.log / playwright-ui-report/ / $FLOW_E2E_LOG"
say "done: $([ "$STATUS" = 0 ] && echo ALL GREEN || echo FAILURES) (log: $LOG)"
exit "$STATUS"
