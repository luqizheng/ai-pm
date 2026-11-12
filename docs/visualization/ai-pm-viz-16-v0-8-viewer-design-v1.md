# AI-PM Visualization V0-8 技术设计（Viewer 前端）V1.0

> 状态：设计稿（证据齐备，待 Frontend/Security Owner 签收） · 日期：2026-11-12
> 上游：[ADR V1.0.1 + V1.0.2 待签收修订](ai-pm-viz-09-architecture-decision-record-v1.md)（附录 A 的 M-1/M-4/M-7）· [15 号安全审查](ai-pm-viz-15-v0-6-security-review-v1.md)（产物静态化）· [18 号 UX 规格 V1.1](ai-pm-viz-18-v0-8-ux-spec-v1-1.md)（**交互规格权威**：本文只讲前端落点）· [13 号 V0-2 数据模型](ai-pm-viz-13-v0-2-data-model-design-v1.md)· [14 号 V0-3 投影](ai-pm-viz-14-v0-3-projection-consistency-design-v1.md)
> 落地约定：本文所有"仓库事实"均来自 2026-11-12 对 `ai-frontend/` 的实证（路径:行号随附）。

## 1. 范围与形态（按 15 号修订后的最终形态）

| 通道 | 渲染内容 | 渲染器 | 交互 | 依赖 |
|---|---|---|---|---|
| **产物通道**（Artifact） | 服务端产出的 **SVG** | `<img src=blob:…>` | **无**（静态预览） | 无图形库 |
| **交互通道**（Live） | VizSpec 数据 | **G6 5**（P1 引入） | 完整（hover/focus/route/lens） | G6 |
| **降级通道** | 表格 | `Table.vue`（既有） | 排序/分页 | 无 |

**P0 交付 = 产物通道 + 降级通道**（零新增图形依赖）；**P1 交付 = 交互通道**。

## 2. 组件结构

```
src/components/viz/
  VizArtifactFrame.vue      产物通道：<img> 引用 SVG（blob）+ 状态条（stale/draft）+ 操作槽
  VizSpecTable.vue          降级通道：nodes/edges 表格（含 source_ref 列）
  VizTracePanel.vue         溯源侧栏（复用 Sheet 范式）
  VizVersionBar.vue         版本切换 / 对比 / 导出菜单
  VizStaleBar.vue           黄条 + 重新生成
  VizEmptyState.vue         空态（禁演示图）
src/components/viz/responses/
  VizArtifactResponse.vue   对话卡（AiResponseType 第 9 类）
src/lib/viz/
  types.ts                  VizSpec TS 类型（与后端契约同源）
  specAdapter.ts            后端 payload → VizSpec 的唯一转换点
  useVizArtifact.ts         pinia store：加载/审批/导出
```

**复用既有件（不自造）**：

| 需求 | 复用 | 证据 |
|---|---|---|
| 表格 | `src/components/ui/table/Table.vue` + TableHeader/Body/Row/Cell（`sortable`/`sort`/`selected`/`numeric`） | 实证 §4 |
| 徽章 | `Badge.vue`（`variant: neutral\|accent\|count\|success\|warning\|danger\|info`，`size`，`outline`） | 同上 |
| 空态 | `EmptyState.vue`（`title/description/icon/actionLabel` + emit `action`） | 同上 |
| 抽屉（溯源侧栏） | `Sheet.vue`（`{open,title,description,closeLabel,side}`）+ 业务范式 `SourceSheet.vue` | `requirement-detail/components/SourceSheet.vue` |
| 标签页 | `Tabs.vue`（reka-ui `TabsRoot/TabsList/TabsTrigger/TabsContent`，`tabs: TabItem[]`） | `ui/tabs/Tabs.vue:9-16` |
| 图形侧检查器范式 | `DependencyGraphView.vue:485-577`（`<aside class="pg-side">` + 面板） | — |
| 滚动 | `ScrollArea.vue` | — |

## 3. 产物通道（V0-8 核心，P0）

### 3.1 呈现方式：`<img>` + Blob objectURL

**不能用 `<img src="/api/...">`**：`<img>` 无法带 Bearer 头（后端用 JWT）。**既有范式**：`src/api/base/self-service.ts:90-108` —— SDK 下载字节 → `URL.createObjectURL(new Blob([bytes], {type}))`。

