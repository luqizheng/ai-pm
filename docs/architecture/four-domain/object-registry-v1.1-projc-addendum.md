# 四域对象注册表 V1.1 附录 — projc.* 纳入与 pc.* 遗留标记（V1.2）

- 日期：2026-11-17（V1.2）· 基准：master `ea9aafb`（阶段一）+ 阶段二切换批 · 性质：V1.0 注册表（审计产物，基准 05ce33d8）的**增量附录**，不改动 V1.0 原文
- 依据：《Phase3-P3-11下一阶段实施方案-T1T2T3-v1.0.md》（V1.1）+ 《Phase3-裁决稿-…-v1.1.md》（R-01~R-10）+ 专家评审意见
- 状态：**开发落实产物，待验收**；验收后随 P3-11 正式冻结一并生效
- 版本记录：V1.1（Stage 1 数据面，0025）→ **V1.2（T-1/T-2/T-3 正式链：0026/0027 + 提案/发布/关系/冲突服务 + worker 重挂 + close 双域分流 + 字典废弃）**

## 1. Project Cognition 域（owner_domain=PROJECT_COGNITION）注册条目更新

| Object Code | 对象 | 当前实现 | 写入权（正式） | 审批权 | 正文权威 | 版本权威 | 证据模型 | 实现状态（V1.2） |
|---|---|---|---|---|---|---|---|---|
| PROJ-COG-OBJECT | 正式认知对象 | **projc.cognition_object**（0025） | 草稿面=projc::upsert_object_draft（AI/服务可写，产物恒 draft）；正式推进=ProjC 发布事务（真人主体+A3 门） | 提案无门、发布有门（pc 同口径） | PG content（经 projc.document_revision；Git=目标态偏差，同 pc/kb 已认账项） | current_revision_id 钉扎 + revision_no | projc.evidence | **COMPLETE**（仓库分析链 W4 起真实写入者：repository/tech_stack/repository_module 三类） |
| PROJ-COG-REVISION | Cognition Revision | **projc.cognition_revision**（0025，行推号 + content_hash） | 随对象 upsert 唯一写点 | — | 经 projc.document_revision（不可变触发器） | revision_no 单层 | 经 evidence.object_revision_id | **COMPLETE** |
| PROJ-COG-VERSION | Cognition Version | **projc.cognition_version + version_member**（0025 表 + 0026 端点闭合触发器） | **projc::publish_version**（报告 close 双域分流 `projc-decision:{id}` / 显式补发布；真人+A3+CAS+advisory+规范哈希+账本幂等） | ensure_formal_authorized（真人+A3） | snapshot_hash 锚 | version_no+predecessor 链 | version_member 端点闭合触发器（0026，0001:314 同款） | **COMPLETE**（T-2 落地） |
| PROJ-COG-PROPOSAL | Cognition Proposal | **projc.proposal + proposal_member + approval**（0025 表 + 服务层 T-2） | **projc::propose_from_analysis**（origin_kind='repo_analysis'，W4 起分析链真生产者）+ create/submit 命令 | approve/reject 需真人门（decide 暂记，报告关闭冻结） | proposal_content_hash 锚 | expected_version_no 乐观锁 | members+source_id | **COMPLETE**（T-2 落地） |
| PROJ-COG-SOURCE / EVIDENCE | 来源/证据 | **projc.source / projc.evidence**（0025） | projc::register_source / register_evidence（**W4 起分析链每次登记**，G-1 双源之二） | — | — | — | DomainRef target 侧四字段（kb.memory_ref/pc.source 同款契约） | **COMPLETE** |
| PROJ-COG-RELATION | 关系 | **projc.relationship/relationship_revision/relationship_type**（0026，0001:3293/3327/3369 同构） | projc::create_relationship_draft（端点类型合法/跨域 `domain.type` ext 键/自引用/活跃唯一） | 关系成员随提案走发布链 | 端点语义快照自包含 | relationship_revision 推号 | RR content_hash 规范序列化 | **COMPLETE**（T-2 落地；种子 implements/depends_on directed L1，owner_domain CHECK IN ('project_cognition')） |
| PROJ-COG-CONFLICT | 冲突 | **projc.conflict + conflict_object**（0026，0001:3108/3134 同构；R-06 ProjC 自建） | projc::create_conflict / resolve_conflict（人工，真人+A3） | L3 冻结=未解决冲突圈定 submit/approve/发布 403（三处同口径） | — | — | conflict_object 圈定双方（对象 XOR 关系） | **COMPLETE**（T-2 落地；pc.conflict 维持 Legacy） |

