# S01 · I5 项目总控台 · 读模型契约（I5-B）

> 依据：`docs/s01-全局主菜单与信息架构设计基线/ui/smark-pm-s01-i5-project-command-center-v1-1.md`（V1.1 定稿）§3/§4/§5/§6/§7/§8
> 事实源：`docs/s01-全局主菜单与信息架构设计基线/docs/s01-i5-code-facts.md`（I5-A 核验，缺口编号 `I5-G-xx`）
> 状态：**定稿**（2026-11-04），I5-C 实现依据
> 原则：**聚合不拥有业务状态**；每个展示值必须能追溯到权威领域数据（设计稿 §6.4）

---

## 1. 端点形态裁定

**裁定：单一聚合端点** `GET /v1/pm/projects/{project_id}/command-center`

理由：

1. 首屏需要「待人工处理 → 当前执行 → 风险」的整体视图，拆成 6 次请求会打乱优先级语义；
2. 六块的数据源彼此独立、任一失败不阻塞其余（§7 部分失败），单端点内按块降级即可；
3. 已有可复用的项目级聚合先例：`GET /v1/pm/projects/{id}/delivery`（S01-W60）——同一项目级读模型范式。

分页只对「最近活动」块开放：`?activity_limit=&activity_offset=`（默认 20 / 上限 100）。
其余块为「计数 + 有限明细」，明细默认最多 N 条（见 §4 各块），**不提供分页**（I5-031 只约束活动流）。

## 2. 响应骨架

```jsonc
{
  "project_id": "…",
  "as_of": "2026-11-04T09:00:00Z",     // 聚合生成时刻（各块另有自己的 as_of）
  "blocks": {
    "project_summary":        { "status": "ok", "as_of": "…", "data": { … } },
    "initialization_progress":{ "status": "ok", "as_of": "…", "data": { … } },
    "work_summary":           { "status": "ok", "as_of": "…", "data": { … } },
    "human_action_summary":   { "status": "ok", "as_of": "…", "data": { … } },
    "risk_summary":           { "status": "ok", "as_of": "…", "data": { … } },
    "recent_activity":        { "status": "ok", "as_of": "…", "data": { … } }
  }
}
```

**块状态位**（`status`，设计稿 §7.5）：

| 值 | 含义 | 展示要求 |
|---|---|---|
| `ok` | 数据源可用 | 正常展示 |
| `unavailable` | 该块数据源查询失败 | **显示「数据暂不可用」+ 错误摘要 + 重试入口**；**不得**显示 0 或空列表冒充「没有事项」 |

`unavailable` 的块同时带 `error: { "code": "…", "message": "…" }`（不含 SQL/内部细节）。

## 3. 通用约束

| # | 约束 | 依据 |
|---|---|---|
| C-1 | **权限前置**：进入聚合前先做项目可见性校验（`tenant_id` + `core.project_member`），不可见一律 404（不泄露存在性） | 设计稿 §7.6；`apps/pm-api/src/support.rs:126-157` |
| C-2 | **读模型层自带 tenant 谓词**：本聚合的所有 SQL **必须**带 `tenant_id` 谓词，不得直接复用只按 `project_id` 过滤的既有函数（如 `delivery::list_project_delivery`、`knowledge::list_documents`）而不补租户条件 | I5-A §12.3/§12.5（`I5-G-13`） |
| C-3 | **每条记录可证属本项目**：聚合出的每条明细必须带 `project_id` 语义来源（来源表主键或过滤条件），不得跨项目拼接 | 设计稿 §7.6-3 |
| C-4 | **不写业务状态**：聚合是纯读；写操作仍走原领域端点 | 设计稿 §6.5/§7.1 |
| C-5 | **展示值必可追溯**：每个指标附 `source`（权威来源表/函数）；禁止由其它展示值推断 | 设计稿 §6.4 |
| C-6 | **原始状态保留**：跨域展示必须保留来源领域原始状态字符串，展示层严重度只是附加映射 | 设计稿 §3.5 |
| C-7 | **无百分比**：初始化进度不产出完成百分比 | 设计稿 §4.8 |
| C-8 | **归档只读**：Project `archived` 时，头部表达只读态，**不提供**新活动入口（写入口本身已由 I1/I4-F 关闭） | 设计稿 §8.6；I5-019/036 |

