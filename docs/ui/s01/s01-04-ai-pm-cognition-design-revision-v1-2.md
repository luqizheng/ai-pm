# S01 设计修订 · 第四份：S01-04 · AI-PM Cognition 详细设计稿 V1.2（专家供稿记录）

> **转交日期**：2026-11-05。本文件是对用户转交的专家设计修订稿的**忠实记录**（以转交文本为准）。
> 文档性质（原稿自带）：产品与 UX 详细设计修订稿。修订依据：S01-04 用户裁决 Q1–Q5（2026-11-04）及后续 Project 创建、Repository 管理和内置 AI-PM 独立可用原则。
> 修订目标：建立 Project **长期认知能力**，明确 AI 分析、认知提案、人工确认、知识沉淀和 Agent Context 的边界。
> **状态（原稿自带）：设计基线候选，待技术现状核验。**
> **章节号事实**：原稿验收标准表（AC-01~15）位于 §13 线框与「15. 技术核验任务」之间，**无「14」章节标题**，转录保持原样（按位置推断为 §14 内容）。
> 按既定协作约定：供稿阶段只记录不评审；评审须用户明确要求。

---

## 0. 本次修订结论

AI-PM Cognition 是 Project 的**长期认知能力**。

它让 AI-PM 能够逐步理解一个 Project 的目标、业务领域、系统结构、工程约定和演进历史，并将经过人工确认的信息沉淀为稳定的项目认知。

核心原则：

1. Cognition 与 Knowledge 是两个独立对象。
2. AI 可以自动准备认知分析材料，但正式分析由人启动。
3. 认知结果必须逐项由人确认。
4. 已确认认知不能被 AI 直接覆盖；发现变化时只能产生提案。
5. Agent 使用混合 Context：核心认知默认注入，其他信息按任务检索。
6. 没有 Repository 或外部 Agent 时，内置 AI-PM 仍然可以工作。
7. AI 的推断、建议和正式确认的项目事实必须明确区分。

---

## 1. 产品定位

### 1.1 Cognition 的定义

Cognition（项目认知）是 AI-PM 对 Project **长期理解的结构化表达**。

它不是一段系统提示词，也不是一个 Markdown 文档集合。

它应当能回答：

- 这个 Project 为什么存在？
- 它要解决什么业务问题？
- 它服务于哪些用户或业务角色？
- 核心业务概念是什么？
- 系统由哪些关键部分组成？
- 各部分之间有什么关系？
- 有哪些架构约束和工程约定？
- 项目经历过哪些重要演进？
- 哪些信息已经被人确认，哪些仍然存在不确定性？

Cognition 是 Project 层的**长期认知资产**。

### 1.2 Cognition 与 Knowledge 的区别

**两者是独立对象，不是同一个对象的两种状态。**

| 比较维度 | Cognition | Knowledge |
| --- | --- | --- |
| 核心定位 | AI-PM 对项目的结构化理解 | 项目保存和管理的知识内容 |
| 主要内容 | 目标、领域、架构、约束、约定、演进认知 | 文档、规范、决策记录、方案、分析材料等 |
| 形成方式 | AI 分析 + 人工逐项确认 | 人工创建、导入、整理或由认知确认流程沉淀 |
| 更新方式 | 产生认知提案，人工确认 | 按知识自身的编辑、审核和发布规则管理 |
| 使用方式 | 默认注入核心认知，辅助 AI-PM 理解项目 | 按任务检索、引用和阅读 |
| 权威性 | 已确认的认知是 Project 认知事实 | 取决于 Knowledge 自身的状态和治理规则 |

重要约束：

- Knowledge 文档不能因为被 AI 读取，就自动成为已确认 Cognition。
- Cognition 不能因为引用了某份 Knowledge，就取代该 Knowledge 的正文和发布治理。
- Cognition 与 Knowledge 可以互相引用，但不能混成一个无边界的数据集合。
- Cognition 的确认不等于 Knowledge 文档发布。
- Knowledge 文档发布不等于 Cognition 自动更新。

### 1.3 AI-PM 独立可用

Project 创建完成后，内置 AI-PM 即可开始工作。

即使没有：

- Repository；
- GitHub / GitLab 授权；
- MCP；
- Codex；
- Claude Code；
- 其他外部 Agent；

用户仍然可以：

- 与 AI-PM 讨论项目目标；
- 提供业务背景；
- 整理项目知识；
- 分析需求；
- 建立项目认知；
- 发现认知缺口；
- 形成认知提案；
- 进行人工确认。

