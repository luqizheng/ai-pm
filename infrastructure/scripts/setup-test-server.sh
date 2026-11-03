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
