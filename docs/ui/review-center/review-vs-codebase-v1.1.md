# 「人工审核中心」专家方案 V1.1 × 本仓现状复核

> 评审对象：[expert-proposal-v1.1.md](expert-proposal-v1.1.md)
> 结论：**V1.1 可冻结为实施基线（R01–R12）**。专家预留的两个「硬前置核实项」均已从代码直接回答——一个现成、一个现成；真正剩下的后端改动只有 1 处（session_id）。另纠正专家 1 处列宽笔误。

---

## 一、专家两个「核实项」的实证答案

| 专家步骤 | 核实项 | 实证结果 |
|---|---|---|
| 2 | Proposal 版本读取接口（expected_version_no 来源） | ✅ **现成**：`GET /v1/pc/proposals/{id}`（p4）响应已含 `expected_version_no`（`knowledge_services.rs:545`），UI 读详情→原样回传 p5 即可 |
| 3 | Delivery link → taskId 可靠性 | ✅ **现成**：聚合 Delivery 条目 `link = { kind: "task_delivery", params: { task_id } }`（`command_center/action.rs:141-143`）；任务详情路由 `/project/tasks/:taskId` 已存在（`router/index.ts:259`）。R08 前提成立，CTA 可直接构造 |

CAS 强度补充：application 层 `DecisionInput.expected_version_no: i64` 非可选（`pc/proposal.rs:135`，「请求侧 CAS：调用方确认的提案基线版本号，须等于提案锚定值」）——R07「必须携带」成立，且缺省/错值必然 409。审批锚定机制（content_hash + FOR UPDATE + 三方哈希比对）见 `pc/proposal.rs:19-21`。

## 二、对 V1.1 的 1 处纠偏

**§7.1 线框把 AI Chat 标成 288px。** 实际列职责（`WorkspaceShell.vue:23-24`）：

| 列 | 宽度 | 说明 |
|---|---|---|
| 左导航 | 232（折叠 60） | SideNav |
| 主内容区 | minmax(0,1fr)，1440 基准 ≈474 | 队列所在 |
| **AI Chat** | **446（常驻）** | 专家线框误标 288 |
| 右上下文栏 | 288（可关） | ContextPanel，Drawer 覆盖域 |

推论：720px Drawer「覆盖主区 + 右上下文栏」尺寸自洽（474+288=762 ≥ 720）；实现时须处理 ContextPanel 开关与 Drawer 的叠加（Drawer 覆盖域按实际开启列计算，zIndex 低于全局模态）。

## 三、R01–R12 冻结确认（附实现锚点）

| # | 基线 | 复核 |
|---|---|---|
| R01 | /project/review-center，项目级 | ✅ 与 C2 一致 |
| R02 | 232/474/446/288 | ✅ |
| R03 | 紧凑队列 + 720px Drawer | ✅ 尺寸自洽（见 §二） |
| R04 | 专注双栏不进 V1 | ✅ |
| R05 | 统一理由组件，领域配置必填 | ✅ Finding answer 必填 / Proposal note 可选已实证 |
| R06 | Finding link 带 session_id | ✅ 表有列（`discovery/finding.rs:3`），待改 `action.rs` |
| R07 | Proposal 必带 expected_version_no，409 不自动重试 | ✅ CAS 硬约束（§一） |
| R08 | Delivery CTA → /project/tasks/:taskId | ✅ 前提已实证（§一步骤 3） |
| R09 | 导航徽标 + 指挥中心计数共用聚合 | ✅ **`NavItem.badge?: number` 字段现成**（`navigation.ts:42`，注释即「收件箱未读数」）；指挥中心面板标题已展示 pendingCount（`CommandCenterView.vue:226`）——两处前端成本极低 |
| R10 | i18n YAML | ✅ |
| R11 | V2 深色、雾蓝青、禁紫 | ✅ |
| R12 | 零 mock | ✅ = AGENTS.md 规则 4 |

专家交付文案的其余可复用细节：Drawer 关闭清 `item` 留 `kind` 的 URL 语义、409 独立文案、Delivery 详情状态文案改「交付证据可查看，验收操作暂未开放」——均采纳。

## 四、修正后的实施顺序（与专家 §八 合并实证结果）

| 顺序 | 工作项 | 状态 |
|---|---|---|
| 1 | **后端**：`command_center/action.rs` Finding 条目 link.params 补 `session_id`（含单测） | 唯一后端改动 |
| 2 | Proposal 版本读取 | ~~核实~~ **已实证现成** |
| 3 | Delivery taskId | ~~核实~~ **已实证现成** |
| 4 | 路由 + navGroups 条目（badge） + `review.yaml` + `stores/reviewCenter.ts` | — |
| 5 | 474px 队列（复用 getCommandCenter 的 human_action_summary block） | — |
| 6 | Drawer 框架 + 三类领域上下文组件 | — |
| 7 | Finding 决策闭环（verification，answer 必填） | 依赖 1 |
| 8 | Proposal 决策闭环（p4→p5，CAS + 409 文案） | — |
| 9 | Delivery 只读 + CTA | — |
| 10 | 异常态/权限反馈/空态 + 徽标接线 | — |

## 五、挂账（不变）

H1 全局跨项目徽标/收件箱（需跨项目聚合端点）· H2 AI Chat 绑定当前审核事项上下文（V1.1）· H3 已处理历史视图 · H4 available_actions 契约 · 专注模式双栏（R04）· T-070 验收工作台 · 导入提案二期。
