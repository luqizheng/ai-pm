# S03-00 · Technical Design & ADR Domain Asset Inventory V1.0

> **【已被取代】** 本稿为第一轮（任务书截断期，VF-01~14 可见范围）交付档案。任务书全文补齐后，权威交付物已按任务书第十部分重构为 `docs/s03/` 目录八份文件（S03-00-code-facts-report.md 为主报告），本稿仅作过程档案保留，内容与 docs/s03/ 冲突时以 docs/s03/ 为准。

## 现有技术设计、架构决策及实施基线资产代码事实核验报告

| 项 | 值 |
|---|---|
| 任务编号 | S03-00 |
| 所属阶段 | S03 — Analysis, Design & Technical Decisions |
| 文档性质 | 代码事实核验报告（只读调查产物） |
| 版本 | V1.0 |
| 日期 | 2026-11-04 |
| 核验基线 | `feat/s01-implementation` @ `3ccb42f16c757bef7a89fa75ada34a7cab23ea7d` |
| 实施权限遵守 | 未修改任何业务代码 / Migration / 配置 / 现有设计文档；未执行任何产生持久化副作用的验证 |

---

# 〇、任务书完整性与覆盖范围声明

**任务书文本在 VF-14 小节中途截断**（列表中断于"乐观锁 / Revision 唯一约束 / 审批幂等性 / 并发双批准 / 审计事件 / 操作人 / 操作时间 / 原状态"）。其后内容——含 VF-15 及以后各条、第五组（S04 交接）、交付物要求与完成标准——**未随任务书到达**。

本报告覆盖：

1. 任务书可见范围全额：第一部分（基线确认）、第二部分（VF-01~04）、第三部分（VF-05~07）、第四部分（VF-08~11）、第五部分可见小节（VF-12、VF-13、VF-14）。
2. 第二节"必须调查的代码范围"中未被 VF 条目直接覆盖的交叉面（Task/Run/Delivery 生命周期、Repository 与 Git 基准、前端、MCP 全量清单、测试盘点），作为第七部分。

**VF-15 及以后待任务书补发后增量补核**（见附录 B）。本报告不包含对缺失部分范围的任何推测。

---

# 一、代码基线（任务书第一部分）

| 项目 | 实际值 |
|---|---|
| Repository | ai-pm（origin = git@192.168.2.16:tooling/ai-pm.git） |
| Worktree | `D:\kudi-project\templates\ai-pm-s01`（主 checkout `D:\kudi-project\templates\ai-pm` 在 master，为下述分支的真子集） |
| Branch | `feat/s01-implementation`（领先 master 43 个提交，**未合并、未推送**；master 本地领先 origin/master 4 个提交，其内容全部包含于本分支） |
| HEAD Commit | `3ccb42f16c757bef7a89fa75ada34a7cab23ea7d` |
| HEAD Commit Time | 2026-11-04 09:37:34 +0800（`fix(s01): I5-E 总控台验收 + 修活动流「去重在分页之后」的次序缺陷`） |
| Working Tree Status | **不干净**。已修改 2 个跟踪文件：`crates/pm-infrastructure/src/outbox_handlers.rs`（业务代码，见下）、`docs/ui/smark-pm-s02-01-requirement-domain-asset-inventory-v1-1.md`；未跟踪 6 个文件：`docs/figures/s03-domain-model.png`、`docs/figures/s03-workspace-ui.png`、`docs/s02-i6a-auth-isolation-verification-v1.md`、`docs/ui/smark-pm-s02-i7-external-commit-adoption-review-v1.md`、`docs/ui/smark-pm-s02-i7-external-commit-adoption-v1-1.md`、`docs/ui/smark-pm-s03-01-analysis-design-technical-decisions-v1.md` |
| Latest Migration | `0034_s01_command_center_indexes.sql`（migrations/ 共 33 个文件，序列 0001~0034，**0029 空号**） |
| Rust Workspace | apps：`pm-api`、`pm-worker`、`ai-pm-worker`；crates：`pm-domain`、`pm-application`、`pm-infrastructure`、`pm-mcp`、`pm-ai` |
| Frontend Workspace | `frontend/`（Vue3 + TS；src/{api,components,lib,shell,styles,workspace}） |

**未提交业务代码改动登记**：`crates/pm-infrastructure/src/outbox_handlers.rs` 工作区版本将 TaskReadyHandler 的归档守卫从 `project_activity_tx`（无锁读，`project_gate.rs:60-75`）替换为 `lock_project_activity_tx`（`SELECT ... FOR UPDATE`，`project_gate.rs:84-99`），并补 I1-B 锁序注释（先锁 Project 再锁 Task）。该改动不写审计/通知，属并发串行化修复，**不影响本报告任何事实口径**；本报告一律以 HEAD `3ccb42f` 为准。

**未合并分支**：`feat/s01-implementation`（即本基线，未并入 master）；`prism-shell-wip`（997aeb5，历史 no-op 留档分支）。

**Migration 登记表 vs 实际文件一致性**：

- 静态（代码基线口径）：33 个文件，编号 0001~0034 连续、无重复、唯一缺口为 0029 空号（历史批次预留后撤销）。
- 运行时（部署口径）：sqlx 登记表 `_sqlx_migrations` 属具体部署。本机演示栈容器 `infrastructure-aipm-pg-1`（只读 SELECT 抽查）登记为 0001~0028 共 28 条——该栈构建于 S01 批次（0030~0034）之前，**与代码基线不同步属预期**；升级演练已有测试背书（commit `bcb169f`「迁移 0031 升级演练通过」）。
- 调查使用 Commit SHA：`3ccb42f`。本报告全部代码引用以该 SHA 为准。

---

# 二、执行摘要：十条关键结论

| # | 结论 | 对 S03 的含义 |
|---|---|---|
| K-1 | **Technical Design 领域实体不存在**。`pm_submit_technical_design` 只是把一条幂等 `technical_design_submitted` 事件写入 `integration.inbound_event` 并排队 `ai_analysis` 作业；不产生实体、版本、Git 正文、审批、需求关联（VF-05 逐环钉死） | TechnicalDesign/DesignRevision/DesignRequirement 全部需新建 |
| K-2 | **ADR 独立实体不存在**。`doc_type='decision'` 只是知识文档的枚举值之一，无接受/否决、无 supersede、无决策人字段、无与需求/设计的结构化关联（VF-08/09） | ADR/ADRRevision 需新建；decision 文档仅可作正文载体候选 |
| K-3 | **需求正文权威仍是 PG JSONB**。`delivery.requirement_revision.content`（JSONB `{"markdown":...}`）是需求正文的唯一运行时写入/读取源；Markdown+Git 权威只属知识空间（`ai.project_document`），需求正文靠 migrate 端点**单向显式镜像**到 Git（VF-02） | 任务书背景假设「正文权威已改 Markdown+Git」对需求域**不成立**；S03 设计若以 Markdown+Git 为设计正文权威，需求域与设计域将出现两种权威模型 |
| K-4 | `approve_requirement_revision` **不是 MCP 工具**，是命令通道 CommandKind；需求定稿入口共 3 个：REST 1 + 执行器命令 1 + 统一审批 decide 间接 1（VF-03） | 红线名单机制（policy yaml）而非 MCP 工具名单，才是 AI 约束的真实作用面 |
| K-5 | **AI 自动定稿需求修订当前仍可能（DEF-02 未修复）**：策略为黑名单模型（`defaults.effect: auto_apply`），红线名单 10 项不含 `approve_requirement_revision`，AutoApply 路径以 ActorKind::AiPm 真实定稿成功（VF-03 第 9 问，证据链完整） | Q4 裁决（AI 做设计人类确认）的代码前提**当前不成立**，须列为 S03 前置修复项 |
| K-6 | **真人门系统性薄弱（P0）**：api_key 令牌链路 provisioning 出 `kind='human'` principal（`auth.rs:486-494` 无条件写 'human'）；`auth_mode` 三分类存在但全库**零准入消费**；11 个确认/审批 REST/MCP 入口逐一体检全部可通过；唯一真闸=命令通道红线（executor 仅认 `Human + RedlineApproval`）（VF-13） | 「系统能够拒绝 AI 自动批准」对绝大多数入口**当前为否**；S03 的确认门禁不能依赖现状，须新建+修复 |
| K-7 | **现有审批引擎不可原样复用为通用审批**：`governance.approval_request` 经 FK+UNIQUE 硬绑 `ai.change_proposal`（1:1），无 (object_type, object_id) 多态列，Approve 语义硬编码=执行 ProjectCommand；仓库先例是**每类审批建专表**（`governance.review_approval` 平行复刻形态）（VF-10） | S03 审批（设计批准/ADR 接受）应循专表先例或新增多态列，属「扩展」而非「直接复用」 |
| K-8 | **现有 Review 是 D5 交付复核专用**（`run_kind='review'` 的 task_run + `governance.review_approval` + append-only findings），不支持任意评审对象类型、多评审人、问题关闭（VF-12） | DesignReview 需新建；可复刻其不可变/锚定/单向 pending 形态 |
| K-9 | 实施基线机制**已有可参照物但无 S03 语义对象**：`task_run.policy_snapshot_hash`（SHA-256 内容寻址快照）+ 每仓基线 commit + `review_approval` 三重锚定 + I1 归档行门闩（E-1/E-2） | ImplementationBaseline 的「不可变快照」形态在 Run 侧已有同构先例 |
| K-10 | 历史缺陷重验（当前 HEAD 静态口径）：DEF-02（AutoApply 定稿）**仍存在**；DEF-03（并发定稿丢失更新）REST 路径**仍无锁防护**（执行器路径有三重 FOR UPDATE）；DEF-04（双入口语义分叉）**仍存在**；DEF-01（非成员写）方向性成立——需求定稿 REST 仅租户校验、无项目成员校验（§3/§6/§10） | 双入口统一（正式写操作走同一应用服务）的架构约束仍未落地，S03 实体设计必须一开始就单入口 |

