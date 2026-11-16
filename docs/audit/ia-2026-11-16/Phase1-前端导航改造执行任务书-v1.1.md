# Phase 1 前端导航改造执行任务书 V1.1

> 日期：2026-11-16
> 执行对象：开发 Agent（Codex / ZCode），仓库 `ai-frontend`（工作目录 `D:\kudi-project\templates\ai-pm\ai-frontend`）
> 上游依据（冲突时以此序为准）：**《全局导航 IA 与 Phase 1 改造要求修订稿 V1.2》（R-2 五项裁决全 A，最高优先）**→ R-1 裁决 + 现状差异登记 → 说明书 V1.0 → Phase 0 核验报告 → 本任务书 V1.0（已被本版取代）
> **V1.0 → V1.1 变更摘要**：①R-2 裁决落地——单侧栏上下分层（R2-D1）、无第二侧栏（D2）、无工作区进入/退出模式（D3）、无项目隐藏 Current Project 区（D4）、总控台顶/模块中/设置底（D5）；②Commit 切分改为 V1.2 §12 的 **C1~C5**（原 01/02/04a/05 → C1/C2/C4/C5，原 BLOCKED 的 Commit 03 → **C3 转 READY**）；③**零新增 Grid 模板分支、零新列**（原「tokens 新列预留」作废）；④`/inbox` 清理从 C2 移至 C5；⑤发现 V1.2 §5/P1-04 项目设置子树缺「AI 配置与执行策略」（起草遗漏推定，见 §6 注）。

---

## 1. 开工前必读（禁止跳过）

| 文件 | 关注点 |
|---|---|
| `AGENTS.md`（仓库根） | 规则 4（release-guard 禁 mock 进 dist）、规则 5（助手隐藏单源与四处同步） |
| `ai-frontend/AGENTS.md` | §壳的约定（三上下文不混、无 Workspace Switcher、助手禁用四步、TeamSwitcher/ProjectSwitcher 双变体）、§三硬性约束（PageFrame/i18n 门禁/每视图 ≥1 vitest/tokens 唯一真源） |
| `docs/audit/ia-2026-11-16/PM-AI-全局导航IA与Phase1改造要求修订稿-V1.2.md` | R2-D1~D5 定义、§4 架构约束 R2-C01~C10、§5 V1.2 导航树、§10 P1-01~08、§13~16 验收与 DoD |
| `docs/audit/ia-2026-11-16/R-1-SSH凭证架构裁决记录-v1.0.md` + 《R-1-SSH裁决现状差异登记-v1.0.md》 | 凭证导航归属、Δ-1~Δ-7 演进冻结项 |
| `src/router/navigation.ts` | `navGroups` 84-271、`flatNavItems` 274、`isNavItemActive` 282-288、`isManagementPath` 305-314、`ASSISTANT_HIDDEN_PREFIXES` 321 |
| `src/router/index.ts` | `navItemRoutes` 59-86、`buildRoutes` 89-546、重定向先例 359-382、`beforeEach` 618-643（登录 redirect `/dashboard` @625） |
| `src/components/shell/use-shell-chrome.ts` | chrome 可见性单源（ASSISTANT_PATH 三列特例 @7） |
| `src/components/shell/WorkspaceShell.vue` | 壳根 data-* @219-247、URL 项目恢复 @44-91（优先级 URL>localStorage>首项）、searchFn target 硬编码 @115、searchCommands @148-191（cmd-open-inbox） |
| `src/components/shell/SideNav.vue` | navGroups 消费、collapsedGroups、reviewCenter 徽标 @49-96、**被注释停用的旧项目上下文头（紧凑头不得复刻其大尺寸形态）** |
| `src/components/shell/ProjectSwitcher.vue` + `src/stores/shell.ts` | `requestProjectSwitch` 守卫链、两阶段 switching、`switchingProject` 提示 |
| `src/components/shell/WorkspaceShell.test.ts` | 助手隐藏**参数化用例**所在（新增禁用路径在此追加） |
| `src/views/settings/nav.ts` | 三设置侧栏（PROJECT_APP_NAV 含 projectAi 与 sshKeys 条目） |
| `src/styles/index.css` 24-149 | grid 五模板分支真值（232/60/446/288，显式列位）——R-2 后**不新增分支** |

## 2. 范围与非目标

