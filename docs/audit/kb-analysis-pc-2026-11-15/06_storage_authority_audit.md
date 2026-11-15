# 06 存储权威审计（Markdown / Git / PostgreSQL / Object Storage / PageIndex）

## 1. 正文权威现状：三套栈三种权威

| 栈 | 表 | 正文权威 | 证据 |
|---|---|---|---|
| ai（旧知识/需求/设计文档） | ai.project_document / ai.document_revision | **Git**：先写文件→git add/commit→DB 记 sha；DB 无 content 列 | knowledge.rs:609-635 `write_revision`（fs::write→git add/commit）；0001:907-930（git_commit_sha NOT NULL，无 content）；读历史 `git show {sha}:{path}`（knowledge.rs:736-747） |
| kb（新知识库） | kb.document_revision | **PostgreSQL**：content 列=Markdown 全文，append-only，无 Git | kb_service/document.rs:351 直插 content；kb_service/ 全目录无 git 调用；canonical_path=`kb/{doc_key}` 为虚拟字符串（实测样本 `repos/{uuid}/repository-snapshot.md` 不对应磁盘文件） |
| pc（产品认知） | pc.document_revision | **PostgreSQL**：content 列唯一权威；git_commit_sha 恒 NULL（PCGIT 后置偏差，显式登记 pc/mod.rs:25-27） | object.rs:413-420,479-486 写入不带 git_commit_sha；write_operation 有 pc_commit_* 命令但 status=git_committed 不可达 |

**双重权威判定**：
- kb / pc：无双权威（正文单份在 PG content；cognition_revision 只存 content_hash）。
- ai：存在**受控投影**——delivery.requirement_revision.content(jsonb) 投影 Git 正文，靠"哈希相等不变量"+`rebuild_projection`（requirement_projection.rs:287-328，按 git ref 读正文→哈希校验→UPDATE，漂移显式拒绝）维持；PG 不是第二权威而是缓存投影，机制成立但有漂移检测依赖。
- **基线 P5"Markdown+Git=正文权威"在 kb/pc 两栈 CONFLICT**（kb 无 Git 通道；pc 有列无实现）。

## 2. 对象存储（Object Storage）

- docproc.document_version.storage_key / docproc.artifact.storage_key = **file-service 文件 ID**（RustFS 由 file-service 托管，0016:58,149）。
- 端口 `ArtifactStore`（put/get/presign，docproc_service.rs:14-24）；适配器 `FileServiceStore`（pm-infrastructure/src/docproc_store.rs:20-40，digua_file_sdk + per-tenant JWT；缺 SERVICE_BOOTSTRAP_TOKEN 诚实降级 503）。
- 上传：file-service 先落字节再写 DB（docproc_service.rs:214-218）；读取按 storage_key（:918-944）。
- kb.source：uri XOR inline_content（CHECK），小文本内联 PG、大文件 uri 外链；**不经对象存储**。
- 原始上传文件（非 docproc 管道）不经 ai-pm 自有表。

## 3. PageIndex / 检索索引

| 索引 | 表 | 写入者 | 性质 |
|---|---|---|---|
| ai 章节索引 | ai.document_section_index（provider=db-section） | knowledge_indexer.rs:445-455（从 Git 工作区文件解析章节树） | 全文派生索引（章节行含 content） |
| kb 章节索引 | kb.document_section_index（provider=kb-section，CJK bigram FTS） | kb_service/indexer.rs:97-176（published 事件驱动，同事务清旧+插章节+upsert state；过期事件只清不覆盖） | 全文派生索引，**可重建**（rebuild_kb_index :267） |
| pc FTS | pc.document_revision.search_tsv 派生列 | 0009 迁移+recompute_pc_revision_tsv | DB 内派生 |
| 向量 | kb/ai.document_index_state（status 含 indexed，预留 embedding 模型版本要素） | **无实现**：RetrievalChannel::KbVector/PcVector"WP-6 后启用"；platform.embedding_config enabled=false（0012"G-3 门控中"）；全仓无 pgvector | 未启用 |

当前检索实际通道：pc_entity / pc_fts / kb_section 三通道 RRF 融合（retrieval/service.rs:183-220）。
**基线 P5"PageIndex=可重建派生索引"：PARTIAL**——FTS 索引已实现且可重建，但"章节级引用定位（PageIndex 原义）"与向量通道未实现；kb.document_index_state 目前只服务 FTS 有效性五联判定。

## 4. 写操作账本（幂等与防重）

| 账本 | 域 | 命令 | 消费方 |
|---|---|---|---|
| ai.knowledge_write_operation | 旧 ai 栈 | create_document/submit_proposal/confirm_redline/veto_revision/bind_requirement_projection | knowledge_write_coordinator.rs:164-235 + 项目级写锁（requirement_projection.rs:75-102，崩溃重入三路幂等） |
| pc.write_operation | PC | pc_commit_object_revision/pc_commit_relationship_revision/pc_publish_version | pc/publish.rs:110-166（operation_id 幂等键；git_committed 不可达） |

## 5. 版本管理总览

| 对象 | 版本机制 |
|---|---|
| ai.document_revision | revision_no + git_commit_sha（Git 真版本） |
| kb.document_revision | revision_no 单调 append-only（触发器禁改）；无跨版本 diff/回滚通道（rolled_back 状态仅 ai 栈有） |
| kb.memory_revision | 双层：revision_no（提案序）+ version_no（审核通过的正式版本号）+ version_status（CURRENT/SUPERSEDED/WITHDRAWN/MERGED） |
| pc.cognition_revision / cognition_version | revision_no（对象内）+ cognition_version 快照（全局 version_no+snapshot_hash+predecessor 链） |
| integration.repo_import_draft_version | (import_id, revision) 每次再生成一版 draft jsonb |
| repo_analysis_report | **无版本列**（重分析=新行） |
| delivery/design/plan revision 表 | 各自 revision_no + guard 触发器 |

**ARC-14（历史版本不得静默覆盖）**：MATCH——所有 revision 表 append-only + 触发器；唯一例外位：repo_analysis_report 无版本概念（按"一次运行一份报告"规避）。

## 6. 跨项目/租户隔离

- 全部业务表带 tenant_id（kb.source 租户级经 source_project 关联项目）。
- 新栈（kb_service/pc）每查询显式 `tenant_id=$1 AND project_id=$2`；旧 ai 栈以 project_id 为查询键、tenant_id 由 project 反查派生（knowledge.rs:424-438），隔离靠 project 归属；pc/evidence.rs:51-54 从 project 反查 tenant（跨租户 project_id 注入→NotFound）。
- 未发现跨项目数据隔离直接漏洞；风险点见 07 §G-7。

## 7. 前端边界

无绕过通道：前端统一走 `VITE_API_BASE`/`/api` REST（ai-frontend/src/api/base/client.ts:33），无 DB/对象存储直连代码。
