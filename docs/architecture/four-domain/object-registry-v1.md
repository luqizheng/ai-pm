# 四域对象注册表 V1.0（审计产出）

> 审计基准：commit `05ce33d8` · full-dev postgres · 迁移至 0024 · 2026-11-15
> 性质：只读审计产物，非已批准基线。对象编码沿用任务书 §5；实现状态取值：
> `COMPLETE`（链路完整）／`PARTIAL`（有实现有缺口）／`STUB`（有表无生产写入者）／`NO_PROD_DATA`（链路存在但全库 0 行）／`DEAD-COLUMNS`（schema 就绪代码未接线）
> 证据缩写：BL=`migrations/0001_baseline.sql`，其余迁移按号；代码路径相对 `crates/`（apps/ 另注）。

## 1. KB 域（owner_domain=KB）

| Object Code | 对象 | 当前实现 | 写入权（正式） | 审批权 | 正文权威 | 状态权威 | 版本权威 | 证据模型 | 引用模型 | 实现状态 |
|---|---|---|---|---|---|---|---|---|---|---|
| KB-SOURCE | Source | kb.source（BL:2982） | `kb_service/source.rs:168` register_source（真人 REST/AI 均可，内容寻址去重） | 无需 | inline_content/uri 即原文（append-only 不可改） | 无状态列 | 无版本（append-only） | 本体即原文 | `kb.source_project` 多对多挂项目 | COMPLETE |
| KB-DOCUMENT | Document | kb.document（BL:2860） | `kb_service/document.rs:333/570` ingest；worker `kb_pc_analysis.rs:495` upsert（幂等） | 无需（身份层） | 无正文（正文在 Revision） | 无状态列 | 无独立版本 | 无 | **与 KB-SOURCE 无引用列**（缺口，见 §5.2-3） | COMPLETE |
| KB-DOCUMENT-REVISION | Document Revision | kb.document_revision（BL:2878+0006+0023） | draft：ingest + worker（DISPATCH_SERVICE_PRINCIPAL）；published：唯一门 confirm_revision | **confirm_revision 三重门**（interactive+require_human+成员，document.rs:60-117） | **PG content 列**（content_hash 寻址；source_commit_sha 是源锚非内容权威）→ 偏离 D01/§9.1 | status∈draft/published/rejected，DB 触发器白名单+应用层 CAS（document.rs:199） | revision_no 单层 | source_ref='docproc:artifact:{id}'+checksum 复检 | 四元组不适用（域内对象） | COMPLETE（正文权威=偏差项） |
| KB-MEMORY | Knowledge/Memory | kb.memory（BL:2901） | 唯一入口 propose_create（memory.rs:365，REST POST /v1/kb/memories） | 提案无门、审核有门 | 正文在 Revision | 无状态列（状态在 Revision） | 经 Revision | 经 memory_ref | 经 memory_ref | COMPLETE 但 **NO_PROD_DATA**（0 行，无自动生成侧） |
| KB-MEMORY-REVISION | Knowledge Revision | kb.memory_revision（BL:2936） | insert_revision 恒 PENDING_REVIEW（memory.rs:290）；审核转换唯一写点 memory::review（:685/703/756） | **require_human + K4 CAS + 行锁 + 触发器白名单**（BL:228-276，全库最严） | title/summary/body 存 Revision 行，content_hash 规范化寻址，无 Git | review_status 4 态（大写）+ version_status 4 态（大写） | 双层：revision_no（提案序）+ version_no（审核版号） | memory_ref role=evidence 强制 ≥1 | memory_ref role=subject（检索锚）+ 四元组 | COMPLETE 但 NO_PROD_DATA |
| KB-EXTRACTION | Extraction | **无实体**（借 docproc.artifact + document_revision.source_ref） | — | — | — | — | — | — | Document→Memory 提取关系缺失（基线 §6.1 要求多对多） | STUB（概念未落） |
| KB-REFERENCE | Knowledge Reference | kb.memory_ref（BL:2914） | memory.rs:327（提案时写入，不可变触发器） | 随 Memory 审核生效 | — | role∈evidence/subject | target_revision_id 钉版本 | 四元组+locator+revision_missing_reason | DomainRef 四元组（限 KB 域内 target_type=source） | COMPLETE 但 NO_PROD_DATA |
| KB-SECTION-INDEX | Section Index | kb.document_section_index（0008） | 唯一消费者 KbDocumentRevisionPublishedHandler（kb_pc_handlers.rs:793→indexer.rs:127） | 无（派生） | 无正文权威（章节副本仅索引） | pending/indexed/stale/failed | 可重建（REBUILDABLE） | — | document_revision_id | COMPLETE（REBUILDABLE） |

