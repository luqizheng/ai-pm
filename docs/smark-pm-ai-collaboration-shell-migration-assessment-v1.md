# AI Collaboration Workspace V1.0 — Shell 组件复用与迁移边界评估（开发评估任务书）

| 项目 | 内容 |
|---|---|
| 日期 | 2026-11-11 |
| 任务性质 | **开发侧评估**（先评估、再实施），不是编码任务 |
| 触发 | 专家正式裁定四栏 Shell（DR-C8），需在动手前摸清现有壳的复用面与迁移边界 |
| 上位 | [AI Collaboration 设计基线](product/ai-pm-ai-collaboration-workspace-v1.md) · [裁决记录 DR-C7/DR-C8](ui/ai-collab/smark-pm-aic-95-decision-record-v1.md) · [第一阶段契约](ui/ai-collab/smark-pm-aic-91-phase1-prototype-contract-v1.md) |
| 产出 | 评估报告（见 §6 交付物），用于排期与分阶段切分 |
| 纪律 | 本任务书所有代码引用均已 grep 实证；评估时如发现引用有误，以仓库实际代码为准并回写 |

---

## 〇、背景与目标

**裁定结论（DR-C8）**：采用四栏 Shell 作为 V1.0 目标结构，在现有三栏壳上**渐进升级**，保留 V2 深色设计系统、52px TopBar、导航体系与现有上下文数据机制，不另起视觉系统。

**本次评估要回答的问题**：

1. 现有壳的哪些组件与样式可以**直接复用**、哪些需要**改造**、哪些必须**新建**？
2. 从三栏到四栏的**迁移边界**在哪里？每条边界波及哪些文件、哪些数据机制、哪些测试？
3. 能否按「① 四栏容器与 AI Chat 列 → ② 迁移现有会话能力 → ③ 升级 Context Panel」三步推进，**每步可独立验收与独立回退**？
4. 有哪些**不可回退**的风险点必须前置处理？

> 评估阶段**不改生产代码**。除必要的探针实验（临时分支）外，产出为文档。

---

## 一、现状事实基线（已实证）

### 1.1 壳的 DOM 骨架

`ai-frontend/src/components/shell/WorkspaceShell.vue:210-238`：

```vue
<div class="workspace-shell" :data-sidebar="shell.sidebarState" :data-context="shell.contextState">
  <TopBar class="workspace-topbar" @search="searchOpen = true" />      <!-- grid-column: 1 / -1; grid-row: 1 -->
  <SideNav class="workspace-leftnav" />                                <!-- grid-column: 1; grid-row: 2 -->
  <main class="workspace-main">                                        <!-- grid-column: 2; grid-row: 2 -->
    <div class="workspace-main-scroll"><RouterView /></div>            <!-- 唯一滚动容器 -->
    <CommandBar class="workspace-commandbar" />                         <!-- 主列底部停靠 -->
  </main>
  <ContextPanel v-if="shell.contextOpen" class="workspace-context" />   <!-- grid-column: 3; grid-row: 2 -->
  <SearchPalette ... />                                                <!-- 浮层，跨栏 -->
</div>
```

### 1.2 当前网格与尺寸 token

`ai-frontend/src/styles/index.css:27-39`：

```css
.workspace-shell {
  --ws-sidebar-current: var(--ws-sidebar-width);
  display: grid;
  grid-template-columns: var(--ws-sidebar-current) minmax(0, 1fr) var(--ws-context-width);
  grid-template-rows: var(--ws-topbar-height) minmax(0, 1fr);
  height: 100dvh;
}
```

`ai-frontend/src/styles/tokens.css:77-82`：

| Token | 现值 | 目标值（DR-C8） |
|---|---:|---:|
| `--ws-sidebar-width` | 232px | **188px** |
| `--ws-sidebar-collapsed` | 60px | 60px（保留） |
| `--ws-context-width` | 320px | **288px** |
| `--ws-topbar-height` | 52px | 52px（保留） |
| `--ws-commandbar-height-min` / `-max` | 72px / 120px | 由 AI Chat 列的 Composer 继承 |
| （新增）Main Area | `minmax(0, 1fr)` | **518px** |
| （新增）AI Chat | — | **446px** |