---

# 三、Requirement 领域现状（VF-01~VF-04）

## VF-01 Requirement 主实体

| # | 调查项 | 结论 | 证据 |
|---|---|---|---|
| 1 | 表结构 | ✅ `delivery.requirement` 8 列 | `migrations/0002_delivery.sql:3-14`；演化：tenant_id `0019:103`、UNIQUE(tenant_id,id) `0019:157`、组合 FK `0019:185,241-243`、索引 `0003:220-221` |
| 2 | 主键/业务 ID | ✅ `id UUID` PK + `code TEXT` UNIQUE(project_id, code)；按 code 幂等 | 0002:4,12；`executor.rs:414-424` |
| 3 | 项目归属 | ✅ `project_id NOT NULL`，组合 FK `(tenant_id,project_id)→core.project` | 0002:5；0019:241-243 |
| 4 | 需求类型 | ❌ 不存在。全库 `requirement_type` 0 命中；`doc_type` 四值枚举属知识文档域而非需求实体 | 搜索：requirement_type / req_type / category 全空 |
| 5 | 当前 Revision 指针 | ✅ `current_revision_id` 可空 + 组合 FK `fk_current_requirement_revision` | 0002:9,36-39 |
| 6 | 状态 | 🟡 `status TEXT DEFAULT 'draft'`，**无 CHECK 约束**；实际取值仅 draft/approved；置 approved 仅 `pm_write/revision.rs:112-120`（REST 定稿）；执行器定稿路径**不置** requirement.status（`executor.rs:908-915`）；无状态机校验 | 0002:8 |
| 7 | 创建人/更新时间 | 🟡 仅 `created_at`；**无 created_by、无 updated_at 列** | 0002:11 |
| 8 | 关联 | project（组合 FK）、requirement_revision（双向组合 FK）、task_requirement、core.principal（经 revision.created_by/approved_by）、audit_log（target_type='requirement'）、知识文档仅逻辑 source_ref 无 FK；**与 decision/discovery 零结构化关联** | 0002 全表；0014 |

## VF-02 Requirement Revision

| # | 调查项 | 结论 | 证据 |
|---|---|---|---|
| 1 | 表结构 | ✅ `delivery.requirement_revision` 11 列，state CHECK 三值 draft/approved/superseded | 0002:16-34；租户化 0019:104,158,244-246 |
| 2 | 版本编号规则 | ✅ 事务内 `max(version)+1`（REST，无并发防护）；冷启动固定 v1 | `pm_write/revision.rs:26-32`（本人复核）；`executor.rs:445-449` |
| 3 | 正文存储 | **PG JSONB 仍是权威**：写入 `json!({"markdown": ...})` | `revision.rs:47`（本人复核）；读取 `pm_lists.rs:510`、`read.rs:43` |
| 4 | Markdown 文件路径 | 🟡 单向显式镜像：`knowledge_migrate.rs:1-6`（"PG 原行不删除不改写"）；端点 `routes.rs:255-258` → `documents.rs:495-517`；Git 路径规则 `projects/<code>/<doc_type>/<key>.md` | `knowledge.rs:257-271` |
| 5 | Git Commit 关联 | ❌ revision 表**无 git 列**；Git commit sha 存在 `ai.document_revision.git_commit_sha NOT NULL`（知识域），由 `knowledge.rs::write_revision:600-618` 写入 | 0006:27 |
| 6 | content_hash | ✅ 但**双公式不一致**：REST=`sha256("{rid}:{version}:{markdown}")`；冷启动=`sha256(content_json)` | `revision.rs:34-35`（本人复核）；`executor.rs:432-433`、`knowledge.rs:690-697` |
| 7 | 是否允许覆盖 | ✅ 正文不可覆盖：全库仅 4 处 UPDATE 且只改 state/approved_by，无 content UPDATE | `executor.rs:892,901`；`revision.rs:98,105` |
| 8 | approved 不可变性 | 🟡 无 DB 级保护（无触发器/约束）；REST 有 draft 门槛（`revision.rs:93-95`，本人复核）；**执行器无 state 前置——superseded 修订可被"复活"为 approved**（DEF-04 组成部分） | `executor.rs:847-907` |
| 9 | 与 Requirement 关联 | ✅ FK `(project_id,requirement_id)` + UNIQUE(requirement_id,version) | 0002:30-33 |
| 10 | 历史读取 | 🟡 仅 REST：`GET /v1/pm/requirements/{id}/revisions`（version DESC）；MCP 无任何 requirement 工具；任务上下文携带最新锁定修订正文 | `pm_lists.rs:39-42,503-531`；`read.rs:46-51` |

**特别确认（任务书原问）**：Markdown+Git **不是**需求域的正文权威。0006 注释"authoritative body…Deliberately no column stores the body"（0006:1-5）与 0014:5-7 只适用知识空间；需求域 PG JSONB 是唯一运行时写入/读取源，Git 侧是 migrate 单向镜像。

## VF-03 Requirement 正式确认

| # | 调查项 | 结论 | 证据 |
|---|---|---|---|
| 1 | 入口清单 | REST `POST /v1/pm/requirements/{rid}/revisions/{rvid}/approve` + 命令通道 `CommandKind::approve_requirement_revision`（**非 MCP 工具**；19 个 MCP 工具中零 requirement 工具，`server.rs:73-449`） | `pm_writes.rs:235-245`；`executor.rs:53-115`（分派 :76-83） |
| 2 | Handler | REST `pm_writes.rs:235-245`；执行器 `executor.rs:53-115` | 同上 |
| 3 | Application Service | `pm_write::approve_revision`（revision.rs:77-141）；`PgCommandExecutor::approve_requirement_revision`（executor.rs:809-969）；`supervision::decide_approval`（approval.rs:8-140，仅红线提案经此执行） | — |
| 4 | 权限校验 | ❌ 无 permission code / 角色 / 项目成员校验；仅 bearer + require_active（`routes.rs:344`）+ 租户检查（`pm_writes.rs:54-66`）；`has_permission` 仅用于 /mcp 路由（`auth.rs:581,603`） | — |
| 5 | 人类身份校验 | 🟡 隐式：HTTP 端点 provision 固定 `kind='human'`（`auth.rs:486-494`，本人复核——**任何令牌链路含 api_key 都 provision 成 human**）+ service token 拒绝（`auth.rs:238-240`）；无显式 kind 断言；执行器侧 AiPm 显式放行（见第 9 项） | — |
| 6 | 并发控制 | ❌ REST 路径无防护：无锁读 state 后连发 UPDATE，无 FOR UPDATE、UPDATE 无 state 条件守卫（TOCTOU 窗口；同需求不同修订并发定稿可短暂双 approved，无唯一约束兜底）。执行器路径有三重 FOR UPDATE（executor.rs:823-855,872-880） | `revision.rs:84-120`（本人复核） |
| 7 | 审计 | ✅ `requirement.propose_revision` / `requirement.approve_revision` / `approve_requirement_revision`（含 before/after + compensation）→ governance.audit_log | `revision.rs:52-60,121-129`；`executor.rs:950-961` |
| 8 | 失败场景 | 404 not found（跨租户防探测）、409 "not a draft revision"、409 "already decided"、409 "command execution failed"；错误体仅 `{message}` 无 code | `pm_writes.rs:54-66,179-182`；`revision.rs:92-95`；`approval.rs:54-56`；`support.rs:1-34` |
| 9 | **自动确认绕过（重点）** | **能。DEF-02 在当前 HEAD 仍成立**。完整链：ai-pm-worker Orchestrator（`apps/ai-pm-worker/src/main.rs:62,199`）→ 自审门（`self_review.rs:45-78`）→ 策略黑名单模型 `defaults.effect: auto_apply`（`policy_engine.rs:31-32`）→ 红线名单 10 项**不含 approve_requirement_revision**（`policies/ai-pm-ai-lead-v2.yaml:16-39`，本人复核；`policy_engine.rs` DEFAULT_POLICY_YAML 同值）→ `PolicyDecision::AutoApply`（`orchestrator.rs:270-279`）→ `ActorKind::AiPm` 执行（orchestrator.rs:496-514）→ 执行器红线拦截仅对 is_redline 生效（executor.rs:58-60）→ **真实定稿**（executor.rs:900-915）；事后仅 `change_applied` 通知 + veto 窗口补偿（orchestrator.rs:516-544；veto.rs:26-60）；escalate 路由只发通知、不建 approval_request、无人类处理端点（orchestrator.rs:622-648） | 全链证据齐 |
| 10 | 统一决定服务关系 | 🟡 I2-C `decide_approval` 机制已实现（原子 pending→decided + 仅 `Human + RedlineApproval` 执行通道，approval.rs:39-56,92-101），但**管不到需求定稿**：仅红线提案建 approval_request（orchestrator.rs:584-620），而 approve_requirement_revision 永不入红线名单 → 永不经真人门；REST 定稿与 decide 完全独立 | — |

## VF-04 Requirement 与 Task 关联

