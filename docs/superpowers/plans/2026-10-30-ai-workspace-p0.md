# AI Workspace P0（工作空间基础版）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 P0 工作空间基础版：Workspace Shell、混合式模式切换、AI 指令栏、项目总控、/focus 对象聚焦视图、组件注册表与统一对象路由——验收标准是"需求详情 ↔ AI 分析无损往返"。

**Architecture:** 在 ai-pm-web 新增独立的 workspace 模块（`src/workspace/`），静态路由树 `/workspace/:projectId/**` 挂进现有 vue-router；视图渲染走 WorkspaceSpec 协议（zod 校验 → 组件注册表 → 4 布局模板），P0 数据全部来自现有 mock fixtures（零 BFF/后端改动）。存量七页按 B-3 裁决挂到 `/classic/*`。

**Tech Stack:** Vue 3 + TypeScript + Vite；vue-router 5、pinia 3、Tailwind + shadcn-vue（PRISM 壳既有）；**新增依赖仅 zod**（spec/props 校验）；vitest（测试）。

**Spec:** `docs/architecture/ai-pm-hybrid-workspace-design.md`（V1.0 定稿；重点 §一.3 路由、§1.4 与 B-3 归属表、§5.1/5.5 Registry 与生命周期、§5.6 布局模板、§6.5 WorkspaceSpec 与安全边界、附录 C /focus 规格、附录 D Session 语义）

## Global Constraints

- 新增 npm 依赖**只有 zod**；不引入新 UI 框架、不换路由/状态库（spec §6.4 裁定的 V1 必需集里 Pinia/Vue Router/shadcn-vue/Tailwind 已在库中）。
- 所有新文件 **LF 行尾**，提交前过 prettier（CRLF 曾打挂契约测试）。
- 界面文案全部走 i18n（`src/shell/i18n.ts`，zh-CN 与 en 同步补 key）。
- 生命周期硬规则（spec §5.5）：可执行动作不得本地点击即判成功——P0 无真后端，`useCommand` 的 succeeded 由 mock 回执驱动，但状态机必须完整。
- 响应式契约（spec B-2 裁决）：主区 < 1280px 隐藏右上下文栏、< 1024px 折叠左导航至图标栏——用壳的 `useResponsive`。
- 本计划**不改** ai-pm 与 project-manager 两仓；BFF（ai-pm-web/backend）P0 零改动，数据一律走 `src/api/mock/fixtures.ts` 扩展。
- 测试命令：`cd ai-pm-web/frontend && pnpm test:unit`（vitest，组件测试沿用 `*.test.ts` 同目录模式）。

## Review Focus

1. URL 直访未知 objectType（`/focus/foo/1`）→ 400 风格降级页，不白屏 → Task 10 测试。
2. spec/registry 校验失败（未注册组件、config 类型非法）→ FallbackPanel 文本+引用降级，不渲染半成品 → Task 2/3 测试。
3. /focus 深链刷新（F5）→ 从 session 快照/fixtures 恢复，不白屏 → Task 10/13 测试。
4. 无项目上下文时指令栏发送 → 阻止并提示选择项目（spec §6.5：上下文由路由解析）→ Task 8 测试。
5. 从 /focus 返回时存在未保存编辑 → 明确提示，不静默丢弃（spec §1.4④）→ Task 11 测试。

---

## File Structure

