#!/usr/bin/env bash
# S02 QA knowledge probe V1.3: source_ref + projects/ in markdown (both gates).
set -u
BASE=http://127.0.0.1:18090
EV="$(cd "$(dirname "$0")" && pwd)"
LOG="$EV/knowledge-probe-v13.log"
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
DKEY="qa-kb4-$TS"

say "=== knowledge probe V1.3 start ==="
say "[B06-01] proposal (source_ref + projects/ in markdown): $(http POST /v1/pm/documents/proposals k41.json "{\"project_id\":\"$PID\",\"doc_key\":\"$DKEY\",\"doc_type\":\"knowledge\",\"markdown\":\"# QA knowledge\\n\\nBody one.\\n\\nsource: projects/S02QA-A/requirement/req-q1\\n\",\"change_summary\":\"qa probe v13\",\"source_ref\":\"templates/requirement.md\",\"operation_id\":\"qa-kp5-$TS\",\"created_by\":\"$DEV\"}")"
say "         body: $(head -c 320 "$EV/k41.json")"
DOC=$(PG "SELECT id FROM ai.project_document WHERE project_id='$PID' AND doc_key='$DKEY'")
PROP_ID=$(PG "SELECT id FROM ai.document_proposal WHERE document_id='$DOC' ORDER BY created_at DESC LIMIT 1")
say "         document=$DOC proposal=$PROP_ID"
say "[B06-03] confirm redline: $(http POST /v1/pm/documents/proposals/$PROP_ID/confirm k42.json "{\"project_id\":\"$PID\",\"confirmed_by\":\"$DEV\",\"operation_id\":\"qa-kc5-$TS\"}")"
say "         body: $(head -c 240 "$EV/k42.json")"
REV=$(PG "SELECT id FROM ai.document_revision WHERE document_id='$DOC' ORDER BY revision_no DESC LIMIT 1")
say "         revision=$REV no=$(PG "SELECT revision_no FROM ai.document_revision WHERE id='$REV'") sha=$(PG "SELECT git_commit_sha FROM ai.document_revision WHERE id='$REV'")"
if [ -n "$REV" ]; then
  say "[B06-04] read: $(http GET "/v1/pm/documents/$DOC?project_id=$PID" k43.json)"
  say "[B06-07] index-state: $(http GET "/v1/pm/documents/$DOC/index-state?project_id=$PID" k44.json) $(head -c 220 "$EV/k44.json")"
  say "[B06-08] reindex: $(http POST "/v1/pm/documents/revisions/$REV/reindex" k45.json "{\"project_id\":\"$PID\"}") $(head -c 160 "$EV/k45.json")"
  say "[B06-06] search: $(http GET "/v1/pm/documents/search?project_id=$PID&q=knowledge%20Body%20one" k46.json) hits=$(grep -o "\"$DKEY\"" "$EV/k46.json" | wc -l)"
  say "[B06-09] rev2: $(http POST /v1/pm/documents/proposals k47.json "{\"project_id\":\"$PID\",\"doc_key\":\"$DKEY\",\"doc_type\":\"knowledge\",\"markdown\":\"# QA knowledge\\n\\nBody one EDITED.\\n\\nsource: projects/S02QA-A/requirement/req-q1\\n\",\"change_summary\":\"qa rev2\",\"source_ref\":\"templates/requirement.md\",\"operation_id\":\"qa-kp6-$TS\",\"created_by\":\"$DEV\"}")"
  PROP2=$(PG "SELECT id FROM ai.document_proposal WHERE document_id='$DOC' ORDER BY created_at DESC LIMIT 1")
  say "         confirm rev2: $(http POST /v1/pm/documents/proposals/$PROP2/confirm k48.json "{\"project_id\":\"$PID\",\"confirmed_by\":\"$DEV\",\"operation_id\":\"qa-kc6-$TS\"}")"
  REV2=$(PG "SELECT id FROM ai.document_revision WHERE document_id='$DOC' ORDER BY revision_no DESC LIMIT 1")
  say "         states: $(PG "SELECT revision_no||':'||state FROM ai.document_revision WHERE document_id='$DOC' ORDER BY revision_no" | tr '\n' ' ')"
  say "[B06-10] veto rev2: $(http POST "/v1/pm/documents/revisions/$REV2/veto" k49.json "{\"project_id\":\"$PID\",\"vetoed_by\":\"$DEV\",\"reason\":\"qa veto\",\"operation_id\":\"qa-veto3-$TS\"}")"
  say "         $(head -c 200 "$EV/k49.json")"
  say "         states after veto: $(PG "SELECT revision_no||':'||state FROM ai.document_revision WHERE document_id='$DOC' ORDER BY revision_no" | tr '\n' ' ')"
  say "[B05-08] idempotency re-confirm settled: $(http POST /v1/pm/documents/proposals/$PROP_ID/confirm k50.json "{\"project_id\":\"$PID\",\"confirmed_by\":\"$DEV\",\"operation_id\":\"qa-kc5-$TS\"}") $(head -c 160 "$EV/k50.json")"
  say "         revision count: $(PG "SELECT count(*) FROM ai.document_revision WHERE document_id='$DOC'")"
  KR="D:/kudi-project/templates/.s02qa-knowledge"
  say "git log: $(git -C "$KR" log --oneline --all | grep -c "$DKEY") commits for $DKEY"
  FP=$(find "$KR" -path "*$DKEY*" -name "*.md" -not -path "*/.git/*" | head -1)
  say "file: $FP content_head=$(head -c 80 "$FP" 2>/dev/null)"
fi
say "=== probe V1.3 end ==="