KB 域附注：
1. **旧 ai 知识栈**（ai.project_document/ai.document_revision/ai.document_proposal）：Git 正文权威（knowledge.rs:137 KnowledgeRoot，write_and_commit:603，读校验 hash :539-577，历史经 git show :742）；8 态提案机 + veto 回滚（knowledge_proposal/submit.rs:290 require_human；veto.rs:106）。处理策略 KEEP（独立服务需求/ADR/设计投影），与 kb.* 零写入互通。
2. kb.memory 生成侧缺位：无事件/命令/outbox 自动提案，worker/MCP/PC 链均不写 kb.memory（全仓仅 REST 入口）→ 场景 B（资料提取为知识）当前无法端到端发生。

## 2. Analysis 域（owner_domain=ANALYSIS）

| Object Code | 对象 | 当前实现 | 写入权（正式） | 审批权 | 正文权威 | 状态权威 | 版本权威 | 证据模型 | 引用模型 | 实现状态 |
|---|---|---|---|---|---|---|---|---|---|---|
| AN-RUN | Analysis Run | ai.analysis_run（BL:675） | **双写入者**：pm-ai 编排 start_run/finish_run/fail_run（pm-ai/context.rs:80-165/291/312）＋冷启动导入链一步插终态行（pm-application/project_import/proposal.rs:312） | 无 | 无正文（result jsonb） | **应用层独占**（无 CHECK/无触发器/无乐观锁）；状态词已分叉 completed vs succeeded | 无版本 | result.findings.evidence_refs（字符串数组） | event_id FK→inbound_event；与 repo_import 裸列关联 | PARTIAL（D02 契约未建；状态枚举分叉） |
| AN-REPORT | Analysis Report | integration.repo_analysis_report（0023/0024） | analysis_report.rs 全套（ensure_for_intake:121 / mark_analyzed:222 / mark_failed:253 / close_report:387）；**cancelled 无写入者** | close 真人门（ensure_closable:552）+ 报告 open 门 409（repo.rs:824） | **无正文列，借 kb.document_revision**（裸 kb_revision_id） | status 5 态 CHECK，守卫全应用层（WHERE+FOR UPDATE） | **无 Revision**（一次 intake=一行） | 经 pc.evidence（worker 落） | intake_id UNIQUE 幂等；kb_revision_id/pc_source_id 裸 UUID | PARTIAL（D01 目标态缺 Revision） |
| AN-REPORT-REVISION | Report Revision | **不存在** | — | — | 目标 Markdown+Git | — | — | — | — | STUB（D01 冻结裁决的目标对象，未实施） |
| AN-FINDING | Analysis Finding | **无独立实体**：ai.analysis_run.result JSONB findings（pm-domain/proposal.rs:58，无稳定 ID）＋分散 4 套领域 Finding | 各领域自有（discovery/finding.rs:418、review/worker.rs:237、design/finding.rs:58） | discovery 验证人工门（discovery/web.rs:119）；design 状态机人工推进 | — | 5 套状态机互异（矩阵见 lifecycle-matrix §3） | 无 | discovery 有独立 evidence 表；delivery/design 无 | 各域自有裸引用 | STUB（D03 基础契约未建） |
| AN-EVIDENCE | Evidence | pc.evidence 被分析链复用（kb_pc_analysis.rs:936）＋discovery_finding_evidence | worker 经 pc::register_evidence（evidence.rs:177） | verification 人工验证仅 discovery 域 | — | pc.evidence.verification_status 4 态；discovery 大写 4 态 | evidence_hash + 四元组钉 KB 修订 | 结构化定位列最全 | DomainRef 四元组（pc 侧） | PARTIAL（归属 PC 域，Analysis 无自有证据对象） |
| AN-CONCLUSION | Conclusion | result.summary 字段 + change_proposal.rationale | pm-ai orchestrator.rs:326 | — | — | 无 | 无 | — | change_proposal FK→analysis_run | STUB（未独立成对象） |
| AN-RECOMMENDATION | Recommendation | ai.change_proposal（BL:742）由 analysis_run 产物落行（orchestrator.rs:240-256） | pm-ai 写入；人工审批走 governance.approval_request | approval_request 人工门 | — | 7 态 CHECK（proposed/executed/redline_pending/escalated/vetoed/expired/failed） | 无 | evidence jsonb 列（BL:753） | analysis_run_id NOT NULL FK | PARTIAL（语义=导入命令提案，非通用建议） |
| AN-INPUT-REF | Input Reference | context_snapshot/config_snapshot JSONB（pm-ai/model.rs:15-24） | pm-ai context.rs:167-265 装配 | — | — | — | 各 ref 为"项目内 JSON 行快照"，无版本钉扎 | — | JSONB 内嵌（绕过 DomainRef） | PARTIAL（ARC-05 钉版本要求未满足；导入链写 `{}` 或 `{"import_id":…}` 结构漂移） |

