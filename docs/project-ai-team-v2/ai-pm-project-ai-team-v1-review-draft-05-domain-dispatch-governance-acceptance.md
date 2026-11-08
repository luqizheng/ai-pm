# Project AI Team V1.0 产品需求与架构设计基线（评审稿）· 分篇 5/5：领域模型深化、执行治理与验收标准（§16–§22）

> 状态：评审稿转录 · 未冻结 · Q2–Q4 仍待确认
> 记录日期：2026-11-08 · 来源与拆分说明见 [00-index](ai-pm-project-ai-team-v1-review-draft-00-index.md)
> 本篇为第一轮（§0–§15，见分篇 1~4）之后用户回复「继续」触发的第二轮深化。原稿本轮明确声明：**不把 Q2、Q3、Q4 的推荐方案当成用户裁决，也不重复要求立即选择**；本轮补齐 Domain 模型、核心对象关系、调度规则、状态转换、异常处理和验收标准，使后续 Q2–Q4 选择变化时底层架构不需要推倒重来。

---

## 16. 核心 Domain 模型

### 16.1 Domain 边界

建议 Project AI Team 划分为六个逻辑子域。

| 子域 | 职责 | 核心对象 |
| --- | --- | --- |
| Team Organization | 项目 AI 团队组织 | ProjectAITeam、ProjectAIRole |
| Role Definition | Role 定义与版本 | AIRoleTemplate、AIRoleVersion |
| Client Management | Client 注册、授权和能力 | AIClient、ProjectAIClientBinding |
| Dispatch & Execution | 调度、分配和执行 | AITask、AITaskAssignment、AITaskExecution |
| Work Coordination | 用户工作过程与任务关系 | WorkSession、WorkSessionTask |
| Governance | 权限、审批、审计 | PermissionGrant、Approval、TeamEvent |

这六个子域不意味着必须建设六套微服务。V1.0 可以先在现有后端中按 Domain 模块组织，避免过早拆分服务。

### 16.2 建议核心实体

（原文为实体关系图，粘贴文本中图形丢失，以下按原稿文字项重建。）

```text
project_ai_team（项目级 AI 团队根对象，关联 Project）
  │
  ├── Role：project_ai_role ── Role Version：ai_role_version
  │
  ├── Client：ai_client ── Project Binding：project_ai_client_binding
  │
  ├── WorkSession：ai_work_session ── WorkSessionTask：ai_work_session_task
  │
  ├── AITask：ai_task
  │     ├── TaskAssignment：ai_task_assignment
  │     ├── TaskExecution：ai_task_execution
  │     └── TaskDependency：ai_task_dependency
  │
  └── Governance & Audit：permission grants ／ approvals ／ team events
```

这里需要特别强调：AI Project Manager 不应仅仅作为一个 ai_role 普通记录存在。它是 Project AI Team 的管理角色，具有独立的身份、职责和系统约束。

可以通过 project_ai_team.manager_role_id 指向对应的 Role Definition，但需要保证每个 Project AI Team 只能有一个有效的 Manager。

### 16.3 实体职责和关键字段

| Entity | 关键字段建议 | 说明 |
| --- | --- | --- |
| project_ai_team | id、project_id、manager_role_id、status | 项目 AI 团队 |
| ai_role_template | role_key、category、is_system | 平台预置 Role 模板 |
| project_ai_role | id、team_id、role_key、enabled、current_version_id | 项目启用的 Role |
| ai_role_version | id、role_id、version、definition、created_at | 不可变角色版本 |
| ai_client | id、owner_user_id、client_type、status | 个人 Client |
| project_ai_client_binding | id、project_id、client_id、granted_by、scope、status | 项目授权 |
| ai_task | id、project_id、role_version_id、status、priority | 工作任务 |
| ai_task_assignment | id、task_id、client_id、role_id、status | 执行分配 |
| ai_task_execution | id、task_id、assignment_id、attempt_no、status | 一次执行尝试 |
| ai_work_session | id、project_id、conversation_id、status | 用户工作过程 |
| ai_work_session_task | work_session_id、task_id、relation_type | Session 与 Task 关联 |
| ai_task_dependency | predecessor_task_id、successor_task_id、dependency_type | 任务依赖 |

