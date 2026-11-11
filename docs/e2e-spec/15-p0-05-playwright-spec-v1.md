# AI-PM · P0-05 Task 执行与交付 · Playwright E2E 规格 V1.0

> **性质**：P0-05 套件的**可执行**自动化规格（扩编后 7 条案例：P0-015 ~ P0-021）。
> **上位文件**：`docs/e2e-spec/10-p0-test-specification-v1.md`（P0 总规格；本套件扩编使总数 24→28，见 D22）。
> **设计权威**：`docs/ai-pm-slice-d-execution-pipeline-v1.md`（D1 调度）、`docs/ai-pm-task-state-machine-v1.md`（13 态）、`docs/ai-pm-taskready-idempotency-v1.md`（幂等，已实施验证）。
> **边界**：ACP Continuation 遵守 **S03 冻结基线（D-01~D-07）**，本套件只验实现符合规范，不重定义规则。
> **基线**：master @ `2b3f509e`；迁移基线 `0001_baseline.sql` + `0011/0012`（Run 唯一性 + 事件幂等）+ `0026`（review 并发触发器）。

---

## 0. 评审结论速览（先读这节）

规格的**三类防失误目标**（状态失真 / 交付失真 / 归属错误）全部有真实机制可断言，但 8 处锚点需校正：

| # | 级别 | 规格假设 | 本仓事实 | 处置 |
|---|---|---|---|---|
| **R33** | ✅改判（2026-11-11 修订） | 「创建 Task」（直接 POST） | **直接建 Task REST 存在**：`POST /v1/pm/tasks`（`pm_lists.rs:46` → `pm_writes::create_task_handler` → `pm_write::create_task`，**`require_human_conn`** 于 `pm_write/task.rs:180`，AI 403）。Task 人类创建路径两条：① **直接 REST**（human 闸，Slice A）② 计划物化 `materialize_plan`（`pm_plans.rs:235`）。初稿「无直接建 Task REST」定性错误 —— 更正（波及 10 号同步）。**R33 安全含义反转**：负向断言 ai_pm 令牌建任务 → **403**（可测，归入 DEF-P0-01 的 REST 实证面） | P0-015 两条链均可走；负向 403 断言加入 |
| **R34** | ✅ | 防重复触发 | **三重真实机制**：mark-ready → TaskReady 事件 → `queued` Run，单事务内「任务行锁 + 活跃 Run 部分唯一索引 `uq_task_run_active` + 事件级幂等（迁移 0012，已实施验证）」；锁序 `run(FOR UPDATE SKIP LOCKED) → task(FOR UPDATE)`，INSERT 在 task 锁后（`run_dispatch/mod.rs:6-10`）；TaskReady 幂等 = 任务级 + **事件级**两项独立要求（`ai-pm-taskready-idempotency-v1.md` 〇） | P0-016 重复触发/重放断言有硬锚点 |
| **R35** | ✅ | Run 与 Task 的 claimed 是一回事 | **两个不同常量**（S4 裁决）：Run `claimed` = 已领取未启动（+`stage` ∈ `dispatch/started`）；Task `claimed` 是 13 态之一（`run_dispatch/mod.rs:25-31`）；活跃 Run 集合 = `queued/claimed/running`（B1，与唯一索引谓词一致） | 断言按双枚举分别取值 |
| **R36** | ✅ | 「创建 Delivery」 | **无独立创建 Delivery API**。交付是读面：`GET /v1/pm/tasks/{task_id}/delivery`（`pm_delivery.rs:27`）+ 项目级卷起 `delivery_state ∈ delivered/partial/missing/no_contract`（FLOW-07）——**部分交付（partial）是真实状态值** | P0-018 断言 = 读面 + DB，不虚构创建端点 |
| **R37** | 🟡 | Review 有独立审核接口与审核人字段 | 读面 = `GET /v1/pm/review-approvals`（`pm_reviews.rs:35`）；裁决路径三重防护（行锁 + 状态重读 + 0026 触发器）→ 输家 `AlreadyDecided`（`review/decide.rs:178,536`）。**R8 缺口在此兑现**：`auto_completable()` + 「无层表态 = 放行」（`policy_composition.rs:82-86`）默认允许 `reviewing→completed` 无人工 | P0-019 拆 C1（身份越权 → 拒，可断言）+ C2（auto_accept 默认放行 → **登记缺陷 DEF-P0-01**，不作 PASS 用例，D24） |
| **R38** | ✅ | 多仓库 Commit 关联 | 真实表 = `delivery.task_repository_commit` + `delivery.task_repository`；读面 = `GET /v1/pm/task-runs/{run_id}/commits`（`pm_execution.rs:45`）；多仓基线快照在 dispatch 单事务内（run_dispatch B2/B3） | P0-020 全部可落地 |
| **R39** | ✅ | 人工介入（ACP） | `awaiting_decision`（Task 13 态之一）+ `POST /v1/pm/interactions/{interaction_id}/responses`（**强制 `idempotency_key`**，幂等内建，`pm_interactions.rs:19-27`）+ 读面 `GET .../waiting`、`GET .../agents`（`pm_execution.rs:22,31`）；supervision 面 = `veto / freeze / unfreeze` 三端点（`routes.rs:213-221`） | P0-021 按 S03 冻结基线断言 |
| **R40** | ⚠️ | 「可控失败的 Agent 环境」 | 执行器 = `AI_PM_RUN_EXECUTOR` env 选择，**缺省 `NoopExecutor`**（`apps/ai-pm-worker/src/main.rs:136-143`）；真实执行器 = Codex adapter（`codex_executor.rs`，run_root 下 `instruction.md`） | 可控结果用 executor env 注入，**不依赖真实 Codex 成功率**（与规格 §三 特别注意一致）；Noop 终态语义实现时确认（D23） |

