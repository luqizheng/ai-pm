# S03-ACP 实施设计 · 通信面切片（ACP-01 / ACP-02 / ACP-08）V1.1
# —— N-1 / N-2 设计闭合与命令查询面

| 项目 | 内容 |
|---|---|
| 文档性质 | ACP 切片实施设计稿（待评审、待用户裁决） |
| 版本 | **V1.1**（收编初步独立评审 V1.0 全部 P0×4 / P1×5 / 验收 ACP-437~445 / R-ACP 裁决建议） |
| 裁决状态 | **R-ACP-1~9 已全部裁决（2026-11-07 用户裁决，见 §9）**；本稿待独立复审 |
| 设计基线 | **S03-ACP V1.3.1（已冻结，2026-11-07 用户决议）**：D-01～D-07 为已生效架构裁决；N-1/N-2 纳入 §23.3 须在切片实施设计完成前闭合 |
| 覆盖切片 | ACP-01（Interaction 领域模型）· ACP-02（Process Termination）· ACP-08（Adapter 能力与集成） |
| 本稿闭合项 | §23.3：Worker Command 持久化/轮询/幂等/失败升级 · N-1（等待窗口调度互斥）· N-2（interaction.required 与消费回执物理通道）· Interaction CANCELLED/TIMED_OUT/唯一活跃约束 · awaiting_decision 映射 · Process Termination 失败处理 · Adapter 能力验证 |
| 不覆盖（另稿） | ACP-03（REST/UI 回复入口权限模型）· ACP-04（Delivery/Consumption 持久化细节）· ACP-05（Continuation 状态机）· ACP-06（Checkpoint/Workspace Lock）· ACP-07（Deadline 授权）· ACP-09（Model A 回归） |
| 代码基线 | ai-pm master @ 923ab83（2026-11-07）；关键事实引用见 §2 |
| 关联裁决 | V-05-1（device 升级 Client Registry）· V-05-2（Client 自管 push + ADR-0002 口径修订）· V-05-5（Runner 走 MCP 自领，ZCode/Codex/DSH 优先）· A*（正式确认仅可信 REST/UI，MCP 无确认入口）· V-6（正式写操作统一走同一业务应用服务） |

## 修订记录

- **V1.0 → V1.1**（收编初步独立评审 2026-11-07）：
  - P0-1：N-1 屏蔽判据由「Interaction.status 推导」重构为**持久化调度保护状态** `task_run.scheduling_shield`（§4.1），六情形决策表全覆盖，修掉 timed_out/cancelled 与 SQL 判据的矛盾。
  - P0-2：Worker Command 引入**领取租约**（claimed_by/claimed_at/claim_expires_at），delivered 过期可同目标重投；四态状态机不动（§3.2）。
  - P0-3：命令通道与 Run Lease 解耦，改用**独立 Command Delivery 身份**（Client Registry 凭据 + worker_id 绑定 + generation 只读比对）（§3.3）。
  - P0-4：标记文件路径修正为 `{AI_PM_WORKSPACE_ROOT}/{run_id}/{workspace_path}/.aipm/`；补齐 schema/校验/event_id 键控归档/重启扫描/改名失败恢复（§1.2）。
  - P1-1：投递拆为 SESSION_CREATED / INPUT_INJECTED / CONSUMPTION_CONFIRMED 三阶段；continue acked 最低判据上调为 INPUT_INJECTED；注入失败必须终止新进程（§1.4）。
  - P1-2：唯一活跃约束由「单条 PENDING」升级为「至多一条未完成处置的**介入链**」（§5.2）。
  - P1-3：terminate 失败冻结语义拆分为 `continuation_start_frozen_at`（启动冻结）与回复提交入口（仍按 §8.3 开放），与基线 §11.4 字面差异挂待裁 R-ACP-7（§6.5）。
  - P1-4：文件协议责任边界补齐——标记文件非权威存储（§1.2 末）。
  - P1-5：新增工具不再挂 `PM_MCP_READ`，设独立权限码 `PM_MCP_AGENT_REPORT` / `PM_MCP_COMMAND`（§1.6）。
  - 验收补 ACP-437~445（§8）；待裁清单更新为 R-ACP-1~9（§9）。

---

## 0. 载体双形态声明（本稿最重要的一层适配）

V1.3.1 中 "Worker" 一词在 V0.5 语境下由 **ai-pm-runner（用户 PC 客户端）** 承担（V0.5 基线 §2.3），但服务器 Model B 通道（ai-pm-worker 进程内编排）维持现状不变（V-05-2 裁决③）。因此本稿所有协议面按**双形态等价**设计：

| 载体 | 部署位置 | 命令查询面接入方式 | Interaction 上报接入方式 |
| --- | --- | --- | --- |
| ai-pm-runner（V0.5 目标形态） | 用户 PC，持 Client 级凭据（V-05-1 device 升级签发） | **MCP 工具轮询**（`pm_poll_commands` / `pm_ack_command`） | MCP 工具（`pm_report_interaction` / `pm_report_consumption`） |
| ai-pm-worker（服务器 Model B，现状保持） | 服务端容器 | 直连同一应用服务（进程内，不经 HTTP） | 同一应用服务直连 |

**统一纪律（V-6 架构约束）**：两条接入路径**必须**落在同一业务应用服务（`pm-application`）上；MCP 工具与 Worker 进程内调用都只是适配层。禁止为 Runner 另写一套写入逻辑。

