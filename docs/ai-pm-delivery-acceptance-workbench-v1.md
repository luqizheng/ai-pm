# T-070 设计稿：项目交付验收工作台 V1

| 项 | 内容 |
| --- | --- |
| 任务 | T-070 执行中心接真数据 + 交付验收页（M7，前置 T-031 / T-041 已完成） |
| 状态 | **已实施**（2026-11-02 用户以「执行」过稿并同日落地，实施记录见 §十） |
| 日期 | 2026-11-02 |
| 前置文档 | `ai-pm-task-state-machine-v1.md`（13 态 / E2 门）、`ai-pm-slice-d4-delivery-v1.md`（§2.3 清单读模型）、`ai-pm-slice-d5-review-v1.md`（§5.2 审批裁决）、`ai-pm-task-detail-page-v1.md`（页面契约 / W1 写路径红线）、`ai-pm-multi-repo-model-v1.md`（§8.3 交付清单口径）、`ai-pm-ui-pages-v1-2.md`（V1.2 原型） |
| 上位约束 | `target.md` §P10「AI 执行中心 / 交付验收」两行；`AGENTS.md`（无存量兼容负担）；todo.md §0.1/§0.2 |

## 〇、一句话定位

**项目级交付验收工作台：新增项目级聚合读模型，补齐验收证据读取能力，复用现有审批写路径，不改变任务状态机契约。**

---

## 一、意图与 why

target.md §P10 的六大工作区里，「AI 执行」与「交付验收」是两个真空。执行中心已接真数据（commit `1682abe`）但详情缺租约 / executor 信息；交付验收页完全缺失。T-041（D4 交付事实）与 T-043（D5 审批裁决）已把事实与裁决落库，缺的是把它们组织成验收人可用的界面与读模型。

验收人的真实动线（用户裁定）：**第一眼看到「项目里所有待验收的交付」并快速判断每个是否具备验收条件 → 点进单个任务核对证据 → 做出通过 / 退回决定**。

### 四概念分离（贯穿全稿的语义基础）

Task 完成、Agent/Run 完成、交付完成、人工验收通过是**四个不同概念**，本稿全部分开取数、分开展示，绝不合并成一个状态：

| 概念 | 载体 | 来源 |
| --- | --- | --- |
| 任务生命周期 | `delivery.task.status`（13 态） | `Submitted` / `Reviewing` = 待验收入口 |
| 执行 Run | `delivery.task_run.status` + 租约列 | 执行中心页 |
| 仓库交付事实 | `v_task_delivery_manifest.delivery_state` | 交付清单（`delivered ≠ 已验收`，D4 §2.3） |
| 人工验收决定 | `governance.review_approval` | D5 单事务裁决 |

---

## 二、目标 / 非目标

### 目标

1. **项目交付验收总表**（新页面）：顶部三指标（待验收 / 可验收 / 存在阻塞）+ 待验收任务列表（含「待补充」任务，不被过滤）。
2. **任务交付验收详情**：多仓交付清单卡（基线 / commit / 分支 / 测试证据 / 交付态）、验收资格判定、Agent 执行情况、验收问题（findings）、验收标准逐条裁决、审批历史。
3. **验收动作**：通过 / 退回（复用 D5 decision 端点），记录不可覆盖。
4. **执行中心详情补齐**：run 详情展示租约（`lease_expires_at`）、`executor_kind` / `executor_session`。
5. 全部走真数据：`VITE_USE_MOCK=0` 必须能跑；mock 分支同步更新 fixtures 供单测。

### 非目标（不做）

