# Server Execution Management Architecture V1

| 项目 | 内容 |
|---|---|
| 文档性质 | 正式架构基线（服务边界与部署拓扑） |
| 状态 | V1 架构方向已确认；实施细节按本文标注分阶段闭合 |
| 日期 | 2026-11-24 |
| 适用范围 | AI-PM Server Worker、Session 管理、Client Runner、Harness 执行协调 |
| 不在范围 | Artifact Service / SeeCode、仓库制品托管、通用消息平台、微服务拆分实施 |

## 1. 架构决策摘要

用户确认：**Worker 与 Session 管理能力部署在一台独立的 Docker 服务器上**。该服务器是 AI-PM 的常驻 Server Execution Management Node（服务端执行管理节点），与 AI-PM Web/API 所在部署环境在部署边界上分离。

本基线确定：

1. Server Worker 与 Session Manager 属于同一个独立 Docker 部署单元，部署在专用 Docker 主机上。
2. V1 不要求 Worker 与 Session Manager 拆成两个微服务或两个独立仓库。它们是两个逻辑职责，可以先在同一 Rust 应用/同一容器中协作。
3. Server Worker 负责服务端 Run 的领取、启动、监督、终止、回收和执行结果事实采集。
4. Session Manager 负责跨执行节点的 Session Registry、生命周期协调、恢复判定、接管协调和事件索引；它不伪装成 Harness，也不声称能直接控制 Client Runner 上的本地进程。
5. Client Runner 仍安装在用户设备上，负责本机工作区、Harness CLI 启动、进程监督、Agent 事件采集和上报。
6. Codex、Z-Code、DSH、Claude Code 等通过 Harness Adapter 接入；Harness 的原生 Session 能力由 Adapter 声明，不能假定所有 Harness 都支持跨设备恢复。
7. AI-PM Platform 仍是 Task、Run、Delivery、Review、Interaction 和用户决策的业务权威。执行管理节点不自行裁决业务完成。
8. Runner 监督模式（Supervised / Unattended）是设备/Runner 注册属性；人工介入要求是单次 Run 的执行属性。
9. 无人监督 Runner 执行失败或无法自主恢复时，创建 Human Intervention Request（HIR），通知用户；由用户选择有人监督 Runner。系统不自动选择接手 Runner。
10. 高频人工介入只形成统计和建议，不自动修改 Runner 监督模式或 Run 执行策略。

## 2. 术语和边界

| 名称 | 定义 |
|---|---|
| AI-PM Platform | 项目管理与执行控制平台，包含业务 API、MCP 接口、前端和权威业务数据 |
| Server Execution Management Node | 独立 Docker 服务器上的常驻执行管理部署单元 |
| Server Worker | 在服务端执行节点领取并执行服务端允许运行的 Run |
| Session Manager | 统一管理 Session 元数据、生命周期和恢复/接管协调的逻辑模块 |
| Client Runner | 用户设备上的执行代理，管理本机工作区与 Harness 进程 |
| Harness | Codex、Z-Code、DSH、Claude Code 等实际 Agent 执行引擎 |
| Harness Adapter | 把各 Harness 的启动、输入、事件、Session 恢复和终止差异适配为统一契约 |
| Execution | 一次具体的执行过程 |
| Attempt | 同一 Run 下的一次启动/重试尝试 |
| Session | 与 Run/Execution 关联的 Agent 会话元数据及 Harness 原生会话引用（若有） |
| HIR | Human Intervention Request，协调异常后人工接管的独立请求 |

必须区分以下身份与状态：

- Runner 在线不代表有人在线。
- Runner 心跳不代表 Agent 正在推理或执行工具。
- 进程存活不代表 Harness 正在有效工作。
- Session Manager 中有 Session 记录不代表 Harness 原生 Session 一定可恢复。
- Worker 不是 Scheduler 的同义词；调度/协调是平台能力，Worker 是执行节点进程。
- Session Manager 是逻辑职责，不等于一个必须独立部署的微服务。

