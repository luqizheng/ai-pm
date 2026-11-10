# S05 · AI Agent Execution & Collaboration — UI 设计提示词索引

> - 记录日期：2026-11-10
> - 领域：S05 AI Agent 执行与协作（Figma AI 高保真页面级提示词）
> - 状态：v1 已落库，待逐页执行
> - 职责边界（五阶段既有裁决）：S01 项目初始化与代码认知 / S02 需求管理 / S03 技术设计与实施基线 / S04 任务拆解与执行规划 / **S05 Agent 执行、协作、人工介入与交付验收**
> - S05 消费已批准的任务与执行依据（S04-07 Readiness Handoff 的接收端），不创建正式需求、不做正式设计确认、不定义知识权威、不管 Git 配置（入口在 Project Settings）

## 视觉对齐裁决

- 全部页面锁定 V2 设计系统「Slate & Muted Teal」（`docs/design/pm-ai-design-tokens-v2.css`，全应用视觉唯一真源）：**深色主题为默认渲染目标**，浅色为 token 映射变体。
- 本次输入简报 §13 的「Neutral light background + restrained blue accent」为已被否决的早期草稿方向（与 `docs/ui/s04/INDEX.md` 视觉对齐裁决同源），**以 V2 为准**：主强调色雾蓝青 `#75A9A4`；**全站禁止紫色与渐变**；AI 生成内容用 Info 蓝 tint +「AI」徽标。
- 字阶：Page 21 / Section 15 / Body 13 / Caption 12 / Micro 11；ID 与计数用 tabular numerals。

## 领域语义裁决（防状态混淆，全部页面强制）

- **Task ≠ Run ≠ Agent ≠ Delivery ≠ Review**：Task 是工程工作项（13 态状态机，含 awaiting_decision 单状态 + waiting reason **五值** needs_info / needs_decision / needs_approval / external_dependency / **no_executor_available**——**不做 WAITING_FOR_* 状态**）；Run 是一次执行尝试（7 态 queued/claimed/running/succeeded/failed/timed_out/cancelled，attempt_no 按 kind 分轨 delivery/review，**一个 Task 可多次 Run**）；Agent 是参与执行的 AI 工人（Device 载体 + readiness：installed/connected/authenticated/ready/degraded/failed）；Delivery 是执行产出（逐仓 commit + 测试证据 + 交付清单）；Review 是人对 Delivery 的裁决（**三值** approved / changes_requested / inconclusive——未评审时为空，非第四值）。
- **完成三态分离**：Agent 声称完成（Run succeeded / Delivery submitted）≠ AI-PM 检查通过（manifest 对账 + review verdict）≠ 人工正式验收（Task completed，需验收证据齐备）。UI 必须让三者视觉可区分。
- 执行授权是「任务级一次性授权」（五重资格门 + 授权范围），ACP-07 的 Deadline 重授权是 Run 级续期——两者 UI 语义不同，不得混用。

## 文件清单

| 编号 | 文件 | 页面 | 核心作用 |
|---|---|---|---|
| 00 | [smark-pm-s05-00-design-master-prompt-v1.md](smark-pm-s05-00-design-master-prompt-v1.md) | 模块级总提示词 | 整体语境、设计哲学、领域语义 |
| 01 | [smark-pm-s05-ui-01-execution-center-v1.md](smark-pm-s05-ui-01-execution-center-v1.md) | Execution Center | 执行中心：需要人工处理 / 正在执行 / 排队 / 交付 / 活动流 |
| 02 | [smark-pm-s05-ui-02-execution-detail-v1.md](smark-pm-s05-ui-02-execution-detail-v1.md) | Execution Detail | 单任务执行过程：时间线 / Agent 活动 / 上下文 |
| 03 | [smark-pm-s05-ui-03-multi-agent-collaboration-v1.md](smark-pm-s05-ui-03-multi-agent-collaboration-v1.md) | Multi-Agent Collaboration | 多 Agent 并行、依赖与协调 |
| 04 | [smark-pm-s05-ui-04-human-intervention-v1.md](smark-pm-s05-ui-04-human-intervention-v1.md) | Human Intervention | 等待澄清 / 阻塞 / 失败 / 权限 / 决策五场景 |
| 05 | [smark-pm-s05-ui-05-execution-authorization-v1.md](smark-pm-s05-ui-05-execution-authorization-v1.md) | Execution Authorization | 执行授权确认流（S04 Handoff 的接收端） |
| 06 | [smark-pm-s05-ui-06-delivery-review-v1.md](smark-pm-s05-ui-06-delivery-review-v1.md) | Delivery Review | 交付审查：Commit / 测试证据 / 报告 / 验收清单 |
| 07 | [smark-pm-s05-ui-07-execution-history-v1.md](smark-pm-s05-ui-07-execution-history-v1.md) | Execution History | 执行历史与既往审查记录 |

