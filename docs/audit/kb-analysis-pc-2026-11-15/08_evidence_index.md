# 08 证据索引（Evidence Index）

> 所有结论的证据锚点。DB 事实以 full-dev 库（superpowers-full-postgres-1，库 postgres）只读查询为准；代码路径相对仓库根 `D:\kudi-project\templates\ai-pm`。
> 行号为本仓 2026-11-15 master（05ce33d8）快照。

## 1. 数据库证据

| 结论 | 证据 |
|---|---|
| 迁移 24 条全绿、无 ENUM 类型 | `public._sqlx_migrations`；`pg_enum` 0 行 |
| 迁移机制唯一=sqlx | apps/pm-api/src/main.rs:28 |
| 19 schema / 209 表 / 2228 列 / 1398 FK / 612 索引 / 53 触发器 / 2 视图 | information_schema + pg_catalog 系统目录导出（本次审计 out-01~out-08） |
| 全部状态合法值 | pg_constraint CHECK 导出（out-10） |
| repo_analysis_report 无 analysis_run_id、kb_revision_id/pc_source_id 无 FK | migrations/0023_repo_analysis_report.sql:16-33；pg_constraint 该表仅 7 条（PK/UNIQUE/FK×3/CHECK×2） |
| repo_import.analysis_run_id **有**复合 FK（fk_repo_import_tenant_analysis） | out-05-fk.txt 实测 + migrations/0001_baseline.sql:2765；对比：repo_analysis_report 无 analysis_run_id 列 |
| pc 状态机 CHECK/触发器全集 | 0001_baseline.sql:3033-3479；触发器 trg_pc_* 15 个 |
| kb 状态机/守卫 | 0001_baseline.sql:2878-3006 + 0006 + 触发器 trg_kb_* 8 个 |
| discovery 状态大小写混用 | out-10（discovery_finding_verification_status_check 用大写值） |
| 行数统计（kb.memory=0、pc.*=0 等） | out-11-counts.txt |
| 脱敏样本 | out-12-samples.txt（kb 快照修订 1 行 published/llm；pc.source 1 行 target=knowledge_base；repo_analysis_report 1 行 failed；repo_import 6 行；draft jsonb 15239 字节） |

## 2. 代码证据（按域）

### KB
- kb_service/mod.rs:17-20（唯一写入方自述）；kb_service/document.rs:60-248（confirm_revision 真人门）、:297-383（ingest_from_docproc）、:402-524（读）、:541-620（ingest_document）、:127-164（checksum 复检）；kb_service/memory.rs:43-65（ref_role 枚举）、:174-209（ref 校验）、:285-343（insert_revision）、:349-414/:492（propose）、:618-780（review+CAS）、:629（require_human）；kb_service/indexer.rs:97-176/:180/:267/:366；kb_service/source.rs:115-135/:156-210；kb_service/port.rs:69-101,116
- 旧栈：knowledge.rs:183-209,424-470,601-645,609-635,736-747（Git 写读）；knowledge_indexer.rs:300-309,379,445-455,548,578-586；knowledge_write_coordinator.rs:164-235；knowledge_reference.rs:62；knowledge_proposal/submit.rs:199
- API：apps/pm-api/src/kb_memory_api.rs:45-56,185-235,296-340；knowledge_services.rs:52-86；routes.rs:278
- 分析链写入：pm-infrastructure/src/kb_pc_analysis.rs:495-510,683-700（insert_kb_revision）

### Analysis
- ai.analysis_run：pm-ai/src/context.rs:143-161,287-303；pm-domain/src/proposal.rs:30-50（AnalysisResult/Finding/Proposal）；pm-ai/src/self_review.rs:33-40,82；project_import/proposal.rs:312-324,356-363；pm-ai/src/orchestrator.rs:326,506-538,605-615
- repo_analysis_report：migrations/0023:16-33、0024:14-19；pm-application/src/analysis_report.rs:1-16,10-13,50-58,93-105,203,222-250,318-372,387-551,410-414,416-444,447-501,504-539,560-567；repository/repo.rs:782-798,813-899；repository/mod.rs:76-78
- repo_import：project_import/lifecycle.rs:530-542,611-629,799-800,867；project_import/advance.rs:20-100；pm-domain/src/import.rs:61-185（ProjectDraft）；apps/ai-pm-worker/src/main.rs:63,90-96
- cognition：repo_cognition.rs:202-210,249-261,272-283；apps/pm-api/src/pm_cognition.rs:12,279-310
- findings：discovery_context.rs:1563-1578,1647；requirement_traceability.rs:306；review/worker.rs:237；review/decide.rs:706；design/finding.rs:58-79
- 报告门 409：repository/mod.rs:76-78 + repo.rs:813-825；前端深链（见记忆 repos-status-report-link）

