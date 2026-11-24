# 架构决策记录、已关闭问题与验证门槛（decision-record）

- 性质：Server Execution Management Architecture V1 的正式决策登记。
- 规则：每条决策 = 结论 + 依据 + 何时可重启/推翻。

## A. 已裁决（本 V1 生效）

| # | 决策 | 依据 | 推翻条件 |
|---|---|---|---|
| ADR-SEM-01 | 循环 1/2/3/5/6/7/8 + pm-worker + pm-api audit listener 留 Platform；循环 4 执行子步迁 SEM | SEM-01 + 本轮复核（`worker-role-separation.md` §1） | 出现新的执行型循环且不 spawn 进程 |
| ADR-SEM-02 | Outbox 单消费者：循环 3 只在 Platform 运行；SEM 角色不装配 handler，误配 fail-fast | `register_all_handlers` 全集装配（`outbox.rs:394`）；单消费者消解集合分叉面 | 引入真正的消息中间件（当前禁止） |
| ADR-SEM-03 | 角色开关 `AI_PM_WORKER_ROLE=platform\|execution\|all`，默认 all；Phase 1 不拆新二进制 | 单文件改动、开发态零漂移 | execution 角色膨胀出独立调度需求时派生 `apps/ai-pm-execution-worker` |
| ADR-SEM-04 | 回收必须以终止证据为前提（E1/E2/E3 才可重派；E4 只隔离；E5 永不回收） | DB 租约 ≠ 进程死亡；代码证据 `claim.rs:574-674`、`run_executor.rs:489-583` | 出现比三证据/心跳更可靠的进程事实源 |
| ADR-SEM-05 | Workspace 唯一所有权：一区一进程；新 Attempt 永远新工作区 | 双写冲突不可由 DB fencing 防护（fencing 盲区） | 工作区改为无状态对象存储（远超 V1 范围） |
| ADR-SEM-06 | Session Manager = `pm-application` 逻辑模块，非微服务；Attempt = task_run 行链（attempt_no + supersedes_run_id），不新增 execution 表 | 现有模型已表达 attempt 血缘；Greenfield 审慎新增 | attempt 维度出现独立查询/权限需求 |
| ADR-SEM-07 | Harness Native Session 以 `task_run.harness_session_id` + `harness_kind` 两列采集；能力表在代码（const），不进 DB | 能力是编译期事实；`executor_session` 语义不得混用（任务明令） | 能力需要按租户/项目动态配置时 |
| ADR-SEM-08 | Platform PostgreSQL 是业务 Run/Lease/Generation 的唯一权威源；宿主机文件系统是本机进程证据；不引入 Redis/Kafka/RabbitMQ | 执行权必须由业务权威方仲裁；服务边界另由 ADR-SEM-19 定义 | 业务权威存储策略改变时复审 |
| ADR-SEM-09 | 跨 Runner 接管 = 新设备全新合法 claim（generation+1、token 重签）；不放宽 `lease_client_id` 校验 | 任务硬约束；正门 claim 已具备全部仲裁能力 | 「预授权转移」真实需求出现（另立迁移+校验） |
| ADR-SEM-10 | HIR 决策 Q1=B / Q2=B / Q3=A；E4/E5 挂起的 Run 以 HIR 为唯一自动出路 | 用户已确认三项决策 | 用户改裁 |
| ADR-SEM-11 | Greenfield 政策适用：可直接重构 Schema/接口，不为未发布行为保留兼容层；执行安全不降低 | `engineering-lifecycle-policy.md` | 项目进入有客户/生产数据阶段时复审 |
| ADR-SEM-12 | HIR 使用独立实体 `governance.human_intervention_request`，关联 Run/Attempt/Project/Runner；不复用审计事件承载业务生命周期 | 需要可查询、可处置、可统计的独立生命周期 | HIR 被证明不需要独立查询/状态/授权时 |
| ADR-SEM-13 | Harness Resume 能力默认关闭；能力声明与真实恢复成功分离，未完成版本化实测不得启用 | 当前没有充分实测证据；失败必须降级为 Fresh Attempt | 对具体 Harness 版本完成可重复恢复验收 |
| ADR-SEM-14 | Runner 命令与 ACK 通过 Runner 主动建立的认证 WSS 长连接传输；ACK 必须绑定 run/attempt/generation，并由 Runner 在 OS 进程句柄确认退出后发送 | WSS 是 SEM↔Runner 的唯一实时控制通道；普通消息 ACK/命令送达不构成终止证据 | 若 WSS 在特定环境不可用，需另立 ADR 定义安全降级通道 |
| ADR-SEM-15 | Runner 默认 `unattended`；仅显式配置为 `supervised` 才要求领取前用户确认 | 用户明确决策；监督模式与在线/忙碌状态正交 | 用户明确改变产品默认值 |
| ADR-SEM-16 | E4 Run 进入持久化 scheduling shield：不进入普通 claim 候选集、不因租约过期自动重派；由 HIR 处置；只允许受控人工/证据驱动解除 | 避免 E4 被其他回收路径重新领取；必须有可审计解除条件 | 状态模型重构时，以等价或更强的持久化隔离替代 |
| ADR-SEM-17 | Workspace/Worktree/Git Mirror 不做备份；运行期间仍使用持久卷，丢失后按 E4/E5 安全隔离，不得推断进程死亡 | 用户明确决策；这些内容可由唯一 origin 重建，运行现场不作为备份目标 | 产品未来明确要求灾难恢复现场 |
| ADR-SEM-18 | 每个 Repository 仅配置一个权威 Git `origin`；SEM 不管理 Git 远端同步或双远端合并 | 用户明确决策；Git Mirror 是可重建缓存 | 仓库管理策略另行改变 |
| ADR-SEM-19 | **Platform 与 SEM 通过版本化 HTTPS API 通信，各自管理数据库；禁止 SEM 直连/共享 Platform PostgreSQL。** Platform 是 Run、Lease、Generation、业务状态的权威方；SEM 通过 API 请求认领、取消、查询并上报执行状态/事件。Platform 使用 Transactional Outbox 触发派发，SEM 以幂等方式接收；SEM 可通过 API 拉取/对账补偿。Runner 与 SEM 使用 WSS。V1 不引入消息代理。 | 用户于 2026-11-24 明确确认方案 A；服务独立部署需要明确 API 契约和数据所有权 | API 吞吐/可靠性需求证明 HTTP + Outbox 不足时，另立决策评估消息代理；不得恢复共享数据库耦合作为默认方案 |
| ADR-SEM-20 | **SEM 是 Runner 控制面的唯一派发与接管协调者。** Runner 不再主动轮询 Platform MCP Claim、命令或恢复接口；SEM 选择/确认目标 Runner 后，代表该 Runner 调用 Platform HTTPS API 请求 Claim，Platform 在权威事务中校验目标设备/项目授权/可执行性并签发 Lease、Generation。成功后 SEM 才能经 WSS 派发。Harness 保留 Platform MCP 业务访问。 | 用户于 2026-11-24 明确确认；避免 Runner 直连 Claim 与 SEM 调度形成双控制面 | 若未来需 Runner 直连 Platform 控制面，必须另立 ADR；不得静默恢复双路径 |

