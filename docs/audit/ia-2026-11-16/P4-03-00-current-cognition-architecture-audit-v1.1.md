# P4-03-00 — Current Cognition Architecture Audit V1.1

- 阶段：Phase 4 · 切片 P4-03（Cognition Ownership Boundary）· 阶段 1 交付物
- 日期：2026-11-17
- 版本记录：**V1.0（2026-11-17，基于 master `268787a`）→ V1.1（审计执行期间并行会话完成血缘合并，HEAD 变更为 `e05693a`，P4-00/P4-01/P4-02 冻结文档入库；本版撤销 V1.0 的 F-1「冻结输入不可核验」结论，新增 P4-01/P4-02 冻结约束提取；代码与迁移在两血缘间零差异，V1.0 全部代码级发现原样成立）**
- 性质：**只读架构审计**。未修改任何代码、迁移、数据库、裁决文档。
- 证据基准：master `e05693a`（=P3-11 血缘 `268787a` ⊕ P4 血缘 `24c6a25` 的合并；其中 P4 血缘含冻结锚点 `002c825c`=P4-01 FROZEN、`0a0e0e12`=P4-02 FROZEN、`24c6a25`=P4-00 审计）。工作区在途文件（design.* 5M+2 未跟踪、P3-D04-01 v1.0/v1.1、P3-D04-02 v0.1）未触碰。
- 输入文档：任务书 V1.0；P3-11 裁决稿 V1.1 + 冻结令（台账 §10.8）；P3-11 交付/验收/一致性核查三报告；四域注册表 V1.3 附录；PC 领域模型基线 V1.0；P3-D04-01 调查报告（并行在途，只读引用）；**P4-00 现状审计 V1.0；Phase 4 规划基线 V1.0；P4-01 设计+DR（DR-01~06 ACCEPTED，冻结评审 PASS）；P4-02 设计+DR（DR-01~06 ACCEPTED，冻结评审 12/12 PASS）+ 一致性修订报告（F-01~F-10）**。
- 独立核验手段：migrations 全文检索、服务层源码直读、live DB 只读查询（docker exec psql，127.0.0.1:16433，`aipm_mig_full`）、两血缘 git diff。

---

## 0. 执行摘要

1. **F-1（V1.0 曾报阻塞，V1.1 撤销）**：V1.0 审计基于 master `268787a`（仅 P3-11 血缘），当时 P4-01/P4-02 冻结文档与锚点在本工作区不可见。审计执行期间并行会话完成合并提交 `e05693a`，P4 血缘（`002c825c`/`0a0e0e12`/`24c6a25`）入库，锚点解析成功。**P4-01 DR-01~06 与 P4-02 DR-01~06 均已 2026-11-17 人工逐项裁决 ACCEPTED，两份冻结评审 PASS，冻结输入完整可核验**。两血缘 diff 仅 10 个文档文件、零代码/迁移变化，故 V1.0 全部代码级发现原样有效。
2. **术语映射（F-2）**：任务书的 Product Cognition = 本仓库 `pc.*` schema（产品定位/能力/规则，18 表，14 类 identity 字典）；Project Cognition = `projc.*` schema（技术架构/约定/组件，18 表，3 类字典）。P3-11 已完成双域拆分并冻结 12 项架构基线。与 P4-00 审计、Phase 4 规划基线的语境一致。
3. **现状权威归属（F-3，P4-03 核心问题的现状答案）**：**全部认知（含产品认知）在数据层均项目私有**——pc/projc 全部表携带 `project_id NOT NULL`（0001:3044、0025:74）；PC 基线 V1.0 §2.1 明文「项目级」；Product Profile/Vision 单例**随项目创建**（bootstrap.rs:325，单例部分唯一索引 0001:6854）。全库无 Product 实体、无产品级认知主体、无任何跨项目认知共享/引用/感知机制。**P4-00 审计同判**（其 §7.2：「『唯一』在单项目内成立，跨项目不成立，N 份单例互不认账——此为 Phase 4 需裁决而非缺陷」）。
4. **P4-01 DR-07 骨架即 P4-03 的授权范围**（F-4）：三候选占位（上移 / 维持项目私有+聚合引用 / 双层混合——P4-00 曾推荐混合但**明文不视为已确认**）；已锁约束=PC 唯一权威（C-3/C-4）、人工确认不可自动化；若上移触及 pc 已冻结模型 B01~B10，必须专项 DR + 架构评审；逐项目单例骨架数据近零可直改契约。P4-01 交付给 P4-03 的=稳定主体锚点（core.product.id/product_key 不可变）+ 身份分离裁定（Product 主体 ≠ pc.product_* 认知对象，**P4-03 决定引用方式而非身份合并**）。
5. 阶段 1 完成（V1.1）。**进入阶段 2 无阻塞项**；需在阶段 2 中落实的协调项见 §9.3。

