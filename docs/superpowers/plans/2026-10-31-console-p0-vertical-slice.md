# AI 总控台 P0 竖切 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 前端迁入 ai-pm 同仓后，交付设计系统 token 体系 + Workspace Shell 三栏框架 + 总控台 V1.1 单页竖切（fixtures 零后端）。

**Architecture:** 纯新增 `frontend/src/workspace/` 模块（shell/console/fixtures/store），静态路由挂进现有 `shell/router/routes.ts`，不碰存量七页与 BFF。新 token 层 `styles/workspace/tokens.css`（深色默认）与 PRISM token 层并存，作用域化 `--prism-*` 桥接让 PRISM 组件在工作区深色下正确显色。

**Tech Stack:** Vue 3 + TS + pinia + vue-router 5 + vitest/@vue/test-utils/happy-dom（现仓既有，**新增依赖零**）。

**Spec:** [../specs/2026-10-31-console-p0-vertical-slice.md](../specs/2026-10-31-console-p0-vertical-slice.md)（裁决表 §三/§四、页面规格 §五、验收 §七）

## Global Constraints

- 一律用**绝对路径**操作（跨仓事故史）；每任务结束即 commit（并行会话对策）；commit 只落本地 master，**不 push**。
- 新文件 **LF 行尾**；`pnpm lint`（eslint+prettier）通过；界面文案全部走 vue-i18n，`locales/zh-CN/workspace.yaml` 与 `locales/en/workspace.yaml` 同步补 key。
- 新增 npm 依赖**零**；不改 `src/styles/prism/`（其 tokens.test.ts 契约必须保持全绿）；不改 ai-pm-web 仓任何文件。
- 工作区组件颜色只允许引用 `--ws-*` 变量（复用 check-prism-tokens 门禁，扫描范围扩展见 Task 1）。
- 可执行动作不得本地点击即判成功：指令栏状态机 idle→submitting→(receipt)→done，无真回执不进 done。
- 测试命令：`pnpm test:unit`（在 `D:\kudi-project\templates\ai-pm\frontend` 下运行，vitest 2.x，测试文件与源码同目录 `*.test.ts`）。

## Review Focus

1. **未知 projectId 直达** `/workspace/whatever/console` → 降级页（i18n 文案+返回入口），不得白屏不得渲染演示数据 → Task 5 测试。
2. **刷新主题防闪烁/持久化**：默认深色；`index.html` 预挂载脚本在首帧前设 `data-theme`；useTheme 初始化读 localStorage → Task 1 测试（预挂载脚本由 Task 8 集成测试读 DOM 断言存在）。
3. **PRISM 组件在工作区深色下发灰**：`--prism-*` 桥接块必须定义且作用于 `.workspace-shell` 作用域 → Task 1 测试（tokens.test.ts 断言桥接变量存在）。
4. **存量七页回归**：workspace 路由纯新增，guard/动态菜单/mock 插件不受影响 → Task 8 全量 test:unit + build + `git ls-files | grep -c $'\r'` 为 0。
5. **指令栏伪成功**：submit 后必须经历 submitting（可取消），mock 回执前不得出现完成态 → Task 6 测试断言状态机。

## File Structure

