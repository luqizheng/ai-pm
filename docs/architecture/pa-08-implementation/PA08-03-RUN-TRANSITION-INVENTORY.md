# PA08-03 — Run Transition Inventory

- **Status:** INVENTORY BASELINE (implementation not started)
- **Date:** 2026-11-22
- **Branch:** `pa-08/implementation-baseline`
- **Purpose:** Map existing Run status writes before introducing a shared transition guard. This is a code inventory, not proof that every concurrency path is correct.

## 1. Non-negotiable semantic boundaries

1. A Run is an execution attempt; it is not the Task state machine.
2. Delivery Runs and platform Reviewer Runs share `delivery.task_run`, but their execution/recovery lifecycles are distinct. The guard must branch on `run_kind`; it must not apply delivery lease/session rules to Reviewer Runs.
3. Existing statuses are constrained by the baseline schema to:
   `queued`, `claimed`, `running`, `succeeded`, `failed`, `timed_out`, `cancelled`.
4. Outcomes are constrained to terminal outcome values:
   `succeeded`, `failed`, `timed_out`, `cancelled`.
5. Terminal Run rows are historical records. PA-08 must not reopen them, overwrite their outcome, or mutate Task state as a side effect of Run status.
6. Lease fencing remains token + generation + active-status validation. A transition guard is not a substitute for credential verification or authorization.
7. State mutation, audit, failure evidence, Run events and outbox effects must retain their existing transaction boundaries.

## 2. Delivery Run write-path inventory

| Path | Current operation | Existing protection / concern |
|---|---|---|
| `run_dispatch/claim.rs::claim_core` | `queued → claimed`; expired `claimed` may be reclaimed | Candidate selected with `FOR UPDATE SKIP LOCKED`; claim update increments generation and signs lease. Reclaim semantics include stage/session-dependent handling and must remain unchanged. |
| `run_executor.rs::claim_and_start` | `claimed → running` after executor start confirmation | Project activity lock + conditional update on `status='claimed'` and matching executor session; Task transition, audit and Run event share transaction. |
| `run_context.rs::report_progress_impl` | MCP first-start `claimed → running` | Lease row is locked and validated first; branch requires claimed + no executor session. The final Run UPDATE currently predicates only on id after the locked precondition. |
| `run_executor.rs::start_failed` | `claimed/running → queued`; optionally `queued → failed` when infrastructure retry budget is exhausted | Conditional update exists for active states; retry count/failure evidence and events are transactionally recorded. Verify caller semantics and whether queued-to-failed is the only allowed exhaustion edge. |
| `run_dispatch/lease.rs::requeue_with_backoff` | active/eligible Run → `queued` | Helper UPDATE currently predicates on id only; callers and lock preconditions need explicit audit before changing it. |
| `run_dispatch/lease.rs::requeue_for_redispatch` | `claimed → queued` for probe release | Helper UPDATE currently predicates on id only; invoked inside claim transaction. Preserve probe semantics. |
| `run_dispatch/lease.rs::isolate_started_run` | started expired Run → `timed_out` | Helper UPDATE currently predicates on id only; caller must hold the expected Run/Task locks and prove the run is still active/expired. |
| `run_dispatch/lease.rs::cancel_run` | Run → `cancelled` | Helper UPDATE currently predicates on id only; audit callers and terminal protection before centralizing. |
| `run_executor.rs::record_termination` | `claimed/running → timed_out/cancelled/failed` based on reason | Conditional active-state update, terminal credential revocation, failure evidence, audit, event and outbox in one transaction. |
| `run_executor.rs::finish_run` | `claimed/running → succeeded/failed` from process outcome | Conditional active-state update; immutable failure evidence, continuation closure, audit, event and outbox share transaction. |
| `archive.rs::close_project` | queued Runs → `cancelled` | Bulk conditional update in the Project-close transaction; active Runs are handled later through the established cancellation protocol. |

## 3. Reviewer Run write-path inventory (separate lifecycle)

