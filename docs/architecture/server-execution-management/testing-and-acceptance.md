# SEM Testing and Acceptance — SEM-02E

- Status: Review Pending. Normative baseline: [SEM-02E](./sem-02e-architecture-convergence.md).

## Phase 0 mandatory tests
1. Active process + expired Lease never creates a second process.
2. Platform DB-only orphan scanner detects candidates without Worker restart.
3. Runner project candidates are scanned; no candidate exclusion bypass.
4. Barrier created atomically with HIR/audit/notification Outbox; no intermediate queued state.
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
