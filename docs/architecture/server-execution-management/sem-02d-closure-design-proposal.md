# SEM-02D — 阻断项关闭设计提案（D-01–D-12）

- 状态：**Technical Design Proposal — 待用户逐项确认；不是已冻结 ADR**
- 日期：2026-11-25
- 基线：当前 master 的 SEM-02D 审查报告及 ADR-SEM-01–25
- 范围：把 D-01–D-12 从“问题清单”推进为可审阅、可实现的具体设计。
- 明确边界：本文件只提出设计，不代表已修改代码、Migration、API、WSS、Docker 或运行配置；Phase 0 仍未启动。
- 不重开：独立 DB、单一 Git origin、不备份 Workspace、不引入消息中间件、SEM 唯一 Runner 控制面、Lease Renewal 唯一路径、Runner 默认 unattended 等已确认决策。

## 0. 设计总原则

1. Platform PostgreSQL 是 Run、Lease、Generation、HIR、命令业务状态及业务审计的唯一权威。
2. SEM DB 只记录执行侧与传输侧事实；不得以本地副本授权执行或解除 Platform 安全屏障。
3. 对所有自动 Claim、重派、回收和 retry/requeue，使用同一个 Platform 侧执行安全屏障谓词。
4. 任何不确定的进程状态都 fail closed：保留隔离、创建/关联 HIR，不自动重派。
5. HTTP/WSS 采用显式版本、稳定幂等键和可恢复状态；传输 ACK 不等于业务完成，命令 receipt 不等于进程终止。
6. D-01–D-12 的技术提案须经用户确认后才能从 Proposed 转为 Ratified；代码事实仍须在 Phase 0 前按最新 HEAD 复核。

## 1. D-01（P0）执行不确定性屏障物理模型

**建议：在 Platform 新增独立表 `delivery.run_execution_barrier`，不复用现有 Interaction `scheduling_shield`。**

建议字段：
- `id uuid PK`
- `tenant_id uuid NOT NULL`
- `project_id uuid NOT NULL`
- `task_run_id uuid NOT NULL`
- `lease_generation bigint NOT NULL`
- `barrier_kind text NOT NULL`（V1 固定 `execution_uncertain`）
- `state text NOT NULL`（`active` / `released`）
- `evidence_class text NOT NULL`（`E4` / `E5`）
- `reason_code text NOT NULL`
- `first_detected_at timestamptz NOT NULL`
- `last_observed_at timestamptz NOT NULL`
- `hir_id uuid NOT NULL`
- `released_at timestamptz NULL`
- `released_by uuid NULL`
- `release_reason text NULL`
- `release_evidence_ref text NULL`
- `version bigint NOT NULL DEFAULT 1`

约束与语义：
- 以 `(tenant_id, project_id, task_run_id, lease_generation)` 为范围；同一 Run+Generation 最多一条 active barrier（partial unique index）。
- `hir_id` 外键指向 Platform HIR；创建 HIR 与 barrier 必须同一事务提交。
- barrier 不覆盖、不改写 Interaction shield；自动调度必须同时满足“无活动 Interaction shield”与“无活动 execution barrier”。
- 唯一检查入口封装为 Platform domain predicate，例如 `is_run_dispatchable(run_id, generation)`；claim、B3-2、B06-D3、orphan reconciliation、retry/requeue、handoff claim 全部调用同一语义。
- E1/E2/E3 证据通过验证后，Platform 在单事务内记录证据引用、终态化/回收旧 Run、释放 barrier 并生成下一 Attempt；不得先释放后回收。
- E4/E5 不允许自动释放。人工风险接受是独立的高权限处置路径，必须显式风险确认、记录旧进程/凭据处置策略，并且**不能把人工接受风险伪装成 E1/E2/E3**。

## 2. D-02（P0）Phase 0 最小 HIR 与 Phase 4 完整 HIR

**建议：Phase 0 创建正式 HIR 主表，Phase 4 扩展同一实体，不做临时表或二次迁移替换。**

Phase 0 最小字段：
- `id uuid PK`、`tenant_id`、`project_id`、`task_run_id`、`lease_generation`
- `reason_code`、`evidence_class`（E4/E5）、`status`（`open` / `resolved` / `closed`）
- `created_at`、`updated_at`、`version bigint`
- `resolution_code NULL`、`resolved_at NULL`、`resolved_by NULL`、`resolution_evidence_ref NULL`

