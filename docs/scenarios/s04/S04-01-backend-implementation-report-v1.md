# S04 后端实施报告 V1.0

- 日期：2026-11-10
- 范围：S04 — AI 任务拆解与研发组织（Development Planning & Task Orchestration，后端全部）
- 设计依据：[S4 后端设计方案提案 V1](ai-pm-s04-backend-design-proposal-v1.md)（§1.3/Q1~Q6 裁决）
- 上游：S03 冻结基线（`docs/scenarios/s03/S03-01-backend-implementation-report-v1.md`）　下游：S05 执行（Run 域已有）

## 一、交付总览

**核心立场落地：不新建任务体系**。Task Plan 是新增的「计划」聚合层；正式任务全部复用 `delivery.task`（13 态状态机/依赖 DAG/仓库绑定/验收标准/审计原样使用）。

| 能力 | 实现 | 位置 |
|---|---|---|
| Task Plan 聚合根（Q1） | `plan.task_plan`（draft/in_review/approved/superseded；`source_baseline_id` 钉定 S03 冻结基线） | 迁移 0011 |
| Task Plan Revision | `plan.task_plan_revision`（任务草案 jsonb + 内容哈希；draft 可改稿，提交后库级触发器冻结） | 迁移 0011 |
| 计划审批（复用 R6 专表） | `design.design_approval` 增 `plan_id` + kind `'task_plan'`（pending 唯一索引复用；`target_hash` 锚定批准内容） | 迁移 0011 |
| Task ← 计划追溯（Q2 映射锚） | `delivery.task.plan_revision_id` + `plan_task_key` + 部分唯一索引（同修订每键至多正式化一次） | 迁移 0011 |
| 计划/修订/审批/正式化/交接/进度服务 | 单入口统一服务 | `crates/pm-application/src/task_plan.rs` |
| AI 拆解编排 | 准入门（frozen 基线 + Gate 复判）→ `LlmWriteBackModel::decompose_tasks`（DK-F 生产链）→ 确定性校验 → 新修订草案 + diff | `crates/pm-ai/src/plan_decomposition.rs` |
| 计划状态机纯函数 | `TaskPlanStatus` 四态 + `TaskPlanRevisionState` 五态 | `crates/pm-domain/src/plan.rs` |
| REST（14 端点） | `apps/pm-api/src/pm_plans.rs`；裁决复用 `/v1/pm/design-approvals/{id}/decide` | pm-api |

## 二、治理红线（提示词 §一/§6 原则）

1. **AI 无批准路径**：计划批准/退回/替代一律 `require_human` + `ensure_trusted_context` 双闸；拆解产物恒为 `draft`，不得直接写 `delivery.task`（测试 `ai_cannot_decide_plan_approval` 覆盖服务层 Forbidden + 未批准计划正式化 Conflict）。
2. **未批准计划不得产生正式任务**：`materialize_plan` 硬检查计划 `approved` + 修订 `approved`（红线用例覆盖）。
3. **draft→ready 沿用命令通道红线**：正式化任务一律 `draft`；置 ready 必须真人（复用 `mark_task_batch_ready` 的 executor 红线，测试覆盖 AI 被拒）。
4. **Q2 重拆不覆盖**：拆解永远产生新 Revision；diff（add/update/remove）在 warnings 透出；`removed` 明确标注「已正式化任务不被触碰」；修订触发器冻结已提交草案。
5. **Q6 状态映射**：PLANNED→`draft`、READY→`ready`、BLOCKED→`blocked`，不新增 13 态。
6. **审计**：`task_plan.created/submitted/approval.requested/decided/materialized/superseded` + 修订级 `created/updated`，全落 `governance.audit_log`。

## 三、核心流程与机制

```
S03 冻结基线（frozen + Gate 通过）
    → 建计划（钉基线）→ AI 拆解（准入=frozen+Gate 复判；LLM 产出草案批）
    → 确定性校验（键唯一/验收非空/依赖前向图——系统拥有骨架）
    → 新 Revision 草案 → 人工修改（可选）→ 提交（in_review，正文冻结）
    → 人类批准（Revision approved + 旧批准修订 superseded）
    → 正式化（幂等：任务 draft + 验收标准 + 仓库建议绑定 + 依赖边 + 追溯列）
    → 真人置 ready → 交接资格检查 → 执行上下文载荷 → S05
```

