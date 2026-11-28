# SEM-02D Closure Design Proposal — Superseded by SEM-02E

> **HISTORICAL — SUPERSEDED, NOT AN IMPLEMENTATION CONTRACT (2026-11-25):** This AI-PM-side SEM design copy records the former Platform Claim/Lease/Run/Task/HIR/Barrier control-plane proposal. It is retained for source-audit and migration traceability only. Do not implement its SEM business orchestration, database ownership, state machines or API flows. The current bilateral boundary and active SEM protocol drafts are indexed in [the AI-PM SEM integration review](./README.md) and maintained canonically in [luqizheng/smark-sem](https://github.com/luqizheng/smark-sem).


- Historical proposal; do not use as current normative baseline.
- Current architecture baseline: [SEM-02E](./sem-02e-architecture-convergence.md).
- Current review status: Changes Required / Review Pending.

The SEM-02D proposal's independent HIR table name `governance.human_intervention_request`, Run+Generation-only Barrier scope, Phase numbering, and earlier message/API alternatives are superseded. SEM-02E proposes:
- reuse/extend `governance.human_intervention`, `governance.human_consultation`, `integration.notification`;
- Task-scoped Execution Barrier;
- periodic Platform DB-only orphan scanner in Phase 0;
- process identity sequence within Run+Generation;
- E6 as human risk acceptance, not termination evidence;
- API prefix `/internal/sem/v1/`;
- WSS names `runner.process.exit`, `runner.process.reconciled`, `runner.reconnect_sync`, `continuation.consumed`;
- unified Phase 0–5 from SEM-02E §14;
- RabbitMQ existing standalone/no-cluster, non-authoritative and non-required for safety.

This file is retained as historical review context only. Do not implement from it.