信息来源可以包括用户输入、Project 内已有需求、Knowledge、决策记录及其他已授权的项目资料。

Repository 接入后，AI-PM 可以进一步获得代码和工程结构证据，但 **Repository 不是 Cognition 的启动条件**。

---

## 2. Cognition 的认知范围

建议将项目认知划分为以下领域。

### 2.1 Project Identity

项目身份与目标：项目名称和定位；项目使命；核心目标；项目范围；目标用户；成功标准；当前发展阶段。

### 2.2 Business Domain

业务领域认知：业务背景；业务角色；核心业务概念；业务对象及其关系；关键业务流程；业务规则；领域边界；业务术语。

### 2.3 Product Cognition

产品认知：产品定位；核心能力；用户场景；功能边界；用户操作流程；产品约束；关键体验原则。

### 2.4 Architecture Cognition

系统架构认知：系统整体结构；核心模块；模块职责；模块依赖；数据流；关键接口；技术架构约束。

- 没有 Repository 时，这部分可以根据用户提供的架构资料和 Knowledge 建立初步认知。
- 有 Repository 时，可以通过代码分析补充证据。
- **不得将缺少代码证据的推断伪装成代码事实。**

### 2.5 Development Convention

开发约定：编码规范；目录组织约定；测试要求；错误处理约定；数据库变更约定；分支与提交约定；代码审查规则；工程质量约束。

### 2.6 Evolution Cognition

项目演进认知：重要设计决策；关键架构变化；已确认的需求变化；重要技术调整；已废弃的设计；变化原因；历史决策与当前决策之间的关系。

演进认知必须保留**时间和来源**，不能只保留当前结论而丢弃历史。

---

## 3. Cognition 生命周期

### 3.1 总体流程

```text
Project Created
      |
      v
Cognition Workspace Available
      |
      v
Collect Available Context
      |
      v
Prepare Analysis Materials
      |
      v
Human Starts Formal Analysis
      |
      v
AI Performs Cognition Analysis
      |
      v
Generate Cognition Proposals
      |
      v
Human Reviews Each Proposal
      |
      +------ Reject
      |
      +------ Request Revision
      |
      +------ Confirm
                |
                v
       Confirmed Cognition
                |
                v
       Available to AI-PM
                |
                v
       Context Injection / Retrieval
```

### 3.2 自动准备，人工启动正式分析

**对应 Q2=C。**

系统可以自动：

- 汇集已有 Project 信息；
- 整理用户输入；
- 识别已有 Knowledge；
- 整理已确认需求；
- 汇总项目决策；
- 读取已授权的 Repository 信息；
- 标记信息来源和缺口；
- 准备认知分析材料。

但**自动准备不等于正式分析**。正式分析必须由用户主动启动。

启动入口可以是：Start Cognition Analysis；Analyze Project Context；Analyze Changes。

AI **不得**因为创建了 Project、连接了 Repository 或导入了一份文档，就自动把推断写入正式认知。

### 3.3 分析范围

正式分析可以选择：

- Initial Cognition：首次建立项目认知；
- Incremental Analysis：基于新增资料补充认知；
- Change Analysis：检查已有认知是否需要变化；
- Focused Analysis：只分析某个认知领域。

以上是 **UX 分析模式建议**，是否需要独立持久化为分析类型，应由技术模型核验。

---

## 4. 认知提案与逐项确认

### 4.1 提案机制

**对应 Q3=B。**

AI 分析后，**不直接生成一份可覆盖的完整 Cognition**。

AI 应当产生一组**可独立审核的 Cognition Proposals**。每个提案只表达一个可判断的认知结论或变更。

例如：

- 项目主要服务对象是企业内部研发团队；
- 系统采用 Rust + Bevy 作为核心技术栈；
- 项目地图使用自上而下的 2D 表现方式；
- 某模块负责处理项目初始化事项；
- 某业务规则已由用户正式裁决。

以上示例只有在**有证据支持**时才可提出。

### 4.2 提案内容

每个 Cognition Proposal 应展示：

| 字段 | 说明 |
| --- | --- |
| Title | 认知结论标题 |
| Domain | 所属认知领域 |
| Proposed Content | AI 建议的认知内容 |
| Evidence | 支撑该结论的证据 |
| Source | 来源类型及来源位置 |
| Confidence | AI 对分析结果的置信表达 |
| Change Type | 新增、修订、冲突或废弃建议 |
| Impact | 对现有认知的影响 |
| Created At | 提案时间 |

