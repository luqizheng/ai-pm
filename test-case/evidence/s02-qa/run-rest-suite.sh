#!/usr/bin/env bash
# S02 QA REST acceptance suite V1.0 (evidence script)
# Target: local pm-api (auth=off dev-off, port 18090), independent knowledge root.
set -u
BASE=http://127.0.0.1:18090
EV="$(cd "$(dirname "$0")" && pwd)"
LOG="$EV/rest-execution.log"
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
jid()   { grep -oE '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}' "$EV/$1" | head -1; }
jrow()  { grep -oE '"row_revision":[0-9]+' "$EV/$1" | head -1 | grep -oE '[0-9]+'; }
jhash() { grep -oE '"content_hash":"[0-9a-f]{64}"' "$EV/$1" | head -1 | cut -d'"' -f4; }
jref()  { grep -oE '"markdown_version_ref":"[^"]*"' "$EV/$1" | head -1; }
jstate(){ grep -oE '"state":"[a-z_]+"' "$EV/$1" | head -1; }
PG() { docker exec infrastructure-aipm-pg-1 psql -U aipm -d aipm -tA -c "$1"; }
TS=$(date -u +%H%M%S)
CODE_A="S02QA-A-$TS"
CODE_B="S02QA-B-$TS"

say "=== S02 QA REST suite start (project A=$CODE_A B=$CODE_B) ==="

say "[B01-00] health: $(http GET /health h00.json)"
say "[B01-01] create project A: $(http POST /v1/pm/projects h01.json "{\"code\":\"$CODE_A\",\"name\":\"S02 QA Project A\"}") -> project=$(jid h01.json)"
PID=$(jid h01.json)

say "[B01-01] create requirement REQ-QA-1: $(http POST /v1/pm/projects/$PID/requirements h02.json '{"code":"REQ-QA-1","title":"S02 QA requirement one","requirement_type":"functional","priority":"high","markdown":"# REQ-QA-1\n\nBody v1"}')"
RID=$(jid h02.json)
say "         requirement_id=$RID"
say "[B01-02] GET detail: $(http GET /v1/pm/requirements/$RID h03.json)"
say "         states: $(grep -oE '"state":"[a-z_]+"' "$EV/h03.json" | tr '\n' ' ')"
V1=$(grep -oE '"id":"[0-9a-f-]{36}"' "$EV/h03.json" | head -2 | tail -1 | cut -d'"' -f4)
H1=$(jhash h03.json)
ROW=$(jrow h03.json)
say "         v1_revision=$V1 hash=$H1 row_revision=$ROW"

say "[B01-03] PATCH draft v1: $(http PATCH /v1/pm/requirements/$RID/revisions/$V1 h04.json '{"markdown":"# REQ-QA-1\n\nBody v1 edited"}')"
H1B=$(jhash h04.json)
say "         hash v1 -> $H1B"
say "[B01-04] GET list: $(http GET /v1/pm/projects/$PID/requirements h05.json) items=$(grep -o '"code"' "$EV/h05.json" | wc -l)"
say "[B01-04] GET list with unsupported filter probe: $(http GET "/v1/pm/projects/$PID/requirements?lifecycle_status=archived" h05b.json) (record actual)"

say "[B01-09] duplicate create same code: $(http POST /v1/pm/projects/$PID/requirements h06.json '{"code":"REQ-QA-1","title":"dup probe","markdown":"# dup"}') -> ids=$(grep -oE '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}' "$EV/h06.json" | head -1)"
say "[B01-12] GET unknown requirement: $(http GET /v1/pm/requirements/11111111-1111-1111-1111-111111111111 h07.json)"
say "[B01-12] POST unknown project: $(http POST /v1/pm/projects/22222222-2222-2222-2222-222222222222/requirements h08.json '{"code":"X","title":"x","markdown":"x"}')"

say "[B02-03] submit v1: $(http POST /v1/pm/requirements/$RID/revisions/$V1/submit h09.json) -> $(jref h09.json)"
say "[B05-07] re-submit v1 (already bound): $(http POST /v1/pm/requirements/$RID/revisions/$V1/submit h10.json)"
say "[B02-04] confirm v1: $(http POST /v1/pm/requirements/$RID/revisions/$V1/confirm h11.json "{\"expected_content_hash\":\"$H1B\",\"expected_row_revision\":$ROW,\"expected_current_revision_id\":null}")"
ROW2=$(jrow h11.json)
say "         after confirm: row_revision=$ROW2 $(jstate h11.json)"
say "[B02-08] duplicate confirm v1: $(http POST /v1/pm/requirements/$RID/revisions/$V1/confirm h12.json "{\"expected_content_hash\":\"$H1B\"}")"
say "[B02-07] PATCH approved v1: $(http PATCH /v1/pm/requirements/$RID/revisions/$V1 h13.json '{"markdown":"# hack"}')"

