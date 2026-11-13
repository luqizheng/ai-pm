# S02-01 · 需求领域资产盘点（Requirement Domain Asset Inventory）V1.1

> **阶段定位**：S02 场景「需求提出与需求管理」的**现状依据**（代码事实核验）。在 S01 系列中，本位置对应「S01 现状依据 V1.1」；它是 S02 后续所有设计稿的事实前提。
> **核验基线**：worktree `D:\kudi-project\templates\ai-pm-s01`，分支 `feat/s01-implementation`，HEAD `807ddae`（"docs(s01): I6-B-05/06 回填 Interface Matrix 与 Acceptance Matrix"）。
> **本稿性质**：事实记录，**不是设计稿**，也**不是 S02 最终设计基线**。本 V1.1 在 V1.0 基础上修正事实表述、状态标记与裁决边界（外部专家评审收编，见 §14）；不新增领域对象，不写 Migration，不冻结任何尚未裁决的字段。设计裁决留到 S02 最终基线。
> **上游参照**：S01 的 I6-A 代码事实核验报告（`smark-pm-s01-i6-code-facts-v1.md`）、I6-B 边界评审（`smark-pm-s01-i6-b-boundary-review-346-v1.md`）、S01 最终基线 V1.2/V1.3。

---

# §0. 结论摘要

V1.1 修订重点：明确 NFR 统一纳入 Requirement 的已确认业务裁决（类型仅 `functional` / `non_functional`）；将 `constraint`、项目级基线、正文权威等保持为待裁决项；修正验收方式、修订关联与静态证据强度表述。逐条修订记录见 §14。

## 0.1 总体判断

**现有系统已经具备需求领域实体与修订定稿机制，不是"只有 Markdown 文档"。** 但 S02 的核心结果「形成经过人类确认的需求基线」在当前代码里**没有任何强制机制**——需求定稿既可以被 AI 自动执行，也可以被任意租户成员直接调用 REST 完成。

三条主线结论：

**① 已实现且可复用（地基比预期好）**

- `delivery.requirement` / `delivery.requirement_revision` / `delivery.task_requirement` 三张表自迁移 `0002_delivery` 起存在，具备**独立 UUID 稳定 ID**、项目内唯一 `code`、修订版本机（`draft → approved → superseded`）、定稿人 `approved_by`、`content_hash`、行级乐观并发计数 `row_revision`。
- REST 读 / 写 / 定稿全链路**实现齐备、测试用例在仓**（本稿为静态核验，未复跑；证据强度约束见 §14.4）；任务上下文读模型能带出关联需求的最新修订正文。
- 需求正文有**第二处表示**（知识空间 Markdown 文档，`doc_type='requirement'`），并有单向迁移 + 回滚工具。

**② 部分实现 / 机制缺口（都是 S02 必须补的）**

| 缺口 | 性质 |
|---|---|
| **无需求创建端点** | `POST /v1/pm/requirements` 不存在；需求**只能**由冷启动导入（`CreateRequirements`）创建 |
| **人工确认缺失** | `approve_requirement_revision` **不在策略红线名单** → AI 提案走 `AutoApply` 自动执行，无人类确认 |
| **授权边界偏弱** | 需求写路径只校验租户，**不校验项目成员**（读路径校验）；无任何角色校验 |
| **两条定稿路径语义分叉** | REST 路径无行锁、无乐观并发校验、无补偿；执行器路径三样都有。两套审计词汇并存 |
| **MCP 侧零需求工具** | 18~20 个 MCP 工具无一针对需求；需求仅作为 `pm_get_task_context` 的一个附带字段暴露 → 触发 I2-D「REST/MCP 必须同修防分叉」铁律 |
| **需求行级属性缺失** | 无 `requirement_type`（已确认类型 functional / non_functional，字段尚不存在）、无 `priority`、无 `created_by`（`created_by` 只存在于修订表） |
| **追溯关系近乎空白** | 全库外键扫描：除需求自身与 `task_requirement` 外，**无任何表引用需求** |

**③ 完全不存在**

- 需求类型字段与类型约束——字段当前不存在；用户已于 2026-11-03 裁决功能需求与非功能需求纳入同一需求体系、通过类型区分（该**业务裁决已确认，数据库实现尚不存在**，见 §11 / §14.1）
- 需求状态的完整生命周期（无拒绝 / 撤回 / 作废 / 归档）
- 需求之间的依赖、冲突、派生关系
- 需求变更影响分析
- 项目级需求基线快照——当前只有每条 Requirement 的 `current_revision_id`（**单条需求的当前修订指针，不能等同于项目级冻结基线**）
- 需求级验收标准载体（§11.3 约束③，V1.1 新增登记）
- 需求 ↔ 架构决策关联（§11.3 约束②，V1.1 新增登记）

## 0.2 对第一轮盘点的三处更正

第一轮盘点（基于 I6-A 报告与 I6-B 材料的二手归纳）在三个点上有偏差，已用代码实证更正：

| # | 第一轮说法 | 核验结论 | 证据 |
|---|---|---|---|
| C-1 | "目前没有证据证明已实现：独立 Requirement 实体及稳定 ID" | **证伪**。实体存在，`id UUID PK`，项目内 `code` 唯一，修订版本与定稿人齐备 | `migrations/0002_delivery.sql:3-39` |
| C-2 | "`knowledge_migrate` 可能只是把历史需求文档迁移到知识版本体系" | **部分正确但结论方向错了**。它确实是单向迁移（需求正文 JSONB → 知识 Markdown），但正确表述是：**需求模型存在，正文存在第二处表示**，而非"系统里没有需求模型" | `crates/pm-application/src/knowledge_migrate.rs:1-5,16-64` |
| C-3 | "Finding 的 CONFIRMED 不能直接代表需求已被正式批准" | **成立，且比第一轮所述更严重**：不仅 Finding 的确认不等于需求批准，**需求自己的定稿环节也不要求人类参与**——不存在"人类确认"这一环 | `policies/ai-pm-ai-lead-v2.yaml:6-39` + `crates/pm-application/src/command_executor/executor.rs:52-56` |

第一轮判断正确、予以保留的部分：Finding / Proposal / Requirement 三者不混同；不建议把需求塞进 `ai.document_proposal`（该表 `doc_type` 的 `requirement` 取值指的是"文档类型是需求文档"，属知识空间，与需求领域对象不是一回事——见 §8.2）。

## 0.3 证据标记含义

| 标记 | 含义 |
|---|---|
| `[已实现]` | 表 / 代码 / 测试**用例**三者齐备，且路径可达（**静态判定**——测试用例存在不等于本轮运行通过） |
| `[部分实现]` | 结构存在但链路不全、或只覆盖部分场景 |
| `[仅设计]` | 仅存在于设计稿 / 种子数据，代码未落地 |
| `[不存在]` | §0.4 所列扫描范围内未发现任何实现或结构 |
| `[存在缺陷]` | 有实现但行为与预期不符，或存在安全 / 一致性缺口 |
| `[仅静态]` | 本轮为静态代码核验，未实跑数据库或端到端验证 |

> **本稿整体标记为 `[仅静态]`**：所有结论来自源码、迁移与测试代码的静态阅读，未连库实跑、未跑端到端。§13 列出需实跑复核的项。

## 0.4 核验范围与深度

| 覆盖 | 深度 |
|---|---|
| 迁移 `0001`–`0028`（基线）+ `0030`、`0031`（S01 新增） | 全量逐表扫读，重点表逐列登记 |
| `apps/pm-api/src/`（30 个源文件） | 需求相关路由、handler、鉴权、中间件逐个确认 |
| `crates/pm-domain` / `pm-application` / `pm-ai` / `pm-mcp` | 领域命令、执行器、读模型、AI 编排、工具面 |
| `policies/ai-pm-ai-lead-v2.yaml` + `policy_engine.rs` | 全文比对，确认红线名单与执行语义 |
| 前端 `frontend/src/` | 仅确认页面与 API 客户端方法存在，**未做交互级核验** |
| 真实模型输出行为 | **未实证**（fake model 未产出需求定稿提案，见 §7.1） |

---

# §1. 核验基线与证据来源

| 类别 | 位置 | 说明 |
|---|---|---|
| 代码基线 | `D:\kudi-project\templates\ai-pm-s01`（`feat/s01-implementation` @ `807ddae`） | 唯一同时含 S01 实施代码（迁移 0030/0031）与 `docs/ui/` 全套文档的检出 |
| 参照基线 | `D:\kudi-project\templates\ai-pm`（`master` @ `2933db8`） | 仅文档基线，迁移止于 0028，**不含** S01 实施成果，不作为代码事实来源 |
| 迁移 | `migrations/0001_core.sql` … `0028_run_execution_mode.sql` + `0030_project_member_role.sql` + `0031_s01_discovery_import_v2.sql` | 注：`0029` 号缺号（迁移登记表载明 I6-B 批落在 `0031`，`0029` 预留） |
| 领域命令 | `crates/pm-domain/src/command.rs` | `ProjectCommand` 全量 |
| 执行器 | `crates/pm-application/src/command_executor/executor.rs` | `PgCommandExecutor::execute` / `compensate` |
| 写路径 | `crates/pm-application/src/pm_write/`（`revision.rs` / `mod.rs` / `audit.rs`） | REST 侧直写实现 |
| 读模型 | `crates/pm-application/src/read.rs` | 任务上下文 |
| 策略 | `policies/ai-pm-ai-lead-v2.yaml`（`policy_engine.rs` 内 `DEFAULT_POLICY_YAML` 逐字镜像） | 红线名单唯一真源 |
| AI 编排 | `crates/pm-ai/src/orchestrator.rs`、`skill_router.rs`、`openai_adapter.rs` | 提案 → 策略 → 执行/审批链路 |