- **不做「部分通过」**：13 态状态机没有部分完成的生命周期语义；部分通过涉及交付范围拆分与任务拆分，是领域模型变更而非 UI 功能（用户裁定，V1 明确不支持）。V1 审批结果只有 `approved` / `rejected`。遗留登记见 §九。
- **不做 Agent 管理页**（P9 延后，todo 既有边界）。
- **不做假数据页**、不改 13 态状态机、不新增审批状态 / 状态转换（Q3）。
- **不做执行日志内容读取**：日志在执行端磁盘 `{AI_PM_WORKSPACE_ROOT}/{run_id}/logs/`，不在库内；V1 单机部署下 pm-api 与执行器虽可能同机，但 M8（T-081）跨机暴露后该耦合即断裂。**裁定（设计者裁量）**：V1 只展示 `executor_session` + `workspace_path` 定位信息，日志内容读取留 M8 一并设计（与 T-081 同批裁「跨机日志怎么读」）。
- 不做交付 diff 预览、合并操作（属 Git 工作流，`target.md` P10 未要求）。

---

## 三、硬约束（用户裁定 Q1–Q3）

| # | 约束 |
| --- | --- |
| **Q1** | 待验收集合**以 `Submitted` / `Reviewing` 为唯一任务级入口口径**，不得通过仓库交付状态筛选任务。未提交任何仓库交付的待验收任务也必须出现在总表（标「待补充」）。 |
| **Q2** | 验收资格与阻塞原因**由后端统一计算**，至少考虑：预期仓库与实际交付数量（`expected/actual_delivery_count`）、每仓 `delivery_state`、Run 级 `integration_status`、未解决的 blocker 级 `review_finding`。**前端禁止自行拼表推导**——所有徽标（可验收 / 待补充 / 存在阻塞）直接消费后端字段。 |
| **Q3** | 验收决定 V1 只允许 `approved` / `rejected`，**复用 T-043 既有写路径**（`POST /v1/pm/review-approvals/{approval_id}/decision`），不新增审批状态和状态转换。 |

### 三指标口径（三者是不同维度的概念，不得混同）

| 指标 | 维度 | 口径 |
| --- | --- | --- |
| 待验收 | 任务生命周期 | 项目内 `status ∈ {Submitted, Reviewing}` 的任务数 |
| 可验收 | 交付证据完整性 | 待验收中：交付证据满足验收前置条件（见 §四判定规则） |
| 存在阻塞 | 验收问题 / 交付条件 | 待验收中：存在未解决 blocker 级验收问题或交付阻塞 |

每个待验收任务恰好落一个徽标：**存在阻塞 > 可验收 > 待补充**（优先级从高到低，互斥展示）。

---

## 四、验收资格判定规则（后端唯一实现，Q2）

按 task 计算（输入全部来自已落库事实）：

```
eligible（可验收）     = 所有 delivery_required 仓库 delivery_state='delivered'
                         AND 最新 delivery run integration_status='passed'
                         AND 不存在未解决的 blocker review_finding
blocked（存在阻塞）    = 存在未解决的 blocker review_finding
                         OR（已有交付活动且最新 delivery run integration_status='failed'）
missing（待补充）      = 其余情况（零交付、部分交付且集成未失败、无审查记录等）
```

- 「交付阻塞」按用户口径表原话「未解决的验收问题**或交付阻塞**」实现：`integration_status='failed'` 属阻塞（集成事实失败，验收人需要看到失败原因），而零交付 / 部分交付属「待补充」——证据还没齐，谈不上阻塞。

- 「未解决」的定义：blocker finding 所属 review run 之后没有一次 `succeeded` 的 review run 覆盖（即该发现未被后续审查关闭）。V1 简化：**取每个 task 最新一次 `run_kind='review'` 且 `status='succeeded'` 的 run，其 blocker findings 即未解决集**；更早 review run 的 blocker 视为已被处理。此简化不引入新表、不改写历史。
- `integration_status` 取 `v_task_delivery_manifest` 已有列（最新 delivery run 级）。
- 判定结果作为**聚合读模型字段**返回，前端不重复计算。

---

## 五、后端设计（apps/pm-api + crates/pm-application）

### 5.1 新增：项目级聚合读端点（第一屏）

```
GET /v1/pm/acceptance-overview?project_id={uuid}
```

落点：新文件 `apps/pm-api/src/pm_acceptance.rs`，应用层逻辑 `crates/pm-application/src/acceptance_overview.rs`。

响应：

