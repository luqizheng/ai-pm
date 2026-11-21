# PA-07 Developer Implementation Specification V1.0

- 项目：smark-pm
- 关联架构：PA-07 Task Execution Orchestration & Cross-Product Collaboration V1.0
- 日期：2026-11-20
- 状态：开发实施基线（目标规范；代码映射仍需按当前 HEAD 复核）
- 技术基线：Rust 2021 / axum 0.8 / sqlx 0.8 / PostgreSQL 16 / Tokio / MCP Streamable HTTP / Vue 前端
- 现状证据：`runner-codebase-baseline-assessment.md`，审查快照 HEAD `a176e5c`，迁移 0001–0041

## 1. 文档目的与使用规则

本文件把 PA-07 V1.0 架构边界转成开发可执行的设计、迁移、API、事务、事件、测试和验收要求。它不是对现有代码的完成声明，也不是未经核对即可直接执行的 SQL migration。

开发必须先核对当前分支和最新 migration，复用已有表、服务、状态机、审计和 outbox。若当前代码与本文件列出的快照不同，以实际代码为事实；若与 PA-07 架构目标冲突，记录差距并通过本规范的实施任务修复，不得静默更改架构语义。

## 2. 实施原则

1. 模块化单体优先，不拆微服务。
2. PostgreSQL 是 ID、状态、Revision 指针、关系、授权、审计和事件的权威来源。
3. Markdown + Git 是正式需求/知识正文权威；PageIndex 等为可重建派生索引。
4. 复用 `pm-domain`、`pm-application`、`pm-infrastructure`、`pm-mcp`、`pm-ai`、`ai-pm-worker`、`ai-pm-runner`。
5. 不创建平台自有 Agent Work Unit；Harness 内部协作由 Harness 管理。
6. 不引入 Kafka、分布式锁、图数据库、独立调度引擎或独立恢复服务。
7. 状态变更在短事务内完成；不在数据库事务中等待模型、Provider、Git 网络操作或外部进程。
8. 外部执行端只能通过受控 MCP/API 命令报告事实，不得直接写正式业务状态。

## 3. 当前代码基线（2026-11-20 审查快照）

### 3.1 已存在并应优先复用的执行能力

| 能力 | 当前代码事实（审查快照） | 实施要求 |
|---|---|---|
| Task | `delivery.task`，13 态；`revision` 乐观锁；唯一迁移入口 `apply_task_transition` | 保留受守卫状态机，不另造 Task 状态写入口 |
| Run | `delivery.task_run`，7 态；Task:Run 1:N；一次 attempt 即一条 Run | 复用 Run，不新增 Attempt 实体 |
| Claim/Lease | 两阶段 claim、行锁/SKIP LOCKED、token hash、generation fencing、绝对 deadline、七门检查 | 保留核心协议，派发和启动前补充 PA-07 Product/Revision/Dependency 门禁 |
| Retry/Recovery | attempt、infrastructure retry、dispatch attempts 分层；B3-1/B3-2、reconcile、cleanup ledger | 保留既有恢复内核，AI-PM 只负责语义恢复决策 |
| Runner | `apps/ai-pm-runner`，本地进程执行、MCP 轮询、心跳、命令、收割 | 保留 Runner 客户端定位，不把 MCP 变成 Runtime |
| Server Executor | `CodexExecutor` / `NoopExecutor` | 保留执行适配边界；Noop 不得伪造成功 |
| MCP | 26 个 `pm_*` 工具，Streamable HTTP | 复用工具命名和认证，不开放通用状态修改工具 |
| Delivery | `task_repository_commit`、Git 事实校验、指纹幂等、Review 管线 | 复用交付事实校验；补齐多 Product 验收表达 |
| Audit/Event | `governance.audit_log`、`delivery.task_run_event`、`integration.outbox_event` | 复用现有审计和 Outbox，不新建第二套事件平台 |
| Device/Auth | `devenv.device`、cpk 凭据、项目绑定和权限码 | 保留 fail-closed、最小权限和撤销优先策略 |

### 3.2 当前代码事实与目标架构的差异

