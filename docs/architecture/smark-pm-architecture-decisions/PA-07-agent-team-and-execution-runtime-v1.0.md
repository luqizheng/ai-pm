# PA-07：Agent Team 与执行 Runtime 架构基线

- **决策编号：** PA-07-AT
- **文档版本：** V1.0
- **状态：** 当前架构基线补充
- **日期：** 2026-11-21
- **适用范围：** Project AI-PM、专业 Agent、Workstream、Agent Work Unit、Runner、Harness、Session、Workspace、Process、MCP 与执行结果回传
- **上位基线：** PA-06《AI Agent 与人类的协作边界》；PA-07《Task Execution Orchestration 与跨 Product 协作架构基线》

> 本文件补齐 PA-07 中 Agent Team 和 Runtime 的架构边界。它与 PA-07 Task / Run / Workflow 架构基线配套使用，不另建一套项目管理或执行状态体系。

## 1. 核心决策

### 1.1 一个逻辑 Project AI-PM，按需扩展专业能力

每个 Project 只有一个逻辑 AI-PM，作为项目负责人和统一协调者。它不是必须常驻运行的独立进程；具体请求可由 Worker / 模型调用承载。

AI-PM 根据工作需要按需组织专业 Agent 能力，不预先建立固定、庞大的 Agent 组织，也不要求每个专业角色长期运行。

AI-PM 负责：
- 理解并维护项目目标、约束和整体计划；
- 分析需求、拆解 Task、识别依赖与风险；
- 决定是否需要专业 Agent，以及专业工作范围；
- 组织并行执行、交接、整合和复核；
- 处理跨 Workstream 的目标冲突、资源竞争和优先级冲突；
- 统一维护项目级知识与正式研发上下文；
- 将结果映射回平台正式 Task / Run / Delivery / Review 流程。

### 1.2 多 Agent 并行，知识写入统一协调

专业 Agent 可以并行承担互不冲突的工作。并行不意味着可以各自独立修改项目级权威知识。

- 专业 Agent 在授权范围内独立调查、分析、设计、编码、测试和形成报告。
- 专业 Agent 可提交知识候选、事实证据、设计建议和变更草稿。
- AI-PM 负责整合、去重、解决冲突、检查项目上下文一致性，并决定写入何种知识层。
- 正式知识或项目级共享上下文的写入由 AI-PM 统一协调；涉及正式需求含义、重大技术方向或业务决策时仍遵守 PA-06 的人类确认边界。
- 不能因为多个 Agent 得出相同结论，就自动将未验证推断提升为正式事实。

### 1.3 人类与 Agent 的边界继承 PA-06

人类确定业务目标、关键约束、重大业务取舍和方向。AI-PM 与专业 Agent 在这些边界内自主规划、执行、验证和交付。

专业 Agent 可以自主调整实现方法，但不得突破：
- 原 Task 目标与交付范围；
- 已确认的验收条件；
- 当前授权范围；
- 资源与预算限制；
- 正式需求、业务目标和已批准架构约束。

如果需要改变 Task 目标、交付范围或验收契约，专业 Agent 必须向 AI-PM 提议；由 AI-PM 判断是否需要 Task Revision 或人类决策。不得直接修改正式业务基线。

## 2. Agent、Workstream 与 Agent Work Unit

### 2.1 专业 Agent 是能力角色，不是平台业务实体

专业 Agent 表示承担某类专业工作的逻辑角色，例如需求分析、架构分析、代码实现、测试验证、文档整理或安全审查。

Agent 角色可以由不同模型、Harness 或工具承载；角色名称不代表固定模型、固定进程或固定机器。平台 V1 不建立独立的 Agent 身份目录、Agent 组织树或 Agent 生命周期状态机。

### 2.2 Workstream

Workstream 是 Project 内用于组织一条相对连贯工作方向的逻辑工作流/上下文边界。它用于帮助 AI-PM 区分目标、背景、约束、进展和依赖，不取代正式 Task。

Workstream 可以关联一个或多个 Task；一个 Project 可以同时推进多个 Workstream。是否将 Workstream 持久化为独立数据库实体，不由本架构文件预设；V1 优先采用满足实际需要的轻量表达，不为概念完整性新增表和状态机。