约束：
- 同一 tenant + Run + Generation 最多一个活动 HIR（partial unique index）。
- HIR 的创建、execution barrier 创建、Run 安全状态/审计写入必须在同一 Platform 事务中。
- HIR 使用乐观并发版本 `version`；每次状态变化写审计事件，禁止只改当前行而无历史。
- Phase 0 只需内部创建、查询、受控关闭接口；没有通知 UI、Runner 选择 UI 或自动 handoff。
- Phase 4 在同一实体上增加通知投递、assignee、用户选择意图、处置步骤、关闭原因及完整 UI/API；不能因 Phase 4 再建一张 HIR 表。
- HIR 关闭本身不解除 execution barrier；只有符合 D-01 的安全事务可以解除。

## 3. D-03（P0）Platform–SEM HTTPS API wire contract

**建议：所有内部接口置于 `/internal/v1`，JSON DTO，RFC 3339 UTC 时间，UUID 标识；统一错误 envelope。**

建议路由：

| Method / Route | 调用方 | 用途 / 幂等 |
|---|---|---|
| POST `/internal/v1/sem/inbox/events` | Platform → SEM | 接收 Outbox 事件；`event_id` 唯一 |
| POST `/internal/v1/sem/dispatch/operations` | SEM → Platform | 读取当前候选/资格并请求指定 Runner Claim；`claim_request_id` |
| GET `/internal/v1/sem/dispatch/operations/{claim_request_id}` | SEM → Platform | 查询同一 Claim 幂等结果，不创建新 Claim |
| POST `/internal/v1/sem/leases/renewals` | SEM → Platform | Lease Renewal；`request_id` |
| GET `/internal/v1/sem/runners/{runner_id}/authorization` | SEM → Platform | 授权重验证；不得仅凭 SEM 缓存授权 |
| GET `/internal/v1/sem/commands` | SEM → Platform | 按 Runner/Run/Generation 读取待投递命令 |
| POST `/internal/v1/sem/commands/{command_id}/receipt` | SEM → Platform | 记录 Runner receipt ACK；幂等 |
| POST `/internal/v1/sem/executions/reports` | SEM → Platform | 执行观察/状态报告；幂等 |
| POST `/internal/v1/sem/executions/termination-evidence` | SEM → Platform | 提交 E3；按 Run+Generation+command+process identity 幂等 |
| POST `/v1/runner/connect-tickets` | Runner → Platform | 设备凭据认证后签发一次性 WSS Ticket；专用设备认证端点 |

通用请求头：`Authorization: Bearer <service JWT>`（设备 Ticket endpoint 使用设备认证）、`Idempotency-Key`（所有变更请求）、`X-Correlation-Id`、`X-Protocol-Version: 1`。

通用成功 envelope：`{ "request_id": "...", "result": { ... } }`。错误 envelope：`{ "error": { "code": "...", "message": "...", "retryable": false, "correlation_id": "..." } }`。不得返回 SQL、堆栈、凭据或 token verifier。

错误分类建议：
- 400 schema/字段非法；401 身份无效；403 scope/设备授权失败；404 资源不存在；409 generation/owner/idempotency 冲突；410 ticket/lease 已过期；422 状态不允许；429 限流；503 可重试服务不可用；504 结果未知，调用方必须用原幂等键查询/重试。
- Claim 的 504/连接超时永远表示“结果未知”，不得生成新 key 再 Claim。
- 建议默认超时：普通 API 5s、Claim 10s、Inbox accept 3s；重试指数退避并加 jitter，受总预算约束。具体值允许部署配置，但上限由服务端策略限制。

Claim 请求必须包含目标 Runner 身份、稳定 `claim_request_id`、候选 Run 或调度请求上下文；Platform 自行校验设备注册、项目绑定、监督模式、资格、所有 Shield、并发锁及 Generation。SEM 不得提交任意 Runner ID 即获得租约。

## 4. D-04（P0）Runner–SEM WSS wire contract

统一 JSON envelope：
```json
{
  "schema_version": 1,
  "message_id": "uuid",
  "connection_epoch": 12,
  "correlation_id": "uuid",
  "sent_at": "RFC3339 UTC",
  "type": "runner.heartbeat",
  "run_id": null,
  "generation": null,
  "payload": {}
}
```

