# PM-AI 全局导航 IA 现状审计报告 V1.0

> 审计对象：`ai-frontend`（Vue 3 + vue-router）当前仓库真实代码
> 审计日期：2026-11-16
> 审计性质：**只读现状提取**。未修改任何代码、路由、菜单或数据库。
> 版本：V1.0（事实基线，供《PM-AI 全局导航 IA 基线》制定使用）

## 0. 阅读约定

| 标记 | 含义 |
|---|---|
| 【事实】 | 代码/文档可直接验证的陈述 |
| 【证据】 | `文件:行号` 或组件/函数名 |
| 【判断】 | 基于证据的推论，可被推翻 |
| 【建议】 | 后续 IA 评审可采用的处置方向，不构成裁决 |
| 【待裁决】 | 必须由产品负责人决定的问题（汇总于 D-08） |
| UNKNOWN | 证据不足，未推断补全 |

实现状态口径（任务书 §3.3）：`IMPLEMENTED` / `PARTIAL` / `SHELL` / `STUB` / `PLANNED` / `UNKNOWN`。

> 重要前提：本报告中的「一级菜单」指**导航项**（可点击目的地），「分组」指左栏分节标题。二者数量不同，混用会导致统计口径失真。

---

## 1. 执行摘要

【事实】当前全局导航由**单一数据源**派生：`ai-frontend/src/router/navigation.ts` 定义 `navGroups`，`ai-frontend/src/router/index.ts` 的 `buildRoutes()` 由导航树生成路由，再加手工补充路由。菜单与路由不存在漂移风险（这是本仓库最健康的结构特征）。

【事实】导航树共 **4 个分组、16 个导航项**；**导航树级二级菜单为 0**（`NavChild` 类型存在但无任何 `item.children`）；左栏项目上下文头已被**注释停用**。

【事实】四个知识/分析领域在前端的落点高度重叠：
- `KB`、`Product Cognition(PC)`、`Analysis Reports` 三个页面全部塞进**同一个路由** `/project/knowledge` 的三个页签；
- `Project Cognition` 有独立页面 `/project/cognition`，但**无任何菜单入口、无任何页内链接**（孤立路由）；
- `Project Baseline` 同为零入口孤立路由；
- `Analysis` 没有独立菜单，散落在 Knowledge 页签、技术设计「分析」子页、人工审核中心、项目总控台初始化卡四处。

【判断】当前导航的主要矛盾不是「菜单太少」，而是**领域权威边界在 UI 上被抹平**：KB / PC / Analysis / Project Cognition 在 UI 层被合并为「知识审阅」一个入口，与 `docs/architecture/four-domain/authority-matrix-v1.md` 确立的四域独立权威直接冲突。

【事实】前端**没有任何基于角色的菜单可见性控制**：`SideNav.vue` 对 `navGroups` 全量 `v-for` 渲染，`/admin/*` 四个租户级运营面与普通成员可见的项目模块平铺在同一导航中。

---

## D-01 当前主菜单树

【事实】来源：`src/router/navigation.ts:84-271`（`navGroups`）。图标为 lucide 组件；顺序即数组顺序。

```text
PM-AI（应用壳）
├── 【分组】个人  (labelKey: shell.app.side.group.personal)
│   ├── 我的主页        /personal              IMPLEMENTED · Personal
│   └── 收件箱          /inbox                 PLANNED（无组件 → PlaceholderView）· Personal
├── 【分组】工作区  (shell.app.side.group.workspace)
│   ├── 项目总览        /dashboard             IMPLEMENTED（实为 Team Dashboard）· Team
│   └── 项目            /projects              IMPLEMENTED · Team
├── 【分组】项目模块  (shell.app.side.group.modules)  projectScoped = true
│   ├── 代码检索        /project/discovery     IMPLEMENTED · Project
│   ├── 需求            /project/requirements  IMPLEMENTED · Project
│   ├── 技术设计        /project/design        IMPLEMENTED · Project
│   ├── 任务管理        /project/tasks         IMPLEMENTED · Project
│   ├── AI 执行中心     /project/execution     IMPLEMENTED · Project
│   ├── 知识审阅        /project/knowledge     IMPLEMENTED（KB+PC+分析报告三合一）· Project
│   ├── 开发环境        /project/devenv        IMPLEMENTED · Project
│   └── 人工审核中心    /project/review-center IMPLEMENTED · Project
└── 【分组】管理  (shell.app.side.group.admin)
    ├── 设置            /admin/settings        IMPLEMENTED · Team/平台
    ├── 成员            /admin/members         IMPLEMENTED · Team
    ├── 公告管理        /admin/announcements   IMPLEMENTED · Team
    └── AI模型          /settings/team/ai-providers  IMPLEMENTED · Team
```

【证据】
- 分组与条目：`src/router/navigation.ts:84-271`；`personal` 88-101、`workspace` 106-127、`modules` 133-224、`admin` 230-269。
- 显示名：`ai-frontend/locales/zh-CN/shell.yaml:199-223`（`side.group.*` / `side.item.*`）；`人工审核中心` 取自 `locales/zh-CN/review.yaml:2-3`（`review.nav.title`）。
- 图标：`navigation.ts:1-18` 的 lucide 导入，逐项 `icon:` 字段。
- `/inbox` 无 `component`：`navigation.ts:95-100`；路由回落 `item.component ?? PlaceholderView`：`router/index.ts:82`；占位文案 `shell.yaml:352-354`。
- 分组折叠：`SideNav.vue:58`（`collapsedGroups` 默认全展开）、`62-71`（`toggleGroup`）。
- 项目上下文头停用：`SideNav.vue:191-203`（整段 HTML 注释，理由「多团队、多项目之后这里展开之后太大了」）。

【事实】**不存在**导航树级二级菜单：`NavChild` 接口定义于 `navigation.ts:60-67`，但 `navGroups` 中无任何 `children` 赋值；`SideNav.vue:256-276` 的子菜单渲染分支在当前数据下为不可达代码。

【事实】其他导航要素（非左栏）：
- 顶栏团队切换 `TeamSwitcher`（`TopBar.vue:219`）、项目切换 `ProjectSwitcher`（`224-234`）。
- 全局搜索 / 命令面板（⌘K）：`WorkspaceShell.vue:49-50, 202-259`；命令项 5 条 `148-191`。
- 通知下拉 + 完整通知中心 `/notifications`（`TopBar.vue:259-262`；`router/index.ts:443-447`）。
- 账户菜单 5 个去向（`TopBar.vue:170-176`）：`/settings/personal/profile`、`/settings`、`/settings/team`、`/settings/personal/security`、`/settings/personal/preferences`。
- 面包屑：**非全局派生**，由页面通过 `PageFrame.breadcrumbs` 手工传入（`PageFrame.vue:23-32`、`PageHeader.vue:12-35`）。
- 页内二级导航（等价二级菜单，共 5 类）：项目设置侧栏 6 项（`views/settings/nav.ts:65-72`）、个人设置侧栏 4 项（`31-37`）、团队设置侧栏 1 项（`49-51`）、右上下文栏 5 页签（`ContextPanel.vue:126-132`）、工作区页内页签（Knowledge 3 / Tasks / Design / Execution）。

【事实】响应式：左栏只有「232px 展开 / 60px 折叠」两态（`SideNav.vue:37-40`, `122-128`）；未发现移动端抽屉导航实现——`shell.yaml:17-20, 36-37` 的 `actions.openNavigation` / `drawer.title` 无对应组件（全局搜索无命中）。

---

## D-02 路由与页面注册表

【事实】路由总账（`src/router/index.ts`）：
- 应用壳路由记录 **77 条** = 67 个页面组件路由 + 9 条重定向 + 1 条 404 兜底。
- 认证路由 **9 条**（`src/router/auth.ts:24-82`）。
- 设计目录路由 **43 条**（`src/router/design.ts:81-436`），仅 `import.meta.env.DEV` 注册（`router/index.ts:581-592`），发布构建剔除。
- 视图文件 **199 个 `.vue`**，其中 `views/design` 53 个、非 design 146 个（含组件级子目录）。