---

# §2. 需求领域数据库事实

## 2.1 `delivery.requirement`（S02-01-001 ~ 010）`[已实现]`

来源：`migrations/0002_delivery.sql:3-14`，租户化改造见 `migrations/0019_tenant.sql:103,157,185,241-243`。

| 列 | 类型 | 约束 / 默认 | 事实判定 |
|---|---|---|---|
| `id` | UUID | PK, `gen_random_uuid()` | **稳定 ID 存在**（S02-01-001） |
| `project_id` | UUID | → `core.project(id)`，0019 改为租户复合外键 | 项目归属成立 |
| `code` | TEXT | `UNIQUE (project_id, code)` | 项目内人类可读编号成立（与 `delivery.task` 的 `title` 风格不同，需求有业务编号） |
| `title` | TEXT | NOT NULL | 标题 |
| `status` | TEXT | NOT NULL DEFAULT `'draft'` | **仅 `'draft'` / `'approved'` 两值被代码写入**（S02-01-002，见 §3.4） |
| `current_revision_id` | UUID | FK `(id, current_revision_id)` → `requirement_revision(requirement_id, id)` | **当前修订指针（单条需求粒度；非项目级基线快照）**成立（S02-01-003，V1.1 措辞修正见 §14.3） |
| `row_revision` | BIGINT | NOT NULL DEFAULT 1 | 乐观并发计数**被写但未被校验**（S02-01-004，见 §7.4） |
| `created_at` | TIMESTAMPTZ | NOT NULL DEFAULT now() | |
| `tenant_id` | UUID | NOT NULL（0019 补列），`UNIQUE (tenant_id, id)` | 租户隔离成立 |

**索引**：`idx_requirement_project ON delivery.requirement(project_id, status)`（`migrations/0003_platform.sql:220-221`）——恰好覆盖「按项目 + 按状态」的项目级需求 Dashboard 查询形态（S02 场景用户时刻 1）。

**行级属性缺失（S02-01-005）**：
- **无 `requirement_type` 列** → 无法承载已确认的 functional / non_functional 类型区分（字段形态待 Q-16 裁决；`constraint` 未确认为类型）
- **无 `priority` 列** → 优先级只写在首版修订的 JSONB `content.priority` 里（`executor.rs:427-431`），**不可查询、不可排序、不可索引**
- **无 `created_by` 列** → 需求由谁提出不可知（`created_by` 只在 `requirement_revision` 上，见 §2.2）
- **无 `archived_at` / `superseded_by` 等生命周期列**

## 2.2 `delivery.requirement_revision`（S02-01-011 ~ 018）`[已实现]`

来源：`migrations/0002_delivery.sql:16-34`。

| 列 | 类型 | 约束 | 事实判定 |
|---|---|---|---|
| `id` | UUID | PK | 修订独立 ID |
| `requirement_id` / `project_id` | UUID | FK → `requirement(project_id, id)` | 归属一致 |
| `version` | INT | NOT NULL, `CHECK (version > 0)`, `UNIQUE (requirement_id, version)` | **版本序列成立**，按需求内单调递增（S02-01-011） |
| `content` | JSONB | NOT NULL | 正文载荷。实际写入形态为 `{"markdown": "..."}`（`revision.rs:44`）或 `{"title","markdown","priority"}`（`executor.rs:427-431`）——**两种形态不一致**（S02-01-012） |
| `source_artifact_id` | UUID | 可空 | 来源产物指针（**代码中未见写入**） |
| `state` | TEXT | `CHECK (state IN ('draft','approved','superseded'))` | **三态修订状态机成立**（S02-01-013） |
| `content_hash` | TEXT | NOT NULL | `sha256("{rid}:{version}:{markdown}")`（`revision.rs:33`）——**绑定 rid+version+markdown，不是纯正文哈希**（S02-01-014） |
| `created_by` | UUID | → `core.principal(id)` NOT NULL | 起草人成立 |
| `approved_by` | UUID | → `core.principal(id)` 可空 | **定稿人成立**（S02-01-015） |
| `tenant_id` | UUID | 0019 补列 | |

**缺列（S02-01-016）**：无 `approved_at`（定稿时间不可知）、无 `rejected_by` / `reject_reason`、无 `superseded_by`、无 `parent_revision_id`（修订之间的血缘/派生关系不可表达）。

## 2.3 `delivery.task_requirement`（S02-01-019 ~ 024）`[已实现但有约束隐患]`

来源：`migrations/0002_delivery.sql:68-80`。

```
project_id, task_id, requirement_id, revision_id, tenant_id
PRIMARY KEY (task_id, revision_id)
FK (project_id, task_id)        → delivery.task(project_id, id)
FK (project_id, requirement_id)  → delivery.requirement(project_id, id)
FK (requirement_id, revision_id)→ delivery.requirement_revision(requirement_id, id)
```

**成立的部分**：
- 需求 ↔ 任务多对多成立（一张需求可挂多个任务，一个任务也可挂多个需求）——**S02-01-019**
- 关联**指向具体修订**（`revision_id`），因此"任务依据哪一版需求"可追溯——**S02-01-020**，修订级追溯有真实价值

**V1.1 措辞修正（§14.3）**：关联记录**指向**具体 revision 是事实；但该关联**并非不可变**——是否允许同一任务关联同一需求的多个修订、关联是否可改绑、改绑是否留历史，均为**待裁决**（Q-17），不在本稿冻结。

**隐患（S02-01-021）**：
- 主键是 `(task_id, revision_id)`，**不含 `requirement_id`** → 同一任务可同时关联同一需求的**两个不同修订**，产生重复行；任务上下文读模型用 `ORDER BY rr.created_at DESC, rr.version DESC LIMIT 1` 兜底（`read.rs:47-53`），即**同任务同需求多修订时静默取最新**，旧修订链接变成噪声行。
- 无 `created_at` → 关联关系自身无审计痕迹。

**写入路径仅一处**：`executor.rs:605-618`（`create_tasks` 时随任务创建一并落库，`ON CONFLICT DO NOTHING`）。REST 侧**无任务 ↔ 需求关联端点**。

## 2.4 全库外键扫描：谁引用了需求（S02-01-022 ~ 025）

对 `migrations/*.sql` 全量扫描 `REFERENCES ... requirement`：

| 引用方 | 位置 | 性质 |
|---|---|---|
| `delivery.requirement.current_revision_id` | `0002_delivery.sql:36-39` | 需求自身当前修订指针 |
| `delivery.task_requirement.revision_id` | `0002_delivery.sql:78-79` | 需求 → 任务 |

**除此之外，全库没有任何一张表以外键引用 `delivery.requirement` 或 `delivery.requirement_revision`。**

推论（S02-01-023）：

| 追溯方向 | 状态 |
|---|---|
| Requirement → Task | ✅ 外键 + 钉修订 |
| Requirement → Design Decision | ❌ **不存在**（`governance.approval_request` / `ai.change_proposal` 的 `target_type` 实际取值只有 `'task'` / `'task_run'` / `'project_repository'`，无 `'requirement'`；**该列无 CHECK，扩值零迁移成本**——S02-01-039） |
| Requirement → Knowledge Document | ⚠️ **非外键**，靠 `ai.document_proposal.source_ref = 'requirement_revision:<id>'` 字符串约定 + `change_summary` 文本 |
| Requirement → Finding / Discovery Session | ❌ **不存在**（`discovery.discovery_finding` 连 `project_id` 都没有，见 §8.1） |
| Requirement → Requirement | ❌ **不存在**（无依赖表、无冲突表、无派生表） |

---

# §3. 需求生命周期：两条定稿路径

系统的"需求定稿"有**两条互不相干、严谨度不同的实现路径**。这是本轮最重要的结构性发现。

## 3.1 路径 A：REST 直写（`pm_write`）

**端点**：`POST /v1/pm/requirements/{requirement_id}/revisions`（起草）、`POST /v1/pm/requirements/{requirement_id}/revisions/{revision_id}/approve`（定稿）——`apps/pm-api/src/pm_lists.rs:40-45`。

**起草** `pm_write::create_revision`（`pm_write/revision.rs:4-75`）：
1. 取 `content.markdown`，空则 `Validation` 拒绝
2. `version = max(version)+1`（按需求内计数）
3. `content_hash = sha256("{rid}:{version}:{markdown}")`
4. 插入 `state='draft'` 修订
5. 写审计 `requirement.propose_revision`

**定稿** `pm_write::approve_revision`（`pm_write/revision.rs:77-140`）：
1. 读修订 `state`；非 `'draft'` → `Conflict`（409）
2. 目标修订 → `'approved'`，写 `approved_by`
3. 同需求其余 `'approved'` → `'superseded'`
4. `requirement.current_revision_id = 新修订`，`status='approved'`，`row_revision + 1`
5. 写审计 `requirement.approve_revision`

**该路径缺失的能力**：

| 缺失 | 证据 | 后果 |
|---|---|---|
| 无行锁 | `revision.rs:80-90` 读修订不带 `FOR UPDATE` | 并发定稿时，双方都可能读到目标仍为 `draft`、先后通过检查，随后**交错写入**（双双推进 `current_revision_id` / 双审计 / `row_revision` 双跳），无锁不互斥（V1.1 措辞修正见 §14.3） |
| 无乐观并发校验 | `CreateRevisionRequest` 只有 `content.markdown`（`pm_write/mod.rs:109-112`），**没有 `expected_revision` 字段**；定稿请求体为空 | `row_revision` 只增不校，客户端无从表达"我基于第 N 版定稿" |
| 无补偿 | 该路径不写 `compensation_command` 审计字段 | 定稿**不可回滚**（执行器路径可回滚） |
| 无角色校验 | `approve_revision_handler`（`pm_writes.rs:236-247`）只做 `requirement_in_tenant` | 任意租户成员可定稿任意需求 |

