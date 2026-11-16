# Phase 1 前端导航改造执行任务书 V1.0

> 日期：2026-11-16
> 执行对象：开发 Agent（Codex / ZCode），仓库 `ai-frontend`（工作目录 `D:\kudi-project\templates\ai-pm\ai-frontend`）
> 上游依据：《前端导航改造源码核验报告 V1.0》（Phase 0，已验收）+ 说明书 V1.0 + 审阅意见（2026-11-16）
> 任务状态口径：**READY（允许开工）/ BLOCKED（等设计输入）/ DEFERRED（本轮冻结）**

---

## 1. 开工前必读（禁止跳过）

按说明书 §2.1 纪律，编码前必须读取原文（行号为 2026-11-16 快照，读取时以符号定位为准）：

| 文件 | 关注点 |
|---|---|
| `AGENTS.md`（仓库根） | 规则 4（release-guard 禁 mock 进 dist）、规则 5（助手隐藏单源与四处同步） |
| `ai-frontend/AGENTS.md` | §壳的约定（三上下文不混、无 Workspace Switcher、助手禁用四步）、§三硬性约束（PageFrame/i18n 门禁/每视图 ≥1 vitest/tokens 唯一真源） |
| `src/router/navigation.ts` | `navGroups` 84-271、`flatNavItems` 274、`isNavItemActive` 282-288、`isManagementPath` 305-314、`ASSISTANT_HIDDEN_PREFIXES` 321、`isAssistantHiddenPath` 326-331 |
| `src/router/index.ts` | `navItemRoutes` 59-86、`buildRoutes` 89-546、手工路由与重定向、`beforeEach` 618-643（登录 redirect 落 `/dashboard` @625） |
| `src/components/shell/use-shell-chrome.ts` | 全文 64 行：chrome 可见性单源（assistantVisible/contextVisible/focusAvailable） |
| `src/components/shell/WorkspaceShell.vue` | 壳根 data-* @219-247、URL 项目恢复 @44-91、全局搜索 searchFn @101-118（target 硬编码 `/project/knowledge` @115）、命令面板 @148-191（含 cmd-open-inbox） |
| `src/components/shell/SideNav.vue` | navGroups 消费、折叠、reviewCenter 徽标 @49-96、项目切换守卫 @116-119 |
| `src/components/shell/WorkspaceShell.test.ts` | 助手隐藏**参数化用例**所在（新增禁用路径在此追加） |
| `src/views/settings/nav.ts` | PERSONAL_APP_NAV / TEAM_APP_NAV / PROJECT_APP_NAV 三侧栏 |
| `src/styles/index.css` 24-149 | grid 五模板分支真值（232/60/446/288，显式列位） |
| `docs/audit/ia-2026-11-16/前端导航改造源码核验报告-v1.0.md` | §3 新发现 N-1~N-7、§6 兼容清单 C-1~C-18 |

## 2. 范围与非目标

**本任务书只授权 READY 类任务（Commit 01/02/04/05）。**以下一律不做：

- 不实现 ProjectNavigation 组件、不改 Shell 列布局、不新增 grid 模板分支（= Commit 03，BLOCKED 于 R-2 + Figma 第一批定稿）。
- 不迁移 `/project/settings/ssh-keys` 现有页面/路由/API（R-1 已裁决目标归属=并入仓库分区凭证绑定，属后续改造；Phase 1 原样保留、不 redirect，过渡态按现状语义运行，见《R-1-SSH裁决现状差异登记-v1.0.md》§4）。
- 不碰 ContextPanel 的 TABS 定义与 Graph 页签（DEFERRED，R-5 未决）。
- 不做四域页面拆分、不改全局搜索落点实现、不做 `?doc=/?object=/?report=` 迁移（DEFERRED，Phase 4）。
- 不做后端改动、不做 Schema Migration、不引入 Workspace Switcher。
- 改动后助手隐藏行为**除 §6.4 明示的两条行为等价迁移外必须零变化**。

## 3. Phase 0 Baseline（红项账本，正式）

以下为 2026-11-16 实测基线，**每批验收以「本表不扩大」为门槛**：

| 检查项 | 基线结果 | 验收要求 |
|---|---|---|
| `pnpm -s exec prettier --check src` | 21 文件失败 | 不新增失败文件 |
| `node ../scripts/check-i18n-hardcode.js` | exit 1（39 处硬编码中文） | 不新增硬编码 |
| `node ../scripts/check-i18n-namespace.js` | 通过（47 命名空间/5370 key） | 持续通过 |
| `pnpm -s vitest run src/styles/layout.test.ts` | 1 failed / 13 passed | 不新增失败 |
| SeeCode 3 渲染用例 | 既往 flaky | 单独跟踪，不混入导航回归；本轮不跑 e2e |

