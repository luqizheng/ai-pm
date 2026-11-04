# S01 · I5 项目总控台 · 代码事实核验（I5-A）

> 依据：`docs/ui/smark-pm-s01-i5-project-command-center-v1-1.md`（V1.1 定稿）§9.1 I5-A 十四项
> 核验基线：分支 `feat/s01-implementation`，HEAD `b35f068`，迁移 `0001–0033`
> 等级：**已证实**（代码/迁移可直接引用）｜**待核验**（代码存在但口径需确认）｜**需新增**（当前不存在）
> 交付物：本文件 = I5-A 交付物；其后 I5-B 读模型契约见 `docs/s01-i5-read-model-contract.md`

---

## 0. 结论摘要

| # | I5-A 核验项 | 结论 |
|---|---|---|
| 1 | Project 查询与权限路径 | **已证实**（权限事实源 = `core.project_member`）；**缺单项目详情端点**；`core.project.status` 无 CHECK、仅 3 值、导入失败不回退 |
| 2 | `project_repository` 关联查询 | **已证实**（列表端点存在；status 4 值；role 是类型标签不是权限） |
| 3 | `repo_import` / `repo_import_source` 结果查询 | **已证实**（导入作业 10 态；逐仓结果只在单导入详情内；**无按仓库的独立查询**） |
| 4 | Discovery Finding / Proposal 查询入口 | **已证实**（**只有 session 级**，无 project 级入口）；`display_state` 推导口径已就位 |
| 5 | 两套 Proposal 状态机 | **已证实**（8 态 vs 4 态，完全独立，**无任何映射代码**） |
| 6 | Goal 正式数据结构与状态 | **已证实**（正式 Goal = 最新 published 修订）；**无「Goal 确认状态」概念 → 需新增** |
| 7 | Task / Run / Delivery 项目级统计 | **部分需新增**（只有总数 count；**按状态分组计数缺失**；project 级活跃 Run 判定缺失） |
| 8 | 最近活动与审计事件查询 | **已证实**（`governance.audit_log` 结构）；**无应用层读取函数**；**无 `(project_id, created_at)` 索引** |
| 9 | Agent 与 Run 关联关系 | **已证实**（`task_run.agent_id` **恒 NULL**；权威是 `task.assignee_id`） |
| 10 | 开发环境真实就绪条件 | **已证实**（`status`/`readiness` 两列正交）；**无 project 级聚合查询** |
| 11 | 领域事件持久化与查询能力 | **已证实**（**不存在统一领域事件表**；outbox 不是活动历史） |
| 12 | 聚合读取的 Project / Tenant 权限边界 | **已证实**（三种并存口径；知识域**只按 project_id 过滤** → 读模型层必须补 tenant 谓词） |
| 13 | 现有接口是否足以满足总控台 | **不足**（缺 command-center 聚合端点与单项目详情；已有 delivery 聚合 / goal / members / imports / discovery 可复用） |
| 14 | 稳定排序 / 分页 / 去重 | **已证实**（多数列表**缺唯一 tiebreaker**；关键聚合**无分页**；audit_log **无项目维度索引**） |

---

## 1. Project 查询与权限路径

| # | 事实 | 等级 | 证据 |
|---|---|---|---|
| 1.1 | 项目读取主入口是**列表** `GET /v1/pm/projects`；SQL 内联 `p.tenant_id = $5 AND EXISTS(core.project_member)` | 已证实 | `apps/pm-api/src/pm_lists.rs:32,93-103` |
| 1.2 | **不存在** `GET /v1/pm/projects/{project_id}` 单项目详情端点 | 已证实 | `apps/pm-api/src/` 全部 `/v1/pm/projects` 路由均带二级后缀；`frontend/src/api/projects.ts:11,17` 亦只调 list/POST |
| 1.3 | 通用可见性 `project_visible`：`p.id AND p.tenant_id AND EXISTS(core.project_member)`；不可见即 404 | 已证实 | `apps/pm-api/src/support.rs:126-157` |
| 1.4 | 管理面判定 `is_project_manager`：租户 admin **或** `project_member.role='owner'` | 已证实 | `crates/pm-application/src/project_member.rs:178-207` |
| 1.5 | `core.project_member.role` CHECK 白名单 6 值：`owner/member/ai_pm/architect/developer/tester` | 已证实 | `migrations/0030_project_member_role.sql:10-11` |
| 1.6 | `core.project.status` **无 CHECK 约束**；实际取值只有 3 个：`planning`（默认）/ `initializing` / `archived` | 已证实 | `migrations/0001_core.sql:18-29`；写入点 `bootstrap.rs:263-271`、`project_import/lifecycle.rs:252`、`advance.rs:72-77`、`archive.rs:78` |
| 1.7 | 门禁只认 `archived`：其余一切状态（含 `initializing`）一律 Active | 已证实 | `crates/pm-application/src/project_gate.rs:51-99` |
| 1.8 | **缺口**：导入失败路径不回写 `project.status`，项目会停在 `initializing` → **初始化进度不能用 `project.status` 判定** | 已证实 | `crates/pm-application/src/project_import/lifecycle.rs:516-566` |

