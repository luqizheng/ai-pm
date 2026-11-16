# Phase 3 三域产品设计基线 V0.1 — 评审意见 V1.0

- 日期：2026-11-16 · 只读源码核验 · 基准：master `d3b30739`
- 评审对象：`Phase3-三域产品设计基线-V0.1.md`（同目录）
- 证据基准：`docs/architecture/four-domain/object-registry-v1.md`（审计基准 05ce33d8）、KB-01/02/03 补审三分册、`migrations/0001_baseline.sql`、现行路由与视图源码
- 结论速览：**READY WITH CONDITIONS** —— 可作为 P3-D01 设计基线记录；但存在 **1 处与现行实现的落点冲突（必须升裁决）+ 2 处与冻结导航 IA 的出入 + 4 处对象模型与注册表缺口**，逐项见下。

---

## A. 与已冻结裁决的一致性核对

| # | 基线主张 | 冻结依据 | 判定 |
|---|---|---|---|
| A-1 | 四域独立、Analysis 不并入三域设计 | 四域对象模型 V1.0、三域裁决 Q1~Q12 | ✅ 一致 |
| A-2 | AI 分析完成 ≠ 正式认知成立；Finding 不直接变正式认知 | D01、审阅流 v2（决定暂记 + close 发布） | ✅ 一致，且现有代码已按此实现（`close_report` 仅发布 approved 提案） |
| A-3 | PC V1 项目级范围、中期 Product 演进 | 已冻结范围约束 | ✅ 一致 |
| A-4 | 审核中心只聚合不接管审批权威 | D-6 统一人工待办中心 | ✅ 一致 |
| A-5 | Accept/Approve/Confirm/Apply 四动作分离 | DecisionBody action∈confirm\|reject\|defer\|investigate | ✅ 方向一致；V1 落地时四动作须映射到既有枚举，不得新造平行动词 |
| A-6 | 図7 全局导航含 Analysis / Knowledge Base / Product Cognition | **IA V1.2 冻结：四域入口在项目区（P-03/P-09），全局区无四域入口** | ⚠️ 冲突，见 C-2 |
| A-7 | KB IA（図2）六页签，未提 Reports | **Phase 2 裁决 D-1：知识页默认页签=kb，reports 为页签之一** | ⚠️ 缺去向声明，见 C-3 |

## B. 源码现状映射（基线主张 vs 实现）

### B-1 KB 域

| 基线对象 | 现状（注册表+迁移实证） | 缺口 |
|---|---|---|
| Source | `kb.source`（BL:2982）+ `kb.source_project` 多对多挂项目，COMPLETE | 无 |
| Document | `kb.document`（BL:2860），身份层无正文 | **与 Source 无引用列**（注册表 §5.2-3） |
| Revision | `kb.document_revision`（BL:2878+0006），confirm_revision 三重门，COMPLETE | 正文权威=PG content 列（已知偏差项，KB-02：迁 Git 有 0023 不可变触发器硬阻碍） |
| Memory | `kb.memory` + `kb.memory_revision`（BL:2901/2936），全库最严审核门 | **NO_PROD_DATA（0 行）**；生成侧缺位：文档发布→记忆形成链路整体断裂（KB-01 结论），KB-EXTRACTION=STUB |
| Proposal | **无 KB-PROPOSAL 注册对象** | 提案语义双轨：`kb.memory_revision` PENDING_REVIEW（memory 侧）+ 旧栈 `ai.document_proposal` 8 态（document 侧，KEEP） |
| Review | 非对象，是机制（memory::review 唯一转换点 + governance.review_approval） | 与基线「Review 是对象」的表述需对齐 |

### B-2 PC 域

