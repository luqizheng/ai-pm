# 派发器 ↔ Runner 竞争缓解裁决记录 V1.0

> 归档 2026-11-08。本记录是 R4 验收记录（`docs/acceptance/checkpoint2-r4-acceptance-record-v1.md`）
> §六-1 遗留项的**独立裁决落档**：裁决包当时未在库，本批代行补档 + 事实核查 + 勘误。
> 仓库 pin：master@5b212c6。

## 一、裁决原文（用户 2026-11-08 直接裁定）

| 编号 | 议题 | 裁决 | 本批动作 |
|---|---|---|---|
| D-1 | 立即缓解（派发器 ↔ 领取型 Runner 竞争） | **b**：接线 `AI_PM_RUN_DISPATCH` 透传，并按租户/项目粒度关闭无人值守派发，使领取型 Runner 的项目不再被 worker 派发器干扰 | **实施**（零迁移目标） |
| D-2 | 正式方向 | **c**：Runner 心跳注册为派发可用性来源（可用性真源统一） | **只登记方向，不实施** |

## 二、事实核查（file:line 钉死）

### 2.1 `DispatchMode::from_env` 代码默认 Off

`crates/pm-application/src/run_dispatch/mod.rs:111-133`：

- `enum DispatchMode { Off, Probe, On }`（111-118）。
- `from_env()`（120-127）：`Some("on") → On`、`Some("probe") → Probe`、**其余（含缺省）→ Off**（125 `_ => Self::Off`）。
- `claims()`（130-132）：`!matches!(self, Off)` —— **仅 Probe/On 领取**。

结论：**代码默认 Off**，与 R4 记录「默认 On」不符。

### 2.2 `.10` 实况：env 未透传，但运行时实际在派发

- `.10` 两 worker 容器 env **均无 `AI_PM_RUN_DISPATCH`**（`infrastructure-pm-worker-1` 仅
  `DATABASE_URL`；`infrastructure-ai-pm-worker-1` 仅 `DATABASE_URL` + `AI_PM_POLICY_FILE`）。
- compose 未透传该键属实（`infrastructure/docker-compose.ai-pm.yml:103-124` 两 worker
  的 `environment` 无此键）。
- **但运行时仍持续派发**，机制不是「env 默认 On」，而是：
  - `apps/ai-pm-worker/src/main.rs:120` 读 `DispatchMode::from_env()` → 仅在 `:144` 打日志；
  - 真正的领取循环 `main.rs:146-152` 调 `claim_and_start`，而
    `crates/pm-application/src/run_executor.rs:49` 内部**硬编码 `DispatchMode::On`**：
    `claim_once(pool, worker_id, cfg, DispatchMode::On)`。

结论：**`AI_PM_RUN_DISPATCH` 在当前 worker 主循环里是「只读不打」的死开关**——
`claim_and_start` 无视 env、恒以 `On` 领取。R4 记录「默认 On」描述的是运行时效果，
但归因错误：不是 env 缺省为 On，而是 `claim_and_start` 硬编码 On + env 变量只被读取未参与门控。
（这正是 D-1=b 必须先「接线」的原因：仅 compose 透传不足以生效。）

### 2.3 演练期真实 deny 串（`.10` 库 run `6f4ec3d4` / task `74acb744`）

`delivery.task_run.release_reason` 与 `governance.audit_log` 双侧证据：

| 时段 | action | release_reason | dispatch_attempts | 对应代码 |
|---|---|---|---|---|
| 03:46–04:00 | `skip_task_run` ×11 | **`no_executor_assignee`** | 1→11 | `policy.rs:179-190` |
| 04:14 起 | `claim_task_run`（e2e-runner-59） | —（领取成功） | — | Model A |
| 05:57–06:16 | `skip_task_run` ×12 | **`task_waiting`** | 1→12 | `policy.rs:168-176` |
| 最终行 | — | **`task_not_ready`** | 36 | `claim.rs:649-656` |