- 身份来自已验证 Ticket 与当前连接，不信任 payload 的 runner/device ID。
- 执行消息必须携带 `run_id`、`generation`；命令消息另带 `command_id`；进程证据另带完整 `process_identity`。
- V1 消息类型：`session.ready`、`runner.heartbeat`、`lease.renew`、`execution.dispatch`、`execution.accepted`、`execution.rejected`、`command.deliver`、`command.received`、`command.result`、`command.failed`、`process.terminated`、`runner.reconnect_sync`、`connection.revoked`、`server.drain`、`server.shutdown`。
- 每条需要可靠投递的服务端命令先写 SEM `command_delivery` 再发送；Runner receipt ACK 只证明收到，不证明执行成功或进程退出。
- `process.terminated` 仅在 OS wait/handle 确认目标进程树终止后发送；SEM 转发 Platform 验证，不能凭 WSS ACK 生成 E3。
- 连接 epoch 每次成功重连单调增加；旧 epoch、旧 generation、重复 message_id 被拒绝或按幂等结果响应。
- 推荐心跳 15s，连接失活阈值 45s；值由部署配置但有硬上限。最大消息 1 MiB；每连接有界 in-flight 数与背压，超过阈值拒绝新 dispatch，不丢弃已持久化命令。
- 重连顺序：重新获取 Ticket → 建立新 epoch → 身份/授权重验证 → Runner 上报活动进程清单及每项 process identity → SEM 与 Platform 对账 Run/Generation/command → 决定继续、取消或 E4/E5 隔离 → 才恢复新命令投递。不能把“重连成功”当作进程状态证明。

## 5. D-05（P0）SEM 七表物理模型基线

所有表位于 SEM 自有 PostgreSQL schema `sem`；主键 UUID；时间戳 UTC；外部 Platform ID 仅作为引用值，不建立跨数据库 FK。租户/项目标识仅作隔离和查询，不构成授权依据。

| 表 | 关键字段与唯一性 | 状态/索引建议 |
|---|---|---|
| `inbox_event` | `event_id PK`、type、schema_version、payload_ref/minimal_payload、received_at、status、attempt_count、next_attempt_at、processed_at、last_error_code | `received/processing/processed/retry/dead`；索引(status,next_attempt_at) |
| `runner_connection` | `connection_id PK`、runner_id、credential_ref、epoch、connected_at、last_seen_at、auth_valid_until、status、revoked_at、disconnected_at | 每 Runner 最多一个 active epoch；索引(runner_id,status,epoch DESC) |
| `runner_ticket_use` | `jti PK`、runner_id、consumed_at、expires_at | 按 expires_at 清理，但保留时间不得短于 Ticket 最大寿命+时钟偏差+安全余量 |
| `dispatch_operation` | `operation_id PK`、event_id、run_id、target_runner_id、claim_request_id UNIQUE、stage、platform_result_ref、generation、created_at、updated_at、last_error | `created/claim_pending/claimed/delivery_pending/dispatched/cancel_pending/terminal/blocked`；索引(stage,updated_at) |
| `command_delivery` | `delivery_id PK`、platform_command_id、run_id、generation、runner_id、message_id UNIQUE、status、attempt_count、sent_at、received_at、terminal_ack_at、next_attempt_at | 唯一(platform_command_id,generation,runner_id)；`queued/sent/received/acked/failed/expired`；索引(status,next_attempt_at) |
| `platform_delivery` | `delivery_id PK`、operation_ref、idempotency_key UNIQUE、target_operation、payload_ref、status、attempt_count、next_attempt_at、last_http_status、last_error_code | `pending/sending/delivered/retry/dead`；索引(status,next_attempt_at) |
| `process_observation` | `observation_id PK`、runner_id、run_id、generation、connection_epoch、command_id NULL、process_identity JSONB、evidence_class、observed_state、observed_at、evidence_digest UNIQUE、payload_ref | append-only；索引(run_id,generation,observed_at DESC) |

通用规则：
- 状态更新使用条件 UPDATE（旧状态 + version/updated_at）或行锁；并发 worker 用 `FOR UPDATE SKIP LOCKED` 领取待处理记录。
- `process_observation` 只追加，不覆盖历史；证据原文最小化，优先保存规范化字段和 digest。
- Ticket replay 记录清理不早于 Ticket 有效期+时钟偏差+最大连接握手窗口；其他表按业务审计/重试要求设置保留期，不无限保存 payload。
- SEM migration 由 SEM 部署单元单独执行；启动 readiness 必须验证 schema 版本、数据库可写、Workspace root 可用。SEM 不读取 Platform `DATABASE_URL`。
- 最终 SQL 类型、FK 与索引名称应在迁移文件评审时按当前领域 UUID/租户约束统一；本表是 DDL 生成前的结构契约，不应误称为现有 Schema。