---

## 4. 数据块契约

### 4.1 `project_summary`（项目头部）

| 字段 | 权威来源 | 说明 |
|---|---|---|
| `project_id` / `code` / `name` | `core.project` | — |
| `lifecycle` | `core.project.status` | 原样透传（`planning`/`initializing`/`archived`）；**不新增枚举、不推断**（I5-A §1.6） |
| `archived` | `core.project.status = 'archived'` | 权威来源：Project 生命周期（禁止由任务数推断） |
| `customer_name` | `core.customer` | 可空 |
| `goal` | I4 `project_goal::load_official_goal` | `{ "present": bool, "revision_no": int?, "published_at": timestamp?, "change_summary": string? }`；**不含正文**（正文在知识仓，需另行读取） |
| `goal_pending_proposal` | `ai.document_proposal`（`doc_key='project-goal'` 且状态非终态） | 布尔：`true` 表示「有待人工确认的 Goal 修改」 |
| `repository_count` | `integration.project_repository` | 计数 |
| `verified_repository_count` | 同上 `status='verified'` | 计数 |
| `member_count` | `core.project_member` | 计数 |
| `last_activity_at` | `governance.audit_log`（本项目白名单 action 的最大 `created_at`） | 无记录 = `null` |
| `as_of` | 本块查询时刻 | — |

> **Goal 状态口径裁定（I5-G-14）**：当前**不存在**「Goal 是否确认」的领域状态字段（I5-A §6.5）。
> 因此本块**不输出** `goal.confirmed` 之类臆造字段，只表达两个事实：
> `goal.present`（是否存在正式 published 修订）与 `goal.pending_proposal`（是否有待确认修改）。
> 设计稿 §6.4「不允许根据文档是否存在推断 Goal 是否确认」在**读模型层**通过
> **不下结论**来满足：页面把这两个事实分别展示，由人判断。

### 4.2 `initialization_progress`（初始化进度）

**七个事项分组**（设计稿 §4.3）与**展示层状态**（§4.4：未开始 / 进行中 / 已完成 / 存在阻塞 / 不适用 / 尚无法判断）。

> `status` 是**展示层状态**，不是领域状态；每个事项的 `source_state` 保留领域原始状态。

| 事项 | 判定规则（可执行） | 权威来源 | `required` |
|---|---|---|---|
| `project_basics` 项目基本信息 | 恒「已完成」（`core.project` 存在即成立） | `core.project` | true |
| `project_goal` 项目目标 | 正式修订存在 → 「已完成」；无正式修订但有待确认提案 → 「进行中」；否则 → 「未开始」 | I4 Goal + `ai.document_proposal` | true |
| `repository_access` 仓库接入 | 无仓库 → 「未开始」（**零仓库合法**，不判失败）；有仓库且全部 `verified` → 「已完成」；存在 `unreachable` → 「存在阻塞」；存在 `pending` → 「进行中」 | `project_repository.status` | **false**（零仓库合法，见下方裁定） |
| `code_analysis` 代码分析 | 无仓库 → 「不适用」；有在途导入作业 → 「进行中」；最新有效结果失败且无更新的成功 → 「存在阻塞」；存在 `draft_ready`/`applied` → 「已完成」；否则 → 「未开始」 | `repo_import.status` + `repo_import_source_attempt` | **false**（依赖仓库接入） |
| `project_cognition` 项目认知 | 无 session → 「未开始」；有 session 且状态未终结 → 「进行中」；全部终结 → 「已完成」 | `discovery.discovery_session` | **false** |
| `dev_environment` 开发环境 | 无绑定设备 → 「未开始」；存在设备 `readiness='ready'` → 「已完成」；存在 `degraded`/`failed` → 「存在阻塞」；仅 `installed/connected/authenticated` → 「进行中」；**从未自检（readiness IS NULL）** → 「尚无法判断」 | `devenv.device` + `device_project_binding` | **false** |
| `human_actions` 人工处理 | 无待办 → 「已完成」；有待办 → 「进行中」 | 见 §4.4 各域 | true |