> **对读模型的约束**：项目头部展示「生命周期」时，归档态以 `project.status='archived'` 为权威；
> 初始化进度必须走 `core.project_initialization_operation` + 导入/仓库事实，**不得**用 `project.status`。

## 2. `project_repository` 关联查询

| # | 事实 | 等级 | 证据 |
|---|---|---|---|
| 2.1 | 「列某项目全部仓库」入口：`GET /v1/pm/projects/{project_id}/repositories` → `pm_application::repository::list_repositories` | 已证实 | `apps/pm-api/src/pm_repositories.rs:31,141-156`；`crates/pm-application/src/repository/repo.rs:3-20` |
| 2.2 | `list_repositories` SQL **不含 `tenant_id`**，隔离依赖 handler 前置 `project_visible` | 已证实 | `repository/repo.rs:8-15`；`pm_repositories.rs:109-120,146` |
| 2.3 | `status` CHECK 四值：`pending`（新建默认）/ `verified`（ls-remote 成功）/ `unreachable`（校验失败）/ `archived` | 已证实 | `migrations/0009_repo_import.sql:20-21`；`repository/repo.rs:258-339,191-227` |
| 2.4 | `role` CHECK：`main/docs/service/infra`——**仓库类型标签，不是权限等级** | 已证实 | `migrations/0008_repositories.sql:23-24`；`repository/mod.rs:24` |
| 2.5 | `head_commit_sha` = 最近一次校验拿到的默认分支 HEAD；`last_verified_at` = 校验时间（**不是分析时间**） | 已证实 | `repository/repo.rs:284-339` |
| 2.6 | 稳定错误码：`ssh_key_missing/timeout/auth_failed/not_found/too_large/git_failed` | 已证实 | `repository/repo.rs:262-266,313-320` |
| 2.7 | 排序 `created_at ASC, name ASC`——**name 可重复 → 缺唯一 tiebreaker**；无分页 | 已证实 | `repository/repo.rs:14` |

## 3. `repo_import` / `repo_import_source` 结果查询

| # | 事实 | 等级 | 证据 |
|---|---|---|---|
| 3.1 | 导入作业 status CHECK 十态：`queued/cloning/reading/analyzing/draft_ready/proposed/applied/rejected/failed/cancelled` | 已证实 | `migrations/0009_repo_import.sql:31-34` |
| 3.2 | 在途四态常量 `IN_PROGRESS = [queued, cloning, reading, analyzing]` | 已证实 | `crates/pm-application/src/project_import/mod.rs:122` |
| 3.3 | 每项目同时只允许一个在途导入（部分唯一索引） | 已证实 | `migrations/0031_s01_discovery_import_v2.sql:205-208` |
| 3.4 | `repo_import_source` 已被 0031 整表重建：现列仅 `tenant/project/import/repository/head_commit_sha/current_attempt_no`；逐仓事实迁到 `repo_import_source_attempt`（status 四态 `running/succeeded/failed/cancelled`） | 已证实 | `migrations/0031:60-101` |
| 3.5 | 现有读入口：`list_imports`（项目级作业列表，**只 9 列、无 sources、无分页**）、`get_import`（含逐仓 `sources[]` + `attempts[]`） | 已证实 | `project_import/lifecycle.rs:3-33,267-391`（逐仓 SQL `:310-324`） |
| 3.6 | **不存在**「按 `repository_id` 过滤的分析结果查询」——`repo_import_source` 全仓 8 处引用，无按仓读函数 | 已证实 | `project_import/lifecycle.rs:211,222,315,594,713,782`；`archive.rs:117` |
| 3.7 | 派生语义映射：分析进行中 = `IN_PROGRESS`；分析成功 = `draft_ready`；分析失败 = `failed`（带 `error_code`） | 已证实 | `project_import/mod.rs:122`；`lifecycle.rs:516-566,808-812` |
| 3.8 | 缺口：`list_imports` 不返回 `error_code/error_detail/stage_detail`；`repo_import` 无 `updated_at` 语义字段 | 需新增 | `lifecycle.rs:8-10`；`project_import/mod.rs:193-203` |

## 4. Discovery Finding 与 Proposal 查询入口

