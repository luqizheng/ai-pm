#!/usr/bin/env bash
# S02 QA REST acceptance suite V1.1 (evidence script)
# Fix vs V1.0: knowledge root outside repo; lifecycle route without prefix;
# revision ids/hashes/row_revision taken from PostgreSQL (DB as evidence).
set -u
BASE=http://127.0.0.1:18090
EV="$(cd "$(dirname "$0")" && pwd)"
LOG="$EV/rest-execution-v11.log"
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
# DB-backed facts (requirement id RID, revision version N)
rid_state() { PG "SELECT state FROM delivery.requirement_revision WHERE requirement_id='$1' AND version=$2"; }
rid_rev()   { PG "SELECT id FROM delivery.requirement_revision WHERE requirement_id='$1' AND version=$2"; }
rid_hash()  { PG "SELECT content_hash FROM delivery.requirement_revision WHERE requirement_id='$1' AND version=$2"; }
rid_row()   { PG "SELECT row_revision FROM delivery.requirement WHERE id='$1'"; }
rid_cur()   { PG "SELECT COALESCE(current_revision_id::text,'null') FROM delivery.requirement WHERE id='$1'"; }
TS=$(date -u +%H%M%S)
CODE_A="S02QA-A-$TS"
CODE_B="S02QA-B-$TS"

say "=== S02 QA REST suite V1.1 start (A=$CODE_A B=$CODE_B) ==="
say "[B01-00] health: $(http GET /health h00.json)"
say "[B01-01] create project A: $(http POST /v1/pm/projects h01.json "{\"code\":\"$CODE_A\",\"name\":\"S02 QA Project A\"}")"
PID=$(grep -oE '"id":"[0-9a-f-]{36}"' "$EV/h01.json" | head -1 | cut -d'"' -f4)
say "         project_id=$PID"
say "[B01-01] create requirement: $(http POST /v1/pm/projects/$PID/requirements h02.json '{"code":"REQ-QA-1","title":"S02 QA requirement one","requirement_type":"functional","priority":"high","markdown":"# REQ-QA-1\n\nBody v1"}')"
RID=$(grep -oE '"id":"[0-9a-f-]{36}"' "$EV/h02.json" | head -1 | cut -d'"' -f4)
say "         requirement_id=$RID"

say "[B01-02] GET detail: $(http GET /v1/pm/requirements/$RID h03.json) rev1_state=$(rid_state $RID 1)"
say "[B01-03] PATCH draft v1: $(http PATCH /v1/pm/requirements/$RID/revisions/$(rid_rev $RID 1) h04.json '{"markdown":"# REQ-QA-1\n\nBody v1 edited"}') hash_now=$(rid_hash $RID 1)"
say "[B01-04] GET list: $(http GET /v1/pm/projects/$PID/requirements h05.json) items=$(grep -o '"code"' "$EV/h05.json" | wc -l)"
say "[B01-04] list unsupported-filter probe: $(http GET "/v1/pm/projects/$PID/requirements?lifecycle_status=archived" h05b.json) (record: filter ignored)"
say "[B01-09] duplicate create same code: $(http POST /v1/pm/projects/$PID/requirements h06.json '{"code":"REQ-QA-1","title":"dup probe","markdown":"# dup"}') db_count=$(PG "SELECT count(*) FROM delivery.requirement WHERE project_id='$PID' AND code='REQ-QA-1'")"
say "[B01-12] GET unknown requirement: $(http GET /v1/pm/requirements/11111111-1111-1111-1111-111111111111 h07.json)"
say "[B01-12] POST unknown project: $(http POST /v1/pm/projects/22222222-2222-2222-2222-222222222222/requirements h08.json '{"code":"X","title":"x","markdown":"x"}')"

V1=$(rid_rev $RID 1); H1B=$(rid_hash $RID 1); ROW1=$(rid_row $RID)
say "[B02-03] submit v1: $(http POST /v1/pm/requirements/$RID/revisions/$V1/submit h09.json) state=$(rid_state $RID 1) ref=$(PG "SELECT COALESCE(markdown_version_ref,'none') FROM delivery.requirement_revision WHERE id='$V1'")"
say "[B05-07] re-submit v1 (idempotent/pending): $(http POST /v1/pm/requirements/$RID/revisions/$V1/submit h10.json)"
CUR=$(rid_cur $RID)
say "[B02-04] confirm v1: $(http POST /v1/pm/requirements/$RID/revisions/$V1/confirm h11.json "{\"expected_content_hash\":\"$H1B\",\"expected_row_revision\":$ROW1,\"expected_current_revision_id\":null}")"
say "         after: state=$(rid_state $RID 1) current=$(rid_cur $RID) row=$(rid_row $RID)"
ROW2=$(rid_row $RID)
say "[B02-08] duplicate confirm v1: $(http POST /v1/pm/requirements/$RID/revisions/$V1/confirm h12.json "{\"expected_content_hash\":\"$H1B\"}")"
say "[B02-07] PATCH approved v1: $(http PATCH /v1/pm/requirements/$RID/revisions/$V1 h13.json '{"markdown":"# hack"}')"
say "         approved content unchanged: $(rid_hash $RID 1) (expect $H1B)"