```json
{
  "project_id": "...",
  "counts": { "pending": 8, "eligible": 5, "blocked": 3 },
  "tasks": [
    {
      "task_id": "...", "title": "...", "status": "reviewing",
      "badge": "eligible | blocked | missing",
      "delivery_summary": {
        "expected_repos": 3, "delivered_repos": 2,
        "missing_repos": ["shared-contracts"],
        "integration_status": "passed"
      },
      "blocker_count": 0,
      "latest_delivery_run_id": "...", "latest_attempt_no": 2,
      "created_at": "..."
    }
  ],
  "total": 8
}
```

- 数据源：`delivery.task`（Q1 状态过滤，含 `delivery_required` 仓库关联存在性）左连 `v_task_delivery_manifest`（按 task 聚合 expected/actual、missing 仓名）+ §四的 blocker 集计算。
- 可见性：复用 `require_project_visible`（T-022 口径）。
- 每个 task 额外携带两个字段供前端解释动作区状态（**裁定**：避免前端从审批行自行推断）：
  - `review_status`：`none`（尚无 review run）/ `running`（review run queued/claimed/running）/ `awaiting_decision`（存在 pending 审批行）/ `decided`（最新审批行已 approved/rejected）。
  - `pending_approval_id`：可操作时为该 pending 行 id，否则 null。
- 排序：blocked 优先，其次 missing，eligible 靠后；同徽标按 `created_at` DESC。总表就是验收动线，**默认就按处理优先级排**。

### 5.2 扩展：run 列表补执行上下文 + run 详情端点（执行中心）

1. `pm_runs.rs` 列表 SELECT 增加 `lease_expires_at`、`executor_kind`、`executor_session`、`agent_id`、`run_kind`（列均已存在：0011:31-40 / 0020:43-48 / 0026:16；纯读扩展，不改契约既有字段）。
2. 列表端点补两个可选过滤参数（**裁定**：详情区⑤「Agent 执行情况」需要 task 级 run 列表，扩参而非新建端点）：`task_id`（按任务过滤）、`run_kind`（默认不过滤；`delivery` / `review` 分轨展示，attempt_no 自 0026 起按 run_kind 分轨编号）。
3. 新增 `GET /v1/pm/task-runs/{run_id}`：单 run 详情 = 列表字段 + `claimed_at` / `heartbeat_at` / `started_at` / `finished_at` / `deadline_at` / `release_reason` / `stage` + 按 `task_run_repository` 列出本 run 锁定的各仓基线（`base_ref` / `base_commit` / `workspace_path`）。前端用它渲染执行中心详情面板的租约 / 执行器 / 基线区。可见性兜底与 `/delivery` 同口径（run 不存在 / 跨租户 / 非成员 → 404）。
4. **日志入口（裁定见 §二非目标）**：详情响应含 `executor_session` 与各仓 `workspace_path`，页面显示为「日志定位」文本区，不提供日志内容读取。

### 5.3 新增：验收证据读端点（第二层详情）

| 端点 | 内容 | 数据源 |
| --- | --- | --- |
| `GET /v1/pm/tasks/{task_id}/review-findings` | 该任务全部 review findings（kind / severity / code_location / message / evidence_ref / created_at + 所属 review run），`severity=blocker` 置顶 | `delivery.review_finding` ⋈ `task_run`（`run_kind='review'`） |
| `GET /v1/pm/tasks/{task_id}/criterion-verdicts` | 验收标准逐条裁决：标准正文 + `verdict`（passed / failed / not_evaluable / awaiting_manual）+ `evidence` + 人工回填（decided_by/at）。**口径裁定**：`acceptance_criterion_verdict` 按 (criterion, review_run) 唯一、重审产生新行——端点返回**每标准最新一次成功 review run 的裁决**（详情表展示当前快照，历史演进不在此端点）；从未被任何成功 review run 覆盖的标准左连返回 `evaluated=false`（verdict 为 null），不造 `awaiting_manual` 假值 | `delivery.acceptance_criterion_verdict` ⋈ `delivery.task_acceptance_criterion` |
| `GET /v1/pm/tasks/{task_id}/review-approvals` | 该任务的审批历史（含 pending） | `governance.review_approval`（已有多任务级列表的 `pm_reviews.rs` list 按 project 查询；本端点为 task 级过滤投影，同一读模型，不重复建逻辑） |