| # | 调查项 | 结论 | 证据 |
|---|---|---|---|
| 1 | 表结构 | ✅ `delivery.task_requirement` 4 列，PK(task_id, revision_id)，三组合 FK | 0002:68-80；租户化 0019:107,253-255 |
| 2 | 关联具体 Revision | ✅ `revision_id NOT NULL` + 组合 FK `(requirement_id,revision_id)` 防跨需求串版本 | 0002:72,78-79 |
| 3 | Task 创建锁定 | ✅ 唯一写点=命令执行器：`create_tasks` 按 code 解析 `SELECT id, current_revision_id` → INSERT … ON CONFLICT DO NOTHING；REST 建任务不支持关联；解析失败记 findings 不阻断 | `executor.rs:535-546,603-617`；`task.rs:6-51` |
| 4 | 需求变化后行为 | ❌ 无失效/重校验：两条定稿路径均不触碰 task/task_requirement；无触发器；任务上下文恒读锁定集合中 created_at 最新一条 | `read.rs:46-51` |
| 5 | 历史版本引用 | ✅ revision_id 永久指向建任务时刻修订；PK 允许一任务多修订共存；无 DELETE 路径 | 0002:68-80 |

**交付结论（S03 可复用项）**：requirement/requirement_revision 实体与 `current_revision_id` 指针模式、UNIQUE(requirement_id,version)、task_requirement 的复合 FK 绑 revision 模式、superseded 语义——均为 S03 DesignRevision/DesignRequirement 的现成结构参照。**不可复用**：正文权威模型（JSONB）、确认门禁（无权限/无真人门/并发缺口）、需求状态机（无约束无校验）。

---

# 四、Technical Design 领域现状（VF-05~VF-07）

## VF-05 pm_submit_technical_design 完整追踪

**总评：仅是"提交设计摘要事件 + 排队 AI 分析"的入口；完整 Technical Design 业务流程（实体/版本/审批/关联）不存在。**

调用链（每环一层，证据到行）：

```text
1. MCP 注册     crates/pm-mcp/src/server.rs:72-91  #[tool(name="pm_submit_technical_design")]
                描述原文："Persists an idempotent 'technical_design_submitted' event and queues AI analysis"
2. HTTP 权限门  apps/pm-api/src/auth.rs:556-568    全部 /mcp 需 PM_MCP_READ；本工具额外需 PM_MCP_DOCUMENT_WRITE（:563 硬编码表）
3. 调用人上下文 crates/pm-mcp/src/caller.rs:29-46  rmcp extensions 读 pm-api 注入的 HumanActor + DeviceClaim（无 IO）
4. 真人成员校验 crates/pm-mcp/src/caller.rs:73-112  require_project_member（非成员 404）→ 解析项目 dev_agent principal 为执行主体
5. Task 归属    crates/pm-mcp/src/tools.rs:180-192  task_context（read.rs:36-71，project_id 谓词）跨项目/悬空 → NotFound
6. 入库         crates/pm-application/src/ingestion.rs:62-141（本人复核）
                单事务 INSERT integration.inbound_event(source='mcp', event_type='technical_design_submitted',
                payload={task_ref,fixture,summary}, payload_hash, ON CONFLICT(project_id,source,idempotency_key) DO NOTHING)
                + integration.job(job_type='ai_analysis')
7. 幂等语义     ingestion.rs:127-134  同键同 hash→Duplicate 回放；同键异 hash→IdempotencyConflict（tools.rs:214-217→Conflict）
8. 工具返回     crates/pm-mcp/src/tools.rs:225-234  {event_id, processing_status:"queued"|"duplicate"}
9. Job 领取     apps/ai-pm-worker/src/main.rs:1-13  领 ai_analysis job → Orchestrator
10. 分析编排    crates/pm-ai/src/orchestrator.rs:118-376  8 步：start_run(ai.analysis_run)→context(任务+最新需求修订+milestone)
                →快照→路由→模型→逐提案落 ai.change_proposal(:442-492)→自审/策略门(:248-315)→finish_run+事件状态(:317-339)
11. 技能路由    crates/pm-ai/src/skill_router.rs:12-16  technical_design_submitted → requirement_analysis /
                task_analysis / architecture_consistency（三个只读分析技能）
12. 分析产物    crates/pm-domain/src/command.rs:14-69  通用 ProjectCommand（11 种，无任何 design 命令）；
                AutoApply 经 executor 执行并写 audit（orchestrator.rs:496-579 + command_executor/audit.rs:7-48）
```

逐项回答：

| # | 调查项 | 结论 | 证据 |
|---|---|---|---|
| 1 | 注册位置 | server.rs:72-91；tools/list 共 **19 工具**由 `crates/pm-mcp/tests/tools.rs:408-431` 钉死 | — |
| 2 | Input Schema | 5 参数全必填：`project_id:Uuid / task_ref:String / idempotency_key:String / fixture:String / summary:String`——**无 requirement 关联、无 markdown 正文** | `tools.rs:41-55`（本人复核） |
| 3 | 输出 | `{event_id, processing_status}` | tools.rs:225-234 |
| 4 | 权限 | PM_MCP_READ + PM_MCP_DOCUMENT_WRITE + 真人成员校验 + dev_agent 执行主体 | auth.rs:562-568；caller.rs:73-112 |
| 5 | 项目归属校验 | task 须 project 内可见；project 不存在→NotFound | tools.rs:184-192；ingestion.rs:72-77 |
| 6 | Requirement 关联 | ❌ 参数/payload 均无 requirement id；仅分析期经 task_requirement 读最新需求修订作上下文快照 | tools.rs:206-210；read.rs:41-53 |
| 7 | 正文存储 | ❌ 不存正文：summary 只是 inbound_event.payload 里的字符串；不写 ai.project_document、不写 Git | ingestion.rs:79-110 |
| 8 | Git Commit | ❌ 本链路无任何 commit；Git 写入只在知识文档链路（knowledge.rs:572-594；提案发布 submit.rs:248），与设计无关联列 | — |
| 9 | 状态转换 | 平铺字段 inbound_event.status：received→completed_no_change/applied/needs_clarification；**无设计状态机** | 0003:15；orchestrator.rs:200-205,327-339 |
| 10 | 审批 | ❌ 无设计审批；governance.approval_request 挂的是 change_proposal（红线提案） | 0003:157-168 |
| 11 | Revision | ❌ 不创建：新幂等键=新事件行（不可变、无版本链）；同键重放=duplicate | 0003:17；ingestion.rs:98-134 |
| 12 | 审计 | ❌ 提交点不写 audit_log（事件行即事实记录）；审计发生在命令执行（audit.rs:7-48）与 run_completed（outbox_handlers.rs:239-255） | — |

**任务书关键结论题**：按名称推测其具备设计生命周期**不成立**——该工具与技术设计的全部关系只有固定 event_type 字符串 `technical_design_submitted`（tools.rs:36，本人复核）。

## VF-06 Technical Design 数据实体

| # | 调查项 | 结论 | 证据 |
|---|---|---|---|
| 1 | 独立实体 | ❌ `rg -ni "design" migrations/` = 0 命中；`TechnicalDesign/DesignRevision/DesignRequirement` 代码 0 命中 | 搜索词全列 |
| 2 | project_document 表示 | 🟡 设计正文只能以 `doc_type='architecture'` 知识文档承载（通用知识语义，非设计领域语义）。DDL：0006:7-20；doc_type ∈ ('requirement','decision','architecture','knowledge')（0006:11-13；0014:27-29；常量 knowledge.rs:19）。**无 'technical_design' 值**；表无正文列（Git 权威） | — |
| 3 | 设计版本实体 | ❌ 现存版本实体属他域：ai.document_revision（draft/published/rejected/rolled_back）、delivery.requirement_revision（draft/approved/superseded） | 0006:22-39；0002:16-34 |
| 4 | 稳定 Design ID | ❌ 未找到 | — |
| 5 | Design Revision ID | ❌ 未找到 | — |
| 6 | 当前有效指针 | ❌ 设计域无（需求域 current_revision_id 0002:9；知识域 published+MAX(revision_no) 推导 knowledge.rs:534-542） | — |
| 7 | 历史版本关系 | ❌ 设计域无 | — |
| 8 | 设计批准 | ❌ 不存在（需求批准链存在：command.rs:26-30 → executor.rs:809+） | — |
| 9 | 设计废弃 | ❌ 不存在（近似物：知识修订 veto 回滚 server.rs:218-237；需求 superseded） | — |
| 10 | 版本比较 | ❌ 后端无端点（routes.rs:145-259 全表无）；`pm_artifacts.rs:1-5` 自认"文件 Diff 后端无数据源"；前端 fixtures.ts:743-757 DiffLine 为 mock（fixtures.ts:1-8 自述纯演示）；无 DesignPage | — |

当前实际实体关系图：

```text
core.project
 ├─< delivery.requirement ──current_revision_id──> requirement_revision (draft/approved/superseded)
 │                    └─< delivery.task_requirement (task_id, revision_id) ──> delivery.task
 ├─< integration.inbound_event  ★technical_design_submitted 行 = pm_submit_technical_design 全部落库产物
 │    │                            (payload={task_ref,fixture,summary}；无正文/版本/审批/需求关联)
 │    ├─< integration.job(ai_analysis) ──> ai.analysis_run ──< ai.change_proposal (7 态)
 │    │                                      ├─< governance.approval_request（仅红线提案，1:1）
 │    │                                      └─< integration.notification（veto 窗口）
 ├─< ai.project_document (doc_type 4 值, 无 design) ──< ai.document_revision (Git 账本)
 │         └─< ai.document_proposal (8 态) / document_index_state / knowledge_reference
 ├─< delivery.task_run / task_repository_commit（执行与交付）
 └─< governance.audit_log（仅命令执行/outbox 动作时写入）

Git 知识根：projects/<project-code>/<doc_type>/<doc_key>.md（knowledge.rs:259-272），仅知识域
```