```
后端 SVG 文本/字节
  → api 层 http.get（带 Authorization）
  → new Blob([bytes], {type: "image/svg+xml"})
  → URL.createObjectURL(blob)
  → <img :src="objectUrl" alt="...">
  → onUnmounted → URL.revokeObjectURL(url)   ← 必须，否则 Blob 泄漏
```

| 决策 | 取值 | 理由 |
|---|---|---|
| 引用方式 | `<img>`（**非** `<object>`/`<iframe>`） | `<img>` 引用 SVG 时浏览器**不执行脚本**（15 号 §3）；`<object>` 可执行 SVG 内脚本 → 禁用 |
| 传参 | Blob objectURL（非 data: URI） | data: URI 过大且进 DOM 属性；objectURL 可 revoke |
| 降级 | Blob 创建失败 → `VizSpecTable` 降级表格 | 链路不阻断 |
| alt 文本 | 用 spec 的 title + 节点数（可访问性） | — |
| 尺寸 | CSS `width:100%` + `height:auto`；容器高度上限 `--size-ai-artifact-preview-height`（360px，tokens.css:449-467） | 不新增 token |

**安全约束（15 号 §3.3 L1/L2 在前端的落点）**：
- 前端**不解析** SVG 内容，**不注入** innerHTML，**不加载**任何产物脚本；
- `vite` 构建产物中不得出现 SeeCode 产物 JS（AC-18′ 源码 + 构建双断言）；
- `<img>` 引用天然阻断脚本执行（SVG in `<img>` 为非脚本上下文）。

### 3.2 产物卡（对话内）

```
┌─ VizArtifactResponse ─────────────────────────────┐
│ [静态预览图 · <img>]              ⚠ stale / draft │
│ 标题 · 节点 N · 边 M · 生成时间                    │
│ [查看详情] [下载 SVG] [接受] [拒绝] [反馈]          │
│ 来源：3 个对象已溯源 · [查看溯源]                    │
└──────────────────────────────────────────────────┘
```

| 元素 | 行为 | 门 |
|---|---|---|
| 预览 | `<img>` 静态 | — |
| 状态条 | draft=灰徽「草稿·未生效」/ stale=黄条 / published=正常 | — |
| 接受/拒绝 | **仅**宿主卡面按钮（不在产物内） | `ensure_trusted_context` + 真人门 |
| 拒绝 | reason 必填 | `viz_approval_reject_reason_check`（13 号 §3.3） |
| 下载 SVG | 走 API 取字节 → `URL.createObjectURL` → `<a download>` | 权限同读取 |
| 溯源入口 | 打开 `VizTracePanel` | — |
| 解释图 | `nature=explanatory` 恒显「AI 生成 · 非正式认知」徽标，**无接受/发布入口** | 13 号 §3.2 DB CHECK + 14 号 §6 |

**交互归属变更（15 号 §5.2）**：产物卡**不再有** hover trace / focus panel / route journey（那些由产物 JS 提供，已弃用）。取而代之：
- 节点级交互在 **P1 的 G6 交互通道**提供；
- P0 期间，节点级信息通过 **`VizSpecTable`（降级通道）** 暴露 —— 它不是"降级"，而是 P0 的**节点导航通道**（可点击跳转原文档）。

> 这需要在 UI 上明示，避免用户以为功能缺失：卡片底部标注「交互探索在 P1 上线，当前提供表格视图」。

### 3.3 溯源侧栏（VizTracePanel）

复用 `Sheet.vue` + `SourceSheet.vue` 范式（右侧抽屉）。内容：

| 段 | 内容 |
|---|---|
| 标题 | 节点 label + kind 徽章 + status |
| 溯源 | source_ref 全量字段（domain/type/id/revision_id/locator）+ [打开原文档] 按钮 |
| 关系 | 入边/出边列表（按 relation 分组，Direct/Indirect 分区显示，indirect 标「经 X 推导」） |
| 计数 | 出边数 / 入边数（**裁剪后**的值，14 号 §3.2 L2-d） |
| 无溯源 | `unlinked` 灰标 + 「该节点无溯源」文案（15 号 AC-15） |

**权限**：节点来自裁剪后的 spec → 侧栏不再自行查权限（避免二次不一致）；[打开原文档] 走目标域自身路由，由该路由的权限门裁决。

## 4. 交互通道（P1，G6 5）

