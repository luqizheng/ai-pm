# S01 Migration Registry V1.0

> 文档性质：S01-0 退出产物（总体计划 §3.1 W03、§10）
> 上位：`docs/s01-全局主菜单与信息架构设计基线/docs/s01-implementation-plan-v1.md`
> 代码基线：`master 2933db8`，`migrations/0001_core.sql` ~ `0028_run_execution_mode.sql`
> 版本：V1.0　日期：2026-11-03

---

## 1. 管理原则

1. Migration **不以 TASK 编号为唯一依据**，也不允许随意修改已执行的历史 Migration（总体计划 §10.1）。
2. 三类状态：`draft`（设计未完成）/ `reviewed`（DDL、兼容性、测试已审查）/ `frozen`（允许正式实施）。
3. **已冻结的 Migration 不直接修改**；修正优先新增增量 Migration。
4. 每项变更必须记录：migration_id / source_design / source_task / dependencies / upgrade / fresh_install / constraints / data_backfill / rollback / test / freeze_status（总体计划 §10.1）。
5. **本项目为新项目、无存量数据**（`AGENTS.md` 硬规则 1）：凡"回填/兼容旧数据"的设计一律降级或删除；需要改契约就直接改。

> 本条与 I6-B 合并规格中的"历史数据回填"要求冲突时，**以 AGENTS.md 为准**（见 §5 裁决 C-10）。

---

## 2. 存量 Migration 盘点（0001–0028）

### 2.1 逐文件清单

