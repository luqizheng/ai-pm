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



## 9. Second audit pass — Run writes, FK/index/trigger side effects

Source baseline remains `master@d0e72fa6805aebeadf1b282e430deba493072fd6`. This pass inspected the concrete Run creation/retry implementations and the checked-in schema/inventory evidence. It remains read-only.

### 9.1 Run INSERT paths — expanded findings

The application source confirms the five business creation paths listed above. Their transaction properties are not equivalent:

| Path | Source behavior | Lock/gate result |
|---|---|---|
| Delivery TaskReady | `run_dispatch::insert_task_run`; shared helper documents that caller must already hold Task `FOR UPDATE` | Helper itself does not acquire Project gate or Task Barrier; caller must supply both |
| Delivery lease-loss recovery | `recover_lost_run` → terminalize old Run → `insert_task_run` | Caller precondition is Task row lock; Run transition happens before new Run insert; Barrier/gate still must be supplied by outer transaction |
| Review enqueue | `review::queue::enqueue_review` → Project activity gate → Task `FOR UPDATE` → `insert_review_run` | Project gate and Task lock exist; Task Barrier check is absent |
| Review retry | `review::worker::schedule_retry` directly inserts `delivery.task_run` | **No Project activity gate, no Task row lock, no Task Barrier check**; reads max review attempt and current Task revision without locking Task |
| Delivery retry after failure | `review::failure::judge_delivery_failures` → `insert_task_run` | The retry transaction updates Task before INSERT, which obtains a Task row lock, but this code path has no Project activity gate or Task Barrier check |

Review retry has an additional concurrency defect candidate: two workers can read the same `max(attempt_no)+1` before either inserts. The partial unique index `uq_task_run_attempt_review` rejects the duplicate, while `uq_task_review_active` only permits one queued/running Review Run. These constraints are useful last-line integrity guards; they do not replace the missing Task lock, Project gate or Barrier check. A deterministic two-connection retry/retry test is required.

The failure-judge path needs a precise lock-order interpretation: it performs a conditional Run `UPDATE finished_at` as a standalone pool statement, so that Run row lock is released before its later transaction updates Task and inserts a replacement Run. The inspected code therefore does **not**, by itself, prove a simultaneous Run→Task lock hold in that specific retry transaction. It still lacks Project activity and Barrier checks, and the scan/claim/decision sequence must be tested against duplicate workers and Project closure. Do not overstate this path as another proven Run→Task deadlock.

### 9.2 Run UPDATE and ownership-transition paths

The inspected source confirms the following distinct classes of writes:

- Delivery claim/lease issuance and same-Run reacquire update lease owner, token hash, expiry and generation.
- Recovery can terminalize an old Run and create a successor in the same transaction.
- Dispatch qualification can requeue an eligible queued Run with backoff; redispatch also performs a guarded same-row queued update.
- Run executor and cancellation/finalization paths update status/outcome/finished timestamps and record Run events.
- Review worker paths update Review Run status through separate Review-only transitions: queued→running, running→succeeded/failed/timed_out. Review retry creates a successor Review Run.
- Project archive cancels queued Runs in its Project-close transaction; active Run cancellation is then handled by the follow-up cancellation protocol.
- Run cleanup updates `task_run_cleanup`, not the authoritative Run lease/status. Cleanup state must not be treated as process-exit evidence.

This is a source-path inventory, not yet a claim that every SQL statement in every module has been exhaustively enumerated. The remaining code-search pass must continue across all Run state columns and all API/MCP/background entrypoints before the inventory can be marked closed.

### 9.3 Foreign-key side effects

The checked-in schema evidence establishes that `delivery.task_run` is not an isolated row:

- Parent references include `(tenant_id, project_id) → core.project`, `(project_id, task_id) → delivery.task`, device identity through `(tenant_id, lease_client_id)`, and self-reference through `supersedes_run_id`.
- Child references include Interaction, Continuation, interaction responses/deliveries, checkpoints, worker commands, failure records, Run repository snapshots, cleanup, execution authorization, review findings/verdicts/approvals and repository commits.
- Several child references use `ON DELETE CASCADE` (for example cleanup, Run repository snapshot, review finding/verdict/approval rows); others use RESTRICT/NO ACTION or the default behavior. Run deletion is therefore not a harmless cleanup operation and must remain outside the execution-recovery protocol.
- In PostgreSQL, FK validation and unique-index conflict handling can acquire implicit key-share/index locks. The transaction graph must account for these effects, especially when a path inserts a child row while another path updates/deletes its parent, or when two paths concurrently allocate the same per-Task attempt number.

The FK/index inventory also shows why a Task-level Barrier must be enforced before creating either Run kind: both delivery and review Runs share the same Task parent and the same `delivery.task_run` identity space.

### 9.4 Unique indexes and triggers

The checked-in index inventory records these important guards:

- `uq_task_run_active`: at most one active Delivery Run per Task for queued/claimed/running.
- `uq_task_review_active`: at most one active Review Run per Task for queued/running.
- `uq_task_run_attempt_delivery` and `uq_task_run_attempt_review`: per-kind attempt uniqueness.
- `uq_task_run_source_event`: one Run per non-NULL source event.
- `uq_task_run_project_id` / `uq_task_run_tenant_id`: composite FK targets.

These indexes serialize some conflicting inserts or reject duplicates, but none encodes “Task has an unreleased Execution Barrier”; they cannot be used as a substitute for the Platform transaction predicate.

The inspected trigger inventory shows:
- `trg_review_report_immutable` on `delivery.task_run` (BEFORE UPDATE) protects Review report/anchor immutability.
- `trg_task_run_event_guard` protects the append-only Run event table.
- `task_run_failure_immutable` protects failure evidence against UPDATE/DELETE.

No inspected trigger was found to acquire Project, Task, Run, Interaction or Command locks as a secondary side effect. The Review report trigger runs on the already-targeted Run row; the event/failure triggers target their own row. This is limited to the inspected trigger inventory and must be rechecked against the final migration chain, not generalized to every database trigger.

### 9.5 Updated conclusions

1. Review retry is a confirmed direct Run-creation bypass: no Project gate, no Task row lock and no Task Barrier.
2. Delivery retry after failure has a Task write/row lock but still lacks the Project activity gate and Barrier check.
3. Review Claim and stuck-Review recovery still operate without the Project activity gate; Project archive pairing tests remain required.
4. Unique indexes are integrity backstops, not dispatch authorization. They do not close Barrier bypasses.
5. FK validation introduces implicit parent-key locks and must be included in the final lock graph. The current evidence does not justify declaring a complete global lock order.
6. The exact failure-judge sequence must not be mislabeled as a simultaneous Run→Task lock inversion: its standalone Run update commits/releases before the later Task transaction.

### 9.6 Remaining audit work

- Finish exhaustive code search for every Run INSERT and every write to `status`, `lease_generation`, `lease_token_hash`, `lease_owner`, `lease_expires_at`, `scheduling_shield`, `executor_session`, `supersedes_run_id` and process-sequence fields.
- Finish the full Task/Interaction/Continuation/Command/HIR/Consultation/Barrier/Notification read-write route inventory.
- Reconcile migration DDL from the current migration head against the 2026-11-15 schema inventory; the inventory is evidence, not a substitute for replaying the current migration chain.
- Add deterministic tests for Review retry vs Review retry, Review retry vs Project archive, Review Claim vs Project archive, Delivery retry vs Project archive, and FK/unique-conflict wait behavior.
- Only after those passes, derive the canonical lock order and decide whether some writes need a shared transaction helper or a single dispatchability predicate.

Status remains **Changes Required / Audit In Progress**. No migration, business code, API/WSS, Dockerfile, Compose, Worker/Runner loop or runtime configuration was changed. Phase 0 remains Not Started; PR #9 must not be merged.


## 10. Third audit pass — lease business writes, legacy control entrypoints and governance carriers

This pass traced the production application call sites for lease renewal/progress, delivery submission, Runner-facing claim/recovery/command operations, Human Intervention, Human Consultation and notification feed. It remains a static source audit against the same master baseline.

### 10.1 Additional Run → Task lock edges

The first lock graph omitted several ordinary business paths that also hold a Run lock before transitioning Task:

| Path | Observed order | Consequence |
|---|---|---|
| MCP `report_progress` first-start path | `lock_lease` locks Run `FOR UPDATE` → `apply_task_transition(Running)` locks Task | Run → Task |
| MCP `submit_result` | delivery pipeline performs `lock_lease` fencing → accepted submission transitions Task to `submitted` in the same transaction | Run → Task |
| Platform Worker spawn confirmation | Project activity gate → Run transition lock/update → `apply_task_transition(Running)` locks Task | Project → Run → Task |
| Review success/advance | Review Run is locked/finalized → `advance_after_review` may transition Task (including Completed) | Review Run → Task; terminal path may then acquire graph advisory lock |

