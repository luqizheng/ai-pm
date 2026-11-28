# Service Protocol, Security and Persistence — SEM-02E

> **HISTORICAL — SUPERSEDED, NOT AN IMPLEMENTATION CONTRACT (2026-11-25):** This AI-PM-side SEM design copy records the former Platform Claim/Lease/Run/Task/HIR/Barrier control-plane proposal. It is retained for source-audit and migration traceability only. Do not implement its SEM business orchestration, database ownership, state machines or API flows. The current bilateral boundary and active SEM protocol drafts are indexed in [the AI-PM SEM integration review](./README.md) and maintained canonically in [luqizheng/smark-sem](https://github.com/luqizheng/smark-sem).


> **Authority-boundary revision (2026-11-25):** This document's former SEM→Platform Claim/Lease/business-state API path is **SUPERSEDED** by [authority-boundary-decision.md](./authority-boundary-decision.md). Independent service authentication, TLS, idempotency, durable message delivery and no-shared-database principles remain applicable; endpoint scopes and persistence entities must be redesigned around opaque execution messages and infrastructure observations.


- Status: Review Pending alignment.
- Normative baseline: [SEM-02E](./sem-02e-architecture-convergence.md).

## Service boundary
Platform and SEM are independent services with independent databases. SEM never connects to Platform PostgreSQL. Platform Outbox → versioned HTTPS → SEM Inbox; SEM calls Platform HTTPS for Claim, Lease renewal, status/evidence and reconciliation. Runner connects to SEM by authenticated WSS. Harness business operations remain Platform MCP.

## API and authentication
Single candidate API prefix: `/internal/sem/v1/`. Use independent short-lived asymmetric service JWT, TLS, strict issuer/audience/scope/jti/kid, rotation and revocation. Runner uses short-lived single-use device-bound WSS ticket. No service token is resolved as a human principal.

## SEM private persistence
Seven minimum tables remain: inbox_event, runner_connection, runner_ticket_use, dispatch_operation, command_delivery, platform_delivery, process_observation. These do not copy Platform Run/Lease/Generation/HIR/worker_command authority. Resource snapshots use runner_connection; reservation uses dispatch_operation.

## Idempotency
Persist dispatch intent before Platform Claim; stable claim_request_id; Platform Claim and idempotency outcome in one transaction. Unknown response retries same key and same target Runner. SEM never persists plaintext Lease Token. Inbox acknowledges only after durable commit.

## Process evidence
Use runner.process.exit, runner.process.reconciled, runner.reconnect_sync and continuation.consumed. Process exit is bound to Run+Generation+process_sequence+boot_id+OS process identity; command_id optional for natural exits. Durable local spool remains until SEM durable ACK. Late evidence only affects its own sequence.

## RabbitMQ
Existing independent single-instance RabbitMQ has no cluster. It is not required for service protocol correctness and is not the authoritative delivery path. Optional wake-up only after approval; DB Outbox/Inbox and HTTPS reconciliation remain mandatory. Broker ACK is never an application ACK, Claim commit or E3.
