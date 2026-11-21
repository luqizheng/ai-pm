# PA07-IMP-003：PA-07 原始实施审计后的差异复核

- **状态：** IMPLEMENTATION EVIDENCE
- **复核日期：** 2026-11-22
- **仓库：** `luqizheng/ai-pm`
- **分支：** `master`
- **原始差异复核 HEAD：** `6ed0f1a5e7339cd701e538c2a13513cd9c0e9adb`
- **代码修复核对 HEAD：** `4c34d1550968bbcdf051ba3791e8901481131ac8`
- **原始审计基准：** `75094e1c2b88719d34364373b338609ed3805b32`
- **复核方式：** Git compare、提交 diff、当前源码、测试源码和相关规范交叉核对
- **范围限制：** 本报告只核验原始 IMP-002 之后的提交差异及其中可确认的门禁问题；不是对 IMP-001~019 的全量重新实跑。

## 1. 结论摘要

| 项目 | 当前判定 | 证据 |
|---|---|---|
| E2：AI 经命令通道将 Task 置为 Ready | **RESOLVED** | `6ed0f1a` 单点守卫 + AiPm/DevAgent 拒绝、Human 放行测试 |
| P2：批量 Task 的 blast radius 低估 | **IMPLEMENTED — TESTS NOT EXECUTED HERE** | `impact_count()` + 编排器按整轮实际影响工作项数量饱和求和；新增 10/11 条目阈值测试源码 |
| IMP-001~019 其余状态 | **INHERITED — NOT RE-VERIFIED** | 继续引用 IMP-002 的原始 `75094e1` 核对结果，不声称本报告重新核验 |

## 2. 原始审计与提交差异

截至文档整理后的 master HEAD `b6a42a6dd240dd6db605e5a00e2368378938bd8d`，Git compare `75094e1...b6a42a6` 显示后续共有 12 个提交、14 个变更文件。与 PA-07 实施代码有关的变化集中在：

- `crates/pm-application/src/command_executor/transition.rs` 与 `crates/pm-application/tests/command_executor.rs`：E2 Ready actor 守卫和边界测试。
- `crates/pm-domain/src/command.rs`：命令影响条目数计算及领域单测。
- `crates/pm-ai/src/orchestrator.rs` 与 `crates/pm-ai/src/self_review.rs`：批量影响范围聚合、阈值和跨提案聚合测试。
- `crates/pm-application/src/policy_engine.rs`、`crates/pm-application/tests/policy_engine.rs`、`policies/ai-pm-ai-lead-v2.yaml`：策略说明注释同步。

其余变更为 PA-07 文档整理。对照 IMP-002 的原始代码核对基准，未发现其余 IMP-001~019 对应实施模块在这 12 个后续提交中发生代码变更；但“没有后续 diff”不等于本次重新运行或重新验证了这些模块，其状态仍明确标为 inherited。

Git compare `75094e1...6ed0f1a` 显示 E2 修复前后的差异仅涉及：

- 后续共有两个提交。
- 变更文件为：
  - `crates/pm-application/src/command_executor/transition.rs`
  - `crates/pm-application/tests/command_executor.rs`
  - `docs/architecture/pa-07-implementation/PA07-IMP-002-implementation-audit.md`

因此，原始 IMP-002 的 E2 结论必须按后续代码修复更新；blast-radius 相关代码也已在后续提交修改。其余原始实施判定不能自动升级为新的 HEAD 核验结论。

## 3. E2 命令通道缺口

**判定：RESOLVED — verified against current HEAD.**

### 原始问题

原始审计发现 AI-PM 可以提出 `UpdateTaskStatus{to: Ready}`，在自审判定为 Auto 后以 AiPm 主体执行，绕过 E5 的人工 Ready 确认边界。

### 修复证据