每份页面提示词内嵌全局框架与 V2 令牌，**独立可投喂 Figma AI，不依赖上下文**；均含低保真线框、组件清单、交互状态、视觉风格四件套。

**参考件**：[reference/00-input-brief-s05-v1.md](reference/00-input-brief-s05-v1.md) — 原始需求简报归档 + 收敛映射表（哪条采纳、哪条按我们的设计覆盖、依据在哪）。**简报作参考，我们的提示词才是投喂件。**

## 连续工作流（与 S04 的闭环衔接）

```
S04-07 Execution Readiness「Prepare S05 Handoff」（S04 出口）
  → 05 Execution Authorization   人工授权（资格门 + 范围确认）
  → 01 Execution Center          总览执行态势与待办
  → 03 Collaboration             多 Agent 并行执行
  → 04 Human Intervention        等待澄清/阻塞/失败时人工介入（Journey B/C）
  → 02 Execution Detail          审计单任务执行过程
  → 06 Delivery Review           依据证据审查交付（Journey A/D）
  → 07 Execution History         回溯执行与审查记录
```

**生成顺序**：按用户裁决——**先整批生成 01→07 七页建立完整信息架构**；第二轮再单独迭代三个重点页的交互细节：**01 Execution Center（日常入口）→ 03 Multi-Agent Collaboration（核心差异化）→ 06 Delivery Review（业务闭环）**。

## 四条旅程（原型必须连通）

- **A 主闭环**：Approved Task → Authorization → Agent Execution → Delivery Submission → Human Review → Acceptance。
- **B 澄清恢复**：Agent Execution → Waiting for Clarification → Human Response → Resume（同一 Run 的 Continuation，不新建 Task 状态）。
- **C 失败重试**：Multi-Agent 并行 → 一 Agent 失败 → Human Intervention → Retry（新 Run 代次）→ Delivery。
- **D 退回重做**：Execution Complete → Delivery Review → Request Changes → New Execution Attempt（changes_requested，旧 Run 保留不覆盖）。

## 三个关键设计不变量

1. **执行可审计**：时间线事件（authorized / agent_started / stage_changed / delivery_submitted / command_*）全部带时间戳与来源；默认视图是可读的结构化事件，不是原始日志。
2. **人工控制点显式化**：授权、澄清答复、失败决策、验收四处人工动作都有明确的守卫状态与禁用原因；AI/Agent 永不代行人工决策。
3. **状态分层呈现**：页面任何位置都区分 Task 状态、Run 状态（含 attempt #）、Agent/Device readiness 三层，禁止用单一「状态」字段混排。

## 共享词汇

- 执行状态色：Ready `#8DC5AF` · Running/Info `#6EA8FE` · Waiting/Warning `#E0B15E` · Failed/Blocked `#F07E88` · Completed `#34C08B` · 中性（queued/cancelled/draft）灰系 `#A8BBC9`。
- 优先级：P0 Critical / P1 High / P2 Medium / P3 Low。
- Agent 命名：Architecture / Backend / Frontend / Test / Review Agent（角色徽标 + Device 名，禁止装饰性头像）。

## 契约真源（2026-11-10 逐项核对库级 CHECK）

| 契约 | 取值 | 出处 |
|---|---|---|
| Task 状态（13 态） | draft / ready / claimed / running / awaiting_decision / blocked / submitted / reviewing / completed / changes_requested / failed / cancelled / superseded | `delivery.task` CHECK |
| Run 状态（7 态） | queued / claimed / running / succeeded / failed / timed_out / cancelled | `delivery.task_run` CHECK |
| Run kind | delivery / review（attempt_no 按 kind 分轨） | `task_run_run_kind_check` |
| 执行形态 | human（Agent 自执行） / spawn（平台拉起进程） | `task_run_execution_mode_check` |
| Waiting reason（5 值） | needs_info / needs_decision / needs_approval / external_dependency / **no_executor_available** | `delivery.task_waiting` CHECK |
| Interaction 生命周期 | pending → responded / timed_out / cancelled → consumed | `delivery.task_interaction` CHECK |
| Continuation 状态 | open / started / consumed / closed / frozen | `delivery.continuation` CHECK |
| Review verdict（3 值） | approved / changes_requested / inconclusive（未评审为空） | `task_run_review_verdict_check` |
| Device readiness（6 态） | installed / connected / authenticated / ready / degraded / failed | `devenv.device` CHECK |
| Worker 命令 | 类型 agent.process.terminate / agent.task.continue；状态 issued → delivered → acked / failed | `delivery.worker_command` CHECK |
| 执行事件（8 类） | run_claimed / run_started / run_stage_changed / run_finished / command_issued / command_delivered / command_acked / command_failed | 迁移 0013 `delivery.task_run_event` |

**注**：`needs_info` 是历史值，域层已并入 `awaiting_decision`（V3 L1 合并裁决）——UI 展示统一用 `awaiting_decision` + 上述五值 reason，不再单独展示 needs_info 状态。
