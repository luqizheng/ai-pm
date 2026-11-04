# S03-00 · 领域资产清单（Domain Asset Inventory）V1.1

> V1.1：收编 R1（K-3 偏差口径）/R3（DEF-02 关联 S02）/R4（不可变性三层表述）/R6（审批路线边界）——修订记录见 [S03-00-v1.1-revision-log.md](S03-00-v1.1-revision-log.md)。

覆盖任务书 VF-01~VF-12。基线 `feat/s01-implementation` @ `3ccb42f`。标签：✅已实现｜🟡部分实现｜❌不存在。

---

## 1. VF-01 Requirement 主实体

| # | 调查项 | 结论 | 证据 |
|---|---|---|---|
| 1 | 表结构 | ✅ 8 列 | migrations/0002_delivery.sql:3-14；演化 0019:103,157,185,241-243；索引 0003:220-221 |
| 2 | 主键/业务 ID | ✅ id UUID PK + code TEXT UNIQUE(project_id,code)；按 code 幂等 | 0002:4,12；executor.rs:414-424 |
| 3 | 项目归属 | ✅ project_id NOT NULL + 组合 FK(tenant_id,project_id) | 0002:5；0019:241-243 |
| 4 | 需求类型 | ❌ 全库 requirement_type 0 命中 | 搜索词：requirement_type/req_type/category |
| 5 | 当前 Revision 指针 | ✅ current_revision_id 可空+组合 FK fk_current_requirement_revision | 0002:9,36-39 |
| 6 | 状态 | 🟡 status TEXT DEFAULT 'draft' 无 CHECK；实际仅 draft/approved；置 approved 仅 REST 定稿（pm_write/revision.rs:112-120）；执行器定稿不置（executor.rs:908-915）；无状态机校验 | 0002:8 |
| 7 | 创建人/更新时间 | 🟡 仅 created_at；无 created_by/updated_at | 0002:11 |
| 8 | 关联 | project/requirement_revision/task_requirement/core.principal(经 revision.created_by,approved_by)/audit_log(target_type='requirement')；知识文档仅逻辑 source_ref 无 FK；decision/discovery 零结构化关联 | 0002/0014 |

## 2. VF-02 Requirement Revision

| # | 调查项 | 结论 | 证据 |
|---|---|---|---|
| 1 | 表结构 | ✅ 11 列，state CHECK 三值 draft/approved/superseded | 0002:16-34；租户化 0019:104,158,244-246 |
| 2 | 版本编号 | ✅ 事务内 max(version)+1（无并发防护）；冷启动固定 v1 | pm-write/revision.rs:26-32（本人复核）；executor.rs:445-449 |
| 3 | 正文存储 | **PG JSONB 为当前运行时正文读写源【与 S02 正式裁决存在架构偏差（K-3）：已裁决目标=Markdown+Git 唯一权威、PG 管业务 ID/版本引用/审批/状态/审计/事件、PageIndex 为可重建派生索引】**：`json!({"markdown":...})`；读取 pm_lists.rs:510、read.rs:43 | revision.rs:47（本人复核） |
| 4 | Markdown 路径 | 🟡 单向显式镜像（"PG 原行不删除不改写"）；端点 routes.rs:255-258→documents.rs:495-517 | knowledge_migrate.rs:1-6 |
| 5 | Git commit 关联 | ❌ revision 表无 git 列；git_commit_sha 存在知识域 ai.document_revision（0006:27），由 knowledge.rs:600-618 写入 | — |
| 6 | content_hash | ✅ 双公式不一致：REST=sha256("{rid}:{version}:{markdown}")；冷启动=sha256(content_json) | revision.rs:34-35（本人复核）；executor.rs:432-433；knowledge.rs:690-697 |
| 7 | 覆盖路径 | 🟡 应用层：已检索写路径未发现正文覆盖操作（仅 4 处 UPDATE 且只改 state/approved_by）；数据库层：尚无正文不可变保护（无触发器/无约束）——目标要求两层共同保障（R4） | executor.rs:892,901；revision.rs:98,105 |
| 8 | approved 不可变 | 🟡 无 DB 保护（无触发器/约束）；REST 有 draft 门槛（revision.rs:93-95）；执行器无 state 前置——superseded 可复活为 approved | executor.rs:847-907 |
| 9 | 关联 | ✅ FK(project_id,requirement_id)+UNIQUE(requirement_id,version) | 0002:30-33 |
| 10 | 历史读取 | 🟡 仅 REST GET /v1/pm/requirements/{id}/revisions（version DESC）；MCP 无 requirement 工具；任务上下文带最新锁定修订 | pm_lists.rs:39-42,503-531；read.rs:46-51 |