多仓交付清单**复用** `GET /v1/pm/tasks/{task_id}/delivery`（`pm_delivery.rs`，零改动）。

三个端点均为只读、均过 `require_project_visible`、均 404 兜底（task 不存在 / 跨租户 / 非成员）。

### 5.4 写路径（零新增）

验收动作直接调既有 `POST /v1/pm/review-approvals/{approval_id}/decision`（`pm_reviews.rs` / `pm_application::review::decide_review_approval`）：单事务、锚定回读、E2 门复核、审计 + `DeliverySubmitted` 事件链齐全。前端按钮仅在存在 `pending` 审批行时可点；`anchor_drift` / `completion_gate_refused` 等 409 错误码原样透出为错误提示，不静默重试。

### 5.5 明确不做的后端改动

- 不新增迁移（所有列 / 表 / 视图已就位：0010 / 0011 / 0020 / 0022 / 0026）。
- 不改 `v_task_delivery_manifest` 视图定义。
- 不动 `decide_review_approval` 及状态机。

---

## 六、前端设计（frontend/src/workspace）

### 6.1 信息架构与导航

- 新页 `DeliveryPage.vue`，路由 `/workspace/:projectId/delivery`，nav「交付验收」（`aiGroups` 追加一项，icon `ClipboardCheck`）。AI 模式六项：目标计划 / AI 执行 / 决策 / 项目认知 / 产物验收 / **交付验收**。
- **产物验收（ArtifactsPage）与交付验收（DeliveryPage）职责不混**：前者 = 需求修订产物（requirement_revision），后者 = 多仓交付 + 测试证据 + 审批结论（用户裁定）。

### 6.2 页面结构（单页主从，沿用 V1.2 工作台母版）

```
PageHeader（标题 + 副标题）
┌ 三指标条：待验收 N · 可验收 N · 存在阻塞 N（点数字过滤列表）
├───────────────┬─────────────────────────────────────┐
│ 待验收列表     │ 任务交付验收详情（选中任务）            │
│ （左栏卡片）   │ ① 验收资格横幅（可验收/暂不可验收+原因）│
│ 任务标题       │ ② 多仓交付清单卡（复用 /delivery 渲染， │
│ 交付摘要       │    每仓：名称/交付态徽标/commit/分支/   │
│ 徽标          │    基线/测试证据摘要；未交付仓红标）     │
│               │ ③ 验收标准逐条裁决表（verdict 四值）    │
│               │ ④ 验收问题列表（blocker 置顶，定位+证据）│
│               │ ⑤ Agent 执行情况（按 task_run：attempt、│
│               │    agent、executor_session、状态、时间）│
│               │ ⑥ 审批历史（谁/何时/结论/理由，含pending）│
│               │ ⑦ 验收动作区（通过/退回，见 6.3）       │
└───────────────┴─────────────────────────────────────┘
```

- 徽标色语义沿用 PRISM token：可验收 `--success` / 待补充 `--warning` / 存在阻塞 `--danger`；任务态徽标与交付态徽标**分列展示**（C3 教训，task-detail-page-v1 §七）。
- 空态：项目无待验收任务（引导语「当前没有等待验收的交付」）；错误态：加载失败可重试；未选项目：提示选择。
- i18n：全部文案走 `workspace.pages.delivery.*` 命名空间，双语落 `locales/{zh-CN,en}/workspace.yaml`。

### 6.3 验收动作区

