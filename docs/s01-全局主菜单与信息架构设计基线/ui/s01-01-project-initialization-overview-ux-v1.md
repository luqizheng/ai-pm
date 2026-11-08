# S01-01 · Project Initialization Overview（项目初始化总览）UX 详细设计稿 V1.0

> **文档编号**：S01-01（项目初始化总览）
> **所属阶段**：S01 — Implementation Planning & UX Design
> **设计对象**：smark-pm 项目级「初始化总览」页面（Project Initialization Overview）
> **文档性质**：页面 UX 详细设计稿（信息架构 + 状态模型 + 交互规则 + 验收标准）
> **版本**：V1.0
> **日期**：2026-11-04
> **状态**：待评审
> **视觉真源**：《AI R&D Workspace — 高保真设计系统 V1.0》→ `ai-frontend/src/styles/tokens.css`（`--ws-*`）
> **上位依据**：`smark-pm-s01-final-baseline-v1-2.md`（S01 最终基线 V1.2）、`smark-pm-s01-i5-project-command-center-v1-1.md`（I5 总控台 V1.1 定稿）、`smark-pm-user-scenario-interaction-model-v1.md`（用户场景交互模型）

---

## §0 编者按：本稿定位、事实口径与两处前置冲突

> **2026-11-05 修订登记**：五份专家修订稿已到达并记录（Command Center V1.1 / S01-02 V1.1 / S01-03 V1.2 / S01-04 V1.2 / S01-05 V1.1，索引见本目录 README）。与本稿直接相关：① 裁决 **R-S01-01-01 方案 A** 已收编本稿（§1.3 G-9、§9.5、I5 §4.7）；② Command Center 修订稿 §九 确认 **S01-01=Initialization Overview**，且初始化总览=「Command Center 内模块 + 完整详情页」双形态（本稿 §2 定位将随之修订，待基线确认）；③ 本稿 §14.1 缺口表（G-01~G-23）已并入 [统一核验清单](s01-technical-verification-unified-checklist-v1.md)，后续核验以清单为准；④ 本稿 §14.3 已登记零依赖/等待态实测缺口。

### 0.1 本稿做什么

本稿定义 **S01 场景内、Project 创建之后、正式交接之前** 的项目级初始化总览视图。它回答六个问题：

1. 项目初始化进行到了哪一步？
2. 哪些工作已经完成？
3. AI Agent 当前正在执行什么？
4. 哪些工作需要人类确认或介入？
5. 当前是否存在阻塞？
6. 下一步应该执行什么？

并给出与 S01-02（项目创建）、S01-03（仓库管理）、S01-04（AI-PM 认知）的边界，以及完成初始化后如何进入正常研发管理。

### 0.2 事实标记法

沿用 S01 V1.3 的四级事实标记，正文逐条标注：

| 标记 | 含义 |
| --- | --- |
| **【EXISTING】** | 代码/数据库中已存在并已验证（含 `file:line`） |
| **【VERIFY】** | 代码中存在但语义/口径需进一步核验 |
| **【NEW】** | 本稿提出的**尚未实现**的设计候选，需另立实施任务 |
| **【DECIDED】** | 已有用户正式裁决，不得在本稿重新裁决 |

### 0.3 冲突一：本稿与既有 S01-01 编号撞号

**事实**【EXISTING】：`docs/s01-全局主菜单与信息架构设计基线/ui/` 目录已有 `s01-01-project-baseline-confirmation-ux-v1.md`（项目基线确认 UX 规范 V1.0，状态=待评审）；同目录 `README.md:25-26` 的推进顺序表把 **「S01-01 项目初始化总览」** 与 **「S01-01 基线确认」** 并列为第 1、2 项——即仓库内已存在两个 S01-01。

**本稿处理**：本文件是 README 第 1 项「S01-01 项目初始化总览」的交付物，文件名 `s01-01-project-initialization-overview-ux-v1.md`；**不覆盖、不改名、不合并**既有基线确认稿。

**建议（待裁决，本稿不单方面执行）**：把序列统一为依赖顺序，消除双 S01-01：

| 编号 | 文档 | 现状 | 建议 |
| --- | --- | --- | --- |
| S01-01 | 项目初始化总览 | 本稿（新增） | 保持 |
| S01-02 | 项目创建 | `s01-02-...-v1-1.md` | 保持 |
| S01-03 | Repository Management | `s01-03-...-v1-1.md` | 保持 |
| S01-04 | AI-PM Cognition | `s01-04-...-v1-1.md` | 保持 |
| **S01-05** | **项目基线确认** | 现文件名 `s01-01-project-baseline-confirmation-ux-v1.md` | **改编号为 S01-05**（依赖顺序上位于认知之后、交接之前） |
| S01-06 | Codex / MCP Bootstrap | 未建 | 保持 |
| S01-07 | 初始化完成与交接 | 未建 | 保持（Stage 6 的成文形态） |
| **S01-08** | **Personal Workspace / Personal Home** | 现文件名 `s01-05-personal-*-ux-*.md` | **改编号为 S01-08**（个人工作台与项目初始化无依赖关系，排在最后） |

**影响面**：需改 `docs/s01-全局主菜单与信息架构设计基线/ui/README.md` 表格与推进顺序表 + 两个既有文件的文档编号行与文件重命名 + 交叉引用。**该动作影响其他会话已产出的稿件，须由用户裁决后统一执行。**

### 0.4 冲突二：与 I5 项目总控台（Command Center）的职责重叠

**事实**【EXISTING】：`GET /v1/pm/projects/{project_id}/command-center` 已实现（路由 `apps/pm-api/src/command_center.rs:37-40`，handler `:43`），返回六数据块，其中 `initialization_progress` 已包含 **7 个初始化事项**及其派生状态（`crates/pm-application/src/command_center.rs:630-762`）。

**事实**【DECIDED】：I5 V1.1 §4.1 已裁决「**初始化进度始终保留在总控台中**」；§4.2 已裁决「初始化**不能**被建模为强制线性状态机」；§4.8 已裁决「第一版**不展示无依据的初始化百分比**」。

**本稿处理（关键架构决策）**：

> **S01-01 不建立第二套初始化状态来源。**
> 本页的「Initialization Progress」是 I5 `initialization_progress` 投影的**阶段化展开视图**，二者必须由**同一份派生逻辑**产出。
> 本稿不新增初始化状态表、不新增初始化状态机、不新增独立后端端点。

由此产生三条硬约束：

- **C-1**：六阶段是**人类阅读分组与导航结构**，不是持久化状态机、不是强制线性流程、不是门禁序列。任何阶段都可以「不适用」「跳过」「乱序完成」（对齐 I5 §4.2）。
- **C-2**：阶段状态词表与 `command_center.rs:74-81` 的 `init_status` 常量**必须同源**；新增状态值必须同时改后端常量 + 前端 `InitItemStatus` 类型 + i18n 三处，不得单边修改。
- **C-3**：交接（Stage 6）**不得**表述为「初始化完成」——`command_center.rs:759` 的 `completion` 当前恒为 `"not_determinable"`，I5 §4.5 已明确此时页面只能展示「项目接入概况」。

### 0.5 本稿的硬性禁区（自检）

- [x] 未引入 Workspace / WorkspaceMember 等未经确认的领域模型
- [x] 未创造新的数据库实体、枚举或持久化状态
- [x] 未创造新的后端 API（V1 全部消费已注册端点；缺口集中列在 §14）
- [x] 未把 AI 推断表现为已确认事实（视觉通道见 §7.4）
- [x] 未允许 AI 代替人类完成正式确认（§8）
- [x] 未重复实现 S01-02 / S01-03 / S01-04 的功能（§12）
- [x] 未把初始化总览设计成长期主导航（§2.1）
- [x] 未用统一 Failed 状态掩盖不同业务原因（§10）
- [x] 未在页面内提供第二个 AI 聊天窗口（§7.3）

---

## §1 产品定位与设计目标

### 1.1 定位

Project Initialization Overview 是**项目创建成功后、正式交接前的项目级总控视图**。它的读者是**人类负责人**（Project Owner / 具备治理权限的成员），在任意时刻进入都能立即判断：

> 项目现在处于什么阶段、AI 正在做什么、哪些事情已经成为正式事实、哪些事情还需要我确认、下一步应该做什么。

它**不是**：新的 Workspace 领域实体；长期占据主导航的独立业务模块；AI 聊天页面；线性安装向导；把初始化任务混在一起的普通任务列表。

### 1.2 页面存在的判据（五问）

沿用用户场景交互模型的「页面存在的判据」：

| 五问 | 本页答案 |
| --- | --- |
| 为什么需要此页 | 六阶段分属四个领域（项目/仓库/发现/设备），没有一个页面能同时呈现它们的当前态、阻塞与待办 |
| 进入时业务状态 | Project 已创建（`core.project` 存在），初始化工作尚未交接 |
| 帮用户做什么判断 | 「我现在该做什么 / 谁该做 / 什么卡住了 / 能不能进总控台」 |
| 可采取什么行动 | 下钻阶段详情、进入确认流程、处理阻塞、恢复失败项、进入总控台 |
| 操作后业务状态如何变化 | 阶段状态与待办集合变化；**页面自身不产生业务状态**（除下钻与确认入口） |

### 1.3 设计目标

| 编号 | 目标 | 可度量落点 |
| --- | --- | --- |
| G-1 | 用户 5 秒内知道「现在最该做的一件事」 | Current Action 区块在首屏第一视觉位；决策规则确定且可复算（§5） |
| G-2 | 阶段状态只来自权威数据，不来自推测 | 每条阶段状态标注 `source_state` 与派生依据（§4.2） |
| G-3 | AI 产出与人类确认在视觉上不可混淆 | 实心/描边双通道（§7.4），全站无 `circle-check` 用于 AI 事件 |
| G-4 | 阻塞必须可解释、可解除 | 每条阻塞给出原因、影响范围、解除条件（§10.3） |
| G-5 | 待办只收「需要人做决定或授权」的事项 | 纳入/排除规则双清单（§6.1/§6.2） |
| G-6 | 初始化完成不得被简化成一个百分比或一句「完成」 | 无百分比；Stage 6 采用四态交接口径（§3.6） |
| G-7 | 已完成阶段保留历史，不因后续操作丢失 | 阶段卡片常驻历史入口；命令中心 `recent_activity` 不覆盖历史（§3.0.4） |
| G-8 | 与总控台共用同一份初始化派生 | 本页不建表、不建新状态机（§0.4） |
| G-9 | 无仓库、无设备、分析在跑，项目都照常可用 | 零依赖独立工作 + 分析等待非阻塞（§9.5；裁决依据 R-S01-01-01，2026-11-05） |

### 1.4 非目标（明确不做）

| 不做 | 理由 |
| --- | --- |
| 不做项目创建表单 | 归属 S01-02；本页 Stage 1 只回显创建结果 |
| 不做仓库接入/授权/核验操作界面 | 归属 S01-03；本页只做导航与状态呈现 |
| 不做认知逐项审核工作台 | 归属 S01-04；本页只做导航与状态呈现 |
| 不做基线提案审核与差异比对 | 归属基线确认规范（§0.3 编号冲突项）；本页只做导航与状态呈现 |
| 不做设备/执行环境管理台 | 归属 Codex/MCP Bootstrap 规范；本页只做状态与阻塞摘要 |
| 不做全项目百分比进度 | I5 §4.8 已裁决 |
| 不做第二套任务列表 | 待办只聚合各领域权威待确认项（§6） |
| 不做跨项目视图 | 项目级视图；跨项目属 Personal Home / S01-05 |
| 不做初始化状态写入 API | 本页是读模型 + 导航消费者 |

---

## §2 页面信息架构

### 2.1 页面在工作空间中的位置

```
AI R&D Workspace Shell
└── Project 工作空间（项目作用域）
    ├── 总控台 Command Center            ← 初始化完成后的默认落地页
    ├── 目标与计划
    ├── AI 执行
    ├── 决策中心
    ├── 项目认知
    ├── 产物验收
    └── ★ 项目初始化总览（Initialization Overview）
        ├── S01-02 Project Creation       （创建完成后进入本页）
        ├── S01-03 Repository Management
        ├── S01-04 AI-PM Cognition
        ├── 基线确认（基线确认规范）
        └── Codex / MCP Bootstrap
```

**导航规则**【DECIDED·I5 §2.1】：

- 初始化总览**不占用长期主导航位**。它是**项目生命周期内的临时工作区**，由项目创建完成事件或「项目尚未就绪」提示进入。
- 进入条件（满足其一）：① 刚创建项目（S01-02 §7.1 第 7 条「自动进入项目初始化工作空间」）；② 项目存在未完成/阻塞的初始化事项；③ 用户从总控台的初始化进度区下钻。
- 退出条件：初始化事项全部收口后，总控台**接管**该视图的常驻职责；本页保留可再次进入的入口（历史与补做事项仍在）。

**路由提案**【NEW】：

| 路由 | 用途 | 现状 |
| --- | --- | --- |
| `/project/initialization/overview` | 本页（项目作用域由 `useScopeStore.currentProjectId` 提供） | `ai-frontend` **无** `/project/:projectId/*` 参数路由，项目作用域目前是 store 制的（`ai-frontend/src/api/scope.ts`），**不新造参数路由** |

### 2.2 五区结构与职责

| 区 | 职责 | 回答的问题 | 数据来源（已注册端点） |
| --- | --- | --- | --- |
| **① Project Header** | 项目身份与当前上下文 | 这是哪个项目？我是谁？我在项目里是什么角色？项目处于什么状态？ | `command-center` → `project_summary`【EXISTING】 |
| **② Initialization Progress** | 六阶段状态全景 | 走到哪一步了？哪些已完成？ | `command-center` → `initialization_progress`【EXISTING】 |
| **③ Current Action** | 当前最重要的一件事 | 现在谁在做什么？我要做什么？ | `initialization_progress` + `human_action_summary` 派生【EXISTING】 |
| **④ Human Actions** | 待人类介入清单 | 哪些事必须我亲自做？ | `command-center` → `human_action_summary`【EXISTING】 |
| **⑤ AI Context Panel** | AI 工作上下文 | AI 正在做什么？分析出什么了？哪些是我还没确认的？ | 领域端点直读【EXISTING】 |

**⑤ 是右侧上下文栏（320px）**，遵循设计系统 §二 布局规则 1：**不是第二个聊天窗口**。

### 2.3 布局基线

沿用《AI R&D Workspace 高保真设计系统 V1.0》：

| 区域 | 规格 | 职责 |
| --- | --- | --- |
| 顶部栏 | 52px | 壳级：租户、项目切换、全局操作 |
| 左侧导航 | 232px（可折叠 60px） | 壳级：项目工作区导航 |
| 中央主工作区 | 自适应 | 本页 ①～④ |
| 右侧上下文栏 | 320px（可隐藏/固定） | 本页 ⑤ AI Context Panel |
| 底部 AI 指令栏 | 收起 72px / 展开 120px | **壳级全局能力**，非本页内容 |

