# AI-PM UI 设计系统 V1.0 — 分析

> **⚠️ 已废弃（2026-11-06）**
> 分析对象 [docs/ui/ai-pm-ui-design-system-v1.md](docs/ui/ai-pm-ui-design-system-v1.md) 已被 [`docs/design/pm-ai-ui-design-system-v2.md`](design/pm-ai-ui-design-system-v2.md)（Slate & Muted Teal）取代，本分析结论随之失效，仅作历史评审记录保留。

> 分析对象：[docs/ui/ai-pm-ui-design-system-v1.md](docs/ui/ai-pm-ui-design-system-v1.md)（用户供稿记录，一~八章）
> 触发：用户 2026-10-31 明示「分析」；供货阶段未做任何评审，本篇为首次评审输出
> 事实核查基础：ai-pm-web/frontend 实仓（package.json、src 结构）+ ai-pm 既有设计稿

---

## 1. 总评

这套 V1.0 的成熟度高于一般「设计规范」：它有清晰的产品观点（右栏不是第二个聊天窗、AI 指令栏属于工作上下文），更有一条防住 AI 应用经典陷阱的架构级规则——**业务组件由业务对象 ID 加载数据，不依赖 AI 聊天消息维护正式状态**。配合「AI 只组合受控组件、不能注入任意前端代码」，AI 边界是这稿最值钱的部分。

**结论：可作为 V1.0 基线交付，但有 2 个必须先裁决的冲突、7 个发给实施方前要补的缺口。** 最大的问题不在设计本身，而在第六章的技术栈落点与 ai-pm-web 现仓的衔接。

---

## 2. 必须先裁决的两个冲突

### 2.1 技术栈：第六章是 React 命名，现仓是 Vue

- V1.0 第六章：`TopBar.tsx`、`workspace-renderer.tsx`、`workspace-store.ts` —— React 载体。
- 现仓事实（ai-pm-web/frontend/package.json）：`vue@3.5` + `pinia` + `vue-router` + `reka-ui`（shadcn-vue 底座）+ `vue-i18n`；架构原则稿第十五章的建议栈也是 vue+TS。

**建议：按 Vue 落地，保留第六章的分层，只换实现载体。** 分层本身（shell / layouts / registry / state / features / components / styles）是好的，映射很直接：

| V1.0 原文 | Vue 落地 |
|---|---|
| `TopBar.tsx` 等五件套 | `TopBar.vue` 等 SFC |
| `workspace-store.ts` | Pinia store（现仓已在用） |
| `workspace-schema.ts` / `component-registry.ts` | 保持 TS，不变 |
| `OverviewLayout.tsx` 等四布局 | `OverviewLayout.vue` 等 |

不推荐反方向（为这套目录改用 React）：ai-pm-web 已有壳、七业务页、容器化与测试，推倒成本不成立。

### 2.2 与 ai-pm-web 现状的衔接（三件事）

1. **页面映射未定义**。六页 IA 与现仓页面的对应关系需要一张迁移表，建议起点：
   - 总控台 ← dashboard + pm/overview 演进合并
   - 目标与计划 ← pm/tasks + pm/requirements
   - AI 执行 / 决策中心 / 项目认知 ← 全新
   - 产物验收 ← pm/approvals + artifacts
   - pm/projects、pm/audit、pm/settings 等 → `classic/`（传统视图）
   - login / profile / settings 留在 app 层，不进工作空间 IA
2. **token 双轨风险**。现仓已有 PRISM token 体系（`src/styles/prism` + `check-prism-tokens.js` 门禁），V1.0 的 `tokens.css` 与它的关系未定义：并存、别名桥接、还是替换？不给结论就会漂移成两套主题。建议桥接：PRISM 组件继续吃 `--prism-*`，在新 tokens.css 里做一层 `--prism-x: var(--x)` 反向映射，门禁脚本加白名单过渡。
3. **左栏折叠形态缺失**。验收清单第 3 条要求左栏可独立折叠，但全文没给折叠后的宽度/形态（旧口述稿是 232→60px 图标栏，未随迁进 V1.0）。

---

## 3. 规格缺口（发给实施方前要补）

