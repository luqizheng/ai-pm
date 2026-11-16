# Phase 3 — 基线校准与裁决台账 V1.0

- 日期：2026-11-16
- 性质：对《Phase3-基线评审意见-v1.0.md》的处理决定 + P3-11 方向确认 + 裁决台账
- 效力：P3-D01 V0.1 保留为**基线记录**（非闭合基线）；本台账为 Phase 3 现行工作清单真源
- 结论：**评审 READY WITH CONDITIONS 被接受；P3-11 方向确认（评审者附条件），正式裁决待开**

---

## 1. 对评审意见的处理决定

1. 接受 READY WITH CONDITIONS 结论。
2. P3-D01 V0.1 保留为基线记录，不视为已闭合的三域设计基线。
3. 此前 KB V0.2 草案中四项主张**立即修正**（§2），两张绑定表暂缓（§2.2）。

## 2. KB 领域模型四项修正

| 原设计主张 | 评审事实 | 修正决定 |
|---|---|---|
| Document 与 Memory 都以 Markdown+Git 为正文权威 | Document Revision 正文在 PG；Memory 正文在 Revision；Git 是 Document 的目标演进方向（KB-02 已认账偏差+硬阻碍） | **不得将目标态写成现状约束** |
| Review 是 KB 独立领域对象 | Review 是审核机制，记录由 governance.review_approval 等承载 | 产品概念可保留 Review，**不宣称存在 kb.review 对象** |
| Memory 可直接关联 Document | kb.memory_ref 只支持 KB 域内 target_type=source | **必须设计 Extraction 或扩展引用模型，不假定已有能力** |
| Proposal 是统一 KB 提案对象 | 两条实现路径：kb.memory_revision PENDING_REVIEW（memory 侧）+ ai.document_proposal 8 态（document 侧，KEEP） | **保留现状差异，不直接合并为统一 Proposal 实体** |

### 2.2 绑定表暂缓

`kb.document_project_binding` / `kb.memory_project_binding` 暂缓。已存在 `kb.source_project`；Document、Memory 的项目关联方式须依注册表与业务需求确定，不得因产品关系是 N:M 就直接指定两张表。

### 2.3 校准原则

**产品领域对象、现行数据库对象、目标架构对象，必须分别描述。**

## 3. P3-11：仓库分析正式认知归属与 pc.* 双职责治理 —— 方向确认

### 3.1 评审者立场：**方向确认，附以下边界**

方案③（ProjC 沿用 pc.* 基础设施、独立字典与确认权威）作为正式裁决讨论起点，前提是五条边界成立：

1. 不共享正式知识对象身份。
2. 不共享未经定义的状态机。
3. 不允许一个领域直接修改另一个领域的正式认知。
4. 不允许两个领域同时成为同一知识正文的权威。
5. 跨域关联必须采用明确的引用契约。

**共享数据库基础设施 ≠ 共享领域权威。**

### 3.2 分流判据：按正式认知的业务含义，不按分析结果类型机械分流

| 仓库分析结果 | 正式认知归属 |
|---|---|
| 项目使用哪些技术栈 | Project Cognition |
| 当前仓库的模块结构 | Project Cognition |
| 系统技术架构 | Project Cognition |
| 已确认的技术约定 | Project Cognition |
| 产品包含哪些业务能力 | PC |
| 产品业务规则 | PC |
| 产品模块与功能 | PC |

仓库扫描发现的内容首先是**分析证据或候选 Finding**，不因分类命中自动成为正式认知（与冻结原则一致）。

### 3.3 正式裁决前必须裁定的三个子题（评审者补充输入）

| 子题 | 选项 | 评审者建议 |
|---|---|---|
| ① 物理复用形态 | a. pc.* 表加域判别列（object_type/cognition_object 无 owner_domain 列，需加列并穿透发布事务/CAS/触发器）；b. 独立 schema（projc.*）复用 DDL 模式与服务层模式 | pc.* 核心对象全库 0 行（迁移红利窗口）+ AGENTS.md 禁兼容分支 → **b 优先评估**；迁移成本不是决定因素，长期治理才是。不先以复用为由定业务模型 |
| ② 字典与权威列 | pc.object_type 无域列；关系的确认权威域在 pc.relationship_type.owner_domain（0007 终裁语义=确认权威域） | 无论 a/b，owner_domain 枚举须扩 PROJECT_COGNITION 值；对象字典须带域归属列（或独立字典） |
| ③ 过渡期处置 | 现行 close_report→publish_version→pc.cognition_version 链在裁决落地前的处理 | 审阅流 v2 语义须保留（Phase 2 冻结）；确认/发布 UX 不动，仅落点域按裁决切换；snapshot 四类在裁决前不新增 charter 外语义 |