- 两个按钮：**通过验收** / **退回修改**。
- **权限口径（builder check 裁定）**：decision 端点现状仅做成员可见性校验（`require_project_visible`），全库不存在审批权限码，前端 `hasPermission` 先例仅一处且与审批无关。V1 **跟随现状不加权限码**（避免凭空造码把按钮锁死），与全库写按钮现状一致；权限码体系落位时统一补齐（登记 §九遗留）。AGENTS 规则 7 的门禁以「代码评审检查点」形式标注本例外。
- 按钮可用性由 §5.1 聚合字段驱动：仅 `review_status='awaiting_decision'` 且 `pending_approval_id` 非空时可点。
- **动作区空态语义（builder check 裁定）**：`review_status='none'` → 显示「尚未发起审查」说明；`running` → 显示「审查进行中，等待审查结论」；`decided` → 显示最新结论与时间。三种情况下按钮均不可点，前端不做任何本地推断。
- 退回必填理由（走既有 `reason` 字段）；通过理由可选。
- 提交后原位刷新详情（审批历史 + 资格横幅 + 总表计数），不整页跳转。
- 成功回执展示 `resulting_revision` 语义的提示（复用 D5 decide 返回结构）。
- **不提供 partially_approved 任何 UI 痕迹**（Q3）。

### 6.4 执行中心（ExecutionPage）增补

- 真后端分支详情面板：替换现有「detailsPending」占位，渲染 5.2 详情端点数据——租约区（`lease_expires_at` 剩余时间、`heartbeat_at`）、执行器区（`executor_kind` / `executor_session`）、基线区（各仓 base_ref/base_commit/workspace_path 定位）+ 失败摘要（已有）。
- mock 分支 fixtures 同步加租约 / executor 字段，保持两分支同构。

### 6.5 前端读模型与测试

- 新增 `src/api/acceptance.ts`（overview 类型 + 请求）、`src/api/review.ts`（findings / verdicts / approvals / decision）；`src/api/execution.ts` 扩展 `TaskRunDetail`。
- vitest（每视图 ≥1，断言行为）：总表三指标与徽标互斥渲染；「待补充」任务不被过滤（Q1 回归用例）；退回无理由被拦截；详情区按证据分段渲染；ExecutionPage 租约字段渲染。mock 数据走 fixtures 更新，不新建第二套 mock 体系。

---

## 七、数据流（验收一次决定的全链路）

```
验收人打开总表 ── GET /acceptance-overview ── 后端聚合（Q1 状态过滤 + Q2 判定）
      │ 点任务
      ▼
详情面板并发拉取 ── /delivery（复用）+ /review-findings + /criterion-verdicts
                   + /review-approvals + run 执行情况
      │ 通过/退回
      ▼
POST /review-approvals/{id}/decision（T-043 既有单事务：
行锁 → 锚定回读 → E2 门 → 审批行落定 → 任务状态迁移 → 审计 + 事件）
      │
      ▼
原位刷新（approval 历史 / 资格横幅 / 总表计数）
```

红线核对：页面不改任务状态（W1）——状态变化全部发生在 D5 事务内；前端只消费回执。

---

## 八、验收标准（对 todo T-070 的细化）

1. 真库下（T-031 产生 run、T-041 产生交付）：总表三指标数字与库内事实一致；「待补充」任务（零交付的 Submitted 任务）出现在列表。
2. 详情页六段证据（清单 / 裁决 / findings / 执行情况 / 审批历史 / 资格横幅）全部来自后端字段，前端无资格判定逻辑（代码评审检查点）。
3. 通过 / 退回走通：退回后任务到 `changes_requested`、总表实时少一个；审批历史出现不可覆盖新行。
4. 执行中心 run 详情显示租约 / executor_session / 基线；无日志内容读取。
5. `VITE_USE_MOCK=0` 全流程可跑；`vue-tsc` 0 error；vitest 全绿；i18n 双语门禁通过。

## 九、遗留登记