> 以上是建议模型，不代表已经检查过当前代码库并确认这些表都不存在。正式进入数据库设计时，应先检查现有实体和表，复用已有模型，避免重复建模。

---

## 17. 核心关系与数据约束

建议采用以下关系约束。

| 关系 | 基数 | 约束 |
| --- | --- | --- |
| Project → Project AI Team | 1:1 | 每个 Project 一个 AI Team |
| Project AI Team → AI Project Manager | 1:1 | 必须有且只有一个有效 Manager |
| Project AI Team → Professional Roles | 1:N | 可以启用多个专业 Role |
| Role → Role Versions | 1:N | 每次正式变更产生新版本 |
| User → AI Clients | 1:N | 用户可注册多个 Client |
| AI Client → Project Bindings | 1:N | 一个 Client 可授权多个项目 |
| Work Session → AI Tasks | M:N | 通过关联表实现 |
| AI Task → Assignments | 1:N | 支持重新分配 |
| AI Task → Executions | 1:N | 支持多次执行尝试 |
| AI Task → Dependencies | M:N | 支持任务依赖 |

### 17.1 必须保持的数据不变量

1. 同一个 Project 不能存在两个有效的 Project AI Team。
2. 每个 Project AI Team 必须有且只有一个有效 AI Project Manager。
3. 一个 Task 创建后必须保留创建时的 Role Version 引用。
4. 一个 Task 的每次执行尝试都必须有独立 Execution 记录。
5. 被撤销授权的 Client 不允许接受新的 Task Assignment。
6. 已经完成的 Task 不应被普通重试操作重新打开；需要重新工作时应创建新 Task 或明确的后续 Task。
7. Task 的历史 Assignment、Execution 和事件不能因为 Client 解绑而丢失。
8. Project A 的 Task 不得访问 Project B 的私有资源。
9. 所有影响执行权限、审批状态和知识权威内容的操作必须有审计记录。

### 17.2 Role Version 快照

建议 Task 不只保存 role_id，还必须保存 role_version_id。

例如：

```text
Requirement Analyst v1.0
       |
       +-- Task 001
       +-- Task 002

Requirement Analyst v1.1
       |
       +-- Task 003
```

这样，即使管理员修改 Role Instructions，历史 Task 仍能追溯当时的职责、输入输出契约和权限约束。

另外，模型策略也应记录执行时实际使用的策略版本或必要快照。否则仅凭 Role Version 无法完整解释一次历史执行。

---

## 18. Task Dispatch 进一步细化

### 18.1 将任务规划与资源分配分开

建议明确两个不同的职责。

**Task Planning —— 由 AI Project Manager 负责：**

- 理解用户目标。
- 判断是否需要专业 Role。
- 将复杂工作拆分为 Task。
- 定义 Task 输入、输出和验收条件。
- 建立 Task 依赖关系。
- 标记人工确认节点。

**Task Dispatch —— 由 Platform Dispatch Engine 负责：**

- 识别可执行资源。
- 检查 Client 授权。
- 检查 Capability。
- 检查权限。
- 检查可用性。
- 选择执行资源。
- 创建 Assignment 和 Execution。

这样可以避免 AI Project Manager 把资源选择逻辑和业务任务分析混在一起。

### 18.2 调度结果必须可解释

每次 Dispatch 应保存：

| 字段 | 说明 |
| --- | --- |
| dispatch_id | 调度记录 |
| task_id | 对应任务 |
| required_capabilities | 任务所需能力 |
| candidate_clients | 候选资源 |
| rejected_candidates | 未选中原因 |
| selected_client_id | 最终资源 |
| selection_reason | 选择原因 |
| policy_version | 使用的调度策略版本 |
| dispatched_at | 调度时间 |

