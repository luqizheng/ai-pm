# S03-ACP V1.3《Task-Based Continuation》评审报告 V1.0

| 项目 | 内容 |
|---|---|
| 评审对象 | `docs/scenarios/s03/ui/acp/smark-pm-s03-acp-task-continuation-v1.3.md`（S03-ACP V1.3 正式设计方案 — Task-Based Continuation，专家讨论稿整理转录稿） |
| 评审性质 | 静态架构评审 + 代码事实核验 |
| 代码基线 | ai-pm master @ db466ab（2026-11-06） |
| 评审方法 | 文档内部一致性核对 + 对照 migrations/crates 实际实现逐条核验 |
| 结论 | **有条件通过**：方向采纳（取代 V1.2 Session 恢复路线），但存在 3 项 P0 必须先闭合，另有 7 项待裁决 |

---

## 1. 总评

V1.3 的核心转向 —— 放弃跨进程 Session 恢复、以 Task 为连续性根标识 —— **方向正确，且比 V1.2 更贴合代码现状**：

- 现状 Codex 执行器本来就是每次 Run 全新 spawn、收割即终，从未实现任何 resume（`codex_executor.rs` 无 `codex resume`/rollout 调用）。V1.2 要求的"跨进程 Session 恢复"在现状下是零基础纯新建；V1.3 的"新 Session 重建上下文"反而与现状同构。
- V1.3 §6.2 声明沿用的 Lease Generation + Token 双重校验，现状确实存在且语义一致（`claim.rs:722-735`、`lease.rs:228-245`、`tools.rs:566-568`）。
- Agent Deadline 绝对固化、不得重置的裁决（T-032，`0023:15-16`、`run_context.rs:146-160`）与 V1.3 §8.3 一致。

但评审同时发现：**V1.3 把未实施的 V1.2 纸面方案当作"现有架构"来描述**（§12 保留表中 Interaction、RESPOND_INTERACTION、interaction.consumed、Outbox/Inbox、Session 实体在仓库中全部不存在），导致实施范围被系统性低估。三项 P0 均由此而生。

---

## 2. 代码事实基线（核验所据）

以下事实全部核验于 master @ db466ab，评审发现均引用此处：

| # | 事实 | 证据 |
|---|---|---|
| F1 | Interaction 领域对象不存在：migrations 39 个文件与 crates 中 `interaction` 零命中；无 `interaction.consumed` 事件 | `outbox.rs:100-182` 事件目录无任何 interaction 事件 |
| F2 | `RESPOND_INTERACTION` 工具不存在：全仓 rs/sql/ts/md 零命中；MCP 工具共 19 个，均无 EXECUTED 类返回语义 | `server.rs`（#[tool] 清单） |
| F3 | 语义最近的既有对象是 `delivery.task_waiting`：reason 四值（needs_info/needs_decision/needs_approval/external_dependency）、questions JSONB、**有 deadline 列**（无消费者）、ended_at/ended_by/resolution_note、**每任务仅一条活跃等待**（部分唯一约束） | `0010_task_state_machine.sql:90-106` |
| F4 | Agent Session 实体不存在；现有 `task_run.executor_session` 是平台侧编排器生成的 `{executor_kind}:{uuid-v4}` opaque 标识，非 Agent 内部会话；重领时旧 session 被覆盖并留 `session_replaced` 事件 | `port.rs:154-165`、`run_executor.rs:68-124` |
| F5 | Run 状态 7 值 `queued/claimed/running/succeeded/failed/timed_out/cancelled`；每任务至多一个活跃 delivery Run（部分唯一索引 `uq_task_run_active`） | `run_dispatch/mod.rs:26-34`、`0011:82-83` |
| F6 | **Run deadline 绝对固化**：`task_run.deadline_at` 首次 claim 时 `COALESCE(deadline_at, now()+…)` 固化，重领/续租不得重置或延长；租约续期前置条件含 `deadline_at > now()` | `0023:8-16`、`claim.rs:730-731`、`lease.rs:203-222` |
| F7 | **Workspace 路径以 Run 为段**：布局 `{AI_PM_WORKSPACE_ROOT}/{run_id}/{workspace_path}`，创建于 executor start（镜像仓 `git worktree add` checkout）；**全仓无任何删除 Run 工作区目录的代码**（清理账本只 terminate 进程） | `port.rs:56-68,133-137`、`codex_executor.rs:102-104,417-439`、`run_cleanup.rs` |
| F8 | Task 状态 13 值，无 WAITING_HUMAN/CONTINUING；最近的是 `awaiting_decision`（进入需至少一个 open question，离开前必须 resolve 等待上下文） | `0010:149-157`、`pm-domain/src/task.rs:15-29`、`task_transition.rs:56,12,83-88` |
| F9 | 执行形态有两种：Model B（spawn，executor 嵌在 ai-pm-worker 进程内）与 **Model A（human/spawn，外部 MCP 客户端自己执行）**；终止能力具备（Windows Job Object 整树 / Unix 进程组 SIGTERM→SIGKILL + 孤儿按 pid 文件强杀） | `0028_run_execution_mode.sql:15-17`、`apps/ai-pm-worker/src/main.rs:110-183`、`codex_executor.rs:640-685` |
| F10 | 配置面全部为环境变量（`AI_PM_RUN_TIMEOUT_SECS=1800`、`AI_PM_RUN_DEADLINE_DEFAULT_SECS=3600`、`AI_PM_RUN_LEASE_SECS=300` 等），无 toml 配置框架 | `run_dispatch/mod.rs:158-184` |
| F11 | 存在 outbox（事件目录 13 项，无 inbox 表，入站侧另有 `inbound_event`）；OutboxWorker 按 claim 租约分发，无 handler 即死信可 replay | `outbox.rs:100-182`、`pm-infrastructure/src/outbox_worker.rs:113-149` |