**特别确认（K-3 架构偏差）**：当前代码中需求正文唯一运行时写入/读取源=PG JSONB（0006:1-5 的"Git 权威"注释仅适用知识空间表）。**该实现与 S02 正式裁决不一致**——已裁决目标架构：Markdown+Git 为需求正文唯一权威；PostgreSQL 负责业务 ID、版本引用、审批、状态、审计和事件；PageIndex 为可重建派生索引。S03 不得重新裁决需求正文权威，也不得把当前 PG JSONB 实现提升为目标权威。

## 3. VF-03 Requirement 正式确认

| # | 调查项 | 结论 | 证据 |
|---|---|---|---|
| 1 | 入口清单 | REST POST /v1/pm/requirements/{rid}/revisions/{rvid}/approve + 命令通道 CommandKind::approve_requirement_revision（**非 MCP 工具**，19 工具零 requirement） | pm_writes.rs:235-245；executor.rs:53-115(分派:76-83)；server.rs:73-449 |
| 2 | Handler | REST pm_writes.rs:235-245；执行器 executor.rs:53-115 | — |
| 3 | Application Service | pm_write::approve_revision（revision.rs:77-141）；PgCommandExecutor::approve_requirement_revision（executor.rs:809-969）；supervision::decide_approval（approval.rs:8-140，仅红线提案间接） | — |
| 4 | 权限校验 | ❌ 无权限码/角色/项目成员校验；仅 bearer+require_active（routes.rs:344）+租户检查（pm_writes.rs:54-66） | — |
| 5 | 人类身份校验 | 🟡 隐式：provision 固定 kind='human'（auth.rs:486-494——**api_key 亦产 human**）+service token 拒绝（auth.rs:238-240）；无显式 kind 断言 | — |
| 6 | 并发控制 | ❌ REST 无防护：无锁读 state 后连发 UPDATE（无 FOR UPDATE、UPDATE 无 state 守卫）→TOCTOU；执行器有三重 FOR UPDATE（executor.rs:823-880）；无「同需求仅一 approved」约束 | revision.rs:84-120（本人复核） |
| 7 | 审计 | ✅ propose_revision/approve_revision/approve_requirement_revision（含 before/after+compensation） | revision.rs:52-60,121-129；executor.rs:950-961 |
| 8 | 失败场景 | 404 防探测/409 not a draft/409 already decided/409 command execution failed；错误体无 code | pm_writes.rs:54-66,179-182；approval.rs:54-56；support.rs:1-34 |
| 9 | AutoApply 绕过 | **能（DEF-02 成立——S02 已识别的未完成治理缺陷，沿用原编号、关联 S02-I6-A 既有治理记录；治理要求=AutoApply 不得绕过正式人类确认，S03 不依赖该路径）**：defaults auto_apply（policy_engine.rs:31-32）→红线 10 项不含 approve_requirement_revision（policies/ai-pm-ai-lead-v2.yaml:16-39，本人复核）→AutoApply（orchestrator.rs:270-279）→ActorKind::AiPm 执行（:496-514）→真实定稿（executor.rs:900-915）；事后仅 veto 窗口补偿；escalate 只通知不建审批行 | 全链证据 |
| 10 | 统一决定服务关系 | 🟡 I2-C decide_approval 仅覆盖红线提案（orchestrator.rs:584-620）；该命令永不入红线→永不经真人门；REST 定稿与 decide 独立 | approval.rs:39-56,92-101 |

## 4. VF-04 Requirement 与 Task 关联

