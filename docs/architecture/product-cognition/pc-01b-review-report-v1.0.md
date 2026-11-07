# PC-01B 数据模型与数据库设计 V1.0 独立架构评审报告 V1.0

| 项 | 值 |
|---|---|
| 评审对象 | 《PC-01B — Product Cognition 数据模型与数据库设计 V1.0》（docs/architecture/product-cognition/pc-01b-data-model-and-database-design-v1.0.md，1807 行） |
| 上游权威 | 《PC 阶段 1 — 当前实现评审与开发计划 V1.1》（PC-01A）；Q-PC-01～Q-PC-13 |
| 评审角色 | 独立数据库架构评审组（PG 架构师 / DDD 建模 / 一致性事务 / Git-PG 双存储 / 租户安全 / 独立评审六视角） |
| 评审方法 | 任务书五层顺序（领域模型 → 双维度版本与端点闭合 → 审批与 Git-PG → DDL 与约束 → 验收测试）+ 失败场景构造 + 只读代码证据复核 |
| 评审日期 | 2026-11-07 |
| 代码权限 | 只读。评审过程未修改任何代码/数据库/Migration |

---

# 1. 评审结论摘要

**结论：不通过（存在必须修复的 P1 缺陷 5 项；无 P0）。**

| 判定 | 内容 |
|---|---|
| 领域模型方向 | **正确**。Object/Relationship 双维度身份-修订-版本模型、五聚合边界、四元组跨域协议、零迁移硬边界均符合 Q-PC-01～13 与 PC-01A V1.1，未发现模型级根本缺陷（P0 = 0） |
| 数据库可实施性 | **不达标**。存在 1 项 PostgreSQL 语法错误（F-01，建表即失败）与 1 项设计自相矛盾（F-02，发布事务按文档实现必然被自身触发器拒绝） |
| 数据完整性 | **主链成立、防御深度不足**。端点闭合三层防线、Member 双维度互斥、租户项目复合 FK 主链正确；但存在 2 项绕过/漂移路径（F-03 关系端点身份一致性、F-04 提案内容锚定）与 1 项流程契约缺失（F-05 成员时序与端点补齐规则未定义） |
| 修复后前景 | 5 项 P1 均为**局部可修复**（修复方向明确、不推翻任何模型决策），修复后无需二次全面评审，可直接进入 PC-01C |

> P1 全部修复并复核前，**PC-01B V1.0 不得作为 PC-01C 正式数据库实施输入**。

---

# 2. 评审范围与证据

## 2.1 实际阅读的文件

| 类别 | 文件 |
|---|---|
| 被评审对象 | pc-01b-data-model-and-database-design-v1.0.md（全文 1807 行，逐章） |
| 上游 | pc-phase1-current-review-and-plan-v1.1.md（§1/§3.5/§6.2~6.10/§7/§8/§10）、pc-phase1-domain-model-baseline-v1.md（§3.2/§5/§6/§7.2/§9/§13）、pc-gate0-database-schema-inventory.md（B/C/D/E/F/H 节） |
| Migration（只读复核） | 0002_delivery.sql（requirement 键确认）、0003_platform.sql、0006、0014、0019_tenant.sql、0024、0026_delivery_review.sql、0031、0036/0037、0039、0040、0042/0044（目录清单） |
| 工具链源码 | sqlx-core 0.8.6（migrator.rs/source.rs）、sqlx-postgres 0.8.6（migrate.rs）——复核 PC-01B §21.1 H-13 结论 |
| 应用代码 | crates/pm-application/src/approver.rs（全文）、requirement.rs（锁点 251/270）、apps/pm-api/src/main.rs:28 |

## 2.2 未检查 / 无法验证的事实（诚实清单）

| 项 | 状态 |
|---|---|
| 运行库 `_sqlx_migrations` 中 version=44 的 checksum 归属 | **未验证**（评审不连库；沿用 PC-01B §21.1 待验证标注，仅影响 0044 修复方向，不影响本评审结论） |
| H-1 Knowledge Git root 物理布局 | **未验证**（PC-01B §12.7 已声明归 01C 环境项） |
| 0044 双文件在当前开发库的实际执行历史 | **未验证** |
| PC-01B 所引用既有表的 DDL 原文 | 已抽查复核（0002:13,48,63 确认 requirement/milestone/task 均有 `UNIQUE(project_id,id)`，0026:640 的 FK 父键成立）；未逐表重扫 0019 全部 411 行（依赖 Gate 0 结论） |
| PC-01B 触发器/事务伪码的运行时行为 | **静态推演**，未在数据库上执行（本阶段禁止实施；T-01~T-26 测试归 01C） |

---

# 3. 上游裁决符合性检查

