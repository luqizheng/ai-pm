# SEM V1 Domain Model — SEM-02E

- Status: Review Pending.
- Normative baseline: [SEM-02E](./sem-02e-architecture-convergence.md).

## Business-owned Platform entities

- Task / Run / Lease / Generation: Platform authority.
- Interaction: Agent-user input/decision/permission lifecycle.
- Human Intervention: existing `governance.human_intervention`, reused as HIR workflow.
- Human Consultation: existing `governance.human_consultation`, reused for formal human decisions including E6.
- Notification: existing `integration.notification`, reused for recipient feed and delivery visibility.
- Execution Barrier: Task-scoped physical execution uncertainty; prevents any new Run from bypassing an unresolved old process.

## SEM-owned execution entities

RunnerConnection, TicketUse, DispatchOperation, CommandDelivery, PlatformDelivery, InboxEvent, ProcessObservation and SEM-local Detached/Reconciliation state. These do not duplicate Platform Run/Lease/Generation/HIR authority.

## Barrier relationships

Barrier includes task_id, origin_run_id, origin_generation, state, evidence references, HIR link and audit metadata. At most one active/risk_accepted_unconfirmed Barrier per Task. E6 authorization references one Barrier and one unique successor Run.

## Process Identity

A Run+Generation may own an ordered sequence of process identities. Each identity has process_sequence, boot_id, PID + process creation identity, harness kind, launch reason, timestamps and optional command correlation. Continue starts a new identity sequence.

## RabbitMQ

RabbitMQ is an existing standalone service without clustering; it is not a business entity or authority. Broker messages never grant execution rights or prove process termination.
