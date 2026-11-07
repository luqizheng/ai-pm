# PC-01B — 总纲与设计裁决（V1.1 拆分件 00）

> **拆分声明**：本文件为《PC-01B — Product Cognition 数据模型与数据库设计 V1.1》按主题拆分的 7 份文件之一（2026-11-07，用户授权的文档组织重组）。**内容与原文件零设计变更**，章节编号保持原文档编号（§1~§24 及附录），全部交叉引用（§x.y）依然有效。拆分索引见下表；原路径 `../pc-01b-data-model-and-database-design-v1.0.md` 已改写为拆分索引占位页。

| 项 | 值 |
|---|---|
| 文档版本 | **V1.2**（V1.1 + 2026-11-07 PC-01B 差异复核 C-1~C-4 采纳落版：PC-D-02 修订为 PC 独立正文承载（pc.document\* + PCGIT）、doc_type ALTER 取消、pc.source/pc.evidence 撤 ai FK 并撤 document\* 定位列改四元组（用户裁决：无历史数据不留兼容列）、发布事务补 outbox 回写、15 表→17 表。V1.1 = V1.0 独立评审「不通过」后的差异修订版 R1~R12，对照见附录 A） |
| 状态 | PC-01B 设计基线 V1.2，服务化差异复核（C-1~C-4）已闭合（复核报告 2026-11-07：有条件通过、条件全部满足）；待产品负责人终审 + R1~R5 差异复核（评审层次：领域模型 → 双维度版本与端点闭合 → 审批与 Git-PG → DDL 与约束 → 验收测试） |
| 日期 | 2026-11-07（V1.1 修订同日） |
| 阶段 | PC-01B 数据模型与数据库设计（设计交付，不含 Migration 实施） |
| 上游基线 | ①《PC 阶段 1 — 目标领域模型设计基线 V1.0》（其迁移章节已废止）；②《PC 阶段 1 — 当前实现评审与开发计划 V1.1》（PC-01A）；③ Q-PC-01～Q-PC-13 产品裁决；④《PC Gate 0 — 数据库 Schema 核查资料清单》 |
| 代码权限 | 只读。本阶段未执行任何 Migration、未修改任何数据库/代码/配置（§24 附核验方式） |
| 下游 | PC-01C 核心领域服务（本文档为其实施输入） |
| V1.1 修订依据 | ①《PC-01B 数据模型与数据库设计 V1.0 独立架构评审报告 V1.0》（结论：不通过）；② 五项裁决授权（2026-11-07，用户授权评审组定案）：0044 双文件修复授权（执行归 01C）、F-05 端点成员=方案 A（自动并入）、F-06 冻结=approve+publish 双落点、立即启动 V1.1、relationship_type 种子由 01B 补齐 |
| 台账 | 既有 M0～M7 / PC-0001～0707 为唯一正式执行台账；本阶段对应台账 PC-0006（语义已替换为「PC 新表 Migration 起号规划与设计」，见 §22.4） |
| 文档组织 | 已拆分为 7 份（本文件 = 拆分件 00）；拆分不改任何设计内容，不构成对 PC-01B 设计的修订 |

**拆分文件索引**：

| 拆分件 | 文件 | 原文章节 |
|---|---|---|
| 00 | pc-01b-00-overview-and-decisions-v1.1.md | 头部元信息 + §1 + §2 + §11 + §23 + §24 + 禁止事项声明 + 附录 A |
| 01 | pc-01b-01-domain-model-and-lifecycle-v1.1.md | §3 + §4 |
| 02 | pc-01b-02-core-tables-object-relationship-v1.1.md | §5 + §6 |
| 03 | pc-01b-03-version-member-and-closure-v1.1.md | §7 + §8 |
| 04 | pc-01b-04-governance-traceability-v1.1.md | §9 + §10 |
| 05 | pc-01b-05-storage-cross-domain-v1.1.md | §12 + §13 |
| 06 | pc-01b-06-constraints-and-implementation-v1.1.md | §14 ~ §22 |

**标注体系**：〔裁决〕= Q-PC-01～13 已确认产品裁决或 PC-01A 已批准领域裁决；〔事实〕= 代码/Schema/工具链证据（附 文件:行号）；〔推导〕= 本阶段技术设计建议（含 PC-D 设计裁决的技术结论，非产品裁决）；〔待验证〕= 尚未从运行环境证实，不得当作已实现。

