# 0. 所有页面共用的 Global Design Prompt（v1-1）

> 本文件是七份页面提示词的共同前置规范。每次投喂 Figma AI：**先给本文件全文，再给对应页面文件全文**。
> 视觉锁定 V2 设计系统（DR-C1），令牌唯一真源 `docs/design/pm-ai-design-tokens-v2.css`。
> 执行方式（见 90 §A）：按十步顺序生成；**AI-01 完成后复制其 Workspace Shell 到其余六页，禁止每页重新生成四栏布局**。最终验收以 90 §B 三成败场景为门。
> 裁决依据：[95 裁决记录](smark-pm-aic-95-decision-record-v1.md)（DR-C1~C6，与本文件冲突时以裁决记录为准）。

## 0.1 产品定位

设计 Smark PM 的 AI Collaboration Workspace。

它是 Project Management 与 AI Collaboration 的统一工作环境，**不是普通 AI Chat，也不是把 Chat 嵌入传统项目管理页面**。

用户通过三个主要工作区域完成协作：
- **Main Area**：业务操作。
- **AI Chat**：自然语言沟通。
- **Context Panel**：上下文、来源、结果及操作管理。

左侧 Navigation 提供全局导航。

用户可以：查看和管理 Project · 与 AI 讨论项目问题 · 让 AI 分析 Requirements / Design / Tasks / Knowledge / Decisions · 将 AI 输出转化为业务草稿 · 审核修改并接受 AI 建议 · 授权 AI 执行具体操作 · 追踪执行结果与异常。

## 0.2 固定四区域布局

**画板：1440 × 1000px。正式尺寸（DR-C5）：**

| 区域 | 宽度 | 占比 |
|---|---:|---:|
| Navigation | 188px | 13% |
| Main Area | 518px | 36% |
| AI Chat | 446px | 31% |
| Context Panel | 288px | 20% |
| **Total** | **1440px** | **100%** |

```text
┌──────────┬──────────────────┬────────────────┬────────────┐
│          │                  │                │            │
│          │                  │                │            │
│          │                  │                │            │
│ Navigation│   Main Area     │   AI Chat      │  Context   │
│  188px   │     518px        │    446px       │  288px     │
│          │                  │                │            │
│          │                  │                │            │
└──────────┴──────────────────┴────────────────┴────────────┘
```

四区规则：
1. 四区域同时存在。
2. **不增加第五个主区域**。
3. 四个区域支持合理的宽度调整。
4. AI Chat 不因业务页面切换而消失。
5. Context Panel 支持折叠。
6. 用户调整宽度不能破坏布局。
7. **四区域之间不额外添加未计入宽度的固定边距**——视觉分隔由区域内部 padding、分隔线与内容间距承担。
8. **各区域独立滚动**，不得因单个区域内容过长导致整页错位或位移。

> **52px TopBar 保留在四栏之上，横跨主体工作区**（DR-C8）；以上四栏宽度指 **TopBar 下方的主体区域**。
>
> **渐进迁移（DR-C8）**：Shell 骨架按三步推进——① 建立四栏容器与 AI Chat 列 → ② 迁移现有会话能力进常驻 AI Chat 列（复用既有路由与会话机制，**不重建会话体系**）→ ③ 升级 Context Panel（四 Tab + History Drawer）。原底部指令栏（CommandBar）**升级为 AI Chat 列中的输入组件**，不再占据 Main Area 底部。

### A. Left Navigation（188px，固定，不因 AI 操作改变）

导航按**分组分层**呈现（与已上线应用壳一致，非扁平列表）：

| 组 | 项 |
|---|---|
| PERSONAL | Personal Home · Inbox |
| WORKSPACE | Dashboard · Projects |
| MODULES（**仅选中 Project 时出现**） | Discovery · Requirements · Technical Design · Task Management · Execution · AI Assistant · Knowledge · Dev Environment |
| ADMINISTRATION（默认收起） | Settings · Members · Announcements · AI Providers |

底部显示当前用户信息。当前菜单需有清晰 Active 状态；只用 Sidebar 表面色（--ws-sidebar），不用强调色大面块。

### B. Main Area（518px）

显示当前业务对象，包含：Breadcrumb · Page Title · Object Status · Business Content · Relevant Actions。

> **518px 宽度约束（DR-C5 / DR-C8）**：不允许强制双栏 Original / Proposed Diff。差异展示采用 Inline Diff / Unified Diff / 单列 Original–Proposed 切换 / 必要时独立展开比较视图。