### D-02-a 左导航派生路由（16 条，全部实现除 `/inbox`）

| ID | 菜单路径 | 页面名称 | Route | Component | 层级 | 状态 | 权限 |
|---|---|---|---|---|---|---|---|
| N-01 | 个人 / 我的主页 | 我的主页 | `/personal` | `views/personal/PersonalHomeView.vue` | Personal | IMPLEMENTED | 仅登录 |
| N-02 | 个人 / 收件箱 | 收件箱 | `/inbox` | 无 → `PlaceholderView` | Personal | PLANNED | 仅登录 |
| N-03 | 工作区 / 项目总览 | Team Dashboard | `/dashboard` | `views/dashboard/TeamDashboardView.vue` | Team | IMPLEMENTED | 仅登录（块级 admin） |
| N-04 | 工作区 / 项目 | 项目列表 | `/projects` | `views/project/list/ProjectListView.vue` | Team | IMPLEMENTED | 仅登录 |
| N-05 | 项目模块 / 代码检索 | 代码发现 | `/project/discovery` | `views/project/discovery/DiscoveryView.vue` | Project | IMPLEMENTED | 仅登录 |
| N-06 | 项目模块 / 需求 | 需求工作区 | `/project/requirements` | `views/project/requirements/RequirementsView.vue` | Project | IMPLEMENTED | 仅登录 |
| N-07 | 项目模块 / 技术设计 | 设计分析 | `/project/design` | `views/project/design/analysis/DesignAnalysisView.vue` | Project | IMPLEMENTED | 仅登录 |
| N-08 | 项目模块 / 任务管理 | 任务总览 | `/project/tasks` | `views/project/tasks/overview/TasksOverviewView.vue` | Project | IMPLEMENTED | 仅登录 |
| N-09 | 项目模块 / AI 执行中心 | 执行中心 | `/project/execution` | `views/project/execution/ExecutionCenterView.vue` | Project | IMPLEMENTED | 仅登录 |
| N-10 | 项目模块 / 知识审阅 | 知识页（KB+PC+报告） | `/project/knowledge` | `views/project/knowledge/KnowledgeView.vue` | Project | IMPLEMENTED | 仅登录（确认门在后端） |
| N-11 | 项目模块 / 开发环境 | Bootstrap | `/project/devenv` | `views/project/devenv/BootstrapView.vue` | Project | IMPLEMENTED | 仅登录 |
| N-12 | 项目模块 / 人工审核中心 | 审核收件箱 | `/project/review-center` | `views/project/review-center/ReviewCenterPage.vue` | Project | IMPLEMENTED | 仅登录 |
| N-13 | 管理 / 设置 | 系统设置 | `/admin/settings` | `views/admin/AdminSettingsView.vue` | Team/平台 | IMPLEMENTED | **仅登录（无角色门）** |
| N-14 | 管理 / 成员 | 团队成员 | `/admin/members` | `views/settings/team/SettingsTeamMembersView.vue` | Team | IMPLEMENTED | **仅登录（无路由门）** |
| N-15 | 管理 / 公告管理 | 公告 | `/admin/announcements` | `views/admin/AdminAnnouncementsView.vue` | Team | IMPLEMENTED | **仅登录（无角色门）** |
| N-16 | 管理 / AI模型 | 团队 LLM Provider | `/settings/team/ai-providers` | `views/settings/SettingsTeamAiView.vue` | Team | IMPLEMENTED | **仅登录（无角色门）** |

【证据】派生规则：`router/index.ts:59-86`（父项重定向首个子项、子项平铺、缺省 `PlaceholderView`）；`buildRoutes` 92-96。组件绑定：`navigation.ts:93, 113-117, 122-126, 142-145, 151-154, 162-165, 174-177, 184-187, 196-199, 205-208, 219-222, 235-238, 244-247, 253-256, 264-267`。
【证据】devices 真数据：`BootstrapView.vue:14-24, 29-34`（六端点 `/v1/pm` devices 系）。
【证据】公告直连 http：`views/admin/AdminAnnouncementsView.vue`（`http.*` 命中）。

### D-02-b 项目域非导航路由（手工注册）

| ID | 页面名称 | Route | Component | 层级 | 状态 | 入口 | 证据 |
|---|---|---|---|---|---|---|---|
| P-01 | 项目总控台 | `/project/command-center` | `CommandCenterView.vue` | Project | PARTIAL（`?project_id` 真数据 + `api/mock/command-center.ts` 预览分支） | 注释称「项目切换后落地页」；实际链接位置 UNKNOWN | `router/index.ts:136-140`；`api/mock/command-center.ts` 存在 |
| P-02 | 项目认知（Project Cognition） | `/project/cognition` | `CognitionPage.vue` | Project | IMPLEMENTED（真 API） | **无菜单、无页内链接（孤立）** | `router/index.ts:106-110`；`CognitionPage.vue:14-24` |
| P-03 | 项目基线 | `/project/baseline` | `BaselinePage.vue` | Project | IMPLEMENTED（真 API） | **无菜单、无页内链接（孤立）** | `router/index.ts:113-117`；`BaselinePage.vue:12-19` |
| P-04 | 项目创建 | `/project/create` | `ProjectCreateView.vue` | Project（动作页） | IMPLEMENTED | 顶栏「新建项目」+ 个人主页 | `router/index.ts:119-123`；`TopBar.vue:161-163` |
| P-05 | AI 会话完整页 | `/project/assistant` | `AssistantView.vue` | Project | IMPLEMENTED | 指令栏提交 / 深链 | `router/index.ts:128-132`；`CommandBar.vue:98-100` |
| P-06 | 图形产物工作区 | `/project/visualizations` | `VisualizationsView.vue` | Project | IMPLEMENTED | 右栏 Graph 页签 / 对话卡 | `router/index.ts:426-431` |
| P-07 | 需求详情 | `/project/requirements/:requirementId` | `RequirementDetailView.vue` | Project | IMPLEMENTED | 需求列表详情面板 | `router/index.ts:436-439` |
| P-08 | 需求评审 | `/project/requirements/review` | `RequirementReviewView.vue` | Project | IMPLEMENTED | 需求页头 | `router/index.ts:387-392` |
| P-09 | 需求基线 | `/project/requirements/baselines` | `RequirementBaselineView.vue` | Project | IMPLEMENTED | 需求页头 | `router/index.ts:397-404` |
| P-10 | 需求修订历史 | `/project/requirements/:requirementId/history` | `RequirementHistoryView.vue` | Project | IMPLEMENTED | 需求详情两处入口 | `router/index.ts:408-412` |
| P-11 | 变更影响分析 | `/project/requirements/:requirementId/impact` | `RequirementImpactView.vue` | Project | IMPLEMENTED | 需求详情页头 | `router/index.ts:417-422` |

### D-02-c 工作区页内子路由

