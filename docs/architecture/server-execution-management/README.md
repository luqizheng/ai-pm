# Server Execution Management — AI-PM Integration Review

> **Current boundary (2026-11-25):** SEM is an independent downstream service maintained in [luqizheng/smark-sem](https://github.com/luqizheng/smark-sem). The user-confirmed direction is that SEM transports opaque execution messages and manages Runner/Harness infrastructure only. AI-PM retains all business authority; Harness obtains Task context and performs business operations through AI-PM MCP. SEM must not interpret Task/Run/Requirement/Approval/HIR semantics or access the AI-PM database.

## Current AI-PM review entry points

- [Authority boundary decision](./authority-boundary-decision.md) — current Platform / SEM / Runner / Harness ownership.
- [Execution path ownership mapping](./execution-path-ownership-mapping.md) — read-only mapping of existing AI-PM source paths to the target boundary.
- [Harness identity and business fencing review](./harness-identity-and-business-fencing-review.md) — delegated identity, MCP scope, dual fencing, process uncertainty and delivery evidence.
- [Consistency review](./consistency-review.md) — cross-cutting source and contract findings.
- [Code contract mapping](./code-contract-mapping.md) — source-level responsibility mapping.
- [SEM downstream architecture index](https://github.com/luqizheng/smark-sem/blob/docs/sem-authority-boundary-convergence/docs/architecture/server-execution-management/README.md).
- [Platform–SEM execution-message protocol draft](https://github.com/luqizheng/smark-sem/blob/docs/sem-authority-boundary-convergence/docs/protocol/PLATFORM-SEM-EXECUTION-MESSAGE-PROTOCOL-DRAFT.md).
- [Execution identity/lifecycle/fencing/delivery contract draft](https://github.com/luqizheng/smark-sem/blob/docs/sem-authority-boundary-convergence/docs/protocol/EXECUTION-IDENTITY-LIFECYCLE-FENCING-DELIVERY-CONTRACT-DRAFT.md).

## Callback / Inbox contract review

- [AI-PM SEM callback and Inbox design](./ai-pm-sem-callback-inbox-design-draft.md) — source-verified draft for the dedicated service callback, generic Inbox limitations, `execution_id`↔`task_run.id` mapping, per-execution sequence cursor, transactional ACK and retry semantics.

## Authority summary

| Concern | Authority |
|---|---|
| Project, Requirement, Task and business state | AI-PM |
| Business Run/attempt and authorization_generation | AI-PM |
| Human review, approval and acceptance | AI-PM, human-only where required |
| Harness business context/progress/result submission | AI-PM MCP |
| SEM execution_id, delivery/release ledger and binding_epoch | SEM |
| Runner registration, capabilities, resource telemetry and reservations | SEM |
| Local Harness process launch, identity, termination and exit evidence | Runner Supervisor |
| Prompt/body business meaning | Opaque to SEM |

## Cross-repository constraints

- AI-PM and SEM have independent databases. SEM has no AI-PM database credentials and no direct DB access.
- AI-PM → SEM uses the Rust SDK provided and versioned by smark-sem, over its versioned HTTP or HTTPS contract on the company private network. AI-PM's Outbox worker calls the SDK; AI-PM owns Outbox persistence, retry scheduling and reconciliation. The SDK must not access AI-PM's database or make business retry/Task-state decisions. See the [SEM SDK provider/consumer contract draft](https://github.com/luqizheng/smark-sem/blob/docs/sem-sdk-consumer-contract/docs/architecture/sem-sdk-and-consumer-integration-contract-draft.md) , the [draft public API design](https://github.com/luqizheng/smark-sem/blob/docs/sem-sdk-consumer-contract/docs/architecture/sem-sdk-public-api-design-draft.md), and its [source compatibility audit](https://github.com/luqizheng/smark-sem/blob/docs/sem-sdk-consumer-contract/docs/architecture/sem-sdk-source-compatibility-audit.md). HTTPS is optional and supports enterprise CA or explicitly trusted self-signed certificates; certificate validation cannot be disabled. HTTP is restricted to the internal network and does not provide wire confidentiality.
- Runner ↔ SEM uses authenticated WSS with enterprise CA or explicitly pinned self-signed server certificates; certificate validation remains mandatory.
- SEM reports infrastructure facts only; it does not call Task/Run business Claim, Lease, HIR, Barrier, review or completion APIs.
- SEM → AI-PM remains a callback to an AI-PM-owned endpoint. AI-PM owns callback authentication, Inbox transaction, Run observation and ACK-after-commit; the SDK does not embed or own this receiver.
- Runner supervision defaults to unattended; supervised mode is explicit.
- Resource telemetry from Runner informs SEM admission/concurrency.
- Workspace/worktree backup is not required. One authoritative Git origin per repository.
- Existing RabbitMQ is a separate single-instance service and is not a correctness or authority dependency.

## Historical material

The SEM-02E–02J and earlier V1 design package in this AI-PM directory is retained as source-audit and migration history. Its Platform Claim/Lease/Task/HIR/Barrier control-plane assumptions are superseded and must not be implemented. The canonical SEM architecture and protocol documents are maintained in smark-sem.

## Development gate

No SEM or AI-PM implementation, database migration, runtime configuration change or Phase 0 work should begin until the bilateral architecture, identity, protocol, lifecycle, security, operations and acceptance documents are complete, internally consistent, reviewed together and explicitly approved by the user.
