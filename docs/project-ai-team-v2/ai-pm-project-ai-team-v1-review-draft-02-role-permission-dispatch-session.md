# Project AI Team V1.0 产品需求与架构设计基线（评审稿）· 分篇 2/5：Role 定义、权限矩阵、任务调度与工作会话（§4–§8）

> 状态：评审稿转录 · 未冻结 · Q2–Q4 待确认（见分篇 4）
> 记录日期：2026-11-08 · 来源与拆分说明见 [00-index](ai-pm-project-ai-team-v1-review-draft-00-index.md)

---

## 4. Role Definition：角色定义模型

每一个 Role 必须拥有正式的、可版本化的定义。

建议将 Role Definition 分为六组。

| 字段组 | 字段 | 说明 |
| --- | --- | --- |
| Identity | role_key、name、category、version | 身份与版本 |
| Responsibilities | responsibilities、scope、constraints | 职责和边界 |
| Capabilities | capabilities、skills、instructions | 能力与执行规范 |
| Contract | input_schema、output_schema | 输入输出契约 |
| Governance | permissions、risk_level、approval_rules | 权限与审批 |
| Execution | model_policy、client_requirements、acceptance_criteria | 模型、Client 和验收 |

### 4.1 一个 Role 的完整定义示例

（原文为卡片式呈现：Requirement Analyst · Professional · P0 · Version 1.0）

**Responsibilities**

- 原始需求理解。
- 业务目标分析。
- 需求结构化。
- 缺失信息识别。
- 需求冲突分析。
- 澄清问题生成。

**Capabilities**

- requirement_analysis
- requirement_decomposition
- requirement_clarification
- requirement_consistency_check

**Inputs**

- Raw User Input
- Product Cognition
- Project Cognition
- Existing Requirements
- Related Knowledge

**Outputs**

- Requirement Draft
- Analysis Report
- Clarification Questions
- Gap Findings
- Conflict Findings

**Permission Constraints**

- 可以读取项目授权范围内的知识。
- 可以创建需求草稿。
- 可以提出正式需求修改建议。
- 不得自行正式确认需求。

**Acceptance Criteria**

- 分析结果完整。
- 问题与建议有依据。
- 不确定信息被明确标注。
- 输出符合需求契约。

### 4.2 Role 版本管理

建议采用不可变版本。

```text
requirement_analyst
  ├── v1.0
  ├── v1.1
  └── v2.0
```

Task 创建时记录使用的 Role Version。

这样后续 Role 修改不会导致历史执行结果无法解释。

---

## 5. 权限矩阵

权限设计必须区分 Role 的专业职责和实际执行权限。

建议采用以下权限动作：

| 权限 | 含义 |
| --- | --- |
| READ | 读取 |
| ANALYZE | 分析 |
| PROPOSE | 提出修改建议 |
| WRITE_DRAFT | 写入草稿 |
| EXECUTE | 执行已授权操作 |
| APPROVE | 正式批准或确认 |

其中 APPROVE 并不是普通 Agent Role 的默认权限。

### 5.1 初始权限矩阵

下表是建议的业务权限基线，具体资源还需叠加 Project Scope 和用户授权。

| Role | READ | ANALYZE | PROPOSE | WRITE_DRAFT | EXECUTE | APPROVE |
| --- | --- | --- | --- | --- | --- | --- |
| AI Project Manager | ✓ | ✓ | ✓ | ✓ | 受授权 | — |
| Requirement Analyst | ✓ | ✓ | ✓ | ✓ | — | — |
| Product Cognition | ✓ | ✓ | ✓ | ✓ | — | — |
| Project Cognition | ✓ | ✓ | ✓ | ✓ | — | — |
| Knowledge Manager | ✓ | ✓ | ✓ | ✓ | 受授权 | — |
| Solution Architect | ✓ | ✓ | ✓ | ✓ | — | — |
| Design Analyst | ✓ | ✓ | ✓ | ✓ | — | — |
| Developer | ✓ | ✓ | ✓ | 受授权 | 受授权 | — |
| QA Engineer | ✓ | ✓ | ✓ | ✓ | 受授权 | — |
| Risk Analyst | ✓ | ✓ | ✓ | ✓ | — | — |

