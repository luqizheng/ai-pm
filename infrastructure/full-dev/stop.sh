#!/usr/bin/env bash
# stop.sh —— 停止全栈部署单元（infra + observability + lmms services + ai-pm）
# ─────────────────────────────────────────────────────────────────────────────
# 用法：
#   ./stop.sh              # down（保留 named volumes）
#   ./stop.sh --volumes    # 连 named volume 一起删（清库/清监控数据）
#   ./stop.sh --help
#
# 定位依据是 project 名，与启动时用的 overlay 片 / --logs 无关
# （compose down 按 com.docker.compose.project 标签清理全部容器，含 --logs
# 叠加的 loki/promtail）。compose 文件链只取顶层聚合 docker-compose.yml
# （其 include 已含 infra / observability / services / ai-pm 四片），
# 用于解析 networks / volumes 声明。
# 本单元的 project 名是 superpowers-full（与 docker-deploy 的 superpowers-svc 区分）。
#
# env 加载与 start.sh 完全对齐（保证新开窗口直接跑 stop 也能完成插值）：
#   .env（向上回退两级）→ .env.prod → .image.env，后者覆盖前者。
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT="superpowers-full"
DOWN_ARGS=()

for arg in "$@"; do
  case "$arg" in
    --help|-h) grep '^#' "$0" | sed '1d;$d' | sed 's/^# \{0,1\}//'; exit 0 ;;
    *)         DOWN_ARGS+=("$arg") ;;
  esac
done

log() { echo "[stop:$PROJECT] $*"; }

[[ -f "$DIR/docker-compose.yml" ]] || { echo "[stop:$PROJECT] FATAL: docker-compose.yml not found next to stop.sh" >&2; exit 10; }

# ── env 分层加载（与 start.sh 同款）─────────────────────────────────────────
load_env() {
  local f="$1"
  [[ -f "$f" ]] || return 0
  set +u; set -a
  # shellcheck disable=SC1090
  source "$f"
  set +a; set -u
  log "env: $f"
}
load_env "$DIR/../../.env"
load_env "$DIR/../.env"
load_env "$DIR/.env"
load_env "$DIR/../../.env.prod"
load_env "$DIR/../.env.prod"
load_env "$DIR/.env.prod"
load_env "$DIR/../../.image.env"
load_env "$DIR/../.image.env"
load_env "$DIR/.image.env"

# ── compose env 文件（手工执行 compose 时也能复现同一份展开） ────────────────
ENV_ARGS=()
for f in "$DIR/.env" "$DIR/.env.prod" "$DIR/.image.env"; do
  [[ -f "$f" ]] && ENV_ARGS+=(--env-file "$f")
done

# ── 停止 ─────────────────────────────────────────────────────────────────────
cd "$DIR"
log "down..."
docker compose -p "$PROJECT" "${ENV_ARGS[@]+"${ENV_ARGS[@]}"}" \
  -f docker-compose.yml down "${DOWN_ARGS[@]+"${DOWN_ARGS[@]}"}"
exit $?
