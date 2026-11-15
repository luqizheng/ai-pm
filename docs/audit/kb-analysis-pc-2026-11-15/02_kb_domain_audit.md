# 02 KB（Knowledge Base）领域审计

## 0. 结论速览

KB 存在**两套并行体系**：
1. **旧 ai 栈**：`ai.project_document` / `ai.document_revision`——Git 权威（先写文件再 git commit，DB 只存 sha+hash，无正文列）。代码 `crates/pm-application/src/knowledge.rs`。该栈同时服务需求文档/ADR/设计文档（canonical_path 即 Git 仓库内路径）。
2. **新 kb schema（kb_service）**：`kb.document*` / `kb.memory*` / `kb.source*`——**PG 权威**（content 列存 Markdown 全文，append-only）。代码 `crates/pm-application/src/kb_service/`，模块自述"KB 是 kb.* 唯一写入方"（kb_service/mod.rs:17-20）。

架构基线中"Source→Document→Extraction→Knowledge"四段对象在 DB 层的对应：Source=kb.source（+docproc.document/artifact），Document=kb.document+kb.document_revision，Extraction=docproc.processing_job/artifact（解析产物），Knowledge=kb.memory+kb.memory_revision。

## 1. 逐表明细

### kb.document / kb.document_revision
- 列：见 01 文件 §3。正文 `content text NOT NULL`（0001_baseline.sql:2893 附近）。
- 写入入口：
  - 手工/上传摄取：`kb_service/document.rs:541-620` `ingest_document`（REST `POST /v1/kb/documents/ingest`，apps/pm-api/src/kb_memory_api.rs:56,185-235）→ 插 draft 修订。
  - docproc 摄取：`kb_service/document.rs:297-383` `ingest_from_docproc`（引用 artifact 不复制，source_ref=`docproc:artifact:{uuid}`+source_checksum，0017）。
  - 仓库分析自动摄取：`crates/pm-infrastructure/src/kb_pc_analysis.rs:495-510,683-700` `insert_kb_revision`（source_commit_sha=head_commit_sha，analysis_mode=derived/derived_fallback/llm）——**AI 可直接写 draft 修订**。
- 确认门（人工）：`kb_service/document.rs:60-248` `confirm_revision`（REST knowledge_services.rs:58）——三重真人门（auth_mode=interactive、principal.kind=human、项目成员），draft→published + confirmed_by + outbox 事件；CHECK `document_revision_confirm_consistency` 强制 published⇔confirmed_by。0006 迁移即为此门。
- 触发器 `trg_kb_revision_guard`：draft 内容列不可变（append-only）、published 元数据不可改、状态白名单。
- 实测样本（脱敏）：1 行 published 修订，doc_key=repo-{uuid}，analysis_mode=llm，content 1223 字节"仓库快照"Markdown，source_commit_sha=e105e9b9…（真 Git 指针），canonical_path=`repos/{uuid}/repository-snapshot.md`（虚拟路径，**不对应磁盘文件**——kb_service grep 无任何 git 调用）。

### kb.memory / kb.memory_revision / kb.memory_ref
- 语义：memory=对象壳，revision=内容（body text）+ 审核 + 版本；memory_ref=修订级跨域引用。
- 提案流程：`kb_service/memory.rs:285-343` `insert_revision`（触发器强制 PENDING_REVIEW 插入）→ `propose_create`(:349) / `propose_change`(:492)。REST：POST /v1/kb/memories、/v1/kb/memories/{id}/changes。
- **AI 可以提案，不能审核**：propose 入口无 require_human（kb_memory_api.rs:313-322）；审核 `review()` 第一行即 `requirement::require_human`（memory.rs:629）。审核 CAS 校验 base_revision_id（:651-657）；approve→旧行 SUPERSEDED、新行 APPROVED + version_no=MAX+1 + version_status=CURRENT（:685-711）；reject→REJECTED；revise→REVISION_REQUESTED（:756）。outbox 事件 kb.memory.approved。
- memory_type 六类（0001:2967）；change_action=CREATE|UPDATE|WITHDRAW；version_status 含 MERGED 但无写入代码（预留）。
- kb.memory_ref：ref_role=evidence|subject（0001:2928）；target_domain 合法集=`kb-pc-contracts/src/registry.rs:35-69`（requirement/project_goal/knowledge_base/project_cognition/pc/design/adr/product_reality）；knowledge_base 引用仅允许 target_type=source（memory.rs:206-209）；必须≥1 evidence、去重、≤50 条。**当前 0 行**（无真实数据）。