| 基线对象 | 现状 | 缺口 |
|---|---|---|
| 8 类认知对象 | `pc.object_type` 字典已预置 14 类产品类型（product_profile/vision、business_domain/capability、feature_module/feature、actor、scenario、business_process、page、interaction、domain_object、business_rule、product_decision），五层 L1_identity~L5_decision | 基线 8 类 vs 字典 14 类的粒度差异需在 P3-03 对齐（page/interaction/product_decision 是否入 V1） |
| 关系模型 | `pc.relationship(_revision/_type)` 表齐备 | **STUB：字典 0 行，create_relationship_draft 无生产调用者** |
| 前置设计文档 | `docs/architecture/product-cognition/pc-phase1-domain-model-baseline-v1.0.md`、`pc-01b-data-model-and-database-design-v1.0.md`、`docs/product-cognition-knowledge-base/product-cognition-detailed-requirements-v1.1/` | **WP-02.2 必须声明继承或取代关系**，不得 silently 另起炉灶 |

### B-3 Project Cognition 域

| 基线对象 | 现状 | 缺口 |
|---|---|---|
| 页面 | `/project/cognition` 真页已在导航树（navigation.ts:321-325，CognitionPage） | 无 |
| 正式认知 | `ai.project_knowledge`（BL:1101）insert-only | **确认端点无 require_human/可信上下文门（注册表 P0）**；version/supersedes 死列 |
| 提案 | `ai.cognition_item`（BL:770）候选 + 6 态中 4 转移 | superseded 态不可达；evidence 仅 repository 级 |
| 9 类对象分类 | PROJ-COG-REVISION/EVIDENCE/VERSION/RELATION 全 STUB；冲突对象 `pc.conflict` 在 **PC schema** 且无生产写入者 | 基线 9 类中 Conflict、Revision、Confirmation 落点须在 P3-05 裁决（见 C-5） |

## C. 必须裁决的分歧（建议新增 P3-11，P0）

### C-1 仓库分析正式认知落点：现行代码在 PC 域，基线図5 划给 Project Cognition 【最高优先】

- 实证：`crates/pm-application/src/analysis_report.rs` `close_report`——报告关闭时 approved 提案批量 `publish_version` → **`pc.cognition_version`（PC 域发布事务）**；AN-REPORT 挂 `pc_source_id`，worker 经 `pc.evidence`/`pc.source` 落证据。
- 且 `pc.object_type` 的 snapshot 层已预置 **repository / module / tech_stack / delivery_activity** 四类仓库快照对象——pc.* schema 当前同时承载产品认知（14 类产品类型）与仓库技术对象（4 类快照类型），字典层已双职责。
- 基线図5/§5.1 规定技术认知归 Project Cognition（`ai.project_knowledge` 域），与现行链路直接冲突。
- **须裁决**：① 承认现状（pc.* 即技术认知落点，Phase 3 改口）→ 违反 G-01 与四域注册表；② 仓库分析产物分流的类型规则（身份综合→PC，架构/约定→ProjC）→ 需要定义分流判据与迁移序；③ Project Cognition 沿用 pc.* 基础设施（表/发布事务/字典机制）但独立字典与 owner_domain。**建议③为讨论起点，与「不并知识对象、禁双主」边界兼容。**

### C-2 図7 全局导航与 IA V1.2 冻结项冲突

- 实证：全局区现仅 Personal/Dashboard/Projects/Team 设置（navigation.ts 全局段）；四域入口全在项目区 modulesGroup：`/project/analysis`（PLANNED）、`/project/knowledge`（KB 最终定位）、`/project/product-cognition`（PLANNED）、`/project/cognition`（真页）（navigation.ts:294-325）。
- 図7 把 Analysis/KB/PC 提为全局入口。虽自注「待评审映射」，但需显式裁决：**全局域入口是 Phase 3 新诉求还是笔误**。若为诉求，须处理：全局 PC 入口与「PC V1 项目级范围」的落地页语义（无项目上下文时展示什么）、全局 KB 入口与 Team 级文档的关系。冻结项 P-03/P-09 只能在 Phase 3 裁决中修订，不得默认覆盖。

### C-3 KB IA（図2）未声明 Reports 页签去向

- 实证：`KnowledgeView.vue:161-173` 三页签 reports/kb/pc；默认页签现值 `reports`（:46），D-1 冻结目标=kb（P2 任务包落地项）。
- 図2 六页签无 Reports、无 PC 页签。需声明：分析报告视图迁往 `/project/analysis`（Phase 2 已定 PLANNED 占位→Phase 4 页面）、pc 页签迁往 `/project/product-cognition`，并给出三合一页面拆分序（与 navigation.ts:234「迁移期三合一，不伪装已拆分」衔接）。

