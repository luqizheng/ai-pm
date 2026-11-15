# 01 数据库结构清单（Database Inventory）

> 审计日期：2026-11-15。只读提取，未做任何写操作。
> 数据源：full-dev 栈 PostgreSQL（容器 `superpowers-full-postgres-1`，库 `postgres`）系统目录 + `public._sqlx_migrations`。
> 注意：`127.0.0.1:16433`（aipm-pg-gate）是 sqlx 测试门禁库，`public` schema 为空（仅 `_sqlx_test.databases`），非业务数据源。

## 1. 迁移机制

- 唯一迁移机制 = sqlx：`sqlx::migrate!("../../migrations")`（`apps/pm-api/src/main.rs:28`）。
- 已应用迁移 24 条（`public._sqlx_migrations`，全部 success=t），与仓库 `migrations/0001~0024` 一一对应：

| # | 名称 | 与三域相关内容 |
|---|---|---|
| 0001 | baseline | 建 ai/core/delivery/design/kb/pc/discovery/docproc/integration/governance/source/viz 全部基线表（含 KB、PC、cognition、analysis_run） |
| 0002 | conversation_discovery_link | 会话↔发现会话关联 |
| 0003 | conversation_context_anchor | 会话上下文锚 |
| 0004 | llm_call_record_metadata | LLM 调用记录元数据 |
| 0005 | conversation_interaction | 会话交互（accept/reject/defer…） |
| 0006 | kb_revision_confirm_columns | kb.document_revision 加 confirmed_by/published_at（确认即发布门） |
| 0007 | relationship_owner_domain_seeds | pc.relationship_type 加 owner_domain（pc/kb）+ 8 条种子 |
| 0008 | kb_section_index | kb.document_section_index（CJK bigram FTS）+ kb.document_index_state |
| 0009 | pc_revision_fts | pc.document_revision.search_tsv 派生列 |
| 0010 | design_domain | design.*（adr/design/revision/approval/review_finding/implementation_baseline） |
| 0011 | task_plan_domain | plan.task_plan(+_revision) |
| 0012 | system_embedding_config | platform.embedding_config（enabled=false，向量通道 G-3 门控中） |
| 0013 | task_run_event | delivery.task_run_event |
| 0014 | task_knowledge_pin | delivery.task_knowledge_ref |
| 0015 | task_schedule | 任务调度 |
| 0016 | docproc_domain | docproc.*（document/document_version/processing_job/artifact/source_mapping） |
| 0017 | kb_docproc_source_ref | kb.document_revision 加 source_ref/source_checksum（docproc artifact 溯源） |
| 0018 | team_llm_catalog | ai.team_llm_*（provider/model/credential/authorization/policy） |
| 0019 | conversation_context_item | 会话上下文项 |
| 0020 | visualization_artifact | viz.* |
| 0021 | ssh_key_key_type | SSH key 类型 |
| 0022 | ssh_key_multi | project_ssh_key 多行化 |
| 0023 | repo_analysis_report | integration.repo_analysis_report（含 kb_revision_id/pc_source_id） |
| 0024 | analysis_report_run_mode | repo_analysis_report 加 run_mode（incremental/full）/since_commit |

- 无 ENUM 自定义类型（`pg_type` 零行 `pg_enum`）；所有"枚举"= text 列 + CHECK 约束（见 §6）。
- 迁移号占用：master 当前 24 个文件；其余分支/worktree 存在更高号（0055 已占用、历史上限 0064，见仓库纪律文档），本库未应用。

## 2. Schema 清单（19 个）

业务 schema：`ai, core, delivery, design, devenv, discovery, docproc, governance, integration, kb, pc, plan, platform, source, viz`（15 个）。
外部服务残留 schema：`public`（iam_* / ntf_* / token_* / file_* / audit_* / prue_* / _sqlx_migrations）、系统 schema `pg_catalog, pg_toast, information_schema, pg_toast`。

表总数：209（业务 15 schema 约 130 张 + public 79 张外部服务表）。

## 3. 三域相关 Schema · 表 · 行数（2026-11-15 实测）