---

## 3. 发现清单

严重级定义沿用既有惯例：P0 = 必须裁决/修正后才能进基线；P1 = 进实施设计前必须闭合；P2 = 应修但不阻塞。

### R-1 [P0] Continuation 与 Run 的关系未闭合，两种解释在代码事实下后果截然不同

**问题**。V1.3 对"人工介入后继续执行"到底是同 Run 还是新 Run，四处表述互相矛盾：

- §2.1 正文："具体是否创建新的 Run，应依据现有 Run 模型表达执行代次，不应通过创建新 Task 来实现" —— 留口子；
- §2.1 关系图写作 "Run 01 Continuation"（同 Run 下的延续？还是新的 Run？含糊）；
- §4.4 `agent.task.continue` 携带原 `run_id: "run-001"`、§10.1 有 `continuation_generation: u32` —— 指向**同 Run + 代次递增**；
- §10.3 数据关系图把 Continuation 画成 Task 的直接子节点（与 Run 并列）—— 指向**新 Run 或 Run 外实体**。

**为什么这不是措辞问题**（代码事实使两种选择各有硬后果）：

| 选择 | 后果 A：同 Run（generation++） | 后果 B：新 Run |
|---|---|---|
| Deadline | `deadline_at` 沿用原值，符合 §8.3"不得重置" | 新 Run claim 会重新固化 `deadline_at`（F6，`claim.rs:730-731`）—— **直接违反 §8.3 与 T-032 终裁** |
| Workspace | `{root}/{run_id}/` 目录天然延续（F7） | 路径换目录，与"Workspace 独立于 Run 保留"（§6.1/原则 4）冲突，需重设计布局 + 可恢复映射 |
| 唯一性约束 | 原 Run 仍活跃，`uq_task_run_active` 不冲突（F5） | 原 Run 必须先入终态或被隔离 |
| 现有先例 | 租约回收 B3-2"已启动→隔离+新建"走的是同 Run 重领 + `session_replaced` 覆盖（F4），代次由 `lease_generation` 表达 | 无先例 |

**建议**。裁决为**同 Run 模型**：Continuation 是 Run 内的执行代次（`continuation_generation` 与既有 `lease_generation` 的关系需一并定义，建议合并为一个代次概念，避免双计数），新 Session/新 Process 挂同一 Run。§2.1 图、§10.3 图、§10.2 记录据此统一改绘；`TaskContinuation` 的 `run_id` 语义改为"所属 Run"而非"新执行"。

