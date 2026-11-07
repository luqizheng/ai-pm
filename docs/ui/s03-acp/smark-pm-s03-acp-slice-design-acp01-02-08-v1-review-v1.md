# S03-ACP 通信面切片实施设计（ACP-01 / ACP-02 / ACP-08）V1.1 独立复审报告 V1.0

| 项目 | 内容 |
|---|---|
| 评审对象 | `docs/ui/s03-acp/smark-pm-s03-acp-slice-design-acp01-02-08-v1.md`（V1.1，稿头自称"待独立复审，复审通过前不进代码"） |
| 评审性质 | 独立复审（切片稿 §9 裁决记录与 INDEX 状态行所设门槛；本稿是该门槛的执行） |
| 代码核验基线 | **ai-pm master @ e87efd5（2026-11-07）**——比切片稿自述基线 923ab83 新四笔（含 DEF-01 迁移顺移 e87efd5），所有引用均按现行代码重验 |
| 前置输入 | S03-ACP V1.3.1 冻结基线（D-01~D-07、§23.3）；V1.3.1 独立评审 V1.0（N-1~N-6）；R-ACP-1~9 用户裁决（2026-11-07）；V0.5 MVP 范围基线 V1.2（V-05-1/2/4/5、WP-3/WP-5）；A* / V-6 架构约束 |
| 裁决边界 | **不重开 R-ACP-1~9**。全部意见均在已裁框架内提出；凡触及裁决文本的，只做"同一裁决下的定义精化"并明示 |
| 结论 | **不通过（须修订为 V1.2 后再复审）**。P0 × 4（其中 3 项由代码事实错位或缺位导致，1 项为状态机组合缺陷）、P1 × 6、P2 × 6。切片稿的总体路线（标记文件 + 四新工具 + 持久化调度保护 + 命令租约 + 身份双轨）**方向成立、与基线冻结裁决一致**，问题集中在"现有回收/租约/领取机制的事实描述"与"等待窗口两端的衔接"，修订范围有界 |

---

## 1. 结论摘要

1. **R-ACP-1~9 九项裁决在稿内落位一致**（§6），本复审未发现与裁决相悖的设计；R-ACP-7 对基线 §11.4 的操作化记录方式（显式登记为裁决、随稿复审）是正确做法，**应推广到另一处未登记的基线偏离（P1-6）**。
2. **代码事实 C1~C11 复核：9 项相符、1 项错位（C3）、1 项过期（R-ACP-6 迁移号）**（§2）。C3 的错位不是措辞问题——N-1 闭合（§4）整段建立在"B3-2 = 同 Run 重领"之上，而代码中 B3-2 是"隔离旧 Run + **新建** Run"，同 Run 重领是 B3-1 且在等待窗口内不可达。
3. **四项 P0**（§3）均为"按稿实施会在真实场景下卡死或违反基线不变量"的缺陷：
   - P0-1 B3-2 事实错位 → 调度屏蔽点的对象与范围写错（含 poll 第 ⑤ 步未被屏蔽会把等待中的 Run 直接收尾）；
   - P0-2 等待窗口内执行端崩溃/重启后，同 Run **没有任何租约重取路径** → continue 永不可执行（ACP-428 只写了"不得抢跑"，没写"如何合法继续"）；
   - P0-3 `lease_owner` 不是身份、Runner 与 Model A 在领取面不可区分 → 命令面绑定链断裂、`pm_report_interaction` 可在 Model A Run 上制造无人可执行的 terminate；
   - P0-4 Task 离开 `awaiting_decision` 的时机挂在 CONSUMED，而 CONSUMED 被定义为可能永不到达 → 新 Session 的交付被状态机拒绝、第二次介入被开放链挡死。
4. 六项 P1（§4）与六项 P2（§5）为有界修订。**V1.2 修订清单见 §7**，逐项可核。

严重级沿用系列惯例：**P0** = 进代码前必须闭合且改变设计实质；**P1** = 进代码前必须闭合、修订有界；**P2** = 应修但不阻塞。

---

## 2. 代码事实复核（C1~C11 + 稿内其他事实引用）