### 2.3 Agent Work Unit

Agent Work Unit 是 AI-PM 分派给专业 Agent 的逻辑工作包，明确说明目标、输入上下文、输出物、约束、授权和验收方式。

- 一个 Agent Work Unit 可以服务一个或多个 Workstream。
- 跨 Workstream 执行时，必须分别明确各 Workstream 的目标、上下文、授权和输出归属。
- 一个 Agent Work Unit 不因此成为正式 Task，也不拥有独立的 Task / Run 生命周期。
- 它不是平台自有 Agent 身份、常驻 Agent 实例或另一套任务状态机。
- 专业 Agent 可以在一个工作包中完成多步工作，但正式执行授权和执行记录仍由平台 Run / Runner 体系承载。

该定义同时满足跨 Workstream 复用专业能力的需要，并避免与正式 Task / Run 模型重复。

## 3. 专业 Agent 的工作边界与协作

### 3.1 可自主决定的内容

在原工作包和授权范围内，专业 Agent 可自主：
- 选择调查路径、工具和实现方法；
- 将内部工作细分为必要步骤；
- 进行可控的内部并行；
- 根据验证结果调整实现；
- 对可恢复的技术问题进行诊断、修复和有限重试；
- 在输出中说明事实、推断、风险、未决问题和建议。

### 3.2 必须升级给 AI-PM 的情况

- 原目标、交付范围或验收条件需要变化；
- 发现跨 Task / Workstream 的依赖或冲突；
- 与其他 Agent 的结果不一致且影响项目级决策；
- 发生资源竞争、优先级冲突或共享资源风险；
- 发现可能影响正式需求、架构基线、Product 目标或其他项目级知识的事项；
- 当前授权不足、预算不足或存在重大安全/数据风险；
- 无法在既定重试与恢复边界内安全继续。

AI-PM 处理技术计划与执行协调；涉及正式业务目标和重大业务取舍时，按 PA-03 / PA-04 / PA-06 路由给相关人类，不得由 AI-PM 代替人类决定。

### 3.3 结果交接格式

专业 Agent 向 AI-PM 提交结果时，至少应包括：
- 工作包与所属 Task / Workstream 引用；
- 实际完成内容及未完成内容；
- 事实证据和来源；
- 采用的方案及关键理由；
- 变更文件、Commit、测试和交付物引用（适用时）；
- 风险、依赖、冲突和待决问题；
- 对目标、验收条件或正式知识的影响判断；
- 建议的后续动作。

结果报告不等于正式 Task 完成或业务验收；由 AI-PM 与平台既有 Delivery / Review / Acceptance 流程判断。

## 4. 执行 Runtime：平台、Runner 与 Harness

### 4.1 三层职责

| 层 | 责任 | 不负责 |
|---|---|---|
| Platform / AI-PM | Project / Task / Run 权威状态、授权、调度门禁、上下文、交付与 Review 记录、审计 | 不直接管理 Harness 内部 Agent 的全部实现细节 |
| Runner / Provider Adapter | 接收执行意图、准备 Workspace、启动/监控外部 Harness、进程生命周期、心跳、超时、取消、回执、可恢复故障处理 | 不决定业务目标，不取代 Task / Run 状态权威，不解释业务结果 |
| Harness（Codex、Claude Code、OpenCode 等） | 实际工程工作、内部 Agent 协作、工具使用、代码修改、测试、形成结构化结果 | 不直接修改平台正式 Task / Run / Product 状态，不绕过授权和交付契约 |

具体 Harness 可替换；平台不把某一厂商的内部 Agent 组织方式固化为自己的领域模型。

### 4.2 Run、Session、Job、Process 与 Workspace

这些概念属于不同层次，不应混用：

- **Task**：正式研发工作目标和验收契约。
- **Run**：平台授权的一次正式执行及其生命周期；绑定 Task Revision 和必要执行快照。
- **Session**：Harness 一次连续交互上下文，可承载多轮工具交互；它不是 Task 或 Run 的替代物。
- **Job**：Runner / Harness 侧可执行的具体工作请求或调用单元；其重试和内部状态不自动改变平台 Task 状态。
- **Process**：本机实际启动的 OS 进程及其 PID、退出码和资源。
- **Workspace**：Run / Session 使用的工作目录、Git checkout 或隔离工作区。
- **Checkpoint**：可恢复执行所需的上下文/产物引用；只有在被实际保存并验证后才能承诺恢复能力。

