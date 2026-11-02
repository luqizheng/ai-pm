#!/usr/bin/env bash
# PM-AI 环境自检脚本（bash / macOS、Linux）——T-089 占位实现
#
# 用法：./verify-install.sh <base_url> [api_key] [device_id]
# 退出码：0 = 全部通过；1 = 有失败项。Key 只进内存与本次请求头，不落盘。
set -u

BASE_URL="${1:-}"
API_KEY="${2:-}"
DEVICE_ID="${3:-}"
ACCEPT='application/json, text/event-stream'
FAILS=0
TOTAL=0

check() {
  local name="$1" expected="$2" url="$3"; shift 3
  TOTAL=$((TOTAL + 1))
  local code
  code=$(curl -sS -o /dev/null -w '%{http_code}' --max-time 10 "$@" "$url" 2>/dev/null) || code='curl-error'
  if [ "$code" = "$expected" ]; then
    echo "PASS  $name  $code"
  else
    echo "FAIL  $name  got=$code want=$expected"
    FAILS=$((FAILS + 1))
  fi
}

BASE_URL="${BASE_URL%/}"

# 1) /health（公开）
check 'health' 200 "$BASE_URL/health"

# 2) /install/{tool}（公开，agent 读 JSON）
check 'install-guide' 200 "$BASE_URL/install/generic?format=json"

# 3) 认证：/v1/pm/me
if [ -n "$API_KEY" ]; then
  check 'authenticated' 200 "$BASE_URL/v1/pm/me" -H "Authorization: Bearer $API_KEY" -H "Accept: $ACCEPT"
else
  echo 'SKIP  authenticated  (no api_key given)'
  TOTAL=$((TOTAL + 1))
fi

# 4) 设备可见
if [ -n "$API_KEY" ] && [ -n "$DEVICE_ID" ]; then
  check 'device-visible' 200 "$BASE_URL/v1/pm/devices/$DEVICE_ID" -H "Authorization: Bearer $API_KEY" -H "Accept: $ACCEPT"
else
  echo 'SKIP  device-visible  (need api_key and device_id)'
  TOTAL=$((TOTAL + 1))
fi

echo "verify-install: $((TOTAL - FAILS)) passed, $FAILS failed"
[ "$FAILS" -eq 0 ]