**必需项裁定（设计稿 §4.5）**：当前**没有**项目类型或初始化要求配置（I5-A 无此配置源）。
因此：

- `required = true` 的只有 `project_basics` / `project_goal` / `human_actions` 三项；
- **不得**擅自把仓库、环境、认知定义为必需项；
- 由于必需项集合无法表达「全部初始化事项」，本块**不输出**「初始化已完成」结论，
  只输出 `completion: "not_determinable"`（设计稿 §4.5 末句的「展示项目接入概况，不宣称已完成」），
  并附 `required_items` 明细供人工判断 → 对应 I5-029。

**收缩规则（§4.6）**：本块输出 `expanded: bool`——
存在必需事项「进行中/未开始」或任一事项「存在阻塞」→ `expanded = true`；否则 `false`（简要态）。
**收缩不得隐藏阻塞或人工处理事项**（阻塞仍出现在 `risk_summary`，人工处理仍出现在 `human_action_summary`）。

### 4.3 `work_summary`（Task / Run 汇总）

| 字段 | 权威来源 |
|---|---|
| `task_total` | `delivery.task`（project + tenant） |
| `task_by_status` | `GROUP BY status`（13 值域，`I5-G-06`；索引 `idx_task_project_status` 已在） |
| `active_run_count` | `delivery.task_run` status ∈ `ACTIVE_RUN_STATUSES`（`queued/claimed/running`） |
| `active_runs[]` | 见下 |
| `recent_runs[]` | 最近 N=5 条历史 Run（**不覆盖历史**，展示 `attempt_no` 与 `run_kind`） |
| `delivery_summary` | 复用 S01-W60 四态计数：`delivered/partial/missing/no_contract` |

**`active_runs[]` 每条**（I5-G-07）：

| 字段 | 来源 |
|---|---|
| `run_id` / `task_id` / `task_title` / `run_kind` / `attempt_no` / `status` / `stage` | `task_run` ⋈ `task` |
| `agent` | `task.assignee_id` → `core.principal(display_name, kind)`——**权威是 assignee**；`task_run.agent_id` 恒 NULL，**禁止使用**（I5-A §9.3/§9.6） |
| `claimed_at` / `started_at` / `heartbeat_at` / `lease_expires_at` | `task_run` |
| `repositories[]` | `delivery.task_run_repository`（本次领取的基线快照） |
| `last_change_at` | `task_run.status` 最近变更时间（无变更列 → 用 `claimed_at`/`heartbeat_at`，并注明口径） |

**展示区分（设计稿 §3.4）**：历史 Run 与当前有效 Run **分列**；重跑以新的 `attempt_no` 追加，不覆盖。

### 4.4 `human_action_summary`（待人工处理，首屏第一优先级）

**合并四类来源**（`I5-G-09`），每条统一形状：

```jsonc
{
  "kind": "finding_pending" | "finding_revision_requested" | "proposal_pending" | "delivery_pending" | "decision_pending",
  "domain": "discovery" | "knowledge" | "delivery" | "approval",
  "id": "…",                    // 来源记录 ID
  "project_id": "…",           // C-3 可证归属
  "title": "…",
  "detail": "…",
  "source_state": "PENDING",   // 来源领域原始状态（C-6）
  "severity": "…",             // 展示层映射（可选）
  "updated_at": "…",
  "action_hint": "…",          // 需要人做什么
  "link": { "kind": "…", "params": { … } }   // 下钻参数（前端拼路由）
}
```