| # | 调查项 | 结论 | 证据 |
|---|---|---|---|
| 1 | 表结构 | ✅ 4 列，PK(task_id,revision_id)，三组合 FK | 0002:68-80；租户化 0019:107,253-255 |
| 2 | 绑 Revision | ✅ revision_id NOT NULL+复合 FK 防跨需求串版本 | 0002:72,78-79 |
| 3 | 创建锁定 | ✅ 唯一写点=执行器 create_tasks（按 code 解析 current_revision_id→INSERT ON CONFLICT DO NOTHING）；REST 建任务不支持关联；解析失败记 findings 不阻断 | executor.rs:535-546,603-617；task.rs:6-51 |
| 4 | 需求变化后行为 | ❌ 无失效/重校验；两条定稿路径均不触碰 task_requirement；无触发器；读侧恒取最新 | read.rs:46-51 |
| 5 | 历史引用 | ✅ revision_id 永久指向建任务时刻；无 DELETE 路径 | 0002:68-80 |

## 5. VF-05 pm_submit_technical_design 完整追踪

**总评：仅是"提交设计摘要事件+排队 AI 分析"入口；完整 Technical Design 业务流程不存在。**

调用链（12 环）：

```
 1. MCP 注册     pm-mcp/server.rs:72-91  描述："Persists an idempotent 'technical_design_submitted' event and queues AI analysis"
 2. HTTP 权限门  pm-api/auth.rs:556-568  /mcp 需 PM_MCP_READ；本工具另需 PM_MCP_DOCUMENT_WRITE(:563 硬编码)
 3. 调用人上下文 pm-mcp/caller.rs:29-46  rmcp extensions→HumanActor+DeviceClaim（无 IO）
 4. 真人成员校验 pm-mcp/caller.rs:73-112  require_project_member（非成员 404）→解析项目 dev_agent 为执行主体
 5. Task 归属    pm-mcp/tools.rs:180-192  task_context（read.rs:36-71）跨项目/悬空→NotFound
 6. 入库         pm-application/ingestion.rs:62-141（本人复核）单事务 INSERT inbound_event(source='mcp',
                 event_type='technical_design_submitted',payload={task_ref,fixture,summary},payload_hash,
                 ON CONFLICT(project_id,source,idempotency_key) DO NOTHING)+job(ai_analysis)
 7. 幂等语义     ingestion.rs:127-134  同键同 hash→Duplicate 回放；异 hash→IdempotencyConflict
 8. 工具返回     tools.rs:225-234  {event_id, processing_status}
 9. Job 领取     ai-pm-worker/main.rs:1-13
10. 分析编排     pm-ai/orchestrator.rs:118-376  8 步：start_run→context(任务+最新需求修订+milestone)→快照→
                 路由→模型→逐提案落 ai.change_proposal(:442-492)→自审/策略门(:248-315)→finish_run
11. 技能路由     skill_router.rs:12-16  →requirement_analysis/task_analysis/architecture_consistency（只读）
12. 分析产物     pm-domain/command.rs:14-69  通用 ProjectCommand（11 种，无 design 命令）；执行写审计
```

| # | 项 | 结论 | 证据 |
|---|---|---|---|
| 2 | Input Schema | 5 参数全必填：project_id/task_ref/idempotency_key/fixture/summary；**无 requirement id、无 markdown 正文** | tools.rs:41-55（本人复核） |
| 3 | 输出 | {event_id, processing_status} | tools.rs:225-234 |
| 4 | 权限 | PM_MCP_READ+PM_MCP_DOCUMENT_WRITE+真人成员+dev_agent 主体 | auth.rs:562-568；caller.rs:73-112 |
| 5 | 项目归属 | task 须 project 内可见；project 不存在→NotFound | tools.rs:184-192；ingestion.rs:72-77 |
| 6 | Requirement 关联 | ❌ 参数/payload 均无；仅分析期读最新修订作上下文 | tools.rs:206-210；read.rs:41-53 |
| 7 | 正文存储 | ❌ 不存正文；不写 project_document、不写 Git | ingestion.rs:79-110 |
| 8 | Git Commit | ❌ 本链路无 commit（Git 写仅在知识链路 knowledge.rs:572-594；submit.rs:248） | — |
| 9 | 状态转换 | inbound_event.status 平铺：received→completed_no_change/applied/needs_clarification；无设计状态机 | 0003:15；orchestrator.rs:200-205,327-339 |
| 10 | 审批 | ❌ 无设计审批 | 0003:157-168 |
| 11 | Revision | ❌ 不创建（新键=新不可变事件行；同键回放） | 0003:17；ingestion.rs:98-134 |
| 12 | 审计 | ❌ 提交点不写 audit_log（事件行即事实） | — |