```
ai-pm/frontend/                        # Task 0 自 ai-pm-web/frontend 整体迁入（pnpm workspace 根）
ai-pm/scripts/check-prism-tokens.js    # Task 0 自 ai-pm-web/scripts 迁入（路径解析依赖 ROOT=仓根/frontend 布局，原样可用）
ai-pm/docs/prism/phase1-visual-direction.md   # Task 0 迁入（tokens.test.ts 候选路径 cwd/../docs/prism/）

frontend/src/workspace/
  fixtures.ts            # 演示数据与类型（V1.1 图面文案照录）
  fixtures.test.ts
  store.ts               # pinia：选中态/栏开关/指令栏消息
  store.test.ts
  useTheme.ts            # 主题 composable（dark 默认，localStorage 持久化）
  useTheme.test.ts
  shell/WorkspaceShell.vue   # grid 三栏壳（TopBar/LeftSidebar/主区/ContextSidebar）
  shell/TopBar.vue           # 面包屑 + 主题切换 + 图标占位
  shell/LeftSidebar.vue      # V1.1 双分组 IA + 232↔60 折叠
  shell/ContextSidebar.vue   # 右栏 320px：当前待办/AI 建议/占位区
  shell/WorkspaceShell.test.ts
  components/AICommandBar.vue    # 指令栏（chip/发送/取消/mock 回复气泡）
  components/AICommandBar.test.ts
  console/ConsolePage.vue    # 总控台编排 + 未知项目降级
  console/BriefingCard.vue   # 简报
  console/MetricCards.vue    # 指标卡 ×3
  console/AttentionList.vue  # 需要你处理 + 选中态
  console/GoalStrip.vue      # 目标进展（简版）
  console/DynamicsFeed.vue   # 项目动态（简版）
  console/ConsolePage.test.ts

frontend/src/styles/workspace/
  tokens.css             # --ws-* 全集 + .workspace-shell 内 --prism-* 桥接
  index.css              # @import tokens + shell 布局基类
  tokens.test.ts         # token 存在性 + WCAG AA 对比度断言

Modify: frontend/index.html（防闪烁脚本）、frontend/src/shell/entry.ts（import workspace 样式）、
        frontend/src/shell/router/routes.ts（挂 /workspace/:projectId 子树）、
        frontend/src/shell/views/dashboard/DashboardView.vue（预览入口）、
        locales/{zh-CN,en}/workspace.yaml（新增）、ai-pm/.gitignore（node_modules/dist）
```

---

### Task 0: 前端迁入 ai-pm 同仓

**Files:**
- Create: `ai-pm/frontend/**`（自 ai-pm-web/frontend 复制，排除 node_modules、dist、.eslintcache、*.log）
- Create: `ai-pm/scripts/check-prism-tokens.js`、`ai-pm/docs/prism/phase1-visual-direction.md`
- Modify: `ai-pm/.gitignore`

**Interfaces:**
- Produces: `ai-pm/frontend` 可独立 `pnpm install && pnpm test:unit && pnpm build`；`pnpm check:prism-tokens` 可用（脚本在 `../scripts/` 相对 frontend 解析，与原布局一致）。

- [ ] **Step 1: 复制**（robocopy 排除目录；源仓只读，不删不移不改）：`frontend/` → `ai-pm/frontend/`，`scripts/check-prism-tokens.js` → `ai-pm/scripts/`，`docs/prism/phase1-visual-direction.md` → `ai-pm/docs/prism/`。
- [ ] **Step 2: .gitignore** 追加 `frontend/node_modules`、`frontend/dist`、`frontend/.eslintcache`、`*.local`；`git status` 核对无 node_modules 泄入。
- [ ] **Step 3: 验证**：`cd /d/kudi-project/templates/ai-pm/frontend && pnpm install && pnpm test:unit`（Expected: 全绿，含 prism tokens 契约测试——它从 `cwd/../docs/prism/` 找到规范文档）+ `pnpm build` 通过。
- [ ] **Step 4: Commit** `chore(frontend): adopt frontend workspace from ai-pm-web (per 2026-10-31 adjudication)`

### Task 1: workspace token 体系 + 主题 + 门禁扩展

**Files:**
- Create: `frontend/src/styles/workspace/tokens.css`、`frontend/src/styles/workspace/index.css`、`frontend/src/styles/workspace/tokens.test.ts`、`frontend/src/workspace/useTheme.ts`、`frontend/src/workspace/useTheme.test.ts`
- Modify: `frontend/index.html`（head 内联防闪烁脚本）、`frontend/src/shell/entry.ts`（+`import "@/styles/workspace/index.css"`）、`ai-pm/scripts/check-prism-tokens.js`（DEFAULT_SCAN_ROOTS 追加 `workspace` 组件目录与 `styles/workspace`；ALLOW_RAW_COLOR_FILES 追加 workspace/tokens.css）

