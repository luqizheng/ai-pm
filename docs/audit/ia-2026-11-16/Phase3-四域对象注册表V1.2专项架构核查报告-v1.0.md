# 四域对象注册表 V1.2 专项架构核查报告 V1.0

- 日期：2026-11-17 · 对象：《object-registry-v1.1-projc-addendum.md》**V1.2**（开发落实产物，待验收）
- 依据：P3-11 T-1/T-2/T-3 交付代码（master `e9f66ec`）+ 已批准 R-01~R-10（裁决稿 V1.1）+ 此前冻结的四域权威原则（裁决稿 §1 五条边界、§7 两条红线）
- 性质：**只读核查**——不修改裁决、不修改注册表、不扩大实施范围、不 push、不冻结、不宣布验收
- 结论分级：PASS / FAIL / BLOCKED / NEEDS_DECISION；对象实现度按 **IMPLEMENTED（服务层+测试就绪）＜ INTEGRATED（接入真实生产调用链）＜ COMPLETE（INTEGRATED + 门禁/生命周期/幂等并发/审计逐一验证）** 三级严格评定

---

## REG-01 正文权威核查 — **PASS（附措辞建议）**

| 核查点 | 证据 | 结论 |
|---|---|---|
| 1. cognition_object 正文实际存储位置 | 0025:71-87 `projc.cognition_object` **无 content 列**（列=id/tenant/project/object_type/object_key/display_name/lifecycle_status/current_revision_id/superseded_by…）；正文在 `projc.document_revision.content`（0025:98-110），经 `cognition_revision.document_revision_id`（0025:112-128）到达 | ✅ 正文不在身份行 |
| 2. document_revision 职责 | 正文承载 + 内容寻址去重载体；不可变触发器 `trg_projc_document_revision_immutable`（0025:433-434，probe 实测 UPDATE content 被拒）；每修订一行，`content_hash` 同值入 cognition_revision | ✅ 正文版本载体，非身份 |
| 3. cognition_revision.content_hash 语义 | `sha256(markdown)`（projc/mod.rs `sha256_hex`；object.rs 起草/追加修订时与 document_revision.content_hash 同源同值）；**不是**版本身份——版本身份锚 = `cognition_version.snapshot_hash`（成员集规范哈希，publish.rs ⑦先算后插） | ✅ 修订级内容指纹，与版本身份严格分层 |
| 4. Markdown / Git / PostgreSQL 权威关系 | 0025 头注：「正文权威：PG content 列（与 pc/kb 同款已知偏差；Git 权威为目标态，P3-D02.2 冻结裁决：不进 Phase 3 关键路径，禁 PG/Git 双主）」；注册表 V1.2 表述「PG content（…Git=目标态偏差，同 pc/kb 已认账项）」与冻结裁决稿 §1「正文与版本权威 = PG+Revision/Version（ProjC 域），各自单一权威」逐字一致 | ✅ 无新裁决、无双主 |
| 5. 正文 / 版本身份 / 当前状态三分 | 正文=不可变 document_revision 行（hash 锚定）；版本身份=cognition_version（version_no + predecessor 链 + snapshot_hash + source_proposal_id，`forbid_version_mutation` 锚列不可变，0025:337-355）；当前状态=lifecycle_status（终态守卫）+ current_revision_id 指针（行锁 CAS 回填，object.rs:150-160） | ✅ 三者分离，指针可变而锚不可变 |

**REG-01 结论**：实现与冻结裁决一致——PG 是**当前正文载体**（已认账偏差），不因 PG 存正文而新立 PG 为知识权威；注册表措辞沿用裁决稿 §1 冻结表述。**建议（不改自查）**：V1.2 表头「正文权威」列可加半句「（载体；权威=hash 锚定修订链，Git 目标态）」以杜绝误读——交用户裁量。

## REG-02 旧 CognitionPage 权威边界核查 — **PASS（P0 缺陷维持已登记状态）**