---

## 1. 审计范围与方法

| 项 | 内容 |
|---|---|
| 审计对象 | 认知领域现状：pc.*（产品认知）、projc.*（项目技术认知）、kb.*（知识库）、ai.cognition_*/ai.project_knowledge（旧旁路）、Git 知识空间（ai.project_document） |
| 核验基线 | master `e05693a`；migrations 0001~0027；`crates/pm-application/src/{pc,projc}/`、`apps/pm-api/src/{pm_cognition,knowledge_services}.rs`、`crates/pm-infrastructure/src/{repo_cognition,kb_pc_analysis}.rs`；bootstrap.rs |
| 治理文档 | P3 侧：台账 §9~§10.8、裁决稿 V1.1 §8、注册表 V1.3 附录、验收/核查/交付三报告。P4 侧：规划基线 V1.0、P4-00 审计、P4-01/P4-02 设计文档+DR 草案+冻结评审+一致性修订报告 |
| live 核验 | `aipm_mig_full` 行数实证（§4.6）；默认库 `aipm` 仅 `_sqlx_test`、无认知 schema |
| 合规 | 修改文件 0；DB 写操作 0；迁移执行 0；并行在途文件未触碰；未 commit/push |

---

## 2. 冻结输入核验 —— V1.0 F-1 的撤销记录

### 2.1 事件还原（审计过程透明性）

| 时点 | 事件 |
|---|---|
| V1.0 审计时 | HEAD=`268787a`（P3-11 血缘头）；P4 血缘的 10 个文档不在工作区，锚点 `002c825c`/`0a0e0e12` 不在本 HEAD 可达集合内 → V1.0 据实登记 F-1「不可核验」 |
| 用户反馈 | 「之前没有获取最新代码」 |
| 复查时 | 并行会话已创建合并提交 `e05693a`（parents=`24c6a25`+`268787a`，合并信息明言「两侧路径不相交，保留冻结锚点 002c825c/0a0e0e12」）→ 锚点解析为 commit，冻结文档全部入库 |

### 2.2 核验结果（V1.1）

| 任务书冻结输入 | 核验结果 |
|---|---|
| P4-01 V1.0 FROZEN @ `002c825c` | ✅ commit 存在；DR-01~06 全部 ACCEPTED（2026-11-17 人工逐项裁决，决议记录在各 DR 正文）；冻结评审 PASS（P4-01-freeze-review-v1.0.md） |
| P4-02 V1.0 FROZEN @ `0a0e0e12` | ✅ commit 存在；DR-01~06 全部 ACCEPTED（分三组裁决）；冻结评审 12/12 PASS；一致性修订 F-01~F-10 已执行且未退回任何裁决 |
| Product 归属 Team / 身份分离 / active+archived / dissolved 终态 | ✅ 与任务书摘要一致且更细（DR-01/03/04 + 补充 A/B） |
| N:M / 无主产品 / 独立 Project 合法 / Link 独立聚合实体 / 权限不穿透 / Link 生命周期分离 | ✅ 一致且更细（P4-02 DR-01/02/03/06） |
| 代码影响 | ✅ 两血缘 diff 仅 10 个 docs 文件；Product 实体在代码中仍不存在（与 P4-00「NOT IMPLEMENTED」、P4-01「冻结≠实施授权」一致） |

**结论：F-1 撤销，冻结输入完整有效。**

---

## 3. 术语映射（F-2）

| 任务书用语 | 本仓库实现 | 证据 |
|---|---|---|
| Product Cognition（PC） | `pc.*` schema（18 表）：产品定位/愿景/业务域/能力/模块/角色/场景/流程/页面/领域对象/业务规则/产品决策（14 类 identity 链字典 + 4 类 snapshot 已废弃） | 0001:10255-10268；裁决稿 V1.1 §1 |
| Project Cognition | `projc.*` schema（18 表）：repository / tech_stack / repository_module 三类技术认知字典 | 0025/0026；R-01 |
| 旧技术认知旁路 | `ai.cognition_run`/`ai.cognition_item`/`ai.project_knowledge`（CognitionPage/S01-04 通道） | P3-D04-01 D1-⑧ |
| 知识库（Team 级） | `kb.*`：source（Team 级）/ document+memory（项目强绑，P3-KB-01 已裁未落地=GAP-02 P1） | 0001:2860/2863/2904/2982 |
| Git 知识空间 | `ai.project_document` 族（正文权威=Git，现役活跃） | knowledge.rs:424-436 |
| Product 主体（任务书隐含） | **不存在**（无表/无服务/无路由/无 UI）；P4-01 已冻结其**目标**模型（core.product），实施未授权 | P4-00 §3.1；P4-01 DR-02 |