**页面级栅格（1440×900 设计基准）**：

```
┌──────────────────────────────────────────────────────────────────────┐
│ TopBar 52px                                                           │
├────────────┬────────────────────────────────────┬────────────────────┤
│            │ ① Project Header                   │                    │
│  SideNav   ├────────────────────────────────────┤ ⑤ AI Context Panel │
│  232px     │ ③ Current Action（首屏第一视觉块）  │      320px         │
│            ├────────────────────────────────────┤                    │
│            │ ② Initialization Progress（六阶段） │  · AI 执行记录     │
│            │   Stage 1..5 卡片 + Stage 6 交接区  │  · 最近完成的分析 │
│            ├────────────────────────────────────┤  · 已形成的提案   │
│            │ ④ Human Actions（待人类处理）      │  · 当前发现的问题 │
│            │                                    │  · 下一步计划       │
├────────────┴────────────────────────────────────┴────────────────────┤
│ CommandBar 72px（壳级）                                               │
└──────────────────────────────────────────────────────────────────────┘
```

**响应式**：右侧栏隐藏时中央区自动扩展（设计系统 §五）；<1280px 时 Human Actions 由双列降单列；<1024px 时 AI Context Panel 折叠为区块置于主工作区末尾。

### 2.4 首屏优先级

与 I5 §3.1 一致但按初始化语境重排：

1. **Current Action**（本页的第一视觉块）— 因为它把「谁在做什么」压缩成一句话
2. **Human Actions** — 需要人动手的
3. **阻塞**（并入 Current Action 的阻塞位与阶段卡片的阻塞徽标）
4. **Initialization Progress**（六阶段全景）
5. Project Header 的次要字段

> **与总控台的差异**：总控台首屏第一块是「待人工处理」，因为它服务**日常管理**；本页首屏第一块是「Current Action」，因为它服务**一次性但可中断的初始化**。两者数据同源、排序规则不同，且本页的排序规则可解释（§5.2）。

### 2.5 页面线框（ASCII）

```
┌─ ① Project Header ────────────────────────────────────────────────────┐
│ [Avatar] 星河客服中台  SHIELD          生命周期：planning             │
│           项目负责人：林岸（Owner）     仓库 3 · 已验证 2 · 成员 8     │
│           创建于 2026-09-28 14:02      [进入总控台 →]                  │
└────────────────────────────────────────────────────────────────────────┘
┌─ ③ Current Action ────────────────────────────────────────────────────┐
│ ⟳ AI-PM 正在分析仓库 smark/shield-web                                 │
│   阶段：读取 commit 与文档（45%）   来源：repo_import #imp-9f2        │
│   无需你操作。完成后会在此提示基线确认。              [查看执行详情 →]   │
└────────────────────────────────────────────────────────────────────────┘
┌─ ② Initialization Progress ────────────────────────────────────────────┐
│ ① 创建项目     ● 已完成        ② 仓库接入     ◐ 进行中                │
│   2026-09-28   SHIELD          3 个仓库      [进入 S01-03 →]           │
│ ③ 项目认知     ○ 未开始        ④ 基线确认    ○ 未开始                  │
│   依赖 ②       [进入 S01-04 →]   依赖 ②③                              │
│ ⑤ 开发环境     ◐ 进行中        ⑥ 初始化交接  ◌ 尚无法判断             │
│   1 台设备      1 台待绑定      [进入 Bootstrap →]                    │
└────────────────────────────────────────────────────────────────────────┘
┌─ ④ Human Actions ─────────────────────────────────────────────────────┐
│ │ ● 待确认发现      2 项   认知结论待人工确认   [查看 →]              │
│ │ ○ 待审批提案      1 项   知识提案待确认       [查看 →]              │
│ │ ○ 待验收交付      0 项                                                  │
└────────────────────────────────────────────────────────────────────────┘
```

---

## §3 六个初始化阶段的详细 UX

### 3.0 六阶段总表

#### 3.0.1 阶段 ↔ 现有初始化事项 ↔ 权威数据源

**事实**【EXISTING】I5 已实现的 `initialization_progress` 有 7 个事项（`command_center.rs:630-762`）。本稿的六阶段是对它们的**阶段化重组**，并**新增 1 个事项**（基线确认）。

| 阶段 | I5 现有事项 | 事项 key | 权威数据源 | 阶段详情页（归属） |
| --- | --- | --- | --- | --- |
| ① 创建项目 | 项目基本信息 | `project_basics` | `core.project` | S01-02（创建表单） |
| ② 仓库接入与代码分析 | 仓库接入 + 代码分析 | `repository_access` + `code_analysis` | `integration.project_repository`、`integration.repo_import`、`integration.repo_import_source` | S01-03 |
| ③ AI-PM 认知 | 项目认知 | `project_cognition` | `discovery.discovery_session`、`discovery.discovery_finding`、`discovery.discovery_proposal`、`ai.document_proposal` | S01-04 |
| ④ 基线确认 | **（无对应事项）** | `baseline_confirmation` **【NEW】** | 派生自 discovery/discovery_proposal/document_proposal（无独立基线表） | 基线确认规范 |
| ⑤ Codex / MCP 接入 | 开发环境 | `dev_environment` | `devenv.device`、`devenv.device_project_binding` | Codex/MCP Bootstrap 规范 |
| ⑥ 初始化交接 | 人工处理 | `human_actions` | 各领域权威待确认项 | 交接规范 |
| （贯穿） | 项目目标 | `project_goal` | Goal 数据（I4，尚未实施） | I4 Goal |

> **重要**：`project_goal`（项目目标）**不等于**「基线」。Goal 是目标陈述（I4，Knowledge + `doc_key=project-goal`）；基线是对项目当前事实的正式确认。二者是不同对象。本页必须避免用「基线」措辞指代 Goal（§12.4）。

#### 3.0.2 阶段状态词表

**事实**【EXISTING】展示层状态来自 `command_center.rs:74-81` 的 `init_status` 常量：

`not_started` | `in_progress` | `completed` | `blocked` | `not_applicable` | `undetermined`

**提示词要求**区分「等待人类操作」与「被外部条件阻塞」，但**现有六态词表不含「等待人类」**。

**事实**【EXISTING】前端已有一个未被使用的 `waiting_for_human` 值：
- `ai-frontend/src/api/types/initialization.ts:16-21` → `InitializationStatus = not_started | in_progress | waiting_for_human | blocked | completed`
- `ai-frontend/locales/{en,zh-CN}/s01/initialization.yaml` → `state.waiting_for_human`

**结论与建议**：

| 方案 | 说明 | 代价 |
| --- | --- | --- |
| **A（建议）**：展示层扩展第七态 `waiting_for_human` | 与前端既有类型/文案对齐；与「blocked」语义严格区分（blocked=外部条件阻塞且用户当下无动作可做；waiting=系统在等用户） | 需同步改后端 `init_status` 常量、前端 `InitItemStatus`、`pm.commandCenter.init.*` i18n 三处（约束 C-2） |
| B：六态不变，用 `in_progress` + 文案「等待你确认」表达 | 零改动 | `in_progress` 无法区分「AI 在跑」和「等 human」，与提示词 §三「必须区分正在执行 / 等待人类操作」直接冲突；也会污染 `expanded` 收缩规则（`command_center.rs:746-753`） |

**【已裁决 2026-11-04】方案 A 通过**：合并为七值。`waiting_for_human` = 系统已产生需要人类处理的事项（球在人手里），与 `in_progress`（系统在工作）、`blocked`（外部条件阻塞、当前无可执行的人类确认动作）三者边界为冻结基线（S01-01 · G-01 InitItem 七态展示状态模型 V1.1）。判定必须依据事项与人的**实际操作责任**，不得把所有 PENDING 记录盲目映射为等待人类。

#### 3.0.3 阶段的通用卡片结构

每张阶段卡片固定五要素（信息密度与卡片顺序对齐 I5 §4.3/§4.4）：

```
┌─────────────────────────────────────────────────────────────┐
│ ② 仓库接入与代码分析                              [状态徽标]  │
│ 一句话实况（detail，由后端 source_state/detail 派生）          │
│ 权威来源标识（source_state，例如 import 作业 id + status）    │
│ 依赖提示（依赖哪些阶段 / 被谁依赖）                            │
│ [进入阶段详情 →]  [处理本阶段待办（N）]                        │
└─────────────────────────────────────────────────────────────┘
```

**通用规则**：

| 规则 | 说明 |
| --- | --- |
| R-S1 状态徽标必须带文字 | 设计系统 §1.1：颜色必须同时搭配文字和图标，不能只靠颜色 |
| R-S2 `detail` 必须是事实描述 | 来自后端 `detail` 字段（已存在）；不得由前端拼装推测 |
| R-S3 必须显示权威来源 | `source_state` 已是 `InitItem` 字段【EXISTING】`command-center.ts:52`；为空时显示「暂无权威来源」而非省略 |
| R-S4 阶段卡片恒显示 | 不因后续阶段推进而移除（对齐提示词 §五.7「已完成的阶段应保留历史记录」） |
| R-S5 状态为 `not_applicable` 时不显示「进入详情」按钮 | 改为解释文案（例：零仓库项目 → 代码分析不适用） |

#### 3.0.4 历史保留

- 阶段卡片常驻「历史」入口 → 领域历史（`repo_import` 列表【EXISTING】`GET /v1/pm/projects/{p}/imports`、`discovery_session` 列表【EXISTING】`GET .../discovery/sessions`、`document_proposal` 列表【EXISTING】`GET /v1/pm/documents/proposals`）。
- 总控台的 `recent_activity` **不覆盖历史**：I5 §3.4 已裁决「重新执行不能覆盖历史 Run」，本页同理继承。
- 历史中的失败记录**不**把当前阶段判为失败（I5 §4.4：「不能仅凭一条历史失败记录，将项目永久判为失败」）。

---

### 3.1 Stage 1 — Create Project（建立项目身份）

**职责**：回显项目身份，声明创建已完成。**不实现创建表单**（S01-02）。

| 展示项 | 取值 | 来源 |
| --- | --- | --- |
| 项目名称 / 标识 | `core.project.name` / `.code` | `project_summary`【EXISTING】 |
| 生命周期状态 | `core.project.status` 实际值 | 【EXISTING】`migrations/0001_core.sql:24` |
| 创建时间 | `core.project.created_at` | 【EXISTING】 |
| 当前用户项目角色 | `core.project_member.role` | 【EXISTING】`migrations/0030_project_member_role.sql:7-9` |
| 负责人 | `project_member.role='owner'` | 【EXISTING】`crates/pm-application/src/project_member.rs:454` |

**事实**【EXISTING】`core.project.status` 是 `TEXT NOT NULL DEFAULT 'planning'`，**无 CHECK 约束**（`migrations/0001_core.sql:24`）。代码实际写入值只有三个：

| 值 | 写入点 | 语义 |
| --- | --- | --- |
| `planning` | `bootstrap.rs:264-271`（创建）、`project_import/advance.rs:73`（应用后回写） | 正常项目 |
| `initializing` | `project_import/lifecycle.rs:252` | 仓库导入进行中 |
| `archived` | `archive.rs:78` | 已归档 |

> ⚠️ `command_center.rs:634-637` 把 `project_basics` 的 `source_state` **硬编码为字符串 `"planning"`**，与实际可能为 `initializing`/`archived` 不一致 → **G-02（缺口，P1）**。本页展示生命周期时必须显示真实 `project_summary.lifecycle`（`command_center.rs:150-151` 透传 `core.project.status`），而不是重复读 init item 的 `source_state`。

**阶段状态**：恒为 `completed`（项目已存在是既成事实）。**注意**：不得据此宣称「初始化完成」（C-3）。

**边界**：创建失败、幂等、Owner 授权校验等属 S01-02 §9/§11；本页只在「用户无权限 / 项目不存在」时进入 §10.12 的降级态。

---

### 3.2 Stage 2 — Repository Management（仓库接入与代码分析）

**职责**：呈现仓库关联、授权、Primary、代码分析作业四类事实，并承接仓库变化确认。

#### 3.2.1 展示内容

| 项 | 事实来源【EXISTING】 |
| --- | --- |
| 已关联仓库数 / 已验证数 | `integration.project_repository`（status ∈ pending/verified/unreachable/archived，`migrations/0009_repo_import.sql:16-17`） |
| 每个仓库的角色 | `project_repository.role` ∈ **main/docs/service/infra**（`migrations/0008_repositories.sql:23-24`） |
| 每仓库最近核验 | `head_commit_sha` / `last_verified_at` / `last_error_code`（`migrations/0009_repo_import.sql:18-21`） |
| 代码分析作业 | `integration.repo_import` 十态 + `progress` + `stage_detail`（`migrations/0009_repo_import.sql:27-49`） |
| 分析基准 Commit | `integration.repo_import_source.head_commit_sha`（`migrations/0031_...:63-79`） |

#### 3.2.2 仓库授权状态 —— 结构性缺口

**事实**【EXISTING】仓库接入当前依赖 **SSH Key / 项目级凭证**：`integration.project_ssh_key`、`project_repository.ssh_key_id`（`migrations/0009_repo_import.sql:15`，**该列在应用代码中从未被写入**）、以及 `project_import` 作业的 `last_error_code ∈ {ssh_key_missing, timeout, auth_failed, not_found, too_large, git_failed}`（`crates/pm-application/src/repository/repo.rs:264,314-320`）。

**事实**【NEW】S01-03 §5.2 已裁决「优先采用 Provider OAuth / App 授权」，但**后端不存在** `git-providers` 相关表与端点；前端 `ai-frontend/src/api/initialization.ts:200-222` 已写好 `listGitProviders` / `startGitProviderAuthorization` / `revokeGitProviderAuthorization` 三个调用，**全部无后端路由**。

**处理**：本页「仓库授权」必须**按实际接入方式展示**：
- 有 SSH Key 路径：展示「SSH Key 已配置 / 未配置 / 已撤销」，缺失时给 `ssh_key_missing` 错误码与恢复入口（`POST /v1/pm/projects/{p}/ssh-key`【EXISTING】）。
- OAuth / App 路径：**当前不可用**，不得渲染成一个永远点不动的按钮。若产品决定保留 OAuth 路线，须先落 G-04 的数据与端点。

#### 3.2.3 Primary Repository —— 展示层概念，不是系统事实

**事实**【EXISTING】后端**不存在** Primary Repository 概念：全仓无 `is_primary`/`primary_repository_id` 列、无相关代码。唯一含 `primary` 字样的 CHECK 是 `delivery.task_repository.role ∈ ('primary','secondary','knowledge')`（`migrations/0011_run_dispatch.sql:94-95`），那是**任务级**仓库角色，与项目级 Primary 无关。

**事实**【NEW】前端已假定其存在：`InitializationState.primary_repository_id`（`api/types/initialization.ts:69`）、`POST /v1/pm/repositories/{id}/set-primary`（`api/initialization.ts:170-174`）——后端**无此路由**。且前端角色词表 `primary|supporting|infrastructure|test`（`types/initialization.ts:79`）与后端 `main|docs|service|infra` **词表冲突**。