## 6. VF-06 Technical Design 数据实体

| # | 项 | 结论 | 证据 |
|---|---|---|---|
| 1 | 独立实体 | ❌ rg -ni "design" migrations/=0；TechnicalDesign/DesignRevision/DesignRequirement 代码 0 命中 | — |
| 2 | project_document 承载 | 🟡 仅 doc_type='architecture'（通用知识语义）；doc_type 4 值无 technical_design；表无正文列 | 0006:7-20；0006:11-13；0014:27-29；knowledge.rs:19 |
| 3 | 版本实体 | ❌（现存版本实体属他域：document_revision/requirement_revision） | 0006:22-39；0002:16-34 |
| 4 | 稳定 Design ID | ❌ | — |
| 5 | Design Revision ID | ❌ | — |
| 6 | 当前指针 | ❌ 设计域无（需求域 0002:9；知识域 published+MAX(revision_no) 推导 knowledge.rs:534-542） | — |
| 7 | 历史版本关系 | ❌ | — |
| 8 | 批准 | ❌（需求批准链存在：command.rs:26-30→executor.rs:809+） | — |
| 9 | 废弃 | ❌（近似物：知识修订 veto 回滚 server.rs:218-237；需求 superseded） | — |
| 10 | 版本比较 | ❌ 后端无端点（routes.rs 全表无）；pm_artifacts.rs:1-5 自认无数据源；前端 DiffLine 全 mock（fixtures.ts:1-8,743-757） | — |

实体关系图与 K-1 结论见主报告图 1。

## 7. VF-07 Design 与 Requirement 关联

| 目标模型要素 | 判定 | 证据 |
|---|---|---|
| DesignRequirement 关联表 | ❌（关联表全清单仅 task_requirement 0002:68-80/finding_repository 0025:67/knowledge_reference 0014:82） | — |
| 一设计↔多需求 | ❌ 无设计实体 | — |
| 一需求↔多设计 | ❌ | — |
| 绑 requirement_revision_id | 🟡 仅 task_requirement 存在该模式 | 0002:72,78-79 |
| 关联审计 | ❌ 仅命令执行级审计 | audit.rs:7-48 |
| 历史关联查询 | ❌ 读侧只取最新 | read.rs:46-51 |
| 需求更新后旧设计 | ❌ 无处理逻辑 | read.rs:48-50 |

## 8. VF-08 decision 类型文档

| # | 项 | 结论 | 证据 |
|---|---|---|---|
| 1 | doc_type='decision' | ✅（仅枚举值） | 0006:11-13；0014:27-29；knowledge.rs:19；frontend documents.ts:10-14 |
| 2 | Markdown+Git | ✅（知识域权威） | 0006:1-5；knowledge.rs:257-271 |
| 3 | 独立 ID | 🟡 =project_document 主键；UNIQUE(project_id,doc_key)；无 ADR 式编号 | 0006:8,17 |
| 4 | 生命周期 | 🟡 本体无状态列；在 document_revision.status 4 值 | 0006:29-31 |
| 5 | 接受/否决 | 🟡 仅发布管线语义（红线确认 submit.rs:282-411+事后 veto veto.rs:85-120）；无 decision 级 accept/reject | — |
| 6 | supersede | ❌ supersed* 仅 delivery 域（0002:24；0010:6-14,73；0011:42） | — |
| 7 | 决策人 | ❌ 无 decided_by；最近似 redline_confirmed_by（0014:13） | — |
| 8 | ↔Requirement | ❌ 无列无表 | — |
| 9 | ↔Technical Design | ❌ 无设计实体 | — |

## 9. VF-09 ADR 独立实体搜索（逐词）

