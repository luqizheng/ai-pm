# P4-00 — Current Architecture & Ownership Audit V1.0

> **阶段：** Phase 4 · **切片：** P4-00 · **性质：** 只读架构审计（Read-only Architecture Audit）
> **状态：** Draft — 待人工确认 · **审计执行日：** 2026-11-16 ~ 2026-11-17
> **本报告取代** 上一轮草稿 `docs/architecture/phase4/P4-00-现状审计报告-v1.0.md`（该草稿仅覆盖本任务书约七成核查面，保留作过程档案）。
> **写操作声明：** 本审计唯一写操作 = 创建本报告文件（任务书允许的审计结果输出）。未触碰任何代码 / Migration / 配置 / 正式设计文档 / 数据库。

---

## 1. Executive Summary

**一句话结论：系统是「Project 中心的单层资产模型」；Product 作为业务主体完全不存在（NOT IMPLEMENTED）；Product Plan 同样不存在；KB 的 Team 级所有权裁决（P3-KB-01）已确认但未落地。Phase 4 是从零新建 Product 域，而非改造已有实现。**

任务书 §一 十个审计问题的直接回答：

| # | 问题 | 答案 | 关键证据 |
|---|---|---|---|
| 1 | 是否存在独立 Product 领域实体？ | **否，NOT IMPLEMENTED**（无表/无服务/无路由/无 UI） | §3 实体清单；registry.rs:48-57 仅协议枚举 |
| 2 | Product/Team/Project 实际数据关系？ | Team=core.tenant（无 team 表）；project FK tenant；**Product 不存在，无任何关系可言** | 0001:1300/1251 |
| 3 | KB Source/Document/Memory 如何归属？ | Source=**tenant 级**（已符合 P3-KB-01）；Document/Memory=**project 行级强绑（NOT NULL）**（不符合 P3-KB-01，已裁未落地） | 0001:2982 vs 0001:2863/2904 |
| 4 | Product Cognition 如何识别 Product？ | **每项目一份 `product_profile`/`product_vision` 认知单例**（项目创建时自动建骨架）；无跨项目产品身份 | bootstrap.rs:325；0001:6854 部分唯一索引 |
| 5 | Project Cognition 如何识别 Project？ | 新域 projc.*：列位 tenant+project（写入未接线，Stage 1）；旧链 ai.cognition_run/item：project 强绑 | 0025:71；0001:796 |
| 6 | Requirement/Design/ADR 与 Product/Project 关系？ | 全部 **project 行级强绑**；design↔requirement 为 m2m（钉 approved revision）；ADR 独立实体六态；与 Product 零关系（不存在通过 Project 间接识别 Product 的实现） | 0010:40/137；§8 |
| 7 | Product Plan 是否已实现？属于哪个领域？ | **NOT IMPLEMENTED**。协议层以 `TargetDomain::ProductPlan=PermanentlyExcluded` 固化（Q-PC-13）；文档 19 篇，代码零对应物 | registry.rs:57；§9 |
| 8 | 是否存在资产归属与关联混用问题？ | **主链路未混用**：归属列零 UPDATE 通道、关联前同租户同项目校验；**一处现例**=docproc.document（project 可空+租户全量读，关联/所有权解耦）；**一处借住**=分析报告正文借 kb 存储 | §4/§10 GAP-08/09 |
| 9 | 与既有裁决的差异？ | 12 项登记（1×P0 / 1×P1 / 8×P2 / 2×P3），含 1 项已闭合核对更新 | §10 |
| 10 | Phase 4 必须裁决的问题？ | P4-01~08 八项积压，核心两问=P4-Q-01（Product 落点）与 P4-Q-04（产品认知权威上移） | §12 |

**最终判定：PASS WITH ISSUES**（§14）。

---

## 2. 审计基准与范围

### 2.1 Git 与文件安全记录（任务书 §五）

| 项 | 值 |
|---|---|
| 分支 | `master` |
| HEAD | `b0d8973abffd7d97270aaeaf5c868e6d0e09b61f`（2026-11-16 23:31 +0800） |
| 工作区 | **不干净**：20 个已修改文件（全部在 ai-frontend/src，属并行会话在途工作）+ 1 个未跟踪目录 `docs/architecture/phase4/`（Phase 4 基线存档 + 上一轮审计草稿） |
| 本审计动作 | 仅新增本报告；未执行 checkout/reset/clean/stash/commit、未跑 Migration、未格式化、未改任何在途文件 |

### 2.2 审计基准与免责边界

- **Schema 真源 = migrations/0001~0025 as code**。运行中数据库的实际应用状态与行级数据**未检视**（只读 DB 查询不在任务书 §五 允许清单内）→ 涉及处标 UNKNOWN（§13）。任务书 §四.4（Migration 存在≠已应用）据此全篇适用。
- **API 存在≠业务闭环**：所有桩/未接线面显式标注（§3/§7.3/§10）。
- **方法**：4 路并行代码取证（Schema 全量 / API+UI 全量 / 字段级约束 / 所有权调用链）+ 载重断言手工复核（`require_project_visible` 实现、0025 主键缺失、P3-KB-01 台账原文、PC 单例 bootstrap 调用点均已亲验）。
- 证据格式：`文件:行号`；SQL 引用 = migrations 目录。

---

## 3. Current Architecture Inventory（实体清单）

### 3.1 任务书 §二.A 十三实体逐项判定