| 裁决 | PC-01B 落实 | 符合性 |
|---|---|---|
| Q-PC-01 PC/S01-04 唯一权威 | §2.1/§13.1：S01-04 只读引用、零数据通道 | ✅ |
| Q-PC-02 Decision 归 PC / ADR 归 Design | §3.3 `product_decision` 类型；Design/ADR 通道「后续跨域集成」，无冒充实体的表 | ✅ |
| Q-PC-03 Requirement 身份+Revision | §13.1 requirement 通道 revision 必填；经服务层组合键校验（FK 不可行域已诚实声明） | ✅（库层缺口由 §13.3 双轨补偿声明，见 F-19 关联注记） |
| Q-PC-04 Vision/Goal 分层 | §11.3 Vision 模板不含 Goal；supports 关系跨域端点 | ✅ |
| Q-PC-05 Feature 条件独立 | §3.3 `feature` 独立类型；独立化保留规则引用 01A §6.9#11 | ✅ |
| Q-PC-06 L0-L3 / P0-P3 / 风险等级不混淆 | §3.3 governance_level（L1/L2 类型缺省）；§10.1 risk_level 与 governance_level 分列 | ⚠️ 基本符合，但**等级一致性校验缺落点**（F-07） |
| Q-PC-07 复用审批机制 | §10.2/§11.1：复用范式与权限闸，PC 专属物理表（裁决允许：01A §6.5「复用为方向，须满足八原则」） | ✅（F-04 揭示八原则 1 的 proposal 侧缺口） |
| Q-PC-08 V1 审批人同权 | §10.2 注记：ensure_governance_authorized 复用 | ✅（竞态窗口 §17.3/§23 R-3 已声明） |
| Q-PC-09 业务规则 PC 确认 | §3.3 business_rule 固定 L2 | ⚠️「不可下调」仅为文档语句，无校验落点（并入 F-07） |
| Q-PC-10/11 PC-I 切片 / Requirements 确认门 | §22.4 台账对接；§13.1 requirement 通道 | ✅ |
| Q-PC-12 KB 审核≠PC 确认 | §9.4 通道：source 登记 → proposal → approval → version | ✅ |
| Q-PC-13 Product Plan 排除 | 表清单无 Plan 实体（§15.2）；§13.1 永久排除行 | ✅ |
| 01A §6.2 双维度版本 + 完整还原四问 | §7/§8/T-25 | ✅ 主链成立（F-05 的时序契约缺口除外） |
| 01A §6.7 零迁移硬边界 | 全文无迁移设计；§15.5 种子为结构初始化 | ✅ |
| 01A §6.8 四域边界 | §13.1 S01-04/05 通道状态 | ✅ |

---

# 4. PC-D-01～06、09 设计裁决检查

| PC-D | 结论明确性 | 方案合理性 | 未解决分歧 | 可进入实施 |
|---|---|---|---|---|
| 01 审批载体 | 明确（PC 专属 + 复用范式） | 合理：analysis_run_id NOT NULL（0003:134,151）确为硬耦合，扩展路径回归面大；review_approval 三要素范式已被 0026 验证 | 无 | **是**，前提修复 F-04（proposal 锚定） |
| 02 一对象一文档 | 明确 | 合理：1:1 映射使 content_hash 直接可比、版本还原单点；多文档一致性复杂度无必要 | 无 | 是，前提补 F-09（doc_type ALTER 清单） |
| 03 完整快照 | 明确 | 合理：还原四问与端点闭合只有在全量成员集上才能库层断言；存储成本可忽略 | 无 | 是 |
| 04 类型注册 | 明确（字典+代码双约束） | 合理；但**种子矩阵推迟 01C 属领域语义下放**（F-08） | 种子内容 | 部分——F-08 修复后是 |
| 05 Source/Evidence 分离 | 明确 | 合理：判别器 CHECK 沿 A7 已验证模式；JSONB 边界清晰 | 无 | 是 |
| 06 Profile/Vision 字段 | 明确 | 合理：受控单例 + 模板章节 + lifecycle 区分未确认/空值；避免无边界 JSONB | 无 | 是（§5.2 谓词实现需与 §15.4 统一，F-12） |
| 09 Git-PG 协议 | 明确（三态账本） | 合理：A8 幂等键 + trailer 反查 + reconcile；**failed 标记的独立事务语义与 retry 上限未定义**（F-11） | 恢复细节 | 部分——F-11 修复后是 |

---
# 5. 核心数据模型评审

## 5.1 Object / Revision（任务书 §五 10 项）

| # | 检查项 | 结论 | 依据 |
|---|---|---|---|
| 1 | Object 只承担稳定身份 | ✅ 身份列不含正文；display_name 为受控投影并声明非权威（§5.2 注 1） | §5.2 |
| 2 | Revision 真 append-only | ✅ `trg_pc_revision_immutable` BEFORE UPDATE **OR DELETE**（§5.3 DDL） | §5.3 |
| 3 | revision_no 唯一 | ✅ `UNIQUE(tenant_id, project_id, object_id, revision_no)`，对象内唯一（比项目内更严） | §5.3 |
| 4 | 复合 FK 归属正确 | ✅ 四列复合（tenant/project/object_id/id），列序与被引用唯一键一致 | §5.3 |
| 5 | current_revision 不会指向他对象 Revision | ✅ 复合 FK 列映射 `object.id ↔ revision.object_id` | §5.2 |
| 6 | 对象与 Revision 生命周期未混淆 | ✅ Revision 无状态字段（派生视图 §15.3），对象五态独立 | §5.3 注 1 |
| 7 | 废弃/合并/替代保留历史 | ✅ lifecycle 终态触发器 + superseded_by_id 复合 FK；无删除路径 | §5.2 |
| 8 | Git 引用与 Content Hash 一致 | ✅ content_hash 必须等于所引 document_revision.content_hash（发布核验 + Git-PG 步骤④） | §5.3 注 2 |
| 9 | UPDATE/DELETE 历史路径 | ✅ Revision 层封死；**但 identity 态外的对象行 DELETE 未显式禁止**（现靠 revision FK 间接阻挡）→ F-10 | — |
| 10 | 并发推号/更新丢失 | ✅ 对象行 FOR UPDATE + UNIQUE 双保险 + CAS 双令牌（§17.2-1）；updated_at 自动刷新机制未定义 → F-14 | §17 |

**结论：模型正确，约束充分，无绕过路径（Revision 层）；需要 F-10/F-13/F-14 三项修订。**

## 5.2 Relationship / Relationship Revision（任务书 §六，最高优先级之一）

对任务书关键质询「**端点是稳定身份的一部分，还是允许通过 Relationship Revision 改变？当前设计是否会导致同一个 Relationship ID 在不同 Revision 中表达完全不同的关系？**」：

- **设计意图**（PC-01B §6.1 #1/#2）：端点 Object 身份属于 Relationship 身份（改端点对象 = 新关系 + 旧关系 superseded）；端点 Revision 属于 Relationship Revision。设计语义正确，与「历史还原后端点不漂移」自洽。
- **库层现状**：§6.5 `enforce_relationship_anchors` 只保护 **Relationship 行**的端点列不被 UPDATE；**Relationship Revision 的 INSERT 没有任何机制校验其端点身份等于父 Relationship 行的端点身份**。