### 3.4 两个细项另议

- `delivery_activity`（近期 commit 流摘要，checkpoint-1 派生）：更像交付动态指标而非认知对象，是否保留为认知对象待裁决（可留在 Analysis 快照或裁掉）。
- `module`（热点文件映射）与 PC `feature_module`（产品功能模块）跨域同名：引用契约中必须消歧。

## 4. KB P3-D02 重整结构（代替原 V0.2 草案推进方式）

### 4.1 现行对象与目标能力分开

| 业务能力 | 现行实现 | Phase 3 设计目标 |
|---|---|---|
| Source | kb.source | 保留 Team 级来源管理及项目关联 |
| Document | kb.document | 保留独立文档身份 |
| Document Revision | kb.document_revision | 保留 Revision 和确认门 |
| Memory | kb.memory | 保留正式知识单元身份 |
| Memory Revision | kb.memory_revision | 保留 Revision + 人工审核硬门 |
| Document Proposal | ai.document_proposal | 保留现有提案语义，评估与新模型的衔接 |
| Memory Proposal | 由 Memory Revision 审核流程承载 | 补充完整提案生成及消费链路 |
| Review | 现有审核机制 | 明确产品流程与审核记录，不擅自新增独立对象 |
| Extraction | STUB / 缺失 | 补足 Document → Memory 的可追溯提取关系 |

关键认识：**Memory 不是 Document 的另一种正文格式，而是经过独立审核确认的正式知识单元。Document 发布不直接等于 Memory 成立。**

### 4.2 Memory 生成链路入 V1（Extraction 六步）

1. Document Published：文档完成发布，产生可引用的 Document Revision。
2. Knowledge Extraction：AI 分析文档，识别可形成正式知识的内容，保留来源位置及版本证据。
3. Memory Proposal：生成 Memory 新增/修改/废弃建议。
4. Human Review：KB 审核人检查内容、来源、重复性、冲突和适用范围。
5. Memory Revision Confirmation：通过现有正式确认机制形成新的正式 Memory Revision。
6. Knowledge Availability：正式知识进入 KB 检索和引用范围。

Extraction 必须可追溯：来源 Document → 实际使用的 Document Revision → 原始 Source → 提取内容及其来源位置 → AI 生成过程及证据 → 最终形成的 Memory。

### 4.3 Memory 四条设计红线

1. Memory 正文变更必须经过 Revision。
2. Memory Revision 必须经过正式人工审核和确认。
3. Accept ≠ Confirm；接受提案不等于发布正式 Revision。
4. 不得新增绕过现有审核门的直写通道。

现行 require_human、K4 CAS、行锁、触发器白名单**作为既有实现约束继承**，不重新设计替代机制。

## 5. 导航 IA 处置（不覆盖已冻结 IA）

### C-2 全局导航：恢复冻结 IA

- 全局：Personal、Dashboard、Projects、Team Settings。
- 项目区四域入口：Analysis、Knowledge Base、Product Cognition、Project Cognition。
- Team 级 KB 全局入口如要做，**单独提出导航变更裁决**。
- 维度区分：Team 级=KB 资产归属；Project 级=当前冻结的导航入口。与 P3-KB-01（Team 级资产，Project 作为关联范围）不矛盾。

### C-3 Reports 页签：按 Phase 2 D-1 执行

- kb 是 Knowledge 页默认页签；reports 继续存在，不混入 KB 六页签。
- Analysis 报告迁 `/project/analysis`（既有拆分计划）；PC 页签归 `/project/product-cognition`。
- 迁移完成前三合一状态显式保留，不伪装已拆分。
- 记录为**导航迁移约束**，不是新 IA 设计。

## 6. 调整后的工作优先级

| 级别 | 事项 | 依赖 |
|---|---|---|
| P0（领域权威冲突） | P3-11 仓库分析正式认知归属与 pc.* 双职责治理 | — |
| P0 | P3-08 Revision/Confirmation/Audit 边界（继承现有矩阵并完成评审） | — |
| P1（修正三域详细设计） | P3-D02 KB 领域模型（纳 Extraction 与 Memory 生成链路） | 依赖 P0 |
| P1 | P3-D03 PC 领域模型（处理 14 类对象与既有设计文档继承关系） | 依赖 P0 |
| P1 | P3-D04 Project Cognition 领域模型（正式认知、提案及确认边界） | 依赖 P0 |
| P2（统一产品入口与交互） | P3-09 导航 IA 与页面拆分裁决；三域审核中心/知识引用/跨域协作 UX | 依赖领域设计 |

P3-11 先于 P3-D03/D04：仓库分析正式认知归属未定，PC 与 Project Cognition 职责边界无法可靠冻结。