### R-2 [P0] V1.3 把未实施的 V1.2 纸面方案当作"现有架构"，实施范围被低估

**问题**。§12 保留表把以下机制描述为"保留"，但它们在代码中**全部不存在**（F1/F2/F4/F11）：

| §12 声称"保留" | 代码事实 |
|---|---|
| Interaction | 无表、无结构体、无事件（F1） |
| RESPOND_INTERACTION | 工具不存在（F2） |
| interaction.consumed | 事件不存在（F1） |
| Outbox / Inbox | outbox 有；**inbox 无**（F11） |
| Agent Session（"允许一个 Task 多个 Session"） | Session 实体不存在，需新建（F4） |
| Adapter Capability Discovery | 无对应实现 |

§10 开头"在现有领域模型上扩展，不要重复建立已存在的实体"方向正确，但按 F1~F4，实际要**从零新建五个领域对象**：Interaction、Session、Continuation、Checkpoint、（Response/Delivery 记录）。

**建议**。V1.3 增补"§0 代码基线现状"一节，明确三档：① 已存在且直接沿用（Run/Lease/Deadline/Task 状态机/Outbox/进程终止）；② 不存在、本轮新建（上述五对象 + 命令通道）；③ 不存在且本轮不建（inbox 等）。§12 表格改列"V1.2 稿内机制"而非"现有机制"。这是验收 ACP-401~420 能否排期的基础。

### R-3 [P0] Workspace 生命周期模型与现状路径布局冲突，且清理策略缺位

**问题**。V1.3 原则 4"Workspace Persistent：独立于 Process 和 Session"与 §2.2"workspace_path 必须有可恢复映射"，对照现状有两个硬缺口：

1. **路径布局绑死 Run**：现状 `{AI_PM_WORKSPACE_ROOT}/{run_id}/{workspace_path}`（F7）。若 R-1 裁决为同 Run 模型，此矛盾自然消解（Run 目录跨代次延续）；若新 Run 模型，则必须重设计布局（如按 task_id/workspace_id 分段）+ 定义 Worker 重启后的重定位机制。两者 V1.3 均未写。
2. **清理策略只有半句**：现状从不删除 Run 工作区（F7），磁盘单调增长；§6.1 只说"清理必须由独立的任务清理策略决定"，该策略本身无任何定义（谁触发、保留多久、与 Task 终态的关系、审计要求）。

**建议**。随 R-1 裁决定布局；补一节"Workspace 清理策略"最小定义（终态 Task 保留 N 天 / 人工触发清理 / 清理前校验无未消费成果），并注明现状零清理是暂态事实。

### R-4 [P1] Platform → Worker 命令通道的传输机制未定义

**问题**。§4.2/§4.4 定义了 `agent.process.terminate` / `agent.task.continue` 报文（含 command_id 幂等要求），但**没有定义命令如何到达 Worker**。现状 Worker 是纯 DB 轮询模型：`poll_active_runs` 只处理 `lease_owner = 本 worker_id` 的 Run（F9），没有平台推送通道。

**建议**。二选一并写明：① 命令落库（新表或扩展现有清理账本模式）+ Worker 轮询，幂等键 = command_id，与现状同构；② 复用 outbox 派生。顺带定义命令的生命周期状态（issued/delivered/acked/failed）与超时升级。

### R-5 [P1] Model A（外部 MCP 客户端执行）完全未覆盖

**问题**。现状存在 `execution_mode = human/spawn` 两种形态（F9）。Model A 下"Agent 进程"是外部客户端的进程，Worker 无法执行 §4.2 终止命令，§3.1 全流程（终止→确认→新 Session）不可实施。

**建议**。V1.3 明确适用范围声明：Task-Based Continuation 仅适用于 Model B（spawn）执行形态；Model A 的人工介入退化为现有 `awaiting_decision` 流程（等待 + resolve，无进程生命周期操作）。或者，若要求 Model A 也纳入，需另行定义对外部客户端的约束协议。

### R-6 [P1] 人工回复入口未绑定 A* 隔离约束；RESPOND_INTERACTION 语义措辞有越线风险

**问题**。§12"RESPOND_INTERACTION 保留人工回复操作语义"未说明"人工回复操作"的主体是谁。结合已裁决的 A*（正式确认仅可信人类交互式 REST/UI、MCP 无正式确认入口、fail-closed）：

