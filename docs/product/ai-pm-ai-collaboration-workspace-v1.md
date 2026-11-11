# AI Collaboration Workspace — Project / Chat / Context Integration V1.0

| 项目 | 内容 |
|---|---|
| 文档性质 | **UI 设计基线**（交互骨架定义，非实施设计稿） |
| 日期 | 2026-11-11 |
| 状态 | 待评审（本稿为设计基线草案，尚未进入实施拆分） |
| 范围 | 三栏工作区布局 · AI Chat Assistance 交互 · Right Context Panel 组织 · 六类核心场景 · 交互对象模型 · Figma 页面规划 |
| 不覆盖 | 后端接口契约 · 数据库迁移 · 注入分节预算 · 权限模型的实现选型（只定交互语义） |
| 上位依据 | AI Interaction Component Library V1.0（8 类响应卡 + 11 态生命周期）· `docs/ai-pm-hybrid-workspace-design.md`（壳结构）· AI-PM 权威边界（Markdown + Git 管正式内容 / PostgreSQL 管状态、版本引用、审批与审计） |
| 关联文档 | `docs/product/ai-pm-assistant-context-integration-plan-v1.md`（既有对话 × 上下文整合计划，Batch 1~4 已落地）· `docs/product/ai-pm-dialog-orchestration-design-v1.md`（DK-0 对话编排底座） |

---

## 〇、核心原则

### 0.1 三者定义

| 对象 | 定义 | 回答的问题 |
|---|---|---|
| **Project** | 用户正在处理什么业务、需求、任务或项目对象 | 「我在做什么业务」 |
| **AI Chat Assistance** | 用户通过自然语言向 AI 提出问题、发出指令、讨论方案、要求分析和执行 | 「我想让 AI 做什么」 |
| **Right Context Panel**（右侧上下文面板） | 展示 AI 当前理解了什么、正在参考什么、产生了什么结果，以及用户可以对这些结果进行什么操作 | 「AI 看到了什么、做了什么，我能对它做什么」 |

### 0.2 三者闭环

三者不是三个并列模块，而是形成一个闭环：

```mermaid
flowchart TB
    P["Project / 当前业务页面<br/>Requirements · Tasks · Designs · Knowledge · Decisions"]
    C["AI Chat Assistance<br/>理解、讨论、分析、建议、指令"]
    R["Right Context Panel<br/>上下文、关联对象、AI 结果、操作确认"]
    F["Project 正式业务数据<br/>人工确认 → 正式提交 → 版本记录 → 审计"]

    P <--> C
    C <--> R
    C <--> F
    P <--> F

    style P fill:#E6F1FB,stroke:#185FA5,color:#0C447C
    style C fill:#E1F5EE,stroke:#0F6E56,color:#085041
    style R fill:#FAEEDA,stroke:#854F0B,color:#633806
    style F fill:#F1EFE8,stroke:#5F5E5A,color:#2C2C2A
```

> 原图见 [`docs/figures/ai-collab-v1-three-zone-loop.png`](../figures/ai-collab-v1-three-zone-loop.png)

### 0.3 AI 建议与正式业务数据分离（硬边界）

**AI 对话产生的内容，不应该自动变成 Project 的正式数据。**

AI 可以提出建议、生成草稿、修改方案、分析风险，但正式 Requirement、Knowledge、Design、Decision 的变更，**必须按照各自领域的确认权限和审批规则提交**。

这符合 AI-PM 已确定的权威边界：

| 内容类型 | 权威载体 | 写入方式 |
|---|---|---|
| 正式内容正文 | Markdown + Git | 领域单点服务 → Git 提交 |
| 状态、版本引用、审批、审计 | PostgreSQL | 同事务写入 |

「AI 说过了」≠「已确认」；「用户点了 Accept」≠「已生效」。这条边界贯穿全文的所有卡片动作与权限判据。

---

## 一、三栏主 UI 的职责

沿用此前已确认的 AI Interaction Center 结构：**三栏 + 底部输入框**。

![三栏整体布局](../figures/ai-collab-v1-three-column-layout.jpg)

