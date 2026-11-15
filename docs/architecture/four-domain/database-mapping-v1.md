# 现有数据库表 → 四域目标对象映射表 V1.0（审计产出）

> commit `05ce33d8` · 2026-11-15 · 处理策略枚举：KEEP / EVOLVE / ADAPTER / NEW / MIGRATE / DEPRECATE / PROJECTION / REBUILDABLE / UNRESOLVED
> 本表覆盖与四域相关的表；platform/public 等基础设施 schema 仅列与四域有写入关联者。全部映射标注证据（表结构=迁移，写入权=代码）。
> **U01~U25 未裁决，凡涉及目标态结构的选择均标 UNRESOLVED/DECISION_REQUIRED，审计不代裁决。**

## 1. kb schema

| 当前表 | 当前用途 | 目标域 | 目标对象 | 策略 | 依据 |
|---|---|---|---|---|---|
| kb.document | 知识文档身份 | KB | KB-DOCUMENT | EVOLVE（补 source 引用关系，U12） | BL:2860；写入 kb_service/document.rs:333 |
| kb.document_revision | 文档版本+正文 | KB | KB-DOCUMENT-REVISION | EVOLVE（正文权威迁 Git=U13 顺序裁决；KBPG 偏差未登记） | BL:2878+0006；content TEXT |
| kb.memory | 知识对象身份 | KB | KB-MEMORY | KEEP | BL:2901；唯一入口 memory.rs:365 |
| kb.memory_revision | 知识版本+审核 | KB | KB-MEMORY-REVISION | KEEP | BL:2936；触发器 BL:228-276 |
| kb.memory_ref | 知识引用 | KB | KB-REFERENCE | KEEP | BL:2914；不可变触发器 |
| kb.source | 来源 | KB | KB-SOURCE | EVOLVE（与 kb.document 建立引用，当前无列） | BL:2982 |
| kb.source_project | 来源-项目多对多 | KB | KB-SOURCE 附属 | KEEP | BL:3008 |
| kb.document_section_index | 章节 FTS 索引 | KB | KB-SECTION-INDEX | REBUILDABLE（保持） | 0008 |
| kb.document_index_state | 索引台账 | KB | KB-SECTION-INDEX 台账 | REBUILDABLE | 0008 |

## 2. pc schema

| 当前表 | 当前用途 | 目标域 | 目标对象 | 策略 | 依据 |
|---|---|---|---|---|---|
| pc.cognition_object | 正式认知身份 | PC | PC-OBJECT | EVOLVE（PCGIT 落地时正文引用改 Git 锚） | BL:3041 |
| pc.cognition_revision | 认知修订 | PC | PC-REVISION | EVOLVE（同上） | BL:3064 |
| pc.cognition_version | 版本快照 | PC | PC-VERSION | KEEP | BL:3087；publish.rs:455 |
| pc.version_member | 版本成员 | PC | PC-VERSION 成员 | KEEP | BL:3418；闭合触发器 |
| pc.proposal / proposal_member | 提案 | PC | PC-PROPOSAL | KEEP | BL:3240/3271 |
| pc.approval | 审批 | PC | PC-APPROVAL | KEEP | BL:3021 |
| pc.source | 认知来源 | PC | PC-SOURCE | KEEP | BL:3391 |
| pc.evidence | 证据 | PC | PC-EVIDENCE | KEEP（verification 无人工程序=P2 缺口） | BL:3182 |
| pc.document / document_revision | 认知正文 | PC | PC-REVISION 正文载体 | MIGRATE（PCGIT：迁 Markdown+Git；git_commit_sha 恒 NULL 已登记） | BL:3149/3163；object.rs:233/413/479 |
| pc.relationship / relationship_revision | 关系 | PC | PC-RELATION | UNRESOLVED（有代码无生产调用者；先补字典数据与调用链，或 DEPRECATE，U18） | BL:3293/3327 |
| pc.relationship_type | 关系字典 | PC | PC-RELATION 字典 | EVOLVE（种子须与业务数据分级防清库丢失；owner_domain='kb' 2 行归属待 U18） | 0007:47-74 |
| pc.object_type | 对象字典 | PC | PC-OBJECT 字典 | KEEP（纯字典，代码只读） | BL:3220 |
| pc.conflict / conflict_object | 冲突 | PC | PC-CONFLICT | NEW-接线（表 KEEP，检测写入者与 resolve 流程需新建，U17） | BL:3108/3134 |
| pc.write_operation | 发布幂等账本 | PC | PC-PUBLISH | EVOLVE（启用 git_committed 中间态与闲置命令） | BL:3453；publish.rs:112 |

## 3. ai schema（混合归属）

