#!/usr/bin/env bash
# S02 QA knowledge/finding probe V1.1: redline-compliant proposal + finding lifecycle
set -u
BASE=http://127.0.0.1:18090
EV="$(cd "$(dirname "$0")" && pwd)"
LOG="$EV/knowledge-finding-probe-v11.log"
: > "$LOG"
say() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$LOG"; }
http() {
  local m=$1 p=$2 f=$3 b=${4:-}
  if [ -n "$b" ]; then
    curl -s -m 20 -o "$EV/$f" -w "%{http_code}" -X "$m" -H 'content-type: application/json' -d "$b" "$BASE$p"
  else
    curl -s -m 20 -o "$EV/$f" -w "%{http_code}" -X "$m" "$BASE$p"
  fi
}
PG() { docker exec infrastructure-aipm-pg-1 psql -U aipm -d aipm -tA -c "$1"; }
DEV=00000000-0000-0000-0000-0000000000ff
TS=$(date -u +%H%M%S)
PID=95466cf3-8abf-4e4c-bc15-c8f92abbe765
DKEY="qa-kb2-$TS"

say "=== knowledge/finding probe V1.1 start ==="
say "[B06-01] redline-compliant proposal (source_ref=doc path): $(http POST /v1/pm/documents/proposals k21.json "{\"project_id\":\"$PID\",\"doc_key\":\"$DKEY\",\"doc_type\":\"knowledge\",\"markdown\":\"# QA knowledge\\n\\nSection one body.\",\"change_summary\":\"qa probe\",\"source_ref\":\"templates/requirement.md\",\"operation_id\":\"qa-kp-$TS\",\"created_by\":\"$DEV\"}")"
say "         body: $(head -c 300 "$EV/k21.json")"
DOC=$(PG "SELECT id FROM ai.project_document WHERE project_id='$PID' AND doc_key='$DKEY'")
say "         document=$DOC db_state=$(PG "SELECT outcome FROM ai.document_proposal WHERE document_id='$DOC' ORDER BY created_at DESC LIMIT 1" 2>/dev/null || echo n/a)"

PROP_ID=$(PG "SELECT id FROM ai.document_proposal WHERE document_id='$DOC' ORDER BY created_at DESC LIMIT 1" 2>/dev/null)
say "[B06-03] confirm redline: $(http POST /v1/pm/documents/proposals/$PROP_ID/confirm k22.json "{\"project_id\":\"$PID\",\"confirmed_by\":\"$DEV\",\"operation_id\":\"qa-kc-$TS\"}")"
say "         body: $(head -c 240 "$EV/k22.json")"
REV=$(PG "SELECT id FROM ai.document_revision WHERE document_id='$DOC' ORDER BY revision_no DESC LIMIT 1" 2>/dev/null)
say "         revision=$REV sha=$(PG "SELECT git_commit_sha FROM ai.document_revision WHERE id='$REV'" 2>/dev/null)"