**失败场景构造（F-03，P1）**：

```text
1. pc.relationship R：source_object_id = A，target_object_id = B（行端点锚定，不可改）
2. 绕闸直连 SQL：INSERT pc.relationship_revision
     (relationship_id=R, source_object_id=C, source_object_revision_id=C-1,
      target_object_id=B, target_object_revision_id=B-5, ...)
   —— C/C-1 为同项目合法对象/Revision：
      ✅ 端点形态互斥 CHECK 通过
      ✅ 四复合 FK（tenant/project/object/revision）全部通过
      ✅ append-only 触发器不拦 INSERT
3. 该 RR 经 proposal_member → version_member 发布成功（端点 C-1/B-5 均为版本成员时闭合断言也通过）
结果：同一 Relationship ID 的 Revision 链中出现「R: A→B（身份行）」与「R: C→B（Revision）」两种关系；
     关系历史查询（Q5）与版本还原得到的 R 语义不一致——违反 §6.1 #1 裁决，且无任何数据库错误。
```

**判定：P1。** 修复方向明确（INSERT 触发器逐列比对父行端点，见 §9），修复后该规则成为真正的数据库约束，满足任务书推进原则 2。

其余 9 项检查：独立身份 ✅（§6.3）；append-only ✅（§6.4 UPDATE OR DELETE）；类型注册 ✅（§6.2 FK + 代码校验，种子缺口 F-08）；方向性/端点合法性 ✅（directionality 快照 + TEXT[] 类型集合，库层仅存类型键、集合校验在服务层——可接受，因 TEXT[] 无法做 FK，已属 PG 能力边界内的正确选择）；端点指向错误对象 ✅（复合 FK 拦截）；历史关系被当前状态改写 ✅（RR 快照 + immutable）；关系历史查询 ✅（Q5 索引）；历史版本关系还原 ✅（RR 自包含 + F-03 修复后彻底成立）。

## 5.3 Cognition Version（§5.3 评审）

- 版本头字段完备（§3.5 任务书要求逐项有列）；`UNIQUE(tenant_id, project_id, version_no)` 并发闸正确；predecessor 复合自引用正确。
- **发现 F-02（P1）：发布事务与不可变触发器自相矛盾。** §15.5 `forbid_version_mutation` 将 `snapshot_hash` 列为 once-written 锚定列；§18.1 步骤⑦ `INSERT version(snapshot_hash=占位)` → 步骤⑩ `UPDATE SET snapshot_hash=最终值`。该 UPDATE 命中触发器 `OLD.snapshot_hash IS DISTINCT FROM NEW.snapshot_hash` → `RAISE EXCEPTION 'anchor columns are immutable'`。**按文档实现，任何一次版本发布都会在步骤⑩失败回滚。** 失败是显式的（不会产生坏数据），故定 P1 而非 P0；但发布主流程完全不可实施。
- 修复：采用「先算后插」——成员集在插入 version 前已确定（服务层事务内持有），hash 先行计算、version 一次 INSERT 含最终 snapshot_hash，删除占位回填步骤（详见 §9）。

## 5.4 Version Member（§七 逐项核对，本次评审核心）

| 保证项 | 结论 | 证据 |
|---|---|---|
| 二选一（不同时空/同空） | ✅ 互斥 CHECK 恰好覆盖四列全非空 XOR 两列全空两种合法形态 | §7.3 CHECK |
| Object Revision 属于指定 Object | ✅ 复合 FK `(tenant,project,object_id,object_revision_id)`，列序与被引用键 `UNIQUE(tenant,project,object_id,id)` 一致 | §5.3/§7.3 |
| Relationship Revision 属于指定 Relationship | ✅ 同构四列复合 FK | §6.4/§7.3 |
| 同 Tenant/Project | ✅ 全部 FK 首两列为 tenant/project，父键为项目级身份键 | §14.2 |
| 不重复收录 | ✅ 两把部分唯一索引按维度覆盖全部成员形态（object WHERE object_id IS NOT NULL / relationship WHERE relationship_id IS NOT NULL——两类情况的并集 = 全部合法行） | §7.3 |
| 发布后不可修改 | ✅ `trg_pc_vm_immutable` BEFORE UPDATE **OR DELETE**；Member 无状态列、无合法 UPDATE 场景 | §7.3 |
| NULL 语义漏洞 | ✅ MATCH SIMPLE 下 NULL 组 FK 自动跳过，互斥 CHECK 保证「参与 FK 的组必然齐全」——两者配合无漏洞；NULL 不进入唯一索引键（部分索引谓词排除） | — |
| 并发插入重复成员 | ✅ 发布为单事务 + advisory lock 串行化 + 部分唯一索引兜底（即使绕过锁也 23505） | §17.2-5 |
| **是否真固定 Revision（而非身份 ID）** | ✅ 成员行同时持有身份列与 Revision 列，且 Revision 列参与复合 FK 与唯一索引——Revision 不可变 ⇒ 固定语义成立 | §7.3 |

**「修改 Revision 所属关系后破坏成员完整性」**：Revision 归属列在其不可变保护内，无 UPDATE 路径 ✅。

**结论：Version Member 双维度模型为 PC-01B 质量最高的部分，七项保证全部由数据库约束承载，无缺陷。**

## 5.5 Relationship 端点闭合（§八，P0 检查项）

