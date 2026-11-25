# SEM-02D Architecture Review Package — Superseded by SEM-02E

> **HISTORICAL — SUPERSEDED, NOT AN IMPLEMENTATION CONTRACT (2026-11-25):** This AI-PM-side SEM design copy records the former Platform Claim/Lease/Run/Task/HIR/Barrier control-plane proposal. It is retained for source-audit and migration traceability only. Do not implement its SEM business orchestration, database ownership, state machines or API flows. The current bilateral boundary and active SEM protocol drafts are indexed in [the AI-PM SEM integration review](./README.md) and maintained canonically in [luqizheng/smark-sem](https://github.com/luqizheng/smark-sem).


- Status: Historical review record.
- Current baseline: [SEM-02E Architecture Convergence](./sem-02e-architecture-convergence.md).
- Current review package: [SEM-02E Review Package](./sem-02e-review-package.md).
- Current decision: **Changes Required / Review Pending**.
- Phase 0: Not Started.

The earlier D-01–D-13 and corrective findings package is retained as historical context only. Its Run-scoped Barrier proposal, independent HIR table proposal, older API/WSS alternatives, and earlier Phase numbering are superseded by SEM-02E.

Current normative decisions:
- Task-scoped Execution Barrier.
- Reuse existing `governance.human_intervention`, `governance.human_consultation`, and `integration.notification`, subject to full schema/routes/consumer audit.
- Platform DB-only periodic orphan scanner in Phase 0; B06-D3 is folded into it.
- Run+Generation supports ordered Process Identity sequence for Interaction continue.
- E6 is a human risk authorization, not termination evidence; one bound successor only; old Barrier remains unresolved.
- Platform–SEM API candidate prefix `/internal/sem/v1/`.
- WSS events: `runner.process.exit`, `runner.process.reconciled`, `runner.reconnect_sync`, `continuation.consumed`.
- RabbitMQ exists as an independent single-instance service without clustering; it is not a safety-critical dependency or authoritative delivery path.
- Phase 0–5 numbering follows SEM-02E §14.

Do not implement from this historical package. Final architecture freeze requires full review of all 23 topic documents and explicit user approval.


## SEM-02F superseding implementation contract

The historic SEM-02D review package is superseded for physical schema/protocol details by [SEM-02F](./sem-02f-executable-physical-and-protocol-contracts.md). Do not implement the old independent HIR table name, Run+Generation-only Barrier, or earlier notification Outbox assumptions. The current candidate uses a discriminator on `governance.human_intervention`, Task Barrier, `notification_recipient`, and Platform process identity fencing. This historical document is not an implementation source.