| 核查点 | 证据 | 结论 |
|---|---|---|
| ai.cognition_item / ai.project_knowledge 生产写入者 | 全仓 grep `INSERT INTO ai.cognition_item / ai.project_knowledge`：生产代码**仅** `crates/pm-infrastructure/src/repo_cognition.rs:174/265`（`PgCognitionRepo::start_run` 等，经 `apps/pm-api/src/pm_cognition.rs` 调用）；其余命中均为 tests | ✅ 写入者唯一且集中 |
| 读写入口全集 | `apps/pm-api/src/pm_cognition.rs:347-366` 路由：GET overview、GET knowledge（读 ai.project_knowledge）；POST runs（→ai.cognition_run+cognition_item 派生草稿）、POST items/{id}/decision（旧通道确认态翻转 confirm/reject）、POST identity-synthesis（走 pc 正式提案链=identity.rs，不落 ai.*）；前端入口 `ai-frontend/src/api/cognition.ts:70` GET overview；AI 侧消费=读取（ai_context_items.rs SELECT、viz live_graph/resolver 读） | ✅ 无第二个写入口 |
| 正式发布绕过路径 | `decide_item`（pm_cognition.rs:282+）仅 UPDATE `ai.cognition_item.state`——**不写 pc.\* 亦不写 projc.\*，不产生任何 cognition_version 行**；正式技术认知唯一入正式态通道=projc::publish_version（其唯一生产调用方=close_report 双域分流，阶段二 e9f66ec）；新分析链（kb_pc_analysis）对 ai.\* 零写入（grep 证实无双写） | ✅ **旧 CognitionPage 不能绕过 ProjC 正式发布链建立新的正式技术认知** |
| 新旧双写 / 双重权威 | projc.*（新正式链）与 ai.*（旧展示载体）并存=「迁移期事实」——注册表 V1.1 附录即登记「维持现状（CognitionPage 现役）…确认门缺失（P0）记录继续有效，P3-D04 收口」；两域正式版本表互不写入 | ✅ 并存已登记，非新增违规；**旧通道自身 confirm 无治理门（P0）在 P3-D04 前继续有效**——此为已登记缺陷，非本次交付引入 |

**REG-02 结论**：旧通道不可能产生任一域的正式版本；「实际地位 = 迁移期只读展示 + 未治理旧确认面，P3-D04 按 projc.* 目标态收口」与注册表登记一致。**PASS**（P0 缺陷维持登记，不因本次核查升级或降级）。

## REG-03 PC 遗留隔离核查 — **PASS（三层如实区分；DB 层无硬约束=如实登记）**

| 对象 | 新建入口 | worker 写入者 | 绕过服务层路径 | 分层结论 |
|---|---|---|---|---|
| pc.conflict | **无**（grep `INSERT INTO pc.conflict`：生产代码 0 命中，仅 pc_model/pc_proposal 测试夹具直插 L3 冻结场景） | 无 | 测试夹具直插（非生产路径） | 文档标记（注册表 LEGACY）+ 服务层（无写入者，L3 读取面保留=契约 C-4 语义）+ DB 层（状态机 CHECK 在，无写者） |
| pc.object_type snapshot 四类 | 服务层拒（`load_object_type` 仅 active；测试 `snapshot_dictionary_deprecated_fails_closed` 四类型全 Invalid） | **无**（W4 起派生全挂 projc；`kb_pc_repo_flows::verify_triggers_full_kb_pc_chain` 断言 pc snapshot 恒 0 行） | **直插 SQL 可成功**（验收探针 P7：object_type='repository' 落库成功——cognition_object.object_type 对字典无 FK/CHECK） | **文档标记**（0027 UPDATE deprecated + 注册表 DEPRECATED）与**服务层约束**（fail-closed）成立；**数据库约束不存在**——隔离依据=V-6 服务层唯一写面纪律（架构纪律），非 DB 硬约束 |
| 旧数据重新进入正式发布链 | pc 发布链（close C-4 分支）读取 proposal_member **不复检字典**（pc/publish.rs 既有语义，本批按契约逐行不动） | — | — | 生产 pc.* 核心表 **0 行**（注册表 V1.0 迁移红利窗口实证）→ 无存量可复入；新增路径已断（字典拒起草）；残留面=「若未来出现 pc snapshot 提案行，DB/发布层均不拦截」——P3-D04 硬化候选项 |

