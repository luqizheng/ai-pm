# S01-05 · Personal Home 高保真 UI 设计稿 V1.0

> **文档编号**：S01-05
> **所属阶段**：S01 — Implementation Planning & UX Design
> **设计对象**：smark-pm 个人主页 **Personal Home**（登录后默认落地页）
> **文档性质**：高保真页面设计（视觉 + 布局 + 状态 + 组件拆分）
> **版本**：V1.0
> **状态**：待评审（UX 基线见 `s01-05-personal-workspace-ux-v1-1.md` V1.1）
> **命名裁决（2026-11-04 用户）**：**Personal Home / My Home 定名** —— §11 Q1/Q2 命名冲突就此裁决；V1.1 正文中的 Workspace 命名为旧称，待同步修订
> **日期**：2026-11-04
> **视觉原型**：[`assets/s01-05-personal-home-prototype.html`](assets/s01-05-personal-home-prototype.html)（自包含，可直接浏览器打开；含正常态 + 6 类状态 + 深浅双主题切换）
> **视觉真源**：《AI R&D Workspace — 高保真设计系统 V1.0》→ `ai-frontend/src/styles/tokens.css`（`--ws-*`，本稿数值逐条取自该文件）

---

## 0. 本稿定位、与既有 V1.1 的关系

`s01-05-personal-workspace-ux-v1-1.md` §15 已把 S01-05 的后续工作排成五步，本稿对应 **第 3 步「高保真页面设计」**（第 2 步「代码事实与接口核验」尚未开始，第 4 步 UX Review、 第 5 步冻结在其后）。

因此本稿**不重开 UX 裁决**，只做四件事：把 V1.1 的文字规范落成可视页面、补齐六类状态、给出组件拆分建议、标出仍需接口核验的断言。

### 0.1 本次收编的口径变更

| 变更项 | V1.1 口径 | 本稿口径 | 依据 |
| --- | --- | --- | --- |
| 页面命名 | Personal Workspace / My Workspace | **Personal Home / My Home** | 提示词明确「Personal Home 是前端页面概念，不是领域实体」；本稿据此**禁用 Workspace 作为领域实体**，页面结构图中不再出现 Project Workspace 分支 |
| 首页分区 | Overview / Inbox / Projects / **Watchlist** / Activity | Overview / Inbox / Projects / Activity | 提示词限定四个分区；Watchlist 从首页移出（见 §11 待裁决 Q3） |
| 概览指标 | 08 Urgent · 03 Decisions · 02 Attention | **Needs Attention / Assigned to Me / Active Projects** 三项 | 提示词 §三.3；且「Decisions」是 Human Actions 的子集，单列会与 Inbox 重复计数 |
| 栅格 | 文字描述，无比例 | Row2 = Inbox 60% / Projects 40%（实现取 `1.6fr / 1fr`） | 提示词 §四 |
| 首屏标题 | Good morning（无副标题规范） | Good morning, {User} + `Here's what needs your attention today.` | 提示词 §三.2 |

### 0.2 本稿的硬性禁区（自检通过）

- [x] 未引入 Workspace / PersonalWorkspace / WorkspaceMember 等领域对象
- [x] 未新增个人任务状态机；首页只是跨领域读模型的消费端
- [x] 首页主体无 AI 聊天面板（页底指令栏是**壳级全局能力**，非首页内容）
- [x] 未把普通通知当待办（`ui/notification-item` 不用于 Inbox，见 §9.3）
- [x] 未虚构后端接口；所有字段都标注为「需接口核验」（§11）
- [x] 空态 / 错误态 / 无权限态未用示例数据填充

---

## 1. 设计原则继承

沿用 V1.1 §1.2 六条，并补充本稿的三条视觉纪律：

| 原则 | 本稿落法 |
| --- | --- |
| Human-centric | 首屏第一视觉块是「需要我处理的事」，不是系统数据 |
| Cross-project | 顶栏上下文显示 `All projects`（无单一项目），首页所有条目携带来源项目 |
| Action-oriented | Inbox 优先于 Projects；Overview 指标可点，直接下钻 |
| Non-duplicative | 项目卡只给 6 个字段，不复制项目总控台的统计 |
| AI-assisted | AI 只出现在左导航入口、顶栏图标、Activity 的 AI 事件三类位置 |
| Permission-aware | 无权限时整页降级，左导航相关入口同时置为不可用 |

| 补充纪律 | 说明 |
| --- | --- |
| **D-a 颜色只承载紧急度，不承载类型** | Human Actions 的 5 类（Decision / Approval / Review / Needs Input / Attention）统一用中性 chip；颜色只给左侧 2px 紧急度轨。避免彩虹化 |
| **D-b 实心 = 已成立的正式事实，描边 = 尚待人工确认的 AI 产出** | 见 §5.2。这是「AI 的建议不得表现为已获人类确认」的视觉保证 |
| **D-c 失败不等于空** | 任何区域加载失败时，该区域显示错误块 + Retry，对应指标显示 `—` + `Unavailable`；绝不用空态掩盖失败 |

---

## 2. 页面骨架与「提示词元素 → 壳层级」映射