say "[B02-05] create draft v2: $(http POST /v1/pm/requirements/$RID/revisions h14.json '{"markdown":"# REQ-QA-1\n\nBody v2"}')"
V2=$(jid h14.json); H2=$(jhash h14.json)
say "         v2=$V2 hash=$H2"
say "         submit v2: $(http POST /v1/pm/requirements/$RID/revisions/$V2/submit h15.json)"
say "         confirm v2: $(http POST /v1/pm/requirements/$RID/revisions/$V2/confirm h16.json "{\"expected_content_hash\":\"$H2\",\"expected_row_revision\":$ROW2,\"expected_current_revision_id\":\"$V1\"}")"
say "[B02-06] detail after supersede: $(http GET /v1/pm/requirements/$RID h17.json)"
grep -oE '"id":"[0-9a-f-]{36}"|"state":"[a-z_]+"' "$EV/h17.json" | paste - - | sed 's/"//g' | tee -a "$LOG"

say "[B02-16] create draft v3: $(http POST /v1/pm/requirements/$RID/revisions h18.json '{"markdown":"# REQ-QA-1\n\nBody v3"}')"
V3=$(jid h18.json); H3=$(jhash h18.json)
ROW3=$(jrow h18.json)
say "         v3=$V3 hash=$H3 row=$ROW3"
say "         submit v3: $(http POST /v1/pm/requirements/$RID/revisions/$V3/submit h19.json)"
say "         reject v3: $(http POST /v1/pm/requirements/$RID/revisions/$V3/reject h20.json '{"reason":"qa reject probe"}')"
say "         confirm rejected v3 (must fail): $(http POST /v1/pm/requirements/$RID/revisions/$V3/confirm h21.json "{\"expected_content_hash\":\"$H3\"}")"
say "         PATCH rejected v3 (must fail): $(http PATCH /v1/pm/requirements/$RID/revisions/$V3 h22.json '{"markdown":"# x"}')"

say "[B02-13] create draft v4: $(http POST /v1/pm/requirements/$RID/revisions h23.json '{"markdown":"# REQ-QA-1\n\nBody v4"}')"
V4=$(jid h23.json); H4=$(jhash h23.json); ROW4=$(jrow h23.json)
say "         v4=$V4 hash=$H4 row=$ROW4"
say "         submit v4: $(http POST /v1/pm/requirements/$RID/revisions/$V4/submit h24.json)"
say "         confirm v4 with STALE row_revision ($ROW4, actual now $((ROW4+1)) expect 409): $(http POST /v1/pm/requirements/$RID/revisions/$V4/confirm h25.json "{\"expected_content_hash\":\"$H4\",\"expected_row_revision\":$ROW4,\"expected_current_revision_id\":\"$V2\"}")"
ROW5=$(jrow h25.json)
say "         row_revision after 409: $ROW5"
say "         confirm v4 correct: $(http POST /v1/pm/requirements/$RID/revisions/$V4/confirm h26.json "{\"expected_content_hash\":\"$H4\",\"expected_row_revision\":$ROW5,\"expected_current_revision_id\":\"$V2\"}")"

say "[B01-13] lifecycle withdraw: $(http POST /v1/pm/requirements/$RID/lifecycle/withdraw h27.json)"
say "         draft after withdraw (expect 409): $(http POST /v1/pm/requirements/$RID/revisions h28.json '{"markdown":"# x"}')"
say "         restore: $(http POST /v1/pm/requirements/$RID/lifecycle/restore h29.json)"
say "         archive: $(http POST /v1/pm/requirements/$RID/lifecycle/archive h30.json)"
say "         draft after archive (expect 409): $(http POST /v1/pm/requirements/$RID/revisions h31.json '{"markdown":"# x"}')"
say "         restore archived (expect 409, I1-D05): $(http POST /v1/pm/requirements/$RID/lifecycle/restore h32.json)"