---

# 1. 执行摘要

| # | 结论 | 性质 |
|---|---|---|
| 1 | PC 采用独立 `pc` schema，共 **17 张新表**（§15.2 清单；V1.2 增 pc.document/pc.document_revision 正文承载两表，C-1）：对象与关系双维度的身份/修订/版本六表 + 类型注册两表 + 文档承载两表 + 来源/证据两表 + 提案/审批/冲突三表 + 提案成员表 + Git-PG 幂等账本表。零历史数据迁移，旧域原样保留（〔裁决〕01A §6.7） | 〔推导〕+〔裁决〕C-1（2026-11-07） |
| 2 | **PC-D-01 = 方案 B（PC 专属提案/审批结构）**：`pc.proposal` + `pc.approval`（A2 不可变范式）+ 复用 A3 权限闸与 governance.audit_log。不改 `ai.change_proposal` / `governance.approval_request`——其 `analysis_run_id NOT NULL`（0003:134）与 PC 提案来源（KB/人工/冲突处置）根本不匹配；Q-PC-07 的「复用机制」落实为复用范式与权限闸，而非物理表。01A §6.5「扩展前必须先加固 m-3」前提随本裁决解除，m-3 加固归 S01 既有链路自身演进 | 〔推导〕 |
| 3 | **PC-D-02 = 方案 A（一个 Object 一个 Markdown 文档）+ PC 独立正文承载（V1.2/C-1 修订）**：Object Revision 领域版本 1:1 映射文档 Revision；正文承载 `pc.document` + `pc.document_revision`（域内闭环，PCGIT 独立 Git 根），文档身份模型同构 A9（doc_key/canonical_path/git_commit_sha/content_hash），Git 权威、哈希链全部保留；不再复用 ai 容器、无 doc_type 扩值 | 〔裁决〕ADR-06a + 差异复核 C-1（2026-11-07） |
| 4 | **PC-D-03 = 完整快照**：Version Member 全量固定双维度成员；否决增量快照与基线合并（端点闭合与完整还原四问在增量方案下无法用库层约束闭合表达） | 〔推导〕 |
| 5 | **PC-D-04 = 数据库字典表 + 代码注册表共同约束**：`pc.relationship_type` / `pc.object_type` 双注册表承载端点类型合法性、方向性、治理等级、单例性；服务层编译期种子注册 + 运行时加载校验；非法关系被「类型 FK + 提案成员完整性 + 发布事务校验」三道闸阻断 | 〔推导〕 |
| 6 | **PC-D-05 = Source / Evidence 实体化分离 + 结构化定位列**：禁止单 JSONB 混用语义（〔裁决〕01A §6.4）；Evidence 采用 A7 判别器 CHECK 模式（0031:128-171），证据钉 Revision 级 | 〔推导〕 |
| 7 | **PC-D-06 = Profile / Vision 均为受控单例 Object 类型**：字段清单与模板章节固化（§10.4），正文权威在 Git，PG 只存 `display_name` 等受控检索副本；Vision 治理等级 L2（类型注册表 `governance_level` 列承载） | 〔推导〕 |
| 8 | **PC-D-09 = PC 专属写幂等账本 + 三态协议**：`pc.write_operation`（pending → git_committed → completed / failed / abandoned），Operation ID 幂等 + commit trailer 反查对账 + 后台 reconcile；H-4 实际事务顺序验证归 PC-01C（§12.7） | 〔推导〕 |
| 9 | **端点闭合三层防线**：① Relationship Revision 端点复合 FK（创建时）；② Version Member 双维度复合 FK（固定时）；③ 发布事务 SQL 断言 + **约束触发器（DEFERRED）** 在提交时校验集合级闭合 + 服务层二次校验。Revision 行不可变触发器保证历史还原后端点永不漂移（§8） | 〔推导〕 |
| 10 | **H-13 已完成源码级验证**（结论升级）：sqlx 0.8.6 对重复版本号**无加载期检测**（source.rs:57-144）；`_sqlx_migrations.version` 为 PRIMARY KEY（sqlx-postgres migrate.rs:120）；同号双文件在全新库必然以记账主键冲突失败、在已有 44 记账的库必然以 `VersionMismatch(44)` 失败——`apps/pm-api/src/main.rs:28` 启动即跑迁移，该故障是**必然**而非概率。0045 起号结论成立；0044 修复方向取决于运行库 checksum 归属〔待验证〕（§21.1） | 〔事实〕+〔待验证〕 |
| 11 | H-11 完成：现有需求确认链在事务内 `FOR UPDATE` 锁 requirement 与 revision 两行（requirement.rs:251,270），但 `ensure_governance_authorized` 为无锁只读判定（approver.rs:56-79），`ai.change_proposal`/`governance.approval_request` 全链无行锁。PC 审批/发布路径必须补齐锁序（§21.2） | 〔事实〕+〔推导〕 |
| 12 | 租户隔离：全部新表执行「`UNIQUE(tenant_id, id)` + `UNIQUE(tenant_id, project_id, id)` + `(tenant_id, project_id) → core.project` 组合 FK」双键模式；内部成员/端点引用全部复合 FK（MATCH SIMPLE 空集跳过语义），隔离不依赖应用层过滤（§14） | 〔推导〕 |