**置信度不能代替人工确认。** 没有证据的内容应明确标记为假设、推断或待澄清。

### 4.3 单项审核

用户对每条提案分别执行：Confirm；Reject；Request Revision。

**允许部分确认**。例如，AI 产生 8 条提案，用户可以确认 5 条、驳回 1 条、要求修订 2 条。

**不允许用一次 Confirm All 隐式确认全部重要认知。** 可以提供批量操作的 UI 优化，但必须保证每项都具有独立、可追溯的确认结果；默认不应鼓励无差别批量确认。

### 4.4 确认后的结果

确认后：

- 该项成为已确认 Cognition；
- 记录确认人和确认时间；
- 保留提案来源和证据；
- 记录原有版本与新版本之间的变化；
- 对相关 AI-PM Context 产生更新；
- 如需沉淀为 Knowledge，必须经过**独立的 Knowledge 创建或更新流程**。

---

## 5. 已确认认知的变化治理

**对应 Q4=C。**

### 5.1 不允许 AI 直接覆盖

已确认 Cognition 是**受治理的项目认知事实**。AI 不得：

- 直接修改已确认内容；
- 删除已确认认知；
- 因新文档与旧认知冲突而自动覆盖；
- 因 Repository 代码变化而自动改写架构认知；
- 因用户的一句普通讨论而自动把正式认知改成新结论。

### 5.2 变化提案

当 AI 发现新信息与已有认知不一致时，产生 **Change Proposal**。

变化类型：

- New Information：新增信息；
- Revision：建议修订；
- Conflict：与现有认知冲突；
- Deprecation：建议标记旧认知不再适用；
- Uncertainty：发现原认知证据不足。

变化提案至少展示：当前已确认内容；新发现的信息；证据来源；两者差异；建议变化；影响范围。

### 5.3 人工处理

用户可以：Confirm Change；Reject Change；Request More Evidence；Request Revision。

确认变化后，生成**新的认知版本**。不得直接抹掉旧版本。

### 5.4 历史追溯

每次认知变化应能回答：原来是什么；现在是什么；谁确认了变化；什么时候确认；为什么变化；依据是什么；关联哪些提案和来源。

---

## 6. Context 使用策略

**对应 Q5=C：混合 Context 模式。**

### 6.1 核心认知默认注入

以下内容属于建议默认注入的核心认知：

- Project Identity；
- 核心目标；
- 核心业务领域；
- 关键产品边界；
- 重要架构原则；
- 关键开发约定；
- 重要约束；
- 当前有效的关键决策。

默认注入的目的是保证 AI-PM 在不同任务中具有**稳定的项目理解**。不是把整个 Cognition 数据库全部塞进每一次模型请求。

### 6.2 其他认知按任务检索

以下内容通常按需检索：详细业务流程；次要模块架构；历史演进记录；低频技术约定；某个功能的详细设计；某个领域的完整术语表；与当前任务相关的历史认知提案。

### 6.3 Context 组装

建议流程：

```text
Incoming User Task
       |
       v
Identify Task Scope
       |
       v
Load Core Cognition
       |
       v
Retrieve Relevant Cognition
       |
       v
Retrieve Related Knowledge
       |
       v
Assemble Task Context
       |
       v
AI-PM Response / Analysis
```

Context 组装必须区分：

- Confirmed Cognition；
- Unconfirmed Proposal；
- Retrieved Knowledge；
- Historical Information；
- Inference。

**不允许将未确认提案伪装成已确认项目事实。**

### 6.4 Context 可解释性

在需要时，用户应能够查看：本次 AI-PM 使用了哪些核心认知；检索了哪些认知条目；引用了哪些 Knowledge；是否使用了未确认信息；是否存在认知冲突。

---

## 7. AI-PM 与外部 Agent 的边界

### 7.1 内置 AI-PM

内置 AI-PM 是产品原生能力。在没有 Repository、MCP 和外部 Agent 时，仍可以：进行项目对话；分析需求；整理项目背景；建立 Cognition；生成 Cognition Proposal；检索 Knowledge；识别认知缺口；形成分析建议。

### 7.2 外部 Agent

Codex、Claude Code 等外部 Agent 是**工程执行能力的扩展**。接入后可以在授权范围内：读取代码；分析工程结构；执行工程任务；根据任务上下文开展工作；回传工程执行结果。

