# Phase 1 实施任务包 C1~C5 V1.1（开发执行版·冻结）

> 日期：2026-11-16
> 性质：开发 Agent 可直接执行的任务卡。每卡 = 一个 commit，含源码定位 / 修改边界 / 实施步骤 / 测试命令 / 验收条件 / 禁止事项 / 回滚条件。
> 上游：《全局导航 IA 与 Phase 1 改造要求修订稿 V1.2》→ R-1 裁决 + 现状差异登记 → 任务书 V1.1 → 本任务包 V1.0（已被本版取代）。
> **V1.0 → V1.1 修订（审查意见五项全落）**：①执行顺序**串行化 C1→C2→C3→C4→C5**（C3/C4 同触 navigation.ts+index.ts，禁止并行）；②C1 增「存储顺序≠视觉顺序」约束 + admin 遗留入口独立标识；③C3 增 devenv 父子路由五项验收 + planned 标记消费验证；④C4 增助手可见性行为等价表（含 AI Provider 现状源码实证）；⑤C5 增 `shell.nav.inbox`（公告中心）先查引用禁盲删 + `/inbox` 三态测试。

## 三条不可动摇原则（每卡开头自查）

1. **配置源拆分 ≠ 导航容器拆分**：只有一个左侧导航栏（R2-D1/D2）。
2. **路由迁移 ≠ 权限迁移**：导航归属变化不改变任何后端权限边界（V1.2 §14.6）。
3. **占位路由 ≠ 业务能力已实现**：Analysis / Product Cognition / Executors / Git 凭证在 Phase 1 只是规划入口（V1.2 §11.12）。

## 0. 使用说明

| 序 | 卡 | 内容 | 前置（**严格串行**） | 状态 |
|---|---|---|---|---|
| 1 | C1 | 导航配置模型拆分 + 多源路由派生 | 无 | READY |
| 2 | C2 | Global Navigation 呈现 + `/dashboard` 改名 | C1 | READY |
| 3 | C3 | Project Navigation 单侧栏分层 + 紧凑 Project Switcher | C2 | READY |
| 4 | C4 | Team 设置归属迁移 + isManagementPath 显式化 + Git 凭证占位 | C3 | READY |
| 5 | C5 | 助手隐藏全量回归 + `/inbox` 清理 + 注释订正 | C4 | READY |

**串行化原因（审查意见二.1）**：C3 与 C4 都修改 `navigation.ts` 与 `index.ts`，并行会产生合并冲突/互相覆盖。正式执行顺序 = 上表序号，**禁止并行**、禁止乱序。

**公共纪律（每卡适用）**：

1. 一卡一 commit，独立可回滚——revert 本卡后系统仍处正确状态。
2. 质量基线（不得扩大）：`prettier --check src` 21 文件红 / `check-i18n-hardcode` 39 处红 / `check-i18n-namespace` 绿 / `styles/layout.test.ts` 1F13P。存量修复禁止混入。
3. 每 commit 必跑公共四命令：`pnpm -s vitest run src/router/ src/components/shell/`、`pnpm typecheck`、`node ../scripts/check-i18n-hardcode.js`、`node ../scripts/check-i18n-namespace.js`；C5 另跑 Release Guard。
4. zh-CN 与 en 的 yaml 同步增删；`.vue` 改后 `git diff` 目检模板闭合。
5. 助手可见性改动必更 `WorkspaceShell.test.ts` 参数化用例（AGENTS 规则 5 步骤 3）。
6. 每 commit 交付：变更文件清单 + 路由变更清单 + 测试输出摘要 + 回滚声明 + 涉 UI 截图/录屏。

---

## C1 — 导航配置模型拆分 + 多源路由派生（P1-01）

### 源码定位

| 文件 | 符号/位置 | 现状 |
|---|---|---|
| `src/router/navigation.ts` | `navGroups` :84-271（4 组 16 项）、`flatNavItems` :274、`isNavItemActive` :282-288、`isManagementPath` :305-314（动态派生 admin 组）、`ASSISTANT_HIDDEN_PREFIXES` :321、`isAssistantHiddenPath` :326-331 | 单一 `navGroups` |
| `src/router/index.ts` | `navItemRoutes` :59-86、`buildRoutes` :89-546（:92-96 遍历） | 单源遍历 |
| `src/router/navigation.test.ts` | :33/:41/:49/:70 断言 `flatNavItems` | 数据不变则零改动通过 |
| `src/router/router.test.ts` | :17 遍历 `flatNavItems` 验证路由 | 同上 |
| `src/components/shell/SideNav.vue` | :15-21 导入；:151 `v-for navGroups`；`showDivider` :77-80 硬编码组 id | C1 不改本文件 |

