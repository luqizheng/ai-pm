# P3-D04-02 Step 3 与 Schema 差距分析依赖核查报告 V1.0

- 日期：2026-11-17
- 任务：P3-D04-02 Step 3 与 Schema 差距分析依赖核查（只读；冻结后、Step 3 启动前）
- 执行：开发 Agent（ZCode）；**本轮零实施**——未修改任何代码/Schema/Migration/API/测试，未执行迁移
- 冻结基线：`docs/audit/ia-2026-11-16/P3-D04-02-第一批目标领域模型设计-v0.6.md`（2026-11-17 Step 1+2 冻结，批准令=`P3-D04-02-Step12冻结批准令-v1.0.md`）
- 证据约定：每个结论标注【已确认】/【部分确认】/【未确认】/【需人工裁决】；证据=文件路径+符号+行号+Migration+活库只读抽查

---

## 1. 任务背景及冻结基线

Step 1 + Step 2 已于 2026-11-17 冻结（R37 维持通过、R38/R36 完整性通过）。冻结令规定下一项正式工作=「Step 3 与 Schema 差距分析依赖核查」，产出执行顺序裁决输入。本报告即该核查产出：只调查、只分类、只建议，不实施。

**冻结保护声明**：本报告全程以 V0.6 为前置边界；现有代码/Schema 与冻结契约不一致之处一律登记为**实现差距**（含 F-01~F-04 已登记闭合项），不构成对冻结契约的重开理由，也不重新包装为 Step 1/2 缺陷。

## 2. 核查范围与排除范围

**纳入**：`pc.*`/`projc.*` 两 Schema（18+18 表）、`crates/pm-application/src/pc|projc`（7,761 行）、写入调用链（`pm-infrastructure/src/kb_pc_analysis.rs`、`analysis_report.rs` close 分流、`pm-ai` 分析链）、HTTP 层（`apps/pm-api`）、migrations 0001/0007/0009/0025/0026/0027、测试（pc_proposal.rs 8 用例、projc_formal_chain.rs 6 用例、audit_actor_coverage.rs）、活库只读抽查（aipm_mig_full）。

**排除**：KB/Analysis/Discovery/Design/Plan/Delivery 域内部实现；前端；Step 3 正式设计本身；任何写操作（含 docker exec 仅 `psql -Atc SELECT` 只读）。

## 3. PC 现状代码与 Schema 盘点

### 3.1 Schema（migrations/0001_baseline.sql:3021-3470 + 5366-5370 + 5746 + 6693-7064）

| 表 | 关键列/约束 | 证据 |
|---|---|---|
| cognition_object | lifecycle_status CHECK ∈ {identity,draft,active,deprecated,superseded}；current_revision_id、superseded_by_id、deprecated_at；**UNIQUE(tenant,project,object_key)**（0001:5366）；禁删触发器（identity 态外） | 0001:3041/5366/6980/6987 |
| cognition_revision | revision_no≥1、content_hash、analysis_mode CHECK；**全拒 UPDATE/DELETE 触发器** | 0001:3064/7043 |
| cognition_version | status∈{published,archived}；**confirmed_by+confirmed_at 决策锚列**；锚列不可变触发器（forbid_version_mutation，published→archived 单向） | 0001:3087/7050/7057 |
| proposal | status 8 值 CHECK（draft→submitted→approved/rejected→applied/failed，withdrawn/expired）；expected_version_no、proposal_content_hash；draft 外内容锚列不可变触发器 | 0001:3240/7008 |
| approval | status∈{pending,approved,rejected}；**decided_by/decided_at/decision_reason 决策事实列**；触发器：锚列不可变+pending→终态单向+已决不可改（forbid_approval_mutation） | 0001:3021/474/6966 |
| write_operation | operation_id；命令 CHECK={pc_commit_object_revision,pc_commit_relationship_revision,pc_publish_version}；状态 CHECK={pending,git_committed,completed,failed,abandoned}；**UNIQUE(tenant,project,operation_id)**（0001:5746）=幂等账本 | 0001:3453/5746/458 |
| evidence | **verification_status CHECK∈{unverified,verified,refuted,conflicted}**；uq_pc_evidence_identity 幂等唯一索引 | 0001:3182/6844 |
| source | 四字段 DomainRef 形（target_domain/type/id/revision_id）+revision_missing_reason | 0001:3391 |
| relationship(+revision/type) | 活跃身份唯一 uq_pc_relationship_identity（0001:6865）；revision 全拒改删；成员端点闭合触发器 | 0001:3293/3327/7015/7022 |
| version_member | XOR CHECK；uq_pc_vm_object/relationship；DEFERRED 端点闭合 CONSTRAINT TRIGGER | 0001:3418/7064 |
| conflict(+object) | 6 态状态机 CHECK；resolved/dismissed 需 resolved_by+resolved_at | 0001:3108 |
| **FK** | pc schema 内 **0 条 FOREIGN KEY**——跨对象一致性全部由触发器+服务层承担 | 0001:3021-3470 grep REFERENCES=0 |