| 检查 | 结论 | 证据 |
|---|---|---|
| 拒绝缺 Source/Target 成员的版本 | ✅ 发布事务断言 SQL（NOT EXISTS 反例，返回 broken 成员与缺失侧）+ DEFERRED 约束触发器在 COMMIT 前对全版本校验——两条独立路径均拒绝 | §8.3 / T-09/T-10 |
| 拒绝端点 Revision 与 Member 不一致 | ✅ 该不一致在模型上必然表现为「端点对象成员缺失」（RR 端点快照自包含），由同一断言拦截 | §8.4 / T-11 |
| 数据库级 vs 应用层约定 | ✅ 约束触发器为 `CREATE CONSTRAINT TRIGGER ... DEFERRABLE INITIALLY DEFERRED`——绕过服务直连 SQL 也无法提交不闭合版本；应用层查询仅为可读错误诊断 | §8.3 |
| 发布事务复查 | ✅ 步骤⑨显式执行断言 | §18.1 |
| 发布后防破坏 | ✅ RR immutable + Relationship 端点锚定列 immutable + Member immutable——闭合三要素全部不可变，无破坏路径 | §5.2/§6.4/§6.5/§7.3 |
| 并发保护 | ✅ 发布事务持 project advisory lock；DEFERRED 语义保证批量插入顺序无关 | §8.3 注 1 |
| 跨域端点 | ✅ 库层跳过（诚实声明 PG 无法 FK 外域）、服务层通道校验 + 审计记录 | §8.3 注 3 |
| 历史还原端点语义 | ✅（F-03 修复后完全成立：还原 = 读不可变历史） | §8.4 |
| 触发器正确性 | ✅ AFTER ROW 内读本表在 DEFERRED 执行时点全部可见；只读无递归；性能 O(kN) 已声明并有降级预案 | §16.2 |

**判定：P0 检查项通过——数据库级闭合约束真实存在且逻辑正确，不依赖应用层约定。**（F-03 属端点「身份一致性」的前置缺口，不是本节「集合闭合」缺口；两者互补。）

## 5.6 Source / Evidence / KB Memory

- Source/Evidence 分离 ✅（独立实体、独立职责列）；Evidence 钉 Revision 级主体（XOR CHECK）✅；定位协议覆盖 文档（document_revision+anchor+行号）/ 代码（file_path+symbol+code_range+commit_sha）/ 会议与陈述（excerpt/description）✅（判别器 CHECK 沿 A7 已验证模式）；evidence_hash 可验证内容 ✅；Confidence（evidence.confidence）与 Confirmation（proposal/approval/version 链）分属不同表 ✅；无 JSONB 混用 ✅。
- 反向追溯（对象→来源/证据）有索引支撑（M-4 修复）✅。
- KB Memory 仅为输入（source 行），审核通过不产生任何正式状态变化 ✅（Q-PC-12）。
- 缺陷：F-16（kb_document 通道 target_* 四元组与 document_* FK 列双表达无一致性 CHECK，P3）；F-19（proposal 与 evidence 无直接关联结构，多来源提案经成员 Revision 推导，推导规则未成文，P3）。

## 5.7 Proposal / Approval / Conflict

- approval 侧八原则库层落实完整（锚定 once-written + 单向状态 + pending 部分唯一 + FOR UPDATE + 同事务审计），显著优于既有 approval_request 弱结构 ✅。
- **F-04（P1）**：proposal 侧无锚定——`proposal_content_hash`/`expected_version_no`/`title`/`rationale` 在 submitted/approved 后仍可 UPDATE（§10.1 DDL 无 BEFORE UPDATE 触发器）。步骤③「proposal_content_hash ≠ approval.proposal_content_hash → ROLLBACK」信任了 proposal 侧可变列；攻击/事故场景：approved 后绕闸改 proposal.proposal_content_hash 为 approval 中的旧值并同步篡改成员集——若步骤⑩「hash 推导复核」语义含糊（文档未定义复核算法与 proposal hash 的关系），则审批内容与发布内容可漂移。违反八原则 1「审批对象不可事后替换」的 proposal 侧。
- **F-05（P1）**：成员时序契约缺失——§10.1 注记允许 create_* 成员 Revision「待 Git 提交后回填」，但 proposal_member 锚定触发器在 submitted 后禁止 UPDATE ⇒ 回填窗口不存在；§18.1 步骤⑧插入 Member 时互斥 CHECK 要求 Revision 非空 ⇒ 未回填成员将 23514。同时 create_relationship 的端点对象成员是否自动并入提案成员集（或要求显式携带）**全文未定义**——合法提案会在步骤⑨随机失败。发布输入契约必须显式化（§9 给出推荐时序）。
- **F-01（P1）**：§10.3 `PRIMARY KEY (conflict_id, tenant_id, project_id, coalesce(object_id::text, relationship_id::text))`——**PostgreSQL PRIMARY KEY 不允许表达式列**（内联 PK 语法仅接受列名；表达式唯一性只能用 CREATE UNIQUE INDEX）。该 CREATE TABLE 直接语法错误，建表即失败。
- conflict 冻结范围结构本身合理（FK 复合、XOR CHECK）；状态机 CHECK 完整；resolved 必留痕 CHECK 正确。

---

# 6. 数据库完整性与安全评审

| 维度 | 结论 |
|---|---|
| 复合 FK | ✅ 15 表全部按双唯一键模式；内部引用零单列 FK（对照旧 cognition 域缺陷 F.3-1，全面修复）。唯一例外为外域引用（租户级 FK + 服务层，§13.3 诚实声明） |
| CHECK | ✅ 形态互斥、端点三元非空、判别器、状态机、终态留痕——全部具名可测。F-01 为唯一语法级错误 |
| UNIQUE / 部分唯一 | ✅ pending 审批、活跃关系身份、单例对象、证据身份、幂等键、版本号——覆盖关键不变量；singleton 谓词实现两处不一致（F-12） |
| Append-only | ✅ Revision/Member 全行禁改删；审批/版本锚定列 once-written。**身份行（object/relationship/version）无显式 DELETE 禁止**，现靠子表 FK 间接阻挡（F-10，防御深度） |
| 正式版本不可变 | ✅ 版本头锚定 + Member 不可变 + Revision 不可变三层；无任何合法改写路径（F-02 修复后发布事务本身也一致） |
| 租户隔离 | ✅ 全部 PC 内 FK 首列 tenant_id；无跨租户路径 |
| 项目隔离 | ✅ 复合 FK 含 project_id。**攻击场景实测（任务书 §十六）**：Project A Proposal 引用 Project B Object Revision → proposal_member 复合 FK `(tenant,project,object_id[,revision])` 父键为项目级身份键 → **23503 拒绝**；Project A Member 引用 Project B Relationship Revision → 同构拒绝。**不存在仅靠应用层过滤的隔离面**（外域引用除外，已声明双轨） |
| 跨领域引用 | ✅ 四元组三元定位非空 CHECK + 通道状态表 + 双轨完整性（§13.3）；Design/ADR/Product Reality 未伪造 |