注意两条时间线：①PC 基线 V1.0 §2.2 的「Project Cognition（S01-04）」指旧旁路，P3-11 后正式技术认知权威已转移 projc.*；②P4-00 审计基于 `b0d8973a`（早于 P3-11 阶段二 `e9f66ec`），其 projc 表述（Stage 1 未接线/零 REST/写入未开）已被 P3-11 交付超越——本报告 §4.2 为合并后权威现状。

---

## 4. 认知载体全景（现状五域 + 一旁路）

### 4.1 pc.*（Product Cognition，0001:3021-3453 + 0007/0009/0027，18 表）

- **身份结构**：`cognition_object(tenant_id, project_id NOT NULL, object_type, object_key, lifecycle_status[identity/draft/active/deprecated/superseded], current_revision_id)`（0001:3041-3057）；object_key UNIQUE(tenant,project,object_key)（0001:5369）；Profile/Vision 单例部分唯一索引（0001:6854）。正文承载 `pc.document_revision.content`（PG 载体；Git 权威=已认账目标态偏差 PCGIT）。
- **单例骨架**：项目创建事务内 `ensure_product_singletons_in`（bootstrap.rs:325，PC-D-06）为每项目建 product_profile+product_vision——**同一产品在 N 个项目中有 N 份互不关联的认知单例**（P4-00 §5 同判）。
- **版本机制**：cognition_revision（行锁推号 + content_hash=sha256(markdown)，不可变触发器 trg_pc_revision_immutable 0001:7043）→ cognition_version + version_member（发布快照、predecessor 链、snapshot_hash 规范哈希、端点闭合触发器）→ write_operation 幂等账本。
- **提案链**：proposal 8 态（0001:3267），origin_kind 5 值，governance_level L1/L2；approval（pending→approved/rejected 单向、每提案一个 pending 行）；审阅流 v2 = decide 暂记、submitted/approved/rejected 三态可互转（重决新插 pending 行、旧行留痕），关联报告 closed 后决策冻结（Phase 2 冻结语义）。
- **正式化唯一通道**：`close_report` 批量发布（analysis_report.rs:548→pc::publish_version，幂等键 `pc-decision:{id}`）。
- **字典现状**：identity 链 14 类 active；snapshot 4 类 0027 废弃（fail-closed=服务层 `load_object_type` 仅取 active；DB 层无 FK/CHECK 绑定，直插 SQL 可成功=P7 探针实证，O-03 待 P3-D04 收口）；**无 product_plan**（PermanentlyExcluded，Q-PC-13）。
- **旁路对象**：source/evidence（DomainRef 溯源）、relationship 族（owner_domain CHECK IN ('pc','kb')；PC 侧无生产起草入口）、conflict 族（Legacy/Not for New Use，R-06，生产零写入者）。

### 4.2 projc.*（Project Cognition，0025/0026，18 表）

- 与 pc 完全同构（复用模式不复用业务表）；**P3-11 后已全链接通**（V1.0 审计确认，超越 P4-00 的 Stage 1 表述）：worker `derive_projc_cognition` 重挂、G-1 双源登记（`mark_analyzed` 单事务硬门）、P5 决策端点 `POST /v1/pm/projc/proposals/{id}/decide`、close 双域分流发布（幂等键 `projc-decision:{id}`，独立失败计数）。
- 差异点：字典 3 类；origin_kind 六值（增 `repo_analysis`）；write_operation 仅 2 命令值（缺 `projc_commit_relationship_revision`，P1-C）；relationship_type owner_domain CHECK 恒 `'project_cognition'`，种子 implements（repository_module→pc.feature_module 跨域 ext 键）+ depends_on；conflict_type 八值含 `cross_domain_authority`（0026:148）；conflict 六态 detected/under_review/awaiting_human→resolved/dismissed→reopened（resolve/dismiss 须带 note）；**零 FK 且 cognition_revision 无不可变触发器**（pc 有 trg_pc_revision_immutable——约束强度不对称实证，P2-F 关联）；conflict 服务已实现、生产登记入口未建（O-06）。
- 发布事务=pc §18.1 十四步同构（advisory→账本幂等→FOR UPDATE 消费 approved→审批哈希三方比对→CAS 失配提案内置 failed→L3 冻结→成员集构建→snapshot_hash 先算后插→闭合断言→审计→outbox→applied+completed→生命周期推进→commit）。
- 展示面：分析报告详情 projc_* 字段（W6）；CognitionPage 后端对 projc **0 引用**（grep count=0）。

### 4.3 kb.*（Team 级知识库）