**处理**（对齐 S01-03 §3.2 已裁决 Q2「允许暂时没有 Primary Repository」「不得自动将第一个接入的仓库提升为 Primary」）：

| 情况 | 本页呈现 |
| --- | --- |
| 有恰好 1 个 `role='main'` 的仓库 | 可提示「主仓库（建议）」，措辞为**建议**而非事实，标 `建议` 而非 `已指定` |
| 有 0 个或 ≥2 个 `role='main'` | 呈现「未指定主仓库（可选）」，进入 S01-03 处理 |
| 任何情况 | **不得**出现「已设为主仓库」的实心结论 chip（未确认事实） |

→ G-03（P0）：需要一次口径裁决：**主仓库是新增持久化字段，还是仅由 `role='main'` 派生**。S01-03 §3.2 要求「用户可以通过受控操作指定或更换 Primary Repository」——该能力当前**无落点**。

#### 3.2.4 仓库变化确认

**事实**【EXISTING】存在两个可比较的基准：`project_repository.head_commit_sha` 与 `repo_import_source.head_commit_sha`。

**事实**【NEW】「人类确认更新」**没有承载位置**：无确认记录表、无确认端点。`POST /v1/pm/repositories/{id}/verify`【EXISTING】返回 `VerifyOutcome`（含 `status` / `last_error_code`），但不记录「人确认采用新基准」这一决定。

**处理**：本页只呈现**可核对的事实差异**（仓库 HEAD 变化），并**明确标注**「更新尚未经人工确认」；确认动作在 G-05 落地前，下钻到 S01-03 处理，不得在总览页提供无持久化效果的「已确认」按钮。

#### 3.2.5 阶段状态派生

沿用 `command_center.rs:653-693` 的既有派生：

| 子项 | 判定 |
| --- | --- |
| `repository_access` | `repo_total == 0` → `not_started`；`repo_unreachable > 0` → `blocked`；`repo_pending > 0` → `in_progress`；否则 → `completed` |
| `code_analysis` | `repo_total == 0` → `not_applicable`；最新 import ∈ {queued, cloning, reading, analyzing} → `in_progress`；`failed` → `blocked`；{draft_ready, proposed, applied} → `completed`；{rejected, cancelled} → `blocked`；无作业 → `not_started` |

**阶段聚合规则**（本稿新增，需在实现时与 I5 一致）：

```
stage_status(repo_access, code_analysis):
  若任一 = blocked                      → blocked
  否则若任一 = in_progress               → in_progress
  否则若二者均 completed                → completed
  否则若 repo_access = not_started
       且 code_analysis = not_applicable → not_started   （零仓库合法，不判失败）
  否则                                   → in_progress
```

> **零仓库是合法状态**（I5 §4.7）。零仓库项目在本页必须显示为「尚未关联仓库（可选）」，**不得**显示为阻塞或失败，也不得阻止用户进入其他阶段。

#### 3.2.6 与 S01-03 的边界

| 本页做 | S01-03 做 |
| --- | --- |
| 显示仓库关联数、角色、核验状态、分析作业态 | 添加/移除仓库、Provider 授权、角色指派、Primary 指定、版本核验、HEAD 变更处理、长期仓库管理 |
| 提供下钻入口 | 提供全部操作界面 |
| 不提供任何写操作按钮 | — |

---

### 3.3 Stage 3 — AI-PM Cognition（AI-PM 认知）

**职责**：呈现 AI 对项目的认知建立进度，并**严格区分四层事实**（提示词 §三 Stage 3）。

#### 3.3.1 四层事实分离（硬性）

| 层 | 含义 | 权威判据【EXISTING】 |
| --- | --- | --- |
| **L1 AI 已分析** | AI 完成了资料收集与分析 | `discovery_session.status == 'COMPLETED'`（`migrations/0025_discovery.sql:18-20`）或 `repo_import.status ∈ {draft_ready, proposed, applied}` |
| **L2 AI 已形成提案** | AI 产出了待审内容 | `discovery.discovery_proposal.status == 'pending'`（4 态：`pending/approved/rejected/conflict`，`migrations/0025_discovery.sql:123-124`）或 `ai.document_proposal.status ∈ ACTIVE_PROPOSAL_STATUSES`（`command_center.rs:65-71`） |
| **L3 人类已确认** | 人类对具体条目做出了决定 | `discovery_finding.verification_status == 'CONFIRMED'`（`migrations/0025_discovery.sql:55-56`）/ `discovery_proposal.status == 'approved'` / `document_proposal.status ∈ {confirmed, committed}` |
| **L4 系统已正式写入** | 内容进入正式知识库 | `ai.document_proposal.status == 'committed'`（`migrations/0014_knowledge_v2.sql:38-41`） |

> ⚠️ **【EXISTING】代码事实警告**：`ai.document_proposal` 的 `quality_gates_passed` 与 `confirmed` 两个状态**从未被应用代码写入**——全仓仅在读侧常量出现（`command_center.rs:67,69`、`review/knowledge.rs:20,22`）。因此 L3/L4 判定**不能**依赖 `confirmed` 这一值，必须以 `committed` 为准 → **G-07（P1）**。

> ⚠️ **【EXISTING】既有派生的语义缺陷**：`command_center.rs:694-707` 判定 `project_cognition`：`session_total == 0` → `not_started`；`session_open > 0` → `in_progress`；**否则 → `completed`**。这会把「分析已完成但条目未确认」显示成「项目认知已完成」，违反 S01-04 §5.2「Run 的完成状态与 Item 的确认状态相互独立……不能把 Completed 解释成全部认知已经确认」→ **G-06（P0，语义错误，非样式问题）**。

**本稿建议的修正派生**（需与 I5 同步落地，不得单边改）：

```
project_cognition:
  session_total == 0                          → not_started
  存在 PENDING finding 或 pending proposal     → waiting_for_human   （G-01 已实施）
  存在 in-flight session / in-flight import    → in_progress
  存在 CONFLICT finding / conflict proposal    → blocked
  全部 proposal approved 且 finding 非 PENDING  → completed
  以上均不满足                                 → undetermined
```

#### 3.3.2 展示内容

| 展示项 | 事实来源 |
| --- | --- |
| 认知会话数 / 当前会话状态 | `discovery.discovery_session`（kind 仅 `INITIAL_ONBOARDING` 被实际创建，`discovery/web.rs:41`） |
| 已识别结构与技术信息 | `discovery_finding.kind ∈ {observed, user_confirmed, speculation, contradicted}`（`migrations/0025_discovery.sql:52`） |
| 已分析仓库信息 | `repo_import_source_attempt`（facts/commit_count/doc_count，`migrations/0031_...:82-106`） |
| 认知构建进度 | `discovery_session.status` + 关联 import 作业 `progress`/`stage_detail` |
| 待人类确认的认知结果 | `discovery_finding.verification_status == 'PENDING'` |
| 未解决的认知问题 | `discovery_finding.verification_status == 'CONFLICT'` + `discovery_proposal.status == 'conflict'` |

#### 3.3.3 Finding 的四态显示

**事实**【EXISTING】`discovery_finding` 只有两个枚举列，但存在**派生的第四显示态**：

| 显示态 | 判据 |
| --- | --- |
| 待确认 | `verification_status = 'PENDING'` |
| 已确认 | `verification_status = 'CONFIRMED'` |
| 已拒绝 | `verification_status = 'REJECTED'` |
| 存在冲突 | `verification_status = 'CONFLICT'` |
| **要求补充信息** | 展示层派生：`finding.rs:55,89-103` `REVISION_REQUESTED`，**不改变** `verification_status` |

> **【VERIFY】** `finding.rs:89-103` 的 `derive_display_state` 依据最近一次审计动作派生；I2 V1.1 已裁决「不得从 PENDING+open question 或 session 状态推断『待补充』，应从最近人工决定审计推导」。本页**不得**自行推导，须由后端返回。

#### 3.3.4 与 S01-04 的边界

| 本页做 | S01-04 做 |
| --- | --- |
| 显示认知四层事实的**聚合状态**与待确认数 | 六阶段认知建立流程、Preparation、Cognition Run/Item 全生命周期、逐项审核界面、Knowledge 更新与冲突处理、Agent Context 检视 |
| 提供下钻入口 | 提供全部操作界面 |
| 不提供逐项确认按钮 | — |

---

### 3.4 Stage 4 — Project Baseline Confirmation（项目基线确认）

**职责**：呈现基线提案的待确认状态与冲突，并**把正式确认入口交给基线确认规范**。这是初始化阶段中**唯一必须由人类完成**的正式确认环节。

#### 3.4.1 结构性事实

**事实**【EXISTING】**当前不存在基线实体**：
- 无 `project_baseline` / `project_baseline_proposal` / `baseline_confirmation` / `baseline_risk` 表
- 无 baseline 相关 REST 路由
- `core.project` 无 `current_baseline_id` 列

**事实**【EXISTING】S01 基线 V1.2 Q3 与 §6 的口径是「**基线是派生判断**」：基线 = `discovery_session` 处于 `COMPLETED` **且** 其下提案已 approved **且** 无未决 Finding。V1.2 §3 明确「**不新增初始化状态机**」，§8 明确「**明确不新增 10 项**」。

**事实**【NEW】基线确认规范（`s01-01-project-baseline-confirmation-ux-v1.md` §2）提出的是「**建议标识**」：`project_baseline_proposal`、`project_baseline`、`project.current_baseline_id`、`baseline_confirmation`、`baseline_diff`、`baseline_risk` —— 均为**尚未实现**的设计候选。

#### 3.4.2 本页的阶段状态派生（V1 可实现）

基线确认阶段的判定**完全派生自 discovery/discovery_proposal/document_proposal**，不依赖尚不存在的基线表：

| 条件 | 阶段状态 |
| --- | --- |
| 无 `discovery_session` 或 session 未 `COMPLETED` | `not_started`（AI 尚未完成分析） |
| session `COMPLETED`，但仍有 `PENDING` finding / `pending` proposal | `waiting_for_human`（G-01 已实施；由 `init_judgement::cognition_status` 判定） |
| 存在 `CONFLICT` finding 或 `conflict` proposal | `blocked`（需人类裁决冲突） |
| 全部 proposal `approved`、finding 非 `PENDING`、且 `ai.document_proposal` 达到 `committed` | `completed`（正式事实已写入） |
| 上述均不满足（例如用户拒绝后未再生成） | `undetermined`（**不得**显示为失败） |

#### 3.4.3 展示内容

| 展示项 | 说明 | 事实来源 |
| --- | --- | --- |
| AI 已整理出的基线内容 | 摘要 + 差异统计 | 派生自 proposal 内容【EXISTING·无独立基线表】 |
| 待确认的项目认知与技术信息 | 待确认条目清单（按证据类型分层） | `discovery_finding` + `discovery_proposal` |
| 已确认的内容 | 已 `approved`/`CONFIRMED`/`committed` 的条目 | 同上 |
| 尚未确认的内容 | `PENDING` 条目计数与列表 | 同上 |
| 存在冲突或需要决策的内容 | `CONFLICT` finding / `conflict` proposal | 同上 |
| 查看详情并进行确认的入口 | 下钻到基线确认规范页面 | **入口可用，但目标页后端未就绪 → G-08（P0）** |

#### 3.4.4 「已确认」与「已写入」的措辞纪律

基线确认横跨**两条不同的提案状态机**：

| 提案管线 | 状态机 | 事实 |
| --- | --- | --- |
| `discovery.discovery_proposal` | 4 态：`pending / approved / rejected / conflict`（`migrations/0025_discovery.sql:123-124`） | **无 REST 路由**，仅 MCP 写入（`crates/pm-mcp/src/tools.rs:1294,1309,1343,1364`；工具 `pm_discovery_submit_proposal`，`crates/pm-mcp/src/server.rs:340`） |
| `ai.document_proposal` | 8 态：`draft / quality_gates_passed / redline_pending / confirmed / commit_pending / committed / rejected / rolled_back`（`migrations/0014_knowledge_v2.sql:38-41`） | REST 可写：`POST /v1/pm/documents/proposals`、`POST /v1/pm/documents/proposals/{id}/confirm`【EXISTING】 |

**规则**：

| 状态 | 允许的措辞 | 禁止的措辞 |
| --- | --- | --- |
| proposal `pending` / finding `PENDING` | 「待确认（AI 产出）」 | 「已确认」「已生效」「已纳入基线」 |
| proposal `approved` / finding `CONFIRMED` | 「人类已确认」 | 「系统已写入知识库」 |
| `document_proposal.status == 'committed'` | 「系统已正式写入」 | — |
| finding `kind == 'speculation'`（即使已确认） | 「人类已确认的推断」 | 「项目事实」——F3 类内容经人工确认后仍是**人类断言**，不是代码事实 |

> **I2 V1.1 铁律**：R2-B 必须落在服务端可信身份（`core.principal.kind == 'human'`，`migrations/0001_core.sql:33-35`）上，Agent 与 AI-PM 均不得自行提升。本页展示的所有「人类已确认」必须来自服务端返回，不得由前端推断。

#### 3.3.5 与基线确认规范的边界

| 本页做 | 基线确认规范做 |
| --- | --- |
| 显示基线阶段状态、待确认/已确认/冲突的**聚合计数**与下钻入口 | 提案全文、版本差异、风险裁决、二次确认弹窗、版本历史、并发控制 |
| 不提供确认按钮 | 提供全部确认交互 |
| 不定义基线版本语义 | 定义 V1/V2+ 版本模型、不可覆盖约束、并发令牌 |

---

### 3.5 Stage 5 — Codex / MCP Bootstrap（Codex / MCP 接入）

**职责**：呈现 AI 研发执行环境接入状态。**不得**与 Stage 3 的认知状态混为一谈。

#### 3.5.1 权威事实

**事实**【EXISTING】`devenv.device`：

| 列 | 枚举 | 位置 |
| --- | --- | --- |
| `status` | `active` / `revoked` | `migrations/0027_devenv.sql:33-34` |
| `readiness` | `installed` / `connected` / `authenticated` / `ready` / `degraded` / `failed` | `migrations/0027_devenv.sql:36-38` |
| `last_check` (JSONB) | `{installed, connected, tool, verified_repository_count, declared_repository_count, evidence}` | `crates/pm-application/src/devenv/checks.rs:93-100` |
| `executor_kind` | 默认 `codex`（无 CHECK） | `migrations/0027_devenv.sql:36` |

**事实**【EXISTING】`readiness` 是**服务端计算**的六层快照（`devenv/checks.rs:139-164` `compute_readiness`），**不采信客户端自报**；且与生命周期 `active/revoked` **正交**（`migrations/0027_devenv.sql:14-16` 注释）。

**事实**【EXISTING】设备↔项目绑定是独立表 `devenv.device_project_binding`（`migrations/0027_devenv.sql:58-79`），含 `declared_repository_ids`（声明的仓库集）与 `verified_repositories`（已验证子集），`UNIQUE(device_id, project_id)` 保证幂等重绑。

