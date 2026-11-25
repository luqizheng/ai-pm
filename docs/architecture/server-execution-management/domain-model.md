# SEM V1 Domain Model — SEM-02E

- Status: Review Pending.
- Normative baseline: [SEM-02E](./sem-02e-architecture-convergence.md).
- This document defines target ownership and relationships; it does not claim that the target schema already exists.

## 1. Platform-owned business entities

| Entity | Existing carrier / target | Authority and semantics |
|---|---|---|
| Task / Run / Lease / Generation | Existing Platform delivery model | Platform is the sole business execution-right authority |
| Interaction | `delivery.task_interaction` | Agent input/decision/permission lifecycle; not HIR and not process-termination evidence |
| HIR | Extend `governance.human_intervention` | Execution exception and human-work item; schema extension is proposed and requires complete DDL/consumer audit |
| Human Consultation | `governance.human_consultation` | Formal human decision with existing four-state CHECK, decision-completeness CHECK and append-only supersede relation |
| Notification | Extend `integration.notification` | Project feed and recipient delivery visibility; preserve existing read semantics; recipient/delivery contract remains to be designed |
| Execution Barrier | New Platform-owned entity/table or equivalent normalized structure | Task-scoped execution uncertainty; blocks all new Run/Generation dispatch for the Task until safely released or explicitly risk-accepted |

Repository evidence describes the existing `governance.human_intervention` as an execution-intervention/action record (for example veto/rollback/freeze), while `governance.human_consultation` is a separate formal-decision record. Reusing the former as the HIR work-item carrier therefore requires a deliberate schema/domain refactor; it must not silently conflate an action log with an open work queue. Preserve the distinction between HIR lifecycle and individual intervention actions, either through clearly separated fields/child records or a justified redesign documented before migration.

## 2. SEM-owned execution entities

SEM owns RunnerConnection, TicketUse, DispatchOperation, CommandDelivery, PlatformDelivery, InboxEvent, ProcessObservation, resource snapshots/reservations, and persistent Detached/Reconciliation state. These are execution/transport facts and must not become a second Platform Run/Lease/Generation/HIR authority.

The minimum SEM persistence baseline remains the seven tables in ADR-SEM-24. Process identity sequence details may be normalized within `process_observation` unless implementation proves a separate SEM-private table necessary; no eighth table is approved by this document.

## 3. Task Execution Barrier

Barrier is Platform-owned and Task-scoped. It records at least `task_id`, `origin_run_id`, `origin_generation`, state (`active`, `risk_accepted_unconfirmed`, `released`), evidence references, HIR link, actor and timestamps, and optional E6 Human Consultation/successor references.

At most one unreleased Barrier may exist per Task. The database must enforce this under concurrent create/claim/requeue paths. A new Run row or Generation cannot bypass an existing Task Barrier.

E6 does not release the Barrier. It changes the Barrier to `risk_accepted_unconfirmed` and authorizes exactly one pre-bound successor.

## 4. Process Identity sequence

A Run+Generation may contain multiple sequential process identities, especially when Interaction continuation terminates one Harness process and starts another without changing the Run Generation.

Each identity includes a monotonically increasing `process_sequence`, Runner/device identity and `boot_id`, OS PID plus process creation identity, Harness kind and launch reason, start/exit observations, and optional originating `command_id`.

Late evidence for sequence N may close only sequence N; it cannot close or overwrite sequence N+1.

**Open contract requirement:** SEM records OS process observations, but Platform is the Lease/Generation authority. Before implementation, the protocol must define how Platform learns and fences the accepted active `process_sequence` (for example, an authenticated continuation-consumed API that atomically advances a Platform-side active-process fence). Platform cannot validate Lease Renewal using a process sequence that exists only in SEM DB.

## 5. E6 Human Consultation

E6 is a formal risk-acceptance decision recorded through `governance.human_consultation`, not a process-exit event. It binds one HIR, one Task Barrier and one unique successor Run, with operator, decision time, reason, residual-risk summary, mitigations and explicit confirmation. The successor is preallocated/reserved atomically with authorization. Existing Human Consultation constraints (status CHECK, decision tuple and supersede chain) must be preserved or deliberately amended.

## 6. RabbitMQ

RabbitMQ is an existing independent single-instance service without clustering. It is not a business entity or authority. Broker messages never grant execution rights, commit business state, release a Barrier or prove process termination.
