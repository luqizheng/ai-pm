#!/usr/bin/env bash
# E2E 全流程一键执行入口（test-case/e2e-test-plan-v1.md · FLOW-01 ~ FLOW-16）
#
# 载体：真 axum router + 真 PostgreSQL + **真 Git 远端**（SSH over 本机真实私钥）。
# 仓库：GitLab `git@192.168.2.16:tooling/ai-pm.git`
#        GitHub `git@github.com:luqizheng/world-view-rs.git`
#
# 前置检查（第 0 步，失败即中止——不做"假装通过"）：
#   1. 本机私钥可读且是 ED25519（`~/.ssh/id_rsa`，可用 AI_PM_E2E_SSH_KEY 覆盖）
#   2. 两个真实远端都能用该私钥 `git ls-remote`（用平台同款 GIT_SSH_COMMAND）
#   3. QA PG 可 CREATE DATABASE（sqlx::test 每用例独立临时库）
#
# 用法：
#   bash test-case/evidence/e2e-flow/run-e2e-flow.sh                # 全量
#   bash test-case/evidence/e2e-flow/run-e2e-flow.sh --skip-ssh-check  # 跳过 SSH 前置（仅本地排障）
#
# 退出码：0 = 全绿；非 0 = 有失败（失败详情见 $EV/e2e-<stamp>.log）。
set -uo pipefail

EV="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$EV/../../.." && pwd)"
STAMP="$(date +%Y%m%d-%H%M%S)"
LOG="$EV/e2e-$STAMP.log"

SKIP_SSH_CHECK=0
for arg in "$@"; do
  case "$arg" in
    --skip-ssh-check) SKIP_SSH_CHECK=1 ;;
    *) echo "unknown arg: $arg" >&2; exit 2 ;;
  esac
done

say() { echo "[$(date +%H:%M:%S)] $*" | tee -a "$LOG"; }

# ── 环境：QA PG（127.0.0.1，不用 localhost——本机 IPv6 回环是黑洞） ──────
PG_CONTAINER="${E2E_FLOW_PG_CONTAINER:-aipm-pg-gate}"
PG_PORT="${E2E_FLOW_PG_PORT:-16433}"
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
export DATABASE_URL="postgres://aipm:aipm@127.0.0.1:${PG_PORT}/aipm"
export NO_PROXY="127.0.0.1,localhost${NO_PROXY:+,$NO_PROXY}"
export no_proxy="$NO_PROXY"

# sqlx::test 临时库残留清理（只清本容器，不碰共享 PG 的 template1）。
docker exec "$PG_CONTAINER" psql -U aipm -d postgres -tAc \
  "SELECT 'DROP DATABASE \"'||datname||'\" WITH (FORCE);' FROM pg_database WHERE datname LIKE '_sqlx_test_%'" \
  | docker exec -i "$PG_CONTAINER" psql -U aipm -d postgres -q >/dev/null 2>&1 || true

# ── 第 0 步：SSH 前置检查 ───────────────────────────────────────────────
SSH_KEY="${AI_PM_E2E_SSH_KEY:-$HOME/.ssh/id_rsa}"
if [ "$SKIP_SSH_CHECK" = 0 ]; then
  say "precheck: ssh private key at $SSH_KEY"
  if [ ! -r "$SSH_KEY" ]; then
    say "PRECHECK FAIL: private key not readable at $SSH_KEY"
    exit 1
  fi
  if ! ssh-keygen -l -f "$SSH_KEY" >/dev/null 2>&1; then
    say "PRECHECK FAIL: ssh-keygen cannot read $SSH_KEY"
    exit 1
  fi
  # 平台同款 GIT_SSH_COMMAND（与 crates/pm-application/src/git_access.rs 一致）。
  KEY_WIN="$(cygpath -w "$SSH_KEY" 2>/dev/null || echo "$SSH_KEY")"
  KH="$(mktemp -d)/known_hosts"
  KH_WIN="$(cygpath -w "$KH" 2>/dev/null || echo "$KH")"
  export GIT_SSH_COMMAND="ssh -i \"$KEY_WIN\" -o IdentitiesOnly=yes -o BatchMode=yes -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile=\"$KH_WIN\""
  for repo in "${AI_PM_E2E_REPO_GITLAB:-git@192.168.2.16:tooling/ai-pm.git}" \
              "${AI_PM_E2E_REPO_GITHUB:-git@github.com:luqizheng/world-view-rs.git}"; do
    say "precheck: git ls-remote $repo"
    if ! git ls-remote --heads "$repo" 2>&1 | head -3 | tee -a "$LOG" | grep -q 'refs/heads/'; then
      say "PRECHECK FAIL: cannot list remote heads of $repo with the local private key"
      exit 1
    fi
  done
  unset GIT_SSH_COMMAND
  say "precheck: OK"
fi

# ── 执行 ───────────────────────────────────────────────────────────────
say "run: cargo nextest run -p pm-api --test e2e_flow --no-fail-fast"
cd "$REPO" || exit 1
cargo nextest run -p pm-api --test e2e_flow --no-fail-fast 2>&1 \
  | tee "$EV/e2e-nextest-$STAMP.log" \
  | grep -E 'PASS|FAIL|Summary|tests run' | tee -a "$LOG"
STATUS="${PIPESTATUS[0]}"

say "evidence: $EV/e2e-nextest-$STAMP.log"
say "done: $([ "$STATUS" = 0 ] && echo ALL GREEN || echo FAILURES) (log: $LOG)"
exit "$STATUS"