### C-4 Memory↔Document 关联与现状 schema 不符（P3-01 输入）

- 实证：`kb.memory_ref` DomainRef 四元组**限 KB 域内 target_type=source**（注册表 KB-REFERENCE）；基线§12 主张「Memory 可以关联一个或多个 Document」。
- 结论：P3-01 若采纳该主张，需扩展 memory_ref target 支持 document（或引入 KB-EXTRACTION 概念对象承载提取关系，注册表基线 §6.1 本要求多对多）。

### C-5 Technical Conflict 的 schema 归属

- `pc.conflict/conflict_object` 在 PC schema（BL:3108/3134）且无生产写入者（P6 读端点 501）；基线把 Conflict 划归 Project Cognition。P3-05 须裁决：沿用 pc.conflict（改语义）或 Project Cognition 自建。

### C-6 PC 前置设计文档未纳入基线引用

- `pc-phase1-domain-model-baseline-v1.0`（2026-11-06）与 `product-cognition-detailed-requirements-v1.1` 已存在。WP-02.2 开工前必须声明对它们的继承/修订/取代关系，否则出现双权威文档，违反单一真源。

## D. P3-01 正式评审意见（Document vs Memory）

**结论：初步方案成立，附 4 条条件后可作为 KB 详细设计（P3-D02）的起点。**

与实证一致性：方案与注册表对象语义、KB-01 审计结论（断裂在提取环节而非对象模型）、kb.memory_revision 全库最严审核门（require_human + K4 CAS + 行锁 + 触发器白名单）全部兼容——「Memory 不成为第二套知识正文」正是现行 schema 的既成事实（Memory 正文在 Revision，审核转换唯一写点）。

条件：

1. **Memory 一切正文变更必须走 Revision + 审核门**，禁止任何直写通道（现状已是唯一写点，写进规则即可）；Accept ≠ Confirm 在 KB 域的映射=接受提案≠发布 revision。
2. **补 KB-EXTRACTION 概念对象**（现 STUB）：Document→Memory 的提取关系需要可追溯载体，否则「Memory 关联 Document」无落点（memory_ref 现限 source，见 C-4）。
3. **Memory 生成侧入 V1 范围**：现状全库 0 行、无任何自动提案入口（KB-01：无 worker/MCP/AI 提案消费者）——若 P3-D02 不含生成侧，Memory 只是空页面。
4. **正文权威口径**：Memory 走 PG+Revision（现状），不承诺 Git；Document revision 的 Git 权威属目标态（KB-02 已认账偏差+两处硬阻碍），P3-D02 不得把「Git 权威」写成现约束。

## E. 其他意见

- P3 编号追加 **P3-11：仓库分析正式认知的领域归属与 pc.* 双职责治理（P0）**；C-2 导航出入若确认为诉求则并 P3-09。
- P3-08（Revision/Confirmation/Audit 边界）不必从零设计：`lifecycle-matrix-v1.md`、`authority-matrix-v1.md` 已有矩阵底稿，直接继承评审。
- 每周进度纪要：按「先记录后评审」惯例在本文档序列中追加 `Phase3-周报-WNN-v1.0.md` 即可，无需新机制。
- AGENTS.md 纪律沿用：发布态禁止 mock、助手隐藏路径只改 navigation.ts 清单、四处同步收列（Phase 3 新页面全部适用）。

## F. 结论

| 项 | 判定 |
|---|---|
| P3-D01 总体架构与领域边界 | ✅ 可记录为 V0.1 基线（本文档序列） |
| 冻结决策冲突 | C-2/C-3 两处，须显式裁决后方可进入详细设计 |
| 实现落点冲突 | C-1 一处，**建议列为 P3-11（P0），先于 P3-D03/D04 开工裁决** |
| P3-01 | ✅ 附 D 节 4 条条件，可进入 KB 详细设计 |
