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


## Current-master verification refresh — 2026-11-25

The latest master code confirms that `reconcile_orphans` is called once at startup in `apps/ai-pm-worker/src/main.rs`; the dispatch loop has no periodic call. The current `reconcile_orphans` still contains B06-D3 direct `running → queued` behavior after a one-hour lease threshold. This must be replaced, not merely wrapped in a timer.

The current claim path locks candidate Run then Task and checks `candidate_shield` only in the expired-claimed recovery branch; the normal queued path needs the same Task Barrier + Interaction shield predicate. `insert_task_run` is a shared creation helper but all direct Run INSERTs still require a repository-wide audit.

Current Interaction creation writes task_interaction, shield, Task waiting, terminate command and audit in one transaction, but no notification. Current project Feed is `integration.notification`; `supervision/feed.rs` reads it and multiple producers write it. SEM-02F therefore preserves this parent Feed and adds recipient rows instead of changing the parent read semantics.

The code evidence baseline is master `4bc78620f54440a21a113cf7cdb14964d1a14dcb`. No business code was modified by this review.