| 差异 | 当前审查证据 | 处理优先级 |
|---|---|---|
| Task Revision 不可变快照模型尚未在审查报告中证明为独立实体 | `delivery.task.revision` 已用于乐观锁；Run 需固定执行契约版本 | P0 设计核对：不得把整数 CAS 版本误当完整 Revision 历史 |
| Product/Project 多对多、Product 执行状态与 Task-Product 服务范围未在执行审查中完成字段级核验 | 执行审查对非执行域 90+ 表仅表名级；Product 领域未深查 | P0 核对实际 schema 后映射/补迁移 |
| Task-Product 独立验收结果尚未在审查报告中证实 | 当前交付 Review 以 Task/Run 为中心，partial 交付语义缺失 | P0 设计最小验收扩展 |
| Task Dependency 正式关系表/事务 DAG 校验尚未在审查报告中证实 | PA-07 V1.0 审查71–83已定义目标 | P0 实施 |
| Workflow Execution 与模板版本的现状需字段级核对 | 代码基线审查重点在 Run/Runner，未完整核验 Workflow 表 | P1 核对并复用现有模型 |
| Human Consultation 与既有 `task_waiting`/interaction 的关系需核对 | 当前存在 `task_waiting`、`human_intervention_requests` 等执行等待语义 | P1 统一语义，不重复建审批系统 |
| Product Archived 生命周期未在执行审查中证明 | Product 领域未深查 | P1 核对并实现归档，不做硬删除 |
| `discovery_writeback` 的 AI 建 Task 链路存在已记录断裂 | `pm_write/task.rs` 的 `require_human` 与 AI writeback 冲突；2 个测试持续红 | **人类业务授权阻塞项**，不得由技术实施擅自改权限 |
| Runner per-device 活跃 Run 上限仅由客户端单槽位约定 | 服务端无 DB 级 per-device 上限 | P1，推荐增加 DB 级单活跃 Run 约束 |
| 服务端 CodexExecutor SSH `.pem` 明文残留 | `codex_executor.rs:110-133` | P0 安全修复 |
| Run 工作目录 / Git mirror 无磁盘回收 | `run_cleanup` 只处理进程清理 | P1 清理策略 |
| Runner 主循环无直接单测 | 组件测试 + E2E，主循环无单测 | P1 补关键状态组合单测 |
| `pm-ai` LLM 无 token 级预算上限 | 当前字符预算/轮数上限 | P2，后续预算改进 |

注意：以上代码事实仅基于指定审查快照。全量测试未运行，live DB 未连接验证，services/、policies/、部分前端和非执行域未深查。

## 4. 目标数据模型与映射策略

### 4.1 复用现有表

优先复用：

- `core.project`
- `delivery.task`
- `delivery.task_run`
- `delivery.task_run_repository`
- `delivery.task_repository_commit`
- `delivery.task_run_event`
- `delivery.task_run_failure`
- `delivery.worker_command`
- `delivery.task_run_cleanup`
- `devenv.device` / `devenv.device_project_binding`
- `integration.project_repository` / `integration.outbox_event`
- `governance.audit_log` / `governance.tenant_audit_log`

实际 schema 名称和约束以 migration 为准；不要仅凭本文的概念名新建重复实体。

### 4.2 Task Revision

目标要求：每个 Run 固定一个不可变 Task Revision。现有 `delivery.task.revision` 是并发控制版本号，不能自动视为完整的业务 Revision 历史。

实施步骤：
1. 先检查当前 Task 执行契约是否已有不可变 JSONB 快照、版本表或 Run context 快照。
2. 若已有完整不可变快照，复用并补齐 Revision 标识、校验和及查询路径。
3. 若没有，新增最小 `delivery.task_revision`（或遵循现有命名）记录：`task_id`、`revision_no`、`goal`、`scope`、`acceptance_criteria`、`execution_contract`、`created_at`、`created_by`、`change_reason`；内容创建后不可原位更新。
4. Task 保存当前 Revision 指针；Run 保存 `task_revision_id` 或等价不可变快照引用。
5. Revision 创建与 Task 当前指针更新在同一事务内，并使用 `expected_revision`/CAS 防止覆盖。

不要把模型名、Provider 名或具体 Harness 工具名写入通用 Task 目标契约，除非它们是明确的执行配置而非业务目标。

### 4.3 Task-Product 服务范围

目标要求 Task 可以服务多个 Product，且区分业务服务归属与技术依赖。