**V1 实现覆盖补注（2026-11-07 复审 F-3）**：本批实现（84c67e9/1234162）的命令消费与标记文件侦测**仅 Runner 载体接线**；服务器 ai-pm-worker 载体仅接了超时清扫（terminate/continue 消费与侦测未接）——按 V-05-2③「服务器 Model B 通道维持现状」属显式延后，非双形态已等价；Worker 载体的命令消费随服务器侧需求另行接线。

本稿正文统一使用 "执行端" 指代 Runner / Worker 双形态。

---

## 1. N-2 闭合：interaction.required 与消费回执的物理通道

### 1.1 三段式通道模型

```text
Agent CLI（Codex / ZCode / DSH）
   │  ① Agent → 执行端：Adapter 侦测约定（Workspace 标记文件，§1.2）
   ▼
ai-pm-runner / ai-pm-worker（持有 Run Lease 身份）
   │  ② 执行端 → Platform：MCP 工具 pm_report_interaction（§1.3）
   ▼
Platform（pm-application InteractionService → task_interaction + awaiting_decision + 命令）
   │  ③ 回复消费方向：Platform 持久化回复（ACP-03）
   │     → worker_command 投递（§3）→ 新 Session 创建 + 初始输入注入（§1.4）
   ▼
执行端 → Platform：MCP 工具 pm_report_consumption（§1.5，消费回执）
```

### 1.2 ① Agent → 执行端：标记文件协议（完整版）

**主通道 = Workspace 标记文件；stdout JSONL 不作为 V1 通道**（理由：现状 stdout 解析仅服务于进程收割的 ExitOutcome 判定，`codex_executor.rs` 无 JSONL 协议面；混载引入截断/交织风险；标记文件可承载完整 payload 且可重读、可审计）。

**路径（对齐现状布局 F7）**：

```text
{AI_PM_WORKSPACE_ROOT}/{run_id}/{workspace_path}/.aipm/interaction.json
```

`workspace_path` 为该 Run 的仓库 checkout 段；Checkpoint 相对路径（§6.1）按**同一 Workspace 根解析规则**。协议全表：

| 项 | 约定 |
| --- | --- |
| Schema | `{"schema_version":"aipm.interaction.v1", "event_id", "reason", "title", "question", "options"?, "required_response", "checkpoint"?}`；`reason` 四值沿用等待语义（needs_info/needs_decision/needs_approval/external_dependency） |
| 大小限制 | ≤ 64 KiB，UTF-8；超限按非法处理 |
| 创建责任 | `.aipm/` 目录由 Agent 首次写入时创建（instruction.md 引导，§7.2）；**执行端不得代写标记文件**，只做校验、上报与归档改名 |
| 校验 | 执行端上报前校验 schema_version/reason 枚举/必填字段；不合格 → 改名 `interaction.{event_id}.invalid` 隔离 + 告警指标，**不产生 Interaction** |
| 归档 | 上报成功后改名 `interaction.{event_id}.reported`（**event_id 键控归档**，支持多次介入的历史文件并存，不复用固定 `.reported` 名） |
| 重启扫描 | 执行端启动/重连时扫描工作区中未归档的 `interaction.json` 与历史 `*.reported`/`*.rejected`/`*.invalid`；未归档文件按 event_id 幂等重报（Platform 返回原 interaction_id） |
| 改名失败恢复 | 上报成功但改名失败 → 文件保持原名，下轮按同 event_id 重报（幂等）+ 重试改名；连续失败计告警指标 |
| 并发新事件 | 上一文件未归档又出现新写入：按写入时间顺序逐个上报；被 Platform 唯一活跃链约束（§5.2）拒绝的 → 改名 `interaction.{event_id}.rejected` + 指标，不重试 |
| 幂等键 | `event_id`（Platform 侧唯一索引；重复上报返回原 interaction_id） |

**责任边界（P1-4）**：标记文件**不是 Interaction 的权威存储**——Platform 的 `task_interaction` 是唯一权威。Agent 只能**创建新事件**，不得修改已提交事件（instruction.md 明示）；执行端负责校验、幂等、上报、归档；文件上报成功 ≠ 人类回复已提交，更 ≠ 已消费。

### 1.3 ② 执行端 → Platform：新 MCP 工具 `pm_report_interaction`

**不修改现有四工具契约**（工程纪律红线）。本工具沿用**执行面身份**（Run Lease 身份，见 §3.3 双轨表）：

```text
pm_report_interaction(
  project_id, task_id, run_id,
  lease_token, lease_generation,          -- T-032 双校验，同 pm_get_run_context
  event_id,                               -- 幂等键
  reason, title, question,
  options: Option<Vec<{value,label}>>,
  required_response: bool,
  checkpoint: Option<{relative_path, available}>
)
→ { interaction_id, status: "pending", deadline_at, task_status }
```

| 设计点 | 规则 |
| --- | --- |
| 身份与权限 | 权限码 `PM_MCP_AGENT_REPORT`（新，§1.6）+ 租约三要素（token/generation/run scope）。**不新增人类确认语义，不违 A\***（§18.4：MCP 可用于 Agent 提交；人类回复仅 REST/UI，属 ACP-03） |
| 幂等 | `event_id` 唯一索引；重复上报返回原 `interaction_id` 不新建 |
| 介入链唯一 | 该 Task 存在**开放介入链**（§5.2）→ Conflict 拒绝 |
| awaiting_decision 映射 | 同一事务：Task → `awaiting_decision`（经既有 task_transition 门槛）+ `task_waiting` upsert + 调度保护状态 `human_wait`（§4.1） |
| deadline | `deadline_at = now() + AI_PM_HUMAN_INTERVENTION_TIMEOUT_SECS`（默认 120s，§8.1）；持久化于 Platform，重启不重算（ACP-419） |
| 终止命令触发 | Interaction 持久化成功后，**同一事务**创建 `worker_command(agent.process.terminate)`（D-03：立即终止；§11.3 步骤 3） |

