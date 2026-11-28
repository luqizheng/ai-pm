# Failure Recovery — SEM-02E

> **HISTORICAL — SUPERSEDED, NOT AN IMPLEMENTATION CONTRACT (2026-11-25):** This AI-PM-side SEM design copy records the former Platform Claim/Lease/Run/Task/HIR/Barrier control-plane proposal. It is retained for source-audit and migration traceability only. Do not implement its SEM business orchestration, database ownership, state machines or API flows. The current bilateral boundary and active SEM protocol drafts are indexed in [the AI-PM SEM integration review](./README.md) and maintained canonically in [luqizheng/smark-sem](https://github.com/luqizheng/smark-sem).


- Status: Review Pending.
- Normative baseline: [SEM-02E](./sem-02e-architecture-convergence.md).

| Failure | Required behavior |
|---|---|
| Lease expired, process unknown | Platform periodic DB-only scanner creates/retains Task Barrier + HIR; no requeue |
| Worker does not restart | Periodic scanner still runs; startup reconciliation is supplemental |
| Runner WSS lost | Detached; SEM reconnect reconciliation; no inference of process death |
| Runner restarts | Durable spool replay + boot/process identity reconciliation; unknown → E5 |
| Workspace/PID/log volume lost after started | E5/Uncertain; no Done{Lost} automatic re-dispatch |
| Interaction continue starts new process | Register new process_sequence; validate current sequence on renewal |
| Old terminate evidence arrives late | Attribute to old process identity only; never close new sequence |
| SEM-local worker restarts during human wait | Restore awaiting-continue and Interaction shield; do not terminalize/requeue |
| RabbitMQ unavailable | No safety-state change; Outbox retry/DB reconciliation continues |
| Platform API unavailable | SEM cannot create/renew Claim authority; retain durable operation and retry same idempotency key |
| E6 accepted | One pre-bound successor; old Barrier remains risk_accepted_unconfirmed |
| Duplicate/out-of-order WSS | Idempotency + connection epoch + process identity sequence; stale message diagnostic only |

## Recovery ordering

1. Preserve process evidence locally before transmission.
2. Reconnect/authenticate Runner and validate boot/process identity.
3. Query authoritative Lease/Generation via Platform API.
4. Reconcile process sequence and outstanding commands.
5. Release Barrier only after Platform validates E1/E2/E3.
6. Otherwise quarantine and create/associate HIR.
7. Dispatch successor only through normal Platform Claim or exact E6 authorization.
