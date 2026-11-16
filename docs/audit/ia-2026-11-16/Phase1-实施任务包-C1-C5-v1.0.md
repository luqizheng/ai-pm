# Phase 1 实施任务包 C1~C5 V1.0

> 日期：2026-11-16
> 性质：开发 Agent 可直接执行的任务卡。每卡 = 一个 commit，含源码定位 / 修改边界 / 实施步骤 / 测试命令 / 验收条件 / 禁止事项 / 回滚条件。
> 上游：《全局导航 IA 与 Phase 1 改造要求修订稿 V1.2》（裁决与验收主表）→《Phase 1 执行任务书 V1.1》（范围与开放项）→ 本任务包（执行粒度）。
> 基线依据：Phase 0 核验报告（行号为 2026-11-16 快照，执行时以符号定位为准）。

## 0. 使用说明

| 卡 | 内容 | 前置 | 状态 |
|---|---|---|---|
| C1 | 导航配置模型拆分 + 多源路由派生 | 无 | READY |
| C2 | Global Navigation 呈现 + `/dashboard` 改名 | C1 | READY |
| C3 | Project Navigation 单侧栏分层 + 紧凑 Project Switcher | C1、C2 | READY |
| C4 | Team 设置归属迁移 + isManagementPath 显式化 + Git 凭证占位 | C1 | READY |
| C5 | 助手隐藏全量回归 + `/inbox` 清理 + 注释订正 | C1~C4 | READY |

**公共纪律（每卡适用，不再重复写入卡内）**：

1. 一卡一 commit，独立可回滚——revert 本卡后系统仍处正确状态。
2. 质量基线（不得扩大）：`prettier --check src` 21 文件红 / `check-i18n-hardcode` 39 处红 / `check-i18n-namespace` 绿 / `styles/layout.test.ts` 1F13P。存量修复禁止混入。
3. 每 commit 必跑：`pnpm -s vitest run src/router/ src/components/shell/`、`pnpm typecheck`、`node ../scripts/check-i18n-hardcode.js`、`node ../scripts/check-i18n-namespace.js`；涉及构建产物认知时另跑 Release Guard。
4. zh-CN 与 en 的 yaml 同步增删；`.vue` 改后 `git diff` 目检模板闭合。
5. 助手可见性改动必更 `WorkspaceShell.test.ts` 参数化用例（AGENTS 规则 5 步骤 3）。
6. 涉 UI 的任务交付截图/录屏 + 变更文件清单 + 路由变更清单 + 测试输出摘要 + 回滚声明。

---

## C1 — 导航配置模型拆分 + 多源路由派生（P1-01）

### 源码定位

| 文件 | 符号/位置 | 现状 |
|---|---|---|
| `src/router/navigation.ts` | `navGroups` :84-271（4 组 16 项）、`flatNavItems` :274、`isNavItemActive` :282-288、`isManagementPath` :305-314（**动态派生 admin 组**）、`ASSISTANT_HIDDEN_PREFIXES` :321、`isAssistantHiddenPath` :326-331 | 单一 `navGroups` 数组 |
| `src/router/index.ts` | `navItemRoutes` :59-86（children→父 redirect 首子+子平铺；缺省 PlaceholderView :82）、`buildRoutes` :89-546（:92-96 遍历 navGroups） | 单源遍历 |
| `src/router/navigation.test.ts` | :33/:41/:49/:70 断言 `flatNavItems` 路径与 label | 需保持通过（数据不变则不需改） |
| `src/router/router.test.ts` | :17 遍历 `flatNavItems` 验证路由可达 | 同上 |
| `src/components/shell/SideNav.vue` | :15-21 导入 `navGroups`/类型 | C1 不改本文件 |
| `src/components/shell/WorkspaceShell.vue` | :148-191 `searchCommands`（label 用 `shell.app.side.item.*` 与 `global.search.commands.*`） | C1 不改本文件 |

### 修改边界

- **允许改**：`navigation.ts`（内部结构重构 + 新增导出）、`index.ts` 的 `buildRoutes` 遍历段（:89-96 一带）、新增 `src/router/navigation/` 拆分文件（可选）。
- **禁止改**：`navItemRoutes` 派生规则、`beforeEach`、`isManagementPath` 实现、`SideNav.vue`、任何 `.vue`、任何业务页面、路由路径集合。

### 实施步骤

