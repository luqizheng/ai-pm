# 写入权与存储权威矩阵 V1.0（审计产出）

> commit `05ce33d8` · 2026-11-15 · 只读审计产物
> 写入者分级：**正式**（业务命令/服务）· **初始化**（种子/引导）· **派生**（worker/索引器）· **兼容**（旧链）· **测试**（#[cfg(test)]/tests/，不计入治理面）
> 存储权威判定纪律：不得因存在 canonical_path/git 相邻列就认定 Git 权威——以实际读写路径为准。

## 1. 写入权矩阵

### 1.1 KB 域

| 对象 | 正式写入 Service | Repository/SQL 位点 | API | Worker | Trigger/DB 守卫 | 其他写入者 | 多写入者风险 |
|---|---|---|---|---|---|---|---|
| kb.document | kb_service/document.rs:333 ingest_from_docproc、:570 ingest_document | 同文件（upsert ON CONFLICT） | /v1/kb 文档摄入 | kb_pc_analysis.rs:495（upsert updated_at） | 唯一约束 BL:5234 | 测试种子 discovery_context.rs:1563 | 幂等 upsert，无越权风险 |
| kb.document_revision(draft) | document.rs:351 | document.rs | /v1/kb 摄入 | kb_pc_analysis.rs:683 insert_kb_revision（DISPATCH_SERVICE_PRINCIPAL） | trg_kb_revision_guard（0006）；CHECK BL:2893 | tests | draft 双写入方已收敛到同一守卫面 |
| kb.document_revision(published) | **唯一门 confirm_revision document.rs:60** | CAS :199-202 | POST /v1/kb/documents/{id}/revisions/{rid}/confirm（kb_memory_api.rs:48） | worker 可置 rejected（analysis_report.rs:481，0023 白名单） | confirm_consistency CHECK | — | 无绕过面（全仓无 UPDATE content） |
| kb.memory(+revision/ref) | propose_create/insert_revision/review（memory.rs:365/290/685-756） | 同文件 | POST /v1/kb/memories、/{id}/changes、/{id}/review | **无任何自动写入** | guard_memory_revision BL:228-276 | tests | 唯一 INSERT 面；生成侧缺位见 object-registry 附注 |
| kb.source | source.rs:168 | 同文件 | 来源注册端点 | 无 | trg_kb_source_immutable | tests | 无 |
| 索引两表 | indexer.rs:127/151/297 | 同文件 | 无 REST | KbDocumentRevisionPublishedHandler（kb_pc_handlers.rs:793） | CHECK | — | 派生专用，REBUILDABLE |

### 1.2 旧 ai 知识栈

| 对象 | 正式写入 | 通道 | DB 守卫 | 风险 |
|---|---|---|---|---|
| ai.document_revision | knowledge.rs create_document:424 / finalize_committed（veto.rs:208-289） | MCP 工具 pm_create_document（pm-mcp/tools.rs:431）→ draft；真人 confirm → Git 提交+published | **无触发器**（ai.* 零触发器），守卫全应用层 | 应用层门齐全（require_human:298 + 幂等账本 claim_operation:164） |
| ai.document_proposal | knowledge_proposal/submit.rs:199 | REST | 8 态 CHECK | 无 |
| 系统投影（requirement/goal） | requirement_projection.rs:182/197、project_goal.rs:69、command_executor/executor.rs:723 | 内部 | 无 | 兼容写入面，与 proposal 链并存（写入路径已登记） |

### 1.3 ANALYSIS 域