Phase 1 交付（V1.2 §9）：三层导航配置模型 + 多源路由派生 + 单侧栏分层呈现 + 紧凑 Project Switcher + 切换复用 + 助手隐藏迁移回归 + `/inbox` 清理 + 消费点同步 + 注释订正。

**明确不做（V1.2 §11 全文适用）**，另加本仓细则：

- 不迁移 `/project/settings/ssh-keys` 现有页面/路由/API（R-1 目标态=并入仓库分区凭证绑定，属后续改造；过渡态原样保留、不 redirect、按现状语义运行）。
- 不实现 Team 凭证管理 UI / Repository 凭证绑定 UI / Δ-1~Δ-7 任何项（差异登记冻结）。
- 不做四域**页面**拆分与深链迁移（DEFERRED Phase 4；导航项落位映射见 §5 C3）。
- 不迁移 `/admin/settings`、`/admin/announcements`（NAV-0103 待核验；admin 组本轮保留这两项）。
- 不动 ContextPanel TABS 与 Graph 页签、不动 `/project/visualizations`（R-5 待裁决）。
- 助手隐藏行为除 §5 C4 明示的行为等价迁移外**零变化**。

## 3. Phase 0 Baseline（红项账本，正式）

| 检查项 | 基线结果 | 验收要求 |
|---|---|---|
| `pnpm -s exec prettier --check src` | 21 文件失败 | 不新增失败文件 |
| `node ../scripts/check-i18n-hardcode.js` | exit 1（39 处） | 不新增硬编码 |
| `node ../scripts/check-i18n-namespace.js` | 通过（47/5370） | 持续通过 |
| `pnpm -s vitest run src/styles/layout.test.ts` | 1 failed / 13 passed | 不新增失败 |
| SeeCode 3 用例 | 既往 flaky | 单独跟踪，不混入导航回归 |

双保险：账本不扩大 **且** 新改动自身单测/`pnpm typecheck`/助手回归全绿；存量修复不混入导航 commit。

## 4. 全局执行纪律

1. 每 Commit 独立可回滚：revert 任一后续 commit，系统仍处正确状态（路由可达、助手可见性正确）。
2. 每 Commit 单独提交；并行会话只 `git add` 自己路径；`.vue` 模板改后 `git diff` 目检闭合。
3. 文案零硬编码中文：zh-CN 与 en 的 yaml 同步增删；两个 i18n 门禁必跑。
4. 涉助手可见性的改动必更 `WorkspaceShell.test.ts` 参数化用例。
5. Release Guard（AGENTS 规则 4）：发布构建不含 mock/fixtures/demo（command-center mock-preview 分支与 WorkspaceShell `demo.ts` 项目清单分支同属核验对象）。
6. 遇源码事实与本任务书冲突：停手登记阻塞项，不自行造语义。

## 5. READY 任务（C1~C5，对应 V1.2 §12）

### Commit C1 — 导航配置模型拆分 + 多源路由派生（P1-01）

**纯重构，渲染与路由表输出零变化。**

1. `navigation.ts` 拆三个具名配置源（Global / Project / Settings Config；**同侧栏消费，不是三套 Sidebar**——R2-D1）：
   - `globalNavConfig`：personal + workspace + admin 组（admin 组含 `/admin/settings`、`/admin/announcements` 两项**保留不动**至 NAV-0103；成员与 ai-providers 两项本 commit 暂留，C4 迁出）；
   - `projectNavConfig`：项目域全部项（结构见 C3 终稿顺序，本 commit 仅数据搬家、渲染不变）；
   - `settingsNavConfig`：项目设置子树（V1.2 §5 + AI 配置项，见 §6 注）。
2. `buildRoutes()` 改多源遍历；派生规则不变（children → 父 redirect 首子 + 子平铺；缺省 PlaceholderView）。
3. `flatNavItems` 与命令面板（`searchCommands` 硬编码 5 条 + `flatNavItems`）同步，**导航数据零断裂**（P1-07 消费点清单）。
4. `isManagementPath` 本 commit 不改实现。

验收：`pnpm -s vitest run src/router/ src/components/shell/` 全绿；typecheck 绿；四门禁红项不超基线；自查路由表路径集合与改造前一致。
回滚：revert 后渲染与路由完全恢复现状。

### Commit C2 — Global Navigation 呈现（P1-02 前两区）

前置 C1。