提示词 §三 描述的页面结构，与已落地的应用壳（Figma「03 — Application Shell」）并非同一层级。本稿采用**壳 + 页面**两层结构，提示词要求的每个元素都有确定归属：

| 提示词要求的位置 | 提示词元素 | 本稿实际归属 | 说明 |
| --- | --- | --- | --- |
| 左侧导航栏 | Logo `smark-pm` | **壳 TopBar**（左：品牌标记 + 产品名） | 左导航顶部不做 Logo，避免与顶栏双标识 |
| 左侧导航栏 | My Home（选中） | `shell/SideNav` 第 1 项 | 激活态 = accent 着色面 + 2×16 accent 指示条 |
| 左侧导航栏 | My Inbox / My Projects | `shell/SideNav` 第 2/3 项 | Inbox 带计数徽标 |
| 左侧导航栏 | 分隔线 | `shell/SideNav` 组间 1px 分隔线 | — |
| 左侧导航栏 | 全局 AI Assistant 入口 | `shell/SideNav` 第 4 项（sparkles + accent 图标） | **偏差 D-1** |
| 左侧导航栏 | 用户头像、个人设置入口 | `shell/SideNav` 底部（头像 + 姓名/角色 + 设置图标钮） | — |
| 页面顶部 | 标题 Good morning, [User] | 页面标题区 `shell/PageHeader` | 21px semibold |
| 页面顶部 | 副标题 Here's what needs your attention today. | `shell/PageHeader` description 槽 | — |
| 页面顶部右侧 | 搜索入口 | **壳 TopBar** 全局搜索（360px，⌘K）；页面标题区另放一个 36px 搜索图标钮 | **偏差 D-2** |
| 页面顶部右侧 | Create Project 主操作按钮 | 页面标题区 actions 槽，`Button variant=primary` | 唯一主操作 |
| 页面顶部右侧 | 用户头像 | **壳 TopBar** 账户菜单 | **偏差 D-2** |
| — | （未在提示词中列出） | 壳右上下文栏 320px **本稿关闭** | 右栏是可选壳能力，首页不需要 |

### 2.1 偏差说明

- **D-1 · AI Assistant 入口位置**：提示词要求左导航有 AI Assistant 入口；壳顶栏已有一个 AI 助手按钮。为避免两个同权重入口争抢首页注意力，本稿把**顶栏改为无标签的 sparkles 图标钮**，左导航保留带标签的主入口。两者指向同一全局能力。
- **D-2 · 搜索与头像不重复**：提示词同时在「左侧导航」和「页面顶部右侧」两处要求头像。头像由壳 TopBar 承担，页面标题区不再重复；搜索入口由 TopBar 承担，页面标题区保留一个 36px 图标钮作为页内快捷入口（聚焦 ⌘K）。
- **D-3 · 顶栏项目上下文**：个人主页是跨项目上下文，顶栏项目位显示 `All projects`（次文字色、无 chevron、不可切换），而不是某一个项目。

---

## 3. 布局规格

以 1440px 画框为准（左导航 232 + 主区 1208）。

```
┌──────────────────────────────────────────────────────────────────────┐
│ TopBar 52px  [mark] smark-pm │ 星河集团 / All projects │ search ⌘K │ ♪ ✨ ? │ 头像 │
├───────────────┬──────────────────────────────────────────────────────┤
│ SideNav 232   │ main (canvas)                                        │
│  PERSONAL     │  PageHeader  标题 21px + 副标题 13px   [🔍][+ Create]│
│  ▸ My Home    │  ┌────────────────────────────────────────────────┐  │
│    My Inbox ④ │  │ My Overview  [4][12][4]  三指标横向条带           │  │
│    My Projects│  └────────────────────────────────────────────────┘  │
│  ─────────    │  ┌──────────────────────────┐ ┌────────────────────┐  │
│  ✨ AI Assistant│ │ My Inbox   60%          │ │ My Projects  40%   │  │
│               │ │  Human Actions           │ │  Active × 4        │  │
│               │ │  Assigned Tasks          │ │  ▸ Archived (折叠)  │  │
│  (头像) ⚙     │ └──────────────────────────┘ └────────────────────┘  │
├───────────────┴──────────────────────────────────────────────────────┤
│ CommandBar 72px  ✨ Ask AI about your work…            ⌘J 📎 ↑     │
└──────────────────────────────────────────────────────────────────────┘
```

| 尺寸 | 值 | token |
| --- | --- | --- |
| 左导航 | 232px（折叠 60px） | `--ws-sidebar-width` |
| 顶栏 | 52px | `--ws-topbar-height` |
| 主区内边距 | 20px 上 / 32px 左右 | `--space-5` / `--space-8`（**V1.1 修订**：随 2026-11-05 用户裁决并入全产品统一页框 PageFrame，原 24px 口径作废） |
| 区块间距 | 16px | `--space-4` |
| Row2 栅格 | `minmax(0,1.6fr) minmax(0,1fr)`，gap 16px | ≈ 60 / 40 |
| 卡片 | 8px 圆角、1px 边框、surface 面 | `--ws-radius-card` |
| 正文 | 13px / 19px | `--font-size-body` |
| 分区标题 | 11px medium + `tracking .025em` + 大写 | `--tracking-caption` |
| 卡片标题 | 15px semibold | `--ws-font-size-section` |
| 指标数值 | 20px semibold + tabular-nums | `--font-size-h3` |
| 行高 | 导航 36px / 控件 36px | `--ws-row-height` / `--size-control-md` |