提交：[`6ed0f1a`](https://github.com/luqizheng/ai-pm/commit/6ed0f1a5e7339cd701e538c2a13513cd9c0e9adb)

- `crates/pm-application/src/command_executor/transition.rs`：在统一状态转换入口 `apply_task_transition` 增加 actor 守卫。迁入 Ready 仅允许 Human 和 Service；AiPm / DevAgent 被拒绝。
- `crates/pm-application/tests/command_executor.rs`：增加三条边界测试：
  - AiPm 经命令通道迁入 Ready 被拒绝，Task 状态和 revision 不变。
  - DevAgent 经命令通道迁入 Ready 被拒绝。
  - Human 经命令通道迁入 Ready 仍然允许。
- Service 放行用于平台依赖解锁路径，避免破坏 D7/D8 下游解锁行为。

### 测试证据边界

`6ed0f1a` 提交说明记录 12 个测试套件 135/135 通过，包括 command_executor 13 项及 task_dependency 18 项等。该结果作为提交所报告的测试证据保留；本次 GitHub 源码审查没有在本地重新运行这些测试。

**结论：** 原始 E2 缺口已由当前 HEAD 的单点状态转换守卫关闭，不应继续列入当前待修复清单。

## 4. P2：批量 Task 的 blast radius 低估

**判定：IMPLEMENTED — static source review passed; test execution pending.**

### 修复提交与代码证据

- `626c6e41bd1e1012964efe7b82ec17b7527e25bb`：新增 `ProjectCommand::impact_count()`，批量命令按集合长度计数，普通单对象命令计为 1；增加领域单测覆盖 11 个 Task、批量里程碑/需求和单对象命令。
- `a69297284b75262cb75c8ae04bb6843073f1d985`：编排器对整轮 proposals 的 `impact_count()` 做饱和求和，并继续将聚合值传给 self-review。
- `e0986bebcd617918c4ee4b343954034b58b891f7`：增加 self-review 回归测试，验证单条 CreateTasks 提案含 10 个 Task 不因 blast radius 升级，含 11 个 Task 会升级。
- `4c34d1550968bbcdf051ba3791e8901481131ac8`：将整轮聚合抽为 `total_blast_radius()`，新增 6 + 5 个 Task 分属两条提案时合计 11 的单元测试。
- `crates/pm-ai/src/self_review.rs`：风险判断顺序保持不变，red line 仍优先于低置信度和 blast radius；函数只接收聚合整数，不读取命令内容。
- `crates/pm-application/src/policy_engine.rs`、`policies/ai-pm-ai-lead-v2.yaml` 和 policy 测试副本的注释已同步，明确阈值按整轮实际影响工作项数量计算。

### 当前语义

`blast_radius = saturating_sum(proposal.command.impact_count())`。

- 普通单对象命令计为 1。
- CreateTasks / CreateRequirements / CreateMilestones / WriteDocuments 按条目数计数。
- 阈值仍是严格大于 10；10 不升级，11 升级。
- 不改变 `create_tasks` 的授权类别，不把它重新加入红线，也不改变 PA-06 的 AI Task 创建授权。
- 不读取模型控制的标题、描述等内容，避免扩大 prompt-injection 输入面。

### 测试执行状态

测试源码已补充，但本次会话没有成功运行 Cargo 测试；GitHub 对相关提交返回的 commit status 为 `pending` 且没有状态检查记录。因此本报告只确认代码和测试源码已提交，并完成静态交叉核对；**不宣称测试已通过**。合并/发布前仍需在可运行 Rust 工具链和 PostgreSQL 测试环境中执行：

- `cargo test -p pm-domain impact_count_uses_batch_cardinality_and_single_commands_count_one`
- `cargo test -p pm-ai create_tasks_batch_cardinality_drives_blast_radius_threshold`
- `cargo test -p pm-ai blast_radius_sums_batch_items_across_multiple_proposals`
- `cargo test -p pm-ai`
- `cargo test -p pm-application`

**结论：** 原 P2 代码缺陷已实施修复，静态审查通过；测试执行仍是发布前验证项。

## 5. PA-06 / PA-07 授权一致性

E2 修复落实了 PA-06 的授权边界：AI 可以分析、拆解和创建授权范围内的 Draft Task，但不能自行完成 Draft → Ready 的人工确认门。

P2 修复只调整风险规模的计算，不改变 AI 创建执行性 Task 的授权原则：

- 不恢复 `create_tasks` 红线。
- 不把普通任务拆解改成逐条人工审批。
- 仅当单轮提案实际影响工作项数量超过现有阈值时触发升级。
- 正式业务目标、验收条件和权限边界仍由 PA-06 控制。

## 6. 后续核验边界

IMP-002 的 IMP-001~019 统计（9 已完成、5 部分完成、5 未完成）仍是原始 HEAD `75094e1` 的审计快照。本报告确认原始审计之后的 E2 修复，并记录 P2 修复代码和测试源码已提交；但未在本次会话重新运行 Rust 测试，也没有重新核验其余 19 项的全部代码、迁移和测试。

后续如需对全部 IMP-001~019 出具当前 HEAD 的正式状态，应逐项重新核对对应实现、migration、专项测试和运行结果，并为每项记录最新证据 commit。

## 7. 参考

- [PA-07 Task Execution / Cross-Product Collaboration 基线](../smark-pm-architecture-decisions/PA-07-task-execution-orchestration-cross-product-collaboration-v1.0.md)
- [PA-07 Agent Team / Execution Runtime 基线](../smark-pm-architecture-decisions/PA-07-agent-team-and-execution-runtime-v1.0.md)
- [PA-07 Developer Implementation Specification](../smark-pm-architecture-decisions/PA-07-developer-implementation-specification-v1.0.md)
- [PA07-IMP-001 历史 Schema Mapping](PA07-IMP-001-schema-mapping.md)
- [PA07-IMP-002 原始实施审计](PA07-IMP-002-implementation-audit.md)
- [PA-06 协作边界基线](../smark-pm-architecture-decisions/PA-06-agent-human-collaboration.md)