| 区域 | 职责 | 用户主要操作 |
|---|---|---|
| 左侧导航 | 产品及 Project 导航 | 切换项目、进入业务模块 |
| 中间主区域 | 当前业务工作区 | 查看、编辑 Requirement、Task、Design 等 |
| **AI Chat Assistance** | 人机交流与协作 | 提问、指令、讨论、追问 |
| **右侧 Context Panel** | AI 工作上下文与结果 | 检查来源、选择对象、确认结果 |

> 原图见 [`docs/figures/ai-collab-v1-three-column-layout.jpg`](../figures/ai-collab-v1-three-column-layout.jpg)

### 1.1 AI Chat 是持续能力，不是独立产品

**AI Chat Assistance 是持续存在的交互能力，不应该成为一个独立于 Project 的聊天产品。**

它有两种入口：

| 入口 | 触发位置 | 上下文状态 |
|---|---|---|
| **Global AI** | 从 Personal Home 或全局入口发起 | 尚未绑定具体 Project |
| **Project AI** | 进入 Project 后自动获得该项目上下文 | 自动锚定当前 Project |

### 1.2 Project 切换不得静默替换上下文

当用户从一个 Project 切换到另一个 Project 时，**不能悄悄把原来的对话上下文替换掉**。应明确提示三选一：

- 新建会话
- 切换上下文
- 保留原会话

### 1.3 已确认的交互细节（继续保留）

- 发送消息后，对话区域**渐进展开**
- **Composer 固定在底部**
- Context Panel **不随对话滚动而消失**
- Context Panel **可以固定或隐藏**
- **切换业务页面不能无提示地替换上下文**
- **对话草稿应该保留**

---

## 二、具体操作场景

至少需要设计以下六个核心场景。

### 场景 1：用户进入 Project，直接向 AI 提问

**基础场景 · P0**

这是最重要的场景——用户**不需要先配置 Git、MCP、Runner 或外部 Agent**，就应该能够使用 AI-PM。

**操作过程**

1. 用户进入 Project Command Center。
2. AI Chat Assistance 自动获得 Project ID。
3. 右侧 Context Panel 显示当前 Project。
4. 用户询问：「目前这个项目有哪些重要风险？」
5. AI 查询项目的 Requirements、Tasks、Knowledge、Decisions。
6. AI 在对话区返回分析。
7. 右侧面板显示引用的业务对象和风险结果。

**右侧面板可呈现**

![Context Panel：来源与风险结果](../figures/ai-collab-v1-context-panel-sources.png)

> 原图见 [`docs/figures/ai-collab-v1-context-panel-sources.png`](../figures/ai-collab-v1-context-panel-sources.png)

**关键设计**：AI 需要告诉用户——

- 我正在处理哪个 Project
- 我参考了哪些对象
- 哪些内容是正式知识
- 哪些是 AI 推断
- 哪些问题还需要人工确认

**不能只给出一段没有来源的 AI 答案。**

---

### 场景 2：用户在 Requirement 页面要求 AI 分析需求

**核心业务场景 · P0**

这是 Project 与 AI Chat 联动最有价值的地方。

**前置**：用户正在查看 `REQ-001 · Project Initialization`。

**用户在 AI Chat 输入**：「帮我检查这个需求是否完整，特别是验收条件和异常流程。」

**交互过程**

1. Project 当前页面是 Requirement Detail。
2. AI 自动识别当前打开的 Requirement。
3. Context Panel 自动加入该 Requirement。
4. AI 分析 Requirement 内容。
5. AI 返回结构化分析卡片。
6. 用户选择需要进一步处理的结果。

![结构化分析卡片](../figures/ai-collab-v1-card-analysis.png)

> 原图见 [`docs/figures/ai-collab-v1-card-analysis.png`](../figures/ai-collab-v1-card-analysis.png)

**必须区分的两个动作**

| 动作 | 语义 | 落点 |
|---|---|---|
| **接受 AI 建议** | 将选定内容纳入编辑草稿 | 编辑草稿（未提交） |
| **正式提交 Requirement Revision** | 经过用户检查和确认后，才写入正式知识 | 领域单点服务 → Git 提交 |

**不能把点击 Accept 直接等同于正式发布。**

---

### 场景 3：用户在 AI 对话中要求创建新的业务对象

**核心业务场景 · P0**

**用户输入**：「帮我创建一个 Requirement，描述支持多个 Repository 的项目初始化能力。」