合计目标：188 + 518 + 446 + 288 = **1440px**。

### 1.3 状态单一真源（`ai-frontend/src/stores/shell.ts`）

| 字段 | 行 | 语义 | 迁移影响 |
|---|---:|---|---|
| `contextOpen` | 88 | 右上下文栏开关（整栏从 DOM 移除） | 专注模式需与之协同，**不能重置** |
| `commandbarExpanded` | 89 | 指令栏收起/展开 | Composer 迁入 AI Chat 列后语义保留 |
| `pendingAssistantPrompt` | 97 | 指令栏提交、待会话页消费的提问 | **快照交接的关键**，跨列后不得丢 |
| `switchingProjectId` / `switching` | 107 / 120 | 切项目期间冻结上下文 | 四栏下须覆盖全部列 |
| `assistantContext` | 147 | **上下文单一真源**（指令栏作用域串 / 右栏卡 / 会话页三处共用） | **绝不可分叉** |
| `contextState` / `sidebarState` | 117 等 | 驱动根元素 `data-*` 属性 | 专注模式可沿用同机制 |

### 1.4 已落地的上下文数据机制（必须保住）

| 机制 | 位置 | 迁移时的要求 |
|---|---|---|
| 提交时刻快照交接 | `stores/shell.ts:200` `queueAssistantPrompt` → `:211` `takeAssistantPrompt` | 导航与切项目不改变快照；列位置变化不影响 |
| 上下文单一真源 | `stores/shell.ts:147` `assistantContext` | 消费方由 3 处变更多处，但**仍是同一个对象** |
| 线程锚点落库 | `migrations/0003_conversation_context_anchor.sql` | 后端不动 |
| 锚点校验 | `crates/pm-application/src/ai_conversations.rs:82` | 后端不动 |
| 按锚点重投影 | `crates/pm-ai/src/chat_context.rs:100` `build_chat_context` | 后端不动 |
| 注入记账 | `crates/pm-ai/src/conversation_reply.rs:438` `metadata.injection` | 后端不动（但前端仍零消费，见 §4 已知断点） |
| 信封协议 | `crates/pm-application/src/ai_interaction_envelope.rs` | 八类白名单 + 11 态；前端不得扩白名单 |

### 1.5 现有测试的硬断言（迁移的直接闸门）

| 测试 | 断言内容 | 迁移后是否需改 |
|---|---|---|
| `styles/layout.test.ts:29-33` | 壳为 grid；列含 `var(--ws-sidebar-current)` + `minmax(0, 1fr)` + `var(--ws-context-width)` | **需改**（四列） |
| `styles/layout.test.ts:38-57` | 顶栏 `grid-column: 1 / -1`；三区各自显式 `grid-column: 1/2/3` + `grid-row: 2` | **需改**（四区显式列位） |
| `styles/layout.test.ts:62-71` | main `overflow: hidden`；滚动容器 `overflow-y: auto`；壳内**禁止** `position: sticky/absolute` | 保留；新增列须同守 |
| `styles/tokens.test.ts:123-126` | `sidebar-width` = `232px`；`context-width` = `320px`；`topbar-height` = `52px` | **需改**（188/288；52 保留） |
| `shell/WorkspaceShell.test.ts:16-24` | 渲染**五个区域**：顶栏 / 左导航 / 主区 / 右上下文栏 / **指令栏** | **需改**（指令栏成为 AI Chat 列内组件） |
| `shell/WorkspaceShell.test.ts:27-46` | `data-sidebar` / `data-context` 属性与右栏 DOM 移除行为 | 保留，另加专注模式属性 |
| `shell/CommandBar.test.ts` · `ContextPanel.test.ts` · `SideNav.test.ts` · `shell.dom.test.ts` | 各组件行为与 DOM 断言 | 需逐条评估（**blast radius 最大处**） |
| `styles/tokens.test.ts` 其余 | 全应用 token 契约（含禁紫/语义色） | 不改 |

---

## 二、组件复用矩阵（初判，须由开发逐条确认）

