#!/usr/bin/env bash
# infrastructure/scripts/setup-test-server.sh
# ─────────────────────────────────────────────────────────────────────────────
# 内网测试机一次性初始化：
#   1. 从 .env.prod.example 生成 .env.prod，所有 __SET_BY_OPS__ 占位符替换为随机强口令
#   2. 打印实际生效的关键变量（口令打码）供核对
#
# 幂等：.env.prod 已存在时不覆盖，只重新补齐仍为占位符的字段。
# 用法：bash infrastructure/scripts/setup-test-server.sh
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

INFRA_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$INFRA_DIR"

if [[ ! -f .env.prod.example ]]; then
  echo "[setup] FATAL: .env.prod.example not found in $INFRA_DIR" >&2
  exit 10
fi

[[ -f .env.prod ]] || cp .env.prod.example .env.prod

gen_secret() { head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n'; }

# 逐字段补齐：仍是占位符或空的才生成新值
fill() {
  local key="$1" cur val
  cur="$(grep -E "^${key}=" .env.prod | head -1 | cut -d= -f2- || true)"
  if [[ -z "$cur" || "$cur" == "__SET_BY_OPS__" ]]; then
    val="$(gen_secret)"
    sed -i "s|^${key}=.*|${key}=${val}|" .env.prod
    echo "[setup] ${key}: <生成新口令>"
  else
    echo "[setup] ${key}: <已存在，保留>"
  fi
}

echo "[setup] infra dir: $INFRA_DIR"
fill POSTGRES_PASSWORD
fill RABBITMQ_DEFAULT_PASS
for v in IAM_DEV_BT TOKEN_DEV_BT NOTIFICATION_DEV_BT AUDIT_DEV_BT \
         PRUE_DEV_BT GATEWAY_DEV_BT CONFIG_DEV_BT FILE_DEV_BT; do
  fill "$v"
done

# 只看赋值行（注释里提到 __SET_BY_OPS__ 的说明文字不算残留）
LEFT="$(grep -cE '^[[:space:]]*[A-Za-z_][A-Za-z0-9_]*=.*__SET_BY_OPS__' .env.prod || true)"
echo "[setup] 残留占位符(赋值行): ${LEFT}"
[[ "$LEFT" == "0" ]] || { echo "[setup] FATAL: 仍有占位符未填" >&2; exit 11; }

# T-081 跨机接入的两个机器相关变量：不是密钥，不自动生成，必须由运维按本机
# 对外地址填写——留空时 test-server overlay 的 `:?` 会让 compose 直接失败。
for v in AI_PM_MCP_ALLOWED_HOSTS AI_PM_PUBLIC_BASE_URL; do
  cur="$(grep -E "^${v}=" .env.prod | head -1 | cut -d= -f2- || true)"
  if [[ -z "$cur" ]]; then
    echo "[setup] 注意：${v} 仍为空——跨机接入(/mcp)会 403 或指引指向回环地址"
    echo "[setup]      请按本机对外地址填写："
    echo "[setup]        AI_PM_MCP_ALLOWED_HOSTS=192.168.2.10   # 只写 host 表示不限端口"
    echo "[setup]        AI_PM_PUBLIC_BASE_URL=http://192.168.2.10:8090"
  else
    echo "[setup] ${v}: <已设置>"
  fi
done

# 校验加载后能否正确展开 DATABASE_URL（依赖 POSTGRES_* 在前）
set +u
# shellcheck disable=SC1091
source .env
# shellcheck disable=SC1091
source .env.prod
set -u
case "${DATABASE_URL:-}" in
  *"${POSTGRES_USER}"*"${POSTGRES_PASSWORD}"*"${POSTGRES_DB}"*)
    echo "[setup] DATABASE_URL 展开正确" ;;
  *)
    echo "[setup] FATAL: DATABASE_URL 展开异常: ${DATABASE_URL:-<空>}" >&2
    exit 12 ;;
esac
echo "[setup] 完成"
