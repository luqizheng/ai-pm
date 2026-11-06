#!/usr/bin/env bash
# Team QA · 一键执行入口（test-case/team-qa-test-cases-v1.md 的自动化证据）
#
# 两层：
#   L1 后端集成  cargo test：team_onboarding（16）+ team_onboarding_qa（33，含 3 个待裁决 ignore）
#               + tenant_members / project_members 回归；sqlx::test 每用例独立临时库。
#   L2 UI E2E   Playwright（ai-frontend/playwright.team.config.ts）：独立 pm-api（jwks 模式，
#               自签令牌）+ 一次性空库 + vite :8850；不使用 mock fixtures。
#
# 数据保护：只使用 QA 专用 PG 容器（默认 aipm-teamqa-pg:16499，不存在则自动创建）。
#           绝不连接 infrastructure-aipm-pg-1 的 aipm 库，也不碰共享 PG 的 template1。
#
# 迁移撞号（TEAM-QA-DEF-01，P0）：master 上 0044 有两个文件，sqlx 迁移主键冲突。
#   默认模式 = 直接在仓库上跑 → 撞号未修复时 L1 全部失败、L2 的 pm-api 无法启动，如实报红。
#   --shim     = 在仓库外临时副本里把 0044_s02_i3 顺移为 0045 后执行（不改仓库），
#                结果只代表「以顺移为前提」的团队逻辑，报告中须注明。
#
# 用法：bash test-case/evidence/team-qa/run-team-qa.sh [--shim] [--only-backend|--only-e2e] [--ignored]
# 退出码：0 = 全部通过；非 0 = 有失败（失败层见日志）。
set -uo pipefail

EV="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$EV/../../.." && pwd)"
SHIM=0; RUN_BACKEND=1; RUN_E2E=1; RUN_IGNORED=0
for arg in "$@"; do
  case "$arg" in
    --shim) SHIM=1 ;;
    --only-backend) RUN_E2E=0 ;;
    --only-e2e) RUN_BACKEND=0 ;;
    --ignored) RUN_IGNORED=1 ;;
    *) echo "unknown arg: $arg" >&2; exit 2 ;;
  esac
done

PG_CONTAINER="${TEAM_QA_PG_CONTAINER:-aipm-teamqa-pg}"
PG_PORT="${TEAM_QA_PG_PORT:-16499}"
STAMP="$(date +%Y%m%d-%H%M%S)"
LOG="$EV/run-$STAMP.log"
export NO_PROXY="127.0.0.1,localhost${NO_PROXY:+,$NO_PROXY}"

say() { echo "[$(date +%H:%M:%S)] $*" | tee -a "$LOG"; }

# ── QA 专用 PG ─────────────────────────────────────────────────────────────
if ! docker ps --format '{{.Names}}' | grep -qx "$PG_CONTAINER"; then
  if docker ps -a --format '{{.Names}}' | grep -qx "$PG_CONTAINER"; then
    docker start "$PG_CONTAINER" >/dev/null
  else
    say "create QA postgres $PG_CONTAINER on :$PG_PORT"
    docker run -d --name "$PG_CONTAINER" -e POSTGRES_USER=aipm -e POSTGRES_PASSWORD=aipm \
      -e POSTGRES_DB=aipm -p "$PG_PORT:5432" postgres:16 >/dev/null
  fi
  for _ in $(seq 1 30); do docker exec "$PG_CONTAINER" pg_isready -U aipm -q && break; sleep 1; done
fi
# sqlx::test 的临时库残留清理（只清本容器）。
docker exec "$PG_CONTAINER" psql -U aipm -d postgres -tAc \
  "SELECT 'DROP DATABASE \"'||datname||'\" WITH (FORCE);' FROM pg_database WHERE datname LIKE '_sqlx_test_%'" \
  | docker exec -i "$PG_CONTAINER" psql -U aipm -d postgres -q >/dev/null 2>&1 || true
export DATABASE_URL="postgres://aipm:aipm@127.0.0.1:$PG_PORT/aipm"

