# PA06-IMP-001：PA-06 协作边界执法现状盘点与实施规范

- **任务：** PA-06《AI Agent 与人类的协作边界》落地前置盘点（用户 2026-11-21 选定「实施盘点+落地」路径）
- **日期：** 2026-11-21
- **核对基准：** HEAD `4e9b73d`，migrations 0001–0052，`apps/pm-api/src`、`crates/{pm-application,pm-mcp,pm-ai,pm-infrastructure}`
- **处理策略：** 无存量数据，差距直接改契约，不写兼容层（AGENTS.md 规则 1）

## 1. 总体结论

PA-06 六条决策的执法现状分三层：

1. **已落实（机制层）：** 命令通道红线闸、`require_human` 显式查库单点、知识草稿/定稿分离、K4 审阅链、Completed 前验收门。
2. **拓扑排除而非显式执法：** 产品验收、human_consultation、Issue/Risk/Blocker 的人类限定靠「agent 凭据仅 `/mcp`、REST 拒 service token、JWT 主体恒 provision 为 human」的认证拓扑，代码层无兜底拒绝——一旦按 PA-06-02 给 agent 开通道，这些面立即失守。
3. **授权未接线：** PA-06-02/04 授予 agent 的自主通道（分解建 Task、管理 Issue/Risk/Blocker、普通关联）当前要么被人工闸阻断，要么根本没有 agent 可达通道。

**显式人类执法单点：** `require_human_conn`（`crates/pm-application/src/requirement.rs:210-226`，查 `core.principal.kind='human'`，不信任 token 声明）。

## 2. 逐条盘点总表

| PA-06 决策 | 判定 | 现状与证据 |
|---|---|---|
| **01 自主权限原则** | ✅ 已落实 | 红线闸：`command_executor/executor.rs:58`（`is_redline && actor≠Human → RedLineBlocked`），红线单 `policies/ai-pm-ai-lead-v2.yaml`（create_tasks=红线，required_role=project_owner）；AI 提案经 RedlineApproval 由人类执行（`supervision/approval.rs:98`）=「重大事项人类批准」已执法 |
| **02 执行性事项** | ⚠️ 部分落实 | ①decompose→create_task 被 `require_human` 阻断（`pm_write/task.rs:24`），PA-07 §11 标 `BLOCKED_HUMAN_DECISION` 三选一待裁（G-01）；②Issue/Risk/Blocker 服务层无守卫（`issue_risk.rs:68-99`）但 agent 无通道（MCP 26 工具无此域工具）=授权落空（G-02）；③agent 普通关联建立无通道；`remove_relation` 仅人类=API 不暴露且无路由（死代码，G-03）；④`draft→ready` 真人闸（E2/DK-0，`pm_write/task.rs:226,561`）**保留**——ready=进入执行闸，归 PA-07 run 门禁体系，非 PA-06-02 事项修改权范围 |
| **03 正式知识** | ✅ 已落实 | Requirement：AI 草稿畅通（`discovery_writeback.rs:1045-1093` 止于 pending_confirmation），定稿 `confirm_revision`=require_human+治理角色+CAS 双令牌（`requirement.rs:799-840`）；Design/ADR：AI 可建草稿/提交评审/请求批准，`decide_approval`/`freeze_baseline`/`supersede_adr` 人类独占（`design/mod.rs:9-14` 边界明文）；K4 `review()` require_human（`kb_service/memory.rs:669`）；边界说明：`pm_submit_document` 以 ai_pm 直落 Git 知识文档（`pm-mcp/tools.rs:420`，持 PM_MCP_DOCUMENT_WRITE）——知识文档≠正式需求/设计/决策，红线类走 V2 提案链人类确认，与 PA-06-03 不冲突 |
| **04 执行计划调度** | ⚠️ 同 02 | 创建/拆分任务通道=G-01；依赖/顺序/分配经 AI-PM 编排器命令通道（受红线闸）；`task_dependency` 服务成熟但 agent 无直接建边通道（并入 C 批次考量） |
| **05 交付与验收** | ⚠️ 部分落实 | 衔接裁决见 §3；agent 链内自主验收/完成已通（`apply_task_transition` Completed 前 `acceptance_gate`，`transition.rs:108-118`；delivery/review 链 Service 主体）；`requirement_acceptance` require_human ✅；**正式产品验收面守卫缺失（G-04）**：`record_product_acceptance` 唯一调用方=REST（`apps/pm-api/task_products.rs:195`），守卫仅 tenant+project_member（`require_task_project`），无 require_human、无 Product Membership 检查——DR-PA-06 第一轮守卫（人类 principal+排除 Agent）未实现；`source ∈ {human, platform_flow}` 为调用方自报非执法 |
| **06 异常与升级** | ⚠️ 未打通 | 两条独立机制未互认：agent 升级走 `pm_report_interaction`→awaiting_decision+task_waiting（`interaction.rs:100` 起），human_consultation 仅 REST 人类创建（`human_consultations.rs:110` 唯一调用方），仅在 `task_waiting` 领取阻塞上汇合（G-05）；§8.5 五要素决策上下文无承载；**consultation start/decide/cancel/supersede 端点无项目可见性守卫**（仅 tenant 级隔离，`human_consultations.rs:148-215`，G-06）；重试上限/局部暂停归 runner/run_dispatch（P3-D04 链，本规范不展开） |

