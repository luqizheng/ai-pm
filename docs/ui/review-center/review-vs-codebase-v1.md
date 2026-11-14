# 「人工审核中心」专家方案 V1.0 × 本仓现状评审

> 评审对象：[expert-proposal-v1.md](expert-proposal-v1.md)（外部专家建议稿，非终裁）
> 评审方式：逐条对照本仓代码实证（2026-11-14，master f5fb98f7）
> 结论：**采纳 19 项 / 修正 6 项 / 挂账 4 项 / 视觉否决 1 项**。修正项集中在「专家按过时布局与全局 IA 设计」，领域方案本身全部成立。

---

## 一、事实核验表（专家断言 → 代码实证）

| # | 专家断言 | 实证 | 依据 |
|---|---|---|---|
| F1 | 聚合三类条目，字段 kind/domain/id/title/detail/source_state/updated_at/action_hint/link | ✅ 一致 | `crates/pm-application/src/command_center/action.rs:50-145` |
| F2 | detail 截断 160 字 | ✅ | `action.rs:78,108`（`truncate(&s, 160)`） |
| F3 | Finding 已有决策端点 | ⚠️ 有，但**要 session_id** | `apps/pm-api/src/pm_discovery.rs:50`：`POST …/sessions/{session_id}/findings/{finding_id}/verification`；聚合 link 只带 `finding_id`（`action.rs:83`）→ **缺口，见 C4** |
| F4 | 决策端点理由参数存疑 | ✅ 存在且**必填** | `VerificationBody{status, answer, confirmed_finding?, operation_id?}`（pm_discovery.rs:319-326）；合法决策值 `CONFIRMED`/`REJECTED`（`discovery/mod.rs:88`） |
| F5 | Proposal 确认/否决端点已有 | ✅ | `knowledge_services.rs:71-84`：`GET /v1/pc/proposals/{id}`、`POST /v1/pc/proposals/{id}/decision`、`GET /v1/pc/proposals` |
| F6 | Delivery 有证据读取、无验收 | ✅ 仅两个 GET | `pm_delivery.rs:27-30`：`GET /v1/pm/tasks/{task_id}/delivery`、`GET /v1/pm/projects/{project_id}/delivery` |
| F7 | 四栏 188/518/446/288（518 队列 + 446 详情） | ❌ 过时 | 实际 **232/474/446/288**（`WorkspaceShell.vue:23-24`），主区 `minmax(0,1fr)`，1440 基准下仅 474px（`styles/index.css:39`） |
| F8 | `/review` 顶层路由 | ⚠️ 不成立 | 本仓项目页全部在 `/project/*` 下；且 `review` 名已被占：`/project/execution/review`（`router/index.ts:164`） |
| F9 | 全局一级导航 + 全局徽标 | ⚠️ 数据不支持 | 聚合接口是**项目级**（`load_human_action_summary(pool, tenant_id, project_id)`）；无跨项目聚合端点 |
| F10 | id 全局唯一存疑 | ✅ 谨慎正确 | kind+id 复合键采纳；三类 id 分属不同表 |
| F11 | i18n 用 `review.json` | ❌ 不符约定 | 本仓全 YAML：`ai-frontend/locales/zh-CN/{*.yaml, s01/…, ai/…}`；同层禁同名键 |
| F12 | processing/completed 仅为 UI 文案 | ✅ | 与 D7 一致 |
| F13 | 指挥中心面板只跳转不处理 | ✅（现状事实） | `CommandCenterView.vue:114-129` 仅 `router.push` |

## 二、裁决表

### 2.1 采纳（19 项）

| # | 专家决策 | 备注 |
|---|---|---|
| A1 | D1 定位三分：指挥中心=看盘 / 审核中心=决策 / AI Chat=辅助 | — |
| A2 | D4 先看上下文再决策；**列表不放决策按钮** | — |
| A3 | 混排队列 + 类型 Tab 作筛选（不按 kind 分页） | — |
| A4 | 「队列 + 详情」双区概念 | 形态按 C3 修正落地 |
| A5 | V1 无批量决策 | — |
| A6 | V1 无撤销窗口 | 领域可逆性未证 |
| A7 | 三阶段交互状态机（Pending / Processing=前端临时态 / Completed） | — |
| A8 | 异常分支表（409 重载、403 无权、**超时≠失败不盲目重试**） | — |
| A9 | 处理后移除 + 自动定位下一条 + 空态四类 | — |
| A10 | D6 权限：后端裁决，403 反馈，不预判隐藏按钮 | 本仓 API 错误体只有 message，UI 直接展示 |
| A11 | D8 组件纯展示化、文案经 props 传入 | 与本仓 AGENTS.md §三.11 组件策略完全同款 |
| A12 | D9 零 mock | = AGENTS.md 规则 4；专家配图页脚「以上为虚构展示数据」即反面示范 |
| A13 | kind+id 复合键、URL 恢复筛选与选中 | — |
| A14 | 数据映射表 + 禁推导字段清单（不伪造 priority/assignee/diff/…） | — |
| A15 | Pinia `stores/reviewCenter.ts` | 本仓 stores/ 已有 auth/shell/notifications/team/assistant 先例 |
| A16 | 组件树四层；AI Chat 复用壳，不在页内重建 | — |
| A17 | Delivery V1 只读预留 + 「工作台未开放」透明空态 | F6 实证无验收端点 |
| A18 | 仓库导入提案二期 + 禁伪造 ActionItem | — |
| A19 | 三原则：非第二状态权威 / AI 不代审 / 统一队列≠统一决策 | — |