if [ -n "$REV" ]; then
  say "[B06-04] read document: $(http GET "/v1/pm/documents/$DOC?project_id=$PID" k23.json)"
  say "         body head: $(head -c 140 "$EV/k23.json")"
  say "[B06-07] index state: $(http GET "/v1/pm/documents/$DOC/index-state?project_id=$PID" k24.json) body=$(head -c 220 "$EV/k24.json")"
  say "[B06-08] reindex: $(http POST "/v1/pm/documents/revisions/$REV/reindex" k25.json "{\"project_id\":\"$PID\"}") body=$(head -c 160 "$EV/k25.json")"
  say "[B06-06] search (urlencoded): $(http GET "/v1/pm/documents/search?project_id=$PID&q=QA%20knowledge" k26.json) doc_key_hits=$(grep -o "\"$DKEY\"" "$EV/k26.json" | wc -l)"
  say "[B06-09] rev2 proposal: $(http POST /v1/pm/documents/proposals k27.json "{\"project_id\":\"$PID\",\"doc_key\":\"$DKEY\",\"doc_type\":\"knowledge\",\"markdown\":\"# QA knowledge\\n\\nSection one body EDITED.\",\"change_summary\":\"qa rev2\",\"source_ref\":\"templates/requirement.md\",\"operation_id\":\"qa-kp2-$TS\",\"created_by\":\"$DEV\"}")"
  PROP2=$(PG "SELECT id FROM ai.document_proposal WHERE document_id='$DOC' ORDER BY created_at DESC LIMIT 1")
  say "         confirm rev2: $(http POST /v1/pm/documents/proposals/$PROP2/confirm k28.json "{\"project_id\":\"$PID\",\"confirmed_by\":\"$DEV\",\"operation_id\":\"qa-kc2-$TS\"}")"
  REV2=$(PG "SELECT id FROM ai.document_revision WHERE document_id='$DOC' ORDER BY revision_no DESC LIMIT 1")
  say "         revisions: $(PG "SELECT revision_no||':'||state FROM ai.document_revision WHERE document_id='$DOC' ORDER BY revision_no" 2>/dev/null | tr '\n' ' ')"
  say "[B06-10] veto rev2: $(http POST "/v1/pm/documents/revisions/$REV2/veto" k29.json "{\"project_id\":\"$PID\",\"vetoed_by\":\"$DEV\",\"reason\":\"qa veto probe\",\"operation_id\":\"qa-veto-$TS\"}")"
  say "         body: $(head -c 200 "$EV/k29.json")"
  say "         states after veto: $(PG "SELECT revision_no||':'||state FROM ai.document_revision WHERE document_id='$DOC' ORDER BY revision_no" 2>/dev/null | tr '\n' ' ')"
  say "[B05-08] idempotent re-confirm (same operation_id qa-kc-$TS on settled proposal): $(http POST /v1/pm/documents/proposals/$PROP_ID/confirm k30.json "{\"project_id\":\"$PID\",\"confirmed_by\":\"$DEV\",\"operation_id\":\"qa-kc-$TS\"}")"
  say "         body: $(head -c 160 "$EV/k30.json")"
  say "         total revisions (expect unchanged): $(PG "SELECT count(*) FROM ai.document_revision WHERE document_id='$DOC'" 2>/dev/null)"
else
  say "         proposal still rejected - inspect k21.json"
fi

say "=== B07 finding lifecycle ==="
SID=$(grep -oE '"id":"[0-9a-f-]{36}"' "$EV/f01.json" 2>/dev/null | head -1 | cut -d'"' -f4)
say "[B07-01] session (from v1.0 probe): $SID"
say "[B07-02] add finding kind=observed: $(http POST /v1/pm/projects/$PID/discovery/sessions/$SID/findings f22.json '{"kind":"observed","content":"qa probe finding v11","confidence":0.9,"evidence":{},"repository_ids":[]}')"
FID=$(grep -oE '"id":"[0-9a-f-]{36}"' "$EV/f22.json" | head -1 | cut -d'"' -f4)
say "         finding=$FID verification_status=$(PG "SELECT verification_status FROM discovery.discovery_finding WHERE id='$FID'" 2>/dev/null) actor_role=$(PG "SELECT actor_role FROM discovery.discovery_finding WHERE id='$FID'" 2>/dev/null)"
say "[B07-03] confirm finding: $(http POST /v1/pm/projects/$PID/discovery/sessions/$SID/findings/$FID/verification f23.json '{"status":"CONFIRMED","answer":"confirmed by qa"}')"
say "         status now: $(PG "SELECT verification_status FROM discovery.discovery_finding WHERE id='$FID'" 2>/dev/null)"
say "[B07-03b] reject path (new finding): $(http POST /v1/pm/projects/$PID/discovery/sessions/$SID/findings f24.json '{"kind":"speculation","content":"qa speculation probe","evidence":{},"repository_ids":[]}')"
FID2=$(grep -oE '"id":"[0-9a-f-]{36}"' "$EV/f24.json" | head -1 | cut -d'"' -f4)
say "         reject: $(http POST /v1/pm/projects/$PID/discovery/sessions/$SID/findings/$FID2/verification f25.json '{"status":"REJECTED","answer":"rejected by qa"}') -> $(PG "SELECT verification_status FROM discovery.discovery_finding WHERE id='$FID2'" 2>/dev/null)"
say "[B07-03c] illegal verification status probe (expect 4xx): $(http POST /v1/pm/projects/$PID/discovery/sessions/$SID/findings/$FID/verification f26.json '{"status":"BOGUS","answer":"x"}')"
say "[B07-05] audit: $(PG "SELECT action||'x'||count(*) FROM governance.audit_log WHERE target_id='$FID' GROUP BY action" 2>/dev/null | tr '\n' ' ')"
say "=== probe V1.1 end ==="