外部 Agent **不应成为 Cognition 的唯一生产者**，也**不能绕过人工确认机制**。

### 7.3 统一认知治理

无论认知提案来自：内置 AI-PM；Repository 代码分析；外部 Agent；用户手工输入；Knowledge 分析——

**只要它要成为正式 Cognition，都必须进入同一套提案和人工确认机制。**

---

## 8. 页面信息架构

建议在 Project Navigation 中设置：

- Project Command Center
- Requirements
- Discovery
- **Cognition**
- Knowledge
- Repositories
- Agent / Execution
- Settings
- Audit

Cognition 与 Knowledge **分开呈现**。不要把 Cognition 页面设计成 Knowledge 文档浏览器。

### 8.1 Cognition 首页

- 页面标题：Project Cognition
- 副标题：Build and maintain a shared understanding of this project.
- 主操作：Start Analysis；Review Proposals。

页面内容：1. Cognition Overview；2. Cognition Domains；3. Pending Proposals；4. Recent Changes；5. Knowledge References；6. Context Usage Summary。

### 8.2 Cognition Overview

展示：Confirmed Cognition 数量；Pending Proposals 数量；Domains Coverage；Last Analysis；Last Confirmed Update。

这些数据只描述**认知治理情况**，不代表 Project 健康度或完成度。

### 8.3 Cognition Domains

按六个认知领域展示：Project Identity；Business Domain；Product Cognition；Architecture Cognition；Development Convention；Evolution Cognition。

每个领域展示：已确认条目；待审核提案；最近更新时间；信息缺口提示。

### 8.4 Proposal Review

提案审核页重点展示：AI 建议；现有认知（如为变更）；证据；来源；影响；确认 / 驳回 / 要求修订操作。

### 8.5 Cognition Detail

详情页应包含：当前有效内容；认知领域；确认状态；来源与证据；版本历史；关联 Knowledge；关联提案；变更记录。

---

## 9. 空状态与缺口体验

### 9.1 尚无认知

> 标题：No project cognition yet
>
> 说明：AI-PM can help build an understanding of your project from the information already available. You can start with project goals, business context, requirements, or documents.
>
> 操作：Start Cognition Analysis

### 9.2 没有 Repository

> 说明：Repository is not required to establish project cognition. You can begin with project information, requirements, and knowledge already available.

**不得把没有 Repository 显示成错误或阻塞状态。**

### 9.3 资料不足

AI 应**指出缺少什么，而不是编造内容**。例如：项目目标尚未明确；业务角色缺少定义；架构资料尚未提供；某项业务规则存在多个冲突说法。

可以给出补充信息建议，但**不能把建议当作事实**。

---

## 10. 权限与审计

以下操作必须受权限控制：

- Start Cognition Analysis；
- Confirm Proposal；
- Reject Proposal；
- Request Revision；
- Confirm Change；
- 手动编辑正式认知；
- 查看受权限保护的来源资料。

至少需要记录：Project；Cognition Item；Proposal；操作人；操作类型；操作时间；原内容；新内容；证据与来源；操作结果。

**AI 可以创建提案，但不能冒充人类确认者。** 系统自动分析、用户确认和外部 Agent 回传必须有**可区分的 Actor 身份**。

---

## 11. 状态语义建议

Cognition Item 的状态与 Proposal 的状态**必须区分**。

建议 Cognition Item：Confirmed；Superseded；Deprecated。

建议 Proposal：Pending；Confirmed；Rejected；Revision Requested；Superseded。

建议 Analysis：Prepared；Running；Completed；Failed；Cancelled。

以上为 **UX 语义建议，不能直接视为现有数据库枚举**。技术侧应先核验已有 Cognition / Proposal / Analysis 模型，避免重复造状态机。

尤其注意：

- Prepared 不代表正式分析已开始；
- Analysis Completed 不代表所有提案已确认；
- Proposal Confirmed 不代表关联 Knowledge 已发布；
- Cognition 已建立不代表 Repository 已连接；
- Cognition 覆盖率不能直接等同于 Project 完成度。

---

## 12. Figma AI · UX/UI 设计提示词

> 设计一个企业级 AI Project Management 产品中的 Project Cognition 页面。产品名称：AI-PM。
> 核心概念：Project Cognition 是 AI-PM 对项目形成的长期结构化认知；Knowledge 是独立的项目知识管理对象。