- 若指**人类经 MCP 工具回复** —— 直接违反 A*，不可接受；
- 若指 **Agent 侧消费回复的凭证操作** —— 成立，但必须改写措辞并明确消费判据。

同时，V1.3 全文未定义人类回复的入口（REST 端点？UI 页面？）与身份/权限要求（谁有权回复哪个项目的 Interaction —— I6-A Lead/Approver 语义是否延伸至此）。

**建议**。① 明确：人类回复仅经 REST/UI（沿用 I6-A 的身份基座），MCP 侧最多提供 Agent 消费回执；② RESPOND_INTERACTION 的语义重写为"Agent 确认已消费回复"的凭证操作（与 §9 `supports_interaction_consumption_receipt` 对齐）；③ 增补回复入口的权限模型（项目成员、角色门槛）。

### R-7 [P1] 回复投递模型自相矛盾：初始输入注入 vs 启动后投递失败

**问题**。§4.5 将人工回复定义为新 Session 的**初始输入**（创建时注入，注入成功即送达）；§11.2 却描述"Session 已创建、进程已启动之后回复提交失败、需要 delivery_id 幂等重试"的场景。若回复真是初始输入，创建成功 ≡ 投递成功，§11.2 场景不存在；若存在启动后投递，则回复不是初始输入而是独立投递步骤。两者只能选一（或分层共存但需写明边界）。

**建议**。统一为三层模型：持久化（Human Response 落库，先于一切）→ 投递（初始输入注入 or 消费拉取，可重试，delivery_id 幂等）→ 消费回执（CONSUMED 判据，与 R-6 的回执语义合并）。对 CLI 型 Agent（Codex），建议以"初始输入注入 + 启动回执"为 V1 落地形态，`supports_interaction_consumption_receipt` 作为能力分级项而非硬门槛（否则部分 Agent 接入即被排除，与 §9.1 兼容性目标矛盾）。

### R-8 [P1] Task 状态映射未给出，且 CONTINUING 建议不新增

**问题**。§7.1 列出 WAITING_HUMAN/CONTINUING 等并说"映射到现有 Task 领域模型"，但未给映射表。现状 13 值状态机门槛明确（F8）：`awaiting_decision` 进入需 open question、离开需 resolve —— 这与 Interaction 生命周期天然咬合。

**建议**。映射建议：`WAITING_HUMAN → awaiting_decision`（复用 task_waiting：reason=needs_decision/needs_info，deadline 列恰好承载 120s 截止，F3）；`CONTINUING → 不新增 Task 状态`，由 Run/Continuation 记录表达（Task 回到 claimed/running），避免动 `task_transition.rs` 的迁移面；`INTERACTION_REQUIRED` 为瞬时态，不落 Task。§7.1 据此重写。

### R-9 [P1] 立即终止的时机值得再裁决：可自动回答的问题也付出全流程代价

**问题**。§3.1 流程中 interaction.required 一到就立即终止进程。两个后果：① V0.1 设计目标中"依据已有基线自动回答"的路径被消灭（每次提问都= 杀进程 + 新 Session + 上下文重建）；② 人工即使秒回 reject/cancel，任务也已付出至少一次进程终止 + （若继续）新 Session 代价。结合 terminate 超时 10s 与 120s 等待，一次介入的任务中断窗口 ≈ 10~130 秒。

**建议**。裁决终止时机：**A. interaction.required 即杀**（现稿，简单、资源释放最快、但代价如上）；**B. 确认转人工后杀**（Platform 先判定是否可自动回答/是否需要人类，再发终止命令 —— 多一步判定，保住自动回答路径与 reject 场景）。倾向 B；若裁决 A，应在 §1.3 目标中显式接受该代价并删去自动回答预期。

### R-10 [P1] Interaction 生命周期缺少两个终态分支，且与 task_waiting 唯一性约束的咬合未写明

