# PA07-IMP-003：PA-07 原始实施审计后的差异复核

- **状态：** IMPLEMENTATION EVIDENCE
- **复核日期：** 2026-11-22
- **仓库：** `luqizheng/ai-pm`
- **分支：** `master`
- **复核 HEAD：** `6ed0f1a5e7339cd701e538c2a13513cd9c0e9adb`
- **原始审计基准：** `75094e1c2b88719d34364373b338609ed3805b32`
- **复核方式：** Git compare、提交 diff、当前源码、测试源码和相关规范交叉核对
- **范围限制：** 本报告只核验原始 IMP-002 之后的提交差异及其中可确认的门禁问题；不是对 IMP-001~019 的全量重新实跑。

## 1. 结论摘要

| 项目 | 当前判定 | 证据 |
|---|---|---|
| E2：AI 经命令通道将 Task 置为 Ready | **RESOLVED** | `6ed0f1a` 单点守卫 + AiPm/DevAgent 拒绝、Human 放行测试 |
| P2：批量 Task 的 blast radius 低估 | **OPEN** | 当前编排器按 proposal 数量计数；单条 CreateTasks 可承载多条 Task |
| IMP-001~019 其余状态 | **INHERITED — NOT RE-VERIFIED** | 继续引用 IMP-002 的原始 `75094e1` 核对结果，不声称本报告重新核验 |

## 2. 原始审计与提交差异

Git compare `75094e1...6ed0f1a` 显示：

- 后续共有两个提交。
- 变更文件为：
  - `crates/pm-application/src/command_executor/transition.rs`
  - `crates/pm-application/tests/command_executor.rs`
  - `docs/architecture/pa-07-implementation/PA07-IMP-002-implementation-audit.md`

因此，原始 IMP-002 的 E2 结论必须按后续代码修复更新；其余原始实施判定不能自动升级为新的 HEAD 核验结论。

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

**判定：OPEN — current source confirms the gap.**

### 当前代码证据

- `crates/pm-ai/src/orchestrator.rs`：当前以 `result.proposals.len()` 计算 `blast_radius`，再将同一个值传给每条提案的 self-review。
- `crates/pm-ai/src/self_review.rs`：风险判断按 proposal 级别应用阈值；当前自审函数不读取命令的批量项目数量。
- `crates/pm-domain/src/command.rs`：`ProjectCommand::CreateTasks` 持有 `tasks: Vec<TaskDraft>`。
- `crates/pm-application/src/command_executor/guards.rs`：单条批量命令上限为 `MAX_BATCH_ITEMS = 200`。
- `crates/pm-application/src/project_import/proposal.rs`：导入路径按 200 条分块；这只能限制单条命令大小，不能解决 AI 提案的 proposal 数量与实际影响 Task 数量不一致的问题。
- `crates/pm-application/src/policy_engine.rs`：`create_tasks` 不属于红线命令；策略注释将大影响面升级与 `blast_radius > 10` 联系起来。

### 具体影响

当一条高置信度 `CreateTasks` 提案包含 11–200 个 Task 时，当前 proposal 数量仍为 1。若其他升级条件均未命中，proposal 数量型 blast radius 不会触发阈值 10 的升级判断。

这与策略注释中“大批量应通过 blast-radius 升级”的预期不一致。

### 推荐修复

将 blast radius 定义为**提案所代表的受影响工作项数量**，而不是提案记录条数：

- 普通单对象命令计为 1。
- `CreateTasks` 按 `tasks.len()` 计数。
- 其他批量命令按其项目数量计数。
- 对一轮 proposals 求和，使用饱和加法避免整数溢出。
- 仍保持现有阈值 `> 10`，不改变 `create_tasks` 的授权类别，也不把它重新加入红线。
- 自审只读取批量集合的 cardinality，不读取模型控制的任务标题、描述或其他内容，不扩大 prompt-injection 输入面。

应补充阈值边界测试：10 个受影响 Task 不因 blast radius 单独升级；11 个受影响 Task 必须升级。还应覆盖多条提案合计影响数量超过阈值的情况。

**本报告不声称该修复已经完成。** 在代码和测试提交完成前，该项保持 OPEN。

## 5. PA-06 / PA-07 授权一致性

E2 修复落实了 PA-06 的授权边界：AI 可以分析、拆解和创建授权范围内的 Draft Task，但不能自行完成 Draft → Ready 的人工确认门。

推荐的 P2 修复只调整风险规模的计算，不改变 AI 创建执行性 Task 的授权原则：

- 不恢复 `create_tasks` 红线。
- 不把普通任务拆解改成逐条人工审批。
- 仅当单轮提案实际影响工作项数量超过现有阈值时触发升级。
- 正式业务目标、验收条件和权限边界仍由 PA-06 控制。

## 6. 后续核验边界

IMP-002 的 IMP-001~019 统计（9 已完成、5 部分完成、5 未完成）仍是原始 HEAD `75094e1` 的审计快照。本报告确认原始审计之后的 E2 修复，并复核了 P2 问题仍存在；它没有重新运行其余 19 项的全部代码、迁移和测试核验。

后续如需对全部 IMP-001~019 出具当前 HEAD 的正式状态，应逐项重新核对对应实现、migration、专项测试和运行结果，并为每项记录最新证据 commit。

## 7. 参考

- [PA-07 Task Execution / Cross-Product Collaboration 基线](../smark-pm-architecture-decisions/PA-07-task-execution-orchestration-cross-product-collaboration-v1.0.md)
- [PA-07 Agent Team / Execution Runtime 基线](../smark-pm-architecture-decisions/PA-07-agent-team-and-execution-runtime-v1.0.md)
- [PA-07 Developer Implementation Specification](../smark-pm-architecture-decisions/PA-07-developer-implementation-specification-v1.0.md)
- [PA07-IMP-001 历史 Schema Mapping](PA07-IMP-001-schema-mapping.md)
- [PA07-IMP-002 原始实施审计](PA07-IMP-002-implementation-audit.md)
- [PA-06 协作边界基线](../smark-pm-architecture-decisions/PA-06-agent-human-collaboration.md)