1. SideNav 呈现 Personal / Team 两区（区域标题 + 分隔线，沿用 collapsedGroups；R2-C08 不扁平混排）。
2. `/dashboard` 显示名改「团队总览」（Q-08 A：**只改名不改路径**；zh/en 文案 + 受影响断言；登录 redirect @625 不动）。
3. 布局约束（R2-D2/C10）：零新增 grid 分支/新列；232/60 不变；AI Chat/Context Panel 列不动。
4. `/inbox` 本 commit **不动**（C5 统一清理）。

验收：SideNav.test/WorkspaceShell.test/TopBar 相关全绿；typecheck；i18n 门禁红项不超基线。
回滚：revert 后恢复旧组结构与文案。

### Commit C3 — Project Navigation 单侧栏分层 + 紧凑 Project Switcher（P1-02/03/04）

前置 C1、C2。**R-2 裁决后本 commit READY。**

1. **Current Project 区域（条件渲染，R2-D4/C06）**：`shell.currentProjectId` 非空时渲染；空时整区隐藏（不渲染灰色禁用菜单、不伪造默认项目）；switching 态区域保持并显示切换态（复用 `switchingProject` 提示），不显示旧项目选中项。
2. **紧凑 Project Switcher（R2-D3/P1-03）**：区域顶部；显示当前项目名（长名截断）+ 下拉切换入口（支持搜索/选择，复用现有 ProjectSwitcher 逻辑）；**必须走 `requestProjectSwitch`**（保留活跃会话守卫 + 两阶段切换 + 切换中 `currentProject=null` 不泄漏旧数据）；折叠态 60px 下为紧凑图标入口 + Tooltip；**禁止**直接改 `currentProjectId`、禁止在侧栏重造守卫、禁止以路由匹配替代上下文状态。
3. **区域结构（R2-D5/C07）**：项目总控台（置顶）→ 业务模块组（居中）→ 项目设置组（置底，可展开子菜单；分组折叠沿用 collapsedGroups）。
4. **业务模块 10 项落位映射**（P1-04 严格按 V1.2 树；页面未拆分的域按此过渡）：
   - 现有 6 项直连：需求、技术设计、任务管理、AI 执行中心、代码发现、人工审核中心（reviewCenter 徽标逻辑随迁）；
   - **KB → `/project/knowledge`**（兼容清单 C-1：该页最终定位即 KB 入口，迁移期页面内容仍三合一，不伪装已拆分）；
   - **项目认知 → `/project/cognition`**（真页直连，顺带消除 ORP-1 零入口）；
   - **Analysis → PLANNED 占位路由 `/project/analysis`**、**产品认知 → PLANNED 占位路由 `/project/product-cognition`**（渲染 PlaceholderView、导航标 `planned`；Phase 4 页面落地后替换路由）。
5. **项目设置子树（置底）**：基础设置（`general`，proposed 现状）→ 成员（PLANNED 占位现状 `members`）→ **仓库**（`repositories`；**Repository 凭证绑定入口仅预留导航位**，不加子路由，UI 属后续——R-1 附二）→ **开发环境**（子树：Devices→现有 `/project/devenv`（Phase 3 才迁移路径，暂指旧路由）、Executors→PLANNED 占位 `/project/settings/devenv/executors`、Runners→`/project/settings/runners` + `:deviceId` 详情深链保留）→ **AI 配置与执行策略**（`/project/settings/ai`→roles；见 §6 注）。
6. **布局约束（R2-C10）**：零新增 grid 分支/新列/新 data-属性；**禁止**恢复旧大尺寸项目头（裁决依据明确要求紧凑形式）。

验收（映射 P1-AC-01/03/04/05/06/07/13/14）：三层区域呈现且不混排；无项目时整区隐藏（SideNav.test 断言）；切换走守卫（测试：活跃会话切换弹确认、取消后上下文不变、切换中无旧数据）；顺序正确（总控台首项、设置末组）；占位路由渲染 PlaceholderView 且标 planned；命令面板与侧栏一致；深链 `/project/cognition`、`/project/knowledge` 不回归；typecheck + 相关 vitest 全绿；四门禁不超基线。
回滚：revert 后 SideNav 恢复旧 modules 组渲染，占位路由无残留（路由随配置派生，自动消失）。

### Commit C4 — Team 设置归属与路径迁移 + isManagementPath 更新（P1-05 前置部分）

前置 C1（可与 C2/C3 并行，建议 C2 先合）。