1. 拆三个具名配置源（同文件或 `navigation/` 子模块，自行判断；**不要求机械拆分**）：
   - `globalNavConfig`：personal + workspace + admin 组（**四项 admin 项本卡原样保留**，C4 才迁出）；
   - `projectNavConfig`：现 modules 组 8 项 + C3 将启用的结构占位（本卡仅数据搬家）；
   - `settingsNavConfig`：项目设置子树骨架（V1.2 §5 + AI 配置与执行策略项，见任务书 V1.1 §6 遗漏注记；**不含独立 ssh-keys 条目**）。
2. 保留兼容导出 `navGroups = [...三源展开]`、`flatNavItems`——**保证 SideNav 与两个测试文件零改动即通过**（零视觉变化是本卡验收核心）。
3. `buildRoutes()` 改为遍历三源（内部仍逐 group 调 `navItemRoutes`）。
4. 路由表自检：临时打印改造前后路径集合 diff，确认一致后移除调试代码。

### 测试命令

```bash
pnpm -s vitest run src/router/ src/components/shell/
pnpm typecheck
node ../scripts/check-i18n-hardcode.js && node ../scripts/check-i18n-namespace.js
pnpm -s exec prettier --check src
```

### 验收条件

- [ ] 上述四命令结果 ≤ 基线（测试全绿、typecheck 绿、红项不扩大）；
- [ ] 路由表路径集合与改造前 diff 为空；
- [ ] SideNav 渲染输出与改造前一致（既有测试零改动通过即证明）；
- [ ] `flatNavItems`/命令面板数据无断裂。

### 禁止事项

- 禁止改任何渲染/布局/文案（本卡用户不可感知）；
- 禁止改 `isManagementPath` 语义（C4 才动）；
- 禁止新增/删除路由路径；
- 禁止引入三套 Sidebar 概念（V1.2 R2-D1：三源服务同一侧栏）。

### 回滚条件

`git revert` 后 `navGroups`/路由表/渲染与现状完全一致。

---

## C2 — Global Navigation 呈现 + `/dashboard` 改名（P1-02 前两区）

### 源码定位

| 文件 | 符号/位置 | 现状 |
|---|---|---|
| `src/components/shell/SideNav.vue` | 模板 `:151` `v-for group in navGroups`、`showDivider` :77-80（**硬编码 `group.id === "modules" || "admin"`**）、`:132` 控制行硬编码「WORKSPACE」文案 | 四组平铺渲染 |
| `src/router/navigation.ts` | `workspace` 组 :103-128（dashboard :107-117、projects :118-127）、personal 组 :85-102 | — |
| `locales/zh-CN/shell.yaml` | `app.side.item.dashboard: 项目总览`（:209 附近） | 待改「团队总览」 |
| `locales/en/shell.yaml` | 同键位（en 文案同步） | 待改 Team Overview 语义 |
| `src/router/index.ts` | :625 登录 redirect `"/dashboard"` | **不动**（只改名不改路径） |
| `src/components/shell/SideNav.test.ts` | :25 标签顺序断言、:80 分隔线数=2 | 断言需随渲染更新 |

### 修改边界

- **允许改**：`SideNav.vue`（区域呈现/分隔逻辑/标题）、`navigation.ts`（labelKey 文案键值、组 label）、两份 `shell.yaml`、`SideNav.test.ts`。
- **禁止改**：路由路径、`/dashboard` 组件、`router/index.ts`（含 :625）、Grid 模板、`use-shell-chrome.ts`、`/inbox` 项（C5 才删）。

### 实施步骤

1. SideNav 按区域呈现 Personal / Team（区域标题来自组 label；分隔线逻辑从硬编码组 id 改为「区域边界」驱动，**同步更新 SideNav.test :80 断言**）。
2. `side.item.dashboard` zh「团队总览」/ en 同步；核对引用该 key 的测试与 `searchCommands`（`cmd-open-project` 等 label 不受影响则不动）。
3. admin 组在本卡保持原位呈现（C4 迁移）。
4. 「WORKSPACE」控制行文案：可顺手改为区域标题派生，**不得**扩大为样式重构。

### 测试命令

公共四命令 + `pnpm -s vitest run src/components/shell/SideNav.test.ts src/components/shell/TopBar.test.ts`。

### 验收条件