### 1.4 ③ 回复投递三阶段（P1-1）

回复经 REST/UI 持久化（ACP-03）后，经 `agent.task.continue` 命令投递。三个阶段严格分离，**各阶段失败不得相互冒充**：

| 阶段 | 判据/证据 | 状态影响 |
| --- | --- | --- |
| **SESSION_CREATED** | 新进程 spawn 成功（pid/Job Object/进程组证据） | Continuation 域记录（ACP-05）；命令内部阶段，**单独不构成任何成功语义** |
| **INPUT_INJECTED** | 初始输入注入成功 + 注入后校验（写入回读/文件证据） | Delivery → `DELIVERED`；**`agent.task.continue` acked 的最低判据**；调度保护释放点（§4.1） |
| **CONSUMPTION_CONFIRMED** | `pm_report_consumption` 双证据（§1.5） | Delivery → `CONSUMED`；`interaction.consumed_at` 落值 |

**注入失败处置（不变量）**：SESSION_CREATED 成功但 INPUT_INJECTED 失败 → **立即终止该新进程**（未携带人工回复的进程不得存活）→ 命令 `ack(outcome=failed, error=injection_failed)` → 调度保护**不释放**（保持 `start_wait`）→ Delivery 按原 `delivery_id` 幂等重投 → continue 重试 → 新 Session。系统不变量：**任何存活进程要么已注入人工回复，要么正在被终止**。

因此 acked 从严判据（对 §3.2 修订）：continue 命令 acked 要求 receipt 同时含 `session_created: true` **且** `input_injected: true`。

### 1.5 ④ 消费回执：新 MCP 工具 `pm_report_consumption`

```text
pm_report_consumption(project_id, task_id, run_id, lease_token, lease_generation,
                      delivery_id, interaction_id,
                      evidence: { kind, detail })
→ { interaction_status: "consumed" | "rejected", delivery_status }
```

| 设计点 | 规则 |
| --- | --- |
| 权限/身份 | 权限码 `PM_MCP_AGENT_REPORT`（§1.6）+ 执行面租约三要素 |
| 证据种类（V1） | `initial_input_injected`（初始输入写入成功）+ `process_alive`（注入后进程存活读取，带 pid/job-object 证据）。二者同时具备才接受 |
| N-6 合法稳态 | Adapter 无法提供可靠证据时**不伪造回执**（§8.6）；`RESPONDED 未消费 + Continuation RUNNING` 为**合法稳态**：不阻塞执行、不计失败，计告警指标（`interaction_unconsumed_total`），由 ACP-08 按 Adapter 分级声明 |
| 租约过期限定 | 该工具仍走执行面租约校验；若等待期间 Deadline 过期致租约不可续（D-02 后果），回执提交失败 → Delivery 停留 `DELIVERED`，落入 N-6 稳态告警路径——可接受（Run 终止在即），不为此放宽命令面身份 |
| 幂等 | 同一 `delivery_id` 只能有一条有效消费结果；重复提交返回原结果 |

### 1.6 权限边界（P1-5，新增工具不挂 READ）

现有四执行器工具沿用 `PM_MCP_READ` 历史语义（= 执行器面通行码）**不动**（契约红线；随 V-05-1 迁移批再评估重命名）。本稿四个新工具设独立权限码：

| 工具 | 权限码 | 身份轨（§3.3） | 写语义 |
| --- | --- | --- | --- |
| `pm_report_interaction` | `PM_MCP_AGENT_REPORT`（新） | Run Lease | 写 task_interaction / task_waiting / 调度保护 / 命令 |
| `pm_report_consumption` | `PM_MCP_AGENT_REPORT`（新） | Run Lease | 写 delivery consumed / interaction consumed_at |
| `pm_poll_commands` | `PM_MCP_COMMAND`（新） | Command Delivery | 命令领取（delivered + claim 租约） |
| `pm_ack_command` | `PM_MCP_COMMAND`（新） | Command Delivery | 命令回执（acked/failed + receipt） |

权限码语义边界：`PM_MCP_AGENT_REPORT` = 执行端**运行事件上报**（Agent 侧事实，永无人类确认语义）；`PM_MCP_COMMAND` = 命令通道**收发**；读取 Run 上下文 = 既有执行面；**人类正式确认 = 永不出现在以上任何码内**（A*，仅 REST/UI，ACP-03）。权限码命名已经 2026-11-07 用户裁决（R-ACP-8）确认；字典落地随 V-05-1 迁移批。

---

## 2. 代码事实基线（引用）

