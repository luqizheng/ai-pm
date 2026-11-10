#!/usr/bin/env bash
# P0 发布验收 · 可执行套件一键入口（docs/e2e-spec/11~17 号规格的落地）
#
# 载体：Playwright（MS Edge headless 1440×900 zh-CN）+ 真 vite(:8850, VITE_USE_MOCK=0)
#        + 真 pm-api(:18490, jwks) + 一次性库(aipm_teamqa_e2e)。
# 与 team-qa 共用栈与端口（串行复用，不可并行）；证据落 test-case/evidence/p0/。
#
# 用法：
#   bash test-case/evidence/p0/run-p0.sh                # 全量（当前已落地 = 阶段 A：P0-001a/002/003/004/005/006/007）
#   TEAM_QA_KEEP_DB=1 bash test-case/evidence/p0/run-p0.sh   # 保留一次性库（调试）
#
# 退出码：0 = 全部通过；非 0 = 有失败（现场见 evidence 目录）。
set -uo pipefail

EV="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$EV/../../.." && pwd)"
STAMP="$(date +%Y%m%d-%H%M%S)"
LOG="$EV/run-$STAMP.log"

say() { echo "[$(date +%H:%M:%S)] $*" | tee -a "$LOG"; }

# ── QA PG（team-qa 专用容器；127.0.0.1，禁 localhost） ──────────────────
PG_CONTAINER="${TEAM_QA_PG_CONTAINER:-aipm-teamqa-pg}"
PG_PORT="${TEAM_QA_PG_PORT:-16499}"
if ! docker ps --format '{{.Names}}' | grep -qx "$PG_CONTAINER"; then
  if docker ps -a --format '{{.Names}}' | grep -qx "$PG_CONTAINER"; then
    say "start QA postgres container $PG_CONTAINER"
    docker start "$PG_CONTAINER" >/dev/null
  else
    say "create QA postgres $PG_CONTAINER on :$PG_PORT"
    docker run -d --name "$PG_CONTAINER" -e POSTGRES_USER=aipm -e POSTGRES_PASSWORD=aipm \
      -e POSTGRES_DB=aipm -p "$PG_PORT:5432" postgres:16 >/dev/null
  fi
  for _ in $(seq 1 30); do docker exec "$PG_CONTAINER" pg_isready -U aipm -q && break; sleep 1; done
fi
export NO_PROXY="127.0.0.1,localhost${NO_PROXY:+,$NO_PROXY}"
export no_proxy="$NO_PROXY"

# ── 构建 pm-api（global-setup 由 TEAM_QA_PM_API_BIN 拉起） ───────────────
say "build: pm-api"
cd "$REPO" || exit 1
if ! cargo build -q -p pm-api --bin pm-api 2>&1 | tail -3 | tee -a "$LOG"; then
  say "BUILD FAIL: pm-api"; exit 1
fi
BIN="$REPO/target/debug/pm-api.exe"
[ -x "$BIN" ] || BIN="$REPO/target/debug/pm-api"
export TEAM_QA_PM_API_BIN="$BIN"
export TEAM_QA_PG_CONTAINER="$PG_CONTAINER"
export TEAM_QA_PG_PORT="$PG_PORT"
export TEAM_QA_LOG="$EV/pm-api-$STAMP.log"

# ── 执行 ───────────────────────────────────────────────────────────────
say "run: playwright p0 suite (vite :8850 / pm-api :18490 / jwks :18491)"
cd "$REPO/ai-frontend" || exit 1
pnpm exec playwright test -c playwright.p0.config.ts 2>&1 \
  | tee "$EV/e2e-$STAMP.log" \
  | grep -E '✓|✘|passed|failed|skipped|flaky|Error' | tee -a "$LOG"
STATUS="${PIPESTATUS[0]}"

say "evidence: $EV/e2e-$STAMP.log / playwright-report/ / $TEAM_QA_LOG"
say "done: $([ "$STATUS" = 0 ] && echo ALL GREEN || echo FAILURES) (log: $LOG)"
exit "$STATUS"