---

# 2. 设计依据与边界

## 2.1 必须遵守的裁决

| 来源 | 约束 |
|---|---|
| Q-PC-01 | PC 与 S01-04 按「产品认知 / 技术认知」划分唯一权威；PC 不读、不写、不迁移旧域数据 |
| Q-PC-02 | 统一决策记录模型：Product Decision 为 PC 对象，ADR 权威归 Design 域（尚无实体，不冒充） |
| Q-PC-03 | Requirement 关联同时记录身份与 Revision（必填 revision 级引用） |
| Q-PC-04 | Product Vision（PC）与 Project Goal（项目管理域）分层，Vision→Goal 支撑关系经四元组 |
| Q-PC-05 | Feature 按关联与治理需要决定是否独立建模（独立化时保留原信息与关联，01A §6.9#11） |
| Q-PC-06 | L0～L3 变更权限等级；P0～P3 处理优先级；PC-G17 风险等级——三者不得混淆 |
| Q-PC-07 | 复用平台提案与审批**机制**，必须满足基线 §9.3 八条技术原则 |
| Q-PC-08 | V1 审批人：Project Lead 与授权审批人同权（`core.approver_grant`） |
| Q-PC-09 | 业务规则仅由 PC 正式确认；Requirements 统一服务调用 PC 检查 |
| Q-PC-10/11 | PC-I0～I6 切片；Requirements 确认门契约 |
| Q-PC-12 | KB 审核通过 → PC 分析 → Change Proposal → 人工确认 → 正式版本；审核 ≠ 确认 |
| Q-PC-13 | Product Plan 不属于 PC：不纳入 Object/Revision/Version 模型；表清单不含 Plan 实体 |
| 01A §6.7 | **零迁移硬边界**：不设计历史数据迁移/兼容/回填/双写；Git-PG 一致性协议是 PC 自身架构要求，不在禁令内 |
| 01A §6.8 | 旧 Baseline 四域边界；PC 不复用 `project_baseline`、不受旧表结构反向约束 |
| 01A §6.2 | 双维度版本模型：Version Member 必须同时固定 Object Revision 与 Relationship Revision；完整还原四问 |

## 2.2 事实基线（Gate 0 / 01A 证据，本设计直接引用）

