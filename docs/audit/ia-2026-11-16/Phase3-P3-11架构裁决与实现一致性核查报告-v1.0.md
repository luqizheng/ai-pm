# P3-11 架构裁决与实现一致性核查报告 V1.0

- 日期：2026-11-17 · 性质：**只读架构审查**（不修改代码/迁移/注册表/裁决文件；不 push、不冻结、不宣布验收）
- 基准：①台账 V1.0 ②裁决稿 V1.1 ③交付报告 V1.0 ④注册表 V1.1 附录 V1.2 ⑤master `9a38935`（阶段二工作树）
- 关联：前序核查《Phase3-四域对象注册表V1.2专项架构核查报告-v1.0.md》（REG-01~04）与《Phase3-P3-11-T1T2T3-验收补充报告-v1.0.md》——本报告不重复其证据细节，引用并补充裁决维度
- 总览：**二/三/四 = PASS；一 = NEEDS_DECISION（文档级，非代码级）；五 = 三维分离呈现（2 项措辞偏差待裁决）**

---

## 一、正式裁决状态核查 — **NEEDS_DECISION（唯一一项；缺的是"正式批准记录"，不是实现）

**事实链（四个不同效力层级，不混同）：**

| 层级 | 载体 | 状态 |
|---|---|---|
| 方向确认 | 台账 §3.1（评审者立场：方案③为正式裁决讨论起点，附五条边界） | ✅ 已记录 |
| 专家评审 | 台账 §10.1：裁决稿 V1.0 评审=**有条件通过**（9 项方向确认、4 项必须修订）；V1.1 已落实全部修订 | ✅ 已记录 |
| 开发放行 | 方案 V1.1 头注「放行记录：close 契约核查通过，允许实施…」+ 用户范围确认四点（①字典废弃②P5 并行端点③T-3 口径④关系种子） | ✅ 已记录（属**实施放行**，非正式裁决） |
| **正式逐项裁决** | 台账 §9 正式裁决状态列：P3-11.1~.5/P3-05 全部「**⬜ 待裁决**」；裁决稿 §8 检查单 R-01~R-10+红线全部「**[ ] 未勾选**」；全 docs 检索无任何后续勾选/批准记录 | ❌ **缺失** |

**逐项核对表（裁决内容 → 专家修订 → 实现 → 注册表；批准列见上）：**

| R | 裁决内容（裁决稿 V1.1） | 专家修订 | 实现（代码/迁移/测试） | 注册表 V1.2 |
|---|---|---|---|---|
| R-01 | 独立 projc.* schema（附 projc.source 语义约束） | 确认（收编附注） | 0025+0026；projc 服务层五文件；`projc.source` 仅本域证据登记（kb_pc_analysis.rs 注释+实现） | §1 一致 |
| R-02 | 双域权威边界五条 | 确认，建议冻结 | 本报告 §二 全项 PASS | §1 一致 |
| R-03 | 分流判据+模块消歧+D-4 限定 | V1.1 补 | worker 重挂（kb_pc_analysis.rs plan/derive）、repository_module 命名、identity.rs 零改动；kb_pc_repo_flows 断言 | §1/§2 一致 |
| R-04 | 有界过渡（七条就绪判据+过渡行为） | V1.1 升级停止条件 | 判据①②③⑤⑥⑦由 Stage1+T-1/T-2/T-3 兑现；④DomainRef 解析/引用权限=部分（ext 键校验+SEC-P2-2，完整 resolver 归 P3-10）；T-1 过渡守卫由「字典废弃+重挂+分流断言」替代（交付报告 §6 登记） | §2 一致 |
| R-05 | delivery_activity 移除 | 确认 | plan_cognition_objects 停派（kb_pc_analysis.rs 测试断言）；0027 字典废弃 | §2 一致 |
| R-06 | ProjC 自建 conflict；pc.conflict=Legacy | V1.1 补归属原则 | 0026 conflict(+object)；服务层 create/resolve；pc.conflict 生产零写入者（grep 实证） | §1/§2 一致 |
| R-07 | DomainRef 四字段契约，与 owner_domain 分离 | V1.1 P0 修订 | kb-pc-contracts domain_ref/registry；0026 relationship_type owner_domain CHECK IN ('project_cognition')（探针 P1） | §3 一致 |
| R-08 | 统一检索三层+结果元数据 | V1.1 收编 | **未实现**（P3-10 前置；方案 §5 明确不做清单） | §4 一致（KB/Retrieval 无变化） |
| R-09 | close→publish 分流落点+过渡行为 | 确认（V1.1 明确过渡行为） | close_report 双域分流（e9f66ec）；C-4 pc 分支逐行不动 | §1 VERSION 行一致 |
| R-10 | 存量=Legacy Transitional | 确认 | 无存量迁移（AGENTS 规则 1）；pc snapshot 字典 0027 deprecated | §2 一致 |