### 修改边界

- **允许改**：`navigation.ts` 内部结构 + 新增导出（可选拆 `navigation/` 子模块）、`index.ts` 的 `buildRoutes` 遍历段。
- **禁止改**：`navItemRoutes` 规则、`beforeEach`、`isManagementPath` 实现、任何 `.vue`、路由路径集合、任何业务页面。

### 实施步骤

1. 拆三个具名配置源（Global / Project / Settings Config；同侧栏消费，R2-D1）：
   - `globalNavConfig`：personal + workspace + **admin 组（仅剩 `/admin/settings`、`/admin/announcements` 两项？否——本卡 admin 四项原样保留，C4 才迁出）**；
   - `projectNavConfig`：项目域全部项（结构按 C3 终稿预排，本卡仅数据搬家）；
   - `settingsNavConfig`：项目设置子树（V1.2 §5 + AI 配置与执行策略，按任务书 V1.1 §6 遗漏注记含入；不含独立 ssh-keys 条目）。
2. 保留兼容导出 `navGroups`、`flatNavItems`——SideNav 与两测试文件零改动通过。
3. `buildRoutes()` 改多源遍历；路由表自检 diff 为空后移除调试代码。
4. **【V1.1 新增】存储顺序 ≠ 视觉顺序约束**：
   - 三源的**数组存储顺序不决定渲染顺序**；渲染顺序由显式的「区域排序规则」决定：`Personal → Team → Current Project（条件显示，C3 启用）→ 遗留管理入口（尾部）`；
   - admin 遗留两项在配置数据中**独立标识**（如 `legacyPending: true` + 注释「NAV-0103 待核验」），渲染上**维持现有「管理」组原样**：不改名、不加「Platform Administration」之类新区块标题、不升格为平台管理分区（V1.2 §11.9 + 审查意见二.2）；
   - 在 `navigation.ts` 顶部注释写明该排序规则，C2/C3 实现区域渲染时以此为准。

### 测试命令

公共四命令（§0.3）+ 路由表 diff 自检（临时打印，提交前移除）。

### 验收条件

- [ ] 公共四命令 ≤ 基线（测试全绿、typecheck 绿）；
- [ ] **路由路径集合与原始集合完全一致**（diff 为空）；
- [ ] **三源合并后的区域排序规则已在配置层显式声明**（注释/常量），且当前渲染输出与改造前一致；
- [ ] admin 遗留两项带独立标识、未包装成新分区；
- [ ] `flatNavItems`/命令面板无断裂。

### 禁止事项

- 禁止改渲染/布局/文案；禁止改 `isManagementPath` 语义；禁止增删路由路径；
- 禁止让存储顺序隐式决定视觉顺序；
- 禁止把 admin 遗留项包装成 Platform Administration 分区；
- 禁止三套 Sidebar 概念（R2-D1）。

### 回滚条件

revert 后 `navGroups`/路由表/渲染与现状完全一致。

---

## C2 — Global Navigation 呈现 + `/dashboard` 改名（P1-02 前两区）

### 源码定位

| 文件 | 符号/位置 | 现状 |
|---|---|---|
| `src/components/shell/SideNav.vue` | 模板 :151 `v-for`、`showDivider` :77-80、:132「WORKSPACE」 | 四组平铺 |
| `src/router/navigation.ts` | workspace 组 :103-128 | — |
| `locales/zh-CN/shell.yaml` / `en/shell.yaml` | `app.side.item.dashboard`（zh :209 附近「项目总览」） | 待改「团队总览」/ Team Overview |
| `src/router/index.ts` | :625 登录 redirect `/dashboard` | **不动** |
| `src/components/shell/SideNav.test.ts` | :25 标签顺序、:80 分隔线=2 | 随渲染更新 |

### 修改边界