### 3.2 状态机（服务层唯一推进面；crates/pm-application/src/pc）

- **提案**：`draft ─submit→ submitted ─approve→ approved ─publish→ applied；submitted ─reject→ rejected；draft/submitted ─withdraw→ withdrawn；submitted/approved ─CAS失配→ failed`（proposal.rs:5-10 模块文档）。
- **对象**：identity→draft（首修订）→active（首次入正式版本）；→deprecated（提案废弃，终态）；superseded 须带 successor（触发器强制）。终态禁再变更（触发器 enforce_object_lifecycle）。
- **审批**：pending→approved/rejected（单向、已决不可改）；**重决=新插 approval 行、旧行留痕**（proposal.rs:983-985）——append-only 决策历史，与 R36 契约同构。
- **冲突（L3）**：detected/under_review/awaiting_human/reopened=冻结圈；resolved/dismissed=解冻（mod.rs `frozen_by_conflict`）。
- **版本**：published→archived 单向（触发器）。

### 3.3 代码行为（实际调用链）

| 操作 | 事务内容 | 并发控制 | 证据 |
|---|---|---|---|
| upsert_object_draft | **单事务**：object→document→document_revision→cognition_revision→指针回填 | object 行 FOR UPDATE 推号（§17.2-1）+ 指针 CAS（`IS NOT DISTINCT FROM $4`）+ DB object_key 唯一约束兜底；内容寻址去重（同 hash 复用） | pc/object.rs:75-272/132-138/153-163 |
| create/submit proposal | 单事务；submit 写 pending 审批行（hash+expected_version_no 锚）；L3 冻结前置 | uq_pc_approval_pending 兜底并发提交 | pc/proposal.rs:716-868 |
| approve/reject/decide | 单事务：proposal FOR UPDATE→approval pending FOR UPDATE→**三方哈希重算比对**→L3（approve）→approval 终态化→proposal 翻转→审计→commit；同态重放幂等返回 | 行锁+expected_version_no CAS+行数断言（rows_affected≠1→409） | pc/proposal.rs:937-1096 |
| propose_from_analysis（AI 链起草） | 单事务：advisory(`pc_analysis_proposal:{project}`)→候选排除（已入最新版本/已在开放提案）→create+submit→outbox（idempotency_key=`analysis_outcome_key(task_id)`） | 项目级 advisory 串行化+候选排除 | pc/proposal.rs:1401-1539 |
| **publish_version（十四步）** | **单事务**：advisory(`pc_publish:{project}`)→write_operation 幂等账本（ON CONFLICT DO NOTHING，completed 短路幂等返回）→proposal FOR UPDATE→approval FOR UPDATE+库内重算哈希→（A3 门在事务外，H-11 已知窗口）→**CAS version_no==expected_version_no**（失配→提案内置 failed+审计+commit+409）→L3 冻结→成员集构建（携带−废弃−替换+端点并入，每身份恰一 Revision 断言）→snapshot_hash 先算后插→version+members→闭合断言→**审计同事务**→**Outbox 同事务**（PC_COGNITION_VERSION_PUBLISHED）→proposal applied→账本 completed→对象/关系生命周期推进→COMMIT | advisory+双 FOR UPDATE+CAS+唯一约束+行数断言 | pc/publish.rs:87-695（门禁链注释 72-75） |
| close_report（审阅流 v2） | **外层事务**（report FOR UPDATE）+ 双域分流：每 approved 提案调 publish_version=**独立子事务**（幂等键 `pc-decision:{id}`/`projc-decision:{id}`），失败计数不阻断；未决提案 withdraw 于外层事务+逐条审计 | 报告行锁；子发布自带 advisory/幂等；withdraw(pending 集)与 publish(approved 集)行集不相交 | analysis_report.rs:548-680 |

