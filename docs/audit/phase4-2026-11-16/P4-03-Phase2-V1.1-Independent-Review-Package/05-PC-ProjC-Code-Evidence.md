# 05 — PC / ProjC 当前代码架构证据（C 节 14 项）

**证据基准**：master `e05693afdc0a9ae534735c9b6f67117a6b92de8d`（分支 master）。行号以该 HEAD 为准。本包只读取证，**未修改任何文件**。
**总判读**：以下 14 项中凡标注 `DESIGN_ONLY` 者，仅为 P4-03 V1.1 的目标态概念，代码中不存在；凡给出路径行号者为已实现现状。V1.1 对现状的引用经逐项核对的结论汇总见 §15。

---

## C.1 PC 领域模块目录结构（已实现）

`crates/pm-application/src/pc/`（6 文件，3798 行）：

```
evidence.rs      266 行   来源与证据登记（PC-D-05）
mod.rs           335 行   门面：错误域/规范哈希/A3 门禁/类型注册校验
object.rs        510 行   对象草稿与修订推号、Profile/Vision 单例
proposal.rs     1541 行   提案/审批/裁决/撤回命令
publish.rs       725 行   版本发布事务（十四步）
relationship.rs  421 行   关系草稿与端点解析（跨域通道）
```

关联：契约 crate `crates/kb-pc-contracts/src/`（domain_ref/registry/events/ports/envelope/retrieval）；close 分流 `crates/pm-application/src/analysis_report.rs`；identity 链 `crates/pm-application/src/identity.rs`（339 行）。

## C.2 ProjC 领域模块目录结构（已实现，P3-11 落地）

`crates/pm-application/src/projc/`（7 文件，3963 行）：mod.rs 345 / object.rs 409 / proposal.rs 1539 / publish.rs 725 / relationship.rs 420 / **conflict.rs 282** / evidence.rs 243。与 pc 同构但独立（表/字典/审计/事件全独立）；镜像声明原文：

- `projc/proposal.rs:1-2`：「ProjC 提案与审批命令（pc/proposal.rs KP-2 / PC-01C 同款镜像，真源 = pc 同名文件…）」
- `projc/publish.rs:1-3`：「ProjC 版本发布事务（pc/publish.rs 01B §18.1 同款镜像，真源 = pc 同名文件…）」
- 红线一表述（0025 迁移头注 L7-10）：「复用 pc.* 已验证的机制模式（内容寻址 + CAS 推号、行不可变触发器、生命周期 CHECK、提案锚定、幂等账本），不复用业务表（红线：共享技术基础设施不得产生共享业务权威…）」

## C.3 Product 主体模型及其与 core.product 的关系

**`DESIGN_ONLY`（全链）**：core.product 表不存在（migrations 27 文件 `CREATE TABLE` 无 product 表；运行库 `aipm_mig_full` pg_class 核查 core.product 不存在）；无 Product Rust 模型（`struct Product`/`ProductStatus` 0 命中）；无 Product Owner 角色（0 命中）。P4-01 仅设计冻结 @ `002c825c`（三文件纯 .md diff 实证）。现行「product 载体」唯一存在形态 = `pc.cognition_object` 的 product_profile/product_vision 两类 identity 态单例（bootstrap.rs:323-333 → pc/object.rs:322-357，每项目一份）。V1.1 §2.2/§7.1 对 P4-01 DR-03 的引用（身份分离）与冻结文本一致（见 09 号文件）。

## C.4 Project 主体模型及其与 core.project 的关系（已实现）

`core.project`（`migrations/0001_baseline.sql:1251`）：id uuid PK、customer_id NOT NULL、code（UNIQUE(tenant_id,code)）、name、status DEFAULT 'planning'、revision、tenant_id。生命周期：`archived` 归档事务 `crates/pm-application/src/archive.rs:1-17`（status→archived+审计 `project.archived`）；Team dissolve 软删 `team_onboarding.rs:1232` 起（status='deleted'）。**pc/projc 全部表的 project_id 为软引用**（无 FK；对照：integration.repo_analysis_report 0023:16 有显式 FK→core.project，证明 pc/projc 有意不用 FK）。**C-01 前置（P4-02 DR-02 要求 core.project 补 UNIQUE(tenant_id,id)）未实施**——现状无该组合唯一约束（P4-02 未实施的一致结果）。

## C.5 PC 当前认知对象模型、类型注册和作用域（已实现）