**事实**【EXISTING】可用端点：`POST /v1/pm/devices`（注册）、`GET /v1/pm/devices/{id}`（详情）、`POST /v1/pm/devices/{id}/bindings`（绑定）、`DELETE /v1/pm/devices/{id}/bindings/{bid}`（解绑）、`POST /v1/pm/devices/{id}/checks`（自检→重算 readiness）、`POST /v1/pm/devices/{id}/revoke`（吊销）；引导页 `GET /install`、`GET /install/{tool}`（`apps/pm-api/src/onboarding.rs:27-28`），JSON 变体含 `bootstrap` 段（`onboarding.rs:157-187`）。

#### 3.5.2 展示内容

| 展示项 | 事实来源 |
| --- | --- |
| Codex 接入状态 | `device.executor_kind` + `device.status` |
| 六层 readiness 逐步展开 | `device.readiness` + `device.last_check` |
| 开发环境初始化状态 | `device.readiness` + `device_project_binding` |
| 是否具备执行条件 | 派生：至少 1 台 `readiness == 'ready'` **且** 绑定声明仓库均已验证（`verified_repositories` ⊇ `declared_repository_ids`）——**执行就绪是派生量，不是第七种状态** |
| 授权/配置/连接阻塞 | `readiness ∈ {degraded, failed}` 时的 `last_check.evidence` |
| 用户需要执行的下一步 | 派生动作：注册设备 / 绑定项目 / 执行自检 / 重新授权 |

#### 3.5.3 阶段状态派生

沿用 `command_center.rs:708-726`：

| 条件 | 状态 |
| --- | --- |
| `device_total == 0` | `not_started` |
| 存在 `degraded` 设备 | `blocked` |
| 存在**从未自检**的设备 | `undetermined`（真实就绪条件尚未核验） |
| 存在 `ready` 设备 | `completed` |
| 其余 | `in_progress` |

#### 3.5.4 MCP 侧的结构性缺口（必须诚实呈现）

**事实**【EXISTING】pm-api **没有** MCP 状态端点：MCP 服务在 `crates/pm-mcp`；`/install/{tool}` 是**引导文档**，不是连接状态源。

**事实**【EXISTING】`BootstrapInfo` 在前端只是**类型**（`ai-frontend/src/api/devices.ts:124-135`），**没有任何 fetch 函数**——`/install/{tool}?format=json` 未被前端消费。

**处理**：

- 「MCP 配置与连接状态」在 V1 **必须**显示为「**暂无可用状态源**」或直接省略该行，**不得**渲染一个永远显示「已连接」的假状态。
- 若产品坚持展示 MCP 状态，须先补 G-09（一个只读的 MCP 接入状态读模型）。
- Codex 侧可以完整展示（device + readiness 已具备）。

---

### 3.6 Stage 6 — Enter Project（初始化交接）

**职责**：判断项目是否具备进入正常研发管理的条件，并提供交接出口。**不是**新的领域实体，**不是** Workspace 概念。

#### 3.6.1 交接判定（严格遵守 I5 §4.5）

**事实**【EXISTING】`command_center.rs:759`：`completion` 恒为 `"not_determinable"`——因为当前**没有项目类型/初始化要求配置**，不得擅自把 Goal、仓库、开发环境全部定义为必需项。

**事实**【EXISTING】`required_items` 当前硬编码为 `project_basics`、`project_goal`、`human_actions`（`command_center.rs:740-744`）。其中 `project_goal` 依赖 I4（尚未实施），`human_actions` 是**跨领域集合**（含交付验收，与初始化无关）。

**结论**：在 I4 与 human-action 子集拆分就绪前，**Stage 6 只能是 `undetermined`**。

#### 3.6.2 四种交接态（本稿定义）

| 态 | 条件 | 呈现 | 出口 |
| --- | --- | --- | --- |
| **H1 具备交接条件** | 全部 required 事项 `completed`，无 `blocked` | 「可以进入项目总控台」 | 「进入总控台 →」 |
| **H2 部分就绪** | 无 `blocked`；required 中存在 `not_started`/`in_progress`/`not_applicable` | 「可以进入，剩余事项将在总控台持续跟踪」 | 「进入总控台 →」+ 剩余事项清单 |
| **H3 存在阻塞** | 任一事项 `blocked` | 「暂不建议交接」+ 阻塞清单与解除条件 | 交接按钮**禁用**并说明原因（不得隐藏，须解释） |
| **H4 尚无法判断** | `completion == 'not_determinable'`（**当前恒真**） | 「项目接入概况」——**不宣称完成** | 「进入总控台 →」（允许进入，但措辞不得暗示初始化已完成） |

> **强制措辞**：H4 下页面**不得**出现「初始化完成」「初始化 100%」「项目已就绪」中的任何一个。允许的措辞：「项目接入概况」「基础接入已建立，完整就绪状态待项目类型配置补充后确定」。

#### 3.6.3 交接后的行为

- 交接**不改变任何领域状态**，只是改变默认落地页。
- 交接后，总控台的 `initialization_progress` 区块**接管**常驻展示（I5 §4.1），本页转为**可再次进入**的补做入口（剩余事项、失败重试、历史记录）。
- 交接不结束初始化：新增仓库、认知更新、设备换绑都会在总控台重新点亮对应事项（I5 §4.6 收缩规则的反向）。

---

## §4 各阶段状态与状态转换

### 4.1 状态词表与来源

| 展示状态 | 常量值 | 中文文案 | 语义 | 来源 |
| --- | --- | --- | --- | --- |
| Not Started | `not_started` | 未开始 | 该事项尚未开始，且当前无外部条件阻塞它 | 【EXISTING】`command_center.rs:74-81` |
| In Progress | `in_progress` | 进行中 | 有执行主体正在推进（人或系统） | 同上 |
| Waiting for Human | `waiting_for_human` | 等待你确认 | **已裁决（G-01 V1.1）**：系统已交出球，等待人类行动；展开走自有分支 | 【EXISTING】`command_center.rs` `init_status::WAITING_FOR_HUMAN` + `pm.commandCenter.init.waitingForHuman` |
| Blocked | `blocked` | 存在阻塞 | 被外部条件阻塞，当前无动作可做 | 【EXISTING】 |
| Completed | `completed` | 已完成 | 该事项的完成条件已满足 | 【EXISTING】 |
| Not Applicable | `not_applicable` | 不适用 | 该事项对本项目无意义（如零仓库的代码分析） | 【EXISTING】 |
| Undetermined | `undetermined` | 尚无法判断 | 真实条件尚未核验（如设备从未自检） | 【EXISTING】 |

**约束 C-2（再次强调）**：新增/修改状态值必须**三处同改**——后端 `init_status` 常量（`crates/pm-application/src/command_center.rs:74-81`）+ 前端 `InitItemStatus`（`ai-frontend/src/api/command-center.ts:44-50`）+ i18n `pm.commandCenter.init.*`（`ai-frontend/locales/{en,zh-CN}/pm.yaml:368-377`）。

> **【EXISTING】i18n 现状**：`pm.commandCenter.init.*` 已有 6 个状态文案 + `unknown` + `required` + `notDeterminable`；前端 `s01/initialization.yaml` 的 `state.*` 已有 5 个（含 `waiting_for_human`）但**不被任何组件引用**（孤儿词表）。两套词表并存，实施时须择一为主，避免双份真源。

### 4.2 逐阶段派生规则（判定表）

| 阶段/事项 | 判定（自上而下命中即返回） | 权威数据 |
| --- | --- | --- |
| `project_basics` | 恒 `completed` | `core.project` 存在 |
| `repository_access` | 仓库数=0 → `not_started`；有 `unreachable` → `blocked`；有 `pending` → `in_progress`；否则 → `completed` | `project_repository.status` |
| `code_analysis` | 仓库数=0 → `not_applicable`；最新 import ∈ {queued,cloning,reading,analyzing} → `in_progress`；`failed` → `blocked`；{draft_ready,proposed,applied} → `completed`；{rejected,cancelled} → `blocked`；无作业 → `not_started` | `repo_import.status` |
| `project_cognition` | **【G-06 待修正】** 现状：session 数=0 → `not_started`；有 open session → `in_progress`；否则 → `completed`。本稿建议见 §3.3.1 | `discovery_session.status` |
| `baseline_confirmation` | **【NEW】** 见 §3.4.2 | discovery/proposal 派生 |
| `dev_environment` | 设备数=0 → `not_started`；有 `degraded` → `blocked`；有未自检 → `undetermined`；有 `ready` → `completed`；否则 → `in_progress` | `device.readiness` / `last_check` |
| `human_actions` | 待处理数=0 → `completed`；否则 → `in_progress` | `human_action_summary.total` |

**派生规则的通用纪律**：

| 纪律 | 说明 |
| --- | --- |
| D-1 只用最新有效结果 | 取每个项目/仓库的**最新**作业；历史失败不得永久压过新成功（I5 §4.4） |
| D-2 不从状态缺失推断成功 | 数据不可得 → `undetermined`，不得默认 `completed` |
| D-3 状态由服务端派生 | 状态在服务端算好下发（`InitItem.status`【EXISTING】），前端不重算业务状态 |
| D-4 每个非 `completed` 状态必须可解释 | `detail`/`source_state` 至少一项非空，否则该事项视为数据不完整 → 触发该区块的「数据暂不可用」降级（§10.12） |

### 4.3 状态转换

**核心原则**：本稿**不定义**新的状态机。状态转换是**领域状态变化在展示层的投影**。

| 领域事件 | 展示层反应 | 是否由本稿定义 |
| --- | --- | --- |
| `repo_import` 作业 `queued → cloning → reading → analyzing` | `code_analysis`：`not_started → in_progress` | 否（领域已定义） |
| `repo_import` 作业 → `applied` | `code_analysis` → `completed`；`project_cognition` 可能点亮 | 否 |
| `repo_import` 作业 → `failed` | `code_analysis` → `blocked` | 否 |
| `repo_import` 作业 → `rejected`/`cancelled` | `code_analysis` → `blocked` | 否 |
| `discovery_session` → `COMPLETED` | `project_cognition` 状态上移（**不自动 = completed**，见 G-06） | 否 |
| finding `PENDING → CONFIRMED` | `human_actions` 计数下降；`baseline_confirmation` 可能上移 | 否 |
| `discovery_session` → `NEEDS_USER_INPUT` / `REVIEW_REQUIRED` | `project_cognition` → `waiting_for_human` | 否 |
| `device` readiness 变化 | `dev_environment` 重算 | 否 |
| `core.project.status` → `archived` | 全页转只读（§11.3） | 否 |

**合法转换矩阵（展示层）**：

| 从 \ 到 | not_started | in_progress | waiting_human | blocked | completed | n/a | undetermined |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **not_started** | — | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| **in_progress** | ✗ | — | ✓ | ✓ | ✓ | ✓ | ✓ |
| **waiting_human** | ✗ | ✓ | — | ✓ | ✓ | ✓ | ✗ |
| **blocked** | ✓ | ✓ | ✗ | — | ✓ | ✓ | ✓ |
| **completed** | ✗ | ✗ | ✗ | ✗ | — | ✗ | ✗ |
| **n/a** | ✓ | ✓ | ✗ | ✓ | ✓ | — | ✓ |
| **undetermined** | ✓ | ✓ | ✗ | ✓ | ✓ | ✓ | — |

> `completed` 是**终态**（领域事实一旦成立不得回退展示为未完成）；若领域事实确实回退（如仓库被归档导致 `repository_access` 变化），由领域事件驱动重新派生，而不是用户操作页面造成。

### 4.4 不做百分比、不做线性门禁

| 禁止 | 依据 |
| --- | --- |
| 「初始化完成度 73%」 | I5 §4.8：各事项不等权重、部分可并行、可不适用、无权重依据的百分比制造错误的精确感 |
| 「必须完成 Stage 2 才能进入 Stage 3」 | I5 §4.2：不同初始化事项可并行，也可不适用 |
| 「初始化完成」徽章 | I5 §4.5 + `command_center.rs:759` `not_determinable` |
| 「Stage N 未完成 → 隐藏 Stage N+1」 | 六阶段是阅读分组，不是门禁；隐藏会让用户失去全局判断 |

---

## §5 Current Action 的决策规则

### 5.1 定义

**Current Action = 当前最重要的一件事**，是一个**单条**结构化对象，回答：正在做什么 / 谁负责 / 什么状态 / 已取得什么结果 / 要不要你动 / 你能做什么。

### 5.2 决策阶梯（确定性规则，可复算）

```
输入：六个阶段事项（含 status / source_state / detail）
      + human_action_summary.items
      + 领域在飞任务（in-flight repo_import / discovery_session / device checks）

Step 1  存在 blocked 事项？
        → 取阶段序最小（①→⑥）者；同级取 source_state 的最早发生时间者
        → 责任主体 = System（外部条件）或 Human（若阻塞原因是待人类处理）
        → 动作 = 解除阻塞

Step 2  存在 waiting_for_human 事项？
        → 取阶段序最小者；同级按「风险等级 P0 > P1 > P2」再按创建时间
        → 责任主体 = Human
        → 动作 = 进入确认流程

Step 3  存在 in_progress 且执行主体 ∈ {AI, System} 的在飞工作？
        → 取阶段序最小者；同级取最近更新的作业
        → 责任主体 = AI Agent 或 System
        → 动作 = 无需用户操作（可选：查看执行详情 / 取消）

Step 4  存在 not_started 且用户可单步启动的下一步？
        → 取阶段序最小者
        → 责任主体 = Human
        → 动作 = 启动该下一步

Step 5  以上皆无
        → 责任主体 = —
        → 动作 = 无待办；展示交接出口（Stage 6 的 H1/H2/H4）
```

### 5.3 责任主体的判定

| 主体 | 判据【EXISTING】 |
| --- | --- |
| **Human** | `core.principal.kind == 'human'`（`migrations/0001_core.sql:33-35`）；具体事项：finding 确认、proposal 审批、仓库授权/主仓库指定、设备绑定与自检、基线确认 |
| **AI Agent** | `core.principal.kind ∈ {ai_pm, dev_agent}`；在飞作业：`repo_import` ∈ {queued,cloning,reading,analyzing}、`discovery_session.status == 'IN_PROGRESS'`、`document_proposal` 生成中 |
| **System** | 后台派生与写入：`device` readiness 重算（`devenv/checks.rs:116-126`）、知识索引（`POST /v1/pm/documents/{id}/index-state`）、outbox 推进、项目生命周期变更 |

**规则**：责任主体显示为**可验证的来源**，不写「AI 正在思考」这类无来源描述。AI 主体必须同时给出 `source_state`（如 `repo_import #imp-9f2 = analyzing`）。

### 5.4 Current Action 的内容契约

