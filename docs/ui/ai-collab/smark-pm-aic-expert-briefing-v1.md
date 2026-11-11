# AI Collaboration Workspace V1.0 — 现状说明（给专家）

| 项目 | 内容 |
|---|---|
| 日期 | 2026-11-11 |
| 目的 | 让专家看到**我们现在的 UI 实际长什么样**，再据此给意见；之前专家稿基于旧版本理解 |
| 读者 | 设计专家 |
| 自包含 | 本文含全部关键事实，**无需翻我们的仓库**；文末附文件地图便于深挖 |
| 事实来源 | 前端源码 + 设计系统文档 + 本仓设计文档（均在文末给出路径与行号） |

---

## 〇、一页速览

**我们正在做的产品**：Smark PM —— AI 驱动的项目管理协作工作台（Web 桌面端）。

**当前状态**：
- 已上线一个 **三栏 Shell**：左导航 + 主区 + 右侧上下文栏，另有顶部全局栏与**底部停靠的 AI 指令栏**。
- AI 对话目前是**独立路由页面**（`/project/assistant`），不是常驻侧栏。
- 右侧栏目前是**单张上下文卡片**（当前项目 / 仓库 / 关联 / 注入来源），**没有 Tab**。

**本轮要做的**（设计阶段，非编码）：
把 Shell 演进为**四区**——导航 188 / 主区 518 / **AI Chat 446（升级为常驻独立列）** / 上下文面板 288，并把右栏升级为 **Context / Sources / Results / Actions 四 Tab + History Drawer**。

> **一句话给专家**：我们不是从零设计一个新壳，而是在**已上线三栏壳**上做一次结构升级；AI Chat 从「底部 72~120px 指令栏 + 独立路由页」变成「常驻 446px 独立列」，这是本轮最大的结构变化。

---

## 一、现状 A — 已上线 Shell 的真实结构

代码事实（`ai-frontend/src/styles/index.css:24-99`、`ai-frontend/src/styles/tokens.css:77-82`）：

```text
┌──────────────────────────────────────────────────────────────┐
│ TopBar  52px  （横跨三栏）                                     │
├────────────┬────────────────────────────────┬────────────────┤
│            │                                │                │
│ SideNav    │  Main Area (自适应 1fr)          │ Context Panel  │
│  232px     │                                │   320px        │
│ (折叠 60px) │  ┌──────────────────────────┐  │ (整栏可关)      │
│            │  │  页面内容（独立滚动）       │  │                │
│            │  └──────────────────────────┘  │  单张上下文卡    │
│            │  ┌──────────────────────────┐  │  （无 Tab）      │
│            │  │  CommandBar 72~120px     │  │                │
│            │  │  （AI 指令栏，停靠底部）    │  │                │
└────────────┴────────────────────────────────┴────────────────┘
```

关键实现特征：

| 事实 | 位置 | 说明 |
|---|---|---|
| 三栏 grid `232px \| 1fr \| 320px` | `index.css:30-33` | 顶部 52px 顶栏另占一行（`:34`） |
| 主区自适应，不固定宽 | `index.css:32` `minmax(0, 1fr)` | 主区宽度随窗口变化 |
| 右栏可整栏关闭 | `index.css:45-49` `data-context="closed"` | 不是折叠为图标，是完全移除该列 |
| 左栏可折叠为 60px | `tokens.css:78` | |
| 滚动收敛在主区内部 | `index.css:68-82` | main 本体 `overflow:hidden`，只有 `.workspace-main-scroll` 滚动；这是修复过一次「长页内容渗出盖住指令栏」的结论 |
| 三栏子项**显式声明 grid 列位** | `index.css:51-54` 注释 | Grid 自动放置曾导致右栏抢列、主区被挤到第 3 列（真实踩过的坑） |

**导航结构**：不是扁平列表，而是**四组分层**（`ai-frontend/src/router/navigation.ts:88-258`，菜单与路由同源）：

| 组 | 项 |
|---|---|
| PERSONAL | Personal Home · Inbox |
| WORKSPACE | Dashboard · Projects |
| MODULES（**仅选中 Project 时出现**） | Discovery · Requirements · Technical Design · Task Management · Execution · AI Assistant · Knowledge · Dev Environment |
| ADMINISTRATION（默认收起） | Settings · Members · Announcements · AI Providers |

---