## 3. 关键衔接裁决：PA-06-05 × DR-PA-06（消除评审遗留条件）

评审条件「PA-06-05 与 DR-PA-06 验收语义未衔接」按**验收分层**消解，两者不冲突：

| 层 | 主体 | 载体 | 依据 |
|---|---|---|---|
| **技术验收 + 客观业务验收**（链内） | Agent（经 delivery/review 链，Service 主体迁移） | `apply_task_transition` + `acceptance_gate` 前置于 Completed | PA-06-05 §7.1/§7.4：明确、客观、可验证条件 Agent 自主判断 |
| **正式产品验收**（对外记录） | 人类 Product Member 专属，Agent 不得代替 | `delivery.task_product_acceptance` REST 面 + Issue 验收 | DR-PA-06：守卫=人类 principal + 排除 Agent + Product Membership（资格项随 DR-PA-04⑤，台账号 6 留空） |
| **主观业务验收** | 人类 | 升级请求链（G-05 打通后经 consultation） | PA-06-05 §7.2/§7.4 |

**落地动作 = G-04**：把 DR-PA-06 第一轮守卫落为显式执法。已核实 `record_product_acceptance` 无 platform_flow 隐藏调用方，收紧不破坏既有链路。

## 4. 差距清单

| # | 优先级 | 差距 | 批次 |
|---|---|---|---|
| G-01 | P0·裁决 | decompose→create_task 被 require_human 阻断，PA-06-02/04（选择 A=自主创建）与 PA-07 §11 fail-closed 待裁决（见 §6-1） | C |
| G-02 | P1 | Issue/Risk/Blocker agent 自主管理无通道（授权落空）；开通道前 REST 人类面须先显式固化 | A2+C2 |
| G-03 | P2 | work_item 关联：agent 建普通关联无通道；`remove_relation` 无路由（死代码）；Timeline origin 硬编码 human | C3 |
| G-04 | **P0** | 正式产品验收面无显式人类守卫（DR-PA-06 第一轮守卫未实现） | A1 |
| G-05 | P1 | interaction 升级请求与 human_consultation 未打通；§8.5 五要素上下文无承载；§8.6 决策后恢复无回读通道 | B |
| G-06 | **P0** | consultation start/decide/cancel/supersede 缺项目可见性守卫（同项目成员可见性口径缺口） | A3 |

## 5. 批次划分

- **批次 A（P0 守卫收紧，纯收紧不改变人类使用面，无待裁决依赖）：**
  - A1：`record_product_acceptance` + `pm_acceptance` 写端点补 `require_human_conn` + Product Membership 检查（DR-PA-06 第一轮守卫；资格项留空）
  - A2：Issue/Risk/Blocker 服务层补 `require_human_conn`（为 C2 开 MCP 通道前固化人类/agent 分流边界）
  - A3：consultation 四端点补 `require_project_visible`（对齐 create/list 口径）
  - 验收：新增守卫拒绝用例 + 现有测试回归
- **批次 B（P1，PA-06-06 打通）：**
  - B1：`pm_report_interaction` 升级链对接 `create_consultation`（五要素：问题描述/影响范围/已尝试方案/可选方案/推荐及理由），consultation↔task_waiting 互认
  - B2：人类决策结果→agent 可读（§8.6 恢复执行；决策后不要求重排全部计划）
- **批次 C（授权接线，依赖 §6-1 裁决）：**
  - C1：decompose 链正式建 Task 闸处理（G-01）
  - C2：MCP Issue/Risk/Blocker 工具（agent 自主创建/迁移，PA-06-02）
  - C3：agent 普通关联建立通道（PA-05-04：语义明确的普通关联；人类修改过的关系不覆盖；`remove_relation` 维持不暴露）

## 6. 待裁决项（人类决定）

1. ~~**BLOCKED_HUMAN_DECISION（G-01）**~~ **已裁决（2026-11-21 用户）：按 PA-06-02 放开 decompose 链正式建 Task，红线单 `create_tasks` 同步摘除**——PA-06 是授权边界的权威上位基线（documentation-governance §8：PA-07 不得改变 PA-06 业务授权原则），PA-07 §11 预留的「有权人类明确决定」出口由 PA-06-02/04（选择 A）满足。AI 建任务落 draft 态，`draft→ready` E2 真人闸（PA-07 run 门禁）不变。
2. **A1/A2 执法形式**：推荐显式 `require_human_conn` 查库（不依赖认证拓扑），与既有单点一致。
3. **DR-PA-06 资格项**：维持台账号 6 OPEN（读取资格接口留空），本轮守卫=人类 principal + Product Membership。