| # | 切片稿断言 | 复核结果 | 证据（e87efd5） |
|---|---|---|---|
| C1 | MCP 四工具已齐且契约冻结；claim 返回一次性明文 lease_token + generation | ✅ 相符。补注：`claim_task` 先过 `authorize(..., ExecutorKind::DevAgent)` 真人成员校验（T-051） | `crates/pm-mcp/src/tools.rs:664-693`（claim）、`696-712`（context）、`715-745`（progress）、`759-841`（submit）；`server.rs:381-469` |
| C2 | Worker 纯 DB 轮询：`claim_and_start` + `poll_active_runs` 四信号 + 下半程续租；无推送 | ✅ 相符。补注：`poll_active_runs` 只扫 `lease_owner = $worker_id` 的 Run；循环体是 **①②③④ 四信号 + ⑤ 存活探测/兜底续租/收尾回写**，⑤ 内含 `finish_run`（进程退出→Run 终态）与 `recover_lost`（续租失败→B3-2） | `run_executor.rs:233-357`（⑤ 在 316-354） |
| **C3** | 回收路径 B3-2：lease 过期 → 隔离 + **同 Run 重领** + `session_replaced` 覆盖 | ❌ **错位**。B3-2 = `isolate_started_run` + `insert_task_run`（**新 Run**，`attempt_no+1`、`supersedes_run_id` 指旧 Run，旧 Run 落 `timed_out`）。**同 Run 重领是 B3-1**，仅当 `executor_session IS NULL OR stage='dispatch'`（启动意图未确认）；`session_replaced` 属 B3-1 的 `claim_and_start` 覆盖路径。等待窗口内 Run 为 `running`/`started`/有 session → **只可能走 B3-2** | `claim.rs:100-102`（注释定义）、`496-547`（分支）、`lease.rs:137-167`（`recover_lost_run`）、`run_executor.rs:71-124`（session_replaced） |
| C4 | 续租前置 `deadline_at > now()`；deadline 首次 claim 固化 | ✅ 相符。**补充关键事实**：两条续租通道均要求**租约尚未过期**——平台兜底 `renew_lease` 带 `lease_expires_at > now()`；MCP 通道 `verify_row` 以 `lease_live` 判定，过期即 `LeaseError::Expired`。**过期租约在任何通道都不可续活** | `lease.rs:203-222`、`run_context.rs:102,142-146`；`claim.rs:729-731` |
| C5 | `task_waiting`：reason 四值 + questions JSONB + deadline + 每任务一条活跃 | ✅ 相符（`uq_task_waiting_active ... WHERE ended_at IS NULL`；`questions` 默认 `'[]'` 为数组） | `migrations/0010_task_state_machine.sql:90-105` |
| C6 | Task 13 态含 `awaiting_decision`（进入需 open question，离开需 resolve） | ✅ 相符。**补充关键事实**：`awaiting_decision` 的合法出边仅 **Cancelled / Ready / Running** 三条，**无 → Submitted**；离开前必须 resolve 等待上下文 | `0010:149-157`；`crates/pm-domain/src/task_transition.rs:37-39,83-88` |
| C7 | 进程终止能力现成：Job Object / 进程组 + ExitOutcome 五值 | ✅ 相符（`Success/NonZeroExit/Signaled/Killed/Lost`） | `pm-infrastructure/src/codex_executor.rs:417-560`（start/pid 文件）、`640-685`（terminate）；`port.rs:173-184` |
| C8 | MCP 鉴权面：purpose 映射 + Host 白名单 + `MCP_TOOL_PERMISSIONS`（执行器工具仅 `PM_MCP_READ`） | ✅ 相符（权限表 5 行全为文档写工具；四执行器工具只需 `PM_MCP_READ`） | `apps/pm-api/src/auth.rs:133-150,586-612` |
| C9 | `worker_command` / `task_interaction` / interaction 事件零实现 | ✅ 相符（全仓无同名表/事件/工具） | — |
| C10 | Outbox 事件目录 13 项，无 inbox，无 interaction 事件 | ✅ 相符（`CATALOG` 13 个 event_type） | `outbox.rs:100-` |
| C11 | Workspace 布局 `{AI_PM_WORKSPACE_ROOT}/{run_id}/{workspace_path}`；无删除 Run 工作区代码 | ✅ 相符 | `port.rs:133-137`、`codex_executor.rs` |
| R-ACP-6 | 迁移号 0045~0046 一批两表；"0044 撞号在裁，不影响 0045+" | ❌ **已过期**。DEF-01 修复（e87efd5，2026-11-07）把 `s02_i3` 顺移为 **`0045_s02_i3_source_traceability.sql`（已应用）**。ACP 批次须顺移 **0046~0047** | `ls migrations/` 尾项；`docs/ai-pm-migration-ledger.md` 0045 行 |
| §6-6 | "Process 状态更新：扩展既有进程清理账本（§12.6 优先扩展不新建）" | ⚠ **形状不合**。`delivery.task_run_cleanup`（0035）：`PRIMARY KEY (run_id)`、单 `session` 列、状态域 = 清理进度（pending/cleaning/completed/needs_human）、写入时机 = "取消/收割路径在撤凭据时"。它不能承载每 Run 多 Process、终止请求/实际退出时间、ExitOutcome、确认状态；且介入终止**不撤凭据**，现有写入路径不会触发 | `migrations/0035_s01_run_cleanup.sql:16-33`、`run_cleanup.rs` |
| §7.2 | instruction.md 现状位置 | ✅ 相符 | `codex_executor.rs:358-361` |
| 补核 | 默认时长 | `AI_PM_RUN_LEASE_SECS=300`、`AI_PM_RUN_DEADLINE_DEFAULT_SECS=3600`、心跳陈旧 900、进展陈旧 3600 | `run_dispatch/mod.rs:167-174` |
| 补核 | 启动期回收 | worker 启动时 `reconcile_orphans`（`run.orphan_suspected` 审计，>1h）；`run_executor.rs:1009` 的 `recover_lost` 同样复用 `recover_lost_run`（B3-2） | `apps/ai-pm-worker/src/main.rs:~124`、`run_executor.rs:438-470,1009` |
| 补核 | MCP 领取的 `lease_owner` 取值 | **`format!("mcp:{client_name}:{token[..8]}")`**，`client_name` 为 `ClaimTaskArgs` 调用方自报文本；注释明言"`lease_owner` 不是凭证"；`execution_mode` 对 MCP 领取恒为 `"human"`，仅 Worker 为 `"spawn"`（T-059 / ADR-0002 L5） | `claim.rs:32,701-720` |