先核对现有 Product 表、Product-Project 关系和权限模型。若已有 Task-Product 关系，补齐历史有效性和服务范围；若没有，新增最小关联表。关联至少表达：`task_id`、`product_id`、有效状态/范围、创建/撤回时间及操作者。关系变化必须可审计；Run 启动快照固定当时有效的 Product 关联和影响范围。

不得将 Product 的代码/API 依赖直接写成 Task-Product 业务归属；不得从 Project 共享关系自动推断 Task 必须等待另一个 Task。

### 4.4 Task Dependency

V1 使用一张正式有向边表，字段语义：

- `predecessor_task_id`
- `successor_task_id`
- `required_condition`：`completed` / `accepted` / `deliverable_approved`
- `required_deliverable_ref`：可选，但 `deliverable_approved` 必须指定明确交付物及版本
- 审计字段（创建/更新人、时间、原因）

约束：
- 同一对 Task 不重复相同方向边；禁止自依赖；
- 新增/修改依赖时在同一事务中做 DAG 环路校验；
- 并发写边必须使用足够的事务隔离/锁定策略，避免两个分别通过的请求组合成环；
- 不支持 OR、任意表达式、脚本或 force-start；
- 当前依赖图服务未来调度，Run 快照解释历史执行。

Run 启动快照至少记录前置 Task、对应 Revision、满足条件、指定交付物版本、检查结果和检查时间。

### 4.5 Product 生命周期与门禁

Product 执行状态目标：`Normal`、`Suspended`、`Resuming`、`Archived`。先映射现有 Product 状态/归档字段，不可在未检查前假设表名或权限结构。

- `Suspended`：受影响开发工作不可派发/启动；不批量修改 Task 状态。
- `Resuming`：执行 Portfolio Review，按门禁逐步开放；不批量恢复 Task。
- `Archived`：不接收常规新需求；归档前逐项处理未完成工作；不级联删除研发历史。
- 多 Product Task 必须所有相关 Product 都允许执行。
- 完全无关且影响边界明确的工作可以继续。

Orchestrator 派发前和 Run 启动前分别检查 Product 门禁。门禁计算不写成 Task 生命周期状态。

### 4.6 多 Product 验收的最小表达

V1 不建立独立 Product Delivery 子系统。先检查现有 Review/acceptance 表是否能以 Product 为维度记录：Product、Revision、验收条件集合、结果、证据引用、验收人/来源、时间。

若现有模型不能表达，则新增最小 Product 级验收记录（例如 `delivery.task_product_acceptance`），并关联 Task、Task Revision、Product 和交付物版本。验收记录应保留历史，不覆盖已通过的记录。共同验收条件与 Product 专属条件可通过结构化条件引用或现有 acceptance schema 表达；避免任意脚本表达式。

整体 Task Completed 条件：全部仍有效且必要的 Task 工作完成，所有共同交付条件通过，所有仍关联 Product 的必要专属验收条件通过，且无未处置的重大交付缺陷。某 Product 撤回或归档后，其专属条件经正式处理退出当前有效范围，但历史验收保留。

### 4.7 Workflow 与 Human Consultation

复用现有 Workflow/Stage/Execution 模型；模板必须版本化，运行中的 Workflow 固定模板版本。Workflow Execution 不拥有 Task/Run，不把阶段失败直接映射为 Task Failed。

Human Consultation 复用现有人工介入/等待能力，先核对 `human_intervention_requests`、`task_waiting`、interaction 和现有决策记录。只在缺少正式决策内容、决定人、时间、适用 Product/范围和状态时补充字段或最小表。禁止另建通用审批流。

状态语义：`pending`、`in_progress`、`decided`、`cancelled`。多 Product 决策分别保存，不能把一个 Product 的决定当成另一个 Product 的同意。

### 4.8 Product 业务标识（Key）

**决策来源**：UX 修订交付（2026-11-21）发现 UI 已按「name / KEY」呈现 Product（如 `Authentication service / AUTH`），而实体模型没有对应字段——模型与 UI 不一致，本节将其修正为正式域决策（新增决策，非既有决策追述）。

**身份双轴**：Product ID 与 Product Key 是两个概念，不得互相替代——

| | Product ID | Product Key |
|---|---|---|
| 标识对象 | 数据库内部主键 | 面向业务与系统集成的稳定业务标识 |
| 用途 | 实体关联、外键引用（task_product、product_project_link、product_member、Run 快照等） | UI 识别、业务文档/ADR/需求引用、AI-PM 与 MCP/API 间传递身份、审计与日志、数据导入导出 |
| 作为外键 | 是 | **否**——一切关联仍走 ID |