- **允许改**：`SideNav.vue`（区域呈现/分隔逻辑）、`navigation.ts`（组 label/排序规则消费）、两份 `shell.yaml`、`SideNav.test.ts`。
- **禁止改**：路由路径、`/dashboard` 组件、`router/index.ts`、Grid 模板、`use-shell-chrome.ts`、`/inbox` 项（C5）、admin 组构成（C4）。

### 实施步骤

1. SideNav 按 C1 排序规则呈现 Personal / Team 区（区域边界驱动分隔线，同步 :80 断言）。
2. `side.item.dashboard` 改名 zh/en；登录 redirect 不动。
3. admin 遗留两项**原样呈现**（沿用现「管理」组 label，不新增分区标题——审查意见三.C2）。
4. 「WORKSPACE」控制行文案可顺手区域化，不得扩大为样式重构。

### 测试命令

公共四命令 + `pnpm -s vitest run src/components/shell/SideNav.test.ts src/components/shell/TopBar.test.ts`。

### 验收条件

- [ ] Personal / Team 分层呈现、不混排（P1-AC-01 部分）；
- [ ] 232/60 不变、无新列（P1-AC-02/03）；
- [ ] 「团队总览」生效，`/dashboard` 路径与登录跳转不变（**只改展示语义**）；
- [ ] **admin 遗留入口未形成新的平台管理分区**（沿用原组呈现）；
- [ ] 公共四命令 ≤ 基线。

### 禁止事项

- 禁止动 Current Project 区（C3）、admin 组构成（C4）、`/inbox`（C5）、Grid（R2-C10）。

### 回滚条件

revert 后恢复四组平铺与「项目总览」文案，无 i18n 键残留。

---

## C3 — Project Navigation 单侧栏分层 + 紧凑 Project Switcher（P1-02/03/04）

### 源码定位

| 文件 | 符号/位置 | 现状 |
|---|---|---|
| `src/components/shell/SideNav.vue` | :189-203 注释块（`ProjectSwitcher variant="header"` 完整用法：`:projects`/`:current-id`/`:switching-to-id`/`@select`）；:116-119 `onSelectProject`（守卫链）；:49-55 徽标 watch；:90-96 `itemBadge`；:256-276 children 子菜单分支（**父项激活时才渲染**——isNavItemActive children 语义） | 项目头停用 |
| `src/components/shell/ProjectSwitcher.vue` | `variant="header"`、props、`@select` | 现成复用 |
| `src/stores/shell.ts` | `requestProjectSwitch` :365-372、两阶段 :345-357、switching 中 `currentProject=null` :155-158 | 只复用不改 |
| `src/router/index.ts` | 手工路由样例 :106-140；PlaceholderView 缺省 :82；`/project/settings` redirect→general :284-289；`/project/settings/ai` redirect→roles :327-330；**「路由表无重复路径」守卫**（:147 注释提及） | 占位路由照此注册 |
| `src/components/shell/SideNav.test.ts` / `WorkspaceShell.test.ts` | 断言与参数化范式（:57-79） | 随结构更新 |
| `src/views/project/devenv/BootstrapView.vue` | 纯设备页 | Devices 暂指旧路由 |

### 修改边界

- **允许改**：`SideNav.vue`（区域+紧凑头+分组）、`projectNavConfig`/`settingsNavConfig`、`index.ts` **仅新增** 3 条占位路由、占位路由 i18n、`SideNav.test.ts`/`WorkspaceShell.test.ts`、reviewCenter 徽标挂接点。
- **禁止改**：`stores/shell.ts`、`use-shell-chrome.ts`、Grid/列宽（R2-C10）、现有页面组件、ssh-keys 相关、assistant chrome 逻辑。

### 实施步骤

（继承 V1.0 全部步骤：条件渲染 R2-D4 → 紧凑 Switcher 复用守卫链 → 三段结构 → 10 项落位映射 → 占位路由 → 项目设置组 → 折叠态 → showDivider 区域化。）此处列 V1.1 强化点：