| 对象 | 正式写入 | 位点 | DB 守卫 | 多写入者风险 |
|---|---|---|---|---|
| ai.analysis_run | ① pm-ai 编排 start_run/save_snapshot/finish_run/fail_run（context.rs:80-165/274/291/312）② 导入链一步插 succeeded（project_import/proposal.rs:312-316） | 直 SQL | **无 CHECK、无触发器、无乐观锁** | **P0：状态词分叉**（completed/failed vs succeeded），start_run 幂等复用只认 completed，导入链行游离于其语义外 |
| integration.repo_analysis_report | analysis_report.rs：ensure_for_intake:121 / mark_kb_stage:203 / mark_analyzed:222 / mark_failed:253 / close_report:387 | 直 SQL（WHERE+FOR UPDATE） | 仅 CHECK（0023）+ 部分索引 | 单文件收敛，调用点在 kb_pc_handlers（846/879/917/1044）；**cancelled 无写入者** |
| integration.repo_import | lifecycle.rs:197（创建）/456/738/1003（attempt）、proposal.rs:374（proposed+回填 run）、advance.rs:54/65/99、archive.rs:118 | 直 SQL | 10 态 CHECK | 多位点但状态机单向，风险低 |
| delivery.task_run | run_dispatch/port.rs:30、review/queue.rs:306、review/worker.rs:379；状态机 run_executor.rs:84/169/729/780 | 直 SQL | 7 态 CHECK + 租约列 | review run 与 delivery run 共表（run_kind 区分），写入面清晰 |
| discovery_finding | discovery/finding.rs:418/433/490；验证 set_finding_verification_status_tx:672（仅 discovery/web.rs:119 可调） | 直 SQL | 大写 4 态 CHECK | 收敛 |
| ai.change_proposal | pm-ai orchestrator.rs:240-256；导入链 proposal.rs:337（直接 redline_pending） | 直 SQL | 7 态 CHECK | 双生产写入者、**初始状态不同**（proposed vs redline_pending）——语义分叉登记 |
| governance.approval_request | project_import/proposal.rs:357-367 | 直 SQL | proposal_id UNIQUE | 专属 change_proposal，不覆盖 cognition 决策（见 §1.4） |

### 1.4 PROJECT_COGNITION 域

| 对象 | 正式写入 | 位点 | API | DB 守卫 | **门强度问题** |
|---|---|---|---|---|---|
| ai.cognition_run/item | repo_cognition.rs start_run:161-187 | apps/pm-api/pm_cognition.rs:96-161 | POST /cognition/runs | 7/6 态 CHECK | 仅 require_project_visible（pm_cognition.rs:96 起）——AI 主体可发起派生（候选层，可接受但应登记） |
| cognition_item 决策 + **ai.project_knowledge INSERT** | repo_cognition.rs decide_item:279-293（同事务 INSERT project_knowledge:263-276、回填 knowledge_id、confirmed_count+1） | apps/pm-api/pm_cognition.rs:279-318 | POST /cognition/runs/{run}/items/{item}/decision | 6 态 CHECK+FOR UPDATE | **P0：仅成员可见性门**；未调 require_trusted/require_human；对照 baseline 确认三重门（pm_baseline.rs:183-238）缺失 auth_mode+kind 校验——持 api_key 的项目成员 AI 可写正式认知 |
| （测试/无） | — | — | — | — | tests 仅覆盖 non_member 404（tests/pm_cognition.rs:210），无 AI 拒绝用例 |

### 1.5 PC 域

| 对象 | 正式写入 | 位点 | DB 守卫 | 说明 |
|---|---|---|---|---|
| pc.cognition_object/revision/document* | pc 服务层 object.rs:208/115/154/187/264/246/427/492/223/469 | ←worker（kb_pc_analysis.rs:913）+identity.rs:161 | 14 个触发器（生命周期/锚定/不可变） | draft 面多通道收敛于唯一服务层（mod.rs:1-18 自述唯一业务写入面） |
| pc.cognition_version/version_member | **仅 publish.rs:455/473** | — | 不可变+delete 禁止触发器 | 唯一发布事务 |
| pc.proposal/approval | proposal.rs:644/810/1064/1181/819/1000/1047；close 批量 analysis_report.rs:458 | P5 端点 + close | 8 态 CHECK+锚定触发器+uq_pc_approval_pending | decide 侧 allow submitted/approved/rejected 互转（审阅流 v2） |
| pc.source/evidence | evidence.rs:56/177 ←worker kb_pc_analysis.rs:880/936 | — | source 不可变触发器 | worker 专用 |
| pc.write_operation | publish.rs:112/287/611 | — | 状态触发器 BL:458-467 | 3 命令中仅 1 个在用 |
| pc.relationship(_revision) | create_relationship_draft relationship.rs:333/360 —— **无生产调用者** | — | 端点守卫触发器 | 桩 |
| pc.conflict(_object) | **无生产写入者**；事件常量无生产者 | — | CHECK | 桩（L3 冻结守卫=死代码） |
| 字典两表 | 仅迁移种子（0001:10255-10291、0007） | — | — | full-dev 实测 0 行（清库丢失） |