---

## 3. P0 意见（进代码前必须闭合，改变设计实质）

### P0-1 B3-2 事实错位 → N-1 屏蔽点的对象、范围与"情形 5"全部需重写

**问题**。切片 §4 以"B3-2 = 同 Run 重领"为前提布置 S1/S2/S3，但代码事实（C3 复核）是：等待窗口内的 Run（`running`、`stage='started'`、`executor_session` 非空）一旦租约过期，**唯一可达的回收路径是 B3-2 = 旧 Run 落 `timed_out` + 新建 Run（新行 `deadline_at` 经 `COALESCE` 重新固化）**。这恰是基线 §1.3"不允许通过新建 Run 自动刷新 Deadline"与 D-01 同 Run 裁决要防的事。由此衍生四处失准：

1. **S1 范围**："guard ≠ none 的 Run 全部四信号跳过"只覆盖 ①②③④；第 ⑤ 步（`run_executor.rs:316-354`）仍会执行 `executor.poll(session)`——旧进程已被 terminate 命令杀掉 → `ExecutorProgress::Done` → **`finish_run` 直接把等待中的 Run 收尾为终态**；或续租失败 → `recover_lost` → B3-2 新建 Run。等待窗口在服务器 Worker 形态下第一轮 poll 即被破坏。
2. **S2 措辞**："拒绝重领"应为"拒绝 `recover_lost_run`（隔离 + 新建 Run）"，且调用方不止 `claim_core` 回收分支：`run_executor::recover_lost`（:1009）、`reconcile_orphans` 的后续清理路径同样复用该函数（`lease.rs:138` 注释明令复用）。
3. **§4.3 "guard → none 后自然走既有回收路径（不新建专用路径）"**：既有回收路径在该 Run 上的结果是**新 Run**，不是"同 Run 继续"——这句话与 continue 路径（必须同 Run）直接矛盾。
4. **情形 5（介入取消、Task 活）**："走既有 B3-2/无人值守重领，携带原始上下文"——真实结果是新建 attempt Run（`supersedes_run_id`）、原 Run 落 `timed_out`、`release_reason='lease_expired_started'`。这可能是可接受的（介入撤销 + 进程已杀 = 原 Run 事实放弃），但必须**如实写出并裁定**：是否接受"取消后以新 Run 重试"，以及旧 Run 的 `release_reason` 应为新值（如 `interaction_cancelled`）而非冒充租约过期。