**自适应**：≤1120px 时 Row2 折为单列、Activity 四列折两列；≤860px 时 Overview 折为纵向、Activity 折一列；≤700px 时隐藏左导航（保留顶栏与页内导航入口）。

---

## 4. 区域规格

### 4.1 My Overview（三指标横条）

单张 surface 卡片，内部三等分，列间 1px 分隔线（不是三个独立卡片——避免「卡片过多」）。

| 指标 | 语义 | 副标题（hint） | 点击行为 |
| --- | --- | --- | --- |
| Needs Attention | warning 点 | `2 overdue · 1 due today` | 跳 Inbox 的 Human Actions 分区 |
| Assigned to Me | info 点 | `3 due this week` | 跳 Inbox 的 Assigned Tasks 分区 |
| Active Projects | accent 点 | `2 need your input` | 跳 My Projects 分组 |

规则：
- 数字用 `font-variant-numeric: tabular-nums`，20px semibold；标签 12px 主文字；hint 11px 次文字。
- **不画图表、不画进度条、不画趋势线**。数字只用于概览。
- 加载失败时对应指标显示 `—` + `Unavailable`（不是 0）。
- 指标块是 `<button>`，键盘可达，焦点环 2px accent。

### 4.2 My Inbox Preview（首页最重要的区域）

单卡内两个分区，**Human Actions 视觉优先级高于 Assigned Tasks**：

| 维度 | Human Actions | Assigned Tasks |
| --- | --- | --- |
| 行结构 | 两行（标题行 + 元信息行） | 两行但更紧凑、无紧急度轨 |
| 颜色 | 左侧 2px 紧急度轨 | 无轨，优先级用等宽大写标签 |
| 类型标记 | 中性 chip（Decision / Approval / Review / Needs Input / Attention） | 状态 chip（In Progress / Backlog / …） |
| 右上 | 截止信息（逾期 / 今日 / 日期 / 无期限） | 截止信息 |

每条事项必须包含（缺一不可）：
1. **类型 chip**
2. **事项标题**
3. **来源项目**（必显）
4. **原始业务对象入口**（`Requirement REQ-102 ›` 可点，进原对象）
5. **当前待办态**（待决策 / 待审批 / 待验收 / AI 请求补充）
6. **截止信息**（无期限显式写 `No due date`，不留空、不虚构）

**Human Actions 的 5 个类型 chip 全部中性色**；紧急度只由左侧轨承担：

| 紧急度 | 轨色 | 右上文案 | 对应排序档位 |
| --- | --- | --- | --- |
| 已逾期 | `--ws-color-danger` | `2d overdue`（danger 文字） | 档位 1 |
| 临近截止（≤3 天） | `--ws-color-warning` | `Due today`（warning 文字） | 档位 2–3 |
| 普通待处理 | 无轨 | `Nov 7`（次文字） | 档位 4 |
| 无明确期限 | 无轨 | `No due date`（次文字） | 档位 5 |

计数口径：`My Inbox` 卡标题徽标 = 两个分区之和（示例 16）；左导航 `My Inbox` 徽标 = **待人工处理条数**（示例 4），不含普通任务——避免导航徽标把普通待办也渲染成"要人处理"。

### 4.3 My Projects

| 字段 | 展示 | 来源 |
| --- | --- | --- |
| 项目名称 | 必显 | — |
| 项目角色 | 必显（Owner / Lead / Member 描边小 chip） | 当前用户在该项目的成员关系 |
| 项目状态 | 必显（Active=success / Planning=info / Archived=neutral） | — |
| 简短描述 | 必显，单行省略 | — |
| 我的待办数 | 必显（`3 pending`） | 聚合读模型 |
| 最近活动时间 | 必显（`updated 2h ago`） | — |
| 关注指示 | 有真实逾期货时才出现 6px warning 点 | 后端给出的事实，不推断 |

- 分组：**Active Projects**（展开）+ **Archived Projects**（**默认折叠**，chevron 可展开）。
- 卡头右侧：View All（文字链）+ 筛选图标钮。
- 不展示任务总数 / 需求数 / Agent 数——那是项目总控台的职责。

### 4.4 Recent Activity（横向列表，4 列）

每条动态三行：

```
[操作者标记] [操作者名] [事件类型 chip]              [时间]
活动摘要（最多 2 行）
项目 · 业务对象入口 ›
```

**操作者标记**：Human = 姓名首字母头像；AI Agent = accent 着色面 + bot 图标；System = elevated 面 + 次文字色图标。

---

## 5. 视觉规则

### 5.1 颜色分工