**问题**。§7.2 状态机（PENDING→RESPONDED→CONSUMED / PENDING→TIMED_OUT）缺：① **CANCELLED**（Task 被 cancel/archive 时挂起 Interaction 的归宿）；② TIMED_OUT 之后用户仍要求继续时"创建新的人工介入事项"（§8.2）对新 Interaction 的创建来源/与原 Interaction 的关联字段。另：现状每任务仅一条活跃等待（F3 部分唯一约束），ACP-420"同一 Task 多次人工介入"隐含要求两次介入不重叠（上一条结束才能开下一条）—— 该约束应写进 V1.3 成为显式规则。

**建议**。状态机补 CANCELLED 分支；增补"同一 Task 同时至多一个 PENDING Interaction"约束（与 F3 约束对齐）；TIMED_OUT 后新建 Interaction 时记录来源（superseded_by / retry_of）。

### R-11 [P1] 等待人工期间 Agent Deadline 是否冻结 —— 现稿与 T-032 终裁组合会把长任务推进死区

**问题**。现状 deadline 绝对固化、续租前置 `deadline_at > now()`（F6），"绝对截止优先于租约续期"。V1.3 未讨论等待人工的 120 秒（乃至多次介入累计）是否消耗执行预算。若不冻结：每次介入直接吃掉 run 剩余时间；多次介入后 deadline 耗尽 → §8.3 拒绝继续 + 不允许自动刷新 → 任务永久卡死，唯一出路"明确的上层授权"未操作化（谁授权、什么入口、什么记录）。若冻结：与 0023 注释"不得重置或延长"字面冲突，需要新裁决。

**建议**。此为必须裁决项（见 §5 Q-2）。倾向：**人工等待时间不计入 Agent Deadline**（deadline_at 顺延等待时长，修订 0023 语义注释为"不含人工等待"），或者至少将"上层授权"操作化为一个具体的 REST 操作 + 审计记录（复用 governance.human_intervention，intervention_type 增列）。验收集需补对应场景（见 §6）。

### R-12 [P2] Checkpoint 存储位置与"终止前记录"的可操作性

**问题**。§5.2 结构体有了，但存储位置（Platform DB 还是 Workspace 内约定文件）未定；§5.3 第 3 条"Worker 终止进程前，尽可能完成必要的工作状态记录"不可操作 —— Worker 无法得知 Agent 的心智状态，能做的只是落盘 Agent 已写出的文件。

**建议**。定义 checkpoint = Agent 写入 Workspace 的约定文件（如 `.aipm/checkpoint.json`），Platform 仅登记 `checkpoint_id` 引用并随 Interaction 请求上收摘要；"终止前记录"改为"终止前 Worker 确认 checkpoint 文件已刷新（若存在）"。崩溃场景（无 checkpoint）现稿已自洽（ACP-410），保持。

### R-13 [P2] 协议细节三处

1. §4.3 示例 `"exit_code": 0` 有误导性：被 SIGKILL/Job Object 终止的进程不会有 0 退出码；建议对齐现状 `ExitOutcome = Success/NonZeroExit/Signaled/Killed/Lost`（F9）改示例并声明 process_exited 与 exit_code 的组合语义。
2. 进程终止失败的升级路径缺失：`termination_timeout_seconds=10` 到期后呢？§11 只说"禁止启动新 Agent"，未定义 Platform 端等待终止确认的超时策略（重试 terminate？升级人工？Task 挂什么状态）。建议补：终止失败 → 重试 N 次 → 仍失败 → Task 进 blocked + 告警，Interaction 保持 PENDING 并冻结回复入口。
3. §8.1 toml 配置载体与现状环境变量面不一致（F10）；建议 V1 用环境变量（`AI_PM_HUMAN_INTERVENTION_TIMEOUT_SECS` 等）对齐现状，toml 作为后续配置框架演进的预留。

### R-14 [P2] 版本追溯断裂与验收集补齐

1. **版本追溯**：S03-ACP V1.0~V1.2 均不在仓内（仓内唯一相关稿为 `smark-pm-s03-agent-interaction-v0.1.md`），§14 版本表"V1.2 已被新裁决取代"无档可查，ACP-DEC-003-Q1 裁决同样无档。建议将 ACP 系列历史稿落仓归档（哪怕仅存目），并为 V1.3 附 revision-log。
2. **验收集补齐**（对应本次评审发现）：