**规则**：

| 属性 | 决定 |
|---|---|
| 必有性 | `core.product.key` 为正式 Domain Entity 属性（NOT NULL），不是 UI 临时生成的显示字段 |
| 唯一性 | **全局唯一**（跨租户）；DB 以 `lower(key)` 唯一索引保证（大小写不敏感） |
| 生成 | 创建 Product 时由服务层从 name 派生自动生成；API/MCP 不接受指定 key |
| 稳定性 | 创建后不可变；无重命名/重设 key 的 API |
| 格式 | `^[A-Z0-9][A-Z0-9-]{0,62}[A-Z0-9]$`（2–64 字符；大写字母数字与连字符，首尾为字母数字） |
| 派生规则 | name 中非 `[A-Za-z0-9]` 连续序列折叠为单个 `-`，去首尾 `-` 后大写，截断 64；结果不足 2 字符（或 name 无字母数字）时以 `P-` + id 前 8 位十六进制兜底；与既有 key 冲突（大小写不敏感）时追加 `-2`、`-3`… |
| 与 name 关系 | 相互独立，name 变化不联动 key |
| 边界 | key 是业务身份标识，不是业务名称，也不是权限标识——授权一律经由 ID + Membership，不读 key |

**用途**：UI 展示（列表/详情/选择器以 `name / KEY` 呈现）、业务文档引用、AI-PM 会话识别、Task 业务范围标识、MCP/API 外部引用、审计事件、导入导出。API 侧 `ProductProfile` 从本修订起携带 `key`。

**三实体标识一致性核查结论（2026-11-21）**：

- `core.project.code`（0001 基线）：NOT NULL、创建时人工指定、唯一约束为 `uq(tenant_id, code)` **租户内唯一**、大小写敏感——与 Product Key 的「全局唯一 + 大小写不敏感」口径不一致。
- `core.team`：**无**任何业务标识列；API `team.slug` 实为 `core.tenant.slug` 借用（`principals.rs` memberships 查询 SELECT `tn.slug`），属命名错位而非 Team Key。
- **待裁决项**（不随本修订实施，另行决策）：①Project code 是否同步升级为全局唯一/大小写不敏感（涉及现有创建链路与 0001 约束）；②Team 是否需要自有业务 Key；③`team.slug` 字段语义是否正名（移除或改为真实 Team Key）。在裁决前，三实体标识规则不得各自继续演化。

## 5. Orchestrator 调度与 Run 启动协议

### 5.1 统一门禁

派发和启动必须检查：
1. Task 仍有效，且当前 Revision 可执行；
2. 所有关联 Product 对该工作范围允许执行；
3. 所有 Task Dependency 满足其条件；
4. Repository、Provider、Runner/设备和用户授权有效；
5. 执行资源可用；
6. 没有未决 Human Consultation 或其他明确限制该执行范围的门禁。

### 5.2 两个检查点

- **派发前**：用于避免明显不具备资格的 Task 进入 Runner 队列。
- **Run 启动前**：再次检查关键条件，防止排队期间 Revision、Product、Dependency 或授权变化。

不得将外部进程 spawn 放在长数据库事务中。事务先原子创建/领取 Run 和启动意图，再由 Runner/Executor 执行；启动确认通过受守卫回报推进状态。若 spawn 结果不确定，按现有 B3/reconcile 机制对账，不盲目创建重复执行。

### 5.3 阻塞原因

阻塞是调度读模型/计算结果，不是 Task 状态。至少区分：`blocked_by_dependency`、`blocked_by_product`、`blocked_by_authorization`、`blocked_by_resource`、`blocked_by_human_decision`、`blocked_by_revision`。这些名称是 API/read-model 推荐值；若项目已有 reason enum，应映射复用。允许同时存在多个阻塞原因。

## 6. 状态转换与事务边界

### 6.1 Task

保留现有 13 态状态机和唯一迁移入口 `apply_task_transition`。新增状态转换必须经过 `pm-domain` transition validation；不开放 SQL 直写或通用 PATCH status。Task 终态不得由普通更新接口重开。

### 6.2 Run