1. **开发环境父子结构**：可展开 NavItem `path:/project/settings/devenv`（redirect 首子）children=[Devices→`/project/devenv`、Executors→`/project/settings/devenv/executors`（PLANNED）、Runners→`/project/settings/runners`]。
2. **planned 标记必须可消费**：`planned?: boolean` 进 `NavItem` 类型，SideNav 渲染可见标识（复用 settings 侧栏 `proposed` 徽标范式），**SideNav.test 断言标识存在**——禁止只在 meta/config 里躺死字段（审查意见二.3）。

### 测试命令

公共四命令 + `pnpm -s vitest run src/components/shell/SideNav.test.ts src/components/shell/WorkspaceShell.test.ts src/stores/reviewCenter.test.ts`。

新增用例最低集：
- 无项目不渲染 Current Project 区；有项目渲染且顺序=总控台/模块组/设置组；
- 切换走守卫（活跃会话弹确认、取消上下文不变）；
- 占位路由渲染 PlaceholderView 且**导航项 planned 标识可见**；
- **【V1.1 新增】devenv 父子路由五项**：
  1. 访问 `/project/settings/devenv` 重定向到 Devices（`/project/devenv`）；
  2. Devices 页面仍由 `BootstrapView.vue` 承载（真设备 API）；
  3. `/project/devenv` 无路由冲突（重复路径守卫不触发）；
  4. 父项选中状态正确：位于任一子路由时父项 isNavItemActive=true 且无多激活；
  5. Runners 详情深链 `/project/settings/runners/:deviceId` 不回归；
- `/project/cognition`、`/project/knowledge` 深链可达。

### 验收条件

- [ ] P1-AC-01/02/03/04/05/06/07/13/14 对应项通过；
- [ ] **devenv 父子路由五项验收全过**（含 Phase 3 前提：旧路径 `/project/devenv` 保持可用、迁移时按兼容清单 C-7 再 redirect）；
- [ ] **planned 标记有真实 UI 消费且被测试断言**；
- [ ] 切换中无旧项目选中态与数据；`isManagementPath` 行为不变；
- [ ] 公共四命令 ≤ 基线。

### 禁止事项

- 禁止绕过 `requestProjectSwitch`；禁止侧栏重造守卫；禁止路由匹配替代上下文状态；
- 禁止新增 Grid 分支/第二侧栏/新 data-属性/恢复旧大项目头；
- 禁止把「Repository 凭证绑定」放进开发环境（R-1）；
- 禁止给占位项挂真实业务 UI；禁止删除 `/project/devenv` 旧路由。

### 回滚条件

revert 后 SideNav 恢复 C2 状态；占位路由随配置派生自动消失。

---

## C4 — Team 设置归属迁移 + isManagementPath 显式化 + Git 凭证占位（P1-05 前置）

### 源码定位

| 文件 | 符号/位置 | 现状 |
|---|---|---|
| `src/router/navigation.ts` | admin 组 4 项 :226-270（**`/settings/team/ai-providers` 在 admin 组内 :260-268——这就是其现状「隐藏」的源码实证**）；`isManagementPath` :305-314 | 动态派生 |
| `src/router/index.ts` | settings 域 :452-536（general 先例 :532-536）；redirect 先例 :359-382 | — |
| `src/views/settings/team/SettingsTeamMembersView.vue` | 视图本体 | 现路由 `/admin/members` |
| `src/views/settings/nav.ts` | `TEAM_APP_NAV` :49-51 | 待增 teamMembers/aiProviders |
| `src/views/settings/SettingsTeamLandingView.vue` | :10-11 自述 | 文案订正 |
| `src/components/shell/WorkspaceShell.test.ts` | :57-79 参数化范式 | 追加路径 |

### 修改边界

- **允许改**：`navigation.ts`（admin 组收窄+显式规则清单）、`index.ts`（成员路由迁挂+redirect+credentials 占位）、`settings/nav.ts`、LandingView 文案、i18n、两类测试。
- **禁止改**：`use-shell-chrome.ts`、四处 chrome 消费点、`/admin/settings` 与 `/admin/announcements`（NAV-0103）、后端任何代码（**路由迁移不等于权限迁移**）、成员视图组件内部。

### 实施步骤

（继承 V1.0 步骤 1~5：成员迁移+redirect、AI Provider 归组、Team 子树+Git 凭证 PLANNED 占位、显式规则清单 4a、LandingView 文案。）V1.1 强化：

