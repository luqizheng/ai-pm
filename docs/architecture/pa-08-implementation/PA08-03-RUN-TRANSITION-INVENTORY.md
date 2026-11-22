# PA08-03 — Run Transition Inventory

- **Status:** IMPLEMENTED (initial guard + PostgreSQL verification; final scoped update revalidation pending)
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
| `run_context.rs::report_progress_impl` | MCP first-start `claimed → running` | Uses the transaction-scoped guard and compare-and-set predicate on Run id, Project id, and `status='claimed'`. |
| `run_executor.rs::start_failed` | `claimed/running → queued`; optionally `queued → failed` when infrastructure retry budget is exhausted | Conditional update exists for active states; retry count/failure evidence and events are transactionally recorded. Verify caller semantics and whether queued-to-failed is the only allowed exhaustion edge. |
| `run_dispatch/lease.rs::requeue_with_backoff` | queued Run scheduling/backoff update (status remains `queued`) | Not a status edge; caller holds the candidate Run lock and UPDATE now requires `status='queued'`. |
| `run_dispatch/lease.rs::requeue_for_redispatch` | `claimed → queued` for probe release / unstarted reclaim | Helper itself locks and validates the edge, then compare-and-sets by Run id, Project id, and source status. |
| `run_dispatch/lease.rs::isolate_started_run` | started expired Run → `timed_out` | Shared `recover_lost_run` guard locks/validates the Delivery edge before the scoped active-status UPDATE. |
| `run_dispatch/lease.rs::cancel_run` | queued/claimed Delivery Run → `cancelled` | Helper itself locks/validates and compare-and-sets by Run id, Project id, and source status. |
| `run_executor.rs::record_termination` | `claimed/running → timed_out/cancelled/failed` based on reason | Conditional active-state update, terminal credential revocation, failure evidence, audit, event and outbox in one transaction. |
| `run_executor.rs::finish_run` | `claimed/running → succeeded/failed` from process outcome | Conditional active-state update; immutable failure evidence, continuation closure, audit, event and outbox share transaction. |
| `archive.rs::close_project` | queued Delivery/Reviewer Runs → `cancelled` | Selects and locks queued rows, validates each using its `run_kind` matrix, then performs the bulk update in the Project-close transaction; active Runs are handled by the established cancellation protocol. |

## 3. Reviewer Run write-path inventory (separate lifecycle)

| Path | Current operation | Existing protection / concern |
|---|---|---|
| `review/queue.rs` | Insert new Reviewer Run as `queued` | Creation path; not a transition of an existing Run. |
| `review/worker.rs::claim_review_run` | Reviewer `queued → running` | Atomic UPDATE selecting a queued review Run with `FOR UPDATE SKIP LOCKED`. |
| `review/worker.rs` budget enforcement | Reviewer `running → timed_out` | Transaction-scoped Reviewer guard + Project-scoped `status='running'` compare-and-set. |
| `review/worker.rs` successful pipeline | Reviewer `running → succeeded` | Locks and validates before persisting findings; Project-scoped compare-and-set must affect exactly one row before advancing. |
| `review/worker.rs` failed pipeline | Reviewer `running → failed` | Transaction-scoped Reviewer guard + Project-scoped `status='running'` compare-and-set; append-only validation failure evidence remains atomic. |
| `review/worker.rs` retry | Insert a new queued Reviewer Run superseding the prior attempt | New Run creation; prior terminal Run remains historical. |

Reviewer Runs use platform execution and their own budget/retry pipeline. `crates/pm-application/src/run_transition_guard.rs` now supplies separate Delivery and Reviewer edges. The shared low-level row-lock/transition validator is used by both lifecycles; Project archival is the only allowed active Reviewer cancellation reason. Reviewer lease/session fields, queue selection, and verdict semantics remain separate.

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
| review | running | cancelled | Project archival only; the shared termination path explicitly rejects other Reviewer cancellation reasons |

No transition from a terminal state is allowed. Repeated delivery/command events must be idempotent and must not produce a second terminal write or duplicate terminal side effects.

## 6. PA08-03 implementation and verification

Implemented in `crates/pm-application/src/run_transition_guard.rs`:
- Shared typed RunKind / RunStatus parser and separate Delivery / Reviewer transition matrices.
- Terminal states are immutable; same-state Delivery lease reacquisition is treated separately from status transitions.
- Transaction-scoped `SELECT ... FOR UPDATE` guard validates Run kind, Project scope, current status and target edge; callers retain compare-and-set UPDATE predicates in the same transaction.
- Guard integrated into claim, executor start/failure/termination/completion, MCP first-start, lease requeue/isolation/cancellation, Project archive, and Reviewer claim/success/failure/timeout paths.
- Reviewer active cancellation is permitted only for Project archival; other reasons are rejected.

Hosted evidence:
- [Run #52](https://github.com/luqizheng/ai-pm/actions/runs/44970075346) completed successfully: workspace check, Runner tests, PostgreSQL integration tests (concurrent terminal writers, immutable terminal, Delivery/Reviewer isolation), and guard unit tests.
- [Run #57](https://github.com/luqizheng/ai-pm/actions/runs/44970217926) completed successfully with the additional Project-archive cancellation integration test.
- Latest Project-scoped infrastructure-exhaustion UPDATE tightening is under validation in [Run #58](https://github.com/luqizheng/ai-pm/actions/runs/44970354176).

Remaining PA08-03 gates:
1. Complete latest Run #58.
2. Add/verify claim-vs-archive and stale lease-generation/token concurrency coverage with real PostgreSQL where the existing fixtures permit.
3. Audit every direct Run status UPDATE against this inventory and keep this file synchronized.

## 7. Verified source files

- `crates/pm-application/src/run_dispatch/claim.rs`
- `crates/pm-application/src/run_dispatch/lease.rs`
- `crates/pm-application/src/run_executor.rs`
- `crates/pm-application/src/run_context.rs`
- `crates/pm-application/src/archive.rs`
- `crates/pm-application/src/review/queue.rs`
- `crates/pm-application/src/review/worker.rs`
- `crates/pm-application/src/run_transition_guard.rs`
- `crates/pm-application/tests/run_transition_guard.rs`
- `migrations/0001_baseline.sql`

The verified implementation preserves Task transition routing through `apply_task_transition`; no PA-06 authorization or PA-07 state separation changes are intended. The latest Project-scope hardening remains pending Run #58.

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