```
ai-pm-web/frontend/src/workspace/
  spec/types.ts            # WorkspaceSpec TS 类型（与 spec §6.5 协议一致）
  spec/schema.ts           # zod schema + parseWorkspaceSpec
  spec/schema.test.ts
  registry/registry.ts     # 组件注册表（§5.1）
  registry/registry.test.ts
  registry/FallbackPanel.vue   # 清单外/校验失败的文本+引用降级
  layout/templates.ts      # 4 模板定义 + 槽位契约（§5.6）
  layout/WorkspaceCanvas.vue   # spec → 模板 + panels 渲染管线
  layout/templates.test.ts
  composables/useResource.ts   # loading→ready→stale/error（§5.5）
  composables/useCommand.ts    # idle→submitting→succeeded/failed（§5.5）
  composables/useResource.test.ts / useCommand.test.ts
  routes.ts                # /workspace/:projectId/** 静态路由树（含 classic/focus）
  store/workspace.ts       # pinia：projectId/subject/UI 现场（§1.4④）
  store/workspace.test.ts
  session/sessionStore.ts       # 快照存取 + 最近 10 份历史（附录 D）
  session/localAdapter.ts       # localStorage 适配器（D.2 P0 形态）
  session/sessionStore.test.ts
  shell/WorkspaceShell.vue      # 左导航双空间 + 右上下文栏 + 指令栏的壳
  shell/nav.ts                  # 双空间导航配置
  shell/WorkspaceShell.test.ts
  components/AICommandBar.vue   # Registry 组件：指令入口
  components/ContextInspector.vue   # Registry 组件：右栏上下文
  components/AICommandBar.test.ts / ContextInspector.test.ts
  focus/presets.ts          # objectType → 默认 WorkspaceSpec（附录 C.2）
  focus/FocusView.vue       # /focus 渲染 + 403/404/未知类型降级（C.3）
  focus/focusHrefs.ts       # createFocusHref / 返回恢复工具（C.4）
  focus/presets.test.ts / focusHrefs.test.ts
  home/HomeView.vue         # 项目总控 P0 版（简报/待我处理/进展）
  home/HomeView.test.ts
```

Modify：`src/shell/router/routes.ts`（挂载 workspace 路由树）、`src/api/mock/fixtures.ts`（requirement/task fixture 带 id/version/关联）、i18n 两个 locale 文件。

---

### Task 1: WorkspaceSpec 协议与 zod 校验

**Files:**
- Create: `src/workspace/spec/types.ts`、`src/workspace/spec/schema.ts`
- Test: `src/workspace/spec/schema.test.ts`

**Interfaces:**
- Produces: `type WorkspaceSpec`（字段与 spec §6.5 逐字一致：schemaVersion/workspaceId/projectId/subject?/layout/panels）；`type LayoutTemplate`；`type FocusObjectType = (typeof FOCUS_OBJECT_TYPES)[number]`；`const LAYOUT_TEMPLATES = ["overview","split","timeline","explorer"] as const`；`const FOCUS_OBJECT_TYPES = ["requirement","task"] as const`（附录 C.1 P0 子集）；`parseWorkspaceSpec(input: unknown): { ok: true; spec: WorkspaceSpec } | { ok: false; errors: string[] }`。

- [ ] **Step 1: 写失败测试**：合法 spec 通过；layout 越界、panels 空、component 非字符串、dataRef 缺 id、schemaVersion ≠ "1.0" 各自返回 `ok:false` 且 errors 非空。
- [ ] **Step 2: 跑测试确认失败**（parseWorkspaceSpec 未定义）。
- [ ] **Step 3: 实现 types.ts + schema.ts**（zod `z.object`；subject/dataRef 用 `z.object({ type: z.string(), id: z.string() })`；config 用 `z.record(z.unknown()).optional()`——协议层保持 §6.5 的刻意宽松）。
- [ ] **Step 4: 跑测试确认通过。**
- [ ] **Step 5: Commit** `feat(workspace): workspace spec schema (spec §6.5)`

### Task 2: 组件注册表与降级渲染

**Files:**
- Create: `src/workspace/registry/registry.ts`、`src/workspace/registry/FallbackPanel.vue`
- Test: `src/workspace/registry/registry.test.ts`

**Interfaces:**
- Produces: `interface WorkspaceComponentDef { id: string; component: Component; propsSchema?: ZodType }`；`registerWorkspaceComponent(def: WorkspaceComponentDef): void`；`getWorkspaceComponent(id: string): WorkspaceComponentDef | undefined`。FallbackPanel props：`{ reason: string; dataRef?: string }`。

- [ ] **Step 1: 失败测试**：注册后可取回；未注册 id 返回 undefined；带 propsSchema 的组件 `validateProps(def, config)` 非法时返回 errors。
- [ ] **Step 2: 确认失败 → Step 3: 实现**（普通 Map 注册表 + zod safeParse 包装）。
- [ ] **Step 4: 通过 → Step 5: Commit** `feat(workspace): component registry with schema validation (spec §5.1)`

### Task 3: 布局模板引擎（4 模板 + 槽位顺序绑定）

**Files:**
- Create: `src/workspace/layout/templates.ts`、`src/workspace/layout/WorkspaceCanvas.vue`
- Test: `src/workspace/layout/templates.test.ts`