### kb（知识库，9 表）

| 表 | 列数 | 行数 | 职责 |
|---|---|---|---|
| kb.document | 8 | 1 | 文档登记（doc_key/doc_type/canonical_path，UNIQUE(tenant,project,doc_key)） |
| kb.document_revision | 16 | 1 | **Markdown 正文权威行（PG content 列）**，draft/published/rejected，analysis_mode=derived/derived_fallback/llm |
| kb.document_index_state | 13 | 1 | 章节索引状态（provider/revision，UNIQUE(revision,provider)） |
| kb.document_section_index | 15 | 6 | 章节级 FTS 派生索引（search_tsv） |
| kb.memory | 6 | 0 | 知识对象壳（无内容列） |
| kb.memory_revision | 20 | 0 | 知识修订（正文 body 列 + review_status + version_no/version_status） |
| kb.memory_ref | 12 | 0 | 修订级跨域引用（ref_role=evidence/subject，target 四元组） |
| kb.source | 11 | 0 | 团队级来源池（uri XOR inline_content CHECK） |
| kb.source_project | 5 | 0 | 来源↔项目多对多 |

### pc（产品认知，18 表 + 1 视图）

| 表 | 行数 | 职责 |
|---|---|---|
| pc.cognition_object | 0 | 认知对象（object_type/lifecycle_status/current_revision_id/superseded_by_id） |
| pc.cognition_revision | 0 | 对象修订（content_hash 钉 pc.document_revision 正文行；analysis_mode） |
| pc.document / pc.document_revision | 0 | 认知对象 Markdown 正文（PG content 权威；git_commit_sha 恒 NULL） |
| pc.cognition_version / pc.version_member | 0 | 发布快照版本（version_no/snapshot_hash/predecessor_version_id + 成员行） |
| pc.proposal / pc.proposal_member | 0 | 提案（origin_kind=ai_agent/kb_memory/…；成员 change_kind 六值） |
| pc.approval | 0 | 人工审批（乐观锁 proposal_content_hash + expected_version_no） |
| pc.write_operation | 0 | 发布幂等账本（operation_id/command/status） |
| pc.object_type / pc.relationship_type | 18 / 20 | 类型字典（layer/governance_level/singleton；relationship owner_domain=pc/kb） |
| pc.relationship / pc.relationship_revision | 0 | 认知关系（内部对象端 + 跨域 ext 四元组端） |
| pc.source | 1 | 跨域来源登记（source_kind 8 值，kb_document 通道强制 target=knowledge_base+revision） |
| pc.evidence | 0 | 证据钉扎（主体=object_revision XOR relationship_revision；定位器按 evidence_type 分型） |
| pc.conflict / pc.conflict_object | 0 | 冲突（生产无写入者，桩） |
| 视图 pc.v_revision_versioned | — | 标记每条 cognition_revision 是否被 version_member 钉扎（in_published_version） |

### 分析相关（分散 3 schema，无独立 analysis schema）

| 表 | 行数 | 职责 |
|---|---|---|
| ai.analysis_run | 0 | 分析执行 Run（status/model_name/config_snapshot/context_snapshot/self_review/result jsonb/attempt_count/event_id） |
| ai.cognition_run / ai.cognition_item | 0 / 0 | 认知采集 Run + 发现项（domain 6 值/state 六值/knowledge_id/decided_by） |
| ai.change_proposal | 0 | 分析产出的变更提案（analysis_run_id FK → ai.analysis_run） |
| ai.document_proposal | 0 | 文档提案（proposed_markdown/quality_gate_result/redline_*） |
| ai.project_knowledge | 0 | 旧知识对象（domain 6 值/status=active/superseded/retired） |
| ai.knowledge_reference | 0 | 引用账（reference_kind=evidence/decision/constraint；source_kind=analysis_run/event/manual） |
| integration.repo_analysis_report | 1 | 仓库分析报告门（status=analyzing/analyzed/closed/failed/cancelled；run_mode；kb_revision_id/pc_source_id 裸引用无 FK） |
| integration.repo_import (+source/attempt/draft_version) | 6/—/1 | 仓库导入执行链（status 10 值；analysis_run_id FK；draft jsonb 版本化） |
| integration.repository_version_confirmation | — | 仓库版本人工确认（selected_branch/commit） |
| delivery.task_run (+event/failure/cleanup/execution_authorization/repository) | 0 | 任务执行 Run（与 analysis_run 独立） |
| discovery.discovery_session (+finding/question/answer/message/proposal/evidence/repository) | 1/0/… | 发现会话套件（finding 独立实体+证据+仓库绑定） |
| delivery.requirement_finding / delivery.review_finding / design.review_finding | — | 领域内 finding（各自独立表） |