| # | 事实 | 证据 |
| --- | --- | --- |
| C1 | MCP 四工具已齐且契约冻结：claim（一次性明文 lease_token）/context/progress（心跳=续租）/submit；错误码九项声明 | `crates/pm-mcp/src/tools.rs:664-841`、`server.rs:381-469` |
| C2 | Worker 纯 DB 轮询：`claim_and_start` + `poll_active_runs` 四信号（deadline①/legacy④/心跳陈旧/进展陈旧）+ 下半程续租；无推送通道 | `pm-application/src/run_executor.rs:43-357`、`apps/ai-pm-worker/src/main.rs:104-183` |
| C3 | 回收路径 B3-2：lease 过期 → 隔离 + 同 Run 重领 + `session_replaced` 覆盖 | `run_dispatch/claim.rs:490-548`、`lease.rs:60-167` |
| C4 | 续租前置 `deadline_at > now()`；deadline 首次 claim 固化 | `claim.rs:730-731`、`0023:8-16` |
| C5 | `delivery.task_waiting`：reason 四值 + questions JSONB + deadline 列 + 每任务一条活跃（部分唯一约束） | `0010_task_state_machine.sql:90-105` |
| C6 | Task 13 态含 `awaiting_decision`（进入需 open question，离开需 resolve） | `0010:149-157`、`task_transition.rs` |
| C7 | 进程终止能力现成：Windows Job Object / Unix 进程组 + ExitOutcome 五值 | `codex_executor.rs:484-502,640-685` |
| C8 | MCP 鉴权面：purpose claim 映射（auth.rs:133-148）、Host 白名单、权限表 `MCP_TOOL_PERMISSIONS`（执行器工具仅 `PM_MCP_READ`） | `apps/pm-api/src/auth.rs:586-661` |
| C9 | `worker_command` / `task_interaction` / interaction 事件：**零实现**，仅设计稿提议 | V1.3 评审 F1/F2 + 本稿复核 |
| C10 | Outbox 事件目录 13 项，无 inbox；无 interaction 事件 | `outbox.rs:100-182` |
| C11 | Workspace 布局 `{AI_PM_WORKSPACE_ROOT}/{run_id}/{workspace_path}`，创建于 worktree checkout；全仓无删除 Run 工作区代码 | `port.rs:56-68,133-137`、`codex_executor.rs:180-276,417-439` |

---

## 3. Worker Command 命令查询面（§23.3 闭合项 1）

### 3.1 存储：`worker_command` 表（迁移号落地前重核 `docs/ai-pm-migration-ledger.md`，见 R-ACP-6）

按 V1.3.1 §10.2 全字段落地 + 本稿实施定案：

| 字段 | 实施定案 |
| --- | --- |
| `worker_id` | **= 目标 Run 的 `lease_owner` 值**（下发时固化）。不做广播命令（V1 无需求） |
| `lease_generation` | 下发时自 Run 读取固化（**独立列**，非仅 payload 内）——供命令面只读比对防跨代次误领（§3.3），不要求调用方出示 token |
| `command_type` | V0.5 集合 = `agent.process.terminate` · `agent.task.continue`（§5.2/§5.4 报文即 payload 结构；扩充经正式评审） |
| `payload` | JSONB：terminate = §5.2 报文；continue = §5.4 报文（含 `response`、checkpoint 引用、`continuation_id`） |
| `status` | `issued → delivered → acked \| failed`（基线 §10.3 四态，CHECK 约束；**本稿不改四态**） |
| `claimed_by` / `claimed_at` / `claim_expires_at` / `claim_count` | **领取租约元数据（P0-2）**，见 §3.2 |
| 幂等 | `command_id` 主键；同 `(run_id, command_type, interaction_id)` 仅允许一条非 failed 活跃命令（部分唯一索引） |

Outbox 分工不变（§10.5）：命令生命周期不入 Outbox。

### 3.2 领取租约与崩溃重投（P0-2）

`delivered` 的语义收窄为「**已被某执行端实例领取，处于有界 claim 租约内**」，不再是终局归属。重投规则：

```text
可领取条件：status = 'issued'
         OR (status = 'delivered' AND claim_expires_at < now())

领取（原子）：
  UPDATE worker_command
     SET status='delivered', claimed_by=调用方, claimed_at=now(),
         claim_expires_at=now()+AI_PM_COMMAND_CLAIM_LEASE_SECS,
         claim_count=claim_count+1
   WHERE command_id=? AND worker_id=调用方
     AND (status='issued' OR (status='delivered' AND claim_expires_at<now()))
  RETURNING ...     -- 影响行数=0 → 不返回该命令
```

- `AI_PM_COMMAND_CLAIM_LEASE_SECS` 默认 300s。
- **重投复用同一 `command_id`**；执行端按 command_id 幂等执行：terminate → 进程已退出则直接 ack；continue → 按 continuation_id 核实 Session 已存在则核实注入证据后 ack，不存在则（重新）执行。
- `delivered` 且 claim 未过期时，同目标执行端重启后仍可**提前重领**（ claimed_by == 调用方 时允许无条件重领，幂等）。
- **failed 仅由明确最终失败条件产生**（按命令类型的有界重试预算耗尽，§3.5），**claim 过期永远不是 failed**。
- 指标：`worker_command_claim_expired_total`、`claim_count` 分布、`worker_command_pending_age`。

### 3.3 身份双轨（P0-3）：Run Lease 身份 vs Command Delivery 身份

| 通道 | 身份 | 校验要素 | 依赖有效 Run Lease？ |
| --- | --- | --- | --- |
| **执行面工具**（get_run_context / report_progress / submit_result / report_interaction / report_consumption） | Run Lease 身份 | token（一次性明文）+ generation + run scope（T-032 / ADR-0002 D2 七点） | **是** |
| **命令面工具**（poll_commands / ack_command） | **Command Delivery 身份 = Client Registry 身份**（V-05-1 签发的 Client 凭据；服务器 Worker 形态 = worker_id 进程内直连） | ① 凭据有效且未撤销（V-05-1 撤销即通道关闭）② **worker_id 绑定**：命令路由目标 == 调用方身份 ③ **generation 只读比对**：`command.lease_generation == run.lease_generation`（服务端读库比对，防过期执行端跨代次误领，无需 token） | **否** |