| 实体 | 是否实现 | 数据表 | 所属领域 | 证据 |
|---|---|---|---|---|
| Team | ⚠️ **语义映射实现**（无 team 表，载体=core.tenant） | core.tenant | core/IAM | 0001:1300 |
| Product | ❌ **NOT IMPLEMENTED** | 无 | — | 全仓 grep 实证（§9 前置核查）；仅 PC object_type 三键 |
| Project | ✅ | core.project（FK tenant+customer；status 生命周期；project_member 角色） | core | 0001:1251/1287 |
| Product Cognition | ✅（**项目私有**形态） | pc.cognition_object/_revision/_version/_document/_proposal/_source 等 15 表 | pc | 0001:3041-3453 |
| Project Cognition | ⚠️ **部分实现（Stage 1）**：新域 13 表已建、数据面服务在位、**零 REST、写入者未接线**；旧链 ai.cognition_run/item 在役（S01-04） | projc.*；ai.cognition_run/item/project_knowledge | projc / ai（legacy） | 0025:1-44；projc/mod.rs:16-21；0001:796 |
| KB Source | ✅（tenant 级） | kb.source + kb.source_project | kb | 0001:2982/3008 |
| Document（KB） | ✅（project 强绑） | kb.document / kb.document_revision | kb | 0001:2860/2878 |
| Memory（KB） | ✅（project 强绑） | kb.memory / kb.memory_revision / kb.memory_ref | kb | 0001:2901-2934 |
| Requirement | ✅ | delivery.requirement / requirement_revision / confirmation / acceptance_criterion / source_input / finding | delivery | 0001:1529-1660 |
| Design | ✅ | design.design / design_revision | design | 0010:40/60 |
| Design Revision | ✅（不可变，触发器守卫） | design.design_revision | design | 0010:60；trg_design_revision_guard 0010:114 |
| ADR | ✅（六态+supersede 链） | design.adr / adr_revision / adr_design_revision / adr_requirement | design | 0010:137-230 |
| Product Plan | ❌ **NOT IMPLEMENTED** | 无 | — | registry.rs:57（PermanentlyExcluded）；§9 |

### 3.2 约束保证层级四分类（任务书 §二.B 要求）

| 层级 | 内容 | 证据 |
|---|---|---|
| ① 数据库已实现约束 | 0001 基线全域（core/delivery/kb/pc/ai/discovery/governance/integration/devenv/source）：PK、组合键 FK、UNIQUE、CHECK、不可变触发器。例：kb.document UNIQUE(tenant,project,doc_key) 0001:5233；pc 部分唯一 uq_pc_object_singleton 0001:6854；kb.source 触发器全拒 UPDATE/DELETE 0001:6959 | §附录字段清单 |
| ② 仅应用代码保证的约束 | **design（0010）/ plan（0011）/ projc（0025）三域全部表：grep 实证零 `PRIMARY KEY` 声明**——id 仅 NOT NULL DEFAULT gen_random_uuid()，实体唯一性无库级保证；零 FK（0025 头注自述「T-2 启用」）；同 project 归属校验、关联合法性、状态机全部在服务层 | 0025:71-90 亲验；0010:40-56；0011:27-45 |
| ③ 仅存在于设计文档的约束 | **P3-KB-01「KB 资产 Team 级统一所有权」**（台账已确认 A 不重开，代码未落地）；V2.0 §9.1 Git 正文权威（kb/pc 目标态）；四域注册表 target 态条目 | 台账 v1.0:145；authority-matrix-v1.md §2 |
| ④ 尚未实现的约束 | Product 实体与生命周期；Product–Project 关系；Product 资产关联；Product Plan；ProductReality 引用通道（registry.rs 预留 NotYetOpen）；projc REST 与 T-2 约束启用；pc.relationship 生产调用者（桩） | registry.rs:48-57；§12 |

**索引与软删除摘要**：0001 域有针对性索引（如 idx_kb_memory_revision_review_queue 部分索引 0001:6658）；design/plan/projc 索引极少（projc.cognition_object 仅 1 个 partial 唯一索引 0025:458）。**全库无 deleted_at 类软删除列**——生命周期以 status/lifecycle_status 枚举承载（project.status 含 'deleted'，support.rs:229 可见）。

---

## 4. Current Ownership Matrix（资产归属矩阵）

> 判定纪律：**「所有者」= 行级不可改的绑定维度（归属列）；「关联」= 可挂接/解除的引用（关联表或可空列）。二者全篇严格区分。**

| 资产 | 当前事实所有者 | Team 归属列 | Product 关联 | Project 关联 | 实现证据 |
|---|---|---|---|---|---|
| kb.source | **tenant**（行级唯一绑定） | tenant_id NOT NULL | 无 | **关联表** kb.source_project（只增不可删） | 0001:2982/3008 |
| kb.document(+revision) | **project**（行级强绑） | tenant_id | 无 | 行级绑定（单项目、无改属通道） | 0001:2860/2878 |
| kb.memory(+revision) | **project**（行级强绑） | tenant_id | 无 | 行级绑定 | 0001:2901 |
| kb.memory_ref | project（随 revision） | tenant_id | 无 | 行级 + 四字段软引用外部对象（追加式不可删） | 0001:2914-2934 |
| pc.*（含 product 三键对象） | **project**（行级强绑） | tenant_id | 无（Product 不存在；product_* 仅是对象类型键） | 行级绑定 + 组合 FK core.project | 0001:9549 |
| projc.* | project（列位在；写入未接线） | tenant_id | 无 | 行级列位（无 FK，T-2） | 0025:71 |
| delivery.requirement(+revision) | project | tenant_id | 无 | 行级绑定 | 0001:8069 |
| design.design(+revision) / adr(+revision) | project | tenant_id | 无 | 行级列位（无 FK） | 0010:40/137 |
| plan.task_plan(+revision) | project | tenant_id | 无 | 行级列位（无 FK） | 0011:27 |
| integration.repo_analysis_report | project | tenant_id | 无 | 行级真 FK + →kb/pc 裸 ID 引用 | 0023:18-21 |
| docproc.document | **tenant**（project 仅软关联、可空） | tenant_id | 无 | 可空列（应用层校验） | 0016:28-46 |
| ai.team_llm_provider/model/credential | **tenant** | tenant_id | 无 | — | 0018:14+ |
| platform.embedding_config | 平台全局单例（无租户） | — | 无 | — | 0012:18 |
| **Product** | **不存在** | — | — | — | §3.1 |

**权限校验面（读/写分离）**：

| 面 | 机制 | 证据 |
|---|---|---|
| 读+写通用门 | `require_project_visible`：单条 SQL 同时校验 **tenant 匹配（p.tenant_id=$2）+ 项目成员（m.principal_id=$3）+ 未软删**；不可见一律 404 不泄露存在性 | support.rs:225-250（亲验） |
| 创建时归属判定 | 统一模式：tenant_id←认证 caller（auth.rs:166-172）；project_id←URL 路径/query+可见性门；created_by←caller.principal_id；**请求体不承载归属字段**（requirement.rs:125-137、document.rs:529-534、source.rs:37-52 实证） | §附录调用链 |
| PC 正式面 | `ensure_formal_authorized`：真人 principal（kind='human'）+ A3 治理闸（project_member role='owner' OR approver_grant）——**项目级角色，非 tenant 角色** | pc/mod.rs:152-176；approver.rs:56-79 |
| 真人决策门 | `require_human_decision_context`：interactive auth_mode + 查库 kind='human'（api_key/Agent 拒） | support.rs:111-138 |
| tenant 级权限唯一现役点 | LLM Provider 管理面（caller.is_admin） | team_llm.rs:222-224 |