### kb.source / kb.source_project
- 团队级"Original Sources"来源池；唯一入口 `kb_service/source.rs:156-210` register_source（REST POST /v1/kb/sources）。
- source_kind 9 值；uri XOR inline_content（CHECK source_check）；应用层 inline 上限 MAX_INLINE_BYTES + sha256 校验（source.rs:115-135）。**无对象存储引用**——小文本内联 PG，大文件 uri 指外部。当前 0 行。

### kb.document_section_index / kb.document_index_state（PageIndex 现状）
- 是**章节级 FTS 派生索引**（provider=`kb-section`，search_tsv=CJK bigram+to_tsvector('simple')，0008 迁移；`kb_service/indexer.rs:124-141`），**不是**基线设想的"可重建派生 PageIndex"完整形态——它是全文检索索引，含 content 列（章节正文副本）。
- 重建者唯一：`kb_service/indexer.rs:97-176` `index_sections`（同事务清旧+插章节+upsert state）；事件驱动：outbox `kb.document_revision.published` → `KbDocumentRevisionPublishedHandler`（pm-infrastructure/src/kb_pc_handlers.rs:748-810）→ `index_published_revision`；过期事件只清不覆盖。另有 `rebuild_kb_index`（indexer.rs:267）。
- embedding 向量通道**未实现**：RetrievalChannel::KbVector/PcVector 标注"WP-6 后启用"（kb-pc-contracts/src/retrieval.rs:70-76）；platform.embedding_config enabled=false（0012 注"Vector 通道 G-3 门控中"）；全仓无 pgvector。当前检索只有 pc_entity/pc_fts/kb_section 三通道（retrieval/service.rs:183-220）。

## 2. 任务书 10 问逐答

1. 表：kb.document / kb.document_revision / kb.memory / kb.memory_revision / kb.memory_ref / kb.source / kb.source_project / kb.document_section_index / kb.document_index_state（+旧栈 ai.project_document / ai.document_revision / ai.project_knowledge / ai.document_proposal）。
2. 代码：kb_service/*（新栈）、knowledge.rs + knowledge_indexer.rs + knowledge_proposal/*（旧栈）、kb_pc_analysis.rs（分析链写入）、kb_memory_api.rs / knowledge_services.rs（REST）。
3. 字段/主外键：见 01 文件。
4. 状态值：document_revision=draft|published|rejected；memory_revision.review_status=PENDING_REVIEW|APPROVED|REJECTED|REVISION_REQUESTED；version_status=CURRENT|SUPERSEDED|WITHDRAWN|MERGED。
5. 版本：document_revision 用 revision_no 单调递增（UNIQUE(document_id,revision_no)，append-only，触发器禁改）；memory_revision 双层（revision_no 提案序 + version_no 审核通过后的正式版本号）。
6. 正文位置：**kb.document_revision.content（PG，Markdown 全文，权威）**；旧 ai 栈正文在 Git（ai.document_revision 无 content 列）。kb 域无 Git 写回。
7. 来源追踪：kb.source（团队池）+ source_project（项目关联）+ document_revision.source_ref/source_checksum（docproc artifact）+ source_commit_sha（仓库快照指针）+ memory_ref（修订级跨域 evidence/subject 引用，四元组）。
8. 审核机制：document=confirm_revision 真人门（确认即发布）；memory=review() 真人门（PENDING_REVIEW→APPROVED/REJECTED/REVISION_REQUESTED，CAS+版本号推进）。审阅中心（command_center/action.rs:104-125）聚合的是 ai.document_proposal 待批与 repo 分析报告待审理——**kb.memory_revision 待审未接入审阅中心卡片**（缺口）。
9. 与其他域关联：memory_ref→7 个 target_domain；pc.source.source_kind=kb_document 钉 kb.document；pc.evidence(document_section)→knowledge_base revision；repo_analysis_report.kb_revision_id（裸引用，无 FK，当前样本未填）。
10. PageIndex：章节 FTS 索引已实现（published 事件驱动重建）；向量索引未启用。

## 3. MATCH/GAP 摘要（对照基线）

| 基线要求 | 判定 | 证据 |
|---|---|---|
| Source→Document→Knowledge 分离 | PARTIAL | source/document/memory 三壳分离，但 Extraction 无独立 KB 侧对象（借 docproc） |
| AI 生成≠人工确认 | MATCH | confirm_revision/review() 真人门 + CHECK/触发器 |
| Markdown+Git=正文权威 | CONFLICT（kb 新栈） | kb.document_revision.content 是 PG 权威，无 Git 写回；仅旧 ai 栈 Git 权威 |
| PageIndex 可重建派生 | PARTIAL | 章节 FTS 索引已实现且可重建；无向量/嵌入 |
| 引用优于复制（四元组） | MATCH | memory_ref 四元组 + registry 校验 |
| 审核待办统一入口 | GAP | memory 待审未进审阅中心 |