## 3. 部署拓扑

### 3.1 目标拓扑

```text
┌─────────────────────────────────────────────────────────────┐
│ AI-PM Platform Deployment                                    │
│                                                             │
│  Web / pm-api / MCP                                         │
│  Task · Run · Interaction · HIR UI · User Decisions         │
│  PostgreSQL (authoritative business state)                  │
└──────────────────────────────┬──────────────────────────────┘
                               │
                   authenticated private connection
                    API/MCP + DB access as required
                               │
┌──────────────────────────────▼──────────────────────────────┐
│ Dedicated Docker Host: Server Execution Management          │
│                                                             │
│  Docker Compose deployment unit                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ ai-pm-worker                                          │  │
│  │ - server-side Run dispatch / execution                │  │
│  │ - process supervision / cleanup / reconciliation     │  │
│  │ - server-side Harness Adapter                         │  │
│  └───────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ Session Manager (V1 logical module)                   │  │
│  │ - Session Registry / lifecycle                        │  │
│  │ - recovery & handoff coordination                     │  │
│  │ - event correlation / audit references                │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  Persistent server execution workspace volume               │
│  Persistent process/session metadata as required             │
└──────────────────────────────┬──────────────────────────────┘
                               │
                    authenticated Runner protocol
                               │
            ┌──────────────────┴──────────────────┐
            │                                     │
┌───────────▼────────────┐             ┌──────────▼───────────┐
│ Client Runner A        │             │ Client Runner B      │
│ Unattended             │             │ Supervised           │
│ local Harness + repo   │             │ local Harness + repo │
└────────────────────────┘             └──────────────────────┘
```

### 3.2 独立 Docker 服务器的含义

“独立 Docker 服务器”是独立的部署/主机边界，不只是把 Worker 容器加到现有 Web/API 的 Compose 文件中。

目标部署要求：

- 专用 Docker 主机或独立 VM，运行 Server Execution Management Compose 项目。
- Worker 容器通过受控网络访问 AI-PM 数据库和必要的平台接口。
- 不要求把 PostgreSQL 搬到该主机；数据库仍由平台既有数据库部署提供。
- 不把 Docker socket 挂载给 Worker；V1 不使用 Docker-in-Docker 管理 Agent。
- Worker 的服务端工作区使用专用持久卷；容器重建不得静默丢失执行现场。
- Client Runner 不部署在这台 Docker 主机内；它仍运行于用户设备。
- API、MCP 和 Runner 通信应使用认证与 TLS/受信任私网。数据库不应为了 Worker 而暴露到公网。
- Server Worker 与 Client Runner 可以并行存在，但同一 Run 必须遵守现有原子 claim、lease 和 generation fencing 规则。

### 3.3 V1 容器形态

初始建议一个独立 Compose 部署单元，逻辑上包含：

- `ai-pm-worker`：沿用现有 Worker 二进制作为第一阶段载体。
- Session Manager：先作为 Worker 内部模块/应用服务，不强制单独容器。
- 持久化工作区卷：仅服务端 Worker 执行的仓库和进程证据。
- 配置与密钥：通过部署 Secret/环境注入，不写入镜像或仓库。

只有当 Session 管理需要独立扩缩容、独立故障域、独立升级节奏或被多个 Worker 集群共享时，才考虑拆成单独进程/容器。V1 不引入 Redis、RabbitMQ、Kafka 等新基础设施，优先复用 PostgreSQL 事务、现有命令和事件机制。

## 4. 职责划分