注意：**「未新增红项」≠ 导航改造质量通过**。新改动自身的单测、`pnpm typecheck`、助手隐藏回归必须全绿。

## 4. 全局执行纪律

1. 每个 Commit 独立可回滚：**任何时刻 revert 掉后续 commit，系统仍处正确状态**（路由可达、助手可见性正确）。禁止「前一个 commit 依赖后一个才恢复正确」。
2. 每 Commit 单独提交；并行会话同仓只 `git add` 自己改动的路径。
3. 修改 `.vue` 模板后必须 `git diff` 目检模板完整性（标签闭合）再提交。
4. 文案零硬编码中文：新增/改动文案走 `locales/zh-CN/*.yaml` + `en/*.yaml` 同步，跑两个 i18n 门禁。
5. 每个被改视图的相关单测必须更新并实际执行；`pnpm typecheck` 全绿。
6. 涉及助手可见性的改动，必须更新 `WorkspaceShell.test.ts` 参数化用例（AGENTS 规则 5 步骤 3）。
7. 遇到与本任务书冲突的源码事实：停下登记阻塞项，不得自行创造语义。

## 5. READY 任务

### Commit 01 — 导航配置模型与路由派生改造

**状态：READY。原则：纯重构，渲染与路由表输出零变化；不实现任何新 UI。**

实施要点：

1. `navigation.ts` 内部重构为三个具名配置源（是否拆文件自行判断，**不要求机械拆分**，保持「菜单与路由单源」注释有效）：
   - `globalNavConfig`（personal + workspace + admin 三组，**admin 组本轮留在全局配置**，Commit 04 才迁移）；
   - `projectNavConfig`（modules 组 8 项 + 预留四域/设置项的数据结构位置，**本轮不新增渲染**）；
   - `projectSettingsNav`（项目设置子树骨架，按 R-1 附一 V1.2 口径：基础设置 / 成员（规划项）/ 仓库（含凭证绑定预留）/ 开发环境（Devices/Executors/Runners）/ AI 配置与执行策略；**不含独立 ssh-keys 条目**——现有 ssh-keys 页面原样保留至后续迁移）。
2. 保留组合导出 `navGroups`（= 全部组）供 `SideNav.vue` 与既有测试零改动消费；或同步更新其导入——二选一，以「渲染输出不变」为准。
3. `buildRoutes()` 改为遍历三个配置源派生；`navItemRoutes` 派生规则不变（children → 父 redirect 首子 + 子平铺；缺省 PlaceholderView）。
4. `flatNavItems` 与命令面板数据源（`WorkspaceShell.vue:148-191` searchCommands 引用的导航文案）不得断裂。
5. `isManagementPath` 本 commit **不改实现**（仍从配置派生，admin 组未动）。

文件范围：`src/router/navigation.ts`、`src/router/index.ts`、（如拆文件）`src/router/navigation/*`、受导入影响的 `SideNav.vue`。

验收（全部实际执行并记录输出）：

- `pnpm -s vitest run src/router/ src/components/shell/` 全绿（navigation.test / router.test / WorkspaceShell.test / SideNav.test / SearchPalette.test / shell.dom.test）；
- `pnpm typecheck` 绿；
- prettier / 两个 i18n 门禁：红项不超 §3 基线；
- 自查断言：路由表路径集合与改造前一致（可在分支上临时打印 diff，提交前移除调试代码）。

回滚条件：`git revert` 单 commit 后路由表与 UI 完全恢复现状。

### Commit 02 — Global Navigation 调整（/inbox 删除 + /dashboard 改名）

**状态：READY。前置：Commit 01 已合入。**

实施要点：

1. 删除 `/inbox` 导航项（Q-10 已裁决）：
   - 移除 navigation 配置中的 inbox 项；
   - 新增过渡重定向 `/inbox → /notifications`（模式照 `router/index.ts:359-382` 旧路由重定向先例）；
   - 删除命令面板 `cmd-open-inbox`（`WorkspaceShell.vue:184-189`）；
   - 清理 i18n key：`shell.app.side.item.inbox`、`global.search.commands.openInbox`（zh-CN 与 en 同步删，防 namespace 门禁失衡）；`shell.yaml` 中 inbox 相关残留文案一并核对（现状 `inbox: 公告中心` 等）。