| ID | 页面名称 | Route | Component | 状态 | 所在页内页签 | 证据 |
|---|---|---|---|---|---|---|
| T-01 | 任务池 | `/project/tasks/backlog` | `TasksBacklogView.vue` | IMPLEMENTED | TasksFrame | `router/index.ts:273-277` |
| T-02 | 任务拆解 | `/project/tasks/breakdown` | `TaskBreakdownView.vue` | IMPLEMENTED | TasksFrame | `231-244` |
| T-03 | 依赖图 | `/project/tasks/planning/graph` | `DependencyGraphView.vue` | IMPLEMENTED | Planning 页签 | `245-251` |
| T-04 | 时间线 | `/project/tasks/planning/timeline` | `TimelineView.vue` | IMPLEMENTED | Planning 页签 | `252-258` |
| T-05 | 执行准备度 | `/project/tasks/readiness` | `ExecutionReadinessView.vue` | IMPLEMENTED | TasksFrame | `259-265` |
| T-06 | 任务详情 | `/project/tasks/:taskId` | `TaskDetailView.vue` | IMPLEMENTED | 下钻 | `266-272` |
| T-07 | 技术设计列表 | `/project/design/technical-designs` | `TechnicalDesignsView.vue` | IMPLEMENTED | DesignFrame | `191-197` |
| T-08 | 技术设计详情 | `/project/design/technical-designs/:designId` | `TechnicalDesignDetailView.vue` | IMPLEMENTED | 下钻 | `198-203` |
| T-09 | 架构决策（ADR） | `/project/design/adrs` | `AdrView.vue` | IMPLEMENTED | DesignFrame | `204-208` |
| T-10 | ADR 详情 | `/project/design/adrs/:adrId` | `AdrDetailView.vue` | IMPLEMENTED | 下钻 | `209-213` |
| T-11 | 实施准备度 | `/project/design/readiness` | `ImplementationReadinessView.vue` | IMPLEMENTED | DesignFrame | `214-221` |
| T-12 | Run 详情 | `/project/execution/runs/:runId` | `RunDetailView.vue` | IMPLEMENTED | ExecutionFrame | `148-153` |
| T-13 | 任务协作 | `/project/execution/collaboration/:taskId` | `CollaborationView.vue` | IMPLEMENTED | ExecutionFrame | `154-159` |
| T-14 | 人工介入 | `/project/execution/intervention` | `HumanInterventionView.vue` | IMPLEMENTED | ExecutionFrame | `160-165` |
| T-15 | 执行授权 | `/project/execution/authorization` | `ExecutionAuthorizationView.vue` | IMPLEMENTED | ExecutionFrame | `166-171` |
| T-16 | 交付复核 | `/project/execution/review` | `DeliveryReviewView.vue` | IMPLEMENTED | ExecutionFrame | `172-177` |
| T-17 | 执行历史 | `/project/execution/history` | `ExecutionHistoryView.vue` | IMPLEMENTED | ExecutionFrame | `178-183` |

### D-02-d 设置域（三域并列，各有独立侧栏）

| ID | 页面名称 | Route | Component | 层级 | 状态 | 证据 |
|---|---|---|---|---|---|---|
| S-01 | 个人设置落地 | `/settings/personal` | `SettingsLandingView.vue` | Personal | IMPLEMENTED | `router/index.ts:458-462` |
| S-02 | 个人资料 | `/settings/personal/profile` | `SettingsProfileView.vue` | Personal | IMPLEMENTED | `463-467` |
| S-03 | 安全 | `/settings/personal/security` | `SettingsSecurityView.vue` | Personal | IMPLEMENTED | `468-472` |
| S-04 | 偏好 | `/settings/personal/preferences` | `SettingsPreferencesView.vue` | Personal | IMPLEMENTED | `473-477` |
| S-05 | 通知偏好 | `/settings/personal/notifications` | `SettingsNotificationsView.vue` | Personal | IMPLEMENTED | `478-482` |
| S-06 | 团队设置落地 | `/settings/team` | `SettingsTeamLandingView.vue` | Team | IMPLEMENTED（仅 General 一张卡） | `486-490`；`nav.ts:49-51` |
| S-07 | 团队常规 | `/settings/team/general` | `SettingsTeamGeneralView.vue` | Team | IMPLEMENTED（Owner 转移等） | `532-536`；`SettingsTeamGeneralView.vue:51-52` |
| S-08 | 项目设置常规 | `/project/settings/general` | `SettingsProjectPlaceholderView.vue` | Project | SHELL（proposed） | `290-294`；`nav.ts:66` |
| S-09 | 项目设置成员 | `/project/settings/members` | `SettingsProjectPlaceholderView.vue` | Project | SHELL（proposed） | `295-299`；`nav.ts:67` |
| S-10 | 仓库 | `/project/settings/repositories` | `SettingsProjectRepositoriesView.vue` | Project | IMPLEMENTED | `300-304` |
| S-11 | SSH 私钥 | `/project/settings/ssh-keys` | `SettingsProjectSshKeysView.vue` | Project | IMPLEMENTED | `305-309` |
| S-12 | 执行环境（Runners） | `/project/settings/runners` | `SettingsProjectRunnersView.vue` | Project | IMPLEMENTED | `310-314` |
| S-13 | Runner 详情 | `/project/settings/runners/:deviceId` | `SettingsProjectRunnerDetailView.vue` | Project | IMPLEMENTED | `315-319` |
| S-14 | AI Roles | `/project/settings/ai/roles` | `SettingsProjectAiRolesView.vue` | Project | IMPLEMENTED | `331-335` |
| S-15 | AI Overview | `/project/settings/ai/overview` | `SettingsProjectAiOverviewView.vue` | Project | IMPLEMENTED | `336-340` |
| S-16 | 模型策略 | `/project/settings/ai/model-policies` | `SettingsProjectAiModelPoliciesView.vue` | Project | IMPLEMENTED | `341-345` |
| S-17 | 执行监控 | `/project/settings/ai/monitor` | `SettingsProjectAiMonitorView.vue` | Project | IMPLEMENTED | `346-350` |
| S-18 | 执行策略 | `/project/settings/ai/execution-policies` | `SettingsProjectAiExecutionPoliciesView.vue` | Project | IMPLEMENTED | `351-355` |

### D-02-e 全局 / 认证 / 生命周期路由

| ID | 页面名称 | Route | 层级 | 状态 | 证据 |
|---|---|---|---|---|---|
| G-01 | 通知中心 | `/notifications` | Personal | IMPLEMENTED | `router/index.ts:443-447` |
| G-02~G-05 | 团队引导 4 页 | `/onboarding`、`/create`、`/invite`、`/link` | Team | IMPLEMENTED（`layout: bare`） | `501-522` |
| G-06 | 邀请接受 | `/invitation/accept` | Team | IMPLEMENTED（`layout: auth`） | `525-529` |
| G-07 | 404 兜底 | `/:pathMatch(.*)*` | 全局 | SHELL | `538-543` |
| A-01~A-09 | 登录/初始化/找回/重置/邀请/登录中/会话过期/无权限/样式 | `/auth/*` | — | IMPLEMENTED | `auth.ts:24-82` |
| D-01~D-43 | 设计系统目录 | `/design/*` | — | SHELL（仅 DEV） | `design.ts:81-436`；`index.ts:581-592` |

### D-02-f 重定向（9 条，无页面）

`/project/settings`→`general`（`284-289`）、`/project/settings/ai`→`roles`（`326-330`）、`/project/tasks/planning`→`graph`（`231-237`）、`/project/repository`(`359-363`)、`/project/repository/overview`(`364-367`)、`/project/repository/branches`(`368-371`)、`/project/environments`(`372-375`)、`/project/environments/:deviceId`(`376-381`)、`/settings`→`/settings/personal`(`452-457`)。

> 【事实】重定向的存在本身是历史残留证据：Git 仓库与执行环境原为独立一级菜单，现已迁入项目设置（`navigation.ts:134-137, 210-211`）。

---

## D-03 Personal / Team / Project 页面归属表