## 7. 裁决台账（现行真源）

| 编号 | 事项 | 当前处理意见 |
|---|---|---|
| P3-KB-01 | Team 级统一资产 | **已确认 A，不重开**；下一步与 kb.source_project 及 Document/Memory 真实实现对齐 |
| P3-KB-02 | Team Knowledge Visibility | 待裁决，不预设方案 |
| P3-08 | Revision / Confirmation / Audit | 继承 lifecycle-matrix / authority-matrix，补充评审 |
| P3-09 | 全局入口与导航 IA | 恢复冻结 IA，变更需单独裁决 |
| P3-10 | KB Reports 页面去向 | 按 Phase 2 D-1 和拆分计划执行 |
| P3-11 | 仓库分析认知归属与 pc.* 双职责治理 | **P0 待裁决**（方向已确认，见 §3） |
| P3-01 | Document vs Memory | 有条件通过，补充 Extraction |
| P3-03 | PC 对象分类与既有设计继承关系 | 待细化 |
| P3-04 | Project Cognition 对象模型 | 待细化 |
| P3-05 | Technical Conflict 归属 | 待裁决 |

> 编号漂移注记：本台账 P3-03/04/05/10 的语义相对基线 V0.1 §9 已重新划分（原 P3-04「PC 项目级归属」已冻结故移出；原 P3-05 对象粒度并入现 P3-04；原 P3-10 跨领域搜索暂缓）。**以本台账为 Phase 3 编号真源。**

## 8. 执行顺序

1. 记录本文件与评审意见（已完成）。
2. **P3-11 领域归属正式裁决**（输入包已备：§3.3 三子题 + §3.4 细项）。
3. 更新 P3-D01：明确继承、修订、待裁决清单。
4. 重整 P3-D02 KB 领域模型（Extraction + Memory 生成链路，§4 结构）。
5. 继承 P3-08 生命周期与权威矩阵，避免重复设计。
6. 再进入 PC 与 Project Cognition 详细设计。

**不沿 KB 权限选择题（P3-KB-02）直接推进**——KB 模型须先校正，且 P3-11 影响三域权威边界。

## 9. 专家建议条目（V1.0，2026-11-16 回复；状态=专家建议，经正式裁决确认后冻结）

全文见 `Phase3-专家反馈意见-V1.0.md`。总原则：**领域独立性由业务身份、权威边界、状态机保证，共享底层基础设施不等于共享权威**。

| 编号 | 专家建议裁决内容 | 专家状态 | 正式裁决状态 |
|---|---|---|---|
| P3-11.1 | ProjC 使用独立 projc.* schema，复用 DDL/服务层模式，不加 owner_domain 列 | 建议确认 | ☑ **批准**（2026-11-17 独立专家裁决 V1.0，§10.7） |
| P3-11.2 | PC 与 ProjC 独立对象身份、字典、状态机、确认权威、版本边界 | 建议确认 | ☑ **批准**（同上；五条边界=P3-D04 设计前提） |
| P3-11.3 | Finding 按业务含义分流，不自动成为正式认知 | 建议冻结 | ☑ **批准**（同上） |
| P3-11.4 | 过渡期不改审阅 UX，但不允许错误领域形成新正式权威（有界过渡；停止条件=七条就绪判据+验收） | 建议确认 | ☑ **修改后批准**（④DomainRef 完整 resolver=Partial 归 P3-10；七项不得记全部满足，§10.7） |
| P3-11.5 | delivery_activity 不纳入 ProjC 正式认知对象（无消费者则裁掉） | 建议确认 | ☑ **批准**（同上） |
| P3-05 | Technical Conflict 由 ProjC 自建（DomainRef 引用双方）；pc.conflict=Legacy/Not for New Use（留表/不解冻守卫/不插数/独立迁移清理） | 建议确认 | ☑ **修改后批准**（projc.conflict=IMPLEMENTED 保留；生产登记入口 O-06 归 P3-D04，§10.7） |
| P3-KB-02 | Default Deny + 权限交集；KB Permission/Asset Visibility/Operation Permission 为必要条件；**Project Association 不进授权链**；不建第二套成员体系；保密范围差异用资产级权限或内容拆分 | 建议确认 | ⬜ 待裁决 |
| P3-D02.1 | Extraction 入 KB V1：可追溯过程与关系载体，≠Memory、无确认权、结果必走 Proposal/Review/Revision；**须钉扎来源 Document Revision** | 建议确认 | ⬜ 待裁决 |
| P3-D02.2 | Git 迁移不进 Phase 3 关键路径（≠放弃目标架构，禁 PG/Git 双主） | 建议确认 | ⬜ 待裁决 |
| P3-D02.3 | Apply 是执行动作不映射 DecisionBody：confirm（Accept/Approve/Confirm）→reject/defer/investigate；决策与系统执行分离 | 建议确认 | ⬜ 待裁决 |
| P3-08 | 继承现有矩阵，核验冲突后逐条记录继承/修订/待裁决 | 进入评审 | ⬜ 待开工 |

