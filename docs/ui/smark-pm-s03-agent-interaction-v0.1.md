# S03 · Agent Interaction & Human-in-the-Loop
# 编码 Agent 交互与人工介入机制详细设计稿 V0.1

| 项目 | 内容 |
|---|---|
| 所属阶段 | S03 — Analysis, Design & Technical Decisions |
| 文档性质 | 架构能力详细设计稿（讨论版） |
| 版本 | V0.1 |
| 状态 | 待评审、待用户裁决 |
| 适用范围 | Codex、TRAE、DSH、Z-Code 及未来接入的外部编码 Agent |
| 核心目标 | 统一管理 Agent 执行过程中的问题、提问、人工决策和恢复执行 |

---

## 0. 设计目标与边界

### 0.1 背景

外部编码 Agent 在执行任务时，并不总能自主完成全部工作。

典型情况包括：

1. 需要进一步理解需求。
2. 发现现有代码与设计基线不一致。
3. 需要选择技术实现方案。
4. 需要获得文件、网络、命令或其他权限。
5. 发现任务范围需要调整。
6. 等待用户回答客户端交互问题。
7. 发生执行阻塞，需要人工介入。

如果 AI-PM 只负责创建任务、分配 Agent 和收集执行结果，那么上述问题仍然需要用户直接面对不同的编码工具。

这会造成三个问题：

- 用户需要频繁切换客户端。
- 项目决策分散在不同 Agent 的对话记录中。
- AI-PM 无法掌握真实的执行阻塞和人工决策过程。

因此，需要建立统一的 Agent Interaction 能力。

### 0.2 核心目标

AI-PM 成为外部 Agent 与人类之间的统一协调层。

具体实现：

- 接收 Agent 的问题和交互请求。
- 理解问题所属的任务和项目上下文。
- 检索已有需求、设计、ADR 和正式裁决。
- 判断是否可以依据已有基线自动回答。
- 对需要人类决定的问题形成正式待处理事项。
- 将人类的回答安全地返回 Agent。
- 记录完整交互过程。
- 在问题解决后协调原执行流程恢复。

### 0.3 非目标

本设计不负责：

1. 重新实现 Codex、TRAE 等工具自身的对话系统。
2. 替代各 Agent 的代码执行能力。
3. 替代 Task、Run、Delivery。
4. 建立新的通用聊天平台。
5. 允许 AI 自动批准需求变更、架构变更和权限申请。
6. 让 MCP 调用直接成为人类正式确认。
7. 让 AI-PM 绕过 Executor Runtime 直接操纵执行进程。

---

## 1. 总体架构

### 1.1 架构关系

```text
                  Human
                    |
                    v
          Personal Workspace
                    |
               Inbox / Detail
                    |
                    v
          Human Interaction API
                    |
                    v
+---------------------------------------+
|                 AI-PM                 |
|                                       |
|       Agent Interaction Manager       |
|                    |                  |
|          Context Resolver             |
|                    |                  |
|          Decision Coordinator         |
|                    |                  |
|          Response Coordinator         |
|                    |                  |
+---------------------------------------+
                    |
             Application Service
                    |
           Agent Integration Layer
                    |
        +-----------+-----------+
        |           |           |
      Codex        TRAE       DSH
        |           |           |
        +-----------+-----------+
                    |
                  Z-Code
```

说明：

- Personal Workspace 负责提供人类处理入口。
- AI-PM 负责问题理解、判断和协调。
- Agent Integration 负责不同工具的协议适配。
- Executor Runtime 继续负责实际执行环境。
- Application Service 负责统一业务操作和权限校验。

### 1.2 设计原则

**原则 A：Agent 不直接决定项目事项。**

Agent 可以提出问题、给出建议和执行操作，但不能自行改变已经批准的项目基线。

**原则 B：AI-PM 不直接创造人类授权。**

AI-PM 可以引用既有授权，但不能将推测、默认值或历史聊天内容当作正式授权。