| Path | Current operation | Existing protection / concern |
|---|---|---|
| `review/queue.rs` | Insert new Reviewer Run as `queued` | Creation path; not a transition of an existing Run. |
| `review/worker.rs::claim_review_run` | Reviewer `queued → running` | Atomic UPDATE selecting a queued review Run with `FOR UPDATE SKIP LOCKED`. |
| `review/worker.rs` budget enforcement | Reviewer `running → timed_out` | Conditional `status='running'` update. |
| `review/worker.rs` successful pipeline | Reviewer `running → succeeded` | Conditional `status='running'` update and persisted verdict. |
| `review/worker.rs` failed pipeline | Reviewer `running → failed` | Conditional `status='running'` update and append-only validation failure evidence. |
| `review/worker.rs` retry | Insert a new queued Reviewer Run superseding the prior attempt | New Run creation; prior terminal Run remains historical. |

Reviewer Runs use platform execution and their own budget/retry pipeline. The delivery transition guard must not alter Reviewer Run queue selection, lease fields, session fields, or verdict semantics. Whether to share only the low-level terminal-protection primitive or maintain a dedicated reviewer guard is an implementation decision to resolve before code changes.

## 4. Existing database constraints

Baseline `migrations/0001_baseline.sql` defines:
- `task_run_status_check`: the seven status values listed above.
- `task_run_outcome_check`: the four terminal outcome values.
- `run_kind`: `delivery` or `review`.
- Reviewer isolation constraints requiring platform execution and excluding delivery lease/session state.

The schema currently constrains status vocabulary, not the allowed source-to-target transition matrix. Application code relies on conditional UPDATE predicates, row locks, and caller sequencing.

## 5. Initial transition matrix to validate against PA-07 semantics

This table is an implementation hypothesis to verify against all call sites and tests; it is not yet an approved replacement state machine.

| Run kind | From | To | Trigger |
|---|---|---|---|
| delivery | queued | claimed | initial claim |
| delivery | claimed | claimed | expired lease reclaim / same-run reacquire, with generation fencing |
| delivery | claimed | running | confirmed executor start or first valid MCP started report |
| delivery | claimed | queued | spawn/start failure retry or probe release, with distinct reason and counters |
| delivery | queued | failed | infrastructure retry exhaustion |
| delivery | claimed/running | timed_out | deadline/timeout, heartbeat/progress termination, or lease-loss isolation (reason-specific) |
| delivery | claimed/running | failed | execution termination or non-success process exit |
| delivery | claimed/running | cancelled | human cancellation / Project close |
| delivery | claimed/running | succeeded | successful process outcome / Agent delivery completion path |
| delivery | queued | cancelled | Project close before claim |
| review | queued | running | reviewer worker claim |
| review | running | succeeded/failed/timed_out | reviewer result, pipeline failure, or budget exhaustion |

No transition from a terminal state is allowed. Repeated delivery/command events must be idempotent and must not produce a second terminal write or duplicate terminal side effects.

## 6. PA08-03 implementation questions / gates

Before changing production SQL:
1. Enumerate every caller of `requeue_with_backoff`, `requeue_for_redispatch`, `isolate_started_run`, and `cancel_run`; establish the caller's lock and expected source status.
2. Inspect `run_executor::reconcile_orphans`, dispatch claim reclaim branches, archive cancellation, and Reviewer Run retry for their exact transaction/locking order.
3. Decide whether the guard is a transaction-scoped application service that receives a locked current row, or a narrower status-edge validator paired with conditional SQL. Do not perform a read-then-write without a row lock or compare-and-set predicate.
4. Keep the Run transition and Task transition separate. Existing Task updates continue through `apply_task_transition`.
5. Preserve Project activity locking for Project close vs executor start.
6. Test concurrent terminal writers, stale generation, stale token, duplicate events, and claim-vs-cancel using real PostgreSQL. Wiremock-only tests are insufficient for these cases.

## 7. Verified source files

- `crates/pm-application/src/run_dispatch/claim.rs`
- `crates/pm-application/src/run_dispatch/lease.rs`
- `crates/pm-application/src/run_executor.rs`
- `crates/pm-application/src/run_context.rs`
- `crates/pm-application/src/archive.rs`
- `crates/pm-application/src/review/queue.rs`
- `crates/pm-application/src/review/worker.rs`
- `migrations/0001_baseline.sql`

Static source inspection only. No claim is made that the current code has a reproduced race or that the proposed matrix has passed PostgreSQL concurrency tests.
