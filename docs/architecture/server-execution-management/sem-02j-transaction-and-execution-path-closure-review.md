# SEM-02J — Platform Transaction and Execution-Path Closure Audit

- Status: **Changes Required / Audit In Progress**.
- Source baseline: GitHub `master@d0e72fa6805aebeadf1b282e430deba493072fd6`.
- Related design: [SEM-02H](./sem-02h-ddl-integrity-and-transaction-review.md), [SEM-02I](./sem-02i-corrected-platform-schema-and-process-api-candidate.md).
- Scope: read-only source audit of Platform Run/Task/Interaction/Command/Recovery/Review paths.
- Change boundary: this audit changed no application code, migrations, API, WSS, Dockerfile, Compose or runtime configuration. Phase 0 remains Not Started and unauthorized.
- Important: line references below refer to the source baseline above; they are navigation hints, not stable API contracts.

## 1. Executive result

The source audit confirms that the existing code has multiple cross-aggregate lock-order inversions. The Interaction Run↔Interaction inversion already identified in SEM-02H is real at the SQL lock-acquisition level. The wider audit found additional Run↔Command and Run↔Task inversions.

The Task Execution Barrier cannot safely be implemented by adding a `SELECT ... FOR UPDATE` to only the new scanner. Existing Claim, Task cancellation, Interaction, recovery and review paths acquire the same rows in inconsistent orders. A single global lock graph and a complete dispatchability gate are P0 prerequisites.

No lock-order remediation is approved by this document. The final order must be selected only after the remaining source paths and foreign-key/trigger side effects are inventoried.

## 2. Confirmed lock-order inversion candidates

### P0-01 — Run ↔ Interaction

Observed paths:

- `interaction.rs::report_interaction` locks `delivery.task_run` first (`FOR UPDATE OF r`, around line 129), then locks the active `delivery.task_interaction` chain row (`FOR UPDATE`, around line 197).
- `interaction.rs::retry_interaction` locks the prior Interaction first (around line 783), then locks its Run (around line 816).
- `interaction.rs::cancel_open_interaction_tx` updates/locks the open Interaction first (around line 705), then updates the Run scheduling shield (around line 720).
- Continue ACK also follows Run → Interaction: `worker_command::ack_command` locks the command, updates the Run shield, then invokes continuation/Interaction recovery writes (around lines 365, 500 and 537).

A report/retry, report/cancel, or continue-ACK/cancel interleaving can form a wait cycle on the same Run and Interaction. This is not closed by the existing single-request idempotency tests.

### P0-02 — Run ↔ worker_command

Observed paths:

- `recovery::reacquire_lease` locks the Run first (`FOR UPDATE`, around line 189), then updates all non-terminal `delivery.worker_command` rows for that Run (around line 282).
- `worker_command::ack_command` locks the selected command first (`FOR UPDATE`, around line 365), then updates the Run scheduling shield for a continue ACK (around line 500), and may update Interaction/Continuation/Task state afterwards.

A concurrent ACK and same-Run reacquire can therefore hold Command→Run and Run→Command respectively. The existing ACK generation fence does not eliminate this database deadlock possibility.

### P0-03 — Run ↔ Task

Observed paths:

- `run_dispatch::claim_core_in_project` locks Project, then the queued Run (`FOR UPDATE OF r SKIP LOCKED`), then the Task (`FOR UPDATE`, around lines 417, 445/462 and 522). The source comment explicitly documents Project → Run → Task.
- Run transition/recovery paths commonly lock/update the Run before transitioning or updating the Task. Examples include `run_transition_guard::lock_and_validate_transition`, `run_executor::recover_lost` and the shared `recover_lost_run` path.
- Task cancellation first locks the Task row in `pm_write::update_task` or `command_executor::apply_task_transition` (around `pm_write/task.rs:217` and `command_executor/transition.rs:67`). The cancellation hook then enters `task_cancelled_linkage_tx` → `cancel_open_interaction_tx`, which updates Interaction and then the Run (around `interaction.rs:686-724`).

A Claim/Run-transition transaction can hold Run and wait for Task while a Task-cancellation transaction holds Task and later waits for the same Run. This is a cross-module inversion, not merely an Interaction-local issue.

### P0-04 — The current lock graph is not globally specified

The paths above are only the confirmed cycles found so far. Existing lock patterns include:

- Project → Run → Task: normal delivery Claim and Project archive.
- Project → Task → new Run insert: TaskReady handler and review enqueue.
- Run → Task: Run terminal/recovery transitions.
- Task → Interaction → Run: Task cancellation linkage.
- Run → Interaction → Task: report/continue lifecycle.
- Interaction → Run: retry/cancel.
- Command → Run → Interaction → Task: continue ACK.
- Run → Command: lease reacquisition.
- Consultation → Task waiting state: existing generic Human Consultation transition path.