**原则 C：所有正式决定可追溯。**

必须能够追溯：

- 谁提出的问题。
- 哪个任务产生的问题。
- 哪个 Agent 提出的问题。
- AI-PM 如何分析。
- 谁作出了决定。
- 决定依据是什么。
- 决定返回给哪个 Agent。
- Agent 是否成功接收。

**原则 D：不改变既有执行架构。**

Interaction 是执行过程中的协调能力，不是新的执行引擎。

---

## 2. 核心领域模型

### 2.1 Agent Interaction

建议新增 AgentInteractionRequest，作为一次 Agent 向 AI-PM 发起交互请求的持久化记录。

注意：该对象不是 Task，也不是 Run，更不是 Delivery。

它用于表达：

某个 Agent 在某次任务执行过程中，向 AI-PM 提出了需要处理的问题。

建议字段：

| 字段 | 类型 | 说明 |
|---|---|---|
| id | UUID | 交互请求编号 |
| project_id | UUID | 所属项目 |
| task_id | UUID | 关联任务 |
| run_id | UUID | 关联执行记录 |
| agent_instance_id | UUID | 发起 Agent 实例 |
| interaction_type | Enum | 交互类型 |
| question | Text | Agent 原始问题 |
| context_snapshot | JSONB | 提问时的必要上下文快照 |
| status | Enum | 交互生命周期状态 |
| resolution_id | UUID? | 关联处理结果 |
| created_at | Timestamp | 创建时间 |
| resolved_at | Timestamp? | 解决时间 |

上述字段是设计提案，正式实施前必须核对现有 Agent、Run 和数据库结构，避免重复保存已有字段。

### 2.2 交互类型

建议定义：

| 类型 | 说明 | 默认处理 |
|---|---|---|
| QUESTION | 普通问题 | AI-PM 分析 |
| CLARIFICATION | 需求澄清 | AI-PM 分析，必要时人工 |
| TECHNICAL_DECISION | 技术方案决策 | 根据已有 ADR 判断 |
| SCOPE_CHANGE | 需求范围变更 | 人工确认 |
| PERMISSION_REQUEST | 权限请求 | 授权主体确认 |
| APPROVAL_REQUEST | 正式审批请求 | 既有人工确认协议 |
| EXECUTION_BLOCKER | 执行阻塞 | AI-PM 协调 |

类型应保持语义清晰，不能将所有问题简单归类为 Question。

### 2.3 交互处理结果

建议使用独立的 InteractionResolution 记录，保存处理过程。

主要字段：

- interaction_id
- resolution_type
- response_content
- evidence_refs
- decision_ref
- responder_type
- responder_id
- created_at

其中 responder_type 可以区分：

- AI-PM
- Human
- System

AI-PM 自动回答与人类正式裁决必须可以明确区分。

如果处理结果属于 ADR、正式设计批准或权限授权，应引用对应领域记录，而不是再建立一份平行的权威决策实体。

### 2.4 与既有对象的关系

| 现有对象 | 关系 |
|---|---|
| Project | Interaction 所属项目 |
| Requirement | 提问可能引用需求 |
| Technical Design | 提问可能引用设计 |
| ADR | 技术决策依据或结果 |
| Task | Interaction 的任务归属 |
| Run | Interaction 的执行来源 |
| Delivery | 问题可能关联交付内容 |
| Personal Workspace Inbox | Interaction 的人工处理投影 |
| Human Confirmation | 正式人工确认机制 |

不允许因为 Agent 提问而自动创建新的 Task 或 Run。

---

## 3. 交互生命周期

### 3.1 状态设计

建议：

