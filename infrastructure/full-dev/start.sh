#!/usr/bin/env bash
# start.sh —— 全栈部署单元启动（infra + observability + lmms services + ai-pm）
# ─────────────────────────────────────────────────────────────────────────────
# 用法：
#   ./start.sh               # 构建前端 dist + ai-pm 四服务镜像，然后 up（默认 prod 片）
#   ./start.sh --no-build    # 跳过全部构建，直接 up（沿用现有镜像与 dist）
#   ./start.sh --no-fe-build # 跳过 pnpm dist 构建（dist 已是最新时用），仍构建 ai-pm 镜像
#   ./start.sh --remote      # 跨主机分置片（保留业务端口供 consul 回拨）
#   ./start.sh --logs        # 叠加日志栈（loki + promtail）
#   ./start.sh --force-recreate  # 其余参数透传 docker compose up
#
# 构建口径（与 start.ps1 对齐，2026-11-11 重写，替换旧 file-service 存在性探测）：
#   - 仅 ai-pm 四服务本地构建：pm-api / pm-worker / ai-pm-worker / ai-frontend-web，
#     共用仓库根 Dockerfile 按 target 出镜像（见 docker-compose.ai-pm.yml）。
#   - ai-frontend-web 前置：ai-frontend/dist 须先在宿主产出（pnpm build），
#     根 Dockerfile 只装 nginx。默认自动执行；--no-fe-build 或 --no-build 跳过。
#   - 其余 lmms 服务一律用 .image.env 的 ${IMAGE_*} 镜像，不构建。
#
# env 加载（后者覆盖前者）：
#   1. .env       —— 环境配置（IP / 口令 / 拓扑）
#                  向上回退：$DIR/../../.env → $DIR/../.env → $DIR/.env
#   2. .env.prod  —— 运维覆盖层（可选；缺失不阻塞）
#   3. .image.env —— **镜像钉版**，最后加载 → 权威
#
# compose 的 image: 只引用 ${IMAGE_*}，值来自 .image.env；缺变量直接报错。
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$DIR/../.." && pwd)"
PROJECT="superpowers-full"
OVERLAY="prod"
NO_BUILD=0
NO_FE_BUILD=0
WITH_LOGS=0
UP_ARGS=()
# 仅 ai-pm 自有服务需要本地构建（docker-compose.ai-pm.yml 定义）
AIPM_SERVICES=(pm-api pm-worker ai-pm-worker ai-frontend-web)

# ── 双横线参数归一（与 start.ps1 同款）─────────────────────────────────────
for arg in "$@"; do
  case "$arg" in
    --remote)       OVERLAY="remote" ;;
    --prod)         OVERLAY="prod" ;;
    --logs)         WITH_LOGS=1 ;;
    --no-build)     NO_BUILD=1 ;;
    --no-fe-build)  NO_FE_BUILD=1 ;;
    --help|-h)      grep '^#' "$0" | sed '1d;$d' | sed 's/^# \{0,1\}//'; exit 0 ;;
    *)              UP_ARGS+=("$arg") ;;
  esac
done

log() { echo "[start:$PROJECT] $*"; }
die() { echo "[start:$PROJECT] FATAL: $*" >&2; exit "${2:-10}"; }

# ── 基础文件校验 ─────────────────────────────────────────────────────────────
[[ -f "$DIR/docker-compose.yml" ]]                        || die 'docker-compose.yml not found next to start.sh'
[[ -f "$DIR/docker-compose-services.$OVERLAY.yml" ]]      || die "overlay docker-compose-services.$OVERLAY.yml not found"
[[ -f "$DIR/.image.env" ]]                                || die ".image.env not found in $DIR"
[[ -f "$REPO_ROOT/Dockerfile" ]]                          || die "repo-root Dockerfile not found: $REPO_ROOT"

# ── env 分层加载（对齐 bash `set -a; source` 语义） ──────────────────────────
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

# ── REQUIRED 校验 ────────────────────────────────────────────────────────────
for v in DATABASE_URL RABBITMQ_DEFAULT_PASS POSTGRES_PASSWORD \
         IAM_DEV_BT TOKEN_DEV_BT NOTIFICATION_DEV_BT AUDIT_DEV_BT \
         PRUE_DEV_BT GATEWAY_DEV_BT FILE_DEV_BT; do
  [[ -z "${!v:-}" ]] && die "env var $v empty (set in .env)"
  [[ "${!v}" == "__SET_BY_OPS__" ]] && die "env var $v still __SET_BY_OPS__ placeholder"