| 通道 | 承载 | 不承载 |
| --- | --- | --- |
| 左侧 2px 轨 | 紧急度 | 类型、状态 |
| chip 填充 | 事实是否成立（实心=成立 / 描边=待确认） | 紧急度 |
| 中性 chip 底色 | 类型 | — |
| accent `#8b83f6` | AI 产出、激活导航、主操作、Active Projects 指标点 | 人类确认结论（人类确认用 success） |

**铁律（承接 AGENTS.md）**：AI 建议用 accent，人工确认提示用次文字色/语义色；人类确认的结论绝不使用 accent。

### 5.2 四类活动的视觉区分（对应提示词 §三.6）

| 事件 | 芯片样式 | 图标 | 颜色 | 文案示例 |
| --- | --- | --- | --- | --- |
| 人类确认 | **实心**（16% success 底） | `circle-check` | success | `Confirmed` |
| 系统正式提交 | **实心**（16% info 底） | `send` | info | `Submitted` |
| AI 提交提案 | **描边**（透明底 + accent 描边） | `sparkles` | accent | `Proposal` |
| AI 执行完成 | **描边**（透明底 + accent 描边） | `sparkles` | accent | `AI Completed` |

判定规则：
- **AI 事件一律不用对勾**。`circle-check` 只允许出现在「人类确认」上。
- AI 事件必须带 `AI` 语义前缀（操作者名为 Agent 名 + 芯片文案含 `AI`/`Proposal`）。
- AI 事件摘要用「等待人类确认 / 尚未经人工验收 / 尚未成为正式需求」等措辞，不得写成已完成事实。
- 实心 vs 描边是这套区分的**主通道**，颜色是次通道——即使灰度打印也能分辨。

---

## 6. 排序规则（Human Actions）

沿用 V1.1 §4.5，并给出可实现的排序键：

| 档位 | 判定 | 视觉 |
| --- | --- | --- |
| 1 | 已逾期（后端给出 `overdue`，不由前端推断） | danger 轨 + `Nd overdue` |
| 2 | 高紧急程度且仍待处理 | warning 轨 |
| 3 | 临近截止（≤3 天） | warning 轨 + `Due today` / 日期 |
| 4 | 普通待处理 | 无轨 + 日期 |
| 5 | 无明确期限 | 无轨 + `No due date` |

同档内：具有明确人工阻塞影响者优先，其次按截止时间升序，再次按创建时间升序。

**禁止**：
- 不得凭 AI 推断逾期；不得把「最近更新」当紧急度。
- 不得在没有截止时间时虚构期限。
- 不得因事项来自 AI 就自动提权。

---

## 7. 交互清单

| # | 触发 | 目标 |
| --- | --- | --- |
| I-1 | 点击左导航 `My Home` | 回到本页 |
| I-2 | 点击 `My Inbox` | 进独立 Inbox 页面（`/inbox`） |
| I-3 | 点击 Inbox 卡头 `View All` | 进 Inbox 完整列表（带筛选） |
| I-4 | 点击任一 Human Action 行 | 跳原始业务对象（需求 / 审批 / 验收 / 任务） |
| I-5 | 点击任一 Assigned Task 行 | 跳任务详情 |
| I-6 | 点击项目卡 | 进项目总控台 |
| I-7 | 点击 `Archived Projects` 折叠头 | 展开 / 折叠归档分组 |
| I-8 | 点击 `Create Project` | **复用 S01-02 项目创建流程**（不在本页实现表单） |
| I-9 | 点击 `AI Assistant`（左导航 / 顶栏 / 指令栏） | 同一全局 AI 能力 |
| I-10 | 点击 Overview 指标 | 下钻到对应分区（人类事项 / 分配任务 / 项目列表） |
| I-11 | 点击 Activity 的对象入口 | 跳原始业务对象 |
| I-12 | 点击搜索图标钮 / ⌘K | 全局搜索面板 |
| I-13 | 错误态 `Retry` | 重试该区域请求（只重试该区域） |

键盘：所有可点行与指标为原生 `<button>`，焦点环 2px accent（`--ws-focus-ring`）。

---

## 8. 页面状态矩阵

原型内置切换条可逐个查看。

| # | 状态 | 触发条件 | 页面表现 | 明确禁止 |
| --- | --- | --- | --- | --- |
| S-1 | 正常 | 有数据 | Overview 3 指标 + Inbox 两分区 + Projects 4 卡 + Activity 4 条 | — |
| S-2 | 首次登录 · 无任何项目 | 项目集合为空 | Overview / Inbox / Activity 整块隐藏，只留居中引导面板（`package` 图标 + 标题 + 说明 + 单一 `Create Project`） | 不得用虚构任务/动态/计数填充 |
| S-3 | 没有待处理事项 | Inbox 为空 | Overview 计数归零 + 解释性 hint（`Nothing waiting on you`）；Inbox 卡片内显示平静空态「All caught up」（success 圆环，非告警）；Projects 与 Activity 保持可用 | 不得用错误/警告制造紧张感 |
| S-4 | 项目加载失败 | Projects 请求失败 | **仅** Projects 区域变错误块（`triangle-alert` + 标题 + 说明 + `Retry` + 错误码）；Active Projects 指标显示 `—` + `Unavailable`；其余区域照常 | 不得让整页塌陷；不得渲染成空列表 |
| S-5 | Inbox 加载失败 | Inbox 请求失败 | **仅** Inbox 区域变错误块；Needs Attention / Assigned to Me 指标显示 `—` | 不得把 API 失败渲染成「没有待办」 |
| S-6 | 没有访问权限 | 当前用户不属于任何项目 | 整页降级为权限态面板（`shield` + 原因说明 + 两条真实可执行动作），并声明全局 AI Assistant 与设置仍可用；左导航 `My Inbox` / `My Projects` 同时置为不可用 | 不得展示任何受限数据；不得显示假的 0 值 |
| S-7 | 正在加载 | 请求在途 | 骨架屏**按真实布局形状**占位（指标条 / 事项两行 / 任务行 / 项目卡 / 活动卡）；主操作按钮禁用但保留可见 | 不得出现布局跳动 |