These sequences cannot be combined safely by adding Barrier/HIR locks ad hoc. The final lock order must cover Project, Task, Run, Barrier, Interaction, worker_command, HIR and Human Consultation, including all multi-row Task operations and deterministic ordering when a Task has multiple Runs.


### P0-05 / P1 — Project dependency-graph advisory lock participates in the lock graph

The Task lifecycle has a transaction-scoped project graph lock in `task_dependency::acquire_project_graph_lock`. Its source contract explicitly says it must be acquired before any row lock.

- `pm_write::update_task` for terminal transitions acquires the graph advisory lock before locking the Task row (`pm_write/task.rs:200-222`).
- `command_executor::apply_task_transition` also acquires the graph lock for terminal transitions, before its Task `FOR UPDATE` (`command_executor/transition.rs:55-73`).
- However, some callers already hold another row lock before invoking that transition service. In particular, `review::run_review` locks and finalizes the Review Run, then calls `advance_after_review`; the approved auto-completion path calls `apply_task_transition(TaskStatus::Completed)`, which acquires the graph lock afterwards (`review/worker.rs` run-finalization path; `review/decide.rs:240-382`). This violates the graph-lock helper's documented “before any row lock” rule.

This is a lock-order contract violation that must be included in the full graph and tested against Task cancellation, dependency-edge mutation and other terminal transitions. The current evidence is sufficient to reject a claim that the global lock order is already consistent, but not sufficient to assert a specific deadlock cycle for every such pairing.

### P0-06 — Task cancellation extends the inversion beyond a single Interaction helper

Both Task cancellation entrypoints lock the Task before invoking `task_cancelled_linkage_tx`:
- REST direct update: `pm_write::update_task` locks Task at `pm_write/task.rs:217`, updates it, then invokes the cancellation linkage.
- Command channel: `command_executor::update_task_status` calls `apply_task_transition` (Task lock) and then invokes the same linkage.

The linkage then takes Interaction → Run locks. This gives Task → Interaction → Run, while delivery Claim and several recovery paths take Run → Task. The Task-level cancellation race therefore remains even if report/retry are individually reordered.

## 3. Execution Barrier bypasses found in existing paths

The proposed Task Barrier must be checked at both Run creation and every dispatch/ownership transition. Current source paths do not yet have this gate.

| Path | Current source behavior | Required SEM contract |
|---|---|---|
| Delivery Claim | Locks Project → Run → Task; no Task Barrier check exists | Under the canonical Task/Run lock order, atomically check active Barrier and Interaction shield before Claim/Generation |
| TaskReady handler | Locks Project → Task, checks active delivery Run, then calls `insert_task_run`; no Barrier check | Must refuse/record suppression while an active Task Barrier exists |
| Review enqueue | Locks Project → Task, then creates a review Run; no Barrier check | Define whether Barrier blocks review Run creation; V1 should not allow a different Run kind to bypass the Task-level safety fence |
| Review Claim | `review::worker::claim_review_run` locks only the queued review Run; it does not lock Project/Task or check Barrier | Must use the same Project/Task dispatch gate; no special review bypass |
| Review retry | `review::worker::schedule_retry` directly inserts a new review Run without locking Project/Task or checking Barrier | Must be serialized with Project archive and Task Barrier |
| Delivery retry after failure | `review::failure` updates Task and calls `insert_task_run`; it has a Task lock but the reviewed path does not show a Project activity gate or Barrier check | Add Project activity and Barrier checks in the same transaction |
| Same-Run reacquire | `recovery::reacquire_lease` locks Run and re-signs generation/token, then reroutes commands; no Task Barrier check | Reacquire must not restore execution authority through an active Barrier |
| Orphan reconciliation | Existing `reconcile_orphans` requeues an expired Run and changes Task state without a Task Barrier/HIR transaction | Replace with the new Platform-only scanner; never reuse the old requeue behavior |

The review-specific rows matter because `delivery.task_run` stores both `delivery` and `review` Run kinds. A Task-level Barrier cannot be considered complete if the review queue/worker has a separate dispatch path.

## 4. Run creation inventory (current code)

The source search identified these business creation paths:

1. Delivery TaskReady: `pm-infrastructure/src/outbox_handlers.rs` → `run_dispatch::insert_task_run`.
2. Delivery retry/recovery: `run_dispatch::lease::recover_lost_run` → `insert_task_run`.
3. Delivery retry after failure: `review::failure` → `insert_task_run`.
4. Review enqueue: `review::queue::insert_review_run`.
5. Review retry: `review::worker::schedule_retry` direct `INSERT INTO delivery.task_run`.

The shared delivery helper documents a Task-row-lock precondition (`run_dispatch/port.rs:8`). Review enqueue currently locks Project then Task before insertion. Review retry is a notable exception: the inspected transaction reads the failed Run and inserts a new review Run without acquiring the Task row lock. It also does not acquire the Project activity gate in that transaction.