- [ ] Personal / Team 区域分层呈现，不与项目模块混排（P1-AC-01 部分）；
- [ ] 侧栏 232/60 不变、无新列（P1-AC-02/03）；
- [ ] 「团队总览」生效、`/dashboard` 路径与登录跳转不变；
- [ ] 公共四命令 ≤ 基线。

### 禁止事项

- 禁止动 Current Project 区域（C3）；
- 禁止改 admin 组构成（C4）；
- 禁止删 `/inbox`（C5）；
- 禁止新增 Grid 分支或改列宽（R2-C10）。

### 回滚条件

revert 后恢复四组平铺渲染与「项目总览」文案，无 i18n 键残留。

---

## C3 — Project Navigation 单侧栏分层 + 紧凑 Project Switcher（P1-02/03/04）

### 源码定位

| 文件 | 符号/位置 | 现状 |
|---|---|---|
| `src/components/shell/SideNav.vue` | **:189-203 被注释的旧项目上下文头**（含 `ProjectSwitcher variant="header"` 用法示例：`:projects`/`:current-id`/`:switching-to-id`/`@select`）——紧凑头的直接参照，**禁止复刻其大尺寸形态**；`:116-119` `onSelectProject`（走 `shell.requestProjectSwitch(id, assistant.activeConversationId !== null)`）；`:49-55` reviewCenter 徽标 watch；`:90-96` `itemBadge` | 项目头停用、modules 组平铺 |
| `src/components/shell/ProjectSwitcher.vue` | `variant="header"` 变体、props `projects/currentId/switchingToId`、`@select` | **已具备头部变体，直接复用** |
| `src/stores/shell.ts` | `requestProjectSwitch` :365-372、`switchProject` 两阶段 :345-357、switching 中 `currentProject=null` :155-158、`switchingProject` :161-163 | 切换守卫链（只复用，不改） |
| `src/router/index.ts` | 手工路由样例 :106-140（cognition/baseline/command-center）；PlaceholderView 缺省 :82；`/project/settings` redirect→general :284-289；`/project/settings/ai` redirect→roles :327-330 | 新占位路由照此注册 |
| `src/api/scope.ts` | :46-48 `scope.currentProjectId` 派生自 shell | **不要**在侧栏用它判显隐（用 `shell.currentProjectId`，R2-D4） |
| `src/components/shell/SideNav.test.ts` | :25/:42/:49/:71/:80 各断言 | 随结构更新 |
| `src/views/project/devenv/BootstrapView.vue` | 纯设备页（Phase 0 Q12） | Devices 暂指旧路由，不改本文件 |

### 修改边界

- **允许改**：`SideNav.vue`（Current Project 区域 + 紧凑头 + 分组结构）、`projectNavConfig`/`settingsNavConfig`（C1 产物）、`router/index.ts`（**仅新增** 3 条 PLANNED 占位路由）、新增占位路由的 i18n key、`SideNav.test.ts`/`WorkspaceShell.test.ts`（新增用例）、`reviewCenter` 徽标挂接点。
- **禁止改**：`stores/shell.ts`（切换机制零改动，R2-C03~C05）、`use-shell-chrome.ts`、Grid 模板/列宽（R2-C10）、现有页面组件、`/project/settings/ssh-keys` 相关、assistant chrome 逻辑。

### 实施步骤

1. **Current Project 区域条件渲染（R2-D4）**：`v-if="shell.currentProjectId"` 包裹整区；空时 Personal/Team 照常、无灰置菜单、无伪造上下文。
2. **紧凑 Project Switcher（R2-D3/P1-03）**：区域顶部恢复 `ProjectSwitcher variant="header"`（参照 :189-203 注释块），套紧凑样式类；`@select` 必须接 `onSelectProject`（现有守卫链）；长名截断；折叠态 60px=图标入口+Tooltip（IconTooltip 既有机制）；切换中显示 `switchingProject` 态、不渲染旧项目选中项。
3. **区域三段（R2-D5）**：`项目总控台`（`/project/command-center`，置顶单条）→ `业务模块组`（居中）→ `项目设置组`（置底）。
4. **业务模块 10 项**（`projectNavConfig` 终稿顺序）：
   - 直连 6 项：需求 `/project/requirements`、技术设计 `/project/design`、任务管理 `/project/tasks`、AI 执行中心 `/project/execution`、代码发现 `/project/discovery`、人工审核中心 `/project/review-center`（徽标随迁）；
   - KB → `/project/knowledge`（C-1 最终定位；页面仍三合一，**不得**伪装已拆分）；
   - 项目认知 → `/project/cognition`（真页直连）；
   - Analysis → **PLANNED** `/project/analysis`；产品认知 → **PLANNED** `/project/product-cognition`。