补充规则：
- **S-2 与 S-6 必须区分**：前者是「还没有项目」，后者是「有项目但你没权限」。二者的标题、图标、说明、动作都不同。
- S-2 / S-6 的副标题不得写「Here's what needs your attention today.」——那是在承诺页面给不出的内容。本稿改为「Your personal home is ready — create a project to get started.」/「Your projects, inbox and activity will appear here once you have access.」
- 骨架屏 shimmer 1600ms；`prefers-reduced-motion` 下降级为静态。

---

## 9. 组件拆分建议

### 9.1 直接复用（`ai-frontend/src/components/`）

| 组件 | 用在哪 | 备注 |
| --- | --- | --- |
| `shell/WorkspaceShell` | 整页 | 右上下文栏关闭 |
| `shell/TopBar` | 顶栏 | 项目位改 `All projects` |
| `shell/SideNav` | 左导航 | 只渲染 personal 分组 + AI Assistant + 页脚 |
| `shell/PageHeader` | 标题区 | actions 槽放搜索钮 + Create Project |
| `shell/CommandBar` | 页底 | 收起态 72px |
| `ui/card` | 四个区块容器 | — |
| `ui/badge` | 类型 / 状态 chip | ⚠ success / info / warning / danger 四变体在 Figma 库中**未定义**（FIGMA-SYNC 台账标注为本地延伸），需确认是否补 Figma 变体 |
| `ui/avatar` | 操作者标记、用户头像 | — |
| `ui/button` / `ui/icon-button` | 主操作 / 图标钮 | icon-button 的 `label` 必填 |
| `ui/empty-state` | S-2 / S-3 | 支持 title / description / actionLabel / secondaryActionLabel |
| `ui/alert` | S-4 / S-5 错误块 | `variant="danger"` + 内部 Retry 按钮 |
| `ui/skeleton` | S-7 | 双色：strong=标题、base=正文 |
| `ui/separator` | 分隔线 | — |
| `ui/human-approval-indicator` | Activity「Confirmed」 | `state="approved"` |
| `ui/ai-status-indicator` | AI Agent 事件 | `status="done"` |
| `ui/tooltip` / `shell/IconTooltip` | 图标钮提示 | 壳内用 IconTooltip（asChild） |
| `ui/list-item` | Assigned Tasks 行 | 若不需 2px 轨，可直接复用 |

### 9.2 需新增（先落业务页私有目录，确认复用后再提到 `ui/`）

| 组件 | 职责 | 关键 prop |
| --- | --- | --- |
| `personal-home/OverviewBar.vue` | 三指标横条 | `metrics[]` |
| `personal-home/MetricTile.vue` | 单指标块（含 urgent 语义点、hint、下钻事件） | `label / value / tone / hint / na` |
| `personal-home/InboxActionItem.vue` | Human Action 行（2px 紧急度轨 + 中性类型 chip + 对象入口 + 截止） | `type / title / project / objectRef / dueState / dueLabel` |
| `personal-home/AssignedTaskItem.vue` | 分配任务行（等宽优先级 + 状态 chip） | `priority / title / project / taskCode / status / due` |
| `personal-home/ProjectCard.vue` | 项目卡（名/角色/状态/描述/待办/活跃时间/关注点） | `name / role / status / description / pending / lastActivity / attention` |
| `personal-home/ProjectsGroup.vue` | 可折叠分组（Active / Archived） | `title / count / collapsed` |
| `personal-home/ActivityEntry.vue` | 动态条目（操作者标记 + 事实/待确认芯片 + 摘要 + 对象入口） | `actor.kind / eventKind / actorName / summary / occurredAt / objectRef` |
| `personal-home/SectionState.vue` | 统一空 / 错误 / 无权限容器 | `variant: empty\|error\|forbidden` + `title / description / action` |

候选提到 `ui/` 的通用件：`ui/urgent-rail`（2px 紧急度轨）、`ui/event-chip`（事实/待确认双态芯片）。前者若在 Inbox 以外也复用则值得提级。

### 9.3 明确不用的现有组件