| 组件 / 模块 | 现状 | 四栏下的处置 | 风险与要点 |
|---|---|---|---|
| `WorkspaceShell.vue` | 三栏 grid 容器 | **改造**：四列 + 新增 AI Chat 列容器；保留 TopBar/SideNav/ContextPanel | grid 自动放置曾把右栏挤到第 2 列（`index.css:51-54` 注释留证），新增列必须**显式声明列位**，不得依赖自动放置 |
| `TopBar.vue` (423 行) | 横跨三栏 | **直接复用** | 仅需确认 `grid-column: 1 / -1` 在四列下仍成立 |
| `SideNav.vue` (559 行) | 232px 导航树 | **改造（宽度）**：232 → 188；折叠 60px 保留 | 宽度收窄 44px，需核对分组标题与「当前用户信息」底部的换行表现 |
| `CommandBar.vue` (375 行) | 主列底部停靠，72/120px | **迁移 + 改造**：成为 AI Chat 列内的 Composer | 其内部 scope 串（`:56-67`，取自 `shell.assistantContext`）、附件、上下文引用、发送逻辑**可整体复用**；`commandbarExpanded` 语义保留 |
| `ContextPanel.vue` (492 行) | 单张上下文卡 | **改造（结构性）**：→ 288px 四 Tab + History Drawer | 现内容（项目 / 仓库 / 关联 / 注入来源 / readOnly / pinned）**归入 Context 与 Sources 两 Tab**；`assistantContext` 单一真源消费（`:80/:97/:107`）**必须保留** |
| `AssistantView.vue` | 独立路由 `/project/assistant` 的完整会话页 | **能力整合进常驻 AI Chat 列** | 会话列表（`:113/:272/:422`）、消息加载（`:364`）、发送（`:563-574`）、锚点构造（`:539`）、interview 联动（`:388`）均为**可复用资产**；路由可保留为「全屏会话」入口而非废弃 |
| `stores/shell.ts` | 上下文单一真源 + 壳开关 | **扩展**：新增专注模式状态；**不重写**既有字段 | 新增字段不得改变 `assistantContext` 的单一性；`contextOpen` 与专注模式要能叠加且可恢复 |
| `ProjectSwitcher.vue` (375 行) | 切项目 | **复用**；但「切项目不得静默转移上下文」的**提示交互**待补（基线 G12） | 与 `switching` 冻结机制配合 |
| `PageFrame.vue` / `PageHeader.vue` | 页框 / 页头 | **直接复用** | 主区固定 518px 后需复核内边距策略 |
| `index.css` 壳基类 | 三栏 + 显式列位 | **改造** | 保持「壳内禁 sticky/absolute」与「滚动收敛在容器内」两条既有结论 |
| `tokens.css` | `--ws-*` 尺寸 | **扩展**：新增 Main/Chat 宽度 token | 禁裸值原则；新 token 须有浅色变体与契约测试 |

---

## 三、迁移边界清单（每条须评估波及面）