理由与边界：

- 等待窗口内 Deadline 过期使租约不可续（D-02 + C4），命令通道若仍要求有效租约即自断——必须解耦。
- 解耦后的安全性由 worker_id 绑定 + generation 比对提供等价 fencing；等待窗口内 B3-2 被 S2 拦截（§4.2），`lease_owner` 不变，路由目标稳定。
- ADR-0002 D2 七点管的是**执行面写操作**凭据；命令收发是平台对执行端的**操作投递**，不是 Run 执行写。ADR-0002 口径修订①（V-05-2：执行端不接触平台持有的凭据）不受影响。
- 工具签名相应修订：`pm_poll_commands(project_id, run_id)`、`pm_ack_command(project_id, command_id, outcome, receipt, error?)` —— **不再携带 lease_token/generation**。

### 3.4 执行端轮询节奏

- Runner：复用心跳 poll 循环，`pm_poll_commands` 与 `pm_report_progress` 同循环不同调用；命令轮询间隔 `AI_PM_COMMAND_POLL_INTERVAL_SECS`（默认 3s）。**命令通道不依赖租约续租有效性**（§3.3 已落实）。
- Worker（服务器形态）：进程内直连应用服务，逻辑同构。
- 领取后即时执行；执行结果经 `pm_ack_command` 回执；回执本身失败（网络）→ 下一轮 poll 重领（claim 过期或 claimed_by==自方）后重新回执（幂等）。

### 3.5 失败升级

| 场景 | 升级规则 |
| --- | --- |
| 执行端离线 | 命令保持 issued / delivered（claim 过期后可重投），等待恢复；**不超时作废**（作废引入假稳态） |
| terminate 执行失败 | 执行端按 §6.4 有界重试（`AI_PM_PROCESS_TERMINATION_TIMEOUT_SECS=10`，次数/间隔配置化）；耗尽 → `pm_ack_command(outcome=failed)` → Platform：Task 进阻塞路径 + 告警 + `continuation_start_frozen_at` 落值（§6.5）+ Interaction 保持 PENDING |
| continue 执行失败（含注入失败） | 命令 failed（receipt 注明失败阶段：SESSION_CREATED / INPUT_INJECTED）；回复保留可幂等重投（原 delivery_id）；未携带回复的新进程已按 §1.4 不变量终止 |
| Platform 下发后命令悬挂 | `worker_command_pending_age` 超阈值告警（不自动升级为其他命令） |

---

## 4. N-1 闭合：持久化调度保护状态（P0-1 重构）

### 4.1 调度保护状态机（`task_run.scheduling_shield`）

**载体**：`task_run` 新增持久化列 `scheduling_shield`（P0-1 评审建议的「等价持久化状态」落位；它只是调度器保护标志，**不是 Run 7 态状态机的扩展**，13 态/7 态红线不动；载体已经 2026-11-07 用户裁决 R-ACP-9 确认为 task_run 列先行，ACP-05 设计时再评估迁移 continuation 域）。由 InteractionService 在各事务内**显式维护**，不由 Interaction.status 推导：

```text
none ──创建 Interaction──▶ human_wait ──回复持久化──▶ start_wait ──continue acked──▶ none
  │                           │
  │                           ├──超时判定（§8.2）──▶ timed_out ──retry_of 新建──▶ human_wait
  │                           │                        │
  └──Interaction CANCELLED◄──┼────────────────────────┤（Task 取消/归档 → 全部归 none）
                             ▼                        ▼
                        （Task 活：→ none，走正常重领）（未人工处置：保持）
```

**六情形决策表**（评审要求的完整覆盖）：

| 情形 | Interaction | 调度保护 | 自动调度（poll 四信号/回收/无人值守） | 新 Session 启动 |
| --- | --- | --- | --- | --- |
| 1. 人工等待中 | `pending` | `human_wait` | **屏蔽** | 禁止 |
| 2. 已回复未启动 | `responded` 且无 acked continue | `start_wait` | **屏蔽** | 禁止（仅 continue 命令可启动） |
| 3. 等待超时未处置 | `timed_out`（无 retry 后继） | `timed_out` | **屏蔽** | 禁止 |
| 4. 新 Session 已启动 | `consumed` 或 responded+acked continue | `none` | 正常（新进程归 poll 管辖） | — |
| 5. 介入已取消（Task 活） | `cancelled` | `none` | 正常（走既有 B3-2/无人值守重领，携带原始上下文、无回复——介入已撤销，无回复可携带；ACP-428 不适用，其约束对象是未决介入） | 允许（经正常重领路径） |
| 6. 旧 Process 退出未确认 | 任意 | 维持原值 | **屏蔽** | **禁止**（§9.4 检查 8 的操作化：Run 存在未收到终止回执的 terminate 命令 → 一律拒绝启动） |

补充转换规则：

- `timed_out` 后人工显式处置（重试 → 新 Interaction 挂 `retry_of`，或 Deadline 授权 ACP-07，或取消 Task）才离开 `timed_out`；**超时永不解释为同意**（§8.2）。
- Task 取消/归档：未决 Interaction 全部 `cancelled`（审计保留），guard → `none`（终态本身不可调度，清零仅为 hygiene）。
- 情形 5 的已下发 terminate 命令照常执行（进程终止完成后再重领，避免旧进程与重领进程并存）。
- `interaction_shielded_runs` 指标按 guard 值分维暴露；shielded Run 对自动调度不可见是**预期行为**。