| 页面 | 当前层级 | 实际数据归属 | 当前上下文来源 | 是否合理 | 证据 |
|---|---|---|---|---|---|
| 我的主页 `/personal` | Personal | 按当前登录人聚合 | 登录态 | 合理 | `navigation.ts:88-94`；`api/personal-home.ts` |
| 收件箱 `/inbox` | Personal | 无（占位） | — | 层级合理、功能缺失 | `navigation.ts:95-100` |
| 通知中心 `/notifications` | Personal | 当前用户通知 | 登录态 | 合理 | `TopBar.vue:153-155`；`router/index.ts:443-447` |
| 个人设置 `/settings/personal/*` | Personal | 当前用户 | 登录态 | 合理 | `router/index.ts:458-482` |
| 项目总览 `/dashboard` | Team | 当前团队（成员/邀请/项目） | `teamStore.currentTeam` | **菜单名与实体不符**（名为「项目总览」，实为团队仪表盘） | `TeamDashboardView.vue:23-27, 39-40` |
| 项目 `/projects` | Team | 团队下项目列表 | 团队 | 合理 | `navigation.ts:119-126` |
| 团队设置 `/settings/team*` | Team | 当前团队 | `teamStore` | 合理（但成员页不在其下，见 D-06） | `nav.ts:49-51` |
| 管理组 `/admin/*` | Team/平台 | 租户级 | 登录态（无团队守卫） | **不合理：无角色门** | `navigation.ts:230-269`；`SideNav.vue:205` |
| 项目模块 8 项 `/project/*` | Project | 由 `?project_id=` 或 `shell.currentProjectId` 决定 | 顶栏项目切换 / URL query / localStorage | 基本合理，但无项目内导航 | `WorkspaceShell.vue:44-47, 80-86, 138-142` |
| 项目总控台 `/project/command-center` | Project | 项目级聚合 | `?project_id=` | 合理，但非菜单入口 | `router/index.ts:136-140` |
| 项目认知 `/project/cognition` | Project | 项目级（`ai.cognition_*` / `ai.project_knowledge`） | `route.query.project_id` 或 `scope.currentProjectId` | 层级正确，**导航缺失** | `CognitionPage.vue:56-63` |
| 项目基线 `/project/baseline` | Project | 项目级 | 同上 | 层级正确，**导航缺失** | `BaselinePage.vue:37-43` |
| 知识 `/project/knowledge` | Project | KB 文档 + PC 对象 + 分析报告，均按 `project_id` 过滤 | `shell.currentProjectId` | **不合理：三域合一** | `KnowledgeView.vue:43-45, 67-70` |
| 项目设置 `/project/settings/*` | Project | 当前项目 | `?project_id=` 优先，回退 scope | 合理 | `views/project/settings/shared.ts:25-43` |

【事实】三个上下文的判定机制互不相同：
- 左栏项目模块依赖 `shell.currentProjectId`（`WorkspaceShell.vue:120-142`）；
- 项目设置依赖 `?project_id=` 优先 + scope 回退（`settings/shared.ts:25-30`）；
- 认知/基线页依赖 `?project_id=` 优先 + `scope.currentProjectId` 回退（`CognitionPage.vue:56-63`、`BaselinePage.vue:37-43`）。

【判断】同一「当前项目」概念存在**三套读取路径**，是「同一页面在不同入口下上下文不一致」的结构性风险。UNKNOWN：是否存在实际发生的上下文错配用例，未做运行时验证。

【事实】进入 Project 后**没有独立的项目内导航栏**：项目模块仍是全局左栏的一个分组，靠 `projectScoped: true` 仅作语义标记（`navigation.ts:132`），无渲染差异（`SideNav.vue` 未使用该字段控制显隐）。

---

## D-04 四域导航映射表

### D-04.1 KB（Knowledge Base）

| 项 | 现状 | 证据 |
|---|---|---|
| 当前菜单 | 无独立菜单。内容位于「知识审阅 `/project/knowledge`」的 **kb 页签** | `KnowledgeView.vue:45, 167-170` |
| 当前路由 | `/project/knowledge`（页签状态非路由，不可深链到页签） | 同上；深链仅支持 `?doc=` |
| 页面 | 文档列表（最新修订投影）+ 行内展开详情（正文 / 修订历史 / 确认发布）+ 摄入面板 | `KnowledgeView.vue:505-663` |
| 实现状态 | IMPLEMENTED（真 API：`/v1/kb/documents*`） | `api/knowledge.ts:46,53,69,316` |
| 归属层级 | Project（`project_id` 必填） | `api/knowledge.ts:46-48` |
| Team 级管理 | **无**（无团队级 KB 页面、无跨项目 KB 入口） | 全局搜索 `kb` 仅命中该项目页 |
| Project 关联入口 | 有：随项目进入 | `KnowledgeView.vue:59` |
| 功能项核查 | Source：**UI 无**（仅 API `kb.source` 存在，`authority-matrix` §1.1）；Document ✓；Document Revision ✓；Memory / Memory Revision / Memory Proposal：**UI 无**；Knowledge Review：部分（审核中心不含 memory，见 D-06）；Knowledge Search：间接（顶栏全局检索，结果落 `/project/knowledge`） | `authority-matrix-v1.md:16-17`；`four-domain-audit-report-v1.md:62` |
| 与 PC / 分析报告混淆 | **是**：三者同页三页签 | `KnowledgeView.vue:161-175` |

### D-04.2 Product Cognition（PC）

| 项 | 现状 | 证据 |
|---|---|---|
| 当前菜单 | 无独立菜单。位于 `/project/knowledge` 的 **pc 页签** | `KnowledgeView.vue:45, 171-174` |
| 当前路由 | `/project/knowledge`（`?object=` 深链） | `KnowledgeView.vue:205-212` |
| 页面 | 认知对象列表（类型 / 生命周期 / 最新修订摘要）+ 行内展开全文 | `KnowledgeView.vue:665-736` |
| 实现状态 | **PARTIAL**：只读浏览 + 提案审批（经分析报告页签）；**无**对象创建/编辑、域/能力/模块/角色/场景/业务流程/业务对象/业务规则的分族浏览、**无**冲突检测与解决页 | `api/knowledge.ts:92,118`；`AnalysisReportsPanel.vue:240-261`；`authority-matrix-v1.md:58`（`pc.conflict` 为桩） |
| 归属层级 | Project（`project_id` 必填） | `api/knowledge.ts:92-94` |
| Team 级产品认知管理入口 | **无** | 未发现团队级 PC 路由 |
| 跨项目引用入口 | **无** | — |
| 与 Project Cognition 混淆 | **是**（概念层）：`/project/knowledge` 的 pc 页签与 `/project/cognition` 都以「认知」命名，但前者是 P 域产品认知，后者是项目认知 | `KnowledgeView.vue:35-40` vs `CognitionPage.vue:26-35` |

### D-04.3 Project Cognition

| 项 | 现状 | 证据 |
|---|---|---|
| 当前菜单 | **无**（不在 `navGroups`） | `navigation.ts:84-271` 无 `/project/cognition` |
| 当前路由 | `/project/cognition`（应用壳内，手工注册） | `router/index.ts:106-110` |
| 页面 | 三区：Runs 历史 + 人工启动 / 逐项审核（confirm / edit / reject）/ 已确认项目知识 | `CognitionPage.vue:183-359` |
| 实现状态 | IMPLEMENTED（真 API：overview / knowledge / start run / decide item） | `CognitionPage.vue:14-24`；`api/cognition.ts` |
| 所属 Project | 是（`project_id` 必填，缺失显示 noProject 空态） | `CognitionPage.vue:56-63, 176-180` |
| 独立项目内菜单 | **无** | — |
| 能否从 Project Dashboard 进入 | **否**：全仓仅 `router/index.ts:107` 与测试 `CognitionPage.test.ts:77` 引用该路径，**无任何 `<RouterLink>` / `router.push` 指向** | 全库检索 `/project/cognition` |
| Team 级全局入口 | 无（且不应有：领域权威为项目级） | `authority-matrix-v1.md:44` |
| 与 PC 混淆 | **是**（命名与入口层面，见上） | — |

### D-04.4 Analysis