这是一个非常重要的交互。传统系统会要求用户离开聊天界面，进入 Requirement 页面，再填写表单。

PM-AI 允许用户**直接在对话中完成业务对象的草拟**。

**交互过程**

![对话中生成 Requirement 草稿](../figures/ai-collab-v1-card-draft.png)

> 原图见 [`docs/figures/ai-collab-v1-card-draft.png`](../figures/ai-collab-v1-card-draft.png)

**用户接受草稿后**

- 可以在右侧预览
- 可以跳转到 Requirement 编辑页面
- 可以继续要求 AI 修改
- 最后由有权限的人正式确认

**关键点**：AI 不应该因为用户说了 Create，就直接绕过正式业务流程。

---

### 场景 4：用户切换 Project 页面，但继续当前 AI 对话

**核心场景 · P0**

**示例**：用户先在 Requirement 页面询问需求问题，随后进入 Design 页面，问「这个 Design 是否覆盖刚才讨论的 Requirement？」

**应发生什么**

1. AI Chat Conversation 保持不变。
2. Project 仍然是原来的 Project。
3. 右侧 Context Panel **保留之前的 Requirement**。
4. 新打开的 Design **自动进入 Available Context**。
5. 用户可以选择 **Attach**。
6. AI 同时使用 Requirement 和 Design 进行分析。

**三种上下文状态（重要设计决定）**

| 状态 | 含义 | AI 是否使用 |
|---|---|---|
| **Available** | 当前页面可用对象 | 默认不使用 |
| **Attached** | 用户明确关联对象 | 可以使用 |
| **Read-only** | 只读参考对象 | 可以读取，不可修改 |

> **当前页面发生变化，不代表 AI 的上下文必须立即发生变化。用户应当拥有上下文的控制权。**

---

### 场景 5：AI 发现跨模块问题，主动关联其他业务对象

**智能协作场景 · P1**

**示例**：用户正在分析一个 Requirement，AI 发现——

- Requirement 与 Design 不一致
- Design 中有一个未解决的技术决策
- 某个 Task 缺少验收条件

这时 AI **不应该只在对话中告诉用户三个问题**，而应该将发现转化为可操作的关联对象。

**右侧 Context Panel 可显示（Related Findings）**

![跨模块关联发现](../figures/ai-collab-v1-related-findings.png)

> 原图见 [`docs/figures/ai-collab-v1-related-findings.png`](../figures/ai-collab-v1-related-findings.png)

**对标参考**：侧栏作为助手伴随界面（sidecar）的产品形态参考。

![对标：Aviary 侧栏伴随形态](../figures/ai-collab-v1-ref-aviary-sidecar.png)

> 原图见 [`docs/figures/ai-collab-v1-ref-aviary-sidecar.png`](../figures/ai-collab-v1-ref-aviary-sidecar.png)

**用户可以选择**

- 查看关联对象
- 要求 AI 解释问题
- 生成修改建议
- 创建待处理事项
- 转交给有权限的人员

**注意**：AI 发现问题，**不等于**自动修改正式对象。

---

### 场景 6：用户授权 AI 执行任务

**Agent 执行场景 · P1**

**用户输入**：「根据已经确认的 Design，帮我拆解实施任务。」

此时 AI **不应该直接开始执行一连串不可见的操作**，而应该：

1. 确认当前 Project
2. 识别已批准的 Design
3. 检查关联 Requirements
4. 生成任务拆解方案
5. 展示执行计划
6. **等待用户确认**
7. 执行允许的操作
8. 返回结果和执行记录

**右侧面板从 Context 模式转为 Execution 模式**，展示：

- 执行目标
- 输入对象
- 执行步骤
- 所需权限
- 执行状态
- 待人工处理事项
- 最终交付物

**这时 AI Chat 是交流界面，右侧 Context Panel 是执行控制和结果检查界面。**

这符合当前 Agent UI 的常见设计方向：**对话负责表达意图，侧栏负责计划、证据、审批和执行状态。**

---

## 三、右侧 Context Panel 的组织方式

右侧面板**不应是一个简单的文件列表**，而应该是一个**动态的 AI 工作控制台**。

### 3.1 五个 Tab

![AI Context 五 Tab 结构](../figures/ai-collab-v1-context-tabs.png)

