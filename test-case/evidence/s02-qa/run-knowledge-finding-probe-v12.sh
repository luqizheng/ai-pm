#!/usr/bin/env bash
# S02 QA knowledge/finding probe V1.2: markdown embeds projects/ reference; evidence as array.
set -u
BASE=http://127.0.0.1:18090
EV="$(cd "$(dirname "$0")" && pwd)"
LOG="$EV/knowledge-finding-probe-v12.log"
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
DKEY="qa-kb3-$TS"
SID=$(grep -oE '"id":"[0-9a-f-]{36}"' "$EV/f01.json" | head -1 | cut -d'"' -f4)

say "=== knowledge/finding probe V1.2 start ==="
say "[B06-01] proposal (markdown embeds projects/ source line): $(http POST /v1/pm/documents/proposals k31.json "{\"project_id\":\"$PID\",\"doc_key\":\"$DKEY\",\"doc_type\":\"knowledge\",\"markdown\":\"# QA knowledge\\n\\nBody one.\\n\\nsource: projects/S02QA-A/requirement/req-q1\\n\",\"change_summary\":\"qa probe v12\",\"operation_id\":\"qa-kp3-$TS\",\"created_by\":\"$DEV\"}")"
say "         body: $(head -c 300 "$EV/k31.json")"
DOC=$(PG "SELECT id FROM ai.project_document WHERE project_id='$PID' AND doc_key='$DKEY'")
PROP_ID=$(PG "SELECT id FROM ai.document_proposal WHERE document_id='$DOC' ORDER BY created_at DESC LIMIT 1")
say "         document=$DOC proposal=$PROP_ID proposal_outcome=$(PG "SELECT outcome FROM ai.document_proposal WHERE id='$PROP_ID'")"
say "[B06-03] confirm redline: $(http POST /v1/pm/documents/proposals/$PROP_ID/confirm k32.json "{\"project_id\":\"$PID\",\"confirmed_by\":\"$DEV\",\"operation_id\":\"qa-kc3-$TS\"}")"
say "         body: $(head -c 220 "$EV/k32.json")"
REV=$(PG "SELECT id FROM ai.document_revision WHERE document_id='$DOC' ORDER BY revision_no DESC LIMIT 1")
say "         revision=$REV no=$(PG "SELECT revision_no FROM ai.document_revision WHERE id='$REV'") sha=$(PG "SELECT git_commit_sha FROM ai.document_revision WHERE id='$REV'")"

if [ -n "$REV" ]; then
  say "[B06-04] read: $(http GET "/v1/pm/documents/$DOC?project_id=$PID" k33.json) head=$(head -c 100 "$EV/k33.json")"
  say "[B06-07] index-state: $(http GET "/v1/pm/documents/$DOC/index-state?project_id=$PID" k34.json) $(head -c 200 "$EV/k34.json")"
  say "[B06-08] reindex: $(http POST "/v1/pm/documents/revisions/$REV/reindex" k35.json "{\"project_id\":\"$PID\"}") $(head -c 140 "$EV/k35.json")"
  say "[B06-06] search: $(http GET "/v1/pm/documents/search?project_id=$PID&q=knowledge%20Body%20one" k36.json) hits=$(grep -o "\"$DKEY\"" "$EV/k36.json" | wc -l)"
  say "[B06-09] rev2: $(http POST /v1/pm/documents/proposals k37.json "{\"project_id\":\"$PID\",\"doc_key\":\"$DKEY\",\"doc_type\":\"knowledge\",\"markdown\":\"# QA knowledge\\n\\nBody one EDITED.\\n\\nsource: projects/S02QA-A/requirement/req-q1\\n\",\"change_summary\":\"qa rev2\",\"operation_id\":\"qa-kp4-$TS\",\"created_by\":\"$DEV\"}")"
  PROP2=$(PG "SELECT id FROM ai.document_proposal WHERE document_id='$DOC' ORDER BY created_at DESC LIMIT 1")
  say "         confirm rev2: $(http POST /v1/pm/documents/proposals/$PROP2/confirm k38.json "{\"project_id\":\"$PID\",\"confirmed_by\":\"$DEV\",\"operation_id\":\"qa-kc4-$TS\"}")"
  REV2=$(PG "SELECT id FROM ai.document_revision WHERE document_id='$DOC' ORDER BY revision_no DESC LIMIT 1")
  say "         states: $(PG "SELECT revision_no||':'||state FROM ai.document_revision WHERE document_id='$DOC' ORDER BY revision_no" | tr '\n' ' ')"
  say "[B06-10] veto rev2: $(http POST "/v1/pm/documents/revisions/$REV2/veto" k39.json "{\"project_id\":\"$PID\",\"vetoed_by\":\"$DEV\",\"reason\":\"qa veto\",\"operation_id\":\"qa-veto2-$TS\"}")"
  say "         $(head -c 180 "$EV/k39.json")"
  say "         states after veto: $(PG "SELECT revision_no||':'||state FROM ai.document_revision WHERE document_id='$DOC' ORDER BY revision_no" | tr '\n' ' ')"
  say "[B05-08] idempotency: settled proposal re-confirm: $(http POST /v1/pm/documents/proposals/$PROP_ID/confirm k40.json "{\"project_id\":\"$PID\",\"confirmed_by\":\"$DEV\",\"operation_id\":\"qa-kc3-$TS\"}") $(head -c 140 "$EV/k40.json")"
  say "         revision count: $(PG "SELECT count(*) FROM ai.document_revision WHERE document_id='$DOC'")"
fi

say "=== B07 finding lifecycle ==="
say "[B07-02] add finding evidence=[]: $(http POST /v1/pm/projects/$PID/discovery/sessions/$SID/findings f31.json '{"kind":"observed","content":"qa probe finding v12","confidence":0.9,"evidence":[],"repository_ids":[]}')"
FID=$(grep -oE '"id":"[0-9a-f-]{36}"' "$EV/f31.json" | head -1 | cut -d'"' -f4)
say "         finding=$FID status=$(PG "SELECT verification_status FROM discovery.discovery_finding WHERE id='$FID'")"
say "[B07-03] confirm: $(http POST /v1/pm/projects/$PID/discovery/sessions/$SID/findings/$FID/verification f32.json '{"status":"CONFIRMED","answer":"confirmed by qa"}') -> $(PG "SELECT verification_status FROM discovery.discovery_finding WHERE id='$FID'")"
say "[B07-03b] speculation + reject: $(http POST /v1/pm/projects/$PID/discovery/sessions/$SID/findings f33.json '{"kind":"speculation","content":"qa speculation","evidence":[],"repository_ids":[]}')"
FID2=$(grep -oE '"id":"[0-9a-f-]{36}"' "$EV/f33.json" | head -1 | cut -d'"' -f4)
say "         reject: $(http POST /v1/pm/projects/$PID/discovery/sessions/$SID/findings/$FID2/verification f34.json '{"status":"REJECTED","answer":"no"}') -> $(PG "SELECT verification_status FROM discovery.discovery_finding WHERE id='$FID2'")"
say "[B07-03c] illegal status: $(http POST /v1/pm/projects/$PID/discovery/sessions/$SID/findings/$FID/verification f35.json '{"status":"BOGUS","answer":"x"}') $(head -c 120 "$EV/f35.json")"
say "[B07-05] audit: $(PG "SELECT action||'x'||count(*) FROM governance.audit_log WHERE target_id='$FID' GROUP BY action" | tr '\n' ' ')"
say "=== probe V1.2 end ==="