**对象关系图（规格图1 → 本仓实体）**

```
Project ─→ Task(delivery.task, 13态) ─→ Run(delivery.task_run, 7态+stage)
                                            │
                     ┌──────────────────────┼──────────────────┐
                     ▼                      ▼                  ▼
        命令/事件面(commands,events)   交付读面(delivery,   Commit 关联
                                     4 值含 partial)   (task_repository_commit)
                                            │
                                            ▼
                              Review(governance.review_approval,
                              AlreadyDecided 并发防护)
```

三个区分（规格原文 → 代码实证）：Task ≠ Run（13 态 vs 7 态，双枚举常量分离）；Run ≠ Delivery（`delivery_state` 独立判定，Run 成功 ≠ delivered）；Delivery ≠ Review（`governance.review_approval` 独立行 + `AlreadyDecided` 并发裁决）。

---

## 1. 测试目标与范围

7 条案例覆盖 Task 全生命周期（扩编自原 3 条，D22）：

| 编号 | 测试案例 | 核心验证 | 真实锚点 |
|---|---|---|---|
| P0-015 | Task 创建与项目关联 | 计划物化产物归属、隔离、审计 | `materialize_plan`（`pm_plans.rs:235`）→ `delivery.task` |
| P0-016 | Task 执行与 Run 生命周期 | mark-ready→Run、独立 Run、防重复、终态 | `mark-ready` + `uq_task_run_active` + 0012 |
| P0-017 | Agent 执行结果记录 | commands/events 面、结果归属 Run | `GET .../runs/{id}/commands`、`GET .../events` |
| P0-018 | Delivery 交付记录 | 交付读面、partial 表达、无虚假交付 | `GET /v1/pm/tasks/{id}/delivery` |
| P0-019 | Review 审核流程 | 人工裁决、AlreadyDecided、AI 不得代审 | `review/decide.rs` + 0026 触发器 |
| P0-020 | 多 Repository Commit 交付关联 | Commit 归属、双向追溯、部分完成 | `delivery.task_repository_commit` |
| P0-021 | Run 异常终止与人工介入 | 取消/超时/中断、迟到事件、ACP | `/cancel` + interactions + supervision |