| 字段 | 必填 | 来源 |
| --- | --- | --- |
| `title` | ✓ | 派生（阶段 + 事项名） |
| `owner` | ✓ | Human / AI Agent / System（§5.3） |
| `status` | ✓ | 展示状态（§4.1） |
| `result` | ✓ | `InitItem.detail` / `source_state`【EXISTING】 |
| `needs_user` | ✓ | 由阶梯 Step 决定 |
| `action_label` + `action_link` | ✓ | 下钻路由 |
| `blocking` | ✓ | 阻塞时的原因 / 影响范围 / 解除条件（§10.3） |
| `remaining_count` | 条件 | 同类其余事项数（如「另有 1 项待确认」） |

**渲染规范**：

- 位置：首屏第一视觉块，Project Header 之下。
- 形态：一句话标题 + 一行事实 + 一行来源 + 右侧主行动按钮。
- **不得**用进度百分比表达「AI 执行到哪」；用 `source_state`（作业状态 + 阶段文案）表达。
- 无待办时：明确写「当前没有需要你处理的事项」或「初始化已基本就绪」，**不得**留空或用 loading 骨架长期占位。

---

## §6 Human Actions 的呈现和操作规则

### 6.1 纳入规则（全部满足才纳入）

| # | 条件 |
| --- | --- |
| R-H1 | 该事项要求人类做出**业务决定或授权**（确认 / 拒绝 / 授权 / 指定 / 裁决 / 验收） |
| R-H2 | 存在**权威来源记录**可定位（来源领域 + 来源记录 ID + 可下钻路由） |
| R-H3 | 存在**明确的下一步动作**（不是一个状态，而是一个用户能执行的操作） |
| R-H4 | 存在**确定的责任人范围**（项目 owner / 具备治理权限的成员） |

### 6.2 排除规则（命中任一即排除）

| # | 条件 | 依据 |
| --- | --- | --- |
| X-H1 | 普通系统通知、公告、广播 | 提示词 §四.4：「不要将普通系统通知全部转化为待办」 |
| X-H2 | Agent 的建议、自述、提议（未构成正式待确认记录） | I5 §3.3：「不得把 Agent 建议直接当作待审批事项」 |
| X-H3 | 纯信息同步（如「仓库已验证」「分析已完成」） | 非决定性事项 |
| X-H4 | 已关闭 / 已过期 / 已被后续动作消解的记录 | 避免陈旧待办 |
| X-H5 | 其他项目或其他租户的事项 | I5 §7.6 |
| X-H6 | 无权威来源、无法下钻的条目 | R-H2 反面 |

### 6.3 当前可覆盖的来源（V1 事实）

**事实**【EXISTING】`human_action_summary` 由**读时派生并集**得出（`crates/pm-application/src/command_center.rs:975-1006` `pending_action_total`、`:1008+` `load_human_action_summary`）：

| 来源 | 判据【EXISTING】 | 已有确认入口 |
| --- | --- | --- |
| `discovery` — 待确认发现 | `discovery_finding.verification_status == 'PENDING'` | ✅ `POST /v1/pm/projects/{p}/discovery/sessions/{s}/findings/{f}/verification`（`apps/pm-api/src/pm_discovery.rs:43-46`） |
| `discovery` — 要求补充信息 | 派生态 `REVISION_REQUESTED`（`finding.rs:55,89-103`） | ✅ 同上（`status=PENDING` + `answer`） |
| `knowledge` — 待审批提案 | `ai.document_proposal.status ∈ ACTIVE_PROPOSAL_STATUSES`（`command_center.rs:65-71`） | ✅ `POST /v1/pm/documents/proposals/{id}/confirm`（`routes.rs:229-232`） |
| `delivery` — 待验收交付 | `delivery.task.status ∈ {submitted, reviewing}` | ✅ `POST /v1/pm/review-approvals/{id}/decision`（`apps/pm-api/src/pm_reviews.rs:35-42`） |

### 6.4 初始化场景中**尚无承载**的待办类型

以下四类是提示词 §四.4 明确要求呈现的，但在当前代码中**没有数据来源、没有确认入口**：

| 待办类型 | 现状【EXISTING】 | 缺口 |
| --- | --- | --- |
| 仓库授权 | 无 git-providers 表/端点；只有 SSH Key 路径 | G-04 |
| 仓库变更确认 | 可比较两个 head sha，但无「人类已确认更新」的记录 | G-05 |
| 项目基线确认 | 无基线表、无基线端点；仅有两条提案管线 | G-08 |
| Codex / MCP 授权 | 设备注册/绑定/自检存在；MCP 授权状态无源 | G-09 |

**处理（V1）**：

- 这四类**不进入** `human_action_summary` 聚合（避免污染跨领域计数与 I5 契约）。
- 其中「仓库授权」「设备绑定/自检」可以通过**阶段卡片内的就地行动**承接（因为它们有现成端点），不进入聚合待办区。
- 「仓库变更确认」「基线确认」在 V1 **显示为阶段内的待处理项**（从阶段数据派生，不进入跨领域聚合），并标注 ⚠「确认记录尚未落地」。
- 绝不为了凑齐待办类型而伪造数据源。

### 6.5 呈现与排序

| 规范 | 说明 |
| --- | --- |
| 区域位置 | Project Header 之下、阶段区之上（与 Current Action 相邻，形成「做什么 → 还有哪些要做」） |
| 每条最小字段 | 类型 chip（中性）、标题、来源领域、来源记录 ID 标识、最近更新时间、动作按钮、下钻入口 |
| 排序 | ① 风险等级（P0→P2）② 来源领域严重度 ③ 创建/更新时间最早 |
| 视觉纪律 | 沿用 S01-05 §D-a：**类型用中性 chip，颜色只承载紧急度（左侧 2px 轨）**；禁止彩虹化 |
| 空态 | 「当前没有需要你处理的事项」+ 交接出口，**不得**显示为 0 之外的假空态 |
| 失败态 | 该区加载失败 → 显示错误块 + Retry + 指标 `—`/`Unavailable`（S01-05 §D-c），**不得**显示「没有待办」 |

### 6.6 操作规则

| 规则 | 说明 |
| --- | --- |
| O-1 待办区只做导航 | 点击进入**对应领域的确认流程**，不在总览页内直接改业务状态（I5 §8.1） |
| O-2 一律下钻到权威域 | finding → 认知审核页；proposal → 知识提案页；delivery → 验收页 |
| O-3 写操作由原领域端点完成 | 不允许通过聚合读模型绕过领域服务（I5 §7.4） |
| O-4 服务端二次校验 | 按钮可见性由能力决定，服务端仍须再次校验（基线确认规范 §7.1） |
| O-5 MCP Key 不得执行确认 | 人类确认只允许可信人类身份（`core.principal.kind='human'`） |

---

## §7 AI Context Panel 的信息边界

### 7.1 定位

右侧上下文栏（320px），遵循设计系统 §二 布局规则 1：**「右侧上下文栏不是第二个聊天窗口。它默认显示当前对象、引用知识、Agent 状态、关联任务与操作权限。」**

本页的 AI Context Panel = **初始化场景下的 Agent 状态 + 引用知识 + 操作权限**，是**只读上下文**，不是对话界面。

### 7.2 五类信息与允许/禁止矩阵

| # | 信息类 | 允许内容 | 禁止内容 |
| --- | --- | --- | --- |
| 1 | **AI 执行记录**（事实） | 作业 id、领域状态、阶段文案、开始时间、已产出的量化结果（commit 数/文档数，来自 `repo_import_source_attempt`） | 自由文本的「AI 认为」；未落库的中间推理 |
| 2 | **最近完成的分析**（事实） | 已 `applied` 的 import、已 `COMPLETED` 的 session、已 `committed` 的 proposal，附产出物链接 | 把「分析完成」表述成「结论已确认」 |
| 3 | **已形成的提案**（AI 产出，待确认） | proposal 标题、领域、证据类型、置信度、证据条数 | 给提案打「已确认」「已生效」样式 |
| 4 | **当前发现的问题** | `CONFLICT` finding、`conflict` proposal、`last_error_code`、`device.readiness=degraded/failed` 的 evidence | 把「需要补充信息」由前端自行推导（须后端给派生值） |
| 5 | **下一步计划** | 由系统状态派生的**建议动作**（如「等待 import #imp-9f2 完成后可发起认知」） | Agent 自行书写的计划文本（除非有权威记录） |

### 7.3 与壳级 CommandBar 的边界

| | AI Context Panel（⑤） | 底部 CommandBar（壳级） |
| --- | --- | --- |
| 性质 | **只读上下文** | **输入区**（全局 AI 交互） |
| 内容 | 当前对象、Agent 状态、引用证据、待确认提案、下一步 | 输入框 + `/ 引用` + `@ Agent` + 发送 |
| 是否发起执行 | 否 | 是（但仍受授权链约束） |
| 是否能确认 | **不能** | **不能**（确认必须走 §8 明确流程） |

**规则**：AI Context Panel **不得**出现输入框、不得出现「一键确认」按钮、不得承载自由对话。它回答「AI 现在基于什么工作」，不回答「我该跟 AI 说什么」。

### 7.4 三层事实分区与视觉纪律

沿用 S01-05 高保真稿的 **D-b**（实心 = 已成立的正式事实 / 描边 = 尚待人工确认的 AI 产出）：

| 分区 | 填充 | 图标 | 语义 |
| --- | --- | --- | --- |
| **已成立事实**（人类确认 / 系统写入） | 实心（16% success / info 底） | `circle-check`（**仅**用于人类确认）/ `send`（系统提交） | 已是正式事实 |
| **AI 产出待确认** | 描边 accent（透明底 + accent 描边） | `sparkles` | 尚未成为事实 |
| **执行中记录** | 中性 | `loader` / 状态点 | 过程事实 |

**强制纪律**：

- **AI 事件一律不用对勾**。`circle-check` 只允许出现在「人类确认」上（S01-05 §5.2）。
- 实心 vs 描边是**主通道**，颜色是次通道——灰度打印仍可分辨。
- 不得用同一张卡片混排「已确认」与「待确认」条目；必须分区。

### 7.5 折叠与降级

| 情况 | 行为 |
| --- | --- |
| 面板隐藏 | 中央区自动扩展（设计系统 §五） |
| 面板数据加载失败 | 该面板塌陷为错误块 + Retry；**不得**显示成「AI 没有在工作」 |
| 项目无 AI 活动 | 显示「当前没有 AI 执行记录」+ 说明何时会产生（不填充示例数据） |

---

## §8 人类确认协议

### 8.1 确认主体

| 规则 | 内容 | 事实依据 |
| --- | --- | --- |
| H-1 | 正式确认**只能**由可信人类身份执行 | `core.principal.kind == 'human'`【EXISTING】`migrations/0001_core.sql:33-35`；finding 确认路径已在服务端校验（`discovery/web.rs:200-209`） |
| H-2 | **不得**依赖前端传来的角色字符串或 `kind` 字段做判定 | I2 V1.1 铁律：R2-B 必须落在服务端可信身份上，否则 Agent 改 `kind` 即可伪造已确认事实 |
| H-3 | Agent（`ai_pm` / `dev_agent`）与 AI-PM **均不得**自行把 speculation 提升为已确认 | S01 V1.3 Q5 / I2 §3.4 |
| H-4 | MCP Key 调用确认接口**必须拒绝** | 基线确认规范 §7.1 |

### 8.2 四步确认流程

所有正式确认遵循同一四步骨架（基线确认规范 §4 的四任务模型），**本页只提供入口，不在本页内完成**：

| 步 | 用户动作 | 系统必须保证 |
| --- | --- | --- |
| ① 理解 | 查看提案来源、证据、置信度、代码事实 vs AI 推断 | 事实与推断分层呈现；不得只给一个总体「AI 分析」标签 |
| ② 审查 | 查看差异/变化、影响范围、未决问题 | 不得静默删除条目；基线类必须提供差异视图 |
| ③ 裁决 | 处理冲突、风险、待补充信息 | 冲突未裁决时**阻止**普通确认 |
| ④ 正式确认 | 二次确认弹窗 → 提交 | 携带并发令牌；幂等；服务端二次校验；审计落库 |

### 8.3 可复用入口与缺失入口

| 确认对象 | 端点 | 状态 | 备注 |
| --- | --- | --- | --- |
| Finding 确认 / 拒绝 / 要求补充 | `POST /v1/pm/projects/{p}/discovery/sessions/{s}/findings/{f}/verification` | ✅【EXISTING】 | 体 `{status, answer, confirmed_finding?, operation_id?}`（`apps/pm-api/src/pm_discovery.rs:284-291`）；服务端要求 human、校验归属 session、项目未归档、`operation_id` 幂等（`discovery/web.rs:162-226`） |
| 知识提案确认 | `POST /v1/pm/documents/proposals/{id}/confirm` | ✅【EXISTING】 | `apps/pm-api/src/routes.rs:229-232` |
| 知识修订否决 | `POST /v1/pm/documents/revisions/{rid}/veto` | ✅【EXISTING】 | `routes.rs:233-236` |
| **Discovery 提案决策** | — | ❌ **仅 MCP** | `pm_discovery_submit_proposal`（`crates/pm-mcp/src/server.rs:340`；`tools.rs:1309,1343`）；**UI 无入口** → G-10 |
| 交付验收 | `POST /v1/pm/review-approvals/{id}/decision` | ✅【EXISTING】 | `apps/pm-api/src/pm_reviews.rs:35-42` |
| 设备绑定 / 自检 | `POST /v1/pm/devices/{id}/bindings`、`POST .../checks` | ✅【EXISTING】 | `apps/pm-api/src/pm_devices.rs:36-41` |
| **基线确认** | — | ❌ **不存在** | 无基线表/端点 → G-08 |
| **仓库变更确认** | — | ❌ **不存在** | 有 verify，无「人确认采用新基准」记录 → G-05 |
| **仓库授权（OAuth）** | — | ❌ **不存在** | → G-04 |

### 8.4 并发与幂等

| 规则 | 内容 |
| --- | --- |
| C-1 幂等键 | 确认类写操作携带 `operation_id`，服务端经 `integration.inbound_event` 去重（【EXISTING】`discovery/web.rs:170-197`）；前端重试不得产生第二条确认记录 |
| C-2 并发令牌 | 基线类确认须携带 `expected_current_baseline_id` + `expected_proposal_version`/`expected_row_revision`，冲突返回 `409 BASELINE_CONFLICT`（基线确认规范 §7.2） |
| C-3 乐观并发 | `project_repository.revision`【EXISTING】`migrations/0008_repositories.sql:27` 已是现有乐观锁模式，可作为同类操作的先例 |
| C-4 重复提交 | 提交期间按钮禁用并显示处理中；服务端幂等兜底 |
| C-5 不静默覆盖 | 冲突时提示刷新并重新审核，不允许静默覆盖 |

### 8.5 拒绝与暂缓

| 动作 | 事实支持 |
| --- | --- |
| 拒绝 Finding | `verification_status → REJECTED`【EXISTING】 |
| 要求补充信息 | `verification_status` 保持 `PENDING` + 写 discovery_question + 派生显示态 `REVISION_REQUESTED`【EXISTING】`finding.rs:55,89-103` |
| 拒绝提案 | `discovery_proposal.status → rejected`（**仅 MCP**）/ `document_proposal.status → rejected`【EXISTING】 |
| 记录保留 | 拒绝**不得**删除证据与历史（S01-04 §6.4） |

