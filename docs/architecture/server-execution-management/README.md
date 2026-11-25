# Server Execution Management Architecture V1

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