**Interfaces:**
- Produces: `type Theme = "dark" | "light"`；`const THEME_STORAGE_KEY = "aipm.theme"`；`useTheme(): { theme: Ref<Theme>; setTheme(t: Theme): void; toggle(): void }`（写 `document.documentElement.dataset.theme` + localStorage，初始值：dataset.theme 存在用它，否则 localStorage，否则 `"dark"`）。tokens.css 定义 `:root`（深色值）与 `[data-theme="light"]` 覆盖块，变量前缀一律 `--ws-`，含设计稿 §1.1 全部 8 色 + `--ws-color-info/success/warning/danger`（浅：#3B82F6/#22A06B/#D9A441/#E16B73；深色值执行时以对比度测试定值）+ `--ws-focus-ring` + `--ws-sidebar-width:232px`/`--ws-sidebar-collapsed:60px`/`--ws-context-width:320px`/`--ws-topbar-height:52px`；`.workspace-shell` 作用域内 `--prism-surface-*`/`--prism-text-*`/`--prism-border` 等桥接到 `--ws-*`（具体变量名以 `styles/prism/tokens.css` 实际命名为准）。

- [ ] **Step 1: 失败测试** `tokens.test.ts`：解析 tokens.css 断言（a）深浅两块都含 8 表面/文本/AI 色变量与四语义色、focus ring、四个布局尺寸；（b）对比度——`--ws-text-primary`/`--ws-text-secondary` 对 `--ws-canvas` 与 `--ws-surface`、语义色对 `--ws-surface`，深浅两套全部 ≥4.5:1（文件内实现 WCAG 相对亮度计算，无外部依赖）；（c）桥接块存在 `--prism-` 引用 `var(--ws-`。
- [ ] **Step 2: 失败测试** `useTheme.test.ts`：默认 dark；setTheme("light") → dataset.theme="light" 且 localStorage 写入；toggle 往返；dataset.theme 已有值时初始化采用之。
- [ ] **Step 3: 跑两个测试确认失败**（文件未建）。
- [ ] **Step 4: 实现 tokens.css/index.css/useTheme** + index.html 预挂载脚本（读 localStorage 写 dataset.theme，默认 dark）+ entry.ts import + 门禁脚本扩展。
- [ ] **Step 5: 验证**：`pnpm test:unit` 全绿（prism 契约不受影响）；`node ../scripts/check-prism-tokens.js` 通过（新扫描范围无裸色值）。
- [ ] **Step 6: Commit** `feat(workspace): ws token system with dark default, theme persistence and prism bridge`

### Task 2: console fixtures + workspace store

**Files:**
- Create: `frontend/src/workspace/fixtures.ts`、`frontend/src/workspace/fixtures.test.ts`、`frontend/src/workspace/store.ts`、`frontend/src/workspace/store.test.ts`