双源登记（G-1，§8.1）：`integration.repo_analysis_report.projc_source_id`（0026 加列）与
`pc_source_id` 并列——写入点唯一 = `mark_analyzed`（同一事务双源登记；**projc 源缺失 =
硬失败**，报告不得进入 analyzed，审计事件 `analysis_report.analyze_gate_failed` 留痕）；
NULL 仅出现在 0026 之前的历史行 = 显式 Legacy（close 跳过 ProjC 分支并写
`projc_branch:"legacy_no_source"` 审计键，绝不静默）。

事件族（kb-pc-contracts 目录 16 项，additive-only）：`projc.change_proposed` /
`projc.cognition.version_published`（producer=projc-service；Platform 留痕消费）。

旧载体处置（迁移期事实，不删不改）：

- `ai.cognition_item` / `ai.project_knowledge`：维持现状（CognitionPage 现役）；其「确认门缺失（P0）」等注册表 V1.0 记录继续有效，P3-D04 详细设计时按 projc.* 目标态收口。

## 2. PC 域遗留标记（本附录新增状态取值：`LEGACY`／`DEPRECATED 字典`）

| Object Code | 标记 | 依据 |
|---|---|---|
| PC-CONFLICT（pc.conflict/conflict_object） | **LEGACY / Not for New Use**：留表留迁移记录；不解冻 L3 守卫；不手工插数；未来清理走独立迁移方案；不改名充当 projc 对象、不视图伪装迁移 | R-06 |
| pc.object_type snapshot 四类（repository/tech_stack/delivery_activity/module） | **DEPRECATED（字典废弃，0027）**：`status='deprecated'` → `load_object_type` 即拒 = 错误域草稿源头不可达（fail-closed）；identity 链 product_* 类型不受影响；**delivery_activity 停止派生（R-05）已随 W4 兑现**；T-1 过渡守卫由「字典废弃 + worker 重挂 + close 分流断言」替代落地（P2 零改动红线期间排期解除） | R-03/R-04/R-05/R-10 |

## 3. DomainRef 契约字段（R-07 V1.1，全域统一口径）

- target 侧四字段：`target_domain / target_type / target_id / target_revision_id`（+`revision_missing_reason`），先例=kb.memory_ref、pc.source、projc.source/projc.evidence（0025）。
- 关系确认权威=`*_relationship_type.owner_domain`（0007 冻结语义不变；**projc.relationship_type 已随 0026 落地，owner_domain CHECK IN ('project_cognition')**——独立字典，非 pc 枚举扩展）。
- **禁止把关系所有权与目标对象所有权合并为单一 Domain 字段。**
- 通道注册真源=kb-pc-contracts::registry（TargetDomain::ProjectCognition 目标态 projc.*，通道仍 ReadOnly——DomainRef 引用与事件 payload 均已使用该域）。

## 4. 覆盖率变化速览（相对 V1.0 §5 / V1.1 附录）

| 领域 | 变化 |
|---|---|
| Project Cognition | OBJECT/REVISION/SOURCE/EVIDENCE：COMPLETE（Stage 1）；**VERSION/PROPOSAL/RELATION/CONFLICT：STUB/未落 → COMPLETE（T-2，本批）** |
| PC | CONFLICT：LEGACY；snapshot 字典四类：LEGACY-TRANSITIONAL → **DEPRECATED（0027 字典废弃）**；identity 通道（pc.source/product_*）维持 ACTIVE 不变（G-1 双源） |
| Analysis | 报告行双源（pc_source_id + projc_source_id）；close 双域分流（G-2）；详情/列表 projc_* 加法字段 |
| KB / Retrieval | 无变化（ProjC FTS 归 P3-10） |
