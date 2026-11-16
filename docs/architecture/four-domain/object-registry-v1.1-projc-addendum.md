# 四域对象注册表 V1.1 附录 — projc.* 纳入与 pc.* 遗留标记（V1.3）

- 日期：2026-11-17（V1.3）· 基准：master `9a38935`（阶段一/二 + 三份核查报告）· 性质：V1.0 注册表（审计产物，基准 05ce33d8）的**增量附录**，不改动 V1.0 原文
- 依据：《Phase3-P3-11下一阶段实施方案-T1T2T3-v1.0.md》（V1.1）+ 《Phase3-裁决稿-…-v1.1.md》（R-01~R-10）+ 专家评审意见 + **《P3-11 验收前文档级裁决意见 V1.0》（2026-11-17 专家裁决 D-3）**
- 状态：**已通过工程交付验收**（2026-11-17 用户裁决，台账 §10.6）；**R-01~R-10 正式裁决已完成**（2026-11-17 独立专家意见 V1.0，台账 §10.7：7 项批准、3 项修改后批准）；正式冻结令待用户下达（专家支持有边界条件冻结，建议表述已录台账 §10.7）
- 版本记录：V1.1（Stage 1 数据面，0025）→ V1.2（T-1/T-2/T-3 正式链：0026/0027 + 提案/发布/关系/冲突服务 + worker 重挂 + close 双域分流 + 字典废弃）→ **V1.3（专家裁决 D-3：SOURCE/RELATION/CONFLICT 状态降级 + 实现状态判定口径与晋级规则冻结，见 §5）**

## 1. Project Cognition 域（owner_domain=PROJECT_COGNITION）注册条目更新

| Object Code | 对象 | 当前实现 | 写入权（正式） | 审批权 | 正文权威 | 版本权威 | 证据模型 | 实现状态（V1.3） |
|---|---|---|---|---|---|---|---|---|
| PROJ-COG-OBJECT | 正式认知对象 | **projc.cognition_object**（0025） | 草稿面=projc::upsert_object_draft（AI/服务可写，产物恒 draft）；正式推进=ProjC 发布事务（真人主体+A3 门） | 提案无门、发布有门（pc 同口径） | PG content（经 projc.document_revision；Git=目标态偏差，同 pc/kb 已认账项） | current_revision_id 钉扎 + revision_no | projc.evidence | **COMPLETE**（仓库分析链 W4 起真实写入者：repository/tech_stack/repository_module 三类） |
| PROJ-COG-REVISION | Cognition Revision | **projc.cognition_revision**（0025，行推号 + content_hash） | 随对象 upsert 唯一写点 | — | 经 projc.document_revision（不可变触发器） | revision_no 单层 | 经 evidence.object_revision_id | **COMPLETE** |
| PROJ-COG-VERSION | Cognition Version | **projc.cognition_version + version_member**（0025 表 + 0026 端点闭合触发器） | **projc::publish_version**（报告 close 双域分流 `projc-decision:{id}` / 显式补发布；真人+A3+CAS+advisory+规范哈希+账本幂等） | ensure_formal_authorized（真人+A3） | snapshot_hash 锚 | version_no+predecessor 链 | version_member 端点闭合触发器（0026，0001:314 同款） | **COMPLETE**（T-2 落地） |
| PROJ-COG-PROPOSAL | Cognition Proposal | **projc.proposal + proposal_member + approval**（0025 表 + 服务层 T-2） | **projc::propose_from_analysis**（origin_kind='repo_analysis'，W4 起分析链真生产者）+ create/submit 命令 | approve/reject 需真人门（decide 暂记，报告关闭冻结） | proposal_content_hash 锚 | expected_version_no 乐观锁 | members+source_id | **COMPLETE**（T-2 落地） |
| PROJ-COG-SOURCE / EVIDENCE | 来源/证据 | **projc.source / projc.evidence**（0025） | projc::register_source / register_evidence（**W4 起分析链每次登记**，G-1 双源之二） | — | — | — | DomainRef target 侧四字段（kb.memory_ref/pc.source 同款契约） | **SOURCE：INTEGRATED**（有生产调用链；行级幂等=调用约定「一次分析一条来源」，无自然键去重——V1.3 D-3 降级）；**EVIDENCE：COMPLETE**（uq_projc_evidence_identity 行级幂等） |
| PROJ-COG-RELATION | 关系 | **projc.relationship/relationship_revision/relationship_type**（0026，0001:3293/3327/3369 同构） | projc::create_relationship_draft（端点类型合法/跨域 `domain.type` ext 键/自引用/活跃唯一） | 关系成员随提案走发布链 | 端点语义快照自包含 | relationship_revision 推号 | RR content_hash 规范序列化 | **IMPLEMENTED**（服务层+测试+发布链消费面就绪；**生产起草入口待接**——V1.3 D-3 降级。种子 implements/depends_on directed L1，owner_domain CHECK IN ('project_cognition')） |
| PROJ-COG-CONFLICT | 冲突 | **projc.conflict + conflict_object**（0026，0001:3108/3134 同构；R-06 ProjC 自建） | projc::create_conflict / resolve_conflict（人工，真人+A3） | L3 冻结=未解决冲突圈定 submit/approve/发布 403（三处同口径） | — | — | conflict_object 圈定双方（对象 XOR 关系） | **IMPLEMENTED**（服务层及 L3 冻结消费面已集成 submit/approve/publish；**生产登记/处置入口待接**——V1.3 D-3 降级。pc.conflict 维持 Legacy） |

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
- **字段名称映射（O-09，R-07 修改条件，2026-11-17 独立专家裁决）**：抽象契约层 `revision_context` ↔ 存储层 `target_revision_id`（+`revision_missing_reason` 标记不可达原因）——同一语义的契约名与存储名，不得当作两个不同能力实现；P3-D04 统一契约命名。
- 关系确认权威=`*_relationship_type.owner_domain`（0007 冻结语义不变；**projc.relationship_type 已随 0026 落地，owner_domain CHECK IN ('project_cognition')**——独立字典，非 pc 枚举扩展）。
- **禁止把关系所有权与目标对象所有权合并为单一 Domain 字段。**
- 通道注册真源=kb-pc-contracts::registry（TargetDomain::ProjectCognition 目标态 projc.*，通道仍 ReadOnly——DomainRef 引用与事件 payload 均已使用该域）。

