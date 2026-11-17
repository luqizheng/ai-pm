# P3-11 裁决稿 V1.1 — 仓库分析正式认知归属与双域治理

- 日期：2026-11-16
- 状态：**裁决稿 V1.1（已落实专家评审四项修订），待正式逐项裁决**
- 版本记录：V1.0（2026-11-16 初稿）→ **V1.1（落实专家评审 4 项修订：R-07 DomainRef 字段语义重构 / R-04 停止条件升级为能力就绪+验收 / R-03 模块消歧 / R-06 跨域冲突归属原则；另收编 R-01/R-08 附注与两条架构红线）**。V1.0 与 V1.1 差异明细见 `Phase3-P3-11裁决稿专家评审意见-V1.0.md`
- 裁决范围：台账 §9 中 P3-11.1~P3-11.5 + P3-05
- 不在本稿：P3-KB-02、P3-D02.1~.3、P3-08、P3-10（仅交叉引用）
- 依赖的冻结项：四域对象模型 V1.0、0007 `owner_domain`=关系确认权威域语义、审阅流 v2 语义（Phase 2）、AGENTS.md 纪律
- 源码实证基准：master `d3b30739`；关键证据 `crates/pm-infrastructure/src/kb_pc_analysis.rs:718-797`、`crates/pm-application/src/analysis_report.rs` close_report、`migrations/0007`（`relationship_type.owner_domain DEFAULT 'pc'`）、`Phase2-实施任务包-P2-01-P2-08-v1.0.md` 领域零改动红线

---

## 0. 裁决项总览

| 编号 | 裁决项 | 默认方案 | 专家评审 |
|---|---|---|---|
| R-01 | ProjC 物理形态 | 独立 `projc.*` schema，复用 DDL/服务层模式（=P3-11.1） | 确认（附 source 语义约束） |
| R-02 | 双域权威边界 | 身份/字典/状态机/确认权威/版本边界全独立（=P3-11.2） | 确认，建议冻结 |
| R-03 | 仓库分析产物分流判据 | 按正式认知业务含义分流；Finding 仅候选（=P3-11.3） | 确认（V1.1 补模块消歧） |
| R-04 | 过渡期处置 | UX 不动 + 有界过渡，禁止错误领域形成新正式权威（=P3-11.4） | 修订后确认（V1.1 升级停止条件） |
| R-05 | delivery_activity | 不纳入 ProjC 正式认知对象（=P3-11.5） | 确认 |
| R-06 | Technical Conflict | ProjC 自建 `projc.conflict`；pc.conflict=Legacy（=P3-05） | 确认（V1.1 补归属原则） |
| R-07 | 跨域引用契约 | DomainRef（target_domain 四字段）+ revision 钉扎 | 确认（V1.1 重构字段语义） |
| R-08 | 统一检索 | 三层架构（Storage/Reference/Retrieval），检索能力非权威 | 确认（V1.1 补结果元数据） |
| R-09 | close_report→publish 落点 | 按对象域分流发布，UX 与端点契约不变 | 确认（V1.1 明确过渡行为） |
| R-10 | pc.* 存量 snapshot 数据 | 遗留过渡数据，不构成任何领域正式认知权威 | 确认 |

## 1. PC 与 Project Cognition 职责边界（R-02）

| 维度 | PC（pc.*） | Project Cognition（projc.*） |
|---|---|---|
| 正式认知内容 | 产品定位、能力、模块、业务规则等 | 技术架构、技术约定、技术决策、技术组件等 |
| 对象分类 | 独立字典：现有 14 类产品类型（L1_identity~L5_decision） | 独立字典：repository/tech_stack/repository module/technical component 及后续技术认知类型（粒度随 P3-D04 细化） |
| 正式确认 | PC 自己的人工确认规则（proposal/approval/publish 链） | ProjC 自己的人工确认规则（同模式独立实例） |
| 正文与版本权威 | PG+Revision/Version（PC 域） | PG+Revision/Version（ProjC 域），各自单一权威 |
| 业务身份 | 独立（object_key 命名空间独立） | 独立 |

五条权威边界（冻结表述）：

1. 不共享正式知识对象身份。
2. 不共享未经定义的状态机。
3. 不允许一个领域直接修改另一个领域的正式认知。
4. 不允许两个领域同时成为同一知识正文的权威。
5. 跨域关联只走引用契约（R-07）。