### 4.2 屏蔽点落位（三处，全部服务端判定）

| # | 现有机制 | 屏蔽规则 | 验收 |
| --- | --- | --- | --- |
| S1 | `poll_active_runs` 四信号（run_executor.rs:233-357） | guard ≠ `none` 的 Run **全部四信号跳过**——deadline 到期也不得 TimedOut（ACP-427）；跳过时记录 guard 值供审计 | ACP-427/439 |
| S2 | 回收重领 B3-2（claim.rs:490-548） | 重领前置校验：guard ≠ `none` → **拒绝重领**，隔离原因 = `scheduling_shield`，不启动任何新进程。guard → `none` 后自然走既有回收路径（不新建专用路径） | ACP-428 前半 |
| S3 | 崩溃恢复 + 无人值守领取（`AI_PM_RUN_DISPATCH`） | spawn 前校验 guard ≠ `none` → 不启动。**恢复后不得启动未携带人工回复的新进程**（ACP-428） | ACP-428 |

### 4.3 等待窗口内的租约语义

- 执行端在等待窗口内**保持租约并继续尝试续租**（pm_report_progress）；Deadline 过期后续租失败——**预期行为**，不触发终止、不触发回收（S1 已屏蔽）。
- 「guard ≠ none + lease 过期」为合法稳态：命令面按 §3.3 独立身份可达（ACP-438）；`agent.task.continue` 执行前置的 §9.4 八项检查会因 Deadline 失效拒绝启动并转入 ACP-07 上层授权流程——本稿只保证"拒绝得干净"，授权机制另稿。
- `lease_owner` 在等待窗口内不变更（S2 拦截），命令路由目标稳定。

---

## 5. ACP-01：Interaction 领域模型（落地定案）

### 5.1 `task_interaction` 表

按 V1.3.1 §12.1 全字段落地，实施定案：

| 项 | 定案 |
| --- | --- |
| 状态枚举 | `pending / responded / consumed / timed_out / cancelled`（RESPONDED 未消费合法稳态见 §1.5；N-3 的 AUTHORIZATION_REQUIRED 属 Continuation 域，不进本表） |
| `chain_open` | **持久化布尔列**（§5.2）：由应用服务在各事务内维护——创建/重试时 true，`consumed`/`cancelled` 时 false；`timed_out` 保持 true（链未处置不关闭）。它使「链开放」可被索引表达，避免状态推导 |
| 介入链唯一约束 | `UNIQUE(task_id) WHERE chain_open`（部分唯一索引；**链级**，替代 V1.0 的单条 PENDING 约束，§5.2） |
| `retry_of` | 超时重试挂链字段（§5.2） |
| `continuation_start_frozen_at` | terminate 重试耗尽时落值（**启动冻结**，P1-3，§6.5；替代 V1.0 的 `response_frozen_at` 语义） |
| 事务边界 | 创建 Interaction + Task 转 awaiting_decision + task_waiting upsert + guard=human_wait + terminate 命令创建 = **同一应用服务事务**（§12.10 + V-6） |
| CANCELLED 触发 | Task 取消/归档联动取消未决 Interaction（§14；审计保留不物理删除）；Task 活跃时的显式取消走管理操作（guard → none，§4.1 情形 5） |

### 5.2 介入链（P1-2）：唯一活跃约束的链级升级

**定义**：介入链（intervention chain）= 一条 Interaction 及其全部 `retry_of` 后代。**链开放** ⟺ 链内最新 Interaction.status ∈ {`pending`, `responded`, `timed_out`(未处置)}；`consumed` / `cancelled` 关闭链条。

业务规则：

1. **同一 Task 同时至多一条开放介入链**（唯一活跃约束：`UNIQUE(task_id) WHERE chain_open`，§5.1；链开放性由 `chain_open` 列持久化表达，不由状态推导）。
2. 超时后的重试**必须** `retry_of` 挂同一链（链保持开放、延续等待），不允许无关联新 Interaction 与旧记录并存；重试创建时**旧 Interaction 行 `chain_open` 置 false（superseded）、新行置 true**——链的连续性由 `retry_of` 表达，唯一索引只钉住每链最新行。
3. `responded` 但新 Session 未启动 → 链开放 → **不允许新建**。
4. `cancelled` → 链关闭 → 允许新建（新链）。
5. `task_waiting` 活跃行存在 ⟺ 链开放：链首开时 upsert、链关闭时 resolve（同一事务，§5.3）——与 C5 的每任务一条活跃约束天然对齐。

### 5.3 awaiting_decision 映射（§23.3 闭合项 4）

| 时刻 | Task 状态动作 | task_waiting 动作 |
| --- | --- | --- |
| Interaction 创建（链开） | → `awaiting_decision`（经 task_transition，open question 门槛满足） | upsert 活跃行：reason=interaction.reason，questions={question,options,interaction_id}，deadline=deadline_at |
| Interaction → responded | 不变（仍等待投递/消费） | 不变 |
| continue acked（INPUT_INJECTED） | 不在此事务内自动离开；离开动作 = 链关闭（consumed）或人工处置，走既有 resolve 门槛 | 链关闭时 resolve（resolution_note 引用 interaction_id + delivery_id） |
| Interaction → consumed | 链关闭 → resolve | resolve |
| Interaction → timed_out（无 retry） | 保持 awaiting_decision | 保留活跃行（链仍开放）；deadline 已过由既有展示表达；**不自动 resolve**（§8.2） |
| Interaction → cancelled（Task 活） | 链关闭 → 回转前置状态或随 Task 终态 | resolve（ended_by=system，note=cancelled_reason） |