**Interfaces:**
- Consumes: Task 1 `parseWorkspaceSpec`、Task 2 `getWorkspaceComponent`/FallbackPanel。
- Produces: `const LAYOUT_SLOT_COUNT: Record<LayoutTemplate, number>`（overview:2 / split:2 / timeline:1 / explorer:2）；`WorkspaceCanvas.vue` props `{ spec: WorkspaceSpec }`——按数组顺序绑定槽位（spec §6.5 整理说明 4 的 V1 规则）。

- [ ] **Step 1: 失败测试**：槽位数表正确；`resolveSlots(spec)` 超出槽位数的 panel 归入"溢出折叠区"而非丢弃或报错；**spec 中引用未注册组件 id 时该 panel 渲染 FallbackPanel（Review Focus 2）**。
- [ ] **Step 2–4: 失败→实现（模板用 CSS Grid，纯展示组件）→通过。**
- [ ] **Step 5: Commit** `feat(workspace): layout templates engine (spec §5.6)`

### Task 4: useResource / useCommand 生命周期组合式函数

**Files:**
- Create: `src/workspace/composables/useResource.ts`、`src/workspace/composables/useCommand.ts`
- Test: 两个同名 `.test.ts`

**Interfaces:**
- Produces: `useResource<T>(fetcher: () => Promise<T>)` → `{ state: Ref<"loading"|"ready"|"stale"|"error">; data: Ref<T|undefined>; refresh(): Promise<void> }`；`markStale()` 由外部事件触发。`useCommand<TArgs>(executor: (args: TArgs) => Promise<unknown>)` → `{ phase: Ref<"idle"|"submitting"|"succeeded"|"failed">; submit(args): Promise<void>; error: Ref<string|undefined> }`。

- [ ] **Step 1: 失败测试**：useResource loading→ready、reject→error、markStale→stale；useCommand idle→submitting→succeeded、reject→failed、**submitting 期间重复 submit 被忽略**。
- [ ] **Step 2–4: 失败→实现→通过。**
- [ ] **Step 5: Commit** `feat(workspace): lifecycle composables (spec §5.5)`

### Task 5: workspace 路由树 + 项目上下文 store

**Files:**
- Create: `src/workspace/routes.ts`、`src/workspace/store/workspace.ts`
- Modify: `src/shell/router/routes.ts`（在静态路由中挂 `/workspace/:projectId` 子树，AppShell 之外独立 WorkspaceShell 布局；404 路由保持兜底）
- Test: `src/workspace/store/workspace.test.ts`

