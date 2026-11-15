# PM-AI 四域对象注册表与数据库映射审计总报告 V1.0

> commit `05ce33d8`（2026-11-15 19:18）· full-dev postgres（db=postgres）· 迁移 0024 · 审计执行 2026-11-15
> 任务书 V1.0 · 严格只读（零 DDL/DML、零代码修改、零 Proposal、零 Git 提交）
> 分册：A 对象注册表 / B 数据库映射 / C 生命周期矩阵 / D 关系与 DomainRef 注册表 / E 写入权与存储权威矩阵 / F 差异与待裁决清单（均在同目录）；原始证据 docs/audit/four-domain-2026-11-15/raw/ + 00_evidence_baseline.md

## 1. 总体结论

四域目标架构中，**KB 与 PC 是实现成熟度最高的两域**（唯一服务层写入面 + DB 触发器守卫 + 人工门完整 + 事件同事务 outbox），**Analysis 是结构缺口最大的域**（D01/D02/D03 三项冻结裁决的目标对象——Report Revision、统一 Run 契约/Adapter、Finding 基础契约——全部不存在），**Project Cognition 是治理门最弱的域**（正式对象唯一写入路径缺人工主体门，P0 级）。

**运行事实**：四域核心正式对象全库 0 行（kb.memory、pc 全部对象、ai.project_knowledge/cognition_item/cognition_run），真实数据仅仓库分析链残留（报告 1 行 failed、kb 文档 1、pc.source 1）。**全部四域引用链未在真实数据上走通过一次**——表结构、代码路径、单测存在 ≠ 链路闭环（任务书 §12.3 纪律的实证注脚）。

**一个时间窗红利**：U23 数据迁移负担≈0。核心对象零存量意味着 P0-1/P0-2 这类契约修复和 U01~U21 结构裁决都处在「改契约不搬数据」的最优窗口。

## 2. 四域对象数量与覆盖率

| 领域 | 注册对象 | COMPLETE | PARTIAL | STUB | 关键缺口 |
|---|---|---|---|---|---|
| KB | 8（+旧栈 6 表另册） | 6 | 1 | 1（Extraction 未落） | 正文 PG 权威偏差；memory 生成侧缺位；source↔document 无引用 |
| ANALYSIS | 8 | 0 | 5 | 3 | Report Revision/Run 契约/Finding 契约三项冻结裁决目标全缺 |
| PROJECT_COGNITION | 6+1 | 0 | 3 | 4 | 人工门缺失（P0）；死列成片；溯源粒度弱 |
| PC | 10 | 6 | 2 | 2（conflict/relationship） | 正文 PG 权威（已登记）；字典空置 |

## 3. 数据库映射统计（分册 B）

相关表约 60 张：KEEP 41 / EVOLVE 10 / MIGRATE 2 / ADAPTER 3 / 新建-接线 2 / REBUILDABLE 4 / DEPRECATE 候选 1 / **零 NEW**。双域争用唯一裁决点 ai.project_knowledge 已按写入链证据判定归 PROJECT_COGNITION（cognition 确认事务独占写入，与 kb.* 零互通）。

## 4. P0 / P1 速览（分册 F 全量）

**P0（4）**：① ai.project_knowledge 确认端点无主体门（AI 可写正式认知）；② ai.analysis_run 双写入者状态词分叉且零 DB 守卫；③ cognition 决策无审批权威承接（无审批对象/无审计事件）；④ 报告→KB/PC 引用链真实数据未走通（样本 NULL）。

**P1（9）**：ProjectCognition 四死列；冲突治理双桩；报告无 run 关联无 Revision 且 cancelled 死态；Finding 5 套异构；L3 冻结死代码+手工插行即永久冻结；关系字典越位+空置+写入零校验；ProjectCognition 溯源仅 repository 级；DomainRef 六项校验缺四项；旧 ai 栈零触发器。

## 5. 写入权审计摘要（分册 E）

- 「唯一服务层 + DB 触发器」模式在 kb/pc 成立；worker 全部经服务层写入（kb_pc_analysis→kb_service/pc::），无绕过面。
- 例外两处即两个 P0：ai.analysis_run（应用层独占+双写入者）、ai.project_knowledge（门强度不足）。
- 幂等账本三套（knowledge_write_operation/pc.write_operation/intake event_id）+ 关键事件同事务 outbox——可审计性整体成立。