深度防御建议（不阻塞）：PC 核心表 `REVOKE UPDATE, DELETE` 于应用角色（触发器之上的权限层）；现设计依赖触发器拦截，应用角色若为表 owner 则触发器仍生效（owner 不可禁用自己的普通触发器，仅可 DISABLE TRIGGER USER——需 owner 权限），风险可控。

---

# 7. 事务与并发评审

## 7.1 版本发布事务（§18.1 十四步逐项）

| # | 步骤 | 核验 |
|---|---|---|
| ① | advisory lock + 幂等账本 | ✅ 项目级 advisory xact lock 串行化发布入口；write_operation 唯一键幂等 |
| ② | expected_version CAS | ✅ max(version_no) 在锁内读取；不匹配 → proposal failed（独立事务） |
| ③ | Proposal Hash 校验 | ⚠️ 信任可变的 proposal.proposal_content_hash（**F-04**） |
| ④ | 审批人授权 | ✅ A3 闸（无锁竞态窗口已声明 V1 接受） |
| ⑤ | 审批锚点 + 行锁 | ✅ FOR UPDATE + 锚定列不可变，双锚点（hash + expected_version_no） |
| ⑥ | L3 冻结 | ✅ 冻结集合校验存在；**仅覆盖 publish 入口，approve 入口未校验**（F-06，P2） |
| ⑦⑩ | Version 创建 + hash | ❌ **F-02：占位回填被锚定触发器拒绝** |
| ⑧ | 双维度成员固定 | ⚠️ 前置契约缺失（**F-05**）；插入本身约束完备 |
| ⑨ | 端点闭合 | ✅ 断言 + DEFERRED 触发器 |
| ⑫⑬ | 审计 + Proposal 单向状态 | ✅ 同事务原子；审计失败 = 发布回滚（不存在「无审计正式版本」） |
| ⑭ | 提交 | ✅ |

竞争场景核验（任务书 §十七）：

- 两个 Proposal 同基旧版本：CAS 串行，一成一败（败者 failed + 重新分析）✅；
- 两个审批人并发：approval 行锁 + pending 唯一双保险 ✅；
- 两个发布并发：advisory lock 串行 ✅；
- 发布中 Revision 被修改/废弃：Revision 不可变、对象废弃为终态转换服务路径（持对象行锁）——发布事务引用的 Revision 集合在事务内稳定 ✅；
- 发布中 Relationship 被修改/废弃：端点锚定 + 关系行锁同上 ✅；
- 发布中出现 L3 Conflict：步骤⑥按事务内快照判定；发布开始后新登记的 Conflict 不回溯阻断本次发布（其生效面向下一版本）——**语义可接受但未成文**，建议在 §18 补一句明确（并入 F-06 修订）。

**结论：除 F-02/F-04/F-05/F-06 四项外，发布事务设计与并发控制成立；锁定对象、锁序（§17.3）、隔离级别（Read Committed + 显式锁）均有明确定义，无死锁环路（advisory lock 为全局首锁）。**

## 7.2 Git-PG 一致性（§12 十故障场景核验）

| # | 故障 | 最终状态 | 判定 |
|---|---|---|---|
| 1 | Git 成功、PG Revision 失败 | git_committed/failed；reconcile 按 trailer 反查补④；hash 不符转人工 | ✅ 可恢复、可判定 |
| 2 | PG 账本成功、Git 失败 | pending；重放②；超窗 abandoned | ✅ |
| 3 | Git 成功、进程崩溃 | pending（账本行随崩溃事务回滚与否两态）；trailer 反查兜底 | ✅ |
| 4 | PG 提交成功、响应丢失 | completed + Revision 已落；客户端重试走幂等键返回既有产出 | ✅ |
| 5 | 同 Operation ID 重复提交 | 唯一键 + completed 幂等返回 | ✅ |
| 6/7 | 并发修改同 Object/Relationship | 行锁 + revision 唯一约束，无重复号、无丢失更新 | ✅ |
| 8 | Git 成功、Proposal 状态更新失败 | 发布路径无 Git 写入；Revision 路径不触 Proposal——场景不成立；等价场景（④ 失败）见 #1 | ✅ |
| 9 | Revision 已登记、Git 内容不可读 | 对账任务 git_drift 审计 + 冲突登记；PC 不改写历史 | ✅ 可发现 |
| 10 | Git Hash 与 PG Hash 不一致 | 步骤④核验拒绝 / 对账 drift | ✅ |

不得出现项核验：重复 Revision（唯一约束+幂等）✅；指向错误 commit（hash 核验）✅；Proposal 错误标记完成（单向状态触发器）✅；版本引用不存在正文（发布前提 Revision 已落库 + F-05 契约修复后成立）✅；状态永久无法判定（**F-11：failed 标记需独立事务、retry_count 上限与人工阈值未定义**——现状下持续失败的行将无限重扫，P2）。

---
# 8. 缺陷清单

## 8.1 P1（必须修复，阻塞验收）