---

## 6. ACP-02：Process Termination（落地定案）

1. **触发**：Interaction 持久化事务内创建 terminate 命令（§1.3）——无"确认转人工后杀"分支（D-03 终裁不重开）。
2. **执行**：执行端领命令（§3.2 领取租约语义）→ 校验（command_id/task/run/generation 只读比对/status，§3.3）→ 既有终止能力（Windows Job Object / Unix 进程组，C7）→ 等待真实退出（§4.4 时序：T4 不得提前启动新 Agent）。
3. **回执**：`pm_ack_command(acked, receipt={exit_outcome: ExitOutcome 五值之一, pid, terminated_at})`；异常按 §5.3 如实报告，禁止成功语义。回执失败 → §3.4 重投路径。
4. **重试与超时**：`AI_PM_PROCESS_TERMINATION_TIMEOUT_SECS=10` + 有界重试（次数/间隔配置化）。
5. **重试耗尽**（§11.4 + P1-3 拆分）：
   - `pm_ack_command(failed)` → Task 进阻塞路径 + 告警；
   - **`continuation_start_frozen_at` 落值**：旧 Process 未确认退出前，禁止一切新 Session 启动（决策表情形 6 + §9.4 检查 8）；
   - **人类回复提交入口不因此冻结**：§8.3 六项检查（deadline/状态/权限）独立管辖回复提交；回复照常先持久化（§4.4），终止确认后投递。对基线 §11.4「回复入口冻结」的此操作化已经 **2026-11-07 用户裁决（R-ACP-7）**确认，随本稿进入独立复审。
6. **Platform 收到 acked 后**：Process 状态更新（扩展既有进程清理账本，§12.6 优先扩展不新建）；Workspace 保留（§15 四不清理）。
7. **命令幂等**：重复 terminate（重投）→ 执行端发现进程已退出 → 直接 acked。

---

## 7. ACP-08：Adapter 能力与集成（V0.5 三家）

### 7.1 能力模型（静态声明，不建动态 Discovery —— §13.3）

`AgentCapabilities`（V1.3.1 §9 结构）+ 本稿新增两项，经 **Client Registry（V-05-1 device 升级）** 的能力三分结构声明：

| 能力 | V0.5 要求 | Codex | ZCode | DSH |
| --- | --- | --- | --- | --- |
| Task Execution / Workspace Binding / New Session / Initial Context / Process Termination | §13.1 必须 | ✅（既有 spawn/Job Object 面演化） | ✅（同机制） | ✅（同机制） |
| `interaction_detection` | = `marker_file`（§1.2） | `.aipm/interaction.json` 约定写入引导（instruction.md 扩展） | 同左 | 同左 |
| `consumption_evidence` | §1.5 证据组合 | `initial_input_injected+process_alive` | 同左 | 同左（无法满足 → N-6 合法稳态 + 告警，不得伪造） |
| Consumption Receipt 硬门槛 | **降级为分级声明**（V1.3 评审 R-7 建议采纳） | — | — | — |

能力不足的 Client：调度②（能力过滤）拒绝派发 Model B Continuation 型任务；启动前报告，不允许运行中发现（§13.3）。

### 7.2 instruction.md 扩展（Codex 先行）

在现有 `codex_executor.rs:359-376` instruction 基础上追加：需要人工介入时写 `.aipm/interaction.json`（§1.2 Schema，含 event_id/reason 四值/question/options）；**只创建新事件，不修改已提交事件**；**不得**通过任何 MCP 工具自称获得人工批准（A*：MCP 无确认入口）。

### 7.3 执行端进程面继承

Runner 的 spawn/收割/终止能力自 `codex_executor.rs` 演化（V0.5 基线 §2.2）：原子 pid 文件、ExitOutcome 五值、Job Object/进程组、孤儿按 pid 文件强杀——全部沿用，本稿不重定义。

---

## 8. 验收标准

沿用 V1.3.1 ACP-401~426 编号不动（N-4 映射表随 ACP-03~07 稿补）；ACP-427~436 为 V1.0 已列；本版新增 ACP-437~445（评审建议全数采纳）：