| 资产 | 证据 | 本设计用法 |
|---|---|---|
| A1 文档域七表闭环（Git 权威 + PG 状态） | 0006:1-5（正文不落库声明）、0006:22-42、0014、0024 | PC-D-02 正文承载、PC-D-09 协议蓝本 |
| A2 `governance.review_approval` 不可变范式 | 0026:223-251（表）、0026:250-251（pending 部分唯一）、0026:273-296（不可变+单向触发器） | PC-D-01 审批表库层保护范式 |
| A3 `core.approver_grant` + `ensure_governance_authorized` | 0040:23-38；approver.rs:37-79 | PC 审批人判定（不重复建权限体系） |
| A4 0019 组合 FK 租户化 | 0019:149-168（双唯一键）、0019:235-258（组合 FK） | PC 全表隔离模板（§14.2） |
| A5 `delivery.task_requirement` 复合 FK 钉 Revision | 0002:68-80 | Requirement 引用结构先例 |
| A6 需求修订 CAS 双令牌 | 0039:61-62（base_current_revision_id/base_row_revision） | PC 并发控制模板（§17） |
| A7 `discovery_finding_evidence` 判别器 CHECK | 0031:128-171 | Evidence 定位协议（PC-D-05） |
| A8 `ai.knowledge_write_operation` 写幂等账本 | 0024:41-64（UNIQUE(project_id, operation_id)） | `pc.write_operation` 蓝本（PC-D-09） |
| A9 `ai.project_document` doc_key/canonical_path 身份模型 | 0006:7-20 | PC 文档身份组织（PC-D-02/06）；**承载自 V1.2 起为 pc.document\*（C-1，身份模型同构 A9）** |

## 2.3 边界声明

1. 本文档是**数据库设计基线**：DDL 级伪码、约束、事务、索引、测试方案。不实施（不建 Migration、不改代码、不启用正式认知写入）。
2. 不推翻、不重议 Q-PC-01～13；PC-D-01～06、09 在本文档内以技术结论关闭（性质为〔推导〕设计裁决，产品负责人终审后冻结）。
3. Design/ADR、Product Reality 引用通道 = 后续跨域集成；本文档不为其设计虚假实体（01A §6.6）。
4. Product Plan 无表（Q-PC-13）；Roadmap 边界未定义，不裁决（01A §6.10）。
5. PC-01B 阶段分组不新建任务编号；台账唯一（01A §7 台账唯一性声明）。

---

# 11. PC-D 设计裁决记录（PC-D-01～06、09 全部关闭）

> 性质：〔推导〕设计裁决。产品负责人终审本文件后冻结；不推翻任何 Q-PC 裁决。

| 编号 | 裁决内容 | 结论 | 详细位置 |
|---|---|---|---|
| PC-D-01 | 审批载体：扩展 `ai.change_proposal`+`governance.approval_request` vs PC 专属结构 | **PC 专属（pc.proposal + pc.approval），复用 A2 范式 / A3 权限闸 / governance.audit_log**；不修改既有两表 | §11.1 |
| PC-D-02 | Revision 与 Markdown 文档组织 | **一 Object 一文档 + PC 独立正文承载（V1.2/C-1）**：`pc.document`+`pc.document_revision`（域内闭环，PCGIT 独立根）；doc_type 扩值随 C-3 取消 | §11.2 |
| PC-D-03 | Version 快照方式 | **完整快照**（否决增量、增量+基线合并） | §7.1 |
| PC-D-04 | Relationship 类型注册机制 | **数据库字典表 + 代码注册表共同约束**（object_type / relationship_type 双注册表） | §5.1/§6.2 |
| PC-D-05 | Source / Evidence 物理结构与证据定位协议 | **实体化分离 + 结构化定位列 + 判别器 CHECK**（A7 模式） | §9 |
| PC-D-06 | Product Profile / Product Vision 字段设计 | **受控单例 Object 类型 + 固定 Markdown 模板章节**；字段清单见 §11.3 | §11.3 |
| PC-D-09 | Git-PG 一致性与恢复协议 | **pc.write_operation 三态账本 + Operation ID 幂等 + commit trailer 反查 + 后台 reconcile** | §12 |

## 11.1 PC-D-01 详录：为何否决「扩展既有审批链」

| 判据 | 事实 | 影响 |
|---|---|---|
| 提案来源不匹配 | `ai.change_proposal.analysis_run_id UUID NOT NULL` + `UNIQUE(analysis_run_id, proposal_seq)`（0003:134,151）——提案强绑定 analysis_run | PC 提案来自 KB/人工/冲突处置，无 analysis_run；复用需伪造 run 或 ALTER 放宽约束，后者波及既有 Task 15 提案链消费方 |
| m-3 弱结构 | approval_request：status 无 CHECK、无不可变触发器、无历史表（0003:157-168） | 扩展 = 先加固既有表（01A §6.5 前提），加固动作与在线 Task 15 链路耦合，回归面大 |
| target 无完整性 | target_type 自由 TEXT、target_id 无 FK（0003:133-134） | PC 需要双维度目标完整性（§10.1 proposal_member 四复合 FK），旧表无法承载 |
| 锚定缺失 | 无 content_hash / expected_version 锚定列（0003:128-154 全表） | 八原则 1/6 无法满足 |
| 复用范式可行 | review_approval 已验证「锚定不可变 + 单向状态 + pending 唯一」三要素（0026:223-296） | PC 专属表按同范式新建，零既有耦合 |
| 权限不重复建设 | ensure_governance_authorized + approver_grant 为项目级权限闸（approver.rs:56-79；0040:23-38），与提案载体无关 | 直接复用（§10.2 注记） |