| # | 边界 | 触及的代码/机制 | 评估要点 |
|---|---|---|---|
| **M1** | 网格列数与列位 | `index.css:27-39`、`:51-99` | 四列显式列位；`data-sidebar` / `data-context` 两种状态下的列组合；**不得引入自动放置** |
| **M2** | 主区从自适应变固定 518px | `.workspace-main`、`.workspace-main-scroll` | 内部页面（`PageFrame` 等）是否按百分比/自适应布局；窄屏下主区内容的可读性 |
| **M3** | CommandBar 迁出主列 | `WorkspaceShell.vue:224`、`CommandBar.vue`、`shell.ts:89` | 迁出后主列不再有底部固定条，**主区滚动高度计算需重验**（`index.css:78-82` 的 flex 收敛逻辑） |
| **M4** | 会话能力从路由页进列 | `AssistantView.vue`、`navigation.ts:195-199`、`stores/shell.ts:97/200/211` | 快照交接链（指令栏提交 → 会话页消费）在同列内变成「列内直传」，**但 `/project/assistant` 仍需可用**（全屏/深链），两处不得分叉状态 |
| **M5** | Context Panel 单卡 → 四 Tab | `ContextPanel.vue`、`ContextPanel.test.ts` | 单卡内容如何映射到 Context / Sources；Results / Actions 为**新增视图**（数据来自会话与交互记录） |
| **M6** | History Drawer | 新增 | 覆盖右栏视觉区不改四栏宽度；五类历史数据来源需确认（会话 / AI Action / 上下文变更 / 审批 / 执行）——**其中「上下文变更」与「审批」是否已有可查数据源，需确认** |
| **M7** | 专注模式（Focus Mode） | `shell.ts` 新增状态、`WorkspaceShell.vue`、`index.css` | 与 `contextOpen` 的关系（叠加 / 覆盖 / 互斥）；退出后是否**逐项还原**用户进入前状态（含右栏开合、滚动位置）；编辑草稿与对话不丢 |
| **M8** | 上下文可见性诚实性（G1/G2） | `ContextPanel.vue` 活态读取 vs 后端锚点投影 | **本轮设计可先按「宣称面」画**，但右栏 Sources 若要显示「AI 实际引用的来源」，需消费 `metadata.injection`（当前前端零消费）——评估是否并入本任务，或另开 ticket |
| **M9** | 测试闸门 | `layout.test.ts` / `tokens.test.ts` / `WorkspaceShell.test.ts` / `CommandBar.test.ts` / `ContextPanel.test.ts` / `SideNav.test.ts` / `shell.dom.test.ts` | 列出每个断言的「改 / 保 / 补」；**禁止为了让测试通过而放宽断言**（尤其是禁 sticky/absolute、单一真源一致性） |

---

## 四、明确不在本任务范围

| # | 事项 | 原因 | 去向 |
|---|---|---|---|
| O1 | **DEF-P0-01** 默认放行 → 默认拒绝 | 后端策略语义变更，需服务端自动化测试 | 后端独立任务（`crates/pm-application/src/ai_team_policies.rs:150`） |
| O2 | **DEF-P0-02** `confirm_baseline` 真人审批守卫 | 同上 | 后端独立任务 |
| O3 | 权限交集（Team ∩ Project ∩ Action）实现 | 后端策略引擎 | 后端独立任务 |
| O4 | 会话 / 上下文 / 审批的**数据模型**变更 | V1 不改模型（DR-C6 保持 `project_id NOT NULL`） | 不做 |
| O5 | Figma 视觉稿产出 | 设计侧已出包（`docs/ui/ai-collab/`） | 设计侧 |
| O6 | 移动端布局 | 本轮只保证 1440px 桌面 | 不做 |
| O7 | AI-03~AI-07 五个页面的 Shell 复制 | **门禁未过**：AI-01 与 AI-02 未全部验收 | 第二阶段之后 |

> **UI 调整不能被视为后端权限缺陷已修复**（DR-C8 §6）。本任务只动前端壳与交互，不触碰审批语义。

---

## 五、分阶段切分建议（待评估确认）

| 步 | 内容 | 依赖 | 建议验收 | 回退方式 |
|---|---|---|---|---|
| **①** | 四栏容器 + AI Chat 列骨架（AI Chat 先为占位） | — | 四栏尺寸与显式列位正确；既有页面在主区 518px 下不破版；layout/tokens 测试更新并通过 | 单开关回退三栏（`data-*` 机制已存在） |
| **②** | 迁移会话能力进 AI Chat 列（复用 `AssistantView` 资产） | ① | 快照交接链不变；`/project/assistant` 仍可用且状态不分叉；对话与业务页同屏 | 保留旧入口，列内可退回跳转 |
| **③** | Context Panel 升四 Tab + History Drawer | ①（②可选） | 四 Tab + Drawer；`assistantContext` 单一真源未分叉；既有 ContextPanel 测试逐条迁移 | 右栏可退回单卡（保留旧组件分支） |
| **④** | 专注模式 + 切项目提示交互（G12） | ① | 进出专注模式状态逐项还原；切项目弹确认 | 独立开关 |

> 与 DR-C8 的三步建议（容器 → 迁移会话 → 升级 Context Panel）一致；④ 为本任务追加的低风险尾项，可独立排期。

---

## 六、评估报告交付物（开发产出）