这不是为了让普通用户看到复杂技术信息，而是为了管理员排查：

- 为什么没有分配任务？
- 为什么选择了这个 Client？
- 为什么另一个 Client 没有被选择？
- 为什么任务一直等待？
- 为什么执行权限检查失败？

### 18.3 失败重试规则

建议区分三类失败。

| 失败类型 | 处理策略 |
| --- | --- |
| 临时性失败 | 按策略重试，或重新调度 |
| 配置性失败 | 阻塞，要求修复配置或授权 |
| 业务性失败 | 返回 Review 或 NEEDS_HUMAN |

例如：

- Client 网络中断：可能允许重新调度。
- Team Provider 凭证无效：不能不断重试，应提示管理员。
- 需求信息不足：不属于 Client 故障，应等待用户澄清。
- Agent 输出不符合契约：可以有限次重新执行或进入人工审核。

重试必须有上限、重试原因和执行记录。不能形成无限循环。

---

## 19. 统一事件与审计

建议建立统一的 AI Team Event 机制。

初始事件类型：

| Event | 触发场景 |
| --- | --- |
| TEAM_CREATED | Project AI Team 创建 |
| ROLE_ENABLED | 启用 Role |
| ROLE_DISABLED | 停用 Role |
| ROLE_VERSION_UPDATED | Role 版本更新 |
| CLIENT_AUTHORIZED | Client 获得项目授权 |
| CLIENT_REVOKED | Client 授权撤销 |
| TASK_CREATED | 创建 Task |
| TASK_ASSIGNED | 分配 Task |
| TASK_STARTED | 开始执行 |
| TASK_EXECUTION_FAILED | 执行失败 |
| TASK_BLOCKED | 任务阻塞 |
| TASK_NEEDS_HUMAN | 需要人工介入 |
| TASK_COMPLETED | 任务完成 |
| WORK_SESSION_PAUSED | 工作暂停 |
| WORK_SESSION_RESUMED | 工作恢复 |
| APPROVAL_REQUESTED | 请求审批 |
| APPROVAL_RESOLVED | 审批完成 |

事件建议包含：

```text
event_id
project_id
team_id
aggregate_type
aggregate_id
event_type
actor_type
actor_id
payload
occurred_at
correlation_id
causation_id
```

其中：

- correlation_id 用于追踪一整条工作链路。
- causation_id 用于追踪某个事件由哪个事件触发。

特别是 AI Project Manager 创建 Task、Task 调度、Client 执行、知识协调和人工审批之间，需要能够串联追踪。

---

## 20. 人工审批与权限红线

这一部分建议作为 AI Team 的强制治理规则，而不是普通 Role Instructions。

### 20.1 必须保留人工确认的事项

| 事项 | AI 可以做什么 | 最终决定权 |
| --- | --- | --- |
| 正式需求确认 | 分析、提出草稿 | 人类 |
| Product Cognition 正式变更 | 提议、整理 | 人类按领域规则确认 |
| Project Cognition 正式变更 | 提议、整理 | 人类按领域规则确认 |
| Design 正式批准 | 设计、评审 | 人类 |
| ADR 正式决策 | 分析、提出选项 | 人类 |
| 重大权限变更 | 提出建议 | 有权限的人类 |
| 高风险工程操作 | 准备执行计划 | 经授权的人类 |
| 普通任务分配 | 自动调度 | Platform 策略 |

### 20.2 一个关键设计：审批不属于 Role

不要给 AI Project Manager 配置 APPROVE=true，再依赖 Prompt 要求它不要审批。

应该在 Platform 权限系统中明确：

- AI Actor 不具备人类正式审批身份。
- Role 权限不能创建人类审批结果。
- Client 执行成功不能自动替代人工确认。
- 审批必须记录真实的人类 Actor、审批对象和审批版本。
- 审批后仍需通过 Domain 的状态转换规则。