### 4.1 引入决策（V-1 验证项）

| 约束 | 实证 |
|---|---|
| 仓内**零图形库** | `package.json:36-69` 与 `pnpm-lock.yaml` 精确包名匹配 0 命中（无 d3/cytoscape/mermaid/g6/konva/fabric/echarts/elkjs/graphlib/@vue-flow） |
| 既有图全靠手写内联 SVG | `DependencyGraphView.vue:411-452`（`<svg>` + `<defs><marker>` + `<path>`）、`TimelineView.vue:674-687` |
| 无 sanitize 库 | 无 DOMPurify / sanitize-html；`v-html` 仅 1 处（`MarkdownViewer.vue:82`，Vditor `md2html`） |

**结论**：G6 是**新增依赖**（唯一新增的图形依赖），需按 V-1 实测（gzip ≤ 350KB、150 节点 ≥ 50fps、动态加载不进主 bundle）。**不复用 G6 的 HTML 节点渲染**（`html` shape 允许自定义 HTML → 引入 XSS 面），只用 SVG shape + 文本节点（G6 默认 SVG 文本渲染，不经 innerHTML）。

### 4.2 交互映射（15 号 §5.2）

| SeeCode 原交互 | G6 实现 | 数据依赖 |
|---|---|---|
| hover trace | `mouseenter` 节点 → 高亮入边/出边 + 其余降透明度 | Spec edges |
| focus panel | `click` 节点 → `VizTracePanel`（同 §3.3 组件） | Spec + source_ref |
| route journey | 两点选择 → 服务端算路径（P3 关键路径复用同一算法）→ 高亮 + 步骤列表 | 端点查询 |
| lens | 按 `edge_type`/`kind` 过滤高亮 | Spec lenses 字段 |
| pan/zoom/overview | G6 内置 | — |

**零产物代码**：G6 渲染的是 **VizSpec 数据**（我们自己的 DTO），不涉及任何 SeeCode 产物 → 满足 AC-18′。

## 5. 右栏 Graph Tab（增量）

| 改动点 | 文件:行 | 内容 |
|---|---|---|
| Tab 类型 | `src/stores/assistant.ts:20` | `AssistantTab` 加 `"graph"` |
| Tab 常量 | `src/components/shell/ContextPanel.vue:125-130` | `TABS` 加 `{ id: "graph", labelKey: … }` |
| 渲染分支 | `ContextPanel.vue:290/408/490` 之后 | `v-else-if` 分支 → `VizGraphTab.vue` |
| 状态 | `stores/assistant.ts:63,99-101` | 复用 `activeTab` / `setActiveTab` |
| i18n | `locales/{zh-CN,en}/shell.yaml` | `shell.app.context.tabs.graph`（两语言 key 集合必须一致，`check-i18n-namespace.js`） |

**数据来源**：右栏不另起 fetch（`ContextPanel.vue:428` 注释：唯一写者 `useAssistantConversation.ts:432-465` 的 `publish()`）→ Graph Tab 内容由同一发布通道供数。

**右栏宽度**：288px 固定（`tokens.css:87` + `index.css:26-52`）→ Graph Tab 内容需适配窄栏（槽位列表 + 缩略预览，不放主画布）。

## 6. 路由与视图

| 项 | 做法 | 依据 |
|---|---|---|
| 主视图 | `/project/:projectId/visualizations`（在 `buildRoutes()` 内 `routes.push`，`component: () => import(...)` 懒加载，`meta.title` 为 i18n key） | `src/router/index.ts:404-409` 既有子页写法 |
| 项目上下文 | `?project_id=` 钥匙模式，**query 优先 → 回退 `useScopeStore().currentProjectId` → 排除 `SCOPE_ALL` 哨兵** | `src/views/project/design/use-design-project.ts:20-34`（推荐范式）；哨兵常量 `src/api/scope.ts` |
| 页面根 | 必须 `PageFrame` | `ai-frontend/AGENTS.md:223-232`；`src/components/shell/PageFrame.vue:25-38` |
| 不占主导航 | 放 `navigation.ts` 之外，仅路由注册 | 避免改动既有导航树 |

## 7. 新增第 9 类 `viz_artifact` 的六处必改（漏一处即破 fail-closed）

