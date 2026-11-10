# S05 后台批 1 · 执行监控最小事件面 实施报告 V1.0

| 项目 | 内容 |
|---|---|
| 文档性质 | 实施验收记录（对照 roadmap §9 最小事件方案） |
| 日期 | 2026-11-10 |
| 分支 | `feat/s05-execution-backend`（worktree ai-pm-s05，基线 master @ 89c3db63） |
| 迁移 | `0013_task_run_event.sql`（台账已登记；0012 为 master 在途占用） |
| 现状对照 | `docs/s05/S05-00-code-facts-and-gap-analysis-v1.md`（缺口 S05-05） |

---

## 一、范围与立场

**做**：Run 生命周期与 Worker 命令的库内最小事件账 + 查询面（roadmap §9：「先做最小方案
event_id/tenant/project/task/run/client/event_type/generation/occurred_at/received_at，
幂等写入+审计查询——不做大型事件溯源」）。

**不做**（边界）：
- 不做心跳级事件（`last_agent_heartbeat_at` 列已承载，T4 四信号分立）；
- interaction / continuation / response_delivery 不重复落账（已有专表，监控面联表）；
- review Run（D5 平台内确定性检查，无外部 Agent/无租约世代）不落账，状态直读 run 行；
- `scheduling_shield` 四值、probe release、`integration_status` 不落账（run 行/专表已承载）；
- 事件面是**纯观测旁路**：不参与任何状态机守卫/调度判定（禁重建区红线 5）。

## 二、数据契约（0013）

`delivery.task_run_event`（追加式；无数据库级 FK，维度快照自 run 行）：

- 列：`id / tenant_id / project_id / task_id / run_id / client_id（lease_client_id 快照）/
  event_type / lease_generation / dedupe_tag / occurred_at / received_at / detail / created_at`。
- 幂等键：`UNIQUE (run_id, event_type, lease_generation, dedupe_tag)`，写入
  `ON CONFLICT DO NOTHING`。tag 语义：`run_stage_changed`=stage 名（含 `requeued` 回归
  标记）；`command_*=命令 id`；其余空串（同 generation 天然至多一次）。
- `event_type` CHECK 白名单（8 值）：`run_claimed / run_started / run_stage_changed /
  run_finished / command_issued / command_delivered / command_acked / command_failed`。
- 守卫：BEFORE UPDATE trigger 拒改写（追加式）；DELETE 保留给保留期清理。
- 索引：`(tenant_id, project_id, run_id, occurred_at DESC)` 供项目内时间线。

## 三、写入面（`pm-application/src/run_events.rs` 唯一入口）

`record_run_event(conn, run_id, RunEvent)`：tenant/project/task/client 一律
`INSERT..SELECT` 快照自 `delivery.task_run`（不信任调用点传参）；`generation=None`
时快照行值。与主操作**同事务**（失败整体回滚，不降级吞错）。

| 打点位置 | 事件 | tag |
|---|---|---|
| `run_dispatch/claim.rs`（claim_core 领取） | `run_claimed` | `""`（generation 随领取递增，重领成新行） |
| `recovery.rs`（ACP 续领） | `run_claimed` | `""`（generation+1；detail 带 `reacquired:true`） |
| `run_context.rs::report_progress_impl`（MCP 首报 started，claimed→running） | `run_started` | `""` |
| `run_context.rs`（同函数，stage 变化且非首报） | `run_stage_changed` | stage 名 |
| `run_executor.rs`（spawn 启动 running） | `run_started` | `""` |
| `run_executor.rs::start_failed`（退避回队） | `run_stage_changed` | `requeued` |
| `run_executor.rs::start_failed`（基础设施耗尽终态） | `run_finished` | `""`（detail: outcome=failed, reason=infrastructure_exhausted） |
| `run_executor.rs::record_termination`（人为/监管终止） | `run_finished` | `""`（outcome + reason） |
| `run_executor.rs::finish_run`（进程自然退出） | `run_finished` | `""`（outcome + exit） |
| `run_dispatch/lease.rs::isolate_started_run`（失联隔离） | `run_finished` | `""`（timed_out / lease_expired） |
| `run_dispatch/lease.rs::cancel_run` | `run_finished` | `""`（cancelled + reason） |
| `archive.rs`（归档批量取消 queued Run） | `run_finished` | `""`（逐 Run，同事务） |
| `worker_command.rs::issue_command` | `command_issued` | 命令 id |
| `worker_command.rs::poll_commands`（领取交付） | `command_delivered` | 命令 id（同命令重投不重复） |
| `worker_command.rs` ack（acked / failed） | `command_acked` / `command_failed` | 命令 id |

