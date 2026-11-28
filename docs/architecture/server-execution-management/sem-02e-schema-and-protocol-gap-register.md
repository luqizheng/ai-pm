# SEM-02E — Schema and Protocol Gap Register

> **HISTORICAL — SUPERSEDED, NOT AN IMPLEMENTATION CONTRACT (2026-11-25):** This AI-PM-side SEM design copy records the former Platform Claim/Lease/Run/Task/HIR/Barrier control-plane proposal. It is retained for source-audit and migration traceability only. Do not implement its SEM business orchestration, database ownership, state machines or API flows. The current bilateral boundary and active SEM protocol drafts are indexed in [the AI-PM SEM integration review](./README.md) and maintained canonically in [luqizheng/smark-sem](https://github.com/luqizheng/smark-sem).


- Status: **Changes Required / Review Pending**.
- Evidence baseline: GitHub `master@6f317176c627886a0a616458aabced6f4da33f0b` (migration baseline and current application sources fetched read-only).
- This is a design-gap register, not a migration script and not implementation authorization.

## 1. Existing Platform schema facts

### 1.1 `governance.human_intervention`

The squashed migration baseline defines the current row with:

- `id`, `project_id`, `tenant_id`
- `intervention_type` CHECK limited to `veto`, `rollback`, `freeze`, `unfreeze`, `mode_switch`, `policy_change`, `rollback_failed`
- `target_proposal_id`, `target_command_id`
- `actor_id`, `reason`, `executed_at`, `created_at`

This is an executed intervention/action record, not an open HIR queue. The user-approved direction remains reuse/extension of this table, but a direct rename-in-place of its existing meaning would erase a useful distinction.

**Required design before migration:** define one aggregate/table contract that supports both (a) a HIR work-item lifecycle and (b) append-only intervention action history without confusing them. The preferred shape is to extend the existing table with an explicit record discriminator and nullable lifecycle/work-item columns while preserving current action columns and CHECK semantics for action records; alternatively add a clearly named child/action table only if a single-table discriminator cannot preserve constraints cleanly. Do not create a parallel HIR master table by default. Decide the exact discriminator, CHECK constraints, partial unique indexes, FK scope and append-only/audit rules before implementation.

HIR work-item minimum: task_id, run_id, generation, barrier_id, category/severity, status, owner/recipient, created/updated/closed actor/time, reason, evidence references, disposition and audit. HIR state must be governed by state-machines.md only.

### 1.2 `governance.human_consultation`

Migration `0046_human_consultation.sql` defines:
- status CHECK: `pending | in_progress | decided | cancelled`;
- decided completeness CHECK: decision_content + decided_by + decided_at;
- cancelled_reason requirement;
- `task_id` FK currently `ON DELETE SET NULL`;
- `supersedes_consultation_id` self-FK;
- `product_scope` JSONB and scope_note.

It is suitable as the formal human-decision record, but E6 requires additional typed scope and strong relational constraints: decision_kind (E6), task_id non-null for E6, HIR reference, Barrier reference, unique successor_run_id, residual-risk summary, mitigation, second-confirmation facts, and immutable authorization audit. The E6 row must only become an authorization when status=decided and the E6-specific completeness predicate passes. Preserve existing consultation semantics for non-E6 decisions.

The existing task FK's SET NULL behavior is insufficient for an E6 authorization anchor; E6-specific integrity must prevent deletion/nullification of its Task/Barrier/Successor anchors.

### 1.3 `integration.notification`

Migration baseline defines:
- `id`, `project_id`, `tenant_id`, `source_proposal_id`, `kind`, `payload`, `veto_window_until`, `read_at`, `created_at`;
- unique `source_proposal_id`;
- no explicit recipient, delivery status, attempt count, retry schedule, delivery channel or delivery idempotency key in this table.

This is a project feed, not yet a complete recipient delivery system. Preserve project feed and `read_at`; add explicit recipient semantics and a separate delivery ledger only if required by channel/retry cardinality. The authoritative notification intent must be committed with HIR/Interaction in the Platform transaction or through a transactional outbox row. Delivery failures cannot change HIR/Interaction business state. Missing recipient must be represented in an explicit project-level unassigned queue and observable alert, never silently discarded.

Before selecting whether to add a `notification_delivery` table, inspect all readers/writers and whether one notification can target multiple recipients/channels. Do not overload `read_at` as delivery status.

### 1.4 Existing cleanup ledger is not a process identity ledger

`delivery.task_run_cleanup` has primary key `run_id`, one `session`, cleanup status `pending/cleaning/completed/needs_human`, attempts and retry fields. It is a cleanup-workflow ledger, not a multi-process identity/termination evidence store. Do not repurpose it for Run+Generation process sequences.

## 2. Platform active process-sequence fence — unresolved P0

SEM DB may persist observed process identities, but Platform owns Lease Renewal and cannot query SEM DB. Platform must persist the accepted active process sequence/identity (either normalized process table or carefully scoped Run columns) and expose authenticated APIs for:

1. initial process registration;
2. continuation-consumed sequence advance;
3. Lease Renewal carrying and validating the active sequence;
4. append-only historical exit evidence, including old sequence evidence after continuation;
5. rejecting stale sequence renewals and ensuring late old exits cannot close the new active process.

This must be designed with transaction lock order, idempotency keys, generation fencing and exact FK/index constraints. The seven SEM tables do not solve Platform-side sequence fencing by themselves.

## 3. Barrier and E6 relational contract — unresolved P0

Task Barrier must be Platform-owned and uniquely active per Task. Exact schema must ensure:
- Barrier is anchored to Task and origin Run+Generation;
- at most one unreleased Barrier per Task (including `risk_accepted_unconfirmed`);
- all Run creation and dispatch paths lock/check the Task Barrier;
- E6 consultation references exactly one Barrier/HIR/Task and one preallocated successor Run;
- successor_run_id unique across E6 authorizations;
- authorization and successor reservation are one Platform transaction;
- E6 does not release Barrier or create termination evidence.

The exact partial unique index, lock order and how a reserved successor is represented without becoming claimable before E6 commit remain OPEN.

## 4. Required code and consumer audit before Phase 0

The following are mandatory, not post-implementation cleanup:
- every read/write path for human_intervention, human_consultation and notification;
- every notification producer/consumer and current read_at semantics;
- every Run creation, claim, requeue, retry, B06-D3 and orphan-reconciliation entry point;
- every Lease Renewal entry point and its process identity checks;
- Runner command poll/ACK/continue and SEM-local CodexExecutor interaction path;
- Worker startup and periodic loop scheduling;
- existing DB lock ordering and all relevant partial unique indexes.

## 5. Decision status

User decisions retained:
- reuse/extend the three existing governance/notification tables rather than create parallel master entities;
- Task-scoped Barrier;
- E6 as proposed amendment to ADR-SEM-04/10;
- SEM remains an independent service;
- RabbitMQ is an existing independent single-instance service without clustering and is not a safety authority.

Still OPEN for final freeze:
- executable DDL for HIR discriminator, E6 typed constraints, Notification Recipient, Task Barrier and process registry;
- exact unique indexes, FK actions, trigger/immutability rules and global lock order;
- exact OpenAPI/DTO schemas and idempotency contracts for process registration, continuation-consumed, process-exit evidence and process-bound Lease Renewal;
- recipient Inbox read/assignment routes and complete notification producer/consumer migration;
- exhaustive direct Run INSERT inventory and every claim/recovery/requeue call site;
- full route/consumer/code audit and empty-database/concurrency test plan.

SEM-02F closes the *candidate design shape* for the first five entities; it does not close executable migration/API design or code audit.

No migration, API, WSS implementation or Phase 0 work is authorized by this document.


## 6. SEM-02F proposed closure of schema decisions

The following proposals now have concrete candidate contracts in [SEM-02F](./sem-02f-executable-physical-and-protocol-contracts.md):

- HIR: `governance.human_intervention.record_kind=action|hir`; preserve current action rows and add HIR-only fields/state machine. This is preferred over a parallel HIR master table.
- E6: extend `governance.human_consultation` with typed E6 anchors, residual risk, mitigations and explicit second confirmation; enforce one E6 per Barrier and one successor per E6.
- Notification: keep `integration.notification` as project Feed and add `integration.notification_recipient` for per-principal Inbox/unassigned queue. V1 in-app notification is inserted in the same Platform transaction; do not require RabbitMQ or a separate delivery worker.
- Barrier: `delivery.task_execution_barrier`, one unreleased per Task, exact-successor exception only for E6.
- Process fence: `delivery.task_run_process` plus `task_run.active_process_sequence`; SEM-local process observations are not Platform Lease Renewal authority.
- Orphan scan: Platform Worker periodic DB-only scanner; the existing code's startup-only call is not sufficient. B06-D3 direct requeue is removed.

These are detailed proposals, not executable SQL. Full route/consumer and direct Run INSERT audit remains a mandatory pre-migration task.