**必须严格遵循以下产品裁决**：

1. Cognition 与 Knowledge 是两个独立对象；
2. 系统可以自动准备分析材料，但正式分析必须由用户主动启动；
3. AI 分析结果以独立 Proposal 呈现，用户逐项确认；
4. 已确认认知不得被 AI 直接覆盖；
5. 发现变化时只能生成 Change Proposal，由人决定；
6. 核心认知默认注入 AI Context，其余认知按任务检索；
7. 没有 Repository、MCP 或外部 Agent 时，内置 AI-PM 仍然可用；
8. 不引入 Workspace 业务概念。

**页面一：Project Cognition Overview**——页面标题 Project Cognition；Start Analysis 主按钮；Confirmed Cognition 数量；Pending Proposals 数量；Domains Coverage；Last Analysis；六个认知领域卡片；Recent Cognition Changes；Knowledge References；Context Usage Summary。
六个领域：Project Identity；Business Domain；Product Cognition；Architecture Cognition；Development Convention；Evolution Cognition。

**页面二：Cognition Analysis Preparation**——已收集的 Project 信息；已有 Requirements；Knowledge References；已授权 Repository 信息（如有）；Missing Information；Analysis Scope；Start Formal Analysis 按钮。
注意：资料准备可以自动进行，但**不得自动启动正式分析**。

**页面三：Cognition Proposal Review**——每一项 AI 提案展示 Proposal Title / Domain / Proposed Content / Evidence / Source / Confidence / Change Type / Impact；每项独立提供 Confirm / Reject / Request Revision。**允许部分确认。不要设计成一次点击就无差别接受所有认知的主流程。**

**页面四：Confirmed Cognition Detail**——当前已确认内容；来源与证据；确认人；确认时间；版本历史；关联 Knowledge；关联 Change Proposals。

**页面五：Cognition Change Proposal**——必须展示 Current Confirmed Cognition / Newly Observed Information / Evidence / Difference / Proposed Change / Impact；提供 Confirm Change / Reject Change / Request More Evidence / Request Revision。

**视觉方向**：专业、清晰、可信的企业级 SaaS；让用户一眼分辨「已确认认知」和「AI 提案」；证据和来源可见；待审核状态清晰，但不过度警告；复杂信息采用分层展示；桌面端优先，适配平板和移动端。

**禁止**：

- 不要把 Cognition 设计成普通聊天记录；
- 不要把 Cognition 和 Knowledge 合并；
- 不要将 AI 推断直接展示为正式项目事实；
- 不要让 AI 自动覆盖已确认认知；
- 不要要求 Repository 接入后才能启动；
- 不要把外部 Agent 作为唯一认知来源；
- 不要引入 Workspace；
- 不要把 Cognition 完成度设计为 Project 完成度。

**请交付高保真页面**：1. Cognition Overview；2. Analysis Preparation；3. Proposal Review；4. Cognition Detail；5. Change Proposal Review；6. No Cognition / No Repository 空状态。

---

## 13. 核心页面线框

**A. Cognition Overview**