0. **【先测后改】迁移前基线快照**：改代码**之前**，先以参数化测试（或临时脚本）记录下列每一路径的助手可见性实际值（以 Phase 0 源码与运行结果为准，禁止凭页面名推断），作为行为等价表「修改前」列的证据。
1. **行为等价表（验收交付物，随 commit 提交）**：

| 路径 | 修改前（实证） | 修改后 | 预期 |
|---|---|---|---|
| `/admin/settings` | 隐藏（admin 组） | 隐藏 | 不变 |
| `/admin/announcements` | 隐藏（admin 组） | 隐藏 | 不变 |
| `/admin/members` | 隐藏（admin 组） | redirect → 目标页隐藏 | 等价迁移 |
| `/settings/team/members` | 不适用（新路径） | 隐藏 | 新增等价路径 |
| `/settings/team/ai-providers` | **隐藏**（源码实证：admin 组项，navigation.ts:260-268 → isManagementPath 覆盖） | 隐藏（显式规则） | 保持原语义 |
| `/settings/team/general` | **可见**（不在 admin 组、不在前缀数组） | 可见 | 不误隐藏 |
| `/settings/team`（落地页） | 可见 | 可见 | 不误隐藏 |
| `/settings/team/credentials` | 不适用（新占位） | 可见（4a） | 占位页同 general 口径；4b 通过则随整域隐藏 |

2. 规则清单（exact 或 `path+"/"` 前缀；禁止裸 `/settings/team`）：`/admin/settings`、`/admin/announcements`、`/project/settings`（覆盖 C3 占位）、`/settings/team/members`、`/settings/team/ai-providers`。
3. 逐路径回归：等价表每行一条参数化断言，修改前/后各跑一次，两份输出随 commit 附上。

### 测试命令

公共四命令 + `pnpm -s vitest run src/components/shell/WorkspaceShell.test.ts` + `pnpm typecheck`。

### 验收条件

- [ ] P1-AC-11（四处一致）+ AGENTS 规则 5 四处自查；
- [ ] **行为等价表逐行通过且随 commit 附修改前/后两份实证输出**；
- [ ] `/admin/members` 旧链接 redirect 可用；
- [ ] Git 凭证占位项 planned 标识可见（复用 C3 消费机制）；
- [ ] 公共四命令 ≤ 基线。

### 禁止事项

- 禁止迁移 `/admin/settings`、`/admin/announcements`；
- 禁止裸 `/settings/team` 前缀（4b 未决）；
- 禁止改 `use-shell-chrome.ts`；禁止实现凭证 UI；
- 禁止把 Team Membership 当 Project Member；
- **禁止在未留修改前实证的情况下删除动态派生实现**（等价表无「修改前」列 = 不许合入）。

### 回滚条件

revert 后 admin 组恢复四项、成员页回原路径、助手可见性逐路径与现状一致。

---

## C5 — 助手隐藏全量回归 + `/inbox` 清理 + 注释订正（P1-05 回归 / P1-06 / P1-08）

### 源码定位

| 文件 | 符号/位置 | 现状 |
|---|---|---|
| `src/router/navigation.ts` | personal 组 `/inbox` 项 :95-100（badge:3） | 待删 |
| `src/components/shell/WorkspaceShell.vue` | `cmd-open-inbox` :184-189 | 待删 |
| `locales/zh-CN/shell.yaml` | `app.side.item.inbox: 收件箱` :207；**`shell.nav.inbox: 公告中心` :176（遗留设计期菜单段 `shell.nav.*`，同段 system/audit/workspaceConsole 等为 Phase 0 NAM-4 同族残留；本轮初查全库无 `shell.nav.*` 消费方）** | 分别处置 |
| `locales/en/shell.yaml` | 同键位 | 同步 |
| `locales/zh-CN/global.yaml` + `en/global.yaml` | `search.commands.openInbox` :31 | 待删 |
| `src/router/index.ts` | :226-230 注释漂移；:359-382 redirect 先例 | 订正+redirect |
| `src/stores/shell.ts` | :9 「188px」 | 改 232px |
| `src/components/shell/SideNav.test.ts` | :71 徽标 `["3"]`（inbox 徽标） | 随删除更新 |

### 修改边界