- `ui/notification-item`：**不得**用于 Inbox。通知 ≠ 待办；首页只承载「需要人行动」的事项与「分派给本人的任务」。
- `ui/suggestion`：首页不展示 AI 建议卡（AI 产出只在 Activity 中以事件形式出现）。
- `ui/command-input` / `ui/command`：仅属壳级指令栏与 ⌘K 面板，首页不嵌入。

---

## 10. i18n 键建议

新增命名空间 `pm.personalHome.*`（业务页）与 `shell.app.side.item.personalHome`（导航改名）。

```yaml
pm:
  personalHome:
    greeting:
      morning: "Good morning, {name}"
      afternoon: "Good afternoon, {name}"
      evening: "Good evening, {name}"
    subtitle:
      default: "Here's what needs your attention today."
      firstLogin: "Your personal home is ready — create a project to get started."
      noPermission: "Your projects, inbox and activity will appear here once you have access."
    overview:
      needsAttention: "Needs Attention"
      assignedToMe: "Assigned to Me"
      activeProjects: "Active Projects"
      unavailable: "Unavailable"
    inbox:
      title: "My Inbox"
      viewAll: "View All"
      humanActions: "Human Actions"
      humanActionsHint: "需要人工决策或介入"
      assignedTasks: "Assigned Tasks"
      assignedTasksHint: "分配给你的普通任务"
      types:
        decision: "Decision"
        approval: "Approval"
        review: "Review"
        needsInput: "Needs Input"
        attention: "Attention"
      due:
        overdue: "{n}d overdue"
        dueToday: "Due today"
        noDueDate: "No due date"
      empty:
        title: "All caught up"
        description: "Nothing needs your attention right now. New human actions will show up here."
    projects:
      title: "My Projects"
      active: "Active Projects"
      archived: "Archived Projects"
      viewAll: "View All"
      filter: "Filter projects"
      pending: "{n} pending"
    activity:
      title: "Recent Activity"
      viewAll: "View All"
      events:
        confirmed: "Confirmed"
        proposal: "Proposal"
        aiCompleted: "AI Completed"
        submitted: "Submitted"
    state:
      firstLogin:
        title: "You don't have any projects yet"
        description: "Create your first project to start organizing your development work."
      error:
        inboxTitle: "Couldn't load your inbox"
        inboxDescription: "The inbox service didn't respond. Your projects and activity are unaffected."
        projectsTitle: "Couldn't load projects"
        projectsDescription: "We couldn't reach the project service. Your inbox is still up to date."
        retry: "Retry"
      forbidden:
        title: "You don't have access to any projects"
        description: "Your account isn't a member of any project in {tenant}. Ask a project owner to invite you, or create a project of your own."
    action:
      createProject: "Create Project"
      askForInvite: "Ask for an invite"
```

需要同时处理的既有键：

| 现状键 | 处理 |
| --- | --- |
| `shell.app.side.item.personalWorkspace`（"Personal Workspace" / "个人工作区"） | 改为 `shell.app.side.item.personalHome` = "My Home" / "我的主页"；旧键与 `global.search.commands.openPersonalWorkspace` 一并收编 |
| `global.search.commands.openPersonalWorkspace` | 改为 `openPersonalHome` |
| `shell.status.backHome`（"Back to overview"） | 复用，但语义改为回 My Home |

---

## 11. 待裁决与需接口核验项

### 11.1 待用户裁决

| # | 议题 | 现状 | 本稿倾向 |
| --- | --- | --- | --- |
| Q1 | V1.1 §1.3 的产品结构图含 "Project Workspace" 分支，与「不引入 Workspace 领域对象」冲突 | 未裁决 | 确认 V1.1 §1.3 结构图按本稿 §0.1 修订（改为「项目模块」，非领域实体） |
| Q2 | 导航项改名 `Personal Workspace` → `My Home` | 未裁决 | 同意改名，否则与本稿及提示词长期冲突 |
| Q3 | Watchlist（V1.1 §6）去向 | V1.1 列为 P1「可根据屏幕空间显示」 | 本稿首页不显示；建议移入 Inbox 页的独立分区或后续切片，避免首页四区变五区 |
| Q4 | 左导航徽标口径 | 未定义 | 本稿取「待人工处理条数」；若产品要求显示 Inbox 总数则改 |
| Q5 | 首页是否常驻壳级 CommandBar | 壳当前对所有页常驻 | 本稿保留收起态（72px）；如认为抢戏可让 Personal Home 例外隐藏（属壳级改动，需另行裁决） |

### 11.2 实施前必须核验的代码事实（S01-05 §15 第 2 步）

