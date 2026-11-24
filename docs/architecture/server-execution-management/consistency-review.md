# SEM V1 Architecture Consistency Review

- Review stage: SEM-02B document-set review.
- Result: **Document set expanded and major user decisions synchronized; architecture is not yet marked Design Complete.**
- Scope: documentation-only review. No business code, migrations, Docker, Compose, Worker loops, API or Runner implementation changed.

## 1. Cross-document invariants checked

The following rules are now stated consistently across the overview, decision record, safety, topology, domain, protocol, state-machine, recovery, security, operations, testing and roadmap documents:

1. PostgreSQL is the authority for business state, lease ownership and generation.
2. SEM owns server-side process/workspace lifecycle; Platform retains business orchestration and outbox consumption.
3. Lease expiry is not process-death evidence.
4. Only E1/E2/E3 permit automatic reclaim; E4 permits isolation/HIR only; E5 remains uncertain and is never automatically reclaimed.
5. Scheduling Shield is durable and must be checked by all claim/requeue/recovery paths.
6. Generation fencing does not prevent external Git/network side effects.
7. Runner heartbeat proves supervisor liveness only, not Harness activity or human presence.
8. Runner defaults to unattended; supervised requires explicit configuration and pre-claim user confirmation.
9. HIR is an independent entity and does not replace task_interaction.
10. Workspace/Worktree/Git Mirror are not backed up; runtime persistence is distinct from backup.
11. Each Repository has one authoritative origin; SEM does not implement multi-origin synchronization.
12. Runner handoff uses normal claim, generation increment and fresh token; lease_client_id checks are not weakened.
13. Harness Resume remains disabled until exact-version real-world acceptance.

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
| G-08 Deployment | Independent SEM host can access PostgreSQL and runtime volume; startup/readiness gates prevent claim when prerequisites fail |
| G-09 Documentation/code alignment | All cited code anchors and current behavior are rechecked against the implementation HEAD immediately before Phase 0 |

## 3. Known design details still requiring code-level reconciliation

1. The exact physical representation of Scheduling Shield (dedicated columns/table versus a carefully constrained existing field) must be chosen only after tracing every candidate and recovery query. The invariant is fixed; physical schema is not yet frozen.
2. The exact HIR physical schema and API surface are target designs, not existing endpoints.
3. The current Runner command poll/ack implementation must be traced to prove it can carry E3 evidence securely. If not, document the minimal protocol extension before implementation.
4. The current executor credential path must be inventoried per Harness; do not assume every credential can be revoked while a child process is running.
5. OS-specific process identity and termination observation must be verified on supported SEM host OS/runtime.
6. Exact Harness Resume behavior is an empirical gate, not a paper-only capability flag.

## 4. Explicit non-goals

- No Workspace, Worktree or Git Mirror backup/snapshot system.
- No RPO/RTO for those ephemeral execution assets.
- No multi-origin Git synchronization in SEM.
- No new message broker.
- No new SEM microservice or binary is implied by this document set.
- No code implementation is authorized by this review.

## 5. Exit criteria for SEM-02C

SEM-02C may mark the architecture Design Complete only after:
- every design decision has a single authoritative definition;
- state machines and protocol contracts agree on all transition guards;
- physical DB schema and transaction boundaries are specified sufficiently for implementation;
- code-level reconciliation gates above have evidence or are explicitly deferred to a named Phase acceptance test;
- every Phase has bounded scope, test plan, acceptance and rollback;
- no unresolved product-level choice remains.

Until then README status must remain Architecture Review / Design In Progress, not Design Complete.