**Main Area 专注模式（Focus Mode，DR-C8）**——518px 是常态宽度，复杂编辑不永久受限：

- 临时收起 AI Chat 与 Context Panel，扩大业务工作区。
- 退出后**恢复原来的四栏布局**。
- **不改变**当前 Project、Conversation、Context。
- **不丢失**正在编辑的内容或对话状态。
- 需有明确的进入入口与退出方式（键盘可见的控件 + 视觉状态标识）。

### C. AI Chat Assistance（446px，常驻独立列）

包含：Conversation Header · Conversation Messages · AI Response Cards · Message Composer（Message Input · Attachment · Context Reference · Send / Stop / Retry）。

- AI Chat 独立存在，**不嵌入业务页面**，不使用全屏聊天替代主 UI。
- **Composer 常驻在 AI Chat 列底部**（由原底部指令栏升级而来，见 0.2 渐进迁移说明）。
- 与业务页面**并行共存**：进入 Project 后 AI Chat 持续在场，不需跳转独立页面。

### D. Right Context Panel（288px，始终独立存在）

**仅有四个一级 Tab**（DR-C3）：

| 主 Tab | 职责 |
|---|---|
| Context | 当前项目、可用对象、已附加对象 |
| Sources | AI 使用的信息来源及版本 |
| Results | AI 输出及其状态 |
| Actions | 待授权操作、执行任务及操作结果 |

**History 不是第五个主 Tab**，通过二级入口或 History Drawer 访问。Drawer 内容五类：Conversation History · AI Action History · Context Change History · Approval History · Execution History，支持分类查看；**历史保留发生时的状态，不得被当前 Context 覆盖**；Drawer 不得改变四区布局。

**信息密度规则（DR-C8）**：288px 内**只展示高价值摘要与状态**（名称、类型、状态等关键信息）；**完整内容放入详情 Drawer 或展开视图**，不强行塞入列表。

## 0.3 视觉规范（DR-C1 · V2「Slate & Muted Teal」）

专业、克制、现代的 B2B SaaS 风格，高信息密度，适合长时间工作。**深色为默认渲染目标**；浅色变体用于后续主题切换，Figma 内以深色为准。

| Role | Dark（默认渲染目标） | Light 变体 |
|---|---|---|
| Canvas（Main Area 底） | #171C24 | #F5F8FA |
| Sidebar / Navigation | #1D2631 | #EDF2F5 |
| Surface（卡片、表格、Chat 列底） | #222A34 | #FFFFFF |
| Elevated（嵌套填充、徽标、hover） | #2B3541 | #EAF0F3 |
| Border / separator | #374452 | #DCE4E9 |
| Text primary / secondary | #E7EDF2 / #A8BBC9 | per token file |
| Accent（主操作、选中） | #75A9A4 · hover #8AB9B4 · pressed #63948F | same hue |
| Accent tint surface | #253735 | — |
| Info / Success / Warning / Danger | #6EA8FE / #34C08B / #E0B15E / #F07E88 | per token file |

四区表面映射：Navigation 用 Sidebar #1D2631；Main Area 用 Canvas #171C24 + Surface 卡片；AI Chat 用 Surface #222A34；Context Panel 用 Surface #222A34 + Elevated #2B3541 分组。

状态色映射：运行中/进行 → Info #6EA8FE；等待人工/待授权/Approval Pending → Warning #E0B15E；失败/阻塞 → Danger #F07E88；完成/已接受 → Success #34C08B；中性（排队/取消/草稿）→ 次要文本灰 #A8BBC9 + Elevated 填充。**AI 生成内容一律 Info 色 tint +「AI」徽标。**

Typography：Page 21 / Section 15 / Body 13 / Caption 12 / Micro 11；ID、版本号、SHA、时长用等宽数字（tabular numerals）；间距 8px 体系。

不要使用：
- **紫色**（任何场合）。
- 渐变。
- 大面积装饰性插画。
- 夸张的圆角卡片。
- 过大的 KPI 数字。
- 消费级 AI Chatbot 风格。
- 五 Tab Context Panel。
- 全屏 AI Chat。

组件禁止裸色值：Figma 内建立 color styles 与上表一一对应，命名对齐 `--ws-*` token。

## 0.4 统一交互原则