> 原图见 [`docs/figures/ai-collab-v1-context-tabs.png`](../figures/ai-collab-v1-context-tabs.png)

| # | Tab | 内容 |
|---|---|---|
| 1 | **Context** | 当前 Project、当前页面、选中对象、Attached Objects、Available Objects |
| 2 | **Sources** | AI 引用的 Requirements、Knowledge、Design、Decisions，以及来源版本 |
| 3 | **Results** | AI Analysis、Proposal、Risk、Question、Decision、Execution、Progress |
| 4 | **Actions** | Accept、Reject、Edit、Apply、Approve、Retry、Cancel 等业务操作 |
| 5 | **History** | 上下文变更、AI 结果、用户反馈、审批和操作记录 |

### 3.2 三类上下文状态（沿用已确定口径）

| 状态 | 语义 |
|---|---|
| **Available** | 可选择，但尚未加入当前对话 |
| **Attached** | 已经加入当前对话的工作上下文 |
| **Read-only** | 可以作为参考，但不可执行修改 |

其中 **Attached 对象还应显示其版本或 Revision**，避免 AI 在不同版本间混淆。

---

## 四、AI Chat 与右侧面板如何互相驱动

这是整个设计中最值得仔细处理的部分。

### 4.1 交互规则表

| 用户操作 | AI Chat 行为 | Context Panel 行为 |
|---|---|---|
| 打开 Project | 建立项目上下文 | 显示 Project |
| 打开 Requirement | 可以继续当前对话 | 将 Requirement 放入 Available |
| Attach Requirement | AI 获得明确参考对象 | 显示 Attached |
| 移除 Requirement | 后续分析不再使用它 | 从 Attached 移除 |
| AI 生成分析 | 显示结构化结果卡 | Results 更新 |
| 点击某个 AI 结果 | 可以继续讨论 | 显示结果详情 |
| 接受 AI 建议 | 进入草稿或业务提交流程 | 显示待提交状态 |
| 正式提交 | 返回提交结果 | 更新对象状态和版本 |
| 执行失败 | 说明失败原因 | 显示异常与 Retry |
| 切换 Project | **不自动串用旧项目上下文** | 要求明确切换或新建会话 |

### 4.2 关键附加规则

> **Context Panel 的内容不是全部对话历史，而是当前 AI 任务实际使用的上下文及其状态。**

例如：用户在对话中提到过 Requirement A，但后来移除了它，那么它可以保留在历史记录中，**却不能继续被当作当前 Attached Context**。

---

## 五、正常流程、异常处理、权限控制

本节与 AI Interaction Component Library 的三个维度直接对应。

### 5.1 正常业务流程

| 操作 | 交互方式 |
|---|---|
| AI 分析 | Analysis Card |
| AI 提出建议 | Proposal Card |
| AI 识别风险 | Risk Card |
| AI 需要补充信息 | Question Card |
| AI 生成决策草稿 | Decision Card |
| AI 执行任务 | Execution Card |
| AI 返回进度 | Progress Card |

**每一种 Card 都应该具有自己的操作类型，而不是所有卡片都只有 Accept / Reject 两个按钮。**

| Card | 动作集 |
|---|---|
| Analysis | 查看依据、继续追问 |
| Proposal | 接受、拒绝、修改 |
| Risk | 确认、忽略、转为待办 |
| Question | 选择答案、输入补充信息 |
| Decision | 编辑、提交人工确认 |
| Execution | 授权、暂停、取消 |
| Progress | 查看执行记录、处理阻塞 |

### 5.2 异常处理

统一设计以下状态：

| 状态 | 描述 | 处理方式 |
|---|---|---|
| **Context Missing** | 缺少必要业务对象或上下文 | 选择对象 / 补充信息 |
| **Context Conflict** | 不同对象或版本存在冲突 | 查看差异 / 选择依据 |
| **AI Unavailable** | 模型或 Provider 不可用 | 重试 / 联系管理员 |
| **Execution Failed** | 执行步骤失败 | 查看错误 / 重试 / 取消 |
| **Permission Denied** | 当前用户无权执行操作 | 申请授权 / 转交有权限人员 |
| **Approval Pending** | 操作等待人工确认 | 查看内容 / 确认 / 拒绝 |

