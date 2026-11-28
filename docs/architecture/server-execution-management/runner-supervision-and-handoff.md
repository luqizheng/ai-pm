# Runner Supervision, HIR and Handoff — SEM-02E

> **HISTORICAL — SUPERSEDED, NOT AN IMPLEMENTATION CONTRACT (2026-11-25):** This AI-PM-side SEM design copy records the former Platform Claim/Lease/Run/Task/HIR/Barrier control-plane proposal. It is retained for source-audit and migration traceability only. Do not implement its SEM business orchestration, database ownership, state machines or API flows. The current bilateral boundary and active SEM protocol drafts are indexed in [the AI-PM SEM integration review](./README.md) and maintained canonically in [luqizheng/smark-sem](https://github.com/luqizheng/smark-sem).


> **Authority-boundary revision (2026-11-25):** HIR, E6, Task Barrier and all business intervention/approval semantics belong to AI-PM, not SEM. Any SEM-local state here is infrastructure supervision only. Reconcile this historical proposal against [authority-boundary-decision.md](./authority-boundary-decision.md) before implementation.


- Status: Review Pending.
- Normative baseline: [SEM-02E](./sem-02e-architecture-convergence.md).

## Supervision

Runner mode defaults to unattended. Supervised is explicit and requires user selection before Claim. Online/offline and available/busy are separate from supervision mode.

## HIR and existing governance model

HIR reuses and extends `governance.human_intervention`; formal decisions reuse `governance.human_consultation`; notifications reuse `integration.notification`. Do not create parallel HIR/consultation/notification master tables without a schema audit proving extension impossible. HIR is available from Phase 0: create, query, assign owner/recipient, notify, append evidence, controlled disposition and audit.

## E4/E5

E4/E5 create or associate HIR and Task Barrier. HIR closed does not release Barrier. E1/E2/E3 are termination evidence; E4/E5 are not. Missing Runner heartbeat is not process death.

## E6

E6 is an explicit high-risk human decision, not termination evidence. It requires privileged operator, second confirmation, reason, residual-risk summary, mitigation, immutable audit, and exactly one successor_run_id bound to the HIR/Task Barrier. The old Barrier remains risk_accepted_unconfirmed. Successor still uses normal Platform Claim/new generation. E6 proposes formal amendment to ADR-SEM-04 and ADR-SEM-10; pending approval.

## Handoff

Normal handoff: old process E1/E2/E3 → release old Barrier → target Runner selected → normal Platform Claim → new Generation/token → new Workspace. Never rewrite lease_client_id. E6 exceptional handoff allows one successor while old process may still run, with old Barrier retained and explicit residual-risk display.