**不应顺带测试（规格边界 → 本仓归属）**：需求分析（P0-03）、知识权威（P0-04）、Git OAuth / Runner 安装 / MCP 协议 / 第三方 Agent 执行质量（专项）。

---

## 2. Fixture 与执行环境

### 2.1 测试数据

| Fixture | 内容 | 说明 |
|---|---|---|
| Team A / Project A | 正常执行测试 | 独立 tid（93xx 段） |
| Project B | 跨项目隔离 | 独立 tid |
| Repository A1 / A2 | 项目 A 双仓库 | `integration.project_repository`（本地 fixture 仓库即可，D17 同款 tmp git） |
| Repository B1 | 项目 B 仓库 | 隔离对手方 |
| User Admin / Reviewer | 管理与审核 | human 主体（jwks 自签） |
| AI Agent | 项目配置的执行 Agent | bootstrap 产 `kind='ai_pm'` 主体（不手工建） |

### 2.2 四种执行环境（规格 §三 → executor env 映射）

| 环境 | 实现 | 用途 |
|---|---|---|
| 1 无外部工程工具 | 缺省 `NoopExecutor` + 无仓库 | P0-015/016C：Task 管理能力不依赖外部工具 |
| 2 可正常执行的 Agent | `AI_PM_RUN_EXECUTOR=<可控 adapter>`（fixture 用确定性 stub） | 正常 Run 主链 |
| 3 可控失败 | 同上 adapter 注入失败/超时/空结果 | P0-016B/017/021B |
| 4 多仓库 Git | 本地双仓库 fixture（git-test-helper） | P0-020 |

> **特别注意（规格原文正确）**：不以真实 Codex / Claude Code 成功率为 P0 稳定性前提；adapter 产生确定性结果，真实执行环境只保留一条集成验证（归 L4/专项）。
> **D23**：`NoopExecutor` 下 Run 的终态语义（立即 `succeeded`？停留 `dispatch`？）需在实现首日确认，P0-016C 的断言据此写。

---

## 3. P0-015 · Task 创建与项目关联（双创建链）

> **R33 修订（2026-11-11）**：直接建 Task REST 存在 —— `POST /v1/pm/tasks`（`pm_lists.rs:46` → `pm_write::create_task`，`require_human_conn` 于 `pm_write/task.rs:180`）。本节主链保留计划物化（Flow 覆盖 Requirement→Plan→Task 全链）；直接 REST 作为替代链可互换，**负向断言共用**：ai_pm 令牌调 `POST /v1/pm/tasks` → **403**（DEF-P0-01 的 REST 实证面）。

| Step | 操作 | 预期结果 | 真实锚点 |
|---|---|---|---|
| 1 | 创建计划并审批通过 | 计划 `draft→in_review→approved` | `task-plans` 路由族（`pm_plans.rs:309-330`） |
| 2 | 物化任务 | `delivery.task(draft)` 生成，归属 Project A | `materialize_plan`（`pm_plans.rs:235,242`） |
| 2' | （替代链）直接创建 | 201；**AI 身份 → 403** | `POST /v1/pm/tasks`（`pm_lists.rs:46` + `pm_write/task.rs:180`） |
| 3 | 读取 Task 详情 | 标题/描述/验收标准完整 | `GET /v1/pm/tasks/{task_id}`（`pm_tasks.rs:20`） |
| 4 | 检查引用关系 | 引用 Requirement/知识对象为引用而非副本 | `delivery.task_requirement` / `delivery.task_knowledge_ref` 关联表 |
| 5 | 刷新页面 | 数据不丢 | 详情 API 重读 |
| 6 | Project B 主体读 Task A | **404** | `require_project_visible`（12 号 §6.3 口径） |
| 7 | Project B 主体写 Task A | 拒绝且数据不变 | 同上 |

**异常测试**：缺必填字段（计划校验门禁 400，FLOW-06）；Project 不存在 / 无权限（404）；重复物化（幂等/冲突语义以实现为准，实现时确认）。

**隔离要求（规格原文）**：跨项目读写由后端阻止 —— 已实证（`support.rs:178-208`），断言写死 404。