| 建议编号 | 场景 | 预期 |
|---|---|---|
| ACP-421 | 人工回复先于进程终止确认到达 | 持久化回复但不启动新 Session，直至终止确认（§3.2 时序） |
| ACP-422 | 进程终止失败 + 重试耗尽 | Task blocked + 告警，Interaction 冻结回复入口 |
| ACP-423 | 等待人工期间 Agent Deadline 过期 | 按 Q-2 裁决执行（冻结或上层授权），不得静默继续 |
| ACP-424 | 同一 Task 上一条 Interaction 未结束又发起介入 | 依"至多一个 PENDING"约束拒绝或排队（R-10） |
| ACP-425 | Model A 执行形态请求人工介入 | 按 Q-5 裁决的退化行为执行 |
| ACP-426 | Checkpoint 内容与 Workspace 实际代码冲突 | 以代码为准重建上下文（§5.2 原则，现无验收项） |

---

## 4. 与既有裁决的对照

| 既有裁决/事实 | V1.3 关系 | 结论 |
|---|---|---|
| T-032：Run deadline 绝对固化不得延长 | §8.3 一致，但未处理人工等待消耗（R-11） | 需 Q-2 裁决 |
| A*：正式确认仅 REST/UI、MCP 无确认入口、fail-closed | §12 RESPOND_INTERACTION 措辞有歧义（R-6） | 需澄清后一致 |
| I6-A：Lead/显式 Approver、项目级授权模型 | 回复入口权限未提及（R-6） | 需补 |
| 任务状态机（13 值 + awaiting_decision 门槛） | §7.1 声明映射未给出（R-8） | 可闭合 |
| Lease Generation + Token 双校验、多 Runner 保护 | §6.2 沿用声明与现状吻合 | ✅ 一致 |
| S01 收尾确立的"正式写操作统一走同一业务应用服务"（V-6 架构约束） | Platform→Worker 命令、回复持久化应经应用服务层，V1.3 未提 | 实施设计时落实 |

---

## 5. 待裁决清单

| # | 问题 | 选项 | 评审倾向 |
|---|---|---|---|
| Q-1 | Continuation 是同 Run 代次还是新 Run | 同 Run（generation++） / 新 Run | **同 Run**（R-1，避免违反 Deadline 终裁与 workspace 布局冲突） |
| Q-2 | 等待人工期间 Agent Deadline 是否冻结 | 冻结（顺延，修 0023 注释） / 不冻结 + 操作化上层授权 | 冻结等待时长 |
| Q-3 | 进程终止时机 | interaction.required 即杀 / 确认转人工后杀 | 确认转人工后杀（R-9） |
| Q-4 | RESPOND_INTERACTION 语义 + 人类回复入口 | Agent 消费凭证语义 + REST/UI 入口 | 如左（R-6，绑 A*） |
| Q-5 | Model A 适用范围 | 声明仅 Model B / 定义退化行为 | 声明仅 Model B，Model A 走 awaiting_decision（R-5） |
| Q-6 | Checkpoint 存储位置 | Workspace 约定文件 / Platform DB | Workspace 文件 + Platform 登记（R-12） |
| Q-7 | ACP V1.0~V1.2 历史稿与 ACP-DEC-003-Q1 裁决是否落仓归档 | 归档 / 仅存目 | 归档（R-14） |

---

## 6. 结论

**有条件通过**。V1.3 的 Task-Based Continuation 方向采纳为 S03-ACP 的正式路线（取代 V1.2 Session 恢复方案），理由：与代码现状同构、实施成本低于 V1.2、六条冻结原则（§15）本身无异议。

进入基线确认前须完成：

1. **P0 × 3 闭合**：R-1（Continuation/Run 关系裁决 + 全文统一）、R-2（增补代码基线现状节，§12 表重述）、R-3（Workspace 布局裁决 + 清理策略最小定义）；
2. **Q-1~Q-7 裁决**收编进 V1.3 正文（或以修订稿 V1.3.1 形式）；
3. P1 项（R-4~R-11）在实施设计（切片设计稿）中逐项落实，P2 项（R-12~R-14）随修订稿一并修订。

评审通过标准：上述 P0 闭合 + 裁决清单落定后出 V1.3.1，本评审升级为"通过"。