**NEEDS_DECISION 的准确含义**：R-01~R-10 的**方向与内容**经方向确认+专家评审+开发放行三重背书，实现与注册表未发现突破裁决的偏差（§二~§四 全 PASS）；缺的是台账 §9 / 裁决稿 §8 的**正式逐项批准勾选记录**。两个处置选项（互斥，由用户裁决）：
- 选项 A：随 P3-11 验收动作，正式勾选 R-01~R-10（台账 §9 状态列 + 裁决稿 §8 检查单），裁决与验收一次闭环；
- 选项 B：维持「待裁决」直到 P3-D03/D04 前的独立裁决会，验收报告如实记载「实现先行、裁决勾选后置」。

**最小修正建议**：无论 A/B，台账 §9 表头注记「正式裁决状态以勾选记录为准，开发放行不构成正式裁决」一句，防止后续文档再把放行误当裁决。**是否影响 P3-11 验收**：不影响代码/交付验收（实现面全 PASS）；建议把裁决勾选纳入验收仪式。**是否影响 P3-D04 启动**：不影响——P3-D04 依赖的是边界冻结（五条），其内容已被实现与测试锁定；但 D03/D04 正式冻结前补齐勾选更稳。

## 二、领域权威核查 — **PASS**

| 核查点 | 证据 | 结论 |
|---|---|---|
| PC 与 ProjC 独立对象身份 | 独立 schema（pc.* / projc.*）；独立字典互不可见（projc_model 测试：pc 类型 `product_profile` 在 projc 字典 Invalid；0027 后反向 snapshot 类型在 pc 字典 Invalid）；object_key/proposal/source id 命名空间互不交叉（无共享序列/共享表） | ✅ |
| 跨域直接写入 | 写入矩阵 grep（生产代码）：projc 服务层→pc.* 表 **0 命中**；pc 服务层+identity.rs→projc.* **0 命中**；worker（kb_pc_analysis）→ai.* **0 命中**（无双写）；唯一跨域接触=implements 关系 ext 键引用（读语义） | ✅ |
| 同一正式认知的双重正文权威 | pc.document_revision 与 projc.document_revision 分 schema 分表；版本成员钉扎各自域内修订；无共享正文表、无复制路径（grep 无 `INSERT INTO projc.* SELECT ... FROM pc.*` 形态） | ✅ |
| DomainRef 仅引用不授权 | DomainRef 类型无 service 字段（kb-pc-contracts 红线+payload 校验器）；implements 目标端=ext 四元组 `pc.feature_module`（0007 形态），ProjC 不因引用获得 pc 写权——projc 服务层无任何 pc.* 写入点（上行矩阵）；registry 通道 ProjectCognition=ReadOnly | ✅（红线二成立） |
| PC identity 链保持原职责 | identity.rs 在 P3-11 四个提交中**零改动**（git log 末次=91003f6，早于本批）；仍经 pc 服务层起草 product_profile/product_vision 提案并挂报告 pc_source_id；0027 废弃 snapshot 四类不影响 product_* | ✅ |
| **CognitionPage 与 projc.\* 实际读写关系** | `apps/pm-api/src/pm_cognition.rs` 对 "projc" **0 引用**（grep count=0）：CognitionPage 后端路由（overview/knowledge/runs/decision/identity-synthesis）只触 ai.* 与 pc 正式提案链；**projc.* 对 CognitionPage 完全不可见**——读不到也写不进；projc 的展示面=分析报告详情 projc_* 字段（W6） | ✅ 新旧完全隔离，无交叉 |