- `kb.source`：tenant_id 无 project_id（0001:2982-2994），Team 级唯一认知资产；source_project 多对多挂项目（只增不可删）。
- `kb.document / kb.memory`：**project_id NOT NULL 行级强绑**（0001:2863/2904）——与 P3-KB-01「Team 级所有权」裁决不一致=**GAP-02（P1，已裁未落地）**，裁决方知情登记；P4-04 的硬前置。
- kb.memory：全库最严守卫（触发器白名单+K4 CAS+行锁）；生成侧缺位。

### 4.4 旧认知旁路（S01-04）

- `ai.cognition_run → cognition_item`（6 态一次定型）→ confirm 事务 INSERT `ai.project_knowledge`。真人门已有（pm_cognition.rs:295-296 SEC-P1-1）；**无修订/无版本链/无证据/无审计落痕**（P0-A/O-05）。
- `ai.project_knowledge` 生产写入者唯一（repo_cognition.rs:265）；live 0 行；读取方 5 处。D04-01 给出 **REPLACE** 建议，待 O-05 裁决。

### 4.5 Git 知识空间

- `ai.project_document` 族，正文权威=Git；提案 8 态+redline confirm 真人门；与 kb/pc/projc 零互通。

### 4.6 live 行数实证（aipm_mig_full，只读）

| 表 | 行数 | 说明 |
|---|---|---|
| pc.cognition_object | **1** | `repository / draft`、project_id 非空——snapshot 存量遗留行（0027 注释「≈0 行」不实） |
| pc.cognition_version | 0 | 无历史 snapshot 正式版本 |
| projc.cognition_object / cognition_version | 2 / 1 | 迁移验证种子 |
| ai.project_knowledge | 0 | 旧通道空 |

（部分闭合 P4-00 U-1「运行库状态未检视」：本审计获只读 DB 授权口径，验证库已实证；生产运行库状态仍以运维确认为准。）

### 4.7 权威边界（写入面矩阵摘要）

- **AI/worker 可写（产物恒非正式）**：pc/projc 对象/关系/来源/证据草稿、kb.document draft、ai.change_proposal、pc/projc.proposal（止于 submitted）。
- **人工门（AI/服务恒 403，fail-closed）**：pc/projc decide+publish+冲突处置（真人 `kind='human'` + A3 闸）；kb confirm/memory review；知识提案 confirm/veto；requirement confirm；design approval；ADR deprecate；mark-tasks-ready；cognition item decision（SEC-P1-1）。
- **审计缺口**：S01-04/S01-05 确认链无审计落痕（P0-A，O-05）；`ai.analysis_run` 双写入者（P4-00 GAP-01 P0，[P3] 归属，移交 Phase 3）。

---

## 5. 所有权与权限边界现状

### 5.1 所有权矩阵（数据层实证）

| 认知资产 | tenant_id | project_id | 所有主体（现状） |
|---|---|---|---|
| pc.* 全 18 表 | ✓ | **NOT NULL** | 项目（产品认知内容项目私有） |
| projc.* 全 18 表 | ✓ | **NOT NULL** | 项目 |
| kb.source | ✓ | 无列 | **Team** |
| kb.document / memory | ✓ | NOT NULL | 项目（GAP-02：与 P3-KB-01 裁决不一致，已裁未落地） |
| ai.project_knowledge | ✓ | ✓ | 项目（旧旁路） |
| ai.project_document | ✓ | ✓ | 项目（Git 知识空间） |
| Product / Product–Project Link | — | — | **不存在**（P4-01/02 已冻结目标模型，实施未授权） |

### 5.2 跨域授权现状

- DomainRef 四字段契约仅承担溯源/引用表达，无授权语义；registry 通道 `ProjectCognition=ReadOnly`（红线二）。
- 跨域权限闭环原则（P3-11 冻结令第 12 项）：调用方授权 + 目标领域授权 + 目标对象当前状态约束三者齐备；现状完整 resolver=Partial（O-01 归 P3-10）。
- 权限底座：core.principal/tenant_member（is_admin）/project_member + core.approver_grant（A3 闸锚，approver.rs:37-79，Lead=role 'owner'，grant/revoke Lead 专属）。**无任何「Product 成员」概念**；tenant 级权限唯一现役点=LLM Provider 管理面（team_llm.rs:222-224，P4-00 GAP-10）。

### 5.3 AI Agent 授权边界现状

- AI 主体对一切正式写入恒 403；提案/草稿面不判主体身份（产物恒 draft、lifecycle 永不入 active）。
- pc/projc decide 端点均叠加项目成员 guard + `ensure_trusted_context`（API Key 403，knowledge_services.rs:613/663）；报告关闭要求 `auth_mode='interactive'`（analysis_report.rs:801-808）；withdraw 特例=提案人本人 OR 真人+A3；MCP 确认类工具服务层强制真人但权限位口径不一致（P2-H，D5-⑻ 待裁）。
- AI 跨域分析（kb_pc_analysis worker）读仓库/KB、写本域草稿与提案；无跨域写权；`pc.endpoint_in_scope` 硬编码四域未含 projc（P1-D）。

