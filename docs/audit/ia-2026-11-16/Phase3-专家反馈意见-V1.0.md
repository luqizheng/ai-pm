# Phase 3 — 专家反馈意见 V1.0（记录）

- 日期：2026-11-16 · 来源：专家对《Phase3-专家反馈包-v1.0.md》的回复
- 性质：**专家建议裁决**。按专家执行边界：本文档条目先入台账标记「专家建议」，**经正式裁决流程确认后方冻结**；不代表已修改源码、迁移或 Phase 2 实施分支
- 总体结论：同意 A 节全部通报；B 节六项按下述意见进入裁决台账

> 核心架构原则（专家原话归纳）：**领域独立性由业务身份、权威边界和状态机保证，而不是要求所有领域使用完全独立的底层技术设施；但也不能因复用基础设施就让不同领域共享正式认知对象或互相写入对方的数据。**

## B-1 物理复用形态：确认独立 projc.* schema

采用独立 `projc.*` schema，复用 DDL 模式、服务层模式和通用基础设施，**不在 pc.* 加 owner_domain**。

理由：① pc.* 核心 0 行=架构调整窗口，加 owner_domain 会使对象身份/版本/CAS/发布事务/触发器/约束全路径长期维护域判别逻辑，形成数据库层隐性耦合；② schema 直接表达边界（pc.*=PC，projc.*=ProjC）；③ 复用模式 ≠ 复用业务表。

**跨域检索与关系遍历（不否决独立 schema，但须补适配层）**，分三层：

| 层 | 职责 |
|---|---|
| Domain Storage | pc.*、projc.* 各自维护对象与版本 |
| Domain Reference | 统一 DomainRef 契约引用其他领域对象 |
| Unified Retrieval | 聚合各领域检索结果，不接管领域正文和权威 |

0007 冻结项继续继承：关系类型由确认权威域维护；普通遍历不因跨 schema 放宽；跨域引用显式声明目标领域/对象类型/ID/版本上下文；统一搜索聚合结果但不产生跨域写入权；**FTS、PageIndex 是检索能力，不是新的知识权威**。P3-10 经统一检索适配层聚合，不为统一检索把各域认知放回同一 schema。

**实施设计闭合条件**（不改方向，裁决稿必须说明）：DomainRef 四元组跨 schema 解析；跨域关系授权与遍历边界；删除/归档/版本更新后的引用完整性；统一检索保留原始领域身份与权限过滤；PageIndex 多域索引不产生双重正文权威。

## B-2 过渡期处置：有条件接受，不允许错误领域形成新正式权威

区分三个东西：审阅流程与用户交互 / 分析快照与临时结果 / 正式认知发布事务。

| 事项 | 过渡期策略 |
|---|---|
| 审阅流 v2 | 保持冻结，不重新设计 |
| Finding 产生 / 人工确认 | 可以继续，按既有规则 |
| 分析报告与快照 | 可以继续，但必须标识分析产物性质 |
| **正式发布到 pc.*** | **不应再把 ProjC 对象作为 PC 正式认知发布** |
| ProjC 正式发布 | 待独立 schema 和发布链路具备后启用 |

细则：snapshot 类已写入的 pc.* 数据在迁移完成前视为**遗留实现中的过渡数据**，不形成新领域权威；有界过渡五要素=过渡范围 / 数据性质 / 停止正式写入触发条件 / 禁止过渡数据用于 PC 正式检索或发布 / 不新增依赖错误归属的业务能力。**不建议双写**。

> 设计组核验补充（2026-11-16）：过渡数据的精确性质已实证——`crates/pm-infrastructure/src/kb_pc_analysis.rs:718-797`，现行链发布的即 repository / tech_stack / delivery_activity / module 四类 snapshot 对象（写入 pc.cognition_object/revision/version，经 close_report→publish_version）。裁决稿的「停止触发条件」须与 Phase 2 在途任务包（P2-01~08）协调。

