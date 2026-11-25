# SEM Code Contract Mapping — SEM-02E

- Status: **Implementation mapping pending re-audit**.
- Architecture baseline: [SEM-02E](./sem-02e-architecture-convergence.md).
- Historical code references must be revalidated against the implementation HEAD before Phase 0; line numbers from older snapshots are not current facts.

## Platform-owned contracts
- Task/Run/Lease/Generation and Claim transaction.
- Task-scoped Execution Barrier.
- Unified transaction-scoped dispatchability predicate.
- Periodic DB-only orphan scanner.
- Existing governance table extensions: human_intervention, human_consultation, notification.
- worker_command and audit.
- All E6 authorization and unique successor allocation.

## SEM-owned contracts
- SEM DB and migrations.
- Platform HTTPS client/server boundary; no Platform PgPool or DATABASE_URL in SEM.
- Runner WSS, ticket validation, connection epoch.
- Dispatch operation, command delivery, process observation and resource reservation.
- Local CodexExecutor Execution Control adapter.
- Detached/reconnect state and evidence spool ingestion.

## Runner-owned contracts
- OS process supervision and process identity.
- Run+Generation process_sequence.
- Durable evidence spool before WSS send; delete only after SEM durable ACK.
- continuation.consumed after response injection and new process start.
- Resource report is not Lease renewal or termination evidence.

## Mandatory code inventory before implementation
Re-audit all Run creation sites, claim/recovery/requeue paths, B06-D3, worker startup/recovery, SEM-local terminate/poll, Runner terminate/continue, shield checks, HIR routes, notification creation, migrations and all PgPool use. Map each SQL operation to Platform or SEM ownership.

## Current evidence status
The prior review identified P0 gaps in periodic orphan discovery, Task Barrier scope, process identity sequence and SEM-local waiting recovery. This file does not claim those gaps are fixed in code. Phase 0 remains Not Started.