Analysis 域附注：
1. 四条执行链盘点（V2.0 §3.4 映射）：ai.analysis_run=事件分析编排；integration.repo_import=导入工作流（KEEP+关联）；delivery.task_run=交付/评审工作流（KEEP，55 列租约 fencing）；discovery_session=探索工作流（KEEP，无 run 表）。**无任何 Adapter 代码**（grep RunAdapter/run_contract 仅命中 delivery 专用 RunContract，run_context.rs:202）。
2. 仓库分析链执行主体实际是 integration.intake_task（PENDING/DISPATCHED/…，intake/task.rs:10），ai.analysis_run 在该链中不参与——三链三模型现状与上轮审计结论一致并有新证据（状态词分叉）。

## 3. Project Cognition 域（owner_domain=PROJECT_COGNITION，D04 目标域）

| Object Code | 对象 | 当前实现 | 写入权（正式） | 审批权 | 正文权威 | 状态权威 | 版本权威 | 证据模型 | 引用模型 | 实现状态 |
|---|---|---|---|---|---|---|---|---|---|---|
| PROJ-COG-OBJECT | 正式认知 | ai.project_knowledge（BL:1101） | **唯一 INSERT**：cognition_item 确认事务（repo_cognition.rs:263-276）；insert-only，无 UPDATE/DELETE | **确认端点仅成员可见性门，无 require_human/可信上下文门**（pm_cognition.rs:279-318）——P0 | content TEXT 列（PG） | status∈active/superseded/retired | version 恒 1、supersedes_id 零写入（**DEAD-COLUMNS**） | 无自有证据（仅 source_item_id 回溯） | source_item_id FK→cognition_item | PARTIAL（人工门缺失=P0） |
| PROJ-COG-REVISION | Cognition Revision | **无**（version/supersedes 死列） | — | — | — | — | — | — | — | STUB |
| PROJ-COG-PROPOSAL | Cognition Proposal | ai.cognition_item（BL:770）即候选 | 唯一 INSERT：start_run 派生（repo_cognition.rs:161-187；无 LLM、同步结构化事实） | decide_item 6 态中实装 4 转移，一次性决策 409（repo_cognition.rs:241-245）；**同缺主体门** | — | state 6 态 CHECK；superseded 态不可达（DEAD-COLUMNS） | 无 | evidence jsonb（仅 repository_id，无 commit/路径/行号） | run_id FK、knowledge_id FK | PARTIAL |
| PROJ-COG-EVIDENCE | Cognition Evidence | cognition_item.evidence jsonb（无独立对象） | pm_cognition.rs:130-151 | — | — | — | — | 定位粒度=repository 级 | — | STUB |
| PROJ-COG-VERSION | Cognition Version | **无快照机制** | — | — | — | — | — | — | — | STUB |
| PROJ-COG-RELATION | Cognition Relationship | **无** | — | — | — | — | — | — | — | STUB |
| （执行） | Cognition Run | ai.cognition_run（BL:796） | start_run（repo_cognition.rs:161） | — | scope/repo_versions jsonb | status 7 态 CHECK | 无 | repo_versions 记 "current" 非具体 revision | baseline_proposal.source_run_id FK（**零填充死列**） | PARTIAL |

Project Cognition 域附注：
1. `ai.cognition_item` 语义判定：**候选对象**（proposed 起点、一次性人工决策）；`ai.project_knowledge` 语义判定：**确认后落地的正式对象（追加式）**，但确认门强度不足（见 P0）。
2. 与 PC 唯一交叉：identity-synthesis 端点（pm_cognition.rs:168-228→identity.rs:178-206）产 PC 提案；反向零引用。
3. 冷启动导入链不落本域（落地 delivery.milestone/requirement/task，executor.rs:399/474/632）。
4. 冲突治理：has_conflict 列零写入（桩）；无冲突对象。

## 4. PC 域（owner_domain=PC）