## 二、现状 B — 视觉系统（已锁定，请勿另起）

**V2「Slate & Muted Teal」**——石墨蓝灰 + 雾蓝青，**深色为默认主题**（`:root` 即深色，浅色经 `[data-theme="light"]` 覆盖）。

| 角色 | Dark（默认渲染目标） | Light 变体 |
|---|---|---|
| Canvas | #171C24 | #F5F8FA |
| Sidebar / nav | #1D2631 | #EDF2F5 |
| Surface（卡片、表格） | #222A34 | #FFFFFF |
| Elevated（嵌套、徽标、hover） | #2B3541 | #EAF0F3 |
| Border / separator | #374452 | #DCE4E9 |
| Text primary / secondary | #E7EDF2 / #A8BBC9 | per token file |
| Accent（主操作、选中） | #75A9A4 · hover #8AB9B4 · pressed #63948F | 同色相 |
| Info / Success / Warning / Danger | #6EA8FE / #34C08B / #E0B15E / #F07E88 | per token file |

- Typography：Page 21 / Section 15 / Body 13 / Caption 12 / Micro 11；ID、SHA、时长用 tabular numerals；间距 8px 体系。
- **明确禁止**：紫色（任何场合）、渐变、大面积装饰插画、夸张圆角卡片、过大 KPI 数字、消费级 Chatbot 风格。
- 令牌唯一真源：`docs/design/pm-ai-design-tokens-v2.css`；设计系统说明 `docs/design/pm-ai-ui-design-system-v2.md`。
- 组件**禁止裸色值**，一切引用 token。

> 说明：专家上一稿写的是「White background / Professional blue primary」。那是更早的浅色方向，**已被内部裁决两次收敛到 V2 深色**，全应用只有一套 token。若专家坚持浅色，请说明是要推翻整套 token（影响全应用），还是本模块豁免双主题。

---

## 三、现状 C — 当前 AI 协作的实际形态

这是专家最需要更新认知的部分——**现状与目标差得最远的就是这里**。

### 3.1 AI 指令栏（CommandBar，壳级组件）

停靠在**主列底部**，与主列等宽（不压右栏）：

| 形态 | 高度 | 内容 |
|---|---|---|
| 收起 | 72px | 输入框 + 附件 + 上下文引用 + 发送 |
| 展开 | 120px | 追加「作用域条」（scope）+ 提示条（hints） |

位置 `ai-frontend/src/components/shell/CommandBar.vue`；样式 `index.css:101-104`；高度 token `tokens.css:81-82`。

### 3.2 完整 AI 对话是独立路由页

`/project/assistant` → `ai-frontend/src/views/project/assistant/AssistantView.vue`。包含会话切换器（select + 新建 + 状态）、对话时间线、interview 阶段视图。**它占用主区整块，不与业务页面并存。**

### 3.3 右侧上下文栏（现状：无 Tab）

`ai-frontend/src/components/shell/ContextPanel.vue`（492 行）目前是**一张滚动卡片**，自上而下：

- 当前 Project（`context.currentProject`）
- Repository 详情（`context.repositoryDetails`）
- Related（`context.related`）
- AI 提示（`context.aiNotice`）
- 注入来源（`context.injectionSources` / sourceProject / sourceRepository / sourceNone）
- Read-only 标记（`context.readOnly`）
- Pinned（`context.pinned`）

**没有 Context / Sources / Results / Actions 任何 Tab**，也没有 History Drawer。

### 3.4 AI 上下文与注入的真实机制（已实现部分）

| 能力 | 实现 |
|---|---|
| 上下文单一真源 | `stores/shell.ts:147` `assistantContext`（指令栏作用域串 / 右栏卡 / 会话页三处同源） |
| 提交时刻快照交接 | `stores/shell.ts:200` `queueAssistantPrompt` → `:211` `takeAssistantPrompt`（导航与切项目不改变快照） |
| 线程级锚点落库 | `migrations/0003_conversation_context_anchor.sql`（JSONB 单列 + 白名单 + 跨项目 CHECK） |
| 锚点校验 | `crates/pm-application/src/ai_conversations.rs:82` `ContextAnchor::validate` |
| 服务端按锚点重投影 | `crates/pm-ai/src/chat_context.rs:100` `build_chat_context` |
| 注入记账落库 | `crates/pm-ai/src/conversation_reply.rs:438` `message.metadata.injection` |
| 响应卡信封协议 | `crates/pm-application/src/ai_interaction_envelope.rs`（8 类白名单 + 11 态 + payload 必填，fail-closed） |
| 交互写回闭环 | `migrations/0005_conversation_interaction.sql`（终态幂等 / 换 action 409 / 属主外 404 同形） |

