# 数据所有权 Code Review 检查单（P1-2 交付物）

| 项 | 值 |
|---|---|
| 版本 | V1.0（2026-11-07） |
| 上游 | 《KB-PC 服务化总体架构 V1.1》§7.1 表级所有权矩阵（ADR-06a 载体）；§5.4 服务自治原则（ADR-01）；§7.3 四元组引用契约（ADR-07）；§16.4 DB Role 白名单 |
| 用途 | code review 时的逐项检查单：凡新增/修改涉及数据读写、Schema、事件、引用的 PR，按本单过一遍。机械化守卫见 `crates/kb-pc-contracts/tests/guard_ai_container_users.rs` |
| 维护 | 所有权矩阵变更走 V1.1 评审 + ChangeLog；本单随之更新 |

---

## 一、共享容器红线（ADR-06a 过渡期规约）——违反即返工

- [ ] **C-1**：未新增任何 `ai.project_document` / `ai.document_revision` 使用方（守卫测试 `guard_ai_container_users.rs` 必须绿；新增文件命中两表名即违规）。
- [ ] **C-2**：PC 域代码**禁止接入** `ai.document*` 容器——PC 正文一律走 `pc.document*`（C-1 差异复核结论；本批次不建表，仅指代码引用）。
- [ ] **C-3**：`ai.document*` 的既有使用方（KB/Goal/Discovery/投影五方）行为不变；修改既有使用方时不得扩大其对该容器的写入面（如新增 doc_key 值需评审——新 doc_key = 新使用方语义）。
- [ ] **C-4**：新 Migration 不得引用 `ai.project_document` / `ai.document_revision`（守卫测试迁移面同样全等断言；ADR-06b 批准的迁移除外，届时须先更新守卫白名单）。

## 二、写入权检查（直写他人领域表 = 违规）

- [ ] **W-1**：本变更写入的每张表，其 schema 归属与变更所属服务一致（对照 §7.1 矩阵：core.* = Platform；delivery.* = Platform；integration.intake_* = Platform；kb.* = KB；pc.* = PC；governance.audit_log = 各域写自己 source_kind）。
- [ ] **W-2**：跨服务业务写入一律走：同步=端口调用（`kb-pc-contracts` ports），异步=Outbox 事件；不存在第二个服务直写第一服务业务表的代码路径。
- [ ] **W-3**：Outbox/Inbox（`integration.outbox_event` / `inbound_event`）为共享基建：业务代码只经端口写入（`OutboxAppend`），禁止绕过端口直拼 SQL 写事件表；禁止业务反查事件表作为业务数据源。

## 三、引用契约（ADR-07 四元组本体）

- [ ] **R-1**：任何新增的跨域持久化引用，本体 = `DomainRef` 四元组（target_domain/target_type/target_id/target_revision_id），且 domain 值在注册表内（`kb-pc-contracts::registry`）。
- [ ] **R-2**：`service` 字段只出现在事件信封顶层 `producer`、`intake_task.target_service` 列、HTTP URL 前缀三处；任何引用 JSON 结构（payload 内嵌对象、result_ref、DB 列）不得内嵌 service 字段。
- [ ] **R-3**：事件 payload 纪律：只含四元组引用 + 摘要（≤256 字符）+ hash，禁止内联正文（生产前跑 `events::validate_payload_references`）。
- [ ] **R-4**：有 Revision 机制的对象，引用必填 revision；空缺必须记录原因（空缺原因归领域承载列，如 `pc.source.revision_missing_reason`）。
- [ ] **R-5**：禁止跨库/跨 schema JOIN 作为集成机制；跨域存在性校验走所有者 API（V1 进程内端口）或四元组组合键服务层校验。

## 四、事件目录（单一真源）

