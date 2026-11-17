# 06 — 数据库与 Migration 只读证据（D 节）

**证据基准**：master `e05693afdc0a9ae534735c9b6f67117a6b92de8d`。migration 文件行号以该 HEAD 工作区为准（migrations/ 目录无未提交改动）。**本包全程只读（SELECT/信息视图查询），未执行任何数据库写操作。**

**方法说明（区分两态）**：
- **Migration 定义态** = 仓库 `migrations/0001~0027` 文件内容。
- **运行库实际态** = 本机 Docker 容器 `aipm-dev-postgres`（127.0.0.1:16433，库 `aipm_mig_full`，AGENTS 规则 3 要求 127.0.0.1）内的只读查询结果。该容器为自建开发库（机毁即失），其数据 ≠ 生产。
- 追溯性限制：`aipm_mig_full`/`aipm_mig_s1` 中**不存在 `_sqlx_migrations` 台账表**（information_schema 查询零命中），无法从库内核验迁移应用台账；schema 全集与表结构经直接查询核验（见 §7）。`aipm` 库仅含 `_sqlx_test` schema（sqlx::test 模板）；`_sqlx_test_%` 泄漏库实测 0 个。→ 迁移应用的权威台账标记 `NOT_VERIFIED_AT_RUNTIME`，schema/行数事实为已核验。

---

## 1. 27 个 Migration 清单（定义态）

| 文件 | 摘要 |
|---|---|
| 0001_baseline.sql | 2026-11-09 冻结点基线合并（原 0001~0065 压缩），12+ schema、128 表+种子 |
| 0002~0005 | 会话/发现链接、上下文锚点、LLM 元数据、交互写回 |
| 0006 | kb.document_revision 确认/发布通道列 |
| 0007 | pc.relationship_type 加 owner_domain（CHECK IN ('pc','kb')）+种子 |
| 0008/0009 | KB 检索索引、pc 修订 FTS |
| 0010~0024 | design/task_plan/embedding/run_event/knowledge_pin/schedule/docproc/kb_docproc/llm_catalog/context_item/viz/ssh×2/repo_analysis_report/run_mode |
| **0025_projc_domain_bootstrap.sql** | P3-11 R-01/R-02：projc 独立 schema（13 表+8 触发器+15 索引+3 字典种子） |
| **0026_projc_formal_chain.sql** | P3-11 阶段一：关系 3 表+冲突 2 表+闭合触发器+0025 缺陷修正+`repo_analysis_report.projc_source_id`（G-1 双源） |
| **0027_pc_snapshot_dictionary_deprecated.sql** | W7 pc 收敛：4 类 snapshot 字典 UPDATE status='deprecated'（软废弃，不删行不动结构） |

## 2. pc 域 schema/表/索引/约束（定义态）——实测 18 表，与 V1.1「18 表」一致

pc.approval(3021)、pc.cognition_object(3041)、pc.cognition_revision(3064)、pc.cognition_version(3087)、pc.conflict(3108)、pc.conflict_object(3134)、pc.document(3149)、pc.document_revision(3163)、pc.evidence(3182)、pc.object_type(3220,字典)、pc.proposal(3240)、pc.proposal_member(3271)、pc.relationship(3293)、pc.relationship_revision(3327)、pc.relationship_type(3369,字典)、pc.source(3391)、pc.version_member(3418)、pc.write_operation(3453)；另有视图 pc.v_revision_versioned(3465)。

关键唯一索引/约束：
- cognition_object：UNIQUE(tenant,project,object_key)；UNIQUE(tenant,project,id)；UNIQUE(tenant,id)；lifecycle CHECK
- cognition_revision：UNIQUE(tenant,project,object_id,revision_no)（CAS 推号）
- cognition_version：UNIQUE(tenant,project,version_no)；status CHECK(published/archived)
- proposal：status 8 态 CHECK(3258)、origin_kind 5 值、expected_version_no≥0
- 单例：`uq_pc_object_singleton` 部分唯一索引(6851-6854)；审批单 pending 行 `uq_pc_approval_pending`(6830)
- evidence：5 个 CHECK（类型/置信/验证态/subject XOR/locator 分型）；source：revision XOR CHECK(3410)