### 8.6 审计

| 事件 | 落点【EXISTING】 |
| --- | --- |
| Finding 决定 | `finding.decided` / `discovery_finding_revision_requested`（`discovery/web.rs:297-311`） |
| 项目归档 | `action='project_archived'` 复用 `governance.audit_log`（I1 §11） |
| 确认主体 | `actor_client` / `actor_auth_mode` / `claimed_device_id` 列已存在（`migrations/0027_devenv.sql:87-95`）；`pm_write/audit.rs` 有现成 helper 可复用 |

**要求**：本页所有「人类已确认」展示，其审计记录必须可从该记录反查确认人、时间、来源提案版本。

---

## §9 阶段依赖关系

### 9.1 依赖图

```
      ① 创建项目
           │
           ▼
   ┌───────────────┐
   │ ② 仓库接入     │◄──────────────┐
   │   + 代码分析   │                │
   └───────┬───────┘                │
           │                        │
           ├────────────┐           │
           ▼            ▼           │
   ┌───────────────┐  ┌────────────┐│
   │ ③ AI-PM 认知   │  │ ⑤ 开发环境 ││
   └───────┬───────┘  └─────┬──────┘│
           │                  │       │
           └────────┬─────────┘       │
                    ▼                 │
            ┌───────────────┐         │
            │ ④ 基线确认     │─────────┘
            └───────┬───────┘
                    ▼
            ┌───────────────┐
            │ ⑥ 初始化交接   │
            └───────────────┘
```

### 9.2 依赖分级

| 依赖 | 级别 | 说明 |
| --- | --- | --- |
| ① → ② | **软依赖** | 零仓库项目合法，② 可跳过（I5 §4.7） |
| ② → ③ | **软依赖** | 认知可从项目文档/信息起步；无仓库时认知材料来源不同 |
| ② → ④ | **硬依赖（内容依赖）** | 基线必须记录「基于哪个仓库哪个 Commit」，无仓库时基线为「无仓库基线」（仍合法） |
| ③ → ④ | **硬依赖** | 基线内容来自认知结果；③ 未产出时 ④ 无内容可确认 |
| ⑤ → ④ | **无依赖** | 设备就绪与基线确认互不阻塞 |
| ④ → ⑥ | **软依赖** | 交接不强制要求基线已确认（I5 §4.5 未把基线列为 required） |
| ⑤ → ⑥ | **软依赖** | 「执行就绪」是派生量，与「初始化交接」不是同一件事 |
| ④ → ② | **反向弱依赖** | 基线确认后若发现仓库基准变化，需回到 ② 重新核验（G-05） |

### 9.3 并行与乱序

| 允许 | 说明 |
| --- | --- |
| ② / ③ / ⑤ 并行 | 不同领域互不阻塞（仓库分析在跑时用户可同时绑定设备） |
| ③ 先于 ② 完成（部分） | 认知可基于已有文档先建，再由仓库分析补充 |
| ④ 反复发生 | 基线确认是一个**可重复**的阶段（V1.2 基线可演进至 V2/V3），不是一次性门禁 |
| ⑥ 随时可进入 | 交接不锁死项目；未完成事项在总控台持续跟踪 |

### 9.4 依赖表达方式

| 阶段卡片的依赖提示 | 规则 |
| --- | --- |
| 「依赖 ② 完成」 | 仅当该依赖是**硬依赖**且**当前未满足**时显示 |
| 「可并行推进」 | 软依赖阶段在满足自身条件时显示 |
| 「不适用」 | 零仓库时 ② 的代码分析子项标 `not_applicable` |
| **不得** | 用灰色/禁用遮罩暗示「不可进入」——所有阶段详情页始终可达 |

---

### 9.5 分析等待态与零依赖独立工作（非阻塞原则）

> **2026-11-05 用户确认需求增补**（引专家裁决 **R-S01-01-01 · Native AI-PM Independence**，裁决记录见 [s01-01-native-ai-pm-independence-ruling-v1.md](s01-01-native-ai-pm-independence-ruling-v1.md)）：① 项目在**未接入 Git 仓库、未接入 MCP/Codex/外部 Agent** 时，总控台与项目工作空间必须可以正常工作——录入需求等人类工作与**内置 AI-PM 能力**（AI 需求分析、AI 知识整理）照常可用；② 用户**写入 Git URL 之后，AI 进入分析，呈现等待 UI；分析可能持续一段时间**。本节把这两条固化为正式设计约束，与 I5 §4.7「独立工作模式」同源。

#### 9.5.1 分析是长时异步作业

- **事实**【EXISTING】用户提交 Git URL 后触发的是服务端作业：`repo_import` 十态 + `progress` + `stage_detail`（`migrations/0009_repo_import.sql:27-49`）；认知建立由 `discovery_session` 在后端推进（`migrations/0025_discovery.sql:18-20`）。
- 作业在后端继续执行，**与浏览器会话无关**：用户离开页面、离开项目、关闭浏览器都不中断分析。
- 前端**不得**假设分析秒级完成；也**不得**把轮询超时/网络失败推断为作业失败（作业失败以 `repo_import.status='failed'` 为准，见 §10.3）。

#### 9.5.2 等待 UI 形态（非阻塞）

| 规则 | 内容 |
| --- | --- |
| 呈现 | 阶段子项/卡片 `in_progress` + 作业进度（`progress` / `stage_detail`）+ 明确措辞「分析可能需要一段时间，期间可离开本页继续其他工作」 |
| 允许 | 随时离开本页/项目；返回后按读模型重算，显示**当前真实状态**（§10.2 场景 11），不重放等待过程 |
| 允许 | 等待期间照常进行其他一切不依赖该作业结果的工作（§9.5.3） |
| **禁止** | 模态/全屏 spinner 把用户钉在等待页；**禁止**禁用侧边导航、其他阶段卡片或任何与该作业无关的功能区 |
| **禁止** | 把「分析中」渲染成必须守候的阻塞态；等待不是 `blocked` |
| 完成/失败通知 | 通过状态跃迁（§4.3）+ Current Action / Human Actions 呈现（完成→可确认条目点亮；失败→§10.2 场景 4），不依赖 toast 竞速或强制守候 |

#### 9.5.3 等待期间与零依赖状态下照常可用的工作

与 I5 §4.7「零仓库、零设备 Project：独立工作模式」同一原则：以下工作**既不依赖**在飞的分析作业，**也不依赖**仓库/设备接入，任何时刻都不得被禁用：

- 需求录入与管理（S02 域能力）；
- Goal 与知识条目的人工录入/维护（I4）；
- 任务管理与人工待办处理；
- 基线条目的人工补充与确认（含无仓库基线，§9.2 ②→④）；
- 设备注册/绑定（⑤，§9.3 并行）。

配套呈现规则（**R-S01-01-01 裁决收编**）：

- **内置 AI-PM 能力在零外部接入时照常可用**：AI 需求分析、AI 知识整理、认知与建议只依赖产品自身的 AI 模型服务，**不要求** Repository / MCP / Codex / 外部 Agent（R-S01-01-01 §一、§三）；
- 呈现「尚未接入（可选）」的仅限**外部工程集成**两类：代码仓库分析（②）与外部 Agent 执行（⑤），各配接入入口；不得把「尚未接入」呈现为故障或阻塞；
- 零仓库时 ② 显示「尚未关联仓库（可选）」、代码分析子项 `not_applicable`（§3.2.5）；零设备时 ⑤ 显示「尚未接入执行设备（可选）」，二者均为可选状态，**不是失败、不是阻塞**（I5 §4.7）；
- **禁止**以「仓库分析进行中」或「未接入仓库/设备」为前置条件拦截上述任何入口；六阶段卡片始终可达（§9.4），分析中的 ②/③ 只影响自身子项状态。

---

## §10 异常、失败和恢复流程

### 10.1 通用失败形态

**每个可失败的阶段必须给出四元组**：

> **原因**（来自权威错误码） → **影响范围**（哪些阶段/能力受影响） → **当前可用替代路径** → **解除条件**（可验证）

**事实**【EXISTING】可用的权威错误码来源：
- `project_repository.last_error_code` ∈ {`ssh_key_missing`, `timeout`, `auth_failed`, `not_found`, `too_large`, `git_failed`}（`crates/pm-application/src/repository/repo.rs:264,314-320`）
- `repo_import.error_code` / `error_detail`（`migrations/0009_repo_import.sql:41-42`）
- `repo_import_source_attempt.error_code` / `error_detail`（`migrations/0031_...:82-106`）
- `device.last_check.evidence`（`devenv/checks.rs:93-100`）

### 10.2 场景表

| # | 场景 | 判定（权威依据） | 本页呈现 | 恢复路径 |
| --- | --- | --- | --- | --- |
| 1 | 刚创建、尚未配置仓库 | `project_repository` 计数=0 且无 import 作业 | ② 显示「尚未关联仓库（可选）」；代码分析 `not_applicable`；**不判失败**；当前动作 = 建议关联仓库 | 引导到 S01-03 添加仓库 |
| 2 | 多仓库但无 Primary | 无 `role='main'` 或 ≥2 个 | ② 显示「未指定主仓库（可选）」+ 建议标记；**不得**显示为错误 | 到 S01-03 指派（G-03 落地前为人工约定） |
| 3 | 仓库授权失败 | `project_repository.status='unreachable'` 或 `last_error_code ∈ {auth_failed, ssh_key_missing}` | ② `blocked`；Current Action = 该项；显示错误码对应原因与解除条件（配置/更换 SSH Key） | `POST /v1/pm/projects/{p}/ssh-key`（生成/更换）→ `POST /v1/pm/repositories/{id}/verify` 重试 |
| 4 | AI-PM 认知执行失败 | `repo_import.status='failed'` 或 session 异常终止 | ② `blocked`（代码分析）；③ 不受影响或 `not_started`；**错误原因与修复动作不同**（见下） | 重试 import（`POST .../imports`）；历史失败记录保留但不永久判失败 |
| 5 | AI 认知结果等待确认 | `discovery_finding.verification_status='PENDING'` 或 proposal `pending` | ③ `waiting_for_human`（待裁决 G-01）；④ 显示「待确认 N 项」；Human Actions 出现对应条目 | 进入 S01-04 逐项确认（finding 确认端点已存在） |
| 6 | 基线存在冲突 | finding `CONFLICT` 或 proposal `conflict` | ④ `blocked`；显示冲突双方来源与版本（S01-05 §七：「知识存在冲突 → 展示两个来源及其版本」） | 人类裁决冲突；不得由 AI 自行消解 |
| 7 | 用户拒绝 AI 提案 | finding `REJECTED` / proposal `rejected` | ④ **不判失败**；显示「已拒绝」+ 拒绝记录；允许「要求重新生成」 | 重生成走领域端点；拒绝历史保留供后续参考（S01-04 §6.4） |
| 8 | Codex/MCP 初始化失败 | `device.readiness='failed'` 或 `status='revoked'` | ⑤ `blocked`；展示 `last_check.evidence` 摘要 + 解除条件（重新安装/重新授权/重绑） | `POST .../checks` 重检；`POST .../revoke` 后重新注册 |
| 9 | 外部依赖未满足 | 无对应领域字段 | ⑤ `undetermined`（设备从未自检）；**不得**显示失败 | 引导执行自检；显示「就绪条件尚未核验」 |
| 10 | 多阶段处于不同状态 | 各阶段独立派生 | 六阶段卡片各显各态；Current Action 按阶梯选唯一一项 + 「另有 N 项」 | —— |
| 11 | 用户离开后重新进入 | 页面按读模型重算 | 恢复后显示**当前真实状态**；不重放历史动作；已确认事项不再出现在待办 | —— |
| 12 | 页面加载失败 / 无权限 | 端点 403/5xx；区块 `status='unavailable'` | 整页降级：保留壳与项目身份，说明原因；分块失败只塌该块 + Retry + 指标 `—`/`Unavailable`（S01-05 §D-c）；**不得**显示为「无待办」 | 重试 / 申请权限 / 返回项目列表 |

### 10.3 「仓库分析失败」与「仓库授权失败」必须可区分

两者在 UI 上共享 `blocked`，但**原因与解除条件完全不同**：

| 维度 | 授权失败 | 分析失败 |
| --- | --- | --- |
| 错误码 | `auth_failed` / `ssh_key_missing` | `git_failed` / `too_large` / `timeout` |
| 影响 | 无法读取仓库 → 分析无法开始 | 已能读取 → 读取/分析阶段失败 |
| 解除条件 | 配置/更换凭证 | 重试作业 / 调整范围 |
| 恢复入口 | SSH Key 管理 | 重新发起 import |

**禁止**用一个统一 `Failed` chip 覆盖二者。

### 10.4 恢复路径的统一形态

```
失败发生 ──► 展示：原因（错误码 + 详情） + 影响范围 + 解除条件
        ──► 提供：恢复动作（可执行，重试/更换/重建）
        ──► 保留：失败记录与历史（可回看）
        ──► 禁止：自动把状态刷成成功、掩盖失败记录
```

---

## §11 权限与审计要求

### 11.1 读取权限

| # | 规则 | 事实依据 |
| --- | --- | --- |
| P-1 | 确认调用者对目标 Project 有读取权限 | I5 §7.6 |
| P-2 | 每个数据源继续遵守原领域 Tenant / Project 边界（`tenant_id` + `project_id` 复合约束已 pervasive） | 如 `migrations/0019_tenant.sql:331-333` |
| P-3 | 每条聚合记录必须能证明属于当前 Project | I5 §7.6.3；`discovery_session`/`finding` 均已带 `project_id`（`migrations/0031_...:112-117`） |
| P-4 | 不允许通过传入 Project ID 拼接其他项目数据 | I5 §7.6.4 |
| P-5 | 无权限时整页降级，不做局部隐藏造成误判 | S01-05 §1.2「Permission-aware」 |

### 11.2 写与确认权限

| # | 规则 | 事实依据 |
| --- | --- | --- |
| P-6 | 确认类动作**服务端二次校验**身份与归属 | 【EXISTING】`discovery/web.rs:200-226`（human + 归属 session + 未归档） |
| P-7 | 本页**不提供**任何绕过领域服务的写入口 | I5 §7.4 / §8.1 |
| P-8 | 可见性 ≠ 授权；按钮隐藏不构成安全边界 | 基线确认规范 §7.1 |
| P-9 | Agent 角色不得执行人类确认 | `core.principal.kind` 判定 |
| P-10 | 项目归档后本页全只读 | 见 §11.3 |

### 11.3 归档只读

**事实**【EXISTING】`core.project.status = 'archived'` 由 `archive.rs:78` 写入；消费方门禁 `project_gate.rs:52-56,70-74,94-98` 对 `archived` 特判；归档时仓库 import 被终止（`project_import/lifecycle.rs:98-101`）、初始化操作被 release（`archive.rs:108-111`）。