- **人工门**：正式写入（approve/reject+publish）=真人主体+A3（Lead OR core.approver_grant），AI/服务主体恒 403（mod.rs:151-179 `ensure_formal_authorized`；fail-closed）。
- **审计**：全部原事务内写 governance.audit_log（四身份列全写）；覆盖由静态清点测试守卫（audit_actor_coverage.rs PRODUCTION_SITES 清单）。
- **写入口现状**：HTTP 层仅**读路由**（`GET /v1/pc/objects`、`/v1/pc/proposals`、`/v1/pc/analyses/{id}`，apps/pm-api/src/knowledge_services.rs:74-86）；**approve/reject/decide/publish 无任何 HTTP/MCP 入口**；`PcServicePort::decide_proposal`=NotWired（kb-pc-contracts/src/ports.rs:387）。生产写路径仅两条：分析链自动起草（identity.rs:161-203、kb_pc_handlers.rs:304）+close_report 批量发布。【已确认】

## 4. ProjC 现状代码与 Schema 盘点

### 4.1 Schema

- **0025_projc_domain_bootstrap.sql**：13 表（object/document/document_revision/cognition_revision/cognition_version/version_member/write_operation/proposal/proposal_member/approval/source/evidence/object_type）+触发器+索引；显式复用 pc 机制模式（行不可变触发器、生命周期 CHECK、提案锚定、幂等账本），不复用业务表（R-01/R-02 独立权威）。有意差异（0025 头注登记）：origin_kind 增 `repo_analysis`；source_kind 以 product_cognition/repo_analysis 替换 project_cognition；write_operation 命令 `projc_*`；`object_type_singleton` 恒 false（V1 无单例）；无 snapshot 字典层种子（R-05）。
- **0026_projc_formal_chain.sql**：补齐 relationship_type（+owner_domain CHECK='project_cognition'，种子 implements/depends_on 跨域端点走 `pc.feature_module` 键）/relationship/relationship_revision/conflict(+object)（R-06：ProjC 自建，pc.conflict=Legacy）；成员端点闭合 CONSTRAINT TRIGGER（DEFERRED）；**0025 两处缺陷修正**（proposal_member 锚定触发器 BEFORE→AFTER CONSTRAINT 重绑——原绑定会静默吞掉全部成员插入；write_operation 补幂等唯一约束）；integration.repo_analysis_report 加 projc_source_id。
- **0027**：pc.object_type snapshot 四类字典废弃（fail-closed：load 仅取 active）。
- 活库抽查（aipm_mig_full，只读）：triggers pc=22/projc=12；行数 pc_obj=1（repository 遗留，V1.1 已载）、pc_ver=0、pc_prop=0、projc_obj=2、projc_prop=1——迁移红利窗口近零数据。【已确认】

### 4.2 代码

`crates/pm-application/src/projc` 为 pc 同构镜像（publish.rs 头注：「真源 = pc 同名文件；域内改名」）；diff 统计：publish 150 行//object 193 行/proposal 312 行差异——均为表前缀、advisory 键名（`projc_publish:`/`projc_analysis_proposal:`）、事件名、错误类型、origin_kind='repo_analysis' 的域内改名，事务结构逐句同构。【已确认（diff 比对）】

**ProjC 独有（PC 没有的）**：`projc/conflict.rs`——create_conflict/resolve_conflict 完整冲突服务（resolve 走 ensure_formal_authorized+FOR UPDATE+合法转换校验，conflict.rs:204-240）。

**PC 独有（ProjC 没有的）**：product_profile/product_vision 单例骨架（ensure_product_singletons，object.rs:322-360）；uq_pc_object_singleton；v_revision_versioned 视图+FTS（0009；projc 侧归 P3-10 前置任务，0025 头注登记）。