**REG-03 结论**：隔离在文档层与服务层成立、已被真实 worker 链测试与字典守卫测试证明；DB 层无硬约束（探针 P7 实证）——按任务要求明确区分：**这不是文档标记与服务层约束的混用，三层各自如实呈现**。FAIL 不成立（架构裁决本就依赖服务层写面纪律）；登记两条 P3-D04 候选硬化项（见 §最小修正清单）。

## REG-04 对象逐个定级核查 — **5×PASS，2×NEEDS_DECISION**

| 对象 | 数据结构 | 服务层写入者 | 真实生产调用链 | 权限 | 生命周期/幂等并发 | 审计 | 测试 | **定级** |
|---|---|---|---|---|---|---|---|---|
| OBJECT | 0025:71-87 | `projc::upsert_object_draft` | **kb_pc_analysis::derive_projc_cognition**（worker 全链，阶段二） | 草稿面不判主体（设计）；字典 fail-closed；终态守卫 | object_key 幂等 + 内容寻址去重 + 行锁推号 + CAS 指针（superseded/deprecated 拒复活） | 间接：分析层 `projc_cognition_drafted`（kb_pc_analysis.rs，盘点 2） | projc_model 3/3、projc_formal_chain、projc_derive_switch | **COMPLETE**（对象级审计为分析层间接归因——如实注明） |
| REVISION | 0025:112-128 | 同上（唯一写点随 upsert） | 同上 | 同上 | revision_no 行锁推号；document_revision 不可变（projc_model 断言触发器拒改）；uq 索引族 | 同上 | 同上 + lifecycle_and_immutability_guards | **COMPLETE** |
| VERSION | 0025:132-159 + 0026 闭合触发器 | `projc::publish_version` | **close_report 双域分流**（阶段二）+ 显式补发布 | ensure_formal_authorized（真人+A3，AI 403 测试锁定） | CAS expected_version_no + advisory 锁 + 账本幂等（探针 P2）+ 每身份恰一 Revision + 端点闭合（探针 P4） | 直接：`projc_version.publish`/`publish_failed`（盘点 projc/publish.rs=2）+ outbox `projc.cognition.version_published` | projc_formal_chain（生命周期/CAS/重放）+ close 双域 2 用例 | **COMPLETE** |
| PROPOSAL | 0025:186-242 | create/submit/approve/reject/withdraw/decide + `propose_from_analysis` | **kb_pc_handlers::PcAnalyzeRequestedHandler → projc::propose_from_analysis**（origin_kind='repo_analysis'，阶段二接线） | AI 起草可达；approve/reject=真人+A3；报告 closed 冻结决策 | 审批锚定三方哈希比对 + `uq_projc_approval_pending` + 锚定触发器（探针 P3）+ L3 冻结三处同口径 | 直接：`projc_proposal.*` 助手（盘点 projc/proposal.rs=1） | projc_formal_chain 全生命周期 + formal_writes_fail_closed | **COMPLETE** |
| SOURCE | 0025:246-266 | `projc::register_source` | **derive_projc_cognition**（G-1 双源之二，worker 链） | SEC-P2-2 存在性/同租户同项目探测 + 通道状态校验 + kb_document 钉扎校验 | **行级幂等约束缺失**（无自然键去重；「一次分析一条来源」=调用约定；mark_analyzed 取 latest 兜底） | 间接：分析 after_data.analysis_source_id | projc_model::source_and_evidence_registration + derive_switch | **INTEGRATED**（按严格口径差行级幂等——与 pc::register_source 同语义，非本批回归） |
| EVIDENCE | 0025:268-299 | `projc::register_evidence` | 同上（worker 链） | 主体归属校验 + 判别器 CHECK（库层兜底）+ KB 引用组合键即时校验 | **`uq_projc_evidence_identity` 身份去重**（真实行级幂等） | 间接：分析归因 JSON（description，白名单 refs 过滤断言在 kb_pc_repo_flows） | projc_model + kb_pc_repo_flows（unverified/medium 全量断言） | **COMPLETE** |
| RELATION | 0026:1-96（0001:3293/3327/3369 同构）+ 种子 implements/depends_on | `projc::create_relationship_draft` | **无生产调用方**（全仓 grep：仅定义/导出/测试） | 草稿面不判主体；端点类型合法/跨域 ext 键/自引用/活跃唯一 | uq_projc_relationship_identity + RR 规范哈希；发布链消费面就绪（F-05 并入+闭合触发器，探针 P4/P5） | （草稿面无独立审计——随提案成员进发布审计） | projc_formal_chain::relationship_endpoints_types_ext_keys_and_closure | **IMPLEMENTED**（⚠️ 注册表 V1.2 误标 COMPLETE） |
| CONFLICT | 0026 conflict(+object)（0001:3108/3134 同构） | `projc::create_conflict` / `resolve_conflict` | **无生产调用方**（grep 证实）；但 **L3 冻结消费面已集成**（frozen_by_conflict 于 submit/approve/publish 三处，测试锁定） | 真人+A3（AI 403 测试锁定） | 状态机 CHECK（resolved 需 resolved_by/at；服务层 note 必填 + 非法迁移 409） | `projc_conflict.create/resolve`（pm_write 助手） | projc_formal_chain::conflict_freezes_and_resolves | **IMPLEMENTED**（⚠️ 注册表 V1.2 误标 COMPLETE） |