---

## 6. 对 P4-03 的硬约束提取

### 6.1 来自 P3-11 冻结令（台账 §10.8，12 项）与裁决稿 V1.1

| # | 约束 | 对 P4-03 的含义 |
|---|---|---|
| H1 | 双域权威五条（不共享身份/状态机/确认权威/正文权威；跨域只走引用契约） | Product Cognition 无论归属谁，pc/projc 身份与状态机不得合并为无边界对象（与任务书 §六 一致） |
| H2 | DomainRef 四字段契约 + 与 owner_domain 职责分离；引用完整性细则 O-02 归 P3-D04 | Product↔Project 认知引用必须走 DomainRef 形态；引用≠授权（红线二） |
| H3 | 跨域权限闭环原则 | DR-08 只能在此框架内细化 |
| H4 | 分流判据（R-03） | PC 上移与否不改变对象域标定，变的是 PC 的 owner 主体 |
| H5 | Conflict 归属原则（R-06）：冲突对象归主要冲突认知所属域；对方经 DomainRef 引用、各自履行确认责任；无 Global Conflict 域 | 跨 Product/Project 认知冲突按此落位 |
| H6 | 统一检索三层（R-08）+ 结果元数据要求 | 跨项目认知聚合归检索层聚合而非共享权威 |
| H7 | Legacy Transitional（R-10）+ pc.conflict Legacy | 迁移影响研究沿用「不搬运、不伪装迁移」纪律 |
| H8 | 人工正式确认原则 | DR-04/DR-07 保留真人确认门 |
| H9 | 有界过渡 + 不双写 | 实施过渡不得引入 pc/projc 双写 |

### 6.2 来自 P4-01 冻结（`002c825c`，DR-01~06 ACCEPTED + DR-07 骨架）

| # | 约束 | 对 P4-03 的含义 |
|---|---|---|
| H10 | DR-01：Product 归属 Team（领域语义）、物理承载 tenant_id；**V1 不变量：Product 不能活得比 Team 久；不存在跨 Team 产品转移**；Team 解散→Product 置 `dissolved`（系统终态，不可恢复，仅 dissolve_team 清扫写入） | DR-09 的认知处置必须建模 Team 解散场景；dissolved Product 名下认知的语义需 P4-03 定义 |
| H11 | DR-02：core.product（UUID PK + UNIQUE(tenant_id,id) 组合键 + 重 FK 惯例） | 未来认知引用的锚点形态=core.product.id（主体锚），非认知对象 id |
| H12 | DR-03：product_key 调用方提供、租户内唯一、不可变；**产品身份与 PC 认知对象身份严格分离——P4-03 决定引用方式而非身份合并** | P4-03 DR-02 的直接边界条件 |
| H13 | DR-04：active/archived + 可恢复（纯主体态翻转，不触发关联域任何自动化动作）+ 无硬删；**四生命周期（Product/Project/Association/PC Version）严格分离、互不驱动（无传播原则，双向锁定）——对 P4-02/03/04 生效** | DR-09 核心：Product 归档/恢复不得自动改认知；PC 版本生命周期是四生命周期之一，P4-03 必须给出其与 Product 主体的联动边界（哪些是「关联自身生命周期对锚点终态的响应」、哪些是禁止的传播） |
| H14 | DR-06：Product 权限=创建 tenant owner/admin；变更/归档/恢复/移交=Product Owner∪tenant admin；**列表最小字段集全租户成员、详情 Owner∪admin 其余 403**；无 product_member 表；**Product↔Project 双向不穿透** | DR-08 的现成框架：产品认知若随 Product 上移，其读取权限必须落在 Product 权限模型内（详情受限层），不得因 Project 成员身份自动获得 |
| H15 | DR-07 骨架（P4-03 授权范围）：三候选占位；已锁=PC 唯一权威（C-3/C-4）、人工确认不可自动化；**上移触及 pc 已冻结模型 B01~B10 → 专项 DR + 架构评审，禁止顺手改**；逐项目单例骨架数据近零可直改契约；P4-00 混合推荐不视为已确认 | P4-03 阶段 2/3 的工作方式契约 |

### 6.3 来自 P4-02 冻结（`0a0e0e12`，DR-01~06 ACCEPTED + F-01~F-10 修订）