| 搜索词 | 命中 |
|---|---|
| adr | 仅 docs/ 人工 Markdown（ADR-0001/0002）+注释（bootstrap.rs:250,358、authz.rs:10）+测试 fixture（contract.rs:177-178）；实体零命中 |
| architecture_decision / technical_decision / decision_record / alternatives / consequences / considered_options | crates/apps/frontend/locales/migrations 全 0 命中（workspace-view.ts:20 DecisionRecord=决策中心读模型=approval_request⋈change_proposal，非 ADR） |
| approval_request / change_proposal / document_proposal | 存在（专用，见权限与审批清单） |

十问（独立表/ID/状态机/Revision/备选/理由/后果/supersedes/跨设计引用/独立查询审计）**全部不存在**。

## 10. VF-10 审批能力（四类区分+复用判断）

四实体（approval_request/change_proposal/document_proposal/review_approval）DDL、状态机逐态触发点、入口全表移至 [S03-00-permission-and-approval-inventory.md](S03-00-permission-and-approval-inventory.md) §3~4。

- 四类审批：①文档发布审批=已存在（document_proposal+veto）②普通变更审批=已存在（approval_request 1:1→change_proposal；escalated 无审批行）③技术设计批准=不存在 ④ADR 正式接受=不存在。
- 复用判断：**实体不可原样复用；形态可复制（先例=每类审批建专表）**。理由：FK+UNIQUE 硬绑、Approve 语义=执行 ProjectCommand、无多态列；可复用资产=pending 守卫/触发器不可变/字段形状/审计/通知/质量门。**边界（R6）**：五类业务决策（Requirement Approval/Design Approval/ADR Acceptance/Design Review Decision/Implementation Baseline Freeze）须各有独立领域语义与审计依据；具体结构路线（独立专表/共享基础设施/其他）留待 S03-05 切片论证；`governance.approval_request` 的 1:1 绑定语义不得直接复用为通用设计审批机制。

## 11. VF-11 ADR 与 Requirement/Design 关联

ADR↔Requirement、ADR↔Design、ADR↔DesignRevision 三向关联能力全部不存在（无列无表）。decision 侧间接痕迹：质量门要求正文含 source_events:/projects/ 引用（knowledge_quality.rs:152-159）；source_ref 不落库。唯一落库引用设施 ai.knowledge_reference（0014:82-104）方向相反（分析→文档；reference_kind 含 'decision'；可指历史修订）。

## 12. VF-12 设计评审能力

| # | 项 | 结论 | 证据 |
|---|---|---|---|
| 1 | 独立 Review 实体 | 🟡 无独立表；=task_run 行 run_kind='review'（0026:15-23）+governance.review_approval（0026:223-248） | review/mod.rs:1-57；pm_reviews.rs:33-43 |
| 2 | 对象任意挂载 | ❌ 锚定仅 delivery run 逐仓 commit+验收条目（anchor.rs:21-36、queue.rs:82-117） | — |
| 3 | 绑 Revision | 🟡 绑内容寻址哈希（reviewed/input_manifest_hash+policy_version，anchor.rs:93-129）；条目修订→哈希漂移拒收 | collect.rs detect_anchor_drift |
| 4 | 多评审人 | ❌ 单一平台评审者 REVIEWER_ROLE="reviewer"（queue.rs:30）+人工单 decided_by（0026:236） | — |
| 5 | 多轮 | ✅ attempt_no 按 kind 分轨（0026:62-67）+uq_task_review_active（0026:77-79） | — |
| 6 | 问题登记 | ✅ review_finding severity info/warning/blocker（0026:165-186）；reject 理由转 finding（decide.rs:681-705） | — |
| 7 | 问题关闭 | ❌ 无状态列+append-only 触发器（0026:265-266） | — |
| 8 | 专家与裁决分离 | 🟡 平台三值 verdict vs 人工 decided_by 回填（0026:22-23,203-204）；非双角色设计 | — |
| 9 | 重新提交 | 🟡 ChangesRequested→Ready（task_transition.rs:36）+新 DeliverySubmitted→新 review run；无显式 resubmit API | outbox_handlers.rs:147-185 |
| 10 | 历史保留 | ✅ 禁 UPDATE 触发器（0026:257-268）+once-written 报告列（0026:300-318） | — |