**异常不能只显示在 Chat 对话中。**

例如 AI 执行任务失败，右侧面板应该保留**失败步骤、错误原因、已完成步骤和可以恢复的操作**。

### 5.3 权限控制（三层）

| 层次 | 判断内容 | 示例 |
|---|---|---|
| **Project Permission** | 用户能否访问对象 | 能否读取 Project |
| **Business Permission** | 用户能否修改业务对象 | 能否确认 Requirement |
| **AI Execution Permission** | AI 能否执行指定动作 | 能否调用 Runner |

**需要特别区分**

- AI 有读取权限，**不代表**有写入权限
- AI 能生成草稿，**不代表**能正式确认
- 用户有权限，**也不代表** AI 可以绕过授权流程
- **只有有权限的用户**才能完成相应的正式确认

> 已确定的约束：**AI 不得 AutoApply 正式 Requirement，不得自动审批。**

---

## 六、完整的端到端操作案例

**任务**：检查 Requirement、关联 Design，并创建实施 Task。

推荐作为 Figma 主演示流程。

```mermaid
flowchart TB
    S1["1. 用户进入 Project<br/>Command Center 自动绑定当前项目<br/>右侧显示 Project Context<br/>🟦 Project Bound"]
    S2["2. 用户打开 Requirement<br/>AI 识别当前页面<br/>Requirement 出现在 Available Context<br/>🟦 Context Available"]
    S3["3. 用户要求 AI 分析<br/>「检查这个 Requirement 是否与现有 Design 一致」<br/>AI 分析后发现缺少一个验收条件<br/>🟦 Analysis Complete"]
    S4["4. 用户选择 Design<br/>Design 加入 Attached Context<br/>AI 进行交叉分析<br/>🟦 Context Attached"]
    S5["5. AI 返回 Proposal<br/>生成 Requirement 修改草稿<br/>用户修改并接受建议<br/>🟨 Draft Created"]
    S6["6. 人工正式确认<br/>有权限的用户确认 Revision<br/>系统保存正式版本<br/>🟩 Revision Approved"]
    S7["7. AI 拆解实施 Task<br/>根据正式 Requirement 和已批准 Design<br/>生成任务拆解计划<br/>🟦 Plan Ready"]
    S8["8. 人工授权执行<br/>用户确认计划<br/>AI 创建允许创建的 Task 并返回结果<br/>🟩 Tasks Created"]

    S1 --> S2 --> S3 --> S4 --> S5 --> S6 --> S7 --> S8

    style S1 fill:#E6F1FB,stroke:#185FA5,color:#0C447C
    style S2 fill:#E6F1FB,stroke:#185FA5,color:#0C447C
    style S3 fill:#E6F1FB,stroke:#185FA5,color:#0C447C
    style S4 fill:#E6F1FB,stroke:#185FA5,color:#0C447C
    style S5 fill:#FAEEDA,stroke:#854F0B,color:#633806
    style S6 fill:#EAF3DE,stroke:#3B6D11,color:#27500A
    style S7 fill:#E6F1FB,stroke:#185FA5,color:#0C447C
    style S8 fill:#EAF3DE,stroke:#3B6D11,color:#27500A
```

> 原图见 [`docs/figures/ai-collab-v1-end-to-end-flow.png`](../figures/ai-collab-v1-end-to-end-flow.png)

这个案例可以成为整个 AI Chat Assistance 的核心 Figma Prototype。

---

## 七、三个不同的交互对象

> **架构问题，必须在 UI 设计阶段明确：不要将 Chat、Context、Execution 混成同一个对象。**

| 对象 | 作用 | 生命周期 |
|---|---|---|
| **Conversation** | 保存用户与 AI 的交流 | 可以长期持续 |
| **Context Set** | 定义当前对话或任务使用的业务上下文 | 随任务变化 |
| **AI Action / Run** | 代表一次具体分析、生成或执行工作 | 独立运行及审计 |

### 7.1 关系约束

- 一个 **Conversation** 可以有多次 **AI Action**
- 一次 **AI Action** 可以使用多个 **Attached Objects**
- 多个 **AI Action** 可以共享一个 **Conversation**，但每次执行都应有自己的**输入上下文快照**