| # | 事实 | 等级 | 证据 |
|---|---|---|---|
| 4.1 | Finding 列表只有 **session 级**：`list_findings(pool, tenant_id, session_id)` | 已证实 | `crates/pm-application/src/discovery/finding.rs:553-584` |
| 4.2 | **不存在**「按 project_id 列 Finding / Proposal」的函数（0031 已补 `discovery_finding.project_id` 列，但无查询封装；索引只有 `(tenant_id, session_id, kind)`） | 需新增 | `migrations/0031:112-117`；`migrations/0025_discovery.sql:63-64` |
| 4.3 | 「待人工确认」与「待补充」口径**不从 open question 推导**，而由 `verification_status` + 最近一次有效人工决定审计推导：`CONFIRMED/REJECTED/PENDING/REVISION_REQUESTED` | 已证实 | `discovery/finding.rs:89-103`（审计白名单 `:51-52`；`DISTINCT ON` 取最近决定 `:61-86`） |
| 4.4 | `REVISION_REQUESTED` 不是状态值，是审计动作 `discovery_finding_revision_requested` 推出的展示态 | 已证实 | `discovery/finding.rs:54-55,96-99`；`discovery/web.rs:228-236` |
| 4.5 | `DiscoveryFindingView` 已含 `display_state`；**前端 TS 类型未含该字段** | 需新增（前端） | `discovery/finding.rs:29-47`；`frontend/src/api/discovery.ts:30-41` |
| 4.6 | Discovery Proposal 列表只有 session 级 `list_proposals(pool, tenant_id, session_id)`；`DiscoveryProposalView` 无 `display_state`、无项目归属 | 需新增 | `discovery/proposal.rs:37-51,152-166` |

## 5. 两套 Proposal 状态机

| # | 事实 | 等级 | 证据 |
|---|---|---|---|
| 5.1 | `ai.document_proposal` **八态** CHECK：`draft/quality_gates_passed/redline_pending/confirmed/commit_pending/committed/rejected/rolled_back` | 已证实 | `migrations/0014_knowledge_v2.sql:36-41` |
| 5.2 | Rust 侧**无该状态枚举**，全部以 `&str` 字面量流转；唯一近似是活跃集常量 `ACTIVE_PROPOSAL_STATUSES`（5 值） | 已证实 | `knowledge_proposal/submit.rs:140-145`、`veto.rs:102,286`；`review/knowledge.rs:18-24` |
| 5.3 | `confirmed` 与 `quality_gates_passed` 在当前实现中**无独立写点**（确认直接落 `committed`）→ 空转状态 | 已证实 | `knowledge_proposal/submit.rs:320-326,360-370` |
| 5.4 | `discovery.discovery_proposal` **四态** CHECK：`pending/approved/rejected/conflict`；只允许 `pending →` 三值迁移 | 已证实 | `migrations/0025_discovery.sql:123-124`；`discovery/proposal.rs:207-232` |
| 5.5 | 两套状态**取值域完全不重叠**，唯一关联是 `discovery_proposal.document_proposal_id → ai.document_proposal(id)` 外键 | 已证实 | `migrations/0025_discovery.sql:125,132` |
| 5.6 | **不存在**两套状态之间的映射函数 | 需新增 | 全仓 grep 无映射实现 |

> **读模型约束**：总控台必须**分开读、分开显示**；不得合并成一个通用 Proposal 状态（设计稿 §6.2）。

## 6. Project Goal 正式数据结构与状态

| # | 事实 | 等级 | 证据 |
|---|---|---|---|
| 6.1 | Goal = `ai.project_document` 中 `doc_key='project-goal'`，唯一性由既有 `UNIQUE (project_id, doc_key)` 保证 | 已证实 | `crates/pm-application/src/project_goal.rs:8-9,22`；`migrations/0006_knowledge_documents.sql:17` |
| 6.2 | 正式 Goal = **最新 `published` 修订**（`JOIN LATERAL ... ORDER BY revision_no DESC LIMIT 1`）；口径与 `knowledge::list_documents` / `knowledge_write_coordinator::current_head` 完全一致 | 已证实 | `project_goal.rs:88-122`（SQL `:93-104`） |
| 6.3 | 未建立 → `load_official_goal() == None`；HTTP 层返回 `{missing:true, goal:null}` | 已证实 | `project_goal.rs:87-91`；`apps/pm-api/src/documents.rs:88-95` |
| 6.4 | 读 Goal 会**惰性引导**（仅当 `AI_PM_KNOWLEDGE_ROOT` 显式配置）→ 「读到就建」 | 待核验 | `apps/pm-api/src/documents.rs:76-82` |
| 6.5 | **不存在** Goal 状态判读函数（待确认/已确认/待修改）；`OfficialGoal` 只有 `published_at`，无「人工确认」字段 | 需新增 | `project_goal.rs:34-44`（全模块仅 4 个函数） |
| 6.6 | **不存在** `goal.*` 审计 action——Goal 复用知识域通用 `commit_document_revision` / `veto_document_revision` | 需新增 | `knowledge_proposal/veto.rs:295-299,125-129` |
| 6.7 | Goal 提交受**归档守卫**拦截（I4-F 落地） | 已证实 | `knowledge_proposal/submit.rs:163-164,341-343`；`veto.rs:223-226` |

