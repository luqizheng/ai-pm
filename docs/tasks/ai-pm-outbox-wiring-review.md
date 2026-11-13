# AI-PM Outbox 生产接线（Slice C）：现状评审与推进方案

> 面向：技术评审方
> 日期：2026-10-31
> 范围：`ai-pm/docs/tasks/ai-pm-task-state-machine-v1.md` §20「Outbox 事件持久化与异步分发（首版必需）」在生产路径的落地
> 已交付基线：`59a686b`（写入端口 + 同事务接线 + 功能开关 + 死信 + 测试）
> 结论摘要：**基础设施已就位，缺的是"消费侧真实动作"；其中 `TaskReady` 的落点已存在（`delivery.task_run`），建议据此收口 ③-D；其余三类事件维持"不产生 + 明确死信"。**

---

## 1. 前因：这件事从哪来

### 1.1 规格要求（为什么必须有）

`docs/tasks/ai-pm-task-state-machine-v1.md` §20 把 Outbox 定为**首版必需**（§20.1「首版就纳入 Outbox 事件写入」、§20.6.1「业务状态变更与事件写入必须同一事务」）。其设计意图是一句硬约束：

> **业务事务只负责保证事件不会丢，Worker 负责最终执行。不要在业务事务中直接调用 MCP 或 Codex。**（§20.3）

首版范围（§20.2）：持久化表、同事务写入、最小消费、重试、幂等、死信+人工重放——**全部要求**；通用事件总线/完整事件溯源**不在**首版。

必须进 Outbox 的事件共六类（§20.5）：

| 事件 | 语义（触发的异步工作） |
| --- | --- |
| `TaskReady` | 任务满足执行条件，等待调度 |
| `RunRequested` | 请求创建或启动新的执行实例 |
| `RunCompleted` | 执行结束，需要收集结果和验证交付 |
| `DeliverySubmitted` | 交付已提交，需要触发 Review |
| `ReviewCompleted` | 评审结束，需要推进任务状态 |
| `KnowledgeBaseWriteRequested` | 需要将结果/决策回写知识库 |

### 1.2 既有建设（Slice A / Slice B 已做）

| 切片 | 交付 | 证据 |
| --- | --- | --- |
| Slice A：域层 13 态 | `TaskStatus` 13 值 + 迁移规则矩阵 | `crates/pm-domain/src/task.rs`、`task_transition.rs` |
| Slice B：迁移 0010 | `delivery.task_run` / `task_run_failure` / `task_supersession` / `task_waiting` / `task_policy_override` / `integration.outbox_event` + 13 态约束 | `migrations/0010_task_state_machine.sql:123-144` |
| Slice B：消费骨架 | `outbox::claim/complete/fail/dead_letter/replay/list` + `OutboxWorker` 注册表 | `crates/pm-infrastructure/src/outbox.rs`、`outbox_worker.rs` |

---

## 2. 后果：我们发现了什么（问题定性）

在推进 Slice C 前做了一次生产侧探查，得到三条**互相叠加**的事实：

### 2.1 生产路径根本没有写入点

`outbox::append` 只被测试调用；**没有任何业务事务写入 outbox**。六类事件的字面量只出现在 `crates/pm-infrastructure/tests/*`。也就是说：outbox 表建了、worker 循环建了，但**没有事件进入**——基础设施在空转。

### 2.2 消费侧零 handler

`apps/ai-pm-worker` 首版**不注册任何 handler**，`OutboxWorker` 对未注册事件一律 `dead_letter`（`outbox_worker.rs:109-117`）。设计上这是**诚实的**（不静默 Delivered），但结果是所有事件都进死信，异步分发从未真正发生。

### 2.3 依赖方向约束

`outbox::append` 收 `&mut Transaction`（同事务），但该函数在 `pm-infrastructure`；而 `pm-application` 的**生产依赖不含** `pm-infrastructure`（仅 dev-dependencies）。因此写入点不能直接 `use pm_infrastructure`。