| Object Code | 对象 | 当前实现 | 写入权（正式） | 审批权 | 正文权威 | 状态权威 | 版本权威 | 证据模型 | 引用模型 | 实现状态 |
|---|---|---|---|---|---|---|---|---|---|---|
| PC-OBJECT | Cognition Object | pc.cognition_object（BL:3041） | pc 服务层 upsert_object_draft（object.rs:208/115/154/187/264）←worker+identity；生命周期推进仅发布事务（publish.rs:628/644）；bootstrap 种子（bootstrap.rs:325） | 正式化随 Version 门 | 正文经 Revision 间接 | lifecycle 5 态 CHECK+触发器（BL:3056、trg_pc_object_lifecycle） | current_revision_id 钉 | 经 pc.evidence | relationship 端点 | COMPLETE（**NO_PROD_DATA**） |
| PC-REVISION | Cognition Revision | pc.cognition_revision（BL:3064） | object.rs:246/427/492 | 行不可变触发器 | **PG content**（经 document_revision，git_commit_sha 恒 NULL=PCGIT 后置偏差） | 无状态列（append-only） | revision_no | content_hash+source_context | document_id FK | COMPLETE |
| PC-VERSION | Cognition Version | pc.cognition_version+version_member（BL:3087/3418） | **仅 publish.rs:455/473**（发布事务） | **ensure_formal_authorized 三重门**（publish.rs:100） | snapshot_hash 快照锚 | published/archived | version_no+predecessor 链 | version_member 端点闭合触发器 | source_proposal_id | COMPLETE（NO_PROD_DATA） |
| PC-PROPOSAL | Proposal | pc.proposal+proposal_member（BL:3240/3271） | create/submit/decide/withdraw（proposal.rs:644/810/1064/1181）；close 批量 withdraw（analysis_report.rs:458） | approve/reject 需真人门（proposal.rs:916） | proposal_content_hash 锚定 | 8 态 CHECK+锚列不可变触发器 | expected_version_no 乐观锁 | members+source_id | source_id→pc.source | COMPLETE |
| PC-APPROVAL | Approval | pc.approval（BL:3021） | proposal.rs:819/1000/1047 | 本体即审批 | — | pending/approved/rejected+终态不可变触发器 | — | — | proposal_id FK | COMPLETE |
| PC-PUBLISH | Publish | pc.write_operation（BL:3453）+ publish.rs 事务 | publish.rs:112（INSERT）；**CHECK 允许的 pc_commit_object/relationship_revision 两命令零写入者** | — | — | pending→completed（git_committed 中间态闲置） | 幂等键重放（publish.rs:111-176） | — | — | PARTIAL（PCGIT 需启用闲置命令） |
| PC-SOURCE | Source | pc.source（BL:3391） | register_source（evidence.rs:56）←worker | — | locator/section_anchor | 无状态列 | — | source_content_hash | **自带 DomainRef 四列**+revision_missing_reason | COMPLETE（NO_PROD_DATA） |
| PC-EVIDENCE | Evidence | pc.evidence（BL:3182） | register_evidence（evidence.rs:177）←worker | verification 无人工程序（4 态枚举） | — | unverified/verified/refuted/conflicted | evidence_hash | 7 类 evidence_type+三形态 CHECK（代码/文档节/口述） | target 四元组+object_revision_id | COMPLETE（NO_PROD_DATA） |
| PC-RELATION | Relationship | pc.relationship(_revision/_type)（BL:3293/3327/3369） | **create_relationship_draft 无生产调用者**；lifecycle 推进在发布事务（publish.rs:663/678） | 字典 owner_domain 语义=确认权威域（0007 终裁） | — | relationship 端点不可变+delete 禁止 | relationship_revision 钉端点 | — | 双端 ext 四元组 | **STUB**（字典空置：full-dev 0 行） |
| PC-CONFLICT | Conflict | pc.conflict/conflict_object（BL:3108/3134） | **无生产写入者**（仅测试直插）；事件常量在 events.rs:49 但无生产者；P6 读端点 501 | resolve 无人工程序（生产代码只读） | — | 6 态 CHECK | — | — | conflict_object 挂对象 | **STUB**（L3 冻结守卫因此为死代码；若手工插行将永久冻结且无解冻入口） |

## 5. 覆盖率统计与结论速览

| 领域 | 目标对象数 | COMPLETE | PARTIAL | STUB | NO_PROD_DATA 标注 |
|---|---|---|---|---|---|
| KB | 8（+旧栈 6 表另册） | 6 | 1（DOC-REVISION 正文权威偏差） | 1（EXTRACTION） | memory/memory_ref/memory_revision（0 行） |
| ANALYSIS | 8 | 0 | 5 | 3（REPORT-REVISION/FINDING 契约/CONCLUSION） | — |
| PROJECT_COGNITION | 6+1 | 0 | 3 | 4 | project_knowledge/cognition_item/cognition_run（0 行） |
| PC | 10 | 6 | 2 | 2 | 除 pc.source 全部 0 行 |

**结论**：四域中 PC 与 KB 的正式对象链路最完整（含 DB 层守卫），Analysis 是结构缺口最大的域（D01/D02/D03 三项冻结裁决的目标对象全部不存在），Project Cognition 是治理门最弱的域（人工确认门缺失 + 死列成片）。
