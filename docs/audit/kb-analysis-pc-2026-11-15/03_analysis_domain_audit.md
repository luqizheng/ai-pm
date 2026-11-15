# 03 Analysis（分析领域）审计

## 0. 结论速览

**"分析领域"没有独立 schema，对象分散在 ai / integration / delivery / discovery 四个 schema，且存在三条互不相同、未统一的分析链**：

| 链 | Run 载体 | Report 载体 | Finding 载体 | 正文/结果位置 |
|---|---|---|---|---|
| ① 仓库再分析（verify/analyze 触发） | intake_task（kb_analyze/pc_analyze）+ analysis_report 状态推进 | **integration.repo_analysis_report**（报告门实体） | 无独立 finding（产物=PC 提案+KB 草稿） | 正文=kb.document_revision.content（KB 修订即报告正文） |
| ② 冷启动导入 | **ai.analysis_run**（analysis_run_id 挂 repo_import） | 无 Report 实体 | ai.change_proposal（逐命令提案）+ result jsonb 内嵌 findings 数组 | result jsonb（PG） |
| ③ 任务执行/评审 | delivery.task_run（run_kind=delivery\|review） | 无 | delivery.review_finding / design.review_finding | review 产物行 |

**基线意义上的独立 Analysis Report（独立 ID、独立版本、独立正文权威、Run 与 Report 分离）只部分实现**：repo_analysis_report 有独立 ID 与状态机（ARC-10 Run/Report 分离 MATCH），但**无正文列、无版本管理、无 analysis_run_id 列**——正文借住在 KB 修订里，与执行 Run 仅经 intake_id 间接关联。

## 1. ai.analysis_run（分析执行 Run，事件→提案模型）

- 列：id/project_id/event_id/status/model_name/policy_version/config_snapshot/context_snapshot/self_review/result(jsonb)/failure_code/attempt_count/started_at/finished_at/tenant_id。
- 创建者 A：`pm-ai/src/context.rs:143-161` `start_run`（status='analyzing'）；finish：`context.rs:287-303`（UPDATE result+self_review）。
- 创建者 B（冷启动导入）：`project_import/proposal.rs:312-324`（status='succeeded'，result=`{"import_id":…}`）。
- result jsonb 结构：`pm-domain/src/proposal.rs:46-50` `AnalysisResult{summary, findings: Vec<Finding>, proposals: Vec<Proposal>}`；Finding{skill,severity,description,evidence_refs}（:36-42）。**finding 嵌在 jsonb 里，无独立表、无独立状态生命周期**。
- self_review：`pm-ai/src/self_review.rs:33-40` ProposalOutcome{Auto|RequirePreConfirmation{role}|Escalate{reason}}。
- 谁引用它：repo_import.analysis_run_id（无 FK，application 层写，proposal.rs:356-363）；governance.audit_log.analysis_run_id；llm_call_record 按 event_id 关联。

## 2. integration.repo_analysis_report（仓库分析报告门，0023/0024）

- 列：id/repository_id/commit_sha/intake_id/status/stage_detail/error_code/error_detail/**kb_revision_id/pc_source_id**/closed_at/closed_by/run_mode/since_commit。UNIQUE(intake_id)；部分索引 `idx_repo_analysis_report_open`（0023:36-38，每仓库至多一份 open 报告）。
- **报告正文 = kb_revision_id → kb.document_revision.content**（analysis_report.rs:11,50-58,318-332）。报告行本身无正文列。
- **finding 级产物** = PC 提案（pc.proposal，analysis_report.rs:334-351）+ PC 对象修订（pc.cognition_revision 经 pc.evidence 钉扎反查，:355-372）。
- kb_revision_id / pc_source_id 是**裸 uuid 引用，无 FK 约束**（0023 DDL 全列无外键定义）。
- close 流程（`close_report` :387-551）：仅 analyzed 可关；真人门（ensure_closable :560-567 interactive）；approved PC 提案批量 publish_version（:419-444，幂等键 `pc-decision:{id}`）；draft/submitted 提案→withdrawn；KB 草稿→rejected；固化锚点 project_repository.last_analyzed_commit_sha（:530-539）。
- run_mode=incremental|full（0024；服务端归一化 repo.rs:782-798：incremental 无锚点自动退 full）。
- 报告门：repo.rs:813-825，存在 open 报告时新分析 409 ReportOpen（带 report_id）。
- **报告无版本管理**（无 revision_no/parent）；"修订 vs 重新分析"目前仅靠 run_mode+since_commit 区分，重新分析=新报告行。

## 3. ai.cognition_run / ai.cognition_item（项目认知条目，非泛化 Finding）

- cognition_run：status 7 值（pending/collecting/analyzing/completed/partially_completed/failed/cancelled）；V1 确定性派生直接 completed（repo_cognition.rs:202-210）；scope/repo_versions jsonb 记录输入。
- cognition_item：domain 6 值（identity/repository/domain/architecture/convention/evolution）；state 6 值（proposed/confirmed/rejected/investigation/deferred/superseded）；evidence jsonb；knowledge_id→ai.project_knowledge（confirm 时 INSERT project_knowledge 并回写 source_item_id，repo_cognition.rs:272-283）。
- 决策是人工：apps/pm-api/src/pm_cognition.rs:279-310（decided_by 取真人主体；:12 注释"AI 无确认入口"）；仅 proposed 可决、重复 409（repo_cognition.rs:249-261）。
- 注意：cognition_item 的"确认"直接写 **ai.project_knowledge**（旧 ai 域知识），**不写 pc.cognition_object 也不写 kb.memory**——同一项目里三套知识对象并存（ai.project_knowledge / kb.memory / pc.cognition_object），互不外键。

## 4. Finding 对象全景（4 处，生命周期各异）