| 状态 | 含义 |
|---|---|
| RECEIVED | 已收到 Agent 请求 |
| ANALYZING | AI-PM 正在分析 |
| AUTO_RESOLVED | 依据已有基线自动解决 |
| HUMAN_REQUIRED | 需要人类处理 |
| WAITING_RESPONSE | 已提交 Agent，等待接收确认 |
| RESOLVED | 交互已完成 |
| CANCELLED | 请求已取消 |
| EXPIRED | 请求已过期 |

这些是 AgentInteraction 自身的展示与处理状态，不是 Task 或 Run 的状态。

### 3.2 状态流转

```text
RECEIVED
   |
   v
ANALYZING
   |
   +------ 已有明确依据 ------> AUTO_RESOLVED
   |                                |
   |                                v
   |                         WAITING_RESPONSE
   |                                |
   |                                v
   |                            RESOLVED
   |
   +------ 需要人工 ----------> HUMAN_REQUIRED
                                    |
                                    v
                             人类正式处理
                                    |
                                    v
                             WAITING_RESPONSE
                                    |
                                    v
                                RESOLVED
```

补充规则：

- CANCELLED 不能直接视为 RESOLVED。
- EXPIRED 不代表问题已得到解决。
- 已经产生正式裁决的请求，不允许通过普通取消操作抹除裁决记录。
- Agent 接收响应失败时，必须允许可靠重试。
- 重复提交相同请求不能产生重复裁决。

### 3.3 与 Task / Run 的关系

Interaction 的状态变化不能直接决定 Task 的状态。

例如：

Codex 提出一个普通问题：

- Interaction 进入 HUMAN_REQUIRED。
- Task 是否阻塞，依据既有 Task 状态治理规则处理。
- Run 是否暂停，由 Executor Runtime 根据实际执行状态负责。
- 人类回答后，AI-PM 协调 Agent Integration 发送回复。
- Executor Runtime 根据实际执行条件恢复执行。

AI-PM 不得仅凭 Interaction 已解决，就宣称 Run 已经恢复。

---

## 4. AI-PM 自动处理机制

### 4.1 上下文解析

收到请求后，AI-PM 应首先获取：

1. 当前 Project。
2. 关联 Task。
3. 当前 Run。
4. Agent 身份和能力。
5. 当前 Requirement Revision。
6. 已批准 Technical Design Revision。
7. 相关 ADR。
8. 已确认的人工裁决。
9. 相关 Delivery 和代码核验信息。

所有上下文必须遵守权限边界。

### 4.2 自动回答条件

只有同时满足以下条件，才允许自动回答：

- 已有明确、有效的依据。
- 依据适用于当前项目和任务。
- 不存在相互冲突的正式决策。
- 回答不产生新的授权。
- 不扩大已批准的需求范围。
- 不违反安全和权限边界。

自动回答必须附带依据引用。

### 4.3 不允许自动回答的情况

以下情况必须提交人类：

1. 需求本身存在未解决歧义。
2. 需要改变已批准的需求范围。
3. 需要新增或修改重要架构决策。
4. 需要改变已经批准的设计基线。
5. 需要授予新的敏感权限。
6. 多个既有正式决策相互冲突。
7. 自动回答可能产生不可逆的业务影响。

### 4.4 AI 建议与人类裁决

AI-PM 可以给出：

- 问题分析。
- 推荐方案。
- 方案比较。
- 影响分析。
- 风险分析。
- 建议理由。

但是：

**推荐方案不等于批准方案。**

例如：

AI-PM 推荐增加独立关联表。

用户最终选择方案 B。

只有用户的正式决定才构成该事项的人工裁决。

---

## 5. Personal Workspace 集成

### 5.1 Inbox 定位

不新增 Agent Questions 菜单作为独立工作台。

在现有 Personal Workspace Inbox 中，将 Agent 产生的待处理事项作为一种来源或事项类型。

建议分类：

- Agent Question
- Requirement Clarification
- Technical Decision
- Scope Change
- Permission Request
- Approval Request

上述分类是 Inbox 的事项类型，不是新增的任务实体。

### 5.2 Inbox 列表