say "[B01-10/DF1] create project B: $(http POST /v1/pm/projects h33.json "{\"code\":\"$CODE_B\",\"name\":\"S02 QA Project B\"}") -> $(jid h33.json)"
PIDB=$(jid h33.json)
say "         create requirement in B: $(http POST /v1/pm/projects/$PIDB/requirements h34.json '{"code":"REQ-QA-B1","title":"B one","markdown":"# B1"}')"
RIDB=$(jid h34.json)
DEV_OFF=$(PG "SELECT m.principal_id FROM core.project_member m JOIN core.project p ON p.id=m.project_id WHERE p.code='$CODE_B' AND m.role='owner' LIMIT 1")
say "         remove dev-off membership in B (principal=$DEV_OFF): $(PG "DELETE FROM core.project_member WHERE project_id='$PIDB' AND principal_id='$DEV_OFF' RETURNING project_id" >/dev/null; echo done)"
say "         [DF1-01] GET detail in B (expect 404): $(http GET /v1/pm/requirements/$RIDB h35.json)"
say "         [DF1-02] list in B (expect 404): $(http GET /v1/pm/projects/$PIDB/requirements h36.json)"
say "         [DF1-03] create in B (expect 404): $(http POST /v1/pm/projects/$PIDB/requirements h37.json '{"code":"EVIL","title":"evil","markdown":"# e"}')"
say "         [DF1-04] draft in B (expect 404): $(http POST /v1/pm/requirements/$RIDB/revisions h38.json '{"markdown":"# e"}')"
say "         [DF1-05] cross-URL: GET project-A req via B context probe (B404 path): $(http GET /v1/pm/requirements/$RID/revisions h39.json) (B-membership removed only for RIDB)"

say "[B05-01/DEF-03] create draft v5 v6 (same base v4):"
S5=$(http POST /v1/pm/requirements/$RID/revisions c18-d5.json '{"markdown":"# REQ-QA-1\n\nConcurrent A"}'); V5=$(jid c18-d5.json); H5=$(jhash c18-d5.json)
S6=$(http POST /v1/pm/requirements/$RID/revisions c18-d6.json '{"markdown":"# REQ-QA-1\n\nConcurrent B"}'); V6=$(jid c18-d6.json); H6=$(jhash c18-d6.json)
say "         v5=$V5 ($S5) v6=$V6 ($S6)"
say "         submit v5: $(http POST /v1/pm/requirements/$RID/revisions/$V5/submit c18-s5.json)"
say "         submit v6: $(http POST /v1/pm/requirements/$RID/revisions/$V6/submit c18-s6.json)"
ROWC=$(jrow c18-s6.json)
say "         row_revision before race: $ROWC"
say "         RACE confirm v5 & v6 in parallel:"
( curl -s -m 20 -o "$EV/c18-c5.json" -w "%{http_code}" -X POST -H 'content-type: application/json' -d "{\"expected_content_hash\":\"$H5\",\"expected_row_revision\":$ROWC,\"expected_current_revision_id\":\"$V4\"}" "$BASE/v1/pm/requirements/$RID/revisions/$V5/confirm" > "$EV/c18-code5.txt" ) &
( curl -s -m 20 -o "$EV/c18-c6.json" -w "%{http_code}" -X POST -H 'content-type: application/json' -d "{\"expected_content_hash\":\"$H6\",\"expected_row_revision\":$ROWC,\"expected_current_revision_id\":\"$V4\"}" "$BASE/v1/pm/requirements/$RID/revisions/$V6/confirm" > "$EV/c18-code6.txt" ) &
wait
say "         race result: v5=$(cat "$EV/c18-code5.txt") v6=$(cat "$EV/c18-code6.txt")"
say "         $(jstate c18-c5.json) / $(jstate c18-c6.json)"

say "=== DB final state ==="
PG "SELECT r.version, r.state, r.id FROM delivery.requirement_revision r WHERE r.requirement_id='$RID' ORDER BY r.version" | tee -a "$LOG"
say "current_revision_id: $(PG "SELECT current_revision_id FROM delivery.requirement WHERE id='$RID'")"
say "confirmations: $(PG "SELECT action, count(*) FROM delivery.requirement_confirmation WHERE requirement_id='$RID' GROUP BY action ORDER BY action")"
say "audit events: $(PG "SELECT action, count(*) FROM governance.audit_log WHERE entity_id='$RID' GROUP BY action ORDER BY action")"
say "dup probe: $(PG "SELECT count(*) FROM delivery.requirement WHERE project_id='$PID' AND code='REQ-QA-1'")"

KR="$EV/knowledge-root"
say "=== Git authority ==="
say "git root: $(git -C "$KR" rev-parse --is-inside-work-tree 2>&1)"
say "commits: $(git -C "$KR" log --oneline | head -20 | tr '\n' ' | ')"
find "$KR" -name "*.md" -not -path "*/.git/*" | while read -r f; do
  say "file: ${f#$KR/} sha256=$(sha256sum "$f" | cut -d' ' -f1)"
done
say "=== suite end ==="