**修订方向**（在 R-ACP-2 框架内）：

- C3 改正；S1 改为"guard ≠ none 的 Run **整条迭代跳过**（①②③④⑤，含 `finish_run` / 兜底续租 / `recover_lost`），跳过时记录 guard 值"；
- S2/S3 的拦截对象明确为 `recover_lost_run` 的**全部调用方**（`claim_core` 回收分支、`run_executor::recover_lost`、`reconcile_orphans` 清理后续）+ `claim_and_start` 的 spawn 前校验；拦截时审计 `reason='scheduling_shield'`；
- 情形 5 按真实路径重写并给出裁决项（§7 R-ACP-10）；
- ACP-428 文本改为"B3-2 隔离+新建 Run 被 S2 拦截"。

### P0-2 等待窗口内执行端崩溃/重启后，同 Run 无租约重取路径 → continue 永不可执行

**问题**。续租两通道都要求租约未过期（C4 补充）；同 Run 重领 B3-1 要求 `executor_session IS NULL OR stage='dispatch'`——等待窗口内为假；B3-2 新建 Run 被 S2 正确拦截。于是：**执行端离线超过租约 TTL（默认 300s）后恢复，或 Runner 进程重启丢失一次性明文 `lease_token`（稿内未要求本地持久化）**，该 Run 的租约永远无法重新取得。而 `agent.task.continue` 执行前置的 §9.4 检查 6（Worker Lease 有效）与 §11.1 步骤 4~6 永不满足 → 命令停留 delivered（claim 反复过期重领）、guard 停在 `start_wait`、Task 停在 `awaiting_decision`、**已持久化的人工回复永远投不出去**，且稿内没有为此定义告警或人工处置出口。§4.3 只处理了"Deadline 失效 → ACP-07 授权"这一支，漏掉"Deadline 仍有效、租约已死"这一支——后者才是 ACP-428（崩溃于等待窗口后恢复）的**正向半边**：不得抢跑已写，**如何合法继续**未写。PC 休眠/重启、`start_wait` 与 `timed_out` 无上界，这不是边角场景。

**修订方向**（二选一，推荐 A；在 R-ACP-3 身份双轨框架内）：

- **A）同主体续领（same-owner re-lease）**：新增一条应用服务操作（作为 continue 执行的第 0 步，Runner 形态经新工具 `pm_reacquire_lease(project_id, run_id, command_id)` 暴露；Worker 形态进程内直连）：当 `scheduling_shield = start_wait` 且调用方 Command Delivery 身份 == `command.worker_id` 且 Deadline 有效时，在**同一 Run** 上重签租约（`lease_generation + 1`、新 token、`deadline_at` 不动——与 T2/T3 字面一致，也落实基线 §7.2"新 Continuation 必须重新取得合法 Lease"）。随之必须修订 §3.3 的 ③ generation 只读比对：命令行记录 `issued_generation`，比对规则改为 `run.lease_generation ∈ {issued_generation, 经本路径续领的 generation}`（命令行写回 `reacquired_generation`），否则续领后命令自我封锁。
- **B）Runner 本地加密持久化租约凭据** + 显式接受"离线超过 TTL ⇒ 本次 continue 不可达"：Delivery → FAILED(`lease_unrecoverable`) + 告警 + 人工处置入口（取消介入 → 情形 5 新 Run 重试）。B 单独不足以闭合（TTL 窗口仍在），只能作为 A 的补充。

新增验收：**ACP-446** Runner 于 `start_wait` 崩溃、离线超过 `AI_PM_RUN_LEASE_SECS` 后恢复 → continue 仍在同 Run 上执行成功，`deadline_at` 不变，`attempt_no` 不变；**ACP-447** 旧 generation 执行端在续领后调用任何执行面工具 → 拒绝（T2 不变）。

### P0-3 命令面身份绑定链断裂：`lease_owner` 不是身份，Runner 与 Model A 在领取面不可区分

**问题**。切片 §3.1/§3.3 以"`worker_id` = Run 的 `lease_owner`"做命令路由，以"Client Registry 凭据 + `worker_id` 绑定 + generation 比对"做命令面安全。代码事实（§2 补核）：

