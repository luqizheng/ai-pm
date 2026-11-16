# P3-11 开发落实与差异报告 V1.0

- 日期：2026-11-16 · 提交基准：master `79a82908`（本次开发在其上，未冻结、**待验收**）
- 性质：P3-11 裁决稿 V1.1 的 **Stage 1 开发落实**——按「先落实后验收」提交，不自行标记正式冻结
- 裁决稿基础：`Phase3-裁决稿-P3-11仓库分析认知归属与双域治理-v1.1.md`（R-01~R-10 默认方案 + 专家四项修订）
- 测试库：aipm-pg-gate（127.0.0.1:16433，AGENTS.md 口径）

---

## 1. R-01~R-10 逐项落实情况

| 裁决项 | 状态 | 本批落实 | 差异/剩余 |
|---|---|---|---|
| R-01 独立 projc.* schema | **代码已落实** | 迁移 0025 建 `projc` schema 13 张机制表（§3）；服务层 `pm-application::projc`（§4） | 提案/审批/版本**表就绪、写入者未启**（T-2） |
| R-02 双域权威边界五条 | **代码已落实** | 独立字典（projc.object_type + 3 种子；pc 类型在 projc 被拒，测试断言）；独立状态机（lifecycle 触发器域内独立）；草稿面/正式面分离 | 正式确认链随 T-2 |
| R-03 分流判据+模块消歧 | **部分落实** | 字典承接 repository/tech_stack/repository_module（消歧命名，专家 R-03 修订已入字典标签）；D-4 限定与分流表=裁决稿 §1（设计层冻结项） | worker 重挂未做（见 §5 时序） |
| R-04 有界过渡 | **设计落实，守卫未编码** | 过渡行为/七条就绪判据=裁决稿 §5.2/5.3；T-1 过渡守卫=**独立任务项，等 P2-01~08 落地**（P2 零改动红线禁改 close 事务，§6） | T-1 + 停用开关 + E2E-02 断言调整 |
| R-05 delivery_activity 移除 | **设计落实** | 字典种子不含 delivery_activity；registry 附录标记移除（§7） | 派生停止随 T-1（worker 改动） |
| R-06 ProjC 自建 Conflict | **设计落实，表未建** | pc.conflict 标 Legacy/Not for New Use（registry 附录）；projc.conflict 随 P3-D04（裁决稿既定） | D04 落表 |
| R-07 DomainRef 契约 | **代码已落实** | projc.source/projc.evidence 按四字段契约（target_domain/target_type/target_id/target_revision_id+missing_reason）；kb-pc-contracts TargetDomain::ProjectCognition 注释更新为目标态 projc.*；**禁止合并字段**写入契约注释与 registry 附录 | 跨 schema resolver 实现细则（开放项 §8） |
| R-08 统一检索三层 | **未落**（既定后置） | — | 检索适配层=P3-10 前置任务；PageIndex/FTS 未建 |
| R-09 close 分流发布 | **设计落实，未编码** | 落点映射=裁决稿 §5.1；同 R-04 时序约束 | T-1/T-2 |
| R-10 存量=Legacy Transitional | **文档落实** | registry 附录标记；pc.* 实测仍 0 行（§7 证据），无存量搬运需求 | 弃置/重跑决策随 T-2 |

## 2. 修订后的模型与接口

### 2.1 领域模型（差异 vs 裁决稿，均登记）

- projc 对象生命周期沿用 5 态（identity/draft/active/deprecated/superseded），语义独立定义（触发器域内实现，非引用 pc 函数）。
- 字典种子 3 类：`repository`（L1_identity）/`tech_stack`、`repository_module`（L2_structure）——**不带 snapshot layer**（R-05：本域对象是一等技术认知，动态指标不入典）。
- `origin_kind` 增 `repo_analysis`；`source_kind` 以 `product_cognition`/`repo_analysis` 替换 pc 侧 `project_cognition`（target 语义分离）。
- `projc.object_type_singleton` 恒 false（V1 无单例技术对象），函数保留以承载 uq 部分索引机制。

### 2.2 数据模型（迁移 0025，表清单）

`projc` schema：object_type / cognition_object / document / document_revision / cognition_revision / cognition_version / version_member / write_operation / proposal / proposal_member / approval / source / evidence。
触发器 8 个：object_delete / object_lifecycle / document_revision_immutable / cognition_version_immutable / write_op_state / proposal_anchors / proposal_member_anchored / approval_immutable。
索引：9 普通 + 6 唯一（uq_object_singleton / uq_evidence_identity / uq_approval_pending / uq_pm_target / uq_vm_object / uq_vm_relationship）。
**未建**（登记）：relationship 三表、conflict 两表、成员端点闭合触发器、v_revision_versioned 视图、FTS。

### 2.3 接口