专家补充的**统一检索三层**（Domain Storage / Domain Reference / Unified Retrieval）与**五条实施闭合条件**（DomainRef 跨 schema 解析、遍历边界、引用完整性、检索保留领域身份与权限过滤、PageIndex 不产生双重正文权威）为 P3-11 裁决稿必备章节；FTS/PageIndex 是检索能力不是知识权威。

**设计组核验补充**：过渡数据性质已实证——`crates/pm-infrastructure/src/kb_pc_analysis.rs:718-797`，现行链发布的即 repository/tech_stack/delivery_activity/module 四类 snapshot 对象；P3-11.4 停止触发条件须与 Phase 2 在途任务包（P2-01~08）协调。术语注记：专家授权链中的「K4」与代码中 K4 CAS 控制点语义需在 P3-D02 权限模型落位时对齐。

### 9.1 效力层级与裁决状态约定（2026-11-17 专家裁决意见 V1.0 · D-1/D-2 采纳）

**裁决状态口径（D-1 = 选项 B「实现先行、裁决后置」）**：P3-11 当前采用「实现先行、裁决后置」方式推进。方向确认、专家评审有条件通过及开发放行，构成当前实施依据，但不构成 R-01~R-10 的正式裁决。正式裁决以逐项裁决记录及对应批准状态为准。R-01~R-10 应在 P3-D03/P3-D04 领域模型正式冻结前完成裁决。本表（§9）状态列保持「待裁决」，不因 P3-11 验收通过而批量勾选；裁决稿 §8 检查单保留原检查项、不预先勾选，注记「当前版本已作为实施依据，但尚未完成正式逐项裁决」，后续由独立裁决会议逐项形成批准/修改后批准/不批准记录。（**2026-11-17 更新**：正式逐项裁决已完成，记录见 §10.7——本段「保持待裁决」口径就此闭合；「放行≠裁决」的效力区分继续有效。）

**效力层级词汇表（Phase 3 统一使用；四层不得互相替代）**：

| 效力层级 | 定义 | 能证明什么 | 不能证明什么 |
|---|---|---|---|
| 方向确认 | 确认方案方向可继续研究和推进 | 方案方向获得认可 | 具体架构条款已正式批准 |
| 专家评审 | 专家对方案进行评估，可能附带条件 | 方案通过评审或具备推进条件 | 专家评审自动替代用户裁决 |
| 开发放行 | 允许开发按指定方案和约束实施 | 当前实施范围和开发依据获得授权 | 架构决策已正式冻结 |
| 正式裁决 | 对具体架构决策逐项作出批准、修改或否决 | 对应决策获得正式批准 | 代码已经实现或验收通过 |

相关第五术语（不入四层效力链）：**工程验收** = 检查已交付实现是否符合已批准或授权的实施范围、质量标准和验收条件。正式裁决与工程验收是两个正交维度。

**正式注记（写入本台账统一口径）**：Phase 3 中「方向确认」「专家评审」「开发放行」「正式裁决」具有不同效力，不得互相替代。开发放行不等于正式裁决；正式裁决状态以对应决策项的正式裁决记录为准。工程验收独立于架构裁决，验收通过不自动产生架构批准或领域模型冻结效力。

**双台账关联约定（专家补充 3.1 采纳）**：后续每个对象/能力保留两个独立状态——Architecture Decision（待裁决/已批准/修改后批准/不批准）与 Implementation & Acceptance（未实现/已实现/已集成/待验收/验收通过），防止注册表的 COMPLETE 被误读为架构决策已完成。

## 10. 执行顺序（专家反馈后更新）

1. 记录专家意见并入台账（已完成，本文档 §9）。
2. **交付《P3-11 仓库分析认知归属与双域治理裁决稿》**（内容清单见专家反馈意见文末），独立 projc.* schema 为默认推荐方案 → 正式裁决评审。**V1.0 已落盘（`Phase3-裁决稿-P3-11仓库分析认知归属与双域治理-v1.0.md`，R-01~R-10 裁决项含默认方案），待正式裁决评审**。
3. 交付《P3-D02 KB 领域模型 V0.3》。
4. 交付《P3-08 生命周期与权威矩阵继承评审》。
5. 三份产物经正式裁决后更新本台账状态列，再冻结 D03/D04。
6. **先不改数据库设计**；专家建议条目在正式裁决前不驱动任何实现分支。