### 4.3 ProjC 写入口

分析链：kb_pc_analysis.rs:890-970（pc::register_source→projc::register_source→projc::upsert_object_draft→projc::register_evidence）→kb_pc_handlers.rs:304（outbox 事件→projc::propose_from_analysis）→close_report 双域分流（analysis_report.rs:657 projc::publish_version，幂等键 `projc-decision:{id}`）。审批入口同样 NotWired。【已确认】

## 5. 状态机现状及差异

| 维度 | PC | ProjC | 差异定性 |
|---|---|---|---|
| 提案状态机 | 8 态+服务层唯一推进面 | 同构镜像 | 无实质差异 |
| 对象生命周期 | 5 态+终态保护 | 同构 | 无实质差异 |
| 决策事实 | approval 多行追加+重决留痕；version.confirmed_by/at 锚定 | 同构 | 无实质差异（与 R36/R38 契约一致：outcome=status 显式列、决策时间=decided_at 独立列、非状态推导）【已确认】 |
| 冲突治理 | pc.conflict=**Legacy Not for New Use**（R-06）；L3 冻结查询在位（frozen_by_conflict）但**无新建冲突入口**（pc 模块无 conflict.rs）——当前 L3 对 PC 实际为「空集恒通过」 | 自建冲突对象+完整 create/resolve 服务 | **实质差异**。PC L3 机制在位但冲突源 Legacy 化；O-06 归口 S5。Step 3 须表态：PC 状态机是否保留 L3 分支及冲突源归属【需人工裁决（不重开 R-06，仅澄清现状事实）】 |
| 人工确认入口 | approve/reject/decide **无生产接线**（NotWired+HTTP 仅读路由） | 同样 NotWired | 同构缺失。状态机内部已完备（含门禁），外部触发面缺失 |
| 版本状态 | published→archived 单向（触发器） | 同构 | 无差异 |

## 6. 事务边界现状及差异

| 操作对 | 原子性现状 | 证据 |
|---|---|---|
| 创建对象+初始化关联（doc+rev） | **同事务**（object+document+document_revision+cognition_revision+指针） | pc/object.rs:205-272 |
| 创建 Revision+更新当前版本指针 | 同事务+行锁推号+CAS | pc/object.rs:132-163 |
| 人工确认+Decision 事实 | 同事务（approval 终态化+proposal 翻转+审计） | pc/proposal.rs:1047-1096 |
| 决策结果+状态翻转 | 同事务（发布内生命周期推进） | pc/publish.rs:620-685 |
| 状态翻转+审计 | 同事务（十四步⑫⑫b+⑬） | pc/publish.rs:537-618 |
| 多对象关联更新（版本闭合） | 同事务：成员集构建+每身份恰一 Revision 断言+服务层闭合断言+DEFERRED 触发器兜底（双保险） | pc/publish.rs:347-535；0026:§4 |
| AI 提案物化 | 起草段=单事务+advisory+候选排除+outbox 幂等键；发布段=write_operation 账本幂等 | pc/proposal.rs:1401-1539；pc/publish.rs:110-179 |
| 失败回滚/重复请求 | CAS 失配=同事务内置 failed（避免自死锁）+commit+409；同 operation_id 重放=幂等返回既有版本（replayed=true） | pc/publish.rs:264-317/147-164 |
| close_report 批量 | **跨事务协调**：外层报告事务与每提案独立发布事务分离；失败计数不阻断（补偿=幂等键可重放） | analysis_report.rs:580-607 |

**结论**：PC/ProjC 单命令事务边界已完整实现且两域同构；唯一跨事务协调点=close_report 批量（有意设计，幂等键可重放补偿）。【已确认】

## 7. Schema 差距对照表

