# SEM V1 Architecture Consistency Review

- Review stage: SEM-02C code-contract mapping (initial pass).
- Result: **Initial code-contract map added; major service-boundary gaps and Runner/MCP/WSS overlap identified. Architecture is not yet marked Design Complete.**
- Scope: documentation-only review. No business code, migrations, Docker, Compose, Worker loops, API or Runner implementation changed.

## 1. Cross-document invariants checked

The following rules are now stated consistently across the overview, decision record, safety, topology, domain, protocol, state-machine, recovery, security, operations, testing and roadmap documents:

1. Platform PostgreSQL is authoritative for business state, Run lifecycle, Lease and Generation; SEM cannot access it directly.
2. Platform and SEM own separate databases and communicate through versioned HTTPS APIs; Platform Outbox provides durable dispatch triggers, SEM Inbox is idempotent, and reconciliation repairs missed delivery.
3. SEM owns server-side process/workspace lifecycle and SEM-private execution records; Platform retains business orchestration and outbox consumption.
4. Lease expiry is not process-death evidence.
5. Only E1/E2/E3 permit automatic reclaim; E4 permits isolation/HIR only; E5 remains uncertain and is never automatically reclaimed.
6. Scheduling Shield is durable and must be checked by all claim/requeue/recovery paths.
7. Generation fencing does not prevent external Git/network side effects.
8. Runner heartbeat proves supervisor liveness only, not Harness activity or human presence.
9. Runner defaults to unattended; supervised requires explicit configuration and pre-claim user confirmation.
10. HIR is an independent entity and does not replace task_interaction.
11. Workspace/Worktree/Git Mirror are not backed up; runtime persistence is distinct from backup.
12. Each Repository has one authoritative origin; SEM does not implement multi-origin synchronization.
13. Runner handoff uses normal claim, generation increment and fresh token; lease_client_id checks are not weakened.
14. Harness Resume remains disabled until exact-version real-world acceptance.
15. SEM is the sole Runner dispatch/handoff coordinator; Runner does not poll Platform MCP for Claim/control commands. Platform alone grants Lease/Generation.
16. Runner Supervisor Heartbeat, Lease Renewal and Harness Agent Activity are separate signals. Proposed Lease Renewal path is Runner Supervisor WSS → SEM → Platform HTTPS; WSS liveness and Harness Agent Activity do not renew. This remains a proposed baseline pending user confirmation.

## 2. Required pre-implementation closure gates

These are implementation-design/verification gates, not reasons to retain compatibility code:

| Gate | Required evidence before the relevant Phase can pass |
|---|---|
| G-01 Scheduling Shield coverage | Code-level inventory proves every claim, B3-2, B06-D3, orphan reconciliation and retry/requeue path checks the same durable exclusion invariant |
| G-02 Process identity | E1/E2 implementation distinguishes the original process from PID reuse; process identity includes OS-supported discriminators beyond PID where available |
| G-03 E3 proof | Runner supervisor waits on the target OS process and emits authenticated Run/generation/command/process-bound exit evidence; command receipt is separate |
| G-04 Credential controls | Actual Harness credential injection, revocation, scope and redaction are verified; DB token rotation is not represented as revocation of Git/API credentials |
| G-05 HIR schema | Exact FK/tenant constraints, active-HIR uniqueness, concurrency versioning, indexes and audit transaction boundaries are designed before Phase 4 migration |
| G-06 Session Resume | Exact Harness and CLI version passes a repeatable real process/session restoration test; otherwise supports_resume remains false |
| G-07 Empty database | Full migration chain initializes all SEM schema and constraints from an empty database |
| G-08 Deployment | Independent SEM host can access Platform HTTPS API, its own SEM database and runtime volume; startup/readiness gates prevent dispatch when prerequisites fail |
| G-09 Documentation/code alignment | All cited code anchors and current behavior are rechecked against the implementation HEAD immediately before Phase 0 |
| G-10 Service API boundary | Route/version/auth schemas, service identity, idempotency, timeout/retry, Outbox→SEM inbox delivery, reconciliation and failure semantics are specified; SEM has no Platform DB credentials |
| G-11 Data ownership | Every persisted field/table has one owning service; Run/Lease/Generation writes remain Platform-only; SEM DB contains execution-private records only |
| G-12 Cross-service failure handling | No distributed transaction; dispatch loss, duplicate delivery, API outage, SEM restart and partial success have deterministic retry/reconciliation behavior |
| G-13 Runner channel ownership | Every current Runner MCP operation is assigned to exactly one path (Platform MCP or SEM WSS); no duplicate Claim, Lease Renewal, command delivery or ACK authority remains |
| G-14 Service and Runner authentication | Platform–SEM service identity and Runner WSS identity/connection revocation are defined; service tokens do not enter ordinary human-principal flow; cpk device binding is preserved |
| G-15 Runtime/data ownership mapping | Every existing PgPool/SQL operation in run_executor, claim, lease, worker_command, run_events and cleanup is assigned to Platform API or SEM-local process state; SEM has no Platform DB pool |
| G-16 Runner control-channel exclusivity | Integration-level operation inventory proves Runner cannot Claim, renew Lease, poll/ACK commands or perform recovery through Platform MCP; each control operation has exactly one WSS→SEM→Platform API path; Harness business MCP remains available |
| G-17 Target Runner authorization | Platform Claim API validates the target Runner identity against device registration, tenant/project binding, authorization and supervision policy; SEM cannot claim for an arbitrary/self-reported device |
| G-18 Lease renewal separation | **Design closed by ADR-SEM-21**: Runner Supervisor WSS → SEM → Platform HTTPS is the only path; implementation tests must prove Heartbeat/Agent Activity cannot renew and only the exact current generation is renewed |
| G-19 Service identity and scope | **Design closed by ADR-SEM-22**: Platform↔SEM independent service identities, JWT issuer/audience/scope/key rotation/revocation and idempotency; implementation tests must prove service identities never map to human principals |
| G-20 Runner ticket and revocation | **Design closed by ADR-SEM-23**: device-bound, short-lived, single-use WSS ticket; revocation events plus bounded revalidation close active connections; implementation tests must prove fail-closed behavior |
| G-21 SEM persistence and dispatch idempotency | **Design closed by ADR-SEM-24/25**: seven SEM-private tables and single ownership; lost Claim response recovers by same idempotency key without duplicate Claim or plaintext token persistence; implementation/fault-injection evidence remains required |