5. **新增 3 条占位路由**（`PlaceholderView` + `planned` 标记 + meta.title i18n）：`/project/analysis`、`/project/product-cognition`、`/project/settings/devenv/executors`。注册进配置源（路由仍派生，不绕开 buildRoutes）。
6. **项目设置组（置底）**，条目映射：基础设置→`/project/settings/general`（proposed 现状）；成员→`/project/settings/members`（PLANNED 占位现状）；仓库→`/project/settings/repositories`（**凭证绑定仅页面级预留，导航不设子项**——NavChild 机制会把父项变 redirect，与真实父页面冲突，此为边界决策）；开发环境→**可展开 NavItem** `path:/project/settings/devenv`（redirect 首子）children=[Devices→`/project/devenv`（Phase 3 才迁路径）、Executors→占位、Runners→`/project/settings/runners`]；AI 配置与执行策略→`/project/settings/ai`（→roles；V1.2 遗漏注记按含入处理）。
7. **折叠态**：区域随整栏折叠机制走（图标+Tooltip），不得因折叠丢失项目识别（紧凑头有折叠变体）。
8. `showDivider` 改区域边界驱动（替换硬编码组 id），同步 SideNav.test 断言。

### 测试命令

公共四命令 + `pnpm -s vitest run src/components/shell/SideNav.test.ts src/components/shell/WorkspaceShell.test.ts src/stores/reviewCenter.test.ts`。

新增用例最低集：
- 无项目（`currentProjectId=null`）不渲染 Current Project 区；
- 有项目渲染且顺序=总控台/模块组/设置组；
- 切换走守卫（mock 活跃会话 → 弹确认；取消后上下文不变）；
- 占位路由渲染 PlaceholderView 且导航项标 planned；
- `/project/cognition`、`/project/knowledge` 深链可达。

### 验收条件

- [ ] P1-AC-01/02/03/04/05/06/07/13/14 对应项通过；
- [ ] 切换中无旧项目选中态与数据（P1-03 禁止项逐条自查）；
- [ ] `isManagementPath` 未变（settings 前缀覆盖新增占位路由，助手隐藏行为不变）；
- [ ] 公共四命令 ≤ 基线。

### 禁止事项

- 禁止绕过 `requestProjectSwitch` 直接写 `currentProjectId`；
- 禁止在侧栏重造切换守卫/用路由匹配替代上下文状态（P1-03 禁止清单）；
- 禁止新增 Grid 分支/第二侧栏/新 data-属性（R2-C01/C02/C10）；
- 禁止恢复旧大尺寸项目头；
- 禁止把「Repository 凭证绑定」放进开发环境（R-1）；
- 禁止给 Analysis/PC 占位项挂任何真实业务 UI。

### 回滚条件

revert 后 SideNav 恢复 C2 状态（Personal/Team 区 + 旧 modules 组）；占位路由随配置派生自动消失，无孤儿路由。

---

## C4 — Team 设置归属迁移 + isManagementPath 显式化 + Git 凭证占位（P1-05 前置）

### 源码定位

| 文件 | 符号/位置 | 现状 |
|---|---|---|
| `src/router/navigation.ts` | admin 组 4 项 :226-270；`isManagementPath` :305-314 | 动态派生 admin 组 |
| `src/router/index.ts` | settings 域手工注册 :452-536（`/settings/team/general` 先例 :532-536） | 成员页挂在 admin 组派生 |
| `src/views/settings/team/SettingsTeamMembersView.vue` | 视图本体（路径迁移，组件复用） | 现路由 `/admin/members` |
| `src/views/settings/nav.ts` | `TEAM_APP_NAV` :49-51（仅 general）、`TEAM_ICONS` :54-56 | 待增 members / aiProviders |
| `src/views/settings/SettingsTeamLandingView.vue` | :10-11 自述「成员、邀请已迁 /admin/members」 | 文案订正（HIE-1） |
| `locales/*/…` | `settings.nav.*`、新 `shell.app.side.item.gitCredentials` | zh/en 同步 |
| `src/components/shell/WorkspaceShell.test.ts` | :57-79 参数化用例范式 | 追加路径 |
| `src/router/index.ts` :359-382 | 旧路径 redirect 先例（repository/environments 五条） | `/admin/members` redirect 照此 |