**风险与对策**：平台将存在两套提案表（ai.change_proposal 服务 Task AI 提案、pc.proposal 服务 PC）。对策：领域边界明确（不同聚合不同载体）；`source_kind='pc_*'` 审计词表统一入口可观测（§19）。

## 11.2 PC-D-02 详录：正文承载与 doc_type 扩值（V1.2/C-1 修订）

1. 正文承载 `pc.document`（doc_key/canonical_path 身份模型，同构 A9）与 `pc.document_revision`（git_commit_sha + content_hash）；域内闭环，PCGIT 独立 Git 根（ADR-06a/C-1 裁决，不再复用 ai 容器）。`pc.document` 不设 doc_type 列（域内无区分语义，N-8）与 created_by 列（首个 pc.document_revision.created_by 已记，N-8）。
2. `doc_type` CHECK 扩值**随 C-3 取消**（V1.2）：ai.project_document 不再承载 PC 正文，原 §22.2-4 的 ALTER 项从 Migration 清单删除。
3. PC 文档路径规范：`canonical_path = pc/{object_type}/{object_key}.md`；`doc_key = pc:{object_type}:{object_key}`（正则 `^[A-Za-z0-9._-]+$` 兼容，冒号不合规 → 使用 `pc-object-{object_key}` 形态，object_key 为 slug）。Object → 文档 1:1（§5.3 注记 4）；Relationship 正文可选（无则 NULL）。
4. **领域版本 ≠ 文档数量**：Object Revision 是领域版本；一个 Revision 对应恰好一个 document_revision（git commit）。文件不存在拆分/合并问题；**pc.document_revision 不设文档级状态列**（N-4）——ai.document_revision 的 draft/published 状态机（0006:29-31）不再适用：Revision 落库即正式，正式性由 cognition_version/version_member 存在性派生（与 §5.3 注记 1「不冗余 revision_status」同理）；不使用 document_proposal 管线（PC 提案即治理载体，双重提案反增复杂度）。
5. 版本还原：读 Member → relationship/object Revision → pc.document_revision（git_commit_sha + canonical_path）→ **PCGIT** checkout 还原正文。多文档一致性不存在（1:1）。

## 11.3 PC-D-06 详录：Product Profile / Product Vision 字段设计

两者均为 **PC Object（object_type 单例）+ 固定 Markdown 模板章节**：正文权威在 Git（模板章节即字段载体），PG 存受控检索副本与治理元数据。不合并为无边界 JSONB（任务书 §10）。

**Product Profile（product_profile，L1，singleton）**：

| 模板章节（Git 正文） | 必填 | 校验（服务层） | PG 投影列 |
|---|---|---|---|
| Product Name | 是 | 非空、长度 ≤ 200 | pc.cognition_object.display_name |
| Product Summary | 是 | 非空、长度 ≤ 2000 | —（检索走 PageIndex） |
| Product Category | 是 | 受控类别表（服务层常量，V1 内置清单） | — |
| Product Status | 否 | 缺省 `unconfirmed` | — |
| Primary Language | 是 | IETF 语言标签格式 | — |

**Product Vision（product_vision，L2，singleton）**：

| 模板章节 | 必填 | 校验 | 治理 |
|---|---|---|---|
| Vision Statement | 是 | 非空 | L2 |
| Target Users | 是 | 至少一条 | L2 |
| Value Proposition | 是 | 非空 | L2 |
| Product Scope | 是 | 至少一条 | L2 |
| Out of Scope | 否 | —（空 = 未划边界，与「已确认为空」语义不同，见下） | L2 |
| Strategic Direction | 否 | — | L2 |
| Success Intent | 否 | — | L2 |

