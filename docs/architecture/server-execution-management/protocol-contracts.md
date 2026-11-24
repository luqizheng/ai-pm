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
- SEM → Platform: target-Runner claim request, lease renewal, execution lifecycle updates, result/error reports, termination evidence and Runner/connection telemetry.
- SEM is the sole Runner dispatch/handoff coordinator. Runner does not poll Platform MCP for Claim, command delivery, ACK or recovery control.
- Platform remains the only authority that grants a Run lease or changes authoritative Run/Generation state. SEM cannot self-assign a Run; it requests a claim for a specifically authenticated target Runner.

### 2.2 Dispatch and recovery

1. Platform commits the business change and an Outbox event in the same database transaction.
2. Outbox delivery calls the versioned SEM HTTPS API. Delivery is at-least-once; each event has a stable event_id/idempotency key.
3. SEM durably records receipt before acknowledging acceptance; duplicate event delivery returns the prior outcome.
4. SEM selects an eligible Runner (or obtains explicit user selection for `supervised`) and requests Claim through Platform API with the authenticated target Runner/device identity. Platform independently validates device registration, tenant/project binding, authorization, supervision policy, Scheduling Shield and concurrency, then performs generation increment, lease issuance and audit in one authoritative transaction.
5. SEM sends the execution command and Platform-issued lease material to that exact Runner over its authenticated WSS connection only after successful Claim. Runner never calls Claim directly.
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

A successful claim is requested by SEM through the Platform HTTPS API and executed atomically inside Platform:
1. verifies tenant/project/device authorization and candidate eligibility;
2. excludes Runs under Scheduling Shield;
3. serializes ownership using the existing claim transaction pattern;
4. increments generation;
5. issues a fresh lease token and stores only its verifier/hash;
6. binds the lease to the exact client identity;
7. records an audit event.

User selection of a Runner is not a claim and grants no execution ownership. The Runner may start only after a successful claim response.

## 5. Runner liveness, Lease Renewal and Agent Activity

These are three distinct signals and must never be conflated:

1. **Runner Supervisor Heartbeat**: Runner sends over WSS to SEM. It proves only that the Runner supervisor loop is alive and maintains connection liveness. It does not itself prove Harness activity or human presence.
2. **Lease Renewal**: Runner Supervisor sends a dedicated renewal message over WSS. SEM forwards it to Platform HTTPS API. Platform alone validates the exact Runner/device owner, lease proof/token, Run and Generation, and commits renewal. This is the only Lease Renewal path for Client Runner execution.
3. **Agent Activity**: Harness/Agent reports progress through Platform MCP business tools. It records business/execution activity but does not renew the Lease.

Renewal constraints:
- renew only for exact current owner, token proof and generation;
- never clear Scheduling Shield;
- stale heartbeat/renewal cannot resurrect an old generation;
- WSS liveness alone never renews a Lease;
- Agent Activity never renews a Lease;
- no duplicate renewal path may remain in Platform MCP after migration.

## 6. Cancellation and E3

Carry cancellation commands and ACKs over the authenticated Runner↔SEM WSS channel. Runner does not poll Platform MCP for commands or ACKs. Platform retains `worker_command` business state; SEM stores only transport delivery facts. Do not use Platform database polling or add a separate MCP tool solely for cancellation.

A cancel command carries command ID, Run ID, generation and target process identity.
- Command receipt acknowledgement is separate from termination confirmation.
- E3 is sent only after the Runner supervisor observes OS-level process exit through wait/handle result.
- E3 binds command ID, Run ID, generation, process identity, exit observation and timestamp.
- Duplicate E3 is idempotent.
- Mismatched generation/process identity is rejected.
- Timeout, offline Runner, malformed ACK or missing exit observation is not E3; keep E4/E5 shield.

HTTP 2xx, successful polling, command receipt or request acceptance is never proof of process termination.

## 7. Result and error contract

Results are accepted by Platform only after checking current lease verifier, generation and client identity. SEM forwards evidence through the authenticated service API; it does not write Run state directly. Runner never writes Platform PostgreSQL or calls Platform database endpoints directly. Stale results may be retained as diagnostic evidence but cannot mutate authoritative task state. Secrets are redacted before persistence.

Distinct error categories include stale_generation, lease_owner_mismatch, lease_expired, scheduling_shield_active, run_not_claimable, process_identity_mismatch, termination_not_observed and authorization_denied. Map them to existing API conventions rather than inventing a parallel error system.

## 8. Versioning

Persist protocol version in command/event payloads. Unknown mandatory versions fail closed for destructive actions. Exact wire fields and status codes are implementation tasks requiring code-level reconciliation.


## 9. Runner control-channel ownership (ADR-SEM-20)

| Operation | Sole path | Authority |
|---|---|---|
| Runner selection / supervised user confirmation | SEM scheduler + SEM user-notification flow | User intent and SEM coordination; no execution right |
| Claim | SEM HTTPS → Platform Claim API, then WSS dispatch | Platform transaction grants Lease/Generation |
| Runner Supervisor Heartbeat | Runner → SEM WSS | SEM connection/liveness record only |
| Lease Renewal | Runner → SEM WSS → SEM HTTPS → Platform | Platform validates and commits renewal |
| Command delivery / command ACK / cancel / E3 | Platform command intent → SEM WSS → Runner; ACK returns WSS → SEM → Platform API | Platform command business state; SEM transport ledger; Runner OS process observation for E3 |
| Run/Task context, Agent Activity, Interaction, delivery/result | Harness → Platform MCP | Platform business application services |
| Recovery / handoff coordination | SEM scheduler + Platform API Claim/recovery operations + WSS | Platform owns lease/generation; HIR and business audit remain Platform |

After migration, remove Runner MCP polling/write paths for Claim, Lease Renewal, command polling/ACK and recovery. Do not retain duplicate control paths. Keep MCP tools needed by Harness for business operations.