| 来源 | 口径 | 权威来源 | 缺口 |
|---|---|---|---|
| `finding_pending` | `display_state = 'PENDING'`（待人工确认） | `derive_display_state`（`discovery/finding.rs:89-103`） | 需 project 级查询（`I5-G-08`） |
| `finding_revision_requested` | `display_state = 'REVISION_REQUESTED'`（待补充） | 同上（由最近决定审计 `discovery_finding_revision_requested` 推导，**不得**从 open question 推导） | 同上 |
| `proposal_pending` | `ai.document_proposal.status` ∈ `ACTIVE_PROPOSAL_STATUSES`（5 值） | `review/knowledge.rs:18-24` | 需按 `doc_key` 过滤（首提案 `document_id` 为 NULL） |
| `delivery_pending` | Task `status ∈ PENDING_TASK_STATUSES`（`submitted/reviewing`） | `acceptance_overview.rs:15` | 复用 `load_facts` 口径 |
| `decision_pending` | 待裁决事项 | **本仓无独立「待决策」实体** → **第一版不输出该类** | — |

**排序（稳定）**：`kind` 优先级（`finding_pending` → `finding_revision_requested` → `proposal_pending` → `delivery_pending`）
→ `updated_at DESC` → `id ASC`（唯一 tiebreaker，保证连续分页稳定）。

**去重键**：`{domain}:{kind}:{id}`——同一来源记录只出现一次；不同来源的同一业务问题**不跨域合并**
（不同域的原始状态必须分开显示，I5-035）。

**上限**：默认返回最多 50 条，`total` 为各域真实计数（不是截断后的长度）。

### 4.5 `risk_summary`（风险与阻塞）

每条：

| 字段 | 说明 |
|---|---|
| `kind` | `run_failed` / `task_blocked` / `repository_unreachable` / `analysis_failed` / `delivery_exception` / `archive_cleanup` |
| `domain` / `source_id` / `source_state` / `occurred_at` | 来源与原始状态（C-6） |
| `severity` | 展示层映射：`high` / `medium` / `low`（映射表见下） |
| `title` / `suggestion` / `link` | 展示与入口 |

**严重度映射**（设计稿 §3.5「可定义展示层映射，但必须保留原始状态」）：

| kind | severity 判定 |
|---|---|
| `run_failed` | `failed`/`timed_out` → `high` |
| `task_blocked` | `status='blocked'` 或存在 `task_waiting`（`needs_info/needs_decision/needs_approval/external_dependency`）→ `medium` |
| `repository_unreachable` | `status='unreachable'` → `medium`；`last_error_code='auth_failed'` → `high` |
| `analysis_failed` | 最新 attempt `failed` 且无更新的成功 → `high` |
| `delivery_exception` | 视图 `delivery_state='never_submitted'` 且 Task 处于待验收 → `medium` |
| `archive_cleanup` | Project `archived` 且存在残留活跃 Run（`claimed/running`）→ `medium`（I1 P3 事实） |

**去重键**：`{kind}:{source_id}`；同一业务问题被多域引用时**分别保留**（保留各自原始状态），不合并计数——
「不得未经去重就重复计入独立风险数量」通过**同一 kind+id 只出现一次**保证。

### 4.6 `recent_activity`（最近活动）

**权威来源（唯一）**：`governance.audit_log`（设计稿 §5.1 A + §5.3，**不新建活动表**、**不用 outbox**）。

**白名单**：设计稿 §5.3 的活动类别 → 审计 action 映射（§4.7 表）。**非白名单 action 一律不展示**
（心跳、内部重试、技术日志、普通查询、Agent 自述）。

**每条**：