1. Main Area 是业务操作区域。
2. AI Chat 是自然语言交互区域。
3. Context Panel 是 AI 上下文与结果管理区域。
4. AI 生成内容默认为非终态 Draft 或 Proposal。
5. 正式审批必须由有权限的真人完成。
6. Navigation 只改变 Main Area；AI Chat 与 Context Panel 保持挂载。
7. 显式 Attached 的上下文在切页后保持；新打开的业务对象只进 Available，不自动 Attached。
8. Project 切换不得污染原 Conversation 与 Context。
9. 历史结果保留其当时的上下文快照，不被当前 Context 覆盖。

## 0.5 卡名词表映射（DR-C4 · UI 词 ↔ 信封词）

后端维持八类 Envelope，不新增类型。前端必须建立统一 Response Rendering Contract，**不允许自行扩展白名单**。

| UI Card（Figma 用） | Envelope（实现用） | 渲染要求 |
|---|---|---|
| Analysis Card | informational | 分析、结论、证据 |
| Proposal Card | proposal | 修改方案、前后对比 |
| Risk Card | recommendation | severity / impact / likelihood |
| Question Card | question | 问题、选项、补充回答 |
| Decision Card | decision | 决策、备选方案 |
| Execution Card | action-request | 执行计划、授权请求 |
| Progress Card | execution-result | 进度、执行状态、结果 |
| —（无独立主卡） | artifact | 可嵌入业务产物：Requirement Draft · Design Document · Analysis Report · Execution Report |

**Risk 识别**：必须有结构化字段 `severity` · `impact` · `likelihood` · `affected_objects` · `mitigation` · `status`；**没有这些字段时显示普通 Recommendation**，不得由 AI 文本推断风险。

**Progress 识别**：状态必须依据实际执行状态（Running / Partially Completed / Failed / Completed / Cancelled），**不得根据 AI 文本推断成功**。

**Action Request**：`action-request` 仅代表请求执行，**不代表已获授权**。

## 0.6 权限与审批模型（DR-C2 · Default Deny）

```
Effective Permission = Team Policy ∩ Project Policy ∩ Action Policy
```

- **默认拒绝**：任何权限策略没有明确表态 → DENY。不允许默认放行。
- **受保护操作必须真人审批**：Formal Requirement Approval · Formal Requirement Revision Confirmation · Formal Knowledge Confirmation · Formal Decision Approval · 其他正式业务批准或发布操作。`require_human_approval = true` 不可绕过。
- **AI 不得**：批准自己的 Proposal · 自动确认正式 Requirement · 自动批准正式 Revision · 通过 `auto_accept` 绕过人工审批。
- **auto_accept 语义拆分**：自动接受建议进入**非正式 Draft** 允许（须有明确权限）；自动修改正式业务对象、批准正式 Revision、发布正式 Requirement 一律禁止。
- **强制 UI 状态**：Approval Pending · Human Approval Required · Permission Denied · Approved · Rejected。**Approval Pending 是强拦截态**，AI Action 在审批完成前不得产生正式批准结果。

## 0.7 会话与项目绑定（DR-C6）

- **每个 Conversation 必须绑定 Project**；不创建未绑定 Project 的 Conversation（与 `ai.conversation.project_id NOT NULL` 一致）。
- **Global AI 只是 Project 选择入口**，不是独立 Global Conversation：点击 Global AI → 展示 Project 选择界面 → 选 Project → 创建或恢复该 Project 的 Conversation → 进入对应 Project AI Workspace。
- **Project 切换**：当前 Conversation 不自动转移 · 当前 Context 不带入其他 Project · 用户须进入目标 Project 的 Conversation · 历史 Conversation 保留原 Project 归属。
- V1 不含：Team-level Conversation · Multi-project Context · Cross-project Knowledge Search。

## 0.8 共享状态词表

- 上下文三态：**Available / Attached / Read-only**（Attached 必须带版本/Revision）。
- 审批状态机：`PROPOSED → DRAFT_CREATED → SUBMITTED → APPROVAL_PENDING → HUMAN_APPROVED → OFFICIAL_REVISION`（`APPROVAL_PENDING` 可转 `REJECTED`）。**AI Agent 不得从 PROPOSED / DRAFT_CREATED 直跳 HUMAN_APPROVED。**
- 权限状态机：`REQUESTED → POLICY_EVALUATION →(DENIED | APPROVAL_REQUIRED → HUMAN_APPROVAL →(REJECTED | AUTHORIZED) → EXECUTION)`。