**Interfaces:**
- Produces: `workspaceRoutes: RouteRecordRaw[]`（home / classic/* / focus/:objectType/:id，见 §一.3 路由 + B-3 表）；`useWorkspaceStore()` → `{ projectId, subject, viewState, setProject(id), setSubject(s), captureViewState(route, state), restoreViewState(route) }`。守卫：projectId 为空 → 重定向项目选择（暂用 `/dashboard`）。

- [ ] **Step 1: 失败测试**：setProject/setSubject/captureViewState/restoreViewState 的状态流转；viewState 按 route 全名 keyed。
- [ ] **Step 2–4: 失败→实现（pinia setup store）→通过。**
- [ ] **Step 5: Commit** `feat(workspace): route tree and project context store (spec §1.3)`

### Task 6: WorkspaceShell 壳（双空间导航 + 右上下文栏 + 折叠）

**Files:**
- Create: `src/workspace/shell/WorkspaceShell.vue`、`src/workspace/shell/nav.ts`
- Test: `src/workspace/shell/WorkspaceShell.test.ts`

**Interfaces:**
- Consumes: Task 5 store、Task 8 AICommandBar、Task 9 ContextInspector、`useResponsive`。
- Produces: 壳三区布局（§二规格：左导航 232px 可折叠至图标栏、主区 router-view、右栏 300px <1280px 隐藏）；`nav.ts` 导出双空间导航项数组（AI 空间六入口中 P0 仅 home 高亮可用，其余 ComingSoon；传统空间按 B-3 表）。

- [ ] **Step 1: 失败测试**：导航分组渲染（AI 空间/传统空间两组）；折叠态只显图标；右栏在窄断点隐藏。
- [ ] **Step 2–4: 失败→实现（复用 `src/components/ui` 与壳图标）→通过。**
- [ ] **Step 5: Commit** `feat(workspace): workspace shell with dual-space nav (spec §2, B-1)`

### Task 7: 存量页挂载 /classic + overview 重定向

**Files:**
- Modify: `src/workspace/routes.ts`（classic 子路由）
- Test: `src/workspace/routes.test.ts`（新增）

**Interfaces:**
- Consumes: `import.meta.glob` 或直接 import `src/shell/views/pm/*` 既有页面组件。
- Produces: `/workspace/:projectId/classic/{requirements,tasks,approvals}` 挂现有页；`classic/overview` → redirect `/workspace/:projectId/home`（B-3 裁决）；audit/projects/settings 指向占位（P1 迁移，B-3 表口径）。

- [ ] **Step 1: 失败测试**：路径解析到正确组件；overview 重定向目标正确。
- [ ] **Step 2–4: 失败→实现→通过。**
- [ ] **Step 5: Commit** `feat(workspace): mount legacy pages under /classic (B-3)`

### Task 8: AICommandBar 指令栏（注册表组件 + mock 会话）

**Files:**
- Create: `src/workspace/components/AICommandBar.vue`
- Modify: `src/api/mock/fixtures.ts`（新增 `mockAiReply(prompt: string, subject?: SubjectRef): Promise<string>`，固定文案按 subject 类型分支）
- Test: `src/workspace/components/AICommandBar.test.ts`

**Interfaces:**
- Consumes: Task 2 注册（`registerWorkspaceComponent({ id: "AICommandBar", ... })`）、Task 5 store（projectId/subject）。
- Produces: 指令栏 UI（输入、当前上下文 chip 显示 subject、发送）；mock 应答走 fixtures 的 `mockAiReply(prompt, subject)`。

- [ ] **Step 1: 失败测试**：无 projectId 时 submit 被阻止并显示 i18n 提示（Review Focus 4）；有上下文时 chip 显示 `requirement:R-101` 形式；发送后追加一条 mock 回复到本地消息列表。
- [ ] **Step 2–4: 失败→实现→通过。**
- [ ] **Step 5: Commit** `feat(workspace): AI command bar with context binding (spec §2/§6.5)`

### Task 9: ContextInspector 右上下文栏

**Files:**
- Create: `src/workspace/components/ContextInspector.vue`
- Test: `src/workspace/components/ContextInspector.test.ts`

**Interfaces:**
- Consumes: Task 5 store、Task 4 useResource（数据 fixtures）。
- Produces: 右栏内容——项目名、当前 subject（可移除=清空 subject、可固定=P0 记忆到 session）、操作权限占位。

- [ ] **Step 1: 失败测试**：subject 变化右栏跟随；"移除"清空 subject 且路由仅去 context 参数不跳走。
- [ ] **Step 2–4: 失败→实现→通过。**
- [ ] **Step 5: Commit** `feat(workspace): context inspector panel (spec §2)`

### Task 10: /focus 路由 + 预设 + 降级

**Files:**
- Create: `src/workspace/focus/presets.ts`、`src/workspace/focus/FocusView.vue`
- Modify: `src/workspace/routes.ts`（focus 子路由 + 未知类型处理）
- Test: `src/workspace/focus/presets.test.ts`、`FocusView.test.ts`

**Interfaces:**
- Consumes: Task 1 FOCUS_OBJECT_TYPES/parseWorkspaceSpec、Task 3 WorkspaceCanvas、Task 5 store、fixtures。
- Produces: `buildFocusPreset(objectType: FocusObjectType, id: string, projectId: string): WorkspaceSpec`（附录 C.2 的 requirement/task 两行）；`resolveFocus(objectType: string)` → `{ ok: true } | { ok: false; reason: "unknown-type" }`。

- [ ] **Step 1: 失败测试**：preset 的 layout/panels 与 C.2 表一致；未知 objectType 渲染降级页（Review Focus 1）；id 在 fixtures 中不存在渲染 404 降级 + 检索建议（C.3）；深链刷新不白屏（Review Focus 3）。
- [ ] **Step 2–4: 失败→实现→通过。**
- [ ] **Step 5: Commit** `feat(workspace): /focus view with presets and degradation (appendix C)`

### Task 11: 无损往返（AI 分析入口 + 返回恢复现场）

**Files:**
- Create: `src/workspace/focus/focusHrefs.ts`
- Modify: `src/shell/views/pm/requirements`（详情操作区加"AI 分析"按钮，调 `createFocusHref`）、`src/workspace/focus/FocusView.vue`（"返回原页"动作）
- Test: `src/workspace/focus/focusHrefs.test.ts`（+ 集成测试覆盖往返）

**Interfaces:**
- Consumes: Task 5 store 的 viewState、`useConfirm`（壳既有）。
- Produces: `createFocusHref(projectId, objectType, id, anchor?: string): string`（含来源路由 query `from`）；`returnHref(): string`（读 `from`，无则 fallback `/workspace/:projectId/home`）。

- [ ] **Step 1: 失败测试**：进入 /focus 前 captureViewState（筛选/滚动）；返回后 restoreViewState 恢复同值；**存在未保存编辑时返回触发 confirm，取消则留在 /focus**（Review Focus 5）；createFocusHref 带 anchor。
- [ ] **Step 2–4: 失败→实现→通过。**
- [ ] **Step 5: Commit** `feat(workspace): lossless round-trip between classic and focus (spec §1.4, flow E)`

### Task 12: 项目总控 P0 版（/home）

**Files:**
- Create: `src/workspace/home/HomeView.vue`
- Test: `src/workspace/home/HomeView.test.ts`

**Interfaces:**
- Consumes: fixtures（简报 3 条、待处理 2 项、进展 1 组）、Task 4 useResource、Task 11 createFocusHref。
- Produces: 三层结构（§3.1 的 P0 裁剪）：简报卡（文本+引用降级形态）、待我处理（只读卡，显示"为什么/推荐/是否不可逆/最迟何时"四要素）、目标进展（只读 GoalCard 占位）。每张卡可下钻 /focus。

- [ ] **Step 1: 失败测试**：渲染三层数据；待处理卡四要素齐备；点击卡跳对应 /focus href。
- [ ] **Step 2–4: 失败→实现→通过。**
- [ ] **Step 5: Commit** `feat(workspace): home dashboard P0 (spec §3.1)`

### Task 13: WorkspaceSession 本地适配器

**Files:**
- Create: `src/workspace/session/sessionStore.ts`、`src/workspace/session/localAdapter.ts`
- Test: `src/workspace/session/sessionStore.test.ts`

**Interfaces:**
- Consumes: Task 1 WorkspaceSpec。
- Produces: `interface SessionAdapter { load(projectId: string): WorkspaceSpec | undefined; save(projectId: string, spec: WorkspaceSpec): void; history(projectId: string): WorkspaceSpec[] }`；`localStorageAdapter`（附录 D.2 P0 形态：sessionStorage 存会话 id 实现多标签独立，localStorage 存快照，保留最近 10 份历史 D.3）。

- [ ] **Step 1: 失败测试**：save 后 load 返回同 spec；history 上限 10；两个"标签"（不同 session id）互不覆盖。
- [ ] **Step 2–4: 失败→实现→通过。**
- [ ] **Step 5: Commit** `feat(workspace): workspace session local adapter (appendix D)`

### Task 14: 验收集成测试 + 门禁

**Files:**
- Test: `src/workspace/__tests__/p0-acceptance.test.ts`
- Modify: 无（只加测试）

**Interfaces:**
- Consumes: 全部前序任务的公开接口。

- [ ] **Step 1: 写 P0 验收集成测试**（对应九章 P0 验收标准原文"用户能够从需求详情无损切换到 AI 分析，并从 AI 分析返回原始需求"）：classic/requirements → AI 分析（带锚点）→ FocusView 渲染 requirement 预设 → 返回 → 断言筛选与滚动恢复、无未保存编辑时不弹确认。
- [ ] **Step 2: 跑全量测试** `pnpm test:unit`，Expected: 全部 PASS。
- [ ] **Step 3: 门禁**：prettier/lint 通过（LF 核对）、`pnpm build` 通过、`git ls-files | grep -c $'\r'` 为 0。
- [ ] **Step 4: Commit** `test(workspace): P0 acceptance round-trip (spec ch.9)`

---

## Out of Scope（P0 明确不做）

- BFF/后端任何改动（含权限真实校验——P0 用 fixtures 模拟 403 形态）；真 AI 会话（mockAiReply）；/focus 除 requirement/task 外的 objectType 预设；ExecutionTimeline/PlanEditor 等 P1+ 组件；session 的 PG 适配器（P1）；移动端完整适配（B-2：P3 后独立里程碑）。