每条记录至少展示：

| 信息 | 说明 |
|---|---|
| 问题标题 | Agent 提问摘要 |
| 来源 Agent | Codex / TRAE / DSH / Z-Code |
| 项目 | 所属 Project |
| 任务 | 关联 Task |
| 优先级 | 按既有事项优先级规则 |
| 等待时间 | 从提交到现在 |
| AI-PM 建议 | 简短分析 |
| 当前状态 | 是否等待人工处理 |

### 5.3 问题详情

详情页应包含：

1. Agent 原始问题。
2. Agent 当前执行上下文。
3. 关联需求与设计。
4. AI-PM 的问题分析。
5. 可选解决方案。
6. 每种方案的影响。
7. AI-PM 推荐意见。
8. 人类裁决或回复区域。
9. 历史交互记录。

### 5.4 人类处理方式

根据交互类型，提供不同操作：

| 事项类型 | 人类操作 |
|---|---|
| 普通澄清 | 输入文字回答 |
| 技术决策 | 选择方案或提交新方案 |
| 范围变更 | 确认变更并进入既有需求治理流程 |
| 权限请求 | 通过授权协议确认 |
| 正式审批 | 通过现有审批机制处理 |

不能把所有事项都做成简单的 A/B/C 单选框。

---

## 6. Agent Integration 适配机制

### 6.1 统一协议

所有外部 Agent 通过 Agent Integration Layer 接入。

建议抽象接口：

```rust
#[async_trait]
pub trait AgentInteractionAdapter: Send + Sync {
    async fn capabilities(
        &self,
    ) -> Result<AgentInteractionCapabilities>;

    async fn receive_interaction(
        &self,
        event: AgentInteractionEvent,
    ) -> Result<()>;

    async fn deliver_response(
        &self,
        response: AgentInteractionResponse,
    ) -> Result<DeliveryReceipt>;

    async fn interaction_status(
        &self,
        interaction_id: Uuid,
    ) -> Result<AgentInteractionStatus>;
}
```

此接口是概念性设计，实际方法签名需根据现有 Integration 架构调整。

### 6.2 能力协商

不同编码工具的交互能力可能不一样。

建议定义能力描述：

| 能力 | 说明 |
|---|---|
| structured_question | 是否支持结构化提问 |
| interactive_input | 是否支持接收人工输入 |
| pause_execution | 是否支持暂停执行 |
| resume_execution | 是否支持恢复执行 |
| response_ack | 是否支持确认响应已接收 |
| session_reconnect | 是否支持重新连接会话 |
| permission_request | 是否支持结构化权限请求 |

不能假设所有工具都支持上述全部能力。

### 6.3 三种接入模式

**模式 A：原生结构化交互**

Agent 能通过 API 或事件机制提交问题，并接收 AI-PM 的回复。

这是优先支持的模式。

**模式 B：会话交互桥接**

Agent 在终端或客户端中等待人工输入。

AI-PM 通过适配器与该会话建立受控的交互桥接。

需要确保输入内容能够准确关联原始 Session、Run 和 Interaction。

**模式 C：外部客户端不可控**

某些工具不暴露交互事件或恢复接口。

此时 AI-PM 只能记录已知阻塞、展示处理建议，不能假装已经完成自动恢复。

必要时提示用户回到原客户端完成操作。

### 6.4 工具隔离

不得在通用业务层直接写入：

```rust
if agent == "codex" {
    // 特殊业务处理
}
```

具体工具差异应封装在 Adapter 中。

AI-PM 只处理统一的 Interaction 语义。

---

## 7. 人工正式确认与安全边界

### 7.1 人类身份

所有正式裁决必须具备：

- 人类主体身份。
- 操作权限验证。
- 具体裁决对象。
- 明确的裁决内容。
- 裁决时间。
- 适用范围。
- 必要时的理由和证据。

### 7.2 MCP 边界

MCP 可以：