---

## 4. P0-016 · Task 执行与 Run 生命周期

### 场景 A · 正常启动 Run

| Step | 操作 | 预期结果 | 真实锚点 |
|---|---|---|---|
| 1 | 物化可执行 Task | `draft` | 计划链 |
| 2 | mark-ready | Task → `ready`；**「AI 永不置 ready」**（`require_human` 复用，`requirement.rs` 注释 E2） | `POST /v1/pm/projects/{pid}/tasks/mark-ready`（`pm_lists.rs:54`） |
| 3 | TaskReady 事件 → Run 创建 | `delivery.task_run(queued)`，单事务 | `TaskReadyHandler` + `uq_task_run_active` + 0012 |
| 4 | 领取/执行 | `queued→claimed→running`（Run）；Task `ready→claimed→running`（两套常量） | `run_dispatch`（SKIP LOCKED 锁序） |
| 5 | 读取 Run 详情 | 关联正确 task_id / project_id | `GET /v1/pm/task-runs/{run_id}`（`pm_runs.rs:37`） |
| 6 | 终态 | 7 值之一；终态无出边 | `run_status::*` + `task_transition.rs:32` |
| 7 | Task 展示与 Run 一致 | 活跃 Run 集合/终态对齐 | `ACTIVE_RUN_STATUSES` |

**业务规则 1 · 独立 Run（规格图2 → 断言）**：同一 Task 再次 mark-ready → 新 Run（不覆盖）；Run ID 唯一；历史 Run 可读；各自起止时间/结果独立；执行记录不串 Run。

**业务规则 2 · 防重复触发（R34 硬锚点）**：
- 连续触发/重试 → **活跃 Run 唯一索引**拦截（同一 Task 不能同时两条活跃 Run）；
- 旧事件 replay / 崩溃恢复 → **0012 事件级幂等**（`23505` 冲突降级为幂等返回）；
- 并发策略不预设 —— 按已实现语义断言（活跃唯一 + 幂等返回原 Run）。

```ts
test("P0-016 每次 mark-ready 产生独立 Run，重复事件幂等", async () => {
  const t = await materializedTask(a, projectA.id);   // R33：计划物化链
  await markReady(a, projectA.id, t.id);              // → Run#1 queued
  const run1 = await waitForRun(a, t.id);

  await markReady(a, projectA.id, t.id);              // 二次触发
  // 活跃唯一或幂等返回原 Run（按已实现语义断言）
  const runs = await api("GET", `/v1/pm/task-runs?task_id=${t.id}`, a.token);
  const active = runs.body.items.filter(r =>
    ["queued", "claimed", "running"].includes(r.status));
  expect(active.length).toBeLessThanOrEqual(1);       // uq_task_run_active

  await waitForTerminal(a, run1.id);                  // 终态后再次执行
  await markReady(a, projectA.id, t.id);
  const run2 = await waitForRun(a, t.id);
  expect(run2.id).not.toBe(run1.id);                  // 新 Run 独立
  const old = await api("GET", `/v1/pm/task-runs/${run1.id}`, a.token);
  expect(old.status).toBe(200);                       // 历史 Run 不被覆盖
});
```

### 场景 B · Run 执行失败

失败 Run 进 `failed`（或 `timed_out`）；Task **不得**显示为已成功交付；错误与上下文可读（events 面）；失败不产生虚假成功 Delivery；重试 = 新 Run 独立创建。锚点：终态无出边 + delivery 读面独立判定（R36）。

### 场景 C · 无外部工程工具时的 Task 能力（R40）

无 Git / Runner / MCP / Codex：Task 可创建、查看、管理（P0-015 全链不依赖外部工具）；AI-PM 任务分析能力可用（P0-03 已证分析链有 FakeModel 回退）；系统区分「管理分析完成」与「工程执行完成」（`stage` ∈ dispatch/started 是显式字段）；需要外部执行而不可用时 → 明确失败/等待，**不伪造成功**（断言 Run 终态 ≠ succeeded 或带失败码，语义按 D23 确认后写死）。

---