## 三、正文及版本权威核查 — **PASS**

| 维度 | 现状（实现） | 权威表述（不得混淆的两条） |
|---|---|---|
| 正文存储位置 | `projc.document_revision.content`（0025:98-110，不可变触发器），经 cognition_revision.document_revision_id 到达；cognition_object **无 content 列** | 存储是事实描述，不构成权威定义 |
| 正文权威 | **权威=hash 锚定的修订链**（content_hash=sha256(markdown)，修订锚列 once-written）；PG 是当前**载体**（0025 头注「已知偏差」） | ❌不把「PG 存了正文」升格为「PG 是知识权威」——注册表 V1.2「正文权威：PG content」一列沿用裁决稿 §1 冻结表述，建议加「（载体）」半句防误读（前序报告 M-建议，未动） |
| Revision 身份 | `cognition_revision.revision_no`（对象行锁推号）+ content_hash；行不可变锚=document_revision | ✅ |
| Version 身份 | `cognition_version.version_no + predecessor_version_id + snapshot_hash`（成员集规范哈希，发布时先算后插）+ source_proposal_id；锚列不可变（forbid_version_mutation） | ✅ 版本身份≠正文 hash，二者分层 |
| 当前状态权威 | cognition_object.lifecycle_status（终态守卫触发器）+ current_revision_id 指针（行锁 CAS 回填） | ✅ 指针可变、锚不可变 |
| Git 现状与目标态 | **现状=PG 载体**（PCGIT 后置，write_operation 仅发布幂等用，git_commit_sha 恒 NULL）；**目标态=Git 权威**（P3-D02.2 冻结裁决：不进 Phase 3 关键路径；禁 PG/Git 双主） | ❌不把目标态写成已实现——0025 头注与注册表注均如实标「目标态/已认账偏差」 |

## 四、遗留对象核查 — **PASS**

| 对象 | 读取 | 写入 | AI 消费 | 审计 | 正式发布边界 |
|---|---|---|---|---|---|
| ai.cognition_item | CognitionPage overview（GET /v1/pm/.../cognition）+ AI 上下文注入（ai_context_items.rs SELECT）+ viz live_graph/resolver（读） | 生产唯一=repo_cognition.rs:174（start_run 派生草稿）+ decide_item 状态翻转（pm_cognition.rs:282+） | 只读消费（上下文注入） | **confirm/reject 无审计落痕**（grep：pm_cognition/repo_cognition 无 audit INSERT）——已登记 P0「确认门缺失」的组成部分，P3-D04 收口 | **不产生任何域正式版本**：decide_item 仅翻 ai.cognition_item.state，不触 pc.*/projc.*（grep 实证）；CognitionPage 后端对 projc 0 引用 |
| ai.project_knowledge | knowledge 路由 active_knowledge + AI 上下文 SELECT | 生产唯一=repo_cognition.rs:265 | 同上 | 同上（旧通道写面无审计） | 同上（展示层资产，非任一域正式权威） |
| pc.conflict | L3 冻结读取（pc proposal/publish 三处，契约 C-4 保留） | **生产零写入者**（grep `INSERT INTO pc.conflict` 仅测试夹具） | 无 | — | 不可能产生新正式认知（无写者+Legacy/Not for New Use） |
| pc.object_type snapshot 四类 | load_object_type 仅 active → 服务层拒绝（fail-closed，测试 snapshot_dictionary_deprecated_fails_closed） | 生产零写入者（worker 已重挂 projc；直插 SQL 可成功=P7 实证，DB 层无 FK，V-6 服务层纪律兜底） | 无 | — | 新起草不可达；发布链不复检字典（既有契约），0 存量行下无实害（P3-D04 硬化候选） |
| **历史 snapshot 版本** | — | — | — | — | **不存在**：pc.* 核心表生产 0 行（注册表 V1.0 迁移红利窗口实证）；0025 projc 空库建 schema（AGENTS 规则 1：无迁移/无回填）；全仓无 pc→projc 复制路径（grep 实证）；projc 无 FTS（P3-10）。**「历史 snapshot 被误认为当前正式 ProjC 认知」不可能发生**：既无数据、无复制通道、无检索面交叉 |

