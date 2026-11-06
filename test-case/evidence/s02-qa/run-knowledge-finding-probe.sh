#!/usr/bin/env bash
# S02 QA knowledge/finding probe V1.0: Batch 06 (proposals/redline/index/veto) + Batch 07 (finding lifecycle)
set -u
BASE=http://127.0.0.1:18090
EV="$(cd "$(dirname "$0")" && pwd)"
LOG="$EV/knowledge-finding-probe.log"
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
PIDB=$(PG "SELECT id FROM core.project WHERE code LIKE 'S02QA-B-%' ORDER BY created_at DESC LIMIT 1")
DKEY="qa-kb-$TS"

say "=== knowledge/finding probe start (project=$PID B=$PIDB) ==="

say "[B06-01] submit proposal: $(http POST /v1/pm/documents/proposals k01.json "{\"project_id\":\"$PID\",\"doc_key\":\"$DKEY\",\"doc_type\":\"knowledge\",\"markdown\":\"# QA knowledge\\n\\nSection one body.\",\"change_summary\":\"qa probe\",\"operation_id\":\"qa-kp-$TS\",\"created_by\":\"$DEV\"}")"
say "         body: $(head -c 260 "$EV/k01.json")"
DOC=$(PG "SELECT id FROM ai.project_document WHERE project_id='$PID' AND doc_key='$DKEY'")
PROPS=$(PG "SELECT count(*) FROM ai.document_proposal WHERE project_id='$PID'")
say "         document=$DOC proposals_in_db=$PROPS"

say "[B06-02] list proposals: $(http GET "/v1/pm/documents/proposals?project_id=$PID" k02.json)"
PROP_ID=$(PG "SELECT p.id FROM ai.document_proposal p JOIN ai.project_document d ON d.id=p.document_id WHERE p.project_id='$PID' AND d.doc_key='$DKEY' ORDER BY p.created_at DESC LIMIT 1")
say "         proposal_id=$PROP_ID"

say "[B06-03] confirm redline (AI proposal -> human confirm): $(http POST /v1/pm/documents/proposals/$PROP_ID/confirm k03.json "{\"project_id\":\"$PID\",\"confirmed_by\":\"$DEV\",\"operation_id\":\"qa-kc-$TS\"}")"
say "         body: $(head -c 200 "$EV/k03.json")"
REV=$(PG "SELECT id FROM ai.document_revision WHERE document_id='$DOC' ORDER BY revision_no DESC LIMIT 1")
say "         published_revision=$REV published_no=$(PG "SELECT revision_no FROM ai.document_revision WHERE id='$REV'") sha=$(PG "SELECT git_commit_sha FROM ai.document_revision WHERE id='$REV'")"

say "[B06-04] read document: $(http GET "/v1/pm/documents/$DOC?project_id=$PID" k04.json)"
say "         content head: $(head -c 120 "$EV/k04.json")"
say "[B06-07] index state: $(http GET "/v1/pm/documents/$DOC/index-state?project_id=$PID" k05.json) body=$(head -c 200 "$EV/k05.json")"
say "[B06-08] reindex revision: $(http POST "/v1/pm/documents/revisions/$REV/reindex" k06.json "{\"project_id\":\"$PID\"}") body=$(head -c 160 "$EV/k06.json")"
say "[B06-06] search: $(http GET "/v1/pm/documents/search?project_id=$PID&q=QA knowledge" k07.json) hits=$(grep -o '"doc_key"' "$EV/k07.json" | wc -l)"