## 3. projc 域 schema/表/索引/约束（定义态）——0025(13)+0026(5)=18 表

0025：object_type(45,3 类种子)、cognition_object(71)、document(89)、document_revision(98)、cognition_revision(112)、cognition_version(132)、version_member(148)、write_operation(163)、proposal(186,origin_kind 含 'repo_analysis')、proposal_member(212)、approval(229)、source(246,source_kind 含 'product_cognition'/'repo_analysis')、evidence(268)。
0026：relationship_type(25,owner_domain CHECK='project_cognition')、relationship(64)、relationship_revision(93)、conflict(132)、conflict_object(153)；修正 0025 缺陷（proposal_member 锚定触发器 BEFORE→AFTER CONSTRAINT 重绑 211-220；write_operation 补 UNIQUE 224-226）；G-1 列（230）。

部分唯一索引：uq_projc_object_singleton(458，singleton 恒 false 仅承载机制)、uq_projc_evidence_identity(459)、uq_projc_approval_pending(460)、uq_projc_pm_target(461)、uq_projc_vm_object/relationship(462-463)、uq_projc_conflict_object(0026:246)、uq_projc_relationship_identity(0026:247)。

**重大结构事实：projc 18 表零 FOREIGN KEY**；pc 有域内 FK（0001:9510-10086 约 40 条三元组组合 FK+字典 FK）但 project_id/tenant_id 对 core.* 均为软引用；**全库无 product_id 列**（grep 零命中）。**pc 唯一 ON DELETE CASCADE**=conflict_object→pc.conflict(9646)；projc relationship 可 DELETE（无禁删触发器，约束不对称实例）。

## 4. core.product / core.project 主体锚定结构

- **core.product：不存在**（定义态：27 文件 CREATE TABLE 无 product 表；"product" 命中仅 pc 字典键/枚举值/注释；运行态：pg_class 核查不存在）。→ P4-01 目标模型**未实施**。
- core.project（0001:1251）：见 05 号文件 C.4。**无 UNIQUE(tenant_id,id) 组合键**（P4-02 C-01 迁移前置未实施）。
- 认知表↔主体：pc/projc 全部经 project_id 软引用（无 FK/ON DELETE）；解除关联/清扫均无 DB 级联动。

## 5. identity、DomainRef、Link、版本、发布、Evidence、授权相关表结构

| 概念 | 表 | 状态 |
|---|---|---|
| identity | pc/projc.cognition_object（(tenant,project,type,key) 定位） | 已实现 |
| DomainRef 跨域引用 | pc.relationship(_revision)/projc 同构的 ext_* 四元组列；pc.source/projc.source 的 target 四元组；kb.memory_ref(0001:2927 XOR CHECK) | 已实现（零负载，见 §7） |
| Link（product↔project） | **无表**（DESIGN_ONLY，P4-02 未实施） | DESIGN_ONLY |
| 版本/发布 | cognition_version+version_member+write_operation+outbox（无独立 outbox 表——outbox 为应用层 `pm-application/src/outbox.rs` 写事件目录，落 ai/outbox 类表） | 已实现 |
| Evidence | pc/projc.source+evidence（evidence_hash 必填） | 已实现（快照机制 DESIGN_ONLY） |
| 授权 | core.approver_grant(1187，项目级审批授权)；**无消费授权表** | 部分实现（消费授权 DESIGN_ONLY） |
| update-pending | **无任何列/状态/表**（migration 全量 grep 零命中） | DESIGN_ONLY |
| PC-P/pcprod | **无**（schema 全集 17 个：ai,core,delivery,design,devenv,discovery,docproc,governance,integration,kb,pc,plan,platform,projc,public,source,viz） | DESIGN_ONLY |

## 6. Migration 执行顺序与 P3-11 涉及迁移的落地情况