| 字段 | 说明 |
|---|---|
| `id` | 审计行 ID（唯一 tiebreaker） |
| `activity_type` | 归一化类型（如 `finding_decided`、`delivery_submitted`） |
| `source_type` / `source_id` | 来源对象类型与 ID（审计 `target_type`/`target_id`） |
| `occurred_at` | `created_at` |
| `actor` | `{ "kind": "human|agent|system", "id": "…", "name": "…" }`——`kind` 由 `core.principal.kind` 映射；查不到主体 → `system` |
| `summary` | 由 action 映射生成的人类可读短句（**不返回** `before_data/after_data` 原文） |
| `dedup_key` | `{activity_type}:{source_type}:{source_id}` |

**去重**：同 `dedup_key` **保留最新一条**（`created_at DESC, id DESC` 取一）；不同 `activity_type` 不合并。
**去重必须发生在分页之前**（SQL 内 `DISTINCT ON (activity_type, target_type, target_id)`）——
否则一页可能少行、且同一条目会跨页重复（I5-E 发现的次序缺陷，见 ledger G-29）。

**排序与分页**：`ORDER BY created_at DESC, id DESC`，`LIMIT ? OFFSET ?`（offset 分页，
`activity_limit` 默认 20 / 上限 100）。稳定排序由 `id` 保证（I5-031）。

**数据新鲜度**：`as_of` = 本块查询时刻；`generated_at` 同理。

### 4.7 活动白名单映射（action → activity_type）

| 设计稿 §5.3 活动 | 审计 action | `activity_type` |
|---|---|---|
| Project 归档 | `project.archived` | `project_archived` |
| Project 创建 | **无审计 action** → **不展示**（I5-A §8.8；缺口登记，不伪造） | — |
| Goal 创建/确认/修改 | `commit_document_revision`（`after_data` 含 `document_id`）→ 仅当该 document 的 `doc_key='project-goal'` | `goal_committed` |
| Finding 确认/拒绝 | `finding.decided` | `finding_decided` |
| Finding 要求补充 | `discovery_finding_revision_requested` | `finding_revision_requested` |
| 仓库接入 | `repository.created` / `repository.verified` | `repository_created` / `repository_verified` |
| 仓库分析完成 | `repo_import.draft_succeeded` / `repo_import.attempt_succeeded` | `analysis_succeeded` |
| 仓库分析失败 | `repo_import.failed` / `repo_import.attempt_failed` / `repo_import.draft_failed` | `analysis_failed` |
| Agent Run 开始 | `run_started` / `claim_task_run` | `run_started` |
| Agent Run 完成/失败 | `run_finished` / `run_start_failed` / `terminate_task_run` | `run_finished` |
| Delivery 提交 | `delivery.submitted` | `delivery_submitted` |
| Delivery 拒绝 | `delivery.rejected` | `delivery_rejected` |
| 评审完成 | `review.completed` / `review.changes_requested` / `review.approval_decided` | `review_completed` / … |
| 成员授权变更 | `project_member.granted` / `.revoked` / `.owner_transferred` | `member_changed` |
| 设备绑定 | `device.project_bound` / `.project_unbound` | `device_binding_changed` |
| 密钥变更 | `ssh_key.generated` / `.rotated` / `.revoked` / `.deleted` | `ssh_key_changed` |
| 初始化占有 | `project_initialization.acquired` | `initialization_acquired` |

> 映射表是**白名单**：`activity_type` 只能取自本表；其余 action 一律过滤掉。
> 「Project 创建」「Discovery Proposal 决定」「Session/Question 推进」当前**无审计 action** →
> **不展示**（不伪造、不从其它记录推断）。

---

## 5. 空状态规则