**Interfaces:**
- Produces（fixtures.ts，V1.1 图面文案照录）: `const demoProjectId = "proj-worldmap"`；`demoProject { id, name: "世界地图编辑器", iteration: "Iteration 2.0" }`；`type AttentionKind = "decision" | "acceptance" | "blocker"`；`interface AttentionItem { id: string; kind: AttentionKind; title: string; description: string; advice: string }`；`attentionItems: AttentionItem[]`（恰 3 条：地形渲染架构评审/decision、高保数据模块验收/acceptance、森林生成性能页/blocker，第三条 advice 对应「缺少目标帧率与视距范围定义」语境）；`briefing { author: "元构", body, suggestion }`（suggestion=「优先确认地形渲染方案，再推进相关实现」）；`metricCards`（值 12/03/04，caption 含「5 项执行中」「1 项最优先」「1 等待审批」）；`goals: { id, name, milestone, progress }[]`、`dynamics: { id, kind: "change"|"failure"|"decision", text, time }[]`；右栏 `contextAdvice = "先验证 Shader 原型性能，再做正式架构选择"`、`contextPlaceholders = ["批量原型验证", "补充证据"]`。
- Produces（store.ts）: `useWorkspaceStore`（pinia setup）→ `{ projectId: Ref<string>, setProject(id: string), selectedAttentionId: Ref<string | null>, selectAttention(id: string | null), selectedAttention: ComputedRef<AttentionItem | undefined>, sidebarCollapsed: Ref<boolean>, toggleSidebar(), contextOpen: Ref<boolean>, toggleContext(), commandMessages: Ref<{ id: string; role: "user" | "assistant"; text: string }[]>, pushCommandMessage(role, text), clearCommandMessages() }`。

- [ ] **Step 1: 失败测试** fixtures.test.ts：attentionItems 恰 3 条且 kind 依次 decision/acceptance/blocker；metricCards 3 张且值 12/3/4；briefing.suggestion 等供稿原文；contextPlaceholders 恰 2 条。
- [ ] **Step 2: 失败测试** store.test.ts：setProject 后 projectId 生效；selectAttention(id) → selectedAttention 返回对应项；selectAttention(null) → undefined；pushCommandMessage 追加且 id 唯一；clearCommandMessages 清空。
- [ ] **Step 3: 确认失败 → 实现 → 通过。**
- [ ] **Step 4: Commit** `feat(workspace): console fixtures and workspace store (V1.1 demo data)`

### Task 3: WorkspaceShell（TopBar + LeftSidebar + 三栏 grid）

**Files:**
- Create: `frontend/src/workspace/shell/WorkspaceShell.vue`、`frontend/src/workspace/shell/TopBar.vue`、`frontend/src/workspace/shell/LeftSidebar.vue`、`frontend/src/workspace/shell/WorkspaceShell.test.ts`

**Interfaces:**
- Consumes: Task 1 tokens/样式基类（`.workspace-shell` grid：`var(--ws-sidebar-width) minmax(0,1fr) var(--ws-context-width)`；`data-sidebar="collapsed"` 时 60px；`data-context="closed"` 时两列）、Task 2 store。
- Produces: 壳内 `<slot name="main"/>` 与 `<slot name="context"/>`；TopBar props `{ project: string; page: string }`；LeftSidebar 用 store + vue-router（当前路由高亮项目总控）；断点：window.matchMedia("(max-width: 1280px)")→store.contextOpen 强制收起、"(max-width: 1024px)"→左栏折叠（组合式监听，测试用 `vi.stubGlobal`/`matchMedia` mock）。

- [ ] **Step 1: 失败测试** WorkspaceShell.test.ts：渲染 TopBar/侧栏/主区 slot/右栏 slot 四区；nav 渲染「工作空间」组（项目总控/AI 团队/目标计划/项目动态）与「研发模块」组（需求管理/决策中心/项目知识/产物验收）及「传统视图」组（概览入口指向 `/dashboard`）；store.toggleSidebar 后壳根元素 `data-sidebar="collapsed"`；mock matchMedia 窄断点后右栏隐藏；面包屑含 project 与 page 文案；TopBar 含主题切换按钮（点击调 useTheme().toggle）。
- [ ] **Step 2: 确认失败 → 实现 → 通过。**
- [ ] **Step 3: Commit** `feat(workspace): workspace shell with V1.1 dual-group nav (spec §二/§五)`

### Task 4: ContextSidebar 右栏

**Files:**
- Create: `frontend/src/workspace/shell/ContextSidebar.vue`
- Modify: `frontend/src/workspace/shell/WorkspaceShell.vue`（context slot 填充 ContextSidebar）
- Test: `frontend/src/workspace/shell/ContextSidebar.test.ts`