### 10.1 P3-11 裁决稿专家评审结论（2026-11-16）

裁决稿 V1.0 专家评审=**有条件通过**：9 项方向确认，4 项必须修订。V1.1 已落实全部修订并落盘（`Phase3-裁决稿-...-v1.1.md`）：

1. **R-07（P0 修订）**：DomainRef 重构为 `target_domain/target_object_type/target_object_id/revision_context` 四字段，与 `relationship_type.owner_domain`（关系确认权威域，0007 冻结语义）严格分离；禁止把关系所有权与目标对象所有权合并为一个 Domain 字段。
2. **R-04**：停止条件升级为「能力就绪七条判据 + 实施验收」（非代码合入）；明确过渡行为（暂记/close/审核历史继续，snapshot 提案不发布为 PC 正式版本，不为端点行为产生错误领域正式版本）。
3. **R-03**：Product Module（PC）/ Repository Module、Technical Component（ProjC）消歧；D-4 身份综合限定=仅业务语义判断确属产品身份的候选进 PC。
4. **R-06**：跨域冲突归属原则=所有权由主要冲突认知所属领域决定，双方各自履行确认责任；暂不建 Global Conflict。

另收编：R-01 附注（projc.source 不得成为第二 Source 权威）、R-08 附注（检索结果须带原始领域/身份/版本/权限判断元数据）、**两条架构红线**（共享基础设施≠共享业务权威；跨域引用≠跨域授权，进 P3-D05 通用治理规范）、三衔接检查点（注册表/0007 枚举迁移方案/Phase 2 逐项核对——**已核对完成**：P2 零改动红线明令不改 close 事务 → 过渡守卫=Phase 3 独立任务项 T-1，等 P2 落地后实施，E2E-02 close 批发断言同步调整并登记测试漂移）。

专家评审记录全文：`Phase3-P3-11裁决稿专家评审意见-V1.0.md`。**下一步=正式逐项裁决 R-01~R-10（V1.1 为裁决基础稿）。**

### 10.2 P3-11 开发落实状态（2026-11-16，待验收）

用户指示开发落实闭环后，Stage 1 已交付（**待验收，不自行冻结**）：《P3-11 开发落实与差异报告-v1.0.md》。要点：

- **已落实（代码+测试证据）**：R-01 独立 schema（迁移 0025：projc 13 机制表/8 触发器/15 索引/3 字典种子）+ R-02 独立字典/状态机（projc 服务层 object/evidence 写入者）+ R-07 DomainRef 四字段契约（kb-pc-contracts 注释同步）。
- **部分落实（设计冻结、编码待批次）**：R-03 消歧字典已承接（worker 重挂=T-2）、R-04 七条就绪判据仅①满足（T-1 守卫等 P2 落地）、R-05/R-10 registry 附录标记、R-06 pc.conflict=Legacy（projc.conflict 留 D04）。
- **未落（既定后置）**：R-08 统一检索、R-09 分流发布编码（T-1/T-2）。
- 证据：projc_model 3/3 + pc 回归 6/6+8/8 绿（真实 DB 16433）；registry 附录 `object-registry-v1.1-projc-addendum.md`；close/审阅/worker/pc/前端零 diff。
- 待裁决：R 项冻结确认、T-1/T-2 排期、projc REST 批次、存量弃置/重跑。

### 10.3 P3-11 T-1/T-2/T-3 落实状态（2026-11-17，待验收）

放行意见四项闭合已编码落地（两道门槛 G-1/G-2 测试断言先于实现编写；分阶段结构=方案 §8.5）：