## 3.2 路径 B：命令执行器（`ProjectCommand::ApproveRequirementRevision`）

**领域命令**：`crates/pm-domain/src/command.rs:26-28`；**处理器**：`executor.rs:76-82` → `approve_requirement_revision`（`executor.rs:809-971`）。

执行流程（含补偿规格）：
1. 校验 `project_id == ctx.project_id`
2. `SELECT current_revision_id, row_revision FROM delivery.requirement WHERE id=$1 AND project_id=$2 FOR UPDATE`——**行锁**
3. 锁目标修订，校验归属与状态
4. 原 approved 修订 → `'superseded'`（记录快照）
5. 目标修订 → `'approved'` + `approved_by`
6. `requirement.current_revision_id = 目标`，`row_revision + 1`
7. 写审计 `approve_requirement_revision`（含 before/after 快照 + `CompensationSpec::Restore`）
8. 补偿通道 `rollback_command` 可回滚指针与两版修订状态

**该路径的额外能力**：行锁、归属校验、补偿回滚、审计快照、执行回执 `resulting_revision`。

## 3.3 两路径对照 `[存在缺陷]`

| 维度 | 路径 A（REST `pm_write`） | 路径 B（命令执行器） | 判定 |
|---|---|---|---|
| 并发保护 | ❌ 无锁、无校验 | ✅ `FOR UPDATE` | **分叉** |
| 补偿回滚 | ❌ 不可回滚 | ✅ `CompensationSpec::Restore` | **分叉** |
| 审计动作名 | `requirement.approve_revision` | `approve_requirement_revision` | **两套词汇**（S02-01-024） |
| 审计粒度 | 单条动作行 | 动作行 + before/after 快照 + 补偿规格 | **分叉** |
| 角色约束 | ❌ 无 | ⚠️ 依赖策略红线（当前**未配置**，见 §7.1） | **分叉** |
| 触发者 | 任意租户成员 | AI 提案自动执行 / 人工红线审批 | **分叉** |

**S02-01-025 判定**：这不是"两条路径各管一段"，而是**同一语义的两份不一致实现**。S02 必须裁决唯一权威定稿路径，并让另一条路径收敛或移除。

## 3.4 状态取值全集（S02-01-026）

| 对象 | 字段 | 代码中实际写入的取值 |
|---|---|---|
| `delivery.requirement` | `status` | `'draft'`（创建时，`executor.rs:435-437`）、`'approved'`（定稿时，`revision.rs:114` / `executor.rs:909`） |
| `delivery.requirement_revision` | `state` | `'draft'`、`'approved'`、`'superseded'`（`revision.rs:98,105` / `executor.rs:892,901`）——表 CHECK 恰好只有这三值（`0002_delivery.sql:23-25`），**没有拒绝态** |

**结论**：需求**没有"被拒绝"、没有"被撤回"、没有"被作废"、没有"被归档"**。一条被否决的需求只能停在 `draft`，永久占用 `code`。

---

# §4. API 面与双入口

## 4.1 REST 端点清单（完整枚举）

> 本表「已实现」为**静态判定**（实现与测试用例在仓，本稿未复跑，见 §14.4）。

| 方法 | 路径 | 位置 | 鉴权 | 状态 |
|---|---|---|---|---|
| GET | `/v1/pm/requirements` | `pm_lists.rs:33` | 租户 + **项目成员**（`require_project_visible`） | ✅ 已实现（静态判定） |
| GET | `/v1/pm/requirements/{id}/revisions` | `pm_lists.rs:40-42` | 同上（列表侧） | ✅ 已实现（静态判定） |
| POST | `/v1/pm/requirements/{id}/revisions` | `pm_lists.rs:40-42` | 仅**租户** | ✅ 已实现（授权偏弱） |
| POST | `/v1/pm/requirements/{id}/revisions/{rev}/approve` | `pm_lists.rs:43-46` | 仅**租户** | ✅ 已实现（授权偏弱） |
| POST | `/v1/pm/requirements/revisions/{rev}/migrate` | `routes.rs:247-248` | 见 `documents.rs:456` | ✅ 已实现（迁移到知识空间） |
| — | `POST /v1/pm/requirements` | — | — | ❌ **不存在**（S02-01-027） |

配套读模型：
- `GET /v1/pm/requirements/{id}/revisions` 返回 `{items,total}`，`version DESC`；**未知 requirement 不 404，返回空列表**（`pm_lists.rs:503` 注释 + `tests/pm_lists.rs:300`）
- `pm_artifacts.rs` 把需求修订并入"产物"列表（`tests/artifacts.rs:74`）——需求修订被当作交付产物看待

## 4.2 MCP 工具覆盖 `[存在缺陷]`

**MCP 工具全集**（`apps/pm-api/src/auth.rs` 与 `crates/pm-mcp/src/tools.rs` 交叉核对）：`pm_audit`、`pm_claim_task`、`pm_confirm_document_proposal`、`pm_discovery_complete`、`pm_discovery_get`、`pm_discovery_get_context`、`pm_discovery_submit_answers`、`pm_discovery_submit_findings`、`pm_discovery_submit_proposal`、`pm_get_run_context`、`pm_get_submission_status`、`pm_get_task_context`、`pm_list_tasks`、`pm_propose_document`、`pm_report_progress`、`pm_submit_document`、`pm_submit_evidence`、`pm_submit_result`、`pm_submit_technical_design`、`pm_veto_document_revision`。

**针对需求的工具：零个。**

需求在 MCP 侧**唯一**的暴露方式是 `pm_get_task_context` 返回值里的 `requirement` 字段（`crates/pm-mcp/src/tools.rs:317`，内容来自 `read.rs:47` 的 LATERAL JOIN）——即"任务上下文顺带带出需求正文"。没有列出需求、没有起草修订、没有定稿。

**S02-01-028 判定**：这是 I2-D 双入口铁律（`smark-pm-s01-i2-finding-human-confirmation-v1-1.md`）在需求领域的直接适用场景——**S02 若在 REST 侧实现需求治理而不同步 MCP 侧，将立刻产生双入口分叉**。本稿把它列为 S02 必须同批处理的约束。

MCP 侧权限机制可复用：`auth.rs:559-570` 的 `MCP_TOOL_PERMISSIONS` 映射表（工具名 → 写权限码，如 `PM_MCP_DOCUMENT_WRITE`），新增需求工具时可沿用同一模式。

## 4.3 前端

| 位置 | 状态 |
|---|---|
| `frontend/src/workspace/pages/RequirementsPage.vue` | 存在 |
| `frontend/src/shell/views/pm/requirements/index.vue` | 存在 |
| `frontend/src/api/pm.ts:35,85-104` | `listRequirements` / `listRevisions` / `createRevision` / `approveRevision` 四个客户端方法齐备 |

前端已具备"列需求 → 看修订 → 起草修订 → 定稿"的最小闭环实现。**未做交互级核验**（本轮不判交互质量）。

---

# §5. 追溯关系事实

## 5.1 Requirement ↔ Task `[已实现]` ✅

- 结构：`delivery.task_requirement`（§2.3），指向具体修订
- 写入：`executor.rs:605-618`，随 `create_tasks` 落库
- 读取：`read.rs:40-56` 任务上下文读模型，`LEFT JOIN LATERAL` 取最新关联修订正文
- 限制：**只支持"任务 → 需求"单向下钻**。没有"需求 → 任务列表"的读模型（`GET /v1/pm/requirements/{id}/tasks` 不存在）→ S02 的需求详情页想展示"这条需求由哪些任务承载"必须新增端点或复用任务列表反向过滤
- 待裁决：关联的多修订 / 改绑 / 留痕语义 → Q-17

## 5.2 Requirement ↔ Design Decision `[不存在]` ❌

系统的"决策"有两个含义，都与需求无关联：

1. **治理决策**：`governance.approval_request` + `ai.change_proposal`（`/v1/pm/decisions` 读模型，`pm_lists.rs:413-436`）。其 `target_type` 在代码中的实际取值只有 `'task'`、`'task_run'`、`'project_repository'`——**没有 `'requirement'`**（该列 `TEXT NOT NULL` 无 CHECK，扩取值零迁移成本，S02-01-039）
2. **架构决策**：`ai.project_document.doc_type='decision'`（`migrations/0006_knowledge_documents.sql:11-13`）——知识空间的文档类型，与需求之间**无外键**

**S02-01-029**：S02 场景描述里「需求 → 设计决策」的追溯链**当前完全不存在**。这直接影响 S03（分析、设计与技术决策）的衔接设计。

## 5.3 Requirement ↔ Knowledge `[部分实现]` ⚠️

- 知识侧 `ai.project_document.doc_type` 允许 `'requirement'`（`0006_knowledge_documents.sql:11-13`；V2 提案表同，`0014_knowledge_v2.sql:27-28`）
- 迁移工具 `knowledge_migrate::migrate_requirement_revision`（`knowledge_migrate.rs:16-64`）：
  - 读 `delivery.requirement_revision.content` JSONB + `requirement.code` / `title`
  - 渲染 Markdown（含 front-matter `id` / `type: requirement` / `source_events`）
  - 以 `doc_type='requirement'` 写入知识文档，`source_ref = 'requirement_revision:<id>'`
  - `operation_id = 'migrate_requirement:<id>'` 保证幂等