**仓库分析产物分流判据（R-03）**：

| 仓库分析结果 | 正式认知归属 |
|---|---|
| 技术栈构成、仓库快照、模块/热点结构、系统技术架构、已确认技术约定 | Project Cognition |
| 产品业务能力、产品业务规则、产品模块与功能（含 S01 D-4 身份综合） | PC |

**模块消歧（V1.1 新增）**：

| 类型 | 领域 | 含义 |
|---|---|---|
| Product Module | PC | 产品功能、业务能力的组织单元 |
| Repository Module | ProjC | 仓库代码结构中的技术模块 |
| Technical Component | ProjC | 技术架构中的组件 |
| Business Capability | PC | 产品或业务能力 |

相似自然语言名称 ≠ 相同对象身份，字典定义各自独立。Repository Module 实现某 Product Module 时：两域各自维护认知、经显式关系引用；**ProjC 不能因代码扫描发现某功能就自动创建 PC 的 Product Module**。D-4 身份综合限定：**只有经业务语义判断确属产品身份的候选才进入 PC**，不把所有仓库身份扫描结果归入 PC。

仓库扫描发现首先=分析证据/候选 Finding，**不因分类命中自动成为正式认知**。

## 2. 独立 schema 方案（R-01）

### 2.1 被否决方案

| 方案 | 否决理由 |
|---|---|
| ① 承认现状：snapshot 类即 PC 正式认知 | 违反 G-01 与四域注册表；技术认知被登记为产品认知 |
| ② pc.* 加 owner_domain 列 | pc.* 核心 0 行（迁移红利窗口）无需付出长期代价；加列使对象身份/版本/CAS/发布事务/触发器/CHECK 全路径维护域判别逻辑=数据库层隐性耦合；AGENTS.md 禁兼容分支 |

### 2.2 projc.* 机制规划（机制级，非 DDL；详细设计随实施任务包）

- 表集沿用 pc.* 已验证的机制模式：object / object_type（独立字典）/ cognition_revision / cognition_version + version_member / proposal + proposal_member / approval / evidence / source / relationship(_type/_revision) / write_operation / conflict。
- **复用的是模式（发布事务 advisory lock + expected_version_no 乐观锁 + 幂等键重放 + 行不可变触发器 + 生命周期 CHECK）**，不复用业务表实例；每域状态机独立定义。
- projc.object_type 初始字典 = **repository / tech_stack / repository_module（含 technical component 语义，最终拆分随 P3-D04）** 三类；**delivery_activity 不迁**（R-05）。
- 服务层按域拆分（pc:: / projc:: 模块），共享的只有通用基础设施模式。
- **projc.source 语义约束（V1.1 收编）**：projc.source 只能是 ProjC 自己的证据来源登记/领域引用记录，**不得成为与 kb.source 同权威含义的第二正式来源资产**；对正式来源资产的引用一律走 DomainRef 指向 kb.source。不形成两个 Source 权威。

### 2.3 与 0007 的衔接

`pc.relationship_type.owner_domain`（DEFAULT 'pc'）语义=**该类型关系实例的确认权威域**，0007 冻结语义原样保持；跨域关系类型沿用同一注册表机制，owner_domain 枚举值扩展 `project_cognition`（扩展的迁移方案见 §8 检查点）；**禁双 ext 绕过 owner_domain** 的既有裁决继续有效。DomainRef 的字段语义见 §3，与 owner_domain 严格分离。

## 3. DomainRef 跨域引用契约（R-07，V1.1 重构）

**DomainRef 描述被引用对象，RelationshipType.owner_domain 描述关系确认权威，两者分离，禁止合并为一个 Domain 字段。**

DomainRef 四字段：

| 字段 | 回答的问题 |
|---|---|
| `target_domain` | 被引用对象属于哪个领域 |
| `target_object_type` | 被引用对象是什么类型 |
| `target_object_id` / `target_object_key` | 被引用对象是谁 |
| `revision_context` | 引用哪个版本/版本上下文（钉扎） |