| 对象 | 表 | 状态 | 生命周期 |
|---|---|---|---|
| 项目认知条目 | ai.cognition_item | proposed/confirmed/rejected/investigation/deferred/superseded | 独立，逐条人决 |
| 探索/访谈发现 | discovery.discovery_finding | verification=PENDING/CONFIRMED/REJECTED/CONFLICT（kind=observed/user_confirmed/speculation/contradicted） | 独立，可验证改判；证据子表 discovery_finding_evidence（复合 FK 到 repo_import_source_attempt，commit_sha 级溯源） |
| 评审发现 | delivery.review_finding / design.review_finding | delivery：无状态列（append-only，挂 review run）；design：open/acknowledged/resolved | delivery 挂 run；design 独立可人工关闭 |
| 分析 result 内嵌 finding | ai.analysis_run.result jsonb | 无状态 | 无独立生命周期（随 Run 存取） |
| （关联表） | delivery.requirement_finding | relation 恒 'supports' | 纯溯源关联，无生命周期 |

## 5. repo_import（冷启动导入链）与 draft 版本化

- 状态 10 值：queued→cloning→reading→analyzing→draft_ready→proposed→applied / rejected / failed / cancelled（CHECK）。worker 入口 apps/ai-pm-worker/src/main.rs:90-96；阶段推进 project_import/lifecycle.rs:611-629（set_stage 终态守卫 :530-542）。
- draft=pm_domain::import::ProjectDraft（pm-domain/src/import.rs:168-185：profile/milestones/requirements/tasks/risks/unknowns/coverage），版本化于 repo_import_draft_version（import_id+revision PK，generation_status=generating|succeeded|failed）。
- 提案化：proposal.rs:287-363——写 core.project 概要 → ai.analysis_run(succeeded) → 每命令一条 ai.change_proposal(redline_pending) + governance.approval_request(project_owner) → import='proposed'。人工审批执行（orchestrator.rs:506-538）→ advance.rs:29-100 推进 applied/rejected/partial（vetoed→rejected；failed 保持 proposed+error_code='partial'）。
- 实测样本：6 条 repo_import（2 cancelled/failed qwen2.5:3b、1 failed deepseek-chat、1 draft_ready deepseek-chat draft_revision=1），draft jsonb 15239 字节，model_name 随四级模型链记录（非恒 fake）。

## 6. 任务书 14 问逐答

1. 独立 Analysis Run？**部分**：ai.analysis_run（导入链）与 integration.repo_import（状态机本身）、discovery_session、delivery.task_run 都是"执行"载体，但无统一 Run 模型。
2. 独立 Analysis Report？**部分**：仓库分析有 repo_analysis_report（独立 ID/状态机/报告门），导入与任务链无 Report 实体。
3. 报告独立 ID？是（repo_analysis_report.id，UNIQUE(intake_id)）。
4. 报告版本管理？**无**（无 revision 列；重分析=新报告行；历史保留靠行不删）。
5. 报告正文？kb.document_revision.content（KB 域修订），报告行只存指针（裸引用无 FK）。
6. 报告关联执行任务？间接：intake_id→intake_task；**无 analysis_run_id**。
7. 输入对象及版本？部分：repo_analysis_report 记 commit_sha/run_mode/since_commit；repo_import_source.head_commit_sha + repository_version_confirmation（人工选版）；cognition_run.repo_versions jsonb。
8. 独立 Finding 对象？是，但分裂为 4+ 套（见 §4），语义/状态词汇互不一致（大小写混用：discovery 用 PENDING/CONFIRMED，其余小写）。
9. Finding 独立状态管理？cognition_item/discovery_finding/design.review_finding 有；analysis_run.result 内嵌 findings 与 delivery.review_finding 无。
10. Finding 关联多业务对象？cognition_item 无关联表（仅落 project_knowledge）；discovery_finding 有 repository 绑定 + requirement_finding 关联（supports）；pc.evidence 才是多目标四元组（属 PC 域）。
11. 证据引用？discovery_finding_evidence（commit_sha 级）；cognition_item.evidence jsonb；pc.evidence（结构化）；analysis_run Finding.evidence_refs（jsonb 内）。
12. 报告发布/审核机制？有：报告门 409 + 真人 close + 审阅流 v2"决定暂记，close 批量发布"（analysis_report.rs:416-444）。
13. 仓库/需求/产品分析统一模型？**否**——三条链三种模型（§0 表）。
14. 分析报告属于哪个领域？**无归属**：表在 integration schema，正文在 KB，产物在 PC，执行 Run 在 ai——跨域缝合靠代码（analysis_report.rs）而非数据模型。

## 7. 判定汇总（对照基线 ARC）

| 基线要求 | 判定 | 依据 |
|---|---|---|
| Run 与 Report 分离（ARC-10） | PARTIAL/CONFLICT | 仓库链有分离但无 analysis_run_id 关联列（靠 intake 间接）；导入链 Run 即全部、无 Report |
| 报告独立 ID | MATCH（仓库链） | repo_analysis_report.id |
| 报告版本管理 | GAP | 无 revision；重分析=新行，无法表达"同一报告的修订" |
| Finding 独立生命周期 | PARTIAL | 4 套并存、词汇不一致、result 内嵌 findings 无状态 |
| 重新分析=新 Run | PARTIAL | 仓库链新报告行+run_mode/since_commit；无显式 Run 实体承载 |
| 分析成果可追溯（P6：输入+版本） | MATCH（仓库链） | commit_sha/since_commit/last_analyzed_commit_sha 锚点 |
| 报告不能自动成为正式认知（ARC-03） | MATCH | 产物=PC draft+proposal，close 批量发布仍走 pc publish 事务（真人门） |
| 独立一级领域（D01） | GAP | 无 analysis schema，对象散落 4 schema |