## 6. D-06（P0）应用层 Ports / Adapter 与 SQL 归属

**建议按业务权威与执行事实拆分端口，不以 PgPool 作为跨边界依赖。**

Platform-owned ports（由 Platform SQL adapter 实现）：
- `RunClaimPort`、`LeasePort`、`WorkerCommandPort`、`RunLifecyclePort`、`RunEventAuditPort`、`HIRPort`、`RunnerAuthorizationPort`。
SEM-owned ports（由 SEM 本地 adapter 实现）：
- `ProcessSupervisorPort`、`WorkspacePort`、`ProcessObservationPort`、`CommandDeliveryLedgerPort`、`InboxPort`、`PlatformDeliveryPort`。

SEM application service 调用 Platform-owned ports 的 HTTP client adapter；Platform route handler 调用 Platform SQL adapters。Platform PgPool 只能在 Platform composition root 和 SQL adapter 中创建/注入。SEM composition root 只接 SEM DB pool 与本机 executor；禁止 `DATABASE_URL` 指向 Platform。

当前共享 application service 若混合 SQL 与 process side effect，必须先切开用例边界；不能简单把现有 `run_executor.rs` 复制到 SEM。所有 SQL statement 需归入字段/表级 ownership matrix；CI 增加 SEM 构建/配置检查，确保没有 Platform DB secret/连接。

## 7. D-07（P1）Service / Ticket 密钥生命周期

- Platform 与 SEM 使用不同 service signing key pair；Runner WSS Ticket 使用 Platform 的独立 ticket signing key pair，不复用 service JWT 私钥。
- 私钥由各自部署的 secret manager / OS secret store 提供给服务进程；不写入 Git、镜像、Workspace 卷或普通日志。若部署环境无 KMS，使用受限文件权限的外部挂载 secret，并记录轮换责任人。
- 公钥通过受认证的配置分发/固定信任清单发布，携带 `kid`；验证端缓存当前与下一把公钥。未知 kid 触发一次受控刷新，刷新失败则 fail closed。
- 轮换采用 overlap：先发布新公钥，再切换签名，等待最大 JWT/Ticket TTL + clock skew 后移除旧公钥。紧急泄漏时立即禁用 kid、撤销 service/device，并断开受影响 WSS。
- JWT TTL ≤5 分钟；WSS Ticket TTL ≤60 秒、单次使用；允许时钟偏差建议 30 秒以内。Ticket claims 固定包含 `iss/sub/aud/jti/iat/nbf/exp/kid/scope/runner_id/credential_id`。
- 30 秒授权新鲜度是 SEM 从最近一次成功 Platform revalidation 起算的最大缓存年龄；Outbox 撤权事件可提前失效。到期未能重验证，先禁止派发/命令，再关闭连接；活动进程按 E1–E5 处置，不视为已终止。
- `jti` replay 防护按 token/ticket 类型分别处理；不能用 JWT jti 代替业务幂等键。

## 8. D-08（P1）Claim 幂等响应加密与保留

- Claim 事务内保存业务 Lease/Generation 结果及加密的可重放响应元数据；两者同事务提交。
- 建议 AES-256-GCM 信封加密：Platform 持有 KEK（外部 secret manager/KMS），每条记录生成随机 DEK 加密响应，KEK 包装 DEK；记录保存 ciphertext、nonce、tag、wrapped_dek、key_id、created_at、expires_at。不得把 DEK/KEK 写入同一数据库明文。
- 明文 Grant 只在响应构造时解密进入内存；访问日志、Tracing span、异常、SQL 参数日志全部脱敏。SEM 不持久化 Lease Token。
- 相同 `claim_request_id` + 相同规范化 payload 返回同一结果；相同 key + 不同 payload 返回 409。
- 建议幂等响应保留 24 小时（需与最长网络恢复/调度重试预算核对）；过期后不允许该 key 重新 Claim。SEM 应查询 Claim 状态/当前 Lease，而不是创建新 key 重试。若无法恢复原 token，走 Platform guarded cancel/recovery。
- KEK 轮换支持多 key_id 解密旧记录；过期记录清除 ciphertext/wrapped DEK，保留不含秘密的幂等 tombstone，防止旧 key 被重用。