## VF-07 Design 与 Requirement 关联

| 目标模型要素 | 判定 | 证据 |
|---|---|---|
| DesignRequirement 关联表 | ❌ 不存在（关联表全清单仅 task_requirement 0002:68-80 / finding_repository 0025:67 / knowledge_reference 0014:82） | — |
| 一设计↔多需求 | ❌ 不存在（无设计实体） | — |
| 一需求↔多设计 | ❌ 不存在 | — |
| 关联绑 requirement_revision_id | 🟡 仅 Task 关联存在该模式（task_requirement.revision_id NOT NULL + 复合 FK）；设计侧无任何引用 | 0002:72,78-79 |
| 关联审计 | ❌ 不存在（仅命令执行级审计） | audit.rs:7-48 |
| 历史关联查询 | ❌ 不存在（task_requirement 存历史行但读侧只取最新） | read.rs:46-51 |
| 需求修订更新后旧设计 | ❌ 无处理逻辑（上下文恒读最新修订，旧事件不失效不重算） | read.rs:48-50 |

**与目标模型差异结论**：`TechnicalDesign → DesignRevision → DesignRequirement{requirement_id, requirement_revision_id}` 四层中，唯一已有同构先例是 `task_requirement` 的复合 FK 绑 revision 模式；其余三层全部需新建。

---

# 五、ADR 与审批现状（VF-08~VF-11）

## VF-08 decision 类型文档

| # | 调查项 | 结论 | 证据 |
|---|---|---|---|
| 1 | doc_type='decision' 存在 | ✅（仅枚举值） | 0006:11-13；0014:27-29；knowledge.rs:19；frontend/src/api/documents.ts:10-14 |
| 2 | Markdown+Git | ✅（知识域权威；0006:1-5 表无正文列） | knowledge.rs:257-271 |
| 3 | 独立 ID | 🟡 = project_document 主键 id；UNIQUE(project_id, doc_key)；无 ADR 式编号序列 | 0006:8,17 |
| 4 | 生命周期 | 🟡 project_document 本体无状态列；生命周期在 ai.document_revision.status（draft/published/rejected/rolled_back） | 0006:29-31 |
| 5 | 正式接受/否决 | 🟡 仅发布管线语义：红线确认（routes.rs:230-231 → submit.rs:282-411）+ 事后 veto（veto.rs:85-120）；**无 decision 级 accept/reject 入口** | — |
| 6 | 被新决策替代（supersede） | ❌ supersed* 仅 delivery 域（0002:24；0010:6-14,73；0011:42）；decision/文档四表零命中 | — |
| 7 | 决策人记录 | ❌ 无 decided_by 列；最近似=document_revision.redline_confirmed_by（0014:13）；决策人只能在 Markdown 正文/Git author 里 | — |
| 8 | ↔ Requirement | ❌ 无列无关联表 | — |
| 9 | ↔ Technical Design | ❌ 无设计实体；最接近的 doc_type='architecture' 同为文档类型 | — |

## VF-09 ADR 独立实体搜索（逐词报告）

| 搜索词 | 命中情况 |
|---|---|
| `adr` | 仅 docs/ 人工 Markdown（ADR-0001/0002）+ 代码注释（bootstrap.rs:250,358、authz.rs:10）+ 测试 fixture（pm-mcp/tests/contract.rs:177-178）；migrations/前端/i18n 零实体命中 |
| `architecture_decision` / `technical_decision` / `decision_record` / `alternatives` / `consequences` / `considered_options` | crates/apps/frontend/locales/migrations **全部 0 命中**（workspace-view.ts:20 的 TS `DecisionRecord` 是「决策中心」读模型=approval_request⋈change_proposal，非 ADR，已辨析） |
| `approval_request` | 存在（专用，见 VF-10a） |
| `change_proposal` | 存在（专用，见 VF-10b） |
| `document_proposal` | 存在（专用，见 VF-10c） |

**十问结论**：独立表/独立业务 ID/状态机/决策 Revision/备选方案字段/决策理由（ADR 维度）/后果记录/替代关系（supersedes）/跨设计引用/独立查询审计——**全部不存在**。

## VF-10 现有审批能力（四类审批严格区分）

### a) governance.approval_request（0003:157-168 + 0019 租户化）

- DDL：id、project_id、`proposal_id UUID NOT NULL UNIQUE REFERENCES ai.change_proposal(id)`（**1:1 硬绑**）、status default 'pending'（无 CHECK）、required_role、decided_by、decision_reason、decided_at；0019:374-375 复合 FK。
- 状态机：`pending → approved | rejected`（守卫 UPDATE...WHERE status='pending'，supervision/approval.rs:39-56）。
- 创建：pm-ai orchestrator.rs:589-598（仅 AI 提案判 RequirePreConfirmation 时）；裁决入口 `POST /api/v1/supervision/approvals/{id}/decision`（routes.rs:150-151）+ `POST /v1/pm/approvals/{id}/decide`（pm_writes.rs:74）。
- Approve 语义硬编码=反序列化 proposed_data 为 ProjectCommand 执行（approval.rs:89-113）→ proposal 'executed'；Reject → proposal 'vetoed'。
- **无对象类型字段**：只能挂 change_proposal；间接对象集=target_type ∈ {task, requirement, milestone, project, audit_log}（command_target，orchestrator.rs:695-728，源自 pm-domain/src/command.rs:14-69）。

### b) ai.change_proposal（0003:128-154）

- DDL：target_type/target_id/expected_revision/command_type/proposed_data(JSONB)/rationale/evidence/risk_level/confidence + status CHECK **7 态**。
- 状态机逐态触发点：`proposed`（orchestrator.rs:440-477）→ `executed`（AutoApply :271-277 / approve 后 approval.rs:104-113）｜`redline_pending`（:602）｜`escalated`（:630，**无审批行仅通知+veto**）｜`vetoed`（approval.rs:60 等）｜`failed`（:316-318、approval.rs:125）｜`expired` **仅 DDL:146 存在，代码零写入**。executed 后否决走补偿链不改状态（supervision/mod.rs:8-20）。
- 用途：审批 AI 变更命令（任务/需求修订/里程碑/红线命令/回滚/冷启动批量写），与文档无关。

### c) ai.document_proposal（0014:21-51）

- DDL：document_id 可空、doc_type 4 值、proposed_markdown（候选正文非第二权威，0014:5-7）、base_revision_id、quality_gate_result、redline_kind、redline_confirmed_by/at、resulting_revision_id、status CHECK **8 态** default 'draft'。
- 八态 vs 代码实际：`redline_pending`（submit.rs:140-146）｜`commit_pending`（同处）｜`rejected`（同处+rejection_reason）｜`committed`（finalize_committed，veto.rs:286，回填 document_id+resulting_revision_id）｜`rolled_back`（veto.rs:107，伴随 revision 'rolled_back' + Git revert + 指针回退）；**`draft` / `quality_gates_passed` / `confirmed` 三态无代码写入路径**（仅 DDL+展示常量 command_center.rs:64-70；红线确认只写 redline_confirmed_by/at，submit.rs:361-368，状态直跳 committed）。
- 发布管线：质量门过且无红线→同请求内平台执行 Git commit（submit.rs:242-249）→ published 修订→索引重建；红线不写 Git 待人工 confirm；Goal 文档强制红线（:103-110）；入口 routes.rs:225-235 + MCP tools.rs:73-127。

### 补充发现 d) governance.review_approval（0026:223-249）

交付复核审批专表：kind ∈ ('auto_accept_gate','manual_review','review_degraded')，status pending/approved/rejected，锚定三元组（review run + delivery run + manifest_hash）once-written + pending 单向迁移触发器（0026:273-296）；写链 review/decide.rs:460-498，裁决 decide.rs:508-610（锚定回读校验+E2 复核）。

### 四类审批区分表（任务书原问）

| 类别 | 结论 | 挂哪个实体 |
|---|---|---|
| 1. 文档发布审批 | ✅ 已存在 | ai.document_proposal 红线管线 + 事后 veto |
| 2. 普通变更审批 | ✅ 已存在（限 AI 提案红线条目） | governance.approval_request 1:1→change_proposal（escalated 无审批行）；交付侧另有 governance.review_approval |
| 3. 技术设计批准 | ❌ 不存在 | 无设计实体；architecture 文档仅走第 1 类管线 |
| 4. ADR 正式接受 | ❌ 不存在 | 无 ADR 实体；decision 无 accept/reject/supersede/决策人 |

**复用判断（任务书原问）**：**实体不可原样复用；形态可复制（先例=每类审批建专表）**。理由：

1. approval_request 被 FK+UNIQUE 锁死 change_proposal（0003:160-161；0019:218,374-375），无 (object_type, object_id) 多态列；
2. Approve 语义硬编码=执行 ProjectCommand（approval.rs:89-101），换对象即换语义——设计批准/ADR 接受是「批准状态」语义而非「执行命令」语义；
3. 仓库先例：review_approval 平行新建专表、复刻同形态（0026）；
4. 可复用资产：pending 守卫单语句迁移（approval.rs:39-56）、DB 触发器版不可变约束（0026:273-296）、required_role/decided_by/reason/at 字段形状、审计写入（pm_write/audit.rs）、通知流（integration.notification 0003:40-49）、quality gate（knowledge_quality.rs）。