- **阶段一（纯增量，master `ea9aafb`）**：迁移 0026（projc.relationship(_type/_revision)/conflict(+object)/端点闭合触发器/`repo_analysis_report.projc_source_id`）+ projc 服务层镜像五文件（proposal/publish/relationship/conflict/mod 补齐）+ 事件目录 16 项（projc.change_proposed / projc.cognition.version_published）+ Platform 留痕 handlers + `projc_formal_chain` 6/6。含 0025 两处缺陷修正（proposal_member 锚定触发器 BEFORE→AFTER 约束触发器、write_operation 账本幂等唯一约束——0001 真源同款）。
- **阶段二（切换批，本批提交）**：W4 worker 全链重挂 projc（delivery_activity 停派、module→repository_module、LLM 归因 derived/llm/derived_fallback 不变）+ G-1 双源登记（mark_analyzed 同事务 pc_source_id+projc_source_id；projc 源缺失硬失败+审计留痕）+ G-2 close 双域分流（pc 分支逐行不动；`projc-decision:{id}`；独立计数；Legacy 显式留痕）+ W6 并行决策端点 `POST /v1/pm/projc/proposals/{id}/decide` + 前端最小渲染（技术认知徽标+ProjC 提案/修订列表）+ W7 迁移 0027（pc.object_type snapshot 四类 deprecated，identity 链 product_* 不受影响）。
- **T-3 证据**：`projc_derive_switch` 2/2（派生域归属+字典守卫）+ `kb_pc_repo_flows` 7/7（worker 全链双域口径改写）+ `analysis_reports` 11/11（含 G-1 双源/G-2 分流/close_compat_old_consumers）+ pc_model 6/6 + pc_proposal 8/8 + projc_model 3/3 + projc_formal_chain 6/6 + pm-application 全量 800 passed 0 failed（阶段一基线）。完整矩阵见《P3-11 T-1/T-2/T-3 交付报告》。
- 登记事项：① 契约核查 C-9/C-10 写「加法 2 键」，G-2 闭合项四要求发布失败独立计数 → 实落 3 键（projc_applied/projc_withdrawn/projc_publish_failed_proposals），向后兼容性不变；② `prior_cognition_digest`/NO_CHANGE result_ref 随全链切换改读 projc（P3 `/v1/pc/analyses/{id}` source 分支对 projc ref 返回任务面无对象展开，ProjC 展示走报告详情 projc_* 字段）；③ 存量测试对齐三处（0023 守卫白名单、DEF-CHAIN-02 NotFound、审阅流 v2 暂记语义 pc_proposals 两用例——均系先前批次变更未同步断言，非本批裁决改动）。
- 待裁决：R 项冻结确认、存量 snapshot 弃置/重跑、P0 E2E 真实跑批（确认点③，不在本批硬门）。

### 10.4 专家裁决意见 V1.0 执行记录（2026-11-17，专家裁决 → 文档级动作已执行）

《P3-11 验收前文档级裁决意见 V1.0》（专家，2026-11-17）四项裁决执行如下；总体结论=**P3-11 可进入验收，但「工程验收通过」与「R-01~R-10 正式架构裁决」为两个独立动作**，验收结论限定为工程交付验收，不得写成双域架构全面冻结。

| 项 | 裁决 | 执行 |
|---|---|---|
| D-1 裁决勾选方式 | **B：实现先行、裁决后置**——R-01~R-10 不随验收自动勾选；须在 P3-D03/P3-D04 领域模型正式冻结前经独立裁决会议逐项完成（批准/修改后批准/不批准） | §9.1 口径已写入；§9 状态列与裁决稿 §8 检查单保持未勾 |
| D-2 效力层级注记 | **修改后采纳**：固化四层效力词汇表 + 工程验收独立第五术语 | §9.1 词汇表与正式注记已写入；后续设计稿/实施报告/验收报告/台账统一使用 |
| D-3 注册表状态 | **接受降级 + 冻结 COMPLETE 判定口径**：SOURCE→INTEGRATED、RELATION/CONFLICT→IMPLEMENTED；判定口径与晋级证据要求冻结 | 注册表升 **V1.3**（状态行已改 + §5 判定口径/晋级规则已写入） |
| D-4 硬化候选 | a/b/c 分层纳入 P3-D04；d 可选清理；**均不提升为 P3-11 验收条件** | 见下表登记 |

**P3-D04 候选登记（D-4 分层）**：

| 项 | 内容 | P3-D04 处理意见 | 附带裁决要求 |
|---|---|---|---|
| a（优先硬化候选） | pc/projc.cognition_object.object_type 对字典无 FK/CHECK，废弃类型直插 SQL 可成功（验收探针 P7 实证；隔离=服务层 fail-closed） | 优先硬化候选 | 研究并裁决：字典维护权、是否 FK、是否 CHECK、类型废弃/迁移/兼容/新增处理、服务层与 DB 层职责分工。**「迁移红利窗口」作为实施条件记录，不作为取消硬化工作的理由**（专家补充 3.3 采纳） |
| b（条件性硬化候选） | PC 发布链读 proposal_member 不复检字典（既有契约；生产 0 存量行） | 风险清单/条件性候选 | 待设计裁决：proposal_member 是否恒经受控入口写入、发布链是否复检类型、是否需要 DB 约束+发布事务双重校验；若受控入口+DB 约束可覆盖完整性，可不重复检查 |
| c（**必须处理的治理问题**） | CognitionPage confirm/reject 旧通道无审计落痕（已登记 P0「确认门缺失」组成） | P3-D04 正式治理闭合要求 | 必须明确：①CognitionPage 正式认知对象是否保留；②confirm/reject 是否继续允许；③保留则是否接入统一正式确认链；④废弃则旧数据/历史操作迁移方案；⑤旧入口是否需立即禁止正式写入。**P3-D04 启动时首先确认旧通道现行生产写入状态及临时控制策略，必要时先限制正式写入再迁移设计**（专家补充 3.2 采纳） |
| d（可选技术债） | locales/pm.yaml 历史文案在发布产物字符串级残留（模块级门禁不受影响） | 不纳入 P3-D04 | 普通技术债，前端/国际化专项清理；不影响验收与 D04 |