**初始化规则**：项目初始化时自动创建两个单例 Object（lifecycle='identity'）+ 对应 Git 文档（模板骨架）+ `pc.source(source_kind='manual_input')` 登记初始化来源；identity 态无 Revision、无版本成员——「未确认/空值」与「确认值为空」的区分：**前者 = 对象 identity/draft 态（无已确认 Revision）；后者 = active 对象的模板章节内容显式为空**（模板内以空节标记），二者在对象生命周期上可机器区分。

**Revision 规则 / 审批等级**：Profile 描述修改 L1；Vision 全部变更 L2（§3.3 类型注册表 `governance_level` 承载）。

**关联方式**：与 Cognition Object 的关联 = 同一模型（它们就是 Object）；与 Project Goal 的支撑关系走 Relationship（类型 `supports`，target 为跨域端点四元组 `project_goal/project_goal/doc_key='project-goal'`，§13.1）。

---

# 23. 风险与待解决问题

| # | 风险/问题 | 影响 | 处置归属 |
|---|---|---|---|
| R-1 | 0044 双文件在任何带 44 记账库上的必然启动失败（§21.1） | 部署/重启阻塞 | 01C 前置修复（需批准） |
| R-2 | 外域表无项目级组合键 → 跨域引用项目级隔离靠服务层 | 校验漏写即隔离漏洞 | 01C/01D 实现组合键校验 + 对账任务；外域补键后 FK 升级 |
| R-3 | ensure_governance_authorized 无锁竞态窗口 | 授权撤销与审批提交并发时审批仍可能落定 | V1 接受（终态事实语义）；后续可加授权行 FOR SHARE |
| R-4 | 闭合触发器性能（O(kN)） | 大版本发布耗时 | §16.2 降级方案（语句级触发器）预留 |
| R-5 | H-1 Knowledge Git root 布局未定 | Git-PG 协议落地的环境前提 | 01C 实施环境项 |
| R-6 | relationship_type 种子矩阵需逐条人工确认（端点合法性） | 类型种子错误影响后续全部关系 | 01C 实施时按基线 §7.2 逐条核对，产品负责人抽查 |
| R-7 | object_key 命名冲突（slug 唯一性）与改名语义 | 对象 key 不可变（锚定）| 服务层创建时校验；改名 = 新对象 + supersede 链 |
| R-8 | 冲突冻结只校验「提案目标」，不校验「提案影响传播」（如废弃 A 而 B 依赖 A） | 间接影响未被 L3 拦截 | V1 范围：直接目标校验；影响传播分析归 01F 影响分析能力（01A §7.6） |
| R-13 | pc.source/pc.evidence 撤 ai FK（C-4/V1.2）后的存续期漂移：KB 侧文档删除/改写不再被 FK RESTRICT 阻止（写入时仍有服务层组合键即时校验；真时窗=对账周期内的存续期漂移，评审 N-6 精化） | 证据引用失效未及时发现 | 对账任务周期核对（**数据源=K3/PK1 只读 API，禁止直查 KB schema**，N-5）+ evidence_hash 内容锚 + revision_conflict 冲突登记通道；KB 文档删除非高频路径 |

---

# 24. 验收对照表（任务书 §二十一 20 条）