- **部分通过（partial approval）**：V1 明确不支持（用户裁定）。未来出现分阶段交付需求时，须与「任务拆分 / 交付范围 / 验收生命周期」一起做领域模型设计，不是 UI 追加。建议在 todo.md M7 下登记一条「保持开放」条目（同 T-051 风格）。
- **执行日志内容读取**：随 M8 / T-081 跨机暴露一并设计。
- **验收资格判定的「未解决 blocker」简化口径**（§四取最新成功 review run）：若后续 review 重审频率高，可演进为 finding 级 resolved 标记（需加列），V1 不做。
- **审批写操作权限码**：decision 端点与验收按钮 V1 均仅做成员可见性校验（全库权限码体系未覆盖审批域，§6.3）；权限码体系落位时统一补齐，属横向课题非本任务范围。

## 十、实施记录（2026-11-02 · V1 落地）

> 依据：本稿 §一～§九 全部条款；用户以「执行」过稿（状态自「待用户确认」转「已实施」）。
> 执行方式：独立 worktree `feat/t070-delivery-acceptance`（基于 master `8fe3b84`），与并行会话隔离。
> **零新增迁移**（§5.5）、**零新增写路径**（§5.4 / Q3）、**13 态状态机零改动**（Q3）。

### 10.1 文件级落地对照

| 层 | 文件 | 落地 |
| --- | --- | --- |
| 应用层 | `crates/pm-application/src/acceptance_overview.rs` | ✅ 新建。`load_facts`（一条 SQL：Q1 状态过滤 + 左连 `v_task_delivery_manifest` 聚合 + 最新 delivery run + 最新**成功** review run 的 blocker 集 + 审批状态）→ `build_overview` 纯函数：`classify_badge`（§四规则唯一实现，`blocked > eligible > missing` 互斥）、`classify_review_status`（`awaiting_decision > running > decided > none`）、按处理优先级排序（blocked → missing → eligible，组内 `created_at DESC`）、三指标汇总。9 条纯函数单测 |
| API | `apps/pm-api/src/pm_acceptance.rs` | ✅ 新建。`GET /v1/pm/acceptance-overview?project_id=`（缺参 400、不可见 404）；`GET /v1/pm/tasks/{task_id}/review-findings`（blocker 置顶，携带所属 review run）、`/criterion-verdicts`（LEFT JOIN LATERAL 取每标准**最新一次成功 review run** 的裁决，未覆盖 `evaluated=false`、`verdict=null`）、`/review-approvals`（复用 `pm_reviews::ApprovalRow` 同一读模型的 task 级投影） |
| API | `apps/pm-api/src/pm_runs.rs` | ✅ 列表补 `run_kind` / `agent_id` / `executor_kind` / `executor_session` / `lease_expires_at` + `task_id` / `run_kind` 过滤（非法值视为未传，与 `project_filter` 同口径）；新增 `GET /v1/pm/task-runs/{run_id}`：阶段时间线 + 租约 + 执行器 + `task_run_repository` 各仓基线（`workspace_path` = 日志定位文本），**不读日志内容**；404 口径与 `/delivery` 一致 |
| API | `apps/pm-api/src/support.rs` / `pm_delivery.rs` | ✅ `require_task_project` 自 `pm_delivery.rs` 上收到 `support.rs` 供三个证据端点共用（任务不存在 / 跨租户 / 非成员一律 404） |
| API | `apps/pm-api/src/routes.rs` / `lib.rs` | ✅ 注册 `pm_acceptance::router()` |
| 测试 | `apps/pm-api/tests/acceptance.rs`（3 条） | ✅ 真库种子覆盖 §四五场景：blocked(blocker) / blocked(integration failed) / missing(partial) / missing(bare, **Q1**) / eligible(awaiting_decision)；`completed` 不入集；三指标；排序；`review_status` + `pending_approval_id`；findings 置顶序；verdicts 最新成功 run 口径；审批历史；400 / 404 边界（含跨项目任务） |
| 测试 | `apps/pm-api/tests/task_runs.rs`（+2 条） | ✅ 新列可见、`run_kind` / `task_id` 过滤、非法 `run_kind` 忽略；run 详情租约 / 执行器 / 基线 + 404 |
| 前端 API | `frontend/src/api/acceptance.ts`（新）/ `review.ts`（新）/ `execution.ts`（扩） | ✅ overview + `TaskDeliveryRow`（D4 清单类型化）；findings / verdicts / approvals / `decideReviewApproval`（直打 `POST /v1/pm/review-approvals/{id}/decision`，`ReviewDecisionOutcome` 与 D5 `ApprovalOutcome` tagged union 同构）；`TaskRun` 补执行上下文列、`TaskRunDetail` + `getTaskRun`、`listTaskRuns` 增 `task_id` / `run_kind` |
| 前端页 | `frontend/src/workspace/pages/DeliveryPage.vue` | ✅ 新建。三指标条（点数字过滤）+ 左栏待验收列表 + 右栏七段详情（①资格横幅 ②多仓清单 ③逐条裁决 ④findings ⑤Agent 执行情况（delivery/review 分轨）⑥审批历史 ⑦动作区）。**mock / 真后端在数据层分叉、模板唯一**（天然同构）；徽标 / 横幅 / 按钮可用性全部消费后端字段，组件内无任何资格推导 |
| 前端页 | `frontend/src/workspace/pages/ExecutionPage.vue` + `components/RunContextPanel.vue`（新） | ✅ 替换 `detailsPending` 占位：真后端分支按选中 run 拉详情端点，渲染租约（到期 / 心跳）、执行器（kind / session）、阶段、各仓基线与日志定位；mock 分支用同一面板消费 `agentRun` 新增字段（两分支同构）；详情头增 `run_kind` 徽标 |
| 前端 | `frontend/src/workspace/acceptance/fixtures.ts`（新，由 `fixtures.ts` 统一 re-export） | ✅ 与五个端点响应**完全同构**的演示数据（5 任务覆盖全部徽标与动作区状态）；`agentRun` 增 `leaseExpiresAt / heartbeatAt / executorKind / executorSession / baselines` |
| 前端 | `nav-config.ts` / `shell/router/routes.ts` / `locales/{zh-CN,en}/workspace.yaml` | ✅ AI 模式导航第 6 项「交付验收」（`ClipboardCheck`）；路由 `/workspace/:projectId/delivery`；`workspace.nav.delivery` + `workspace.pages.delivery.*`（70 键）+ `pages.execution.*` 租约 / 执行器 / 分轨键，双语对齐；删除已无引用的 `detailsPending` |
| 前端测试 | `DeliveryPage.test.ts`（8 条）/ `pages.test.ts`（+1）/ `fixtures.test.ts`（+3）/ `nav-config.test.ts` / `WorkspaceShell.test.ts` | ✅ 三指标与徽标互斥、**「待补充」零交付任务不被过滤（Q1 回归）**、指标过滤、详情分段渲染（含部分交付「未交付」红标、`running` 态无按钮）、退回无理由被拦截且不调端点、通过 → 调用既有 decision 端点 + 回执 + 原位刷新（overview 二次拉取）、退回理由透传、`none` 态提示、失败态不伪装空态；ExecutionPage 上下文面板租约 / 会话 / 基线渲染且无日志正文区；导航 6 项与壳层标签 |

