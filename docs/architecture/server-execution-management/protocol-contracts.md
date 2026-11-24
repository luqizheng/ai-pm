# SEM V1 Protocol Contracts

Status: normative design target. Exact route/function names must be reconciled with code before implementation.

## 1. Trust boundaries

- Platform and SEM are independent services and own separate databases.
- Platform PostgreSQL is authoritative for Project/Task/Run/Lease/Generation, business status, authorization and audit. SEM must not connect to or issue SQL against Platform PostgreSQL.
- SEM owns only execution-side private persistence: durable command-delivery records, connection/session metadata and local execution coordination. These records must not become a second Run/Lease authority.
- Platform ↔ SEM communication uses versioned HTTPS service APIs. Platform Transactional Outbox reliably triggers dispatch; SEM processes requests idempotently and reconciles through Platform APIs.
- Runner ↔ SEM communication uses authenticated WSS. Runner supervises its local Harness process through OS process APIs.
- No shared database and no message broker in V1.

## 2. Platform–SEM service API

### 2.1 Direction and responsibilities

- Platform → SEM: dispatch notification, cancellation request, policy/configuration changes, and authorized control requests.
- SEM → Platform: claim request, execution lifecycle updates, result/error reports, termination evidence, Runner/connection telemetry and HIR-related execution evidence.
- Platform remains the only authority that grants a Run lease or changes the authoritative Run/Generation state. SEM cannot self-assign a Run.

### 2.2 Dispatch and recovery

1. Platform commits the business change and an Outbox event in the same database transaction.
2. Outbox delivery calls the versioned SEM HTTPS API. Delivery is at-least-once; each event has a stable event_id/idempotency key.
3. SEM durably records receipt before acknowledging acceptance; duplicate event delivery returns the prior outcome.
4. SEM requests claim through Platform API. Platform performs authorization, eligibility checks, concurrency control, generation increment, lease issuance and audit in one authoritative transaction.
5. SEM sends the execution command to the selected Runner over WSS only after successful claim.
6. SEM reports state and evidence through Platform APIs. On timeout/restart, SEM retries idempotently and periodically reconciles outstanding operations with Platform.
7. Outbox notification is a wake-up/trigger, not the task payload authority. API query/reconciliation is the recovery path for missed or delayed notifications.

### 2.3 API contract requirements

- Version prefix (for example `/internal/sem/v1/`); exact routes are finalized during SEM-02C code mapping.
- Service-to-service authentication, TLS, least-privilege scopes, request correlation ID, idempotency key, timeout and bounded retry policy.
- Mutating calls carry Run ID and expected generation where applicable; stale generation and mismatched ownership fail closed.
- No distributed transaction across Platform and SEM databases. Use outbox/inbox, idempotency, explicit operation state and reconciliation/compensation.
- API acceptance is not proof of process start or process termination; process evidence has separate semantics.

## 3. Common execution envelope

Every execution-control message must be scoped to tenant/project/task as applicable, task_run_id, lease_generation, client/Runner identity, command/event ID, timestamp and idempotency key.

Stale generation, mismatched client identity or missing required scope is rejected. Duplicate delivery is idempotent.

## 4. Claim

A successful claim atomically:
1. verifies tenant/project/device authorization and candidate eligibility;
2. excludes Runs under Scheduling Shield;
3. serializes ownership using the existing claim transaction pattern;
4. increments generation;
5. issues a fresh lease token and stores only its verifier/hash;
6. binds the lease to the exact client identity;
7. records an audit event.

User selection of a Runner is not a claim and grants no execution ownership. The Runner may start only after a successful claim response.

## 5. Heartbeat

Heartbeat proves only that the Runner supervisor loop is alive:
- renew only for exact current owner, token proof and generation;
- never clear Scheduling Shield;
- stale heartbeat cannot resurrect an old generation;
- heartbeat does not prove Harness activity or user presence.

## 6. Cancellation and E3

Extend the existing Runner command poll/ack channel; do not add a second MCP tool solely for cancellation.

A cancel command carries command ID, Run ID, generation and target process identity.
- Command receipt acknowledgement is separate from termination confirmation.
- E3 is sent only after the Runner supervisor observes OS-level process exit through wait/handle result.
- E3 binds command ID, Run ID, generation, process identity, exit observation and timestamp.
- Duplicate E3 is idempotent.
- Mismatched generation/process identity is rejected.
- Timeout, offline Runner, malformed ACK or missing exit observation is not E3; keep E4/E5 shield.

HTTP 2xx, successful polling, command receipt or request acceptance is never proof of process termination.

## 7. Result and error contract

Results are accepted by Platform only after checking current lease verifier, generation and client identity. SEM forwards evidence through the authenticated service API; it does not write Run state directly. Stale results may be retained as diagnostic evidence but cannot mutate authoritative task state. Secrets are redacted before persistence.

Distinct error categories include stale_generation, lease_owner_mismatch, lease_expired, scheduling_shield_active, run_not_claimable, process_identity_mismatch, termination_not_observed and authorization_denied. Map them to existing API conventions rather than inventing a parallel error system.

## 8. Versioning

Persist protocol version in command/event payloads. Unknown mandatory versions fail closed for destructive actions. Exact wire fields and status codes are implementation tasks requiring code-level reconciliation.