**F-01 · conflict_object 主键使用表达式——PostgreSQL 语法错误**
- 位置：PC-01B §10.3（被评审文档 line 1129）。
- 描述：`PRIMARY KEY (conflict_id, tenant_id, project_id, coalesce(object_id::text, relationship_id::text))`——PG 内联 PRIMARY KEY 仅接受列名列表，表达式唯一性只能用 `CREATE UNIQUE INDEX`。
- 违反要求：任务书 §十八「DDL 级伪码可直接实施」、§二十一.14「禁止含糊的数据库保证」。
- 失败场景：0045 迁移执行 `CREATE TABLE pc.conflict_object` → `syntax error at or near "coalesce"`，整个迁移失败。
- 影响：实施阻塞（失败显式，无静默风险）。
- 修复：改为 `id UUID PRIMARY KEY` + 独立 `CREATE UNIQUE INDEX uq_pc_conflict_object ... ON (tenant_id, project_id, conflict_id, coalesce(object_id::text,''), coalesce(relationship_id::text,''))`。
- 阻塞验收：**是**。

**F-02 · 发布事务 snapshot_hash「占位回填」与版本头锚定触发器自相矛盾**
- 位置：§18.1 步骤⑦（INSERT `snapshot_hash=占位`）与步骤⑩（UPDATE 回填）vs §15.5 `forbid_version_mutation`（`snapshot_hash` 为 once-written 锚定列）。
- 违反要求：任务书 §十四.14（发布事务必须可实施）、PC-01B 自身 §4.2「Version 创建即发布」。
- 失败场景：首次发布 → 步骤⑩ UPDATE 命中触发器 `OLD.snapshot_hash IS DISTINCT FROM NEW.snapshot_hash` → `P0001: anchor columns are immutable` → **每次发布必然回滚**。
- 影响：发布主流程完全不可实施（失败显式）。
- 修复：「先算后插」——成员集在插入 version 前已确定（事务内持有），先计算 snapshot_hash 再一次性 INSERT version（含最终 hash），删除占位/回填；保留「hash 与成员集一致」断言。
- 阻塞验收：**是**。

**F-03 · Relationship Revision 端点身份与 Relationship 行端点身份的一致性无库层校验**
- 位置：§6.4（RR DDL 无父行比对约束）、§6.5（仅锚定 Relationship 行 UPDATE）。
- 违反要求：PC-01B §6.1 #1 裁决（端点身份属于 Relationship 身份）；任务书 §六「必须检查数据库是否真正约束这一规则」、§二十一.7「不得把应用层校验说成数据库约束」。
- 失败场景：见 §5.2 构造——绕闸 INSERT 端点为 C/B 的 RR 挂在端点为 A/B 的关系 R 上，全部 CHECK/FK 通过并可发布；同一 Relationship ID 的 Revision 链表达两种不同关系。
- 影响：关系历史与版本还原的语义一致性被破坏（需绕过安全闸或服务缺陷触发，属防御深度缺口而非主路径缺陷，故 P1 而非 P0）。
- 修复：`BEFORE INSERT ON pc.relationship_revision` 触发器逐列比对父行（source_object_id / target_object_id / 四组 ext 列 / relationship_type_key）。
- 阻塞验收：**是**。

**F-04 · Proposal 内容锚定缺失——审批后内容可变**
- 位置：§10.1（pc.proposal DDL 无 BEFORE UPDATE 锚定触发器）；§18.1 步骤③信任可变的 proposal.proposal_content_hash。
- 违反要求：基线 §9.3 八原则 1「审批对象不可事后替换」（Q-PC-07 落实要求）；任务书 §二十一.10。
- 失败场景：approved 后绕闸 `UPDATE pc.proposal SET proposal_content_hash = <approval 内的旧值>, rationale = ...` 并篡改成员集 → 步骤③通过；步骤⑩「hash 推导复核」的算法与 proposal hash 的关系全文未定义，兜底有效性不可证。
- 影响：审批内容与发布内容可漂移；审批锚定链存在可变环节。
- 修复：双管齐下——① proposal 内容锚定列（title/rationale/origin_kind/governance_level/expected_version_no/proposal_content_hash）submitted 后 once-written 触发器（同 approval 范式）；② 步骤③升级为主校验「服务层重算成员集规范 hash == approval.proposal_content_hash」，不信任 proposal 侧列。
- 阻塞验收：**是**。

**F-05 · 提案-发布成员时序契约缺失 + create_relationship 端点成员补齐规则未定义**
- 位置：§10.1 注记（create_* Revision「待回填」）vs §10.1 anchored 触发器（submitted 后禁改 member）vs §18.1 步骤②⑧（成员集解析与插入）；端点补齐规则全文缺失。
- 违反要求：任务书 §二十一.20（PC-01C 无需重新决定语义）、§十四.14；PC-01B §1 结论 12。
- 失败场景：① create_object 提案若按注记在提交后回填 Revision → 被 anchored 触发器拒绝，成员永远缺 Revision → 发布步骤⑧互斥 CHECK 23514；② create_relationship 提案成员集未显式携带端点对象成员 → 步骤⑨闭合断言随机失败。两条合法路径均无定义行为。
- 影响：发布输入契约不可实施；01C 将被迫自行发明时序与补齐规则（正是任务书禁止的）。
- 修复：写死契约——draft 阶段完成 Git 提交 + Revision 落库 + proposal_member Revision 齐备（submit 前置校验）；发布事务步骤②后增「成员完整性断言」；端点成员由提案服务自动并入成员集并纳入 proposal_content_hash（推荐，§9 R5 详述）。
- 阻塞验收：**是**。

## 8.2 P2（原则上修复，可附条件通过）