2. `/dashboard` 显示名改「团队总览」（Q-08 裁决 A：**只改名不改路径**）：
   - `shell.app.side.item.dashboard` zh-CN 文案改「团队总览」（en 同步改 Team Overview 语义）；登录 redirect（`router/index.ts:625`）不动。
   - 受影响断言测试（SideNav.test / TopBar 相关）同步更新。
3. 命令面板 `cmd-open-project` / `cmd-navigate-repositories` / `cmd-create-project` / `cmd-open-personal-workspace` 保留不动。

验收：

- 上述测试文件全绿 + `pnpm typecheck`；
- i18n namespace 门禁绿（zh/en key 集合一致）、hardcode 门禁红项不超基线；
- 手动自查：`/inbox` 访问落通知中心；侧栏无收件箱项；命令面板无 inbox 命令。

回滚条件：revert 后 `/inbox` 恢复占位项、`/dashboard` 恢复旧文案，无残留半删状态。

### Commit 04 — Team 归属迁移 + isManagementPath 显式化（分 4a/4b 两步）

**状态：4a READY；4b BLOCKED（待产品确认 §6.4 升级项）。前置：Commit 01 已合入。可与 Commit 02 并行，但建议 02 先合。**

#### 4a（行为等价迁移，本任务书授权范围）

1. **成员页迁移**：`/admin/members` → `/settings/team/members`（视图 `SettingsTeamMembersView.vue` 已存在，仅换挂载点）：
   - 路由：从导航配置 admin 组移除，改手工注册于 settings 域（照 `/settings/team/general` 先例 `router/index.ts:532-536`）；**旧路径 `/admin/members` 加 redirect → `/settings/team/members`**（兼容清单 C-15）。
   - 团队设置侧栏：`nav.ts` `TEAM_APP_NAV` 增 `{ id: "teamMembers", to: "/settings/team/members" }` + `TEAM_ICONS`（图标沿用 `UsersRound`）；i18n `settings.nav.teamMembers` zh/en 新增。
   - `SettingsTeamLandingView.vue` 自述文案订正（现状「成员、邀请已迁 /admin/members」→ 新路径，audit HIE-1 证据）。
2. **AI Provider 归组**：`/settings/team/ai-providers` 从 admin 组移除，改挂团队设置侧栏条目（`TEAM_APP_NAV` 增 `aiProviders`）；**路由路径不变**（零兼容成本）。
3. **isManagementPath 显式化（行为等价版）**：admin 组此时仅剩 `/admin/settings`、`/admin/announcements` 两项。把判定从「动态派生 admin 组」改为**显式规则清单**（页面清单驱动，非裸字符串堆砌），建议数据结构：

```ts
/** 助手禁用路径规则（页面清单驱动；每条含匹配语义注释） */
const ASSISTANT_HIDDEN_RULES: Array<{ path: string; note: string }> = [
  { path: "/admin/settings", note: "平台设置：租户运营面（现状禁用）" },
  { path: "/admin/announcements", note: "公告管理：现状禁用；迁团队后仍禁用" },
  { path: "/project/settings", note: "项目设置：配置面，完整前缀（含全部子路由）" },
  { path: "/settings/team/members", note: "团队成员：原 /admin/members 迁入，行为等价禁用" },
  { path: "/settings/team/ai-providers", note: "团队 AI Provider：原 admin 组迁入，行为等价禁用" }
];
```

   匹配语义（逐条明确，回应审阅意见二.2）：每条规则 = exact 命中或 `path + "/"` 前缀（覆盖子路由）；`/project/settings` 保持完整前缀；**不使用**裸 `/settings/team` 前缀（否则 `/settings/team/general` 行为改变——见 4b）。`/admin/announcements` 未来迁移时同步替换规则路径。实现位置仍在 `navigation.ts`，导出 `isAssistantHiddenPath` 语义不变。
4. **Team 设置子树与 Git 凭证入口（R-1 附二：Phase 1 范围）**：全局导航 Team 组建立「团队设置」可展开子树（`NavItem.children` 机制已备，Phase 0 Q2）：团队常规 / 成员与邀请 / AI Provider / **Git 凭证（SSH Keys，PLANNED）**；「团队公告」为 NAV-0103 待核验项，本轮**不进**子树。「Git 凭证」为 PLANNED 占位：路由 `/settings/team/credentials` → PlaceholderView，导航数据标 `planned`，i18n key（zh/en 同步新增）；**不得实现任何凭证管理 UI**——Team 凭证管理属后续功能设计（R-1 §三）。
4. **助手禁用页面清单（4a 生效版）**：

