# SEM Operational Runbook — SEM-02E

> **HISTORICAL — SUPERSEDED, NOT AN IMPLEMENTATION CONTRACT (2026-11-25):** This AI-PM-side SEM design copy records the former Platform Claim/Lease/Run/Task/HIR/Barrier control-plane proposal. It is retained for source-audit and migration traceability only. Do not implement its SEM business orchestration, database ownership, state machines or API flows. The current bilateral boundary and active SEM protocol drafts are indexed in [the AI-PM SEM integration review](./README.md) and maintained canonically in [luqizheng/smark-sem](https://github.com/luqizheng/smark-sem).


- Status: Review Pending. Normative baseline: [SEM-02E](./sem-02e-architecture-convergence.md).

## Service health
- Platform health: DB, business API, Outbox consumer and periodic orphan scanner.
- SEM health: SEM DB, HTTPS service identity, WSS endpoint, dispatch/reconciliation loops, durable process evidence intake.
- Runner health: authenticated WSS, supervisor heartbeat, process supervision and evidence spool.
- RabbitMQ: existing standalone instance, no cluster. Its availability is informational for optional consumers only; SEM safety and recovery must remain operational without it.

## Orphan scanner
Verify Platform periodic scanner runs independently of worker restart. Monitor last successful scan, candidate count, Barrier/HIR creation, transaction conflicts and retry lag. Scanner failure raises operational alert; it must not fall back to unsafe requeue.

## HIR and Barrier
For each E4/E5, inspect Task Barrier, origin Run+Generation, HIR owner/recipient, evidence, notification state and residual external side effects. Do not manually delete Barrier or mark it released without Platform-validated E1/E2/E3. HIR closure is not Barrier release.

## Runner reconnect and spool
On reconnect, authenticate device, compare boot_id/process sequence, replay durable spool until SEM durable ACK, query Platform Lease/Generation via SEM API, and classify alive/exited/unknown. Unknown becomes E5/Quarantined. Never infer process death from missing Workspace files.

## E6
Require privileged operator, second confirmation, risk summary, mitigation, unique successor binding and audit. Verify old Barrier remains risk_accepted_unconfirmed. No second successor.

## RabbitMQ outage
Do not alter Run/Lease/Barrier/HIR. Confirm Outbox/Inbox retries and API reconciliation continue. Optional wake-up consumers may fall back to DB polling. Broker ACK is not business commit or E3.
