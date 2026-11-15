#!/usr/bin/env bash
# 阶段 D · P1 真实业务链 E2E 一键入口（交付物 05 V1.1 §1 / §5 G3~G4）
#
# 载体：Playwright（API 级，无浏览器）+ 真 pm-api(:18490, jwks) + 真 ai-pm-worker
#       + mock-llm（FakeModel v1）+ 一次性库 aipm_chain_e2e。
# 与 P0/Team QA 共用栈与端口（串行复用，不可并行）；证据落 test-case/evidence/chain/。
#
# 用法：
#   bash test-case/evidence/chain/run-chain.sh
#   TEAM_QA_KEEP_DB=1 bash test-case/evidence/chain/run-chain.sh   # 保留库调试
#
# 退出码：0 = 全部通过；非 0 = 有失败（现场见 evidence 目录）。
set -uo pipefail

EV="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$EV/../../.." && pwd)"
STAMP="$(date +%Y%m%d-%H%M%S)"
LOG="$EV/run-$STAMP.log"
mkdir -p "$EV"

say() { echo "[$(date +%H:%M:%S)] $*" | tee -a "$LOG"; }

# ── QA PG（与 P0 同一容器；127.0.0.1，禁 localhost） ─────────────────────
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

# ── 构建（pm-api + ai-pm-worker；业务链不跑 Run 执行器） ──────────────────
say "build: pm-api + ai-pm-worker"
cd "$REPO" || exit 1
if ! cargo build -q -p pm-api --bin pm-api 2>&1 | tail -3 | tee -a "$LOG"; then
  say "BUILD FAIL: pm-api"; exit 1
fi
if ! cargo build -q -p ai-pm-worker --bin ai-pm-worker 2>&1 | tail -3 | tee -a "$LOG"; then
  say "BUILD FAIL: ai-pm-worker"; exit 1
fi
BIN="$REPO/target/debug/pm-api.exe"
[ -x "$BIN" ] || BIN="$REPO/target/debug/pm-api"
export TEAM_QA_PM_API_BIN="$BIN"
WORKER="$REPO/target/debug/ai-pm-worker.exe"
[ -x "$WORKER" ] || WORKER="$REPO/target/debug/ai-pm-worker"
export P0_WORKER_BIN="$WORKER"

# rust-lld 链接器不读 VS 安装注册表，LIB 缺失即报 secur32.lib 找不到
SDK_LIB="C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0"
MSVC_LIB="$(ls -d "/c/Program Files/Microsoft Visual Studio/18/Community/VC/Tools/MSVC/"*/lib/x64 2>/dev/null | head -1 | sed 's|^/c/|C:/|; s|/|\\|g')"
if [ -n "$MSVC_LIB" ]; then
  export LIB="$SDK_LIB\um\x64;$SDK_LIB\ucrt\x64;$MSVC_LIB"
  export INCLUDE="$SDK_LIB\..\..\Include\10.0.26100.0\um;$SDK_LIB\..\..\Include\10.0.26100.0\ucrt;$SDK_LIB\..\..\Include\10.0.26100.0\shared"
  say "LIB set for rust-lld (MSVC CRT found)"
fi
export TEAM_QA_PG_CONTAINER="$PG_CONTAINER"
export TEAM_QA_PG_PORT="$PG_PORT"
export AI_PM_SECRET_KEY="${AI_PM_SECRET_KEY:-e2e-chain-secret}"
export P0_DB="${P0_DB:-aipm_chain_e2e}"
export P0_LOG_DIR="$EV"
export RUST_LOG="${RUST_LOG:-info}"

# ── 执行 ───────────────────────────────────────────────────────────────
say "run: playwright chain suite (pm-api :18490 / jwks :18491 / db $P0_DB)"
cd "$REPO/ai-frontend" || exit 1
pnpm exec playwright test -c playwright.chain.config.ts 2>&1 \
  | tee "$EV/e2e-$STAMP.log" \
  | grep -E '✓|✘|passed|failed|skipped|flaky|Error' | tee -a "$LOG"
STATUS="${PIPESTATUS[0]}"

say "evidence: $EV/e2e-$STAMP.log / playwright-report/ / pm-api-*.log / worker logs"
say "done: $([ "$STATUS" = 0 ] && echo ALL GREEN || echo FAILURES) (log: $LOG)"
exit "$STATUS"