---

## 5. Product–Project Relationship Analysis

| 问 | 答 |
|---|---|
| 当前是否存在 Product | **否**（NOT IMPLEMENTED，§3.1） |
| Product 与 Project 是否存在直接关系 | 否——连间接识别都不存在（需求/设计/仓库全部 project 强绑，无任何「经 Project 推导 Product」的代码） |
| 一对多/多对多支持 | N/A |
| 关联是否可修改 | N/A；现存最接近的先例 kb.source_project **只增不可删**（无 unlink 端点，全仓无 DELETE FROM kb.source_project） |
| 是否存在历史记录 | N/A；关联表先例的留痕水平：source_project 有 linked_by+linked_at；其余关联表见 GAP-09 |
| 目前缺失 | Product 身份/生命周期全部；产品目录；产品-项目绑定；产品级资产聚合；产品级权限维度 |

**PC 侧「产品身份」现状（识别机制的唯一实现）**：项目创建即 `ensure_product_singletons_in`（bootstrap.rs:325）为每项目建 `product_profile`+`product_vision` 单例骨架（部分唯一索引 (tenant,project,object_type) 0001:6854 保证单例）。**后果：同一产品在 N 个项目中有 N 份互不关联的认知单例；产品认知事实上以项目为单位私有。** 这是 Phase 4 的靶心（→ P4-Q-04 / §12 P4-03）。

---

## 6. Cross-domain Reference Matrix（跨领域引用矩阵）

> 类型标注：**FK**=数据库外键 · **m2m**=中间关联表（无 FK，ID 引用）· **ID**=裸 ID/四字段软引用 · **API**=API 层逻辑关联 · **DOC**=仅文档约定。
> 跨域真 FK 仅指向 core.*（0001 基线 87 条组合键）+ 零星域间 FK（task_run.client→devenv.device 0001:8126 等）；0010 之后新域一律零 FK。

| 从 ↓ 至 → | kb | pc | projc | requirement | design/ADR | task plan | analysis report | product plan |
|---|---|---|---|---|---|---|---|---|
| **kb** | — | pc.source 指 kb_document/kb_memory（**ID** 四字段，0001:3391） | projc.source（**ID**，0025:264） | 无 | 无 | 无 | repo_analysis_report.kb_revision_id（**ID** 裸，0023:28） | DOC（P3-KB-01 关联语境） |
| **pc** | relationship external 端点按域分派（**ID**；**桩**，无生产调用者） | — | 无 | pc.source source_kind=requirement(_revision)（**ID**）+ relationship_type 种子 informs（**DOC** 级语义） | 无 | 无 | 无 | **协议常量 PermanentlyExcluded**（registry.rs:57） |
| **projc** | projc.source（**ID**） | projc.source source_kind='product_cognition'（**ID**） | — | projc.source（**ID**） | 无 | 无 | projc.source 'repo_analysis'（**ID**） | 无 |
| **requirement** | 无 | 无 | 无 | — | design_requirement / adr_requirement（**m2m**，钉具体 approved revision_id，0010:120/220） | 间接：task.implementation_baseline_id→design 域（**ID**） | 无 | DOC |
| **design/ADR** | 无 | 无 | 无 | 同上 | — | plan.task_plan.source_baseline_id→implementation_baseline（**ID**，0011） | design_revision.analysis_run_id（**ID**） | 无 |
| **analysis report** | kb_revision_id（**ID** 裸，可悬空） | pc_source_id（**ID** 裸） | 无 | intake_id UNIQUE（**ID**） | 无 | 无 | — | 无 |
| **conversation/assistant** | context_item 六类对象（**ID**，0019） | 同左（product_cognition 块为 **API** 层组装） | 无 | 同左 | 同左 | 无 | 无 | 无 |
| **docproc** | kb.document_revision.source_ref（**ID**，0017） | 无 | 无 | 无 | 无 | 无 | 无 | 无 |

**API 层逻辑关联**：`POST /v1/retrieval/query` 统一检索跨 kb/pc（retrieval.rs:32）——唯一现役跨域读通道。
**DOC 级约定**：Product Plan 引用 PC 认知（Q-PC-13 契约）；ProductReality 通道（预留未开放）。
**引用健康度**：四域审计已登记「引用链零真实数据」（pc.source/kb.memory_ref 全库近 0 行）——机制在位、从未承载业务负载；analysis_report 裸引用已知可悬空（authority-matrix §2）。

---

## 7. KB / PC / Project Cognition 核查

### 7.1 KB 权威模型 vs P3-KB-01（任务书 §二.D 逐条）

| 裁决条目 | 判定 | 证据 |
|---|---|---|
| Source 采用 Team 级所有权 | ✅ **符合** | kb.source 仅 tenant_id+registered_by，无 project 列（0001:2982）；触发器全拒改删（0001:6959） |
| Document 采用 Team 级所有权 | ❌ **不符合（已裁未落地）** | kb.document project_id NOT NULL（0001:2863） |
| Memory 采用 Team 级所有权 | ❌ **不符合（已裁未落地）** | kb.memory project_id NOT NULL（0001:2904） |
| Project 仅作关联/使用上下文 | ⚠️ 仅 Source 满足（kb.source_project 关联表）；Document/Memory 是**强绑定而非关联** | 0001:3008 vs 0001:2863 |
| Document 支持关联多 Project | ❌ | 行级单绑定，无关联表；全仓零改属通道（§附录 D-2） |
| Memory 支持关联多 Project | ❌ | 同上；kb.memory_ref 是对外部对象的软引用，**不是** project 关联表（0001:2914） |
| 存在 Project 独立拥有 KB 资产的实现 | ✅ **事实存在**——document/memory 即项目所有形态，与裁决冲突即 GAP-02 | 0001:2860-2934 |
| Product 关联影响 Team 级所有权 | N/A（无 Product） | — |

