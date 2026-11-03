#!/usr/bin/env bash
# infrastructure/scripts/start-prod.sh
# ─────────────────────────────────────────────────────────────────────────────
# 单入口启动 ai-pm prod（编排 = 模板基座 + ai-pm 自有服务）。配套 stop-prod.sh。
#
# 用法：
#   ./infrastructure/scripts/start-prod.sh                    # 标准 prod
#   ./infrastructure/scripts/start-prod.sh --force-recreate   # 传透到 docker compose up
#   ./infrastructure/scripts/start-prod.sh --pull             # 先 pull 再 up（强制刷新镜像）
#
# 额外 overlay（可选，放在最后叠加）：
#   COMPOSE_OVERLAY=docker-compose.test-server.yml ./infrastructure/scripts/start-prod.sh
#   COMPOSE_OVERLAY=/abs/path/to/overlay.yml ./infrastructure/scripts/start-prod.sh
#   例：测试机用 docker-compose.test-server.yml 把 pm-api 的 8090 发布到宿主，
#       供内网 Codex / Z-Code 直连 /mcp（见该文件头部说明）。
#
# 设计要点：
#   - 强制 .env.prod 存在且字段填好（fail-fast 在 compose 报错之前）
#   - compose v2.20+ 版本检查（!override 需要）
#   - env 分层加载：先 .env，再 .env.prod（后入覆盖前入）
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INFRA_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ENV_BASE="$INFRA_DIR/.env"
ENV_PROD="$INFRA_DIR/.env.prod"
COMPOSE_BASE="$INFRA_DIR/docker-compose.yml"
COMPOSE_PROD="$INFRA_DIR/docker-compose.prod.yml"

cd "$INFRA_DIR" || { echo "[start-prod] FATAL: cannot cd to $INFRA_DIR" >&2; exit 10; }

die() { echo "[start-prod] FATAL: $1" >&2; exit "${2:-10}"; }
log() { echo "[start-prod] $*"; }

# 语义化比较 compose 版本：要求 v2.20+（更高主版本如 v5/v6 直接放行）。
# 不再用正则匹配 —— 旧正则只认 v2.2x~v2.99，误杀 v5.x，还误放行 v2.10.3。
compose_version_ok() {
  local _v _major _minor
  _v="$(docker compose version --short 2>/dev/null | head -1)"
  _v="${_v#v}"
  _major="${_v%%.*}"
  _minor="$(printf '%s' "$_v" | awk -F. 'NR==1{print $2}')"
  case "$_major" in ''|*[!0-9]*) return 1 ;; esac
  case "$_minor" in ''|*[!0-9]*) return 1 ;; esac
  [ "$_major" -gt 2 ] && return 0
  [ "$_major" -eq 2 ] && [ "$_minor" -ge 20 ] && return 0
  return 1
}

# 1. .env.prod 必须存在
[[ -f "$ENV_PROD" ]] || die "$ENV_PROD not found — copy from .env.prod.example and fill values" 10

# 2. compose v2.20+ 检查
if ! compose_version_ok; then
  die "docker compose v2.20+ required (found: $(docker compose version 2>/dev/null || echo unknown))" 10
fi
log "docker compose version: $(docker compose version | head -1)"

# 3. env 分层加载（base → prod 覆盖）
# ⚠️ 必须 `set -a` 导出：compose 的变量插值只看「进程环境变量 + 项目目录 .env」，
#    不会读 .env.prod。若不导出，${DATABASE_URL} 等插值会静默回落到基础 .env
#    的值，prod 覆盖形同虚设（口令改了但容器没拿到）。
load_env() {
  local f="$1"
  [[ -f "$f" ]] || return 0
  set +u
  set -a
  # shellcheck disable=SC1090
  source "$f"
  set +a
  set -u
}
load_env "$ENV_BASE"
load_env "$ENV_PROD"

# 4. 校验关键 vars（PG 主库 + 中间件 + lmms 登录验证栈的 service-jwt bootstrap token）
for v in POSTGRES_HOST POSTGRES_PORT POSTGRES_USER POSTGRES_PASSWORD POSTGRES_DB \
         DATABASE_URL RABBITMQ_DEFAULT_PASS \
         IAM_DEV_BT TOKEN_DEV_BT NOTIFICATION_DEV_BT AUDIT_DEV_BT \
         PRUE_DEV_BT GATEWAY_DEV_BT CONFIG_DEV_BT FILE_DEV_BT; do
  if [[ -z "${!v:-}" ]]; then
    die "env var $v empty after .env.prod load" 10
  fi
  if [[ "${!v}" == "__SET_BY_OPS__" ]]; then
    die "env var $v still __SET_BY_OPS__ placeholder" 10
  fi
done

# 5. 透传剩余 args
COMPOSE_ARGS=()
DO_PULL=0
for arg in "$@"; do
  case "$arg" in
    --pull)         DO_PULL=1 ;;
    --help|-h)
      sed -n '2,12p' "$0" | sed 's/^# \?//'
      exit 0 ;;
    *)              COMPOSE_ARGS+=("$arg") ;;
  esac
done

# 5b. 额外 overlay（可选，最后叠加）：COMPOSE_OVERLAY=文件名或绝对路径
COMPOSE_FILES=(-f "$COMPOSE_BASE" -f "$COMPOSE_PROD")
if [[ -n "${COMPOSE_OVERLAY:-}" ]]; then
  _ov="$COMPOSE_OVERLAY"
  [[ "$_ov" != /* ]] && _ov="$INFRA_DIR/$_ov"
  [[ -f "$_ov" ]] || die "COMPOSE_OVERLAY not found: $_ov" 10
  COMPOSE_FILES+=(-f "$_ov")
  log "extra overlay: $_ov"
fi

# 6. up
[[ "$DO_PULL" -eq 1 ]] && docker compose "${COMPOSE_FILES[@]}" pull
docker compose "${COMPOSE_FILES[@]}" up -d "${COMPOSE_ARGS[@]}" \
  || die "docker compose up failed" 20

log "running: $(docker compose "${COMPOSE_FILES[@]}" ps --services 2>/dev/null | wc -l) services"
log "public endpoints: http://<host>:8091 (前端 aipm-web) / http://<host>:3000 (登录验证 api-gateway)"
log "private: pm-api:8090 / aipm-bff:8080 / token-service:8082 / iam-service:8081 are container-network only"