重要说明：

1. Developer 的 EXECUTE 仅限授权的工程操作，不等于拥有全部 Repository 权限。
2. Knowledge Manager 的 WRITE_DRAFT 不等于拥有知识正式确认权。
3. AI Project Manager 可以创建和组织工作，但不能绕过正式审批。
4. 即使 Client 本身有系统权限，也不能超过 Platform 为当前 Task 授予的权限。
5. Team Member 的权限、Project Member 的权限和 AI Role 的权限需要共同决定最终有效权限。

建议最终有效权限计算为：

```text
P_effective = P_team ∩ P_project ∩ P_role ∩ P_client ∩ P_task
```

其中任何一层没有授权，都不能执行对应操作。

---

## 6. Task Dispatch：任务调度与协作机制

这是整个 AI Team 最核心的技术机制之一。

### 6.1 调度基本原则

AI Project Manager 不直接指定某个 Client，而是根据 Task 的要求，由 Platform Dispatch Engine 自动选择执行资源。

调度流程：

1. AI Project Manager 创建 AI Task。
2. 根据工作内容确定 Responsible Role。
3. 读取 Role 的 Capability 和 Execution Requirements。
4. 查找已授权的 Agent Clients，以及可用的 Native AI 能力。
5. 过滤不满足 Capability、权限和可用性要求的资源。
6. 根据模型策略、优先级和调度规则选择执行资源。
7. 创建 Task Assignment 和 Task Execution。
8. 执行完成后进入 Review，交由 AI Project Manager 汇总验收。

### 6.2 Client 选择依据

建议按照以下优先顺序：

| 条件 | 说明 |
| --- | --- |
| Role Capability | 是否具备所需专业能力 |
| Authorization | 是否获得该 Project 授权 |
| Task Permission | 是否允许执行当前操作 |
| Availability | 是否在线、可用 |
| Model Policy | 是否符合 Role 模型策略 |
| Execution Constraints | 是否支持当前任务的执行方式 |
| Priority | 是否满足任务优先级和资源策略 |

不要把 Client 的名称作为调度规则。

例如，不应该简单规定：

```text
Developer → Codex
QA → Claude Code
```

而应该是：

```text
Developer Task
  → Required Capability
  → Authorized Clients
  → Available Clients
  → Model Policy
  → Dispatch
```

### 6.3 没有合适 Client 时

必须有明确的处理机制。

| 场景 | 处理 |
| --- | --- |
| Native AI 能力符合要求 | 可以使用 Native AI |
| 外部 Client 可用 | 正常分配 |
| Client 暂时离线 | WAITING_FOR_CLIENT |
| 没有任何符合条件的 Client | BLOCKED |
| Client 授权不足 | BLOCKED / NEEDS_AUTHORIZATION |
| Client 执行失败 | FAILED 或进入重试策略 |
| 需要人类批准 | NEEDS_HUMAN |

不能因为用户提交了工作，就让 AI Project Manager 假装工作已经执行。

### 6.4 并行与依赖

建议 AI Task 支持：

- Sequential：顺序执行。
- Parallel：并行执行。
- Dependency：前置任务依赖。
- Conditional：满足条件后执行。
- Human Gate：等待人工确认后继续。

例如：

```text
Work Session: 用户登录需求分析

Task A: 需求分析
Task B: 产品认知检查
Task C: 历史需求冲突检查

       A + B + C
            |
            v
Task D: AI Project Manager 汇总
            |
            v
Task E: 需求澄清
            |
            v
Task F: 人工正式确认
```

Task D 必须等待必要的上游任务完成或进入可处理的异常状态。

不能让尚未完成的 Agent 结果被误认为完整分析结果。

---

## 7. AI Task 与 TaskExecution 的生命周期