- **拆解准入（§1.2）**：基线必须 frozen；Readiness Gate 复判拦截「冻结后设计被 supersede/需求被取代」的漂移；无 LLM Provider 显式失败（不静默造数据）。
- **正式化（幂等）**：UNIQUE (plan_revision_id, plan_task_key) 库级保证；重复执行只报 `already_materialized`；仓库建议绑定校验属本项目 + verified（无效建议不阻断，可后补）；依赖边指向批内更早键（前向图结构性无环）。
- **交接（§3.3）**：五重资格检查（任务 ready / 来源计划 approved / 前置依赖 completed / 验收标准非空 / 仓库绑定有效）；载荷含追溯链（plan_revision / baseline / 验收 / 仓库 / 依赖）。
- **进度（§3.4）**：按状态分组计数 + 阻塞清单（含未完成上游数）+ 草案任务数对照。

## 四、验证证据

`crates/pm-ai/tests/plan_decomposition.rs` 5 用例全绿：

| 用例 | 覆盖 |
|---|---|
| `full_chain_plan_approve_materialize_ready_handoff` | 全链：基线→计划→批准→正式化（追溯列/验收/依赖边）→幂等→AI 禁 ready→真人 ready→交接载荷→依赖阻塞拦截→进度 |
| `ai_cannot_decide_plan_approval` | AI 批准 Forbidden；未批准计划正式化 Conflict |
| `plan_draft_validation_rejects_bad_batches` | 空批/无验收/重复键/前向依赖五类坏草案整条拒绝 |
| `plan_lifecycle_and_resplit_preserves_materialized` | 批准后禁再批/禁改稿；已正式化任务不被重拆触碰；替代链真人门 |
| `decomposition_requires_frozen_baseline` | 无基线拒绝；跨项目拒绝；无 Provider 显式 Model 失败 |

全 workspace 回归通过（见迁移台账 0011 行记录）。测试库泄漏已清理。

## 五、范围裁决记录（对提示词 WP 的映射）

| 工作包 | 落点 |
|---|---|
| WP-1 领域模型 | 迁移 0011 + `pm-domain/plan.rs`；Task Revision 不建全量表（提案 Q5 裁决：审计+取代链承担） |
| WP-2 S03 接入 | `source_baseline_id` + 拆解准入 + 交接载荷追溯链 |
| WP-3 AI 拆解 | `plan_decomposition.rs`（复用 DK-F 模型链，零新增 LLM 基建） |
| WP-4 依赖图 | 复用 `delivery.task_dependency` + T-023/T-025 引擎；计划内依赖边正式化时落库；计划级拓扑/关键路径视图未建（现有依赖接口已可查边与解锁，读模型归前端批次） |
| WP-5 仓库关联 | 复用 `delivery.task_repository` 绑定（校验本项目+verified）；`repository_ids` 建议随草案承载 |
| WP-6 审核/批准 | 专表审批 kind='task_plan' + 修订冻结 + 退回（reject→draft） |
| WP-7 执行准备 | `prepare_handoff`（资格检查 + 上下文载荷）；Run 创建/租约复用 D1/D2 既有机制，不建执行引擎 |
| WP-8 状态/进度 | 13 态复用 + `plan_progress` 读模型 |
| WP-9 权限/审计 | 真人门双闸 + AI 来源留痕 + 全链审计行 + 部分唯一索引并发防护 |
| WP-10 API/事件/测试 | 14 端点；事件目录**未新增**（`TaskPlanApproved` 等消费方出现时再登记——不假装消费）；5 集成用例 |

## 六、已知边界与后续项

1. **计划级拓扑/关键路径 API 未建**：现有 `task_dependencies` 接口已提供边管理与解锁判定；按依赖重算的进度视图归前端/读模型批次（提案 §4 开放问题 4 的「事件驱动重算」待裁决后实施）。
2. **重拆「逐项采纳」UI 流**：当前 diff 以 warnings 透出 + 整批确认；逐项采纳交互归 S04 前端。
3. **拆解的 `capability/roles` 上下文为空集**：现分解器 DK-F 契约的可选字段，S04 拆解按基线拆解语义未接角色候选（`ai_role_version_id` 仍可经 `update_task` 后补）。
4. **outbox 事件**：正式化/批准未发事件（TaskReady 由置 ready 链路已有）；消费方落地后随命令事务启用。
