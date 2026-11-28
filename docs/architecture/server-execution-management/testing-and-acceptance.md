# SEM Testing and Acceptance — SEM-02E

> **HISTORICAL — SUPERSEDED, NOT AN IMPLEMENTATION CONTRACT (2026-11-25):** This AI-PM-side SEM design copy records the former Platform Claim/Lease/Run/Task/HIR/Barrier control-plane proposal. It is retained for source-audit and migration traceability only. Do not implement its SEM business orchestration, database ownership, state machines or API flows. The current bilateral boundary and active SEM protocol drafts are indexed in [the AI-PM SEM integration review](./README.md) and maintained canonically in [luqizheng/smark-sem](https://github.com/luqizheng/smark-sem).


- Status: Review Pending. Normative baseline: [SEM-02E](./sem-02e-architecture-convergence.md).

## Phase 0 mandatory tests
1. Active process + expired Lease never creates a second process.
2. Platform DB-only orphan scanner detects candidates without Worker restart.
3. Runner project candidates are scanned; no candidate exclusion bypass.
4. Barrier created atomically with HIR/audit/project Feed notification/recipient Inbox rows; no intermediate queued state.
5. New Run row cannot bypass Task Barrier.
6. Concurrent Claim, requeue, scanner and Run creation serialize correctly.
7. Interaction terminate/continue works for Runner and SEM-local CodexExecutor.
8. SEM-local Worker restart during awaiting-interaction/awaiting-continue does not terminalize or requeue.
9. Workspace/PID/log loss after started → E5; no Done{Lost} dispatch.
10. E6 binds exactly one successor and retains old Barrier.
11. Existing governance table constraints and all routes/consumers remain correct after reuse/extension.
12. Empty database migration initializes all required schema.

## Phase 1/2 tests
- Runner process spool: natural exit, cancel, lease-loss cleanup, startup orphan cleanup, shutdown, boot transition.
- Lease expiry does not block historical evidence upload.
- Reconnect same device; wrong boot/process identity; stale generation; duplicate/out-of-order spool.
- Interaction continue creates new process_sequence; late old exit cannot close new process.
- Full independent SEM deployment with no Platform DB credentials.
- API idempotency, WSS ticket single use, revocation, Outbox/Inbox duplicate delivery and recovery.

## RabbitMQ
- Stop the existing single-instance RabbitMQ service during dispatch, recovery and HIR notification.
- Assert Claim/Lease/Barrier/E3 correctness unaffected.
- Optional wake-up consumer falls back to DB polling; duplicate/late broker messages are harmless.
- Never treat broker ACK as business commit.

## Chaos acceptance
Platform/SEM/Runner restart, network partition, clock skew, duplicate events, process PID reuse, missing volume, resource pressure, key revocation and RabbitMQ outage. Assert no dual execution and complete audit trail.


## SEM-02F additional mandatory tests
- `human_intervention` action records continue to satisfy old action constraints; HIR records satisfy the HIR-only discriminator and seven-state transition rules.
- Parent `integration.notification.read_at` and all existing project-feed producers/readers remain unchanged; recipient Inbox read state is per principal.
- Task assignee → owner fallback → unassigned recipient resolution is deterministic and auditable.
- E6 consultation has one Barrier and one unique successor; the successor is non-dispatchable before commit and only exact successor is allowed after commit.
- `task_run_process` rejects duplicate sequence with a conflicting process identity; renewal from a stale sequence fails after continuation.
- Natural process exit without command_id is accepted for a fully registered process identity; late exit cannot close a newer sequence.
- Platform DB scanner runs periodically while worker remains alive and does not call SEM, Runner, CodexExecutor or RabbitMQ.