## 6. 状态机审计摘要（分册 C）

- 20+ 状态机盘点完成；大小写两套词表并存（大写：kb.memory_revision/discovery/intake_*，小写：pc/kb.document_revision/delivery）。
- 死状态清单 6 项（cancelled/superseded/retired/git_committed/MERGED/succeeded-vs-completed）。
- 决策定型语义三种并存：一次性 409（cognition/memory）vs 暂记可改期（pc.proposal v2）vs 审批请求（change_proposal）。

## 7. 存储权威审计摘要（分册 E §2）

正文权威三种模式并存：Git（旧 ai 栈，合规）/ PG content（kb、pc、ai.project_knowledge）/ 借住（报告正文→kb 修订）。V2.0 §9.2 认账的四项偏差全部 CONFIRMED，另确认第五处（ai.project_knowledge，基线未列）。所有 PG 正文对象的修改通道均已收敛（触发器冻结/唯一服务层），无「双重可写」现状。

## 8. 跨域引用审计摘要（分册 D）

- DomainRef 9 domain 注册表齐（project_cognition 已预注册 ReadOnly 位格）；校验仅覆盖 domain+非空，type/id/revision/tenant/project 四项缺。
- 裸引用清单：repo_analysis_report.kb_revision_id/pc_source_id、repo_import.analysis_run_id、version_member、intake_task 两个 ref 列；强 FK 仅 3 处。
- 引用三形态并存（TEXT 四元组/裸 UUID/FK）。
- 事件 14 目录：13 有真实消费者，2 个无生产者（pc.conflict.detected、pc.evidence_requested）；跨域动作全部走 outbox，无跨域直写正式表。
- 事件级红线校验（validate_payload_references）只护事件 payload，不护任务列。

## 9. API / 事件 / 前端审计摘要