| 当前表 | 当前用途 | 目标域 | 目标对象 | 策略 | 依据 |
|---|---|---|---|---|---|
| ai.analysis_run | 事件分析执行 | ANALYSIS | AN-RUN | ADAPTER（V2.0 D02 裁决：适配为统一 Run 契约；补状态 CHECK/乐观锁、统一 succeeded/completed 分叉=P0） | BL:675；双写入者 pm-ai/context.rs vs project_import/proposal.rs:312 |
| ai.project_document / document_revision | 旧知识文档（Git 权威） | KB（旧栈） | 旧栈 Document | KEEP（兼容层待 U22；勿与 kb.* 混同） | knowledge.rs:137-191 |
| ai.document_proposal | 旧栈文档提案 | KB（旧栈） | 旧栈 Proposal | KEEP | 8 态 BL:893 |
| ai.document_index_state / document_section_index | 旧栈索引 | KB（旧栈） | REBUILDABLE | KEEP | 0001 |
| ai.knowledge_reference | 知识引用（旧栈） | KB（旧栈） | — | DEPRECATE 候选（仅测试调用，UNVERIFIED 是否有生产计划） | BL:979 区段 |
| ai.knowledge_write_operation | 旧栈写幂等账本 | KB（旧栈） | — | KEEP | knowledge_write_coordinator.rs:164 |
| ai.cognition_item | 项目认知候选 | PROJECT_COGNITION | PROJ-COG-PROPOSAL | EVOLVE（补主体门=P0；superseded 态接线） | BL:770；repo_cognition.rs:161 |
| ai.cognition_run | 项目认知执行 | PROJECT_COGNITION | （执行实体） | ADAPTER 候选（D02 下可映射统一 Run；repo_versions 需记具体 revision） | BL:796 |
| ai.project_knowledge | 项目正式认知 | PROJECT_COGNITION | PROJ-COG-OBJECT | EVOLVE（补人工门、启用 version/supersedes 或删死列） | BL:1101；repo_cognition.rs:263 |
| ai.change_proposal | 导入链命令提案 | ANALYSIS（AN-RECOMMENDATION 现状载体） | AN-RECOMMENDATION | EVOLVE（或移 delivery 域，语义=命令提案非建议，UNRESOLVED） | BL:742；orchestrator.rs:240 |
| ai.baseline_proposal / baseline_item | 基线裁决 | 非 V2.0 四域（需求/基线域） | — | KEEP（不映射） | BL |
| ai.conversation* / llm_call_record 等 | 对话/模型记账 | 非四域 | — | KEEP | BL |

## 4. integration schema

| 当前表 | 当前用途 | 目标域 | 目标对象 | 策略 | 依据 |
|---|---|---|---|---|---|
| integration.repo_analysis_report | 仓库分析报告 | ANALYSIS | AN-REPORT | EVOLVE→MIGRATE（D01：补 analysis_run_id、独立 Report Revision+Git 正文；kb_revision_id 兼容期保留为历史正文引用） | 0023/0024；analysis_report.rs |
| integration.repo_import (+source/_attempt/draft_version) | 导入工作流 | ANALYSIS 关联（保留工作流） | （原始执行实体） | KEEP+ADAPTER（V2.0 §3.4 明确保留，关联 Analysis Run） | BL:2754；lifecycle.rs:197 |
| integration.intake_record / intake_task | 收件/任务分派 | 集成域（分析链执行载体） | — | KEEP（intake_task.result_ref 已承载 result_refs 概念） | BL:2644；intake/task.rs:48 |
| integration.outbox_event | 事件总线 | 集成域 | — | KEEP | BL:2704 |
| integration.project_repository / project_ssh_key / repository_version_confirmation | 仓库接入 | 集成域 | — | KEEP | BL |

## 5. delivery / design / discovery schema（Finding 与执行链）

| 当前表 | 当前用途 | 目标域 | 目标对象 | 策略 | 依据 |
|---|---|---|---|---|---|
| delivery.task_run (+event/failure) | 交付/评审执行 | ANALYSIS 关联（保留工作流） | （原始执行实体） | KEEP+ADAPTER | BL:1894；run_executor.rs |
| delivery.review_finding | 评审发现 | Delivery 保留（D03 领域专用） | 领域 Finding | KEEP（纳入统一基础契约=U05） | BL:1660；review/worker.rs:237 |
| delivery.requirement_finding | 需求-发现挂链 | Delivery | （关联表） | KEEP | BL:1604 |
| delivery.milestone / requirement / task | 交付对象 | 非 V2.0 四域 | — | KEEP | BL |
| design.review_finding | 设计评审发现 | Design 保留 | 领域 Finding | KEEP（纳入契约=U05） | 0010:234；design/finding.rs:58 |
| design.* 其余 | 设计域 | 非 V2.0 四域 | — | KEEP | 0010 |
| discovery.discovery_finding / finding_evidence | 探索发现+证据 | Discovery 保留 | 领域 Finding（证据形态最全） | KEEP（纳入契约=U05；大小写归一=P2） | BL:2347/2368 |
| discovery.discovery_session 等 | 探索工作流 | Discovery | （原始执行实体） | KEEP+按需 ADAPTER | BL:2465 |

## 6. 其余相关 schema

| 当前表 | 目标域 | 策略 | 依据 |
|---|---|---|---|
| governance.approval_request / review_approval / audit_log | 治理域（供四域审批/审计共用） | KEEP | BL:2485；approval.rs:97 |
| docproc.document / document_version / artifact | 文档处理（KB Extraction 的现状来源） | KEEP+EVOLVE（kb.document_revision.source_ref 指向 artifact，U08 裁决 Extraction 对象时对齐） | 0017 |
| core.project / principal / tenant | 权限底座 | KEEP | BL |
| viz.* | 非 V2.0 四域 | KEEP | BL |
| platform.embedding_config | 派生通道配置 | KEEP | BL |

## 7. 映射统计

- 参与映射的相关表：约 **60 张**（含附属表）；KEEP 41 / EVOLVE 10 / MIGRATE 2（pc 正文、repo_analysis_report 目标态）/ ADAPTER 3（analysis_run、cognition_run、repo_import 关联）/ STUB-新建 2（pc.conflict 接线、KB-EXTRACTION）/ REBUILDABLE 4 / DEPRECATE 候选 1。
- **零 NEW（全新表）**：目标态中需新建的 AN-REPORT-REVISION / AN-FINDING 契约 / PROJ-COG-REVISION 等均可由现有表 EVOLVE 或在 U01~U21 裁决后再定，本审计不擅自标 NEW。
- 双域争用唯一裁决点：ai.project_knowledge（→PROJECT_COGNITION，写入链证据决定，不归 KB——它由 cognition 确认事务写入且仅被 cognition/AI 上下文读，与 kb.* 零互通）。
