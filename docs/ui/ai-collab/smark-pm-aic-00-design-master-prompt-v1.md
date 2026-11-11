# 0. 所有页面共用的 Global Design Prompt

> 本文件是七份页面提示词的共同前置规范。每次投喂 Figma AI：**先给本文件全文，再给对应页面文件全文**。
> 视觉已锁定 V2 设计系统，与全应用令牌真源 `docs/design/pm-ai-design-tokens-v2.css` 一致。

## 0.1 产品定位

设计一个专业的 AI-powered Project Management Workspace。

它不是普通聊天工具，也不是传统项目管理系统增加一个聊天窗口，而是让用户在项目业务环境中与 AI 协作的统一工作空间。

用户可以：
- 查看和管理 Project。
- 与 AI 讨论项目问题。
- 让 AI 分析 Requirements、Design、Tasks、Knowledge 和 Decisions。
- 将 AI 输出转化为业务草稿。
- 审核、修改和接受 AI 建议。
- 授权 AI 执行具体操作。
- 追踪 AI 执行结果和异常。

## 0.2 固定四区域布局

所有页面使用相同的 Desktop Workspace Shell，**1440px 桌面端，四区参考比例 13 / 36 / 31 / 20**（≈190 / 520 / 440 / 290px）。比例仅为初始设计参考，实际宽度支持调整，但四个区域始终保持同一套布局，不出现第五主区。

### A. Left Navigation（≈13%，固定 ~200px，不因 AI 操作改变）

包含：
- Smark PM Logo
- Projects
- Current Project
- Requirements
- Tasks
- Designs
- Knowledge
- Decisions
- AI Assistant
- Settings
- Help
- User Profile

当前菜单需要有清晰的 Active 状态。导航只用 Sidebar 表面色（--ws-sidebar），不使用强调色大面块。

### B. Main Area（≈36%，默认 ~500px）

显示当前业务页面。

根据不同页面显示：
- Project Overview
- Requirement Detail
- Design Detail
- Task Detail
- AI Findings
- Business Object Editor

主区域顶部显示：
- Breadcrumb
- Page Title
- Object Status
- Relevant Actions

### C. AI Chat Assistance（≈31%，默认 ~420px）

包含：
- AI Chat Header
- Conversation Messages
- User Messages
- AI Messages
- AI Response Cards
- Conversation Status
- Fixed Bottom Composer

Composer 包含：
- Text Input
- Attachment
- Context Reference
- Send Button

AI Chat 不随 Main Area 页面切换而销毁，不嵌入业务页面，不使用全屏聊天替代主 UI。

### D. Right Context Panel（≈20%，默认 ~280px，始终独立存在）

顶部：
- Context Panel Title
- Collapse Button
- Panel Tabs

Tabs（4 个）：
1. Context
2. Sources
3. Results
4. Actions

History 通过二级入口或面板内 History Drawer 访问，**不设第五个主 Tab，不增加第五个主区域**。

> 三处细化说明（与设计基线的差异，以本包为准）：视觉锁定 V2（见 0.3）；History 收进 Drawer（替代基线 §3.1 五 Tab）；卡名映射（见 0.5）。

## 0.3 视觉规范（锁定 V2「Slate & Muted Teal」）

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

四区表面映射（对应已确认线框的四色分区）：

| 区域 | 底色 |
|---|---|
| Left Navigation | Sidebar #1D2631 |
| Main Area | Canvas #171C24 + Surface 卡片 |
| AI Chat | Surface #222A34 |
| Context Panel | Surface #222A34，Elevated #2B3541 分组 |

状态色映射：运行中/进行 → Info #6EA8FE；等待人工/待授权 → Warning #E0B15E；失败/阻塞 → Danger #F07E88；完成/已接受 → Success #34C08B；中性（排队/取消/草稿）→ 次要文本灰 #A8BBC9 + Elevated 填充。**AI 生成内容一律 Info 色 tint + 「AI」徽标。**

Typography：Page 21 / Section 15 / Body 13 / Caption 12 / Micro 11；ID、版本号、SHA、时长用等宽数字（tabular numerals）。

不要使用：
- 紫色（任何场合）。
- 渐变。
- 大面积装饰性插画。
- 夸张的圆角卡片。
- 过大的 KPI 数字。
- 类似消费级 AI 聊天产品的视觉风格。

组件禁止裸色值：Figma 内建立 color styles 与上表一一对应，命名对齐 `--ws-*` token。

## 0.4 统一交互原则

1. Main Area 是业务操作区域。
2. AI Chat 是自然语言交互区域。
3. Context Panel 是 AI 上下文与结果管理区域。
4. AI 生成内容默认为 Proposal 或 Draft。
5. 用户确认后才能进入正式业务流程。
6. 页面切换不得静默改变 Attached Context。
7. Project 切换不得自动污染原 Conversation。
8. 所有关键 AI 操作都应有状态反馈。

## 0.5 卡名词表映射（UI 词 ↔ 实现信封词）

Figma 与页面提示词内使用**卡名**（七类）；实现侧信封为 8 类 `RESPONSE_TYPES`（informational / recommendation / proposal / question / decision / artifact / action-request / execution-result）。两边词表必须按下表对齐，禁止出现第三套命名：

| UI 卡名（Figma 用） | 信封 RESPONSE_TYPES（实现用） | 备注 |
|---|---|---|
| Analysis Card | informational / recommendation | 分析类；带建议倾向时用 recommendation |
| Proposal Card | proposal | Accept = 进草稿，非发布 |
| Risk Card | recommendation + severity 徽标 | 风险不是独立信封类型，靠 severity 字段区分 |
| Question Card | question | |
| Decision Card | decision | |
| Execution Card | action-request | 授权入口 |
| Progress Card | execution-result | 运行态/结果态复用同一信封 |
| —（UI 不单设卡） | artifact | 知识/文档产物由 Proposal/Execution 卡内嵌展示 |

## 0.6 共享状态词表

- 上下文三态：**Available / Attached / Read-only**（Attached 必须带版本/Revision）。
- 交互生命周期 11 态与终态（accepted / rejected / deferred / resolved / failed）与实现侧 `INTERACTION_STATUSES` 同源，卡状态设计以 AI-03 为基准。