> **读模型约束**：I5 §6.4 要求「Goal 是否确认 → I4 Goal 状态，**不允许**根据文档是否存在推断」。
> 当前**没有** Goal 确认状态 → I5-B 必须裁定展示口径（见契约 §4.2），否则只能用「正式修订存在 / 待确认提案存在」两个事实组合表达，**不得**臆造状态字段。

## 7. Task / Run / Delivery 项目级统计

| # | 事实 | 等级 | 证据 |
|---|---|---|---|
| 7.1 | `delivery.task.status` CHECK **13 值**：`draft/ready/claimed/running/awaiting_decision/blocked/submitted/reviewing/completed/changes_requested/failed/cancelled/superseded`；领域枚举 `TaskStatus` 同集 | 已证实 | `migrations/0010_task_state_machine.sql:149-153`；`crates/pm-domain/src/task.rs:15-38` |
| 7.2 | 「项目 + 状态」聚合索引已存在：`idx_task_project_status(project_id, status)` | 已证实 | `migrations/0003_platform.sql:217-218` |
| 7.3 | 现有项目级 Task 统计**只有总数 count**；唯一「按状态过滤计数」是 MCP `pm_list_tasks` 硬编码 `status='ready'` | 需新增 | `apps/pm-api/src/pm_lists.rs:313-324`；`crates/pm-mcp/src/tools.rs:367-371` |
| 7.4 | 全仓 GROUP BY 仅 4 处（验收 2 + outbox 2），**无一针对 delivery.task** | 已证实 | `acceptance_overview.rs:242,261`；`outbox_ops.rs:206,224` |
| 7.5 | `delivery.task_run.status` CHECK 七值：`queued/claimed/running/succeeded/failed/timed_out/cancelled`；`outcome` 四值 | 已证实 | `migrations/0011_run_dispatch.sql:26-28`；`0010:26-27` |
| 7.6 | 活跃 Run 集合单一真源 `ACTIVE_RUN_STATUSES = [queued, claimed, running]` | 已证实 | `crates/pm-application/src/run_dispatch/mod.rs:43-50` |
| 7.7 | 现有活跃 Run 判定**全是 task 级**；**无 project 级**判定 | 需新增 | `run_dispatch/claim.rs:319-325`；`pm-infrastructure/outbox_handlers.rs:96-104`；`acceptance_overview.rs:271-273` |
| 7.8 | `delivery.task_run_repository` = **本次领取锁定的基线快照**（1 Run : N 仓），与 `delivery.task_repository`（任务契约仓集合）是两层不同表 | 已证实 | `migrations/0011_run_dispatch.sql:107-121` |
| 7.9 | 「待人工验收」口径在 `acceptance_overview.rs`（**不在 delivery/ 下**）：`PENDING_TASK_STATUSES = [submitted, reviewing]` | 已证实 | `crates/pm-application/src/acceptance_overview.rs:15,221-301` |
| 7.10 | 项目级交付聚合**已落地**（S01-W60）：`GET /v1/pm/projects/{id}/delivery`，四态 `no_contract/delivered/partial/missing` 判定函数 | 已证实 | `apps/pm-api/src/pm_delivery.rs:29,35-65`；`delivery/manifest.rs:236-244` |
| 7.11 | 「风险与阻塞」口径**缺失**：状态机有 `blocked`，但与 `delivery.task_waiting`（reason 四值）联合的项目级读模型不存在 | 需新增 | `migrations/0010:151,90-104` |

## 8. 最近活动与审计事件查询