| # | 约束 | 对 P4-03 的含义 |
|---|---|---|
| H16 | DR-01：N:M、V1 无 is_primary、独立 Project 合法 | 认知共享模型必须支持「一 Product 对 N Project」与「独立 Project（无产品认知关联）」两态 |
| H17 | DR-02：`core.product_project_link` 独立聚合实体（id + active/ended + ended_by/at/reason，部分唯一 active，重建=新行，ended 行四禁守卫整体冻结）；C-01 前置=core.project 补 UNIQUE(tenant_id,id) | 一 Product 多 Project 的**关系底座已定**（P4-02 §14 明言「P4-03 以此为关系底座，不得回改」）；认知引用可与 Link 状态联动但不得复制其模式歧义 |
| H18 | DR-03/05：关联操作=双侧权限交集（单一执行者）；读写分离（读≠写交集）；数据最小化 | 产品认知跨域读取若建，同样须「读≠写」与最小化原则对齐 |
| H19 | DR-06：锚点终态（Team dissolve）清扫同事务将 active Link 置 ended（reason=team_dissolved）；**archived 不动链接**；无传播原则约束主体状态，链接终态化属「关联自身生命周期对锚点终态的响应」 | DR-09 的先例：认知处置可类比「Link 层对锚点终态的响应」，但**不得**据此自动改写认知正文/版本（那属传播） |

### 6.4 实现事实（当前为真、不在冻结范围、可被 P4-03 DR 改变）

| # | 事实 | 出处 |
|---|---|---|
| J1 | PC scoping=项目级（project_id NOT NULL；Profile/Vision 随项目创建；P4-01 DR-07 已登记为待裁靶心） | PC 基线 V1.0 §2.1/§5.2 + bootstrap.rs:325 + P4-00 §7.2 |
| J2 | 无 Product 实体、无产品级认知主体 | migrations 全检 + P4-00 §3.1 |
| J3 | 无跨项目认知共享/引用/派生/感知机制 | 代码与迁移全检 + P4-00 §6 矩阵 |
| J4 | ai.project_knowledge 旧通道现役但空转（0 行、无审计） | D04-01 D1-⑧ + live 实证 |
| J5 | projc 零 FK、cognition_revision 无不可变触发器、词表缺值、字典无 DB 约束（O-03/O-04/P1-C/P2-F） | D04-01 + 本审计 0025/0026 复核 |
| J6 | pc.relationship PC 侧无生产起草入口；pc.endpoint_in_scope 未含 projc | 注册表 V1.3；D04-01 P1-D |
| J7 | 全库引用链零真实数据（pc.source/kb.memory_ref 近 0 行）——机制在位、从未承载业务负载 | P4-00 §6 引用健康度 |

**判据说明**：J1 的项目级 scoping 是 PC-01b 领域模型基线的一部分，属「已裁决」——但 P4-03 的任务本身就是对该 scoping 重新裁决（任务书 §二 明令两方案都须接受论证；P4-01 DR-07 亦如此登记），因此列入 J 而非 H；P4-03 DR 须显式引用并修订 PC 基线对应条款，不得静默覆盖。

---

## 7. P4-03 核心问题的现状答案（§3.2 九问对照）

| 任务书问题 | 现状答案 | 证据 |
|---|---|---|
| 1. Product 是否拥有独立 Cognition 身份？ | **否**——Product 实体不存在；产品认知对象身份锚定 (tenant, project, object_type, object_key)；P4-01 DR-03 已裁「未来引用锚=core.product.id，身份不合并，引用方式由 P4-03 决定」 | §2、§4.1；P4-01 DR-03 |
| 2. 一 Product 的多 Project 是否共享同一产品认知？ | **不共享且无法共享**——每项目独立 pc.* 实例 + 独立 Profile/Vision 单例；两项目建同一产品=两套互不认账的产品认知 | bootstrap.rs:325；0001:6854；P4-00 §5 |
| 3. Project Cognition 可否引用 Product Cognition？ | **无对象级引用机制**——projc→pc 仅有 implements ext 键读语义引用（关系端点 `pc.feature_module`）+ projc.source source_kind='product_cognition'（四字段软引用） | 交付报告 I-8；0025:246-266 |
| 4. 引用是版本固定还是跟随最新？ | **不存在该机制**——DomainRef revision_context 钉扎仅用于 source/evidence 溯源；P4-00 建议未来引用钉 cognition_version.version_no（P4-05 语境，未裁决） | 裁决稿 §3；P4-00 §12 P4-05 |
| 5. 产品认知更新后项目如何感知？ | **无机制**——无跨项目事件/订阅/diff 广播；outbox 事件均为项目内 | 事件目录（addendum §1） |
| 6. 项目可否维护产品认知局部差异？ | **无此概念**——不存在「产品认知基线 vs 项目派生」分层 | — |
| 7. 局部差异会否被误认产品正式认知？ | 现状风险相反：各项目产品认知互不可见、无混淆通道；但同一产品多项目时必然出现「两套都自称产品认知」的语义冲突（P4-03 靶心） | 推论自 J1/J2 |
| 8. Project 不关联 Product 时认知是否完整可用？ | **现状即如此**——全部认知项目私有，独立 Project 完整可用（P4-02 DR-01 明确独立 Project 合法） | J1/J3；P4-02 DR-01 |
| 9. 解除 Product 关联时引用如何处置？ | **问题不存在于现状**——无 Product 关联可解除；P4-02 已定 Link 终态语义（ended 行保留、不复活），认知侧处置归 P4-03 DR-09 | J2；P4-02 DR-02/06 |