### 1.6 多写入者越权风险总结

1. **唯一确认门强度不足**：ai.project_knowledge（§1.4，P0）。
2. **状态权威裸奔**：ai.analysis_run 无 DB 状态约束且双写入者词表分叉（P0，治理面=应用层纪律）。
3. 桩写入面：pc.conflict / pc.relationship / KB-EXTRACTION / AN-REPORT-REVISION / PROJ-COG-REVISION(-VERSION-RELATION)——表结构先于实现，不构成越权，但构成「表存在≠功能存在」的证据（V2.0 §12.7）。
4. 除上述外，四域核心正式对象均收敛到唯一服务层写入面 + DB 触发器，未发现绕过领域服务的直接 SQL 写入（worker 也经服务层）。

## 2. 存储权威矩阵

| 对象 | 正文位置 | Git Commit/Hash | 状态权威 | 版本权威 | 历史找回 | Hash 可验证 | 无 Revision 改正文入口 | 判定 |
|---|---|---|---|---|---|---|---|---|
| kb.document_revision | **PG content 列** | source_commit_sha=仓库源锚（非内容权威）；content_hash 有 | PG（CHECK+触发器） | revision_no 单层 | ✓（修订行永不删） | ✓ | ✗（content 列触发器冻结） | **PG=正文权威（偏离 §9.1）** |
| ai.document_revision（旧栈） | **Git**（KnowledgeRoot） | git_commit_sha 真实+读时 hash 校验（knowledge.rs:539-577） | PG | revision 链 | ✓（git show :742） | ✓ | ✗（先 commit 后落账） | **Git=正文权威（合规）** |
| pc.document_revision | **PG content 列** | git_commit_sha 恒 NULL（object.rs:233/413/479 不绑列） | PG | revision_no | ✓ | content_hash 有 | ✗（0009 守卫） | **PG=正文权威（PCGIT 已登记后置）** |
| ai.project_knowledge | PG content 列 | 无 Git 列 | PG | version 死列 | ✗（insert-only 无修订） | 无 hash | —（无修改入口，也无版本链） | **PG 正文权威 + 版本机制未接线** |
| repo_analysis_report（报告正文） | **借 kb.document_revision.content** | 无 | PG（报告行） | ✗ 无 Revision | 经 kb_revision 找回（裸引用可悬空） | — | —（正文修改=走 KB 新修订） | **域归属未定+版本缺失（D01 目标态：MIGRATE）** |
| ai.cognition_item / project_knowledge 证据 | evidence jsonb | 无 | PG | ✗ | — | — | — | 定位粒度 repository 级（弱溯源） |
| pc.evidence | 结构化定位列 | commit_sha 真实（代码类强制） | PG | evidence_hash | ✓ | ✓ | — | **合规（最完整证据模型）** |
| 派生索引（kb/ai section_index、pc.search_tsv、FTS） | — | — | 台账表 | 可重建 | — | — | — | REBUILDABLE（合规 §9.4） |

## 3. 结论

1. **正文权威现状 = 三种模式并存**：Git（旧 ai 栈）/ PG（kb、pc、project_knowledge）/ 借住（报告正文）。V2.0 §9.2 认账的四项偏差全部 CONFIRMED，另新增确认 ai.project_knowledge 为第五处 PG 正文权威（基线未列）。
2. **写入权威 = 「唯一服务层 + 触发器」模式总体成立**，两处例外：ai.analysis_run（无守卫+双写入者）、ai.project_knowledge（门强度不足）。
3. 幂等账本三套（ai.knowledge_write_operation / pc.write_operation / intake event_id）+ Outbox 同事务回写（document.rs:237、memory.rs:731、publish.rs）——跨域写入可审计性整体成立。
4. 迁移期写入权威建议（供 U13 裁决参考，非决定）：kb/pc 正文迁 Git 前必须先关闭「PG content 可直写」的一切现役通道——当前通道本身已收敛（触发器冻结），主要工作是新建 commit-then-insert 两阶段与读取换源（PC 侧换源清单见 object-registry §4 PC-PUBLISH 行注）。