### PC
- pc/mod.rs:16-31（分工+偏差登记）、:41（单例骨架）、:113-122（snapshot_hash）、:152-176（ensure_formal_authorized）、:180-203（frozen_by_conflict）、:213,244（字典）
- object.rs:188,206-270,413-426,469-491；relationship.rs:1-25,360；evidence.rs:27-79,33-49,121-152,154-175,176-208；proposal.rs:1-31,13-16,645,820-860,977-982,1000-1047,1014-1028,1029-1039,1392-1491；publish.rs:100,110-166,112,181-215,194-215,288,380-448,450-468,469-500,502-535,597-618,612,626-653
- 冲突桩：outbox.rs:326-331（producer=桩）；kb_pc_handlers.rs:895-923（PcConflictDetectedHandler）
- 分析链派生：kb_pc_analysis.rs:815-1027,831-875,880-899,927,936+

### 存储权威/跨域
- docproc：docproc_service.rs:14-24,214-218,918-944；pm-infrastructure/src/docproc_store.rs:20-40；0016:58,149
- 检索：kb-pc-contracts/src/retrieval.rs:70-76；retrieval/channels.rs:240-268；retrieval/fusion.rs:26；retrieval/service.rs:183-220；platform_ops（pm-application/src/repo/platform_ops.rs:31；pm-infrastructure/src/repo_platform_ops.rs:67）
- 四元组：kb-pc-contracts/src/registry.rs:35-90；kb-pc-contracts/src/domain_ref.rs:40-66；discussion_memory.rs:22-91
- 隔离：pc/evidence.rs:51-54；knowledge.rs:424-438,484,515,553
- 事件管线：kb_pc_handlers.rs:725,748-810,895-923,1245
- 前端边界：ai-frontend/src/api/base/client.ts:33；session.ts:67,124,145；knowledge.ts:69,279

## 3. 迁移文件证据

| 迁移 | 支撑结论 |
|---|---|
| 0001_baseline.sql | 三域全部基线表+CHECK+触发器（行号锚点：kb 2878-3006；pc 3033-3479；ai.analysis_run/cognition 742-808；种子 10255-10291） |
| 0006 | kb 确认即发布门（confirmed_by/published_at+守卫） |
| 0007 | pc.relationship_type.owner_domain + 8 种子（3 条 kb） |
| 0008 | kb 章节 FTS 索引 + index_state |
| 0009 | pc FTS 派生列 |
| 0012 | embedding 配置（enabled=false） |
| 0016/0017 | docproc + kb↔docproc source_ref |
| 0023/0024 | repo_analysis_report 全列清单（无 analysis_run_id）+ run_mode |

## 4. 数据样本（脱敏，out-12-samples.txt）

- kb.document_revision：1 行，status=published，analysis_mode=llm，source_commit_sha=e105e9b9…，content 1223B「仓库快照：ai-pm主仓库」（含 Git 远端 URL，样本中已属非敏感仓内地址）。
- kb.document_section_index：6 行，provider=kb-section。
- pc.source：1 行，target_domain=knowledge_base / target_type=document / source_kind=kb_document。
- integration.repo_analysis_report：1 行，status=failed，stage_detail=「KB 分析完成，认知派生中」，run_mode=incremental，kb_revision_id/pc_source_id=NULL。
- integration.repo_import：6 行（draft_ready 1 / failed 3 / cancelled 2），model_name 记录真实模型（deepseek-chat/qwen2.5:3b/fake）；draft_version 1 行 succeeded。
- discovery_session：1 行 INITIAL_ONBOARDING/IN_PROGRESS。
- kb.memory*、pc.cognition_*、ai.analysis_run、ai.cognition_*、discovery_finding 等：**0 行，无样本**（未伪造）。
- 三域间真实引用：仅 pc.source→kb.document 一条实例；报告门 kb_revision_id/pc_source_id 未填——三域引用链在真实数据上**尚未完整走通**。