| # | 文件:行 | 改动 |
|---|---|---|
| 1 | `src/components/ui/ai-interaction/types.ts:17-25` | `AiResponseType` 联合类型加 `"viz-artifact"` |
| 2 | `types.ts:33-42` | `RESPONSE_TYPES` 运行时白名单加同一项（与后端 `RESPONSE_TYPES` 同源） |
| 3 | `types.ts:94-120` | `PAYLOAD_REQUIRED_FIELDS` 新增条目 —— **漏改则必填字段不校验 = fail-closed 破口** |
| 4 | `conversation/AiConversationMessage.vue:35-44` | `CARD_COMPONENTS` Record 新条目 → `VizArtifactResponse.vue` |
| 5 | `card/AiResponseCard.vue:93-102` + `:104-113` | `TYPE_KEY` 与 `TYPE_BADGE`（Record 全量键，缺一即 TS 编译错） |
| 6 | `locales/{zh-CN,en}/ai/card.yaml` + `locales/*/shell.yaml:300-309` | `ai.card.type.viz-artifact`；`shell.app.context.responseTypes.*`（被 `ContextPanel.vue:502` 消费） |

**关键机制**：后端信封 → 前端信封的唯一转换点 `src/composables/useAssistantConversation.ts:67-123`，其中 `:76-81` 白名单校验、`:94-100` payload 深校验，**任一缺失即整信封拒收 → 退化为纯文本**（fail-closed，`:64-66` 注释）。

> ⚠️ **这意味着：新类型未登记前，所有 viz 消息会静默变成文字**，不会报错。验收必须断言"信封被识别为 viz-artifact 类型"（而非"没报错"）。

## 8. 样式与令牌

| 项 | 规则 | 依据 |
|---|---|---|
| 新令牌 | 任何新颜色/尺寸/时长**必须**先写入 `src/styles/tokens.css`（**深浅两主题块都要写**），否则 `check-prism-tokens.js` 规则 2 报错 | `scripts/check-prism-tokens.js:135-141`；`tokens.test.ts:52-56`（变量完整 + WCAG AA 对比度） |
| 禁裸值 | 禁裸 hex/rgb/hsl/lab/具名色；禁裸 px > 2px；禁裸 ms/s；断点 ∈ {1440,1240,1100,880,640} | 规则 1/3/4/5（`check-prism-tokens.js:7-18`） |
| 动效时长 | 用 `--duration-instant/fast/medium/slow`（`tokens.css:133-141`） | 同上 |
| reduced-motion | 逐项降级（`AGENTS.md:203`；既有 59 处 `@media (prefers-reduced-motion: reduce)` 范式） | 同上 |
| 深色阴影 | 深色主题 `shadow-xs..lg: none`（`tokens.css:422-427`）→ 层次靠边框，不靠阴影 | 同上 |
| z-index | 无令牌（PRISM 不检查）；沿用小整数 + 壳层 120 分层（`index.css:193`） | 同上 |
| 图相关令牌 | 仓内**无** viz/图表专用令牌 → 需新增 `viz` 令牌子集（走 tokens.css + tokens.test.ts 流程） | `tokens.css` grep 仅命中 `--ws-canvas` |

## 9. 测试设计

| 项 | 方案 | 依据 |
|---|---|---|
| 组件单测 | 首行 `// @vitest-environment happy-dom`；用 `src/lib/test-utils.ts` 的 `mountShell/settle`；Teleport 需 `afterEach` 清 `document.body.innerHTML` | `responses-a.test.ts:40-46`；`src/lib/test-utils.ts:18-40` |
| 每视图 ≥1 vitest | 断言**行为**而非类名 | `ai-frontend/AGENTS.md:202` |
| 产物卡行为用例 | ① 渲染 `<img>` 且 `src` 为 blob: ② 卸载后 `revokeObjectURL` 被调用 ③ Blob 失败 → 降级表格 ④ stale/draft 徽标正确 ⑤ explanatory 无接受按钮 ⑥ 拒绝必填 reason | — |
| **AC-18′** | 构建产物 grep 无 SeeCode 产物 JS；交互组件源码不 import 产物 HTML；产物以 `<img>` 引用（无 `<object>`/`<iframe>`） | 15 号 §6 |
| **AC-7′** | 前端断言 `<img>` 引用（非 `<object>`）；产物 SVG 静态化断言在**后端侧**执行（入库前） | 15 号 §6 |
| AC-14 i18n | `viz.*` 命名空间 zh-CN 与 en **key 集合完全一致** | `check-i18n-namespace.js`；`check-i18n-hardcode.js`（禁源码中文，`*.test.ts` 豁免） |
| PRISM | `pnpm check:prism-tokens` 绿 | `ai-frontend/package.json:19` |
| E2E | `ai-frontend/e2e/`（`p0-*.spec.ts` 命名，走 `playwright.p0.config.ts`；串行 workers:1） | 实证 §8 |