| # | 事实 | 等级 | 证据 |
|---|---|---|---|
| 8.1 | `governance.audit_log` 列：`id, project_id, actor_id, action, target_type, target_id, before_data, after_data, compensation_command, compensated_command_id, source_event_id, analysis_run_id, created_at` + `source_kind/source_ref` + `actor_identity_id/actor_client/actor_auth_mode` + `tenant_id` + `claimed_device_id`；`project_id` NOT NULL | 已证实 | `migrations/0003_platform.sql:188-203`；`0004:7-9`；`0018:25-27`；`0019:134`；`0027:87-88` |
| 8.2 | **索引缺口**：该表全仓只有 `idx_audit_claimed_device` 一个索引——**没有 `(project_id, created_at)` 索引**（对比租户级 `tenant_audit_log` 有 `(tenant_id, created_at DESC)`） | 需新增 | `migrations/0027_devenv.sql:89-90`；`0019:65-66` |
| 8.3 | 唯一 project 级审计入口是 HTTP `GET /v1/pm/audit-logs?project_id=`；返回 7 列（**无 after_data、无 actor 名称、无摘要**） | 已证实 | `apps/pm-api/src/pm_lists.rs:38,461-503`；`crates/pm-application/src/rows.rs:123-132` |
| 8.4 | **不存在** pm-application 层「取某项目最近 N 条审计」的函数 | 需新增 | 全仓 `FROM governance.audit_log` 仅 5 处，均非列表用途（`command_executor/executor.rs:136`、`run_executor.rs:410`、`discovery/finding.rs:72`、`discovery/web.rs:405`、`pm-ai/orchestrator.rs:427`） |
| 8.5 | 排序 `ORDER BY g.created_at DESC` **缺 id tiebreaker** → 分页不稳定 | 已证实 | `apps/pm-api/src/pm_lists.rs:479` |
| 8.6 | SSE `GET /v1/pm/events/stream` 只推**新**审计行（LISTEN/NOTIFY），无历史回放 | 已证实 | `apps/pm-api/src/audit_stream.rs:74-136`；`migrations/0005_audit_notify.sql:12-35` |
| 8.7 | 已有审计 action **全集**（供「最近活动」白名单）：点号式 42 个 + 下划线式 30 个 + 租户级 5 个（租户级**不属于**项目活动） | 已证实 | 见下方 §8.8 白名单表 |
| 8.8 | **不存在** `goal.*`、`discovery_proposal.*`、`session.*`、`question.*`、`project.created` 等 action（建项目与 discovery proposal 决定**不写审计**） | 需新增 | `project_goal.rs` 全文无审计；`discovery/proposal.rs:200-246` 无审计写入；`bootstrap.rs` 无审计写入 |

**§8.7 审计 action 白名单（项目级，已证实存在）**

- 项目/成员：`project.archived`、`project_initialization.acquired`、`project_member.granted|revoked|owner_transferred`
- 发现：`finding.decided`、`discovery_finding_revision_requested`
- 知识：`commit_document_revision`、`veto_document_revision`、`requirement.propose_revision`、`requirement.approve_revision`、`approval.decide`
- 任务/依赖：`task.create`、`task.update`、`task.update_contract`、`task.failed`、`task.retry_created`、`task.dependency.add|remove`
- 交付/评审：`delivery.submitted`、`delivery.rejected`、`review.queued`、`review.changes_requested`、`review.completed`、`review.approval_decided`、`review.failed`、`review.degraded`
- Run/调度：`claim_task_run`、`reclaim_task_run`、`skip_task_run`、`renew_run_lease`、`run_started`、`run_stage_started`、`run_progress_reported`、`run_blocked`、`run_finished`、`run_start_failed`、`terminate_task_run`、`executor_session_replaced`、`run.orphan_suspected`
- 仓库/密钥/导入：`repository.created|updated|deleted|archived|verified`、`ssh_key.generated|rotated|revoked|deleted`、`repo_import.started|terminated|failed|draft_succeeded|draft_failed|attempt_succeeded|attempt_failed`
- 设备：`device.project_bound`、`device.project_unbound`
- 命令执行器：`rollback_command`、`sign_acceptance`、`create_milestones`、`create_requirements`、`create_tasks`、`create_task`、`write_documents`、`approve_requirement_revision`、`adjust_milestone_plan`、`update_task_status`

## 9. Agent 与 Run 关联关系

| # | 事实 | 等级 | 证据 |
|---|---|---|---|
| 9.1 | `core.principal.kind` CHECK 四值：`human/dev_agent/ai_pm/service` | 已证实 | `migrations/0001_core.sql:33-35` |
| 9.2 | `core.project_member.role` CHECK 六值，**一列两义**：`owner/member` = 真人访问角色；`ai_pm/architect/developer/tester` = Agent 职能定位（与访问权限无关） | 已证实 | `migrations/0030:7-9`；`crates/pm-domain/src/project_member.rs:13-62` |
| 9.3 | `delivery.task_run.agent_id` **恒为 NULL**——全仓无任何生产写入方 | 已证实 | DDL `migrations/0020_agent_run_identity.sql:46`；全仓 grep 无 INSERT/UPDATE（只读出 `pm_runs.rs:44,80,134,178`） |
| 9.4 | `delivery.task_run.triggered_by_user_id` 同样**无生产写入方** | 已证实 | `migrations/0020:47`；`migrations/0028_run_execution_mode.sql:8-11`（注释自认） |
| 9.5 | `executor_id` 语义 = **已校验的设备 id**（非 principal），claim 时绑定 `claimed_device_id` | 已证实 | `run_dispatch/claim.rs:656,665-666`（注释 `:172`） |
| 9.6 | **权威的执行主体关联 = `delivery.task.assignee_id` → `core.principal.kind='dev_agent'`**（调度判定 4 用的就是它） | 已证实 | `run_dispatch/policy.rs:182-189` |
| 9.7 | `core.tenant_agent` 表存在（租户级 Agent 实例映射），但**除 bootstrap 外无任何读查询接口** | 已证实 | `migrations/0020:33-41`；`bootstrap.rs:360-395` |