## 5. P0-017 · Agent 执行结果记录

**读取面**：`GET /v1/pm/task-runs/{run_id}/commands`（命令账）+ `GET .../events`（事件流，`pm_runs.rs:38`）+ `GET .../commits`（`pm_execution.rs:45`）。

**结构化结果示例（规格 JSON 是测试数据非协议 —— 本仓契约 = 命令账 + 事件 + 审计）**：

| 检查项 | 预期 | 锚点 |
|---|---|---|
| Run 关联 | commands/events 行属于当前 run_id | 路由即按 run_id 过滤 |
| Task/Project 关联 | Run → Task → Project 链完整 | `delivery.task_run.task_id` |
| Agent 来源 | 执行主体可识别 | `core.principal`（执行器身份）+ 审计 actor |
| 执行时间 | 起止完整 | Run `started_at/finished_at` + `stage` |
| 错误信息 | 失败保留上下文 | events + 失败码 |
| **报告 ≠ 证据（规格强调）** | Agent 自述与执行器实测分离 | 命令账（executor 实际执行的命令）vs 事件声明；测试结果断言区分「未执行 / 通过 / 失败」三态，来源字段实现时按 events schema 确认（D25） |

**异常覆盖**（7 项规格 → 断言口径）：空结果 / 不合协议 / 超时中断 → Run 进失败终态且 events 留痕；部分产出 + 失败状态 → 产出保留但不计成功交付；重复提交结果 → 事件幂等；跨 Run / 跨 Project 提交 → 拒绝（run_id/project 归属校验）。

---

## 6. P0-018 · Delivery 交付记录

**真实读面**：`GET /v1/pm/tasks/{task_id}/delivery`（任务级 `items/total` 自洽，FLOW-07 已证）；项目级卷起 `delivery_state ∈ delivered / partial / missing / no_contract`。

### 场景 A · 正常交付

| 断言 | 锚点 |
|---|---|
| 交付可追溯 Run 与 Task | delivery items 带 run/task 引用（字段名实现时确认） |
| 摘要与执行结果一致 | 读面 vs Run events 双证据 |
| 不同 Run 的交付不串联/覆盖 | 按 run 维度分组断言 |

### 场景 B · 执行成功但无有效产出

**核心断言**：Run `succeeded` **不能**直接推出 `delivered` —— `delivery_state` 按实际产出判定（`missing` / `no_contract` 是合法值）；不得自动将 Task 标记为完成交付。判断依据 = Task 验收标准，非「是否有 commit」（分析报告亦可交付 —— 与本仓 `no_contract`/partial 语义一致）。

### 场景 C · 部分交付

`partial` 是**真实状态值**（R36）：已完成产出保留；未完成部分不伪装；后续 Run 继续处理剩余工作；历史交付记录不覆盖（按 run 分组各自保留）。

---

## 7. P0-019 · Review 审核流程

**读面**：`GET /v1/pm/review-approvals`（`pm_reviews.rs:35`）。**裁决路径**：`review/decide.rs`（`AlreadyDecided` :178,536）+ 三重并发防护（行锁 + 锁后状态重读 + DB 触发器 0026:273-296）。

### 场景 A · 人工审核通过

| Step | 断言 | 锚点 |
|---|---|---|
| 进入 Review | 审核人可查看交付与验收标准 | 读面 + task 验收字段 |
| 人工确认通过 | 结论落 `governance.review_approval` | decide 路径 |
| Task 状态按规则更新 | `reviewing→completed`（人工路径） | `task_transition` |
| 审计 | 审核人 = 真实 human 主体 + 时间 | `governance.audit_log` |

### 场景 B · 人工拒绝

拒绝原因/反馈按正式字段契约保存（实现时确认字段名）；结论独立记录；交付历史不覆盖；Task 不得显示最终验收通过；重新交付后历史审核仍可读。

### 场景 C · AI 不得伪造人工 Review（拆 C1/C2，R37）