| 规则 | 行为 |
| --- | --- |
| A-1 | 归档项目：页面顶部显示「项目已归档」横幅（`Alert` variant=warning），所有写操作禁用 |
| A-2 | 归档项目：仍可查看完整历史（不隐藏事实） |
| A-3 | 归档项目：Stage 6 显示为「项目已归档」，不显示交接按钮 |
| A-4 | 归档项目：AI Context Panel 只读 |

### 11.4 审计要求

| 事件 | 要求 |
| --- | --- |
| 人类确认 | 记录确认人、时间、来源记录 ID、来源提案版本、操作前后状态（`governance.audit_log`） |
| 授权动作 | Provider 授权、SSH Key 生成/轮换/撤销、设备绑定/解绑/吊销 |
| 恢复动作 | 重试 import、重新核验仓库、重新生成提案 |
| 归档 | `action='project_archived'` |
| 聚合查询 | 聚合本身**不产生**业务写记录；只有领域动作产生审计 |

**要求**：`governance.human_intervention`【EXISTING】是**已行使的人类权力**的追加式审计（`intervention_type` 为封闭七值枚举，`migrations/0003_platform.sql:173-178`），**不是**待办队列；`governance.approval_request`【EXISTING】与 `ai.change_proposal` 是 **1:1**（`proposals_id UNIQUE`，`migrations/0003_platform.sql:160-161`），**不能**承载通用待办。二者均不得被本页当作待办存储。

---

## §12 与相邻文档的边界

### 12.1 边界总表

| 相邻对象 | 归属文档 | 本页**只做** | 本页**不做** |
| --- | --- | --- | --- |
| **S01-02 Project Creation** | `s01-02-project-creation-ux-v1-1.md` | 回显创建结果（名称/标识/负责人/创建时间/角色）；提供下钻 | 创建表单、Key 生成、Owner 指派、AI 辅助创建、创建幂等/失败处理 |
| **S01-03 Repository Management** | `s01-03-repository-management-ux-v1-1.md` | 仓库关联数/角色/核验态/分析作业态；变更差异提示；下钻 | 添加/移除、Provider 授权、角色与 Primary 指派、版本核验、HEAD 变更处理、长期仓库管理 |
| **S01-04 AI-PM Cognition** | `s01-04-ai-pm-cognition-ux-v1-1.md` | 认知四层事实聚合；待确认/冲突计数；下钻 | 六阶段认知流程、Preparation、Run/Item 生命周期、逐项审核、Knowledge 更新、Agent Context 检视 |
| **基线确认规范** | `s01-01-project-baseline-confirmation-ux-v1.md`（编号见 §0.3） | 基线阶段状态、待确认/已确认/冲突聚合计数、下钻 | 提案全文、版本差异、风险裁决、二次确认、版本历史、并发令牌 |
| **I5 Project Command Center** | `smark-pm-s01-i5-project-command-center-v1-1.md` | 初始化**阶段化展开**视图 | 总控台的日常管理职责（工作汇总、运行、风险、活动、目标） |
| **Codex/MCP Bootstrap 规范** | 未建 | 设备接入态与阻塞摘要；下钻 | 安装引导、授权流程、绑定与自检操作界面 |
| **Personal Home / Workspace（S01-05）** | `s01-05-personal-*-ux-*.md` | —— | 跨项目个人视图、Inbox、Watchlist、活动流 |

### 12.2 与 I5 总控台的边界（最易混淆处）

| 维度 | S01-01 初始化总览 | I5 总控台 |
| --- | --- | --- |
| 服务对象 | **一次性的、可中断的**初始化过程 | **长期日常**的项目管理 |
| 生命周期 | Project 创建后 → 交接前 | 交接后常驻 |
| 首屏第一块 | Current Action | 待人工处理 |
| 初始化进度 | **展开为六阶段** | **收缩为事项列表**（I5 §4.6） |
| 数据来源 | **同一份 `initialization_progress` 派生** | 同左 |
| 是否新增事实存储 | 否 | 否（I5 §6.5） |
| 关系 | 交接后由总控台接管；本页保留补做入口 | 接管后仍可回到本页 |

**强制约束**：两页**不得**各自实现一套初始化状态派生。若本页需要比 I5 更细的阶段粒度，应在**共享派生模块**上扩展，而不是在页面层重新计算。

### 12.3 与 I4 Project Goal 的边界（术语纪律）

| 概念 | 归属 | 载体 | 本页措辞 |
| --- | --- | --- | --- |
| **Project Goal**（项目目标） | I4 | Knowledge + `doc_key='project-goal'` | 「项目目标」 |
| **Project Baseline**（项目基线） | 基线确认规范 | 派生自 discovery/proposal（无独立表） | 「基线」「首次基线」 |
| **Cognition Item**（认知条目） | S01-04 | `discovery_finding` / proposal | 「认知结果」 |

**禁止**用「基线」指代 Goal 或认知结果——三者生命周期、确认主体与权威来源均不同。

### 12.4 不重复建设清单

| # | 不重复建设 | 依据 |
| --- | --- | --- |
| 1 | 不新建初始化状态表/状态机 | S01 V1.2 §8「明确不新增 10 项」 |
| 2 | 不新建初始化读模型（复用 command-center 派生） | I5 §6.5「不建立第二套事实存储」 |
| 3 | 不新建人工待办表（复用读时派生并集） | `command_center.rs:975-1006` |
| 4 | 不新建基线实体（复用提案管线 + 派生判断） | S01 V1.2 Q3 |
| 5 | 不新建仓库实体（保持项目级关联） | S01 V1.2 Q1 |
| 6 | 不新建 Primary Repository 列（G-03 待裁决） | 后端现状 |
| 7 | 不新建 Git Provider 授权表（G-04 待裁决） | 后端现状 |
| 8 | 不新增领域状态枚举（除展示层第七态 G-01） | 展示层 ≠ 领域层（I5 §4.4） |
| 9 | 不新增 Workspace / WorkspaceMember | 提示词 §二 |
| 10 | 不合并 `ai.document_proposal`（8 态）与 `discovery.discovery_proposal`（4 态）职责 | 两者是不同状态机（S01 V1.2 Q5 / I5 §6.2） |

---

## §13 页面验收标准（Acceptance Criteria）

> 每条均可被测试用例或代码断言验证。标注 **[现状]** 的条目表示当前代码已满足；标注 **[待实施]** 的需要开发。

### A. 定位与边界

- [ ] **[待实施]** 项目创建成功后进入本页面，不停留在创建表单（S01-02 §7.1.7）
- [ ] **[待实施]** 本页不占用长期主导航位；初始化完成后由总控台接管
- [ ] **[待实施]** 页面内无第二套项目创建/仓库接入/认知审核表单
- [ ] **[待实施]** 页面不含 AI 聊天输入框（AI Context Panel 无输入能力）

### B. 阶段与状态

- [ ] **[待实施]** 六个阶段全部可见，状态各自独立，允许多阶段同时处于不同状态
- [ ] **[待实施]** 每个阶段状态可回溯到 `source_state` / 权威记录
- [ ] **[待实施]** 状态徽标**同时**带文字与图标（不靠颜色单独区分）
- [ ] **[待实施]** 「等待人类」与「存在阻塞」在文案与图标上可区分
- [ ] **[待实施]** 页面**不出现**任何初始化百分比
- [ ] **[待实施]** `completion == 'not_determinable'` 时页面**不出现**「初始化完成」
- [ ] **[待实施]** 零仓库项目的代码分析标 `not_applicable`，且不判失败、不阻塞其他阶段
- [ ] **[待实施]** 已完成阶段持续可见，不因后续阶段推进而消失
- [ ] **[待实施]** 阶段间依赖以文字标注，不使用禁用遮罩

### C. Current Action

- [ ] **[待实施]** 首屏第一视觉块是 Current Action
- [ ] **[待实施]** Current Action 唯一（不是列表），并给出「另有 N 项」余量
- [ ] **[待实施]** 责任主体明确显示为 Human / AI Agent / System 之一，且 AI 主体附 `source_state`
- [ ] **[待实施]** 阻塞时给出原因、影响范围、解除条件三要素
- [ ] **[待实施]** 无待办时显示明确文案 + 交接出口，不留空

### D. Human Actions

- [ ] **[待实施]** 每条待办带：类型、标题、来源领域、来源记录标识、更新时间、动作入口、下钻入口
- [ ] **[待实施]** 普通通知、Agent 建议、信息同步**不出现在**待办区
- [ ] **[待实施]** 待办排序稳定且可复算（风险 → 来源 → 时间）
- [ ] **[待实施]** 待办区点击**只导航**，不直接改业务状态
- [ ] **[待实施]** 待办区加载失败显示错误块 + Retry，**不显示**「没有待办」

### E. AI Context Panel

- [ ] **[待实施]** 面板仅含五类信息（§7.2），无输入框、无确认按钮
- [ ] **[待实施]** 「已成立事实 / AI 产出待确认 / 执行中」三区分离，视觉通道不同（实心 vs 描边）
- [ ] **[待实施]** AI 事件**不出现** `circle-check` 图标
- [ ] **[待实施]** 面板加载失败只塌陷该区，不影响主区

### F. 人类确认

- [ ] **[现状]** 确认动作由服务端校验 `core.principal.kind='human'`（`discovery/web.rs:200-209`）
- [ ] **[待实施]** Agent 身份调用确认端点被拒
- [ ] **[待实施]** 确认为二次确认语义，含并发令牌与幂等键
- [ ] **[待实施]** 冲突返回明确 409，不静默覆盖
- [ ] **[待实施]** 拒绝与暂缓保留历史与证据

### G. 异常与恢复

- [ ] **[待实施]** 12 个场景逐条可复现并符合 §10.2 的「判定 / 呈现 / 恢复」
- [ ] **[待实施]** 授权失败与分析失败在文案、错误码、恢复入口上可区分
- [ ] **[待实施]** 用户拒绝 AI 提案**不**导致阶段判为失败
- [ ] **[待实施]** 历史失败记录不导致永久判失败（最新有效结果优先）
- [ ] **[待实施]** 每条失败均可展开看到原因与恢复动作

### H. 权限与审计

- [ ] **[待实施]** 非项目成员访问被拒（整页降级）
- [ ] **[待实施]** 归档项目全页只读且历史可读
- [ ] **[待实施]** 每次人类确认在审计中可反查确认人/时间/来源版本

### I. 视觉与可访问性

- [ ] **[待实施]** 遵循设计系统令牌（`--ws-*`）与三栏基线
- [ ] **[待实施]** 全部可点元素为原生可聚焦元素，焦点环可见
- [ ] **[待实施]** 深/浅主题均满足状态可区分（灰度可分辨）
- [ ] **[待实施]** 状态不单独依赖颜色传达（WCAG 1.4.1）

---

## §14 需要进一步代码事实核验的接口与数据缺口

### 14.1 缺口总表