- 回滚 `rollback_requirement_migration`（`knowledge_migrate.rs:66-95`）按 `change_summary LIKE 'migrated from requirement_revision%'` 定位删除

**性质判定（S02-01-030）**：这是**单向的、正文搬运**，不是双向同步。
- 当前实现事实：正文由 `requirement_revision.content`（JSONB）生成知识 Markdown；知识侧是**派生产物**，不回流
- 两侧之间**没有外键**，关联靠 `source_ref` 字符串约定
- 需求定稿后，知识文档**不会自动更新**——需要再次调用迁移端点

**S02 须裁决（Q-3）**：需求正文以谁为权威。**本稿建议**（仅建议，非裁决）：以 `requirement_revision.content` 为需求领域权威、知识侧为派生表示——与 I6-B「Markdown+Git 管知识正文权威、PG 管业务状态权威」裁决同构；正式权威边界留待 S02 最终基线裁决（§14.2）。

## 5.4 Requirement ↔ Discovery Finding `[不存在]` ❌

- `discovery.discovery_finding`（`migrations/0025_discovery.sql:48-66`）列：`id, tenant_id, session_id, kind, content, confidence, verification_status, evidence, actor_role, created_at`
- ~~**无 `project_id` 列**（与 I6-A 报告的既有结论一致）→ finding 连"属于哪个项目"都不可知~~
  **【2026-11-04 勘误】本条已过期**：核验基线 HEAD `807ddae` 已含迁移 `0031`（I6-B 批），其 `:112-121` 为 `discovery_finding` 补了 `project_id`（NOT NULL + `UNIQUE(tenant_id, project_id, id)`）并把 `evidence` 列迁入独立表 `discovery_finding_evidence`（含 `repository_id` + `commit_sha` + `evidence_type='commit'`，`:128-144`）——Commit 级证据通道**已存在**。原稿只查了 `0025` 的建表 DDL、漏了 `0031` 的 ALTER，责任在本稿。本勘误同时影响 §8.1 表「无 `project_id`」行与 S02-I7 设计稿 §7.3 继承的表述。
- 无任何表把 finding 关联到 requirement

**含义**：S02 场景里「AI 协助分析…澄清问题」与「需求」之间**没有数据通道**。要么 S02 引入关联，要么需求澄清完全绕开 discovery 域另建一套——这是 S02 必须裁决的边界问题（Q-4）。

## 5.5 Requirement ↔ Requirement `[不存在]` ❌

无依赖表、无冲突表、无派生表、无追溯表。一条需求无法表达：
- 依赖于另一条需求（前置条件）
- 与另一条需求冲突
- 由另一条需求派生（拆解/细化）

对照参考：`delivery.task_dependency`（迁移 `0021`）已经为**任务**实现了依赖图，S02 若要需求级依赖，可参考该表形态。

---

# §6. 需求创建路径 `[存在缺陷]`

**当前唯一能创建需求的方式是冷启动导入**：

`ProjectCommand::CreateRequirements { requirements: Vec<RequirementDraft> }`（`pm-domain/src/command.rs:58-59`）
→ `executor.rs:398-460` `create_requirements`：
1. 校验 `code` / `title` 非空
2. **同 `code` 幂等**：已存在则跳过（`created: false`）
3. 插 `delivery.requirement`（`status='draft'`）
4. 插首版修订 `content = {title, markdown, priority}`，`state='draft'`
5. 更新 `current_revision_id` 指向首版修订（`executor.rs:459`）

**调用方仅两处**：
- `project_import/proposal.rs:224`（冷启动导入 → 提案 → 审批 → 执行）
- 测试代码

**S02-01-031 判定**：**没有"新建需求"的运行时入口**。REST 侧不存在 `POST /v1/pm/requirements`；MCP 侧无对应工具。这与 S02 的第二个用户时刻（新建需求 = 原始需求进 + 逐步澄清）**直接冲突**——S02 必须新增创建入口，这是本阶段识别出的**第一顺位新增能力**。

**关联事实**：冷启动导入路径本身有先决约束——`project_import/lifecycle.rs:58-63` 检查 `delivery.requirement` 计数非零即 `project already has requirements`（导入只允许在项目为空时进行）。这意味着**导入建的需求与运行时新建的需求会长期共存**，S02 需保证两条入口的语义一致。

**另（S02-01-040）**：`RequirementDraft`（`pm-domain/src/import.rs:83-95`）**无类型字段**——即便裁定落库 `requirement_type`，冷启动导入建的需求当前也无从携带类型（→ Q-16）。

---

# §7. 治理面：权限、审计、并发、人工确认

## 7.1 人工确认缺失 `[存在缺陷 · P0]` —— 本轮最重要的发现

**S02 的核心结果是「形成经过人类确认的需求基线」。当前代码中没有任何机制保证这一点。**

证据链（五段）：

**① 策略是黑名单模型，`approve_requirement_revision` 不在名单**

`policies/ai-pm-ai-lead-v2.yaml:6-7`：
```yaml
defaults:
  effect: auto_apply         # blacklist model: anything not matched below is auto-applied
```
红线名单（`:16-39`）共 11 项：`release_version`、`external_commitment`、`sign_acceptance`、`destructive_write`、`grant_permission`、`change_policy`、`spend_above_quota`、`create_milestones`、**`create_requirements`**、`create_tasks`、`write_documents`。

`policy_engine.rs:297-302,316-321`：
```rust
// Red line → `RequirePreConfirmation`; otherwise default effect (`AutoApply`, blacklist model).
if let Some(role) = self.required_role(command) { return PolicyDecision::RequirePreConfirmation { role }; }
...
pub fn is_redline(&self, command: &ProjectCommand) -> bool { self.red_lines.contains_key(command.kind()) }
```

**`approve_requirement_revision` 不在这 11 项里 → `PolicyDecision::AutoApply`。**

**② AI 编排的 AutoApply 分支会真的执行**

`crates/pm-ai/src/orchestrator.rs`（头部注释第 7 步）：`Auto` 且策略判定 `AutoApply` → **真实执行** `PgCommandExecutor`，`status='executed'`，带审计与通知。

**③ 执行器只在"是红线"时才拦 AI 主体**

`executor.rs:52-56`：
```rust
if self.policy.is_redline(&cmd) && ctx.actor_kind != ActorKind::Human {
    return Err(CommandError::RedLineBlocked { command: cmd.kind() });
}
```
非红线 → 不拦。AI 主体可以执行定稿。

**④ AI 侧 prompt 明确把需求定稿列为合法命令**

`crates/pm-ai/src/openai_adapter.rs:31` 的合法命令清单包含 `approve_requirement_revision`；`:34` 的 `requirement_analysis` 技能提示词定义为"分析客户原始需求、正式需求版本与关联任务，识别歧义、缺失、冲突与变更"。

**⑤ 旁证：同批次的 `create_requirements` 是配了红线的**

`create_requirements`（建需求）被列为 `required_role: project_owner` 红线；**定稿需求（`approve_requirement_revision`）却没有**。这在语义上说不通——"创建"比"定稿"更需要人类把关。

**结论（S02-01-032）**：
> 机制上，AI 可以不经人类确认直接定稿需求修订并推进 `current_revision_id`。当前 fake model 未实际产出该类提案（`crates/pm-ai/src/fake_model.rs` 中无 `approve_requirement_revision`），真实模型行为未在本仓实证——但**机制许可已经存在**，且策略缺口是确定性的（静态核验）。

**S02 裁决建议**（仅建议）：把 `approve_requirement_revision` 提升为红线（`required_role` 视项目角色模型而定），使需求基线在策略层获得"人类确认"强制力（→ Q-2）。

## 7.2 授权边界 `[存在缺陷]`

| 路径 | 校验 | 证据 |
|---|---|---|
| GET 需求列表 / 修订 | 租户 + **项目成员**（`core.project_member` EXISTS） | `pm_lists.rs:256-262, 271-277` |
| POST 起草 / 定稿 | **仅租户** | `pm_writes.rs:54-65` `requirement_in_tenant` |
| REST 全局中间件 | 仅身份层（bearer / `inject_dev_principal`）与 MCP 设备层，**无项目成员中间件** | `routes.rs:86-135` |
| 角色 / 权限码 | 需求写路径**零角色校验**；MCP 侧才有 `PM_MCP_*` 权限码机制 | `auth.rs:559-570` |

**S02-01-033**：租户内非项目成员可直接起草 / 定稿任意项目需求的修订。这是**租户内跨项目越权**，与 I2 V1.1 已识别的 G9/G10 类问题同型（S02 继承 I2 的判定口径）。运行时可否复现待 V-4。

## 7.3 审计

| 动作 | 审计动作名 | 位置 |
|---|---|---|
| 起草修订 | `requirement.propose_revision` | `pm_write/revision.rs:57-64` |
| 定稿修订 | `requirement.approve_revision` | `pm_write/revision.rs:117-124` |
| 定稿修订（执行器） | `approve_requirement_revision`（含 before/after + 补偿规格） | `executor.rs:941-953` |
| 需求创建 | 走通用命令审计 | `executor.rs` 通用路径 |
| 需求 ↔ 任务关联 | ❌ **无审计**（随 `create_tasks` 一笔带过，无独立动作名） | `executor.rs:605` |

**S02-01-034**：需求领域的审计动作词表**三套并存**（`requirement.*` 点号式 / `approve_requirement_revision` 下划线式 / 任务关联无动作），S02 建立领域基线时须统一。

## 7.4 并发