## VF-11 ADR 与 Requirement / Design 关联

- **ADR↔Requirement：不存在**（无列无表；多需求关联/具体 Revision 绑定/替代后历史保留——四项全不支持）。
- **ADR↔Technical Design / Design Revision：不存在**（设计实体本身缺失）。
- decision 侧间接痕迹：正文质量门要求含 `source_events:` 或 `projects/` 引用（knowledge_quality.rs:152-159）；ProposalRequest.source_ref 仅内存标记不落库（knowledge_proposal/mod.rs:38-39；knowledge_quality.rs:99-110）。
- 唯一落库引用设施 `ai.knowledge_reference`（0014:82-104）方向**相反**：analysis_run/event/manual → 引用文档+修订+章节（reference_kind ∈ evidence/decision/constraint）；修订历史永久保留（0006:35），引用可指向历史修订——可作 S03 反向引用（设计/ADR→需求）的参照而非现成能力。

---

# 六、评审与权限治理（VF-12~VF-14）

## VF-12 现有设计评审能力

**总评：仓库的 Review 是 D5 交付复核专用，非通用设计评审。**

| # | 调查项 | 结论 | 证据 |
|---|---|---|---|
| 1 | 独立 Review 实体 | 🟡 无独立 review 表；评审= `delivery.task_run` 行 `run_kind='review'`（0026:15-23）+ 专表 governance.review_approval（0026:223-248）；编排 review/mod.rs:1-57；REST pm_reviews.rs:33-43 | — |
| 2 | 评审对象任意挂载 | ❌ 锚定仅 delivery run 逐仓 commit+验收条目（review/anchor.rs:21-36、queue.rs:82-117）；无 target_type/subject_type 多态 | — |
| 3 | 绑定 Revision | 🟡 绑定内容寻址哈希（reviewed/input_manifest_hash + policy_version，anchor.rs:93-129），非 revision 实体；条目修订→哈希漂移拒收（collect.rs detect_anchor_drift） | — |
| 4 | 多位评审人 | ❌ 单一平台评审者 REVIEWER_ROLE="reviewer"（queue.rs:30）executor_kind='platform'（queue.rs:33）；无评审人子表；人工侧单 decided_by | 0026:236 |
| 5 | 多轮评审 | ✅ attempt_no 按 kind 分轨唯一索引（0026:62-67）；uq_task_review_active（0026:77-79）；重审=新 run 新行 | — |
| 6 | 问题登记 | ✅ delivery.review_finding（0026:165-186，severity info/warning/blocker）；reject 理由转 finding（decide.rs:681-705） | — |
| 7 | 问题关闭 | ❌ finding 无状态列、append-only 触发器（0026:265-266）；resolved/closed 零命中 | — |
| 8 | 专家意见与用户裁决分离 | 🟡 平台三值 verdict（0026:22-23）vs 人工 decided_by（0026:203-204 awaiting_manual+回填）；非「专家 vs 用户」双角色双字段设计 | — |
| 9 | 重新提交 | 🟡 状态机 ChangesRequested→Ready（task_transition.rs:36）；再交付→新 DeliverySubmitted→新 review run（outbox_handlers.rs:147-185）；无显式 resubmit API | — |
| 10 | 历史评审意见保留 | ✅ finding/verdict 禁 UPDATE 触发器（0026:257-268）；报告列 once-written（0026:300-318）；审批行锚定不可变+pending 单向（0026:273-296） | — |

## VF-13 正式审批权限

**1. 角色体系**【✅】：`core.project_member.role CHECK IN ('owner','member','ai_pm','architect','developer','tester')`（0030:7-13）；代码域模型 ProjectMemberRole{Owner,Member}/is_manager（pm-domain/project_member.rs:14-41）——**DB 六角色 vs 代码两档（manager/非 manager）存在收窄**；租户 TenantRole{Owner,Admin,Member}（tenant.rs:19-58）；消费点：require_project_member（authz.rs:33-57，仅成员资格）、caller_is_manager（project_member.rs:174-208）、grant 仅 human（project_member.rs:265）。

**2. Permission Code**【🟡】：定义=JWT `permissions` claim（auth.rs:107-135），**仓库内无权威清单**；消费全集仅 `PM_MCP_READ`（auth.rs:556）+ `PM_MCP_DOCUMENT_WRITE`（auth.rs:562-568，覆盖 5 个 MCP 写工具含 confirm/veto）；**REST 审批/确认/定稿端点零 permission code 校验**；`PM_MCP_TASK_UPDATE` 已定义未分配（auth.rs:561）。

**3. 审批人身份验证**【🟡】：三层中间件 require_bearer→resolve_principal→require_active（routes.rs:341-353）；review-approvals decided_by=认证 principal+成员校验（pm_reviews.rs:105-118）；**`/api/v1/supervision/*` actor_id 取自 body**（routes.rs:464,410,500-537；mod.rs:70,108）且 handler 不填充认证身份（mod.rs:73-75）；**required_role（policy yaml）无裁决人角色比对消费点**（policy_engine.rs:300-322 只用于标记需预确认）。

**4. MCP 调用人上下文**【✅】：`caller_from_ctx(&RequestContext<RoleServer>)` 读 extensions→http Parts→HumanActor/DeviceClaim（pm-mcp/caller.rs:29-46）；HumanActor 唯一生产点=鉴权层（auth.rs:178-186,247-248）。

**5. REST/MCP 是否共享校验**【🟡 分叉仍在】：服务层共享（confirm_redline_proposal/veto_revision/decide_approval 同一 application service），但 MCP 丢弃客户端 confirmed_by 改用认证人（tools.rs:502-504）而 REST 透传 body.confirmed_by/vetoed_by（documents.rs:286,311）；supervision decide 双 REST 入口一个用认证人（pm_writes.rs:130）一个用 body.actor_id（routes.rs:464）；校验强度不一（supervision /api 版无成员/租户校验，approval.rs:24-32）。

**6/7. 真人门逐入口体检表（AI 持 api_key 能否过）**：

背景事实：auth_mode 三分类仍在 auth.rs:121-131，但**全库零准入消费**（仅写入 audit 列 0018:25-28）；真人门原语=principal.kind='human'（support.rs:4-17、discovery/web.rs:199-209）；**provisioning 把任何令牌链路（含 api_key）都落成 kind='human'**（auth.rs:486-494，本人复核）。

| 入口 | 范围校验 | 身份来源 | 真人门 | AI(api_key) 能否过 |
|---|---|---|---|---|
| POST /v1/pm/review-approvals/{id}/decision | 项目成员（pm_reviews.rs:105） | 认证 principal | 无 | **能**（→任务可 completed，decide.rs:596-613） |
| POST /v1/pm/approvals/{id}/decide | 仅租户（pm_writes.rs:85-91） | 认证 principal | 无 | **能** |
| POST /api/v1/supervision/approvals/{id}/decision | 无（approval.rs:24-32） | **body.actor_id** | 无 | **能，可冒充** |
| POST /api/v1/supervision/veto | 无 | body.actor_id | 无 | **能** |
| POST /api/v1/supervision/freeze、unfreeze | 无 | body.actor_id | 无 | **能** |
| POST /v1/pm/documents/proposals/{id}/confirm | 项目成员（documents.rs:277） | **body.confirmed_by**⚠ | require_human | **能**（自报者亦 kind='human'） |
| POST /v1/pm/documents/revisions/{id}/veto | 项目成员（documents.rs:302） | **body.vetoed_by**⚠ | require_human | **能** |
| MCP pm_confirm_document_proposal | 成员+双权限码 | 认证 caller（客户端值丢弃 tools.rs:504） | require_human | **能** |
| MCP pm_veto_document_revision | 同上 | 认证 caller（tools.rs:527） | require_human | **能** |
| POST discovery findings/{fid}/verification | 项目成员（pm_discovery.rs:299） | 认证 principal | kind='human' 二次校验（web.rs:205） | **能** |
| POST requirements/{rid}/revisions/{rvid}/approve | 仅租户（pm_writes.rs:60-65） | 认证 principal | 无 | **能** |
| 命令通道红线自我执行 | — | — | policy RequirePreConfirmation（policy_engine.rs:301-303）+ executor 仅认 Human+RedlineApproval（approval.rs:86-101） | **不能**（唯一真闸，仅覆盖命令通道） |

**任务书特别验证题**：「AI Agent 尝试批准 Design Revision → 系统拒绝」——**验证对象不存在（N/A）**：无 Technical Design Revision 实体与批准入口（VF-06）；技术上最接近的路径是技术设计提交→change_proposal→红线审批（上表第 2/3 行，api_key 可过）。需求修订定稿是另一对象（VF-03）。**当前系统只能拒绝「服务主体（dev_agent/ai_pm）」与「命令通道红线自我执行」；持 api_key 的 agent 以自己 human 类 principal 可通过全部 11 个确认/审批入口。**

**8. 越权风险点清单**：