### 10.5 P3-11 / P3-D04 边界原则（专家裁决正式采纳，独立约束条目）

**原则一**：P3-11 验收仅证明仓库分析认知归属及 PC/ProjC 双域治理工程实现符合本批验收范围，不代表 Project Cognition 完整领域模型已经设计完成、正式批准或冻结。P3-D04 必须独立完成领域对象、对象类型、生命周期、关系、冲突、确认机制和检索模型设计及裁决。

**原则二**：ProjC 已具备独立工程链，不代表 Project Cognition 的产品领域设计已经完成。对象字典粒度、旧认知载体收口、统一检索及跨域引用消费等问题，应在 P3-D04/P3-10 对应范围内独立设计和裁决，不得以当前工程实现替代领域设计结论。工程对象存在，不等于领域概念、业务边界和产品交互已经完整。

### 10.6 P3-11 工程交付验收记录（2026-11-17，用户裁决）

- **验收裁决：通过**（用户，2026-11-17）。本节更新 §10.2/§10.3 的「待验收」状态。
- **验收范围（依专家裁决意见 V1.0 与验收补充报告 §6 限定）**：工程交付验收 only——证明仓库分析认知归属及 PC/ProjC 双域治理工程实现符合本批验收范围；验收结论不构成双域架构全面冻结。
- **验收依据**：《Phase3-P3-11-T1T2T3-交付报告-v1.0.md》九条完成标准 + 《…验收补充报告-v1.0.md》（§1~§4 全 PASS）+ 三份只读核查报告 + 专家裁决文档校准（`210105b`）。测试基线：pm-application 800/0、pm-api 486/0、pm-infrastructure 143 passed、前端 vitest 38/38 + vue-tsc + 生产构建、迁移实测（0001→0027 全链 + 探针 P1~P7）。
- **效力边界**：本次验收不产生架构批准或领域模型冻结效力（台账 §9.1 词汇表、§10.5 边界原则）。R-01~R-10 正式裁决**保留待办**——须在 P3-D03/P3-D04 领域模型正式冻结前经独立裁决会议逐项完成（D-1=选项 B）。
- **后续动作**：①push 由用户执行；②正式冻结未宣布（须待 R-01~R-10 裁决后由用户单独裁决）；③P3-D04 可启动前期研究/现状分析，启动时先执行 §10.4-c 的旧通道写入状态确认与临时控制策略。

### 10.7 R-01~R-10 正式裁决记录（2026-11-17，独立专家意见 V1.0）——闭合 §10.6 保留的裁决待办

裁决性质：独立架构裁决（非工程验收、非代码审查、不自动执行冻结）。证据边界（专家自述，如实记录）：基于用户提供的裁决会议材料/验收结果/迁移探针/核查报告结论判断，本轮未重新回溯 a781ad4 源码。

**裁决总表（10 项批准/修改后批准，0 项不批准）**：