| 页面 | 路径 | 现状 | 4a 后 | 依据 |
|---|---|---|---|---|
| 平台设置 | `/admin/settings` | 禁用 | 禁用 | 现状保持 |
| 公告管理 | `/admin/announcements` | 禁用 | 禁用 | 现状保持 |
| 团队常规 | `/settings/team/general`（含 `/settings/team` 落地页） | **可见** | **可见（不变）** | 行为等价 |
| 团队成员 | `/settings/team/members` | （新路径；原路径禁用） | 禁用 | 行为等价迁移 |
| 团队 AI Provider | `/settings/team/ai-providers` | 禁用 | 禁用 | 行为等价 |
| 项目设置 | `/project/settings/*` | 禁用 | 禁用 | 现状保持 |
| 个人设置 | `/settings/personal/*` | 可见 | 可见 | 不在机制内 |

#### 4b（BLOCKED，待产品确认后另起 commit）

- 待确认项：团队设置**整域**（含 `/settings/team/general` 与 `/settings/team` 落地页）是否并入助手禁用（配置面口径，与 `/project/settings` 对齐）。
- 确认通过 → 规则清单追加 `{ path: "/settings/team", note: "团队设置整域：配置面" }` 并移除 4a 的两条单页规则；确认否 → 维持 4a。
- 在此之前**不得**用 `/settings/team` 前缀实现匹配。

验收（4a）：

- `WorkspaceShell.test.ts` 参数化用例追加：`/settings/team/members`（断言隐藏）、`/settings/team/ai-providers`（隐藏）、`/settings/team/general`（**断言可见**——4a 行为锁）、`/admin/members`（redirect 后目标页隐藏）；
- `pnpm vitest run src/components/shell/WorkspaceShell.test.ts` + `pnpm typecheck`（AGENTS 规则 5 步骤 4）+ `src/router/ src/components/shell/` 全量绿；
- Git 凭证占位项：`/settings/team/credentials` 渲染 PlaceholderView、导航项带 PLANNED 标识、助手可见性与团队常规页同态（4a 为可见）；
- 四处同步自查：顶栏助手按钮、AI Chat 列、右上下文栏、壳根 `data-ai-panel`/`data-context`（均由 useShellChrome 派生，确认无遗漏调用点）；
- i18n 两门禁红项不超基线。

回滚条件：revert 后 admin 组恢复四项、成员页回 `/admin/members`、助手可见性与现状完全一致（含 general 页可见）。

### Commit 05 — 回归收尾与账目清理

**状态：READY。前置：Commit 01/02/04 全部合入。**

实施要点：

1. **助手隐藏全量回归**：对 §6.4 清单全部路径执行可见性断言（参数化测试覆盖）；跑 `pnpm vitest run src/components/shell/` 全量。
2. **注释漂移订正（N-7）**：
   - `router/index.ts:226-230`（S04 任务子页 mock 预览描述与 breakdown 现状不符）按现实改写；
   - `stores/shell.ts:9` 「展开 188px」→「232px（折叠 60px）」（以 `styles/index.css` 真值为准）。
3. **Tokens 准备（N-4，仅准备不消费）**：若 R-2 已定稿，按定稿列宽在 `tokens.css` 预留 `--ws-projectnav-*` 变量（不接线）；未定稿则本项跳过并在交付物记录 DEFERRED。
4. 汇总 Phase 1 交付物（§8）。

验收：全量 `src/components/shell/` + `src/router/` vitest 绿、typecheck 绿、四门禁红项不超基线。

回滚条件：本 commit 仅注释/测试/预留变量，revert 无功能影响。

---

## 6. BLOCKED 任务

### Commit 03 — ProjectNavigation（BLOCKED）

**前置输入（缺一不开工）：**

1. **R-2 定稿（Figma 第一批）**：全局↔项目导航呈现形态（整栏切换 / 全局窄轨常驻+项目导航第二列 / 内嵌折叠组）、进入与退出项目工作区的交互、1440px 布局与新列宽。**（R-1 已于 2026-11-16 裁决解除：`projectSettingsNav` 子树按 R-1 附一 V1.2 口径定稿——仓库含凭证绑定预留、不含独立 ssh-keys 条目，见《R-1-SSH凭证架构裁决记录-v1.0.md》。）**
2. R-5（可延后，但 ContextPanel 相关改动冻结至其决出）。