## 五、注册表状态核查 — 三维分离

| 对象 | ①Architecture Decision | ②Implementation | ③Acceptance |
|---|---|---|---|
| OBJECT / REVISION | 方向确认+专家确认+放行；**正式裁决勾选缺失**（§一 NEEDS_DECISION，R-01/R-02） | **COMPLETE**（REG-04） | NOT_ACCEPTED |
| VERSION / PROPOSAL | 同上（R-01/R-02/R-09） | **COMPLETE** | NOT_ACCEPTED |
| **SOURCE** | 同上（R-01 附注语义约束） | **INTEGRATED**（生产调用链在；行级幂等=调用约定，无自然键去重——注册表 V1.2 标 COMPLETE 偏高，待裁决修正） | NOT_ACCEPTED |
| **EVIDENCE** | 同上 | **COMPLETE**（uq_projc_evidence_identity 行级幂等在）——与 SOURCE 分列，二者实现度不同 | NOT_ACCEPTED |
| RELATION | 同上（R-07） | **IMPLEMENTED**（无生产调用方；发布链消费面就绪）——注册表 V1.2 标 COMPLETE 偏高，待裁决修正 | NOT_ACCEPTED |
| CONFLICT | 同上（R-06/P3-05） | **IMPLEMENTED**（L3 消费面集成；登记/处置无生产入口）——同上待修正 | NOT_ACCEPTED |
| R-08 统一检索 | 方向确认 | **未实现**（P3-10；方案 §5 明确不在本批） | NOT_ACCEPTED（不阻塞 P3-11） |

三维度无一处混用：Decision 层唯一的缺口=正式勾选记录（文档级）；Implementation 层两处措辞偏高（SOURCE/RELATION/CONFLICT——即前序核查报告 M-1~M-3）；Acceptance 层全部 NOT_ACCEPTED（与「待验收」现状一致，无 BLOCKED）。

## 六、结论汇总

| # | 项 | 状态 | 影响验收 | 影响 P3-D04 启动 |
|---|---|---|---|---|
| 1 | R-01~R-10 正式裁决记录 | **NEEDS_DECISION** | 不影响（建议纳入验收仪式勾选） | 不影响（边界五条已被实现+测试锁定） |
| 2 | 领域权威（身份/写入/双权威/DomainRef/identity 链/CognitionPage 隔离） | **PASS** | — | — |
| 3 | 正文及版本权威（六维+两条禁混淆） | **PASS** | — | — |
| 4 | 遗留对象（含历史 snapshot 版本不存在实证） | **PASS** | — | — |
| 5 | 注册表三维分离 | **PASS（2 处措辞待修正）** | 不影响（描述性偏差） | 不影响 |
| 6 | FAIL / BLOCKED | **无** | — | — |

**最小修正建议（均未执行，交用户裁决）**：
1. 验收时勾选 R-01~R-10（台账 §9+裁决稿 §8）——或在台账登记「实现先行、裁决后置」；
2. 台账 §9 加注「开发放行 ≠ 正式裁决」；
3. 注册表 V1.2 SOURCE/RELATION/CONFLICT 状态行按 INTEGRATED/IMPLEMENTED 降级（=前序报告 M-1~M-3）；
4. P3-D04 候选硬化项照登（DB 层 FK/CHECK、发布链字典复检、旧通道审计补痕、mock 文案清理）。

**边界原则（响应审查意见，记入本报告作为后续 Phase 3 约定）**：
- **P3-11 验收通过 ≠ P3-D04 领域模型冻结**——验收只证明工程链与既有裁决一致；
- **ProjC 工程链完整 ≠ Project Cognition 产品领域设计完成**——字典粒度（repository_module/technical component 拆分等遗留开放项）、旧载体收口、统一检索均归 P3-D04/P3-10。

**状态声明**：本报告只读；未修改代码/迁移/注册表/裁决；未 push（origin/master 止于 65ef3af）；未冻结；未宣布验收通过。