裁决台账自认状态：P3-KB-01「已确认 A，不重开；**下一步与 kb.source_project 及 Document/Memory 真实实现对齐**」（Phase3-基线校准与裁决台账-v1.0.md:145）——差异被裁决方知情登记，非执行偏离。

### 7.2 Product Cognition 核查（任务书 §二.E）

| 检查项 | 结论 | 证据 |
|---|---|---|
| 实体结构 | pc 域 15 表：对象/修订/版本/正文/提案/审批/来源/证据/写账本；0001 域内完整 PK/FK/UNIQUE/触发器 | §附录 |
| Product 身份标识 | **项目私有单例**（每项目 product_profile+product_vision）；object_key 唯一 (tenant,project,object_key) 0001:5369 | bootstrap.rs:325 |
| 与 Team 关系 | tenant_id 列位（经 project 组合 FK 传导）；无独立 Team 级认知 | 0001:9549 |
| 与 Project 关系 | 行级强绑（组合键 FK） | 0001:9549 |
| 正式认知所有者 | **项目级治理角色**（project owner OR approver_grant）+ 真人门；AI/服务主体恒 403 | pc/mod.rs:152-176；approver.rs:56-79 |
| AI 建议 vs 人工确认 | ✅ 机制完整：AI 产物恒 draft/提案（8 态），发布唯一事务 publish.rs，确认门 require_human_decision_context | pc/proposal.rs；publish.rs:455 |
| 认知版本管理 | revision（追加式不可变触发器）→ cognition_version（published/archived，唯一发布事务，不可删） | 0001:7043/7050 |
| 多 Project 共享同一 Product Cognition | ❌ **不存在** | 结构性不可能（行级 project 强绑） |

**对照裁决「Product Cognition 是产品正式认知的唯一权威来源」**：机制面成立（唯一服务层写入面+触发器+发布事务，authority-matrix §1.5 无越权面）；但权威的**作用域以项目为单位**——「唯一」在单项目内成立，跨项目不成立（N 份单例互不认账）。此为 Phase 4 需裁决而非缺陷（→ P4-Q-04）。

### 7.3 Project Cognition 核查（任务书 §二.F）

| 检查项 | 结论 | 证据 |
|---|---|---|
| 实体结构 | projc 13 表（0025，镜像 pc 机制）；**无 PK/FK**（T-2 承诺）；触发器部分在位（object/version/proposal 有，cognition_revision **无任何触发器**——与 pc 的差异点） | 0025:429-444 |
| 与 Project 关联 | tenant+project 列位（零 FK） | 0025:71-87 |
| 技术认知维护方式 | Stage 1 数据面：upsert_object_draft / register_source / register_evidence（projc/object.rs、evidence.rs），唯一消费方是集成测试 | projc/mod.rs:16-21 |
| 正式认知确认机制 | **未接线**：提案/审批表「结构先建、写入者后启」；正式面规划=照抄 pc::ensure_formal_authorized（真人+A3） | 0025 头注；projc/mod.rs:16-21 |
| 版本及历史追踪 | 表结构在位（revision/version 同构 pc）；无生产数据 | 0025:112-160 |
| 与 PC 引用关系 | projc.source source_kind='product_cognition'（四字段软引用）；字典级消歧：repository_module 注释「非 PC Product Module」（0025:63） | 0025:246-266 |
| 重复维护/权威冲突 | **当前无冲突面**（写入未开）；两域红线已立：「复用机制、不复用业务表」+ 不共享身份/状态机/确认权威/正文权威/字典（R-02） | 0025:9-12 |
| 遗留双轨 | 旧链 ai.cognition_run/item/project_knowledge（S01-04 项目认知 Run，在役）与 projc 目标态并存——过渡期双轨由 P3-11 W 批处理（切换批已放行） | 0001:796；P3-11 方案 |

---

## 8. Requirement / Design / ADR 核查（任务书 §二.G）

| 检查项 | 结论 | 证据 |
|---|---|---|
| Requirement 独立实体 | ✅ 身份+不可变修订+真人确认（requirement_confirmation 表记录 action/actor）+ 验收标准 + 来源/发现关联 | 0001:1529-1660 |
| Requirement 归属主体 | **project**（行级强绑；tenant 库内派生防 caller 不一致 requirement.rs:461） | 0001:8069 |
| Design 关联多 Requirement | ✅ m2m（design_requirement），**钉具体 approved requirement_revision**（服务层校验仅 approved 可挂 technical.rs:401-417）；S03 多对多裁决未被改变 | 0010:120 |
| Design Revision 独立记录 | ✅ 不可变（禁删+提交后内容冻结触发器）；source human/ai 留痕 | 0010:60-114 |
| ADR 独立实体 | ✅ 六态 proposed/under_review/accepted/rejected/superseded/deprecated + supersede 链 + deprecate + implementation-blocking | 0010:137-157；adr.rs |
| ADR 与 Requirement/Design 关系 | adr_design_revision + adr_requirement（m2m，钉 revision）；**无解除路由**——解除只能走新 ADR 修订 | 0010:204-230 |
| 与 Product 关系 | **零**（无 Product 可关联，也无经 Project 间接识别 Product 的实现） | §5 |
| 人工确认原则 | 完整在位：设计/ADR 批准走 ensure_trusted_context + 审批表；ADR supersede/deprecate require_human_conn（adr.rs:571/647/698） | pm_designs.rs:344 |

**S03 既有裁决遵守情况：多对多关系与人工确认原则均未被改变，无差异。** design/plan 两 schema 的零 PK/FK 属实现惯例（②类约束），不构成裁决冲突（GAP-04 记录其风险）。

---

## 9. Product Plan 核查（任务书 §二.H）