- 三层模型（0001_baseline.sql）：`pc.cognition_object`（:3041，身份层；`(tenant_id,project_id,object_type,object_key)` 定位；lifecycle CHECK identity/draft/active/deprecated/superseded）→ `pc.cognition_revision`（:3064，内容寻址修订；revision_no≥1；analysis_mode derived/derived_fallback/llm）→ `pc.cognition_version`（:3087，发布快照；version_no 唯一、snapshot_hash、predecessor_version_id）。
- **作用域：仅 project**。project_id NOT NULL；全 pc 表无 product_id（grep 零命中）；服务层由 project 反查 tenant（object.rs:68、publish.rs:93 `SELECT tenant_id FROM core.project WHERE id=$1`）。→ V1.1「现状=项目私有 scoping」（审计 F-3/J1）**与代码一致**。
- 类型注册：字典表 `pc.object_type`（0001:3220）；服务层 `load_object_type`（pc/mod.rs:213-230）仅取 `status='active'`，未注册类型→`Invalid`（fail-closed）。种子 18 行（0001:10255-10272）：14 类 identity 字典（product_profile/product_vision 单例 + business_domain/business_capability/feature_module/feature/actor/scenario/business_process/page/interaction/domain_object/business_rule/product_decision）+ 4 类 snapshot（0027 已废弃 status='deprecated'）。运行库实证：active=14、deprecated=4。

## C.6 ProjC 当前认知对象模型、类型注册和作用域（已实现，P3-11）

`projc.*` 18 表（0025 建 13 + 0026 建 5），与 pc 同构。字典恰好 3 类（0025:60-63）：repository（L1_identity）/tech_stack（L2_structure）/repository_module（L2_structure，注释「专家 R-03 消歧：非 PC Product Module」）；`object_type_singleton()` 恒 false（V1 无单例技术对象）。**projc 域全部 18 表零 FK**（0025/0026 grep FOREIGN KEY|REFERENCES 零命中），纯 CHECK+触发器+部分唯一索引；且 **cognition_revision 无不可变触发器**（0025 仅建 document_revision/version/write_op 等不可变触发器；对照 pc 有 trg_pc_revision_immutable 0001:7043）——审计 V1.1 J5/P2-F「约束强度不对称」**与代码一致**。关系字典 2 条（0026:47-53）：implements（repository_module→pc.feature_module 跨域，owner_domain CHECK 单值 'project_cognition'）、depends_on（域内）。

## C.7 PC / ProjC identity 路由与解析入口（已实现）

- **D-4 身份综合链**：`identity.rs:1-5` 头注「D-4 · 业务身份综合：输入装配与三提案落地（pc product_profile/product_vision…）」。入口 `propose_pc_object`（:140-207）：`upsert_object_draft`（object_type=product_profile/product_vision，object_key=product-profile/product-vision）→`create_proposal`（origin_kind='ai_agent'，挂当前 open 分析报告 source_id，:219-230 无 open 报告即拒）→`submit_proposal`。`apply_identity_drafts`（:211-339）：profile/vision 走 pc 提案；goal 走知识 8 态管线；审计 `identity_synthesis.applied`（:325）。LLM 侧 `pm-ai/src/identity_synthesis.rs:53` `synthesize`（开关 `AI_PM_IDENTITY_SYNTHESIS`，off→501）。
- **单例骨架**：`bootstrap.rs:323-333` 调 `ensure_product_singletons_in`（实现 pc/object.rs:322-357；identity 态、无修订、ON CONFLICT DO NOTHING 可重入，依赖部分唯一索引 `uq_pc_object_singleton` 0001:6851-6854）。**V1.1 §8 引用「bootstrap.rs:325 现状」行号准确。**
- ProjC 起草入口：`projc/proposal.rs:1399` `propose_from_analysis`（origin_kind='repo_analysis'，advisory 锁+候选对象自动成成员+同事务 outbox `projc.change_proposed`）。
- **路由分流目标态（PC-P 链）**：`DESIGN_ONLY`。

## C.8 DomainRef 定义、序列化、解析、校验和错误处理（已实现）

