#!/usr/bin/env bash
# checkpoint-1 E2E 驱动脚本（可重放）：
#   1. 用户注册成功 → 创建第一个 team
#   2. 创建第一个项目（ai-pm）→ 关联本项目仓库 URL → verify → 自动生成 PC/KB
#
# 两阶段（首管理员 subject 鸡生蛋）：
#   phase=register  # 仅注册，打印 iam 首用户 id（拿去配 AI_PM_FIRST_ADMIN_SUBJECT）
#   phase=run       # 登录 + 建团 + 建项目 + 仓库链 + PC/KB 断言（要求 pm-api 已带 FIRST_ADMIN 对重建）
#
# 前置（checkpoint 栈，见 infrastructure/docker-compose.ckpt-override.yml 头注）：
#   DATABASE_URL=...lmms_ckpt + ckpt overlay 起栈；
#   phase=run 前：AI_PM_FIRST_ADMIN_SUBJECT=<register 阶段的 id> 重建 pm-api。
# 人工步骤（仅一次）：phase=run 打印的 deploy key 公钥需挂到 GitLab
#   （192.168.2.16 → tooling/ai-pm → Settings → Repository → Deploy keys，只读即可）。
set -euo pipefail

GW="${GW:-http://127.0.0.1:3000}"
PM="${PM:-http://127.0.0.1:8090}"
PHASE="${1:-run}"
REPO_URL="${REPO_URL:-git@192.168.2.16:tooling/ai-pm.git}"
PROJECT_CODE="${PROJECT_CODE:-ai-pm}"
TEAM_NAME="${TEAM_NAME:-checkpoint-team}"
USERNAME="${USERNAME:-checkpoint-leo}"
PASSWORD="${PASSWORD:-Checkpoint@123456}"
NICKNAME="${NICKNAME:-Checkpoint Leo}"
EVIDENCE_DIR="${EVIDENCE_DIR:-/tmp/ckpt1-evidence}"
mkdir -p "$EVIDENCE_DIR"

step() { printf '\n=== %s ===\n' "$*"; }

jq_get() { python -c "import sys,json;print(json.load(sys.stdin)$1)" 2>/dev/null || cat; }

# ── 公共：登录拿 token ──────────────────────────────────────────────────────
login() {
  curl -sf -X POST "$GW/v1/iam/auth/login" \
    -H 'content-type: application/json' \
    -d "{\"username\":\"$USERNAME\",\"password\":\"$PASSWORD\"}" \
    | tee "$EVIDENCE_DIR/login.json" | python -c "import sys,json;print(json.load(sys.stdin)['accessToken'])"
}

if [ "$PHASE" = "register" ]; then
  step "① setup-status（全新系统应 setup_required=true）"
  curl -sf "$GW/v1/iam/auth/setup-status" | tee "$EVIDENCE_DIR/setup-status.json"
  echo
  step "② 注册首用户（$USERNAME）"
  curl -sf -X POST "$GW/v1/iam/auth/register" \
    -H 'content-type: application/json' \
    -d "{\"username\":\"$USERNAME\",\"password\":\"$PASSWORD\",\"nickname\":\"$NICKNAME\"}" \
    | tee "$EVIDENCE_DIR/register.json"
  echo
  step "③ 把上述 id 配入 AI_PM_FIRST_ADMIN_SUBJECT 后重建 pm-api，再跑：$0 run"
  exit 0
fi

# ── phase=run ───────────────────────────────────────────────────────────────
step "① 登录（$USERNAME）"
TOKEN="$(login)"
AUTH="Authorization: Bearer $TOKEN"

step "② GET /v1/pm/me（应 onboarding_required=true）"
curl -sf "$PM/v1/pm/me" -H "$AUTH" | tee "$EVIDENCE_DIR/me-before.json"

step "③ POST /v1/pm/teams 建第一个团（$TEAM_NAME）"
curl -sf -X POST "$PM/v1/pm/teams" -H "$AUTH" -H 'content-type: application/json' \
  -d "{\"name\":\"$TEAM_NAME\",\"description\":\"checkpoint-1 first team\"}" \
  | tee "$EVIDENCE_DIR/team.json"
