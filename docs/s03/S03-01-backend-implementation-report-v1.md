# S03 后端实施报告 V1.0

- 日期：2026-11-10
- 范围：S03 — 分析、设计与技术决策（后端全部）
- 设计依据：[总体设计 V1.0](../ui/smark-pm-s03-01-analysis-design-technical-decisions-v1.md)（Q1–Q5 五项裁决）
- 核验依据：[S03-00 代码事实核验主报告 V1.1](S03-00-code-facts-report.md)（七项核心缺口）

## 一、交付总览

S03-00 确认的七项核心领域能力缺口**全部落地**，全部走统一应用服务单入口（§16.1 教训）：

| 缺口 | 实现 | 位置 |
|---|---|---|
| Technical Design 实体 | `design.design`（稳定身份 + 乐观锁 + 代码唯一） | 迁移 0010 |
| Design Revision（不可变） | `design.design_revision`（版本链 + 内容哈希 + 库级触发器不可变守卫） | 迁移 0010 |
| Design ↔ Requirement 多对多（Q1） | `design.design_requirement`（绑具体 requirement_revision_id；只许绑 approved 修订） | 迁移 0010 |
| ADR 独立实体（Q3） | `design.adr`（六态生命周期：proposed/under_review/accepted/rejected/superseded/deprecated）+ `design.adr_revision`（提交后内容冻结） | 迁移 0010 |
| ADR 关联（§3.4） | `design.adr_design_revision`（引具体设计版本）+ `design.adr_requirement`（绑需求修订） | 迁移 0010 |
| Design Review | `design.review_finding`（问题登记/知悉/关闭三态；blocker 未关闭阻止批准） | 迁移 0010 |
| 专表审批（R6） | `design.design_approval`（三 kind：design_revision / adr_acceptance / baseline_freeze；pending 唯一索引；target_hash 锚定） | 迁移 0010 |
| Implementation Baseline（Q5） | `design.implementation_baseline`（内容 + 判定报告 + 哈希同事务冻结；仅 status 可显式变更） | 迁移 0010 |
| Readiness Gate 五条件 | `design::evaluate_readiness`（dry-run 端点 + freeze 前置共用同一实现） | `crates/pm-application/src/design.rs` |
| Task 追溯列（E3） | `delivery.task.implementation_baseline_id`（创建时钉定 frozen 基线，跨项目/superseded 拒绝） | 迁移 0010 + `pm_write/task.rs` |

扩展项（S03-00 差异矩阵 B 类）：kb-pc-contracts `design`/`adr` 通道开放（NotYetOpen → Open）；outbox 事件族 5 项目录留位（不假装消费）；真人门沿用 `ensure_trusted_context` + `require_human` 双闸。

## 二、治理红线落实（原则 5 / A*）

- **AI 无批准路径**：批准/接受/冻结/替代/废弃/阻塞标记一律 `require_human`（服务层，`core.principal.kind='human'`）+ REST 适配层 `ensure_trusted_context`（`auth_mode=api_key` 一律 403）双闸。MCP 未新增任何确认类工具（A* ②：MCP 不提供正式确认能力）。
- **AI 可为的部分**：AI 主体（`kind='ai_pm'` 等）可创建设计/修订/ADR 草稿、提交评审、请求批准——修订行 `source='ai'` 留痕。
- **设计修改 ≠ 设计批准**：draft 可改稿；`submit` 后正文库级冻结（触发器拒绝 UPDATE/DELETE），唯一出口是审批行裁决。
- **审计**：全链点号式审计词表（`design.created` / `design_revision.submitted` / `design_revision.approved` / `design_requirement.linked` / `adr.accepted` / `approval.decide` / `baseline.frozen` / `baseline.superseded` …），落 `governance.audit_log`。

## 三、关键机制

1. **版本不可覆盖（SC-02）**：新修订批准时旧批准版本自动置 `superseded`，正文原样保留；`current_revision_id` 指针切换；其他对象的历史引用不受影响。
2. **并发批准（SC-06）**：审批裁决在行锁事务内进行；`expected_current_revision_id` / `expected_row_revision` 双令牌可选校验，后至者显式 409；`target_hash` 与库存哈希不符即拒（跨版本重放防线）。
3. **门禁五条件（SC-07）**：①需求依据已确认（绑定的需求修订 approved 且为当前版本，旧版本引用=影响评估缺失拦截）②设计已批准 ③阻塞项 ADR 必须 `accepted`（superseded/deprecated 视为未解决，须改链后继）④实施范围 `implementation_scope.included` 非空 ⑤验收标准逐条有语句 + 合法 `verification_kind`。
4. **基线隔离（SC-08）**：冻结后内容/判定/哈希不可变（触发器）；Task 引用基线 id 而非内容；设计推进新版本不影响已冻结基线；基线替代走显式 supersede + 后继指针。
5. **评审闭环**：blocker 级评审问题未 resolved 时批准命令 409（问题登记 → 修订 → 批准的 S03-05 工作流）。

## 四、API 面（REST，24 端点，`apps/pm-api/src/pm_designs.rs`）

