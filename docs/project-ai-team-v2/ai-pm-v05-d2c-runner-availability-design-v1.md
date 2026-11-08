# D-2=c 派发可用性真源统一 · 裁决/设计包 V1.0

> 归档 2026-11-08。本包是裁决记录
> `ai-pm-v05-dispatch-competition-adjudication-record-v1.md` §四 D-2=c 的正式设计输入，
> 并入 roadmap §9「调度入口」设计闭合（AI Team V2 硬前置）。
> 仓库 pin：master@a1bd7f4（含 D-1=b 已落地）。

## 一、定位与范围

D-2=c = **Runner 心跳注册为派发可用性来源（可用性真源统一）**。本包只做三件事：
代码事实核验（file:line 钉死）→ 缺口判定 → 方案与待裁。**不落代码**，产出供
roadmap §9 调度入口闭合与后续实施批次消费。

范围边界（承接裁决记录 §四）：本包不动 A-1、不动 MCP 四工具契约、不动
`uq_task_run_active`、不碰 worker spawn 通道（CodexExecutor）。

## 二、代码事实核验

### 2.1 领取门现状：静态 dev_agent 判定，无「在线」概念

`crates/pm-application/src/run_dispatch/policy.rs:136-231` `claimability_assessment`
七道门，顺序即拒绝优先级：

| 门 | 位置 | 判定 |
|---|---|---|
| retry_exhausted | :159-166 | task.retry_exhausted |
| policy_requires_approval | :163-166 | 审批证据 |
| task_waiting | :168-176 | 有未结 task_waiting |
| **no_executor_assignee** | **:179-190** | `SELECT (p.kind='dev_agent') FROM delivery.task t JOIN core.principal p ON p.id=t.assignee_id` |
| no_repository_scope | :192-201 | 无可写仓 |
| repository_not_verified | :203-215 | 仓非 verified |
| baseline_unavailable | :217-229 | 基线不可解析 |

「执行端可用性」的**唯一**表达就是 `no_executor_assignee`：只判 assignee 是不是
`dev_agent` principal——**没有心跳/在线/新鲜度任何维度**。R4 演练期的 `no_executor_assignee`
×11 退避即此门（当时 task 未绑 dev_agent；绑定后 Runner 04:14 领取成功）。

### 2.2 run 级信号四件套（task_run 列）

| 列 | 迁移 | 语义（注释钉死） |
|---|---|---|
| `lease_owner` | 0011:33 | 持有者快照：`worker:{id}` / `mcp:{name}:{token8}` / `client:{device_id}` |
| `heartbeat_at` | 0011:35 | **混合信号**：claim / worker 兜底续租 / agent 心跳 都刷新 |
| `last_agent_heartbeat_at` | 0023:10 | **仅 MCP 通道（renew/progress）刷新**；与进程存活/进展/截止分立；「心跳不能自动视为执行进展」 |
| `last_progress_at` | 0023:11 | stage 变化才刷新 |
| `lease_generation` / `deadline_at` | 0023:8-9 | 执行权世代 / 绝对截止（T2/T3） |
| `execution_mode` | 0028 | `human`/`spawn`（Worker→spawn、Mcp→human、`runner:` 前缀 Mcp→spawn、Client→spawn） |
| `lease_client_id` | 0049:29 | Client 通道稳定身份（V-05-1） |

### 2.3 心跳写入方分叉（关键）

| 通道 | 函数 | 写 last_agent_heartbeat_at? |
|---|---|---|
| Agent（MCP，token 凭证） | `run_context.rs:514` `renew_lease_by_token` / `:588` `report_progress` | **是**（`run_context.rs:541/653`） |
| worker 兜底续租（lease_owner 凭证） | `lease.rs:197` `renew_lease` | **否**（`lease.rs:195-196` 注释明说「不写 last_agent_heartbeat_at」） |
| claim | `claim.rs:875` | 只写 `heartbeat_at=now()` |

→ `last_agent_heartbeat_at` 是**唯一**「执行端 Agent 活着」的干净信号；`heartbeat_at`
被 worker 续租污染，不能当可用性真源。

### 2.4 设备 readiness（领取链零消费）

`crates/pm-application/src/devenv/checks.rs`：`report_checks`(:36) 写
`devenv.device.readiness`+`last_check`；`compute_readiness`(:144-169) 服务端算
installed/connected/authenticated/degraded/ready，**不采信客户端 "ready" 声明**。
注释 :5-10 钉死「Ready 只描述设备自身——项目权限不纳入 Device 的 Ready」。
`device_project_binding`（0027:57-82）是**声明+事实记录，非权限**。

**结论**：`readiness` 是设备自检的静态结论，与「此刻是否在线可取活」无必然关系，且
**领取链（run_dispatch）从不读它**。

### 2.5 Runner 回路（apps/ai-pm-runner/src/runner_loop.rs）

- 自领：`list_ready_tasks` → `claim_task`（Model A，`pm_claim_task`）(:386-396)。
- 心跳：`tick` → `report_progress`（"runner heartbeat"）(:309/:458/:566)。
- 只写 **run 级**心跳/租约，**不写 `devenv.device.readiness`**——设备自检上报是
  另一条独立通道（`POST /v1/pm/devices/{id}/check`）。

### 2.6 领取形态识别现状（claim 后回写，claim 前不可知）

`claim.rs:838-849` 的 `execution_mode` 由 `LeaseOwner` 派生：
`Worker→spawn`、`Mcp→human`、`Mcp{runner:}→spawn`、`Client→spawn`。
`devenv.device.executor_kind`（0027:30，缺省 `codex`）是**执行器类型**，不是
「领取型 Runner vs worker-spawn」的项目级标识。

## 三、缺口判定