| 能力 | AI-PM Platform | Server Worker | Session Manager | Client Runner | Harness |
|---|---|---|---|---|---|
| Task / Run 业务状态 | 权威 | 按协议上报事实 | 不裁决 | 不裁决 | 不裁决 |
| Run claim / lease | 规则与持久化权威 | 服务端执行领取 | 接管协调 | 客户端显式领取 | 无 |
| Runner 注册/监督模式 | 权威目录 | 读取候选信息 | 读取监督属性 | 注册并报告状态 | 无 |
| 服务端进程启动 | 无 | 负责 | 记录关联 | 无 | 被启动 |
| 客户端进程启动 | 无 | 无 | 记录关联 | 负责 | 被启动 |
| Session Registry | 持久化权威可由平台 DB 承载 | 报告服务端 Session | 统一逻辑管理 | 报告客户端 Session | 提供原生引用（若支持） |
| Session 恢复判定 | 策略/授权 | 执行服务端恢复 | 协调 | 执行本机/接管恢复 | 声明能力并执行 |
| 跨 Runner 接管 | 用户决策与授权 | 可作为源/目标执行端 | 协调状态转移 | 接受接管并执行 | Adapter 支持时恢复 |
| 人工介入请求 | 权威状态与通知 | 上报执行事实 | 关联 Session/Execution | 上报等待/异常事实 | 提供事件 |
| 高频介入统计 | 展示与决策 | 提供事件 | 关联聚合 | 提供事件 | 提供事件 |

## 5. Worker 与 Session Manager 的边界

### 5.1 Server Worker

现有 `apps/ai-pm-worker` 已承担多条后台循环，并非纯粹的 Run Worker。当前源码包含 AI 分析/编排、outbox 消费、Run dispatch、Interaction timeout sweep、清理与 reconcile 等职责。

V1 不在本架构文档中要求立即重写该二进制，但后续实施应将职责在代码模块层分离，避免继续把所有循环和 Session 逻辑堆进 `main.rs`。

Server Worker 的执行职责：

1. 读取服务端允许领取的 Run。
2. 使用共享 Run Dispatch 规则领取任务并取得租约。
3. 构造服务端工作区。
4. 通过服务端 Harness Adapter 启动 Harness。
5. 监督进程、心跳、超时、退出状态和清理。
6. 上报 Execution Event，不直接替代 AI-PM 做业务裁决。
7. 容器重启后执行孤儿 reconcile；不能确证旧进程已终止时不得盲目重新执行。

### 5.2 Session Manager

Session Manager 是服务端的统一 Session 管理逻辑，负责：

- Session Registry：关联 `session_id`、`run_id`、`execution_id`、`attempt_id`、`runner_id/device_id`、`harness_type`。
- 生命周期：created、starting、active、waiting_input、waiting_permission、suspended、completed、failed、unknown。
- 原生 Session 引用：记录 Harness 原生 ID、Adapter 类型、是否可恢复及恢复范围。
- 事件关联：把 Runner 心跳、进程状态、Harness 活动事件分开保存/关联。
- 恢复协调：判定同设备恢复、跨设备接管、重建 Session 的前置条件。
- Handoff 协调：记录用户选定目标 Runner、接管邀请、接受结果、lease generation 变化。
- 审计：保留每次接管决策和状态变更证据。

Session Manager 不负责：

- 代替 Runner 启动 Client 本地进程。
- 远程伪造/注入 Harness 原生 Session ID。
- 将进程存活推断为 Agent 正在工作。
- 未经用户决策自动选择 Supervised Runner。
- 将 HIR 当作 Task 或 Run 的替代状态。

## 6. Client Runner 注册与监督模式

Runner 注册必须声明：

`supervision_mode = supervised | unattended`

建议同时保留彼此独立的字段：

| 字段 | 语义 |
|---|---|
| `is_runner` | 设备是否具备 Runner 身份（现有字段） |
| `supervision_mode` | 有人监督 / 无人监督（新增） |
| `last_heartbeat_at` | 最近一次经认证的设备活动时间（现有字段） |
| `availability` | 根据状态、心跳新鲜度等派生的可用性 |
| `capabilities` | OS、Harness、Adapter 版本、恢复能力、工作区能力 |
| `active_execution_count` | 当前执行负载（可由服务端派生） |

约束：