| R | 结论 | 要点 | 对 P3-D04 |
|---|---|---|---|
| R-01 独立 projc.* schema + projc.source 语义 | **批准** | projc.source 仅本域证据登记，不得扩展为与 kb.source 并列的第二来源资产体系 | 不阻断 |
| R-02 双域权威边界五条 | **批准** | =P3-D04 设计前提；未来须持续检查服务调用/异步任务/统一检索的间接越权 | 前提约束 |
| R-03 分流判据+消歧+D-4 限定 | **批准** | 含「扫描发现首先是证据/候选 Finding」；禁止按名称/目录建自动映射 | 不阻断 |
| R-04 有界过渡 | **修改后批准** | 过渡方案与已落实过渡行为批准；**七项停止条件=部分满足**（④=Partial） | ④为跨域引用能力前置约束 |
| R-05 delivery_activity 移除 | **批准** | 未来如需表达工程活动，先明确业务身份/生命周期/权威归属，不得原形式回流 | 不阻断 |
| R-06 ProjC 自建 conflict；pc.conflict=Legacy | **修改后批准** | 三层区分登记：领域模型已实现 / 生产登记入口未闭合 / 产品入口待定；**IMPLEMENTED 状态保留**，不因缺入口降级，也不提前标 COMPLETE | 纳入领域设计 |
| R-07 DomainRef 四字段契约 | **修改后批准** | 契约成立；引用完整性细则（删除/归档/版本更新/权限变化/废弃重定向）=P3-D04 开放项；**字段名称映射**：抽象契约 `revision_context` ↔ 存储 `target_revision_id`（+`revision_missing_reason`），须统一命名（O-09） | 细则须在领域模型闭合 |
| R-08 统一检索三层+结果元数据 | **批准** | 架构原则批准；实现归 P3-10；防止「无领域归属合并对象/索引当权威/缓存绕权限/摘要当正文/索引态当版本态」 | 不阻断 |
| R-09 close→publish 分流 | **批准** | 长期约束：close 成功≠全域发布成功——调用方/审计/UI 必须能区分 close 成功、PC 发布结果、ProjC 发布结果、失败待人工；不得为单一成功态丢失领域发布失败信息 | 不阻断 |
| R-10 存量=Legacy Transitional | **批准** | 三概念区分：历史记录/正式认知/可迁移数据（后者须经重新验证流程）；未来发现真实历史数据不得直接复制进 ProjC 正式版本 | 不阻断 |
| 红线一/二 | **批准** | 纳入四域通用治理规范，P3-D05 承接；另采纳**跨域权限闭环原则**：跨域操作须同时满足调用方授权+目标领域授权+目标对象当前状态约束，引用关系本身不构成任何一方授权凭证（覆盖读/确认/撤回/发布/历史版本/统一检索摘要与衍生内容） | P3-D05 转化为检查规则 |

**R-04④ 正式登记**：**部分满足（Partial）**——ext 键校验及 SEC-P2-2 足以支撑当前限定范围；完整跨 schema resolver、引用解析及引用权限闭合由 **P3-10 承接（O-01）**；该能力完成前不得宣称跨域引用完整性全面闭合，不得在台账写「七项全部就绪、过渡正式结束」。登记状态：有界过渡方案=已批准；当前过渡行为=已落实；过渡停止条件=部分满足。

**未决开放项清单（不因裁决通过而消失）**：

| 编号 | 开放项 | 责任 | 处理要求 |
|---|---|---|---|
| O-01 | 完整跨 schema DomainRef resolver | P3-10 | 闭合 R-04④ |
| O-02 | 引用删除/归档/版本更新/权限变化语义 | P3-D04 | 闭合 R-07（与 O-01 不得混同：O-01=工程能力，O-02=领域语义与生命周期规则） |
| O-03 | object_type 数据库约束与字典治理 | P3-D04 | 决定 FK/CHECK 或其他可靠约束（承接台账 §10.4-a） |
| O-04 | proposal_member 发布时字典复检 | P3-D04 | 按可信写入边界决定是否纵深校验（承接 §10.4-b） |
| O-05 | CognitionPage confirm/reject 旧通道治理 | P3-D04 | 禁用/迁移/接入正式确认链（承接 §10.4-c 必须治理项） |
| O-06 | ProjC Conflict 生产登记入口 | P3-D04 | 明确业务场景与产品入口 |
| O-07 | 统一检索架构及权限元数据 | P3-10 | 按 R-08 落地 |
| O-08 | 注册表 COMPLETE 及状态晋级口径 | 注册表 V1.3 | 维持 Decision 与 Implementation/Acceptance 双维度 |
| O-09 | revision_context 与存储字段映射统一命名 | P3-D04/契约文档 | 统一契约名称与字段语义 |

**对 P3-D04 启动**：可以正式启动领域模型设计（启动≠直接冻结完整领域模型）。建议展开面：对象字典粒度、正式认知载体收口、生命周期、关系模型、冲突模型、来源与证据职责边界、统一检索衔接、确认与审计。

**P3-11 正式冻结**：专家**支持有边界的条件冻结**（不支持无条件全面冻结）。冻结令由用户另行下达，建议表述（已录备用）：

> P3-11 R-01~R-10 及双域治理红线已完成正式架构裁决。冻结本次批准的架构边界、分流原则、过渡规则和跨域治理约束。R-04④完整 resolver、R-07 引用完整性细则及其他登记开放项继续由 P3-10/P3-D04 承接，不因本次冻结视为完成。后续设计和实施不得擅自改变已批准的架构原则；如确需调整，必须重新提交架构裁决。

冻结令可冻结：双域权威边界、独立 projc.* schema 方向、分流原则、有界过渡原则、Conflict 领域归属原则、DomainRef 四字段契约、统一检索三层原则、close→publish 分流原则、Legacy Transitional 原则、两条跨域治理红线。**不得写成已完成**：完整跨 schema resolver、引用完整性全部语义、ProjC 完整领域模型、统一检索实现、CognitionPage 旧载体最终收口、Conflict 产品登记入口。
