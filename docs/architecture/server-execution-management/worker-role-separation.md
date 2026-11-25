# Worker Role Separation — SEM-02E

> **HISTORICAL — SUPERSEDED, NOT AN IMPLEMENTATION CONTRACT (2026-11-25):** This AI-PM-side SEM design copy records the former Platform Claim/Lease/Run/Task/HIR/Barrier control-plane proposal. It is retained for source-audit and migration traceability only. Do not implement its SEM business orchestration, database ownership, state machines or API flows. The current bilateral boundary and active SEM protocol drafts are indexed in [the AI-PM SEM integration review](./README.md) and maintained canonically in [luqizheng/smark-sem](https://github.com/luqizheng/smark-sem).


- Status: Review Pending.
- Normative baseline: [SEM-02E](./sem-02e-architecture-convergence.md).

## Platform role

Platform owns all business DB state and DB-only loops: Run/Task transitions, Claim transactions, Lease/Generation, Interaction, HIR, Human Consultation, project notification Feed + recipient Inbox rows, worker_command, audit, and **periodic orphan scanner**. The scanner is DB-only and runs periodically on Platform; it must not depend on SEM, Session Manager, Runner availability or process polling.

## SEM role

SEM is an independent service, not a Platform Worker role that shares Platform PgPool. It owns Runner connection/WSS, scheduling coordination, dispatch operation ledger, process observation, local CodexExecutor adapter, Workspace lifecycle and resource admission. Every business mutation goes through Platform HTTPS API.

## Outbox

Platform Transactional Outbox has one consumer in Platform. Platform delivers versioned HTTPS events to SEM Inbox. SEM Inbox is idempotent and durably commits before acknowledging. SEM does not register Platform Outbox handlers and does not access Platform DB.

## Interaction adapters

Execution Control Port has two adapters:
- Runner adapter: Platform command intent → SEM command delivery → Runner WSS.
- SEM-local adapter: Platform command intent → SEM local command consumer → CodexExecutor.

Both must implement terminate, continue_with_response, process evidence and continuation.consumed. No consumer means visible failure, never fake delivery.

## Recovery and dispatch

Platform owns the single dispatchability predicate and Task Barrier. All new Run creation, Claim, retry/requeue, B3-1/B3-2, B06-D3, orphan scan, handoff and E6 successor pass through it under transaction locks. B06-D3 is folded into periodic orphan scan; no direct requeue path remains.

## RabbitMQ

Existing standalone RabbitMQ is not the authoritative Outbox consumer and is not required for worker correctness. Optional wake-up only after separate approval; DB polling/retry is the fallback.