- 示例：ProjC 维护一条「Repository Module 实现 Product Module」关系——该关系类型的 `owner_domain=project_cognition`（确认权威在 ProjC），其 DomainRef 的 `target_domain=pc`（目标对象身份属于 PC）。两者互不替代。
- 跨 schema 解析：各域提供本域 resolver（domain→schema 路由表），引用方不直接跨 schema 查表。
- 目标不可达时记录 `revision_missing_reason`（沿用 pc.source 先例），不悬空。
- 本节为领域契约表达，不要求立即建立数据库字段；实际建列随实施任务包。
- 引用完整性闭合条件（实施设计必须回答，不阻塞本裁决）：删除/归档/版本更新后的引用行为；遍历边界（0007 双端隔离规则跨 schema 继续生效）；授权边界（红线二）。

## 4. 统一检索适配原则（R-08）

| 层 | 职责 |
|---|---|
| Domain Storage | pc.*、projc.* 各自维护对象与版本，各自 FTS/PageIndex 索引 |
| Domain Reference | DomainRef 契约引用他域对象 |
| Unified Retrieval | 聚合各域检索结果；不接管正文与权威 |

继承规则：FTS/PageIndex 是检索能力**不是新的知识权威**；统一搜索聚合结果不产生跨域写入权；不为统一检索把各域认知放回同一 schema。

**结果元数据要求（V1.1 收编）**：统一检索返回的每一条结果必须可识别**原始领域、原始对象身份、版本上下文、访问权限判断结果**——不能只返回合并后的文本片段，否则无法准确追溯与执行操作。

P3-10（跨域搜索产品形态）以本层为前提，另行设计。

## 5. 过渡期处置（R-04 / R-09 / R-10）

### 5.1 现行链组件 → 目标落点（R-09）

| 现行组件（实证） | 现行为 | 目标落点 |
|---|---|---|
| worker `plan_cognition_objects`（kb_pc_analysis.rs:735-797，确定性无 LLM） | repository/tech_stack/delivery_activity/module 四类草稿 upsert 进 pc.* | 裁决冻结后：repository/tech_stack/module（→ repository_module）→ **projc.***（新链就绪时）；delivery_activity → 停止派生（R-05） |
| identity.rs `propose_pc_object`（D-4 身份综合） | 产品身份类草稿进 pc.* | **留在 pc.***；仅业务语义判断确属产品身份的候选进入（§1 限定） |
| pc.proposal + 审阅 v2（决定暂记） | 人工 approve 暂记 | **UX/端点契约不变**；提案按目标对象域分流到各自 proposal 表（新链就绪时） |
| close_report → publish_version | approved 提案批量发布 pc.cognition_version | **分流发布**：PC 提案照旧 pc.cognition_version；ProjC 提案 → projc 发布链（就绪后启用）；就绪前 snapshot 类提案**不发布为 PC 正式版本**（§5.2 过渡行为） |
| AN-REPORT 裸列 pc_source_id / kb_revision_id | 报告挂 PC 证据源与 KB 修订 | 保留；ProjC 证据源注册（projc.source）随新链启用，是否加列属实施设计 |
| pc.evidence / pc.source（worker 证据注册） | 混载分析证据 | 模式复用，按目标域分流注册；正式来源资产引用一律 DomainRef→kb.source |

### 5.2 过渡行为（V1.1 明确，消除 R-04/R-09 表述不一致）

ProjC 发布链就绪前：

- 审阅流 v2 的**决定暂记继续**；
- **close_report 可以完成报告关闭**（状态机、锚点固化、门释放不变）；
- snapshot 类提案**保留审核结果与历史记录**；
- **但不允许将其发布为 PC 正式认知版本**——需一个过渡守卫（实施任务项 T-1，见 §5.4）：close 发布分支对 snapshot 类提案跳过/挂起正式发布并计数留痕；
- **不能为了维持端点行为而继续产生错误领域的正式版本**；
- **不双写**（不因过渡在 pc 与 projc 间双写）。

### 5.3 有界过渡：停止条件（V1.1 升级）与五要素

**停止条件=能力就绪并通过实施验收**，不是代码合入 master。就绪判据七条：

1. projc.* schema 与对象字典就绪；
2. ProjC 独立 Revision/Version 发布链就绪；
3. 人工确认权限与正式发布硬门就绪；
4. DomainRef 解析及引用权限检查就绪；
5. 审阅流 v2 可将提案路由到正确领域；
6. 相关测试、注册表、迁移记录完成；
7. 旧路径有明确停用开关和验证结果。

