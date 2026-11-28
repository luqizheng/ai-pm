# SEM Security Model — SEM-02E

> **HISTORICAL — SUPERSEDED, NOT AN IMPLEMENTATION CONTRACT (2026-11-25):** This AI-PM-side SEM design copy records the former Platform Claim/Lease/Run/Task/HIR/Barrier control-plane proposal. It is retained for source-audit and migration traceability only. Do not implement its SEM business orchestration, database ownership, state machines or API flows. The current bilateral boundary and active SEM protocol drafts are indexed in [the AI-PM SEM integration review](./README.md) and maintained canonically in [luqizheng/smark-sem](https://github.com/luqizheng/smark-sem).


- Status: Review Pending.
- Normative baseline: [SEM-02E](./sem-02e-architecture-convergence.md).

## Service identity and ownership

Platform↔SEM uses TLS and short-lived asymmetric service JWT with strict issuer/audience/scope/jti/kid. Runner obtains single-use device-bound WSS ticket. SEM has no Platform DB credentials. Service identity never maps to human principal.

## Execution authority

Only Platform grants Lease/Generation. SEM selects Runner but cannot self-grant execution. Runner receives execution authorization only after successful Platform Claim. All Run creation/Claim/recovery/requeue paths check Task Barrier and Interaction shield under Platform transaction locks.

## Process evidence

E3 requires authenticated Runner Supervisor OS observation bound to registered process_sequence, boot_id, PID and creation identity. Local spool persists until SEM durable ACK. command receipt, command result, E3 and continuation.consumed are separate. Lease expiry does not block reporting historical process evidence, but stale evidence cannot mutate new generations.

## E6 authorization

E6 requires dedicated permission, explicit second confirmation, immutable Human Consultation record, reason, residual risk, mitigation, exact HIR/Task Barrier and unique successor. It is not E3 and does not release old Barrier.

## Credentials and external effects

Git/SSH/API/LLM credentials must be least privilege, scoped, short-lived and redacted. Generation fencing does not revoke credentials already held by a process. Phase 0 includes credential injection/inheritance/revocation inventory and tests.

## RabbitMQ

Existing RabbitMQ is a standalone non-clustered service. Do not place security authority, Claim, Lease, E3, Barrier release or HIR decision on broker delivery. If used for optional wake-up, authenticate producers/consumers and treat all messages as hints; DB/API remains authority.