| 组 | 端点 |
|---|---|
| 设计 | `GET/POST /v1/pm/projects/{pid}/designs`、`GET /v1/pm/designs/{id}`、`POST .../revisions`、`PATCH .../revisions/{rid}`、`POST .../revisions/{rid}/submit` |
| 需求关联 | `GET/POST .../revisions/{rid}/requirements`、`DELETE .../requirements/{req_id}` |
| 评审 | `GET/POST .../revisions/{rid}/findings`、`POST .../findings/{fid}/close` |
| 审批 | `POST .../revisions/{rid}/approvals`、`POST /v1/pm/design-approvals/{id}/decide`（真人门）、`POST /v1/pm/adrs/{id}/approvals` |
| ADR | `GET/POST /v1/pm/projects/{pid}/adrs`、`GET /v1/pm/adrs/{id}`、`POST .../revisions`、`POST .../revisions/{rid}/submit`、`POST .../revisions/{rid}/designs`、`POST .../revisions/{rid}/requirements`、`POST .../supersede`（真人门）、`POST .../deprecate`（真人门）、`POST .../implementation-blocking`（真人门） |
| 基线 | `POST /v1/pm/projects/{pid}/implementation-readiness`（dry-run）、`GET/POST .../implementation-baselines`（POST=冻结，真人门）、`GET /v1/pm/implementation-baselines/{id}`、`POST .../supersede`（真人门） |

## 五、验证证据

- **S03-00 八场景**：`crates/pm-application/tests/design_s03.rs` 8 用例全绿（SC-01~08 全覆盖 + 库级不可变守卫用例）。
- **pm-domain 单测**：`design.rs` 状态机矩阵 / wire 往返 / 阻塞 ADR 解决判定。
- **全工作区回归**：`cargo test --workspace` 全绿（含更新后的 kb_service 通道守卫测试与 task_ready_batch 夹具）。
- **迁移**：`migrations/0010_design_domain.sql`，编号 0010 按台账纪律取号（先对账 `ls migrations/`），已登记 [迁移台账](../ai-pm-migration-ledger.md)。
- 测试环境：`DATABASE_URL='postgres://aipm:aipm@127.0.0.1:16433/aipm'`（本机 Docker 端口映射，127.0.0.1 直连）。

## 六、范围裁决记录（对应总体设计 §11 待裁事项）

| 项 | 裁决 | 依据 |
|---|---|---|
| D-01 Technical Analysis 独立 Revision | 不设独立实体，随 `design_revision.content.technical_analysis` 承载 | S03-00 七缺口清单未含 TA；修订本身不可变即满足追溯 |
| D-02 设计↔需求绑版本 | 强制绑 `requirement_revision_id`（缺省解析当前正式版本），仅 approved 修订可绑 | §2.2 / §3.4 关键约束一 |
| D-03 ADR Revision | 设 `adr_revision`（draft 可改 / submitted 冻结）；已接受 ADR 不可改，决策改变 = 新 ADR + supersede | §3.3 不可变决策记录 |
| D-04 多设计引用同一 ADR | 支持（adr_design_revision 多对多，UNIQUE(adr_revision, design_revision)） | §3.4 |
| D-05 Review 实体形态 | 评审 = `review_finding` 问题集（轻量）；批准 = 专表 `design_approval`（对齐 review_approval 先例，不复用 1:1 硬绑 change_proposal 的 approval_request） | S03-00 K-7/K-8/R6 |
| D-06 权限码 | 批准/接受/冻结共用真人门（kind=human + trusted context），差异体现在审批 kind 的领域语义与审计词表；细粒度权限码归既有 RBAC 治理线 | S03-00 §八（REST 权限码现状） |
| D-07 一个基线含多个设计 | 支持（freeze 请求多 revision）；一个基线可含多设计、多需求、多 ADR | §6.3 |
| D-08 设计变更影响识别 | 基线内容含全部指针（design/requirement/adr revision id），可反查受影响任务（task.implementation_baseline_id） | §6.3 |
| D-09 基线与仓库 Commit 绑定 | `repository_refs`（repository_id + base_commit）入基线快照，冻结时校验仓库属本项目 | §6.3 |
| D-10 多设计覆盖同一需求 | 门禁条件 1 按所选修订集判定；跨设计覆盖完整性检查归 S04 计划面 | — |

## 七、已知边界与后续项

1. **MCP 面未扩展**：旧 `pm_submit_technical_design`（inbound_event→change_proposal）保持原样（S03-00 K-1：无实体语义）。AI 经 REST（api_key 或 ai_pm 主体）创建草稿即可满足 Q4；MCP 设计工具归 S04/S05 上下文注入批次。
2. **真人门深度（A* ③④⑤）**：`ensure_trusted_context` 仅拒 `api_key` auth_mode（沿用 S02-I6-A 现状边界）；身份模型改造（step-up 票据等）属 S02 既有治理线，S03 未重复建设。
3. **事件未生产**：5 项 S03 事件仅目录登记（has_handler=false），消费方（通知/S04 计划面）落地后随命令事务启用。
4. **需求正文权威（K-3）**：S03 遵循既有裁决不重新裁量；设计正文与 requirement_revision 同形态存 PG jsonb（+内容哈希），Git 权威收敛属 S02-I2 落地线。