七条全满足前，旧 snapshot 正式发布路径保持 §5.2 守卫态，避免「旧路径停了、新路径不能发布、正式认知链路中断」。

五要素：① 过渡范围=kb_pc_analysis.rs snapshot 派生链 + close_report 的 snapshot 类提案发布路径（identity 综合链不在过渡范围）；② 数据性质=pc.* snapshot 类已写入行标记 **Legacy Transitional**，不构成任何领域正式认知权威；③ 停止触发=上述七条就绪验收；④ 禁止事项=过渡数据进 PC 正式检索/发布语义、不新增依赖错误归属的业务能力、不双写；⑤ Phase 2 协调=见 §5.4。

### 5.4 Phase 2 协调（V1.1 实证结论）

`Phase2-实施任务包-P2-01-P2-08-v1.0.md` 逐项核对结果：

- P2 有**领域零改动红线（六条）**：不创建 Report Revision/FCR 实体或 API；不迁移 KB/PC 审批逻辑；不改 `repo_analysis_report` 状态机；**不改 close 事务（批量发布/撤回/锚点固化）**；不复制报告正文造第二权威。
- P2 触及 close 路径的仅为 UI 消费与测试断言（报告关闭按钮 `POST /v1/pm/analysis-reports/{id}/close`、E2E-02「暂记→互转→close 批发全链 + 409 冻结路径」），**无任务修改 close 事务本身**。
- 结论：**过渡守卫 T-1 不能放进 P2 任务包**（红线明令不改 close 事务），须编 Phase 3 独立实施任务项（close_report 发布分支守卫 + 停用开关 + 验证结果），时序上等 P2-01~08 落地或独立分支协调，避免同文件冲突。
- **E2E-02 的 close 批发断言须随 T-1 同步调整**（登记测试漂移），否则过渡期测试会断言出错误领域的正式版本。

### 5.5 存量处置（R-10）

pc.* snapshot 类存量行：保留原位不删除（审计口径），标记过渡性质；迁移与否由独立迁移方案在 projc.* 启用时决定（全库 0 行现状下=直接弃置重跑分析，无搬运）。**不通过手工插入、改名或视图伪装完成迁移**。

## 6. delivery_activity 与 pc.conflict 处置（R-05 / R-06）

**delivery_activity**：从 PC/ProjC 正式认知对象字典移除；commit 流摘要/checkpoint 派生归 Analysis 或执行监控能力（由 Analysis 产品设计决定）；交付活动中形成的技术发现走 Finding→人工确认→分流（R-03）。**不为兼容历史 snapshot 继续作为正式认知类型，不为凑齐快照类型把动态数据升级成正式认知**；无消费者则裁掉。

**Technical Conflict（V1.1 补归属原则）**：ProjC 自建 `projc.conflict`（+conflict_object 挂冲突双方）；冲突双方经 DomainRef 引用正式认知、Finding、Source/Evidence、涉及的 Document/Memory。

所有权原则：**冲突对象的所有权由主要冲突认知所属领域决定；涉及其他领域时通过 DomainRef 引用对方对象，双方各自履行确认责任。**

| 冲突情形 | 归属 |
|---|---|
| 技术架构 ↔ 技术约定 | ProjC |
| 产品业务规则 ↔ 产品决策 | PC |
| 技术架构 ↔ 产品业务规则 | 主要冲突认知所在域持对象；对方经 DomainRef 引用，各自履行确认责任 |

示例（规则与技术架构矛盾）：PC 不擅自改技术架构；ProjC 不擅自改产品业务规则；ProjC 可提技术侧替代方案；PC 可重新评估业务规则；双方确认后各自更新本域正式认知。**暂不新增 Global Conflict 领域或统一冲突中心**；跨域协调经引用与待处理事项完成。

pc.conflict 处置：留表留迁移记录；不解冻 L3 守卫；不手工插数；注册表标 **Legacy / Not for New Use**；未来清理走独立迁移方案；不改名充当 ProjC 对象、不视图伪装迁移。

## 7. 架构红线（V1.1 新增；进四域通用治理规范，不限于 PC/ProjC）

**红线一：共享技术基础设施不得产生共享业务权威。**