**C1（可断言，PASS 用例）**：
1. `kind='ai_pm'` 主体令牌直调审核裁决接口 → **拒绝**（预期 403，状态码实现时确认）；
2. 请求参数伪造 `reviewer_id` = 人类用户 → 服务端以**主体身份**（token 解析 + DB kind）判定，不信参数 → 拒绝；
3. 普通成员（非 Lead/Approver）越权审核 → `ensure_governance_authorized` 拒绝；
4. 审核通过必须留真实人类身份 + 审计。
**不能只验前端按钮隐藏 —— 必须直调后端**（与 13 号 §5.3 同方法论）。

**C2（登记缺陷，不作 PASS 用例，D24）**：`auto_completable()` + 「无任何策略层表态 = 放行」（`policy_composition.rs:82-86`、`ai_team_policies.rs:17-18`）默认允许 `reviewing→completed` 无人工 —— **登记 DEF-P0-01**（S1 级：审核边界缺口），修复后 C2 转正为回归用例。

### 场景 D · 并发审核

User A 通过 / User B 基于旧状态拒绝 → 输家收 `AlreadyDecided`（409 族），**不静默覆盖**；审计反映真实顺序 —— 三重防护已实证（decide.rs + 0026 触发器），断言直接复用。

---

## 8. P0-020 · 多 Repository Commit 交付关联

**真实表**：`delivery.task_repository`（Task↔Repository）+ `delivery.task_repository_commit`（交付↔Commit）；**多仓基线快照**在 dispatch 单事务内（run_dispatch B2/B3）；读面 = `GET /v1/pm/task-runs/{run_id}/commits`。

### 场景 A/B · 单仓与多仓

| 检查项 | 断言 | 方式 |
|---|---|---|
| Commit A1 ∈ Repository A1 | SHA 真实存在于该仓库 | `git -C A1 cat-file -e <sha>`（绕过应用） |
| Commit A2 ∈ Repository A2 | 同上 | `git -C A2` |
| 关联行正确 | `task_repository_commit` 外键三元组（task/commit/repo）无错位 | psql 直查 |
| 双向追溯 | Task → 全部 Commit；Commit → 交付 → Task | API 双向 + DB |
| 重复关联 | 幂等或唯一约束（实现时确认唯一键，D26） | 重复提交同一 sha |

### 异常 8 项（规格 → 口径）

SHA 不存在 / 跨仓 Commit / 仓不属于本 Project / 重复关联 / 两仓错位 / Commit 串到其他 Task / Provider 信息与本地不一致 → 全部由**归属校验 + 真实 Git 验证**拒绝；最后一项（一仓成功一仓失败）→ **`partial` 表达**（R36），断言整体不得标记全部成功。

> 规格注意事项（正确）：按现有 schema 验证实际外键 / 唯一约束 / 生命周期，不信 UI 显示 —— 本仓唯一索引与外键在 0001 基线 + 后续迁移，实现时列出 `task_repository_commit` 约束清单（D26）。

---

## 9. P0-021 · Run 异常终止与人工介入（ACP）

> **遵守 S03 冻结基线（D-01~D-07）**：不重定义 Continuation 规则，只验实现符合。

### 场景 A · 执行中取消

| 断言 | 锚点 |
|---|---|
| 取消有权限控制 | `POST /v1/pm/task-runs/{run_id}/cancel`（`project_lifecycle.rs:33`，P2 外部入口） |
| Run 进终态 `cancelled` | `run_status::CANCELLED`；终态无出边 |
| 取消原因与时间可追溯 | events + 审计 |
| Task 不误显示成功交付 | delivery 读面独立（R36） |
| **迟到 Agent 回调不得复活终态 Run** | 终态无出边（`task_transition`/run 状态机）+ 事件幂等（0012）+ supervision veto（`/api/v1/supervision/veto`，`routes.rs:213-221`）—— 断言口径 = 状态机不变量，而非专门 guard 模块（D25） |
| 重复取消 | 幂等/409（以实现为准，实现时写死） |
| 取消 × 交付竞态无虚假成功 | 取消后 delivery 断言仍非 delivered |

### 场景 B · 执行中断