**影响**：spec §20 的首版要求，在代码层面**尚未在生产生效**。这不能算"已实现"，也不该"补几个 handler 就收工"。

---

## 3. 本次已交付（③-A~③-E，提交 `59a686b`）

按"纵向切片"落地，先打通可靠性与事务边界，再谈业务动作：

| 阶段 | 落点 | 结果 |
| --- | --- | --- |
| ③-A Schema | `migrations/0010_task_state_machine.sql:123-144` | **核对确认已满足**：`event_type/aggregate_*/payload`、`status` CHECK（4 态）、`attempts/max_attempts/next_attempt_at`、`locked_until`+`lease_owner`、`last_error`、`idempotency_key UNIQUE`、`schema_version`、`created_at/delivered_at`、部分索引 `idx_outbox_claim`。**未重复建表** |
| ③-B 事务端口 | `crates/pm-application/src/outbox.rs`（新增） | `OutboxAppend` 端口（`append(&mut Transaction)`）+ PG 直写适配 `PgOutboxAppend`（`ON CONFLICT (idempotency_key) DO NOTHING` 幂等）；**应用层不反向依赖基础设施层** |
| ③-C 写入接线 | `crates/pm-application/src/command_executor.rs`（`update_task_status`） | 在既有 `pool.begin()` 事务内、`commit` 前写入；仅接两条语义无歧义的迁移边：`Draft→Ready`=`TaskReady`、`Running→Submitted`=`DeliverySubmitted` |
| ③-D 消费 | 功能开关 `AI_PM_OUTBOX_EVENTS`（默认开 `TaskReady`/`DeliverySubmitted`，空串全关） | **不注册假 handler**；未注册事件明确死信且可 replay |
| ③-E 测试 | `crates/pm-application/tests/outbox.rs`、`outbox_wiring.rs`（新增） | 事务原子性、幂等、回滚；`FailingOutbox` 证明"事件写失败 → 业务状态回滚" |

**验收映射（spec §20.6）**：

| 要求 | 覆盖 |
| --- | --- |
| 状态变更成功 + 事件写入成功 | ✓ `ready_transition_enqueues_task_ready_in_the_same_transaction` |
| 事件写入失败 → 业务事务回滚 | ✓ `outbox_write_failure_rolls_back_the_status_change` |
| 状态变更失败 → 不产生事件 | ✓ `revision_conflict_leaves_no_event` |
| Worker 崩溃后事件可恢复 | ✓ `expired_lease_is_reclaimed_as_the_same_row`（Slice B） |
| 重复消费不重复创建执行实例 | **不适用**（尚无创建实例的 handler/触发源） |
| 无 handler 事件不标记 Delivered | ✓ `unregistered_event_type_goes_to_dead_letter`（Slice B） |

**门禁**：后端全量 `276 tests run: 276 passed`；`cargo build` 通过；依赖方向未破坏；提交 `17ff1e6..59a686b` 已推送 `origin/master`。

---

## 4. 核心争议：六类事件里，究竟哪几类"没有落点"

这是本次评审真正要拍板的问题。逐类核对（**含一处对既有判断的更正**）：

| 事件 | 触发动作（spec） | 生产落点是否存在 | 结论 |
| --- | --- | --- | --- |
| `TaskReady` | 任务进入可调度 | **有**：`delivery.task_run`（`0010:18-41`，含 `attempt_no`、`UNIQUE(task_id, attempt_no)`、`idx_task_run_active`）；**只缺写入代码** | **可落地**（见 §5-A） |
| `DeliverySubmitted` | 交付已提交 → 触发 Review | Review 编排不存在；但事件本身可先入 outbox 待 replay | 事件可产生，消费待 Slice D |
| `RunRequested` | 请求创建执行实例 | 与 `TaskReady` 同一落点（创建 Run），首版可合并，不单独产生 | 暂不产生 |
| `RunCompleted` | 执行结束 | **无触发源**（无 Codex/MCP 执行器，docs 明确第一阶段不做完整多 Agent） | 暂不产生 |
| `ReviewCompleted` | 评审结论落库 | **无动作/落点**（无评审编排） | 暂不产生 |
| `KnowledgeBaseWriteRequested` | 知识回写 | 知识写入能力存在（`knowledge::create_document`，独立 Git 根），但**索引写接口缺失**（`knowledge_index.rs` 仅 `parse_markdown_tree`/`search_documents`）；且文档写入是事务外（文件+commit），与"同事务写入"冲突 | 暂不产生 |