**已知断点**（诚实性，最优先）：右栏读的是**活态** `shell.assistantContext`，而注入按**线程锚点**重新投影；`metadata.injection` 前端**零消费**。即「右栏宣称 AI 看到了什么」与「AI 实际看到了什么」目前不等价。

---

## 四、目标态 — 四区 Shell（设计中）

```text
┌──────┬───────────────┬─────────────────┬──────────┐
│      │               │                 │          │
│ Nav  │  Main Area    │   AI Chat       │ Context  │
│ 188  │    518        │     446         │  Panel   │
│      │               │                 │  288     │
│      │               │                 │          │
└──────┴───────────────┴─────────────────┴──────────┘
                                            └→ History Drawer
                                                (覆盖右栏，不改宽度)
```

### 与现状的 6 处结构差异（专家评审重点）

| # | 现状 | 目标 | 性质 |
|---|---|---|---|
| 1 | 三栏 232/1fr/320 | 四栏 188/518/446/288 | **壳结构改造**，主区从自适应变为固定 518 |
| 2 | AI Chat = 主列底部 72~120px 指令栏 | AI Chat = **常驻 446px 独立列** | **最大结构变化**：AI 从「随时可叫的指令条」变成「持续在场的工作区」 |
| 3 | 完整 AI 对话在独立路由页 `/project/assistant` | AI Chat 列在业务页面旁**并行存在** | 业务页与对话同屏 |
| 4 | 右栏单张卡片（无 Tab） | 右栏四 Tab + History Drawer | 右栏从「展示」升级为「管理 + 操作」 |
| 5 | 主区滚动 + 右栏独立滚动 | 四区各自独立滚动 | 需防区域重叠与宽度漂移 |
| 6 | 无历史抽屉 | History Drawer（五类记录，可筛选、可看详情） | 新增 |

### 已锁定的交互规则（DR-C3~C6，已内部裁决）

- **右栏严格四个主 Tab**（Context / Sources / Results / Actions），**History 不是第五个 Tab**，走 Drawer；Drawer 覆盖右栏视觉区，不改四区宽度。
- **七类 UI Card ↔ 八类后端 Envelope**（informational / recommendation / proposal / question / decision / artifact / action-request / execution-result）；Artifact 内嵌不设独立主卡；Risk 必须有结构化字段（severity/impact/likelihood/affected_objects/mitigation/status），否则降级为普通 Recommendation；Progress 状态必须来自真实执行状态，不得由 AI 文本推断成功。
- **Default Deny**：有效权限 = Team ∩ Project ∩ Action Policy，无明确表态即 DENY；受保护操作（Requirement / Revision / Knowledge / Decision 的正式审批）必须真人批准，且不可绕过；**Approval Pending 是强拦截态**。`auto_accept` 拆分为「可接受建议进草稿」与「不可正式审批」。
- **会话必须绑定 Project**（与 `ai.conversation.project_id NOT NULL` 一致）；Global AI 入口只是 Project 选择器；切 Project 不转移 Conversation / Context / Sources / Results / Actions / History。
- **Main Area 仅 518px**：不强制双栏 Original/Proposed Diff，改用 Inline / Unified Diff 或单列切换。

---

## 五、当前正在推进的第一阶段

**本轮只做两页**，顺序不可颠倒：

1. **AI-01 Global Workspace Shell** —— 建立四区布局、AI Chat 常驻列、右栏四 Tab、History Drawer 入口、项目选择器流程。
2. **AI-02 Context Management** —— **复制** AI-01 的 Shell（不得重新设计），只深化右栏 Context 管理：Context 列表、状态、Add Context 多选、Enable/Disable/Remove、Details、Sources/Results/Actions 三 Tab、History Drawer 完整交互。

**门禁**：AI-01 与 AI-02 全部通过验收后，才允许把 Shell 复制到其余五个页面；其余页面只能复用，不得自行改尺寸 / Tab 结构 / History 入口规则。

### Context 状态采用双轴（请专家确认这个表达是否够用）