1. **成员页迁移**：`/admin/members` → `/settings/team/members`（视图已有，换挂载点：settings 域手工注册，照 `/settings/team/general` 先例）；旧路径 redirect（先例 ：359-382）；团队设置侧栏 `TEAM_APP_NAV` 增 `teamMembers` + 图标；i18n `settings.nav.teamMembers` zh/en；`SettingsTeamLandingView` 自述文案订正（HIE-1）。
2. **AI Provider 归组**：`/settings/team/ai-providers` 出 admin 组，挂团队设置侧栏；**路由路径不变**。
3. **Team 设置子树补全（V1.2 §5）**：全局导航 Team 区「团队设置」可展开子树 = 团队常规 / 成员与邀请 / AI Provider / **Git 凭证（SSH Keys，PLANNED 占位：路由 `/settings/team/credentials` → PlaceholderView，标 planned，i18n zh/en；不实现任何凭证 UI）**；「团队公告」NAV-0103 待核验，**本轮不进子树**。
4. **isManagementPath 显式化（行为等价版 4a）**：admin 组届时仅剩 `/admin/settings`、`/admin/announcements`。改为**显式规则清单**（页面清单驱动，每条含匹配语义注释）：

```ts
const ASSISTANT_HIDDEN_RULES: Array<{ path: string; note: string }> = [
  { path: "/admin/settings", note: "平台设置：租户运营面（现状禁用）" },
  { path: "/admin/announcements", note: "公告管理：现状禁用；迁团队后仍禁用" },
  { path: "/project/settings", note: "项目设置：配置面，完整前缀（含全部子路由与新增 devenv/executors 占位）" },
  { path: "/settings/team/members", note: "团队成员：原 /admin/members 迁入，行为等价禁用" },
  { path: "/settings/team/ai-providers", note: "团队 AI Provider：原 admin 组迁入，行为等价禁用" }
];
```

   匹配语义 = exact 或 `path + "/"` 前缀；**不使用**裸 `/settings/team` 前缀（否则 `general` 行为改变，见 4b）。`isAssistantHiddenPath` 语义与消费方（use-shell-chrome）不变。

**助手禁用清单（4a 生效态）**：平台设置/公告管理禁用（现状）；`/settings/team/general` 与落地页**可见（不变）**；members、ai-providers 禁用（等价迁移）；`/project/settings/*` 全前缀禁用（含 C3 新增的 devenv/executors 占位，自动被前缀覆盖）；个人设置不在机制内。

**4b（BLOCKED，待产品确认后另起 commit）**：团队设置**整域**（含 general 与落地页）是否并入禁用域（配置面口径）。确认通过 → 追加 `{ path: "/settings/team" }` 规则并移除两条单页规则；在此之前禁止裸前缀实现。

验收（P1-AC-11 + AGENTS 规则 5 四步）：`WorkspaceShell.test.ts` 参数化用例追加 `/settings/team/members`（隐藏）、`/settings/team/ai-providers`（隐藏）、`/settings/team/general`（**可见，行为锁**）、`/settings/team/credentials`（可见占位）、`/admin/members`（redirect 后目标隐藏）；四处同步自查（顶栏按钮/AI Chat 列/ContextPanel/壳根 data-*，均由 useShellChrome 派生）；vitest + typecheck 全绿；i18n 门禁不超基线。
回滚：revert 后 admin 组恢复四项、成员页回原路径、助手可见性与现状一致。

### Commit C5 — AI 助手隐藏回归 + `/inbox` 清理 + 注释订正（P1-05 回归 / P1-06 / P1-08）

前置：C1~C4 对应路径变更完成。

1. `/inbox` 清理（Q-10 + P1-06）：删导航项；`/inbox → /notifications` 过渡 redirect（先例 ：359-382）；删 `cmd-open-inbox` 命令（WorkspaceShell.vue:184-189）；删 i18n key（`shell.app.side.item.inbox`、`global.search.commands.openInbox` + shell.yaml inbox 残留文案，zh/en 同步）。
2. 助手隐藏全量回归：§5 C4 清单全部路径参数化断言 + 四处一致性；`pnpm vitest run src/components/shell/` 全量。
3. 注释订正（P1-08）：`router/index.ts:226-230`（S04 mock 预览描述过期）、`stores/shell.ts:9`（188px→232px，以 tokens/index.css 为准）。
4. Release Guard 核验：发布构建无 mock/fixtures/demo。