- **Stage 1 无 REST 端点**（有意：写入者先于界面，避免无门禁的写通道暴露）。
- Rust 服务接口：`projc::upsert_object_draft` / `register_source` / `register_evidence`（签名与 pc 同形，错误类型 `ProjcError`）。
- T-2 计划接口：`projc::publish_version`（真人主体 + A3 权限闸 + advisory lock + 幂等账本）、审阅流 v2 提案路由、报告详情 ProjC 提案读端点。

### 2.4 过渡方案

裁决稿 §5.1/5.2/5.3 不变，本批补充两点实施口径：

1. pc.* snapshot 链**原样运行**（过渡数据），`projc` 写入者已就位但无生产调用者——切换发生在 T-2（worker 重挂 + 审阅路由），切换前 `projc` 全库预期 0 行。
2. T-1 守卫（snapshot 提案不发布为 PC 正式版本）实施窗口=**P2-01~08 落地之后**，E2E-02 close 批发断言同步调整并登记测试漂移。

## 3. 工作分类（文档 / 数据库 / 服务 / 迁移 / 测试）

| 类别 | 变更 | 文件 |
|---|---|---|
| 数据库迁移 | **0025_projc_domain_bootstrap.sql**（新增） | `migrations/0025_projc_domain_bootstrap.sql` |
| 服务实现 | projc 模块（mod/object/evidence）+ 模块注册 | `crates/pm-application/src/projc/{mod,object,evidence}.rs`、`crates/pm-application/src/lib.rs`（+1 行 `pub mod projc;`） |
| 服务实现（契约） | source_kind 白名单扩 2 类 + TargetDomain 注释更新（仅文档性注释，无行为变更） | `crates/pm-application/src/reference_targets.rs`（+5 行）、`crates/kb-pc-contracts/src/registry.rs`（注释） |
| 测试 | 新集成测试 3 用例 | `crates/pm-application/tests/projc_model.rs` |
| 文档（注册表） | projc.* 纳入 + pc.conflict Legacy + snapshot 过渡标记 | `docs/architecture/four-domain/object-registry-v1.1-projc-addendum.md` |
| 文档（台账） | 落实状态登记 | `docs/audit/ia-2026-11-16/Phase3-基线校准与裁决台账-v1.0.md` |
| **未改动** | close_report / 审阅流 / worker / pc.* 行为 / 前端 | 零改动（红线遵守） |

## 4. R-03 / R-04 / R-06 / R-07 四项修订专项核查

| 修订项 | 核查结论 | 证据 |
|---|---|---|
| R-07 DomainRef 与 owner_domain 分离 | ✅ 无合并字段：projc.source/evidence 用 target 侧四字段（kb.memory_ref 先例同款）；owner_domain 仅存在于关系类型注册机制（本批未建 projc 关系表，枚举扩展留 T-2）；registry.rs 注释明确分离 | `projc/evidence.rs` SourceInput/EvidenceInput 字段；`kb-pc-contracts/src/registry.rs:43-46`；registry 附录 §3 |
| R-04 停止条件=能力就绪 | ✅ 本批把就绪判据①（schema+字典）变为 true，②~⑦显式未满足并在 §1/§6 列出——**未满足前不做任何切换动作**；T-1/停用开关/E2E 调整=待办任务项 | 迁移 0025 头注 Stage 1 边界；§6 排查结论 |
| R-03 模块消歧 | ✅ 字典以 `repository_module` 承接（标签注明「非 PC Product Module」）；D-4 限定写入裁决稿 §1（设计冻结项）；pc 字典 14 类产品类型未被 projc 复用（测试：product_profile 在 projc 被拒） | 0025 种子段；`projc_model.rs` object_draft 测试第一断言 |
| R-06 冲突归属 | ✅ pc.conflict Legacy 标记入 registry 附录（五条处置口径原文）；projc.conflict 留 D04；归属原则=裁决稿 §6 表 | registry 附录 §2；无 pc.conflict 代码改动（红线） |

## 5. Phase 2 在途任务依赖排查（用户要求第 5 项）

核对对象：`Phase2-实施任务包-P2-01-P2-08-v1.0.md` + master 现状。

1. **P2 尚未落地**：master 最近实现类提交止于 Phase 1 C1~C5（3bab0469…3ca5ad45）+ 文档（d3b30739/79a82908）；无 P2-01~08 实施提交。
2. **P2 对旧 snapshot 发布路径的依赖=只读消费**：报告关闭 UI（`POST /v1/pm/analysis-reports/{id}/close`）与 E2E-02「暂记→互转→close 批发全链」断言；P2 红线明令**不改 close 事务**。
3. **结论**：本批零改动 close/审阅/worker，与 P2 无文件冲突面；T-1（守卫）必须编独立任务项且排在 P2 落地后，E2E-02 断言届时同步调整——该结论与裁决稿 §5.4 一致并经本次逐项复核确认。