Agent 断连 / Runner 异常 / 超时事件 → Run 进 `failed` / `timed_out`（7 态含两值）；不永久停留中间态（`stage` + 租约/超时推进，机制实现时确认）；错误上下文保留；无伪造 Delivery；可按正式规则重试（新 Run）。

### 场景 C · 人工介入后继续（ACP 重点）

| Step | 断言 | 锚点 |
|---|---|---|
| 1-3 | 执行中出现需人工决策问题 → Task `awaiting_decision`、介入记录落库 | 13 态 + `governance.human_intervention` + `delivery.task_interaction` |
| 4 | 人工提交决策 | `POST /v1/pm/interactions/{interaction_id}/responses`，body `{response_payload, idempotency_key}`（**幂等强制**） |
| 5 | Task-Based Continuation 继续 | 按 S03 冻结规则（不重定义） |
| 6-7 | 继续执行关联原 Task/历史上下文；全程可追溯 | Run 链 + events |

**重复提交 / 重复恢复 / 迟到事件** → `idempotency_key` 幂等 + 状态机终态不变量，不破坏历史。

---

## 10. 三层验证与数据库关系（规格 §十二 → 本仓落点）

| 层 | 对象 | 检查 |
|---|---|---|
| UI | Playwright | 创建（物化）/mark-ready/查看交付/审核 |
| API/权限 | 直连 pm-api | 状态转换、越权 404/403、幂等、AlreadyDecided |
| DB/Git | psql + git | 行存在、外键三元组、历史未覆盖、SHA 真实存在 |

| 业务对象 | 表 | 必查 |
|---|---|---|
| Task | `delivery.task` | project 归属、创建者、13 态 |
| Run | `delivery.task_run` | task 归属、7 态、stage、时间、`uq_task_run_active` |
| 命令/事件 | 命令账 + events | Agent 来源、Run 归属 |
| Delivery | 读面 + 卷起 | 4 值 delivery_state、partial 表达 |
| Review | `governance.review_approval` | 交付关联、审核人、结论、AlreadyDecided |
| Commit | `delivery.task_repository(_commit)` | 三元组、唯一约束、SHA 真实性 |
| 审计/事件 | `governance.audit_log` / outbox | 状态变化、人工操作、越权拒绝 |

**跨对象一致性断言（规格示例 → 真实字段）**：

```ts
async function assertTaskExecutionIntegrity(taskId: string) {
  const runs = await listTaskRuns(a, taskId);
  for (const r of runs) {
    expect(r.task_id).toBe(taskId);                    // 真实字段名
    if (isTerminal(r.status)) expect(r.finished_at).toBeTruthy();
    expect(ACTIVE_RUN_EXACTLY_ONE_OR_ZERO(runs)).toBe(true); // 活跃唯一
  }
  const delivery = await api("GET", `/v1/pm/tasks/${taskId}/delivery`, a.token);
  for (const item of delivery.body.items) {
    expect(item).toHaveProperty("delivery_state");     // ∈ 4 值
    expect(["delivered","partial","missing","no_contract"]).toContain(item.delivery_state);
  }
  // 每个 delivery 项可追溯到具体 Run（字段实现时确认，不假定必为 run_id 直挂）
}
```

---

## 11. 最终验收清单（26 项，沿用规格原文编号）

P0-015：Task 创建成功关联 Project ／ 跨项目读写后端隔离 ／ 内容引用审计正确
P0-016：独立 Run ／ 状态时间终态正确 ／ 重复触发并发符合已实现策略（活跃唯一 + 0012）／ 无外部工具基础能力可用 ／ 执行条件缺失不伪造成功
P0-017：结果关联正确 Task/Run/Project ／ 失败超时空结果正确处理 ／ Agent 报告与真实证据区分
P0-018：Delivery 真实可追溯来源 Run ／ 无产出不伪造完整交付 ／ partial 与多次交付历史保留
P0-019：人工正常审核 ／ 拒绝及反馈记录 ／ **AI 无法伪造人工审核（C1）** ／ 并发审核无静默覆盖（AlreadyDecided）
P0-020：Commit 与正确 Repository 关联 ／ 多仓 Commit 正确关联同一 Task 交付 ／ 无效/跨项目关联被拒 ／ 部分仓库失败正确表达（partial）
P0-021：取消失败超时正确终止 ／ 迟到事件不复活终态 Run ／ Continuation 符合冻结 ACP 规范 ／ 重复恢复不破坏历史