| # | 断言 | 需要核验 |
| --- | --- | --- |
| F-1 | Home 是跨项目聚合读模型 | 现有 `src/api/command-center.ts` 是**项目级**（`getCommandCenter(projectId)`），**不存在跨项目个人聚合接口** → 需新建个人工作台读模型或按项目 N+1 聚合 |
| F-2 | Human Actions 的五类来源 | 决策 / 审批 / 验收 / 补充信息 / 异常，各自的真实来源表与状态字段（`pm.ts` 有 `listApprovals`/`listRequirements`/`listTasks`，`review.ts` 有 `listReviewApprovals`/`decideReviewApproval`） |
| F-3 | 逾期字段 | 是否存在后端计算的 `overdue` 布尔/天数；若无，需补字段（**不得前端推断**） |
| F-4 | 任务负责人与优先级 | `tasks` mock 有 `assignee_id` / `priority`，真实表是否同构；「分配给我」的判定需含有效分配关系 |
| F-5 | 项目成员关系与角色 | 项目内是否有显式「Owner/Lead/Member」角色字段；缺则需新增（AGENTS.md 记录项目级显式授权当前不存在） |
| F-6 | Activity 的 actor.kind | `ActivityItem.actor.kind` 已有 Human / AI Agent / System 三分类，可直接用 |
| F-7 | `eventKind`（Confirmed / Proposal / AI Completed / Submitted） | 现有 `activity_type` 枚举能否映射到四类；**不能映射则不得用颜色/芯片伪造** |
| F-8 | 权限边界 | 个人聚合不得绕过项目权限；退出项目后聚合需实时剔除 |

---

## 12. 下一步

1. 用户评审本稿（视觉 + 规格 + Q1~Q5）。
2. 执行 §11.2 的 F-1~F-8 代码事实与接口核验，产出核验报告。
3. 按核验结果修订本稿 V1.1（尤其 Inbox 数据来源与 Activity 事件分类）。
4. UX Review：与 S01-02（创建）、S01-03（仓库）、S01-I5（总控台）衔接检查。
5. 冻结后进入前端实施：按 §9.2 拆组件，落 `ai-frontend/src/views/personal/`。

---

## 13. 实施落地记录（2026-11-04）

> §12 第 5 步已执行：页面已落地为真实前端页，登录后默认进入。命名裁决（§11 Q1/Q2）已由用户收编，导航项定名 My Home。

### 13.1 落地文件

| 文件 | 说明 |
| --- | --- |
| `ai-frontend/src/api/personal-home.ts` | 跨项目聚合适配层（读模型消费端，按区块拆分加载） |
| `ai-frontend/src/views/personal/PersonalHomeView.vue` | 页面主体（Overview + Inbox/Projects + Activity） |
| `ai-frontend/src/views/personal/components/` | `MetricTile` / `HumanActionItem` / `AssignedTaskItem` / `ProjectCard` / `ActivityEntry` / `SectionState` 六个页内私有组件 |
| `ai-frontend/src/views/personal/useRelativeTime.ts` | 相对时间格式化 |
| `ai-frontend/src/views/personal/PersonalHomeView.test.ts` | 6 条行为用例（分区呈现 / 区块失败隔离 / 403 / 首次登录 / 空待办） |
| `ai-frontend/src/api/mock/personal.ts` | dev mock：`/v1/pm/me`、`/v1/pm/decisions`、`/v1/pm/projects/{id}/members` |
| `ai-frontend/src/components/ui/badge/Badge.vue` | 新增本地延伸变体 `outline`（透明底 + 同色描边，承载「AI 产出待确认」） |
| `ai-frontend/src/router/navigation.ts` + `index.ts` | 导航项改 `My Home`（Home 图标）+ `NavItem.component` 懒加载登记（保持导航树单源） |
| `ai-frontend/src/views/auth/AuthLoadingView.vue`、`UnauthorizedView.vue` | 登录/403 落地改 `/personal` |
| `ai-frontend/locales/*/pm.yaml` | `pm.personalHome.*` 全量文案 + 48 条审计动作中文映射 |
| `ai-frontend/locales/*/shell.yaml`、`global.yaml` | `shell.app.side.item.personalHome`、`global.search.commands.openPersonalHome` |

### 13.2 数据来源映射（区块 ← 端点）

| 区块 | 数据源 | 口径 |
| --- | --- | --- |
| 当前主体 | `GET /v1/pm/me` | `principal_id` 即 `core.principal.id`，与 `task.assignee_id` 同一命名空间 |
| 我的项目 | `GET /v1/pm/projects` + `GET /v1/pm/projects/{id}/members` | 后端已按 `core.project_member` 过滤；角色取成员行 `role`（0030 白名单 owner/member/ai_pm/architect/developer/tester） |
| Human Actions | `GET /v1/pm/decisions`（pending）+ 我的 `awaiting_decision/submitted/reviewing/blocked` 任务 | 只有真正需要人决策的条目 |
| Assigned Tasks | `GET /v1/pm/tasks` 中 `assignee_id = 当前主体` 且非终态、非人工事项 | 与人工事项不重复计数 |
| Recent Activity | `GET /v1/pm/audit-logs` + `GET /v1/pm/principals`（解析 actor kind/显示名） | 事件分类按 §5.2；未知动作降级 generic 不伪造 |

三个区块**各自独立加载**：Inbox 失败不拖垮 Projects；失败时对应指标显示 `—` + `Unavailable`（失败 ≠ 空）。

### 13.3 与设计稿的偏差（实现时确认）

