# Worker 角色拆分（worker-role-separation）

- 性质：架构设计（Phase 1 的设计依据），本阶段不实施。
- 前提：Greenfield 政策允许直接重构装配代码，不留兼容开关以外的过渡层。

## 1. 循环归属总表

### 1.1 ai-pm-worker 的 8 条循环

| 循环 | 内容 | DB | 进程 | FS | 常驻 | 归属 | 理由 |
|---|---|---|---|---|---|---|---|
| 1 | ai_analysis | ✅ | ❌ | ❌ | ✅ | **Platform** | 只调 LLM API |
| 2 | repo_import | ✅ | ❌ | ❌（knowledge Git 卷由 Platform 侧服务持有） | ✅ | **Platform** | 依赖 Platform 的仓库卷 |
| 3 | outbox 分发 | ✅ | ❌ | ❌ | ✅ | **Platform（唯一消费者）** | 见 §3 D2 |
| 4 | Run 调度与执行 | ✅ | ✅ | ✅（workspace/pid/日志/事件） | ✅ | **拆分**：执行子步→SEM，DB-only 子步→Platform | 见 §2 |
| 5 | review 领取拍 | ✅ | ❌ | ❌ | ✅ | **Platform** | 确定性校验管线，纯 DB+LLM |
| 6 | 失败判定作业 | ✅ | ❌ | ❌ | ✅ | **Platform** | 扫描谓词纯 DB |
| 7 | 会话回复（Batch G） | ✅ | ❌ | ❌ | ✅ | **Platform** | LLM 生产者 |
| 8 | 访谈编排（DK-B） | ✅ | ❌ | ❌ | ✅ | **Platform** | 编排逻辑 |

### 1.2 循环 4 子步拆分

| 子步 | 归属 | 理由 |
|---|---|---|
| `claim_and_start` 的 spawn 半边（领取+租约签发+spawn） | **SEM** | spawn 与工作区/pid 必须**同主机**（SEM-01 结论 10） |
| `poll_active_runs`（三证据 poll + 终止确认） | **SEM** | 同上 |
| `run_cleanup_round`（收尾/失败落库编排） | **SEM** | 需访问本地进程证据 |
| `cleanup_finished_processes`（终态僵尸兜底 terminate） | **SEM** | terminate 必须在本机 |
| workspace reclamation 物理执行 | **SEM** | 删文件必须在工作区卷所在主机 |
| `interaction_timeout_sweep`（会话超时扫描） | **Platform** | 纯 DB |
| `comm_overview`（统计） | **Platform** | 纯 DB |
| `stage_candidates`（候选整理） | **Platform** | 纯 DB |
| `reconcile_orphans` | **SEM**（执行证据半边）+ **Platform**（B06-D3 DB-only 判定的审计半边可留任一处，定 SEM 单点） | 证据 poll 在 SEM 主机才有意义；见 `execution-safety.md` §3.2 |

### 1.3 其他进程

| 进程 | 归属 | 备注 |
|---|---|---|
| `pm-api`（含 audit LISTEN listener） | **Platform** | listener 是 pm-api 进程内 SSE 推送（`main.rs:36-41`），非独立循环 |
| `pm-worker`（repo-import / mcp-inbox，2 条 spawn） | **Platform** | `register_all_handlers` 全集装配（`main.rs:39`） |
| `ai-pm-runner` | **Client Runner** | 永不被 SEM 当本地子进程管理 |

## 2. 角色化装配（`AI_PM_WORKER_ROLE`）

```mermaid
flowchart TD
    M[ai-pm-worker main] --> R{AI_PM_WORKER_ROLE}
    R -->|platform 默认 all 的 Platform 半边| P[循环 1/2/3/5/6/7/8<br/>+ 循环4 DB-only 子步]
    R -->|execution| E[循环4 执行子步<br/>claim_and_start spawn 半边<br/>poll_active_runs / run_cleanup_round<br/>cleanup_finished_processes / reclamation<br/>reconcile_orphans]
    R -->|all 现状默认| A[P + E 同进程<br/>开发/单机零漂移]
```

