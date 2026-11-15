# 04 PC（Product Cognition）领域审计

## 0. 结论速览

PC 已实现**完整提案-审批-发布事务链**，且"正式认知 vs 待确认候选"分离是本仓做得最严格的域：
- AI/分析链只能写 **draft**（lifecycle_status=draft/identity 的 cognition_object/relationship + proposal+pending approval）；
- 只有 publish 事务（前置 approved proposal + approved approval + 真人主体）能把 lifecycle 推到 active/deprecated；
- PG 触发器 + CHECK 全链兜底（审批行不可变、修订不可变、成员锚定后不可改）。
- **当前 0 行生产数据**（字典表 18 object_type / 20 relationship_type 有种子；pc.source 有 1 行来自仓库分析链）。
- 已登记偏差：正文 Git 权威后置（PCGIT）——`pc.document_revision.git_commit_sha` 恒 NULL，PG content 是唯一权威（pc/mod.rs:25-27 显式登记）。

## 1. 逐表明细（表→代码→状态值）

| 表 | 写入代码（crates/pm-application/src/pc/） | 关键约束/状态值 |
|---|---|---|
| cognition_object | object.rs:208-270 `upsert_object_draft`；发布推进 publish.rs:626-653 | lifecycle_status=identity\|draft\|active\|deprecated\|superseded（0001:3056）；UNIQUE 对象身份；superseded 必带 superseded_by_id（CHECK2/4）；trg_pc_object_lifecycle/delete |
| cognition_revision | object.rs:245-262 | revision_no 单调；content_hash=sha256(markdown)；document_id/document_revision_id 钉正文；analysis_mode=derived\|derived_fallback\|llm\|NULL（0001:3078）；trg_pc_revision_immutable |
| pc.document / pc.document_revision | object.rs:222-244,413-426,469-491 | content text NOT NULL=正文权威；git_commit_sha 恒 NULL（PCGIT 偏差）；search_tsv 派生列（0009） |
| cognition_version / version_member | publish.rs:380-500 | status=published\|archived；version_no=current+1；snapshot_hash=成员 canonical hash（mod.rs:113-122）；predecessor_version_id 链；成员=object XOR relationship（CHECK 0001:3428）；trg_pc_vm_immutable/closure |
| proposal / proposal_member | proposal.rs:645(create)/820(submit)/1392-1491(`propose_from_analysis` AI 自动起草) | status=draft\|submitted\|approved\|rejected\|withdrawn\|expired\|applied\|failed；origin_kind=ai_agent\|kb_memory\|manual_input\|requirement_change\|conflict_resolution；change_kind 六值（create/update/deprecate × object/relationship） |
| approval | proposal.rs:1000-1039（裁决） | status=pending\|approved\|rejected；乐观锁三锚：proposal_content_hash（提案行/审批行/成员集重算三方比对）+ expected_version_no CAS + `uq_pc_approval_pending` 部分唯一索引（0001:6833）；trg_pc_approval_immutable |
| write_operation | publish.rs:110-166,288,612 | command=pc_commit_object_revision\|pc_commit_relationship_revision\|pc_publish_version；status=pending\|git_committed\|completed\|failed\|abandoned；operation_id 幂等键；git_committed 状态当前不可达（PCGIT） |
| object_type | 只读字典 mod.rs:213 | layer=L1_identity\|L2_structure\|L3_usage\|L4_semantic\|L5_decision\|snapshot；governance_level=L1\|L2；singleton；doc_template_key。18 条种子（0001:10255-10272）：product_profile/product_vision 单例带模板；business_rule/business_process/product_decision=L2 治理；repository/module/tech_stack/delivery_activity=snapshot 层 |
| relationship_type | mod.rs:244 | directionality=directed\|undirected；source/target_object_types 数组校验；owner_domain=pc\|kb（0007）。12 条 PC 种子 + 0007 加 8 条，其中 3 条 owner_domain='kb'（requirement-implemented-by-design、design-constrained-by-adr、adr-documented-in）——KB 域内边挂 PC 表管理，PC 无确认权（0007:19-21） |
| relationship / relationship_revision | relationship.rs:1-25,360 | 双端：内部对象端（source/target_object_id+revision）或跨域端（ext 四元组 domain/type/id/revision）；CHECK 端点一致；trg_pc_rel_revision_endpoint_match/immutable |
| source | evidence.rs:27-79 `register_source` | source_kind 8 值=kb_document\|kb_memory\|manual_input\|requirement\|requirement_revision\|discovery_finding\|project_cognition\|external_reference；kb_document 通道强制 target_domain='knowledge_base'+target_revision_id（CHECK）；通道开闭校验 DomainChannelState（evidence.rs:33-49） |
| evidence | evidence.rs:121-208 `register_evidence` | 主体=object_revision_id XOR relationship_revision_id（0001:3209）；evidence_type 7 值分型定位器 CHECK（code/test_result/commit→file_path+commit_sha 且 target_domain NULL；document_section→target_domain='knowledge_base'+target_revision_id；meeting/user_statement/system_observation→excerpt/description）；verification_status=unverified\|verified\|refuted\|conflicted |
| conflict / conflict_object | **生产无写入者（桩）** | status=detected\|under_review\|awaiting_human\|resolved\|dismissed\|reopened；conflict_type 8 值；消费方 `frozen_by_conflict`（mod.rs:180-203）——未解决冲突冻结对象/关系，submit/approve/publish 三处 403 |

