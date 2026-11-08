# D-2=c 实施基线 V1.0（领取型 Runner 可用性真源）

> 归档 2026-11-08。承接 `ai-pm-v05-d2c-runner-availability-design-v1.md` §八
> （六项裁决全采纳 + Q-D2C-5 修正）。本基线是 D-2=c 的代码实施契约。
> 仓库 pin：master@7c3361d 之后。

## 一、范围

- 迁移 0063：`devenv.device` 增 `is_runner` + `last_heartbeat_at`（+ 部分索引）。
- 注册面：设备登记可声明 `is_runner`（REST body → `RegisterDeviceRequest`）。
- 心跳真源：已认证设备活动（六工具）服务端归因刷新 `last_heartbeat_at`。
- worker 领取过滤：claim 候选发现排除「领取型项目」（active binding → `is_runner=true`）。
- **不做**：新鲜度不门控 worker 领取（语义定版见下）；D-1=b env 排除列表保留不动；
  Runner 二进制零改动。

## 二、迁移 0063（`migrations/0063_runner_availability.sql`）

```sql
ALTER TABLE devenv.device
    ADD COLUMN is_runner         BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN last_heartbeat_at TIMESTAMPTZ;
COMMENT ON COLUMN devenv.device.is_runner IS
    '领取型 Runner 载体（D-2=c Q-D2C-1）：其绑定项目的 delivery run 由 Runner 显式领取
     （Model A）；worker 无人值守派发（Model B）在候选发现阶段静默排除。';
COMMENT ON COLUMN devenv.device.last_heartbeat_at IS
    '设备级心跳（可用性真源，Q-D2C-2）：已认证设备活动服务端归因刷新（领取/列表/上报/
     续领/发现/命令轮询）；本期只落库与定义语义，不门控 worker 领取（基线 §三）。';
CREATE INDEX idx_device_runner_heartbeat
    ON devenv.device(last_heartbeat_at) WHERE is_runner;
```

## 三、语义定版

1. **worker 对领取型项目恒不领**：判定用静态事实（`is_runner=true` + active binding），
   与心跳新鲜度**无关**——避免「Runner 离线 → worker 抢活」与 Runner A3 离线恢复
   （`pm_reacquire_lease`）的竞态。
2. **心跳新鲜度只落库**：`last_heartbeat_at` + `AI_PM_RUNNER_AVAILABILITY_STALE_SECS`
   （缺省 300s；Runner 空闲期每 10s 轮询 pm_list_tasks，300s ≈ 30 次漏拍）定义
   「可用」语义，供 V2 调度入口闭合与可观测面消费；「离线兜底接管」策略是 V2 裁决
   范围，本批不越界。
3. **Model A 永不受限**（A-1）：`claim_task_run` 不经阶段 A 过滤。
4. **回收分支同排**：领取型项目的租约过期回收（B3）同样由 worker 排除——Runner 自领
   自续（A3 路径）负责其 run 恢复。
5. **D-1=b 兜底**：env `AI_PM_RUN_DISPATCH_EXCLUDE_PROJECTS` 保留，与库内事实
   （is_runner）并集生效（belt-and-suspenders）。

## 四、改动面清单

| # | 文件 | 改动 |
|---|---|---|
| 1 | `migrations/0063_runner_availability.sql` | 两列 + 注释 + 部分索引 |
| 2 | `crates/pm-application/src/devenv/mod.rs` | `RegisterDeviceRequest.is_runner: Option<bool>`；INSERT 两列；`DeviceRow.is_runner: bool` + 各 fetch SELECT 补列 |
| 3 | `crates/pm-application/src/devenv/availability.rs`（新） | `refresh_runner_heartbeat(pool, device_id)`（`WHERE is_runner` 限定）；`runner_availability_stale_secs()`（env）；`is_runner_available(at, stale)` 纯函数 |
| 4 | `crates/pm-application/src/devenv/mod.rs`（mod 声明） | `pub mod availability;` |
| 5 | `crates/pm-mcp/src/tools.rs` | 六工具入口归因刷新：`claim_task` / `list_tasks` / `report_progress` / `reacquire_lease` / `discover_runs` / `poll_commands`（best-effort，失败只日志） |
| 6 | `crates/pm-application/src/run_dispatch/claim.rs` | 阶段 A 两个分支都加 runner-binding 排除（`NOT EXISTS b JOIN device d ON d.is_runner WHERE b.unbound_at IS NULL`） |
| 7 | `apps/pm-api/src/pm_devices.rs` | `RegisterBody.is_runner: Option<bool>`（serde default）→ 透传 |
| 8 | 测试 | 见 §五 |

## 五、测试计划

1. **单元**（`availability.rs`）：`is_runner_available`（新鲜/过期/None=不可用/边界）。
2. **devenv 集成**（`tests/devenv.rs`）：`is_runner=true` 登记回读；`refresh_runner_heartbeat`
   写 `last_heartbeat_at`；非 runner 设备刷新 no-op（`WHERE is_runner` 限定）。
3. **run_dispatch 集成**（`tests/run_dispatch.rs`）：
   - `runner_bound_project_run_is_not_claimed_by_worker`：is_runner 设备 + active binding
     → `claim_once` 返回 None、run 保持 queued、**零审计行**；`claim_task_run`（Model A）
     仍可领（A-1）。
   - `non_runner_binding_does_not_exclude`：is_runner=false 绑定 → 正常领取。
   - `unbound_runner_binding_does_not_exclude`：`unbound_at` 非空 → 正常领取。
4. 存量回归：`exclude_list_miss_and_empty_claim_normally` 等 D-1=b 用例在无 runner 绑定
   数据下全量不变。

## 六、.10 应用配方（待 master 干净 + 镜像重建，承接 D-1=b 验收记录 §四）

1. 迁移 0063 随镜像上栈（sqlx 启动迁移）。
2. `.10` 库：`UPDATE devenv.device SET is_runner = true WHERE name = 'e2e-runner-59';`
   （或设备登记 API 重发带 `is_runner:true`）。
3. `.env.prod`：`AI_PM_RUNNER_AVAILABILITY_STALE_SECS` 可选（缺省 300）；
   `AI_PM_RUN_DISPATCH_EXCLUDE_PROJECTS=e2e-acp09` 保留为兜底（可留空验证库内事实生效）。
4. 验证：worker 日志对 e2e-acp09 零 claim/零 skip 审计；Runner 自领全链不变；
   `devenv.device.last_heartbeat_at` 随 Runner 轮询前进。

## 七、验收口径

- 全量回归不新增红（存量白名单不变）；fmt/clippy 干净；零额外迁移号占用（0063 记账）。
- 台账：`docs/` 迁移台账登记 0063（如有台账文件）。