say "[B02-05] draft v2: $(http POST /v1/pm/requirements/$RID/revisions h14.json '{"markdown":"# REQ-QA-1\n\nBody v2"}')"
V2=$(rid_rev $RID 2); H2=$(rid_hash $RID 2)
say "         v2=$V2 hash=$H2 state=$(rid_state $RID 2)"
say "         submit v2: $(http POST /v1/pm/requirements/$RID/revisions/$V2/submit h15.json) state=$(rid_state $RID 2)"
say "         confirm v2: $(http POST /v1/pm/requirements/$RID/revisions/$V2/confirm h16.json "{\"expected_content_hash\":\"$H2\",\"expected_row_revision\":$ROW2,\"expected_current_revision_id\":\"$V1\"}")"
say "[B02-06] after supersede: v1=$(rid_state $RID 1) v2=$(rid_state $RID 2) current=$(rid_cur $RID)"
say "         confirm superseded v1 (expect 409): $(http POST /v1/pm/requirements/$RID/revisions/$V1/confirm h16b.json "{\"expected_content_hash\":\"$H1B\"}")"

say "[B02-16] draft v3: $(http POST /v1/pm/requirements/$RID/revisions h18.json '{"markdown":"# REQ-QA-1\n\nBody v3"}')"
V3=$(rid_rev $RID 3); H3=$(rid_hash $RID 3)
say "         v3=$V3"
say "         submit v3: $(http POST /v1/pm/requirements/$RID/revisions/$V3/submit h19.json)"
say "         reject v3: $(http POST /v1/pm/requirements/$RID/revisions/$V3/reject h20.json '{"reason":"qa reject probe"}') state=$(rid_state $RID 3)"
say "         confirm rejected v3 (expect 409): $(http POST /v1/pm/requirements/$RID/revisions/$V3/confirm h21.json "{\"expected_content_hash\":\"$H3\"}")"
say "         PATCH rejected v3 (expect 409): $(http PATCH /v1/pm/requirements/$RID/revisions/$V3 h22.json '{"markdown":"# x"}')"
say "         re-submit rejected v3 (expect 409): $(http POST /v1/pm/requirements/$RID/revisions/$V3/submit h22b.json)"

say "[B02-13] draft v4: $(http POST /v1/pm/requirements/$RID/revisions h23.json '{"markdown":"# REQ-QA-1\n\nBody v4"}')"
V4=$(rid_rev $RID 4); H4=$(rid_hash $RID 4); ROW4=$(rid_row $RID)
say "         v4=$V4 row_before=$ROW4"
say "         submit v4: $(http POST /v1/pm/requirements/$RID/revisions/$V4/submit h24.json)"
say "         confirm v4 with stale row_revision ($ROW4, expect 409): $(http POST /v1/pm/requirements/$RID/revisions/$V4/confirm h25.json "{\"expected_content_hash\":\"$H4\",\"expected_row_revision\":$ROW4,\"expected_current_revision_id\":\"$V2\"}")"
ROW5=$(rid_row $RID)
say "         row after 409: $ROW5 (expect unchanged)"
say "         confirm v4 correct: $(http POST /v1/pm/requirements/$RID/revisions/$V4/confirm h26.json "{\"expected_content_hash\":\"$H4\",\"expected_row_revision\":$ROW5,\"expected_current_revision_id\":\"$V2\"}") current=$(rid_cur $RID)"

say "[B01-13] lifecycle withdraw: $(http POST /v1/pm/requirements/$RID/withdraw h27.json) status=$(PG "SELECT lifecycle_status FROM delivery.requirement WHERE id='$RID'")"
say "         draft after withdraw (expect 409): $(http POST /v1/pm/requirements/$RID/revisions h28.json '{"markdown":"# x"}')"
say "         restore: $(http POST /v1/pm/requirements/$RID/restore h29.json) status=$(PG "SELECT lifecycle_status FROM delivery.requirement WHERE id='$RID'")"
say "         archive: $(http POST /v1/pm/requirements/$RID/archive h30.json) status=$(PG "SELECT lifecycle_status FROM delivery.requirement WHERE id='$RID'")"
say "         draft after archive (expect 409): $(http POST /v1/pm/requirements/$RID/revisions h31.json '{"markdown":"# x"}')"
say "         restore archived (expect 409, I1-D05): $(http POST /v1/pm/requirements/$RID/restore h32.json)"