**API**：KB 域 /v1/kb/*（confirm 门+K4 review 真实）；PC 域 /v1/pc/*（P4/P5 真实，P1/P2/P6/P7 501）；报告 /v1/pm/analysis-reports*/close（Platform 权威，批量发布）；cognition /v1/pm/projects/{id}/cognition*（decision 即 project_knowledge 写点）；旧栈挂 /v1/documents（无 /v1/knowledge/* 路径）。
**前端审批权威三分**：KnowledgeView→KB/PC 域；AnalysisReportsPanel→Platform 报告 close；ReviewCenterPage→discovery/PC/delivery/报告四域。CognitionPage 独立于审阅中心。
**审阅中心**：human_action_summary 5 kind（finding×2/proposal/delivery/report）；**kb.memory 审核与 cognition_item 确认无入口**（U25 缺口）。

## 10. 十五个审计问题的回答

1. **四域各自拥有哪些正式业务对象？** KB：Source/Document/DocumentRevision/Memory/MemoryRevision/Reference/SectionIndex（+旧栈）；ANALYSIS：Run/Report（现状），ReportRevision/Finding/Evidence/Conclusion/Recommendation/InputRef 为目标态；PROJECT_COGNITION：CognitionItem(候选)/ProjectKnowledge(正式)/CognitionRun；PC：Object/Revision/Version/Proposal/Approval/Source/Evidence/WriteOperation（Conflict/Relationship 桩）。
2. **核心表归属？** 见分册 B 映射表（60 张，唯一争用点 ai.project_knowledge→PROJECT_COGNITION）。
3. **是否存在一个对象被多域当作权威？** 表级无；概念级一处：报告正文（住在 kb.document_revision，业务身份在 integration.repo_analysis_report）——正是 D01 要收敛的点。另有同名撞车（ai.project_knowledge 表 vs knowledge_write lock）。
4. **ai.project_knowledge 是什么？** 项目认知候选（ai.cognition_item）经人工决策后落地的**追加式正式项目认知对象**：insert-only、content TEXT、version/supersedes 死列、无 Git、溯源仅 source_item_id；写入链证据判定归 Project Cognition，不是 KB 知识。
5. **ai.cognition_item 与正式对象关系？** 唯一候选层：start_run 派生（结构化事实、无 LLM）→ 6 态状态机 → confirm 事务内 INSERT project_knowledge 并回填 knowledge_id。superseded 态不可达。
6. **ai.analysis_run 适合作统一 Run 基础吗？** 结构上勉强（有 event 关联/model_name/snapshot/result），治理上不合格：无 CHECK/乐观锁、双写入者词表分叉、导入链把它当一次性记账快照。D02 裁决本就要求 Adapter 而非直接复用——按契约补齐后方可作基础。
7. **repo_analysis_report 如何演进为 Analysis Report？** 三步：补 analysis_run_id（对齐 repo_import 先例）→ 独立 Report Revision（D01，正文 Markdown+Git）→ kb_revision_id 降级为兼容期历史正文引用（现状裸 UUID，样本 NULL）。
8. **kb.document_revision 是否在承担报告正文职责？** 是（CONFIRMED）：worker 写 draft 修订+报告行裸引用+close 时置 rejected；属于 D01 目标态前的过渡形态，须在基线登记为兼容契约而非权威关系。
9. **Finding 哪些可共享契约、哪些必须独立？** 可共享（D03 基础契约）：稳定 ID/type/severity 词表/来源 run 引用/evidence 引用形态（参照 pc.evidence 三形态 CHECK）；必须独立生命周期：discovery 验证流、delivery/design 评审流、（未来）Analysis Finding。
10. **跨域裸 UUID/缺 Revision 引用？** 分册 D §4：5 处裸 UUID 列、2 个手拼 JSON 任务列、TEXT 四元组（pc.source）、R-11 零填充死 FK、baseline_proposal.source_run_id。
11. **KB/PC 正文如何迁 Git？** 前置条件已备（写入通道收敛+content_hash+tsv 可重建）；PC 侧具体改动点已列（write_operation 启用 git_committed 中间态、闲置两命令落 commit-then-insert、8 处读取换源、doc_slug→canonical_path 对齐锚）；顺序 U13 待裁。
12. **pc.relationship_type 是否越权管理？** 是（CONFIRMED）：owner_domain='kb' 2 行 KB 域内边住在 PC 字典；且写入侧零 owner_domain 校验、字典 full-dev 0 行（种子被清库）。
13. **哪些正式对象存在多写入入口？** ai.analysis_run（双写入者+词表分叉，P0-2）；ai.change_proposal（双写入者+双起点）；kb.document_revision 的 draft 面（应用+worker，已收敛同一守卫面）。其余单面。
14. **哪些对象有状态机结构但无真实数据验证？** 全部 0 行对象（分册行数表）：kb.memory 系、pc 全系、ai.project_knowledge 系、repo_analysis_report 的 closed/failed 语义（唯一样本 failed）、cancelled 全系死态——一律 NO_PRODUCTION_DATA。
15. **最安全的实施顺序？** 分册 F §7：主体门（纯收严）→ analysis_run 词表统一 → Finding 词汇归一 → D01 第一刀（run 关联+Revision 设计）→ ProjectCognition 治理裁决 → Git 迁移顺序裁决 → 端到端演练（U23 窗口期红利：存量 0 行，前 4 步均无数据迁移成本）。

## 11. 下一阶段建议

1. **裁决会输入**：分册 F 已把 U01~U25 逐项对到证据，可直接作为裁决会议材料；先裁 U01/U02/U03/U04（报告族）与 U10/U15（ProjectCognition 治理），这两组分别解锁 D01 第一刀和 P0-1 修复方案。
2. **立即可做（无裁决依赖）**：P0-1 主体门、P0-2 状态词统一+CHECK、Finding 大小写归一、审阅中心接入 kb.memory/cognition 两 kind（U25 的缺口部分不依赖裁决）。
3. **端到端演练（U24）**：四场景 A/B/C/D 用真实数据走通后方可宣布任何链路完成；当前唯一走通过的是仓库分析链的前半段（到 draft_ready），close 批量发布、memory 审核链、cognition 确认链均零真实数据。
4. **运维修复**：字典/种子数据与业务数据分级（清库脚本），防 pc.relationship_type 再次丢失。

---

*审计纪律：本报告全部结论可追溯到 raw/ 系统目录导出（DB 事实）或标注 commit 的 file:line（代码事实）；架构判断均已与事实分离标注。未视作已批准基线，不授权任何重构。*