**Interfaces:**
- Consumes: Task 2 store（selectedAttention / contextAdvice / contextPlaceholders）、`useWorkspaceStore().toggleContext`。
- Produces: 右栏内容组件（无对外 API）；未选中时「当前待办事项」显示 attentionItems[0]（与 V1.1 原型默认选中一致——fixture 首条即选中初值）；选中变更跟随；「AI 建议」区显示选中项 advice，未选中显示 contextAdvice；底部两个占位区块。

- [ ] **Step 1: 失败测试**：默认渲染首条待办与 contextAdvice；store.selectAttention 第 2 条后右栏标题与建议跟随；点击折叠图标调 toggleContext。
- [ ] **Step 2: 确认失败 → 实现 → 通过。**
- [ ] **Step 3: Commit** `feat(workspace): context sidebar bound to attention selection (spec §五.6)`

### Task 5: 总控台页（五区块 + 降级）

**Files:**
- Create: `frontend/src/workspace/console/ConsolePage.vue`、`console/BriefingCard.vue`、`console/MetricCards.vue`、`console/AttentionList.vue`、`console/GoalStrip.vue`、`console/DynamicsFeed.vue`
- Modify: `frontend/src/shell/router/routes.ts`（静态路由新增：`path: "/workspace/:projectId"`，component 懒加载 WorkspaceShell，redirect 到 `console`，children：`console`（name `workspace-console`，ConsolePage）与 `:page`（name `workspace-page`，复用 `../views/ComingSoon.vue`））
- Test: `frontend/src/workspace/console/ConsolePage.test.ts`

**Interfaces:**
- Consumes: Task 2 fixtures/store、Task 3/4 壳与右栏（经路由组合）。
- Produces: 路由 `/workspace/:projectId/console`；ConsolePage 行为：`route.params.projectId !== demoProjectId` → 降级视图（i18n `workspace.unavailable.title/body` + 返回 `/dashboard` 链接）；否则渲染页头（标题+「项目运行中」徽标+副题）、BriefingCard、MetricCards、AttentionList（点击条目调 selectAttention，选中项紫色描边 `--ws-accent` 描边类）、GoalStrip、DynamicsFeed。routes.ts 挂载为纯新增（不动 shell 路由与 404 兜底）。

- [ ] **Step 1: 失败测试**（ConsolePage.test.ts，直接 mount + 提供 pinia 与 stub route params）：默认渲染简报署名「元构」与建议原文；3 张指标卡值 12/3/4；AttentionList 3 条且徽标文案区分 决策/验收/阻塞；点击第 2 条 → store.selectedAttentionId 更新且该条有选中描边类；GoalStrip/DynamicsFeed 各渲染 fixtures 条数；projectId 为 `"proj-other"` 时渲染降级视图且不含演示数据。
- [ ] **Step 2: 失败测试**（routes 挂载验证并入 Task 8 集成测试，此处不单测路由文件）→ 跳过，Task 8 覆盖。
- [ ] **Step 3: 确认失败 → 实现（ConsolePage + 五子组件 + routes.ts 挂载）→ 通过。**
- [ ] **Step 4: Commit** `feat(workspace): console page V1.1 with five sections and degraded route (spec §五)`

### Task 6: AICommandBar + 快捷键

**Files:**
- Create: `frontend/src/workspace/components/AICommandBar.vue`、`frontend/src/workspace/components/AICommandBar.test.ts`
- Modify: `frontend/src/workspace/shell/WorkspaceShell.vue`（main 列底部固定挂载指令栏；全局 keydown：Ctrl+/ 聚焦指令栏输入、Esc 清空 commandMessages）