| # | 文件 | 表（按文件内顺序） | 主题 |
|---|---|---|---|
| 0001 | core.sql | schema core/delivery/integration/ai/governance(L4-8)；`core.customer`(10)、`core.project`(17)、`core.principal`(31)、`core.project_member`(41) | 身份与项目骨架 |
| 0002 | delivery.sql | `delivery.requirement`(3)、`requirement_revision`(16)、ALTER requirement+FK(36-39)、`milestone`(41)、`task`(51)、`task_requirement`(68) | 需求/任务 |
| 0003 | platform.sql | `integration.inbound_event`(6)、`integration.job`(22)、`integration.notification`(40)、`ai.project_ai_config`(52)、`ai.skill_definition`(75)、`ai.project_skill_binding`(82)、`ai.analysis_run`(107)、`ai.change_proposal`(128)、`governance.approval_request`(157)、`governance.human_intervention`(170)、`governance.audit_log`(188)、`ai.agent_feedback`(204) | 平台事件/队列/治理 |
| 0004 | audit_source.sql | ALTER `governance.audit_log` +`source_kind`+`source_ref`(7-9) | 审计来源 |
| 0005 | audit_notify.sql | fn `governance.notify_audit_insert()`(9) + trigger(30) | 审计 NOTIFY 扇出 |
| 0006 | knowledge_documents.sql | `ai.project_document`(7)、`ai.document_revision`(22) | 知识文档 + Git 修订 |
| 0007 | llm_provider.sql | `ai.llm_provider`(7) | LLM 供应商配置 |
| 0008 | repositories.sql | `integration.project_ssh_key`(6)、`integration.project_repository`(18) | 项目↔仓库绑定 |
| 0009 | repo_import.sql | ALTER `project_ssh_key`(6)、ALTER `project_repository`(14)、`integration.repo_import`(27)、`integration.repo_import_source`(60) | 仓库可达性 + 导入会话 |
| 0010 | task_state_machine.sql | ALTER `delivery.task`(5,12,147,149)；`delivery.task_run`(18)、`task_run_failure`(47)、`task_supersession`(73)、`task_waiting`(90)、`task_policy_override`(109)；`integration.outbox_event`(123) | 13 态任务状态机 + Run |
| 0011 | run_dispatch.sql | ALTER `task_run`(26,31)；`delivery.task_repository`(86)、`delivery.task_run_repository`(108) | Run 领取/租约/多仓基线 |
| 0012 | task_run_event_idempotency.sql | ALTER `task_run` +`source_event_id`(7) | 事件级幂等 |
| 0013 | policy_scope.sql | `delivery.policy_scope`(9)；ALTER `task` +`task_type`(21) | 多级审批策略 |
| 0014 | knowledge_v2.sql | ALTER `document_revision`(10)；`ai.document_proposal`(21)、`ai.document_index_state`(60)、`ai.knowledge_reference`(82) | 知识 V2 提案管线 |
| 0015 | task_contract.sql | ALTER `task`(15)；`task_acceptance_criterion`(25)、`task_knowledge_ref`(42)；ALTER `task_run`(58) | 执行契约四列 + 逐条验收 |
| 0016 | policy_composition.sql | ALTER `policy_scope`(13,28) | 策略合成四层能力 |
| 0017 | customer_master_data.sql | ALTER `core.customer`(8) + UPDATE(28) | 客户升一等实体 |
| 0018 | principal_identity.sql | `core.principal_identity`(10)；ALTER `audit_log`(25) | JWT→真实主体 |
| 0019 | tenant.sql | `core.tenant`(21)、`core.tenant_member`(31)、`governance.tenant_audit_log`(51)；ALTER `principal_identity`(77)、`audit_log`(85)；**39 表加 tenant_id**(93-146)；**15 表加 UNIQUE(tenant_id,id)**(154-168)；拆 32 处单列 FK(180-221)；建组合 FK(228-391) | 多租户基础设施 |
| 0020 | agent_run_identity.sql | `ai.agent_definition`(14)、`core.tenant_agent`(33)；ALTER `task_run`(45) | Agent 三层 + Run 归因 |
| 0021 | task_dependency.sql | `delivery.task_dependency`(9) | Task 依赖边 |
| 0022 | delivery_manifest.sql | ALTER `task_run`(21,30,42)；`delivery.task_repository_commit`(51)；VIEW `delivery.v_task_delivery_manifest`(105) | 交付清单事实 |
| 0023 | run_executor_hardening.sql | ALTER `task_run`(7) | 执行器加固四列 |
| 0024 | knowledge_section_index.sql | `ai.document_section_index`(11)、`ai.knowledge_write_operation`(43) | 章节索引 + 写入幂等账本 |
| 0025 | discovery.sql | schema discovery(9)；`discovery_session`(12)、`discovery_message`(33)、`discovery_finding`(48)、`discovery_finding_repository`(67)、`discovery_question`(79)、`discovery_answer`(96)；ALTER `ai.document_proposal`+uq(111)；`discovery_proposal`(113)；ALTER `repo_import` +`discovery_session_id`(139) | Discovery 域 |
| 0026 | delivery_review.sql | ALTER `task_run`(15,26,33)；`delivery.review_finding`(165)、`acceptance_criterion_verdict`(193)、`governance.review_approval`(223)；VIEW 重建(88)；fn/trigger(257…) | Review / 失败判定 / 知识回写 |
| 0027 | devenv.sql | schema devenv(19)；`devenv.device`(23)、`devenv.device_project_binding`(58)；ALTER `audit_log`(87)、`tenant_audit_log`(92) | 开发环境目录域 |
| 0028 | run_execution_mode.sql | ALTER `task_run` +`execution_mode`(15) | Run 执行形态 |

### 2.2 S01 关键表的重点约束核对

#### `integration.repo_import`（`0009:27-49` + `0019` + `0025`）

| 项 | 现状 | 位置 |
|---|---|---|
| `project_id` | ✅ 存在，`UUID NOT NULL` | `0009:29` |
| `draft` | ⚠️ **单列 JSONB，可空**（S01 缺陷 D-1 目标） | `0009:38` |
| `draft_revision` | `BIGINT NOT NULL DEFAULT 0`（S01 降级为指针） | `0009:39` |
| `status` | CHECK 10 值：`queued/cloning/reading/analyzing/draft_ready/proposed/applied/rejected/failed/cancelled` | `0009:31-34` |
| 唯一键 | `UNIQUE (project_id, id)`(0009:48)、`uq_repo_import_tenant_id UNIQUE (tenant_id, id)`(0019:163)、部分唯一索引 `repo_import_one_in_progress ON (project_id) WHERE status IN (...)`（**未租户化**，缺陷 D-9） | 0009:48,52-54；0019:163 |
| 外键 | `fk_repo_import_tenant_project`→`core.project(tenant_id,id)` CASCADE(0019:295-297)；`fk_repo_import_tenant_analysis`→`ai.analysis_run(tenant_id,id)` 无 ON DELETE(0019:350-352)；`fk_repo_import_tenant_discovery_session`→`discovery_session(tenant_id,id)` 无 ON DELETE(0025:139-143) | — |