| 编号 | 目标能力 | 当前 Schema 支持情况 | 代码支持情况 | 差距描述 | 影响 |
|---|---|---|---|---|---|
| S-01 | 状态机持久化 | 状态值域 CHECK 全在位；关键单向转换有触发器强制（approval/version/proposal 锚列/对象终态） | 服务层唯一推进面+fail-closed 门禁完整 | proposal 状态转换无 DB 级转换合法性约束（draft→applied 直跳仅靠服务层） | 低；Step 3 设计沿用「服务层唯一推进面」即可，无阻塞 |
| S-02 | Revision 与版本关系 | revision/revision_no≥1+行不可变触发器+version_member XOR+成员唯一索引 | 行锁推号+内容寻址去重+闭合断言 | cognition_revision/relationship_revision 无 (identity, revision_no) 唯一约束（推号靠行锁，行锁在位） | 低；DB 兜底缺位登记，Schema/API 阶段补 |
| S-03 | Decision 事实持久化 | approval 表=独立决策事实（outcome=status 显式列、decided_by/decided_at/decision_reason 独立列、锚列不可变、重决新行留痕）；version.confirmed_by/at 锚定 | decide/approve/reject 原子落事实+审计 | PC/ProjC 决策事实载体**已在位且与 R36 五元组同构**（F-01 仅涉 KB 域，不在 PC/ProjC 范围）；decision_reason 可空无强制 | 无阻塞；F-01 归 Schema/API 阶段 KB 批次 |
| S-04 | Audit 与 Decision 分离 | governance.audit_log 与 approval 物理分离 | 同事务双写；静态清点测试守卫四身份列 | 无 | 无 |
| S-05 | 状态转换原子性 | 触发器兜底（终态/单向/锚列） | 十四步发布单事务；CAS 失配同事务内置 failed | 无（close 批量的跨事务协调为有意设计） | 无 |
| S-06 | 并发控制 | advisory+FOR UPDATE+CAS+幂等唯一约束齐备 | 锁序文档化（§17.3）；行数断言 409 | **projc.cognition_object 缺 object_key 唯一约束**（pc 有 0001:5366 同款；0025/0026 均未建；活库 pg_constraint 确认仅 lifecycle CHECK+singleton 部分索引）——projc/object.rs:60-90 先查后插的并发首插窗口可产生重复具名身份行 | **中**；违反「具名身份项目内唯一」不变量，Step 3 并发/事务设计引用该不变量 |
| S-07 | 跨对象一致性 | **零 FK**；触发器族（端点闭合 DEFERRED、proposal_member 锚定、endpoint match、单例） | 服务层闭合断言+CAS+锁序 | 无 FK 为既定纪律（跨域一律无 FK）；域内一致性靠触发器+断言双保险 | 无阻塞；维持现状纪律 |
| S-08 | AI 提案物化幂等（F-04） | write_operation 账本仅覆盖发布段；proposal_member/proposal 无幂等键 | 起草段=advisory 串行化+候选排除（开放提案/已入版本）+outbox idempotency_key | **起草段幂等靠 advisory+查询排除，无账本式锚**；非分析链起草（V1 无入口）无幂等保障 | 中；F-04 裁决要求 Step 3 闭合幂等边界——Step 3 事务设计须显式决策起草段幂等锚形态 |
| S-09 | Evidence verification（F-03） | verification_status 四值 CHECK 在位（unverified/verified/refuted/conflicted） | **全库无任何 UPDATE evidence.verification_status 的生产代码**（grep 确认；register_evidence 仅写入默认 unverified） | 验证程序缺失（V1.1「verification 程序待建」的实现面确认） | 无阻塞；F-03 后续闭合，不影响状态机/事务设计 |
| S-10 | 其他实际发现 | pc.write_operation 的 commit_object/commit_relationship 命令**无代码使用**（PCGIT 后置，仅 publish 在用）；pc.conflict=Legacy 无新增入口；projc 侧无 FTS/v_revision_versioned（P3-10 前置） | 均有代码/配置证据 | 登记性事实，非缺口 | 低 |

## 8. A / B / C 三类依赖分类

### A 类：Step 3 设计前置条件（3 项）

