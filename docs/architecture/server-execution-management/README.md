# Server Execution Management Architecture V1

> **Authority-boundary revision (2026-11-25):** The user-confirmed direction is now that SEM transports opaque execution messages and coordinates Runner/Harness runtime infrastructure only; Runner Supervisor owns local OS process lifecycle. AI-PM MCP is the sole Harness business-context and Task mutation channel. The older SEM-02E–02J control-plane assumptions and Protocol V1 Claim/Lease/business-command proxy model are superseded; see [authority-boundary decision](./authority-boundary-decision.md) and the [replacement execution-message protocol draft](https://github.com/luqizheng/smark-sem/blob/docs/sem-authority-boundary-convergence/docs/protocol/PLATFORM-SEM-EXECUTION-MESSAGE-PROTOCOL-DRAFT.md). Do not implement the superseded control path.

- 状态：**Design In Progress — SEM-02E Review Pending**
- 架构收敛主文档：[SEM-02E 全量架构收敛基线](./sem-02e-architecture-convergence.md)
- 评审包：[SEM-02E Review Package](./sem-02e-review-package.md)
- 分支：`docs/sem-02e-architecture-convergence`
- 当前工作仅限架构文档；未修改业务代码、Migration、API、WSS、Dockerfile、Compose 或 Worker/Runner 主循环；Phase 0 未启动。

## 一句话架构

AI-PM 由 Platform、**独立服务 SEM**、Client Runner 和 Harness 构成。Platform 与 SEM 各自独立部署、独立数据库；Platform 通过版本化 HTTPS API 保持 Run/Lease/Generation 和业务治理权威，SEM 负责 Runner 控制面、执行进程监督与执行侧持久状态；Runner 通过 WSS 连接 SEM；Harness 继续通过 Platform MCP 使用业务能力。

## 部署与数据权威

- Platform PostgreSQL：Project/Task/Run/Lease/Generation、Interaction、HIR、Human Consultation、worker_command、业务通知和审计的权威。
- SEM DB：Runner connection、WSS delivery ledger、dispatch operation、process observation、资源快照和 SEM-local recovery state。
- SEM **不得**连接或共享 Platform PostgreSQL。
- Platform Outbox → HTTPS → SEM Inbox；SEM → Platform HTTPS API；Runner ↔ SEM WSS。
- Platform Outbox 只有 Platform 一个消费者；两侧不做分布式事务。
- Runner 不再绕过 SEM 轮询 Platform Claim、command、ACK 或 recovery control。

## 本轮架构裁决

| 主题 | 当前基线 |
|---|---|
| SEM 身份 | 独立服务，不是 Platform Worker 的可选模块 |
| Execution Barrier | Task 作用域，防止新 Run 行绕过旧执行的不确定性 |
| HIR | 复用并扩展 `governance.human_intervention` |
| E6 决定 | 复用并扩展 `governance.human_consultation` |
| Notification | `integration.notification` 保留项目 Feed/read_at；新增 `integration.notification_recipient` 个人 Inbox 子表 |
| Orphan detection | Phase 0 Platform DB-only 周期扫描，不依赖 Worker 重启 |
| Process Identity | Run + Generation 内支持有序进程身份序列，覆盖 Interaction continue |
| E6 | 提议正式修订 ADR-SEM-04/10；风险授权不等于终止证据，不释放旧 Barrier |
| RabbitMQ | 已存在独立单实例、无集群；不是 SEM 安全关键依赖，不替代 HTTPS Outbox/Inbox |
| Workspace | 不备份；卷丢失不能证明进程死亡 |
| Runner | 默认 unattended；supervised 需要用户确认 |
| Git | 每个 Repository 一个权威 origin |

## 23 份专题文档 + SEM-02E/02F 收敛规格

以 SEM-02E 为跨文档一致性的主基线。专题文档必须把规范更新写回正文并删除冲突定义，不允许只在文末追加“以新节为准”。

## 阶段状态

Phase 0–5 的唯一编号和依赖以 [SEM-02E §14](./sem-02e-architecture-convergence.md#14-phase-0–5-统一路线) 为准。Phase 0 未启动。当前整体状态为 **Review Pending**，不代表 ADR 已冻结或实施获批。

## RabbitMQ 部署事实与使用边界

环境已有独立 RabbitMQ 单实例服务，没有集群。SEM V1 不新增 RabbitMQ 必需依赖；Claim、Lease、Barrier、HIR、E3 和恢复不依赖 RabbitMQ 可用性。若未来批准，可将其用于非权威唤醒/通知加速，数据库 Outbox/Inbox 与 API 对账仍是可靠性基础。

## SEM-02F 可实施契约

[SEM-02F — Executable Physical and Protocol Contracts](./sem-02f-executable-physical-and-protocol-contracts.md) 将 Task Barrier、HIR discriminator、E6 anchors、Notification Recipient、Platform Process Sequence、Lease Renewal fencing、周期 orphan scanner 和 Phase 0 验收细化为候选物理/协议契约。它不是已批准 ADR，也不授权实施。

## SEM-02G 数据库与 API 契约草案

[SEM-02G — Database DDL and Service API Contract Draft](./sem-02g-database-and-api-contract-draft.md) 包含 Task Barrier、HIR、E6、Notification Recipient、Process Registry 的 SQL-shaped 草案，以及 Platform–SEM HTTPS / Runner–SEM WSS 消息样例。仅供评审，不是可执行 Migration。

## SEM-02H DDL / transaction review

[SEM-02H — DDL Integrity and Transaction Review](./sem-02h-ddl-integrity-and-transaction-review.md) found concrete issues in the SEM-02G SQL-shaped draft. SEM-02G is not executable; do not translate it into migrations before the SEM-02H findings are closed. No implementation is authorized.

## SEM-02I — Corrected Platform Schema and Process API Candidate

[Open SEM-02I](./sem-02i-corrected-platform-schema-and-process-api-candidate.md). This is a candidate design, not an executable migration contract. The lock graph and API scopes remain under review.

## SEM-02J — Platform Transaction and Execution-Path Closure Audit

[Open SEM-02J](./sem-02j-transaction-and-execution-path-closure-review.md). The source audit confirms multiple cross-aggregate lock-order inversions and identifies Task Barrier bypasses in review, retry, reacquire and existing dispatch paths. SEM-02J remains **Changes Required / Audit In Progress**; it does not authorize code changes or Phase 0.


### SEM-02J second audit pass

The second source pass expanded the Run INSERT/UPDATE inventory and inspected Task/Project/Run foreign keys, partial unique indexes and the checked-in trigger inventory. It confirms a direct Review retry bypass (no Project activity gate, Task row lock or Task Barrier), and confirms Delivery retry-after-failure also lacks Project activity and Barrier checks. FK validation and unique-index conflicts contribute implicit lock edges; unique indexes do not authorize dispatch through a Task Barrier. The failure-judge path is explicitly not classified as a proven simultaneous Run→Task inversion because its standalone Run update releases before the later Task transaction. See [SEM-02J §9](./sem-02j-transaction-and-execution-path-closure-review.md#9-second-audit-pass--run-writes-fkindextrigger-side-effects). Status remains Changes Required / Audit In Progress.


### SEM-02J third audit pass

The third pass traced normal lease/business writes, existing Runner/Worker control entrypoints and governance carriers. It found additional Run→Task lock edges in `report_progress`, `submit_result`, Worker start confirmation and Review completion. It also confirmed that the current repository still exposes Runner-facing MCP Claim/reacquire/command poll/ACK and a Platform Worker dispatch loop; these must be removed or fail-closed at SEM control-plane cutover while retaining Harness business MCP. Current `governance.human_intervention` is an action-history carrier with seven intervention types, not an HIR lifecycle; `human_consultation` is a generic formal decision workflow; `integration.notification` is a project Feed without recipient ownership. See [SEM-02J §10](./sem-02j-transaction-and-execution-path-closure-review.md#10-third-audit-pass--lease-business-writes-legacy-control-entrypoints-and-governance-carriers). No global lock order or implementation is approved.


### SEM-02J fourth audit pass

The fourth pass traced Interaction report/retry/response/consume, Continue and Terminate Command ACK, Continuation lifecycle hooks, lease renewal/progress and cleanup. It confirms the lock graph is connected across Project, Run, Task, Interaction, Command and Continuation; Task cancellation holds Task before Interaction/Run, while several execution/ACK paths hold Run or Command before Task. It adds J11–J18 deterministic concurrency cases. No global lock order is selected. See [SEM-02J §11](./sem-02j-transaction-and-execution-path-closure-review.md#11-fourth-audit-pass--interactioncontinuationcommand-transaction-paths).


### SEM-02J fifth audit pass

The fifth pass identified the current migration head as 0059 and inspected late execution-related DDL 0046 and 0053–0059. `human_consultation` lacks composite tenant/project/task scope integrity and cannot by itself encode E6; 0053 enforces one active Run per `lease_client_id`, not per SEM Runner process/connection; 0054 cleanup fencing and 0055–0057 workspace reclamation remain derived-resource ledgers, not process-exit evidence. The 2026-11-15 schema inventory predates these migrations and cannot certify current DDL. Full 0001–0059 replay and catalog capture remain open. See [SEM-02J §12](./sem-02j-transaction-and-execution-path-closure-review.md#12-fifth-audit-pass--current-migration-head-and-late-execution-related-ddl).


## Transaction and migration audit

- [SEM-02J Platform Transaction and Execution-Path Closure Audit](./sem-02j-transaction-and-execution-path-closure-review.md) — lock-order, Run creation/dispatch/recovery paths, current migration baseline provenance and late DDL. Status remains **Changes Required / Audit In Progress**; full PostgreSQL replay/catalog capture and global lock-order closure are still open.


## Execution-core baseline DDL audit

SEM-02J §14 records the 59-file migration manifest (0001 baseline + 0002–0059, same filenames/blob SHAs on master and the review branch), exact execution-core baseline entities, FK/unique-index/trigger semantics, and post-baseline DDL deltas. This remains a static Git DDL audit only: fresh PostgreSQL replay, catalog capture, full writer validation, and global lock-order closure are still open.


## Application-writer and lock-order evidence

SEM-02J §15 traces production writers for Interaction, Response, ResponseDelivery, Continuation and WorkerCommand. The normal write path derives a coherent Task/Run/Project tuple from a locked Run or Interaction, but the schema does not independently enforce all composite identities. The Run↔Interaction lock inversion is confirmed by exact source statements; the global lock order remains open.


## Implicit FK lock-order finding

SEM-02J §16 identifies a new P0 Task↔Product deadlock schedule: Run claim locks Task then Product, while add_task_product locks Product before its Task FK check acquires key-share on Task. This is an implicit FK edge and requires the association writer to adopt the canonical Task→Product order plus a deterministic two-connection regression test. No production fix has been applied.


## Remaining gate-writer audit

SEM-02J §17 traces Task Revision, TaskReady/Run creation, Human Consultation/TaskWaiting and dependency-gate writers. Normal callers preserve Task ownership through row locks or same-transaction creation; task-bound Consultation also writes TaskWaiting with composite scope FKs. The Task↔Product implicit-FK deadlock in §16 remains open, and predecessor-revision snapshot semantics still need a two-connection test.


## Project–Task implicit audit-FK lock cycle

SEM-02J §18 records a second implicit-FK deadlock candidate: Run claim locks Project→Run→Task, while Task PATCH locks Task then inserts governance.audit_log, whose composite FK takes Project key-share. The resulting Project→Task / Task→Project wait cycle requires two-connection PostgreSQL reproduction and a canonical lock-order correction. No production fix has been applied.


SEM-02J §18.4 records two unselected ways to close the Project→Task / Task→Project audit-FK cycle: evaluate a weaker claim-side Project FOR NO KEY UPDATE lock (still conflicting with Project lifecycle updates but compatible with FK KEY SHARE), or acquire Project KEY SHARE before Task locks in affected writers. Neither is approved pending PostgreSQL lock-mode/concurrency tests.


### SEM-02J twelfth audit pass — Project/Product lock-mode cross-check

SEM-02J §19 confirms the current Project lock inventory and narrows the §18 cycle to Task-first audited writers, especially Task PATCH. Project-first archive, formal knowledge/review/Run-start, member and import paths were inspected. The claim-side `FOR NO KEY UPDATE` option remains a candidate—not an approved fix—because it should retain exclusion against explicit Project `FOR UPDATE` lifecycle guards while allowing audit-FK KEY SHARE. Product lifecycle's Product-row lock has no observed reverse Task edge; `add_task_product` remains the Product→Task FK inversion. TaskProduct withdrawal needs a claim-order semantic test. All conclusions are source-derived; PostgreSQL concurrency tests were not run, and P0 findings / Changes Required / Audit In Progress remain open.


### SEM-02J thirteenth audit pass — Project↔Run implicit Interaction-FK cycle

SEM-02J §20 identifies a further Project lock inversion: Run Claim holds Project `FOR UPDATE` before Run, while `report_interaction` holds Run before inserting a Project-scoped Interaction whose FK requires Project KEY SHARE. This produces a Project→Run / Run→Project deadlock candidate independently of the audit-log insert. The `FOR NO KEY UPDATE` claim-side candidate could address this and §18's audit-FK cycle, but remains unapproved until deterministic PostgreSQL tests also prove Project archive/interaction linearization. Other Run-first writers remain under review. No PostgreSQL concurrency tests have been run; P0 / Changes Required / Audit In Progress remain open.


### SEM-02J fourteenth audit pass — expanded Project↔Run writer inventory

SEM-02J §21 extends the Project→Run / Run→Project lock-cycle inventory to Run lease reacquisition, continue Command ACK and persisted Interaction response. Each path acquires/updates Run before its Project-scoped audit FK; continue ACK additionally holds the Command row, and response persistence holds Interaction first. The Project `FOR NO KEY UPDATE` option remains a candidate only. Add one independent two-connection test per writer and preserve archive/claim serialization. No PostgreSQL concurrency tests were run; P0 / Changes Required / Audit In Progress remain open.