- [ ] **E-1**：新事件必须先入 `kb-pc-contracts::events::CATALOG`（wire 权威）并在 `pm-application::outbox::CATALOG` 登记（运行时面）；未登记类型禁止生产（`event_is_enabled` fail-closed）。
- [ ] **E-2**：事件声明 event_version（起步 v1，ADR-11 additive-only）；破坏性变更 = 新 event_type 或 version 跳变，禁止原地改删。
- [ ] **E-3**：目录允许「登记先行」，但 has_handler=false 的事件禁止生产（不假装消费）；新增 has_handler=true 事件必须同步 `register_all_handlers` 装配（启动校验 assert_catalog_consistent 兜底）。
- [ ] **E-4**：异步机制只用既有 Transactional Outbox（ADR-03：不引入 MQ）；业务状态变更与事件写入同一 PG 事务。

## 五、Intake 与父子聚合（§9）

- [ ] **I-1**：intake_record / intake_task 写入方 = Platform；服务结果经事件由 Platform 消费端回写，服务不直写 intake 表。
- [ ] **I-2**：同一 (intake_id, task_type) 至多一个活跃任务（uq_intake_task_active）；后到信号合并进 signal_ref，不建新行。
- [ ] **I-3**：重试 = 复用原任务行 attempt+1 重置 PENDING；禁止新建任务行重试。
- [ ] **I-4**：父 Intake 整体状态 = 查询时派生（不落列）；存在必需子任务非终态 ⇒ 父 ≠ COMPLETED（物理上由派生计算保证，任何「落列+手写聚合」的捷径都违规）。
- [ ] **I-5**：任务创建与派发事件同 PG 事务（任务已创建 ⇒ 事件必已入账，§10.2-6）。

## 六、权限与隔离（ADR-08 / §16）

- [ ] **A-1**：KB/PC 不直读 `core.*`——权限校验经 Platform 鉴权 API（V1 进程内 = 调用 pm-application 层鉴权函数，如 `require_project_member`）。
- [ ] **A-2**：REST 端点挂接既有 fail-closed 中间件链（require_bearer → resolve_principal → require_active），handler 内做项目可见性校验（require_project_visible 同口径）。
- [ ] **A-3**：新表带 tenant_id + 组合外键（0019 范式）；越权负向用例（无凭据/错租户/错项目/降权）随端点进 CI。
- [ ] **A-4**：审计写 `governance.audit_log` 用调用方自己的 source_kind；日志不落正文。

## 七、迁移纪律

- [ ] **M-1**：Migration forward-only；零历史数据迁移/回填/双写（历史业务数据）。
- [ ] **M-2**：kb.* / pc.* 建表或迁址必须先经 ADR-06b 六问分析批准（本批次仅设计稿，见 kb-document-target-ddl-draft）。
- [ ] **M-3**：迁移起号以 migrations/ 目录实况为准；撞号=后实施者顺移（0044→0045 先例）。

## 八、Phase 边界（本批次红线）

- [ ] **P-1**：不创建 pc.* / kb.* 表或 Migration（仅设计稿）。
- [ ] **P-2**：不实现 PC 认知写入/审批逻辑（M3 前禁开正式认知写入）。
- [ ] **P-3**：不修改 PC-01A / PC-01B 任何文档。
- [ ] **P-4**：Phase 4~7 / PC-01C 工作项不抢跑。

---

## 附：检查单覆盖的 §7.1 矩阵速查（详细证据列见 V1.1 §7.1 原表）

| 域 | 表 | 唯一写入方 |
|---|---|---|
| Platform | core.* / delivery.* / integration.intake_record+intake_task / ai.cognition_* / ai.baseline_* / governance.*（写各自 source_kind） | Platform |
| Platform 自用（过渡期共享容器） | ai.project_document / ai.document_revision（Goal/Discovery/Task Contract/投影部分） | Platform（既有使用方，冻结） |
| KB | ai.document_proposal / knowledge_reference / document_section_index / document_index_state / knowledge_write_operation（目标态 kb.* 同名） | KB |
| PC | pc.*（15 表 + pc.document*，未实施） | PC（PC-01C） |
| 共享基建 | integration.outbox_event / inbound_event（端口写） | 三服务经端口；Platform 运维 |