- `row_revision` 在两条路径都被 `+1`（`revision.rs:114`、`executor.rs:909`），**但两处都没有校验它**，且 REST 请求体里根本没有 `expected_revision` 字段（`pm_write/mod.rs:109-112`）→ **乐观并发字段目前是"只写不校"的装饰**（S02-01-004）
- 执行器路径靠 `FOR UPDATE` 行锁兜底；REST 路径**两者皆无**
- 任务上下文读模型用 `ORDER BY created_at DESC, version DESC LIMIT 1` 兜底多修订（`read.rs:47-53`）
- 两条路径的真实并发行为未实证（→ V-3）

---

# §8. 三个对象的边界界定

S02 必须严格区分以下三者。第一轮已提出该要求，本轮给出**代码级判据**。

## 8.1 Discovery Finding ≠ Requirement

| 维度 | Finding | Requirement |
|---|---|---|
| 载体 | `discovery.discovery_finding`（`0025_discovery.sql:48-66`） | `delivery.requirement`（`0002_delivery.sql:3-14`） |
| 归属 | `tenant_id` + `session_id`，**无 `project_id`** | `project_id` + `tenant_id` |
| 分类 | `kind ∈ ('observed','user_confirmed','speculation','contradicted')`——**代码发现事实的类型** | 无类型字段（裁定要求补，未实现） |
| 验证状态 | `verification_status ∈ ('PENDING','CONFIRMED','REJECTED','CONFLICT')` | `status ∈ ('draft','approved')` |
| 关联 | finding ↔ 仓库（`discovery_finding_repository`）、finding ↔ question | requirement ↔ task |
| 定稿人 | 无（只有 `actor_role` 文本） | `requirement_revision.approved_by` |

**判定**：Finding 是**代码扫描出的事实条目**，`kind` 描述"这条观察的性质"，`verification_status` 描述"这条观察被核对的结果"。二者与"一条被人类定稿的需求"不是同一个轴。**Finding 的 `CONFIRMED` 与需求的 `approved` 无任何代码关联**——第一轮的判断成立。

## 8.2 Knowledge Proposal ≠ Requirement

`ai.document_proposal`（`0014_knowledge_v2.sql:25-52`）是**知识文档的候选修改**，生命周期为 `draft → quality_gates_passed → redline_pending → confirmed → commit_pending → committed → rejected → rolled_back`，正文权威在 Git。

它表里 `doc_type` 的 `'requirement'` 取值（`0014:27-28`、`0006:11-13`）指的是**"这篇知识文档的类型是需求文档"**，与 `delivery.requirement` 领域对象**没有任何外键或代码关联**（除 §5.3 的单向迁移字符串）。

**判定**：把 S02 的需求塞进 `ai.document_proposal` 会导致——（a）需求生命周期被文档发布生命周期绑架；（b）需求拿不到领域 ID、拿不到 `approved_by`、拿不到与 Task 的外键关联；（c）需求的版本语义（`version` 递增 + `superseded`）与文档修订语义（`revision_no` + Git commit）互相对不齐。第一轮建议**不塞进 proposal**，本轮以代码事实支持该建议。

## 8.3 三者与 Task 的责任边界

```
Requirement  →  Design Decision  →  Task  →  Run  →  Delivery
（要什么）        （怎么定）        （做什么）   （谁执行）  （交什么）
```

当前代码里 **Task 及以后（Run / Delivery / Manifest / Review / Acceptance）是齐备的**；**Requirement 到 Design Decision 之间的桥不存在**（§5.2）；Design Decision 本身也没有独立实体，只散落在知识文档与治理提案里。

---

# §9. 资产分类总表

## 9.1 可复用（地基）

| # | 资产 | 位置 | 复用价值 |
|---|---|---|---|
| A-1 | `delivery.requirement` 实体 | `0002:3-14` | 稳定 ID、项目内 code、当前修订指针、状态列、租户化、索引 |
| A-2 | `delivery.requirement_revision` | `0002:16-34` | 三态版本机、定稿人、内容哈希、版本序列 |
| A-3 | `delivery.task_requirement` | `0002:68-80` | 需求↔任务多对多，**指向具体修订**（修订级追溯价值成立；语义待 Q-17） |
| A-4 | `current_revision_id` 当前修订指针 | `0002:36-39` | 单条需求"当前定稿版"概念的现成载体（**非项目级基线**，§14.3） |
| A-5 | `row_revision` + `content_hash` | `0002:10,26` | 乐观并发与内容指纹的现成字段（需补校验） |
| A-6 | 修订定稿 REST 链路 | `pm_lists.rs:40-46` / `pm_write/revision.rs` | 起草 + 定稿 + 审计的完整业务逻辑 |
| A-7 | 需求列表 / 修订列表读模型 | `pm_lists.rs:242-283, 503-520` | 项目级需求查询 + 修订历史 |
| A-8 | 任务上下文带需求正文 | `read.rs:40-56` | 任务执行时自动携带需求原文 |
| A-9 | 需求修订 → 知识文档单向迁移 + 回滚 | `knowledge_migrate.rs:16-95` | 需求正文的知识空间投影 |
| A-10 | 冷启动导入的需求批建 | `executor.rs:398-460` | 幂等按 code 建需求 + 首版修订 |
| A-11 | `delivery.task_dependency` 形态 | `0021` | 需求级依赖图可照抄 |
| A-12 | MCP 工具权限映射机制 | `auth.rs:559-570` | 新增需求工具的权限挂载位 |
| A-13 | 策略红线机制 | `policy_engine.rs:297-321` | 需求定稿"人类确认"的强制点 |

## 9.2 部分实现

| # | 资产 | 已有 | 缺 |
|---|---|---|---|
| B-1 | 需求状态机 | `draft` / `approved` | 拒绝、撤回、作废、归档、变更中 |
| B-2 | 修订内容结构 | JSONB + markdown | 结构不统一（两种形态）、无结构化验收条件字段 |
| B-3 | 需求正文权威 | `requirement_revision.content` + 知识 Markdown | 双表示无同步规则、真源未裁决（Q-3） |
| B-4 | 需求追溯 | → Task | → Decision / → Finding / → Knowledge 均非外键 |
| B-5 | 需求创建 | 冷启动导入 | 运行时入口缺失 |
| B-6 | 需求定稿 | 两条路径 | 权威路径未裁决，严谨度分叉 |
| B-7 | 需求行级属性 | code / title / status | 无 requirement_type / priority / created_by |
| B-8 | 前端需求页 | 列表 + 修订 + 起草 + 定稿 | 无创建、无影响分析、无需求详情聚合 |

## 9.3 仅设计（种子数据有，代码未落地）

| # | 资产 | 证据 |
|---|---|---|
| C-1 | `ai.skill_definition` 中 `requirement_analysis` / `task_analysis` / `risk_detection` / `architecture_consistency` / `scheduling_orchestration` 五个技能 | `0003_platform.sql:93-105` 种子；`skill_router.rs:11-21` 有路由；`openai_adapter.rs:34-38` 有提示词——**属于提示词与路由层，未见针对需求的专用分析服务** |
| C-2 | `ai.project_document.doc_type='requirement'` 作为需求文档类型 | `0006:11-13` 有枚举值，但无需求领域对象与之关联（§5.3） |

## 9.4 完全不存在（静态扫描无证据）

| # | 缺失能力 | 扫描方式 |
|---|---|---|
| D-1 | 需求**类型**字段与类型约束（已确认类型 functional / non_functional；实现不存在） | `delivery.requirement` 无 `requirement_type` 列；全库无需求分类表 |
| D-2 | **项目级需求基线**独立对象（`current_revision_id` 是每条需求的当前修订指针，**不是**项目级需求集合快照；当前无基线编号、快照或基线比较对象） | 无 `requirement_baseline` 表；无基线相关代码 |
| D-3 | 需求间**依赖 / 冲突 / 派生**关系 | 无对应表；全库外键扫描仅 2 处引用需求（§2.4） |
| D-4 | 需求**变更影响分析** | 无相关端点、无相关服务、无相关测试 |
| D-5 | 需求**创建** REST / MCP 入口 | 路由表无 `POST /v1/pm/requirements`；MCP 工具表无需求工具 |
| D-6 | 需求**定稿授权**模型 | 无角色校验；红线名单无 `approve_requirement_revision` |
| D-7 | 需求**拒绝 / 归档**路径 | `requirement_revision.state` CHECK 无拒绝态；无归档列、无归档端点 |
| D-8 | 需求↔**治理决策**追溯 | `change_proposal.target_type` 实际取值无 `'requirement'`（无 CHECK，扩值零迁移成本） |
| D-9 | 需求**统计 / Dashboard**读模型 | 无聚合端点（仅列表 + `count(*)`） |
| D-10 | 需求**优先级**可查询表达 | priority 只在 JSONB 内容里 |
| D-11 | 需求级**验收标准载体**（V1.1 并行工作收编） | 全库唯一验收表 `delivery.task_acceptance_criterion` 硬锚 `task_id`（`0015:25-40`）；需求侧 `grep acceptance` **零命中**，需求正文只有自由文本 markdown |
| D-12 | 需求↔**架构决策文档**关联（V1.1 并行工作收编） | `ai.project_document.doc_type='decision'` 与需求零外键；`delivery.task_knowledge_ref` 有任务→知识文档的先例，需求侧无对应表 |

---

# §10. 复用边界与新增对象候选

> 本节只列**候选与建议**，不冻结。冻结属 S02 最终基线（I 切片）阶段。裁定已锁定项标注【已裁定】。

## 10.1 复用边界