| # | 完成标准 | 落实位置 | 状态 |
|---|---|---|---|
| 1 | PC-D-01～06、09 全部关闭并记录 | §11（六项裁决表 + 详录 §11.1~11.3/§7.1/§9/§12） | ✅ |
| 2 | Object / Revision 模型完整 | §5（DDL + 生命周期触发器 + CAS） | ✅ |
| 3 | Relationship / Relationship Revision 模型完整 | §6（端点身份规则五问 + DDL + 锚定触发器） | ✅ |
| 4 | Version Member 双维度模型具备数据库级约束设计 | §7.3（互斥 CHECK + 四复合 FK + 双部分唯一 + immutable） | ✅ |
| 5 | Relationship 端点与 Version Member 完整闭合 | §8（三层防线 + DEFERRED 触发器 + 断言 SQL） | ✅ |
| 6 | Cognition Version 可完整还原正式对象和关系集合 | §7.1 完整快照 + §8.4 还原矩阵 + T-25 | ✅ |
| 7 | Source / Evidence / KB Memory 追溯链完整 | §9（实体分离 + Revision 级证据 + Q-PC-12 通道 + Q13 反向索引） | ✅ |
| 8 | 跨域引用四元组逐域实现方案 | §13.1 逐域表（含目标版本/计划阶段/当前状态三列区分） | ✅ |
| 9 | 租户和项目隔离落实到复合约束 | §14.2 全表矩阵 + §14.3 攻击面 | ✅ |
| 10 | 审批、发布、审计与并发事务设计完整 | §10.2 八原则逐条 / §18 十四步 + 分工表 / §19 词表 / §17 六场景 | ✅ |
| 11 | Git-PG 一致性和故障恢复协议完整 | §12（三态账本 + 时序 + 故障矩阵 + reconcile 伪代码） | ✅ |
| 12 | 所有核心表均有 DDL 级伪码 | §5/§6/§7/§9/§10/§12 内嵌 DDL + §15 汇总（15 表齐全） | ✅ |
| 13 | 完整性约束矩阵齐全 | §14.2 + §8.4 + §15.5 种子 | ✅ |
| 14 | 核心索引齐全 | §16.1（14 项查询映射） | ✅ |
| 15 | 负向测试和验收方案齐全 | §20（23 项 + 3 项附加断言，逐项含前置/操作/预期/约束/错误码） | ✅ |
| 16 | H-13 在最终确定 Migration 起号方案前得到验证 | §21.1 源码级验证完成；0045 结论成立；运行库 checksum 归属〔待验证〕（不影响起号结论，仅影响 0044 修复方向） | ✅（残余明确标注） |
| 17 | H-11 在复用既有审批链时完成核实 | §21.2（已核实；PC 未复用弱链，缺陷责任转移说明） | ✅ |
| 18 | 全文区分已裁决、事实、技术推导、待验证和当前实现状态 | 全文标注体系 + §13.1 三列状态区分 | ✅ |
| 19 | 未发生任何数据库、Migration 或业务代码修改 | 本次会话只读操作 + 文档/台账两类 Markdown 交付；git status 可核验 | ✅ |
| 20 | 交付物可直接作为 PC-01C 数据库实施的设计输入 | §22 实施建议 + DDL 伪码 + 测试方案 | ✅ |

---

# 附：本阶段禁止事项遵守声明

| 禁止项 | 遵守情况 |
|---|---|
| 执行 Migration / 修改数据库 / 新建修改表 | 未发生（全部 DDL 为文档伪码；未连接任何数据库） |
| 修改 Rust / TypeScript / SQL 业务代码 / API / 前端 | 未发生 |
| 实现 PC Domain Service / Git-PG 写入服务 / 启用正式认知写入 | 未发生（§22 仅为 01C 建议） |
| 设计历史业务数据迁移 | 未发生（零迁移硬边界贯彻；Git-PG 协议已声明为自身架构要求，§12.1-2） |
| 推翻 Q-PC-01～13 | 未发生（§2.1 全表保留；Product Plan 无表；Roadmap 未裁决） |
| 把 Design/ADR 未建设描述成已接通 | 未发生（§13.1 通道状态表标注「后续跨域集成」） |
| 把设计方案描述成已实现 | 未发生（全文〔推导〕/〔事实〕/〔待验证〕标注；H-4 验证移交 01C） |

**交付声明**：本文档为《PC-01B — Product Cognition 数据模型与数据库设计 **V1.2**》（V1.1 差异复核 C-1~C-4 采纳落版），随附台账修订（PC-0006 状态）。评审建议按「领域模型（§3~§6）→ 双维度版本与端点闭合（§7~§8）→ 审批与 Git-PG（§10/§12）→ DDL 与约束（§14~§15）→ 验收测试（§20）」五层顺序进行。

---

# 附录 A：V1.0 → V1.1 修订对照（独立评审 R1~R12 落实表）