1. **组件复用矩阵**（逐组件：复用 / 改造 / 新建 / 废弃 + 依据 + 波及文件清单）。
2. **迁移边界影响面**（M1~M9 逐条：触及文件、数据机制、风险等级、可回退性）。
3. **状态契约说明**（`stores/shell.ts` 新增/变更字段、既有字段的**不变式**清单）。
4. **测试闸门清单**（每个断言标「改 / 保 / 补」，附理由）。
5. **分阶段实施方案**（确认或修正 §5 的切分，标注每步验收与回退）。
6. **风险清单与前置处理项**（含不可回退项，如有任何）。
7. **工作量估算与依赖**（是否需要后端配合，尤其是 M6 History 数据源与 M8 注入记账消费）。

> 报告落 `docs/` 根目录，命名建议 `smark-pm-ai-collaboration-shell-migration-assessment-report-v1.md`。

---

## 八、实施结果回填（2026-11-11）

按 §5 切分推进，已完成 ①②：

| 步 | 状态 | 落地要点 | 提交 |
|---|---|---|---|
| ① 四栏容器 + AI Chat 列 | ✅ | token 改宽（188/518/446/288）；壳 grid 三栏→四列，四区**显式列位**（沿用既有防自动放置结论）；新增 `.workspace-chat`（grid-column 3）；专注模式 `data-focus="on"` 列组合 | `53ddf93` |
| ② 会话能力并入常驻列 | ✅ | 抽出 `useAssistantConversation` composable（会话页与列共用，**不重建会话体系**）；会话页 23/23 测试行为等价；CommandBar 加 `embedded` 就地发送；`/project/assistant` 路由时列收起避免同屏重复 | `64c8e40` |
| ③ Context Panel 四 Tab + History Drawer | ✅ | 新增 `stores/assistant.ts` 派生视图（**单一写者**：会话机制发布；**读者**：列与右栏）；Sources 消费真实 `metadata.injection`，未注入如实显示；History Drawer 覆盖右栏不改四栏宽度 | `846abed` |
| ④ 专注模式 + 切项目提示（G12） | ◐ | 专注模式已落地（顶栏开关 + 退出逐项还原）；切项目提示待实施 | `53ddf93` |

**评估期发现的存量缺陷（已修）**：`SideNav.vue` 存在残缺属性 `<div class="shell-sidenav__groups" ">`（`a3f92f7c` 引入），属性名非法导致**整个壳测试文件在 master 上挂 6 例**——评估前必须先修，否则壳层验证无闸门。

**门禁现状**：全量 186 文件 / 1063 例通过 · vue-tsc 通过 · eslint 通过 · i18n 双语对齐 · prism 门禁零新增。

---

## 七、给评估的三条提醒

1. **先读 §1.5 的测试断言**——它们是这次迁移的真实闸门，比设计稿更能说明改动面。
2. **不要分叉 `assistantContext`**——它是上下文单一真源，已被三处消费并有测试锁定；四栏下消费方变多，但必须是同一个对象。
3. **不要把「壳改造」做成「视觉重做」**——52px TopBar、V2 令牌与现有视觉系统保留；本轮只改结构与交互。

---

## 附：关联文档

| 文档 | 用途 |
|---|---|
| `docs/ui/ai-collab/smark-pm-aic-95-decision-record-v1.md` | DR-C1~C8 裁决真源（DR-C8 为本次任务的依据） |
| `docs/ui/ai-collab/smark-pm-aic-ui-01-project-ai-workspace-v1-3.md` | AI-01 四栏 Shell 设计稿（结构基准） |
| `docs/ui/ai-collab/smark-pm-aic-ui-02-context-management-v1-3.md` | AI-02 Context 管理设计稿 |
| `docs/ui/ai-collab/smark-pm-aic-91-phase1-prototype-contract-v1.md` | 第一阶段契约（状态、专注模式、渐进迁移、演示与验收） |
| `docs/ui/ai-collab/smark-pm-aic-expert-briefing-v1.md` | 现状说明（三栏壳实况 → 四区差异 + 已裁 Q1~Q4） |
| `docs/product/ai-pm-ai-collaboration-workspace-v1.md` | 设计基线（附录 B 差距清单 G1~G13） |