此前已经确定 Task 和 TaskExecution 分离。

建议进一步明确其含义：

- AI Task 表示需要完成的工作。
- TaskExecution 表示某次实际执行尝试。
- Task Assignment 表示将 Task 分配给哪个 Role / Client。

例如，同一个 Task 可以因为 Client 失败而重新执行，但它仍然是同一个 Task。

### 7.1 AI Task 状态

（原文为生命周期图，粘贴文本中图形丢失，以下按原稿文字项重建。）

| 状态 | 含义 |
| --- | --- |
| CREATED | 任务已创建 |
| READY | 满足执行条件 |
| ASSIGNED | 已分配资源 |
| RUNNING | 正在执行 |
| REVIEW | 结果待验收 |
| COMPLETED | 工作完成 |
| BLOCKED | 执行被阻塞 |
| FAILED | 执行失败 |
| NEEDS_HUMAN | 等待人工介入 |
| CANCELLED | 任务取消 |

这里需要注意，BLOCKED、NEEDS_HUMAN 和 FAILED 不应该被当成普通完成状态。

它们代表不同的恢复和处理路径。

### 7.2 TaskExecution 状态

建议独立记录：

- CREATED
- DISPATCHED
- RUNNING
- SUCCEEDED
- FAILED
- CANCELLED
- TIMEOUT

Task 的状态由所有相关 Execution、依赖和验收结果综合决定。

不能简单地把一次 Execution 成功等同于 Task 完成。

### 7.3 结果验收

Task 完成之前，至少要经过：

1. 执行结果接收。
2. 输出契约检查。
3. Acceptance Criteria 检查。
4. 权限与操作结果检查。
5. 需要时进行独立 Review。
6. 由 AI Project Manager 汇总并更新工作状态。

特别是开发任务，代码已经生成不代表工程交付已经完成。

---

## 8. Work Session 与 AI Task 的边界

原稿认为这个领域模型非常值得独立定义。

用户此前提出的体验要求是：用户可以随时切换工作、暂停工作、继续回答问题，而不能丢失上下文。这需要 Work Session 来承载。

### 8.1 两者的定义

| 维度 | Work Session | AI Task |
| --- | --- | --- |
| 面向对象 | 用户 | Platform |
| 目的 | 承载完整工作过程 | 执行具体工作 |
| 是否可包含多个任务 | 是 | 否 |
| 是否可以暂停 | 是 | 是 |
| 是否可以恢复 | 是 | 是 |
| 是否需要 Agent Client | 不一定 | 视任务类型而定 |
| 是否有执行结果 | 工作汇总 | 单个任务结果 |

### 8.2 关系模型

```text
Project
  |
  +-- Conversation
        |
        +-- Work Session A
        |     |
        |     +-- AI Task A1
        |     +-- AI Task A2
        |     +-- AI Task A3
        |
        +-- Work Session B
              |
              +-- AI Task B1
              +-- AI Task B2
```

这里还需要区分：

- Conversation：用户和 AI Project Manager 的交流载体。
- Work Session：一次具体业务工作的生命周期。
- AI Task：Work Session 内部可以调度的工作单元。
- TaskExecution：AI Task 的具体执行尝试。

一个 Conversation 可以有多个 Work Session。

用户切换 Work Session，不应该清空 Conversation，也不应该重建之前的 AI Task。

### 8.3 Work Session 生命周期

建议：

| 状态 | 含义 |
| --- | --- |
| ACTIVE | 当前正在进行 |
| PAUSED | 用户或系统主动暂停 |
| WAITING_FOR_HUMAN | 等待用户输入或确认 |
| WAITING_FOR_EXECUTION | 等待 Task 执行 |
| COMPLETED | 工作结束 |
| CANCELLED | 工作取消 |

特别注意：

> 用户只是暂时转去问其他问题，并不意味着原 Work Session 自动暂停或取消。
>
> 是否暂停，应由明确的工作状态和运行策略决定。
