# PA07-IMP-001：Product / Project / Task-Product Schema 与权限核对报告

> <span>HISTORICAL — 历史核对快照</span>：本报告只描述 HEAD `ecb1ce2`、migrations 0001–0041（2026-11-20）时的 Schema 与权限状态。报告中的“缺失 / 待核 / 待裁决”不得直接当作当前缺陷或当前待决事项。后续实施核对见 [PA07-IMP-002](PA07-IMP-002-implementation-audit.md)，该审计基准为 `75094e1`；其后差异见 [PA07-IMP-003](PA07-IMP-003-current-state-verification.md)。

- **任务：** PA07-IMP-001（PA-07 开发实施规范 §10 P0 第 1 项）
- **日期：** 2026-11-20
- **核对基准：** HEAD `ecb1ce2`，migrations 0001–0041，`apps/pm-api/src`、`crates/pm-application/src`
- **交付：** ER 映射表（现有/缺失标注）+ Product 状态权威字段结论 + 对 IMP-002~007 的范围影响
- **处理策略：** 本项目无存量数据——差距一律直接改契约/重建表，不写数据迁移与兼容层（AGENTS.md 规则 1，用户 2026-11-20 再次确认）

## 1. ER 映射总表

| PA-07 目标实体 | 现状（表/出处） | 判定 | 差距 |
|---|---|---|---|
| Project | `core.project`（0001：code/name/status/revision CAS/customer_id） | **现有，复用** | 无需改；执行门禁是计算值不落 Task/Project 字段 |
| Product（身份） | `core.product`（0038：归属唯一 team） | **现有，复用** | 缺执行状态字段（见 §2） |
| Product Membership | `core.product_member`（0040：PK(product_id,principal_id)，role∈{owner,member}，granted_by） | **现有，复用** | 可直接作 PA-07 验收/决策主体资格来源 |
| Product↔Project M2M | `core.product_project_link`（0038：UNIQUE(product_id,project_id)，created_by/at） | **现有，部分** | `end_link` 为删行（`products.rs:344`），无有效性历史列；PA-07 要求关系变化可审计 |
| Team / TeamMember | `core.team` / `core.team_member`（0038，P4-ORG 基线） | **现有，超出 PA-07 范围** | 本批不动 |
| **Task-Product 服务范围** | —（`delivery.task` 无 product 引用；全 migrations 无 task_product 表） | **缺失** | 新建最小关联表（有效性/范围/撤回时间/操作者/审计），IMP 数据模型批次实施 |
| Task 生命周期 | `delivery.task`（0001：13 态 CHECK + `apply_task_transition` 唯一入口） | **现有，复用** | 与规范假设一致 |
| Task Revision（不可变快照） | 仅 `delivery.task.revision bigint`（CAS 整数）+ task 行上可变 `goal/constraints/deliverables jsonb`（0015 加排期列） | **缺失（P0）** | 不可变 Revision 实体不存在 → IMP-002 核心；参照先例：`delivery.task_requirement.revision_id` 已固定 Requirement 版本 |
| Task Dependency | `delivery.task_dependency`（0001）+ `pm-application/src/task_dependency.rs` | **现有且成熟（超出规范预期）** | 服务层已有：同事务递归 CTE 环检测（审查73 达标）、项目图锁串行化并发建边、FOR UPDATE 行锁、ON CONFLICT 幂等、审计、解锁判定（D7/D8）。**缺：** `required_condition` / `required_deliverable_ref` 列（审查75/81）、updated 审计列与变更原因、Run 启动依赖快照（审查81） |
| Run 执行快照 | `delivery.task_run`（0001：仅 policy_snapshot/repository_policy_snapshots） | **部分** | 无 task_revision_id/goal/验收快照 → IMP-002/004 |
| Product 级验收 | —（`pm_acceptance.rs` 0 处 product 引用） | **缺失（P0）** | → IMP-005 新建 `delivery.task_product_acceptance` |
| Product 执行门禁（调度侧） | `pm_execution.rs` / `pm_lists.rs` / `pm_acceptance.rs` 均 **0 处** product 引用 | **缺失（P0）** | → IMP-004（Run 启动门禁+快照）/ IMP-013（Suspend E2E） |
| Workflow 模板版本 | 未核（超出本批范围） | 待核 | → IMP-006 前置小核 |
| Human Consultation 承接表 | `human_intervention_requests` / `task_waiting` 存在（审查快照） | 待核 | → IMP-006 范围 |
| 审计/Outbox | `governance.audit_log`（dependency 已接 `append_audit_with_after`）、`integration.outbox_event` | **现有，复用** | 无需新建 |

## 2. Product 状态权威字段结论

- **现有权威字段：** `core.product.status`，CHECK `{active, abandoned}`（0040，DR-PA-02 签发）——这是**业务生命周期**（存续/放弃）。
- **PA-07 要求的执行状态：** `Normal / Suspended / Resuming / Archived`（V1.0 §4.1、§11、§12）——**完全缺失**，执行链也无任何消费点。
- **裁决建议（待人类确认）：** 无存量数据，建议直接将单列改为四值 `{normal, suspended, resuming, archived}`，`abandoned` 语义并入 `archived`（0040 的「不再接收常规业务活动」与 PA-07 §12 归档语义一致）。理由：PA-07 未定义除归档外的第二种生命周期终态，双列（lifecycle + execution_state）在 5–30 人规模无实际收益。注意这是对 DR-PA-02 已签发契约的变更，需按变更原则记录决策，不得静默改。
- 附带：Suspend/Resume/Archive 的状态机入口、影响范围记录与 Outbox 事件（`product.execution_state_changed` 等）全部待建（IMP-013）。

## 3. 权限链现状

- **管理面（完整）：** `products.rs` 提供 product CRUD、membership（appoint/demote/owner 保护）、lifecycle、link 管理；错误码含 `team_owner_required / product_owner_required / not_a_product_member / product_abandoned` 等。
- **执行面（零接入）：** 派发/claim/验收/列表代码无任何 Product 检查——PA-07 统一门禁（§5.1）的 6 项检查中第 2 项（Product 允许执行）不存在消费点。

## 4. 对 IMP-002~007 的范围影响

| 任务 | 本核对后的范围修正 |
|---|---|
| IMP-002 Task Revision | 维持：新建不可变 Revision 表 + Run 引用；`task_requirement.revision_id` 可作固定版本引用的实现先例 |
| IMP-003 Dependency | **缩小**：DAG/并发/审计协议已达 PA-07 基线；剩余=加 `required_condition`/`required_deliverable_ref`/变更原因列 + 解锁判定从「前置 Completed」升级为按条件检查 + 补并发环路回归 |
| IMP-004 Run 门禁快照 | 维持（缺口最大项之一）：Revision/Product/Dependency/授权四类快照全缺 |
| IMP-005 Product 验收 | 维持：全新表 |
| IMP-006 决策边界 | 前置补核 `human_intervention_requests` / `task_waiting` / Workflow 模板版本两处小核 |
| IMP-007 审计闭环 | 维持，机制直接复用 |

## 5. 待裁决项（人类决定）

1. **Product 状态单列四值**（§2 建议）——涉及 DR-PA-02 契约变更。
2. **D6 与审查74 的张力：** 现行「仅 draft/blocked 可加边」（`task_dependency.rs` D6）严于 PA-07 审查74「已派发任务的依赖变化→启动前重检/撤回派发」。选一：放宽 D6 至全阶段+按审查74 处理，或维持 D6 并要求依赖调整先回退 Task 状态。
3. `product_project_link` 是否补有效性历史列（影响 Task-Product 表设计的一致性：两者都要求「关系变化可审计」）。