**Interfaces:**
- Consumes: Task 2 store（projectId、commandMessages、pushCommandMessage/clearCommandMessages）、fixtures（chip 文案）。
- Produces: 指令栏组件（无对外 API）。行为：chip 显示「AI 研发负责人 · 当前页面上下文」；projectId 为空 → 输入禁用 + i18n 提示（Review Focus 5 前置）；submit → pushCommandMessage("user", text) 且进入 submitting（显示阶段文案 + 取消按钮，取消回 idle 不产生回复）；800ms 后（`setTimeout`，测试用 `vi.useFakeTimers`）pushCommandMessage("assistant", mockReply) 回 idle；「/ 引用」「@ Agent」按钮为禁用占位（tooltip i18n）。

- [ ] **Step 1: 失败测试**：projectId 空 → submit 不可用且有提示；有项目时发送 → 消息列表出现 user 气泡且 phase=submitting；快进 800ms → assistant 气泡出现且 phase=idle；submitting 期间再 submit 被忽略；取消 → 无 assistant 气泡且 phase=idle；Esc → clearCommandMessages。
- [ ] **Step 2: 确认失败 → 实现（含 WorkspaceShell 快捷键）→ 通过。**
- [ ] **Step 3: Commit** `feat(workspace): AI command bar with context chip and mock lifecycle (spec §五/§六硬规则)`

### Task 7: dashboard 预览入口

**Files:**
- Modify: `frontend/src/shell/views/dashboard/DashboardView.vue`（页头操作区加「AI 工作空间（预览）」按钮，`router.push(\`/workspace/${demoProjectId}/console\`)`）
- Test: `frontend/src/shell/views/dashboard/DashboardView.test.ts`（如已有则追加用例；无则新建，mount + mock router）

**Interfaces:**
- Consumes: Task 2 `demoProjectId`。

- [ ] **Step 1: 失败测试**：按钮渲染（i18n key）；点击触发 push 到 `/workspace/proj-worldmap/console`。
- [ ] **Step 2: 确认失败 → 实现 → 通过。**
- [ ] **Step 3: Commit** `feat(workspace): dashboard preview entry to console`

### Task 8: 验收集成测试 + 全量门禁

**Files:**
- Create: `frontend/src/workspace/__tests__/p0-acceptance.test.ts`

**Interfaces:**
- Consumes: 全部前序任务公开接口；真实路由表（`shell/router/routes.ts` 的 staticRoutes）+ pinia。

- [ ] **Step 1: 集成测试**：createRouter(staticRoutes) + mount ShellApp 级入口（happy-dom），**先预置 auth 会话**（按 `src/shell/store/auth` 实际形态写入 localStorage/session，避免 guard 重定向 `/login`）→ push `/workspace/proj-worldmap/console` → 断言：五区块+右栏+指令栏全部渲染；index.html 含预挂载脚本（读源文件断言含 `data-theme` 与 THEME_STORAGE_KEY）；主题切换按钮往返且 localStorage 值变化；点击待处理第 2 条 → 右栏标题跟随；指令栏发送→快进→回复气泡；push `/workspace/other/console` → 降级视图。push `/dashboard` → 存量页照常渲染（回归）。
- [ ] **Step 2: 全量验证**：`pnpm test:unit` 全绿；`pnpm typecheck` 通过；`pnpm lint` 通过；`pnpm build` 通过；`node ../scripts/check-prism-tokens.js` 通过；`git ls-files | grep -c $'\r'` 为 0。
- [ ] **Step 3: Commit** `test(workspace): P0 acceptance suite and full gates (spec §七)`

## Out of Scope（P0 明确不做）

- WorkspaceSpec 协议/registry/布局模板/`/focus`/session 适配（旧 P0 计划 Task 1–5、7、10–13，P1 原样复用）；BFF/后端任何改动；真 AI 会话；classic 页面收编与六页 IA 映射；移动端（<1024 仅折叠处理）；Ctrl+K 命令面板；ai-pm-web 仓的清理删除（留待用户裁决）。