保留当前 7 态及真实 `timed_out` 语义，不按简化状态图删值。Run 重试创建新 Run，旧 Run 保留终态/失败证据/`supersedes_run_id` 关联。Run 侧若状态转换分散，应在本次改造中至少抽出共享的 transition guard/状态转换测试矩阵；不要求大规模重构全部服务。

### 6.3 Workflow / Product / Human Consultation

每类状态转换有明确的领域服务入口。事务内完成：读取当前状态/版本、检查合法转换、检查权限/前置条件、更新状态、写审计、写 Outbox。不得将外部 LLM/Provider/Runner/Git 请求包在事务中。

### 6.4 Revision CAS

Revision 更新要求客户端/调用方提供预期当前版本。版本冲突返回明确 Conflict，包含当前 Revision 标识；调用方重新读取，不允许 last-write-wins。

### 6.5 Dependency DAG 事务

依赖边变更与闭环检查同一事务。采用行级/事务级并发保护；建议对同一 Project 的 Dependency 图变更串行化（事务 advisory lock 或锁定稳定的 Project/Task anchor 行），避免复杂分布式锁。具体选择以 PostgreSQL 当前隔离级别和现有锁序测试为准。

### 6.6 Run claim 幂等

复用 `uq_task_run_active`、`uq_task_run_source_event`、SKIP LOCKED 和现有幂等指纹。新增 Product/Dependency/Revision 检查时，保持既有 claim 锁序，避免引入死锁。所有事务保持短小。

## 7. API / MCP 契约要求

具体路由沿用现有 `/v1/pm/...` 命名和 `pm_*` MCP 命名，不创建平行 API 体系。至少保证以下操作具备受守卫入口：

| 操作 | 主体 | 必须检查 | 结果/副作用 |
|---|---|---|---|
| 创建/更新 Task | AI-PM/有权人类 | Project 权限、Product 服务范围、Revision CAS | 新 Task/Revision + 审计 |
| 创建/修改/解除 Dependency | AI-PM | Task 归属、DAG、Revision 影响 | 依赖更新 + 审计 + 重算通知 |
| Product Suspend/Resume/Archive | AI-PM 发起、Orchestrator 执行门禁；涉及正式业务决定时先完成人类决定 | Product 决策权、影响范围、在途工作 | Product 状态事件 + Portfolio Review |
| Dispatch/Claim Run | Orchestrator/Runner | 全部门禁、幂等、租约和 fencing | Run claim + 固定快照 |
| Run start confirmation | Runner/Executor | Run generation、当前执行意图 | Run 状态推进 |
| Submit result | Harness/Runner | 身份、lease、generation、Git 事实、幂等 | 交付记录 + Review/验收队列 |
| Product acceptance | 对应 Product 有权验收主体/平台流程 | Product 权限、Revision、交付版本 | Product 级验收历史 |
| Human Consultation decision | 对应 Product 有权人类 | 决策事项状态、Product 范围、身份 | 决策记录 + 影响分析事件 |

错误响应至少区分：Unauthorized/Forbidden、Conflict（Revision/状态竞争）、DependencyCycle、DependencyNotSatisfied、ProductSuspended、HumanDecisionRequired、RunAlreadyActive、StaleLease/Generation、DeliverableInvalid、ValidationFailed。复用项目现有错误 envelope，不另造一套格式。

外部 Harness 不得获得直接写 Task status、Product status、正式 Revision 或 Dependency 的通用接口。

## 8. 事件与 Outbox

复用 `integration.outbox_event` 和现有事件消费者。建议事件类型（如已有相同语义则复用现名）：

- `task.revision_activated`
- `task.dependency_changed`
- `product.execution_state_changed`
- `product.archived`
- `run.started` / `run.terminal`
- `deliverable.registered` / `deliverable.invalidated`
- `product.acceptance.recorded`
- `human_consultation.decided`
- `workflow.stage_changed`

事件最小载荷包含 event_id、aggregate_type/id、project_id、revision/version、occurred_at、actor/source、相关引用；不要在事件中复制整份权威正文。

Outbox 与业务状态更新同事务写入；消费端按 event_id 幂等。事件用于触发重算/通知，数据库当前状态仍是权威。

## 9. Runner / Provider / Harness 执行安全