- 契约定义 `kb-pc-contracts/src/domain_ref.rs:19-29`：`pub struct DomainRef { target_domain: String, target_type: String, target_id: String, target_revision_id: Option<String> }`；头注（:1-13）「唯一持久化引用结构…有 Revision 机制的对象必填 revision 引用，空缺必须记录原因（原因记录归各领域承载表，如 pc.source.revision_missing_reason，本类型不做 reasons 表达）」；ADR-07 service 字段禁令；serde wire 形态冻结（`target_revision_id` 缺失序列化显式 null，测试 :109-136）。**契约文档层（P3-D04-02 V0.6 冻结）第四字段抽象名为 `revision_context`，`target_revision_id` 为其 Pinned 模式存储落位，映射统一=O-09（开放）**（详见 07 号文件）。
- 注册表 `registry.rs`：TargetDomain 9 值（:35-58）：requirement/project_goal/knowledge_base(Open)、project_cognition(**ReadOnly**，迁移期实体族 ai.cognition_*→目标态 projc.*)、`pc`=ProductCognition(Open)、design/adr(Open)、product_reality(NotYetOpen)、product_plan(**PermanentlyExcluded**，Q-PC-13)；`resolve_reference`（:143-155）按通道状态裁决；`parse_domain` 失败→`RegistryError::UnknownDomain`（"alert, do not guess"）。
- 解析层（无单一统一 resolver）：①`reference_targets.rs:53-77` `domain_target`（domain+type→物理表；ProductCognition→pc.cognition_object；:38-48 `allows` 跨租户/跨项目拒绝、逻辑锚 strict=false 放行）；②`discussion_memory.rs:103-217` Platform 侧 resolve_refs（即查即拒）；③`kb_service/memory.rs:171-257` KB 侧 validate_refs（:192-197 revision 与 missing_reason XOR 校验）；④ProjC 关系端点 `projc/relationship.rs:47-215` `resolve_endpoint`（对象存在+生命周期+revision 归属校验；跨域端点按通道状态+存在性探测；requirement/knowledge_base 强制修订级）。**完整跨 schema resolver=Partial（R-04④，O-01 归 P3-10）——冻结令明文不得写成已完成**。
- 错误处理：`PortError::NotFound`（ports.rs:27-28）；`RegistryError`（registry.rs:123-129）；R37 四分支（DENIED/REV_MISSING/NOT_FOUND）为 **V0.6 冻结契约，代码未实现**（DESIGN_ONLY at code level）。

## C.9 Link / 引用关系模型及创建、读取、删除/解除路径（已实现）

- **P4-02 product_project_link：`DESIGN_ONLY`（零代码）**——crates/migrations `product_project_link|ProductProject` 0 命中（仅 4 份 P4-02 文档命中）。设计与裁决已冻结 @ `0a0e0e12`（见 09 号文件）。
- **域内引用现状 = pc.relationship / projc.relationship**（0001:3293、0026:64）：端点二选一 CHECK（对象钉扎 `(source/target_object_id, *_revision_id)` vs 跨域四元组 ext_domain/type/id/revision_id）；创建 `pc/relationship.rs:12-25` PcEndpoint 枚举+resolve_endpoint 校验；活跃身份唯一 `uq_pc_relationship_identity`/`uq_projc_relationship_identity`；**禁删触发器**（trg_pc_relationship_delete 0001:7036；projc 0026 有身份唯一索引但无 delete 触发器——projc relationship 可被 DELETE，属约束不对称另一实例）；解除=生命周期 superseded（superseded_by_id CHECK）。pc.relationship **生产起草入口不存在**（审计 J6：生产代码无 INSERT；仅测试）。引用零负载事实（J7）：pc.source=0 行、运行库实测 0 行。
- 消费引用/钉扎引用（对 cognition_version 的 pin）：**`DESIGN_ONLY`**（无 pin 表/列；唯一 pin 语义=任务知识锚 `pinned_revision_no`（0014/task_knowledge.rs:90）与提案成员 pin，均非产品认知消费引用）。

## C.10 版本、发布、Pinned、LatestPublished 相关实现（已实现部分）