| 检查项 | 结论 |
|---|---|
| 是否实际实现 | ❌ **NOT IMPLEMENTED**——无表、无服务、无路由、无 UI。代码唯一痕迹=`TargetDomain::ProductPlan`（owning_service=Platform，通道状态 **PermanentlyExcluded**，注释援引 Q-PC-13） | 
| 是否属于 PC 正式认知对象 | 否——协议层已固化排除（registry.rs:57、domain_ref.rs:35 未开放通道）；**符合 Q-PC-13，无冲突** |
| 如何引用 Product Cognition | 未实现；文档约定=引用 PC 正式认知 version（19 篇文档语境一致：规划域消费认知，不产认知） |
| 如何关联 Project/Requirement/Design | 未实现 |
| 是否存在规划内容直写正式产品认知 | **无违规面**（Plan 实体不存在，故不存在写入通道；pc 正式面唯一写入者=pc 服务层，authority-matrix §1.5） |
| 易混淆点澄清 | `plan.task_plan`（S04，0011）是**任务计划**聚合层（复用 delivery.task、source_baseline_id 钉 S03 基线），**不是** Product Plan——机制可参照，领域语义必须区分 |

文档侧登记：19 篇文档提及 Product Plan（PC 基线 §13 FR-026 定义、九项裁决保持表、MVP 基线「无表；Roadmap 未裁决」等）——全部为规划语境，无一实现。**FR-026 的功能范围至今「待功能范围确认」（docs/product-cognition-knowledge-base），这是 P4-05 的输入缺口。**

---

## 10. Design vs Implementation Gap（任务书 §三.5）

> 严重度：P0=权威边界或数据安全重大问题 · P1=核心领域模型与既有裁决冲突 · P2=功能或关联能力缺失 · P3=优化建议。
> 归属标注按 Phase 4 基线 §7：**[P3]**=Phase 3 问题（分别登记）· **[P4]**=Phase 4 依赖问题。

| 编号 | 既有设计裁决 | 当前代码实现 | 差异 | 严重度 | 证据 |
|---|---|---|---|---|---|
| GAP-01 | analysis_run 单一状态权威（写入者收敛纪律） | **双写入者分叉仍在**：AI 编排器 start_run→analyzing→completed（pm-ai/context.rs:153-165）vs 导入链直插 succeeded（project_import/proposal.rs:312-321）；无 DB CHECK/触发器/乐观锁 | 同表两条独立状态机，词表分叉（completed/failed vs succeeded） | **P0** [P3] | authority-matrix-v1.md §1.3 登记；本轮调用链复核确认未修复 |
| GAP-02 | P3-KB-01：KB 资产 Team 级统一所有权，Document/Memory 可关联多 Project | kb.document/memory `project_id NOT NULL` 行级强绑 | 裁决态≠实现态（裁决方知情登记「下一步对齐」） | **P1** [P3→P4 前置] | 0001:2863/2904；台账 v1.0:145 |
| GAP-03 | V2.0 §9.1 正文权威=Git（目标态） | 三模式并存：Git（旧 ai 栈）/PG content（kb、pc）/借住（报告正文）+ ai.project_knowledge 第五处 PG 权威 | 四项偏差已认账，P3 治理中；影响 P4 版本化引用设计（钉什么权威） | P2 [P3] | authority-matrix-v1.md §2 |
| GAP-04 | 0025 头注：projc 约束「T-2 启用」；0010/0011 同惯例 | **0010/0011/0025 全部表零 PRIMARY KEY 声明**（grep 实证）；零 FK；projc.cognition_revision 连触发器都没有 | 实体唯一性/引用完整性全靠应用层；T-2 未兑现期拉长即风险累积 | P2 [P3] | 0025:71-90 亲验；0010 全文；0011 全文 |
| GAP-05 | 四域注册表 target 态 | MISMATCH×1 / STUB×2 / MISSING×4；pc.conflict、pc.relationship、KB-EXTRACTION 等「表存在≠功能存在」桩；引用链零真实数据 | 注册表与实现偏差未闭合 | P2 [P3] | four-domain-registry-mapping-audit-taskbook-v1.md |
| GAP-06 | Phase 4 目标：Product 主体 | **不存在**（无表/服务/路由/UI）；产品认知=逐项目私有单例 | Phase 4 靶心，从零新建 | P2 [P4] | bootstrap.rs:325；§3.1 |
| GAP-07 | Q-PC-13：Product Plan 不属 PC，属产品规划域 | 协议层固化排除；**规划域本身零实现** | 与裁决一致（非冲突），但 P4-05 的功能范围（FR-026）至今未确认 | P2 [P4] | registry.rs:57；§9 |
| GAP-08 | docproc D-01：Team 级文档（project 可空，应用层校验） | 读面 `list_documents` 不传 project 时=**租户全量**（含他项目上传文档的解析产物），无项目成员校验；存在性与项目归属解耦 | Team 级设计自洽，但「同租户非项目成员可读项目文档产物」是「关联≠所有权」的现役活例，安全态势需产品负责人确认 | P2 [P4] | docproc.rs:458-486；0016:28-46 |
| GAP-09 | P4-DP-05：关联变化保持历史可追溯 | 六关联表仅 kb.memory_ref 追加式不可删；requirement_source_input/finding、design_requirement 解除=物理 DELETE（有审计行 requirement.source_unlinked 等）；**ADR 关联无解除路由**；task_knowledge_ref 整批替换（DELETE+INSERT）无差量史；**kb.source_project 只增不可删** | 关联历史能力不齐，P4 资产关联设计不能以现状为先例 | P2 [P4] | traceability.rs:222-255；technical.rs:440-481；pm_write/task.rs:451-475 |
| GAP-10 | Phase 4 预设：Product 属 Team（需 Team 级权限维度） | 现行资产权限=项目成员+项目级治理角色单模式；tenant 权限仅 LLM 管理面 | Team→Product 权限维度须新建（P4-Q-11） | P2 [P4] | approver.rs:56-79；team_llm.rs:222 |
| GAP-11 | 文档语境「Team」 | schema 语言=core.tenant，无 team 表；team_llm_* 按 tenant_id | 命名错位，P4-01 定名时须显式声明映射 | P3 [P4] | 0001:1300；0018 |
| GAP-12 | 创建者留痕一致性 | AI 命令执行器写 delivery.requirement 头行**不写 created_by**（executor.rs:473-479），与 REST 路径（requirement.rs:469）不一致 | 审计留痕缺口（该链路仅在真人批准红线提案后发生） | P3 [P3] | executor.rs:473-497 |