### 2.2 修正（6 项）

| # | 专家方案 | 修正为 | 依据 |
|---|---|---|---|
| C1 | 全局一级导航 + 全局徽标 | **项目内**一级页：`navGroups` 项目导航组新增「人工审核中心」；徽标数复用 command-center 聚合 `actionTotal`（`real-data.ts:346`）；指挥中心面板 goto 改指审核中心对应事项 | 聚合是项目级（F9）；跨项目徽标挂账 H1 |
| C2 | 路由 `/review` | `/project/review-center`；查询参数 `?kind=&item=` 保留（恢复语义不变） | F8：层级不符 + `review` 名已被 `/project/execution/review` 占用 |
| C3 | 518 队列 + 446 详情双栏 | **主区（1fr）放队列 + 详情走 Drawer/覆盖面板**；专注模式（`data-focus="on"` 主区独占）下可选双栏；AI Chat 446 常驻保留（意图不变） | F7：1440 基准主区仅 474px，双栏放不下；ui-smoke 已有「窄主区表单破损」前科。Drawer 先例：右栏 History Drawer |
| C4 | Finding「已有决策端点」直接闭环 | 后端**小改先行**：`action.rs` 的 Finding `link.params` 补 `session_id`（`discovery_finding` 有该列，`finding.rs:3`）；前端经 `?item=` 定位后调 verification 端点 | F3：无 session_id 链路断裂；纯代码改，无迁移 |
| C5 | i18n `review.json` | `locales/zh-CN/review.yaml` + `locales/en/review.yaml`（扁平键或分组，注意同层禁同名键） | F11 |
| C6 | 「拒绝时建议填理由（存疑）」 | 决策理由**必填**：`answer` 为必填 String；status 取值 `CONFIRMED`/`REJECTED`（确认与拒绝都建议带理由输入，否则 422） | F4 |

### 2.3 挂账（4 项）

| # | 项 | 条件 |
|---|---|---|
| H1 | 全局（跨项目）徽标/收件箱 | 需新后端跨项目聚合端点 |
| H2 | AI Chat 绑定当前审核事项上下文 | 机制已有（contextItems/会话 composable），V1.1 接；V1 保留普通 Chat |
| H3 | 「已处理」历史视图 | 需后端历史查询能力 |
| H4 | `available_actions` 能力信息契约、优先级/责任人筛选 | 需数据契约扩展 |

### 2.4 否决（1 项）

| # | 项 | 理由 |
|---|---|---|
| X1 | 专家配图中的**紫色**类型标签 | V2 视觉锁定禁紫色；落地按 tokens.css V2 深色语义色档 |

## 三、V1 冻结基线（修正版 D1'–D10'）

- **D1'**（=D1）独立人工决策收件箱，统一接收需人类介入的事项。
- **D2'**（修正 C1/C2）项目内一级页 `/project/review-center` + 项目内徽标 + 指挥中心快捷入口；全局徽标挂账 H1。
- **D3'**（修正 C3）主区队列 + 详情 Drawer；AI Chat 常驻列保留；专注模式可选双栏。
- **D4'**（=D4）先上下文后决策，列表无决策按钮。
- **D5'**（=D5）领域边界：Finding→Discovery、Proposal→Knowledge、Delivery→Delivery；审核中心不做第二状态权威。
- **D6'**（=D6）后端授权，403 反馈。
- **D7'**（=D7）领域状态是业务事实；Processing 仅为前端临时态；成功后刷新聚合。
- **D8'**（=D8）组件纯展示化，文案 props 传入。
- **D9'**（=D9）零 mock，只消费真实接口。
- **D10'**（=D10）Finding/Proposal 完整闭环（前置 C4 后端小改）；Delivery 只读；导入提案二期。

## 四、修正后的实施顺序

1. **后端小改**：`command_center/action.rs` Finding 条目 `link.params` 补 `session_id`（含单测）。
2. 页面骨架：`/project/review-center` 路由 + `navGroups` 条目 + `review.yaml` i18n + `stores/reviewCenter.ts`。
3. 接入聚合：复用 `getCommandCenter` 的 `human_action_summary` block（不加新端点），混排队列 + Tab/筛选/排序。
4. Proposal 闭环：p4 详情 + p5 decision（二次确认弹窗）。
5. Finding 闭环：按 `?item=` 加载 finding 上下文 + verification（理由必填）。
6. Delivery 只读：证据读 + 透明「未开放」空态。
7. 异常/权限反馈 + 空态 + 自动定位下一条。
8. 挂账清单入 backlog（H1–H4 + T-070 验收 + 导入提案二期）。

## 五、聚合容量注意

聚合每类 `LIMIT 50`（action.rs:54/93/123），items 上限 150 条；筛选/排序在前端封顶列表内进行——单类 >50 待办时列表不完整。V1 接受并在页脚标注；后续如需完整分页由后端扩独立聚合端点（与 H1 同批评估）。