1. `supervision_mode` 不等于 online/offline。
2. `supervised` 只表示设备声明具备人工介入条件，不保证人员在场或必然及时响应；UI 应显示“设备在线/可接手”状态，不能把它解释为实时人员监测。
3. `unattended` Runner 不应被分配必须实时人工响应的任务。
4. Run 的 `human_intervention_policy` 独立于 Runner 的监督模式。
5. 监督模式变化必须通过用户明确操作并审计，任务失败或介入频率不得自动修改该值。

## 7. Human Intervention 与跨 Runner Handoff

### 7.1 触发

无人监督 Runner 在以下情况优先执行有限、可配置的自主恢复：

- 短暂网络故障；
- 可重试工具失败；
- 可恢复的进程异常；
- 预先授权范围内的依赖问题。

超过重试策略、需要权限/用户输入、Harness 异常无法恢复或进程状态不确定时，平台创建 HIR。

### 7.2 用户选择流程

1. Source Runner 上报失败/等待事件和执行上下文。
2. Platform 创建 HIR 并通知用户。
3. UI 展示错误类别、原 Runner、Harness、Session 恢复能力、工作区状态和可接手 Runner。
4. 用户明确选择一台 Supervised Runner。
5. 目标 Runner 接受接管邀请。
6. Platform/Session Manager 校验源执行状态、租约、generation、工作区和目标能力。
7. 原子转移执行权；旧 Runner 的 token/generation 失效。
8. 目标 Runner 恢复原 Session；不支持时创建新 Session/Attempt，但仍关联原 Run 和 HIR。
9. 记录结果并关闭/解决 HIR。

用户选择目标 Runner 不等于接管已经成功；只有目标 Runner 接受且平台完成 fencing/lease 转移后才算接管成功。

### 7.3 安全不变量

- 同一 Run 同一时刻只有一个有效执行所有者。
- 接管必须增加 lease generation，并使旧 generation 失效。
- 不允许放宽现有 `lease_client_id` 身份校验来实现接管。
- 旧 Runner 进程必须确认停止，或被隔离到无法写入共享工作区。
- 未回执的 terminate/continue 命令必须按明确规则迁移或失效，不能让旧 Runner 消费。
- 目标 Runner 接管前必须确认仓库/工作区可用。
- 用户选择、目标 Runner 接受、lease 转移和恢复结果全部审计。
- 状态不确定时 fail closed，不允许双执行。

### 7.4 Session 与工作区恢复

恢复优先级：

1. 恢复 Harness 原生 Session（仅当 Adapter 明确声明支持，且原生 Session 可由目标 Runner 访问）。
2. 原生 Session 不可跨设备恢复，但工作区快照/仓库状态可恢复：保留工作区状态，启动新的 Harness Session。
3. 工作区状态也不可安全恢复：向用户明确说明损失风险，等待用户决定重新执行或放弃接管。

V1 不假定本地未提交修改可以自动跨设备迁移。工作区交接协议是实施前置专题，必须保证不会覆盖或丢失未提交变更。本文件不定义 Artifact Service 或文件制品传输服务。

## 8. Session 与 Execution 统一标识

建议统一记录字段：

| 字段 | 说明 |
|---|---|
| `run_id` | 业务 Run |
| `execution_id` | 一次执行生命周期 |
| `attempt_id` / `attempt_no` | 一次启动/重试 |
| `session_id` | 平台 Session 标识 |
| `harness_session_id` | Harness 原生 Session ID，可空 |
| `runner_id` / `device_id` | 当前执行节点 |
| `worker_instance_id` | 服务端 Worker 实例（服务端执行时） |
| `harness_type` | codex / zcode / dsh / claude_code / custom |
| `adapter_version` | Adapter 版本 |
| `lease_generation` | 执行权代次 |
| `workspace_ref` | 工作区定位引用，不等于可跨设备访问的承诺 |

现有 `executor_session` 是服务端执行器启动意图/执行器会话标识，不能直接视为统一的 Harness 原生 Session ID。现有 Client Runner 的 PID 派生 session 标识也不能冒充原生 Session ID。