#### `integration.repo_import_source`（`0009:60-74` + `0019`）

| 项 | 现状 | 位置 |
|---|---|---|
| 列 | `import_id`、`repository_id`、`head_commit_sha`、`commit_count`、`doc_count`、`size_bytes`、`truncated`、`facts JSONB NOT NULL DEFAULT '{}'`、`error_code`、`created_at`、`tenant_id` | 0009:61-72；0019:140 |
| 主键 | ⚠️ **`PRIMARY KEY (import_id, repository_id)`** —— 无 `UNIQUE (tenant_id, id)`（0019 的 15 表清单未含本表） | 0009:73；0019:154-168 |
| 外键 | `fk_ris_tenant_import`→`repo_import(tenant_id,id)` CASCADE(0019:381-383)；`fk_ris_tenant_repository`→`project_repository(tenant_id,id)` **CASCADE**（缺陷 D-8） | 0019:384-386 |
| CHECK | 无 | — |

> **结论**：S01 的 R-6-1（补组合唯一键）与 R-4-1（facts 迁 Attempt）必须**同批改写本表**，是 Migration 0031 的核心。

#### `integration.project_repository`（`0008:18-33` + `0009:14-21` + `0019`）

| 项 | 现状 | 位置 |
|---|---|---|
| 唯一键 | `UNIQUE (project_id, url)`(0008:30)、`UNIQUE (project_id, id)`(0008:31)、`uq_project_repository_tenant_id UNIQUE (tenant_id, id)`(0019:162)；⚠️ **无 `(tenant_id, project_id, id)`** —— merge-spec 补-3 要求新增 | — |
| 列增补 | `ssh_key_id`（**无 FK**，0009:15）、`status` CHECK(`pending/verified/unreachable/archived`)、`head_commit_sha`、`last_verified_at`、`last_error_code`、`last_error_detail` | 0009:14-21 |
| `role` | `TEXT CHECK (role IN ('main','docs','service','infra'))` —— **仓库类型标签，不是权限等级** | 0008:23-27 |
| 外键 | `fk_project_repository_tenant_project`→`core.project(tenant_id,id)` CASCADE | 0019:292-294 |

#### Discovery 域（全部 `0025`）

| 表 | 主键/唯一 | 外键 | CHECK |
|---|---|---|---|
| `discovery_session` | PK `id`；`UNIQUE (tenant_id, id)`(0025:24)；⚠️ **无 `(tenant_id, project_id, id)`**（补-1 要求新增） | `created_by`→`principal`；(tenant_id, project_id)→`core.project(tenant_id,id)` CASCADE(0025:25) | `source IN (WEB,CODEX,SYSTEM)`(16)；`kind IN (INITIAL_ONBOARDING,UPDATE,TASK_DISCOVERY)`(17)；`status IN (IN_PROGRESS,NEEDS_USER_INPUT,REVIEW_REQUIRED,COMPLETED,PAUSED)`(18-20) |
| `discovery_finding` | PK `id`；`UNIQUE (tenant_id, id)`(0025:60)；⚠️ **无 `project_id` 列**（缺陷 D-3） | 仅 `(tenant_id, session_id)`→`discovery_session(tenant_id,id)` CASCADE(0025:61) | `kind IN (observed,user_confirmed,speculation,contradicted)`(52)；`confidence ∈ [0,1]`(54)；`verification_status IN (PENDING,CONFIRMED,REJECTED,CONFLICT)`(55-56) |
| `discovery_finding_evidence` | ❌ **表不存在**（缺陷 D-5） | — | — |
| `discovery_finding_repository` | PK `(tenant_id, finding_id, repository_id)`(0025:71) | `(tenant_id, finding_id)`→finding CASCADE(72-73)；`(tenant_id, repository_id)`→`project_repository(tenant_id,id)` **CASCADE**（缺陷 D-4/D-8） | 无 |
| `discovery_proposal` | PK `id`；`UNIQUE (tenant_id, id)`(0025:128) | 4 条：session CASCADE(129)、`project_document`(130)、`document_revision`(131)、`document_proposal`(132)，**后三条无 ON DELETE** | `action IN (add,modify)`(117)；`status IN (pending,approved,rejected,conflict)`(123-124) |
| `discovery_question` / `discovery_answer` / `discovery_message` | — | 既有 | `status IN (open,answered,deferred)` |