> **关键更正**：此前把 `RunRequested/RunCompleted/ReviewCompleted/KnowledgeBaseWriteRequested` 一并判为"无落点"。核对后发现 **`TaskReady`（调度）确有现成落点**——`delivery.task_run` 本就是 §19.5 定义的执行记录表，只是从未被写入。这使 ③-D 从"无解"变为"可收口"。

**其余三类的共同症结**：不是缺"表"，而是缺**业务动作**（谁在什么时候产生该事件/执行该工作）。在业务动作缺位时强行产生事件，只会制造死信，属于"假装消费"，与 spec §20.6 冲突——**故维持不产生**。

---

## 5. 怎么办：三条路线

### 方案 A（推荐）：`TaskReady` → 创建 `delivery.task_run`，作为"调度"的最小真实落点

**核心判断**：spec 对 `TaskReady` 的定义就是"**等待调度**"。在本仓中，"调度"的最小真实形态 = 为该任务创建一条**待执行记录**（`task_run`，`status='queued'`），供后续执行器（Codex/MCP，Slice D）领取。这**不是假装消费**：`task_run` 是真实、可观测、可被后续处置的排队状态，与 `integration.job` 同构。

| 项 | 设计 |
| --- | --- |
| 落点 | 新增 `crates/pm-infrastructure/src/outbox_handlers.rs`：`TaskReadyHandler`；在 `apps/ai-pm-worker/src/main.rs` 注册 |
| 动作 | 读事件 payload（`task_id`）→ 若该任务**无 active run**（`idx_task_run_active`：`queued/running`）→ `INSERT delivery.task_run(attempt_no = max+1, status='queued')` 并 `UPDATE delivery.task.attempt_count` |
| 幂等 | 至少一次投递下重复消费：以"已存在 active run 则直接返回 Ok（视为已调度）"判重；`UNIQUE(task_id, attempt_no)` 兜底 |
| 事务 | handler 内单事务：查 active + 建 run + 递增计数；失败 → `outbox::fail` 退避重试（既有能力） |
| 验收 | 集成测试：`TaskReady` 事件 → dispatch 后恰 1 条 `task_run(queued)`；**重复 dispatch 不新增 run**；handler 抛错 → 事件重试不多建 |
| 依赖 | 不新增迁移（复用 0010 表）；不破坏依赖方向（handler 纯 SQL） |
| 风险 | 首版 `task_run` 会停在 `queued`（无执行器领取）——这是**诚实的排队态**，非错误；需在 UI/文档标注"待执行" |

**收益**：让 §20 的"最小消费"在生产真正生效（事件从 Pending → Delivered，且产生可见执行记录）；`TaskReady` 闭环可与 ④「AI 执行中心」页天然衔接（该页正需要执行/调度数据源）。

### 方案 B：维持现状（不产生 + 明确死信 + replay）

**做法**：不注册任何 handler，事件一律死信并保留 replay；用 `AI_PM_OUTBOX_EVENTS` 关闭产生。
**适用**：等 Slice D 的 Codex/MCP 执行器与评审编排落地后再统一接。
**代价**：§20 首版要求仍**未在生产生效**；outbox 继续空转/死信堆积。

### 方案 C：新建"统一调度/交付记录表"承载全部六类事件

**做法**：新增迁移（≥0011）建一张带幂等键的通用 dispatch/回执表，所有事件落到该表。
**代价**：与既有 `task_run` / 审计 / 通知语义重叠；引入"第二套记录"，复杂度高、评审收益低。
**判断**：**不推荐**（`task_run` 已足够承载 `TaskReady`，其余三类缺的不是表）。

### 方案对比