这能够避免：用户修改 Context 后，历史 AI 结果看起来像是使用了新上下文。

### 7.2 Context Set 的最小记录

Context Set 应当记录**对象 ID、版本、来源和权限范围**，而不只是保存一组对象名称。

---

## 八、正式 UI 设计基线

**命名**：AI Collaboration Workspace — Project / Chat / Context Integration V1.0

### 8.1 核心设计原则

| # | 原则 |
|---|---|
| 1 | Project 提供业务环境 |
| 2 | AI Chat 提供自然语言交互 |
| 3 | Context Panel 提供上下文管理和结果操作 |
| 4 | AI 输出必须结构化 |
| 5 | AI 建议与正式业务数据分离 |
| 6 | 用户控制上下文附加和移除 |
| 7 | 重要操作必须具备权限检查 |
| 8 | 执行结果必须能够追踪和审计 |
| 9 | Conversation、Context Set、AI Action 独立管理 |
| 10 | 页面切换不能静默改变对话语义 |

### 8.2 Figma 页面规划

不要只设计一个 Chat 页面，而是设计以下七个页面或交互状态：

| 编号 | Figma 页面 | 重点 |
|---|---|---|
| **AI-01** | Project AI Workspace | 三栏整体布局 |
| **AI-02** | Context Management | Available / Attached / Read-only |
| **AI-03** | AI Response Cards | Analysis / Proposal / Risk 等 |
| **AI-04** | Draft & Review | 接受、拒绝、修改、正式提交 |
| **AI-05** | AI Execution Control | Plan、授权、进度 |
| **AI-06** | Exception & Permission | 异常、权限、审批 |
| **AI-07** | Conversation Continuity | 页面切换、Project 切换、历史恢复 |

### 8.3 优先级建议

> **先做 AI-01、AI-02、AI-03、AI-04 的交互原型。**

这四个决定整个产品的交互骨架，其余页面在这个骨架上扩展。

---

## 附录 A、参考的设计方向

| 资料 | 借鉴点 |
|---|---|
| **Agentic Experience UX Handbook** | 对话与 Sidecar 的职责划分 |
| **Agentic UX Patterns** | 计划、审批、执行进度和结果审查的交互模式 |

---

## 附录 B、与现有实现的差距（代码事实核对 · 2026-11-11）

本附录记录本设计基线与当前代码实现之间的差距，作为后续实施拆分的输入。**核对基准：master @ `a3f92f7c`。**

### B.1 已就位（可直接复用）

| 能力 | 实现位置 | 说明 |
|---|---|---|
| 上下文单一真源 | `ai-frontend/src/stores/shell.ts:147` `assistantContext` | 指令栏作用域串 / 右栏上下文卡 / 会话页三处共用同一对象 |
| 快照交接 | `stores/shell.ts:200` `queueAssistantPrompt` → `:211` `takeAssistantPrompt` | 提交时刻定格，导航与切项目不改变快照 |
| 线程级锚点落库 | `migrations/0003_conversation_context_anchor.sql` | JSONB 单列，白名单键 + 跨项目 CHECK |
| 锚点校验 | `crates/pm-application/src/ai_conversations.rs:82` `ContextAnchor::validate` | project_id 须匹配目标项目；仓库须真实存在 |
| 服务端重新投影 | `crates/pm-ai/src/chat_context.rs:100` `build_chat_context` | 按锚点查库重读，注入 S1 项目画像 + S2 仓库事实，独立 8k 预算 |
| 注入记账落库 | `crates/pm-ai/src/conversation_reply.rs:438` | `message.metadata.injection` 含 `sections_included/dropped/chars_total` |
| 响应卡信封协议 | `crates/pm-application/src/ai_interaction_envelope.rs` | 8 类白名单 + 11 态 + payload 必填校验，fail-closed |
| 交互写回闭环 | `migrations/0005_conversation_interaction.sql` | 终态幂等 / 换 action 409 / 属主外 404 同形 |

### B.2 差距清单