#### `delivery.task_repository`（`0011:86-103`）

- PK `(task_id, repository_id)`(0011:97)；**无 `UNIQUE (tenant_id, id)`**。
- FK：`(project_id, task_id)`→`delivery.task` **CASCADE**(0011:98-99)；`(project_id, repository_id)`→`project_repository` **无 ON DELETE ⇒ NO ACTION**(0011:101-102) —— 仓库删除被阻止但**未映射为领域错误（现报 500）**，即 C-4 要修的点。
- 列：`write_access BOOLEAN DEFAULT true`、`base_ref`、`base_commit`、`path_scope TEXT[]`、`role CHECK IN (primary,secondary,knowledge)`。
- 0019 追加 `fk_task_repository_tenant_project`→`core.project(tenant_id,id)` **NO ACTION**(0019:268-270)。

> 注意：`task_repository.write_access` 是**布尔**（单一写权限）。I3 V1.1 简化裁决后，
> **它不再决定访问档位**（范围内且 verified 的仓库一律 `read_write`），
> 仅保留两处用途：**调度前置**（至少一个可写仓库才可领取）与**交付规则**
> （只读仓产出 commit 整单驳回）。原「与六能力枚举语义不等价 → 需换轨」的结论
> 随六能力模型一并废止（见 §3 M-0033-I3-S）。

---

## 3. S01 新增 Migration 规划

### 3.1 编号与归属

存量止于 `0028`。S01 新增自 **0029** 起；`0030` 已由 S01-W13 占用，故 I6-B 批落在 **0031**。**一个数据库变更只有一个主责 TASK**（实施计划 §12.3）。

| migration_id | 名称 | 主责 TASK | 依赖 | 状态 |
|---|---|---|---|---|
| 0030 | `s01_project_member_role` | S01-W13 | 0019 | **已实施（2026-11-03）** |
| **0031** | `s01_discovery_import_v2` | **I6-B-08** | 0030 | **frozen（2026-11-04：§4.4 六项审查全过，含升级演练）** |
| 0032 | `s01_git_capability` | I3-B | 0031 | **已实施（2026-11-04）；I3-D 的 `policy_snapshot_hash` 一并落此** |
| **0033** | `s01_git_access_simplify` | **I3-B（V1.1 简化）** | 0032 | **已实施（2026-11-04）：DROP `integration.git_capability_grant`** |
| **0034** | `s01_command_center_indexes` | **I5-C** | 0033 | **已实施（2026-11-04）：`audit_log (tenant,project,created_at DESC,id DESC)` + `discovery_finding (tenant,project,verification_status,created_at DESC)` 两个索引** |
| **0035** | `s01_run_cleanup` | **I1-E** | 0034 | **已实施（2026-11-04）：`delivery.task_run_cleanup` 持久化进程清理账本（pending→cleaning→completed / needs_human + 退避重试 + 两个部分索引）** |
| 0034 | `s01_project_goal` | I4-B/I4-C | 0031 | draft（I4 复用 `ai.project_document`，实际未新增迁移） |
| 0035 | `s01_project_delivery_read_model` | S01-W60 | 0022、0026 | draft（可能纯视图，无需新表） |

> **0033 详案（`s01_git_access_simplify`）**：依 I3 设计稿 §0.0 V1.1 简化裁决
> （用户 2026-11-04：「不区分，都可以读写。因为这部分太复杂。」），
> 撤下 V1.0 的仓库级能力授权表 `integration.git_capability_grant`（含其索引）。
> 0032 保留的 `task_run.policy_snapshot_hash`（I3-D 锚点）**继续使用**——
> 快照内容由「每仓能力集」改为「每仓可访问清单（单一 `read_write` 档位）」。
> 无回填（AGENTS.md 规则 1：授权表自 0032 起无业务存量数据）。
> **编号勘误**：原规划的 `0033 s01_run_policy_snapshot` 未单独占用
> （该列并入 0032），`0034 s01_project_goal` 未新增迁移（复用既有文档表）。

### 3.2 M-0031 详案（`s01_discovery_import_v2`）

**主责 I6-B-08**；参与 I6-B-01/02/03/04/05/07/09。

对照合并规格 §2/§3/§4 的对象清单：