| 项 | 现状 | 证据 |
|---|---|---|
| 当前菜单 | **无独立菜单**。分析报告是 `/project/knowledge` 的**首屏页签**（D-5「报告优先」） | `KnowledgeView.vue:46, 163-166, 411-415` |
| 当前路由 | `/project/knowledge`（`?report=` 深链） | `AnalysisReportsPanel.vue:174-187` |
| 页面 | 报告列表按状态分区（analyzing / analyzed / closed / failed）、行展开审阅、KB 确认、PC 提案批准/驳回、关闭报告批量发布、身份综合 | `AnalysisReportsPanel.vue:338-654` |
| 实现状态 | IMPLEMENTED（`/v1/pm/analysis-reports*` + close） | `api/analysis-reports.ts`（`list/get/close/synthesize`）；`four-domain-audit-report-v1.md:60` |
| 所属 Project | 是 | `props.projectId`（`AnalysisReportsPanel.vue:32`） |
| 是否存在独立 Analysis 页面 | **否**（另有 3 处「分析」语义）：① 技术设计「分析」子页 `/project/design`（设计↔需求覆盖、ADR 引用，`DesignAnalysisView.vue:19-29`）；② 人工审核中心「分析报告待审阅」（`review.yaml:15`）；③ 总控台初始化卡的「代码分析」动作（`InitializationCard.vue:125-132`，跳 `/project/settings/repositories`） | 同左 |
| 与 Repository 管理 / KB Review 混淆 | **是**：报告正文寄居 `kb.document_revision.content`（`authority-matrix-v1.md:76`），UI 上报告与 KB 文档同页；"代码分析"动作落在仓库设置页 | 同左 |

---

## D-05 权限与菜单可见性矩阵

### D-05.1 菜单可见性（Navigation Visibility）

【事实】**除团队设置页内的按钮级判断外，导航无任何角色门。** `SideNav.vue:205` 对 `group.items` 全量渲染；`TeamSwitcher`/`ProjectSwitcher` 亦无角色分支。

| 菜单/页面 | Owner | Admin | Member | Project Member | 证据 |
|---|---|---|---|---|---|
| 个人组（2 项） | 可见 | 可见 | 可见 | 可见 | `SideNav.vue:205` |
| 工作区组（2 项） | 可见 | 可见 | 可见 | 可见 | 同上 |
| 项目模块组（8 项） | 可见 | 可见 | **可见** | **可见/非成员亦可见** | 同上；无成员判定 |
| 管理组（4 项） | 可见 | 可见 | **可见** | 可见 | 同上；`isManagementPath` 仅用于隐藏 AI 面板（`navigation.ts:305-314`），不控制菜单显隐 |

【判断】管理组对普通成员可见是**明确的越权暴露风险**（导航层）。是否可通过后端拒绝数据读取，取决于各端点鉴权（见 D-05.3，部分为 UNKNOWN / 已知缺口）。

### D-05.2 路由授权（Route Authorization）

【事实】全局只有一个 `router.beforeEach`：判断 `access/refresh token` 是否存在，无角色、无项目成员、无团队校验（`router/index.ts:618-643`）。`/admin/*`、`/project/settings/*`、`/project/cognition` 等均**仅要求登录**。

【事实】`PUBLIC_LAYOUTS = { auth, bare, design }`（`router/index.ts:600`）；`/design/*` 对匿名开放且按前缀二次兜底（`612-633`）。

| 菜单/页面 | 路由保护 | 说明 |
|---|---|---|
| 全部应用壳页面 | 仅「已登录」 | `index.ts:635-640` |
| `/design/*` | 无（公开） | `index.ts:630-633` |
| `/auth/*`、`/onboarding/*` | 无（layout 白名单） | `index.ts:620-627` |
| 角色/成员级路由门 | **不存在** | — |

### D-05.3 API / 数据授权（API Protection，抽样）

| 能力 | 后端门 | 强度 | 证据 |
|---|---|---|---|
| KB 确认发布 | 真人 interactive 会话；API Key / AI 主体 403 | 强 | `api/knowledge.ts:58-63`；`authority-matrix-v1.md:15`（`confirm_revision document.rs:60`） |
| PC 提案决策 | human + 治理授权；AI/Key 403；CAS `expected_version_no` | 强 | `api/knowledge.ts:264-269` |
| Project Baseline 确认 | 三重门（auth_mode + kind）（`pm_baseline.rs:183-238`） | 强 | `authority-matrix-v1.md:45` |
| Project Cognition 决策 | **仅 `require_project_visible`** → 持 API Key 的项目成员 AI 可写正式认知 | **弱（已登记 P0）** | `authority-matrix-v1.md:44-45` |
| 团队设置（Owner 转移/解散） | 前端 `isOwner`/`isAdmin` 按钮级 + 后端（未逐条核验） | 中（前端门≠安全门） | `SettingsTeamGeneralView.vue:51-52, 252-291` |
| 团队成员管理 | 前端 `isAdmin`/`isOwner` 行级；后端 UNKNOWN | 中 | `SettingsTeamMembersView.vue:69-111` |
| `/admin/settings`、`/admin/announcements`、`/settings/team/ai-providers` | UNKNOWN（未逐端点核验） | UNKNOWN | 仅确认前端无角色门 |

【事实】全仓 `require_project_visible` / `require_human` / `require_trusted` 出现在 25 个 Rust 文件（`apps/pm-api/src/*`），说明 API 层普遍有项目可见性门；但**逐端点清单不在本次范围内**（任务书未要求全量枚举，标记 UNKNOWN）。

### D-05.4 结论

- **菜单隐藏不等于权限控制**：当前连菜单隐藏都没有，权限完全依赖 API。
- **已知最严重单点**：Project Cognition 决策门仅项目可见性（`authority-matrix-v1.md:45`，P0），而前端把该能力藏在无入口的孤立页。
- **管理组可见性**是导航层最直接的越权暴露面。

---

## D-06 重复、孤立与职责混淆问题

严重度：P0（阻断 IA 冻结）/ P1（应在本轮解决）/ P2（可延后）。

### 6.1 Duplicate Entry（重复入口）

| # | 问题 | 证据 | 严重度 |
|---|---|---|---|
| DUP-1 | 「收件箱 `/inbox`」（占位，无组件）与「通知中心 `/notifications`」（真实）语义重复，真实入口在顶栏铃铛 | `navigation.ts:95-100`；`TopBar.vue:259-262`；`shell.yaml:176`（`inbox: 公告中心`） | P1 |
| DUP-2 | 「人工审核中心 `/project/review-center`」与「分析报告待审阅」（Knowledge 页签）重叠：`four-domain-audit-report` 明确前端审批权威三分 | `four-domain-audit-report-v1.md:61`；`review.yaml:15` | P1 |
| DUP-3 | 「代码分析」在 3 处出现：总控台初始化卡动作、仓库设置页、Knowledge 分析报告页签 | `InitializationCard.vue:125-132`；`AnalysisReportsPanel.vue` | P1 |
| DUP-4 | 「项目」入口 3 处：`/projects`（列表）、顶栏 ProjectSwitcher、全局命令 `cmd-open-project` | `navigation.ts:119`；`TopBar.vue:224`；`WorkspaceShell.vue:150-160` | P2 |

### 6.2 Orphan Route（孤立路由）

| # | 问题 | 证据 | 严重度 |
|---|---|---|---|
| ORP-1 | `/project/cognition`（Project Cognition 唯一页面）**零入口** | 全库检索仅 `router/index.ts:107` + 测试 | **P0** |
| ORP-2 | `/project/baseline`（Project Baseline）**零入口** | 全库检索仅 `router/index.ts:114` + 测试 | **P0** |
| ORP-3 | `/project/visualizations` 仅右栏 Graph 页签可达（可接受，但无直接入口说明） | `router/index.ts:426-431` | P2 |
| ORP-4 | `/project/assistant` 无导航项（设计如此，靠指令栏/深链） | `router/index.ts:128-132`；`CommandBar.vue:98-100` | P2（已在注释中说明） |

### 6.3 Incorrect Hierarchy（层级错误）

| # | 问题 | 证据 | 严重度 |
|---|---|---|---|
| HIE-1 | 团队成员管理挂在 `/admin/members`，而团队设置落地页自述「成员、邀请已迁 /admin/members」，团队设置侧栏只剩 General → **团队域被拆到两个顶级位置** | `SettingsTeamLandingView.vue:10-11`；`navigation.ts:240-248`；`nav.ts:49-51` | **P0** |
| HIE-2 | 团队级 LLM Provider 挂在「管理」组，路径却在 `/settings/team/ai-providers` → 菜单分组与 URL 域不一致 | `navigation.ts:260-268` | P1 |
| HIE-3 | 「项目总览 `/dashboard`」实为 Team Dashboard（团队档案/成员概览），项目执行指标在 `/project/command-center` → 名称误导一级层 | `TeamDashboardView.vue:23-27`；`navigation.ts:107-117` | P1 |
| HIE-4 | 项目设置（General/Members/AI）为占位却与已实现节并列在侧栏（以 `proposed` 徽标区分，但用户仍可进入） | `nav.ts:65-72`；`SettingsProjectPlaceholderView.vue:35-42` | P2 |