# ── 迁移版本唯一性预检 ──────────────────────────────────────────────────────
DUP="$(ls "$REPO/migrations" | grep -oE '^[0-9]+' | sort | uniq -d | tr '\n' ' ')"
if [ -n "$DUP" ]; then
  say "PRECHECK FAIL TEAM-QA-DEF-01: duplicate migration versions: $DUP"
fi

WORK="$REPO"
if [ "$SHIM" = 1 ]; then
  WORK="$(cd "$REPO/.." && pwd)/.team-qa-shim"
  say "shim mode: export HEAD + working-tree test files to $WORK (repo untouched)"
  rm -rf "$WORK"; mkdir -p "$WORK"
  git -C "$REPO" archive HEAD | tar -x -C "$WORK"
  cp "$REPO/apps/pm-api/tests/team_onboarding_qa.rs" "$WORK/apps/pm-api/tests/"
  if [ -f "$WORK/migrations/0044_s02_i3_source_traceability.sql" ] && [ -f "$WORK/migrations/0044_team_onboarding.sql" ]; then
    mv "$WORK/migrations/0044_s02_i3_source_traceability.sql" "$WORK/migrations/0045_s02_i3_source_traceability.sql"
    say "shim: 0044_s02_i3_source_traceability.sql -> 0045 (QA copy only)"
  fi
fi
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$REPO/target}"

FAIL=0

# ── L1 后端 ────────────────────────────────────────────────────────────────
if [ "$RUN_BACKEND" = 1 ]; then
  say "L1 backend: cargo test (team_onboarding, team_onboarding_qa, tenant_members, project_members)"
  (cd "$WORK" && cargo test --no-fail-fast -p pm-api --test team_onboarding --test team_onboarding_qa \
      --test tenant_members --test project_members 2>&1) | tee "$EV/backend-$STAMP.log" \
      | grep -E '^test |test result|panicked' | tee -a "$LOG"
  [ "${PIPESTATUS[0]}" -eq 0 ] || { FAIL=1; say "L1 backend: FAILED"; }
  if [ "$RUN_IGNORED" = 1 ]; then
    say "L1 pending-decision probes (--ignored; failures here document current behaviour)"
    (cd "$WORK" && cargo test -p pm-api --test team_onboarding_qa -- --ignored 2>&1) \
      | tee "$EV/backend-ignored-$STAMP.log" | grep -E '^test |test result|left:|right:' | tee -a "$LOG"
  fi
fi

# ── L2 UI E2E ──────────────────────────────────────────────────────────────
if [ "$RUN_E2E" = 1 ]; then
  say "L2 e2e: build pm-api ($( [ "$SHIM" = 1 ] && echo shim || echo repo ))"
  (cd "$WORK" && cargo build -q -p pm-api --bin pm-api 2>&1 | tail -5) | tee -a "$LOG"
  BIN_DIR="$(cd "$REPO/.." && pwd)/.team-qa-bin"
  mkdir -p "$BIN_DIR"
  cp "$CARGO_TARGET_DIR/debug/pm-api.exe" "$BIN_DIR/pm-api.exe" 2>/dev/null \
    || cp "$CARGO_TARGET_DIR/debug/pm-api" "$BIN_DIR/pm-api"
  export TEAM_QA_PM_API_BIN="$(ls "$BIN_DIR"/pm-api* | head -1)"
  export TEAM_QA_PG_CONTAINER="$PG_CONTAINER" TEAM_QA_PG_PORT="$PG_PORT"
  export TEAM_QA_LOG="$EV/pm-api-e2e-$STAMP.log"
  say "L2 e2e: playwright (playwright.team.config.ts)"
  (cd "$REPO/ai-frontend" && pnpm exec playwright test -c playwright.team.config.ts 2>&1) \
    | tee "$EV/e2e-$STAMP.log" | grep -E '✓|✘|-  |passed|failed|skipped|flaky' | tee -a "$LOG"
  [ "${PIPESTATUS[0]}" -eq 0 ] || { FAIL=1; say "L2 e2e: FAILED"; }
fi

say "done: $( [ "$FAIL" = 0 ] && echo ALL GREEN || echo FAILURES ) (log: $LOG)"
exit "$FAIL"