### 10.2 关键实现决策（与本稿的偏差及原因）

| # | 决策 | 原因 |
| --- | --- | --- |
| 1 | `review_status` 优先级固定为 `awaiting_decision > running > decided > none` | 存在 pending 审批行即唯一可操作态，必须压过「重审进行中」；审查曾失败但无审批行的任务按 `none` 呈现（其任务通常已不在待验收集合） |
| 2 | 最新 delivery run 直接取自 `task_run`（与视图 `latest_run` 同序），不依赖视图行 | 无任何 `task_repository` 的任务不在视图里，但 Q1 要求它们出现在总表且能带出 run 事实 |
| 3 | `eligible` 的「所有 required 仓已交付」用视图 `actual_delivery_count ≥ expected_delivery_count` 判定；`missing_repos` 按行 `delivery_required AND commit_sha IS NULL` 聚合 | 与 `/delivery` 端点同一事实源，三指标与清单卡不可能互相矛盾 |
| 4 | `DeliveryPage` 的 `useMock` 不含 `MODE === "test"`（兄弟页含） | 单测要覆盖「退回必填理由」「通过后原位刷新」等**真实交互路径**，用 api 替身而非演示分支；演示分支（`VITE_USE_MOCK=1`）走同一模板 |
| 5 | 演示数据放 `workspace/acceptance/fixtures.ts` 而非并入 `workspace/fixtures.ts` | heredoc 写入 1100 行文件在 Git Bash 下被截断（已知坑）；文件名保持 `fixtures.ts` 以命中 i18n 硬编码门禁的演示数据豁免规则（`(^|/)fixtures\.ts$`），由 `fixtures.ts` re-export 保持单一出口 |
| 6 | 路由 `meta.title: "交付验收"` 加 `// i18n-ignore` | 与同文件既有路由（`代码仓库` / `导入项目`）同款：route meta 直接作页签标题 |
| 7 | 不加审批权限码（§6.3 裁定照办） | decision 端点现状仅成员可见性校验，凭空造码会把按钮锁死；AGENTS 规则 7 的例外以代码评审检查点形式标注 |