### 6.4 Domain Ownership Conflict（领域权威冲突）

| # | 问题 | 证据 | 严重度 |
|---|---|---|---|
| DOM-1 | **KB + PC + Analysis 三域合并到 `/project/knowledge` 一个页面**；菜单名「知识审阅」只覆盖 KB 语义 | `KnowledgeView.vue:45, 161-175` | **P0** |
| DOM-2 | Product Cognition 无独立权威面（只有只读列表嵌在 KB 页），与「产品级正式认知权威」定位不符 | `KnowledgeView.vue:665-736`；`authority-matrix-v1.md:48-59` | **P0** |
| DOM-3 | Analysis 报告正文借住 `kb.document_revision.content`，UI 又把报告归入知识页 → 报告与 KB 文档在用户认知中同质 | `authority-matrix-v1.md:76`；`four-domain-audit-report-v1.md:68,73` | **P0** |
| DOM-4 | Project Cognition 页面独立存在但零入口，等于「权威页面被隐藏」；同时 KB 页的 pc 页签以「认知」命名，用户无法区分 P 域与项目认知 | `router/index.ts:106`；`CognitionPage.vue:26-35` | **P0** |
| DOM-5 | 知识审核（KB memory review）**无 UI 入口**；cognition 确认虽在 CognitionPage 有按钮，但该页不可达 | `four-domain-audit-report-v1.md:62`（U25 缺口） | P1 |

### 6.5 Permission Visibility Conflict（权限可见性冲突）

| # | 问题 | 证据 | 严重度 |
|---|---|---|---|
| PER-1 | 「管理」组 4 项对 Member 完全可见（无 `v-if` 角色门） | `SideNav.vue:205`；`navigation.ts:230-269` | **P0** |
| PER-2 | 项目模块 8 项对「非项目成员」也可见（无成员关系判定），可见 `projectScoped` 只是语义标记 | `navigation.ts:132`；`SideNav.vue` 未消费该字段 | P1 |
| PER-3 | Project Cognition 决策 API 门仅项目可见性（后端 P0），前端无任何加强 | `authority-matrix-v1.md:45` | **P0** |
| PER-4 | 团队设置页把 `isOwner/isAdmin` 当权限使用（按钮 disable / `v-if`），属 UI 门而非安全门 | `SettingsTeamGeneralView.vue:51-52, 252` | P2（需确认后端） |

### 6.6 Inconsistent Naming（命名不一致）

| # | 问题 | 证据 | 严重度 |
|---|---|---|---|
| NAM-1 | 菜单「知识审阅」实际是知识库浏览 + PC 对象浏览 + 分析报告 → 名不副实 | `shell.yaml:217`；`KnowledgeView.vue:35-40` | P1 |
| NAM-2 | 菜单「项目总览」实际是 Team Dashboard | `shell.yaml:208`；`TeamDashboardView.vue:24` | P1 |
| NAM-3 | 菜单「代码检索」实际是「代码发现 + 人工确认 finding」（`Discovery`），与「检索/搜索」语义冲突 | `shell.yaml:211`；`DiscoveryView.vue:24-36` | P1 |
| NAM-4 | `shell.yaml` 保留已下线项：`repository: 代码仓库`、`environments: 执行环境`、`personalWorkspace: 个人工作区`、`assistant: AI 助手` | `shell.yaml:205, 210, 216, 219` vs `navigation.ts` 已无对应项 | P2 |
| NAM-5 | 顶栏注释称「会话页仍由左导航『助手』项进入」，但该导航项已被移除 | `TopBar.vue:293-294`；`navigation.ts:189-191` | P2 |
| NAM-6 | 左栏注释称「Repository 带二级上下文子菜单」，实际无任何 `children` | `SideNav.vue:37-39`；`navigation.ts:53-56` | P2 |
| NAM-7 | 右上下文栏注释称「四个一级 Tab」，代码为 5 个（新增 Graph） | `ContextPanel.vue:26` vs `126-132` | P2 |

### 6.7 Missing Navigation Entry（缺失导航入口）

| # | 缺失项 | 证据 | 严重度 |
|---|---|---|---|
| MIS-1 | Project Cognition 无入口 | 见 ORP-1 | **P0** |
| MIS-2 | Project Baseline 无入口 | 见 ORP-2 | **P0** |
| MIS-3 | Product Cognition 无独立入口（仅页签） | 见 DOM-2 | **P0** |
| MIS-4 | Analysis 无独立入口（仅页签） | 见 D-04.4 | **P0** |
| MIS-5 | KB Source / Memory / Memory Proposal 无 UI | `authority-matrix-v1.md:16-17` | P1 |
| MIS-6 | 项目总控台无菜单项（仅作落地页/命令） | `router/index.ts:136-140` | P1 |
| MIS-7 | 团队「成员/邀请」不在团队设置侧栏 | `nav.ts:49-51` | P1 |

### 6.8 Excessive Navigation Depth（层级过深）

| # | 问题 | 证据 | 严重度 |
|---|---|---|---|
| DEP-1 | 项目设置：左栏 → 总控台 → 页头「项目设置」→ 侧栏 6 节 → AI 五子页签（最深 4 层点击） | `router/index.ts:284-356`；`nav.ts:65-72` | P1 |
| DEP-2 | 技术设计：左栏 → 页内 4 页签 → 详情页 → （ADR 详情） | `router/index.ts:191-222` | P2 |
| DEP-3 | 任务管理：左栏 → 5 页签 → 规划 2 子页签 → 任务详情 | `router/index.ts:231-278` | P2 |

---

## D-07 当前导航与已确认产品架构的差距

对照任务书 §D-07 十条基线与 `docs/architecture/four-domain/authority-matrix-v1.md`、`docs/architecture/four-domain/four-domain-audit-report-v1.md`。

| # | 产品基线 | 判定 | 事实依据 |
|---|---|---|---|
| 1 | Personal Workspace 是个人入口 | **PARTIAL** | `/personal` 存在且为个人聚合（`navigation.ts:88-94`）；但「收件箱」为占位（`95-100`），个人域与通知域语义重叠（DUP-1） |
| 2 | Project Dashboard / Command Center 是项目入口 | **PARTIAL** | `/project/command-center` 存在但**无菜单项**（`router/index.ts:136-140`）；菜单里的「项目总览」实为 Team Dashboard（`shell.yaml:208`） |
| 3 | KB 是独立知识领域 | **MISMATCH** | KB 与 PC、Analysis 合并于 `/project/knowledge`（`KnowledgeView.vue:45`），无独立菜单 |
| 4 | Product Cognition 是产品级正式认知领域 | **MISMATCH** | PC 仅为 KB 页的只读页签（`KnowledgeView.vue:171-174, 665-736`）；无对象管理/冲突检测（`authority-matrix-v1.md:58` 为桩） |
| 5 | Project Cognition 是 Project 级正式认知领域 | **PARTIAL** | 页面与真 API 齐备（`CognitionPage.vue`），但**零入口**（ORP-1），实际不可达 |
| 6 | Analysis 是独立分析领域 | **MISMATCH** | 无独立页面/菜单；报告嵌在 KB 页（`KnowledgeView.vue:411-415`）；报告正文借住 KB 修订（`authority-matrix-v1.md:76`） |
| 7 | 三种知识对象不能合并 | **MISMATCH** | 后端已分四域（`authority-matrix-v1.md:7-59`），**前端 UI 合并**为单页三页签 |
| 8 | Team / Project 权限与数据上下文必须明确 | **MISMATCH** | 菜单无角色门（`SideNav.vue:205`）；项目上下文三套读取路径（D-03）；团队成员管理跨顶级（HIE-1） |
| 9 | 不使用泛化 Workspace 作为未经定义的业务实体 | **MATCH（有保留）** | 顶栏明确不引入 Workspace Switcher（`TopBar.vue:41-42`），仅团队+项目二级上下文；但左栏控制行硬编码文案「WORKSPACE」（`SideNav.vue:132`）与分组名「工作区」（`shell.yaml:201`），属可接受的 UI 文案，不构成业务实体 |
| 10 | 导航不应把一次性仓库初始化能力提升为长期核心菜单 | **MATCH** | 仓库/执行环境已从一级菜单迁入项目设置（`navigation.ts:134-137, 210-211`；`router/index.ts:359-382` 重定向），无一级「代码仓库」菜单 |

