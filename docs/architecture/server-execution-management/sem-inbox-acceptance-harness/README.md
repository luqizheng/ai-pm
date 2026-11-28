# SEM Callback Inbox Acceptance Harness

- Status: TEST-ONLY executable acceptance harness (2026-11-28)
- Validates: [SEM Callback Inbox — Exact DDL and Concurrency Design](../sem-callback-inbox-ddl-and-concurrency-design.md) §3/§13/§14
- Scope boundary: this is **not** the production callback adapter, **not** an
  ai-pm migration, and **not** bilateral live HTTP integration. It executes the
  candidate DDL and transaction algorithm against a real, ephemeral PostgreSQL
  database and drops that database afterwards.

## What it proves

Real-PostgreSQL, two-connection execution of the §13 transaction algorithm
(frozen lock order: Run row → execution projection → Inbox → Run observation →
cursor CAS → COMMIT) and the §14 canonical single-table projection rules:

- INBOX-01..09/19/20/21/24/25/26/34/36..38/50..57 baseline + sixth-event scenarios
- BIND-03..09/12/13/14 binding-projection rules, including concurrent first-bind
  and two-epochs-in-flight races
- UNBOUND-03/04/08/09/10 tombstone semantics and release-vs-binding races
- DDL backstops: partial unique `(execution_id, execution_event_sequence)` index,
  observation detail CHECK (including the 2026-11-28 binding_epoch tombstone
  correction), projection state/tuple CHECKs, source CHECK truth table
- Duplicate-JSON-member rejection before persistence

Result ledger: [RESULTS.md](RESULTS.md).

## Running

Requirements: Python 3 with `psycopg2-binary`, network reachability to a
disposable PostgreSQL 16 instance. The harness creates and drops its own
database; the target server must allow `CREATE DATABASE`.

```bash
python run_acceptance.py --dsn postgres://aipm:aipm@127.0.0.1:16433/postgres
```

On unexpected termination the ephemeral database may leak; clean it with:

```sql
DROP DATABASE sem_inbox_acceptance_<ts> WITH (FORCE);
```

## Files

- `fixture_schema.sql` — test-only fixture schema (mirrored baseline
  constraints + candidate §3/§14 DDL). Never apply to a shared database.
- `run_acceptance.py` — the 30-scenario driver with deterministic barriers,
  injected mid-transaction failures, and at-least-once gap-retry semantics.
- `RESULTS.md` — recorded run output.