## 9. D-09（P1）E3 信任与跨 OS Process Identity

V1 信任假设：注册且当前未撤权的 Runner Supervisor 是 OS 进程观察者；E3 是经过身份/Generation/命令/进程绑定的设备证据，不是抵御已攻陷设备的硬件证明。高保证设备证明另立 ADR，不阻塞 V1。

规范化 `process_identity`：
- 通用字段：`pid`、`parent_pid`、`process_started_at`（OS 可提供时）、`executable_fingerprint`、`process_group_or_job_id`、`supervisor_instance_id`。
- Windows：优先以持有的 Process Handle + CreationTime + Job Object 作为观察锚点；不能只用 PID。Runner 重启后句柄丢失时，必须重新验证 PID+CreationTime/Job 关联，无法验证则 E5。
- Linux：pidfd 优先；否则 PID + `/proc/<pid>/stat` starttime + process group/session；无法取得稳定身份则 E5。
- macOS：使用可用的 process start time / audit token 等 OS 标识；无法区分 PID reuse 则 E5。
- 进程树退出判据：Harness 主进程及由 Supervisor 管理的 Job/process group 中所有受管子进程均退出；仅 shell wrapper 退出不构成 E3。
- E3 payload 必须绑定 Runner identity、connection epoch、Run、Generation、command ID、process identity、exit code、observed_at、evidence digest。字段缺失、generation/epoch 不匹配、进程身份不一致或迟到冲突证据均降级 E4/E5，不自动回收。

## 10. D-10（P1）Platform Command 与 SEM Delivery 映射

| Platform worker_command | SEM command_delivery / Runner | 规则 |
|---|---|---|
| issued | queued | Platform API 读出命令后 SEM 建投递记录 |
| delivered | sent 或 received | Platform 只有在 SEM 已持久化投递意图并成功提交相应 API 后更新 delivered；Runner receipt ACK 后单独更新 SEM received |
| acked | acked | 仅代表命令业务确认完成；由 SEM 转发 Runner 结果并由 Platform 校验 |
| failed | failed | Runner 明确报告命令执行失败，Platform 依据 Run/Generation 校验后落业务状态 |
| 终止命令 | receipt ACK + E3 两阶段 | receipt ACK 只证明命令收到；E3 是进程退出证据，不能合并为一个 ACK |

异常处理：
- Runner 收到命令但 receipt ACK 丢失：SEM 重发同 message_id，Runner 幂等响应。
- SEM 已落 receipt、Platform API 不可达：SEM `platform_delivery` 重试同一幂等键；不得重发为新业务命令。
- 迟到 ACK：按 command_id + Run + Generation + message_id 验证；旧 generation 只记审计/拒绝，不改变当前状态。
- E3 独立提交；命令业务 acked 不自动释放 execution barrier。

## 11. D-11（P1）HIR 状态机与屏障解除

建议状态机：
`created → notified → awaiting_runner_selection → accepted → in_progress → resolved → closed`。
无人监督 Runner 默认不要求用户预选；HIR 通知后进入 `accepted` 仅表示处置流程已接收，不表示执行权已转移。

严格推进条件：
- `created`：Platform 在 E4/E5 barrier 同事务创建。
- `notified`：通知服务确认持久化通知投递，不是“尝试发送”。
- `awaiting_runner_selection`：仅 supervised 场景；用户选择写入 intent，不改变 Run lease、Generation 或 barrier。
- `accepted`：有权限的用户/处置流程明确接受 HIR；仍不代表新 Runner 已获执行权。
- `in_progress`：只有 Platform Claim 成功、目标 Runner 身份有效且 Generation 已递增后才能标记 handoff 已执行；UI 应区分“处置处理中”与“新执行已接管”。
- `resolved`：Run 已安全终态化，或新的合法 Attempt 已启动且旧执行端已由 E1/E2/E3 证明终止/隔离策略完成。
- `closed`：复盘与审计完成；关闭 HIR 不自动解除 barrier。
- 原 Run 自愈关闭必须基于同一 Run+Generation 的可信恢复证明；迟到 heartbeat、WSS reconnect、用户选择、单纯超时均不能关闭 barrier。
- 人工风险接受需单独权限、二次确认、风险说明、外部副作用检查项和审计；若旧进程仍不确定，原 barrier 仍保持 active，不能因此允许普通自动重派。