## B. SEM-01 结论修正（相对 SEM-01 文本的两处精化）

1. **outbox「handler 集合一致」→「单消费者」**：SEM-01 的表述是必要不充分条件的保守版；本 V1 收敛为单消费者约束（更强、更简单）。SEM-01 无错误，属精度提升。
2. **reconcile_orphans 风险上调**：SEM-01 称「>1h 回收不带 runner 排除」；复核确认 B06-D3 直改 DB（绕过 claim 候选发现的 runner 排除），**同时缺进程证据**——与 B3-2 同根同罚，Phase 0 一并修复（`execution-safety.md` §3.2）。

## C. 已关闭问题与实施门槛

| OQ | 正式裁决 | 实施约束 / 验收门槛 |
|---|---|---|
| OQ-1 HIR 载体 | **关闭：采用独立表** `governance.human_intervention_request` | Phase 4 定义字段、唯一活动请求约束、状态迁移与审计；HIR 不替代 `task_interaction` |
| OQ-2 Codex Resume | **设计关闭，能力验证门槛保留**：默认 `supports_resume=false` | 每个 Harness/版本需在真实环境通过采集、重启、恢复、上下文连续性测试后，才可单独启用；未通过一律 Fresh Attempt |
| OQ-3 E3 ACK | **关闭：通过认证 WSS 承载 Runner 命令/ACK** | ACK 绑定 run_id、attempt/run 行、lease_generation、command_id、process identity、exit observation；仅在 OS wait/handle 确认进程退出后发送；送达/接收 ACK 不等于终止 ACK |
| OQ-4 监督模式 | **关闭：默认 `unattended`** | 注册持久化字段；首次注册未提供值时取 unattended；supervised 必须在领取前取得用户确认；在线状态不推导监督模式 |
| OQ-5 E4 租约卫生 | **关闭：持久化 scheduling shield** | shield 是 claim 与所有自动回收路径共同遵守的 DB 条件；E4 期间不得因 lease expiry 回 queued；不无限续租伪装健康；只通过证据确认死亡或经授权的人工处置解除 |
| OQ-6 凭据面 | **关闭：纳入执行安全基线并前移至 Phase 0** | 最小权限、短时凭据、可撤销、日志脱敏；终止旧进程后才可确认其进程内凭据失效；无法即时撤销的凭据必须按最坏情况评估外部副作用 |
| OQ-7 Workspace 备份 | **关闭：不备份 Workspace/Worktree/Git Mirror** | 运行期间单一持久卷；日志/审计/执行状态按各自保留策略持久化；卷丢失不等于进程死亡，所有无法确认的活跃 Run 进入 E4/E5 |
| OQ-8 双远端 | **关闭：每个 Repository 只有一个权威 `origin`** | SEM 不配置、不同步、不合并第二远端；Git Mirror 从唯一 origin 重建；远端托管/镜像策略不属于 SEM V1 |