API 面：apps/pm-api/src/knowledge_services.rs:52-86——/v1/pc/analyses(P1 501)、/v1/pc/decisions(P2 501)、/v1/pc/analyses/{id}(P3)、/v1/pc/proposals/{id}(P4)、/v1/pc/proposals/{id}/decision(P5)、/v1/pc/conflicts/{id}(P6)、/v1/pc/versions/{id}(P7)、/v1/pc/objects、/v1/pc/proposals。**MCP 不暴露确认工具**（knowledge_services.rs:26；proposal.rs:16——执行端无 MCP 确认凭据）。

## 2. 任务书 11 问逐答

1. 正式认知对象表：pc.cognition_object（lifecycle_status=active）+ pc.cognition_revision（当前钉扎 current_revision_id）+ 正文 pc.document_revision.content。
2. 类型定义：pc.object_type 字典（18 种子）+ pc.relationship_type（20 种子，owner_domain 分 pc/kb）。
3. 正文存储：PG content 列（唯一权威，git_commit_sha 恒 NULL，PCGIT 后置偏差已登记 pc/mod.rs:25-27）。
4. Revision 管理：revision_no 单调 + append-only（触发器禁改禁删）+ content_hash 钉正文行；发布快照另行 cognition_version/version_member（version_no+snapshot_hash+predecessor 链）。
5. Candidate 存在：以 **draft 形态**存在（lifecycle=draft 的对象/关系修订 + submitted proposal），无独立 candidate 表。
6. 人工确认机制：proposal→submit（自动产生 pending approval）→人工 decision（P5，真人主体强制）→publish 事务（同事务推 lifecycle、落 version、写 write_operation）。三重乐观锁（内容哈希三方比对+expected_version_no+审批行 FOR UPDATE）。
7. AI 直写正式认知：**不可能**。AI 只能 create/submit 提案（propose_from_analysis，origin_kind=ai_agent）；approve/publish 判 principal.kind='human'（ensure_formal_authorized，mod.rs:152-176）；lifecycle 推进仅 publish.rs:626-653；触发器+CHECK 兜底。
8. 认知关系存储：pc.relationship（现行锚）+ relationship_revision（版本行，含双端内部/外部四元组）+ relationship_type 字典校验端点类型与 owner_domain；跨域端允许 ext 端（domain/type/id/revision）。
9. 来源与证据：pc.source=来源登记（一次采集一条，指向跨域对象）；pc.evidence=证据钉扎（挂到具体 revision，分型定位器）；分析链生产调用 kb_pc_analysis.rs:880-899（source_kind=kb_document 钉 KB 修订）、:936+（每对象一条 evidence，evidence_hash=KB 修订指纹）。
10. 与 KB/Analysis 关联：source_kind=kb_document / kb_memory / discovery_finding / project_cognition 四个 KB/Analysis 入口通道（通道状态机校验开闭）；分析链写 draft + proposal 的唯一生产路径=kb_pc_analysis.derive_pc_cognition（815-1027）。
11. 与 Product Plan/Requirement/Design 边界：Plan 在 plan.task_plan；Requirement 在 delivery.requirement*；Design/ADR 在 design.*——均不入 pc schema；跨域关联走 pc.source/pc.evidence/pc.relationship ext 端（如 requirement-* 关系类型）。0007 明确 KB 域内边（design↔adr）挂 pc.relationship_type.owner_domain='kb'，属**边界越位待裁决**（PC 表承载 KB 关系类型）。

## 3. 特别检查：正式认知与待确认候选分离 → **已实现（MATCH）**

- 分离载体：lifecycle_status=draft/identity vs active；视图 v_revision_versioned 的 in_published_version 布尔（repo_kb_pc_query.rs:139,175 消费，区分正式化 vs 草稿修订）。
- 红线符合：ARC-02（PC 正式认知人工确认）、ARC-03（分析报告不能自动成为正式认知）均 MATCH；ARC-14（历史版本不得静默覆盖）——append-only 触发器 + superseded 链 MATCH。

## 4. 缺口/风险清单

| # | 事项 | 级别 |
|---|---|---|
| PC-1 | 正文 Git 权威未实现（PCGIT 后置），pc.write_operation.git_committed 状态不可达，Markdown+Git 权威原则仅登记偏差未兑现 | P1（架构债，已显式登记） |
| PC-2 | conflict 链路是桩：生产无写入者，事件 pc.conflict.detected producer 标"pc 桩"（outbox.rs:326-331） | P2 |
| PC-3 | owner_domain='kb' 的关系类型挂在 pc.relationship_type（域字典越位），KB 关系数据的确认权归属需裁决 | P2 |
| PC-4 | /v1/pc/analyses、/v1/pc/decisions 为 P1/P2 501 占位 | P2 |
| PC-5 | 生产 0 行：全链路仅字典+1 行 source，端到端未在真实数据上验证 | P2（风险提示） |