1. **语义色未 token 化（最大缺口）**。正文只有「绿/蓝/琥珀/红」四个字，1.1 表和 tokens.css 都没有 `--success/--info/--warning/--danger`（深浅两套）。而「四态可区分」「WCAG AA」都是验收项——目前无法验收。建议直接采用被放弃旧稿的四色 hex 作候选起点（信息 #3B82F6、成功 #22A06B、警告 #D9A441、危险 #E16B73），补深色变体并核对比度后转正。
2. **「紫色克制」规则丢失**。旧稿的「AI 紫只用于 AI 交互/选中态/少数主操作；任务状态用语义色；普通导航、文字、容器保持中性」没有进入 V1.0。这是防止「页面变吵」的唯一约束，建议恢复为一等规则写进第一章。
3. **主题默认值三处口径不一**。tokens.css 的 `:root` 是浅色（即默认浅色）；旧稿口径「深色为默认工作主题」；原型截图是深色。需明确：默认主题、持久化（localStorage/系统跟随）、防闪烁（`data-theme` 预挂载脚本）。
4. **focus ring 无 token**。键盘章要求可见焦点样式，tokens.css 未定义 `--focus-ring`。
5. **深色悬浮层策略缺失**。有 Elevated 面板色，但 Dialog/Popover 在深色下的阴影/描边规则未给——深色主题阴影基本失效，通行做法是描边 + 提亮一档，需写明。
6. **组件状态只有状态名**。hover/focus/disabled/loading/error 列了名，空态/骨架屏/错误态无任何视觉规格；DataTable/TreeView/DependencyGraph 是长内容高风险件，至少要给加载/空/错误三态 + 截断规则。
7. **文本截断与 i18n**。导航/表格/徽标的中英文溢出规则未提（现仓有 vue-i18n，双语是现实的）。

---

## 4. 细节一致性与数值核查

**命名不一致（小）**：1.1 表叫 `Elevated`，CSS 叫 `--surface-raised`；`--radius-sm: 6px` 在 1.2 表无对应条目（卡片 8 / 弹窗 12）——统一命名，或注明 6px 的适用对象。

**对比度手算核查**（按 WCAG 相对亮度公式，最终以对比度工具核验为准）：

| 组合 | 结果 | 判定 |
|---|---|---|
| 深色 主文字 #E9ECF3 on #111318 | ~15:1 | ✓ |
| 深色 次文字 #99A2B2 on #111318 | ~7.3:1 | ✓ |
| 深色 AI 主色 #8B83F6 on #111318 / on Elevated #252A35 | ~5.9:1 / ~4.6:1 | ✓ |
| 浅色 AI 主色 #6258D9 on #FFFFFF / on 侧边栏 #F0F2F6 | ~5.4:1 / ~4.8:1 | ✓ |
| **浅色 次文字 #697386 on 画布 #F7F8FA** | **~4.49:1** | **⚠️ 卡在 AA(4.5) 线下** |

浅色次文字在白色 Surface 上是 4.77:1（达标），但落在画布上 12px 辅助文字即不达标。建议压深一档（#647085 附近）或规定次文字只用于 Surface。

**内部一致性（通过）**：三栏尺寸 232/320/52/72–120 在 Shell 图、布局规则、tokens.css 三处一致；`features/` 七目录与六页 + classic 一一对应；「0 / 10」与清单条数一致；原型截图右栏内容与「右栏默认内容」规则吻合。一个实施注意点：AI 指令栏「主工作区底部固定」意味着它属于 grid 的 **main 列**（不跨右栏），与布局规则第 2 条（指令栏感知当前上下文）自洽。

---

## 5. 三级组件体系与 AI 边界

- 核心规则（业务状态不进聊天消息、AI 只组合受控组件）应落地为机制而非约定：**registry 白名单 + props schema 版本化**。
- 与既有设计稿的概念映射建议补一节，三个现成挂点：AICommandBar 的「引用」↔ 混合工作空间稿附录 C 的 /focus 规格；AgentTimeline ↔ AgentRun 事件流（架构稿 SSE/WS）；KnowledgeCorrection ↔ 知识空间稿的六态发布状态机。
- **缺一个组件**：布局规则第 1 条说右栏「可切换为 AI 对话面板」，但 AI 原生组件清单里没有对应物（ChatPanel/ConversationView）。要么补组件，要么明确它由 AICommandBar + 消息流临时组合。

---

## 6. 验收清单评价

10 项中 8 项可测。两项偏主观，建议可测化：

- 「六个 AI 页面使用一致的视觉语言」→ token lint（禁止裸色值，与 check-prism-tokens 同思路）
- 「AI 与传统视图切换保留业务对象上下文」→ 路由切换的状态快照测试

建议补两条：**性能**（DependencyGraph/长列表虚拟化、首屏预算——依赖图是本稿最高风险组件）、**三态覆盖**（空/加载/错误，与第 3 节缺口 6 呼应）。

---

## 7. 建议的下一步（按序）

1. 裁决 2.1：建议 Vue 落地，第六章改写为 Vue 载体 + 一行栈声明
2. 补四组 token（语义色 / focus ring / 左栏折叠宽 / 默认主题）进 1.1 表与 tokens.css；恢复「紫色克制」规则
3. 出「现有页面 → 六页 IA 映射表」+ PRISM token 桥接方案
4. 验收清单可测化，补性能与三态两条
5. 上述完成后可走 writing-plans 出实施计划；P0 建议 shell + 总控台单页竖切，验证三栏框架与 AI 指令栏上下文引用再铺开