【判断】十条基线中 4 条 MATCH/PARTIAL 可接受、**5 条 MISMATCH**、1 条 PARTIAL 因入口缺失实质失效。核心差距集中在「四域在 UI 层被压缩为一个知识页」与「Project Cognition/Baseline 不可达」。

---

## D-08 待 IA 评审的问题

> 以下均需产品负责人裁决；本报告不代替裁决。

| # | 当前事实 | 冲突 / 不确定性 | 可选方案 | 推荐（如有） | 阻塞 IA 冻结 |
|---|---|---|---|---|---|
| Q-01 | KB/PC/Analysis 三合一于 `/project/knowledge` | 与四域独立权威冲突；菜单名「知识审阅」只覆盖 KB | A 拆为 4 个独立菜单；B 保留知识页但把 PC/Analysis 提为独立顶级；C 维持现状 | **B**：KB 留原页，PC 与 Analysis 各自独立入口（改造成本低于 A，且立即消除 DOM-1/2/3） | **是** |
| Q-02 | PC 只有只读页签，无管理面 | 「产品级正式认知」需要对象/提案/冲突管理面 | A 新建 PC 领域页（总览+对象+提案+冲突）；B 仅提升现有只读列表为独立页；C 延后 | **A（分期：先 B 再 A）** | **是** |
| Q-03 | Project Cognition 页面存在但零入口 | 权威页面不可达；同时 KB 页的 pc 页签叫「认知」，概念撞车 | A 在项目模块增加「项目认知」菜单；B 并入「知识」作为子页签；C 合入项目总控台 | **A**（与 PC 命名区分：项目认知 vs 产品认知） | **是** |
| Q-04 | Project Baseline 零入口 | 同上 | A 增加菜单/总控台入口；B 并入项目认知页作为阶段；C 暂不暴露 | **A 或 B**，取决于 baseline 是否被视为认知流程终点 | **是** |
| Q-05 | Analysis 无独立域面 | 报告与 KB 文档同页；报告正文借住 KB 修订 | A 独立「分析」菜单（Run/Report/Finding）；B 留在知识页作首屏页签；C 归入人工审核中心 | **A**（长态），短期 B 可接受但须在基线登记为过渡 | **是** |
| Q-06 | 管理组 4 项对所有成员可见 | 导航无角色门 | A 按角色隐藏（Owner/Admin 才见）；B 全部可见但页内空态/403；C 移出主菜单进账户菜单 | **A + C 混合**：管理组仅 Owner/Admin 可见，团队 AI 配置归团队设置 | **是** |
| Q-07 | 团队成员管理在 `/admin/members`，团队设置有 `/settings/team` | 团队域被拆两处 | A 统一到 `/settings/team/*`；B 统一到 `/admin/*`；C 保留双入口 | **A**（团队域=设置域，管理组只留平台级） | **是** |
| Q-08 | 「项目总览 `/dashboard`」实为 Team Dashboard | 一级菜单名误导层级 | A 改名「团队总览」；B 改为 `/team`；C 保持 | **A**（最小改动，消除 HIE-3） | 否（但强烈建议） |
| Q-09 | 「知识审阅」「代码检索」「收件箱」命名与实际能力不符 | 命名不一致 7 条 | A 逐项改名；B 保留但补副标题 | **A** | 否 |
| Q-10 | `/inbox` 占位 + `/notifications` 真实 | 通知/收件箱二元 | A 删除 `/inbox`；B 把 `/inbox` 指向通知中心；C 保留占位 | **A 或 B** | 否 |
| Q-11 | 项目上下文三套读取路径 | 一致性风险 | A 统一走 `shell.currentProjectId`（URL 仅作初始注入）；B 统一走 `?project_id=`；C 维持 | **A** | 否（但影响 IA 稳定性） |
| Q-12 | 无独立项目内导航（项目模块挂在全局左栏） | 多项目并行时左栏臃肿（项目头被停用的原因） | A 引入项目内二级导航；B 保持全局分组 + 项目切换器；C 折中：折叠项目模块组 | **待裁决**：取决于是否支持并行多项目工作 | **是（若目标为多项目并行）** |
| Q-13 | 是否存在 Team 级 KB / PC | 当前全部按 `project_id` 过滤 | A 保持项目级；B 引入团队级知识库 | **A（V1）** | 否 |
| Q-14 | Analysis 报告域归属（KB 修订 vs 独立 Report Revision） | `authority-matrix` 标「域归属未定，D01 目标态 MIGRATE」 | A 按 D01 迁移独立 Report Revision；B 长期借住 KB | **A**（与后端 D01 对齐） | **是（与 Q-05 联动）** |

---

## D-09 证据索引

### 9.1 导航与壳

| 结论 | 文件 | 符号/行号 |
|---|---|---|
| 导航树为菜单+路由单源 | `ai-frontend/src/router/navigation.ts` | `navGroups` 84-271 |
| 4 分组 16 项 | 同上 | `personal` 85-102 / `workspace` 103-128 / `modules` 129-225 / `admin` 226-271 |
| 无二级菜单数据 | 同上 | `NavChild` 60-67；无 `children` 赋值 |
| 管理路径判定（仅用于隐藏 AI 面板） | 同上 | `isManagementPath` 305-314 |
| 助手隐藏前缀 | 同上 | `ASSISTANT_HIDDEN_PREFIXES` 321；`isAssistantHiddenPath` 326-331 |
| 路由由导航派生 | `ai-frontend/src/router/index.ts` | `navItemRoutes` 59-86；`buildRoutes` 89-96 |
| 缺省占位页 | 同上 | 82；`PlaceholderView` |
| 全局守卫仅 token | 同上 | `router.beforeEach` 618-643；`PUBLIC_LAYOUTS` 600 |
| 设计目录仅 DEV | 同上 | `designRoutesReady` 581-592 |
| 左栏无角色过滤 | `ai-frontend/src/components/shell/SideNav.vue` | `v-for="item in group.items"` 205 |
| 项目上下文头停用 | 同上 | 191-203（注释块） |
| 子菜单分支（不可达） | 同上 | 256-276 |
| 顶栏团队切换 | `ai-frontend/src/components/shell/TopBar.vue` | `<TeamSwitcher />` 219 |
| 顶栏项目切换 | 同上 | `<ProjectSwitcher>` 224-234 |
| 不引入 Workspace Switcher | 同上 | 注释 41-42 |
| 账户菜单去向 | 同上 | `ACCOUNT_SELECT_ROUTES` 170-176 |
| 助手按钮（不跳转） | 同上 | 295-305 |
| 项目上下文优先级（URL>localStorage>首个） | `ai-frontend/src/components/shell/WorkspaceShell.vue` | 74-91 |
| 全局搜索落点 `/project/knowledge` | 同上 | `searchFn` 101-118 |
| 壳列可见性单源 | `ai-frontend/src/components/shell/use-shell-chrome.ts` | 20-63 |
| 面包屑手工传入 | `ai-frontend/src/components/shell/PageFrame.vue` | 23-32；`PageHeader.vue` 12-35 |
| 右栏 5 页签 | `ai-frontend/src/components/shell/ContextPanel.vue` | `TABS` 126-132 |

### 9.2 四域