All five paths must be included in the final Barrier/Project-gate matrix. Any additional direct INSERT found in the remaining repository scan must be added before this inventory is declared exhaustive.

## 5. Project lifecycle and dispatch observations

- Normal delivery Claim uses the Project activity row lock before Run Claim.
- TaskReady and review enqueue also use the Project activity gate.
- Review Claim currently selects/locks a review Run without the Project gate. This can race Project archive: a review worker may claim a queued review Run while archive is changing the Project and cancelling queued Runs.
- Review retry creates a new review Run without the Project gate, so it can create queued work after Project closure unless another invariant blocks it.
- The old orphan reconciliation path has no Project activity lock. Its queued requeue can race archive; the new scanner must explicitly serialize with Project lifecycle.
- Project archive itself locks Project before selecting and cancelling queued Runs. Any path that creates or claims a Run must participate in this same Project gate.

## 6. Existing test evidence and missing concurrency coverage

Existing tests establish useful but narrower properties:

- `run_dispatch::concurrent_claims_yield_exactly_one_claimed_run`: competing delivery claims serialize correctly for one queued Run.
- `run_executor::c66_concurrent_finalization_wins_over_poll`: a Run finalization/poll race is covered.
- ACP tests cover Interaction idempotency, command ACK validation, and repeated continue ACK behavior.

The inspected ACP test file contains many sequential report/retry/ACK assertions, but the targeted search did not find deterministic two-connection tests for:

- report ↔ retry;
- report ↔ Task cancellation;
- continue ACK ↔ Task cancellation;
- ACK ↔ reacquire;
- Claim ↔ Task cancellation;
- Barrier scanner ↔ Claim/TaskReady/retry/review Claim;
- review Claim ↔ Project archive.

Do not treat single-call idempotency tests or random `tokio::join!` tests as proof of deadlock freedom. Required tests must use two independent PostgreSQL connections, explicit transaction barriers, bounded lock/statement timeouts and assert both final database state and absence of partial audit/notification effects.

## 7. Required next audit passes

Before deciding the final lock order, complete these read-only passes:

1. Enumerate every `delivery.task_run` INSERT and every state/lease/generation UPDATE, including delivery submit, review completion/failure, run context, cleanup, project archive and outbox handlers.
2. Enumerate every path that locks or updates `delivery.task`, `task_interaction`, `worker_command`, `continuation`, `human_intervention`, `human_consultation`, Barrier candidates and notification rows.
3. Inspect SQL foreign keys, triggers, cascade behavior and unique-index conflict paths that may acquire implicit row/key locks.
4. Inspect all API/MCP entrypoints and background loops to prove no alternate claim, requeue, continue, terminate, ACK or recovery route bypasses the application transaction contract.
5. Decide whether review Runs are blocked by Task Barrier (recommended: yes) and whether E6 successor authorization applies only to a delivery Run or another explicitly typed successor.
6. Select one global lock order and deterministic ordering for multiple Runs/Interactions per Task; then map every transaction to it.
7. Only after the source audit is complete, author the DDL/API/transaction changes and deterministic concurrency tests.

## 8. Required deterministic concurrency acceptance matrix

| ID | Transaction A | Transaction B | Required result |
|---|---|---|---|
| J01 | Report Interaction | Retry/cancel same Interaction | No deadlock; exactly one valid lifecycle outcome |
| J02 | Continue ACK | Task cancellation | No deadlock; terminal Task cannot be reopened by late ACK |
| J03 | ACK command | Same-Run lease reacquire | No deadlock; current generation fences stale ACK |
| J04 | Delivery Claim | Task cancellation | Exactly one serialized outcome; no active Run on cancelled Task |
| J05 | Periodic scanner | Delivery Claim | Either Claim wins before expiry/Barrier eligibility, or Barrier+HIR+Inbox+Audit commit before any later Claim |
| J06 | Periodic scanner | TaskReady/retry/review enqueue | No Run creation can escape the Task Barrier |
| J07 | Periodic scanner | Review Claim/retry | Review execution cannot bypass Task Barrier |
| J08 | Project archive | Review Claim/retry and orphan scan | No new/claimed Run after Project closure |
| J09 | E6 decision | Competing E6 decision/successor allocation | Exactly one consultation and one successor authorization |
| J10 | Continue new process | Late exit from prior process sequence | Old process evidence cannot clear the active process fence or release Barrier |

Each test must assert transaction completion, lock timeout behavior, unique constraints, Run/Task/Barrier/Interaction/Command state, and audit/outbox/inbox row counts.

## 9. Current decision

**Changes Required.** The Task Barrier design is directionally correct but not safe to implement until the complete lock graph, all Run creation/claim/reacquire paths, Project lifecycle gates and review execution paths are closed.

- No migration authoring.
- No business-code changes.
- No Phase 0 start.
- PR #9 remains Draft and must not be merged.