| # | 设计稿 | 实现 | 原因 |
| --- | --- | --- | --- |
| D-1 | 左导航带标签 AI Assistant 入口 | **未改壳**：AI 入口仍在顶栏 | 并行会话正在改 `shell/TopBar.vue`，避免同文件冲突 |
| D-2 | 页标题区搜索图标钮 | 已实现（→ 派发 ⌘K 开全局搜索面板） | — |
| D-3 | 顶栏项目位显示 `All projects` | **未改壳** | 同 D-1 |
| E-1 | 截止日期 / 逾期轨 | 一律「无期限」 | `delivery.task` / `governance.approval_request` **当前无 due 列**（F-3 证实）；适配层已预留 `DueInfo`，后端补字段自动生效 |
| E-2 | 事实类=实心语义 chip | 退化为「语义色文字 + 图标」；AI 类=accent 描边 chip | token 层**无** success/info 语义填充面（仅 accent/danger 有 surface）；按「颜色只能来自 tokens.css」不得自造中间色 |
| E-3 | Create Project 复用 S01-02 流程 | 暂落 `/projects` | S01-02 创建流程尚未实现（页面层占位）；就位后改指 |
| E-4 | 右上下文栏关闭 | 保持壳默认（开） | 壳级默认行为，跨页影响，不在本页改 |
| E-5 | 根路径 `/` 落到个人主页 | **未加** `/` 重定向 | 初始导航=`/` 的既有测试会与后续 push 竞态；登录落地由 AuthLoadingView 承担，直接访问 `/` 维持 404（既有行为） |
| E-6 | §9.2 页内私有组件（InboxActionItem 带 2px 紧急度轨等自绘样式） | **重构为公共组件组合**（2026-11-05 用户裁决「优先公共组件与 /design，不自造」）：行=`ListItem`、卡=`Card`、计数=`Badge count`、次级链接=`Button ghost`、语义点/关注指示=`StatusIndicator`、折叠行=`ListItem`+`aria-expanded` 透传；页内只剩纯布局样式 | 自绘行组件是裸 `<button>` 吃到浏览器默认 buttonface 灰底（两主题全灰的违和根因）；公共组件自带正确底色/焦点/hover |

### 13.4 门禁与验证结果

| 项 | 结果 |
| --- | --- |
| `pnpm typecheck` / `pnpm lint` | 通过 |
| `pnpm test:unit` | 107 文件 / 651 用例全绿（含本页 6 条） |
| 三道门禁（tokens / i18n 硬编码 / i18n 命名空间） | 全部通过（中英 2399 key 对齐） |
| 浏览器实测 | dev server（mock）`/personal` 渲染正确：三指标、Inbox 双分区、项目三卡 + 归档折叠、Activity 四类事件可辨 |

### 13.5 后续

1. 后端补**个人工作台读模型**（F-1）→ `personal-home.ts` 整体替换为一次 `GET /v1/pm/personal-home`。
2. 后端补**截止/逾期字段**（F-3）→ 紧急度轨与排序档位自动生效。
3. S01-02 创建流程落地 → Create Project 直连创建视图（E-3）。
4. 壳级裁决：AI 入口归属、顶栏项目位、右栏默认态（D-1 / D-3 / E-4）随壳改动一并定。

---

## 附录 A · 原型使用说明

`assets/s01-05-personal-home-prototype.html` 为自包含文件，直接用浏览器打开即可：

- 顶部切换条切换 7 种页面状态（正常 / 首次登录 / 无待办 / Inbox 失败 / 项目失败 / 无权限 / 加载中）。
- 主题切换：Auto（跟随系统）/ Light / Dark。
- `Archived Projects` 分组可点击展开 / 折叠。
- 数值与文案为**示意数据**，取自 `src/api/mock/fixtures.ts` 的项目 / 需求 / 任务命名（星河客服中台、云帆数据门户、REQ-101…、TASK-207…），仅用于呈现密度与层级，**不代表已确认的接口**。

## 附录 B · 颜色 token 对照

| 用途 | token | 深色 | 浅色 |
| --- | --- | --- | --- |
| 画布 | `--ws-canvas` | `#111318` | `#f7f8fa` |
| 侧栏 / 顶栏 | `--ws-sidebar` | `#171a21` | `#eef0f5` |
| 卡片面 | `--ws-surface` | `#1d212a` | `#ffffff` |
| 抬升面 | `--ws-elevated` | `#252a35` | `#e8ebf2` |
| 边框 | `--ws-border` | `#353b48` | `#c7cdd9` |
| 主文字 | `--ws-text-primary` | `#e9ecf3` | `#202532` |
| 次文字 | `--ws-text-secondary` | `#99a2b2` | `#586274` |
| accent（AI / 激活 / 主操作） | `--ws-accent` | `#8b83f6` | `#6257ce` |
| accent 面 | `--ws-color-accent-surface` | `#242238` | `#eeebff` |
| success（人类确认 / Active） | `--ws-color-success` | `#34c08b` | `#22a06b` |
| info（系统提交 / Planning） | `--ws-color-info` | `#6ea8fe` | `#3b82f6` |
| warning（临近截止 / 待办） | `--ws-color-warning` | `#e0b15e` | `#b45309` |
| danger（逾期 / 错误） | `--ws-color-danger` | `#f07e88` | `#e16b73` |