| # | 事项 | 阻塞什么 | 为什么不能并行 | 先完成什么 | 归口 |
|---|---|---|---|---|---|
| A-1 | **projc.cognition_object 补 object_key 唯一约束**（一条迁移，同款 pc 0001:5366） | Step 3 事务边界与并发分析引用「具名身份项目内唯一」不变量；不钉扎则状态机的幂等/重试论证建立在无 DB 兜底的假设上 | 该不变量是 Step 3 论证输入，且修复微小（单迁移），先修成本远低于并行协调成本 | 执行该迁移（S4/Schema 前置批，或依赖核查裁决后随 Step 3 启动批） | Schema 实施（F-02 同批机制纪律：登记收敛，不改冻结契约） |
| A-2 | **人工裁决入口归属表态**：approve/reject/decide 的正式触发面（HTTP/MCP/服务层调用）与审阅流 v2 的关系 | Step 3 状态机的「外部触发面」设计——入口不定，状态机的角色权限矩阵与触发转换图无法定稿 | 这是设计决策而非实现；并行实现会在无裁决下产生返工 | 用户一句话裁决入口归属（可不实现） | Step 3 设计范围声明（用户裁决） |
| A-3 | **PC L3 冲突源 Legacy 现状澄清**：pc.conflict=Not for New Use（R-06 已裁）+PC L3 冻结查询在位→当前恒空集；Step 3 是否沿用 PC 侧 L3 分支、冲突治理归属 O-06/S5 的边界 | PC 状态机门禁设计（submit/approve/publish 的 L3 前置是否保留及语义） | 不澄清则 PC/ProjC 状态机门禁对称性无法定稿；且易被误读为需重开 R-06（不需——只澄清现状与沿用决策） | 登记现状事实+用户确认沿用口径 | O-06（S5）与 Step 3 设计的接口 |

### B 类：可与 Step 3 设计并行（3 项）

| # | 事项 | 并行内容 | 与 Step 3 接口 | 冲突避免 | 一致性核验 |
|---|---|---|---|---|---|
| B-1 | F-03 Evidence verification 机制 | 独立工作项设计验证程序（谁验证、验证什么、状态流转 unverified→verified/refuted/conflicted） | Step 3 仅消费 evidence 既有状态做提案候选判定（propose_from_analysis 候选查询不滤 verification_status——登记为 Step 3 设计可选项） | 验证程序不改 evidence 表结构（字段已冻结在位）；只加服务层 | Step 3 设计评审时核对引用口径；F-03 闭合时回归 propose_from_analysis 行为 |
| B-2 | 检索适配层（v_revision_versioned/FTS，P3-10 前置） | projc 侧读视图/FTS 建设 | 纯读侧，与 Step 3 写路径无交叠 | 只读对象 | P3-10 验收时对 Step 3 状态机终态做读投影核对 |
| B-3 | O-05/O-06 收口（ProjC 旧通道、冲突治理细化） | projc.conflict 服务的治理细化 | L3 冻结查询口径（状态集）为双方接口 | O-06 不改 conflict 状态值域 CHECK（已同构冻结） | Step 3 门禁设计引用同一状态集常量 |

### C 类：延后至 Step 3 之后（4 项）

| # | 事项 | 不阻塞原因 | 延后风险 | 最迟闭合点 |
|---|---|---|---|---|
| C-1 | revision_no DB 唯一约束兜底（pc/projc） | 行锁推号在位，窗口已被锁消除 | 理论性；未来绕过服务层的写入路径出现时才暴露 | Schema/API 阶段（F-01 同批迁移） |
| C-2 | write_operation commit_* 未用命令（PCGIT 后置） | Git 权威迁移已裁不进 Phase 3 关键路径（P3-D02.2） | 无——账本幂等已覆盖发布段 | Git 权威目标态裁决批次 |
| C-3 | pc.conflict Legacy 存量行处置 | R-10 已裁 Legacy Transitional 留原位 | 无 | 已闭合（无需动作） |
| C-4 | proposal 状态转换 DB 级约束化、decision_reason 强制化 | 服务层推进面已守卫 | 低 | Schema/API 阶段按 Step 3 定稿的状态机补触发器 |

**分类纪律自检**：未把一切推入 A（S-09/F-01/F-02 等无依赖项明确后置/并行）；未把真前置（S-06 缺口、入口归属、L3 现状）降入 C。

## 9. F-01～F-04 依赖关系分析