### 支撑域

- docproc（5 表）：原始文件（document_version.storage_key=file-service ID）→ 解析 job → artifact（markdown 等）→ source_mapping（原文↔产物位置映射）。
- source.source_input（1 表，0 行）：统一输入登记（input_kind=human_input/discovery/external_feedback/intake/import）。
- platform.embedding_config（1 行配置）：向量供给配置，enabled=false。
- viz（4 表）、governance（approval_request/audit_log/human_intervention/review_approval）、core/iam 外部。

## 4. 关键唯一约束（三域）

| 约束 | 表 | 意义 |
|---|---|---|
| document_tenant_id_project_id_doc_key_key | kb.document | doc_key 项目内唯一 |
| document_revision_document_id_revision_no_key | kb.document_revision | (document, revision_no) 唯一 |
| uq_kb_index_state_rev_provider / uq_kb_section_index_rev_provider_no | kb | 索引按 (revision, provider) 幂等 |
| source_tenant_id_content_hash_key | kb.source | 来源内容哈希去重 |
| memory_ref_revision_id_ref_role_target_domain_target_type_t_key | kb.memory_ref | 同修订同角色同目标唯一 |
| uq_pc_approval_pending | pc.approval | 每提案至多一条 pending 审批（部分唯一索引） |
| document_revision_document_id_revision_no_key（同名列，两 schema 均有） | pc.document_revision | 修订号唯一 |
| repo_analysis_report_intake_id_key | integration.repo_analysis_report | 一份 intake 一份报告门 |
| cognition_object_current_revision_fkey | pc.cognition_object | current_revision_id 复合 FK（tenant,project,object,revision） |

## 5. FK 交叉要点（跨 schema 真外键）

- `integration.repo_import.analysis_run_id → ai.analysis_run.id`（复合 FK 含 tenant）——导入与 Run 的硬关联。
- `integration.repo_import.discovery_session_id → discovery.discovery_session.id`。
- `ai.change_proposal.analysis_run_id → ai.analysis_run.id`。
- `discovery.discovery_finding_evidence.(import_id, repository_id, attempt_no) → integration.repo_import_source_attempt` 复合 FK。
- `discovery.discovery_proposal.(document_id, base_revision_id, document_proposal_id) → ai.project_document / ai.document_revision / ai.document_proposal`。
- **无 FK 的跨域裸引用列**：`integration.repo_analysis_report.kb_revision_id`（→kb.document_revision）、`.pc_source_id`（→pc.source）；`kb.memory_ref.target_*`；`pc.source.target_*`；`pc.evidence.target_*`；`ai.knowledge_reference.source_id`——统一为"domain/type/id/revision 四元组软引用"，由 `kb-pc-contracts::DomainRef` 在应用层校验。

## 6. 状态值全景（CHECK 约束实测，三域相关）

**KB**
- kb.document_revision.status = draft|published|rejected；analysis_mode = derived|derived_fallback|llm；confirm_consistency：published ⇔ confirmed_by+published_at 同在。
- kb.memory_revision.change_action = CREATE|UPDATE|WITHDRAW；review_status = PENDING_REVIEW|APPROVED|REJECTED|REVISION_REQUESTED；version_status = CURRENT|SUPERSEDED|WITHDRAWN|MERGED（MERGED 无写入代码）。
- kb.document_index_state.status = pending|indexed|stale|failed。
- kb.source.source_kind = markdown|docx|pdf|meeting_notes|email|im_log|design|recording|other。