| # | 对应评审缺陷 | 修订内容 | 落点 |
|---|---|---|---|
| R1 | F-01（P1） | conflict_object 内联表达式 PK（PostgreSQL 语法错误）→ `id UUID PK` + 独立表达式唯一索引 `uq_pc_conflict_object`；§14.2 矩阵行同步 | §10.3 / §14.2 |
| R2 | F-02（P1） | 发布事务「snapshot_hash 占位回填」与 `forbid_version_mutation` 锚定触发器矛盾 → **先算后插**（步骤⑦ 计算 → 步骤⑧ 一次性 INSERT 含最终 hash），删除占位/回填；保留 hash 一致性复核（步骤⑩） | §18.1 ⑦⑧⑩ |
| R3 | F-03（P1） | 新增 `trg_pc_rel_revision_endpoint_match`（BEFORE INSERT 比对父 Relationship 行端点身份与类型键）——RR 端点身份一致性成为库层约束；配套 T-27 | §6.5 / §20 |
| R4 | F-04（P1） | 新增 `trg_pc_proposal_anchors`（submitted 后内容锚定列 once-written）；发布步骤③升级为「服务层重算成员集 hash vs approval 锚定列」主校验（不信任 proposal 可变列）；配套 T-28 | §10.1 / §18.1 ③ |
| R5 | F-05（P1） | 成员时序契约成文（draft 阶段 Revision 落库齐备 → submit 前置校验 → 步骤②b 发布前完整性断言）；端点成员**自动并入**成员集并纳入 proposal_content_hash（方案 A，2026-11-07 授权）；配套 T-29 | §10.1 / §18.1 ②b |
| R6 | F-06（P2） | L3 冻结校验双落点（approve 服务 + 发布事务）；「发布中新增 Conflict 不回溯」语义成文 | §18.1 ⑥ |
| R7 | F-07（P2） | 治理等级一致性契约：提案 governance_level ≥ 目标类型固定等级（business_rule/product_vision/business_process = L2 不可下调），由类型注册表 + 服务层承载 | §3.3 注记（§3.3 governance_level 定义即承载） |
| R8 | F-08（P2） | relationship_type 种子矩阵补齐：17 条注册（基线 §7.2 落地，跨域端点 `domain.type` 形态）+ 2 条由 Source/Evidence 追溯结构承载的设计裁决 | §15.5 / §6.2 |
| R9 | F-09（P2） | Migration 清单补 `ai.project_document` doc_type CHECK 扩值 `'pc_object'` 的 ALTER（含具名约束重定义）。**〔V1.2/C-3〕该项随服务化取消**——ai 表不再承载 PC 正文，ALTER 项从 Migration 清单删除（§22.2-4 替换为 pc.document\* 建表） | §22.2-4 |
| R10 | F-10（P2） | 身份行 DELETE 防护：object/relationship 仅 identity 态可删（`forbid_identity_delete`），version 全禁 | §15.3 |
| R11 | F-11（P2） | Git-PG 恢复细节：failed 标记独立事务；retry_count 上限（MAX_AUTO_RETRY=5）→ 人工介入；四态收敛保证 | §12.4 / §12.5 |
| R12 | F-12（P2） | `pc.object_type_singleton` 硬编码 IMMUTABLE 函数定稿（§5.2 DDL 与 §15.4 统一） | §5.2 / §15.4 |

P3 项（F-13~F-20）处置：F-13 建表顺序已并入 §22.2-5；F-16/F-18 随本轮表述清理在后续终版统一；F-14/F-15/F-17/F-19/F-20 保持 §23 风险清单跟踪（均不阻塞实施）。

**V1.1 核验**：修订仅涉及本设计文档（Markdown），未修改任何代码/数据库/Migration；五项裁决授权已记录于文档头部修订依据；R1~R5 全部以 DDL/伪代码/测试三形态落地，待评审组差异复核。
**V1.2 核验**：2026-11-07 服务化差异复核 C-1~C-4 采纳落版（复核报告：有条件通过、条件全部满足——C-1/C-2 补项目级身份键后闭合、C-3 排查无遗漏、C-4 用户裁决**撤 FK+撤 document\* 定位列改四元组**，无历史数据不留兼容列）；修改仅涉及本设计文档（Markdown），未修改任何代码/数据库/Migration；升版清单 25+ 项已按拆分件 00/02/04/05/06 落实（拆分件 01/03 不涉及本差异）。