- 取值：`platform` | `execution` | `all`（**默认 `all`**——单机开发/部署零行为漂移，独立主机部署时显式设值）。
- 唯一业务代码改动点：`apps/ai-pm-worker/src/main.rs` 的循环装配处按角色过滤 `tokio::spawn`。
- Greenfield 允许把循环 4 拆成两个显式函数（`run_db_only_round` / `run_execution_round`）再按角色装配，不保留「一个循环内部 if 角色」的纠缠写法。
- **不拆新二进制**（Phase 1）；常态化后可选派生 `apps/ai-pm-execution-worker`（复用 pm-application/pm-infrastructure，零新依赖）——属 Phase 2 之后的可选项，非必需。

## 3. Outbox 单消费者约束（D2）

- **约束**：循环 3 在整个系统中**至多一个部署单元运行**（定 Platform 角色）。SEM 角色不装配 outbox worker、不调用 `register_all_handlers`。
- 为什么这就够了：`register_all_handlers` 装配集合 = `EventCatalog` 全集（`outbox.rs:394` 的编译期/启动期校验）。只要消费者唯一，不存在「A 进程注册了事件 X、B 进程没注册 → X 永久 pending/假死信」的分叉面。
- guard：SEM 角色启动时若检测到 outbox 循环被请求装配 → 启动失败（fail-fast），防误配。
- 投递语义不变：outbox 仍是 Platform 内的 DB 轮询分发，SEM 与 Platform 之间通过版本化 HTTPS API 通信；Platform Outbox 负责可靠触发，双方不共享 PostgreSQL。

## 4. 循环 4 的跨角色协作

拆分后循环 4 的 Platform 业务半边与 SEM 执行半边通过版本化 HTTPS API 协作；不共享数据库、不直接读写对方数据：

```mermaid
sequenceDiagram
    participant P as Platform role（循环4 DB-only 子步）
    participant DB as PostgreSQL
    participant S as Execution role（循环4 执行子步）
    P->>DB: interaction_timeout_sweep / stage_candidates 等
    S->>DB: claim_and_start（含租约签发，spawn 本机进程）
    S->>DB: poll_active_runs（三证据 + 终止确认落库）
    S->>DB: run_cleanup_round / cleanup_finished_processes
    Note over DB: 调度开关 AI_PM_RUN_DISPATCH：<br/>Platform 角色设 off（不领新 Run）<br/>Execution 角色设 on<br/>回收不受开关影响（现状语义保留）
```

- `AI_PM_RUN_DISPATCH` 门控沿用现状（只门控领取，不门控回收，`main.rs:143-145` 注释即现状语义）。
- 无人监督领取集中在 Execution 角色；Platform 角色即使误设 `DISPATCH=On`，因不含 spawn 半边也只会空转 claim（claim 成功后无执行器 → 走 NoopExecutor 退避，现状已有该路径）——但**正确配置**是 Platform=off、Execution=on，写进部署清单。

## 5. 角色化后的重复执行风险矩阵

| 风险 | 缓解 |
|---|---|
| 两个角色同时跑 outbox | §3 单消费者约束 + SEM fail-fast |
| 两个 execution 实例领同一 Run | 现有 `FOR UPDATE SKIP LOCKED` + 租约互斥，天然安全 |
| Platform 与 Execution 同时跑 claim 领取 | 配置规约（Platform=DISPATCH off）+ 部署清单校验 |
| B06-D3 被 Platform/Execution 双跑 | 定 SEM 单点；audit `run.orphan_reclaimed` 去重保留为第二道防线（现状已有 NOT EXISTS 去重） |
| workspace reclamation 双跑 | 只装配在 Execution；三重开关不变 |