- 提交 Agent 问题。
- 查询交互状态。
- 读取允许访问的上下文。
- 获取已经完成的人工回复。
- 传递 Agent 执行结果。

MCP 不可以：

- 冒充人类进行正式确认。
- 自动批准架构设计。
- 自动批准需求范围变化。
- 自动完成必须由人类执行的权限授权。

正式确认必须经过既有可信人类确认协议。

### 7.3 防止 Agent 伪造人类决定

必须验证：

1. 决策来自真实认证的人类主体。
2. 决策关联当前有效 Interaction。
3. 决策内容与提交时的选项或裁决对象一致。
4. 决策主体具备对应权限。
5. 决策没有被重复消费。

不得仅凭 Agent 发送的文本：

`User approved option B`

就认为人类已经批准方案 B。

### 7.4 决策不可覆盖

正式裁决一旦生效：

- 保留原始记录。
- 后续修改形成新的裁决或修订记录。
- 不允许 Agent 直接修改裁决结果。
- 不允许通过重新提交问题覆盖历史决定。

---

## 8. 响应回传与执行恢复

### 8.1 响应流程

```text
人类提交回答
      |
      v
身份与权限校验
      |
      v
正式记录裁决
      |
      v
生成 Interaction Response
      |
      v
Agent Integration
      |
      v
目标 Agent 接收
      |
      v
确认接收
      |
      v
Interaction RESOLVED
```

### 8.2 回传失败

如果 Agent 已经完成提问，但响应回传失败：

- 不得丢失人类的正式裁决。
- 保留待发送响应。
- 使用幂等机制重试。
- 记录每次投递结果。
- 不允许因重复发送产生重复决策。
- 超过重试策略后进入人工处理。

### 8.3 执行恢复

Interaction 解决与 Run 恢复是两个不同的动作。

必须分别记录：

- Interaction 是否已解决。
- Agent 是否收到响应。
- Run 当前实际状态。
- Executor Runtime 是否成功恢复。
- Agent 是否继续执行。

AI-PM 只能协调恢复，不能虚构恢复成功。

---

## 9. 超时、重复和并发

### 9.1 重复提问

同一个 Agent 可能反复询问同一问题。

需要支持：

- 请求幂等键。
- 原始请求 ID。
- 相似问题关联。
- 已解决问题引用。
- 防止重复创建 Inbox 事项。

语义相似不等于完全相同，不能仅凭文本相似就自动合并正式裁决。

### 9.2 超时

超过规定时间仍未获得人工回复：

- 继续保留待处理事项。
- 提醒相关负责人。
- 可以按策略升级处理。
- 不得擅自选择默认技术方案。
- 不得将等待超时解释为人类同意。

### 9.3 并发回答

两个具有权限的人类同时提交答案时：

- 使用乐观并发控制。
- 只允许一个有效裁决成功提交。
- 另一个提交应返回冲突状态。
- 冲突提交不得覆盖已经生效的裁决。

---

## 10. 数据一致性与事件处理

建议采用既有 Application Service 和 Outbox 架构。

核心事件建议：

| 事件 | 含义 |
|---|---|
| agent_interaction_received | 收到 Agent 请求 |
| agent_interaction_analyzed | 完成 AI-PM 分析 |
| agent_interaction_requires_human | 需要人类处理 |
| agent_interaction_resolved | 交互处理完成 |
| agent_interaction_response_sent | 响应已发送 |
| agent_interaction_response_acknowledged | Agent 已确认接收 |
| agent_execution_resume_requested | 请求协调恢复执行 |
| agent_execution_resumed | 执行恢复已确认 |

事件命名最终需要对齐现有领域事件规范。

重要约束：

- 人工正式裁决与其对应的业务记录必须保持一致性。
- 响应发送采用可靠投递。
- 消费者必须幂等。
- 不得将事件发送成功等同于 Agent 已经接收。
- 不得将 Agent 接收成功等同于 Run 已恢复。