> **读模型约束**：总控台「当前执行」要展示 Agent，只能走 `task_run ⋈ task.assignee_id`；
> **不得**使用 `task_run.agent_id`（恒 NULL，会造成「无 Agent 在跑」的假象）。

## 10. 开发环境真实就绪条件

| # | 事实 | 等级 | 证据 |
|---|---|---|---|
| 10.1 | `devenv.device` 列：`status`（lifecycle，CHECK `active/revoked`）+ `readiness`（就绪快照，CHECK 六值 `installed/connected/authenticated/ready/degraded/failed`，**可空 = 从未自检**）——**两列正交** | 已证实 | `migrations/0027_devenv.sql:23-51`（裁定注释 `:14-17,32-38`） |
| 10.2 | readiness 由**服务端按可验证事实计算**（`compute_readiness`），**不采信客户端声明** | 已证实 | `crates/pm-application/src/devenv/checks.rs:139-164`（模块头 `:7`） |
| 10.3 | 撤销时 `status='revoked'` 且 `readiness = NULL` | 已证实 | `devenv/mod.rs:398-402` |
| 10.4 | 「项目可用性」是**派生值不落库**（绑定 × 当前调用人成员资格） | 已证实 | `devenv/checks.rs:5-6`；`devenv/mod.rs:367-379` |
| 10.5 | `list_devices` 是 **tenant 级**；**无「按项目列出已绑定设备及其 readiness」的查询** | 需新增 | `devenv/mod.rs:327-347` |

## 11. 领域事件的持久化与查询能力

| # | 事实 | 等级 | 证据 |
|---|---|---|---|
| 11.1 | **不存在**统一领域事件表/事件流（除审计外）：`integration.inbound_event` 是入站幂等台账、`integration.notification` 是通知流、`ai.analysis_run` 是作业记录 | 已证实 | `migrations/0003_platform.sql:6-21,40-50,107` |
| 11.2 | `integration.outbox_event` 是**事务性发件箱**（4 态 `pending/processing/delivered/dead_letter`），**不是活动历史库** | 已证实 | `migrations/0010:122-146`；`crates/pm-application/src/outbox.rs:99-129`（事件类型目录仅 5 个） |
| 11.3 | outbox 现有查询只有死信/重放/统计，**无「按 project 取最近 N 条事件」**；无保留期清理策略 | 已证实 | `outbox_ops.rs:99,161,203` |
| 11.4 | 审计行**不带业务摘要/标题**，只有 `before_data/after_data` JSONB；现有列表端点也不返回 | 需新增 | `migrations/0003_platform.sql:188-203`；`pm_lists.rs:472-473` |
| 11.5 | 活动**去重键**（稳定业务对象 + 动作标识）无既有实现 | 需新增 | 无 |

> **读模型约束**：第一版「最近活动」**以审计为唯一权威来源**（设计稿 §5.1 A + §5.3），
> 不新增活动事实表；outbox 不作为活动历史（I5-033）。

## 12. 聚合读取时的 Project / Tenant 权限边界

| # | 事实 | 等级 | 证据 |
|---|---|---|---|
| 12.1 | 全仓三种并存口径：(a) SQL 内联 `tenant_id + EXISTS(project_member)`；(b) handler 前置 `require_project_visible`，SQL 只按 `project_id`；(c) 管理面 `is_project_manager` | 已证实 | `support.rs:126-157`；`pm_lists.rs:99-100`；`project_member.rs:178-207` |
| 12.2 | 跨租户与「同租户非成员」一律 **404**，不泄露存在性 | 已证实 | `support.rs:146-157` |
| 12.3 | **知识域读路径一律只按 `project_id`，不带 `tenant_id`**（三表虽有 `tenant_id NOT NULL` + 组合 FK） | 已证实 | `knowledge.rs:496-506,533-542`；`knowledge_proposal/query.rs:14`；`project_goal.rs:103` |
| 12.4 | Discovery 域读路径**显式带 `tenant_id`** | 已证实 | `discovery/finding.rs:558-561,591-593` |
| 12.5 | `delivery::list_project_delivery` 等项目级聚合 SQL **只按 `project_id`**，隔离靠 REST 前置 | 已证实 | `delivery/manifest.rs:179-204`；`apps/pm-api/src/pm_delivery.rs:40-46` |
| 12.6 | `latest_decision_actions` 按 `project_id + target_type + action` 过滤，不带 tenant | 已证实 | `discovery/finding.rs:70-76` |
| 12.7 | **读模型层必须自带 tenant 谓词**——若直接复用既有应用层函数而不重复校验，会出现跨租户缺口 | 需新增（读模型层约束） | 由 12.3/12.5 推导 |