## B-3 delivery_activity：不纳入 ProjC 正式认知对象

属运行动态/分析指标，非稳定正式认知。归属建议：近期 commit 活动→Repository/Engineering Activity；checkpoint 派生摘要→Analysis 或执行监控；长期技术架构认知/已确认技术约定→ProjC；交付活动形成的技术发现→Finding 待人工确认。

处置：从 ProjC 正式对象字典移除；是否留 Analysis 快照能力由 Analysis 产品设计定；无实际消费者则裁掉，**不为保留旧表增加业务概念，不把动态数据凑齐快照对象类型而升级成正式认知**。

## B-4 Technical Conflict：ProjC 自建；pc.conflict 标 Legacy

理由：冲突主语义是技术认知矛盾，沿 pc.conflict 会让 PC schema 内存在另一领域正式对象、注册表归属不一致、两种冲突语义混淆；L3 冻结守卫无解冻入口，手工插入不能当迁移方案。跨域证据 ≠ 冲突对象跨域共用：技术↔技术冲突归 ProjC，产品规则↔产品规则归 PC，跨域矛盾由两域协调但一方不得拥有另一方对象。

ProjC Technical Conflict 经 DomainRef 引用：冲突双方正式认知、相关 Finding、Source/Evidence、涉及的 Document/Memory。边界：**冲突跨域不等于冲突对象可替代各领域正式确认权**。

pc.conflict 处置五条：保留原表与迁移记录；不解冻守卫；不手工插数；注册表标记 **Legacy / Not for New Use**；未来清理走独立迁移方案。不改名充当 ProjC 对象，不视图伪装已迁移。

## B-5 P3-KB-02 可见性：Default Deny + Permission Intersection

权限维度与授权性：

| 维度 | 作用 | 单独授予访问权 |
|---|---|---|
| Team Membership | 属于资产所属 Team | 否 |
| KB Permission | KB 访问权限 | **是（必要条件）** |
| Project Membership | 项目访问权 | 否 |
| Project Association | 业务关联 | 否 |
| Asset Visibility | 资产自身可见级别 | **是（必要条件）** |
| Operation Permission | 编辑/确认/发布等 | **是（必要条件）** |

访问判断=权限条件**交集**。三个具体裁决：

1. Team 成员身份不自动授权；Team Role 可配置默认 KB Read；受限资产要求额外权限；编辑/审核/确认/归档独立操作权限。
2. **Project Association 不作为授权来源**：kb.source_project 只表关联；项目上下文打开关联知识仍须过 Team Membership + KB Permission + 资产可见级检查。
3. 直接继承既有身份与成员关系校验机制（专家表述为 K4），KB 只定义自己的资源授权规则，**不建第二套 Team/Project Membership**。

授权链：Identity → Team Membership/K4 → KB Permission → Asset Visibility → Operation Permission → Access Decision；Project Association 不进授权链，只提供业务上下文。

特殊情况（Memory M-001 关联 A/B 两项目）：不能因关联 A 就对全员公开，也不能为 B 的限制复制 Memory；需不同保密范围用**资产级权限或内容拆分**解决，不把项目关联当访问控制。

> 设计组术语核验注记：代码中 K4 现指 memory revision 的治理控制点（K4 CAS 基准，require_human+行锁链），专家此处将其用作身份/成员关系校验层。P3-D02 权限模型落位时须对齐 K4 术语语义，避免与 CAS 控制点混淆。

## B-6 上轮议题衔接