| # | 缺口 | 后果 |
|---|---|---|
| G-1 | dispatcher 无「在线」概念：`no_executor_assignee` 静态判 dev_agent | 领取型 Runner 离线时 worker 仍会 claim 并退避（D-1=b 用排除列表负向隔离，是打补丁） |
| G-2 | 心跳是 run 级、**存活期才有**；无 run 时（领取型项目等待新任务）无任何可用性信号 | 无法在「派发决策点」判断 Runner 是否可接新活 |
| G-3 | readiness（设备自检）与 run 心跳两套信号互不消费，真源分裂 | 三套信号各自为政，无法收敛成单一「可用」判定 |
| G-4 | 「领取型 Runner 的项目」无可查询稳定标识：binding 是声明、execution_mode 是 claim 后回写、device.executor_kind 是执行器类型 | 无法 SQL 圈定「哪些项目该走 Runner 心跳判定」 |

## 四、方案：可用性真源统一

### 4.1 目标态

```
「领取型项目」= 存在活跃 device_project_binding 且该 device 由 Runner 载体服务
「Runner 可用」= 设备级心跳新鲜（可查询、claim 前可判、跨 run 持久）
dispatcher claimability 增门：领取型项目 → runner_available（新鲜才领，否则静默退避）
```

### 4.2 三个候选（推荐 A）

| 方案 | 可用性信号 | 判定 |
|---|---|---|
| **A（推荐）新增设备级心跳** | 新增 `devenv.device.last_heartbeat_at`（或等价 device 级心跳表），Runner 心跳时顺带写 | 覆盖 G-2/G-3；claim 前可判；跨 run 持久 |
| B 复用 run 级 `last_agent_heartbeat_at` 聚合 | `max(last_agent_heartbeat_at)` per device | 只覆盖「有活跃 run」窗口，无 run 时不可判（G-2 依旧） |
| C 复用 `readiness` | 自检静态结论 | 语义不符（自检≠在线），且 claim 前需二次推导，弃 |

**推荐 A**，理由：只有设备级心跳能表达「Runner 此刻在线可取活」，且与 run 级信号
正交（run 级 `last_agent_heartbeat_at` 继续服务 T4 死锁检测，互不取代）。

### 4.3 判定接入点（最小改动面）

- 领取型项目识别：优先复用 `device_project_binding` + 新增 device 级「载体类型」列
  （如 `devenv.device.runner` bool，或 `executor_kind` 扩展 `runner` 值）——见 Q-D2C-1。
- claimability 增门（`policy.rs` 新增分支，位于 `no_executor_assignee` 附近）：
  仅对「领取型项目」判定 `runner_available`；非领取型项目（worker spawn 通道）不受影响。
- 心跳写入：复用 `pm_report_progress` 在空闲期发设备级心跳，或新增轻量
  `pm_runner_heartbeat`（见 Q-D2C-5），**不动现有四工具契约**。

### 4.4 与 D-1=b 的关系

D-1=b 的 `AI_PM_RUN_DISPATCH_EXCLUDE_PROJECTS` 是**负向静态白名单**；D-2=c 是
**正向动态可用性**。两者互补不互斥；D-2=c 落地后，排除列表可退化为「过渡开关」，
最终由心跳真源统一取代（是否删除见 Q-D2C-6）。

## 五、待裁（给用户）

| 编号 | 问题 | 候选 |
|---|---|---|
| Q-D2C-1 | 领取型项目如何标识 | (a) binding + 新增 `device.runner` 列；(b) `executor_kind` 扩展 `runner` 值；(c) 复用 `lease_owner` 前缀历史反推（不推荐，claim 后才有） |
| Q-D2C-2 | 可用性信号载体 | (a) 新增设备级 `last_heartbeat_at`（推荐）；(b) run 级聚合；(c) 复用 readiness（弃） |
| Q-D2C-3 | 心跳新鲜阈值 | 复用 `AI_PM_RUN_AGENT_HEARTBEAT_STALE_SECS`(900s) 还是独立 `AI_PM_RUNNER_AVAILABILITY_STALE_SECS` |
| Q-D2C-4 | Runner 离线时 dispatcher 行为 | (a) 静默退避（与 D-1=b 同质，零审计噪声）；(b) `release_reason=runner_unavailable` 留痕；(c) 降级给 worker spawn（违反 D-2=c 初衷，弃） |
| Q-D2C-5 | 心跳写入通道 | (a) 复用 `pm_report_progress`（空闲期发，不动契约）；(b) 新增 `pm_runner_heartbeat` 工具 |
| Q-D2C-6 | D-1=b 排除列表去留 | (a) D-2=c 落地即删；(b) 保留为强制兜底（belt-and-suspenders） |

## 六、实施切片建议（本包不执行，供后续批次）

1. **S1 信号落库**：`devenv.device` 增设备级心跳列（迁移）+ 领取型项目标识（Q-D2C-1）。
2. **S2 Runner 写心跳**：Runner 空闲期发设备级心跳（Q-D2C-5）。
3. **S3 领取门接入**：claimability 增 `runner_available` 门（Q-D2C-2/3/4）。
4. **S4 收口**：D-1=b 排除列表去留（Q-D2C-6）+ 调度入口闭合总账回填 roadmap §9。

## 七、残余风险

1. 领取型项目标识（G-4/Q-D2C-1）是**数据建模决策**，需与 V-05-1 Client Registry
   （device 身份）对齐，避免再造第二套 device 分类。
2. 设备级心跳引入「device 与 run 两级心跳并存」，需明确二者职责边界（device=可用性、
   run=执行健康），防止未来再次分裂。
3. MCP 四工具契约冻结：若 Q-D2C-5 选 (b) 新增工具，须走契约评审而非本包擅自增面。