- MCP 是通信接口，不是 Runtime。
- Runner 管本机进程、工作区、心跳、命令轮询和回执；Provider Adapter 管服务端执行适配。
- 平台凭据不下发给 Harness；设备凭据只允许最小 MCP 权限。
- lease token 不落明文持久化；日志、process.json、事件和错误信息必须脱敏。
- 授权撤销优先：先落 Run 终态/清凭据，再异步清理进程。
- 不确定外部进程是否仍在执行时，不启动替代 Run；先对账。
- NoopExecutor 只能返回 Unsupported，不得伪造成功。

## 10. 关键代码差距与实施任务

### P0 — 必须先完成

| ID | 任务 | 交付与验收 |
|---|---|---|
| PA07-IMP-001 | 核对当前 Product/Project/Task-Product schema 与权限 | ER 映射表；标注现有/缺失；确认 Product 状态权威字段 |
| PA07-IMP-002 | Task Revision 不可变模型核对与补齐 | 每次 Run 可追溯精确 Revision；并发修改冲突测试 |
| PA07-IMP-003 | Task Dependency 表、DAG 校验和调度门禁 | 多前置 AND、并行后继、自环/间接环拒绝、并发环路测试 |
| PA07-IMP-004 | Run 启动前 PA-07 门禁快照 | 固定 Revision、Product 范围、Dependency 结果、授权快照；Product/Revision 竞态测试 |
| PA07-IMP-005 | Product 维度验收表达 | 多 Product 独立验收；共同条件/专属条件；撤回和归档后的有效范围测试 |
| PA07-IMP-006 | 人工决策边界映射 | Human Consultation 记录决定人、内容、时间、Product/范围；未回应不得视为同意 |
| PA07-IMP-007 | 审计与历史追溯闭环 | Revision、Dependency、Product 状态、Run 快照、验收、归档均可追溯 |

### P1 — 核心安全和可靠性

| ID | 任务 | 交付与验收 |
|---|---|---|
| PA07-IMP-008 | SSH 私钥临时文件 RAII 清理 | 所有成功/失败/崩溃路径无明文残留；权限最小化测试 |
| PA07-IMP-009 | Runner per-device 单活跃 Run 服务端约束 | 数据库/claim 事务级约束；并发双 claim 仅一成功；若产品需要多槽位，必须显式配置而非默认 |
| PA07-IMP-010 | Workspace/Git mirror 回收 | 只清理无活跃 Run、无引用且超过保留期的工作目录；dry-run + 审计 + 恢复测试 |
| PA07-IMP-011 | Runner 主循环单测 | claim→spawn→heartbeat→command→submit→exit；崩溃/迟到回报/重复回报场景 |
| PA07-IMP-012 | Run 状态转换一致性 | 集中共享 transition guard 或统一测试矩阵；迟到事件不得覆盖终态 |
| PA07-IMP-013 | Product Suspend/Resume/Archive 端到端门禁 | 未派发、已派发未启动、运行中、多 Product Task 场景 |

### P2 — 后续优化

| ID | 任务 | 交付与验收 |
|---|---|---|
| PA07-IMP-014 | Submit 事务缩短 | 将 Git 事实校验与 DB 锁定/提交分层，避免远端子进程运行在长事务中；保留 TOCTOU 防护 |
| PA07-IMP-015 | LLM token 预算 | 按模型/任务设 token 上限和成本/超时记录 |
| PA07-IMP-016 | Provider endpoint DNS rebinding 防护 | 解析后再次校验并约束实际连接目标；回归 SSRF 测试 |
| PA07-IMP-017 | Device credential TTL/rotation | 兼容现有 cpk 认证，提供轮换与撤销路径 |
| PA07-IMP-018 | Runner / Workspace 运维可观测性 | 指标、告警、磁盘水位和清理失败队列 |
| PA07-IMP-019 | Product 业务标识 Key（§4.8） | 0049 加列+`lower(key)` 全局唯一索引+格式 CHECK；创建自动生成（派生/兜底/冲突后缀）；API 带 key；大小写不敏感冲突与格式边界测试；已随本修订实施 |

## 11. 必须保留的人类业务授权阻塞项

代码基线报告记录：AI `discovery_writeback` 创建 Task 的链路被 `pm_write/task.rs` 的 `require_human` 门控阻断，且相关 2 个测试持续红。报告明确将其标记为产品授权三选一，而不是普通技术缺陷。