TENANT_ID="$(python -c "import json;d=json.load(open('$EVIDENCE_DIR/team.json'));print(d['id'] or d['tenantId'] or d['tenant_id'])")"
echo "tenant_id=$TENANT_ID"

step "④ GET /v1/pm/me（应 is_active=true、team 就位）"
curl -sf "$PM/v1/pm/me" -H "$AUTH" | tee "$EVIDENCE_DIR/me-after-team.json"

step "⑤ POST /v1/pm/projects 建第一个项目（$PROJECT_CODE）"
PROJECT_ID="$(curl -sf -X POST "$PM/v1/pm/projects" -H "$AUTH" -H 'content-type: application/json' \
  -d "{\"code\":\"$PROJECT_CODE\",\"name\":\"AI-PM\",\"description\":\"checkpoint-1 first project\"}" \
  | tee "$EVIDENCE_DIR/project.json" | python -c "import sys,json;print(json.load(sys.stdin)['id'])")"
echo "project_id=$PROJECT_ID"

step "⑥ POST /v1/pm/projects/{id}/ssh-keys 生成 deploy key（公钥需人工挂 GitLab）"
curl -sf -X POST "$PM/v1/pm/projects/$PROJECT_ID/ssh-keys" -H "$AUTH" -H 'content-type: application/json' \
  -d '{"name":"checkpoint-deploy-key"}' | tee "$EVIDENCE_DIR/ssh-key.json"
PUBKEY="$(python -c "import json;print(json.load(open('$EVIDENCE_DIR/ssh-key.json'))['publicKey'])" 2>/dev/null \
  || python -c "import json;print(json.load(open('$EVIDENCE_DIR/ssh-key.json'))['public_key'])")"
echo
echo ">>>>>>> 请把下面的 deploy key 公钥挂到 GitLab（tooling/ai-pm → Settings → Repository → Deploy keys，只读）："
echo "$PUBKEY"
echo "<<<<<<< 挂好后按回车继续…"
read -r

# ── 可选：LLM Provider 接入（CHECKPOINT1_E2E_PROVIDER=1 启用，P2-13×PC-I6）──
# 注册租户级 enabled Provider 后，三级解析（项目未指定 → 租户首个 enabled）
# 即命中，verify→intake 的 KB/PC 分析走 LLM 增强路径（0062 analysis_mode=llm，
# 可在 K2/P3 产物与 kb.document_revision/pc.cognition_revision 断言）。
# 不设本开关则维持结构化派生（analysis_mode=derived），链路行为与 checkpoint-1
# 验证记录一致。
if [ "${CHECKPOINT1_E2E_PROVIDER:-0}" = "1" ]; then
  step "⑥b 注册并启用 LLM Provider（LLM 接管分析开关）"
  : "${PROVIDER_ENDPOINT:?CHECKPOINT1_E2E_PROVIDER=1 需要 PROVIDER_ENDPOINT（OpenAI 兼容 base url）}"
  : "${PROVIDER_MODEL:?CHECKPOINT1_E2E_PROVIDER=1 需要 PROVIDER_MODEL}"
  : "${PROVIDER_API_KEY:?CHECKPOINT1_E2E_PROVIDER=1 需要 PROVIDER_API_KEY}"
  PROVIDER_NAME="${PROVIDER_NAME:-e2e-provider}"
  PROVIDER_ID="$(curl -sf -X POST "$PM/v1/pm/tenants/$TENANT_ID/llm-providers" \
    -H "$AUTH" -H 'content-type: application/json' \
    -d "{\"name\":\"$PROVIDER_NAME\",\"provider_type\":\"openai_compatible\",\"endpoint\":\"$PROVIDER_ENDPOINT\",\"configuration\":{\"model\":\"$PROVIDER_MODEL\"},\"api_key\":\"$PROVIDER_API_KEY\",\"timeout_secs\":120}" \
    | tee "$EVIDENCE_DIR/provider.json" \
    | python -c "import sys,json;d=json.load(sys.stdin);print(d['id'] or d['providerId'])")"
  echo "provider_id=$PROVIDER_ID"
  curl -sf -X POST "$PM/v1/pm/tenants/$TENANT_ID/llm-providers/$PROVIDER_ID/status" \
    -H "$AUTH" -H 'content-type: application/json' \
    -d '{"action":"enable"}' | tee "$EVIDENCE_DIR/provider-enable.json"
  echo
  echo "（已启用；无 provider/调用失败时分析自动回退结构化派生并标 analysis_mode）"