**Commit 03 边界提示**：R-1 的 Team 凭证管理 UI、Repository 凭证绑定 UI、Schema 演进均属后续（差异 Δ-1~Δ-7 已冻结于《R-1-SSH裁决现状差异登记-v1.0.md》），Phase 1 只做导航占位与配置结构。

**定稿后实施内容（预告，不在本任务书授权内）：** ProjectNavigation 组件挂载、grid 新模板分支 + 新 data-属性（保持 v-if⇒data-* 同步纪律）、`/project/devenv` 迁移与旧路径 redirect（C-7）、项目切换复用 `requestProjectSwitch` 守卫链、`WorkspaceShell.test.ts` 用例扩展。

## 7. DEFERRED 任务与 Phase 4 依赖登记

以下冻结，但**依赖必须现在登记**，不得等 Phase 4 开工时重新发现：

| 任务 | 登记的强制依赖 |
|---|---|
| 四域页面拆分（Phase 4） | **N-1**：`ReviewCenterPage.vue:549` 的 `?report=` 跳转必须与 Analysis 新路由同步改造（NAV-0405 兼容 + NAV-0502 归位） |
| 全局搜索分流（Phase 4） | **N-6**：`WorkspaceShell.vue:115` target 硬编码 + `searchFn` 的 `source_domain` 分流材料（KB/PC 可分，report 待 Analysis 路由） |
| 深链迁移 `?doc=/?object=/?report=` | 兼容清单 C-2/C-3/C-4；旧 `/project/knowledge` 保留为 KB 入口（C-1） |
| R-5 Graph 页签去留 | 未决前 ContextPanel TABS（`ContextPanel.vue:126-132`）与 `/project/visualizations` 路由（`router/index.ts:426-431`）一律不动 |
| SSH Credential 模型演进（后端，R-1 目标态） | 现状 project 池模型 → Team-owned + Grant + Repository Binding 的 **Δ-1~Δ-7 差异冻结于《R-1-SSH裁决现状差异登记-v1.0.md》**；演进按 AGENTS 规则 1 直接改契约、不做双写兼容；**Δ-2（is_default 兜底 vs D2.5）/Δ-3（向导自动建钥 vs D3.1）与裁决语义直接冲突，为演进第一优先项**；Phase 1 现有 ssh-keys 页面按过渡态原样保留 |

## 8. 交付物（每 Commit 一份，随 PR/提交附上）

1. 实施代码 + 变更文件清单；
2. 路由变更清单（新增/删除/重定向，对照兼容清单 C-1~C-18）；
3. 导航变更清单（项增删/改名/归组）；
4. 测试执行结果（命令 + 输出摘要 + 与 §3 基线对照）；
5. 回滚说明（revert 本 commit 后系统状态声明）；
6. 未完成项与阻塞原因。

任务状态标记：NOT_STARTED / IN_PROGRESS / BLOCKED / COMPLETED；**不能仅凭代码提交宣称完成**。

---

## 附：关键源码事实速查（自足性摘录，细节见 Phase 0 报告 §2/§3）

- 单源：`navGroups`（navigation.ts:84-271）→ `buildRoutes()`（router/index.ts:89-546）；`flatNavItems`（:274）供命令面板。
- 助手隐藏：`isManagementPath`（:305-314，动态派生 admin 组）+ `ASSISTANT_HIDDEN_PREFIXES=["/project/settings"]`（:321）→ `isAssistantHiddenPath`（:326-331）→ `use-shell-chrome.ts` 单源消费 → 顶栏按钮/AI Chat 列/ContextPanel v-if + 壳根 data-*。
- Shell grid：`styles/index.css:36-95` 五模板分支；四栏显式列位（topbar 1/-1、main 2、chat 3、context 4；会话页特例 context 占 3）。
- 项目上下文：`stores/shell.ts currentProjectId`（两阶段 switching、会话守卫 `requestProjectSwitch`）；恢复优先级 URL > localStorage(`aipm.current-project`) > 首项（WorkspaceShell.vue:57-91）。
- 登录 redirect：`router/index.ts:625` → `/dashboard`（改名不改路径，不动）。
- 重定向先例：`router/index.ts:359-382`（repository/environments 五条）。

*任务书结束 · V1.0 · 2026-11-16*