**实施规则：不得由开发或 AI-PM 擅自放宽 `require_human`。** 在有权人类明确决定之前：
- 保留 fail-closed；
- AI 可以分析、拆解并生成提案，但不能绕过现有人工授权创建正式 Task；
- 将该项标记为 `BLOCKED_HUMAN_DECISION`；
- 决策应明确哪些 AI 创建动作是草稿/提案，哪些正式 Task 创建需要人工确认，以及授权适用范围。

这属于正式权限边界，不由 PA-07 技术授权自动解决。

## 12. 测试矩阵与验收门槛

### 12.1 单元与领域测试

- Task/Run/Product/Workflow/Human Consultation 合法状态转换与非法跳转；
- Task Revision 不可变、CAS 冲突、历史引用；
- Dependency AND 汇聚、三个有限条件、DAG 环路和事务竞争；
- Product 门禁与局部影响范围；
- 共同交付与 Product 专属验收的独立性。

### 12.2 数据库集成测试

- 并发 claim、重复 outbox、重复 Run 结果回报；
- Revision 更新与 Run 启动竞争；
- Product Suspend 与 Run 启动竞争；
- 两条依赖变更并发形成环路时必须有且仅有合法结果；
- Task Completed 与最后一条必要验收记录原子一致；
- 多 Product Task 撤回一个 Product 后仍保留另一个 Product 的有效工作。

### 12.3 E2E 测试

1. 单 Product Task：创建→Revision→Dependency→Run→交付→验收→Completed。
2. 多 Product Task：共同交付通过、A 验收通过、B 待验收，Task 不提前 Completed。
3. B 验收失败不重置 A 已通过结果；局部修复后 B 再验收。
4. Product A Suspended，A 专属和 A+B 共同工作受限，完全无关 B 工作继续。
5. A 恢复进入 Resuming，Portfolio Review 后按门禁逐步恢复。
6. A 归档，B 仍需要共享 Task/交付物时继续，不级联删除。
7. Harness 提议需求变化，不能直接修改正式 Revision；Human Consultation 决策后 AI-PM 更新计划。
8. Runner/Provider 中断、结果迟到、重复回报、不确定进程状态，不产生重复执行或虚假成功。

### 12.4 发布验收

- 所有 P0 项通过自动化测试；
- P1 安全问题关闭或有经人类明确接受的风险处置；
- 迁移可在当前数据库快照上前滚，备份/恢复路径验证；
- API/MCP 权限测试覆盖拒绝路径；
- 全量后端、前端和 E2E 测试结果有明确报告；
- 不存在未经处理的状态枚举漂移、孤儿引用或历史记录覆盖。

## 13. 开发顺序

1. Schema/ER 核对与目标映射（IMP-001）。
2. Task Revision、Task-Product、Dependency 数据模型及迁移。
3. 领域状态机与门禁服务。
4. Run 快照、claim/启动前检查和 Outbox 事件。
5. Product 级验收与 Human Consultation。
6. Product Suspend/Resume/Archive 流程。
7. Runner 安全、清理、并发约束和可靠性测试。
8. 前端呈现、审计检索和 E2E。

不要先实现 UI，再倒推权限或状态模型。每批迁移必须先有领域测试和回滚/前滚说明。

## 14. 需要开发首轮复核的假设

- 当前 Product 表、Product-Project 关系和 Product 权限实现；
- Task 执行契约是否已有可作为 Task Revision 的不可变快照；
- Workflow/Stage/Execution 现有实体及模板版本；
- Human Consultation 是否能复用现有人工介入模型；
- 现有验收模型是否能表达 Product 级独立验收；
- 当前分支是否已超出 HEAD `a176e5c` 和迁移 0041；
- `runner-codebase-baseline-assessment.md` 标注的 P0/P1 问题是否已修复。

若假设不成立，提交最小差异设计；不扩大架构范围，不擅自改变业务授权。

## 15. 明确非目标

- 不重写现有 Run Dispatch/Runner 核心；
- 不把 Task Run 改成独立 Attempt 实体；
- 不把 MCP 变成执行 Runtime；
- 不引入 Agent Work Unit；
- 不建设分布式调度、消息总线、图数据库或独立恢复微服务；
- 不做常规 Product 硬删除；
- 不擅自解除 AI Task 创建的人工授权门控；
- 不把 World View 的 SQLite Schema 复制到 smark-pm。