## 4. 覆盖率变化速览（相对 V1.0 §5 / V1.1 附录）

| 领域 | 变化 |
|---|---|
| Project Cognition | OBJECT/REVISION/EVIDENCE/VERSION/PROPOSAL：COMPLETE；**SOURCE → INTEGRATED、RELATION/CONFLICT → IMPLEMENTED（V1.3 专家裁决 D-3 降级）** |
| PC | CONFLICT：LEGACY；snapshot 字典四类：LEGACY-TRANSITIONAL → **DEPRECATED（0027 字典废弃）**；identity 通道（pc.source/product_*）维持 ACTIVE 不变（G-1 双源） |
| Analysis | 报告行双源（pc_source_id + projc_source_id）；close 双域分流（G-2）；详情/列表 projc_* 加法字段 |
| KB / Retrieval | 无变化（ProjC FTS 归 P3-10） |

## 5. 实现状态判定口径与晋级规则（V1.3 冻结；2026-11-17 专家裁决 D-3）

本附录实现状态列自 V1.3 起按下表口径判定；状态晋级必须有证据，不允许仅凭设计文档或服务层代码存在而晋级。

| 状态 | 定义 |
|---|---|
| DESIGNED | 已有设计，但尚未形成完整实现 |
| IMPLEMENTED | 核心实现和对应测试已具备，但尚未证明生产集成完整 |
| INTEGRATED | 已进入实际生产调用链，关键调用契约和失败处理已验证 |
| COMPLETE | 生命周期、权限门禁、幂等、并发、审计、失败恢复等**适用**要求均有明确实现，并经过逐项验证 |

口径要点：

1. COMPLETE 不要求每个对象具备全部类型的功能，而是其**适用的治理要求**必须逐项验证。
2. 某项能力若依赖调用方约定而非系统本身提供可靠保证，必须如实标记，不得默认视为已具备（SOURCE 行级幂等即此情形）。

晋级证据要求：

- IMPLEMENTED → INTEGRATED：需生产调用链、失败路径和实际调用契约证据。
- INTEGRATED → COMPLETE：需适用的生命周期、门禁、幂等、并发、审计及恢复验证证据。

与台账的关联（双台账约定，台账 §9.1）：本列仅承载 Implementation 维度；Architecture Decision 维度（R-01~R-10 正式裁决状态）以台账 §9 勾选记录为准，二者不互相替代。「没有生产调用方」不是实现失败——RELATION/CONFLICT 属于已实现、等待领域模型与产品入口设计（P3-D04）的对象。

## 6. Decision 维度（2026-11-17 独立专家裁决 V1.0，台账 §10.7；双台账关联）

Architecture Decision 与 Implementation/Acceptance（§5）为两个独立维度，不得互相替代。

| 对象 | Decision（架构裁决） | Implementation（§5 口径） | Acceptance |
|---|---|---|---|
| OBJECT / REVISION | 已批准（R-01/R-02/R-03） | COMPLETE | 工程交付验收已通过（2026-11-17，台账 §10.6）；未冻结 |
| VERSION / PROPOSAL | 已批准（R-01/R-02/R-09） | COMPLETE | 同上 |
| SOURCE | 已批准（R-01，含 source 语义约束） | INTEGRATED | 同上 |
| EVIDENCE | 已批准（R-01） | COMPLETE | 同上 |
| RELATION | **修改后批准**（R-07：引用完整性细则 O-02 归 P3-D04；字段映射 O-09） | IMPLEMENTED（生产起草入口待接） | 同上 |
| CONFLICT | **修改后批准**（R-06：生产登记入口 O-06 归 P3-D04） | IMPLEMENTED（L3 消费面已集成） | 同上 |
| 统一检索（R-08） | 已批准（架构原则） | 未实现（P3-10 承接 O-07） | 不适用 |

裁决条件跟踪：R-04④=**Partial**（完整跨 schema resolver 归 P3-10，O-01；七项停止条件部分满足，不得登记为全部满足）。开放项 O-01~O-09 全量清单与责任分配见台账 §10.7，不因裁决通过而从台账消失。