**已闭合核对更新**（避免陈旧风险遗留）：authority-matrix-v1.md §1.4 登记的 P0「ai.project_knowledge 决策门强度不足（AI 可写正式认知）」**已在当前 master 修复**——decide 入口现走 `require_human_decision_context`（pm_cognition.rs:295-296；support.rs:111-138），confirmed_by=真人（repo_cognition.rs:265-274）。本轮复核确认闭合。

---

## 11. Ownership Risk Register（任务书 §三.6 八类）

| 风险类 | 判定 | 说明与证据 |
|---|---|---|
| 资产所有权不明确 | ⚠️ 部分存在 | 既有 13 类资产行级归属全部明确（§4）；**Product 层整体缺位**=Phase 4 的结构性空白（GAP-06） |
| Team 隔离不足 | ✅ 未发现跨租户通道 | 归属判定统一取自认证 caller（auth.rs:166-172），请求体不承载归属字段；`require_project_visible` 双条件校验亲验（support.rs:225-250）；抽查 4 个 handler 文件无绕过守卫的写入口。注意：docproc 租户内读面宽松（GAP-08）是租户**内部**问题，非跨租户 |
| Product 与 Project 混淆 | ✅ 未发生（无 Product） | 已有消歧预防：projc repository_module 注释「非 PC Product Module」（0025:63）；ProductMark.vue 为品牌件非业务组件 |
| 关联操作错误改变资产所有权 | ✅ **未发现** | 全仓 grep 对 tenant_id/project_id 的 UPDATE **零命中**；无 reassign/transfer 通道；关联创建前同租户同项目校验（adr.rs:416-431、technical.rs:384-400、discussion_memory.rs:103-143） |
| 跨领域数据复制 | ✅ 引用优先惯例成立 | 软引用四字段是仓库既有惯例（§6 矩阵）；报告正文借住 kb 是已认账的临时借住（GAP-03），非隐性复制 |
| 多领域同时维护同一正式资产 | ⚠️ 一处 | ai.analysis_run 双写入者（GAP-01，P0）；pc/projc 分域红线已立、无冲突面（§7.3） |
| 删除/解除关联导致历史丢失 | ⚠️ 部分存在 | GAP-09：5/6 关联表物理 DELETE（其中 requirement/design 系有审计行补偿）；task_knowledge_ref 整批替换无差量史；ADR 关联无解除路由（以修订代解除，语义自洽） |
| 权限校验遗漏 | ⚠️ 两处 | ①docproc 读面租户全量（GAP-08）；②executor 写 requirement 头行缺 created_by（GAP-12，留痕非权限）。正式面真人门体系（require_human_decision_context / ensure_formal_authorized）抽查完整 |

---

## 12. Phase 4 Decision Backlog（P4-01 ~ P4-08 待裁决事项）

> 每项含：问题描述 / 当前实现 / 为何需裁决 / 可选方案 / 推荐（**推荐≠正式架构决定**）/ 影响范围 / 依赖 / 尚缺证据。

### P4-01 Product Domain Model
- **问题**：Product 作为独立业务主体的身份、落点、生命周期、层级。
- **当前实现**：完全不存在（§3.1）；Team=core.tenant；先例=core.customer（tenant 内父子层级表，0001:1203）。
- **为何需裁决**：一切归属演进的主语缺位；不裁决则 P4-02~07 无从谈起。
- **可选方案**：(a) core.product 新表（tenant 级，仿 customer 模式，组合键 FK）(b) 独立 product schema（仿 0025 惯例，零 FK+应用层）(c) 不建实体、仅上移 PC 单例（违背基线目标 1，不推荐）。
- **推荐**：(a) core.product + lifecycle(active/archived) + tenant 级唯一键；产品族/产品线层级 V1 不做（YAGNI，缺真实场景）。
- **影响**：新表+权限维度+导航+审计；组合键 FK 惯例选择影响后续所有关联表形态。
- **依赖**：无（根问题）。
- **尚缺证据**：产品族/多租户产品共享的真实需求样本；产品命名唯一性范围（tenant 内唯一？）。

### P4-02 Product–Project Relationship
- **问题**：基数（N:N?）、主次、独立 Project 合法性、关联变更权限与历史。
- **当前实现**：无任何关系；最接近先例=kb.source_project（只增不可删）与 docproc 可空列。
- **为何需裁决**：基线 P4-DP-03 禁止一对一固定关系；关联历史受 GAP-09 教训约束。
- **可选方案**：(a) product_project 关联表（m2m+is_primary 标记+linked_by/at）(b) project.product_id 单列（放弃 N:N）(c) 关联表+生效区间列（历史化）。
- **推荐**：(a)，追加式解除（状态列置 inactive 而非物理删除），允许不关联 Product 的独立 Project。
- **影响**：core/导航/权限/审计；project 创建流程（是否强制选产品）。
- **依赖**：P4-01。
- **尚缺证据**：「一项目服务多产品」的真实场景是否存在。

### P4-03 Cognition Ownership Boundary（**Phase 4 最重裁决**）
- **问题**：产品认知权威是否随 Product 上移；一 Product 对多 ProjC 的关联；认知冲突处理与实施发现反馈。
- **当前实现**：PC 单例逐项目私有（bootstrap.rs:325）；projc Stage 1 未接线；PC↔ProjC 仅有软引用枚举。
- **为何需裁决**：现状下跨项目产品认知不共享、互不认账（§7.2）；触及 pc 域已冻结模型（B01~B10），按基线 §7 须单独 Decision Record。
- **可选方案**：(a) product 认知上移 Product 级（pc.object 增 product 维度或新建 product 级认知承载），项目侧派生视图 (b) 保持项目私有，Product 仅聚合引用（弱权威） (c) 双层：产品级权威+项目级实例，冲突时产品级胜。
- **推荐**：(a)+(c) 混合——权威上移至 Product 级、项目级保留派生视图与项目特有认知；遵守 P4-DP-04（引用不复制权威）。
- **影响**：pc 域冻结模型、bootstrap 流程、P3-11 在途实施（projc 镜像 pc 三文件）；数据近零→可直改。
- **依赖**：P4-01；P3-11 落地时序（R-03）。
- **尚缺证据**：跨项目认知冲突的真实案例；pc.cognition_object 单例索引改造对 P3-11 W4~W7 批的影响面。
- **风险**：R-02（高）——不得在 P4 内顺手改 pc，须专项评审。