一个 Run 可以包含多次 Harness 调用或多个内部 Job；一个 Session 的复用必须受 Run 授权、上下文隔离和 Harness 能力约束。不得仅凭 Session 仍存在就认为 Run 仍有效。

### 4.3 Runner 的执行职责

Runner 负责：
1. 获取平台已授权的 Run 执行意图；
2. 验证 Run 标识、generation / lease 等防陈旧执行信息；
3. 准备或复用符合策略的 Workspace；
4. 通过 Harness Adapter 启动指定 Harness；
5. 管理进程、心跳、超时、取消和资源清理；
6. 将执行日志、状态、退出信息和结构化结果回传平台；
7. 对不确定的启动/退出结果进行 reconcile，避免重复执行；
8. 在安全可重试范围内处理基础设施故障，并将智能恢复决策交给 AI-PM。

Runner 不得仅凭 Harness 退出码为 Task 标记 Completed；也不得在无法确认旧进程状态时盲目启动替代 Run。

### 4.4 Harness Adapter

每种外部 Harness 通过适配层接入 Runner。Adapter 负责把平台统一执行请求转换为 Harness 所需的启动参数、环境和输入，并把原始输出转换为统一执行回执。

Adapter 不应把厂商特定 Session、子 Agent、工具调用协议泄漏到平台核心领域模型。厂商特定能力可以作为可选扩展，但核心 Run / Delivery 契约必须保持稳定。

## 5. Runner、Harness 与平台的通信边界

### 5.1 控制面与工具面分离

- **控制面**：平台向 Runner 发出受授权的 Run 执行意图；Runner 启动、监控、取消和回收 Harness。
- **工具/上下文面**：Harness 在授权范围内通过 MCP 或受控工具访问平台上下文、任务资料和允许的操作。
- **结果面**：Runner 将执行结果、证据和交付引用提交平台；平台校验身份、Run generation、lease、幂等键和交付契约后入库。

MCP 是 Agent 与平台之间的通信/工具接口，不是 Runner，不是进程管理器，也不是执行 Runtime。不得用 MCP 调用本身替代 Runner 的 spawn、heartbeat、cancel、reconcile 和资源清理职责。

### 5.2 权威写入

Harness 可以提交结果、事实、交付物和建议，但不得直接写入正式 Task 状态、Task Revision、Product 状态、Dependency 或正式验收结论。

正式状态变更必须经过平台受守卫的应用服务/命令通道。AI-PM 负责判断和协调；平台负责校验权限、状态转换、幂等、版本竞争、审计和 Outbox。

### 5.3 通信协议的稳定边界与实现细节

本基线冻结职责边界和数据流，不将某个 CLI 参数、stdio 消息格式、HTTP 回调路径或厂商 Session API 写成跨 Harness 的领域契约。

具体传输协议应在 Runner / Harness Adapter 实施设计中定义，并至少满足：
- 身份与 Run 绑定；
- 最小权限和凭据隔离；
- 请求/回执幂等；
- 超时、取消和迟到回报可处理；
- 日志与回执脱敏；
- 进程状态不确定时可对账；
- 不允许旧 Run / 旧 generation 的回报覆盖新状态。

## 6. 上下文与知识管理

### 6.1 分层上下文

AI-PM 负责为专业 Agent 构建最小充分上下文，至少区分：
- Project 目标、约束和技术基线；
- 相关 Product 业务上下文及其授权边界；
- 当前 Task / Revision / Run 契约；
- 所属 Workstream 的目标和局部进展；
- 必要的技术设计、ADR、历史决策和证据；
- 允许访问的 Repository、文件和工具范围。

跨 Workstream Agent Work Unit 必须显式携带每个 Workstream 的上下文边界，不能把多个目标混成一个无差别上下文。

### 6.2 事实、推断与建议

Agent 输出必须区分：
- 已验证事实及其证据；
- 基于事实的推断；
- 专业建议；
- 尚未确认的假设和问题。

AI-PM 在合并多 Agent 结果时，负责去重、冲突检查、来源追踪和知识层级判断。不得把推断或建议直接升级为事实或正式决策。