顺序=文件名序号（sqlx 按字典序应用）。P3-11 正式链落地证据链（三向）：①迁移 0025/0026/0027 在仓库且在运行库 schema 中有对应结构（projc schema 18 表存在）；②代码 projc 模块 7 文件+close 双域分流（analysis_report.rs:637-698）+事件 2 项+G-1 双源登记（mark_analyzed 同事务）；③测试 projc_formal_chain 6/6、analysis_reports 11/11（P3-11 验收补充报告 D4 记录，见 08 号文件）。运行库 `aipm_mig_full` 实测 projc.cognition_object=2、cognition_version=1、proposal=1、relationship=1——projc 链有种子级真实数据（与审计「projc 种子级」一致）。

## 7. 运行库实际状态（运行态，只读实测 @ 取证时点）

库 `aipm_mig_full`（schemas 与迁移定义态一致）：

| 表 | 行数 | 说明 |
|---|---|---|
| pc.cognition_object | **1** | `repository / legacy-direct / draft`（**snapshot 存量遗留行**——0027 注释「≈0 行」与实测 1 行不符，审计 V1.1 已登记同一发现） |
| pc.cognition_version / proposal / source / evidence / conflict | 0 / 0 / 0 / 0 / 0 | **pc 正式链零负载（迁移红利窗口）** |
| projc.cognition_object / cognition_version / proposal / conflict / relationship | 2 / 1 / 1 / 0 / 1 | P3-11 链种子级数据（repository_module draft×2；关系 1 行） |
| core.project | 1 | 单项目 |
| ai.project_knowledge / ai.cognition_item | 0 / 0 | 旧通道空转（审计 J4 同判） |
| integration.repo_analysis_report | 0 | 无报告 |
| pc.object_type | active=14 / deprecated=4 | 0027 废弃已生效于字典层 |
| core.product | 表不存在 | P4-01 未实施 |
| pg_database `_sqlx_test_%` | 0 | 无泄漏测试库 |

**三则门禁性提醒（承任务书）**：①Migration 文件存在≠数据库已应用（本库台账缺失，见方法说明）；②迁移已应用≠运行时调用链已使用（pc 生产写入者存在但零负载；projc 仅种子）；③表空≠契约无风险（如 approved→withdrawn 转移缺失、projc revision 不可变触发器缺失均为结构性风险，与行数无关）。

## 8. 触发器与守卫盘点

pc 17 触发器（0001:6966-7078）：approval/document_revision(0009 重绑)/object_delete/object_lifecycle/pm_anchored(×2 CONSTRAINT)/proposal_anchors/rel_revision_endpoint_match/rel_revision_immutable/relationship_anchors/relationship_delete/revision_immutable/version_delete/version_immutable/vm_closure(CONSTRAINT DEFERRED)/vm_immutable/write_op_state。
projc 9 触发器（0025:429-443+0026 重绑/新增 vm_closure 与 proposal_member_anchored 修正）：object_delete/object_lifecycle/document_revision_immutable/**cognition_version_immutable**/write_op_state/proposal_anchors/proposal_member_anchored(修正)/approval_immutable/vm_closure；**缺** cognition_revision/relationship 系不可变与端点匹配触发器（0026 头注「明确不含（后续任务包）」——审计 J5 同判）。
服务层守卫（DB 之外的真实行为层）：ensure_formal_authorized（真人+A3，pc/mod.rs:151-176）、frozen_by_conflict L3 冻结（pc/mod.rs:180-203/projc/mod.rs:181-204，三处同口径）、load_object_type fail-closed、CAS/幂等账本、close 双域分流独立计数。

## 9. P3-11 涉及的 Migration 与正式链当前落地小结

0025（Stage 1 先行批）/0026（T 阶段一 ea9aafb）/0027（T 阶段二切换批 e9f66ec，原 0785f97 amend）+服务层镜像+close 分流+G-1 双源+前端只读节——落地证据链完整（详见 08 号文件）。P3-11 正式链的**机械契约**（12 条 close 契约 10 零改 2 加法）经 close 契约影响核查确认（D7：「契约零破坏…请求放行实施」）。

**结论性事实**：①当前 DB 无 PC-P/PC-J 区分、无消费授权、无 update-pending、无 Link、无 core.product——P4-03 全部目标态在数据层零存在（与本切片「不实施」一致）；②pc 链机制完整但零负载，projc 链机制完整且有种子数据；③结构级风险两处（approved→withdrawn 无路径；projc revision 无不可变触发器）已登记至 11 号文件。