say "[B01-10/DF1] create project B: $(http POST /v1/pm/projects h33.json "{\"code\":\"$CODE_B\",\"name\":\"S02 QA Project B\"}")"
PIDB=$(grep -oE '"id":"[0-9a-f-]{36}"' "$EV/h33.json" | head -1 | cut -d'"' -f4)
say "         create requirement in B: $(http POST /v1/pm/projects/$PIDB/requirements h34.json '{"code":"REQ-QA-B1","title":"B one","markdown":"# B1"}')"
RIDB=$(grep -oE '"id":"[0-9a-f-]{36}"' "$EV/h34.json" | head -1 | cut -d'"' -f4)
DEV_OFF=$(PG "SELECT m.principal_id FROM core.project_member m JOIN core.project p ON p.id=m.project_id WHERE p.code='$CODE_B' AND m.role='owner' LIMIT 1")
PG "DELETE FROM core.project_member WHERE project_id='$PIDB' AND principal_id='$DEV_OFF'" >/dev/null
say "         removed dev-off membership (principal=$DEV_OFF)"
say "         [DF1-01] GET detail in B (expect 404): $(http GET /v1/pm/requirements/$RIDB h35.json)"
say "         [DF1-02] list in B (expect 404): $(http GET /v1/pm/projects/$PIDB/requirements h36.json)"
say "         [DF1-03] create in B (expect 404): $(http POST /v1/pm/projects/$PIDB/requirements h37.json '{"code":"EVIL","title":"evil","markdown":"# e"}')"
say "         [DF1-04] draft in B (expect 404): $(http POST /v1/pm/requirements/$RIDB/revisions h38.json '{"markdown":"# e"}')"
say "         [DF1-05] submit in B (expect 404): $(http POST /v1/pm/requirements/$RIDB/revisions/$(rid_rev $RIDB 1)/submit h38b.json)"
say "         [DF1-06] confirm in B (expect 404): $(http POST /v1/pm/requirements/$RIDB/revisions/$(rid_rev $RIDB 1)/confirm h38c.json '{"expected_content_hash":"0"}')"

say "[B05-01/DEF-03] drafts v5/v6 on same base v4:"
http POST /v1/pm/requirements/$RID/revisions c18-d5.json '{"markdown":"# REQ-QA-1\n\nConcurrent A"}'
http POST /v1/pm/requirements/$RID/revisions c18-d6.json '{"markdown":"# REQ-QA-1\n\nConcurrent B"}'
V5=$(rid_rev $RID 5); V6=$(rid_rev $RID 6); H5=$(rid_hash $RID 5); H6=$(rid_hash $RID 6)
say "         v5=$V5 v6=$V6"
say "         submit v5: $(http POST /v1/pm/requirements/$RID/revisions/$V5/submit c18-s5.json)"
say "         submit v6: $(http POST /v1/pm/requirements/$RID/revisions/$V6/submit c18-s6.json)"
ROWC=$(rid_row $RID)
say "         row before race: $ROWC current=$(rid_cur $RID)"
say "         RACE confirm v5 & v6 parallel:"
( curl -s -m 20 -o "$EV/c18-c5.json" -w "%{http_code}" -X POST -H 'content-type: application/json' -d "{\"expected_content_hash\":\"$H5\",\"expected_row_revision\":$ROWC,\"expected_current_revision_id\":\"$V4\"}" "$BASE/v1/pm/requirements/$RID/revisions/$V5/confirm" > "$EV/c18-code5.txt" ) &
( curl -s -m 20 -o "$EV/c18-c6.json" -w "%{http_code}" -X POST -H 'content-type: application/json' -d "{\"expected_content_hash\":\"$H6\",\"expected_row_revision\":$ROWC,\"expected_current_revision_id\":\"$V4\"}" "$BASE/v1/pm/requirements/$RID/revisions/$V6/confirm" > "$EV/c18-code6.txt" ) &
wait
say "         race result: v5=$(cat "$EV/c18-code5.txt") v6=$(cat "$EV/c18-code6.txt")"
say "         final: v5=$(rid_state $RID 5) v6=$(rid_state $RID 6) current=$(rid_cur $RID) superseded_count=$(PG "SELECT count(*) FROM delivery.requirement_revision WHERE requirement_id='$RID' AND state='superseded'") approved_count=$(PG "SELECT count(*) FROM delivery.requirement_revision WHERE requirement_id='$RID' AND state='approved'")"

say "=== audit trail ==="
PG "SELECT action, count(*) FROM governance.audit_log WHERE target_id='$RID' GROUP BY action ORDER BY action" | tee -a "$LOG"
say "confirmation rows:"
PG "SELECT action, revision_id, human_principal_id, confirmed_at FROM delivery.requirement_confirmation WHERE requirement_id='$RID' ORDER BY confirmed_at" | tee -a "$LOG"
say "audit actor/auth: $(PG "SELECT DISTINCT actor_auth_mode FROM governance.audit_log WHERE target_id='$RID'")"

say "=== Git authority ==="
KR="D:/kudi-project/templates/.s02qa-knowledge"
say "standalone git: $(git -C "$KR" rev-parse --is-inside-work-tree 2>&1)"
git -C "$KR" log --oneline | head -10 | while IFS= read -r l; do say "commit: $l"; done
find "$KR" -name "*.md" -not -path "*/.git/*" | while read -r f; do
  say "file: $f sha256=$(sha256sum "$f" | cut -d' ' -f1)"
done
say "sha256 of v4 approved markdown vs DB hash $H4: see above (req-*.md)"
say "=== suite end ==="