```
┌──────────────────────────────────────────────────────────────────────┐
│ Project / Cognition                                                   │
│                                                                      │
│ Project Cognition                         [Start Analysis]            │
│ Build and maintain a shared understanding of this project.           │
│                                                                      │
│ ┌────────────────┐ ┌────────────────┐ ┌────────────────┐             │
│ │ Confirmed      │ │ Pending        │ │ Domains        │             │
│ │ Cognition      │ │ Proposals      │ │ Coverage       │             │
│ │      32        │ │       5        │ │      4/6       │             │
│ └────────────────┘ └────────────────┘ └────────────────┘             │
│                                                                      │
│ Cognition Domains                                                    │
│ ┌──────────────────────┐ ┌──────────────────────┐                    │
│ │ Project Identity     │ │ Business Domain      │                    │
│ │ 6 confirmed          │ │ 8 confirmed          │                    │
│ └──────────────────────┘ └──────────────────────┘                    │
│ ┌──────────────────────┐ ┌──────────────────────┐                    │
│ │ Product Cognition    │ │ Architecture         │                    │
│ │ 7 confirmed          │ │ 5 confirmed          │                    │
│ └──────────────────────┘ └──────────────────────┘                    │
│ ┌──────────────────────┐ ┌──────────────────────┐                    │
│ │ Development          │ │ Evolution            │                    │
│ │ Convention           │ │ Cognition            │                    │
│ └──────────────────────┘ └──────────────────────┘                    │
│                                                                      │
│ Pending Proposals                                                    │
│ [Review 5 proposals →]                                               │
│                                                                      │
│ Recent Cognition Changes                                             │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

**B. Proposal Review**

```
┌──────────────────────────────────────────────────────────────────────┐
│ Cognition / Pending Proposals                                        │
│                                                                      │
│ Proposal 03 of 05                         [Next Proposal →]           │
│                                                                      │
│ Domain: Architecture Cognition                                       │
│                                                                      │
│ Proposed Cognition                                                   │
│ ┌──────────────────────────────────────────────────────────────────┐ │
│ │ The application uses a layered architecture...                   │ │
│ └──────────────────────────────────────────────────────────────────┘ │
│                                                                      │
│ Evidence                                                             │
│ ├── Source: Architecture Document                                    │
│ ├── Source: Repository Analysis (if available)                       │
│ └── Source: User-confirmed decision                                  │
│                                                                      │
│ Confidence: High                                                     │
│                                                                      │
│ Impact                                                               │
│ This cognition may influence future architecture analysis.           │
│                                                                      │
│ [Reject] [Request Revision]                    [Confirm]             │
└──────────────────────────────────────────────────────────────────────┘
```

**C. 认知变化审核**

```
┌──────────────────────────────────────────────────────────────────────┐
│ Cognition Change Proposal                                            │
│                                                                      │
│ Current Confirmed Cognition                                          │
│ ┌──────────────────────────────────────────────────────────────────┐ │
│ │ Current project architecture...                                  │ │
│ └──────────────────────────────────────────────────────────────────┘ │
│                                                                      │
│ Newly Observed Information                                           │
│ ┌──────────────────────────────────────────────────────────────────┐ │
│ │ New evidence indicates...                                        │ │
│ └──────────────────────────────────────────────────────────────────┘ │
│                                                                      │
│ Evidence / Sources                                                   │
│                                                                      │
│ Proposed Change                                                      │
│                                                                      │
│ [Reject] [Request More Evidence] [Request Revision] [Confirm Change] │
└──────────────────────────────────────────────────────────────────────┘
```

---

### 验收标准（原稿未标章节号，位于 §13 线框与 §15 之间）

| 编号 | 验收项 | 通过标准 |
| --- | --- | --- |
| AC-01 | 独立可用 | 无 Repository、MCP、外部 Agent 时可使用内置 AI-PM |
| AC-02 | 对象分离 | Cognition 与 Knowledge 独立管理 |
| AC-03 | 自动准备 | 可以自动汇集和整理现有上下文 |
| AC-04 | 人工启动 | 正式认知分析必须由用户启动 |
| AC-05 | 提案机制 | AI 分析结果以 Proposal 形式呈现 |
| AC-06 | 逐项确认 | 用户可以独立确认、驳回、要求修订 |
| AC-07 | 证据来源 | 每项认知提案能够查看证据和来源 |
| AC-08 | 防止覆盖 | AI 不得直接修改已确认认知 |
| AC-09 | 变化治理 | 已确认认知变化必须经过人工确认 |
| AC-10 | 历史追溯 | 保留认知版本、确认人和变化原因 |
| AC-11 | 混合 Context | 核心认知默认注入，其他内容按任务检索 |
| AC-12 | Context 区分 | 已确认认知、未确认提案、Knowledge、推断明确区分 |
| AC-13 | 外部来源 | Repository / Agent 分析结果进入统一提案机制 |
| AC-14 | 权限审计 | 确认和修改等关键操作可追溯 |
| AC-15 | 缺口处理 | 资料不足时明确提出缺口，不编造认知 |

---

## 15. 技术核验任务

这份设计稿**不能直接推导出当前后端已经具备 Cognition 领域模型**。技术侧需要先核验：

1. 是否已经存在独立的 Project Cognition 数据模型；
2. 是否已经存在 Cognition Proposal 或可复用的提案机制；
3. 是否已有正式确认人、确认时间及版本历史；
4. 是否能区分 AI Actor 和 Human Actor；
5. Knowledge 是否已有独立生命周期；
6. 是否存在统一的来源和证据引用机制；
7. AI-PM 当前 Context 组装是否支持默认注入与按需检索；
8. 是否能识别未确认提案，防止它们混入正式上下文；
9. Repository / Discovery 分析结果是否能进入统一提案审核流程；
10. 是否存在认知变化检测、冲突检测和历史版本保留机制。

**特别注意**：不要为了实现 Cognition 而直接复制一套与现有 Discovery Proposal、Knowledge Proposal 或 Document Proposal 重复的提案状态机。应先确认哪些机制能够复用，哪些领域语义必须独立。

---

## 结论（原稿自带）

**S01-04 V1.2 已形成设计基线候选。**

本稿最重要的产品定义是：

> AI-PM 持续理解项目，AI 负责提出认知，人负责确认认知，系统负责保存认知历史并在后续工作中正确使用认知。

这不是一个一次性 AI 分析页面，而是 Project 长期运行的**认知治理能力**。

下一步建议让技术侧对第 15 节进行代码事实核验，避免设计与现有 Discovery、Knowledge、Proposal 机制重复建设。

---

## 附 A · 记录者附注：与既有文档的关系登记（事实登记，非评审）

| # | 既有文档/事实 | 关系登记 |
| --- | --- | --- |
| A-1 | 既有稿 `s01-04-ai-pm-cognition-ux-v1-1.md` | **Q1~Q5 裁决正文已定位**：该稿 :56（Q1=B Cognition+Knowledge 双对象）/:241（Q2=C 自动准备+人工启动）/:380（Q3=B 逐项确认）/:438（Q4=C 变化均产生提案）/:493（Q5=C 核心默认注入+按需检索）+ :891-895 收编表。本稿显式映射 Q2=C（§3.2）/Q3=B（§4.1）/Q4=C（§5）/Q5=C（§6）与既有裁决**全部一致**；Q1 未显式标注，但其 §1.2「独立对象」即 Q1=B 的落点 |
| A-2 | 现有提案机制三套（代码事实） | S01-01 总览稿 §3.3 已登记：`discovery.discovery_proposal` 四态（pending/approved/rejected/conflict）、`ai.document_proposal` 八态管线、discovery_finding 四态+派生「要求补充」。本稿 §15「特别注意不要复制重复提案状态机」与该事实同向；§15-1~10 核验项须与总览稿 §14 缺口表（G-06 认知派生语义错误 P0 / G-07 `confirmed` 态从未被写入 P1）对齐合并 |
| A-3 | 已实施 CognitionPage V1 | S01-04 前端已实施（迁移 0036 + `pm_cognition.rs` + CognitionPage，V1=结构化事实派生、不虚构 LLM）；本稿 §2 六领域+§6 Context 注入的范围显著大于已实施 V1，基线确认后须核对实施边界与演进路径 |
| A-4 | 裁决记录 R-S01-01-01 | §0-6、§1.3、§7.1、§9.2（无 Repository/MCP/外部 Agent 时内置 AI-PM 照常可用、Repository 非启动条件）与裁决方案 A 同向 |
| A-5 | 第一份修订稿（Command Center V1.1） | §8 页面 IA 中 Cognition 独立导航项与「Requirements/Discovery/Knowledge/Repositories」并列，比第一份修订稿 §2 五区多了独立 Discovery 项——两稿导航枚举的收敛登记为待基线确认事项 |
| A-6 | 执行就绪/状态隔离惯例 | §11「Prepared/Completed/Confirmed 均不等于下游就绪」的措辞惯例与 S01-03 V1.2 §9（Access Verified ≠ Discovery Completed ≠ Execution Ready）及 I5 §4.5 同构 |
| A-7 | 工作包编号 | 本稿保持 S01-04=AI-PM Cognition，与第一份修订稿 §九 顺序表一致 |

## 附 B · 待基线确认后的执行挂账

1. §15 十项技术核验与总览稿 §14 缺口表（G-01~G-09）、S01-03 V1.2 §10 十七项清单合并为一份统一核验清单，避免三稿三套清单漂移。
2. 已实施 CognitionPage V1（结构化事实派生）与本稿六领域长期认知的范围差：基线确认后明确 V1→V1.2 的演进路径（复用 0036 迁移与新表边界、提案机制复用 vs 新建）。
3. Actor 身份可区分（§10）与既有 A*（MCP 无确认入口、REST 独立交互上下文）治理线的关系核对。
4. §11 状态语义按原稿约束「先核验已有模型，避免重复造状态机」后冻结。
5. 本稿状态=设计基线候选：待技术现状核验 + 用户基线确认；记录阶段不执行冻结。