say "[B06-09] second proposal (rev2): $(http POST /v1/pm/documents/proposals k08.json "{\"project_id\":\"$PID\",\"doc_key\":\"$DKEY\",\"doc_type\":\"knowledge\",\"markdown\":\"# QA knowledge\\n\\nSection one body EDITED.\",\"change_summary\":\"qa probe rev2\",\"operation_id\":\"qa-kp2-$TS\",\"created_by\":\"$DEV\"}")"
PROP2=$(PG "SELECT p.id FROM ai.document_proposal p JOIN ai.project_document d ON d.id=p.document_id WHERE p.project_id='$PID' AND d.doc_key='$DKEY' ORDER BY p.created_at DESC LIMIT 1")
say "         confirm rev2: $(http POST /v1/pm/documents/proposals/$PROP2/confirm k09.json "{\"project_id\":\"$PID\",\"confirmed_by\":\"$DEV\",\"operation_id\":\"qa-kc2-$TS\"}")"
REV2=$(PG "SELECT id FROM ai.document_revision WHERE document_id='$DOC' ORDER BY revision_no DESC LIMIT 1")
say "         rev2=$REV2 no=$(PG "SELECT revision_no FROM ai.document_revision WHERE id='$REV2'")"
say "[B06-10] veto rev2 (rollback): $(http POST "/v1/pm/documents/revisions/$REV2/veto" k10.json "{\"project_id\":\"$PID\",\"vetoed_by\":\"$DEV\",\"reason\":\"qa veto probe\",\"operation_id\":\"qa-veto-$TS\"}")"
say "         body: $(head -c 200 "$EV/k10.json")"
say "         after veto head=$(PG "SELECT revision_no FROM ai.document_revision WHERE document_id='$DOC' AND published=true LIMIT 1" 2>/dev/null || PG "SELECT revision_no FROM ai.document_revision WHERE document_id='$DOC' ORDER BY revision_no DESC LIMIT 1")"
say "[B05-08] re-confirm same proposal+operation (idempotency probe): $(http POST /v1/pm/documents/proposals/$PROP_ID/confirm k11.json "{\"project_id\":\"$PID\",\"confirmed_by\":\"$DEV\",\"operation_id\":\"qa-kc-$TS\"}")"
say "         revisions count: $(PG "SELECT count(*) FROM ai.document_revision WHERE document_id='$DOC'")"

say "=== B07 finding lifecycle ==="
say "[B07-01] create session: $(http POST /v1/pm/projects/$PID/discovery/sessions f01.json '{"source":"WEB","kind":"INITIAL_ONBOARDING"}')"
SID=$(grep -oE '"id":"[0-9a-f-]{36}"' "$EV/f01.json" | head -1 | cut -d'"' -f4)
say "         session=$SID body=$(head -c 160 "$EV/f01.json")"
say "[B07-02] add finding: $(http POST /v1/pm/projects/$PID/discovery/sessions/$SID/findings f02.json '{"kind":"RISK","content":"qa probe finding","confidence":0.9,"evidence":{},"repository_ids":[]}')"
FID=$(grep -oE '"id":"[0-9a-f-]{36}"' "$EV/f02.json" | head -1 | cut -d'"' -f4)
say "         finding=$FID state=$(PG "SELECT status FROM discovery.discovery_finding WHERE id='$FID'")"
say "[B07-03] verify/confirm finding: $(http POST /v1/pm/projects/$PID/discovery/sessions/$SID/findings/$FID/verification f03.json '{"status":"CONFIRMED","answer":"confirmed by qa"}')"
say "         state now: $(PG "SELECT status FROM discovery.discovery_finding WHERE id='$FID'") verification_rows=$(PG "SELECT count(*) FROM discovery.discovery_finding_verification WHERE finding_id='$FID'" 2>/dev/null || echo n/a)"
say "[B07-04] requirement_finding link table (expect absent): $(PG "SELECT count(*) FROM information_schema.tables WHERE table_name='requirement_finding'")"
say "[B07-05] cross-domain check: review_finding vs discovery_finding tables distinct: $(PG "SELECT count(*) FROM information_schema.tables WHERE table_name IN ('review_finding','discovery_finding')")"

say "[B04/DF1-x] non-member discovery access (expect 404): $(http GET "/v1/pm/projects/$PIDB/discovery/sessions/$SID" f04.json)"

say "=== Git authority for knowledge doc ==="
KR="D:/kudi-project/templates/.s02qa-knowledge"
git -C "$KR" log --oneline --all | grep -i "$DKEY" | head -5 | while IFS= read -r l; do say "commit: $l"; done
FP=$(find "$KR" -path "*$DKEY*" -name "*.md" -not -path "*/.git/*" | head -1)
say "file: $FP"
say "content: $(head -c 100 "$FP" 2>/dev/null)"
say "=== probe end ==="