| 维度 | A（推荐） | B | C |
| --- | --- | --- | --- |
| §20 首版生效 | 部分（TaskReady 闭环） | 否 | 部分 |
| 新增迁移 | 无 | 无 | ≥0011 |
| 违反"不假装消费" | 否 | 否 | 否 |
| 风险 | 低（排队态待执行器） | 低但欠交付 | 中高（语义重叠） |
| 与 ④ AI 执行中心衔接 | 强 | 无 | 中 |

---

## 6. 分阶段推进建议

| 阶段 | 内容 | 落点 | 验收 |
| --- | --- | --- | --- |
| ③-D-1 | `TaskReadyHandler` + worker 注册 | `pm-infrastructure/src/outbox_handlers.rs`、`apps/ai-pm-worker/src/main.rs` | 事件 Delivered + 恰 1 条 `task_run`；重复消费幂等 |
| ③-D-2 | UI/文档标注 `task_run='queued'` 为"待执行" | ④ 执行中心页、runbook | 页面可见、文案不误导 |
| ③-E+ | 补"重复消费不重复建 Run"端到端 | `crates/pm-infrastructure/tests/outbox_worker.rs` | 断言 |
| Slice D | Codex/MCP 执行器领取 `queued` run、`RunCompleted`、评审编排、知识回写 | 待裁 | §20.5 全六类闭环 |

> 说明：`RunRequested/RunCompleted/ReviewCompleted/KnowledgeBaseWriteRequested` 的**触发源**属于独立能力建设（执行器/评审/索引），不属本次接线范围；在此之前它们不产生事件、不制造死信。

---

## 7. 待技术方裁决的问题清单

| # | 问题 | 建议 |
| --- | --- | --- |
| Q1 | `TaskReady` 的"调度"是否以"创建 `task_run(queued)`"为最小真实落点？ | **采纳方案 A** |
| Q2 | 其余四类事件首版是否直接"不产生 + 明确死信 + replay"？ | 采纳（避免假装消费） |
| Q3 | Codex/MCP 执行器何时接入（决定 `queued` run 的消化时机）？ | 列入 Slice D |
| Q4 | 路线优先级：④ workspace 两页（AI 执行中心/产物验收，迁移 ≥0011）、⑤ 知识空间 V2（质量门/红线/PageIndex）、⑥ 导航 5 vs 9 | 待排 |

---

## 8. 附录

### 8.1 关键文件定位

| 关注点 | 文件 |
| --- | --- |
| Outbox 表 | `migrations/0010_task_state_machine.sql:123-144` |
| 写入端口 | `crates/pm-application/src/outbox.rs` |
| 写入接线 | `crates/pm-application/src/command_executor.rs`（`update_task_status`） |
| 消费骨架 | `crates/pm-infrastructure/src/outbox.rs`、`outbox_worker.rs` |
| Worker 入口 | `apps/ai-pm-worker/src/main.rs` |
| 规格 | `docs/tasks/ai-pm-task-state-machine-v1.md` §20 |

### 8.2 边界（本方案不做）

- 不引入独立消息队列 / 通用事件总线 / 完整事件溯源（§20.2）。
- 不注册空 handler 冒充消费（§20.6）。
- Outbox 不替代 Task/Run 状态机与审计日志（§20.6.6）。

### 8.3 术语

- **至少一次投递**：事件可能重复送达，消费者必须幂等。
- **死信（Dead Letter）**：超过重试上限或无人消费的事件，保留失败原因、可人工 replay。
- **同事务写入**：事件行与业务变更在同一 `BEGIN/COMMIT` 内提交，保证"状态变了事件就丢不了"。

---

## 9. 一句话结论

**Slice C 的可靠性与事务边界已建成并验证（`59a686b`）；真正卡点是消费侧业务动作。评审建议：用既有 `delivery.task_run` 表把 `TaskReady` 的"调度"做成最小真实闭环（方案 A），其余四类在触发源建成前不产生事件；这样既满足 spec §20 首版要求，又不违反"不假装消费"。**