| 编号 | 场景 | 必须保证 |
| --- | --- | --- |
| ACP-427 | 等待窗口内 Run deadline 到期 | 不触发自动回收/重领；Interaction 生命周期不受 poll 信号干扰（S1） |
| ACP-428 | 执行端崩溃于等待窗口后恢复 | 不得启动未携带人工回复的新进程；B3-2 重领被 S2 拦截 |
| ACP-429 | Agent 写 `.aipm/interaction.json` | 执行端在侦测间隔内上报成功；Interaction + awaiting_decision + guard + terminate 命令同事务成立 |
| ACP-430 | 同 event_id 重复上报 | 幂等返回原 interaction_id，不新建 |
| ACP-431 | 开放介入链存在时再次上报 | Conflict 拒绝，不排队不覆盖 |
| ACP-432 | terminate 命令重复投递 | 幂等：进程已退出则直接 acked |
| ACP-433 | terminate 重试耗尽 | Task 阻塞 + 告警 + `continuation_start_frozen_at` 落值 + Interaction 保持 PENDING |
| ACP-434 | Adapter 无消费证据 | RESPONDED 未消费稳态合法、不阻塞、计告警；禁止伪造 CONSUMED |
| ACP-435 | Runner（Client 凭据）调用新工具 | 执行面/命令面身份轨各自校验生效；越轨调用一律拒绝（ADR-0002 D2 执行面七点无豁免） |
| ACP-436 | 撤销 Client 授权后 | Platform 不再派发新任务，且 Command Delivery 身份失效（命令通道关闭）；不收回用户个人仓库凭据（V-05-2②） |
| **ACP-437** | Command 已 delivered，Runner 在执行前崩溃 | claim 租约过期后，**同一 command 可重新领取**（同 command_id） |
| **ACP-438** | 人工等待期间 Run Lease 过期 | 执行端仍能按命令面身份（Client 凭据 + worker_id 绑定 + generation 比对）读取并处理命令 |
| **ACP-439** | TIMED_OUT Interaction 未处置 | 不触发无人工回复的自动重领（guard=timed_out 屏蔽） |
| **ACP-440** | Interaction 已 CONSUMED，但 Command ACK 丢失 | 不产生第二个并发 Session（claim 重投 + continuation_id 幂等核实） |
| **ACP-441** | Session 创建成功，但人工回复注入失败 | 不得将 Delivery 判定为成功；新进程立即终止；guard 不释放；原 delivery_id 幂等重投 |
| **ACP-442** | 标记文件上报成功，但文件归档（改名）失败 | 重启后重复扫描按同 event_id 幂等，不产生重复 Interaction |
| **ACP-443** | Workspace 中存在多个历史标记文件 | 按 event_id 独立识别与恢复 |
| **ACP-444** | Terminate 失败，人工提交回复 | 回复提交按 §8.3 正常持久化（R-ACP-7 已裁：仅冻结启动）；新 Session 在旧 Process 退出确认前一律禁止启动 |
| **ACP-445** | Task 取消与 Interaction 上报并发 | 不产生已取消 Task 上的有效新 Interaction |

---

## 9. 裁决记录

**2026-11-07 用户裁决：R-ACP-1~9 全部按本稿建议执行**（V1.1 复审前置裁决，R-ACP-7 构成对基线 §11.4 字面的正式操作化修订记录）：

| # | 事项 | 裁决 |
| --- | --- | --- |
| R-ACP-1 | N-2 物理通道 | ✅ **a) 标记文件主通道 + MCP 上报**（协议全案见 §1.2） |
| R-ACP-2 | N-1 屏蔽机制 | ✅ **a) 持久化调度保护状态 `scheduling_shield` + 六情形决策表**（§4.1） |
| R-ACP-3 | 命令路由与身份 | ✅ **a) worker_id=lease_owner + Command Delivery 独立身份**（§3.3） |
| R-ACP-4 | Interaction 与 awaiting_decision/task_waiting 双写 | ✅ **a) 同应用服务事务双写**（§5.3） |
| R-ACP-5 | 消费证据门槛 | ✅ **a) 双证据硬门槛 + N-6 稳态告警 + 三阶段分离**（§1.4/§1.5） |
| R-ACP-6 | 迁移号 | ✅ **0045~0046 一批两表**（task_interaction + worker_command + task_run.scheduling_shield 列）；落地前重核 `docs/ai-pm-migration-ledger.md`（0044 撞号 DEF-01 在裁，不影响 0045+） |
| R-ACP-7 | terminate 失败后的冻结范围 | ✅ **a) 仅冻结新 Session 启动（`continuation_start_frozen_at`），回复提交按 §8.3 保持开放**——对基线 V1.3.1 §11.4「回复入口冻结」的正式操作化：启动冻结承接其安全意图（旧进程未确认退出 ≠ 新进程可启动），回复提交入口的开放由 §8.3 六项检查与 §4.4（回复先行持久化）管辖。此操作化记录为用户裁决，随本稿进入独立复审 |
| R-ACP-8 | 新权限码命名 | ✅ **`PM_MCP_AGENT_REPORT` / `PM_MCP_COMMAND`**（语义边界见 §1.6；字典落地随 V-05-1 迁移批） |
| R-ACP-9 | 调度保护状态载体 | ✅ **a) `task_run.scheduling_shield` 列先行**（显式持久化、事务内维护）；ACP-05 设计时评估迁移至 continuation 域 |

---

## 10. 工程纪律引用

1. **不修改 MCP 四工具契约**；新增四工具（report_interaction / report_consumption / poll_commands / ack_command）走独立权限码（§1.6），`/mcp` 鉴权面仅扩权限表行。
2. 禁重建区继续有效：不建 task_execution/execution_result、不改 13 态状态机与 Run 7 态（scheduling_shield 为调度保护标志非状态机扩展）、WAITING_FOR_* 不做成状态、四工具契约不动。
3. 全部写入经 `pm-application` 统一应用服务（§12.10 清单）；MCP/Worker/Runner 仅适配层（V-6）。
4. 迁移号已裁 0045~0046 一批两表（R-ACP-6）；落地前重核 `docs/ai-pm-migration-ledger.md`；主 checkout 有并行会话在途物，禁 sweep、禁 `git add -A`。
5. 本稿落 `docs/ui/s03-acp/`（ACP 系列归档目录，见该目录 INDEX.md）；commit 时机由用户定。

---

**文档结束**