## D. 用户已确认的产品决策

1. Workspace、Worktree、Git Mirror 不需要备份；不设 RPO/RTO，不建设快照恢复系统。
2. Runner 默认无人值守（`unattended`）。
3. 每个 Repository 只有一个权威 `origin`；SEM 不讨论或实现双远端同步。
4. 用户尚未完成的“4.”不作推断；如后续补充，作为新决策输入。

## E. 仍需实测但不阻塞设计冻结的验证项

- Codex 原生 Session Resume：默认关闭，需在目标 CLI 版本上完成可重复的真实恢复验收。
- Runner E3 ACK：Phase 0 可先让远程 Runner 全部走 E4 安全挂起；E3 通道实现必须满足 OQ-3 的证明标准后才允许作为 E3。
- E4 scheduling shield：必须通过并发 claim、过期租约、迟到心跳、重启恢复和人工解除测试。

## F. Platform–SEM API 边界实施约束

1. SEM 不持有 Platform PostgreSQL 凭据，不执行跨服务 SQL，也不直接读写 `delivery.task_run`。
2. Platform API 是 Run/Lease/Generation 的唯一写入口；SEM claim 请求必须由 Platform 在事务内完成授权、并发仲裁、generation 递增、token 签发和审计。
3. Platform 的 Transactional Outbox 只负责可靠触发；事件不作为业务状态权威。SEM 接收必须有幂等键、持久化接收记录和重试/对账机制。
4. SEM 数据库仅保存执行侧私有状态，例如命令投递账本、连接会话元数据和执行侧协调状态；不得复制并形成第二套 Run/Lease 权威状态。具体表结构在 SEM-02C 定义。
5. 两侧不做分布式事务。跨服务操作采用明确状态机、幂等 API、超时重试、对账与补偿。
6. API 使用服务身份认证、最小权限、TLS、请求签名或等效防重放机制；所有写请求绑定 request/idempotency ID、Run ID 和 generation。

## G. 政策合规自查（`engineering-lifecycle-policy.md` §3/§4）