---

## 11. 审计与可观测性

必须保留完整交互链路：

```text
Agent 原始请求
      |
      v
AI-PM 上下文检索
      |
      v
AI-PM 分析结果
      |
      v
自动回答 / 人工处理
      |
      v
人类正式裁决（如适用）
      |
      v
响应回传
      |
      v
Agent 接收确认
      |
      v
执行恢复结果
```

建议关注以下指标：

| 指标 | 目的 |
|---|---|
| interaction_count | 交互总量 |
| auto_resolution_rate | AI-PM 自动解决比例 |
| human_intervention_rate | 人工介入比例 |
| average_resolution_time | 平均解决时间 |
| response_delivery_failure_rate | 响应投递失败比例 |
| repeated_question_rate | 重复提问比例 |
| blocked_execution_duration | 执行阻塞时长 |

自动解决率不是唯一目标。

不能为了提高自动解决率而降低人工确认要求。

---

## 12. 异常场景

| 场景 | 处理原则 |
|---|---|
| Agent 意外退出 | 保留 Interaction，等待会话恢复 |
| 用户关闭客户端 | 不丢失待处理事项 |
| AI-PM 分析失败 | 保留原始请求，转人工或重试 |
| 人类长时间未回复 | 保持等待，不默认批准 |
| 响应发送失败 | 保留裁决，可靠重试 |
| Agent 已结束 Run | 不向失效 Run 盲目发送 |
| Agent 重复提问 | 幂等与关联处理 |
| 人类提交冲突 | 并发控制 |
| 相关 ADR 已更新 | 重新检查决策适用性 |
| 项目权限被撤销 | 阻止不再授权的操作 |

---

## 13. 实施切片建议

建议将本能力拆成以下实施切片，避免一次性引入过多复杂度。

| 切片 | 名称 | 主要交付 |
|---|---|---|
| I1 | Interaction Core | 交互请求、状态、持久化、幂等 |
| I2 | Agent Adapter Protocol | 统一 Agent 交互适配协议 |
| I3 | AI-PM Resolution | 上下文检索、自动回答、人工分流 |
| I4 | Human Interaction UX | Personal Workspace Inbox 与详情页 |
| I5 | Human Decision Integration | 正式裁决、权限和审计集成 |
| I6 | Response Delivery | Agent 回复回传、可靠投递 |
| I7 | Execution Coordination | 阻塞管理、恢复协调 |
| I8 | Observability & Recovery | 超时、异常恢复和运行指标 |

切片先后关系：

```text
I1
 |
 +---- I2
 |      |
 |      +---- I6
 |
 +---- I3
        |
        +---- I4
        |
        +---- I5
               |
               v
              I7
               |
               v
              I8
```

实际依赖关系需要根据现有代码核验调整。

---

## 14. 代码核验要求

正式进入实施前，必须核对：

1. 当前 Task、Run、Delivery 的实体和状态定义。
2. 当前 Agent 身份、Session 和执行记录的映射。
3. 当前 Agent Integration 实现。
4. 当前 MCP 接口与认证机制。
5. 当前 D3 Executor Runtime 的暂停、终止和恢复能力。
6. 当前 Personal Workspace Inbox 数据来源。
7. 当前 I6-A 人类正式确认协议。
8. 当前权限体系及授权主体。
9. 当前 Outbox 和事件消费机制。
10. 当前数据库迁移和幂等约束。

特别检查：

- 是否已经存在可以承载 Interaction 的领域对象。
- 是否已经存在通用人工待处理事项。
- 是否已经存在人类回复与 Agent 回传机制。
- 是否可以直接复用既有事件和消息设施。

如果已有能力满足要求，应优先复用，而不是重复创建。

---

## 15. 验收标准

### 15.1 基础交互