These are additional concrete Run→Task edges. The Task cancellation path remains Task→Interaction→Run. The report-progress and delivery-submit paths are not merely recovery edge cases; they are normal execution business writes and must be included in the canonical transaction design.

The Project gate in spawn confirmation is important for Project archive serialization, but it does not by itself solve the graph-lock contract: a terminal Task transition that acquires the graph advisory lock after the Run row has already been locked still violates the graph helper's “before any row lock” precondition.

### 10.2 Existing Runner/Worker control-plane entrypoints

The current code still contains the pre-SEM control topology:

- MCP `claim_task_run_as_client` is called from `crates/pm-mcp/src/tools.rs` and exposed by the MCP server. It lets the authenticated Client claim a Run directly from Platform.
- MCP `reacquire_lease` is exposed by the MCP server and calls `recovery::reacquire_lease` directly.
- MCP `poll_commands` / `ack_command` expose command delivery and ACK directly to the Runner.
- The Platform Worker also calls `run_executor::claim_and_start` / `claim_once` and owns local executor polling/recovery paths.

These are existing code paths, not the target SEM topology. Under the confirmed SEM architecture, Runner control must move behind Runner↔SEM WSS and SEM↔Platform HTTPS; SEM alone requests Claim, while Platform remains the Run/Lease/Generation authority. The migration must explicitly remove or fail-closed the old Runner-facing MCP Claim/reacquire/command poll/ACK routes and prevent the old Platform Worker executor loop from acting as a second dispatch authority. Do not remove Harness business MCP operations such as run context, progress, interaction reporting and result submission merely because they share the MCP transport.

The control-plane cutover needs negative tests proving:
- a Runner device credential cannot call legacy MCP Claim/reacquire/poll/ACK routes;
- only the SEM service identity can invoke the dedicated Platform Claim/command/renewal APIs;
- Harness MCP business operations remain available under their separate user/device and Run-lease scopes;
- the Platform Worker cannot independently claim or dispatch after SEM becomes authoritative.

### 10.3 Existing governance carrier audit

#### `governance.human_intervention`

Current application evidence shows:
- The production writer is `supervision::feed::record_intervention`, called by supervision veto/freeze operations.
- Its current intervention type CHECK is limited to seven action values: `veto`, `rollback`, `freeze`, `unfreeze`, `mode_switch`, `policy_change`, `rollback_failed`.
- The current row shape is an action record (actor, reason, optional proposal/command target, executed_at); it is not an open abnormal-execution work item with lifecycle/status, Run/Task/Barrier anchors, ownership, escalation and resolution evidence.
- The inspected production code does not expose a generic UPDATE/DELETE lifecycle for these rows; they are inserted as intervention-action history. Current readers found by the code search are primarily test/security snapshots, while the supervision feed reads `integration.notification`.

Therefore the chosen “extend `human_intervention`” direction is a schema/domain remodel, not a simple additive HIR status column. A discriminator must preserve the seven action values and all existing readers/filters. HIR-specific columns, status transitions and constraints must be scoped to the HIR record type; action history must not accidentally appear as an open HIR or vice versa. Exact DDL and route/read-model changes remain open.

#### `governance.human_consultation`

Current production operations are in `human_consultation.rs` and the PM API:
- create inserts a pending consultation and, when task-bound, inserts `delivery.task_waiting(reason='needs_decision')` in the same transaction;
- start/decide/cancel/supersede lock the Consultation row; decide/cancel end the corresponding Task waiting row;
- readers include the API list and Task traceability projection.

These operations lock Consultation before changing Task waiting state; they do not acquire the Project activity gate or Task row lock. The current table is a generic formal decision work item, but it does not have the proposed unique HIR/Barrier/successor binding or E6 authorization semantics. The E6 extension must be explicit and typed; generic Consultation creation/decision must not silently release an Execution Barrier or allocate a successor Run.

#### `integration.notification`

Current production writers include supervision feed, AI orchestration and outbox handlers; the supervision feed reader is project-scoped and cursor-based. The current table is a project Feed, with no per-principal recipient/inbox ownership model. It must remain non-authoritative: notification insertion or read state cannot prove process exit, Barrier release or E6 approval. The proposed recipient child table and its unique delivery/idempotency keys need their own transaction and reader/writer audit.

### 10.4 Task waiting is not an Execution Barrier

Current dispatch policy and dependency code read active `delivery.task_waiting` rows as a Task eligibility gate. Consultation, Interaction and Review paths write/end waiting rows with their own semantics. This does not provide the required Task Execution Barrier:

- waiting rows do not bind a specific old Run + Generation + Process Sequence;
- they do not encode E1–E6 evidence class or HIR lifecycle;
- their current readers do not enforce all new Run creation, Review Claim, same-Run reacquire and successor-allocation paths;
- generic consultation or interaction resolution is not equivalent to verified process exit.

Keep Task waiting, Interaction scheduling shield and Execution Barrier as distinct mechanisms. The final Platform dispatchability predicate must evaluate all applicable gates in one transaction rather than treating any one of them as a replacement.

### 10.5 New P0 closure items

1. Add normal Run→Task business writes (report_progress, submit_result, Worker start confirmation and Review completion) to the global lock graph and J01/J02/J04-style deterministic tests.
2. Make SEM the only control-plane entrypoint at cutover; explicitly disable the old Runner MCP Claim/reacquire/command poll/ACK paths and the old Worker dispatch authority, while retaining Harness business MCP.
3. Remodel `human_intervention` with a discriminator and preserve its existing seven intervention-action values and readers; do not conflate action history with HIR lifecycle.
4. Extend `human_consultation` only with typed E6 semantics and unique anchors; generic consultation status must not release Barrier or authorize a successor.
5. Add the Notification Recipient/Inbox lifecycle without changing the existing project Feed semantics or treating notification as safety evidence.
6. Prove that Task waiting, Interaction shield and Execution Barrier are independent gates evaluated by one Platform transaction predicate.

Status remains **Changes Required / Audit In Progress**. The global lock order remains deliberately unfrozen. No code, migration, API/WSS implementation or runtime configuration was changed; Phase 0 remains Not Started.


## 11. Fourth audit pass — Interaction/Continuation/Command transaction paths

This pass traced the concrete Interaction response/consume/retry/cancel paths, Continuation lifecycle hooks, lease progress/renewal and Worker Command ACK. It remains static source review; no implementation was changed.

### 11.1 Confirmed lock sequences

| Transaction path | Observed order | Notes |
|---|---|---|
| Interaction report | Run `FOR UPDATE` → Interaction lookup/insert → Run scheduling-shield UPDATE → Task transition (`FOR UPDATE`) → TaskWaiting insert → Command insert | Run→Task is an additional normal path; interaction row is newly inserted, not an existing conflicting lock |
| Interaction retry | Interaction `FOR UPDATE` → Run lookup/lock/update → TaskWaiting / new Interaction / command writes | Confirms the previously identified Interaction→Run edge |
| Interaction response persisted | Interaction UPDATE (row lock) → Run scheduling-shield UPDATE | Interaction→Run |
| Interaction consumed | ResponseDelivery update → Continuation update → Interaction UPDATE → Run shield UPDATE → TaskWaiting resolve → Task `FOR UPDATE` / transition | A multi-aggregate chain; delivery and continuation FKs add implicit parent-key checks |
| Continue Command ACK | Command `FOR UPDATE` → Run generation read/update scheduling shield → ResponseDelivery update → Continuation started update → Interaction chain update → TaskWaiting resolve → Task `FOR UPDATE` / transition | Command→Run→Interaction/Continuation→Task; the Run generation read is not itself a row lock, but shield UPDATE is |
| Terminate Command ACK/failure | Command `FOR UPDATE` → Run/cleanup updates → Continuation freeze → Task `FOR UPDATE` / transition | Command→Run/Continuation→Task |
| Task cancellation linkage | Task transition obtains Task `FOR UPDATE` → Interaction chain updates/cancel → Run shield update / command writes → Continuation close | Task→Interaction→Run/Continuation |
| Run terminalization | Run terminal UPDATE → Continuation close-by-Run → cleanup enqueue and event/audit writes | Run→Continuation |
| Lease renewal | Run `FOR UPDATE` via `lock_lease` → Run lease UPDATE → Task plain SELECT | Task SELECT is MVCC read, not a Task row lock |
| Progress first-start | Run `FOR UPDATE` → Run lease/progress UPDATE → Task transition (`FOR UPDATE`) | Run→Task; normal business path, not only recovery |