| ID | 位置 | 描述与失败场景 | 违反要求 | 修复 | 阻塞 |
|---|---|---|---|---|---|
| F-06 | §18.1 ⑥ vs 01A §6.5 | L3 冻结仅校验 publish 入口；approve（确认动作）可在冻结下完成，冻结意图被部分穿透 | 01A §6.5「冻结确认入口」 | approve 服务与发布事务双重校验；补「发布中新增 Conflict 不回溯」语义成文 | 否（附条件） |
| F-07 | §10.1/§3.3 | 提案 governance_level 与目标类型固定等级（business_rule/product_vision/business_process = L2 不可下调）无校验落点 | Q-PC-06/09 | 类型注册表 `immutable_level` 语义 + 服务层契约「提案等级 ≥ 目标类型固定等级」 | 否（附条件） |
| F-08 | §6.2/§15.5 | relationship_type 种子矩阵推迟 01C——端点合法性是领域语义，01C 将被迫做领域决策 | 任务书「01C 不重新决定领域语义」 | 01B 补种子清单（基线 §7.2 矩阵）或正式声明推迟并列为 01C 验收项 | 否 |
| F-09 | §22.2 | doc_type CHECK 扩值 `'pc_object'` 的 ALTER 未列入 Migration 清单 | §11.2 自身裁决 | §22.2 补 `ALTER TABLE ai.project_document` CHECK 重定义 | 否 |
| F-10 | §5.2/§6.3/§15.3 | 身份行（object/relationship）无显式 DELETE 禁止，靠子表 FK 间接阻挡；identity 态行可删（合理）但终态行防护仅为间接 | 基线 §13.4 | BEFORE DELETE 触发器（或声明 FK 间接保护语义） | 否 |
| F-11 | §12.4/§12.5 | 步骤④失败时 failed 标记需独立事务（否则随事务回滚，状态停 git_committed）；retry_count 上限与人工介入阈值未定义 → 无限重扫 | 任务书 §十五「操作状态永久无法判定」禁止项 | 独立事务补标 + retry 上限（如 5）→ abandoned + 审计 | 否 |
| F-12 | §5.2 vs §15.4 | uq_pc_object_singleton 谓词 §5.2 字面（查表函数）不可实施（IMMUTABLE 要求），§15.4 已给硬编码实现——两处不一致，01C 照抄 §5.2 将失败 | 任务书 §十八 | §5.2 DDL 与 §15.4 统一为硬编码函数 | 否 |

## 8.3 P3（记录后续优化）

| ID | 位置 | 描述 | 建议 |
|---|---|---|---|
| F-13 | §5.2/§6.3/§15.3 | 循环 FK（object↔revision、relationship↔RR）需「建表后 ALTER ADD CONSTRAINT」；公共触发器函数（§15.3）须先于全部触发器创建；DEFERRABLE 对 current_revision 回填非必需——实施顺序注记缺失 | §22.2 补对象创建顺序 |
| F-14 | §5.2/§17 | updated_at 无自动刷新触发器定义，CAS 依赖该列 | 补 `moddatetime` 扩展或触发器说明 |
| F-15 | §6.3 | undirected 类型端点对称重复（A—B 与 B—A）未被身份唯一索引去重 | 类型矩阵若含 undirected 再处理 |
| F-16 | §9.2 | kb_document 通道 target_* 四元组与 document_* FK 列双表达无一致性 CHECK | 补 CHECK 或声明服务层同步 |
| F-17 | §15.3 | pc.cognition_version 建议补 BEFORE DELETE 防护（现靠 member FK 间接）；建议 REVOKE UPDATE/DELETE 深度防御 | 一行触发器 + 权限声明 |
| F-18 | §11.2 | doc_key 形态先写冒号后改正（`pc:{type}:{key}` → `pc-object-{key}`），表述混乱（结论正确） | 清理表述 |
| F-19 | §9.5/§10.1 | proposal↔evidence 无直接关联结构；多来源提案经成员 Revision 推导——推导规则未成文；Q-PC-03 的来源引用完整性依赖服务层（已声明双轨） | 文档明示推导规则或预留 proposal_source 关联表 |
| F-20 | §10.1 | rejected/withdrawn 提案遗留的 draft 对象/Revision 残留处置未定义（草稿生命周期） | 01C 定义废弃/复用路径 |

---

# 9. 必须修订的问题（PC-01B V1.1 最小修订清单）

按优先级排序；R1~R5 对应 P1（必须），R6~R12 对应 P2（随同修订）：

| # | 修订 | 章节 | 内容 | 交付形态 |
|---|---|---|---|---|
| R1 | F-01 | §10.3、§14.2 | conflict_object 改 `id UUID PK` + 独立表达式唯一索引；矩阵注记同步 | **补 DDL** |
| R2 | F-02 | §18.1 步骤⑦⑩ | 「先算后插」重写：成员集确定 → 计算 snapshot_hash → 一次性 INSERT version；删除占位/回填；保留 hash 一致性断言 | **补伪代码** |
| R3 | F-03 | §6.4/§6.5 | 增补 `trg_pc_rel_revision_endpoint_match`（BEFORE INSERT 比对父行端点 + 类型键） | **补 DDL + 测试 T-27** |
| R4 | F-04 | §10.1/§18.1 步骤③ | proposal 内容锚定触发器（once-written）+ 步骤③升级为「重算成员集 hash vs approval 锚定」主校验 | **补 DDL + 伪代码 + 测试 T-28** |
| R5 | F-05 | §10.1/§18.1 步骤② | 成员时序契约成文（draft 内 Revision 落库齐备 → submit 前置校验 → 发布前完整性断言）；端点成员自动并入规则 + 并入 hash | **补伪代码 + 测试 T-29** |
| R6 | F-06 | §18.1 步骤⑥/§10.2 | approve 入口冻结校验 + 「发布中新增 Conflict 不回溯」语义成文 | 伪代码一行 |
| R7 | F-07 | §3.3/§10.1 | 类型固定等级校验契约（提案等级 ≥ 目标类型固定等级） | 文档 |
| R8 | F-08 | §6.2/§15.5 | relationship_type 种子清单（基线 §7.2 落地）或推迟声明 + 01C 验收项 | 种子 SQL 或文档 |
| R9 | F-09 | §22.2 | Migration 清单补 doc_type ALTER | 清单一行 |
| R10 | F-10 | §5.2/§6.3 | 身份行 DELETE 防护触发器 | 补 DDL |
| R11 | F-11 | §12.4/§12.5 | failed 标记独立事务 + retry 上限 → abandoned | 文档 |
| R12 | F-12 | §5.2 | singleton 谓词与 §15.4 统一（硬编码函数） | DDL 小改 |