done

for v in IMAGE_API_GATEWAY IMAGE_TOKEN_SERVICE IMAGE_IAM_SERVICE \
         IMAGE_NOTIFICATION_SERVICE IMAGE_FILE_SERVICE IMAGE_AUDIT_SERVICE \
         IMAGE_PRUE_BACKEND IMAGE_CONSUL IMAGE_REDIS IMAGE_RABBITMQ IMAGE_POSTGRES \
         IMAGE_PROMETHEUS IMAGE_ALERTMANAGER IMAGE_GRAFANA IMAGE_CONSUL_TEMPLATE \
         IMAGE_OTEL_COLLECTOR IMAGE_JAEGER; do
  [[ -z "${!v:-}" ]] && die "env var $v empty (set in .image.env)"
done
if [[ "$WITH_LOGS" == "1" ]]; then
  for v in IMAGE_LOKI IMAGE_PROMTAIL; do
    [[ -z "${!v:-}" ]] && die "env var $v empty (set in .image.env; needed by --logs)"
  done
fi

# ── compose env 文件（手工执行 compose 时也能复现同一份展开） ────────────────
COMPOSE_ENV_ARGS=()
for f in "$DIR/.env" "$DIR/.env.prod" "$DIR/.image.env"; do
  [[ -f "$f" ]] && COMPOSE_ENV_ARGS+=(--env-file "$f")
done

# ── compose 文件链（与 start.ps1 同款）──────────────────────────────────────
# docker-compose.yml 已 include infra / observability / services / ai-pm 四片；
# overlay 叠加在顶层覆盖业务服务端口与拓扑。
COMPOSE_FILES=(-f docker-compose.yml -f "docker-compose-services.$OVERLAY.yml")
[[ "$WITH_LOGS" == "1" ]] && COMPOSE_FILES+=(-f docker-compose-observability.logs.yml)

# ── 前端 dist 构建（ai-frontend-web 的前置产物） ─────────────────────────────
if [[ "$NO_BUILD" != "1" && "$NO_FE_BUILD" != "1" ]]; then
  FE_DIR="$REPO_ROOT/ai-frontend"
  [[ -f "$FE_DIR/package.json" ]] || die "ai-frontend not found: $FE_DIR"
  command -v pnpm >/dev/null 2>&1 \
    || die 'pnpm not found in PATH（前端构建需要 node>=22 / pnpm>=11；或用 --no-fe-build 跳过）'
  log "frontend dist build (pnpm build)..."
  (cd "$FE_DIR" && pnpm build) || die 'pnpm build failed' 15
fi

# ── ai-pm 镜像构建（仅四服务；其余服务全走 .image.env 镜像） ─────────────────
if [[ "$NO_BUILD" != "1" ]]; then
  log "build ai-pm images: ${AIPM_SERVICES[*]}"
  docker compose -p "$PROJECT" "${COMPOSE_ENV_ARGS[@]+"${COMPOSE_ENV_ARGS[@]}"}" \
    "${COMPOSE_FILES[@]}" build "${AIPM_SERVICES[@]}" || die 'docker compose build failed' 15
else
  log 'build skipped (--no-build)'
fi

# ── 启动 ─────────────────────────────────────────────────────────────────────
docker network create superpowers >/dev/null 2>&1 || true

cd "$DIR"
log "up (overlay: $OVERLAY)..."
docker compose -p "$PROJECT" "${COMPOSE_ENV_ARGS[@]+"${COMPOSE_ENV_ARGS[@]}"}" \
  "${COMPOSE_FILES[@]}" \
  up -d ${UP_ARGS[@]+"${UP_ARGS[@]}"} \
  || { echo "[start:$PROJECT] FATAL: up failed" >&2; exit 20; }

n=$(docker compose -p "$PROJECT" ps --services 2>/dev/null | wc -l)
log "running: $n services — frontend :8092 | gateway :3000 | grafana :3001 | prometheus :9090 | jaeger :16686 | consul :8500"
