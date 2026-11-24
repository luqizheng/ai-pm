# SEM V1 Protocol Contracts

Status: normative design target. Exact route/function names must be reconciled with code before implementation.

## 1. Trust boundaries

- Platform API/MCP and Platform Workers coordinate business state through PostgreSQL.
- SEM Worker manages server-side process lifecycle and evidence through PostgreSQL.
- Client Runner uses the existing claim, heartbeat, command-poll/ack and result paths.
- Runner supervises its local Harness process through OS process APIs.
- Platform and SEM have no RPC coupling. Runner does not connect directly to the SEM host. No message broker is introduced.

## 2. Common execution envelope

Every execution-control message must be scoped to tenant/project/task as applicable, task_run_id, lease_generation, client/Runner identity, command/event ID, timestamp and idempotency key.

Stale generation, mismatched client identity or missing required scope is rejected. Duplicate delivery is idempotent.

## 3. Claim

A successful claim atomically:
1. verifies tenant/project/device authorization and candidate eligibility;
2. excludes Runs under Scheduling Shield;
3. serializes ownership using the existing claim transaction pattern;
4. increments generation;
5. issues a fresh lease token and stores only its verifier/hash;
6. binds the lease to the exact client identity;
7. records an audit event.

User selection of a Runner is not a claim and grants no execution ownership. The Runner may start only after a successful claim response.

## 4. Heartbeat

Heartbeat proves only that the Runner supervisor loop is alive:
- renew only for exact current owner, token proof and generation;
- never clear Scheduling Shield;
- stale heartbeat cannot resurrect an old generation;
- heartbeat does not prove Harness activity or user presence.

## 5. Cancellation and E3

Extend the existing Runner command poll/ack channel; do not add a second MCP tool solely for cancellation.

A cancel command carries command ID, Run ID, generation and target process identity.
- Command receipt acknowledgement is separate from termination confirmation.
- E3 is sent only after the Runner supervisor observes OS-level process exit through wait/handle result.
- E3 binds command ID, Run ID, generation, process identity, exit observation and timestamp.
- Duplicate E3 is idempotent.
- Mismatched generation/process identity is rejected.
- Timeout, offline Runner, malformed ACK or missing exit observation is not E3; keep E4/E5 shield.

HTTP 2xx, successful polling, command receipt or request acceptance is never proof of process termination.

## 6. Result and error contract

Results are accepted only after checking current lease verifier, generation and client identity. Stale results may be retained as diagnostic evidence but cannot mutate authoritative task state. Secrets are redacted before persistence.

Distinct error categories include stale_generation, lease_owner_mismatch, lease_expired, scheduling_shield_active, run_not_claimable, process_identity_mismatch, termination_not_observed and authorization_denied. Map them to existing API conventions rather than inventing a parallel error system.

## 7. Versioning

Persist protocol version in command/event payloads. Unknown mandatory versions fail closed for destructive actions. Exact wire fields and status codes are implementation tasks requiring code-level reconciliation.