## 12. D-12（P1）Harness 凭据清单与边界

| 凭据类型 | 典型持有者 | 是否注入 Harness | 规则 |
|---|---|---|---|
| Runner/SEM WSS 身份与 Ticket | Runner Supervisor | 否 | 仅 Supervisor 使用；短时 Ticket；不传 Harness |
| Platform service JWT | SEM service | 否 | 仅 SEM server 使用；最小 scope |
| Run Lease Token | Runner Supervisor / Harness MCP bridge（若业务协议仍要求） | 仅在必要业务调用时 | 优先由 Supervisor 代理业务请求；若 Harness 直连 MCP 是既定模式，则只注入单 Run/Generation token，短时、不可写控制面；日志全脱敏 |
| Git read/write credential | 执行端 credential broker | 按任务最小需要 | 优先短时、Repository-scoped；SSH key 临时文件权限最小、退出/回收清理；无法即时撤销时按潜在外部副作用处理 |
| LLM/provider credential | Harness provider adapter | 是，若 Harness 必需 | 使用项目/用户授权的最小 scope；不得把 SEM service key 或设备凭据传给 Harness |
| MCP URL / public endpoint | Harness | 是 | 非秘密配置；不得携带 bearer token 于 URL/query |
| Device credential / connect Ticket | Runner Supervisor | 否 | 不写 Workspace、不传 Harness、不记录日志 |

Phase 0 前需按实际 Codex/其他 Harness 启动器逐项验证：环境变量、命令行参数、配置文件、子进程继承、stdout/stderr、executor-events、crash dump、临时 SSH 文件和日志脱敏。任何无法即时撤销的 Git/API 凭据必须在 HIR 中明确标注潜在外部副作用；DB Lease Token 失效不代表 Git/LLM 凭据已撤销。

## 13. Phase 依赖与验收调整

- Phase 0 前必须 ratify D-01/D-02，并冻结 barrier/HIR 的 Platform Migration 与统一 dispatch predicate；Phase 0 仅做最小 HIR 持久化，不做通知/选择 UI。
- Phase 1 前必须 ratify D-03–D-08，冻结 HTTPS/WSS wire contract、SEM DDL、ports/adapter、key lifecycle、Claim idempotency encryption。
- Phase 2 部署验收同时验证 SEM 无 Platform DB secret、SEM DB 独立迁移、Outbox→Inbox 持久确认、Workspace 运行期卷与断电恢复；不建设 Workspace 备份。
- Phase 3 只做 Session Manager / native session 采集；Resume 仍默认关闭。
- Phase 4 扩展既有 HIR 表与 UI/handoff，不重建 HIR；落实 D-09–D-11。
- Phase 5 进行真实双 Runner、多 OS、分区、密钥撤销、凭据副作用与并发验收。
- D-12 凭据盘点必须在 Phase 0 代码改动前完成，Phase 0/1/2 各自验收真实注入与撤销行为。

## 14. 仍需用户确认的明确事项

本提案不擅自冻结下列选择：
1. D-01：是否采用独立 `delivery.run_execution_barrier` 表，而不是在 `task_run` 增加 barrier 列。
2. D-02：是否同意 Phase 0 创建最小正式 HIR 表，Phase 4 扩展同一表。
3. D-03/D-04：是否批准建议的 `/internal/v1` 路由前缀、JSON envelope 和消息命名。
4. D-05：是否批准七表字段/状态/唯一性基线，具体 SQL DDL 在此基线上生成。
5. D-06：是否批准按业务权威/执行事实拆分 Ports/Adapters 的边界。
6. D-07/D-08：是否批准独立 Ticket signing key、AES-256-GCM envelope encryption、24 小时 Claim response TTL（TTL 可根据重试预算修订）。
7. D-09：是否接受 V1 将已注册未撤权 Runner Supervisor 视为 E3 可信观察者，而不提供硬件证明。
8. D-10/D-11：是否批准命令双层状态映射与上述 HIR 状态推进条件。
9. D-12：实际 Harness 凭据清单需要按当前实现逐项复核；本文是目标边界，不冒充已完成代码盘点。

在这些事项确认并同步更新所有权威文档前，状态保持 `Design In Progress`，不得启动 Phase 0。