**REG-04 结论**：OBJECT/REVISION/VERSION/PROPOSAL/EVIDENCE 五项 COMPLETE 成立（均有真实生产调用链+门禁+并发/幂等约束+审计+测试）；SOURCE 定 INTEGRATED（行级幂等=调用约定）；**RELATION 与 CONFLICT 注册表 V1.2 的「COMPLETE」与实际「服务层+测试就绪、无生产调用链」不一致 → NEEDS_DECISION**。

## 与已批准裁决的一致性总结

未发现**裁决级**违背：独立 schema（R-01）、权威边界五条（R-02）、分流判据（R-03）、delivery_activity 停派（R-05）、ProjC 自建 conflict（R-06）、DomainRef 分离（R-07）、正文权威口径（§1/§2.2/P3-D02.2）均与实现一致且经测试锁定。发现的是**注册表 V1.2 措辞与实现度的偏差**（COMPLETE 超评）与**登记项遗漏**（SOURCE 行级幂等、REG-03 DB 层无硬约束），均为描述性问题。

## 最小修正清单（交用户裁决，本次未动任何文件）

| # | 提案 | 影响范围 | 建议 |
|---|---|---|---|
| M-1 | 注册表 V1.2 §1 RELATION 状态行改为「**IMPLEMENTED**（服务层+测试+发布链消费面就绪；生产起草入口随 P3-D04/后续任务接入）」 | 文档一行 | 建议采纳 |
| M-2 | 注册表 V1.2 §1 CONFLICT 状态行改为「**IMPLEMENTED**（L3 冻结消费面已集成 submit/approve/publish；登记/处置生产入口待接）」 | 文档一行 | 建议采纳 |
| M-3 | 注册表 V1.2 §1 SOURCE 行补注「行级幂等=调用约定（一次分析一条来源），无自然键去重（与 pc::register_source 同语义）」 | 文档一行 | 建议采纳 |
| M-4 | REG-03 硬化候选：为 `pc/projc.cognition_object.object_type` 增加 FK→字典（或 deprecated 类型 CHECK/触发器），并考虑 pc 发布链提案成员复检字典 | 迁移+服务层（跨 pc/projc 两域） | **不建议本批做**（属实施扩大）；登记 P3-D04 |
| M-5 | `locales/{zh-CN,en}/pm.yaml` 历史设置页文案仍引用 `src/api/mock/fixtures.ts`（发布产物 i18n 字符串级残留，模块级门禁不受影响） | 文案两行 | 建议随文案清扫批处理 |
| M-6 | 验收补充报告登记的 close 主事务「无独立故障注入用例」 | 测试补强 | 可选，登记即可 |

## 状态声明

- 本报告为只读核查产物；未修改裁决、未修改注册表、未修改实现、未 push、未冻结、未宣布验收通过。
- 证据基线：master `e9f66ec`；探针与测试均在真实 DB `127.0.0.1:16433` 执行（详见《Phase3-P3-11-T1T2T3-验收补充报告-v1.0.md》）。