| # | 缺口 | 级别 | 事实证据 | 影响 | 建议动作 |
| --- | --- | --- | --- | --- | --- |
| **G-01** | 展示层是否新增第七态 `waiting_for_human` | **P0（已裁决·已实施 2026-11-04）** | 后端 `init_status` 原六态；前端孤儿词表已有该值但零引用 | 「等待人类」与「AI 执行中」「阻塞」无法区分 | **裁决=方案 A 合并七值**（`waiting_for_human` 严格限定为 InitItem 展示层状态，不进任何领域实体；孤儿 `InitializationStatus` 废弃）。已前后端同批实施：后端 `init_status::WAITING_FOR_HUMAN` + `init_judgement` 纯函数判定（15 单测）+ HTTP 测试 2 条；前端 `InitItemStatus` 扩展 + `pm.commandCenter.init.waitingForHuman` + 孤儿类型 @deprecated。**无迁移** |
| **G-02** | `project_basics.source_state` 硬编码 `"planning"` | P1 | `command_center.rs:634-637` vs `core.project.status` 实际值（`planning/initializing/archived`） | 阶段卡片的权威来源与真实生命周期不符 | 改为透传真实 `core.project.status` |
| **G-03** | Primary Repository 的持久化落点 | **P0** | 后端无 `is_primary`/`primary_repository_id` 列与代码；前端已假定存在（`types/initialization.ts:69`、`initialization.ts:170-174`） | S01-03 §3.2「用户可通过受控操作指定或更换 Primary」当前无落点 | 裁决：由 `role='main'` 派生（则多值需约束）还是新增列；并统一前端/后端角色词表（`primary\|supporting\|infrastructure\|test` vs `main\|docs\|service\|infra`） |
| **G-04** | Git Provider OAuth / App 授权的数据与端点 | **P0** | 无表无路由；前端已写 `listGitProviders` 等三调用（`initialization.ts:200-222`） | S01-03 §5.2「优先 Provider OAuth」无法落地；「仓库授权」待办无来源 | 裁决是否在 V1 落地 OAuth；否则总览页须删除该类待办 |
| **G-05** | 「人类确认仓库基准更新」的记录承载 | P1 | 有 `project_repository.head_commit_sha`、`repo_import_source.head_commit_sha`、verify 端点；**无**确认记录 | 「仓库变化由人类确认」无审计、无幂等、无状态 | 裁决：复用审计表 + 领域动作，还是新增确认记录 |
| **G-06** | `project_cognition` 的 completed 判定把「分析完成」当「认知完成」 | **P0** | `command_center.rs:694-707`；违反 S01-04 §5.2 | 直接违反「不将 AI 推断当正式事实」 | 按 §3.3.1 修正派生；引入 waiting_human 后可正确区分 |
| **G-07** | `ai.document_proposal` 的 `confirmed`/`quality_gates_passed` 从未被写入 | P1 | 仅读侧常量（`command_center.rs:67,69`；`review/knowledge.rs:20,22`） | 以 `confirmed` 判定「人类已确认」会永远为假 | 确认正式写入路径；或统一改用 `committed` 判定 |
| **G-08** | 基线实体、基线端点、基线确认记录 | **P0（2026-11-05 核验勘误：实体与路由已在）** | 无 `project_baseline*` 表、无 baseline 路由、`core.project` 无 `current_baseline_id` → **2026-11-05 核验更正：`ai.baseline_proposal`/`ai.baseline_item`/`ai.project_baseline`（`migrations/0037_s01_baseline.sql`）与 `pm_baseline::router()`（`routes.rs:205`）现已存在，「无表无路由」为过期快照** | Stage 4 的「查看详情并进行确认的入口」无后端目标 → 已有后端目标 | 裁决基线实体化 vs 保持派生 → 实体化已落地；余下细项（确认权限校验是否满足 R-BL-06=B、`core.project.current_baseline_id` 有无）按[统一核验清单](s01-technical-verification-unified-checklist-v1.md) U-B01/U-B05 继续读码 |
| **G-09** | MCP 连接状态的只读来源 | P1 | pm-api 无 MCP 状态端点；`/install/{tool}` 是引导文档；前端 `BootstrapInfo` 无 fetch | Stage 5「MCP 配置与连接状态」无源 | 裁决：新增只读状态读模型，或在 UI 中明示「暂无可用状态源」 |
| **G-10** | `discovery_proposal` 决策无 UI 入口 | P1 | 仅 MCP 写入（`pm-mcp/src/tools.rs:1309,1343`；`server.rs:340`） | 「人类已确认」的一部分无法由 UI 产生 | 裁决：补 REST 端点，或明确该类提案不经 UI 确认 |
| **G-11** | `project_repository.ssh_key_id` 从未被写入 | P2 | `migrations/0009_repo_import.sql:15` 有列，全仓无 `SET ssh_key_id` | 「仓库授权状态」无法按仓储关联展示 | 核验是否为遗留列；决定补写或废弃 |
| **G-12** | `command_center` 的 `session_open` 判定口径未明 | P2 | `command_center.rs:697` 依赖 `session_open` 计数，其 SQL 未在本稿核验范围 | `project_cognition` 的 in_progress/completed 切点可能不准 | 核验该计数的具体定义 |
| **G-13** | 前端无任何初始化相关视图 | P1 | `ai-frontend/src/views/` 仅 auth/design/notifications/placeholder/settings；全部项目路由渲染 `PlaceholderView`（`router/index.ts:49,58`） | 本页是新建页面，非改造 | 实施时新增视图 + 路由 + mock fixture |
| **G-14** | 缺少通用 stage/stepper 组件 | P1（**已规避**） | `ui/` 无 timeline/steps；仅 `components/auth/AuthProgressSteps.vue`（三态 done/current/pending） | 六阶段卡片需自建或提升该组件 | **实施取法**：六阶段用**独立卡片网格**（`repeat(auto-fill, minmax(260px,1fr))`）而非线性 stepper——六阶段非线性流程，stepper 会误导为强制顺序；不新建通用 stepper |
| **G-15** | 两套 i18n 状态词表并存且均为孤儿 | P2 | `pm.commandCenter.init.*`（6 态）与 `s01.initialization.state.*`（5 态含 waiting）均零组件引用 | 实施时可能接错一套 | 裁决主词表；另一套删除或降为别名 |
| **G-16** | mock 层缺 command-center / discovery / devices / imports 夹具 | P1（**部分已实施**） | `api/mock/` 无对应夹具；`VITE_USE_MOCK=1`（`.env.development:19`）下这些端点 404 | 本页在 mock 模式下无法自测 | **已实施**：`ai-frontend/src/api/mock/command-center.ts`（按后端派生规则构造六块夹具，已接入 mock-plugin）；**仍缺**：discovery / devices / imports 夹具 |
| **G-17** | `delivery.task` 的待验收是否属于初始化范畴 | P2 | `command_center.rs:975-1006` 的并集含 `delivery.task ∈ {submitted, reviewing}` | 初始化总览的待办区若直接复用聚合，会混入非初始化事项 | 裁决：初始化总览是否只取初始化相关来源子集 |
| **G-18** | `human_actions` 作为 required 事项的合理性 | P2 | `command_center.rs:735` 硬编码 `required=true`；该集是跨领域的 | Stage 6 交接判定被非初始化事项影响 | 与 G-17 一并裁决：拆分 required 子集 |
| **G-19** | 项目级显式能力授权粒度 | P2 | `core.project_member.role` 只有 6 个成员角色（`0030_project_member_role.sql:7-9`）；无细粒度能力授予 | 「谁能正式确认」目前只能靠 `owner`/治理角色近似 | 核验 I6-A 权限基座的授权粒度结论，必要时引用 |
| **G-20** | 归档门禁对读路径的覆盖 | P2 | `project_gate.rs` 对 `archived` 特判（写路径为主）；读路径是否仍放行待核 | 归档后本页是否应彻底只读 | 核验读路径门禁 |
| **G-21** | 后端 `activity_type` 与前端文案词表不一致 | P1 | 后端活动类型为 snake_case（`analysis_succeeded` / `analysis_failed` / `repository_verified` / `run_finished` …，`crates/pm-application/src/command_center.rs:28-54`）；前端 `pm.commandCenter.activity.*` 是 camelCase（`analysisSucceeded` 等） | 前端按 camelCase 查表会全部落空 | 本页已按**后端 snake_case** 自建 `aiContext.activity.*` 词表；全局统一口径待另立任务 |
| **G-22** | 活动流**只记录已发生的动作**，无「开始」事件 | P1 | 后端 `ACTIVITY_TYPE_BY_ACTION` 全是完成型/变更型动作（`command_center.rs:28-54`） | 「AI 正在做什么」不能从活动流推断，否则会把已完成显示成执行中 | **已实施**：本页「AI 执行中」改由**阶段派生**（作业状态 `source_state`）产出；「最近完成」才取活动流 |
| **G-23** | `ui/descriptions` 的 `columns` 语义与实现相反 | P3 | 原实现把每条 `grid-column: span columns`，`columns=2` 时每条占满整行 | 本页项目身份区被拉成 6 行 | **已修复**（该组件当时无其他消费方）：改为按列流排，`columns` 真正决定栅格列数 |

### 14.2 本稿建议的最小后端补充（非强制裁决项）

按「不创造未经确认的后端 API」的约束，本稿**不提出**新的对外端点。仅列出**若要支撑本稿完整形态**所必需的最小内部补充，供实施设计阶段裁决：

| 优先级 | 补充 | 形式 |
| --- | --- | --- |
| 1 | 共享初始化派生模块 | **内部重构**：把 `command_center.rs` 的初始化派生抽为可复用函数，供总控台与本页共用（不是新端点） |
| 2 | `baseline_confirmation` 事项派生 | **内部派生**：基于 discovery/proposal 的复合判定（不是新表） |
| 3 | `project_cognition` 派生修正 | **内部修正**：按 §3.3.1 重写判定 |
| 4 | `waiting_for_human` 展示态 | **内部常量扩展**：后端 `init_status` + 前端类型 + i18n |
| 5 | 初始化范围内的待办子集 | **内部查询参数**：`human_action_summary` 增加 scope 过滤（不是新端点） |

### 14.3 前端实施缺口

| 缺口 | 现状 | 建议 |
| --- | --- | --- |
| 视图与路由 | 无 | 新增 `views/project/initialization/`，复用壳级 TopBar/SideNav/CommandBar |
| 阶段卡片组件 | 无 | **已实施**：`views/project/initialization/components/InitStageCard.vue`（卡片网格，非 stepper，见 G-14） |
| 状态徽标映射 | `ui/badge` + `ui/status-indicator` 可用 | 复用；`statusVariant`（`api/status.ts:14`）已有映射先例 |
| 命令中心先例 | `frontend/src/shell/views/pm/command-center/index.vue`（479 行）**已实现**六区块总控台，含初始化进度卡（`:336-360`）、状态映射（`:75-86,109-119`） | **未移植**：`ai-frontend/AGENTS.md` §二明确「不要从旧目录恢复或参照旧页面实现」；本页按设计系统与 `ui/` 基座独立实现 |
| i18n | 已有 `pm.commandCenter.init.*`（孤儿） | 接入并补 `waitingForHuman` |
| 零依赖独立工作 + 分析等待非阻塞（§9.5） | **实测反馈（2026-11-05，另会话）：未接入仓库/设备时项目工作空间疑似不可用（需求录入等受阻）；分析等待态疑似阻塞**。需核验 | 按 §9.5 落实：零仓库/零设备=可选状态不拦截入口；Git URL 提交后分析走长时异步等待 UI，禁止模态/全屏阻塞，等待期间其余工作全部可用 |

### 14.4 建议的核验/实施任务拆分

| 任务 | 内容 | 前置 |
| --- | --- | --- |
| **S01-01-A** | G-01 / G-02 / G-03 / G-06 / G-17 / G-18 裁决（口径类，不写码） | 用户裁决 |
| **S01-01-B** | 共享派生模块重构 + `baseline_confirmation` 事项 + `project_cognition` 修正 | A |
| **S01-01-C** | 前端页面骨架 + 六阶段卡片 + 状态映射 + i18n | B（若 B 未完成则先按现有六态实现） |
| **S01-01-D** | Current Action / Human Actions / AI Context Panel 三区 | C |
| **S01-01-E** | 异常态与恢复路径（12 场景） | D |
| **S01-01-F** | mock 夹具与端到端自测（G-16） | D（**夹具已实施**） |

---

## 附录 A 术语表

| 术语 | 定义 | 权威来源 |
| --- | --- | --- |
| Project | 长期业务实体 | `core.project` |
| Initialization Overview | 本页；项目创建后至交接前的阶段总览视图 | 本稿 |
| 阶段（Stage） | 六阶段，人类阅读分组；**非**持久化状态机 | 本稿 §0.4-C-1 |
| 事项（Init Item） | 派生出的初始化条目；`project_basics`…`human_actions` | `command_center.rs:630-762` |
| 展示状态 | 六/七种**展示层**状态；非领域状态 | `init_status`（§4.1） |
| 派生 | 服务端按规则从权威数据算出的展示值 | I5 §4.4 |
| 正式事实 | 经人类确认并由系统写入的、可审计的内容 | I2 / S01 V1.3 |
| 人类确认 | 由 `core.principal.kind='human'` 执行并被审计的决定 | `migrations/0001_core.sql:33-35` |
| 待人工处理 | 满足 R-H1~R-H4 四条的事项 | §6.1 |
| 交接 | Stage 6；进入正常研发管理的出口；**不改变任何领域状态** | §3.6 |
| Project Goal | 项目目标（I4），**不是**基线 | §12.3 |
| Baseline | 首次正式基线；派生自 discovery/proposal 确认结果 | S01 V1.2 Q3 |
| Execution Ready | 派生量（设备 ready × 绑定仓库已验证），**不是**第七种设备状态 | T-088 P1-1 |

## 附录 B 关键代码与文档索引

### B.1 后端

| 主题 | 位置 |
| --- | --- |
| 总控台端点 | `apps/pm-api/src/command_center.rs:37-40`（路由）、`:43`（handler） |
| 初始化派生 | `crates/pm-application/src/command_center.rs:630-762`（items）、`:74-81`（状态常量）、`:975-1006`（待办并集）、`:1008+`（待办明细） |
| 项目与成员 | `migrations/0001_core.sql:24`（status）、`migrations/0030_project_member_role.sql:7-9`（角色 CHECK）、`crates/pm-application/src/project_member.rs:398-454`（Owner 转移） |
| 身份 | `migrations/0001_core.sql:33-35`（principal.kind） |
| 仓库 | `migrations/0008_repositories.sql:18-32`、`migrations/0009_repo_import.sql:15-21`、`crates/pm-application/src/repository/repo.rs:261-340` |
| 仓库作业 | `migrations/0009_repo_import.sql:27-49`、`crates/pm-application/src/project_import/lifecycle.rs:180-810`、`proposal.rs:365`、`advance.rs:55-73` |
| 发现会话 | `migrations/0025_discovery.sql:12-26`、`crates/pm-application/src/discovery/session.rs:125-176`、`web.rs:41` |
| Finding | `migrations/0025_discovery.sql:48-62`、`crates/pm-application/src/discovery/finding.rs:398-643`、`web.rs:120-321`、`apps/pm-api/src/pm_discovery.rs:43-46` |
| 提案双链 | `migrations/0014_knowledge_v2.sql:38-41`、`migrations/0025_discovery.sql:123-132`、`crates/pm-mcp/src/tools.rs:1294-1364` |
| 设备 | `migrations/0027_devenv.sql:23-95`、`crates/pm-application/src/devenv/checks.rs:93-164`、`binding.rs:26-152`、`apps/pm-api/src/pm_devices.rs:34-42` |
| 治理 | `migrations/0003_platform.sql:157-186` |
| 归档 | `crates/pm-application/src/archive.rs:78-111`、`crates/pm-application/src/project_gate.rs:52-98` |

### B.2 前端

| 主题 | 位置 |
| --- | --- |
| 读模型类型 | `ai-frontend/src/api/command-center.ts:15-203`（`InitItem` `:52`、`InitItemStatus` `:44-50`、`InitializationProgress` `:61`、`ActionItem` `:130`、`Block<T>` `:15`） |
| 未实现契约 | `ai-frontend/src/api/initialization.ts`（494 行）、`ai-frontend/src/api/types/initialization.ts`（443 行） |
| 领域 API | `api/repositories.ts`、`api/imports.ts`、`api/discovery.ts`、`api/documents.ts`、`api/devices.ts` |
| UI 套件 | `ai-frontend/src/components/ui/*`（51 个族）、`components/global/*`、`components/auth/AuthProgressSteps.vue` |
| i18n | `ai-frontend/locales/{en,zh-CN}/pm.yaml:331-410`、`s01/initialization.yaml`、`workspace.yaml:354-419` |
| 路由 | `ai-frontend/src/router/index.ts:41-58,65-137`、`router/navigation.ts:72-174` |
| Mock | `ai-frontend/build/mock-plugin.ts`、`src/api/mock/fixtures.ts`、`src/api/mock/initialization.ts`、`.env.development:19` |
| 先例实现 | `frontend/src/shell/views/pm/command-center/index.vue`（479 行，六区块 + 初始化进度卡） |

### B.3 设计文档

| 文档 | 路径 |
| --- | --- |
| S01 最终基线 V1.2 | `docs/s01-全局主菜单与信息架构设计基线/ui/smark-pm-s01-final-baseline-v1-2.md` |
| S01 最终基线 V1.3 | `docs/s01-全局主菜单与信息架构设计基线/ui/smark-pm-s01-v1-3-revision-and-i1-verification-已经过审.md` |
| I5 总控台 V1.1 | `docs/s01-全局主菜单与信息架构设计基线/ui/smark-pm-s01-i5-project-command-center-v1-1.md` |
| I2 Finding 人工确认 V1.1 | `docs/s01-全局主菜单与信息架构设计基线/ui/smark-pm-s01-i2-finding-human-confirmation-v1-1.md` |
| I4 Project Goal | `docs/s01-全局主菜单与信息架构设计基线/ui/smark-pm-s01-i4-project-goal-knowledge-v1.md` |
| 设计系统 V1.0 | `docs/ai-pm-ui-design-system-v1.md` |
| 用户场景交互模型 | `docs/ui/smark-pm-user-scenario-interaction-model-v1.md` |
| S01-02 / S01-03 / S01-04 / 基线确认 | `docs/s01-全局主菜单与信息架构设计基线/ui/s01-02-*`、`s01-03-*`、`s01-04-*`、`s01-01-project-baseline-confirmation-ux-v1.md` |
| S01-05 高保真（视觉纪律 D-a/b/c 源） | `docs/s01-全局主菜单与信息架构设计基线/ui/s01-05-personal-home-high-fidelity-ui-v1.md:44-58,215-235` |

---

> **本稿状态**：待评审。
> **评审后进入**：S01-01-A 口径裁决（§14.4）→ S01-01-B 后端派生重构 → S01-01-C/D 前端页面。
> **本稿不启动实施**：按 S01 既有流程，实施设计需独立确认。