| 编号 | 与 Step 3 的依赖 | 实施时机 | 结论 |
|---|---|---|---|
| F-01 Document 决策五元组物理载体 | **零依赖**——F-01 只涉 kb 域（kb.document_revision 侧）；Step 3 仅做 PC/ProjC，而 PC/ProjC 决策事实载体（approval+version 锚列）已同构 R36 在位（S-03） | Schema/API 阶段 KB 批次 | 不阻塞 Step 3；不得借本轮扩大其范围 |
| F-02 plan/adr 登记收敛 | 零依赖（S4 边界保护）；本轮仅执行其纪律——A-1 的修复迁移按「登记收敛」模式（同 0026 修正先例：纯增量+缺陷修正登记） | S4 批次 | 不阻塞；A-1 迁移属登记收敛范畴 |
| F-03 Evidence verification | 不阻塞状态机/事务设计（B-1 并行）；Step 3 设计可预留候选过滤接口 | 后续独立工作项 | 并行 |
| F-04 AI 提案物化幂等 | **强相关**：冻结令指定「Step 3 落实幂等保障」。现状=发布段账本幂等在位、起草段 advisory+排除式幂等在位；Step 3 事务设计必须显式决策起草段幂等锚是否升级为账本式（operation_id）及重投递自愈语义 | **Step 3 设计范围内必闭合项**（非前置阻塞，是设计必答项） | 列入 Step 3 设计任务书 |

## 10. 三种执行顺序方案比较

**方案 A：Schema 差距分析全部先行，再启动 Step 3**
- 优点：Schema 面完全钉扎后再设计，理论上零返工。
- 缺点：S 表多数项（S-01/S-02/S-04/S-05/S-07）现状已达标或属实施阶段细节；F-01/F-02/F-03 与 Step 3 无依赖；「全部先行」把无关项强加关键路径，显著拖延。
- 风险：冻结令明确的「不要求 Schema 差距分析全部完成后才能开始 Step 3」被违反；近零数据红利窗口（活库实证）被无谓消耗。
- 适用性：**低**。

**方案 B：先完成 Step 3 设计，再开展 Schema 差距分析**
- 优点：最快进入领域设计；PC/ProjC 事务机制已高度成熟，直接设计有大量可复用证据。
- 缺点：A-1（projc 唯一约束缺口）不先修，Step 3 并发/事务论证引用的不变量无 DB 兜底；A-2/A-3 不先裁决，状态机触发面与门禁对称性可能在设计中途被迫回问。
- 风险：状态机设计定稿后发现「具名身份唯一」不成立→事务边界章节返工；入口归属悬而未决→角色权限矩阵空白。
- 适用性：**中**（若 A 类不处理则风险实质存在）。

**方案 C：按依赖分类——A 类先行、B 类并行、C 类后置**
- 优点：A 类仅 3 项且都微小（一条迁移+两句裁决+一项登记确认），先行成本小时级；B 类与 Step 3 无交叠天然并行；C 类不占用关键路径。与冻结令「不预设并行与否、交由依赖核查结果决定」的口径精确对应。
- 缺点：需要一次顺序裁决（本轮报告即为其输入）。
- 风险：A-2/A-3 若用户裁决延迟会拖延启动——但这两项本就是必须由人回答的问题，任何方案下都躲不开。
- 适用性：**高**。

## 11. 推荐执行顺序及理由

**推荐方案 C**，具体序列：

1. **先行批（小时级）**：①A-1 迁移（projc.cognition_object 补 UNIQUE(tenant,project,object_key)，0026 缺陷修正同款登记模式）；②A-2 用户裁决入口归属；③A-3 登记确认 PC L3 现状口径（不重开 R-06）。
2. **Step 3 设计启动**（与先行批 ②③ 可同日）：任务书须含——F-04 幂等边界闭合为设计必答项；PC/ProjC 状态机以现有服务层状态机+触发器族为事实起点（§5/§6 证据）；L3 门禁按 A-3 裁决口径。
3. **并行轨**：B-1（F-03 验证程序）、B-2（P3-10 检索前置）、B-3（O-05/O-06）。
4. **后置批**：C-1/C-4 归 Schema/API 阶段与 F-01 同批迁移；C-2 归 Git 权威裁决批次。