fi

step "⑦ POST /v1/pm/projects/{id}/repositories 关联仓库（$REPO_URL）"
REPO_ID="$(curl -sf -X POST "$PM/v1/pm/projects/$PROJECT_ID/repositories" -H "$AUTH" -H 'content-type: application/json' \
  -d "{\"name\":\"ai-pm\",\"url\":\"$REPO_URL\"}" \
  | tee "$EVIDENCE_DIR/repository.json" | python -c "import sys,json;print(json.load(sys.stdin)['id'])")"
echo "repository_id=$REPO_ID"

step "⑧ POST verify（真 ls-remote；应 verified + head_commit_sha，并自动触发快照 intake）"
VERIFY_JSON="$(curl -sf -X POST "$PM/v1/pm/repositories/$REPO_ID/verify" -H "$AUTH" | tee "$EVIDENCE_DIR/verify.json")"
echo "$VERIFY_JSON"
HEAD_SHA="$(echo "$VERIFY_JSON" | python -c "import sys,json;print(json.load(sys.stdin)['headCommitSha'] or json.load(sys.stdin)['head_commit_sha'])" 2>/dev/null || true)"
echo "head=$HEAD_SHA"

step "⑨ 轮询 intake（kb_analyze→pc_analyze→COMPLETED；worker 消费需要时间）"
INTAKE_ID="$(psql_ok=1; docker exec infrastructure-aipm-ckpt-pg-1 psql -U aipm -d pm_ckpt -tAc \
  "SELECT id FROM integration.intake_record WHERE idempotency_key='repo_verify:$REPO_ID:$HEAD_SHA' LIMIT 1")"
echo "intake_id=$INTAKE_ID"
for i in $(seq 1 30); do
  STATE="$(curl -sf "$PM/v1/pm/intakes/$INTAKE_ID" -H "$AUTH" | tee "$EVIDENCE_DIR/intake-state.json" | python -c "import sys,json;print(json.load(sys.stdin)['derivedStatus'] or json.load(sys.stdin)['derived_status'])" 2>/dev/null || echo pending)"
  echo "  [$i] derived=$STATE"
  [ "$STATE" = "COMPLETED" ] || [ "$STATE" = "Completed" ] && break
  sleep 2
done
curl -sf "$PM/v1/pm/intakes/$INTAKE_ID" -H "$AUTH" | tee "$EVIDENCE_DIR/intake-final.json"

step "⑩ KB/PC 读取面（K2 + P3；真实产物断言见 checkpoint 验证记录）"
KB_TASK="$(python -c "import json;d=json.load(open('$EVIDENCE_DIR/intake-final.json'));print([t['taskId'] or t['task_id'] for t in d['tasks'] if t['taskType']=='kb_analyze'][0])" 2>/dev/null \
  || python -c "import json;d=json.load(open('$EVIDENCE_DIR/intake-final.json'));print([t['task_id'] for t in d['tasks'] if t['task_type']=='kb_analyze'][0])")"
PC_TASK="$(python -c "import json;d=json.load(open('$EVIDENCE_DIR/intake-final.json'));print([t['task_id'] for t in d['tasks'] if t['task_type']=='pc_analyze'][0])")"
curl -sf "$PM/v1/kb/analyses/$KB_TASK?project_id=$PROJECT_ID" -H "$AUTH" | tee "$EVIDENCE_DIR/k2.json" | head -c 2000; echo
curl -sf "$PM/v1/pc/analyses/$PC_TASK?project_id=$PROJECT_ID" -H "$AUTH" | tee "$EVIDENCE_DIR/p3.json" | python -m json.tool | head -60

step "✅ 证据已落 $EVIDENCE_DIR（checkpoint 验证记录文档引用这些文件）"