| # | 对象 | 动作 | 依据 |
|---|---|---|---|
| 1 | `core.project_initialization_operation` | **新建**：`tenant_id, project_id, import_id, status, generation, created_at, released_at`；部分唯一索引 `ON (project_id) WHERE status='active'` | R-3-3、R-3-4、merge-spec §5 |
| 2 | `integration.repo_import_one_active_per_project` | **重建索引**：DROP `repo_import_one_in_progress`(0009:52-54) → 新建含 tenant 的部分唯一索引 | C-3、R-6-1 |
| 3 | `integration.repo_import_draft_version` | **新建**：PK `(tenant_id, import_id, revision)`；列 `draft JSONB`、`source_attempts JSONB`、`dropped_items INT`、`generation_status CHECK(generating/succeeded/failed)`、`generated_at`、`created_at`；FK `(tenant_id, import_id)`→`repo_import(tenant_id,id)` CASCADE | D-1a、merge-spec §1.2 |
| 4 | `integration.repo_import.draft` | **删列**（新项目无存量数据，AGENTS.md 规则 1）；`draft_revision` 保留为指针 | D-1a、merge-spec §1.2 规则 1 |
| 5 | `integration.repo_import_source` | 加 `project_id`；**删** `facts`/`commit_count`/`doc_count`/`size_bytes`/`truncated`（迁 Attempt）；补 `UNIQUE (tenant_id, project_id, import_id, repository_id)`；FK 换轨：`(tenant_id, project_id, import_id)`→`repo_import(tenant_id, project_id, id)`（需先给 `repo_import` 补 `UNIQUE (tenant_id, project_id, id)`）；`(tenant_id, project_id, repository_id)`→`project_repository(tenant_id, project_id, id)` **RESTRICT**（需先给 `project_repository` 补 `UNIQUE (tenant_id, project_id, id)`） | R-4-1、R-6-1（扩展四列）、C-4、merge-spec §2.1 |
| 6 | `integration.repo_import_source_attempt` | **新建**：PK `(tenant_id, import_id, repository_id, attempt_no)`；`UNIQUE (tenant_id, project_id, import_id, repository_id, attempt_no)`；列 `project_id`、`status CHECK(running/succeeded/failed/cancelled)`、`commit_sha`、`facts JSONB`、`commit_count`、`doc_count`、`size_bytes`、`truncated`、`error_code`、`error_detail`、`job_id`、`started_at`、`finished_at`；FK→source 四列 CASCADE；CHECK `finished_at IS NULL OR finished_at >= started_at` | R-4-1、merge-spec §2.2 补-2 |
| 7 | `discovery.discovery_session` | 补 `UNIQUE (tenant_id, project_id, id)` | 补-1 |
| 8 | `discovery.discovery_finding` | 加 `project_id`；`UNIQUE (tenant_id, project_id, id)`；FK 换轨为 `(tenant_id, project_id, session_id)`→`discovery_session(tenant_id, project_id, id)` CASCADE | 补-1、D-3 |
| 9 | `discovery.discovery_finding_evidence` | **新建**：`tenant_id, project_id, finding_id, import_id?, repository_id?, commit_sha?, attempt_no?`；FK `(tenant_id, project_id, finding_id)`→finding CASCADE；FK `(tenant_id, project_id, import_id, repository_id)`→source CASCADE；FK `(tenant_id, project_id, import_id, repository_id, attempt_no)`→attempt CASCADE；**CHECK 判别器**：代码类证据四列全非空、非代码类全空 | R-6-2、补-2、D-5 |
| 10 | `discovery.discovery_finding_repository` | 加 `project_id`；FK 换轨：`(tenant_id, project_id, finding_id)`→finding CASCADE；`(tenant_id, project_id, repository_id)`→`project_repository(tenant_id, project_id, id)` **RESTRICT** | 补-3、D-4 |
| 11 | `discovery.discovery_proposal` | FK 换轨携带 project（session/document/revision/proposal 四条） | merge-spec §2.2 图 |
| 12 | `integration.project_repository` | 补 `UNIQUE (tenant_id, project_id, id)` | merge-spec §2.2 补-3 |
| 13 | `integration.repo_import` | 补 `UNIQUE (tenant_id, project_id, id)` | merge-spec §2.1 |
| 14 | `delivery.task_repository` | FK→`project_repository` 由 NO ACTION 改 **RESTRICT**；应用层映射 409 `repository_has_history` | C-4 |