### 11.2 Important corrections and additional conflict edges

1. The earlier lock inversion is broader than report-vs-retry: response-persisted is Interaction→Run, while report/first-start is Run→Interaction/Task. ACK and cancellation introduce Command and Continuation into the same connected graph.
2. The Task cancellation path's order is Task→Interaction→Run/Continuation. Any path holding Run or Command and then waiting for Task can conflict with cancellation if the reverse edge is held concurrently. This is a concrete candidate cycle family; deterministic pair tests must establish each reachable interleaving.
3. `mark_consumed` updates ResponseDelivery and Continuation before Interaction and Task. It can race with continue ACK, which updates Command, Run shield, ResponseDelivery, Continuation, Interaction and Task. Both are intended to be idempotent at individual state predicates, but predicate idempotency is not proof of deadlock freedom.
4. `open_continuation_tx` allocates `MAX(generation)+1` then inserts. Its normal caller currently holds the Run row lock through Interaction report, which serializes same-Run creation; any future caller must preserve that serialization or replace it with a DB uniqueness/sequence allocation protocol.
5. `close_open_by_task_tx` updates all open/started/frozen Continuations for a Task. It is called from Task cancellation after Task row locking; `close_open_by_run_tx` is called after Run terminalization. Their ordering is Task→Continuation versus Run→Continuation and must be covered in the final graph.
6. The cleanup ledger is deliberately separate from Run authority. `claim_batch` locks only cleanup rows with SKIP LOCKED; `mark_completed/failed` update cleanup rows only. Cleanup completion must never be interpreted as authoritative E3 process-exit evidence unless the new process-evidence contract explicitly binds the verified process identity and generation.

### 11.3 Lock graph is connected; no global order can yet be selected

The current evidence establishes at least these directed edges:

- Project → Run → Task (archive/Worker start confirmation/Run claim paths)
- Run → Interaction (report/progress-related paths and shield updates)
- Interaction → Run (retry/response/cancel paths)
- Task → Interaction → Run (Task cancellation linkage)
- Command → Run → Interaction/Continuation → Task (ACK paths)
- Run → Continuation (Run terminalization)
- Task → Continuation (Task cancellation)
- Task graph advisory lock → Task row (terminal Task transitions and dependency graph mutation)

The Project graph advisory lock has a strict pre-row-lock requirement. Existing Review completion can still reach Task transition after locking Review Run, so the advisory-lock contract violation remains open. The graph must additionally include parent-key locks from FK validation and unique-index conflict waits.

Do not “solve” this by documenting a preferred order without changing call boundaries. A viable redesign likely needs a shared transaction entry protocol that resolves immutable scope IDs without row locks, acquires advisory locks in a deterministic aggregate order, then row-locks Project/Task/Run/Interaction/Command/Continuation in one canonical order. The exact order must be derived from all paths and tested; this paragraph is a design direction, not a selected lock order.

### 11.4 New deterministic concurrency cases

Add these to the existing J01–J10 matrix:

- J11 report_interaction vs retry_interaction and response_persisted.
- J12 continue ACK vs mark_consumed (both race for response delivery/continuation/interaction/task effects).
- J13 terminate ACK failure/freeze vs Task cancellation.
- J14 Run terminalization/continuation close vs Task cancellation/close-by-task.
- J15 lease progress first-start vs Task cancellation.
- J16 Worker spawn confirmation vs Task cancellation and Project archive.
- J17 same-Run concurrent open_continuation attempts; assert one generation allocation and one continuation row per intended event.
- J18 cleanup claim/complete vs Run terminalization and new process-evidence ingestion; assert cleanup state never upgrades to authoritative exit evidence.

Every case uses two independent PostgreSQL connections, explicit barriers, bounded lock/statement timeouts, and verifies final aggregate states plus audit/command/delivery/continuation/event effects.

Status remains **Changes Required / Audit In Progress**. This pass expands the connected lock graph but does not select a global lock order. No code, migration, API/WSS implementation or runtime configuration changed; Phase 0 remains Not Started.


## 12. Fifth audit pass — current migration head and late execution-related DDL

The current `master` migration directory contains migrations through **0059** (latest observed: `0059_discovery_round_error.sql`). This pass checked the late migrations that can affect execution ownership, process cleanup, workspace lifecycle or formal human decisions. It does not claim a full replay/semantic review of every migration from 0001 through 0059; the complete chain reconciliation remains open.

### 12.1 Relevant late migration findings

| Migration | DDL effect | SEM relevance |
|---|---|---|
| 0046 `human_consultation` | Creates generic four-state Consultation; task FK is `ON DELETE SET NULL`; no composite tenant/project/task FK; no HIR/Barrier/successor unique anchors | E6 cannot be inferred from this table's generic `decided` state; E6 needs typed binding and same-scope FK integrity |
| 0053 `device_single_active_run` | Partial unique index `uq_task_run_device_active` on `lease_client_id` for claimed/running | Enforces one active Run per device identity, not one process per Runner host or SEM connection; new SEM Runner slot/concurrency authority must preserve and deliberately replace/extend this invariant |
| 0054 `task_run_cleanup_claim_fencing` | Adds cleanup claim token and due index | Cleanup worker claims are fenced independently; no Platform Run lease/generation authority is transferred |
| 0055 `workspace_reclamation_ledger` | Adds separate workspace reclamation ledger with Project/Repository/RunRepository FKs and unique resource keys | Filesystem reclamation is separate from process termination and Run state; FK restricts deletion while resource ledger exists |
| 0056 `workspace_reclamation_bytes` | Adds logical reclaimed byte count | Metrics/accounting only; no execution authority |
| 0057 `workspace_reclamation_totals` | Adds cumulative resource-kind totals | Metrics/accounting only; no execution authority |
| 0058 `external_commit_adoption` | Adds external Commit discovery/adoption tables and repository scan anchors | No direct Run ownership change identified in this DDL |
| 0059 `discovery_round_error` | Adds discovery session error fields | No direct Run ownership change identified in this DDL |

### 12.2 Migration-level integrity observations

1. `human_consultation` has a task FK with `ON DELETE SET NULL`, but no composite FK that proves the Consultation's tenant/project/task all belong to the same scope. This is a schema integrity gap for future E6 use, not a demonstrated current cross-tenant exploit; existing service authorization and data model need separate audit.
2. The device-active partial unique index is a valuable DB backstop, but its key is `lease_client_id` and it only covers claimed/running. It does not model SEM's Runner connection epoch, host identity, supervisor session, resource reservation or dispatch operation. SEM must not equate device uniqueness with Runner process uniqueness.
3. Workspace reclamation is intentionally derived-resource bookkeeping. Its completion cannot prove process exit; the process may have exited before cleanup, or cleanup may fail after exit. Conversely, cleanup completion is not a substitute for a process-specific, generation-fenced E3 receipt.
4. The late migrations 0055–0057 introduce separate filesystem reclamation state and counters. Do not merge these into Run status or the proposed process-evidence ledger.
5. The current migration head is 0059, whereas the existing schema inventory snapshot is dated 2026-11-15 and predates at least the PA-08 migrations. It is not sufficient evidence for current schema constraints/triggers. Rebuild a fresh schema inventory from the exact current migration head before final DDL approval.

### 12.3 Remaining migration audit

- Inspect baseline DDL sections for `task_run`, `task_interaction`, `worker_command`, `continuation`, `interaction_response_delivery`, `human_intervention`, `task_run_cleanup`, their unique indexes, FKs and triggers.
- Inspect every intervening migration 0002–0059 for DROP/ADD/RENAME of those constraints and for trigger functions that may introduce additional writes.
- Replay the full migration chain against an empty PostgreSQL database and capture `pg_constraint`, `pg_index`, `pg_trigger`, `pg_proc` definitions at the resulting head.
- Compare that live-schema capture with the checked-in migration-derived expectation. The 2026-11-15 inventory must remain historical evidence only.
- Add DDL tests for the composite tenant/project/task identity guarantees required by HIR, Barrier, E6 and process evidence.

Status remains **Changes Required / Audit In Progress**. The latest migration head is identified as 0059, but the full 0001–0059 schema/trigger reconciliation is not yet complete. No global lock order is selected; no code, migration, API/WSS implementation or runtime configuration changed; Phase 0 remains Not Started.


## 13. Sixth audit pass — migration baseline provenance and exact late DDL verification