- MCP 领取的 `lease_owner = "mcp:{client_name}:{token前8位}"`，`client_name` 是调用方自报文本（`ClaimTaskArgs`），注释明言"不是凭证"。它与 V-05-1 将签发的 Client 凭据主体之间**没有任何绑定**；Runner 若要证明"我就是 `lease_owner`"，只能靠自己还持有那枚一次性 token 的前缀——与 P0-2 的崩溃场景正相冲突；任何调用方都可自报同一 `client_name`。"命令路由目标 == 调用方身份"在现行取值下**无可比对的身份**。
- MCP 领取的 `execution_mode` 恒为 `"human"`（T-059）。Runner 经 `pm_claim_task` 领取后与 Model A 真人 MCP 客户端**在库内完全同形**。后果：`pm_report_interaction`（权限码 `PM_MCP_AGENT_REPORT`）可被任何持该码的 Model A 客户端在其 Run 上调用 → 平台在同一事务内创建 terminate 命令 → **无任何执行端能领取执行** → 情形 6 永久（Run 存在未回执 terminate → 一律禁止启动）；反向，基线 §1.2"Model A 不创建 terminate/Continuation"与 ACP-09/ACP-425 失去判别依据。
- ACP-435（越轨拒绝）/ACP-436（撤销即通道关闭）在没有"凭据 ↔ Run 持有者"链路时**不可测试**。

**修订方向**（在 R-ACP-3 框架内，"独立身份"裁决不变，补齐链路）：

1. Runner 领取面：在 V-05-1 Client Registry 凭据下领取时，`lease_owner`（或新增 `lease_client_id` 列）由**经鉴权的 Client 主体**写入，不再由自报 `client_name` 拼接；`execution_mode` 增第三值 **`client_spawn`**（T-059 字典扩展，需在 ADR-0002 L5 处补注），`"human"` 仅保留给真人 MCP 客户端。
2. `pm_report_interaction` 准入：仅 `execution_mode ∈ {spawn, client_spawn}` 的 Run 可用；`human` → 以明确错误码拒绝（Model A 继续走既有 `awaiting_decision`/`task_waiting` 入口）。这就是 ACP-09 边界的操作化。
3. §3.3 ② 绑定改为 `command.worker_id == 调用方经鉴权的 Client 主体`（Worker 形态 == 进程内 worker_id）。
4. **依赖声明升级**：V-05-1（Client Registry + 凭据签发）从"权限码字典随迁移批落地"升级为 **ACP-02 命令面进代码的硬前置**；在此之前只能实施服务器 Worker 形态（进程内直连，无 MCP 命令面）。

新增验收：**ACP-448** `execution_mode='human'` 的 Run 调用 `pm_report_interaction` → 拒绝且不产生命令；**ACP-449** 持合法 Client 凭据但非该 Run 持有者调用 `pm_poll_commands` → 空集且审计。

### P0-4 Task 离开 `awaiting_decision` 挂在 CONSUMED，而 CONSUMED 被定义为可能永不到达

**问题**。§5.3 规定 continue acked（INPUT_INJECTED）时"不在此事务内自动离开 awaiting_decision；离开 = 链关闭（consumed）或人工处置"，§5.2 规则 3 规定链开放期间不允许新建 Interaction；§1.5 同时把"RESPONDED 未消费 + Continuation RUNNING"定为**合法稳态**（N-6，DSH 明确可能永不提供证据）。组合后：

- 新 Session 已在原 Run 上运行，Task 仍为 `awaiting_decision`。`awaiting_decision` 的合法出边只有 Cancelled/Ready/Running（C6 补充），**无 → Submitted**：新 Session 完成后 `pm_submit_result` 的状态迁移将被守卫拒绝——DSH 形态下整条 Continuation 永远无法交付；Codex/ZCode 形态下也存在"注入后、回执前"的窗口。
- 链保持开放 → 新 Session 若再次需要人工介入，`pm_report_interaction` 被 Conflict 拒绝 → ACP-416（同 Task 多次介入）对 DSH 永久不可达。

**修订方向**（在 R-ACP-4/R-ACP-5 框架内）：**把"执行恢复"与"消费证据"解耦**——

