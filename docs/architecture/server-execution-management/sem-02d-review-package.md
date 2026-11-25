# SEM-02D Architecture Review Package — Superseded by SEM-02E

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