| 议题 | 专家意见 |
|---|---|
| Q1 断链 | 确认 Extraction 入 V1；四条边界：**Extraction 是可追溯提取过程与关系载体；≠Memory；无正式知识确认权；结果必须进 Proposal/Review/Revision 流程**。**Document Revision 必须作为实际提取依据钉扎**，不能只关联 Document 身份 |
| Q2 Git 权威 | 确认不进 Phase 3 关键路径；Phase 3 优先=对象身份/Revision/确认机制/Extraction/Proposal/Review/权限审计；0023 触发器+hash 锚链不强行改；**不进关键路径 ≠ 放弃目标架构**，保留演进事项，禁 PG/Git 双主 |
| KB-01~09 | 不凭编号批量冻结；按类别冻结（见台账 §9 P3-D02 组），产品职责可先冻结，**数据库实现不必同时冻结** |
| 四动作 | 映射原则确认；映射：Accept→confirm、Approve→confirm、Confirm→confirm、Reject→reject、Defer→defer、Investigate→investigate；**Apply 不直接映射**——Apply 是执行/应用动作非审核决定：DecisionBody 记录人工确认 → 业务服务执行发布事务 → Revision 成正式版本；把 Apply 当 confirm 会混淆人的决策与系统的执行 |

## 专望建议台账条目（待正式裁决确认）

| 编号 | 建议裁决内容 | 状态 |
|---|---|---|
| P3-11.1 | ProjC 使用独立 projc.* schema，复用基础设施模式 | 建议确认 |
| P3-11.2 | PC 与 ProjC 独立对象身份、字典、状态机、确认权威、版本边界 | 建议确认 |
| P3-11.3 | Finding 按业务含义分流，不自动成为正式认知 | 已有方向，建议冻结 |
| P3-11.4 | 过渡期不改审阅 UX，但不允许错误领域形成新正式认知权威 | 建议确认 |
| P3-11.5 | delivery_activity 不纳入 ProjC 正式认知对象 | 建议确认 |
| P3-05 | Technical Conflict 由 ProjC 自建；pc.conflict=Legacy/Not for New Use | 建议确认 |
| P3-KB-02 | Default Deny + 权限交集；Project Association 不授权 | 建议确认 |
| P3-D02.1 | Extraction 入 KB V1，保留来源 Document Revision 证据 | 建议确认 |
| P3-D02.2 | Git 迁移不进 Phase 3 关键路径 | 建议确认 |
| P3-D02.3 | Apply 是执行动作，不直接映射 DecisionBody 决策动作 | 建议确认 |
| P3-08 | 继承 Revision/Confirmation/Audit 现有矩阵 | 进入评审 |

随 P3-D02 冻结类别：Document≠Memory；Memory Revision+人工确认硬门；Extraction 职责与来源追溯；Memory 生成链路入 V1；Source→Document→Revision 来源关系。结合 P3-08：Proposal/Review 状态机细节；Revision/Confirmation/Audit 边界。结合注册表核验：引用表与关系结构。结合 P3-KB-01/02：访问权限。Phase 3 之外：Git 迁移实现。

## 专家建议的下一步交付（三份，先不改数据库设计）

1. **《P3-11 仓库分析认知归属与双域治理裁决稿》**：PC/ProjC 职责边界；独立 schema 方案；DomainRef 跨域引用方式；统一检索适配原则；过渡期处置；delivery_activity 与 pc.conflict 处置；现行 close_report→publish_version 链路目标落点。
2. **《P3-D02 KB 领域模型 V0.3》**：现行实现与目标产品模型对照；五对象；Extraction 身份与职责；Proposal 双轨现状及统一业务流程；Review 流程机制与审核记录区分；Team 级资产与 Project 关联；权限模型；正式知识生成链路。
3. **《P3-08 生命周期与权威矩阵继承评审》**：核验 lifecycle-matrix / authority-matrix 与三域模型的冲突，逐条记录继承/修订/待裁决。

**专家最终结论**：总体方向认可（独立权威+共享基础设施+引用契约；禁双写/双主/错误权威；Memory 须经 Extraction→Proposal→人工审核→Revision；Team 资产≠全员可见、项目关联≠授权；先 P3-11+P3-08 再冻结 D02/03/04）。下一轮直接进入 **P3-11 正式裁决稿评审**，独立 projc.* schema 作为默认推荐方案。