| # | 设计基线要求 | 当前实现 | 差距性质 |
|---|---|---|---|
| **G1** | 右栏显示「AI 当前实际使用的上下文」 | `ContextPanel.vue:97` 读**活态** `shell.assistantContext`；注入按**线程锚点**重新投影 | 断点①：切 Project 后右栏宣称 ≠ 实际注入（原 F-2 承诺脱节换了位置复发） |
| **G2** | 右栏「Sources」Tab 展示 AI 实际引用的对象与来源版本 | 注入记账 `metadata.injection` 前端**零消费**（`grep` 仅命中 i18n 标签） | 断点②：宣称无证据链，用户无法核对「这条回复 AI 实际看到了什么」 |
| **G3** | 右栏随当前对象（Requirement / Task / Design）变，含 Available / Attached / Read-only 三态 | `ContextPanel` 固定展示当前项目 + main 仓库；三态未实现；`/focus/:type/:id` 机制未实现 | 断点③：只有项目级，无对象级 |
| **G4** | 用户可 Attach / Remove 对象，控制 AI 上下文 | 锚点只能在建会话时自动定格（`AssistantView.vue:537` `contextAnchor`）；无追加/移除入口 | 场景 4「用户拥有上下文控制权」未落地 |
| **G5** | Attached 对象显示版本 / Revision | `ContextAnchor` 无版本字段；`ConversationAnchor` 白名单不含 revision | 版本混淆风险未覆盖 |
| **G6** | Related Findings（AI 主动发现跨模块问题 → 关联对象） | 无对应实体与交互 | 场景 5 未落地 |
| **G7** | Execution 模式（计划 / 授权 / 进度 / 交付物） | 现有 Execution Card 为组件库形态，真栈执行链与右栏执行模式未打通 | 场景 6 右栏侧未落地 |
| **G8** | 异常态在右栏保留失败步骤、错误原因、已完成步骤 | 异常态目前仅在 Chat 侧；`AiInteractionStatus` 11 态有 `failed`，但右栏无对应视图 | §5.2「异常不能只显示在 Chat」未落地 |
| **G9** | Context Set 记录对象 ID + 版本 + 来源 + 权限范围 | 当前锚点为 5 字段（project/repo/branch/role），无权限范围 | §7.2 记录粒度不足 |
| **G10** | Conversation / Context Set / AI Action 三对象独立 | 当前只有 `ai.conversation` + `context_anchor`；无独立 Context Set 实体，无 AI Action 快照 | §7 架构要求未落地 |
| **G11** | 全局 AI 入口（未绑定 Project 的会话） | 会话强制 `project_id`（`ai.conversation` 列 NOT NULL） | Global AI 入口需另立模型（可能涉及迁移） |
| **G12** | 切 Project 时明确提示新建 / 切换 / 保留 | 前端已有快照机制防漂移，但**无切换提示交互** | 交互层缺失 |
| **G13** | 对话草稿保留 | 未见草稿持久化 | 交互层缺失 |

### B.3 差距归类

| 类别 | 条目 | 性质 |
|---|---|---|
| **诚实性** | G1、G2 | 右栏「宣称面」与「实际注入面」不一致 —— **最高优先**，是既有承诺脱节的残留 |
| **上下文控制权** | G3、G4、G5、G9、G12 | 场景 4 的核心能力缺失 |
| **结构化结果与执行** | G6、G7、G8 | 场景 5 / 6 的右栏侧缺失 |
| **架构独立对象** | G10、G11、G13 | 需先做模型裁决，可能涉及迁移与新表 |

---

## 附录 C、Figma 原型与本文档的对应关系

| Figma 页面 | 对应章节 | 依赖的当前实现 |
|---|---|---|
| AI-01 | §一（三栏布局）、§1.1~1.3 | `WorkspaceShell.vue` + `ContextPanel.vue` + `CommandBar.vue` |
| AI-02 | §三（五个 Tab）、§4（驱动规则） | `shell.assistantContext`、锚点读侧（待补 UI） |
| AI-03 | §5.1（Card 类型与动作集） | AI Interaction Component Library V1.0（8 类卡已完工） |
| AI-04 | §0.3、场景 2、场景 3 | 信封协议 + 交互写回闭环（已落地） |
| AI-05 | 场景 6 | Execution 链（待补右栏视图） |
| AI-06 | §5.2、§5.3 | 交互状态机 11 态 + 权限三层 |
| AI-07 | §1.2、场景 4 | 快照交接机制（已落地，提示交互待补） |