This pass re-opened the authoritative migration ledger and the exact checked-in DDL files on the current master baseline. It corrects an important interpretation of the migration chain:

- `migrations/0001_baseline.sql` is **not** the original historical 0001 migration. It is a 2026-11-09 squash artifact produced from the old 0001–0065 chain (64 files; historical 0029 was empty), using the terminal database state via `pg_dump --schema-only + --data-only --inserts`.
- The migration ledger records a zero-diff equivalence check between a database built from the old chain and one built from the squashed baseline. New incremental migrations restart at 0002.
- Therefore, the current replay target is the squashed `0001_baseline.sql` followed by the post-baseline migrations through 0059. A replay of the old 0001–0065 history is not the current sqlx chain and must not be described as such.
- The baseline contains both schema DDL and seed/data INSERT statements. A fresh-database replay must account for that data as part of the migration contract, not treat 0001 as schema-only.

### 13.1 Exact late DDL rechecked

The following files were fetched from current master and their DDL inspected directly:

| Migration | Verified exact effect | Lock / ownership implication |
|---|---|---|
| 0053 `device_single_active_run` | Partial unique index on `delivery.task_run(lease_client_id)` where status is claimed/running and lease_client_id is not null | This is a device-identity uniqueness backstop, not a SEM Runner process/connection-epoch or resource-reservation model |
| 0054 `task_run_cleanup_claim_fencing` | Adds nullable `claim_token uuid` to `delivery.task_run_cleanup`; adds due index over `(next_retry_at, run_id)` for pending/cleaning | DDL alone does not implement atomic claim semantics; the application CTE must set lease expiry and token in one statement, and all completion/failure writes must fence on that token |
| 0055 `workspace_reclamation_ledger` | Creates `delivery.workspace_reclamation`; includes Project/Repository/RunRepository foreign keys, resource-shape/status/path/attempt checks, partial unique indexes for Run worktree and Repository mirror, and due index | Derived filesystem-resource lifecycle only. Its foreign keys can participate in key-share/conflict locking; exact transaction paths must be included in the lock/implicit-lock inventory |
| 0056 `workspace_reclamation_bytes` | Adds non-negative `reclaimed_bytes bigint NOT NULL DEFAULT 0` to the reclamation ledger | Logical byte accounting only; not process-exit evidence |
| 0057 `workspace_reclamation_totals` | Adds a per-resource-kind monotonic aggregate table with non-negative total | Metrics/accounting only; no Run/Lease authority |
| 0058 `external_commit_adoption` | Creates external Commit and clarification tables; adds repository scan-anchor columns | No direct Run lifecycle DDL identified; retain as an adjacent domain migration, not an execution-control entity |
| 0059 `discovery_round_error` | Adds `last_round_error` and `last_round_error_at` to Discovery session | No direct Run lifecycle DDL identified |

The 0054 application behavior was cross-checked against the PA08-04 implementation inventory: claim selection and `cleaning` update are performed in one CTE under `FOR UPDATE SKIP LOCKED`; the claim lease is represented by `next_retry_at`; each claim gets a fresh token; stale claimants must be rejected. This implementation behavior is not inferred from the migration DDL alone.

### 13.2 Migration audit scope correction

The previous phrase “replay 0001–0059” is shorthand for the **current numbered sqlx chain**, not a claim that all historical migrations are still present. The exact target is:

1. Fresh PostgreSQL database.
2. Apply current `0001_baseline.sql` (squashed schema plus seed/data statements).
3. Apply each checked-in post-baseline migration in numeric order, 0002 through 0059, verifying there are no missing/duplicate versions in the actual `migrations/` directory.
4. Capture catalog state after the final migration: constraints, indexes, triggers, trigger functions, and relevant table definitions.
5. Compare the catalog capture with a migration-derived expected inventory and then trace every application SQL writer/transaction that can touch the audited aggregates.

The GitHub file-fetch audit in this pass verified the baseline provenance and selected late DDL, but **did not** enumerate every file in `migrations/`, replay PostgreSQL, or capture a live catalog. Those remain open and are not marked complete.

### 13.3 Updated blockers

- P0: obtain a reproducible full migration-directory manifest for the current master SHA; verify exactly one file per version and no gaps in the current post-baseline range.
- P0: run the current sqlx migration chain against an empty PostgreSQL database and retain the actual command output and database identity.
- P0: capture `pg_constraint`, `pg_index`, `pg_trigger`, and `pg_proc` from that resulting database; include FK-induced key locks and trigger side effects in the global lock graph.
- P1: compare baseline definitions for Run/Interaction/Command/Continuation/Human Intervention/Notification/Cleanup with every post-baseline alteration, then reconcile the current schema against the application SQL.
- P1: add DDL-level tests for tenant/project/task composite identity and for the future Barrier/HIR/E6/process-evidence anchors.

Status remains **Changes Required / Audit In Progress**. This pass improves migration provenance and verifies exact late DDL, but does not close the full migration audit, select a global lock order, authorize schema implementation, or start Phase 0. No business code, migration, API/WSS implementation or runtime configuration was changed.


## 14. Seventh audit pass — exact execution-core baseline DDL and post-baseline delta inventory

This pass inspected the complete current `migrations/` directory and extracted the execution-core table definitions, constraints, foreign keys, unique indexes and relevant triggers from `0001_baseline.sql`. It also fetched every post-baseline migration file and searched its SQL for changes to the execution-core relations.

### 14.1 Migration manifest: verified against both refs

- Current `master` contains exactly **59 SQL files**: `0001_baseline.sql` plus every numbered migration `0002` through `0059`.
- No missing, duplicate or out-of-range version was found in the directory listing.
- The PR branch `docs/sem-02e-architecture-convergence` contains the same 59 filenames and the same blob SHA for every migration file. The PR branch does not carry a divergent migration chain.
- This is a Git tree/filename/blob comparison only. It is not evidence that PostgreSQL accepted the chain.

### 14.2 Baseline execution-core entities and database-enforced relationships

The following definitions were extracted directly from `0001_baseline.sql` (line numbers refer to the current master blob):

| Entity | Baseline definition | Important database constraints / indexes |
|---|---:|---|
| `delivery.task` | 1680–1706 | PK `id`; unique `(project_id,id)`, `(tenant_id,id)`; tenant/project composite FK; milestone composite FK; active/status lookup index; code uniqueness added later by 0050 |
| `delivery.task_run` | 1894–1960 | PK `id`; unique `(project_id,id)`, `(tenant_id,id)`; composite `(project_id,task_id)` FK to Task; `(tenant_id,lease_client_id)` FK to device; partial active Run, attempt, source-event and dispatch/lease indexes |
| `delivery.task_interaction` | 1770–1798 | PK; unique `(project_id,id)`, `(run_id,event_id)`; unique one-open-chain-per-Task partial index; separate single-column FKs to Task, Run and Project |
| `delivery.interaction_response` | 1448–1461 | PK; unique `(project_id,id)`; one response per Interaction and idempotency index; separate single-column FKs to Interaction, Task, Run and Project |
| `delivery.interaction_response_delivery` | 1468–1487 | PK; unique `(project_id,id)`; unique one delivery per response; separate single-column FKs to Interaction, Response, Run and Project |
| `delivery.continuation` | 1427–1441 | PK; unique `(project_id,id)`, `(run_id,generation)`; separate single-column FKs to Interaction, Task and Run |
| `delivery.worker_command` | 2236–2262 | PK; unique `(project_id,id)`; partial active-command unique index on `(run_id,command_type,interaction_id)`; separate single-column FKs to Interaction, Task and Run |
| `delivery.task_waiting` | 2114–2128 | PK; partial one-active-waiting-per-Task unique index; composite project/task FK |
| `delivery.task_run_cleanup` | 2016–2031 | PK on `run_id`; Run FK with cascade; due/human indexes; 0054 adds claim fencing |
| `governance.human_intervention` | 2531–2543 | PK; tenant/project composite FK; proposal/actor FKs; seven action-history types, not a Run-scoped HIR lifecycle |
| `integration.notification` | 2687–2697 | PK; tenant/project composite FK; proposal source unique; Project Feed shape with shared `read_at` |

### 14.3 Composite-identity gaps in the existing execution schema

The baseline has useful composite keys on Task and Run, and the normal Run→Task relation is correctly constrained by `(project_id,task_id)`. However, several child records carry multiple aggregate IDs but validate them independently:

- `task_interaction`: Task, Run, tenant and project are separately referenced; there is no composite FK proving that the referenced Run belongs to the referenced Task and same Project/Tenant.
- `interaction_response`: Interaction, Task and Run are independently referenced. A valid set of individual IDs can therefore still describe different aggregate identities unless the application transaction checks them.
- `interaction_response_delivery`: Interaction, Response and Run are independently referenced; no composite FK proves the response/interaction/run tuple is coherent.
- `continuation`: Task, Run and origin Interaction are independently referenced; no DB constraint proves that all three belong to the same Task/Run lineage.
- `worker_command`: Task, Run and optional Interaction are independently referenced; no composite FK proves that the command target tuple is coherent.
- `human_consultation` (0046): nullable Task FK is only `task_id → task(id) ON DELETE SET NULL`; `tenant_id` separately references Tenant, and `project_id` has no same-Project composite FK. This is a generic consultation row, not a typed HIR/E6 binding.
- `task_run_event` (0013) intentionally has no database FK, as its migration comment explicitly delegates cross-aggregate identity to application validation.

These are **schema-enforced integrity gaps**, not proof that current API handlers accept mismatched IDs. The source transaction audit must verify every writer validates the full tuple under the appropriate locks. SEM's future HIR, Barrier, process-evidence and E6 anchors must not repeat this independent-FK pattern; use matching composite unique targets and deferred constraints where a real cycle requires them.

### 14.4 Revision and start-gate snapshot references

Two additional schema details matter to execution provenance:

- 0042 creates `delivery.task_revision` with `FOREIGN KEY (task_id) REFERENCES delivery.task(id)`; `task.current_task_revision_id` and `task_run.task_revision_id` each reference Revision by ID only. The DDL does not prove that the current Revision belongs to that Task, or that the Run's Revision belongs to the Run's Task. Application CAS/ownership validation is therefore part of the correctness boundary.
- 0044 creates `task_run_dependency_snapshot` with a Run ID FK and a separate Tenant FK, and stores Task/predecessor IDs without a composite Task/Run identity FK. The snapshot is historical gate evidence, but its relational ownership is not fully enforced by the DDL.

### 14.5 Unique-index and trigger semantics that affect concurrent transactions

- `uq_task_run_active`, `uq_task_review_active`, attempt-number indexes, source-event idempotency, open Interaction chain, active Task waiting and active Worker command indexes are database serialization points. Their unique-index waits/conflicts must be included in concurrency tests; they do not replace Task Barrier authorization.
- `uq_worker_command_active(run_id, command_type, interaction_id)` has a nullable `interaction_id`. PostgreSQL unique indexes treat NULL values as distinct, so multiple otherwise-identical active commands with NULL interaction_id are not prevented by this index. Verify whether command creation supplies a separate idempotency key or serializes this case; do not assume this index alone enforces one active terminate command.
- 0013 adds an append-only UPDATE guard to `task_run_event`; DELETE remains permitted by design. This event ledger is not a process-exit evidence ledger.
- The baseline has a BEFORE UPDATE guard on `task_run` for immutable Review report fields. 0050 adds a BEFORE INSERT trigger on `delivery.task` to fill its business code.
- 0050's trigger computes `MAX(code)+1` without an advisory/row lock. Its `EXCEPTION WHEN unique_violation` block encloses only the SELECT, not the outer INSERT that later checks `uq_task_code_project`; a concurrent unique-index conflict raised after the trigger returns cannot be caught by that block. The documented fallback `P-<id>` therefore does not handle the concurrent INSERT collision as written. This is an adjacent Task-creation concurrency defect and must be tracked separately from SEM's Run lock order.
- FK checks take key-share locks on referenced rows; FK deletes/updates and unique-index conflicts can add waits even when application SQL does not explicitly request a row lock. These implicit edges remain part of the complete lock graph.

### 14.6 Post-baseline delta inventory affecting the execution core

A fetch-and-scan of every `0002–0059` SQL file found these direct execution-core changes:

| Migration | Direct effect |
|---|---|
| 0010 | Adds `delivery.task.implementation_baseline_id` |
| 0011 | Adds Task Plan revision/key references and a partial unique index |
| 0013 | Adds `task_run_event` append-only event ledger and indexes/trigger |
| 0014 | Adds pinned knowledge revision to Task knowledge refs |
| 0015 | Adds Task schedule columns and CHECK constraints |
| 0042 | Adds Task Revision table, Task current-revision pointer and Run revision pointer |
| 0044 | Adds Task→Product relation, Run dependency snapshot, and Run product-gate snapshot fields |
| 0045 | Adds Product-specific Task acceptance records |
| 0046 | Adds generic Human Consultation table with nullable Task FK |
| 0047 | Product-link withdrawal semantics; no direct Task/Run DDL |
| 0050 | Adds Task business code, unique index and insert trigger |
| 0051 | No direct Task/Run DDL; execution log remains the Run event ledger |
| 0052 | Adds issue/risk/blocker entities; blocker may reference a Task |
| 0053 | Adds per-device active Run partial unique index on `lease_client_id` |
| 0054 | Adds `task_run_cleanup.claim_token` and changes due index to `(next_retry_at,run_id)` |
| 0055 | Adds separate Workspace reclamation ledger with restrictive FKs to Project, Project Repository and Run Repository |
| 0056–0057 | Add reclaimed-byte accounting and cumulative totals |
| 0058–0059 | No direct Task/Run/Interaction/Command DDL identified |

The inventory is based on static SQL content and direct text inspection. It does not yet include a PostgreSQL parser's resolved dependency graph or catalog-confirmed trigger/function state.

### 14.7 Updated blockers and status

- P0: fresh-database replay of the exact 59-file chain remains unexecuted.
- P0: live catalog capture (`pg_constraint`, `pg_index`, `pg_trigger`, `pg_proc`) remains unexecuted; the Git baseline is not a substitute.
- P0: finish application-writer validation for the independent-FK tuples above and incorporate FK key-share/unique-index waits into the global lock graph.
- P1: resolve the nullable `worker_command` uniqueness case and the 0050 Task-code concurrent allocation defect, or record explicit application-level idempotency/serialization proof.
- P1: add cross-Task/Project negative tests for Task Revision, Interaction/Response/Continuation/Command, Human Consultation and Run dependency snapshots.
- Existing P0 lock-order and Task Barrier bypass findings from §§9–11 remain open. No global lock order is selected by this pass.

Status remains **Changes Required / Audit In Progress**. This pass verifies the Git migration manifest and static DDL relationships; it does not claim a successful PostgreSQL replay, live catalog capture, complete writer audit, or deadlock-free lock order. No business code, migration, API/WSS implementation or runtime configuration was changed. No PR merge or Phase 0 authorization.


## 15. Eighth audit pass — application writers for composite-identity rows and exact lock-order evidence

This pass followed the production SQL writers for Interaction, Response, ResponseDelivery, Continuation and WorkerCommand, rather than relying only on DDL shape.

### 15.1 Normal writer identity validation

The current primary application path does propagate a coherent identity tuple:

- `report_interaction` starts one transaction, locks the target Run while joining its Task, and requires `run.id + run.task_id + run.project_id` to match the caller's Run/Task/Project inputs. It then creates the Interaction and its Terminate command from that validated tuple.
- `submit_interaction_response` locks the Interaction by `interaction_id + project_id`, then derives `task_id` and `run_id` from that locked row. The Response, Continue command, ResponseDelivery and Continuation all reuse those derived IDs in the same transaction.
- `retry_interaction` locks the previous Interaction, derives its Task/Run IDs, locks the referenced Run, and creates the retry Interaction/Terminate command from that lineage.
- `open_continuation_tx` receives the Task/Run/Interaction tuple from the response transaction and inserts the Continuation plus ResponseDelivery back-reference in that same transaction.
- The production insert search found the above modules as the application write owners for these relations; test fixtures and reset scripts also write rows directly, but are not production writers.

This narrows the current risk: the independent-FK DDL gaps do **not** establish that the normal service path currently mixes unrelated IDs. However, the DDL cannot independently guarantee that invariant; all new writer entry points, maintenance jobs, direct SQL, and future HIR/Barrier/E6 writers must preserve the same validation.

### 15.2 Exact lock-order cycle reproduced from source statements

The Run↔Interaction inversion is now tied to the concrete statements:

| Path | First relevant row lock | Later relevant row lock/update | Edge |
|---|---|---|---|
| `report_interaction` | SELECT Run JOIN Task with `FOR UPDATE OF r` | SELECT open Interaction with `FOR UPDATE` | Run → Interaction |
| `submit_interaction_response` | SELECT Interaction with `FOR UPDATE` | `record_response_persisted_tx` updates Run scheduling shield | Interaction → Run |
| `retry_interaction` | SELECT previous Interaction with `FOR UPDATE` | SELECT referenced Run with `FOR UPDATE` | Interaction → Run |