### 修改边界

- **允许改**：`navigation.ts`（admin 组收窄 + 显式规则清单）、`index.ts`（成员路由迁挂 + redirect + credentials 占位路由）、`settings/nav.ts`、LandingView 文案、i18n、两类测试文件。
- **禁止改**：`use-shell-chrome.ts`、四处 chrome 消费点逻辑、`/admin/settings` 与 `/admin/announcements`（NAV-0103 待核验，**本卡不迁**）、后端任何代码、成员视图组件内部。

### 实施步骤

1. **成员迁移**：admin 组删除 `/admin/members` 项 → settings 域手工注册 `/settings/team/members`（组件 `SettingsTeamMembersView`）→ 旧路径 redirect → `TEAM_APP_NAV` 增 `{ id: "teamMembers", to: "/settings/team/members" }` + 图标 `UsersRound` + i18n。
2. **AI Provider 归组**：admin 组删除该项 → `TEAM_APP_NAV` 增 `aiProviders`；**路由 `/settings/team/ai-providers` 不变**。
3. **Team 设置子树（V1.2 §5）**：Team 区「团队设置」可展开 = 团队常规（`/settings/team/general`）/ 成员与邀请（新路径）/ AI Provider / **Git 凭证（PLANNED 占位：`/settings/team/credentials` → PlaceholderView，标 planned；不实现任何凭证 UI）**；「团队公告」不进（NAV-0103 待核验）。
4. **isManagementPath 显式化（行为等价 4a）**——规则清单（每条含语义注释，exact 或 `path+"/"` 前缀）：

```ts
const ASSISTANT_HIDDEN_RULES = [
  { path: "/admin/settings", note: "平台设置：现状禁用" },
  { path: "/admin/announcements", note: "公告管理：现状禁用；迁团队后仍禁用" },
  { path: "/project/settings", note: "项目设置：完整前缀（覆盖 C3 新增 devenv/executors 占位）" },
  { path: "/settings/team/members", note: "原 /admin/members 迁入，等价禁用" },
  { path: "/settings/team/ai-providers", note: "原 admin 组迁入，等价禁用" }
];
```

   `isAssistantHiddenPath` 签名与消费方不变；**禁止**裸 `/settings/team` 前缀（4b 未确认）。`/admin/announcements` 未来迁移时同步替换路径。
5. LandingView 自述文案改新路径；i18n zh/en 全同步。

### 测试命令

公共四命令 + `pnpm -s vitest run src/components/shell/WorkspaceShell.test.ts` + `pnpm typecheck`（AGENTS 规则 5 步骤 4）。

`WorkspaceShell.test.ts` 参数化追加（断言口径照 :57-79 现有范式）：
- 隐藏：`/settings/team/members`、`/settings/team/ai-providers`；
- **可见（行为锁）**：`/settings/team/general`、`/settings/team`、`/settings/team/credentials`；
- redirect 后隐藏：`/admin/members` → 目标页。

### 验收条件

- [ ] P1-AC-11（四处一致）+ AGENTS 规则 5 四处同步自查（顶栏按钮/AI Chat 列/ContextPanel/壳根 data-*）；
- [ ] 行为等价核对表：4a 前后助手可见性差异 = 仅「成员页换了 URL」；
- [ ] `/admin/members` 旧链接可用（redirect）；
- [ ] 公共四命令 ≤ 基线。

### 禁止事项

- 禁止迁移 `/admin/settings`、`/admin/announcements`；
- 禁止用裸 `/settings/team` 前缀做禁用匹配（4b 未决）；
- 禁止改 `use-shell-chrome.ts` 派生逻辑；
- 禁止实现 Git 凭证任何管理 UI（R-1 附二：后续阶段）；
- 禁止把 Team Membership 当 Project Member（V1.2 §11.11）。

### 回滚条件

revert 后 admin 组恢复四项、成员页回 `/admin/members`、助手可见性与现状逐路径一致（含 general 可见）。

---

## C5 — 助手隐藏全量回归 + `/inbox` 清理 + 注释订正（P1-05 回归 / P1-06 / P1-08）

### 源码定位

