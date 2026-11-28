# Harness run record — 2026-11-28

- Environment: disposable `postgres:16` container (PostgreSQL 16.16, Debian)
  on 127.0.0.1:16433, ephemeral database `sem_inbox_acceptance_<ts>` created
  from `fixture_schema.sql`, dropped after the run.
- Driver: `run_acceptance.py` (two independent connections + barriers,
  injected mid-transaction failures, at-least-once gap-retry).
- Final result: **30 passed, 0 failed (total 30)** after three iterations:
  the first run (killed mid-run, leaked one ephemeral database, since
  dropped) exposed a real contract defect; the second fixed harness bugs;
  the third fixed the last retry-order expectation.

## Contract defect found and fixed by this harness

The §3 observation detail CHECK originally required
`COALESCE(detail->>'binding_epoch','') ~ '^[1-9][0-9]*$'` on every
`sem_infrastructure_observed` row. The sixth event
(`execution.unbound_release_applied`) legitimately carries **no**
binding_epoch, so every valid tombstone observation was rejected
(`task_run_event_sem_sequence_check` violation). The CHECK has been amended
(binding-scoped events require a positive epoch; the tombstone event requires
it to be absent/null) in both the design document §3 and `fixture_schema.sql`.
Fabricating epoch 0 remains forbidden on both sides.

## Final pass list (30)

| # | Scenario | Result |
|---|---|---|
| H-01 | INBOX-01 first event commits inbox+observation+cursor atomically | PASS |
| H-02 | INBOX-02 exact replay is duplicate without second observation | PASS |
| H-03 | INBOX-03 same event id changed body is IDEMPOTENCY_CONFLICT | PASS |
| H-04 | INBOX-05 same sequence different event ids: one commits, loser conflicts | PASS |
| H-05 | INBOX-05 backstop: partial unique index admits exactly one sequence owner | PASS |
| H-06 | INBOX-07 sequence gap rejected without writes | PASS |
| H-07 | INBOX-08 commit-before-ACK replay returns duplicate, cursor stable | PASS |
| H-08 | INBOX-09 crash after Inbox insert rolls back everything | PASS |
| H-09 | INBOX-20/21 failure after observation/cursor rolls back all writes | PASS |
| H-10 | INBOX-19 concurrent N and N+1 serialize to ordered cursor N+1 | PASS |
| H-11 | BIND-03 same epoch changed runner tuple rejects (identity mismatch) | PASS |
| H-12 | BIND-04/BIND-11 lower/stale epoch event after release rejects without regression | PASS |
| H-13 | BIND-05 higher epoch after positive release proof atomically replaces tuple | PASS |
| H-14 | BIND-07 higher epoch while prior binding uncertain rejects and retains quarantine | PASS |
| H-15 | BIND-08 concurrent first-binding serializes; exactly one wins | PASS |
| H-16 | BIND-13 two epochs in flight: epoch1 wins, epoch2 lacks release proof | PASS |
| H-17 | BIND-09 mid-transaction failure rolls back all four writes; replay succeeds | PASS |
| H-18 | BIND-06 rebind after explicit host-isolation disposition reference | PASS |
| H-19 | BIND-12 stale runner_key_version observation rejects, cursor unchanged | PASS |
| H-20 | INBOX-50 first sixth event creates released_unbound tombstone | PASS |
| H-21 | INBOX-51 duplicate sixth event ACKs without rewriting projection | PASS |
| H-22 | INBOX-52/UNBOUND-03 sixth event after binding rejects | PASS |
| H-23 | INBOX-53/UNBOUND-04 binding after tombstone rejects regardless of epoch | PASS |
| H-24 | UNBOUND-10 sixth-event mid-transaction failure rolls back all four writes | PASS |
| H-25 | INBOX-54/UNBOUND-09 release-vs-binding race yields exactly one terminal state | PASS |
| H-26 | INBOX-56/UNBOUND-08 tombstone shape enforced by app check + DDL tuple CHECK | PASS |
| H-27 | INBOX-57 sixth event with non-tombstone disposition rejects before writes | PASS |
| H-28 | INBOX-25/26 same global event id from another run conflicts | PASS |
| H-29 | INBOX-24/34 duplicate JSON members rejected before persistence | PASS |
| H-30 | INBOX-36..38 source CHECK truth table: non-SEM row with SEM fields rejects | PASS |

## Still NOT proven by this harness

- The production ai-pm migration, callback route, Rust application port and
  JCS canonicalizer do not exist; this harness validates the candidate DDL and
  transaction algorithm only.
- Bilateral live HTTP callback interoperability, signature/nonce middleware
  ordering and response signing are untested here.
- The JCS implementation in the harness covers the ASCII/integer subset used
  by the scenarios; the pinned RFC 8785 corpus and cross-language vector
  requirements remain separate obligations.