**M-0031 冻结门槛**（总体计划 §6.5 + merge-spec §6）：

| # | 条件 | 判定 |
|---|---|---|
| 1 | D-1、D-2 正式进入 V1.1 正文 | ⬜ 待 A-1 |
| 2 | Attempt / Evidence 复合外键进入 DDL | ✅ 本文件 §3.2 第 6/9 项 |
| 3 | `repo_import_source` 补所需复合唯一键 | ✅ 第 5 项（四列键） |
| 4 | 跨 Project 关联约束采用已确认的数据库级方案 | ✅ D-2a + 补-1/2/3 |
| 5 | 代码/非代码证据 CHECK 判别器明确 | ✅ 第 9 项 |
| 6 | 历史数据兼容性检查通过 | ✅ **按 AGENTS.md 规则 1 改为「空库初始化 + 结构断言」**（见 §5 C-10；已落地） |
| 7 | 旧环境升级通过 | ✅ **真实升级演练通过**（`scripts/upgrade-drill-0028-to-0031.sh`：0028 → 0030 → 0031 升级库与新全序库表/列/约束/索引**完全一致** + 0031 十项关键对象抽查全过，2026-11-04） |
| 8 | 新环境初始化通过 | ✅ I6-B-09 实测（空库 → 0031） |
| 9 | 并发、幂等、恢复、负向约束测试通过 | ⬜ I6-B-09 实测 |

### 3.3 DB 级负向测试 → 约束映射（merge-spec §4）

| # | 负向场景 | 保证约束 |
|---|---|---|
| 1 | 同 Tenant 不同 Project 的 Finding 与 Source 建 Evidence | `fk_evidence_finding_scope` + `fk_evidence_source_scope` |
| 2 | 同 Project 不同 Import 的 Source 被错误 Attempt 引用 | `fk_evidence_attempt_scope`（含 import_id） |
| 3 | Evidence 引用的 Attempt 不属于其 Source | 同上（attempt_no 与 source 四列同键锚定） |
| 4 | Source.project_id 与 Import 不一致时创建/更新 | `fk_source_tenant_project_import` |
| 5 | Finding.project_id 与 Session 不一致时创建/更新 | 补-1 的 session 三列唯一键 + 换轨 FK |
| 6 | 同租户跨项目仓库挂 Finding（D-09 复发） | 补-3 的 finding_repository 换轨 FK |
| 7 | 代码类证据缺 import/commit/attempt_no | R-6-2 CHECK 判别器 |

执行方式：**直接对数据库执行 INSERT/UPDATE 断言约束违例**，不经 REST/MCP。

---

## 4. 重复模型与职责归属登记

| 概念 | A 处 | B 处 | 裁决 |
|---|---|---|---|
| 知识稿件发布 | `ai.document_proposal`（8 态，含 `base_revision_id` 乐观并发） | `discovery.discovery_proposal`（4 态 `pending/approved/rejected/conflict`） | **两套独立生命周期**（总体计划 §8.3）：A = 知识发布管线（权威），B = Discovery 提案审核（镜像 + 溯源）。合并规格要求 B 通过 `document_proposal_id` 关联 A，**不合并状态机** |
| 草案存储 | `integration.repo_import.draft`（单列覆盖写） | 待建 `repo_import_draft_version` | **单列废止**，新版表为唯一权威（merge-spec §1.2） |
| 事实与计数 | `integration.repo_import_source.facts/counts`（每 Source 一份，被覆盖） | 待建 `repo_import_source_attempt.facts/counts`（每轮一份） | **Source 列删除**，Attempt 为唯一权威（R-4-1） |
| 状态机 | Task 状态机（13 态，`pm-domain`） | Discovery Session 状态机（5 态，`discovery/session.rs:161-176`） | **两套并存**，但 Discovery 不得推进 Task 权威状态（缺陷 D-10，收敛于 I6-B-05） |
| 权限码 | `PM_MCP_READ` / `PM_MCP_DOCUMENT_WRITE`（HTTP 层） | 真人成员门 / `resolve_project_executor`（应用层） | 双层并存，各自职责不同；MCP 权限码表**待补全**（D-11） |

---

## 5. 本次核验的裁决追加