| 轴 | 取值 | 含义 |
|---|---|---|
| **归属轴** | Available / Attached / Read-only | 与当前 Conversation 的关系（Read-only 是可写性标记，可与 Attached 组合） |
| **生命周期轴** | Active / Inactive / Pending / Unavailable | 条目自身状态（Disable → Inactive；Pending 绝不自动转 Active；Unavailable 必须给出原因） |

AI 实际使用的是「**Attached + Active**」；Attached + Inactive = 已附加但停用（Disable ≠ Remove）。

### 原型硬要求

演示场景 17 步全部必须**真实改变原型状态**，不允许「只做视觉上可点击的按钮」。Overview 计数由状态驱动，非装饰数字。

---

## 六、真正需要专家回答的 4 个问题

> 这四问是我们自己无法决定、且专家的判断比我们的更有价值的点。其余部分我们已锁定，不必再提。

| # | 问题 | 背景 |
|---|---|---|
| **Q1** | **AI Chat 常驻列 vs 底部指令栏**：把 AI 升为常驻 446px 独立列，是否会挤压主区（518px 在 1440 画板下偏窄）？有无更优解（如 AI 列可拖拽变宽、主区可临时全屏、或 AI 列默认折叠为窄条）？ | 这是本轮最大的结构改动，牵动所有页面 |
| **Q2** | **518px 主区的双栏困境**：Requirement 编辑 + Diff 是本模块高频场景，518px 内如何同时呈现「结构化对比 + 内联编辑 + 可读正文」？我们暂定 Inline/Unified Diff + 单列切换 + 可展开比较视图，是否够用？ | DR-C5 已定死尺寸，交互还需专家给形态 |
| **Q3** | **右栏 288px 要承载 4 Tab + 详情 + Drawer**：是否过窄？Tab 内的 Context 条目（名称/类型/来源/摘要/状态/时间/关联对象/操作）在这个宽度里如何取舍层级——哪些展开、哪些进详情？ | 直接影响 AI-02 的信息密度设计 |
| **Q4** | **Context 双轴状态**：Available/Attached/Read-only × Active/Inactive/Pending/Unavailable 是否过载？是否应合并为单轴，或改用其他词表（如 Attached / Detached / Disabled）？ | 术语一旦定稿会进前后端契约 |

---

## 七、附：文件地图（想深挖时看）

| 主题 | 路径 |
|---|---|
| 视觉令牌真源 | `docs/design/pm-ai-design-tokens-v2.css` |
| 设计系统说明 | `docs/design/pm-ai-ui-design-system-v2.md` |
| 壳布局 CSS | `ai-frontend/src/styles/index.css`（壳基类 24-120 行） |
| 尺寸 token | `ai-frontend/src/styles/tokens.css`（77-82 行） |
| 壳组件 | `ai-frontend/src/components/shell/`（WorkspaceShell / SideNav / TopBar / CommandBar / ContextPanel） |
| 导航树（菜单与路由同源） | `ai-frontend/src/router/navigation.ts` |
| AI 会话页 | `ai-frontend/src/views/project/assistant/AssistantView.vue` |
| 上下文单一真源 | `ai-frontend/src/stores/shell.ts` |
| 响应卡信封协议 | `crates/pm-application/src/ai_interaction_envelope.rs` |
| 会话/锚点持久化 | `migrations/0003_conversation_context_anchor.sql`、`0005_conversation_interaction.sql` |
| 设计基线 | `docs/product/ai-pm-ai-collaboration-workspace-v1.md` |
| Figma 提示词包（00 + 01~07 + 90） | `docs/ui/ai-collab/` |
| 裁决记录（DR-C1~C6） | `docs/ui/ai-collab/smark-pm-aic-95-decision-record-v1.md` |

---

## 八、给专家的三条明确边界

1. **视觉不重开**：V2 深色已锁定两次，令牌唯一。若要浅色，请明确是「全应用换肤」还是「本模块豁免」。
2. **结构不重开**：四区尺寸 188/518/446/288、右栏四 Tab、History 走 Drawer、会话绑定 Project —— 均已裁决。
3. **实现另算**：本轮是设计。默认拒绝策略与真人审批守卫（DEF-P0-01/02）目前**仅登记未修**，后端修复需独立代码验证，**UI 做对了不等于后端已强制**。