Source locations at current master `d0e72fa`:

- `crates/pm-application/src/interaction.rs:120–136`: report path begins transaction and locks Run after validating joined Task/Project.
- `interaction.rs:193–200`: report path subsequently locks any open Interaction for the Task.
- `interaction.rs:224–249`: inserts Interaction using the validated Task/Run tuple.
- `interaction.rs:989–1005` (approximate function region): response path locks Interaction first; the exact SQL is in `submit_interaction_response`.
- `interaction.rs:397–414`: response persistence updates Interaction then updates its Run scheduling shield.
- `interaction.rs:778–790` and following Run lookup: retry locks Interaction, then locks Run.
- `worker_command.rs:360–372`: ACK path locks WorkerCommand first; subsequent ACK effects mutate Run/Delivery/Interaction/Continuation/Task state.

The cycle is not hypothetical: two concurrent transactions can hold opposite endpoints while waiting on the other. It remains P0. The fix must make every relevant path acquire the same canonical order before any writes; changing only one handler or relying on statement idempotency is insufficient. The complete order still cannot be frozen until Project, Task, Run, Interaction, Command, Delivery, Continuation, HIR/Barrier and governance paths are all included.

### 15.3 Continuation generation allocation is not independently serialized by its allocator

`open_continuation_tx` computes `MAX(generation)+1` for a Run and then inserts a row protected by `UNIQUE(run_id,generation)`. The code comment says callers must preserve serialization by holding the Run lock, but the response submission path shown in §15.2 reads the Run without `FOR UPDATE`; its later Run UPDATE does acquire a write lock, but only after the generation allocator has run (the allocator is called after response/command/delivery writes). Therefore the allocator's correctness currently depends on surrounding transaction lock behavior and the single-open-Interaction invariant, not on an explicit lock at the allocation point. The unique constraint prevents duplicate committed generations but a concurrent collision can abort one entire response transaction. Add an explicit two-connection test for same-Run generation allocation, and either acquire the Run lock before allocation in the canonical order or replace max+1 with a database-enforced allocator.

### 15.4 Updated writer-audit status

- Normal production writers for the five relations above were located and their identity tuple derivation was traced.
- The schema still lacks composite FK guarantees for these tuples; the service validation is the current protection.
- Run↔Interaction lock inversion is confirmed by source, not closed.
- WorkerCommand ACK adds Command→Run and downstream mutation edges; it must be incorporated into the same global lock order.
- J11–J18 remain required and must be supplemented with cross-identity negative tests and a same-Run continuation-generation concurrency test.

Status remains **Changes Required / Audit In Progress**. No production code or migrations changed; this pass only strengthens source evidence and test requirements. PostgreSQL replay/catalog capture and global lock-order closure remain open. No PR merge or Phase 0 authorization.


## 16. Ninth audit pass — Product association writer introduces an implicit Task↔Product lock inversion

A source trace found an additional lock-order cycle outside the earlier Run/Interaction/Command cluster.

### 16.1 Claim-side lock order

`run_dispatch/claim.rs` documents and implements the claim order:

1. `core.project FOR UPDATE`
2. candidate `delivery.task_run FOR UPDATE SKIP LOCKED`
3. `delivery.task FOR UPDATE`
4. Product gate locks each associated `core.product` row with `FOR UPDATE OF p`

The Product gate query is in `task_products.rs:262–271`; its caller is the normal claim transaction after Project→Run→Task locks. The claim path therefore holds Task before requesting Product.

### 16.2 Association-write-side lock order

`task_products::add_task_product` currently:

1. Begins a transaction.
2. Reads Task `project_id` by `task_id + tenant_id` without `FOR UPDATE` (`task_products.rs:127–134`).
3. Calls `product::assert_product_active`, which locks `core.product` using `FOR UPDATE` (`product.rs:928–940`).
4. Inserts `delivery.task_product` (`task_products.rs:139–151`).

The INSERT's FK check on `delivery.task_product.task_id → delivery.task(id)` acquires a key-share lock on the referenced Task row. PostgreSQL's key-share lock conflicts with the claim path's Task `FOR UPDATE`.

### 16.3 Concrete deadlock schedule

| Transaction A — Run claim | Transaction B — add Task Product |
|---|---|
| Locks Project, Run, then Task FOR UPDATE | |
| | Locks Product FOR UPDATE in assert_product_active |
| Reaches Product gate and waits for Product | |
| | Inserts TaskProduct; FK check waits for Task key-share |
| **A waits for B's Product lock** | **B waits for A's Task lock** |

This is a concrete lock cycle: Task→Product versus Product→Task (the latter edge is implicit in the FK check, not visible as an explicit SELECT FOR UPDATE). PostgreSQL will abort one transaction as a deadlock victim. This is a new P0 item for the global lock-order audit.

### 16.4 Required closure

- Make Task Product association writers participate in the canonical order by locking and validating the Task row before locking Product; do not rely on a plain Task SELECT followed by Product FOR UPDATE.
- Audit the withdraw path and every future association mutation for the same Task→Product order and for claim snapshot linearization.
- Add a deterministic two-connection test that forces the interleaving above and asserts no deadlock after the correction.
- Include FK key-share edges in the lock graph; an application-only list of explicit FOR UPDATE statements is incomplete.

This pass identifies a source-level deadlock schedule; it does not modify the production code or claim a fix. The Run↔Interaction inversion and all earlier P0s remain open. Status remains **Changes Required / Audit In Progress**; no PostgreSQL replay/catalog capture, global lock-order closure, PR merge or Phase 0 authorization.


## 17. Tenth audit pass — remaining writer ownership and gate-linearization checks

This pass checked the remaining application writers behind Task Revision, TaskWaiting/Human Consultation, dependency gates and Run-start snapshots.

### 17.1 Task Revision ownership

- `snapshot_and_insert` itself does not acquire a Task row lock. Its correctness is a caller precondition.
- The Task PATCH path obtains `delivery.task FOR UPDATE` before updating the Task and calling `snapshot_and_insert`.
- Task creation / plan materialization / command-created Task paths insert the Task and initial Revision in the same transaction before exposing the new Task.
- `insert_task_run` explicitly requires the caller to hold the Task row lock. The TaskReady outbox handler locks Task before calling it; Review retry updates/locks Task before creating the retry Run; lease recovery calls it inside the claim transaction after the claim path has locked Task.
- Thus the normal writers preserve same-Task Revision ownership by transaction construction, despite the missing composite FK from Task/Run to Revision. Keep the caller-lock precondition explicit and test all call sites when adding a new Run-creation path.

### 17.2 Human Consultation and TaskWaiting

- `create_consultation` inserts Human Consultation and, when `task_id` is non-null, inserts `task_waiting` in the same transaction.
- Although 0046's Human Consultation Task FK is only a single-column FK, the paired TaskWaiting row has both tenant/project and project/task composite FKs. A task-bound consultation therefore cannot commit through the current application writer with a Task from a different Project/Tenant: the paired TaskWaiting insert fails and rolls back the transaction.
- The API also performs project visibility/membership checks before creation. Unbound consultations (`task_id IS NULL`) are project-level decisions and have no Task relation.
- The TaskWaiting FK's key-share lock serializes its insertion against Run claim's Task `FOR UPDATE`. No reverse Consultation-row lock edge was found in the claim path. The ordering is linearizable: if claim owns Task first, the consultation is created after that claim; if the waiting row is committed first, claim observes it and is denied. This does not terminate an already-running process; that is a separate execution-intervention requirement.

### 17.3 Dependency gate and Run-start snapshots

- Dependency edge writers acquire the per-Project advisory graph lock before locking the downstream Task; the Run claim path locks the Task before checking dependencies. Both serialize on the downstream Task row for the same Task, and no reverse advisory-lock edge from the claim path was found in this pass.
- The dependency predicate is read-only and does not lock predecessor Tasks. The later Run-start snapshot reads predecessor Task revision pointers and writes `task_run_dependency_snapshot` rows in the same claim transaction. No current cross-project writer was found, but predecessor revision changes are not explicitly row-locked by the snapshot reader; add a two-connection test to establish the intended snapshot semantics if predecessor Task revisions can still change after completion.
- `task_run_dependency_snapshot` has intentionally thin FK coverage; current writer derives Task/Project/Tenant from the locked claim context and dependency edges. This is application-enforced, not catalog-enforced.

### 17.4 Task Product mutation remains the new P0

The Task↔Product deadlock in §16 is not closed by these checks. `withdraw_task_product` also does not lock Task; its concurrent behavior with a claim's Product-gate read must be covered by a deterministic test. The add path must first be corrected to Task→Product order before any global lock order can be declared.