### 10.3 验收证据（2026-11-02）

| 门禁 | 结果 |
| --- | --- |
| `cargo check --workspace --all-targets` | 退出码 0，**0 error / 0 warning** |
| `bash scripts/cargo-test.sh`（nextest 全量 + doctest；template1 指纹预建） | 退出码 0，**609 tests run / 609 passed / 0 skipped**（基线 587 + 本稿 14 + 同日 rebase 带入的 T-080 用例） |
| `pnpm typecheck`（vue-tsc） | 退出码 0 |
| `pnpm test:unit`（vitest） | 退出码 0，**89 文件 / 545 passed**（基线 88 / 533，+12） |
| `pnpm check:i18n`（命名空间对齐 + 硬编码） | ✓ 838 key 双语完全对齐；204 源文件无新增硬编码中文 |
| `pnpm check:prism-tokens` | ✓ 157 tokens 已定义，无裸色值 |
| `eslint --max-warnings 0 <本稿改动的 15 个文件>` | 0 问题（仓级 `pnpm lint` 在 master 上已有与本稿无关的 prettier 存量债，未触碰） |

对 §八 验收标准：1（三指标与库内事实一致、零交付 Submitted 任务出现）→ `acceptance.rs::overview_counts_badges_and_review_status`；2（六段证据全部来自后端字段、前端无资格判定）→ `DeliveryPage.vue` 内无 badge 推导代码 + `DeliveryPage.test.ts` 详情分段用例；3（通过 / 退回走通、审批历史新行）→ 复用 D5 单事务（`decide_review_approval` 既有用例）+ 前端提交 / 刷新用例；4（run 详情租约 / executor_session / 基线、无日志读取）→ `task_runs.rs::run_detail_returns_lease_executor_and_repo_baselines` + `pages.test.ts` 上下文面板用例；5（`VITE_USE_MOCK=0` 可跑、vue-tsc 0 error、vitest 全绿、i18n 门禁）→ 上表。

### 10.4 遗留（在 §九 基础上新增）

- **真库端到端联调**：本稿用例以 `#[sqlx::test]` 种子覆盖了 T-031 run / T-041 交付 / T-043 审批的全部事实形态，但未在 dev 栈上用浏览器走一遍完整验收动线（需 pm-api + pm-worker + 前端三者同时起且有真实 Agent 交付）；建议随下一次 E2E runbook 一并补。
- **仓级 `pnpm lint`**：master 存量 prettier 债（`workspace/spec/*`、`registry/*`、`session/*` 等 20 余文件），与本稿无关、未触碰；属横向清理课题。
- **`.gitattributes` 缺失**：仓库无行尾声明、本机全局 `autocrlf=true`，新 worktree 签出即 CRLF，sqlx 迁移校验和与 `template1` 预建不一致（`VersionMismatch`）；本次已把两处 checkout 归一为 LF 并重建 template1，但根治需 `* text=auto eol=lf`（横向课题，建议单独裁定）。