## 9. 事件模型

必须分离三种信号：

1. Runner Heartbeat：Runner 通信循环存活。
2. Process Liveness：Agent/Harness 进程存活或退出。
3. Harness Activity：Agent 推理、工具调用、等待输入/权限等真实活动（仅在 Adapter 可提供时）。

建议事件信封：

```json
{
  "event_id": "uuid",
  "event_type": "tool_call_started",
  "occurred_at": "RFC3339",
  "sequence": 42,
  "run_id": "uuid",
  "execution_id": "uuid",
  "attempt_id": "uuid",
  "session_id": "uuid",
  "runner_id": "uuid",
  "harness_type": "codex",
  "lease_generation": 3,
  "payload": {}
}
```

标准事件类型至少覆盖：

- `runner.heartbeat`
- `execution.started` / `execution.failed`
- `process.alive` / `process.exited`
- `session.created` / `session.resumed` / `session.completed`
- `agent.thinking`（仅 Harness 明确提供时）
- `tool.call.started` / `tool.call.completed`
- `interaction.required` / `interaction.resolved`
- `human_intervention.created` / `human_intervention.resolved`
- `handoff.requested` / `handoff.accepted` / `handoff.completed` / `handoff.failed`

不支持的 Harness 活动状态必须表示为 `unsupported` 或 `unknown`，不得根据 Runner 心跳或进程存活伪造。

## 10. 当前代码基线与差异

本节按本次只读检查到的 master 源码/文档归纳。旧审查文档的审查基线早于当前代码，不能单独作为当前 HEAD 的完整证据。

| 领域 | 当前实现事实 | V1 目标 | 差异 |
|---|---|---|---|
| Worker 部署 | `infrastructure/full-dev/docker-compose.ai-pm.yml` 已有 `ai-pm-worker` 服务；与 pm-api/pm-worker/前端处于现有 Compose 网络 | 独立 Docker 主机上的独立 Compose 部署单元 | 部署拓扑未拆成独立主机边界 |
| Worker 运行职责 | `apps/ai-pm-worker/src/main.rs` 含多条后台循环，包括 Run dispatch、Interaction timeout、cleanup/reconcile 及 AI/outbox 相关工作 | 明确 Server Execution Management 职责边界 | 主循环职责混合，需模块化和部署职责收敛 |
| 服务端执行 | `run_executor.rs` 提供 claim/start/poll/cancel/reconcile；`CodexExecutor` 负责服务端工作区、进程启动、PID/日志和终止 | 保留服务端执行能力并纳入独立执行管理部署 | 基础能力已有；部署和统一 Session 管理未闭合 |
| Client Runner | `apps/ai-pm-runner` 自主发现/领取、准备本地工作区、启动 Harness、心跳和命令轮询 | 保持为用户设备上的独立执行节点 | 基础模式已有 |
| Device Registry | `devenv.device` 已有 `is_runner`、`last_heartbeat_at`；注册请求支持 `is_runner` | 增加 `supervision_mode` 和接手可用性语义 | 缺监督模式和可接手状态 |
| Worker/Runner 竞争 | D-2=c 明确 Runner 绑定项目由 Runner 显式领取，Worker 在候选发现时排除该类项目；心跳新鲜度不作为自动抢活门控 | 继续保持显式执行模式，接管通过专门协议 | 不得把 Runner 离线直接解释为 Worker 可抢占 |
| Recovery | `recovery.rs` 的 `pm_reacquire_lease` 是同稳定 Client 身份恢复；generation 递增 | 受用户授权的跨 Client handoff | 缺跨 Client 安全转移 |
| Worker Command | 命令路由与 `worker_id`、`lease_generation` 绑定，已有 terminate ACK/continue 保护 | 接管后命令路由原子更新或旧命令失效 | 需定义 handoff 命令处理 |
| Interaction | 已有 `task_interaction`、等待状态、用户回复与原执行端继续机制 | 新增 HIR 作为跨 Runner 协调对象，可关联 Interaction | 现有问答/继续不是跨 Runner 接管 |
| Session | `executor_session` 在 spawn 前持久化，是启动意图；Client 进程 session 当前不等于 Harness 原生 Session ID | 平台统一 Session Registry + 原生 Session 能力声明 | 缺统一注册、生命周期与跨节点恢复语义 |
| Harness Adapter | Runner profiles/adapter 已支持不同启动模板和进程管理，但能力契约不完整 | 统一 spawn/input/event/terminate/session recovery capability contract | 缺统一能力声明及跨设备恢复验证 |
| 高频人工介入 | 有 Interaction/事件基础，未形成跨 Runner 的统计建议闭环 | 统计、阈值提示，不自动修改策略 | 缺统计读面与提示 UI |