| 文件 | 符号/位置 | 现状 |
|---|---|---|
| `src/router/navigation.ts` | personal 组 `/inbox` 项 :95-100（badge:3）；`ASSISTANT_HIDDEN_RULES`（C4 产物） | — |
| `src/components/shell/WorkspaceShell.vue` | `cmd-open-inbox` :184-189 | 待删 |
| `locales/zh-CN/shell.yaml` | `app.side.item.inbox: 收件箱` :207；遗留 `inbox: 公告中心` :176（NAM-4 残留段，一并核对） | 待删/订正 |
| `locales/en/shell.yaml` | :176 `Announcements`、:207 `Inbox` | 同步 |
| `locales/zh-CN/global.yaml` + `en/global.yaml` | `search.commands.openInbox` :31 | 待删 |
| `src/router/index.ts` | :226-230 S04 注释漂移；:359-382 redirect 先例 | 注释订正 + 新 redirect |
| `src/stores/shell.ts` | :9 「展开 188px」过期注释 | 改 232px（以 tokens/index.css 为准） |
| `src/components/shell/SideNav.test.ts` | :71 徽标 `["3"]` 断言（inbox 徽标） | 随删除更新 |

### 修改边界

- **允许改**：navigation 配置（删 inbox 项）、`WorkspaceShell.vue`（删命令对象）、`router/index.ts`（redirect + 注释）、`stores/shell.ts`（注释）、三份 yaml、两类测试。
- **禁止改**：通知中心页面与顶栏通知入口行为、`searchFn`（:101-118 落点分流属 Phase 4）、ContextPanel、任何业务页面。

### 实施步骤

1. 删 `/inbox` 导航项 → redirect `/inbox → /notifications`（照 :359-382 先例）。
2. 删 `cmd-open-inbox` 命令对象；删三个 i18n 键（zh/en 同步；`shell.yaml:176` 残留段一并核对删除或订正）。
3. `SideNav.test` :71 徽标断言更新（inbox 徽标消失；reviewCenter 徽标用例保留）。
4. 注释订正：router :226-230（S04 breakdown 已接真）、stores :9（232px）。
5. **助手隐藏全量回归**：C4 清单全部路径跑参数化断言 + 四处一致性 + 专注模式交互用例（:81-105 现有用例保持绿）。
6. Release Guard：`pnpm build`（或构建命令）确认 dist 无 mock/fixtures/demo。

### 测试命令

公共四命令 + `pnpm -s vitest run src/components/shell/` + `pnpm typecheck` + Release Guard（`pnpm build` 后按 `build/release-guard.ts` 门禁输出确认）。

### 验收条件

- [ ] P1-AC-11/12/13 通过：`/inbox` 落通知中心、命令面板无 inbox、助手四处一致；
- [ ] P1-AC-15：ContextPanel Graph 页签与原功能不受影响（回归断言）；
- [ ] 通知中心原入口/行为不变；
- [ ] 公共四命令 ≤ 基线 + Release Guard 通过；
- [ ] V1.2 §14 安全八条抽查通过（切换无残留/守卫/取消恢复，用 C3 已有用例复核）。

### 禁止事项

- 禁止顺带改全局搜索落点（Phase 4，N-6）；
- 禁止动 Graph 页签（R-5）；
- 禁止把注释订正扩大为样式/结构重构（P1-08）；
- 禁止删通知下拉或改其跳转目标。

### 回滚条件

revert 后 `/inbox` 项、命令、i18n 键全部恢复，redirect 消失，无半删状态。

---

## 附 A：导航消费点总清单（P1-07，每卡收尾自查）

SideNav · `buildRoutes()` · `flatNavItems` · Command Palette（searchCommands + SearchPalette.test） · `use-shell-chrome.ts` · `WorkspaceShell`（data-* + 列渲染） · TopBar（TeamSwitcher/ProjectSwitcher/助手按钮/通知） · ContextPanel `availableObject` 路由推导（:188-206，Phase 1 只验证不重构） · ReviewCenter 跳转（ReviewCenterPage:549，Phase 4 前不动） · Global Search 落点（:115，Phase 4 前不动） · 登录 redirect（:625）。

## 附 B：行号漂移免责

全部行号为 2026-11-16 master 快照；执行时若行号不符，以符号名与相邻注释定位，并以 Phase 0 报告 §2 的行为描述为准；发现现实与任务包冲突 → 停手登记阻塞，不得自创语义。