> 外加 **DEF-P0-01 登记在案**（C2：auto_accept 默认放行缺口，S1）—— 修复前 P0-019C2 不得标 PASS。

---

## 12. 实施顺序与红线

**顺序（规格 §十四 → 采纳）**：015 → 016 → 017 → 018 → 019（第一轮主链）；020 → 021（第二轮专项）；完整验收不含略后两项。

**五条红线 → 代码锚点**

| # | 红线 | 锚点 |
|---|---|---|
| 1 | 不混淆 Task 与 Run | 双枚举常量分离（S4，`run_dispatch/mod.rs:25-31`） |
| 2 | 不混淆 Run 成功与 Delivery 成功 | `delivery_state` 4 值独立判定（FLOW-07） |
| 3 | 不混淆 Agent 报告与真实证据 | 命令账（executor 实测）vs 事件声明；commit 用 git cat-file 验真 |
| 4 | 不允许 AI 冒充人工 Review | 主体 kind 判库 + `ensure_governance_authorized`；C2 缺口已登记 DEF-P0-01 |
| 5 | 不丢失多仓关系与执行历史 | `task_repository_commit` 外键 + Git 直验 + 终态无出边 + 0012 幂等 |

---

## 13. 待裁决项

| # | 议题 | 建议 |
|---|---|---|
| **D22** | 案例编号扩编 | P0-05 扩为 7 例（015~021）；原 P0-06（ACP 3 例）→ **P0-022~024**；原 P0-07（安全 4 例）→ **P0-025~028**；总数 24→**28**，门禁 **28/28 PASS**（10 号文档同步）。**【D27 修订】**：ACP 撤销独立套件（内容 = 本套件 P0-021 场景 C）；新 P0-06 = 外部 Agent/MCP **040~048**；P0-07 顺延 **049~052**；总数 **34**，门禁 **34/34**（见 `16-p0-06` §15） |
| **D23** | `NoopExecutor` 终态语义 | 实现首日确认（succeeded / 停留 dispatch / failed），P0-016C 断言据此写死 |
| **D24** | C2 缺口处置 | 登记 **DEF-P0-01**（`auto_completable` 默认放行，S1），修复后转正回归用例 |
| **D25** | 迟到事件与报告证据口径 | 断言 = 状态机不变量（终态无出边）+ 事件幂等（0012）；「Agent 报告 vs 执行器实测」以命令账 + events schema 为准（实现时确认字段） |
| **D26** | `task_repository_commit` 约束清单 | 实现时列出外键 / 唯一键 / 生命周期，写进用例断言 |

---

## 14. 关联文档

| 文件 | 内容 |
|---|---|
| `docs/e2e-spec/10-p0-test-specification-v1.md` | P0 总规格（编号扩编 D22 落点） |
| `docs/e2e-spec/13-p0-03-playwright-spec-v1.md` | 人工确认三道闸方法论（P0-019C1 复用） |
| `docs/ai-pm-slice-d-execution-pipeline-v1.md` / `ai-pm-task-state-machine-v1.md` | D1 调度与 13 态设计权威 |
| `docs/ai-pm-taskready-idempotency-v1.md` | Run 创建幂等（任务级 + 事件级，已实施验证） |
| `crates/pm-application/src/run_dispatch/mod.rs` | Run 调度（锁序 / 唯一索引 / stage） |
| `crates/pm-application/src/review/decide.rs` | Review 裁决（AlreadyDecided / 三重防护 / auto_completable 缺口） |
| `apps/pm-api/src/pm_plans.rs` / `pm_lists.rs` / `project_lifecycle.rs` | 物化 / mark-ready / cancel 入口 |