## 3. Initial SEM-02C code findings

1. Current ai-pm-worker creates a Platform PgPool from DATABASE_URL and runs Outbox plus Run execution loops in one process; there is no SEM role/DB boundary in current code.
2. Current RunExecutor trait abstracts only local process start/poll/terminate. run_executor and claim/lease/command/event modules still execute direct Platform SQL.
3. Current Runner uses Streamable HTTP MCP to Platform for Claim, Lease Renewal, command polling/ACK, recovery and interaction reporting; Harness receives AI_PM_MCP_URL and Lease Token directly. No WSS client/listener exists.
4. Current pm-api mounts PmMcpServer with a Platform PgPool. Current auth explicitly rejects roles=[service] on ordinary business endpoints; cpk credentials are accepted only on /mcp.
5. Current reconcile_orphans contains a >1h Lease-expiry path that changes a running Run back to queued and releases the Task without first proving process death. This remains a Phase 0 blocker.

## 4. ADR-SEM-20 decisions now frozen

The following decisions are closed: (a) Claim is requested by SEM for a specific target Runner; (b) Runner control operations use WSS, while Harness business operations retain Platform MCP; (c) Runner does not poll Platform MCP for Claim, commands, ACK or recovery. Platform remains the only Lease/Generation authority. These decisions require implementation and acceptance evidence; they are not current-code facts.

Lease Renewal via Runner Supervisor WSS → SEM → Platform HTTPS is a proposed baseline, not yet a user-ratified decision. Service JWT authentication, WSS connect-ticket/revocation and SEM DB schema are likewise proposals pending review.

## 5. Known design details still requiring code-level reconciliation

1. The exact physical representation of Scheduling Shield (dedicated columns/table versus a carefully constrained existing field) must be chosen only after tracing every candidate and recovery query. The invariant is fixed; physical schema is not yet frozen.
2. The exact HIR physical schema and API surface are target designs, not existing endpoints.
3. The Runner WSS command/ACK implementation must be traced to prove it can carry authenticated E3 termination evidence bound to Run, generation, command and process identity. Existing HTTP poll/ack paths must not remain an alternate authority channel unless explicitly retained as a documented fallback.
4. The current executor credential path must be inventoried per Harness; do not assume every credential can be revoked while a child process is running.
5. OS-specific process identity and termination observation must be verified on supported SEM host OS/runtime.
6. Exact Harness Resume behavior is an empirical gate, not a paper-only capability flag.

## 6. Explicit non-goals

- No Workspace, Worktree or Git Mirror backup/snapshot system.
- No RPO/RTO for those ephemeral execution assets.
- No multi-origin Git synchronization in SEM.
- No new message broker in V1; HTTPS API + Transactional Outbox is the initial integration pattern.
- No shared Platform/SEM database and no SEM direct SQL access to Platform PostgreSQL.
- No new SEM microservice or binary is implied by this document set.
- No code implementation is authorized by this review.

## 7. Exit criteria for SEM-02C

SEM-02C may mark the architecture Design Complete only after:
- every design decision has a single authoritative definition;
- state machines and protocol contracts agree on all transition guards;
- physical DB schema and transaction boundaries are specified sufficiently for implementation;
- code-level reconciliation gates above have evidence or are explicitly deferred to a named Phase acceptance test;
- every Phase has bounded scope, test plan, acceptance and rollback;
- no unresolved product-level choice remains.

Until then README status must remain Architecture Review / Design In Progress, not Design Complete. The current code mapping is recorded in code-contract-mapping.md and is an initial pass, not final API or schema approval. ADR-SEM-20 closes Runner dispatch/control-channel ownership. User has now ratified ADR-SEM-21–25, closing the design decisions for G-18–G-21. Their implementation and acceptance evidence remains outstanding; design approval does not mean the code gates have passed.