**理由**：全部来自实测证据——①PC/ProjC 状态机与事务机制已同构成熟（十四步发布/行锁推号/CAS/幂等账本/审计同事务），Step 3 是「定稿+补入口」而非「从零设计」，先决项极少；②唯一实质 Schema 缺口（S-06 projc object_key）修复成本一条迁移，先行收益（不变量钉扎）远大于等待成本；③与 Step 3 无依赖的差距（F-01/F-02/F-03、读侧、Legacy 处置）若强行先行纯属浪费（方案 A 缺陷）；④A-2/A-3 是任何方案都必须的人工决策点，方案 C 把它们显式作为启动门，避免方案 B 的中途回问。

## 12. 尚未确认的问题

| # | 事项 | 状态 | 影响 |
|---|---|---|---|
| U-1 | aipm_mig_full 库缺 `_sqlx_migrations` 账本表（构建方式不明；触发器/行数证据不受影响） | 部分确认 | 无 |
| U-2 | pc/relationship.rs 修订>1 的草稿追加路径是否存在（V1 见仅创建 rev=1；更新似经提案成员钉扎既有修订；未逐行读完 421 行） | 部分确认 | 若 Step 3 需关系修订推号语义，须补查 |
| U-3 | kb_pc_analysis.rs 890-970 双源注册的完整触发条件与失败语义（仅抽查核心段） | 部分确认 | 低；不涉事务边界结论 |
| U-4 | 审批入口未来的产品形态（HTTP 面形态/谁调用 decide） | 需人工裁决（=A-2） | Step 3 触发面设计 |
| U-5 | PC L3 分支沿用口径 | 需人工裁决（=A-3） | Step 3 门禁设计 |

## 13. 证据索引

- Migration：`migrations/0001_baseline.sql`（pc 建表 3021-3470；pc 函数 314-600；触发器 6966-7064；唯一索引 6830-6882；object_key UNIQUE 5366-5370；账本唯一 5746）、`0025_projc_domain_bootstrap.sql`（全文）、`0026_projc_formal_chain.sql`（全文，含两处 0025 缺陷修正）、`0027_pc_snapshot_dictionary_deprecated.sql`、`0009_pc_revision_fts.sql`
- 服务层：`crates/pm-application/src/pc/{object,proposal,publish,relationship,evidence,mod}.rs`、`crates/pm-application/src/projc/{...同构,conflict}.rs`
- 调用链：`crates/pm-application/src/analysis_report.rs:548-680`（close 双域分流）、`crates/pm-infrastructure/src/kb_pc_analysis.rs:515+/890-970`、`crates/pm-infrastructure/src/kb_pc_handlers.rs:304`、`crates/pm-ai/src/discovery_writeback.rs`、`crates/pm-application/src/identity.rs:161-203`
- 端口/HTTP：`crates/kb-pc-contracts/src/ports.rs:361-392`（NotWired）、`apps/pm-api/src/knowledge_services.rs:74-86`（仅读路由）、`apps/pm-api/src/routes.rs:321`
- 测试：`crates/pm-application/tests/pc_proposal.rs`（8 用例，含 decide_is_provisional_and_publish_is_idempotent:1432、publish_replays_receipt_and_cas_mismatch_fails_proposal、formal_writes_fail_closed_for_ai_and_unauthorized）、`projc_formal_chain.rs`（6 用例）、`audit_actor_coverage.rs`（审计写入点静态清点）
- 活库只读抽查（aipm_mig_full @ 127.0.0.1:16433 容器 aipm-dev-postgres）：triggers pc=22/projc=12；pc_obj=1/projc_obj=2/projc_prop=1/pc_prop=0/pc_ver=0；projc.cognition_object 约束清单（仅 lifecycle CHECK+singleton 部分唯一索引——S-06 缺口直接证据）
- 冻结基线：`P3-D04-02-第一批目标领域模型设计-v0.6.md`；`P3-D04-02-Step12冻结批准令-v1.0.md`（F-01~F-04、两句效力边界）

---

**本轮零实施声明**：未修改代码/Schema/Migration/API/测试；仅新增本报告文件；数据库仅执行只读 SELECT。等待人工审阅与正式执行顺序裁决。