### P4-04 Product Asset Association
- **问题**：Product 与 KB/PC/Requirement/Design/ADR 的关联模型（统一表 vs 域内列、多关联、版本化、审批、解除留痕）。
- **当前实现**：零关联；关联机制先例=memory_ref（追加式）/design_requirement（物理删+审计）/source_project（只增）——三种语义并存（GAP-09）。
- **为何需裁决**：基线 P4-DP-02/05；现状关联历史能力不齐，不能作先例。
- **可选方案**：(a) 统一 product_asset_association 表（asset_domain/asset_type/asset_id/revision_no，追加式）(b) 各域加 product_id 列 (c) 混合（KB 用统一表，强绑定域用列）。
- **推荐**：(a) 统一表+版本化引用（钉 revision_no，受 GAP-03 约束先声明权威）+ 解除置 inactive 保留历史；关联审批跟随各域既有真人门（不新增独立审批）。
- **影响**：6 域读面+检索+UX。
- **依赖**：P4-01；**GAP-02（P3-KB-01 落地）是 KB 关联的硬前置**（否则 Document/Memory 关联建立在未实现的所有权上，R-01）；P4-03。
- **尚缺证据**：关联业务量级（是否需要审批）；KB 域去 project 强绑的具体改造方案在 P3 侧尚未设计。

### P4-05 Product Plan Integration
- **问题**：Product Plan 归属主体、域落点、与 PC/Requirement 的生成关系。
- **当前实现**：零实现；协议层 PermanentlyExcluded（符合 Q-PC-13）；FR-026 功能范围未确认。
- **为何需裁决**：基线目标 6；19 篇文档语境不一致，需收敛为 V1 最小面。
- **可选方案**：(a) 新独立域（product-plan schema）(b) 扩展 plan.task_plan 机制复用（领域语义不同，仅机制参照）。
- **推荐**：(a) 独立域；引用 PC 认知**钉 cognition_version.version_no**（版本化，权威唯一）；V1 最小面=产品目标→项目需求包的下发与追踪，不做规划编排。
- **影响**：新域+UX 入口+与 delivery.requirement 的生成链。
- **依赖**：P4-01/P4-03。
- **尚缺证据**：FR-026 确认记录（docs/product-cognition-knowledge-base 挂账中）。

### P4-06 Migration & Compatibility
- **问题**：演进策略、一致性校验、回滚。
- **当前实现**：认知域真实数据近零、引用链零真实数据（四域审计）；AGENTS.md 规则 1（新项目无存量，禁兼容层/双写/迁移脚本）。
- **为何需裁决**：锁定「直改契约」基调，防止执行期无据引入兼容分支。
- **可选方案**：(a) 直改契约+重建表（无数据搬运）(b) 加列兼容+双写（被 AGENTS.md 禁止）。
- **推荐**：(a)；迁移号全分支扫号排期（历史上限教训）；回滚=迁移级 down 脚本纪律。
- **影响**：全部新表+pc 改造点。
- **依赖**：P3-11 的 0026；P4-01~05 定稿。
- **尚缺证据**：运行库实际应用状态（§13 UNKNOWN-1）。

### P4-07 UX & Navigation Impact
- **问题**：Product 入口/Overview/资产视图的 IA；避免复制一套 Project 页面。
- **当前实现**：/project/product-cognition PLANNED 占位（navigation.ts:316-322）；全局导航 IA V1.2 刚冻结（全局区 C2 团队总览 + 项目区 C3 四域入口）；ProductMark.vue 为品牌件。
- **为何需裁决**：基线明令「先场景后页面」；IA V1.2 的 R-2 单侧栏分层裁决是约束面。
- **可选方案**：(a) Product 入口放 Team 全局区（产品目录→产品详情聚合认知/资产/项目/规划）(b) 项目区加 Product 页签。
- **推荐**：(a)——Product 是 Team 级主体，放全局区与 C2 团队总览同层；项目内用关联徽标/面包屑回链产品。
- **影响**：navigation.ts 三源配置、壳 chrome、Figma 设计稿。
- **依赖**：P4-01/02 裁决先行。
- **尚缺证据**：用户工作场景调研（基线 §5 P4-07 要求）。

### P4-08 Architecture Review & Freeze
- **问题**：冻结前的收口条件。
- **当前实现**：—（门禁本身）。
- **冻结前置清单（从本审计导出）**：①GAP-01（P0）由 Phase 3 责任人闭合或书面接受；②GAP-02 落地计划进入 P3/P4 联合排期；③P4-Q-04 专项 Decision Record + 评审（R-02）；④GAP-08 安全态势由产品负责人确认；⑤P4-01~07 全部产生已确认的 Decision Record；⑥§13 UNKNOWN 项逐个消解或书面接受。

---

## 13. 未能确认的问题（UNKNOWN）

| # | 事项 | 阻塞的检查 | 需要的动作 |
|---|---|---|---|
| U-1 | 运行中数据库的实际迁移应用状态与行级数据（含 pc 字典种子行、intake 'product_fact' 使用情况） | 「Migration 存在≠已应用」（§四.4）无法闭合；种子数据丢失风险（authority matrix 曾实测 full-dev 字典 0 行） | 授权只读 DB 检视，或由运维确认 full-dev 当前库状态 |
| U-2 | docproc 租户全量读面是否为**被接受**的产品安全态势 | GAP-08 定级（P2 或升级） | 产品/安全负责人裁决 |
| U-3 | 已删除历史分支的迁移号占位（记忆载明 0055/0064 时代占用）对 Phase 4 排号的影响 | P4-06 排号前提 | 实施期全分支扫号（纪律已在 P4-Q-12） |
| U-4 | ProductReality 引用通道的预留语义（registry.rs 状态 NotYetOpen，无文档说明其预期契约） | P4-04 可选方案完整性 | P4 架构师补设计或显式废弃 |
| U-5 | kb-pc-contracts 通道状态机是否有运行时守卫强制（未逐行审计 events.rs 全部分派点） | GAP-05 闭合度 | P4-04 设计前补充专项核查 |

---

## 14. 审计结论及下一步建议

### 14.1 最终判定：**PASS WITH ISSUES**