- **`no_client_available` 全仓不存在**（grep 零命中）；R4 记录引用的字面量失实。
- 「③ 可用性过涝始终拒绝」的真实门 = **`no_executor_assignee`**（`policy.rs:179-190`：
  `assignee_id` 不是 `dev_agent` principal 即拒）。演练早期 task `74acb744` 无
  `dev_agent` assignee → worker 连续 11 次退避；随后绑定 e2e-runner-59（dev_agent）后
  Runner 于 04:14 领取成功。
- `denied: ` 前缀只出现在 Model A 错误 `TaskNotReady(format!("denied: {reason}"))`
  （`claim.rs:781`），**worker（Model B）路径不产该前缀**——它走 `requeue_with_backoff`
  + `audit_run(release_reason=...)`。

### 2.4 「③ 可用性」的真实代码位 + Runner 不回写 readiness 的机理

- `run_dispatch` 内**无任何 readiness 字样**（grep 仅命中 `devenv/checks.rs`、
  `devenv/mod.rs`、`command_center.rs`）。「可用性」在领取链上的真实表达就是
  `claimability_assessment`（`policy.rs:135-231`）的 §5.6 清单：`retry_exhausted` /
  `policy_requires_approval` / `task_waiting` / `no_executor_assignee` /
  `no_repository_scope` / `repository_not_verified` / `baseline_unavailable`。
- `devenv.readiness`（`devenv/checks.rs:144-168` `compute_readiness`）是**设备自检上报**
  的产物，仅由 `POST /v1/pm/devices/{id}/check` 写入，**领取链从不消费**。
- Runner 的领取/心跳/续租路径（`claim_task_run` → `claimability_assessment`、
  `renew_lease`、`report_progress`）**均不触碰 `devenv.readiness`**——它只在 run 级
  `lease_owner` / `heartbeat_at` 上留痕。因此「dispatcher 无 check-in 概念」成立，
  D-2=c 的「Runner 心跳注册为派发可用性来源」正是要补上这条真源。

## 三、R4 记录 §六-1 勘误

原文（`docs/acceptance/checkpoint2-r4-acceptance-record-v1.md:83-88`）三处失实，勘误如下：

1. **「`AI_PM_RUN_DISPATCH` 默认 On」** → 代码 `from_env()` 默认 **Off**；compose 确未
   透传。运行时持续派发的真因是 `run_executor.rs:49` 硬编码 `On` + env 只读不打。
2. **「readiness 快照无 check-in 概念」** → `run_dispatch` 无 readiness 概念，真门是
   §5.6 的 `no_executor_assignee`（dev_agent assignee 判定）。
3. **「重试后把任务打入 `denied: no_client_available`」** → 字面量不存在；真实 deny 串
   为 `no_executor_assignee`（→ `task_waiting` → `task_not_ready`），且 worker 路径无
   `denied:` 前缀。

## 四、D-2=c 方向登记

**D-2=c（正式方向，本批不实施）**：Runner 心跳注册为派发可用性来源——把 run 级
`heartbeat_at` / `lease_owner`（`client:{device}`）提升为派发器 claim 判定的可用性真源，
取代「assignee 是 dev_agent 即可派」的静态判定（`no_executor_assignee`），使领取型
Runner 的在线状态能被无人值守派发器感知。

**并入点**：`docs/project-ai-team-v2/ai-pm-v05-execution-roadmap-v1.md` §9「调度入口」
（设计闭合硬前置）。本记录为其既定输入之一（该文已加日期注记，见同批 docs commit）。

**已知限制（D-2=c 实施前）**：D-1=b 用「项目排除」做静态隔离，本质是**负向白名单**
——排除列表内的项目 worker 不领取，但无法表达「Runner 此刻在线才可派」的正向可用性。
正向语义须等 D-2=c 把心跳接入 claim 判定后统一（可用性真源唯一）。