验收：全量 shell+router vitest 绿、typecheck 绿、四门禁不超基线、release-guard 通过；`/inbox` 访问落通知中心。
回滚：revert 后恢复 inbox 项与命令，无半删状态。

## 6. 一处起草遗漏待确认（不阻塞，按含入处理）

**V1.2 §5 导航树与 P1-04 的「项目设置」子树未列「AI 配置与执行策略」**，但：①R-1 附一的 Project Settings 清单含它；②现状 `/project/settings/ai/*` 五子页已实现（roles/overview/model-policies/monitor/execution-policies，settings 侧栏 nav.ts:71）。**推定为起草遗漏**，本任务书按「含入项目设置子树」执行；若产品确认应移除，另行裁决其归属（不得静默删除已实现配置面入口）。

## 7. 开放项（不阻塞 commit，禁止顺手处理）

| 项 | 状态 | 边界 |
|---|---|---|
| NAV-0103：`/admin/settings`、`/admin/announcements` 归属 | 待核验 | 本轮 admin 组保留两项不迁；核验后另起 commit |
| 4b：团队设置整域助手禁用 | 待产品确认 | 确认前禁止裸 `/settings/team` 前缀 |
| R-5：Graph 页签去留 | 待裁决 | ContextPanel TABS 与 `/project/visualizations` 一律不动（P1-AC-15） |
| 四域页面拆分 + 深链迁移 | Phase 4 | 占位路由 `/project/analysis`、`/project/product-cognition` 届时替换；**强制依赖 N-1（ReviewCenterPage:549 `?report=` 生产方须同步改）+ N-6（搜索落点分流，evidence 已带 source_domain）** |
| devenv 页迁入 settings 路径（NAV-0301 redirect） | Phase 3 | C3 仅导航登记，Devices 暂指旧路径 `/project/devenv` |
| SSH Credential 模型演进 | 后续技术设计 | Δ-1~Δ-7 冻结；Phase 1 零实现 |

## 8. 验收与完成定义

- **功能验收**：V1.2 §13 P1-AC-01~15 全表。
- **安全与状态验收**：V1.2 §14 八条（切换无残留、守卫触发、取消恢复、失败不留错态、无项目无残留菜单、导航≠权限、凭证人工操作、菜单不推权限）。
- **质量门禁**：V1.2 §15（基线表 = 本任务书 §3；Release Guard 必跑）。
- **完成定义**：V1.2 §16 十二条全满足；每 Commit 交付 §9 清单（代码+文件清单+路由/导航变更清单+测试结果+回滚说明+未完成项）。

任务状态标记：NOT_STARTED / IN_PROGRESS / BLOCKED / COMPLETED；不能仅凭提交或截图宣称完成。

---

## 附：关键源码事实速查（细节见 Phase 0 报告 §2/§3）

- 单源：`navGroups`（navigation.ts:84-271）→ `buildRoutes()`（router/index.ts:89-546）；`flatNavItems`（:274）供命令面板；`searchCommands` 硬编码 5 条（WorkspaceShell.vue:148-191）。
- 助手隐藏：`isManagementPath`（:305-314，动态派生 admin 组）+ `ASSISTANT_HIDDEN_PREFIXES=["/project/settings"]`（:321）→ `isAssistantHiddenPath`（:326-331）→ `use-shell-chrome.ts` 单源 → 顶栏按钮/AI Chat 列/ContextPanel v-if + 壳根 data-*。
- Shell grid：`styles/index.css:36-95` 五模板分支；四栏显式列位；**R-2 后零新增分支**。
- 项目上下文：`stores/shell.ts currentProjectId`（两阶段 switching、`requestProjectSwitch` 会话守卫、switching 中 `currentProject=null`）；恢复优先级 URL > localStorage > 首项（WorkspaceShell.vue:57-91）；侧栏 watch 重拉 reviewCenter 徽标（SideNav.vue:49-55）。
- 重定向先例：`router/index.ts:359-382`；登录 redirect `/dashboard`（:625）。

## 变更记录

- **V1.1（2026-11-16）**：并入 R-2 五项裁决（全 A）与 V1.2 修订稿；Commit 切分改为 C1~C5；原 BLOCKED Commit 03 转 READY（C3）；零新增 Grid 分支；`/inbox` 移入 C5；新增 §6 AI 配置起草遗漏注记；§7 开放项表。
- V1.0（2026-11-16）：初版（Commit 01/02/03/04a/05；R-1、R-2 当时未裁决）。