- R1（P0）REST confirm/veto 身份冒充：body.confirmed_by/vetoed_by 直通（documents.rs:286,311）；
- R2（P0）supervision 遗留端点 actor_id 自报 + 无租户/成员过滤 + 审计回退写冒认身份（routes.rs:410/464；approval.rs:13-17,24-32；veto.rs:182）；
- R3（P0）MCP 与 REST token 同源（同一 JWT/JWKS，auth.rs:190-220），auth_mode 零消费 → 真人 key 经 MCP 使用不可区分；
- R4（P1）auth-off 模式 dev-off owner 后门（auth.rs:299-359；默认 jwks，main.rs:44）；
- R5（P1）required_role 无裁决人比对；
- R6（P2）MCP confirm 与 submit 共用 PM_MCP_DOCUMENT_WRITE 一个权限码。

## VF-14 并发和审计

**1. 乐观锁**【✅】：delivery.task/milestone.revision + requirement.row_revision（0002:10,47,60）；apply_task_transition 行锁+expected_revision 校验+revision+1（transition.rs:34-115）；executor.rs:297,1010 RevisionConflict；REST 任务编辑 revision 不匹配 409（pm_write/task.rs:100-109）；FOR UPDATE 使用点 30+ 处（executor.rs:136,281,825,849,874,991；transition.rs:58；project_member.rs:135-517；discovery/*；outbox.rs:112 SKIP LOCKED 等，清单存 scratch）。

**2. Revision 唯一约束清单**【✅】：requirement_revision UNIQUE(requirement_id,version)+UNIQUE(requirement_id,id)（0002:30-31）；project_document UNIQUE(project_id,doc_key/canonical_path/id)（0006:17-19）；document_revision UNIQUE(document_id,revision_no/id)（0006:35-36）；change_proposal UNIQUE(analysis_run_id,proposal_seq)（0003:151）+ approval_request.proposal_id UNIQUE（0003:160）；租户化 UNIQUE(tenant_id,id)（0019:157-167）；uq_task_run_attempt_delivery/review、uq_task_run_active、uq_task_review_active、uq_review_approval_pending（0026:62-79,250-251）；uq_task_run_source_event（0012:29）；inbound_event UNIQUE(project_id,source,idempotency_key)（0003:17）；outbox_event UNIQUE(tenant_id,idempotency_key)（0019；写入 ON CONFLICT DO NOTHING，outbox.rs:48-52）。**注意：无「同需求仅一 approved 修订」约束**——并发双 approved 无 DB 兜底。

**3. 审批/决定幂等性**【✅】：I2-C operation_id→inbound_event ON CONFLICT+重放零写入（web.rs:169-197）+同值 no-op（web.rs:228-237）；decide_review_approval 第二次→Ok(AlreadyDecided) 200（decide.rs:519-534）；supervision 原子 UPDATE WHERE status='pending'（approval.rs:39-56）→200 already:true（routes.rs:481-485）；confirm/veto 走 operation_id 账本（submit.rs:301-359、veto.rs:28-37）。

**4. 并发双批准（DEF-03 重验）**【🟡 主路径已防护、需求定稿残留】：review_approval=行锁+锁后状态重读+DB 触发器三重（decide.rs:520-534；0026:273-296）→输家 AlreadyDecided；supervision=原子 WHERE 守卫。**残留**：REST approve_revision SELECT 无 FOR UPDATE、UPDATE 无 state 条件（revision.rs:84-120，本人复核）→不同修订并发定稿有 TOCTOU 窗口（可短暂双 approved，无唯一约束兜底）；同修订重复定稿 409。执行器路径有三重 FOR UPDATE（executor.rs:823-880）。

**5. 审计事件体系**【✅】：audit_log 完整 DDL=0003:188-202（含 before_data/after_data/compensation）+0004（source_kind/ref）+0018:25-28（actor_identity_id/client/auth_mode）+0027（claimed_device_id）+0019（tenant_id）+0005:30-31 INSERT 触发器；写入=业务事务直写（command_executor/audit.rs:26；review/queue.rs:360-387 source_kind='review'；pm_write/audit.rs:48-102 等）+ 经 outbox 的 RunCompletedHandler（advisory lock+条件 INSERT 幂等，outbox_handlers.rs:219-284）；outbox 注册事件=TaskReady/DeliverySubmitted/RunCompleted（has_handler=true）+ReviewCompleted/KnowledgeBaseWriteRequested（登记不产，outbox.rs:100-131；装配 outbox_handlers.rs:298-306）。

**6. 操作人/操作时间/原状态**【🟡】：actor 四列齐（0003:191+0018+0027）；created_at 默认 now（0003:201）；before_data 列存在且有写入点（任务迁移 before={status,revision}，transition.rs:114-115；finding.decided 带 before，web.rs:294-306），但 review 审计（queue.rs:368-385）、pm_write（audit.rs:48-102）、RunCompleted（outbox_handlers.rs:238-250）只写 after——**口径不统一**。

---

# 七、交叉面事实（任务书 §2.1 剩余范围）

## E-1 Task / Run / Delivery 生命周期

1. **Task**：`delivery.task`（0002:51-66）status 13 值 CHECK（0010:149-153）+ superseded_by_task_id/attempt_count/consecutive_failure_count/retry_exhausted（0010）；`revision BIGINT` 乐观锁；关联表 task_requirement(0002:68)/task_acceptance_criterion/task_knowledge_ref(0015:25,42)/task_dependency(0021:9)/task_waiting(0010:90)/task_supersession(0010:73)。
2. **状态机**：TaskStatus enum 13 值（pm-domain/task.rs:15-29）；终态 Completed/Cancelled/Failed/Superseded（:33-38）；allowed_edges 30 条边（task_transition.rs:32-66）+ 守卫（等待解除/E1 交付已提交/E2 验收证据/Running→Failed 须 B4-A..D/取代 intent，:73-164）；域层无 I/O。
3. **Run**：`delivery.task_run`（0010:18-41；0011 补租约/回收 13 列+7 值状态 CHECK；0020 补 agent_id/triggered_by_user_id/executor_id；0022 补 integration_status/request_fingerprint；0023 补 lease_generation/deadline_at；0026 补 run_kind/review_verdict 六列；0028 补 execution_mode）。领取=条件 UPDATE→claimed+租约+generation+1（run_dispatch/claim.rs:646-673）；回收/隔离（lease.rs:61-90,140-167 recover_lost_run 新建重试 Run，supersedes_run_id）。
4. **Delivery**：task_repository_commit（0022:51-81，UNIQUE(task_run_id,repository_id)，FK RESTRICT）；视图 v_task_delivery_manifest（0022:105-181；0026:88-159 重建）四态 delivery_state=delivered/partial/never_submitted（计算值非任务状态字段）。
5. **Review 位置**：Delivery 之后 Completed 之前；run_kind='review' 行即报告头；review_finding/acceptance_criterion_verdict append-only（0026:265-268）；governance.review_approval 三重绑定 pending→approved/rejected 单向、同 task 至多一个 pending（0026:250-251）。
6. **execution_mode**（0028:15-17）：'human'（真人 MCP 领取）/'spawn'（Worker 无人值守）；写入点 claim.rs:640-644。
7. **归档（I1 已实施）**：project_gate 行门闩（lock_project_activity_tx FOR UPDATE，锁序 project→run→task，project_gate.rs:7-10,84-99）；归档事务 archive.rs:52-174（锁行→archived→P1 条件取消 queued→N-2 导入终态化/名额释放→审计→提交；P2 事务外逐条 cancel_run）；统一取消协议 run_executor.rs:330-381（先落终态+撤凭据后杀进程）；外部入口 project_lifecycle.rs:33-36,113-158（cancel）/:29-32,46-104（archive），挂载 routes.rs:169。

## E-2 Repository 与 Git 基准机制

1. **仓库表**：integration.project_repository（0008:18-32，UNIQUE(project_id,url)，role 四值）；status('pending','verified','unreachable','archived')/head_commit_sha/ssh_key_id（0009:14-21）；SSH 一项目一把。
2. **导入状态机**：repo_import 10 值 queued→cloning→reading→analyzing→draft_ready→proposed→applied｜rejected/failed/cancelled（0009:31-34），每项目一个进行中；0031 v2：草案版本表（generation_status 三值）、source 重建（head_commit_sha **write-once** 0031:68）、attempt 表（0031:88）、project_initialization_operation 单 active（0031:42-43）、仓库有历史引用 RESTRICT 不可删（0031:77-78）。
3. **Run 仓库基线**：task_run.policy_snapshot_hash（0032:15，SHA-256 内容寻址）+repository_policy_snapshots（0010:35）；claim 事务内逐仓写 task_run_repository（base_ref/base_commit/workspace_path，0011:108-121）；repo_access.rs:61-92（Task 范围∩verified，**单一 read_write 档位**——0033 撤下六能力授权表）；hash=repo_access JSON SHA-256（claim.rs:701-726 + repo_access.rs:99-102）；执行期判定=不可变快照上界（snapshot_allows，缺失 deny-all）∩现状（execution_allows：未归档+仍 verified，repo_access.rs:110-170）。
4. **知识空间 Markdown+Git**：正文权威=Git，PG 只存身份/path/修订号/commit sha/hash（knowledge.rs:1-8）；根=AI_PM_KNOWLEDGE_ROOT 或 `<cwd>/../ai-pm-knowledge`（:142-150）；目录规则 projects/<project-code>/<doc_type>/<doc_key>.md（:259-272）；**I4-F 四知识写入口全部过 ensure_project_active_for_write→lock_project_activity_tx 行锁门禁**：①create_document（knowledge.rs:303,守卫:389）②submit_proposal（submit.rs:40,守卫:162）③confirm_redline_proposal（submit.rs:282,守卫:340）④finalize_committed（veto.rs:205,守卫:226）；并发经 KnowledgeWriteCoordinator（project advisory lock + operation_id 幂等账本 ai.knowledge_write_operation 0024:43 + base_revision 校验）。
5. **document_proposal↔Git commit**：平台进程内 git CLI 执行 commit（write_and_commit knowledge.rs:572-596 / write_revision :600-621 / revert_commit :625-637）；质量门三分岔——rejected／redline_pending（红线不写 Git 等人工确认）／commit_pending（干净提案自动提交→finalize_committed 落 published 修订）；Goal 任何提案强制红线（submit.rs:103-110）。

## E-3 前端现状

1. workspace 清单：console/（六区块总控台）、pages/（Activity/Artifacts/Decisions/Delivery/Devices/Execution/Goals/Knowledge/Models/Requirements/Team）、focus/、layout/、registry/、shell/、components/（AICommandBar/RunContextPanel）、nav-config.ts（classic 9 项/ai 6 项双模式）、fixtures.ts（演示数据）。
2. 相关页面：requirement=pages/RequirementsPage.vue（**纯 fixtures**）；真需求页=shell/views/pm/requirements/index.vue。**design=未找到**（无设计视图/页面/api/design.ts；搜索词 grep -ri design workspace+views/pm 仅命中样式）。decision=pages/DecisionsPage.vue+decisions-model.ts（真模式读 listDecisions/decideApproval）。approval=shell/views/pm/approvals/index.vue+pages/KnowledgePage.vue（提案确认/否决）。review=pages/DeliveryPage.vue+api/acceptance.ts+api/review.ts（写路径复用 review-approvals decision 端点）。
3. 路由：**纯静态**——shell/router/index.ts:14-19 只挂 staticRoutes；DB 菜单只驱动壳菜单可见性；整棵 /workspace/:projectId/* 在 routes.ts:66-179 静态注册。

## E-4 MCP Tools 全量清单（19 个，注册处仅 crates/pm-mcp/src/server.rs）

| # | 工具 | server.rs 行 | 功能 | 写 | S03 相关度 |
|---|---|---|---|---|---|
| 1 | pm_submit_technical_design | :72 | 提交技术设计（幂等 ingestion 事件+AI 分析） | 写 | **是（设计提交）** |
| 2 | pm_get_submission_status | :93 | 读事件处置状态+proposals+analysis run | 否 | 是 |
| 3 | pm_get_task_context | :113 | 读任务上下文（含最新需求修订） | 否 | 是 |
| 4 | pm_list_tasks | :133 | 列 ready 任务+最新 queued run | 否 | 是 |
| 5 | pm_submit_document | :155 | 创建 Git 知识文档（文件+commit+修订） | 写 | **是（需求/决策直创）** |
| 6 | pm_propose_document | :176 | 提案走质量门：干净自动 commit/红线挂起 | 写 | **是（设计/决策提案主通道）** |
| 7 | pm_confirm_document_proposal | :198 | 人工确认红线→写 Git（require_human） | 写 | **是（唯一 MCP 化人工确认门）** |
| 8 | pm_veto_document_revision | :218 | 否决：git revert+修订 rolled_back | 写 | **是（决策否决）** |
| 9 | pm_discovery_get | :239 | 读 discovery 会话 | 否 | 弱 |
| 10 | pm_discovery_get_context | :259 | 汇总 /pm-learn 上下文 | 否 | 弱 |
| 11 | pm_discovery_submit_findings | :279 | 提交分析发现（仅采集） | 写 | 弱 |
| 12 | pm_submit_evidence | :299 | 给 finding 附加锚定证据 | 写 | 弱 |
| 13 | pm_discovery_submit_answers | :320 | 问题/答案采集 | 写 | 弱 |
| 14 | pm_discovery_submit_proposal | :339 | discovery→propose_document 链 | 写 | 是 |
| 15 | pm_discovery_complete | :361 | 关闭采集轮 COMPLETED/REVIEW_REQUIRED | 写 | 弱 |
| 16 | pm_claim_task | :381 | 领取：一次性 lease token+锁每仓基线 | 写 | 参考 |
| 17 | pm_get_run_context | :404 | 读执行契约（基线/验收标准/策略快照） | 否 | 参考 |
| 18 | pm_report_progress | :426 | 心跳续租；首个 started 推进 running | 写 | 参考 |
| 19 | pm_submit_result | :448 | 交付：平台校验 commit=HEAD+基线后代，落清单，任务→submitted | 写 | 参考 |

无独立"审批/评审 MCP 工具"；知识红线确认是唯一 MCP 化人工确认。

## E-5 测试盘点（只读，未运行）

| 域 | 测试文件 | 数量 | 测什么 |
|---|---|---|---|
| requirement/revision | apps/pm-api/tests/pm_writes.rs | 13 | 草稿追加+hash、定稿晋升+superseded、非 draft 冲突、任务乐观锁 |
| 同上 | pm-application/tests/command_executor.rs | 13 | 定稿晋升+补偿、AI redline 拒绝、stale revision 乐观锁冲突 |
| **并发定稿** | — | **0** | 无并发 approve 同一 revision 专项 |
| approval 管线 | pm-application/tests/review.rs | 14 | D5 全链：auto_accept/human 门/changes_requested/immutability/审计幂等 |
| 同上 | pm-application/tests/supervision.rs | 8 | veto 回滚/补偿、approve 恰一次、freeze/unfreeze |
| decision 文档 | pm-application/tests/knowledge_v2.rs | 10 | 自动 commit/红线待确认/非人拒确认/veto 回滚/索引回退 |
| 同上 | apps/pm-api/tests/documents.rs + doc_write.rs | 7+3 | CRUD 回路/重复创建不加 commit/漂移 stale/项目隔离 |
| Goal | tests/project_goal.rs + project_goal_archive.rs | 3+5 | 强制红线+人工确认、stale base、归档竞争永不发布 |
| task_requirement | tests/bootstrap_read.rs + command_batch.rs | 5+4 | 读取映射与行数断言；无独立守卫套件 |
| 审批权限/真人门 | apps/pm-api/tests/mcp_permission.rs | 9 | MCP 权限门分层 |
| 同上 | tests/run_dispatch.rs | 24 | 并发领取唯一、租约回收/隔离、审批证据门 block claim |
| audit/outbox | tests/outbox_ops.rs + audit_stream.rs + outbox 系列 + audit_actor_coverage.rs | 7+4+40+2 | 事件结构/接线/claim/handler/worker/actor 覆盖 |
| 状态机 | task_transition.rs 内联 + tests/state_transition_guard.rs | 16+4 | 30 边矩阵/E1/E2/B4/取代/等待守卫 |
| Run/交付 | run_executor/run_context/task_dependency_concurrency/delivery 等 | 11+11+9+24 | 孤儿 reconcile/契约映射/图锁并发/D4 逐仓校验 |
| 归档/快照 | project_archive/run_scope_snapshot/repo_access_enforcement/project_delivery/acceptance | 5+4+8+4+3 | I1 归档/快照哈希回读/执行期门/交付视图/验收端点 |
| MCP | tools/contract/caller_ctx/hosts/discovery | 6+6+7+7+6 | 工具契约/调用者/Host 白名单 |

**无测试背书的缺口**：需求修订并发定稿（0）；task_requirement 独立守卫套件；execution_mode 写入/查询专项；设计 ingestion E2E 仅 e2e_scenarios.rs 10 例间接覆盖。

---

# 八、S03 目标模型差异分析与复用清单

## 8.1 五领域对象逐一对照

| S03 目标对象（总体设计 V1.0） | 当前代码事实 | 差异定性 |
|---|---|---|
| TechnicalAnalysis | `ai.analysis_run`（AI 分析运行，orchestrator start_run/finish_run）+ `integration.inbound_event` + `ai.change_proposal`；分析产物是提案而非独立"分析报告实体"；无与需求/设计的结构化绑定 | 机制存在、领域对象不存在 → **新建实体，复用 analysis_run/提案机制** |
| TechnicalDesign + DesignRevision | ❌ 全部不存在（VF-06）；知识侧 doc_type='architecture' 文档 + document_revision Git 账本是唯一载体候选 | **新建**；结构参照=requirement_revision（version/指针/superseded）+ document_revision（git_commit_sha/revision_no/content_hash/状态） |
| DesignRequirement 关联（含 requirement_revision 绑定） | ❌ 不存在；唯一同构先例=task_requirement 复合 FK 绑 revision（0002:68-80） | **新建**，直接照抄 task_requirement 形态 |
| ADR + ADRRevision | ❌ 全部不存在（VF-09 十问全否）；decision 仅 doc_type 枚举值，无接受/否决/supersede/决策人 | **新建**；decision 文档可作正文载体候选但须补决策语义层 |
| DesignReview | ❌ 通用设计评审不存在；D5 交付复核（run_kind='review'+review_approval+append-only findings）不支持任意对象/多评审人/问题关闭 | **新建**；形态参照=review_approval 锚定+不可变触发器+多轮 attempt_no |
| ImplementationBaseline（实施准入基线） | ❌ S03 语义对象不存在；同构先例充分：task_run.policy_snapshot_hash SHA-256 快照（0032:15）、task_run_repository 每仓基线、review_approval 三重锚定、uq_review_approval_pending、I1 归档行门闩 | **新建**；「不可变快照」形态有同构先例可复刻 |

## 8.2 三分类清单（任务书 §一 目标 3/4/5）

**可直接复用（不重建）**：

1. 需求实体族：requirement / requirement_revision / current_revision_id 指针 / UNIQUE(requirement_id,version) / superseded 语义 / task_requirement 复合 FK 绑 revision。
2. 知识空间设施：Markdown+Git 权威 + ai.project_document/document_revision 账本 + 八态提案管线 + 质量门（knowledge_quality.rs）+ 红线确认/veto + KnowledgeWriteCoordinator（advisory lock + operation_id 幂等 + base_revision 校验）+ I4-F 四写入口行锁门禁。
3. 审批形态资产：pending 守卫单语句迁移、DB 触发器不可变约束、required_role/decided_by/decision_reason/decided_at 字段形状、多轮 attempt_no 分轨唯一索引。
4. 并发资产：乐观锁模式（revision/row_revision + FOR UPDATE 清单）、outbox SKIP LOCKED、幂等账本三件套（idempotency_key/operation_id/request_fingerprint）。
5. 审计体系：audit_log 全列（含 before_data/after_data/compensation/actor 四列/auth_mode）、INSERT 触发器、outbox RunCompletedHandler。
6. AI 约束机制：policy yaml 红线黑名单 + RequirePreConfirmation + executor「仅 Human+RedlineApproval」执行通道（唯一真闸）。
7. 生命周期门禁：project_gate 行门闩 + I1 归档事务 + Run 快照/执行期双段判定（snapshot_allows deny-all + execution_allows）。

**需扩展（有基础但缺口明确）**：

1. 审批设施多态化或专表化（K-7 四条理由）；对象类型枚举点（command_target，orchestrator.rs:695-728）只能扩「可执行命令」。
2. 真人门覆盖面：11 个入口逐一补人类身份强校验 + auth_mode 准入消费（当前零消费）+ api_key provisioning 身份降级（kind 或独立档位）。
3. 需求定稿并发防护：REST 路径补 FOR UPDATE/UPDATE state 守卫/「同需求至多一 approved」部分唯一索引。
4. 双入口收口：REST approve_revision 与执行器 approve_requirement_revision 语义统一（state 前置、status 回写、审计 before/after 口径）。
5. AutoApply 红线名单：approve_requirement_revision（及 S03 新确认命令）纳入红线或独立人类确认约束。
6. 审计 before_data 口径统一（review/pm_write/RunCompleted 只写 after）。
7. REST 端点 permission code 体系（当前零校验，仓库无权威清单）。
8. required_role 裁决人比对消费。

**需新建（确实不存在）**：

1. TechnicalDesign 实体 + DesignRevision + 当前指针 + 历史。
2. DesignRequirement 关联表（绑 requirement_revision_id）。
3. ADR 实体 + ADRRevision + accept/reject/supersede/决策人/备选方案/后果字段 + 与 Requirement/Design 的结构化关联。
4. 通用/设计评审对象（多评审人、问题登记+关闭、专家意见与裁决分离、重新提交）。
5. 实施基线对象与 Q5 准入门禁（设计批准+ADR 已解决+范围/验收齐备的判定与快照）。
6. 设计版本比较（diff）后端与前端（当前 diff 全 mock）。
7. 设计/ADR 的前端页面与 MCP/REST 读写入口（当前唯一相关入口=pm_submit_technical_design ingestion 事件）。

---

# 九、与历史资料/既有结论的偏差更正

| # | 历史口径 | 当前事实（HEAD 3ccb42f） | 证据 |
|---|---|---|---|
| 1 | 「Markdown+Git 已成为唯一正文权威」（VF-04 背景假设） | 仅知识空间成立；**需求域 PG JSONB 仍是唯一运行时权威**，Git 为单向显式镜像 | revision.rs:47 vs 0006:1-5；knowledge_migrate.rs:1-6 |
| 2 | 「approve_requirement_revision 不在红线名单（MCP 工具口径）」 | 它不是 MCP 工具，是 CommandKind；MCP 19 工具零 requirement 工具；"不在红线名单"结论本身成立 | server.rs:73-449；policy yaml:16-39 |
| 3 | 「MCP 18 tools」 | 当前 19 个（tools.rs 测试钉死） | pm-mcp/tests/tools.rs:408-431 |
| 4 | 「pm_submit_technical_design 存在但生命周期待查」 | 已查明：纯 ingestion 事件+AI 分析排队，无设计生命周期（VF-05 十二环） | — |
| 5 | 「decision 文档 ≠ ADR 实体」（VF-16/17 旧结论） | 成立且已细化：无接受/否决/supersede/决策人/结构化关联 | VF-08/09/11 |
| 6 | 「S03 新迁移自 0035 起可用（0029/0031/0032 不可直接用）」 | 现状：最大 0034，0029 空号；下一可用编号=0035（事实陈述，非实施决定） | migrations/ 清单 |
| 7 | S02-02 DEF-01/02/03/04（运行时发现） | 静态重验：DEF-02 成立；DEF-03 REST 路径成立（执行器路径有三重 FOR UPDATE 属新增事实）；DEF-04 成立；DEF-01 方向性成立（定稿仅租户校验无成员校验；未运行时复测） | VF-03/§6 |
| 8 | 「I6-A：auth_mode 三分类已存在但零准入消费」 | 仍成立（auth.rs:121-131；消费仅 audit 列） | auth.rs:121-131；0018:25-28 |
| 9 | 「document_proposal 八态管线」 | 八态中 draft/quality_gates_passed/confirmed 三态无代码写入路径（死状态）；发布成功≠批准的判断成立 | 0014:38-41；submit.rs/veto.rs 触发点清单 |

---

# 十、风险登记（代码事实级，非实施建议）

| 编号 | 级别 | 风险 | 证据 |
|---|---|---|---|
| RSK-1 | P0 | api_key 令牌链路 provisioning 出 kind='human' principal，且 auth_mode 零准入消费 → 确认/审批入口身份不可区分 | auth.rs:486-494；auth.rs:121-131 |
| RSK-2 | P0 | AutoApply 黑名单模型下 AI 可自动定稿需求修订（DEF-02） | policy yaml:6-39；orchestrator.rs:270-279,496-514 |
| RSK-3 | P0 | supervision 遗留端点 body.actor_id 自报 + 无租户/成员校验 + 审计写冒认身份 | routes.rs:410/464；approval.rs:13-17,24-32 |
| RSK-4 | P0 | REST confirm/veto 透传 body.confirmed_by/vetoed_by（身份冒认入审计） | documents.rs:286,311 |
| RSK-5 | P1 | 需求定稿 REST 并发 TOCTOU（无锁+无 state 守卫+无双 approved 唯一约束兜底） | revision.rs:84-120；0002:30-31 |
| RSK-6 | P1 | 执行器定稿无 state 前置：superseded 修订可复活为 approved（DEF-04） | executor.rs:847-907 |
| RSK-7 | P1 | REST 审批/确认/定稿端点零 permission code 校验；仓库无权限码权威清单 | routes.rs:341-353；auth.rs:556-568 |
| RSK-8 | P1 | 审计 before_data 口径不统一（三处只写 after） | queue.rs:368-385；pm_write/audit.rs:48-102；outbox_handlers.rs:238-250 |
| RSK-9 | P2 | required_role 无裁决人比对；escalated 无审批行仅通知；document_proposal 三死态；change_proposal expired 死态 | policy_engine.rs:300-322；orchestrator.rs:630；0014:38-41；0003:146 |
| RSK-10 | P2 | 需求实体 status 无 CHECK 无状态机；无 created_by/updated_at；content_hash 双公式不一致 | 0002:8,11；revision.rs:34-35 vs executor.rs:432-433 |

---

# 十一、验证方法与局限

1. **方法**：静态核验（migration DDL → 代码写读路径 → 测试背书三层），五路并行调查 + 关键承重点五处本人复核（revision.rs 定稿全文、auth.rs provisioning、policy yaml 红线名单、tools.rs 输入 schema、ingestion.rs 入库事务）。每条结论附 file:line；找不到即写「未找到」并列搜索词。
2. **未运行项**：未运行任何测试套件（避免持久化副作用与并行会话干扰）；并发行为结论（DEF-03 残留窗口等）为**静态代码路径判断**，未做运行时复现；运行时唯一抽查=本地演示栈 DB 的 `_sqlx_migrations` 只读 SELECT（登记表属部署产物）。
3. **工作区状态**：核验期间工作区含 1 处未提交业务代码改动（outbox_handlers.rs 锁序修复，不涉本报告事实）；docs 类未跟踪文件为并行会话交付物。
4. **多基线说明**：主 checkout（master@2933db8）为本基线真子集，故本报告事实对 master 同样成立（除 0030~0034 迁移与 S01 实施内容外无差异）。
5. **原始调查笔记**：五组详细核验底稿存于调查工作目录 `s03-00-scratch/`（group-A~E 共 5 份，仓库外），本报告为收敛后的权威版本。

---

# 附录 B：待补发范围

任务书 VF-14 之后内容（含 VF-15 起各条、第五组 S04 交接、交付物与完成标准）未到达。补发后将按同一方法增量补核并在本报告升版（V1.1）。当前可预告的已知待核区（仅基于既有记忆线索，未调查）：S04 交接面（Task/Run 契约对设计产物的消费点）、迁移号占用复核细则、以及任务书或有的第六部分及以后。

---

*报告完。所有代码引用以 HEAD `3ccb42f16c757bef7a89fa75ada34a7cab23ea7d` 为准。*
