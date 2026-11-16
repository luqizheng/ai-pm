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
| P3-11.1 | ProjC 使用独立 projc.* schema，复用 DDL/服务层模式，不加 owner_domain 列 | 建议确认 | ⬜ 待裁决（方向经专家评审确认，见 §10.1） |
| P3-11.2 | PC 与 ProjC 独立对象身份、字典、状态机、确认权威、版本边界 | 建议确认 | ⬜ 待裁决（同上） |
| P3-11.3 | Finding 按业务含义分流，不自动成为正式认知 | 建议冻结 | ⬜ 待裁决（V1.1 补模块消歧+D-4 限定） |
| P3-11.4 | 过渡期不改审阅 UX，但不允许错误领域形成新正式权威（有界过渡；停止条件=七条就绪判据+验收） | 建议确认 | ⬜ 待裁决（V1.1 升级停止条件） |
| P3-11.5 | delivery_activity 不纳入 ProjC 正式认知对象（无消费者则裁掉） | 建议确认 | ⬜ 待裁决 |
| P3-05 | Technical Conflict 由 ProjC 自建（DomainRef 引用双方）；pc.conflict=Legacy/Not for New Use（留表/不解冻守卫/不插数/独立迁移清理） | 建议确认 | ⬜ 待裁决（V1.1 补跨域冲突归属原则） |
| P3-KB-02 | Default Deny + 权限交集；KB Permission/Asset Visibility/Operation Permission 为必要条件；**Project Association 不进授权链**；不建第二套成员体系；保密范围差异用资产级权限或内容拆分 | 建议确认 | ⬜ 待裁决 |
| P3-D02.1 | Extraction 入 KB V1：可追溯过程与关系载体，≠Memory、无确认权、结果必走 Proposal/Review/Revision；**须钉扎来源 Document Revision** | 建议确认 | ⬜ 待裁决 |
| P3-D02.2 | Git 迁移不进 Phase 3 关键路径（≠放弃目标架构，禁 PG/Git 双主） | 建议确认 | ⬜ 待裁决 |
| P3-D02.3 | Apply 是执行动作不映射 DecisionBody：confirm（Accept/Approve/Confirm）→reject/defer/investigate；决策与系统执行分离 | 建议确认 | ⬜ 待裁决 |
| P3-08 | 继承现有矩阵，核验冲突后逐条记录继承/修订/待裁决 | 进入评审 | ⬜ 待开工 |

专家补充的**统一检索三层**（Domain Storage / Domain Reference / Unified Retrieval）与**五条实施闭合条件**（DomainRef 跨 schema 解析、遍历边界、引用完整性、检索保留领域身份与权限过滤、PageIndex 不产生双重正文权威）为 P3-11 裁决稿必备章节；FTS/PageIndex 是检索能力不是知识权威。

**设计组核验补充**：过渡数据性质已实证——`crates/pm-infrastructure/src/kb_pc_analysis.rs:718-797`，现行链发布的即 repository/tech_stack/delivery_activity/module 四类 snapshot 对象；P3-11.4 停止触发条件须与 Phase 2 在途任务包（P2-01~08）协调。术语注记：专家授权链中的「K4」与代码中 K4 CAS 控制点语义需在 P3-D02 权限模型落位时对齐。

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