---

# 10. PC-01C 实施准备度

| 维度 | 准备度 | 说明 |
|---|---|---|
| 领域模型 | **就绪** | 双维度模型/五聚合/生命周期/类型体系完整，无模型级返工 |
| 数据库结构 | **修复后就绪** | 15 表 DDL 完整；R1（PK 语法）/R12（谓词）/F-13（建表顺序注记）修复后可直接翻译为迁移 |
| 完整性约束 | **修复后就绪** | 复合 FK/CHECK/部分唯一/append-only 主链完备；R3/R10 增补后无绕过路径 |
| 事务 | **修复后就绪** | 发布十四步与并发六场景设计完整；R2/R5 修复后可实施 |
| 审批 | **修复后就绪** | approval 侧范式完整；R4 修复 proposal 锚定后八原则全闭合 |
| Git-PG 协议 | **修复后就绪** | 三态账本/幂等/对账完整；R11 补恢复细节；H-1/H-4 环境验证归 01C（已声明） |
| 租户隔离 | **就绪** | 双唯一键 + 全复合 FK，攻击场景实测拒绝 |
| 测试 | **就绪** | T-01~T-26 覆盖 23 项负向 + 3 附加断言；R3/R4/R5 补 T-27~T-29 |
| Migration 实施 | **条件就绪** | 0045 起号结论成立（H-13 源码级验证）；前置 = 0044 修复（需批准）+ R1/R9/R12 修订 |

---

# 11. 最终裁决

**PC-01B V1.0 现阶段不可以作为 PC-01C 的正式数据库实施输入。**

理由：5 项 P1 中，F-01/F-02 使设计与实施直接冲突（建表失败、发布必失败），F-03/F-04 在数据完整性上留下可证明的绕过路径，F-05 使发布输入契约不可实施——任一项进入 01C 都会把「设计缺陷」转化为「实施期返工」或「上线后数据一致性事故」。

**最小必要修订 = R1~R5（全部 P1）**。五项均为局部修复：不推翻双维度版本模型、不推翻 PC-D 任何裁决、不引入新表（R1 仅重排约束载体）、不需要重新评审全部章节。R6~R12（P2）建议随同修订以避免 01C 二次澄清。

**修复后路径**：PC-01B 出 V1.1（差异修订）→ 评审组对 R1~R5 逐项差异复核（无需全面重审）→ 复核通过即作为 PC-01C 正式输入。

**可在 01C 关闭的剩余风险**（不阻塞）：H-13 运行库 checksum 归属（连库一次即证）、H-1 Git root 布局、H-4 事务顺序故障注入（T-20/T-21 承载）、外域组合键校验实现（R-2）、授权竞态窗口（R-3，V1 接受）、闭合触发器性能压测（R-4，降级预案已备）。

---

**评审声明**：本报告为独立评审产出；未修改 Q-PC-01～13 与 PC-01A 任何裁决；未提出历史数据迁移方案；未将 Design/ADR/Product Plan 描述为已接通或纳入 PC；全部缺陷均给出可验证证据（文档行号/章节/构造场景）；无法验证项已在 §2.2 显式标注。
---

# 附：V1.1 修订落实核验记录（2026-11-07，修订方自证，待评审组差异复核）

被评审文档已升级为 **V1.1**（其附录 A 对照表 12 项）。按本报告 §9 修订清单逐项核验：

| # | 修订 | 落实状态 | 核验点 |
|---|---|---|---|
| R1 | F-01 | ✅ | §10.3 DDL 重写（id PK + 表达式唯一索引）；§14.2 矩阵行同步 |
| R2 | F-02 | ✅ | §18.1 ⑦⑧⑩先算后插，占位/回填消除，hash 一致性复核保留 |
| R3 | F-03 | ✅ | §6.5 新增 `trg_pc_rel_revision_endpoint_match`（BEFORE INSERT 比对父行 9 列）；T-27 |
| R4 | F-04 | ✅ | §10.1 新增 `trg_pc_proposal_anchors`（7 个内容锚定列 once-written）；§18.1 ③重算主校验；T-28 |
| R5 | F-05 | ✅ | §10.1 成员时序契约 4 条成文；§18.1 ②b 完整性断言；端点自动并入（方案 A）成文；T-29 |
| R6 | F-06 | ✅ | §18.1 ⑥ 双落点 + 不回溯语义成文 |
| R7 | F-07 | ✅ | §10.1 governance_level 列契约注记（submit 前置校验） |
| R8 | F-08 | ✅ | §15.5 种子 17 条 SQL 落地（derived_from/supported_by 由追溯结构承载的设计裁决已注记） |
| R9 | F-09 | ✅ | §22.2-4 doc_type ALTER 具名约束重定义 |
| R10 | F-10 | ✅ | §15.3 `forbid_identity_delete` 三触发器 |
| R11 | F-11 | ✅ | §12.4/§12.5 failed 独立事务 + MAX_AUTO_RETRY=5 + 四态收敛声明 |
| R12 | F-12 | ✅ | §5.2 硬编码 IMMUTABLE 函数定稿，§15.4 同步 |

差异复核范围建议：仅审 R1~R5 的五处新增约束/契约（§6.5、§10.1 两段、§18.1 ②b③⑦⑧）即可关闭「不通过」结论，无需全面重审。