这意味着，即使某个 AI Role 或 Client 因配置错误试图越权，Platform 仍然应该拒绝。

---

## 21. 关键业务场景验收标准

为了让设计稿可以进入工程实施，建议增加端到端验收场景。

| ID | 场景 | 验收标准 |
| --- | --- | --- |
| AT-AC-01 | 创建 Project | 自动初始化 Project AI Team 和 AI Project Manager |
| AT-AC-02 | 无外部 Client | 用户仍能向 AI Project Manager 提出项目问题并获得 Native AI 响应 |
| AT-AC-03 | 新建需求工作 | AI Project Manager 创建 Work Session，并组织 Requirement Analyst |
| AT-AC-04 | 多 Role 分析 | 多个专业 Task 可以并行执行并汇总结果 |
| AT-AC-05 | Client 授权 | 未授权 Client 不得被调度到项目 |
| AT-AC-06 | Client 离线 | 新任务不分配给离线 Client，已有执行按恢复策略处理 |
| AT-AC-07 | Task 失败 | 保留失败 Execution，可根据规则重试或人工介入 |
| AT-AC-08 | Role 更新 | 新 Task 使用新 Role Version，旧 Task 保留旧版本 |
| AT-AC-09 | 用户切换工作 | 之前的 Work Session 可继续访问和恢复 |
| AT-AC-10 | 正式需求确认 | AI 不能自行完成正式需求审批 |
| AT-AC-11 | 知识写入 | 所有正式知识更新经过权威 Domain 的规则 |
| AT-AC-12 | Client 撤销授权 | 不再接收新任务，历史记录和审计保留 |
| AT-AC-13 | 项目隔离 | 不同 Project 之间不得发生越权读取和写入 |
| AT-AC-14 | Provider 故障 | 明确失败和提示，不自动静默切换 |
| AT-AC-15 | 执行追踪 | 可以从 Work Session 追踪到 Task、Assignment 和 Execution |

这些验收标准应成为后续 API、数据库和测试设计的依据，而不只是 UI 展示要求。

---

## 22. 本轮设计结论与待裁决事项

### 22.1 本轮新增建议基线

本轮深化了以下内容：

1. 六个 AI Team 逻辑子域。
2. 核心 Entity 和关系模型。
3. Role Version 与 Task 快照原则。
4. Task Planning 与 Task Dispatch 分离。
5. Dispatch 决策可解释机制。
6. 失败分类与有限重试。
7. 统一事件与审计模型。
8. 人工审批的 Platform 强制边界。
9. 15 项端到端验收标准。

以上均为建议设计基线，尚未经过用户逐项正式确认。

### 22.2 仍然没有裁决的问题

| 问题 | 方案 A | 方案 B | 当前状态 |
| --- | --- | --- | --- |
| Q2 专业 Role 执行方式 | 职责独立、执行资源共享 | 专业工作必须独立 Agent 执行 | 待裁决 |
| Q3 自定义 Role | 仅使用平台预置 Role | 支持项目自定义 Role | 待裁决 |
| Q4 管理界面 | 完整 AI Team 管理页面 | 基础配置页面 | 待裁决 |

### 22.3 下一阶段建议

下一步应该进入 AI Team V1.0 实施架构闭合，而不是马上开始 Figma。

建议重点完成：

- Domain 实体与现有系统实体的复用映射。
- AI Project Manager 与专业 Role 的执行边界。
- Dispatch Engine 的接口与调度算法。
- Task、Assignment、Execution 的状态机及并发控制。
- Client 授权撤销和正在执行任务的处理规则。
- Work Session 恢复协议。
- AI Team 与 Team LLM Provider 的接口边界。

这些内容完成后，再形成正式的产品需求与架构冻结稿。

**当前结论**：Project AI Team 的核心架构已经有比较完整的设计基础，但 Q2、Q3、Q4 仍然保持待裁决状态。本稿不能视为已经冻结的 V1.0 正式基线。