- ✅ 事务完整性与并发安全：Phase 0 全部走现有 `claim_core` 事务模板/锁序，不新开旁路。
- ✅ 唯一执行权与安全进程/工作区生命周期：ADR-SEM-04/05 即为此设立。
- ✅ 凭据与权限安全：token 明文一次性下发、库内 hash、卷内无凭据（`deployment-topology.md` §6）。
- ✅ 关键状态迁移可审计：HIR/回收/接管全链路审计行（`runner-supervision-and-handoff.md` §4.2 ⑧）。
- ✅ 空库完整初始化：Phase 2/3 的 Schema 增量走迁移链，验收含空库初始化。
- ✅ 本政策不授权实施：本文档仅为设计，实施待架构复审批准。


## ADR-SEM-20 的强制协议约束

1. SEM 是唯一 Runner 调度入口；Platform 是唯一执行权授予者。SEM 的调度选择不构成执行权，必须等待 Platform Claim 成功响应。
2. Claim 请求必须包含经认证的目标 Runner/device identity。Platform 自行校验设备注册、租户/项目绑定、权限、监督策略、Scheduling Shield 与并发条件；不得信任 SEM 或 Runner 自报的裸 device_id。
3. Runner WSS 控制面唯一承载 Claim 协调、Runner Supervisor Heartbeat、Lease Renewal 请求、命令投递/ACK、取消、恢复/接管控制。Harness Platform MCP 保留业务上下文、Agent Activity、交互和交付/结果操作。
4. Lease Renewal 只由 Runner Supervisor 经 WSS 发起，SEM 通过服务 API 转发给 Platform 校验并提交；Harness Agent Activity 不续租。WSS liveness、Lease Renewal、Harness Activity 是三种不同事实。
5. Runner 不得直接轮询 Platform MCP Claim、命令、ACK 或恢复控制接口。迁移完成后删除这些旧控制写路径，不保留双通道；Harness MCP 业务能力不受影响。
6. `supervised` Runner 的用户确认由 SEM 调度流程收集；用户选择只是授权意图，Platform Claim 成功才授予执行权。默认 `unattended`，按策略自动选择。
7. SEM DB 仅保存 Runner 连接、Inbox、WSS 投递账本、进程观察和本地执行协调状态；不复制 Run/Lease/Generation/worker_command 的业务权威状态。


## F. SEM-02C 服务协议与安全方案（建议基线，待用户确认）

以下方案已形成可评审的完整建议，但尚不登记为用户已确认的 ADR；用户确认后再转入 A 节正式决策：

| 建议编号 | 建议结论 | 主要约束 |
|---|---|---|
| ADR-SEM-21（Proposed） | Lease Renewal 唯一路径为 Runner Supervisor → WSS → SEM → Platform HTTPS API | WSS Heartbeat 与 Harness Agent Activity 均不续租；Platform 是唯一校验和写入者；SEM 不持久化 Lease Token |
| ADR-SEM-22（Proposed） | Platform↔SEM 使用双向独立 service identity + 短时非对称签名 JWT | TLS；audience/scope/kid/jti；建议 JWT 有效期不超过 5 分钟；不进入 human principal；mTLS 可作为附加防护 |
| ADR-SEM-23（Proposed） | Runner 使用 Platform 签发的短时、一次性 WSS connect ticket 连接 SEM | ticket 绑定 device/credential、SEM audience、scope、jti；SEM 原子消费 jti；Outbox 撤权事件 + 有界周期重验证；失去授权新鲜度则关闭连接 |
| ADR-SEM-24（Proposed） | SEM DB 采用最小执行/传输私有表集合 | Inbox、Runner Connection、Ticket Use、Dispatch Operation、Command Delivery、Platform Delivery、Process Observation；不复制 Run/Lease/Generation/HIR/命令业务状态 |
| ADR-SEM-25（Proposed） | Claim 与跨服务副作用必须幂等可恢复 | SEM 先持久化 dispatch operation；Platform Claim 与幂等结果同事务；丢失响应时以同一 claim_request_id 取回同一结果；SEM 不存明文 Lease Token |

完整字段建议、消息、事务边界和故障矩阵见 `service-protocol-security-and-persistence.md`。