### 10.1 关键代码/文档证据

- `apps/ai-pm-worker/src/main.rs`：服务端 Worker 主循环。
- `apps/ai-pm-runner/src/runner_loop.rs`：Client Runner 自领、心跳、命令轮询和监督。
- `crates/pm-application/src/run_executor.rs`：服务端 Run 执行编排。
- `crates/pm-infrastructure/src/codex_executor.rs`：服务端 Codex Executor 和进程监督。
- `crates/pm-application/src/recovery.rs`：同 Client 身份离线恢复。
- `crates/pm-application/src/worker_command.rs`：Worker Command 身份和 generation 约束。
- `crates/pm-application/src/devenv/mod.rs`、`apps/pm-api/src/pm_devices.rs`：Device 注册和 Runner 身份。
- `migrations/0063_runner_availability.sql`：Runner 身份和设备心跳字段。
- `docs/project-ai-team-v2/ai-pm-v05-d2c-implementation-baseline-v1.md`：Runner Availability 的语义基线。
- `infrastructure/full-dev/docker-compose.ai-pm.yml`、根 `Dockerfile`：现有容器构建和部署方式。
- `docs/architecture/runner-codebase-baseline-assessment.md`：既有 Runner 代码审查（审查时间/HEAD 早于本次检查，应后续更新）。

## 11. 实施路线与依赖

### Phase SEM-01：部署边界与现有 Worker 收敛

- 建立独立 Server Execution Management Compose 部署目录/配置。
- 明确独立 Docker 主机的网络、数据库访问、Secret、日志、持久卷和备份要求。
- 明确哪些现有 `ai-pm-worker` 后台循环部署到该节点，哪些仍属于通用后台工作。
- 将 `main.rs` 的循环装配拆成可测试的应用模块；不改变业务语义。
- 验证 Worker 容器重启后进程 reconcile、PID 证据和工作区持久化。

验收：可在独立 Docker 主机启动/停止/升级执行管理部署单元；不依赖 Web/API 容器本地文件系统；现有 Run Dispatch 与回收测试保持通过。

### Phase SEM-02：Runner Supervision Registry

- 为 Device 增加 `supervision_mode`。
- 注册/复用、读取、更新、撤销 API 和前端设置。
- 与 `is_runner`、`last_heartbeat_at`、availability 明确区分。
- 目标 Runner 列表只返回当前用户有权选择且在线、兼容、可接手的设备。

验收：有人监督/无人监督注册和更新可审计；设备在线但模式 unattended 时不能作为人工接管目标。

### Phase SEM-03：Session Registry 与事件协议

- 定义 Session/Execution/Attempt 关系和唯一约束。
- 定义 Runner heartbeat、process liveness、Harness activity 三种事件。
- Harness Adapter 声明原生 Session ID 和恢复能力。
- 对 unsupported/unknown 状态进行正确表达。

验收：不同 Harness 不支持的事件不被伪造；Session 状态可从持久化事件重建。

### Phase SEM-04：HIR 与用户选择接管

- 增加 Human Intervention Request 对象和状态机。
- 无人监督执行无法自主恢复时创建 HIR。
- 通知用户并展示目标 Runner 候选。
- 用户选择目标 Runner；目标 Runner 显式接受。
- 记录拒绝、超时、目标离线和取消等分支。