- **发布事务（十四步）**：`pc/publish.rs:68-75` 门禁链注释=顺序声明；幂等账本 `pc.write_operation`（:110-179，completed 重放返回既有版本）；CAS（:264-317，expected_version_no 失配→同事务置 failed+审计 `pc_version.publish_failed`+COMMIT 返回 Conflict）；`snapshot_hash=canonical_members_hash`（mod.rs:113-122：成员排序后 `kind:identity_id:revision_id` 拼接 sha256；先算后插 :450-451）；成员集携带-替换-并入与每身份恰一修订断言（:347-448）；闭合断言（:503-535）；同事务 outbox `PC_COGNITION_VERSION_PUBLISHED`（:564-594，payload 含 version_ref/proposal_ref 两 DomainRef+snapshot_hash，幂等键 `pc_publish:{version_id}`）；提案 applied+生命周期推进（:596-685）。projc/publish.rs 全同构（事件 PROJ_COGNITION_VERSION_PUBLISHED，幂等键 `projc_publish:`）。
- **版本不可变**：触发器双保险（trg_pc_version_delete 0001:7050；trg_pc_version_immutable :7057，逐列比对 version_no/confirmed_by/source_proposal_id/snapshot_hash/predecessor_version_id）；同类：revision/vm/approval/proposal_anchors/object_lifecycle 等 17 触发器（见 06 号文件 §8）。projc 侧 cognition_version 有不可变触发器（0025:435）但 cognition_revision **无**（见 C.6）。
- **Pinned / LatestPublished：`DESIGN_ONLY`（协议层）**——kb-pc-contracts 全 crate 无 pinned/latest 语义；唯一近似实现=任务知识锚 COALESCE(pinned,latest)（run_context.rs:466-482，属 task_knowledge 通道非 DomainRef）；索引侧「仅最新 published 被索引」测试（tests/retrieval_index.rs:121）。**冻结契约已有定义**：V0.6 §2.2（Pinned=钉扎具体 Revision；LatestPublished=动态选择器，仅限非正式依据）——见 07 号文件。

## C.11 Evidence、Source、快照及 provenance 实现（已实现部分）

- `pc.source`（0001:3391：target 四元组+source_kind 8 值 CHECK+revision_missing_reason XOR CHECK :3410）；`pc.evidence`（0001:3182：7 值 evidence_type、evidence_hash 必填、subject XOR、locator 分型 CHECK×5）。服务层 `pc/evidence.rs`：register_source（:27-135，SEC-P2-2 存在性探测+kb_document 强制钉修订）、register_evidence（:166-266，主体归属+组合键即时校验，注释「写入时点；存续期对账另批」）。projc 双份同构（projc/evidence.rs，Stage1 证据主体仅 ObjectRevision）。
- **证据快照（P4-03 §7.8-E 概念）：`DESIGN_ONLY`**——无「提交时固化内容副本」的表/机制；现有 evidence 是「指向源的四元组+哈希」而非内容副本。检索侧 provenance 元数据契约已存在（retrieval.rs:541-570 RetrievalEvidence）。
- **PC-J provenance 三分类（project_original/derived/variant）：`DESIGN_ONLY`**——无任何列/字段。

## C.12 派生认知与局部差异实现

**`DESIGN_ONLY`（全链）**：无派生/差异对象、无 derives_from、无 divergence_note（全仓 grep 零命中）。现状最近的类比物：pc.cognition_revision.analysis_mode（derived/derived_fallback/llm）=分析来源模式，非跨对象派生关系。V1.1 §7.4 三分类无代码对应——与「本切片不实施」一致。

## C.13 AI 读取、上下文注入、消费授权与人工确认入口（已实现部分）

- **AI 正式写入恒拒**：`requirement.rs:203-227` `require_human_conn`（kind≠human→Forbidden）；`pc/mod.rs:151-176` `ensure_formal_authorized`（"AI is fail-closed"）；projc 同构（projc/mod.rs:168）；REST 映射 `apps/pm-api/src/knowledge_services.rs:121/134`（PcError/ProjcError::Forbidden→403）+`support.rs:40`；决策端点叠加 `ensure_trusted_context`（API Key 403，knowledge_services.rs:602-603/653，MCP 不暴露）；AI 写回白名单 `pm-ai/src/discovery_writeback.rs:44-60`（AI_FORBIDDEN_COMMANDS 6 项；红线「AI 永不可达 approved…require_human 是第二道」）。
- **上下文注入**：任务级 `run_context.rs`（get_run_context_with_injection :380 起；knowledge_sections :523；discussion_memory 开关默认 off）；会话对象级 `ai_context_items.rs`（六类 OBJECT_TYPES :35-42；knowledge 类型注入**直接读 `ai.project_knowledge`** :1159-1170；Pending→Active 真人门 :754）。**`ai.project_knowledge` 旧通道现役**：表仍在（0001:1101，无 DROP 迁移）、读写活跃（repo_cognition.rs:86/139-148/265、ai_context_items、repo_baseline.rs:162）、REST `GET /v1/pm/projects/{id}/cognition/knowledge`（ai-frontend/src/api/cognition.ts:95-100）；前端 `CognitionPage.vue` 在役（`views/project/cognition/CognitionPage.vue`，路由 navigation.ts:325-329）。→ **P3-D04 D4 REPLACE（含 ai.project_knowledge REMOVE）未实施**，方向已裁、收口归 O-05（见 07 号文件 §d）。
- **消费授权：`DESIGN_ONLY`**（consumption_grant 0 命中）。
- **AI 读 PC-P**：对象不存在（PC-P 不存在）；AI 跨项目自由读取=不开放（现状检索入口 `POST /v1/retrieval/query` 先 `require_project_visible`，pm-api/src/retrieval.rs:55-102；统一检索实现归 P3-10/O-07）。