## 13. 现有接口是否足以支撑总控台

**已存在、可直接复用（项目级）**：

| 能力 | 端点 | 证据 |
|---|---|---|
| 项目级交付聚合 | `GET /v1/pm/projects/{id}/delivery` | `pm_delivery.rs:29` |
| 正式 Goal | `GET /v1/pm/projects/{id}/goal` | `routes.rs:211-214`；`documents.rs:69-101` |
| 项目成员 | `GET /v1/pm/projects/{id}/members` | `project_members.rs:36,51-77`（**无 total/分页**） |
| 仓库列表 | `GET /v1/pm/projects/{id}/repositories` | `pm_repositories.rs:31` |
| 导入作业 | `GET /v1/pm/projects/{id}/imports`、`/{import_id}` | `pm_imports.rs:27,31` |
| Discovery 会话 | `GET /v1/pm/projects/{id}/discovery/sessions` | `pm_discovery.rs:28` |
| 审计列表 | `GET /v1/pm/audit-logs?project_id=` | `pm_lists.rs:38` |
| 任务/Run/审批/决策/工件列表（可按 project_id 过滤） | `/v1/pm/tasks`、`/task-runs`、`/approvals`、`/decisions`、`/artifacts`、`/review-approvals` | `pm_lists.rs:34-38`；`pm_runs.rs:31`；`pm_reviews.rs:36`；`pm_artifacts.rs:20` |
| 验收工作台 | `GET /v1/pm/acceptance-overview?project_id=` | `pm_acceptance.rs:27` |

**缺失（需新增）**：

| # | 缺什么 | 依据 |
|---|---|---|
| 13.1 | `GET /v1/pm/projects/{id}` 单项目详情（头部需要项目身份/生命周期单行事实） | §1.2；`ProjectRow` DTO 已具备全部字段（`rows.rs:35-50`） |
| 13.2 | **总控台聚合端点**（`GET /v1/pm/projects/{id}/command-center` 或拆分端点） | 设计稿 §7.3（候选，未实现） |
| 13.3 | 项目级「按仓库看分析结果」查询 | §3.6 |
| 13.4 | 项目级「Task 按状态分组计数」「project 级活跃 Run 判定」「Agent ↔ Run 归因」「项目级设备就绪」 | §7.3/7.7/9.6/10.5 |
| 13.5 | 应用层「取某项目最近 N 条审计」函数 + `(project_id, created_at DESC, id DESC)` 索引 | §8.2/8.4 |
| 13.6 | 「待人工处理」跨域合并读模型（待确认 Finding + 待审批 + 待验收 + 待确认知识提案） | 各域分散，见 §4/§5/§7.9 |
| 13.7 | 每个数据块的「数据新鲜度（as_of）」与「部分失败状态位」字段 | 设计稿 §7.4/§7.5；现有聚合响应无此类字段 |
| 13.8 | 归档只读态的派生表达（archived + 残留清理任务数） | `archive.rs:76-120` 有终态化逻辑但无聚合读 |

## 14. 稳定排序 / 分页 / 去重

| # | 事实 | 等级 | 证据 |
|---|---|---|---|
| 14.1 | **有唯一 tiebreaker 的读路径**：项目级交付聚合（`created_at DESC, id` / `task_id, repository_id`）、验收事实（`created_at DESC, task_id`）、Discovery 会话（`created_at DESC, id`）、成员按 role+display_name（**display_name 非唯一 → 实际不稳**） | 已证实 | `delivery/manifest.rs:185,200`；`acceptance_overview.rs:284`；`discovery/session.rs:97`；`project_member.rs:102` |
| 14.2 | **缺唯一 tiebreaker 的读路径（分页可能漏行/重行）**：项目列表、任务列表、Run 列表（`attempt_no` 仅 task 内唯一）、审批、决策、审计、工件、需求、里程碑、仓库、成员、导入作业、设备 | 已证实 | `pm_lists.rs:102,261,304,346,389,437,479`；`pm_runs.rs:92`；`pm_artifacts.rs:57`；`repository/repo.rs:14`；`project_member.rs:102`；`project_import/lifecycle.rs:10`；`devenv/mod.rs:338` |
| 14.3 | **无分页（一次拉全量）**：项目级交付聚合、验收总表、`review-approvals`（硬 `LIMIT 100`）、文档提案列表、设备列表、Discovery 会话列表 | 已证实 | `delivery/manifest.rs:174-259`；`acceptance_overview.rs:221-291`；`pm_reviews.rs:76`；`knowledge_proposal/query.rs:16`；`devenv/mod.rs:338`；`discovery/session.rs:95` |
| 14.4 | 分页基础设施统一可用：`paging()`（page≥1、size≤200）、响应 `{items,total,page,size}`；**仅 offset 分页，无 keyset/cursor** | 已证实 | `apps/pm-api/src/support.rs:73-112` |
| 14.5 | 去重范式已有：`DISTINCT ON`（验收取最新 run）+ `EXISTS` 半连接（成员过滤） | 已证实 | `acceptance_overview.rs:244-255`；`pm_lists.rs:99-100` |
| 14.6 | `pm_acceptance.rs:149-150` 的 LATERAL `ORDER BY` 缺 `r.id` tiebreaker | 已证实 | `apps/pm-api/src/pm_acceptance.rs:142-151` |