| 编号 | 裁决 | 依据 |
|---|---|---|
| **C-10** | 合并规格中的「历史数据回填 / 旧环境兼容」要求**降级为「空库初始化 + 结构断言」**；`draft` 单列**直接删列**而非"停用保兼容" | `AGENTS.md` 硬规则 1（新项目、无存量数据、不写兼容分支）。同时删除 M-0031 中的 data_backfill 项 |
| **C-11** | `repo_import_source` 的三列唯一键 `(tenant_id, import_id, repository_id)` **不建**（无引用方），只建四列 Project 作用域键 | merge-spec §2.1「建议简化」+ §0「R-6-1 接受并扩展」 |
| **C-12** | `project_repository.role`（main/docs/service/infra）**不是权限等级**，不得当作 Git 能力等级复用 | `0008:23-27` 语义 + I3 §3.1（**V1.1 简化后仍成立**：访问档位与仓库类型标签无关） |
| **C-13** | ~~`delivery.task_repository.write_access BOOLEAN` 不满足 I3 六能力模型，0031 需换轨为能力集合~~ → **作废（2026-11-04 简化裁决）**：六能力模型废止，`write_access` 不再决定访问档位，仅作调度前置与交付规则输入 | 原依据 I3 §3.1 依赖链随 V1.1 简化裁决一并废止（见 I3 设计稿 §0.0、ledger D-C-03） |

---

## 6. 存量 Migration 与 S01 设计的关系（§10.2 已知关联的核验结果）

| Migration | 总体计划 §10.2 已知关联 | 核验结论 |
|---|---|---|
| 0009 | Repo Import 早期模型 | ✅ 属实；**D-1/D-2/D-9 三个缺陷均在本文件涉及**，需在 0031 换轨 |
| 0011 | Task Run 执行模型 | ✅ 属实；`task_repository` 的 NO ACTION→500 是 C-4 要修点 |
| 0014 | Document Proposal | ✅ 属实；保留知识稿件生命周期，**不与 discovery_proposal 合并** |
| 0016 | Policy Composition | ✅ 属实；策略合成四层能力字段，**与 Git 能力无关**（勿混用 auto_* 字段作 Git 授权） |
| 0019 | Repository Import Source 相关结构 | ✅ 属实；**本表被 0019 的「15 表加 UNIQUE(tenant_id,id)」清单遗漏**，是 D-2 的成因 |
| 0022 | 多仓库执行关联 | ✅ 属实；`task_run_repository` 基线快照已存在，S01 不重建 |
| 0025 | Discovery Session、Finding 关联等 | ✅ 属实；**全域已在，D-3/D-4/D-5 均为「已建但约束不足」而非「未建」** |
| 0028 | （总体计划未列） | 当前最新，S01 的升级基线 |

**盘点完成度**：0001–0028 **已全量盘点**（总体计划 §10.2 的「必须完整盘点」要求已满足，见实施计划勘误 E-5）。

---

## 7. 新增 Migration 登记模板（M-0031 ~ M-0035 待细化）

| 字段 | M-0031 | M-0032 | M-0033 | M-0034 | M-0035 |
|---|---|---|---|---|---|
| migration_id | 0031 | 0032 | 0033 | 0034 | 0035 |
| source_design | 合并规格 §2 + I6-B V1.1 | I3 §12 I3-B | I3 §12 I3-D | I4 §12/§13 | 总体计划 §9.1 |
| source_task | I6-B-08 | I3-B | I3-D | I4-B/I4-C | S01-W60 |
| dependencies | 0030 | 0031 | 0032 | 0031、0014 | 0022、0026 |
| upgrade | 0030→0031 | 0031→0032 | 0032→0033 | 0031→0034 | — |
| fresh_install | 全序 | 全序 | 全序 | 全序 | 全序 |
| constraints | 四列组合唯一键 + 换轨 FK + RESTRICT | 能力枚举 CHECK + 授权唯一键 | 快照列 + 索引 | Goal 唯一约束（每 Project 唯一） | 视图定义 |
| data_backfill | **无**（AGENTS.md 规则 1） | 无 | 无 | 无 | 无 |
| rollback | DROP | DROP | DROP | DROP | DROP VIEW |
| test | 0030→0031 实测 + 空库 | 同 | 同 | 同 | 同 |
| freeze_status | draft | draft | draft | draft | draft |
| 门禁 | G-DDL（+G-DATA） | G-USER(B-5/B-6) | G-USER(B-6) | G-USER(B-4) | G-INTERFACE |