1. **不新建需求主表**【已裁定，§14.1】：`delivery.requirement` 已是合格实体，A-1~A-5 可直接继承；功能与非功能共享统一身份 / 版本 / 生命周期 / 审计。新建第二张需求表会产生双权威，直接违反裁定。
2. **修订级任务关联的追溯价值成立**：`task_requirement` 指向具体修订的设计有真实价值；但**关联语义（多修订 / 改绑 / 留痕）不预冻结**，待 Q-17 裁决（§14.3 修正：V1.0 原稿"钉修订的设计是对的，保留"表述过强）。
3. **不把需求塞进 `ai.document_proposal`**【建议，核验支持】：理由见 §8.2。
4. **需求正文权威**【待裁 Q-3】：V1.0 原稿将"正文权威归 PG"写入不动红线，V1.1 降级为**建议**（§5.3），正式权威边界留待 S02 最终基线裁决（§14.2/§14.3）。
5. **Discovery Finding 不升格为需求**【建议】：两类对象轴不同（§8.1）。若 S02 需要"从发现产出需求"，应建**显式关联**（新表），而非复用或改写 finding。
6. **不为非功能需求建第二套领域模型或第二套生命周期**【已裁定，§14.1】：按类型分表拆验收项同样被排除。
7. **不改造 `delivery.task_acceptance_criterion` 为多态锚点**【V1.1 新增建议】：该表被写入、run_context、review 的 collect/decide/queue、验收端点、验收总表共 7 处读写点绑定（§11.4 冲击 A），改成多态会同时伤到 D5 证据链与 S01 已验收的验收闭环。需求级验收标准走**平行新表**（CAND-8）。

## 10.2 新增对象候选（待 S02 最终基线裁决）

| 候选 | 用途 | 触发的缺口 | 备注 |
|---|---|---|---|
| **CAND-1 需求类型列** | `delivery.requirement` 上承载 `requirement_type ∈ ('functional','non_functional')` | D-1 | 业务裁定已确认两类（§14.1）；**字段名 / 枚举约束 / 索引形态待 Q-16 裁决**。`constraint` 未确认为类型，仅作候选（§14.2） |
| **CAND-2 项目级需求基线对象** | 需求定稿产生一个"基线"实体（编号、快照、确认人、时间），供变更影响分析引用 | D-2 | 是"需求基线"成为一等概念的必要条件；是否做、形态如何待 Q-10 |
| **CAND-3 需求关系表** | `delivery.requirement_relation(requirement_id, related_requirement_id, relation ∈ ('depends_on','conflicts_with','derived_from'))` | D-3 | 形态照抄 `delivery.task_dependency`（形态参考，非冻结） |
| **CAND-4 需求创建入口** | `POST /v1/pm/requirements` + 对应 MCP 工具 | D-5 | **第一顺位**，对应 S02 用户时刻 2；入口形态待 Q-8 |
| **CAND-5 需求 ↔ 发现关联** | `delivery.requirement_finding` 或在 finding 上加 `requirement_id` | §5.4 | 取决于 Q-4 |
| **CAND-6 决策 ↔ 需求关联（治理侧）** | 扩展 `change_proposal.target_type` 取值到 `'requirement'`（无 CHECK，零迁移成本），或建关联表 | D-8 | 与 S03 强相关（Q-5） |
| **CAND-7 需求行级补充** | `created_by` / `priority` / `status` 扩展（拒绝/归档） | D-7, D-10 | 可与 CAND-1 合并为一次迁移（迁移号并入登记表） |
| **CAND-8 需求级验收标准载体**（V1.1 收编） | 平行于 `task_acceptance_criterion` 的需求级验收项表；承载**与类型适配**的验收条件与验证方式（含但不限于指标 / 阈值 / 测量方法） | D-11 | 裁定约束③的落地前提；形态待 Q-11/Q-12/Q-13；**不可改造 `task_acceptance_criterion`**（§10.1.7） |
| **CAND-9 需求 ↔ 架构决策关联**（V1.1 收编） | 需求 → 知识空间决策文档（`doc_type='decision'`）的关联，形态参考 `delivery.task_knowledge_ref` | D-12 | 裁定约束②的落地前提；与 CAND-6 互补；形态待 Q-15 |
| **CAND-10 零 Task NFR 的验收裁决路径**（V1.1 收编） | 为无关联 Task 的需求提供验收触发与裁决入口 | §11.4 冲击 B | 形态待 Q-14；决定 S02 对 S04/S05 的接口声明 |

---

# §11. 非功能需求裁定及其影响面（V1.1 修订版）

## 11.1 裁定原文要点（用户，2026-11-03）

> **非功能需求属于正式需求，不是附属备注或项目约束。**
> 统一使用 Requirement 领域模型，通过 `requirement_type` 区分需求类型。
>
> | 需求类型 | 示例 | 验收方式 |
> |---|---|---|
> | `functional` 功能需求 | 用户登录、创建项目、任务执行 | 功能行为、业务规则、验收场景 |
> | `non_functional` 非功能需求 | P95 响应时间、安全隔离、可用性 | 性能测试、安全测试、指标阈值 |
>
> 后续可以根据实际需要细分非功能需求的分类（`performance` / `security` / `reliability` / `scalability` / `compatibility` / `maintainability` / `usability`）——**这些是分类候选，不在当前阶段提前固化枚举**。

**五条模型约束**：
1. 功能需求与非功能需求共享统一身份、版本、生命周期和审计机制。
2. 两种需求都可以关联多个 Task 和架构决策。
3. 两种需求都必须有明确的验收标准。
4. 验收标准根据需求类型采用不同验证方式。
5. 非功能需求不能因为无法直接拆成用户操作就被降级为普通备注。

## 11.2 验收模型的裁定澄清（V1.1，采纳专家修订）

对裁定约束 3/4 的准确展开（修正 V1.0 原稿"功能二值验收 / NFR 量化阈值验收"的过度简化）：

- **验收条件可以多条**——功能需求不限定为单一二值字段；一条功能需求同样允许多个验收条件与验证证据。
- **验证方式按需求性质选择**——量化指标（指标 / 阈值 / 测量方法）适用于部分 NFR（如性能）；安全、可维护性、兼容性等**不应被强制压缩为单一数值阈值**，可采用测试证据、审查准则、验证环境、证据材料和明确通过规则。
- **量化只是选项之一，不是 NFR 验收的统一形态。**

## 11.3 五条约束的代码事实映射

| 约束 | 当前代码事实 | 判定 |
|---|---|---|
| **1. 共享统一身份 / 版本 / 生命周期 / 审计** | `delivery.requirement`（稳定 UUID + 项目内 `code` + `current_revision_id` 当前修订指针）+ `requirement_revision`（三态 + `version` 序列 + `created_by` / `approved_by` + `content_hash`）+ 审计写入点**均已存在且不区分类型** | ✅ **已满足**（静态判定）。同时锁死"不新建需求主表" |
| **2. 两类需求都可关联多个 Task 和架构决策** | 需求 ↔ **Task**：✅ `task_requirement`（多对多 + 指向修订，§2.3；语义待 Q-17）。需求 ↔ **架构决策**：❌ **零关联**（§5.2 / D-12） | ⚠️ Task 侧满足、**架构决策侧需新增**（CAND-9；`change_proposal.target_type` 无 CHECK，扩值零迁移成本） |
| **3. 两类需求都必须有明确验收标准** | **需求级零载体**（D-11）——全库唯一验收表 `delivery.task_acceptance_criterion`（`0015:25-40`）`task_id NOT NULL` + FK `ON DELETE CASCADE` + `UNIQUE (task_id, ordinal)` **硬锚在任务上**；需求侧 `grep acceptance` 零命中，需求正文只有自由文本 markdown | ❌ **不满足，且当前无载体**。验收标准**不能**由任务验收项上卷：(a) 无任务的需求将没有验收项；(b) `ON DELETE CASCADE` 意味着任务删除会连带抹掉验收项，违反约束 1 的生命周期独立。**必须新建需求级载体**（CAND-8） |
| **4. 验收标准按类型采用不同验证方式** | 现有 `verification_kind TEXT CHECK IN ('manual','automated_test','artifact_exists','review')`（`0015:32-34`）为单一枚举；**无承载「指标 / 阈值 / 测量方法」或「测试证据 / 审查准则 / 验证环境 / 通过规则」的结构化字段**（S02-01-036：四取值均偏功能验证形态） | ❌ **不满足**。按 §11.2 澄清，承载层须适配多种验证形态而非仅量化（→ Q-13） |
| **5. NFR 不得被降级为普通备注** | `delivery.requirement` 行上只有 `title` 无正文（正文在 `requirement_revision.content`）；无类型列、无验收项载体 | ❌ **当前实现下 NFR 确实只能是"标题 + markdown 段落"**，与普通备注在数据层无法区分——裁定正面命中现状缺陷 |

## 11.4 三处结构性冲击

**冲击 A：需求级验收标准是全新载体，不是扩展既有表**

`delivery.task_acceptance_criterion` 对 `task_id` 的强绑定是**语义性的**：验收项的收集、裁决、展示链路全部 task 驱动——

| 环节 | 位置 | 锚定 |
|---|---|---|
| 验收项写入 | `pm_write/task.rs:280-287`（DELETE + 整体重建） | task |
| 任务上下文带验收项 | `run_context.rs:320` | task |
| 评审收集 / 裁决 / 队列 | `review/collect.rs:112`、`review/decide.rs:151,650`、`review/queue.rs:127` | task |
| 验收裁决读端点 | `GET /v1/pm/tasks/{task_id}/criterion-verdicts`（`pm_acceptance.rs:129-130`） | task |
| 验收总表 | `GET /v1/pm/acceptance-overview` | task 投影 |