- [ ] 至少一种外部 Agent 能提交结构化问题。
- [ ] 问题能够关联正确的 Project、Task 和 Run。
- [ ] Interaction 生命周期可追踪。
- [ ] 重复请求不会产生重复处理事项。

### 15.2 AI-PM 自动处理

- [ ] 能够检索相关项目上下文。
- [ ] 能够识别已有正式决策。
- [ ] 自动回答能够提供依据。
- [ ] 不确定的问题进入人工处理。
- [ ] 不允许 AI 自动批准需要人类确认的事项。

### 15.3 人工介入

- [ ] Inbox 能展示待处理 Agent 事项。
- [ ] 人类能够查看问题原文和上下文。
- [ ] 人类能够提交正式回答。
- [ ] 正式裁决具备身份和权限校验。
- [ ] 裁决记录能够追溯。
- [ ] 已有裁决不能被 Agent 覆盖。

### 15.4 Agent 恢复

- [ ] 人类回答能够可靠回传。
- [ ] Agent 能够确认接收。
- [ ] 回传失败能够恢复。
- [ ] Interaction 解决不等于 Run 自动恢复。
- [ ] 不能恢复的客户端能够明确提示用户。

### 15.5 架构一致性

- [ ] 不重复建设 Task、Run、Delivery。
- [ ] 不重复建设 Personal Workspace。
- [ ] 不绕过 D3 Executor Runtime。
- [ ] 不绕过 I6-A 正式人类确认协议。
- [ ] REST 和 MCP 复用相同 Application Service。
- [ ] 不允许 MCP 冒充人类正式确认。

---

## 16. 待用户裁决事项

以下事项尚未冻结。

**Q1：Agent Interaction 是否作为独立持久化领域对象？**

A. 是。建立独立的 AgentInteractionRequest，负责交互生命周期与审计。

B. 不建立新领域对象，完全复用现有 Task/Run 相关记录及通用待处理事项。

C. 先核验代码，确认不存在可复用对象后再决定。

**建议：C。** 避免在没有代码事实依据时重复建模。

**Q2：AI-PM 自动回答权限边界？**

A. 仅引用已经批准的需求、设计和 ADR，不允许推断新决定。

B. 在既有决策基础上允许有限推导，但必须记录依据和置信度。

C. 允许 AI 提出新的技术方案并自动选择。

**建议：B。** 允许 AI 进行受约束的执行性判断，但不得替代正式批准。

**Q3：人工介入入口？**

A. 统一进入 Personal Workspace Inbox。

B. Inbox 为主，同时允许从 Project 和 Task 详情页直接进入。

C. 每个 Agent 单独提供问题处理页面。

**建议：B。** 统一处理入口，同时保留上下文操作便利性。

**Q4：Agent 响应回传失败如何处理？**

A. 自动重试，超过策略后转人工。

B. 立即转人工，不进行自动重试。

C. 允许自动重试，同时保留人工强制干预能力。

**建议：C。**

**Q5：首期 Agent 接入策略？**

A. 首期只实现 Codex，验证协议后扩展。

B. 同时完成 Codex、TRAE、DSH、Z-Code 四种适配器。

C. 先实现统一协议和一个参考适配器，其余按工具实际能力逐步接入。

**建议：C。**

---

## 17. 最终设计原则

本能力的核心定位：

**AI-PM 是外部 Agent 的项目级交互协调者，而不是外部 Agent 的另一个客户端。**

它需要具备三种能力：

1. **理解：** 知道 Agent 为什么提问，以及问题与项目基线的关系。
2. **协调：** 知道哪些问题可以自动解决，哪些必须交给人类。
3. **闭环：** 确保人类的正式决定能够准确返回 Agent，并能够追踪执行是否恢复。

最终形成：

Agent 执行 → AI-PM 理解 → AI-PM 协调 → 人类裁决（必要时）→ Agent 接收 → 执行继续。

这套机制将成为未来所有外部 AI Agent 接入 AI-PM 的统一人机协同基础。