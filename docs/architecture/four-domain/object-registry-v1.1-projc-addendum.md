# 四域对象注册表 V1.1 附录 — projc.* 纳入与 pc.* 遗留标记

- 日期：2026-11-16 · 基准：master 79a82908 + 迁移 0025 · 性质：V1.0 注册表（审计产物，基准 05ce33d8）的**增量附录**，不改动 V1.0 原文
- 依据：《Phase3-裁决稿-P3-11仓库分析认知归属与双域治理-v1.1.md》（R-01~R-10）+ 专家评审意见
- 状态：**开发落实产物，待验收**；验收后随 P3-11 正式冻结一并生效

## 1. Project Cognition 域（owner_domain=PROJECT_COGNITION）注册条目更新

| Object Code | 对象 | 当前实现 | 写入权（正式） | 审批权 | 正文权威 | 版本权威 | 证据模型 | 实现状态（V1.1） |
|---|---|---|---|---|---|---|---|---|
| PROJ-COG-OBJECT | 正式认知对象 | **projc.cognition_object**（0025；迁移期旧载体 ai.project_knowledge 维持只读既有事实） | 草稿面=projc::upsert_object_draft（AI/服务可写，产物恒 draft）；正式推进=ProjC 发布事务（T-2，真人主体+A3 门） | 提案无门、发布有门（同 pc 口径，T-2 落地） | PG content（经 projc.document_revision；Git=目标态偏差，同 pc/kb 已认账项） | current_revision_id 钉扎 + revision_no | projc.evidence | **COMPLETE（Stage 1 数据面）**；正式面 STUB→T-2 |
| PROJ-COG-REVISION | Cognition Revision | **projc.cognition_revision**（0025，行推号 + content_hash） | 随对象 upsert 唯一写点 | — | 经 projc.document_revision（不可变触发器） | revision_no 单层 | 经 evidence.object_revision_id | **COMPLETE（Stage 1）** |
| PROJ-COG-VERSION | Cognition Version | **projc.cognition_version + version_member**（0025） | 无写入者（T-2 发布事务） | ensure_formal_authorized 同款模式（T-2） | snapshot_hash 锚 | version_no+predecessor 链 | version_member 端点闭合触发器随 relationship（T-2） | **STUB（表就绪，链未启）** |
| PROJ-COG-PROPOSAL | Cognition Proposal | **projc.proposal + proposal_member + approval**（0025） | 无写入者（T-2：仓库分析 Finding 起草，origin_kind=repo_analysis） | approve/reject 需真人门（T-2） | proposal_content_hash 锚 | expected_version_no 乐观锁 | members+source_id | **STUB（表就绪，链未启）** |
| PROJ-COG-SOURCE / EVIDENCE | 来源/证据 | **projc.source / projc.evidence**（0025） | projc::register_source / register_evidence（AI/服务可写） | — | — | — | DomainRef target 侧四字段（kb.memory_ref/pc.source 同款契约） | **COMPLETE（Stage 1）** |
| PROJ-COG-RELATION | 关系 | projc.relationship 三表**未建**（T-2） | — | — | — | — | 跨域关系类型沿用 0007 owner_domain 机制，枚举扩展 project_cognition 随 T-2 | 未落（计划 T-2） |
| PROJ-COG-CONFLICT | 冲突 | projc.conflict **未建**（P3-D04） | — | — | — | — | R-06：所有权=主要冲突认知所属领域；跨域经 DomainRef | 未落（计划 P3-D04） |

旧载体处置（迁移期事实，不删不改）：

- `ai.cognition_item` / `ai.project_knowledge`：维持现状（CognitionPage 现役）；其「确认门缺失（P0）」等注册表 V1.0 记录继续有效，P3-D04 详细设计时按 projc.* 目标态收口。

## 2. PC 域遗留标记（本附录新增状态取值：`LEGACY`／`LEGACY-TRANSITIONAL`）

| Object Code | 标记 | 依据 |
|---|---|---|
| PC-CONFLICT（pc.conflict/conflict_object） | **LEGACY / Not for New Use**：留表留迁移记录；不解冻 L3 守卫；不手工插数；未来清理走独立迁移方案；不改名充当 projc 对象、不视图伪装迁移 | R-06 |
| pc.object_type snapshot 四类（repository/tech_stack/delivery_activity/module） | **LEGACY-TRANSITIONAL**：现行仓库分析链仍派生（有界过渡，裁决稿 §5.2）；已写入行=过渡数据，不构成任何领域正式认知权威；**delivery_activity 即日起停止纳入正式认知字典（R-05），T-1 守卫落地时同步停止派生** | R-04/R-05/R-10 |

## 3. DomainRef 契约字段（R-07 V1.1，全域统一口径）

- target 侧四字段：`target_domain / target_type / target_id / target_revision_id`（+`revision_missing_reason`），先例=kb.memory_ref、pc.source、projc.source/projc.evidence（0025）。
- 关系确认权威=`*_relationship_type.owner_domain`（0007 冻结语义不变；projc 关系注册时枚举扩展 `project_cognition`）。
- **禁止把关系所有权与目标对象所有权合并为单一 Domain 字段。**
- 通道注册真源=kb-pc-contracts::registry（TargetDomain::ProjectCognition 注释已更新为目标态 projc.*，通道仍 ReadOnly）。

## 4. 覆盖率变化速览（相对 V1.0 §5）

| 领域 | 变化 |
|---|---|
| Project Cognition | OBJECT/REVISION/SOURCE/EVIDENCE：STUB→**COMPLETE（Stage 1）**；VERSION/PROPOSAL：STUB（表就绪）；RELATION/CONFLICT：未落（T-2/D04） |
| PC | CONFLICT：STUB→**LEGACY**；snapshot 字典四类：→**LEGACY-TRANSITIONAL** |
| KB / Analysis | 无变化 |