MCP 四工具契约、任务 13 态、Run 7 态语义零改动——打点全部发生在既有事务内。

## 四、查询面（`apps/pm-api/src/pm_runs.rs`，薄适配）

- `GET /v1/pm/task-runs/{run_id}/events?type=`：run 事件时间线（occurred_at 降序，
  分页复用 `support::paging`）；`type` 只认白名单值，非法视为未传；404 口径与
  `task_run_detail` 一致（不存在/跨租户/非成员）。
- `GET /v1/pm/task-runs/summary?project_id=`：`total / by_status / by_outcome /
  active（queued+claimed+running）/ events_24h / last_event_at`；project_id 必填，
  `require_project_visible` 守卫。

## 五、验收

| 用例组 | 内容 | 结果 |
|---|---|---|
| `pm-application/tests/run_events.rs` | 幂等（同键重插=0 行/新 generation 成新行）；追加式守卫拒 UPDATE；维度快照一致；claim→`run_claimed`；首报→`run_started`；阶段推进/同阶段重报/换阶段 tag 序列；`cancel_run`→`run_finished`（outcome/reason 落 detail） | **3/3 绿** |
| `apps/pm-api/tests/run_events.rs` | events 排序/过滤/分页/非成员 404/未知 run 404；summary 聚合/project_id 必填/非成员 404 | **2/2 绿** |
| 全 workspace 回归 | `cargo test --workspace --no-fail-fast`（DATABASE_URL=127.0.0.1:16433） | **173 个测试二进制 ok / 1434 用例通过**；唯一失败=存量 K29（见 §六） |
| 打点触达路径定向回归 | recovery / run_dispatch / run_executor / acp_comm_face / response_delivery / continuation / command_batch / outbox_events（application 86 用例）+ task_runs / run_claim_lifecycle / run_scope_snapshot / run_executor_handshake / project_archive（api 23 用例） | **全绿** |
| `cargo fmt` | 本批文件已格式化（master 既有 24 文件 rustfmt 漂移未触碰——与本批无关，不在本批修复） | ✅ |
| `cargo clippy` | 本批新增告警 **0**（打点调用点 15 处 auto-deref 已按 clippy 建议修正）；余 6 处告警均为 master 存量（design.rs×2 / kb_service×2 / task_plan.rs / kb_memory_api.rs） | ✅ |

## 六、全量回归结果

- `cargo test --workspace --no-fail-fast`：**173 个测试二进制 ok，1434 用例通过**；唯一失败
  `knowledge_index_v2::k29_two_pools_serialize_the_same_project_and_parallelize_across_projects`。
- **K29 为存量失败，与本批无关**：在 master 基线（主 worktree @ 89c3db63，无本批改动）单独
  复跑同样失败（`create_doc` → `Err(NotFound)`，知识索引双池并行用例，涉及 5433 第二实例
  容器）。本批改动不触碰 kb/索引路径。建议另行登记排查。

## 七、联动更新

- 台账 `docs/ai-pm-migration-ledger.md` §〇·二 增 0013 行。
- 后续批次入口：`docs/s05/S05-00-code-facts-and-gap-analysis-v1.md` §四（B2=ACP-09 回归
  + Claude/Trae profile；B3=执行侧知识注入；B4=Q-D2C 裁决 → 调度策略层）。

---

**文档结束**