- continue acked（INPUT_INJECTED）事务内：Task `awaiting_decision → running`（经 task_transition，resolve `task_waiting`，`resolution_note` 引用 `interaction_id + delivery_id`，`ended_by=system`）；`chain_open → false`（链因"新 Session 已携带回复启动"而关闭）；guard → `none`（已有）。
- `consumed` 仍是 Interaction 级事实（`consumed_at`、Delivery CONSUMED、ACP-414/415 不变），不再承担 Task 状态或链开放的职责；N-6 稳态只影响 Interaction/Delivery 字段与告警指标，不影响执行。
- 同步修订 §5.2 定义（链开放 ⟺ 最新 Interaction ∈ {pending, responded 且无 acked continue, timed_out 未处置}）、§5.3 表、§4.1 情形 4。

新增验收：**ACP-450** DSH Adapter（无消费证据）下 continue acked 后新 Session 提交交付 → 接受；Interaction 停留 `responded`、`interaction_unconsumed_total` 计数；**ACP-451** 新 Session 再次上报介入 → 新链创建成功（旧链已关闭）。

---

## 4. P1 意见（进代码前必须闭合，有界修订）

### P1-1 迁移号 R-ACP-6 已失效

0045 已被 `0045_s02_i3_source_traceability.sql` 占用（DEF-01 修复 e87efd5，台账已登记"已应用"）。按台账纪律"后实施者无条件顺移"：ACP 批次改为 **0046（task_interaction + worker_command）/ 0047（task_run.scheduling_shield 列）**，落地前再核 `ls migrations/`。裁决记录 R-ACP-6 的备注"0044 撞号在裁，不影响 0045+"须改写为事实。

### P1-2 消费证据定义与基线 §8.6 字面冲突（不重开 R-ACP-5，精化定义）

基线 §8.6 明列"OS Process 启动"与"Adapter 将输入写入进程标准输入"**各自**不能证明消费；切片 §1.5 的 `initial_input_injected`（"初始输入写入成功"）+ `process_alive` 字面上恰是这两项的并集。R-ACP-5 已裁双证据门槛，本复审不重开，但 V1.2 须**按注入方式定义"injected"为"已被接收"而非"已写入"**：

| 注入方式 | `initial_input_injected` 的证据 | 说明 |
|---|---|---|
| 启动参数/提示文件（Codex `exec "<prompt>"`、ZCode 同类） | 回复内容是进程**启动参数**的一部分（记录 spawn 参数摘要）；进程启动成功 ⇒ 必然接收 | 此时 `process_alive` 有意义 |
| stdin 流式注入 | 注入后观测到 Agent **首个输出轮次**（首条 stdout/JSONL 事件）| 仅"写入成功"不算 |
| 无法满足 | 不伪造；N-6 稳态 + 指标 | 不变 |

同步落入 §7.1 ACP-08 能力表的每 Adapter 行。

### P1-3 Process 记录落点缺失（基线 §12.6）

§6 第 6 条"扩展既有进程清理账本"不可成立（§2 表 §6-6 行）。V1.2 二选一并写明：
- a）在本切片内声明最小 `task_run_process` 记录（按 §12.6 字段：process_id/run_id/session_id/continuation_id/pid·pgid·job 标识/started_at/terminate_requested_at/exited_at/exit_outcome/confirmation_status），ACP-05 后续归并；或
- b）V1 显式收窄：terminate ack 的 Process 事实只落 `worker_command.receipt` + 审计 + `executor-events.jsonl`，删除"Process 状态更新"一句；情形 6 / §9.4 检查 8 以命令 ack 状态为唯一判据（切片已如此判定，保持自洽即可）。

另注：Runner 形态下服务器侧 `cleanup_finished_processes` / 清理账本触及不到用户 PC 进程——Runner 本地进程账本列入 WP-3 Runner 设计项，本稿只需指针。

### P1-4 Interaction 的 `session_id` / `continuation_generation`（§12.1）与 terminate 报文的 Session/Process 绑定（§5.2）来源未定义

`pm_report_interaction` 签名不含二者；代次 1 没有 Session/Process 记录（`executor_session` 是平台侧 opaque 串，§3.4）。V1.2 写明：平台侧派生 `session_id := task_run.executor_session`（作为 Platform Session ID 可接受）、`continuation_generation := 当前代次（ACP-05 落地前恒 1）`；terminate 命令 payload 的 `session_id` 取同值、`process_id` V1 允许为空，由执行端按 Run（pid 文件）定位进程——并把"§5.2 要求绑定 Process"的这一放宽登记为实施定案。

