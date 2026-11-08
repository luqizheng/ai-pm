# D-2=c 派发可用性真源 · 实施验收记录 V1.0

> 归档 2026-11-08。承接设计包 `ai-pm-v05-d2c-runner-availability-design-v1.md`
> §八裁决 + 实施基线 `ai-pm-v05-d2c-implementation-baseline-v1.md`。
> 实施 commit：`7074644`（feat）。docs 前置：`91fe74e`。

## 一、实施形态（最终）

| 面 | 落点 |
|---|---|
| 迁移 0063 | `devenv.device` + `is_runner BOOLEAN NOT NULL DEFAULT false` + `last_heartbeat_at TIMESTAMPTZ` + 部分索引 `idx_device_runner_heartbeat WHERE is_runner`（列注释钉死语义） |
| 注册面 | `RegisterDeviceRequest.is_runner`（复用登记可更新）→ REST `RegisterBody.is_runner`（serde default）→ `DeviceRow.is_runner/last_heartbeat_at` 回读 + `device_json` 暴露 |
| 心跳真源 | `devenv/availability.rs`：`refresh_runner_heartbeat`（`WHERE is_runner` 限定，非 runner no-op）+ `runner_availability_stale_secs()`（`AI_PM_RUNNER_AVAILABILITY_STALE_SECS` 缺省 300）+ `is_runner_available` 纯函数（None=不可用，fail-closed） |
| 心跳归因点 | pm-mcp 六工具入口（`claim_task`/`list_tasks`/`report_progress`/`reacquire_lease`/`discover_runs`/`poll_commands`）best-effort 刷新——Runner 空闲期 ~10s 轮询 `pm_list_tasks` 即心跳，**Runner 二进制零改动、MCP 工具契约零变化** |
| worker 领取过滤 | `claim.rs` 阶段 A 两分支均加 `NOT EXISTS (active binding → device.is_runner)` 排除——静默、零审计、零退避；回收分支（B3）同排；与 D-1=b env 排除并集生效；**Model A 不经此路径（A-1 不变）** |
| 新鲜度语义 | 本期只落库与定义语义，**不门控 worker 领取**（worker 对领取型项目恒不领，静态 `is_runner` 判定）——「Runner 离线 → worker 兜底接管」是 V2 调度入口闭合的裁决范围，本批不越界 |

## 二、验证证据

| 项 | 结果 |
|---|---|
| 单测 `is_runner_available`（新鲜/过期边界/None=不可用） | 2/2 ✓ |
| devenv 集成：`is_runner=true` 登记回读 + runner 心跳刷新写列 + 非 runner no-op | ✓（devenv 套件 15/15） |
| run_dispatch 集成：`runner_bound_project_run_is_not_claimed_by_worker`（worker None + run 保持 queued + **零审计行** + Model A 仍可领）/ `non_runner_binding_does_not_exclude` / `unbound_runner_binding_does_not_exclude` | ✓（run_dispatch 套件 30/30） |
| pm-mcp 全套件（caller_ctx/contract/discovery/hosts/tools） | 全绿 ✓ |
| **pm-application 全量回归** | **443 passed / 0 failed** ✓ |
| rustfmt（单文件 --check） | 干净 ✓ |
| clippy（三个 crate --no-deps，本批文件） | 零新告警 ✓ |
| 迁移台账 | 0063 记账（无复用号）✓ |

> 本机测试 `DATABASE_URL=postgres://aipm:aipm@127.0.0.1:5433/aipm`（aipm-pg-dev）。

## 三、环境坑修复（本批顺带，aipm-pg-dev 5433）

全量回归首跑**整片 VersionMismatch(25)**——根因是 `template1` 被污染（112 张业务表 +
`_sqlx_migrations` max=62，CRLF 指纹），sqlx::test 新建库继承毒表后与本次嵌入迁移
checksum 不符。处置：DROP 全部用户 schema/表清空 template1 + 删除 29 个泄漏
`_sqlx_test_*` 库。复跑全绿。**提示**：其他共享 PG 容器（16447/16499/16453 等）
若复现同类红，先查 `template1` 的 `to_regclass('_sqlx_migrations')`。

## 四、.10 应用配方（待执行；D-1=b + D-2=c 合并上栈）

1. 干净 worktree（master）构建 **pm-api + ai-pm-worker** 镜像（0063 迁移由 pm-api
   启动迁移，须随 pm-api 上栈）；`docker save/gzip/SFTP/load`（R3 体例）。
2. `.10`：`UPDATE devenv.device SET is_runner = true WHERE name = 'e2e-runner-59';`
   （或设备登记 API 重发带 `is_runner:true`）。
3. `.env.prod`：`AI_PM_RUN_DISPATCH_EXCLUDE_PROJECTS=e2e-acp09` 保留为强制兜底
   （belt-and-suspenders，Q-D2C-6）；`AI_PM_RUNNER_AVAILABILITY_STALE_SECS` 可选。
4. `docker compose up -d --force-recreate pm-api ai-pm-worker`（compose 项目
   `infrastructure`）；验证：worker 日志对 e2e-acp09 零 claim/零 skip 审计；
   Runner 自领全链不变；`last_heartbeat_at` 随 Runner 轮询前进；lmms 八服务冒烟。

## 五、残余风险

1. **新鲜度尚无消费方**：`last_heartbeat_at` 已落库并定义语义，但 V2 调度入口闭合
   （可观测面/离线兜底接管策略）落地前，该真源只有写入没有消费——预期状态，非缺陷。
2. **`is_runner` 是登记时声明**：存量设备（如 e2e-runner-59）需人工 UPDATE 或重发
   登记；声明与事实的一致性（声明 runner 但从不心跳）由心跳真源暴露，策略待 V2。
3. **D-1=b env 排除列表与库内事实并存**：两者并集生效；长期维护成本（两个排除面）
   待 D-2=c 全链稳定后按 Q-D2C-6 再议去留。
4. 设备级与 run 级两级心跳并存（职责边界：device=可用性、run=执行健康），已写入
   迁移注释与设计包 §七-2，防止未来分裂。