| 可共享 | 不可共享 |
|---|---|
| DDL 模式、CAS 与幂等机制、发布事务模式、通用基础设施代码、DomainRef 契约、检索基础设施 | 正式对象身份、未定义的状态机、正式确认权、正文权威、领域业务字典 |

**红线二：跨域引用不构成跨域授权。**

任何领域引用其他领域的对象，都不意味着：可以直接修改目标对象；可以替目标领域确认；可以绕过目标领域的审核；可以自动继承目标对象的全部访问权限。

## 8. 正式裁决确认清单与衔接检查点

逐项确认 R-01~R-10（可改判，改判须记录理由与影响）：

裁决记录（2026-11-17 独立专家意见 V1.0，详情见台账 §10.7）：R-01~R-10 + 红线共 11 项——**7 项批准（R-01/02/03/05/08/09/10）、3 项修改后批准（R-04/06/07）、0 项不批准**；无不批准项。批准架构条款 ≠ 该条款涉及的全部工程能力已完成，R-04/06/07 附加条件保留于台账。

- [x] R-01 独立 projc.* schema（含 projc.source 语义约束）——批准
- [x] R-02 双域权威边界五条——批准（P3-D04 设计前提）
- [x] R-03 分流判据表 + 模块消歧 + D-4 限定——批准
- [x] R-04 有界过渡（七条就绪判据 + 过渡行为）——**修改后批准**：④DomainRef 完整 resolver=Partial 归 P3-10（O-01）；七项不得登记为全部满足
- [x] R-05 delivery_activity 移除——批准
- [x] R-06 ProjC 自建 conflict + 归属原则 + pc.conflict Legacy——**修改后批准**：IMPLEMENTED 保留，生产登记入口 O-06 归 P3-D04
- [x] R-07 DomainRef target_domain 四字段契约——**修改后批准**：引用完整性细则 O-02 归 P3-D04；字段名称映射 O-09（revision_context ↔ target_revision_id/revision_missing_reason）
- [x] R-08 统一检索三层 + 结果元数据要求——批准（架构原则；实现归 P3-10）
- [x] R-09 close→publish 分流落点 + 过渡行为——批准
- [x] R-10 存量=Legacy Transitional——批准
- [x] §7 两条架构红线纳入四域通用治理规范（P3-D05 承接）——批准（另采纳跨域权限闭环原则：跨域操作须同时满足调用方授权+目标领域授权+目标对象当前状态约束）

三个衔接检查点（正式裁决前核验）：

| 检查点 | 状态 |
|---|---|
| 四域注册表：projc.* 纳入注册表、pc.conflict 标 Legacy | ☑ 已执行（注册表 V1.1 附录 V1.3：projc 七对象纳入、pc.conflict=Legacy、Decision 维度 2026-11-17） |
| 0007：owner_domain 保持原语义 ✅（本稿 §2.3 已按原语义表述）；枚举扩展 `project_cognition` 的迁移方案 | ☑ 已按 0026 独立字典实现（projc.relationship_type owner_domain NOT NULL DEFAULT 'project_cognition' + CHECK 收敛；未扩展 pc 枚举——与 R-01 独立 schema 一致） |
| Phase 2 在途任务核对 | ✅ 已完成，结论见 §5.4（T-1 独立任务项 + E2E-02 漂移登记） |

确认后动作：台账 §9 状态列更新 → P0 解除 → P3-D03/D04 开工 → 实施任务包（schema/服务层/过渡守卫 T-1/停用开关）另编。

## 9. 遗留开放项（不阻塞本裁决）

1. 五条实施闭合条件细则：DomainRef 跨 schema resolver 实现、跨域遍历授权边界、引用完整性（删除/归档/版本更新行为）、统一检索权限过滤实现、PageIndex 多域索引防双重正文权威。
2. projc.object_type 最终粒度（repository_module 与 technical component 是否分列等技术认知类型）→ P3-D04。
3. AN-REPORT 是否加 projc_source_id 列、报告详情六节的 ProjC 提案展示 → 实施任务包 + P3-D04。
4. 存量 snapshot 数据弃置/重跑 → 独立迁移方案（迁移红利窗口内）。
5. T-1 过渡守卫的实施窗口与 P2-01~08 的时序协调 → 实施任务包编排。