### P1-5 S1 范围与等待窗口内的心跳/进展语义

（S1 范围已并入 P0-1。）补充两点须写明：
- 等待窗口内 Runner 继续 `pm_report_progress` 续租会刷新 `last_agent_heartbeat_at`（`run_context.rs:531-535`）——此时并无 Agent。可接受（S1 屏蔽），但 V1.2 应声明"等待窗口内的续租是**执行端存活**信号，不是 Agent 心跳"，并在 guard → none 时重置 ②③ 的判定起点（否则新 Session 启动后立即按陈旧阈值误判的风险取决于 Runner 续租频率）。
- continue 后新 Session 的首次 `pm_report_progress(stage='started')` 会尝试 Task → running（`run_context.rs:681`）——若 P0-4 按建议在 acked 事务内已转 running，此处应为幂等（同态跳过），需与 `report_progress_as` 现有"首次 started"判定核对。

### P1-6 基线 §10.4 / §16.3 偏离未按 R-ACP-7 方式登记

切片 §3.3 把 `lease_token / lease_generation` 从命令面工具签名移除、命令面不依赖有效 Run Lease——设计理由成立（D-02 + C4 的自断问题）且在 R-ACP-3 裁决之内；但冻结基线 §10.4 明文要求领取命令时校验 Lease Token、"过期 Worker 不能执行命令"，§16.3 要求 Worker 受 Run Lease 约束。V1.2 须在 §9 裁决记录中增加一行（建议 R-ACP-3 补注或新 R-ACP-11），把它登记为**对基线 §10.4/§16.3 的正式操作化修订**（等价 fencing 由 worker_id 绑定 + generation 比对提供；执行面写操作仍受七点约束），与 R-ACP-7 同一处理规格。冻结基线的偏离清单不完整，基线冻结的意义就打折。

---

## 5. P2 意见（应修，不阻塞）

| # | 意见 | 建议 |
|---|---|---|
| P2-1 | §1.2 固定文件名 `interaction.json` 与"并发新事件按写入顺序逐个上报"矛盾：第二次写入在执行端侦测前覆盖第一次，事件丢失 | 正式文件名改为 `interaction.{event_id}.json`（Agent 写唯一名、执行端扫描 glob），与已采用的 event_id 键控归档一致 |
| P2-2 | `pending → timed_out` 由谁判定（§8.2）未指明：服务端扫描器 / 读取时惰性判定 / 执行端 | 指定为平台侧定时扫描（pm-api 或 worker 进程内，间隔配置化）+ REST/UI 读取时二次判定；ACP-439 依赖此点 |
| P2-3 | `task_waiting.questions` 现为 JSONB 数组（默认 `'[]'`），§5.3 写成对象 `{question,options,interaction_id}` | 写为单元素数组 `[{...}]`，与 0010 形状与既有读取方一致 |
| P2-4 | 本切片事务内发出哪些 §17.1 逻辑事件（`interaction.created/timed_out/cancelled/consumed`）未声明；WP-5 待办/前端依赖 | 列出本切片落 Outbox 的事件子集或显式推迟到 ACP-03/04 |
| P2-5 | ACP-445（Task 取消与上报并发）的互斥机制未点名 | 指明复用 Task 行锁 / `project_gate::lock_project_activity_tx` 先例 |
| P2-6 | §0 把 Runner 形态标为"持 Client 级凭据（V-05-1 签发）"，但 §1.3/§1.5 两执行面工具沿用 Run Lease 身份——Runner 同时持两种身份（Client 凭据过 `/mcp` 鉴权 + lease 三要素过工具校验）这一"双持"应在 §3.3 表头写明，避免实施时把 Client 凭据当作执行面凭据 | 表头补"执行面工具：`/mcp` 鉴权身份（Client 凭据或真人）+ 租约三要素，二者缺一不可" |

---

## 6. R-ACP-1~9 裁决一致性核验（不重开）