---

## 15. 需新增清单（总控台读模型的实施输入）

> 编号 `I5-G-xx` 供 I5-B 契约与 I5-C 实现引用。

### 15.1 端点层

| # | 缺口 | 归属 |
|---|---|---|
| I5-G-01 | `GET /v1/pm/projects/{id}/command-center`（六数据块）或拆分端点 | I5-C |
| I5-G-02 | `GET /v1/pm/projects/{id}` 单项目详情（可复用 `ProjectRow`） | I5-C（可选：并入 command-center） |
| I5-G-03 | 项目级「按仓库看分析结果」查询 | I5-C |
| I5-G-04 | `audit-logs` 稳定排序 + 行动白名单过滤参数 | I5-C（读模型层白名单过滤更稳） |
| I5-G-05 | 项目级设备/环境就绪端点或读模型 | I5-C（可延后） |

### 15.2 查询层

| # | 缺口 | 归属 |
|---|---|---|
| I5-G-06 | 按项目 + Task 状态分组计数（可照抄 `outbox_ops.rs:206` 范式） | I5-C |
| I5-G-07 | project 级活跃 Run 判定 + 活跃 Run 清单（JOIN task.assignee_id） | I5-C |
| I5-G-08 | project 级 Finding 列表（`discovery_finding.project_id` 列已存在，缺索引与查询） | I5-C |
| I5-G-09 | project 级「待人工处理」跨域合并读模型 | I5-C |
| I5-G-10 | 应用层「取某项目最近 N 条审计」函数（含稳定排序 + 白名单过滤） | I5-C |
| I5-G-11 | project 级「仓库分析状态」汇总（仓库 × 最近 attempt 状态/错误码/commit） | I5-C |
| I5-G-12 | project 级设备就绪聚合 | I5-C（可延后到 I5-E） |
| I5-G-13 | 知识域读路径统一补 `tenant_id` 谓词（读模型层） | I5-C |

### 15.3 字段 / 语义层

| # | 缺口 | 归属 |
|---|---|---|
| I5-G-14 | Goal 确认状态判读（I5-B 须裁定口径，不得臆造状态列） | I5-B 裁定 |
| I5-G-15 | 两套 Proposal 状态**分开**的读模型（禁止合并成通用状态） | I5-C |
| I5-G-16 | 每个数据块的 `as_of`（数据新鲜度）与 `status`（ok/unavailable）字段 | I5-C |
| I5-G-17 | 活动去重键（稳定业务对象 + 动作标识） | I5-B 裁定 + I5-C |
| I5-G-18 | 活动可读摘要（审计行无 summary，需按 action 映射生成） | I5-C |
| I5-G-19 | 归档只读态派生（archived + 残留清理计数） | I5-C |
| I5-G-20 | 审计 action 白名单（见 §8.7）作为「最近活动」允许集 | I5-B 固化 |

### 15.4 索引 / 迁移层

| # | 缺口 | 归属 |
|---|---|---|
| I5-G-21 | `governance.audit_log (tenant_id, project_id, created_at DESC, id DESC)` 索引 | 迁移 0034 |
| I5-G-22 | `discovery_finding (tenant_id, project_id, ...)` 索引（project 级 Finding 读用） | 迁移 0034 |

### 15.5 前端契约层

| # | 缺口 | 归属 |
|---|---|---|
| I5-G-23 | `DiscoveryFinding` TS 类型补 `display_state` | I5-D |
| I5-G-24 | 总控台聚合端点的 TS 客户端与类型 | I5-D |

---

## 16. 核验方式

- 全部结论来自只读手段：`Read`（迁移与源码全文）、`grep`/`sed`（行号取证）；**未修改任何文件**。
- 三个域分别核验（项目/仓库/导入、发现/知识/Goal、Task/Run/交付/审计/环境），再按 I5-A 的 14 项归并。
- 等级判据：**已证实** = 迁移/源码可直接引用且语义明确；**待核验** = 代码存在但依赖运行期数据或口径需裁定；**需新增** = 当前代码中不存在。
- 本文结论与 `docs/s01-code-facts-ledger-v1.md`（差异台账 G-xx/D-C-xx）配套：本文件是 **I5-A 专项事实源**，
  台账登记其结论与偏差裁决。