**PC**
- cognition_object/relationship.lifecycle_status = identity|draft|active|deprecated|superseded。
- proposal.status = draft|submitted|approved|rejected|withdrawn|expired|applied|failed；origin_kind = ai_agent|kb_memory|manual_input|requirement_change|conflict_resolution。
- approval.status = pending|approved|rejected；write_operation.status = pending|git_committed|completed|failed|abandoned。
- cognition_version.status = published|archived。
- evidence：evidence_type = code|test_result|commit|document_section|meeting|user_statement|system_observation；verification_status = unverified|verified|refuted|conflicted；confidence = high|medium|low；分型定位器 CHECK（code/commit→file_path+commit_sha；document_section→knowledge_base+target_revision_id；陈述类→excerpt/description）。
- conflict.status = detected|under_review|awaiting_human|resolved|dismissed|reopened；conflict_type 8 值。
- pc.source.source_kind = kb_document|kb_memory|manual_input|requirement|requirement_revision|discovery_finding|project_cognition|external_reference。
- relationship_type.owner_domain = pc|kb。

**Analysis 相关**
- ai.analysis_run.status：无 CHECK（应用层枚举）；repo_import.status = queued|cloning|reading|analyzing|draft_ready|proposed|applied|rejected|failed|cancelled；draft_version.generation_status = generating|succeeded|failed。
- repo_analysis_report.status = analyzing|analyzed|closed|failed|cancelled；run_mode = incremental|full。
- ai.cognition_run.status = pending|collecting|analyzing|completed|partially_completed|failed|cancelled。
- ai.cognition_item：domain = identity|repository|domain|architecture|convention|evolution；state = proposed|confirmed|rejected|investigation|deferred|superseded。
- ai.change_proposal.status = proposed|executed|redline_pending|escalated|vetoed|expired|failed。
- ai.document_proposal.status = draft|quality_gates_passed|redline_pending|confirmed|commit_pending|committed|rejected|rolled_back。
- ai.document_revision.status = draft|published|rejected|rolled_back；ai.project_knowledge.status = active|superseded|retired。
- discovery_finding.kind = observed|user_confirmed|speculation|contradicted；verification_status = PENDING|CONFIRMED|REJECTED|CONFLICT（大写，与他处小写并存）。
- discovery_session.status = IN_PROGRESS|NEEDS_USER_INPUT|REVIEW_REQUIRED|COMPLETED|PAUSED；kind = INITIAL_ONBOARDING|UPDATE|TASK_DISCOVERY。
- source.source_input.input_kind = human_input|discovery|external_feedback|intake|import。

## 7. 触发器（53 个，三域相关节选）

不可变/状态机守卫触发器（BEFORE UPDATE/DELETE）：
- kb：document_revision（trg_kb_revision_guard：draft 内容列不可变、published 元数据不可改、状态白名单）、memory_revision（INSERT 强制 PENDING_REVIEW + 状态白名单）、memory_ref（不可变）、source（不可变）。
- pc：approval 不可变、cognition_revision 不可变、cognition_version 不可变+删禁、document_revision guard、cognition_object lifecycle 守卫+删禁、proposal anchors、proposal_member anchored（锚定后不可改成员）、relationship anchors/endpoint_match、version_member immutable/closure、write_operation state。
- 其他域：delivery.task_run（trg_review_report_immutable）、requirement/design/adr/docproc/viz revision guard 等。

视图仅 2 个：`pc.v_revision_versioned`、`delivery.v_task_delivery_manifest`。

## 8. 与审计目标无关但需知悉

- public schema 79 张表属外部 lmms 服务（iam/ntf/token/file/audit），与三域审计无关，但 `platform`（embedding_config）与 `integration`（repo_*）在 ai-pm 自己的迁移里。
- `docproc` 由 0016/0017 建立，为文档处理 V1；kb.document_revision.source_ref 指向 docproc artifact。

完整逐列清单见本次审计原始导出（`out-03-columns.txt` / `out-10-checks.txt`，保留于审计工作目录）；核心表字段明细内嵌于 02/03/04 号文件各表条目。
