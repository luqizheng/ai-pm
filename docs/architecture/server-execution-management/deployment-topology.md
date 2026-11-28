# Deployment Topology — SEM-02E

> **HISTORICAL — SUPERSEDED, NOT AN IMPLEMENTATION CONTRACT (2026-11-25):** This AI-PM-side SEM design copy records the former Platform Claim/Lease/Run/Task/HIR/Barrier control-plane proposal. It is retained for source-audit and migration traceability only. Do not implement its SEM business orchestration, database ownership, state machines or API flows. The current bilateral boundary and active SEM protocol drafts are indexed in [the AI-PM SEM integration review](./README.md) and maintained canonically in [luqizheng/smark-sem](https://github.com/luqizheng/smark-sem).


> **Authority-boundary revision (2026-11-25):** This document is **under authority-boundary revision**. SEM remains independently deployed and owns Runner connectivity/runtime infrastructure, but the former SEM scheduler/control coordinator acting as a Platform Claim/Lease/business-command proxy is superseded by [authority-boundary-decision.md](./authority-boundary-decision.md). Harness business context and Task mutations are exclusively through AI-PM MCP.


- Status: Review Pending. Normative baseline: [SEM-02E](./sem-02e-architecture-convergence.md).

## Deployment units

1. **Platform**: API, business workers, Platform PostgreSQL, Transactional Outbox. Owns Project/Task/Run/Lease/Generation/Interaction/HIR/Human Consultation/notification/worker_command.
2. **SEM**: independent service on its own Docker host/deployment unit, own PostgreSQL, HTTPS service API, Runner WSS endpoint, scheduler/control coordinator, process supervisor/adapters, Workspace runtime volume. SEM is not merely an optional Platform Worker role and must not connect to Platform PostgreSQL.
3. **Client Runner**: user device agent, connects outbound to SEM over authenticated WSS, supervises local Harness processes and durable evidence spool.
4. **Harness**: Codex/other CLI. Business context and tools remain via Platform MCP; control-plane Claim/command polling/ACK/recovery are not via Platform MCP.

## Data and network paths

- Platform Outbox → HTTPS → SEM Inbox; SEM → Platform versioned HTTPS API; Runner ↔ SEM WSS.
- Platform and SEM databases are independent. No shared DB and no cross-service SQL.
- SEM dispatches only after Platform Claim succeeds.
- Workspace/Worktree/Git Mirror are temporary/rebuildable runtime assets, not backup or business authority.
- SEM local state includes connection epochs, process observations, delivery ledgers, dispatch operations, resource snapshots and local recovery state.

## RabbitMQ environment fact

RabbitMQ already exists as an independent single-instance service with no cluster. This topology does not add another broker and does not assume HA. SEM safety-critical paths do not depend on RabbitMQ availability. Optional non-authoritative wake-up/notification acceleration requires separate approval and must fall back to DB Outbox/Inbox/API reconciliation.

## Availability and failure

RabbitMQ outage must not block Claim authority, Lease renewal, Barrier, HIR, E3 evidence or recovery. SEM outage prevents new dispatch; Platform remains authoritative. Runner WSS loss produces Detached/reconciliation, not automatic process death. Platform API loss must not cause SEM to invent Claim/Lease state.

## Deployment gates

- SEM has no Platform DB credential.
- Separate SEM DB initialization from empty database.
- Runner ticket, service JWT, TLS and revocation are configured.
- SEM readiness requires DB/API/auth dependencies; no dispatch before ready.
- Workspace volume is persistent during execution but not backed up.