## C.14 生命周期状态定义、合法状态转移和清扫逻辑（已实现部分）

- **认知对象/提案状态机**：对象 lifecycle CHECK（identity/draft/active/deprecated/superseded，0001:3056；enforce_object_lifecycle 触发器 :6987）；提案 8 态 CHECK（0001:3258）+转移实证全集：draft→submitted（proposal.rs:810）；submitted↔approved↔rejected 暂记互改（:1063-1066）；draft/submitted→withdrawn（:1181-1183；close 批量同集 analysis_report.rs:615-635）；approved→applied（publish.rs:597）；submitted/approved→failed（publish.rs:277）；**expired 无生产写入路径；approved→withdrawn 无任何代码路径**。
- **Project archived**：archive.rs（状态翻转+审计；认知数据不动）。
- **Team dissolved**：`team_onboarding.rs:1232` `dissolve_team`（owner 独占→项目软删→删 project_member→清邀请/链接/领用→删 tenant_member→租户休眠→审计 `team.dissolved`）。**对 pc/projc/kb 认知数据零处置**（该函数 grep `pc.|projc.|kb.` 零命中）——认知行原位留存；**认知清扫服务不存在**。P4-02 DR-06 清扫扩展（Link 置 ended）为实施期登记项未实施。
- **Product archived/restored/dissolved：`DESIGN_ONLY`**（无 Product 实体）。
- update-pending/再基线：**`DESIGN_ONLY`**（update_pending/rebaseline 全仓 0 命中）。

---

## 15. V1.1 现状引用逐项核对汇总（V1.1 章节号→代码事实）

| V1.1 引用 | 代码事实 | 判定 |
|---|---|---|
| §2.1「现状 pc.*（18 表，项目私有）」 | pc 18 表实证；project_id NOT NULL | ✅ 一致 |
| §7.2「提案 8 态+审阅流 v2 暂记语义+发布事务十四步同构」 | 8 态 CHECK、record_decision 暂记（decide_proposal published_version 恒 None）、publish.rs:68-75 十四步注释 | ✅ 一致 |
| §8「bootstrap.rs:325 现状」 | bootstrap.rs:323-333 调用块，:325 位于其中 | ✅ 行号准确 |
| §7.5「现有项目级草稿通道原样保留（pc::upsert_object_draft 同口）」 | upsert_object_draft 自始独立存在，identity 链与其同口调用 | ✅ 一致 |
| §7.5「现状 close_report 批量发布全部 approved pc 提案」 | analysis_report.rs:588-607 逐个 publish_version | ✅ 一致 |
| §7.9「close_report 批量撤回先例已覆盖含 approved 的未决态撤回」 | close 撤回块 WHERE `status IN ('draft','submitted')`；approved→applied；approved→withdrawn 无代码路径 | ❌ **不符**（登记至 11 号文件 D-1） |
| §9.1-H2「四字段+missing_reason 契约原样」 | domain_ref.rs 四字段+pc.source XOR CHECK 在位；契约抽象名 revision_context↔存储 target_revision_id 映射=O-09 开放 | ✅ 基本一致（命名映射开放项需在 C5 核查中显式处理） |
| 附录「机制镜像依据=pc/mod.rs、projc/mod.rs 头注」 | projc/mod.rs:80「机制复用 = 架构红线一允许项」；0025 头注红线表述 | ✅ 一致（逐字串「共享技术基础设施模式≠共享业务权威」不在 .rs，出处为 0025 迁移头注与设计文档） |
| §11.2「0026/0027 方案 C 零触碰→完全兼容」 | 0026=projc 5 表+双源列；0027=4 行字典 UPDATE；均与 PC-P/PC-J 增量正交 | ✅ 一致 |