| 场景 | 表现 |
|---|---|
| 项目无 Goal | `goal.present = false` + 空状态提示 + 创建入口；**不自动生成空 Goal**（设计稿 §3.6） |
| 项目零仓库 | 仓库接入 = 「未开始」；代码分析 = 「不适用」；**明确说明「不依赖仓库的工作可继续」**（§4.7） |
| 无待人工处理 | `human_action_summary.total = 0` + 「当前没有待人工处理的事项」空状态 |
| 无活跃 Run | `work_summary.active_run_count = 0` + 「当前没有执行中的 Run」 |
| 无风险 | `risk_summary.total = 0` + 「当前没有风险与阻塞」 |
| 无活动 | `recent_activity.items = []` + 「暂无活动记录」 |
| 数据源不可用 | 块 `status = 'unavailable'` + 明确文案 + 重试入口；**不得**显示 0（§7.5） |

## 6. 权限过滤

1. 端点前置：项目可见性（`tenant_id` + `core.project_member`）→ 不可见 404（C-1）；
2. 聚合内每条 SQL 带 `tenant_id` 谓词（C-2）；
3. 每条明细带来源领域 ID，可证属本项目（C-3）；
4. 不因「能看总控台」而获得任何领域写权限——本端点**纯读**（C-4）。

## 7. 部分失败处理

| 情形 | 处理 |
|---|---|
| 单块查询失败 | 该块 `status='unavailable'` + 错误摘要；其余块正常返回；整端点仍 200 |
| 多块失败 | 同样按块降级；**不整体 500**（除非权限校验失败 → 404） |
| 明细为空但块可用 | `total = 0` + 空状态文案（与 unavailable 严格区分） |
| 索引缺失导致慢查询 | 迁移 0034 补索引（`I5-G-21`/`I5-G-22`），**不在读模型层做临时规避** |

## 8. 实施拆分（I5-C / I5-D / I5-E 对应本契约）

| 批次 | 交付 | 依赖 |
|---|---|---|
| I5-C-1 | 应用层 `command_center` 读模型模块（六块查询 + 判定规则 + as_of + status 位） | 本契约 §4 |
| I5-C-2 | REST 端点 + 权限前置 + 参数 | I5-C-1 |
| I5-C-3 | 迁移 `0034`：audit_log 项目维度索引 + discovery_finding project 维度索引 | I5-G-21/22 |
| I5-D | 前端总控台页（六块 + 优先级 + 空状态 + 归档只读 + 下钻） | I5-C-2 |
| I5-E | 验收（36 条 I5-001~036 映射）与性能 | I5-D |

## 9. 与验收条目的映射

| I5 验收 | 契约落点 |
|---|---|
| I5-001/002/003 | §4.1/§4.2（零仓库 → 仓库「未开始」+ 分析「不适用」） |
| I5-004/005/006 | §4.2 代码分析（进行中/阻塞/完成后再次失败 → 以**最新有效 attempt** 判定，不看历史成功） |
| I5-007/008 | §4.1 Goal（只表达 `present` + `pending_proposal`，不下「已确认」结论） |
| I5-009/010/011 | §4.4（`display_state` 三态直接来自 I2 权威推导） |
| I5-012/013 | §4.3（活跃 Run 分列 + 历史不覆盖） |
| I5-014/015 | §4.4 `delivery_pending` + §5 空状态 |
| I5-016/034 | §2 块状态位 + §7 部分失败 |
| I5-017/018 | §6 权限过滤 |
| I5-019/020/036 | §3 C-8 + §4.5 `archive_cleanup` + §4.1 `archived` |
| I5-021 | §4 各块 `as_of` + 「进入详情页重新读权威数据」 |
| I5-022/027/028/029 | §4.2（无百分比 + 收缩规则 + `completion="not_determinable"`） |
| I5-023 | §4.2（不适用事项不计入未完成） |
| I5-024 | C-5（每字段 `source`） |
| I5-025/030/031/032/033 | §4.6 + §4.7 白名单 + 去重键 + 稳定排序分页 + 不用 outbox |
| I5-026 | 端点响应**按优先级顺序**排列块（前端亦按此渲染） |
| I5-034 | §5（区分无数据/未接入/无权限/加载失败） |
| I5-035 | §4.4（不同域原始状态分开） |