## 8. 冲突处理与反馈机制现状（§3.4 对照）

- 冲突发现/记录：pc.conflict（项目内 Legacy 零写入）+ projc.conflict（项目内、服务实现、无生产入口 O-06）+ discovery.finding + ai.change_proposal。
- 冲突范围：**全部项目内**。两个 Project 对同一产品能力形成不同认知——现状下连「发现」都不可能（互不可见）。
- 实施发现反馈产品认知：仓库分析→pc identity 链（D-4 身份综合，业务语义限定）+ snapshot 类已停派；**无跨项目归纳通道**（AI 从多项目归纳产品层候选=现状无载体）。
- 正式链六步（发现→记录→候选修订→人工裁决→正式发布→项目侧同步）：pc/projc 项目内六步齐备；**「项目侧同步」一步在任何域都不存在**。

---

## 9. 差距与未决问题

### 9.1 现状模型 vs 任务书目标问题的差距（描述性，不含目标模型结论）

| 差距 | 现状 | 任务书要求的可论证空间 |
|---|---|---|
| G1 | 产品认知主体=项目 | 三方案论证（任务书 §3.1；P4-01 DR-07 占位一致） |
| G2 | 无 Product 认知身份与版本 | DR-01/DR-02/DR-04 待裁（锚点已由 P4-01 DR-02/03 提供） |
| G3 | 无跨项目共享/引用/感知/派生机制 | DR-03/DR-04 待裁（Link 底座已由 P4-02 提供） |
| G4 | 无产品级候选归纳与冲突裁决流程 | DR-06/DR-07 待裁 |
| G5 | 无产品级权限模型（Product 权限已由 P4-01 DR-06 冻结，认知读取如何挂接待裁） | DR-08 待裁 |
| G6 | 无生命周期处置规则（P4-01/02 已裁主体与 Link 终态；认知侧联动待裁） | DR-09 待裁 |
| G7 | 旧通道 S01-04 空转占位 + ai.project_knowledge 死列 | 与 O-05 裁决联动 |
| G8 | DomainRef resolver Partial、引用完整性语义未闭合（O-01/O-02/O-09） | Product↔Project 引用模型的设计前提 |
| G9 | kb.document/memory 项目强绑 vs P3-KB-01 Team 级裁决（GAP-02 P1） | Product 级认知与 KB 关联建模前的硬前置（P4-04 前置，P3 侧落地或 Phase 4 明示接管） |

### 9.2 尚未裁决的关键问题（阶段 2 逐项转化为 DR）

1. PC 正式权威归属（项目 vs Product vs 分层）——DR-01（P4-01 DR-07 已登记选项占位与禁止事项）。
2. Product 认知身份/版本主体形态——DR-02/DR-04（H12 边界内）。
3. 多 Project 共享与引用模型（含版本钉扎 vs 跟随；Link 底座联动）——DR-03。
4. 项目特有/派生认知与正式认知边界——DR-05。
5. 跨项目冲突处理与 AI 多项目归纳的正式化流程——DR-06/DR-07（H5 框架内）。
6. 跨域权限模型（挂接 P4-01 DR-06 两层可见性）——DR-08（H14/H18 框架内）。
7. 解除关联/归档/解散的认知处置——DR-09（H10/H13/H19 框架内；区分「响应」与「传播」）。
8. 与 O-05/O-01/O-02/O-06/O-09 及 P3-D04-01 D5 新待裁项 ⑴⑵⑶（项目知识归属/CognitionPage 去留/AI 上下文源切换）的归口协调——不重开，但 DR 须与裁决结果兼容。

### 9.3 阶段 2 前置事实项（设计输入，非裁决）

- P-1：pc/projc 机制完全同构且均项目私有——任何「上移」方案是对模式实例的 owner 重锚，而非机制重建（迁移红利窗口：pc 核心 0 行、projc 种子级、引用链零负载 J7）；单例骨架数据近零可直改契约（P4-01 DR-07 已认定）。
- P-2：审阅流 v2 + close 批量发布 + G-1/G-2 双域分流是已验收工程契约；DR 方案不得与 close 契约冲突（任务书 §六：不修改 P3-11 已冻结正式链）。
- P-3：GAP-02（P1）kb Document/Memory 项目强绑未落地 Team 级——P4-03 设计 Team/Product 级认知边界时必须显式声明与 P3-KB-01 落地计划的关系，避免第三个「半 Team 级」权威。
- P-4：P3-D04-01（在途）REPLACE 建议与十项新待裁问题中 ⑴⑵⑶ 与 P4-03 的 PC 归属裁决强耦合，须协调裁决顺序。
- P-5：P4-00 曾推荐「权威上移+项目级派生视图」混合方案（§12 P4-Q-04），**明文未确认**——阶段 2 须以候选方案身份平等论证，不得引用为依据。