（S02-01-037/038：裁决表 `acceptance_criterion_verdict` 以组合外键锚定**任务级**验收项；上述 7 处读写点全部 task 锚定。）

**结论**：需求级验收标准必须是**平行的独立载体**（新表，锚 requirement 或 requirement_revision），而非改造 `task_acceptance_criterion`——后者被评审、裁决、验收三条链路绑定，改成多态锚点会同时动到 D5 证据链与 S01 已验收的验收闭环（§10.1.7）。

**冲击 B：NFR 合法地可能关联零个 Task**

约束 2 的措辞是"**可以**关联多个 Task"——是允许，不是要求；约束 5 又明确 NFR"无法直接拆成用户操作"。推论：

> 一条非功能需求**可以合法地拥有零个 Task**（例如"P95 < 200ms"可能由架构保证而非某个具体任务承载）。因此**验收标准的验证触发不能只挂在 Task 驱动的评审链上**——否则零 Task 的 NFR 将永远无法被裁决，`awaiting_manual` 会成为永久状态。

这条推论直接约束 S02 与 S04 / S05 的边界，属于 S02 必须向后声明的接口（CAND-10 / Q-14）。

**冲击 C：验收承载层须适配类型，但不得按类型分表**

裁定约束 4 + §11.2 澄清要求验收承载适配多种验证形态。候选方案：

| 方案 | 形态 | 代价 |
|---|---|---|
| 扩展枚举 | `verification_kind` 加 `performance_test` / `security_test` / `threshold` 等 | 枚举随 NFR 细分候选持续膨胀，且装不下"指标 + 阈值"结构 |
| JSONB 承载类型相关结构 | 验收项加 `params JSONB`，按 `requirement_type` 决定形状 | 形状分叉需应用层校验；查询统计变难 |
| 按类型分表 | 功能 / 非功能两张验收项表 | 与约束 1"共享统一机制"直接冲突，**已被裁定排除**（§14.1） |

本稿不预选——列入 Q-13。

## 11.5 裁定后新增待裁项索引

Q-11 ~ Q-16（验收载体形态 / 是否随修订版本化 / 异构承载层 / 零 Task NFR 裁决路径 / 架构决策关联形态 / `requirement_type` 取值域与导入默认）——权威描述与事实依据只在 §12 登记，此处仅索引（避免两处分叉）。

## 11.6 本阶段明确不做的（裁定已排除）

| 项 | 理由 |
|---|---|
| 提前固化 NFR 细分枚举（`performance` / `security` / …） | 裁定明文「不在当前阶段提前固化枚举」 |
| 为 NFR 新建第二套领域模型或第二套生命周期 | 约束 1 明文共享统一身份 / 版本 / 生命周期 / 审计 |
| 按 `requirement_type` 分表拆验收项 | 违反约束 1 的"共享统一机制" |
| 把 NFR 塞进项目约束或 `delivery.task.description` | 约束 5 明文禁止降级为备注 |
| 扩展知识侧 `doc_type` 加 `'non_functional_requirement'` | 建议（非裁定）：知识侧是派生表示（§5.3），无需按需求类型分文档 |

## 11.7 裁定与既有 S01 成果的关系

- 裁定与 **I2（Finding 人工确认）** 相互独立：Finding 是代码发现事实（`kind`），需求是正式需求（`requirement_type`），两轴不交叉（§8.1）。
- 裁定触及 **I4（Project Goal）** 的边界：Goal 是项目级目标，NFR 是需求级正式项——不可互相替代，也不应合并为同一载体。
- 裁定与 **I5（项目总控台）** 联动：需求统计与"待人工处理排队区"需按 `requirement_type` 分组（Q-7 已含优先级表达问题）。
- 裁定**不改变** §7.1 的 P0 结论：无论功能还是非功能需求，定稿都须人类确认——非功能需求的门槛不应低于功能需求（→ Q-2）。

---

# §12. 未解决技术问题（交 S02 最终基线裁决）

| # | 问题 | 事实依据 | 影响 |
|---|---|---|---|
| Q-1 | **需求定稿的唯一权威路径是 REST 还是执行器？** 两条路径严谨度分叉（§3.3） | `pm_write/revision.rs` vs `executor.rs:809-971` | 决定 S02 所有需求写接口的形态 |
| Q-2 | **需求定稿是否提升为策略红线？** 当前非红线 → AI 可自动定稿 | `policies/ai-pm-ai-lead-v2.yaml:16-39` | 决定"人类确认"是否成立 |
| Q-3 | **需求正文权威归 `requirement_revision.content` 还是知识 Markdown？** | §5.3 双表示 | 决定迁移工具是单向派生还是双向同步 |
| Q-4 | **需求与 Discovery Finding 是否要建通道？** | §5.4 零关联 | 决定 S02 是否引入新关联表 |
| Q-5 | **需求 ↔ 决策追溯在 S02 落地还是推迟到 S03？** | §5.2 零关联 | 决定 S02/S03 的边界 |
| Q-6 | **需求状态机是否扩展（拒绝/撤回/作废/归档）？**（范围已收窄：裁定约束 1 已定两类需求共享统一生命周期，不分叉状态机，故只剩"是否补齐缺失态"） | §3.4 仅两值 | 决定"被否决的需求"如何处置 |
| Q-7 | **需求优先级如何可查询表达**——行级列 vs 保留在 JSONB vs 不做 | §2.1 无 priority 列 | 影响 Dashboard 排序 |
| Q-8 | **需求创建的运行时入口形态**——REST only 还是 REST+MCP 双入口 | D-5 + I2-D 铁律 | 决定是否触发双入口分叉 |
| Q-9 | **租户内跨项目越权如何修**（需求写路径补项目成员校验 + 角色校验） | §7.2 | 决定是否需要新权限码与新角色 |
| Q-10 | **项目级"需求基线"是否新增实体（快照/编号/比较）？** 当前仅有每条需求的 `current_revision_id` 指针，**不能视为项目级冻结基线**（§14.3） | D-2 | 决定 S02 是否有基线快照与影响分析 |
| Q-11 | **需求级验收标准载体的新表形态**——锚 requirement 还是 requirement_revision？列结构？是否参考 `task_acceptance_criterion` 的 `ordinal` / `verification_kind` 形状？ | D-11；`0015:25-40` | 裁定约束 3 的落地前提 |
| Q-12 | **需求验收项是否随修订版本化**——若绑 requirement（非修订），定稿后修改验收项是否等于绕过定稿？ | §2.2（修订无验收项列） | 裁定约束 1 与 3 的交叉 |
| Q-13 | **类型适配的验收承载层**——扩 `verification_kind` 枚举 / JSONB `params` / 指标-阈值专列；须容纳量化与非量化两种验证形态（§11.2） | §11.4 冲击 C | 裁定约束 4 的落地前提 |
| Q-14 | **零 Task 的 NFR 的验收触发与裁决路径** | §11.4 冲击 B | 决定 S02 对 S04/S05 的接口声明 |
| Q-15 | **需求 ↔ 架构决策的关联形态**——知识文档 `doc_type='decision'` 外键 / `change_proposal.target_type='requirement'` / 两者兼用 | §5.2；S02-01-039（无 CHECK，零迁移成本） | 裁定约束 2 的落地前提 |
| Q-16 | **`requirement_type` 取值域与导入路径默认**——已确认 `functional` / `non_functional`；`constraint` 是否纳入待裁；`RequirementDraft` 无类型字段，冷启动导入建的需求如何定类型 | `pm-domain/src/import.rs:83-95`；S02-01-040 | 裁定约束 1 在导入链路的落地 |
| Q-17 | **任务 ↔ 需求关联语义**——同一任务是否允许关联同一需求的多个修订？关联是否可改绑？改绑 / 重绑是否留历史？（V1.1 新增，§14.2） | S02-01-021（主键不含 `requirement_id` 的重复行隐患）；`read.rs:47-53` 静默取最新兜底 | 决定 `task_requirement` 是否需要约束变更或改绑端点 |

---

# §13. 下一阶段输入

## 13.1 本稿需要实跑复核的项（静态核验未覆盖）

> 完成后应**另附执行命令、环境、结果与失败证据**；不得在文档中预先标记通过（§14.4）。

| # | 复核项 | 方法 |
|---|---|---|
| V-1 | REST 起草→定稿→修订列表的端到端行为（版本递增、superseded 生效、指针迁移） | 连库跑 `apps/pm-api/tests/pm_writes.rs` 用例 |
| V-2 | AI 路径能否真实产出 `approve_requirement_revision` 提案并被 AutoApply 执行 | 真模型调用一次 `requirement_analysis` 场景 |
| V-3 | 需求定稿的并发行为（两条路径各并发两次，验证 §3.1 交错写入推断） | 并发压测 |
| V-4 | `requirement_in_tenant` 的跨项目越权可否复现 | 构造非项目成员身份调用定稿端点 |
| V-5 | 前端 `RequirementsPage.vue` 实际交互与数据契约一致性 | 浏览器实测 |

## 13.2 建议的 S02 后续排布

```
S02-01 需求领域资产盘点（本稿 V1.1）      ✅ 已交付（运行时核验待 V-1~V-5）
   ↓
S02-01a 运行时核验（V-1~V-5，先于场景稿——专家建议一致）
   ↓
S02-02 场景稿（用户时刻 1~N 完整枚举，含新建需求与逐步澄清）
   ↓
S02-03 最终基线（Q-1~Q-17 裁决落定）
   ↓
S02-04 I 切片设计（对齐 S01 命名法）
   ↓
S02-05 总体实施计划
```