## 6. 未解决问题 / 技术风险 / 待产品裁决

**未解决问题（任务项化）**

| 编号 | 内容 | 批次 |
|---|---|---|
| T-1 | close_report snapshot 提案发布守卫 + 停用开关 + E2E-02 断言调整 | P2 落地后 |
| T-2 | worker snapshot 链重挂 projc.* + projc 提案/发布链 + 审阅路由 + 关系三表 + owner_domain 枚举扩展 | 就绪七条判据验收 |
| D04 | projc.conflict(+object) + 冲突归属原则落表 | P3-D04 |
| P3-10 前置 | 统一检索适配层（Domain Storage/Reference/Retrieval 三层）+ FTS/PageIndex | P3-10 开工前 |

**技术风险**

1. **双写窗口风险（低，有界）**：T-2 切换完成前，仓库分析正式化仍落 pc.*（过渡数据）——风险受 §5.2 过渡行为约束，且 pc 全库 0 行、无检索消费方。
2. **projc 零行先于消费者（信息项）**：本批 projc 表无生产写入者，注册表按「表就绪、链未启」如实标记 STUB，避免死代码冻结风险（pc.conflict 教训：有守卫无解冻入口）——projc 侧 trigger 均为 pc 同款已验证语义，且 conflict 守卫类触发器**未**预建。
3. **迁移号窗口**：0025 按全分支扫号（feat/review-center、feat/todo-2026-11-13、origin/* 均止于 0024）；如并行会话新增迁移需再扫号（纪律：历史扫描上限 0064）。
4. **sqlx::test 迁移耦合**：新迁移使全部 `#[sqlx::test(migrations=...)]` 用例重跑 0025（已验证 pc 全量回归绿）；若 0025 后续修订，需保持幂等语义（纯 DDL，无数据回填，风险低）。

**待产品裁决（验收时逐项确认）**

1. R-01~R-10 是否按 V1.1 默认方案正式冻结（含 §7 两条架构红线入 P3-D05）。
2. T-1/T-2 任务包的排期与验收判据（§5.3 七条就绪判据为验收单）。
3. 存量 pc.* snapshot 数据（当前 0 行）弃置/重跑确认。
4. projc REST 面（读端点先行 vs 与发布链同批）——本批有意未开写通道，请裁决接口批次。

## 7. 证据清单（不以「已修改」为完成依据）

**测试证据（真实 DB：127.0.0.1:16433）**

```text
cargo test -p pm-application --test projc_model --test pc_model --test pc_proposal
  projc_model  3 passed; 0 failed   （新增：upsert 幂等/内容寻址、生命周期与不可变守卫、来源/证据契约）
  pc_model     6 passed; 0 failed   （回归：pc 行为零变化）
  pc_proposal  8 passed; 0 failed   （回归：提案/发布链零变化）
cargo check -p pm-api -p pm-domain  → Finished（无错误；pm-infrastructure 1 条既有警告，与本次无关）
```

关键断言（`tests/projc_model.rs`，编号对应裁决条款）：

- pc 类型 `product_profile` 在 projc 被拒 → **R-02 独立字典**；
- 同内容重推 `revision_created=false`、变内容推号 rev2、正文落 projc.document/document_revision（doc_key 前缀 projc-object-）→ **R-01 机制独立**；
- superseded 无后继触发器拒绝 / 终态拒追加 / document_revision 不可变 / 非 identity 禁删 → **R-02 状态机独立**；
- 通道排除域拒绝、product_cognition 目标 SEC-P2-2 探测、kb_document 钉修订、证据身份去重、判别器 CHECK → **R-07 契约生效**。

**迁移证据**：`migrations/0025_projc_domain_bootstrap.sql`（`#[sqlx::test]` 每用例全新库应用成功=迁移可执行；含 13 表/8 触发器/15 索引/3 字典种子）。

**对象注册表证据**：`docs/architecture/four-domain/object-registry-v1.1-projc-addendum.md`（projc 六对象条目 + pc.conflict LEGACY + snapshot 四类 LEGACY-TRANSITIONAL + DomainRef 契约 §3）。

**零改动证据**：`git status/diff` 仅含 §3 表列文件；`crates/pm-application/src/analysis_report.rs`、`pc/*`、`kb_pc_analysis.rs`、前端均无 diff。

## 8. 验收后动作（产品确认后执行）

1. 台账 §9 状态列：P3-11.1/.2/.3/.5/.4（部分）/P3-05 更新为裁决结果；R-01~R-10 复选框勾选。
2. 解除 P0 → P3-D03/D04 开工（其领域模型直接引用 projc.* 实体族）。
3. 编制 T-1/T-2 实施任务包（含 E2E-02 漂移项与 owner_domain 枚举扩展迁移）。