**可以继续进入 Phase 4 领域设计（P4-01/02/03 规划不被阻断）**，因为：审计十问全部有代码级答案；不存在「无法建立现状基线」的阻断；数据近零使演进成本可控。

**但存在必须处理的问题（设计继续的同时并行处理）：**

1. **GAP-02（P1）**：P3-KB-01 已裁未落地——P4-04（KB↔Product 关联）的硬前置；需 P3 侧排期落地或 Phase 4 明示接管。
2. **GAP-01（P0，[P3] 归属）**：ai.analysis_run 双写入者——移交 Phase 3 责任人闭合（基线 §7「分别登记」机制），不阻塞 Phase 4 设计。
3. **R-02（高）**：P4-Q-04 认知上移触及 pc 已冻结模型——必须专项 Decision Record + 架构评审，禁止顺手改。
4. **GAP-08**：docproc 读面安全态势需产品负责人确认（U-2）。
5. **P4-Q-01 与 P4-Q-04 是根问题**：建议作为首批裁决项，先行启动 P4-01。

### 14.2 下一步建议（顺序）

1. 人工确认本报告 → Gate 1 正式闭合。
2. 启动 **P4-01（Product Domain Model）** 切片设计文档，同步预登记 **P4-Q-04** 的 Decision Record 骨架。
3. 将 GAP-01/GAP-02 按 Phase 4 基线 §7 分别登记进 Phase 3 问题清单。
4. P4-04 启动前完成 §13 U-1/U-2/U-4 消解。

---

## 附录 A：字段级约束与调用链证据摘要（支撑正文，供复核）

### A.1 关键表约束速查（完整取证见审计过程档案）

| 表 | PK | 归属列 | UNIQUE | 触发器守卫 |
|---|---|---|---|---|
| kb.source | id（0001:5297） | 仅 tenant | (tenant,content_hash) :5321 | trg_kb_source_immutable（UPDATE/DELETE 全拒）:6959 |
| kb.source_project | (source_id,project_id) :5305 | tenant | =PK | 无（物理 DELETE 通道存在但无路由） |
| kb.document | id :5201 | tenant+**project NOT NULL** | (tenant,project,doc_key) :5233 | 无 |
| kb.document_revision | id :5209 | tenant | (document_id,revision_no) :5213 | trg_kb_revision_guard（0006:92） |
| kb.memory | id :5241 | tenant+**project NOT NULL** | (tenant,id) | 无 |
| kb.memory_revision | id :5265 | tenant | (memory_id,revision_no)+3 个部分唯一 :6672-6686 | trg_kb_memory_revision_guard :6945 |
| kb.memory_ref | id :5249 | tenant+project | (revision,ref_role,target三元组) :5257 | trg_kb_memory_ref_immutable（追加式）:6938 |
| pc.cognition_object | id :5353 | tenant+project | (tenant,project,object_key) :5369 + singleton 部分唯一 :6854 | trg_pc_object_delete/lifecycle :6980/6987 |
| pc.cognition_revision | id :5385 | tenant+project | (object,revision_no) :5417 | trg_pc_revision_immutable :7043 |
| pc.cognition_version | id :5425 | tenant+project | (tenant,project,version_no) :5449 | trg_pc_version_delete/immutable :7050/7057 |
| projc.cognition_object | **无 PK** | tenant+project（无 FK） | 仅 singleton 部分唯一索引 :458 | trg_projc_object_delete/lifecycle :429/431 |
| projc.cognition_revision | **无 PK** | 同上 | 无 | **无任何触发器**（0025:429-444 未覆盖） |
| delivery.requirement | id :4321 | tenant+project | (project,code) :4329 | 无 |
| design.design / adr | **无 PK** | tenant+project（无 FK） | 唯一 code（0010:52/151） | 无（revision 级有 guard :114/:198） |
| plan.task_plan | **无 PK** | 同上 | 唯一 code（0011:42） | 无（revision 级有 guard :99） |
| integration.repo_analysis_report | id（0023:17） | tenant+project（**单列 FK** :18-21） | (intake_id) :33 | 无 |
| docproc.document | id（0016:42） | tenant + **project 可空** | 无 | 无 |

FK 模式注记：0001 基线为混合模式——组合键 (tenant,…,id)→core.* 为主（0001:9373/8069/9453），另有三类单列 FK（core 维度/人员/域内）；0023 反而全单列 FK；0010/0011/0025 零 FK。

### A.2 所有权判定调用链速查

- **创建归属**：tenant←`AuthedPrincipal.tenant_id`（auth.rs:166-172「所有租户业务写入的 tenant_id 来源」）；project←路径/query + `require_project_visible`（support.rs:239-250）；created_by←caller.principal_id。请求体不承载归属字段（requirement.rs:125-137 / document.rs:529-534 / source.rs:37-52）。
- **改属通道**：全仓 grep `SET project_id/tenant_id` UPDATE **零命中**；无 reassign/transfer-asset 函数。
- **关联校验**：跨资源 id 在服务层做同租户同项目校验后才可关联（adr.rs:416-431、technical.rs:384-400、discussion_memory.rs:103-143、pm_write/task.rs:444-450）；关联挂接 link/unlink 均 require_human（traceability.rs:178/229/288/341）。
- **关联生命周期**：source_project 只增不可删；requirement_source_input/finding、design_requirement 可删（物理 DELETE+审计行）；ADR 关联无 DELETE 路由；memory_ref 追加式不可删；task_knowledge_ref 整批替换。
- **AI/真人边界**：require_human_decision_context（support.rs:111-138，interactive+查库 kind='human'）；requirement/design/plan/kb 全域「AI 可 draft、真人可正式化」抽查完整；executor 红线命令 AI 拦截（executor.rs:58）。

### A.3 上游审计引用（本文不重复其内容）

authority-matrix-v1.md · four-domain-audit-report-v1.md · four-domain-registry-mapping-audit-taskbook-v1.md · docs/audit/kb-analysis-pc-2026-11-15/（9 分册）· docs/audit/ia-2026-11-16/Phase3-基线校准与裁决台账-v1.0.md

---

*审计基准 commit：`b0d8973a`。本报告为 Phase 4 规划输入，不代表任何架构决定；最终架构以逐项人工裁决及 Phase 4 Architecture Baseline 为准。*