## 10. 实施任务清单

| # | 任务 | 完成定义 |
|---|---|---|
| V0-8.1 | API 层 | `src/api/visualizations.ts`（顶部块注释写契约与边界 → 类型 → 纯函数 `http.get`；分页用 `Paged<T>`）；Blob 下载 helper（复用 `self-service.ts:90-108` 范式） |
| V0-8.2 | 类型与转换 | `src/lib/viz/types.ts` + `specAdapter.ts`（后端 payload → VizSpec **唯一**转换点） |
| V0-8.3 | 产物卡 | `VizArtifactResponse.vue` + `VizArtifactFrame.vue`（`<img>` + blob + revoke + 状态条 + 操作槽） |
| V0-8.4 | 降级表格 | `VizSpecTable.vue`（nodes/edges + source_ref 列 + unlinked 标注）；**同时作为 P0 节点导航通道** |
| V0-8.5 | 溯源侧栏 | `VizTracePanel.vue`（Sheet 范式 + Direct/Indirect 分区） |
| V0-8.6 | 版本与状态 | `VizVersionBar.vue` + `VizStaleBar.vue` + `VizEmptyState.vue` |
| V0-8.7 | 第 9 类信封 | 六处必改全改 + 信封识别断言用例 |
| V0-8.8 | 右栏 Graph Tab | `stores/assistant.ts` + `ContextPanel.vue` 三处 + i18n |
| V0-8.9 | 路由与主视图 | `/project/:projectId/visualizations` + PageFrame + `?project_id=` 钥匙模式 |
| V0-8.10 | 令牌与 i18n | `tokens.css` 新增 `viz` 子集（深浅双主题）+ `tokens.test.ts` 绿 + `locales/{zh-CN,en}/viz.yaml` |
| V0-8.11 | 测试 | §9 全部用例；`pnpm test:unit` + `check:prism-tokens` + `check:i18n` 全绿 |
| V0-8.12 | **AI 解释面板** | `VizAiExplanation.vue`：Direct / Indirect 分区、关键发现、潜在风险、待确认事项；数据来自信封 payload（非模型自由文本直出，须经白名单字段过滤）——依据 [18 号 UX 规格](ai-pm-viz-18-v0-8-ux-spec-v1-1.md) §4.1 D |
| V0-8.13 | **反馈区** | `VizFeedbackArea.vue`：自然语言反馈 → 触发新一轮 AI 生成（走 V0-5 链路）；**不得直接修改业务对象**；反馈不得携带用户未授权对象 ID（服务端校验）——依据 18 号 §4.1 E |
| V0-8.14 | **P0 缺口明示** | 产物卡与 Viewer 页脚标注「交互探索在 P1 上线，当前提供表格视图」（UX-D11 / UX-P0-11） |

## 11. 待签收与偏差

| # | 项 | 决策人 | 说明 |
|---|---|---|---|
| F-1 | P0 产物为静态预览（无交互） | Product Owner | §3.2；UI 需明示"交互探索在 P1 上线" |
| F-2 | 节点级信息在 P0 由表格通道承担 | Product/Frontend | §3.2；表格兼作导航通道而非仅降级 |
| F-3 | G6 为唯一新增图形依赖，且仅用 SVG shape | Frontend/Architecture | §4.1；禁用 G6 HTML shape |
| F-4 | 右栏 Graph Tab 是否纳入 P0 | Product | §5；右栏窄栏适配成本 vs 价值 |
| F-5 | `viz` 令牌子集的色板归属 | Design/Frontend | 深色 V2（雾蓝青）+ 节点 kind / 边 rel 色板；禁紫色 |
| F-6 | 产物 SVG 在对话卡内的默认尺寸 | Frontend/Design | 复用 `--size-ai-artifact-preview-height`（360px）或新增令牌 |