验收：未得到用户选择时不发生跨 Runner 接管；HIR 不取代 Task/Run/Interaction。

### Phase SEM-05：跨 Client Handoff 与恢复

- 定义专门 handoff 命令/应用服务，不放宽同 Client reacquire 校验。
- 在单一事务中完成接管授权、generation fencing、lease owner 转移和命令路由更新。
- 先完成源进程终止确认或隔离，再允许目标执行。
- 完成工作区可恢复性检查。
- 按 Adapter 能力恢复原 Session，否则新建 Session/Attempt 并保留原 Run/HIR 关联。

验收：并发接管只能一个成功；旧 Client 不能再写入；不确定源进程状态时 fail closed；未提交修改不丢失。

### Phase SEM-06：高频介入统计和建议

- 按任务类型、Harness、错误类别、Runner 和时间窗口统计。
- 展示介入次数/比率和趋势。
- 达阈值时提示用户。
- 不自动变更监督模式和 Run 策略。

验收：统计可追溯到 HIR/Execution Event；系统只建议，不自动改策略。

## 12. 非目标与禁止事项

V1 明确不做：

- 不把 Worker、Session Manager、Client Runner 合并为同一个进程。
- 不把 Session Manager 拆成必须独立部署的微服务。
- 不引入新的消息队列作为前置条件。
- 不将 Docker socket 暴露给 Worker。
- 不允许 Worker 因 Runner 心跳过期而直接抢占 Runner 项目的 Run。
- 不放宽 `lease_client_id` 或 `lease_generation` 的安全校验。
- 不把 Runner heartbeat 当成 Agent 活动证明。
- 不承诺所有 Harness Session 都能跨设备恢复。
- 不自动选择人工接管 Runner。
- 不因人工介入频率自动改变监督模式或执行策略。
- 不由执行管理节点裁决 Task/Delivery/Review 业务终态。
- 不讨论或实现 Artifact Service / SeeCode。

## 13. 待后续专题闭合的问题

以下问题不阻止冻结部署边界，但在对应 Phase 开始前必须形成具体设计：

1. Server Worker 与通用 AI/outbox 后台循环是否继续共用同一二进制，还是拆成不同进程但共享应用 crate。
2. Server Execution Management Node 是只连接 PostgreSQL，还是通过内部 Platform API 承担更多 Session/Runner 命令操作；须避免新增绕过现有授权的旁路。
3. HIR 的通知渠道和通知失败重试机制。
4. Runner 目标选择的租户/项目权限、Harness 兼容性、仓库权限和负载过滤规则。
5. 工作区交接协议：干净 Git worktree、远端分支、补丁/快照等候选方案及未提交变更保护。
6. Session 原生恢复能力的 Harness-by-Harness 兼容矩阵。
7. Session 事件保留周期、体积上限、脱敏与审计策略。

## 14. 冻结决策

本 V1 正式冻结以下架构决策：

- **部署边界**：Worker + Session Manager 部署在独立 Docker 服务器。
- **部署形态**：一个独立 Compose 部署单元；V1 逻辑分层，不强制微服务拆分。
- **执行边界**：Server Worker 管服务端进程；Client Runner 管用户设备上的进程。
- **Session 边界**：Session Manager 管统一元数据与生命周期协调；Harness Adapter 管原生 Session 差异。
- **监督模式**：Runner 注册声明 supervised/unattended。
- **人工接管**：无人监督执行无法恢复时创建 HIR，通知用户，由用户选择有人监督 Runner。
- **恢复优先级**：尽可能恢复原 Session；不支持时保留工作区/证据并创建新 Session/Attempt。
- **安全约束**：跨 Client handoff 必须 fencing，禁止双执行。
- **统计策略**：高频介入只提示，不自动改变策略。
- **业务权威**：AI-PM Platform 仍是 Task/Run/Delivery/Review 的业务权威。

