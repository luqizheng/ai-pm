#!/usr/bin/env bash
# S02 QA race & CAS probe V1.0: DEF-03 parallel confirm + pure row_revision mismatch.
set -u
BASE=http://127.0.0.1:18090
EV="$(cd "$(dirname "$0")" && pwd)"
LOG="$EV/race-probe.log"
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
TS=$(date -u +%H%M%S)
CODE_A=$(PG "SELECT code FROM core.project WHERE id='95466cf3-8abf-4e4c-bc15-c8f92abbe765'")
PID=95466cf3-8abf-4e4c-bc15-c8f92abbe765

say "=== race & CAS probe start (project $PID) ==="
say "create REQ-QA-2: $(http POST /v1/pm/projects/$PID/requirements r01.json '{"code":"REQ-QA-2","title":"race probe","markdown":"# race v0"}')"
RID=$(grep -oE '"id":"[0-9a-f-]{36}"' "$EV/r01.json" | head -1 | cut -d'"' -f4)
say "requirement=$RID"

http POST /v1/pm/requirements/$RID/revisions r02.json '{"markdown":"# race body A"}'
http POST /v1/pm/requirements/$RID/revisions r03.json '{"markdown":"# race body B"}'
VA=$(PG "SELECT id FROM delivery.requirement_revision WHERE requirement_id='$RID' AND version=1")
VB=$(PG "SELECT id FROM delivery.requirement_revision WHERE requirement_id='$RID' AND version=2")
HA=$(PG "SELECT content_hash FROM delivery.requirement_revision WHERE id='$VA'")
HB=$(PG "SELECT content_hash FROM delivery.requirement_revision WHERE id='$VB'")
say "v1=$VA v2=$VB"
say "submit v1: $(http POST /v1/pm/requirements/$RID/revisions/$VA/submit r04.json)"
say "submit v2: $(http POST /v1/pm/requirements/$RID/revisions/$VB/submit r05.json)"
ROW=$(PG "SELECT row_revision FROM delivery.requirement WHERE id='$RID'")
say "row_before_race=$ROW current=$(PG "SELECT COALESCE(current_revision_id::text,'null') FROM delivery.requirement WHERE id='$RID'")"

say "RACE confirm v1 & v2 parallel (same tokens):"
( curl -s -m 20 -o "$EV/r-c1.json" -w "%{http_code}" -X POST -H 'content-type: application/json' -d "{\"expected_content_hash\":\"$HA\",\"expected_row_revision\":$ROW,\"expected_current_revision_id\":null}" "$BASE/v1/pm/requirements/$RID/revisions/$VA/confirm" > "$EV/r-code1.txt" ) &
( curl -s -m 20 -o "$EV/r-c2.json" -w "%{http_code}" -X POST -H 'content-type: application/json' -d "{\"expected_content_hash\":\"$HB\",\"expected_row_revision\":$ROW,\"expected_current_revision_id\":null}" "$BASE/v1/pm/requirements/$RID/revisions/$VB/confirm" > "$EV/r-code2.txt" ) &
wait
say "race result: v1=$(cat "$EV/r-code1.txt") v2=$(cat "$EV/r-code2.txt")"
say "v1 message: $(cat "$EV/r-c1.json" | head -c 200)"
say "v2 message: $(cat "$EV/r-c2.json" | head -c 200)"
say "final: v1=$(PG "SELECT state FROM delivery.requirement_revision WHERE id='$VA'") v2=$(PG "SELECT state FROM delivery.requirement_revision WHERE id='$VB'") current=$(PG "SELECT COALESCE(current_revision_id::text,'null') FROM delivery.requirement WHERE id='$RID'") approved=$(PG "SELECT count(*) FROM delivery.requirement_revision WHERE requirement_id='$RID' AND state='approved'") superseded=$(PG "SELECT count(*) FROM delivery.requirement_revision WHERE requirement_id='$RID' AND state='superseded'")"

ROWN=$(PG "SELECT row_revision FROM delivery.requirement WHERE id='$RID'")
http POST /v1/pm/requirements/$RID/revisions r06.json '{"markdown":"# race body C"}'
VC=$(PG "SELECT id FROM delivery.requirement_revision WHERE requirement_id='$RID' AND version=3")
HC=$(PG "SELECT content_hash FROM delivery.requirement_revision WHERE id='$VC'")
say "submit v3: $(http POST /v1/pm/requirements/$RID/revisions/$VC/submit r07.json)"
say "[CAS-row] confirm v3 with row_revision=999 (expect 409 row mismatch): $(http POST /v1/pm/requirements/$RID/revisions/$VC/confirm r08.json "{\"expected_content_hash\":\"$HC\",\"expected_row_revision\":999,\"expected_current_revision_id\":null}")"
say "message: $(cat "$EV/r08.json" | head -c 200)"
ROWN2=$(PG "SELECT row_revision FROM delivery.requirement WHERE id='$RID'")
say "row unchanged: $ROWN -> $ROWN2"
say "[CAS-cur] confirm v3 correct: $(http POST /v1/pm/requirements/$RID/revisions/$VC/confirm r09.json "{\"expected_content_hash\":\"$HC\",\"expected_row_revision\":$ROWN2,\"expected_current_revision_id\":null}")"
say "final states: v1=$(PG "SELECT state FROM delivery.requirement_revision WHERE id='$VA'") v2=$(PG "SELECT state FROM delivery.requirement_revision WHERE id='$VB'") v3=$(PG "SELECT state FROM delivery.requirement_revision WHERE id='$VC'")"
say "=== probe end ==="