| 裁决 | 稿内落位 | 核验 | 本复审意见触及方式 |
|---|---|---|---|
| R-ACP-1 标记文件 + MCP 上报 | §1.2/§1.3 | ✅ 一致 | P2-1 仅文件名（裁决不变） |
| R-ACP-2 `scheduling_shield` + 六情形 | §4.1/§4.2 | ✅ 载体与状态集一致 | P0-1 修屏蔽点对象/范围与情形 5 的事实描述；P0-4 修情形 4 的触发点——状态集与载体不变 |
| R-ACP-3 worker_id=lease_owner + 独立身份 | §3.1/§3.3 | ⚠ 裁决方向成立，**链路在现行代码下不可成** | P0-3 补齐"经鉴权 Client 主体 → lease_owner/execution_mode"链路；P0-2 续领路径；P1-6 登记基线偏离 |
| R-ACP-4 同事务双写 | §5.1/§5.3 | ✅ 一致 | P0-4 把"离开 awaiting_decision"移到 acked 事务——仍是同应用服务事务双写 |
| R-ACP-5 双证据 + N-6 稳态 + 三阶段 | §1.4/§1.5 | ✅ 一致 | P1-2 精化"injected"定义；P0-4 解耦消费与执行恢复（证据门槛不变） |
| R-ACP-6 迁移号 0045~0046 | §3.1/§10 | ❌ 事实过期 | P1-1 顺移 0046~0047 |
| R-ACP-7 仅冻结启动 | §6-5/§9 | ✅ 一致且登记规范 | 作为 P1-6 的登记范式 |
| R-ACP-8 权限码命名 | §1.6 | ✅ 一致 | — |
| R-ACP-9 `task_run` 列先行 | §4.1 | ✅ 一致 | — |

---

## 7. 复审结论与 V1.2 修订清单

**结论：不通过。** 修订为 V1.2 后复审；V1.2 须满足下列各项，逐项可核：

- [ ] **R-1（P0-1）** C3 改正；S1 = 整条迭代跳过（含 ⑤）；S2/S3 拦截对象 = `recover_lost_run` 全部调用方 + spawn 前校验；情形 5 按"新 attempt Run"事实重写并新增裁决项 **R-ACP-10**（取消后是否接受新 Run 重试 + 旧 Run `release_reason` 新值）；ACP-428 文本修正。
- [ ] **R-2（P0-2）** 选定同主体续领方案（推荐 A）；修订 §3.3 ③ generation 比对规则（`issued_generation` / `reacquired_generation`）；新增 ACP-446/447；明确 Runner 本地租约凭据是否持久化（B 作为补充）。
- [ ] **R-3（P0-3）** `lease_owner`/`lease_client_id` 由经鉴权 Client 主体写入；`execution_mode` 增 `client_spawn`（ADR-0002 L5 补注）；`pm_report_interaction` 按 `execution_mode` 准入；§3.3 ② 绑定对象改为 Client 主体；V-05-1 升级为 ACP-02 命令面硬前置；新增 ACP-448/449。
- [ ] **R-4（P0-4）** INPUT_INJECTED 事务内 Task → running + resolve task_waiting + `chain_open=false`；`consumed` 降为 Interaction 级事实；修订 §4.1 情形 4、§5.2 定义、§5.3 表；新增 ACP-450/451。
- [ ] **R-5（P1-1）** 迁移号 0046~0047，R-ACP-6 备注改写。
- [ ] **R-6（P1-2）** 按注入方式定义 `initial_input_injected`，落 §7.1 每 Adapter 行。
- [ ] **R-7（P1-3）** Process 记录落点二选一写明；Runner 本地进程账本指针。
- [ ] **R-8（P1-4）** `session_id` / `continuation_generation` / terminate `process_id` 来源与 V1 放宽登记。
- [ ] **R-9（P1-5）** 等待窗口续租语义声明 + guard→none 时 ②③ 起点重置 + `report_progress(started)` 幂等核对。
- [ ] **R-10（P1-6）** §10.4/§16.3 偏离登记为正式操作化修订（R-ACP-3 补注或 R-ACP-11）。
- [ ] **P2-1~6** 随 V1.2 收编（不阻塞复审通过）。
- [ ] 代码基线更新为 e87efd5 或更新；§2 事实表按本稿 §2 修订。

复审通过后方可按 §10 工程纪律进代码；在 V-05-1 落地前，可先实施**服务器 Worker 形态**（进程内直连、无 MCP 命令面）的 ACP-01/02 闭环以验证 N-1 屏蔽与状态机，不受 P0-3 阻塞——但 P0-1/P0-2（Worker 形态同样适用：Worker 重启后租约过期走同一路径）/P0-4 仍须先闭合。

---

**文档结束**