- **允许改**：navigation 配置（删 inbox 项）、WorkspaceShell（删命令）、router（redirect+注释）、stores（注释）、yaml、两类测试。
- **禁止改**：通知中心页面与顶栏通知入口行为、`searchFn` 落点（Phase 4/N-6）、ContextPanel、业务页面。

### 实施步骤

1. 删 `/inbox` 导航项 → redirect `/inbox → /notifications`。
2. 删 `cmd-open-inbox`；删 `app.side.item.inbox`（zh/en）+ `global.search.commands.openInbox`（zh/en）。
3. **【V1.1 新增】`shell.nav.inbox`（公告中心）处置——先查引用，禁盲删**：
   - 完整键路径 = `shell.nav.inbox`（位于遗留菜单段 `shell.nav.*`，「公告中心」≠「收件箱」，不同业务概念）；
   - 执行时 `grep -rn "shell\.nav\." src/` 全量复核（本轮初查无消费方）；
   - **零消费方** → 仅删该 key（zh/en），并在 commit message 注明「遗留残留、已验证无引用」；**有消费方** → 保留并迁移为语义正确的新 key，登记遗留问题；
   - `shell.nav.*` 段内其他遗留键**不在本卡范围**，只登记不清理（避免 scope 膨胀）。
4. SideNav.test :71 徽标断言更新。
5. 注释订正（router :226-230、stores :9）。
6. 助手隐藏全量回归（C4 等价表全路径 + 四处一致性 + 专注模式用例 :81-105 保持绿）。
7. Release Guard：构建产物无 mock/fixtures/demo。

### 测试命令

公共四命令 + `pnpm -s vitest run src/components/shell/` + `pnpm typecheck` + Release Guard。

新增用例最低集：
- **`/inbox` 三态**：①直接访问（地址栏输入）→ 落通知中心；②在 `/inbox` 上刷新 → 仍正确跳转；③旧链接（带 query 如 `/inbox?x=1`）→ 跳转不丢参异常；
- 命令面板无 inbox 命令；
- C4 等价表全路径复跑通过。

### 验收条件

- [ ] P1-AC-11/12/13/15 通过（Graph 页签不受影响断言）；
- [ ] `/inbox` 三态测试通过；
- [ ] `shell.nav.inbox` 处置有 grep 证据随 commit 附上；
- [ ] 公共四命令 ≤ 基线 + Release Guard 通过；
- [ ] V1.2 §14 安全八条抽查通过。

### 禁止事项

- 禁止盲删 `shell.nav.inbox`（必须先出 grep 证据）；禁止清理 `shell.nav.*` 段内其他键；
- 禁止顺带改搜索落点（N-6）、动 Graph 页签（R-5）；
- 禁止删通知下拉或改其跳转；
- 禁止把注释订正扩大为重构。

### 回滚条件

revert 后 `/inbox` 项、命令、i18n 键全部恢复，redirect 消失，无半删状态。

---

## 附 A：导航消费点总清单（P1-07，每卡收尾自查）

SideNav · `buildRoutes()` · `flatNavItems` · Command Palette（searchCommands + SearchPalette.test） · `use-shell-chrome.ts` · `WorkspaceShell` · TopBar · ContextPanel `availableObject`（:188-206，只验证不重构） · ReviewCenter 跳转（ReviewCenterPage:549，Phase 4 前不动） · Global Search 落点（:115，Phase 4 前不动） · 登录 redirect（:625）。

## 附 B：行号漂移免责

行号为 2026-11-16 master 快照；不符时以符号名与相邻注释定位，行为描述以 Phase 0 报告 §2 为准；现实与任务包冲突 → 停手登记阻塞，不得自创语义。

## 变更记录

- **V1.1（2026-11-16，开发执行版·冻结）**：按审查意见修订五项——①串行 C1→C2→C3→C4→C5（禁并行）；②C1 存储顺序≠视觉顺序 + admin 遗留独立标识（不成新分区）；③C3 devenv 父子路由五项验收 + planned 标记消费断言；④C4 先测后改 + 行为等价表（含 AI Provider 现状源码实证：admin 组项 :260-268）+「无修改前实证不许合入」；⑤C5 `shell.nav.inbox`（公告中心）先查引用禁盲删 + `/inbox` 三态测试 + 三原则前置。
- V1.0（2026-11-16）：初版。