### 6.3 Markdown 与 Git 知识权威

遵循现有知识架构决策：项目知识正文以 Markdown + Git 为权威；PostgreSQL 保存 ID、状态、版本引用、审批/状态机、审计与事件；PageIndex 等检索索引为可重建派生数据。

多 Agent 的知识变更先作为候选或变更集提交，由 AI-PM 统一协调写入和版本化；不得让并行 Agent 无协调地覆盖同一份权威文档。

## 7. 并行执行、资源竞争与恢复

- 可以并行执行目标独立、上下文清晰、授权不冲突的 Agent Work Unit。
- 同一共享资源、Repository、文件或环境存在写冲突风险时，应由 AI-PM 划分工作区、安排顺序或指定整合责任人。
- 不同 Workstream 的优先级或资源冲突由唯一 AI-PM 统一分析与协调；若冲突实质改变正式交付承诺、业务目标或重大资源取舍，交由相关人类决定。
- 一个 Workstream 暂停，不自动取消 Agent Work Unit 在其他 Workstream 中明确独立且仍获授权的工作。
- 一个 Agent Work Unit 跨多个 Workstream 时，任何一个范围被暂停，AI-PM 必须重新检查该工作包中对应部分；边界明确的无关部分可以继续。
- 基础设施故障和安全可重试故障由 Runner 自动处理；需要判断修复策略、阶段重跑、Workflow 调整或 Harness 切换时交由 AI-PM。
- 无法确认执行安全、权限失效或触及人类业务决策边界时，局部暂停并升级，不以自动重试绕过门禁。

## 8. 与现有 PA-07 Task Execution 基线的关系

| 主题 | 权威文档 |
|---|---|
| Task / Task Revision / Task Dependency | [PA-07 Task Execution 基线](PA-07-task-execution-orchestration-cross-product-collaboration-v1.0.md) |
| Run / Workflow / Delivery / Review / Product 门禁 | [PA-07 Task Execution 基线](PA-07-task-execution-orchestration-cross-product-collaboration-v1.0.md) |
| AI-PM / 专业 Agent / Workstream / Agent Work Unit | 本文件 |
| Runner / Harness / Session / Job / Process / Workspace | 本文件及其后续实施规范 |
| 人类与 Agent 的授权边界 | [PA-06](PA-06-agent-human-collaboration.md) |
| Product / Project 职责和业务决策路由 | PA-03 / PA-04 与 [设计原则](00-design-principles.md) |

本文件不创建新的 Task、Run、Delivery、Review、Product 或 Project 状态机。Agent 内部实现细节也不应反向污染平台核心领域模型。

## 9. V1 非目标

- 不建立平台自有、持久化的 Agent 身份/组织树/生命周期子系统。
- 不建立独立于 Task / Run 的 Agent Work Unit 状态机。
- 不把 Harness 内部子 Agent、Session 或 Job 映射成平台正式 Task。
- 不把 MCP 当作执行 Runtime 或进程控制协议。
- 不把 Runner 变成 AI 项目经理或业务决策者。
- 不建设分布式 Agent 调度集群、消息总线或独立恢复微服务。
- 不要求每个专业 Agent 常驻运行。
- 不允许 Harness 绕过平台命令通道修改正式业务状态。
- 不允许 Agent 越权改变正式需求、Product 目标、重大业务取舍或正式授权边界。

## 10. 后续实施映射

后续实施设计按现有 PA-07 Developer Implementation Specification 和 Runner 代码基线逐项核对，不在本文件中臆造表名或 API：

1. Run 与 Harness invocation / Session / Job 的关联及生命周期映射；
2. Runner Adapter 的统一输入、输出、取消、超时和回执契约；
3. Runner 与 Harness 的身份、lease、generation 和幂等校验；
4. Workspace 隔离、复用、清理与保留策略；
5. Checkpoint 的实际能力、恢复边界和验证标准；
6. 多 Agent 输出的结构化交接与知识写入协调；
7. 日志、事件、审计和敏感凭据脱敏；
8. 并行执行时的 Repository / 文件 / 环境冲突控制。

以上是实施核对项，不表示对应实现已经完成。通信协议细节应在代码基线调查后冻结。