---

## 10. 结论：是否具备进入阶段 2 的条件

| 判定项 | 结论 |
|---|---|
| 现状模型、代码证据、数据库结构、权威边界 | ✅ 完备（§4~§5，附文件:行号与 live 实证；与 P4-00 交叉印证） |
| P3-11 约束提取 | ✅ 完备（§6.1，H1~H9） |
| **P4-01/P4-02 冻结输入** | ✅ **完整可核验（V1.1 §2；锚点有效、12 项 DR 全 ACCEPTED、两份冻结评审 PASS）** |
| P4-01 DR-07 骨架（P4-03 授权范围） | ✅ 已取得（H15；选项占位/已锁约束/依赖） |
| 差距与未决问题 | ✅ 完备（§9，G1~G9 + DR 化清单） |
| 实施授权 | 无（遵守任务书；未动代码/迁移/DB） |

**结论：阶段 1 完成（V1.1），无阻塞项，具备进入阶段 2（Cognition Ownership Boundary Design）的条件。** 阶段 2 须以 H1~H19 为约束框架、J1~J7 为现状基线，对三候选方案平等论证；DR 输出后逐项等待人工裁决。

---

## 附录 A：证据索引

| 主题 | 位置 |
|---|---|
| pc.* DDL（project_id NOT NULL、8 态提案、14+4 字典、单例索引） | migrations/0001_baseline.sql:2860/2982/3021-3453/3240-3270/5369/6854/10255-10268 |
| projc.* DDL 与 bootstrap | migrations/0025_projc_domain_bootstrap.sql:71-（project_id NOT NULL；触发器清单 429-444 不含 cognition_revision）；0026_projc_formal_chain.sql（vm_closure/anchored 触发器、conflict_type 含 cross_domain_authority :148） |
| snapshot 字典废弃 | migrations/0027_pc_snapshot_dictionary_deprecated.sql（注释「≈0 行」与实测 1 行不符） |
| Profile/Vision 单例随项目创建 | bootstrap.rs:325（P4-00/PC-D-06 同证）；pc/mod.rs 头注 KP-2 |
| 真人门/A3/L3 冻结头注 | crates/pm-application/src/pc/mod.rs:1-30；crates/pm-application/src/projc/mod.rs:1-30；approver.rs:37-79 |
| close 双域发布 | crates/pm-application/src/analysis_report.rs:548-663（pc）、追加块（projc，e9f66ec） |
| decide 端点守卫 | apps/pm-api/src/knowledge_services.rs:613（pc）/663（projc） |
| 旧通道无审计 | apps/pm-api/src/pm_cognition.rs:282-320（真人门 :295-296）；crates/pm-infrastructure/src/repo_cognition.rs:162-308 |
| P3-11 治理链 | 台账 §9/§10.6-10.8；裁决稿 V1.1 §8；注册表 V1.3 附录 §1~§6 |
| P4 冻结文档 | docs/architecture/phase4/{Phase4-…规划基线, P4-01-…×4, P4-02-…×4}.md；docs/audit/phase4-2026-11-16/P4-00_….md；冻结锚点 002c825c/0a0e0e12 |
| live 行数 | 本报告 §4.6（docker exec psql，aipm_mig_full，只读） |

## 附录 B：合规记录

- 修改文件 0（本报告为新增交付物；未触碰并行会话在途文件——design.* 5M+2、P3-D04-01 v1.0/v1.1、P3-D04-02 v0.1）。
- 数据库写操作 0；仅 docker exec psql SELECT（127.0.0.1:16433，遵守 AGENTS.md 规则 3）。
- 未 commit、未 push——交付物落盘待人工审查（沿 P3-D04-01 先例）。
- 子代理只读审计 2 路（projc 正式链代码完整完成；认知权限面因并发上限中断，其证据点已由本报告直读核验覆盖），不影响结论完整性。
- V1.0→V1.1 变更：F-1 撤销（§2）；新增 §6.2/§6.3（P4-01/P4-02 冻结约束 H10~H19）；§4 补 bootstrap 单例证据与 projc 触发器不对称；§7/§9 补 P4-00/P4-01/P4-02 交叉引用；§10 阻塞项移除。代码级发现无一处变更（两血缘 diff 仅 docs）。

**报告完。等待人工审查；未经裁决不进入阶段 2（方案比较与目标模型设计）。**
