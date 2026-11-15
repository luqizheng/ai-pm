# 05 三域交叉关系审计

> 判定口径：已实现（有表+代码+真实约束/通道校验）；部分实现（有载体但缺关键环节）；未实现（无载体或纯桩）；无法确认。

| # | 关系 | 判定 | 数据库证据 | 代码证据 |
|---|---|---|---|---|
| 1 | KB Source → KB Document 来源关系 | **部分实现** | kb.document_revision.source_ref/source_checksum（0017，指向 docproc artifact）；kb.source/source_project 与 kb.document 之间**无 FK、无引用列** | kb_service/document.rs:297-383 ingest_from_docproc（确认前按 checksum 复检 artifact，:127-164）；手工 ingest 不强制来源登记 |
| 2 | KB Document → KB Knowledge 提取形成关系 | **未实现**（两壳互不引用） | kb.document 与 kb.memory 无任何 FK/引用列；kb.memory_ref 仅允许 knowledge_base 引用指向 source | 无提取管道代码把 kb.document 内容变成 kb.memory 修订（各自独立入口） |
| 3 | KB Knowledge → Analysis Report（分析输入引用） | **部分实现（方向相反）** | repo_analysis_report.kb_revision_id（裸引用无 FK）指向的是 **kb.document_revision**（报告正文），不是 kb.memory；当前样本该列为 NULL | analysis_report.rs:50-58,318-332；kb.memory 未被任何分析链读取（检索通道 kb_section 读的是 document_section_index） |
| 4 | PC Cognition → Analysis Report（正式认知输入引用） | **未实现** | 无任何表列表达"以报告为输入的正式认知"；pc.source.source_kind 8 值中**没有 analysis_report** 类型 | derive_pc_cognition 的输入是 KB 修订与结构化事实（kb_pc_analysis.rs:815-1027），非报告实体 |
| 5 | Analysis Run → Analysis Report（执行与报告关系） | **部分实现** | repo_analysis_report 无 analysis_run_id；仅 UNIQUE(intake_id)→intake_task 间接关联；repo_import.analysis_run_id 有 FK（导入链，但导入链无报告实体） | analysis_report.rs:10-13 注释明示"经 intake_task 间接关联" |
| 6 | Analysis Report → Analysis Finding（报告与发现关系） | **部分实现** | 无 report_id→finding 关联表；仓库报告的"发现"=pc.proposal/pc.cognition_revision（经 pc.evidence 钉扎反查）；cognition_item 无 report 关联列 | analysis_report.rs:334-372（经 PC 反查） |
| 7 | Analysis Finding → PC Candidate | **已实现**（仓库分析链） | pc.proposal(origin_kind=ai_agent)+pc.proposal_member+pc.approval(pending)；分析链逐对象写 draft+proposal | kb_pc_analysis.rs:815-1027 derive_pc_cognition → proposal.rs:1392-1491 propose_from_analysis（同事务 submit） |
| 8 | PC Candidate → PC Cognition（确认转化） | **已实现** | pc.approval(approved)+pc.cognition_version/version_member+lifecycle draft→active；触发器+乐观锁全链 | publish.rs:194-215,626-653（唯一推进点，真人门）；实测 0 行（未在真实数据走通） |
| 9 | Analysis Finding → KB Knowledge（知识修订建议） | **部分实现** | 仓库链写 kb.document_revision draft（analysis_mode=derived/llm）即"知识建议"，经 confirm_revision 真人门发布；但无 finding 级对象，建议粒度=整份快照 | kb_pc_analysis.rs:495-510,683-700 insert_kb_revision |
| 10 | Analysis Report → PC Revision（认知修订依据） | **部分实现** | pc.source(source_kind=kb_document→target knowledge_base) 是 PC 修订的来源钉扎，但指向 KB 修订而非 repo_analysis_report 行；pc.evidence.evidence_hash=KB 修订指纹 | kb_pc_analysis.rs:880-899,936+ |

## 交叉审计补充发现

1. **四元组软引用是唯一统一跨域机制**：kb.memory_ref / pc.source / pc.evidence / ai.knowledge_reference 都用 domain/type/id/revision 四元组（`kb-pc-contracts::DomainRef`，registry.rs:35-90），通道状态机（DomainChannelState，NotYetOpen/PermanentlyExcluded）在应用层校验开闭（pc/evidence.rs:33-49）。ARC-11 MATCH（应用层实现，DB 层无 FK）。
2. **三套"知识对象"并存**：ai.project_knowledge（cognition_item 确认落点）、kb.memory（新 KB）、pc.cognition_object（正式认知）——互不外键、语义边界靠代码约定，存在职责交叉风险（详见 07）。
3. **跨域写入全部走目标域治理**（P7 MATCH）：KB→PC 提案经 pc.proposal；分析→KB 写 draft 经 confirm 门；分析→PC 经 proposal/publish；无任何代码直接 UPDATE 他域正式表。
4. **ai.change_proposal（导入链）→ governance.approval_request → 审计**是第四条提案-审批链（与 pc.proposal/approval、kb review 并存），词汇与状态机各异。
5. **实测引用现状**：唯一真实跨域引用样本=pc.source 1 行（target=knowledge_base/document/{kb.document.id}）；repo_analysis_report.kb_revision_id/pc_source_id 均为 NULL（该报告 failed 于"KB 分析完成，认知派生中"阶段）——三域间真实数据引用**尚未完整走通一次**。