Status remains **Changes Required / Audit In Progress**. This pass validates current caller ownership patterns for Task Revision, Run creation and task-bound Consultation; it does not repair the §16 deadlock, close the Run↔Interaction inversion, prove predecessor snapshot isolation, execute PostgreSQL replay/catalog capture, or authorize Phase 0.


## 18. Eleventh audit pass — Project↔Task inversion through the audit-log foreign key

A further implicit FK edge creates a second concrete deadlock schedule involving the Project lock at the head of Run claim.

### 18.1 Relevant database and writer facts

- Run claim explicitly locks `core.project FOR UPDATE` first, then the candidate Run, then `delivery.task FOR UPDATE` (claim protocol §5.2 / `run_dispatch/claim.rs`).
- `governance.audit_log` has composite FK `(tenant_id,project_id) → core.project(tenant_id,id)` (baseline `0001_baseline.sql`, constraint `fk_audit_log_tenant_project`).
- `pm_write::audit::append_audit` and `append_audit_with_after` insert an audit row with `project_id` and derive `tenant_id` from `core.project`; the FK check takes a key-share lock on the referenced Project row.
- Task PATCH locks the Task row with `SELECT ... FROM delivery.task WHERE id = $1 FOR UPDATE` and subsequently writes its audit row in the same transaction (`pm_write/task.rs`). The Task PATCH path does not first take the Project row lock.

### 18.2 Concrete deadlock schedule

| Transaction A — Run claim | Transaction B — Task PATCH |
|---|---|
| Locks Project FOR UPDATE | |
| Locks candidate Run | |
| Attempts Task FOR UPDATE and waits | Locks Task FOR UPDATE |
| **A waits for B's Task lock** | Writes audit_log; FK needs Project KEY SHARE |
| | **B waits for A's Project lock** |

This is a Project→Task edge in claim and a Task→Project implicit FK edge in the audited Task writer. Unlike the claim's candidate Run lock (which uses SKIP LOCKED), the Task lock is a normal blocking `FOR UPDATE`; the schedule is therefore not avoided by SKIP LOCKED.

This is a new P0 deadlock candidate backed by the exact FK and writer SQL. It must be reproduced with two independent PostgreSQL connections before being labeled a runtime-reproduced deadlock, but the lock compatibility and wait cycle are explicit.

### 18.3 Required closure

- Inventory every transaction that locks Task (or Run/Interaction/Command) and later inserts `governance.audit_log`, including `append_audit`, `append_audit_with_after` and any direct SQL audit writers.
- Decide a canonical Project lock policy. Options to evaluate include acquiring Project before Task in every affected writer, or changing the claim path's Project row lock strategy only if archive/claim linearization can still be proved. Do not remove the Project lock as a local workaround.
- Add a deterministic two-connection test: claim holds Project and pauses before Task; Task PATCH holds Task and attempts audit insert; assert the corrected order completes without deadlock.
- Include audit-log FK KEY SHARE in the global lock graph and in the catalog replay/capture checklist.

The earlier Task↔Product inversion (§16) and Run↔Interaction inversion (§15) remain open. This pass does not change production code and does not claim a runtime deadlock reproduction. Status remains **Changes Required / Audit In Progress**; no global lock order, PR merge or Phase 0 authorization.


### 18.4 Lock-mode alternatives to evaluate (not yet decided)

Two plausible corrections preserve Project lifecycle serialization; neither is selected without PostgreSQL concurrency tests:

1. **Weaken the claim-side Project lock from FOR UPDATE to FOR NO KEY UPDATE.** The claim path only reads Project status; Project identity/key columns are not changed. PostgreSQL's FOR NO KEY UPDATE conflicts with the Project status UPDATE / archive's explicit FOR UPDATE, but is compatible with the KEY SHARE lock taken by the audit-log FK. This may remove the Task→Project audit-FK edge without forcing every Task writer to serialize on an exclusive Project lock. Verify every Project lifecycle writer and all other claim-side assumptions before adopting.
2. **Acquire Project KEY SHARE before Task locks in affected writers.** This makes the Project edge explicit and early, so a claim holding Project FOR UPDATE waits before the writer acquires Task. It is more invasive across the many Task/Run/Interaction/Command writers and may add contention if the claim retains FOR UPDATE.

Do not combine a local SQL edit with an assumed global fix. The final lock policy must account for all row-lock modes (FOR UPDATE / NO KEY UPDATE / SHARE / KEY SHARE), unique-index waits, FK checks, trigger writes and advisory locks.


## 19. Twelfth audit pass — Project and Product lock-mode cross-check

This pass expands the Project lock inventory and checks whether the two §18.4 mitigation candidates conflict with existing Project lifecycle writers. It also distinguishes the Task↔Product inversion from Product lifecycle serialization.

### 19.1 Project row-lock inventory

| Writer / path | Project lock behavior | Later relevant lock/write | Audit result |
|---|---|---|---|
| Run claim `claim_core_in_project` | Explicit `FOR UPDATE` before Run and Task | Run `FOR UPDATE OF r SKIP LOCKED`, then Task `FOR UPDATE`, Product gate later | Source-confirmed head of Project→Run→Task→Product |
| Project archive `archive::close_project` | Explicit `FOR UPDATE` | Status close, queued Run cancellation, import finalization, audit | Shares the Project serialization point with claim |
| Formal knowledge writes / review enqueue / Run start confirmation | `lock_project_activity_tx` uses `FOR UPDATE` | Domain writes / Run transition | Same Project-first order |
| Project member changes | Explicit `FOR UPDATE` after Team→Tenant | Project-member row changes, then audit | Project is acquired before Project audit FK |
| Project import lifecycle creation | Explicit `FOR UPDATE` | Import initialization writes | Project-first; no Task-first edge found in the inspected path |
| Project info update | Direct `UPDATE core.project` (non-key columns) | Inserts Project audit row | UPDATE's row lock is not an explicit `FOR UPDATE`; audit FK KEY SHARE is compatible with the non-key-update lock mode |
| Task PATCH | No Project lock before Task | Task `FOR UPDATE`, then `governance.audit_log` insert | Remains the concrete reverse Task→Project edge in §18 |

The current source therefore supports a narrow Project→Task cycle through Task PATCH, not a claim that every Project audit writer is inverted. Project member and import writers acquire Project before their downstream writes. Project info update does not first hold a Task/Run/Interaction lock.

### 19.2 Evaluation of the §18.4 Project lock-mode candidate

The candidate claim-side `FOR NO KEY UPDATE` remains structurally plausible for the specific audit-FK cycle:

- It conflicts with the explicit Project `FOR UPDATE` used by archive and `lock_project_activity_tx`, preserving mutual exclusion with those paths.
- It is compatible with the Project KEY SHARE lock required by the `governance.audit_log` composite FK, so a Task PATCH holding Task would not wait on the claim's Project guard merely to append its audit row.
- Project info's non-key UPDATE does not add a new reverse Task edge.
- This is not yet an approved change. The claim lock's exact assumptions, all Project lifecycle transitions, and PostgreSQL row-lock compatibility must be verified in the actual server version with deterministic two-connection tests. Do not weaken the lock based on source reasoning alone.

The alternative—acquiring Project KEY SHARE before Task in every Task-first audited writer—would preserve the current claim `FOR UPDATE`, but requires a complete inventory of all Task/Run/Interaction/Command writers and consistent ordering before their first aggregate lock. It is more invasive and must include direct audit SQL writers, not only the shared helper.

### 19.3 Product lifecycle and TaskProduct mutation cross-check

- `product::set_product_lifecycle` explicitly locks the Product row `FOR UPDATE`, updates its status, and writes `governance.tenant_audit_log`. It does not subsequently lock Task, Run, or Project rows in the inspected transaction.
- Run claim locks Task before calling `product_gate_denied`, which locks associated Product rows `FOR UPDATE`. Product lifecycle therefore serializes on Product without creating a Product→Task reverse edge.
- `add_task_product` is different: it locks Product through `assert_product_active`, then inserts `delivery.task_product`; the Task FK check needs Task KEY SHARE while claim holds Task `FOR UPDATE`. This remains the concrete Product→Task half of the §16 P0 cycle.
- `withdraw_task_product` reads Task without locking it, updates only the association's withdrawal columns, then writes Project audit/activity rows. The inspected UPDATE does not modify FK columns and does not itself add a Task/Product parent-row lock. It therefore does not independently establish the reverse Product→Task edge, but its race with claim's active-association/product snapshot must be tested for intended semantics.
- The Product lifecycle path and the association mutation path must not be conflated: changing Product status is serialized by the Product row; adding a Task association has the additional Task FK lock edge.