> 注：todo.md §10.9 M12 的编号以 `S02-01a / S02-02 场景稿 / …` 登记；专家稿把"补实跑复核"编为 S02-02，两者为同一件事的编号差异，以 todo.md 为准。

## 13.3 与 S01 的关系提示

- S01 的 **I2（Finding 人工确认）** 与 S02 的**需求定稿人工确认**是**两个不同的确认**，不可互相替代（§8.1）。但两者共享"人类确认必须被机制强制"的原则与同型权限缺陷（G9/G10）。
- S01 的 **I5（项目总控台）** 需在 S02 落地后决定是否纳入需求统计（D-9）。
- S01 的 **I4（Project Goal）** 与 S02 的**需求基线**是不同层级：Goal 是项目目标，需求基线是需求集合的冻结点（若 Q-10 裁决建立）。
- S01 的 **I6-B** 迁移批次占用 `0031`；S02 若需新迁移（`0029` 号仍空缺 / `0032`），**须并入迁移登记表并遵守"一个数据库变更只有一个主责 TASK"**。

---

# §14. V1.1 修订裁定与阅读优先级

本节是 V1.0 评审后的正式修订记录。若前文仍存在与本节不一致的旧表述，以本节为准；后续进入 S02 最终基线时，仍须把"事实"与"待裁决方案"分开处理。**本稿（V1.1）是 S02-01 的事实权威稿，不是 S02 最终设计基线。**

## 14.1 已确认的用户业务裁决（不是代码已实现事实）

1. **需求统一建模**：功能需求与非功能需求属于同一 Requirement 体系，以类型区分；不另建 NFR 主表。
2. **已确认类型**：`functional`、`non_functional`。`constraint` **不是已确认类型**，任何字段枚举、候选表或实施任务不得将其写成既定要求。
3. **生命周期统一**：功能/非功能需求共享身份、修订版本、状态流转、审计，以及与 Task / Decision 的追溯机制；不因 NFR 单独拆分生命周期。
4. **验收与验证适配类型**：验收条件可以多条；验证方式按需求性质选择。量化指标适用于部分 NFR（如性能），但安全、可维护性、兼容性等不应被强制要求全部转成单一数值阈值。可采用测试证据、审查准则、验证环境、证据材料和通过规则。
5. **非功能需求不是备注**：NFR 必须作为正式 Requirement 参与修订、确认、追溯和交付验证（五条模型约束原文见 §11.1）。

## 14.2 必须保留为待裁决的设计问题

- 是否增加 `requirement_type` 列、枚举约束或类型字典，以及具体数据库取值形式（Q-16）。
- 验收条件采用 JSONB 结构、独立验收条件表，还是其他承载方式；本盘点不冻结结构（Q-11/Q-12/Q-13）。
- `constraint` 是否需要成为第三种类型；目前未获确认（Q-16）。
- `requirement_revision.content` 是否正式确认为需求正文唯一权威，以及知识 Markdown 的派生/同步策略（Q-3）。
- 是否建立项目级 Requirement Baseline 快照。`current_revision_id` 只是每条需求的当前修订指针，不能视为项目级冻结基线（Q-10）。
- 同一 Task 是否允许关联同一 Requirement 的多个 revision，以及关联变更是否保留历史（Q-17）。
- Requirement 与 Decision、Finding 的关系建模及其 S02/S03 边界（Q-4/Q-5/Q-15）。

## 14.3 对原稿（V1.0）关键误差的修正

| V1.0 表述 / 风险 | V1.1 修正 |
|---|---|
| 将"功能/非功能/约束"并列写成已确定类型（CAND-1 含 `constraint`） | 只确认 `functional` / `non_functional`；`constraint` 降为候选（Q-16） |
| 将 NFR 验收概括为量化阈值（§11 "非功能需求可量化阈值验收"） | 改为按类型选择验收条件、验证方法和证据；量化只是其中一种（§11.2） |
| 将功能需求概括为二值验收 | 允许一条功能需求有多个验收条件与验证证据（§11.2） |
| 把 `current_revision_id` 称为项目需求基线 / "当前需求基线"概念的现成载体（A-4、S02-01-003） | 更正为**单条 Requirement 的当前修订指针**；项目级基线快照仍不存在（Q-10） |
| 把 `task_requirement` 的修订引用描述成不可变（"钉修订的设计是对的，保留"） | 更正为"关联记录指向具体 revision"；是否允许改绑 / 多版本关联仍待裁决（Q-17） |
| 把静态检查结果直接标成运行时已实现（"全链路跑通并有测试覆盖"） | 统一按静态证据表达；本轮没有实际运行验证，不得据此声称端到端通过（§14.4） |
| 把 PG 正文权威写成既定裁决（§10.1.4 不动红线） | 仅记录当前实现事实与建议，正式权威边界留待 S02 最终基线裁决（Q-3） |
| 并发描述把两次读取写成目标"非 draft"（§3.1） | 更正为并发请求可能都读到目标仍为 draft，之后交错写入（§3.1） |

## 14.4 证据与结论强度约束

- 本稿是基于指定 worktree / HEAD 的静态盘点报告；没有在本轮重新 checkout、编译、运行测试、连数据库或发起真实模型调用。
- "测试代码存在"只证明测试用例或断言在仓库中，不证明本轮测试已通过。（旁证：todo.md 阶段记录载有并行会话近期全量测试通过记录——96 套件 / 712 passed——但非本稿所为，不作为本稿证据。）
- "未发现"只表示在 §0.4 所列扫描范围内未发现，不应扩大为全系统绝对不存在。
- §13 的 V-1 至 V-5 是**待执行**验证项；完成后应另附执行命令、环境、结果与失败证据，不应预先标记通过。
- 任何新增 Migration、REST/MCP API、状态字段或关系表，均属于后续设计与实施裁决，不由本盘点报告直接授权。

## 14.5 V1.1 合并说明（并行工作收编）

外部专家评审的基线是 V1.0 **原稿**；与此同时，本仓依据同一份用户裁定（五条模型约束）并行完成了约束 → 代码事实的映射核验。专家评审未覆盖该并行工作的以下内容，V1.1 一并收编（两路结论经比对**无冲突**）：

- D-11（需求级验收标准零载体）/ D-12（需求 ↔ 架构决策零关联）
- CAND-8 / CAND-9 / CAND-10（需求级验收载体 / 架构决策关联 / 零 Task NFR 裁决路径）
- Q-11 ~ Q-16（裁定新增待裁项）
- 事实 S02-01-035 ~ 040（见附录 A）
- §11.3 五条约束代码映射、§11.4 三处结构性冲击

收编时已按 §14.3 的修正口径同步校正上述内容的措辞（如约束 4 的承载层描述改为"容纳量化与非量化两种验证形态"）。

---

## 附录 A：本稿新增/更正的关键事实索引

| ID | 事实 | 位置 |
|---|---|---|
| S02-01-001 | 需求实体有稳定 UUID 与项目内唯一 code | `0002_delivery.sql:3-14` |
| S02-01-004 | `row_revision` 只写不校 | `revision.rs:114` / `executor.rs:909` |
| S02-01-012 | 修订 content 两种形态不一致 | `revision.rs:44` vs `executor.rs:427-431` |
| S02-01-019 | 需求↔任务多对多成立 | `0002_delivery.sql:68-80` |
| S02-01-020 | 关联指向具体修订 | `0002_delivery.sql:72,78` |
| S02-01-021 | 关联表主键不含 requirement_id → 可重复关联 | `0002_delivery.sql:73` |
| S02-01-022 | 全库仅 2 处外键引用需求 | 全量扫描 |
| S02-01-024 | 两套审计词汇并存 | `revision.rs:117` vs `executor.rs:941` |
| S02-01-027 | 无 `POST /v1/pm/requirements` | `pm_lists.rs:33` |
| S02-01-028 | MCP 侧零需求工具 | 工具全表 |
| S02-01-029 | 决策侧 target_type 无 `'requirement'` | `pm_lists.rs:425-436` 等 |
| S02-01-030 | 知识迁移单向、字符串关联、无自动同步 | `knowledge_migrate.rs:16-64` |
| S02-01-031 | 需求创建仅冷启动导入一条路径 | `executor.rs:398-460` |
| S02-01-032 | `approve_requirement_revision` 非红线 → AI 可自动定稿 | `policy_engine.rs:316-321` + `executor.rs:52-56` |
| S02-01-033 | 需求写路径只校验租户，不校验项目成员 | `pm_writes.rs:54-65` |
| S02-01-034 | 需求领域审计动作词表三套并存 | §7.3 |
| S02-01-035 | 需求级验收标准**零载体**；全库唯一验收表硬锚 `task_id`（NOT NULL + CASCADE + `UNIQUE(task_id, ordinal)`） | `0015_task_contract.sql:25-40` |
| S02-01-036 | `verification_kind` 四取值均偏功能验证形态，**无指标 / 阈值 / 测量方式字段** | `0015_task_contract.sql:32-34` |
| S02-01-037 | `acceptance_criterion_verdict` 以组合外键锚定**任务级**验收项 | `0026_delivery_review.sql:210-211` |
| S02-01-038 | 验收链路 7 处读写点全部 task 锚定（写入 / run_context / review×3 / 验收端点 / 总表） | §11.4 冲击 A |
| S02-01-039 | `ai.change_proposal.target_type` **无 CHECK** → 扩 `'requirement'` 零迁移成本 | `0003_platform.sql:133` |
| S02-01-040 | `RequirementDraft` 无类型字段 → 导入建的需求当前无法定类型 | `pm-domain/src/import.rs:83-95` |