| 结论 | 文件 | 符号/行号 |
|---|---|---|
| KB/PC/Analysis 同页三页签 | `ai-frontend/src/views/project/knowledge/KnowledgeView.vue` | `TabKey` 45；`tabItems` 161-175 |
| 分析报告页签首屏 | 同上 | 411-415；`AnalysisReportsPanel` |
| KB 文档列表/详情/确认/摄入 | 同上 | 505-663；`confirmPublish` 96-123；`runIngest` 336-378 |
| PC 对象只读列表 | 同上 | 665-736；`openPcDetail` 132-145 |
| 报告状态机与批量发布 | `ai-frontend/src/views/project/knowledge/AnalysisReportsPanel.vue` | `GROUPS` 72-77；`doClose` 280-299；`decideProposal` 240-261 |
| Project Cognition 三区 | `ai-frontend/src/views/project/cognition/CognitionPage.vue` | 183-359；API 14-24；`projectId` 56-63 |
| Project Baseline | `ai-frontend/src/views/project/baseline/BaselinePage.vue` | 21-25；`projectId` 37-43 |
| KB/PC/检索端点 | `ai-frontend/src/api/knowledge.ts` | 46, 53, 69, 92, 118, 168, 200, 259, 280, 316 |
| 确认门权限说明 | 同上 | 58-63；264-269 |
| 代码发现（AI 只产 finding） | `ai-frontend/src/views/project/discovery/DiscoveryView.vue` | 24-36 |
| 设计分析（需求覆盖/ADR 引用） | `ai-frontend/src/views/project/design/analysis/DesignAnalysisView.vue` | 19-29 |
| 初始化卡「代码分析」跳仓库设置 | `ai-frontend/src/views/project/command-center/components/InitializationCard.vue` | 125-132 |

### 9.3 文档依据

| 结论 | 文件 | 章节 |
|---|---|---|
| 四域划分与写入权 | `docs/architecture/four-domain/authority-matrix-v1.md` | §1.1 KB 9-18；§1.3 ANALYSIS 28-38；§1.4 PROJECT_COGNITION 40-46；§1.5 PC 48-59 |
| 报告正文借住 KB 修订、「域归属未定」 | 同上 | §2 表 76 行；§3 结论 83 |
| Cognition 决策门 P0 | 同上 | §1.4 45 行 |
| 前端审批权威三分 | `docs/architecture/four-domain/four-domain-audit-report-v1.md` | §9 60-62 |
| 15 问中「报告正文在 KB」「project_knowledge 属 Project Cognition」 | 同上 | §10 第 4、8 问 69、73 行 |
| kb.memory 审核与 cognition 确认入口缺口 | 同上 | §9 62 行（U25） |
| 四域对象清单 | 同上 | §10 第 1 问 66 行 |

### 9.4 其他

| 内容 | 文件 |
|---|---|
| 菜单中文文案 | `ai-frontend/locales/zh-CN/shell.yaml:199-223` |
| 审核中心文案 | `ai-frontend/locales/zh-CN/review.yaml:2-3, 13-18` |
| 设置域侧栏条目 | `ai-frontend/src/views/settings/nav.ts:31-37, 49-51, 65-72` |
| Team Dashboard 定位 | `ai-frontend/src/views/dashboard/TeamDashboardView.vue:23-27` |
| 认证路由 9 条 | `ai-frontend/src/router/auth.ts:24-82` |
| 设计目录 43 条 | `ai-frontend/src/router/design.ts:81-436` |

---

## 8. 最终总结

### 8.1 数量口径

1. **一级菜单（导航项）16 个**；若按左栏分组计则为 **4 个分组**（个人 / 工作区 / 项目模块 / 管理）。【事实】`navigation.ts:84-271`
2. **二级菜单（导航树级）0 个**。等价二级导航来自：项目设置侧栏 6、个人设置侧栏 4、团队设置侧栏 1、右栏页签 5、工作区页内页签（Knowledge 3 / Tasks 5 / Design 4 / Execution 7）。【事实】
3. **有效页面路由 67 个**（应用壳内页面组件路由，含 1 个占位页 `/inbox`），另有 9 条重定向、1 条 404 兜底；认证 9 条；设计目录 43 条（仅 DEV）。应用壳路由记录合计 77 条。【事实】计数依据见 D-02。
4. **有完整页面入口的领域**：需求（含评审/基线/历史/影响）、任务（含拆解/规划/准备）、技术设计（含 ADR/实施准备）、执行（含 Run/协作/介入/授权/复核/历史）、人工审核、仓库/SSH/Runner、项目设置 AI 五节、个人/团队设置。**四域中的 KB/PC/Analysis 均在入口层面不完整。**【事实+判断】

### 8.2 四域当前在哪

5. **KB** → 「知识审阅」`/project/knowledge` 的 **kb 页签**；真 API `/v1/kb/documents*`；无独立菜单、无 Team 级入口、无 Source/Memory 面。【事实】
6. **Product Cognition** → 同一路由的 **pc 页签**（只读对象列表 + 分析报告页签内的提案审批）；**无独立页面**。【事实】
7. **Project Cognition** → 独立页 `/project/cognition`，真 API，**零入口（孤立路由）**，实际不可达。【事实】
8. **Analysis** → 无独立域；报告在 `/project/knowledge` 首屏页签；另有设计分析 `/project/design`、审核中心「报告待审阅」、总控台「代码分析」动作三处衍生入口；报告正文借住 `kb.document_revision.content`。【事实】

### 8.3 当前主导航最大的问题

9. **领域权威被 UI 抹平 + 权威页面不可达。** 具体三条：
   - 四域中 3 个（KB/PC/Analysis）挤在 `/project/knowledge` 一页，菜单名「知识审阅」只描述 KB；
   - Project Cognition / Baseline 页面完整但**无任何入口**；
   - 管理组对全员可见，导航层无角色门 —— 「菜单隐藏」这一层安全缓冲也不存在。
   【判断】因此当前 IA 的问题不是「层级太浅」，而是**领域边界与访问控制同时失守**。

### 8.4 建立正式 IA 基线最重要的产品裁决（按优先级）

10. 必须裁决的 8 项（详见 D-08）：
   1. **Q-01** KB / PC / Analysis 是否拆分为独立顶级域（决定整个左栏骨架）。
   2. **Q-03** Project Cognition 的入口位置与命名（与 PC 如何区分）。
   3. **Q-05 + Q-14** Analysis 是否独立成域，以及报告正文权威是否按 D01 迁移（后端联动）。
   4. **Q-02** Product Cognition 是否建立完整管理面（对象/提案/冲突）。
   5. **Q-06** 管理组的角色可见性策略（导航门 or 页内 403）。
   6. **Q-07** 团队域收敛到 `/settings/team/*` 还是 `/admin/*`（消除 HIE-1）。
   7. **Q-04** Project Baseline 是独立入口还是认知流程终点。
   8. **Q-12** 是否引入项目内二级导航（决定多项目并行的可持续性）。

---

## 9. 本报告的边界与未决证据

- **只读**：未修改任何代码/路由/菜单/数据库；本文所有行号对应审计时点快照。
- **UNKNOWN 清单**：
  - `/admin/settings`、`/admin/announcements`、`/settings/team/ai-providers` 的后端逐端点鉴权；
  - 67 个页面路由的逐条 API 授权矩阵（本次仅按 `authority-matrix` 抽样 4 类能力）；
  - `/project/command-center` 的实际链入位置（注释称「项目切换后落地页」，未找到 `RouterLink` 证据）；
  - 移动端/窄屏导航行为（未发现抽屉实现，但未运行验证）；
  - `docs/audit/ia-2026-11-16/` 之外是否存在更早的 IA 基线文档（未做全库文档穷举）。
- **口径声明**：`IMPLEMENTED` 的判定要求「存在真实 API 调用链」；仅静态渲染或 `PlaceholderView` 一律标记 `SHELL`/`PLANNED`。

---

*报告结束 · V1.0 · 2026-11-16*