### 19.4 Required test additions

Add the following to the two-connection PostgreSQL matrix:

1. Project claim `FOR NO KEY UPDATE` versus archive `FOR UPDATE`: prove they block and preserve archive/claim linearization.
2. Project claim `FOR NO KEY UPDATE` versus Task PATCH audit FK KEY SHARE: prove the FK insert no longer waits on the claim's Project guard, while Task lock ordering remains correct.
3. Existing Task↔Product schedule: claim holds Task then pauses before Product; association add must acquire Task first after correction, and both transactions complete without a deadlock.
4. Product lifecycle suspend versus Run claim Product gate: preserve the existing intended serialization and verify the claim cannot pass on a stale Product state.
5. TaskProduct withdraw versus claim: define whether claim observes the association as active based on transaction order; verify snapshot and gate decision agree.

This pass is source-level only. No PostgreSQL server is available in the current audit environment; none of these concurrency schedules has been runtime-executed. The three P0 deadlock candidates in §§15, 16 and 18 remain open; no fix, global lock order, PR merge or Phase 0 authorization is implied. Status remains **Changes Required / Audit In Progress**.


## 20. Thirteenth audit pass — Project↔Run inversion through Interaction creation

The Project lock cycle in §18 is not limited to Task PATCH. A production Run-first writer inserts a Project-scoped Interaction before it reaches the Task transition, so the Project FK itself creates a Project↔Run wait cycle.

### 20.1 Exact source and DDL path

- Run claim acquires `core.project FOR UPDATE` before selecting/locking the candidate Run (`run_dispatch/claim.rs::claim_core_in_project`).
- `interaction::report_interaction` begins by selecting the target Run `FOR UPDATE OF r` (`interaction.rs::report_interaction`); it does not first lock Project.
- The same transaction then inserts `delivery.task_interaction` with `project_id`. The baseline DDL has a Project-scoped FK to `core.project`; PostgreSQL's FK check takes KEY SHARE on the referenced Project row.
- This Interaction INSERT occurs before the later Task transition and before the audit append. Therefore the reverse edge is not dependent on the audit helper: it is present in the primary Interaction creation path itself.

### 20.2 Concrete deadlock schedule

| Transaction A — Run claim | Transaction B — report_interaction |
|---|---|
| Locks Project `FOR UPDATE` | |
| Attempts candidate Run `FOR UPDATE OF r` and waits | Locks the same Run `FOR UPDATE OF r` |
| **A waits for B's Run lock** | Inserts `task_interaction`; Project FK needs KEY SHARE |
| | **B waits for A's Project lock** |

This is a source/DDL-derived P0 deadlock candidate. It is distinct from §18's Project→Task / Task→Project schedule, although both share the same Project lock head. It has not been runtime-reproduced.

### 20.3 Consequences for the Project lock policy

- The §18.4 claim-side `FOR NO KEY UPDATE` candidate is now more valuable than a local Task PATCH workaround: PostgreSQL permits the FK KEY SHARE edge while the claim retains conflict with Project lifecycle writers using explicit `FOR UPDATE`.
- An alternative is for `report_interaction` to acquire Project KEY SHARE before locking Run, but this must be applied consistently to every Run-first writer that inserts Project-scoped rows or appends audit. A one-handler fix does not establish a global order.
- Do not select either correction yet. Verify the exact PostgreSQL lock compatibility and archive/claim/interaction linearization using deterministic two-connection tests.
- The same pattern must be searched for in response persistence, WorkerCommand ACK, continuation/recovery, and other Run/Interaction/Command-first transactions that later insert a Project-scoped row or audit row. Their presence is an audit scope, not yet a claim that every such path is independently deadlocking.

### 20.4 Required deterministic test

Add a two-connection case with explicit pause points:

1. Connection A starts Run claim, locks Project `FOR UPDATE`, then pauses before candidate Run lock.
2. Connection B starts `report_interaction`, locks the same Run, then attempts the `task_interaction` INSERT.
3. Assert the current schedule produces the expected wait cycle/deadlock detector result in the unmodified baseline (or a bounded lock timeout demonstrating both waits).
4. Under the selected correction, assert both transactions complete, and assert the resulting Run/Interaction/Project lifecycle state is valid.
5. In the same test suite, race Project archive against Interaction creation to prove the weaker lock or explicit Project-first ordering preserves the archive gate.

This is a source/DDL-derived finding only. No PostgreSQL concurrency test was executed in this environment. §§15, 16, 18 and 20 remain open P0 lock-order blockers; no global lock order, production fix, PR merge or Phase 0 authorization is implied. Status remains **Changes Required / Audit In Progress**.


## 21. Fourteenth audit pass — additional Run-first writers reaching Project FK/audit

The §20 Project↔Run inversion is a shared transaction-order defect pattern, not an isolated Interaction endpoint. This pass confirms three additional production paths that hold or acquire a Run lock before requesting the Project KEY SHARE edge through the audit FK.

### 21.1 Confirmed writer paths

| Writer | Locks before Project edge | Project edge | Classification |
|---|---|---|---|
| `recovery::reacquire_lease` | Run `FOR UPDATE`; then updates Run lease fields and WorkerCommand routing | `append_audit_with_after("run.lease_reacquired")` inserts `governance.audit_log` | Confirmed Run→Project |
| `worker_command::ack_command` (continue ACK) | WorkerCommand `FOR UPDATE`; then updates Run scheduling shield; downstream Delivery/Continuation/Task recovery writes | Final `append_audit_with_after("command.acked")` | Confirmed Run→Project; Command is an additional held row |
| `interaction::record_response_persisted_tx` | Updates Interaction, then updates Run scheduling shield | `append_audit_with_payload` delegates to `append_audit_with_after` | Confirmed Run→Project |
| `interaction::report_interaction` | Run `FOR UPDATE OF r` | Earlier `task_interaction` INSERT has Project FK KEY SHARE; final audit is a second Project edge | Confirmed Run→Project |

The audit helper implementation inserts `governance.audit_log` with `project_id` and the composite Project FK. This confirms the Project KEY SHARE edge is not merely inferred from helper names.

### 21.2 Shared wait-cycle shape

For each writer above, the critical two-transaction shape is:

- Claim transaction: Project `FOR UPDATE` → waits for the same Run row.
- Run-first writer: holds that Run row (or acquires it before completion) → reaches an Interaction/Project-scoped insert or audit FK requiring Project KEY SHARE.
- The two transactions wait on one another.

The exact pause point differs by writer. For continue ACK, the command row is also held; for recovery, the Run row is explicitly selected `FOR UPDATE`; for response persistence, the Run lock is obtained by UPDATE. These are separate entrypoint schedules sharing one Project→Run / Run→Project cycle.

### 21.3 Scope and limits

- This pass does not claim that every WorkerCommand ACK branch locks Run: the confirmed ACK case is the continue branch that updates `task_run.scheduling_shield`; terminate ACK branches must be analyzed separately.
- It does not claim all Interaction lifecycle methods are Run-first. Timeout sweep and other bulk writers have different acquisition patterns and remain in the inventory.
- The Project `FOR NO KEY UPDATE` candidate would allow these Project FK KEY SHARE edges while still conflicting with Project lifecycle paths using explicit `FOR UPDATE`; this remains a candidate only, not an approved fix.
- A full global order still must account for Command, Interaction, Delivery, Continuation, Task, Product, audit, FK checks, unique-index waits, triggers and advisory locks.

### 21.4 Test-matrix expansion

Add deterministic two-connection cases for:

1. Claim Project→Run versus `reacquire_lease` Run→audit Project.
2. Claim Project→Run versus continue `ack_command` Command→Run→audit Project.
3. Claim Project→Run versus `record_response_persisted_tx` Interaction→Run→audit Project.
4. Claim Project→Run versus `report_interaction` Run→Interaction Project FK.
5. Repeat each schedule under the selected Project lock correction and prove Project archive still serializes with the claim/interaction lifecycle.

Each test must use independent PostgreSQL connections, explicit barriers/pause points, bounded lock waits, and assert both transaction outcomes plus persisted state. Do not treat a unit test that only inspects SQL strings as concurrency evidence.

This remains source-derived. No PostgreSQL server or two-connection test was available in this environment. The Project↔Run P0 family in §§20–21 and the separate Run↔Interaction / Task↔Product / Project↔Task findings remain open. Status remains **Changes Required / Audit In Progress**; no code fix, PR merge or Phase 0 authorization.
