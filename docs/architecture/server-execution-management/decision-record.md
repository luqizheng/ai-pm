# 架构决策记录与未决问题（decision-record）

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
| ADR-SEM-08 | PostgreSQL 是 Session/执行权唯一权威源；宿主机文件系统只是证据；不引入 Redis/Kafka/RabbitMQ | 任务禁令 + 现有 DB 仲裁架构成熟 | 规模或时延需求超出 DB 轮询能力（远期） |
| ADR-SEM-09 | 跨 Runner 接管 = 新设备全新合法 claim（generation+1、token 重签）；不放宽 `lease_client_id` 校验 | 任务硬约束；正门 claim 已具备全部仲裁能力 | 「预授权转移」真实需求出现（另立迁移+校验） |
| ADR-SEM-10 | HIR 决策 Q1=B / Q2=B / Q3=A；E4/E5 挂起的 Run 以 HIR 为唯一自动出路 | 用户已确认三项决策 | 用户改裁 |
| ADR-SEM-11 | Greenfield 政策适用（`engineering-lifecycle-policy.md`）：可直改 Schema/接口、可提删除方案、不留兼容层；但执行安全要求不降低 | 政策文件（GitHub master `f640cc07`） | 项目进入有客户/生产数据阶段时政策自然失效 |

## B. SEM-01 结论修正（相对 SEM-01 文本的两处精化）

1. **outbox「handler 集合一致」→「单消费者」**：SEM-01 的表述是必要不充分条件的保守版；本 V1 收敛为单消费者约束（更强、更简单）。SEM-01 无错误，属精度提升。
2. **reconcile_orphans 风险上调**：SEM-01 称「>1h 回收不带 runner 排除」；复核确认 B06-D3 直改 DB（绕过 claim 候选发现的 runner 排除），**同时缺进程证据**——与 B3-2 同根同罚，Phase 0 一并修复（`execution-safety.md` §3.2）。

## C. 未决问题（Open Questions）

| # | 问题 | 现状与倾向 | 影响面 | 建议裁决时点 |
|---|---|---|---|---|
| OQ-1 | HIR 载体：新表 `governance.human_intervention_request`（方案甲）vs 审计事件复用（方案乙） | **倾向甲**（Q3 频次聚合需要可查询实体）；零数据迁移负担（Greenfield） | Phase 4 迁移设计 | Phase 4 开工前 |
| OQ-2 | codex `supports_resume` 实测未做 | 默认 false（安全）；需真实环境验证 rollout 恢复 | Phase 3 验收 | Phase 3 交付前 |
| OQ-3 | runner cancel ACK 通道（E3）的协议落点：现有命令 poll 通道扩展 vs 新 MCP 工具 | 倾向扩展现有命令通道（runner 已有 poll/ack 骨架） | Phase 0（可先缺省，全走 E4 挂起）| Phase 0 实施设计 |
| OQ-4 | 无人监督/有人监督 Runner 的模式配置键与默认值 | 未定；倾向 env（`AI_PM_RUNNER_SUPERVISION=unattended\|attended`） | Phase 4 runner 改动 | Phase 4 开工前 |
| OQ-5 | E4 挂起的 Run 若 HIR 长期无人处置的租约卫生（行会一直占 `claimed/running` 活跃位） | 倾向：挂起态用 scheduling shield 承载（现状 N-1 语义），不新增状态列；超长挂起告警 | Phase 0/4 交界 | Phase 0 实施设计 |
| OQ-6 | 旧进程外部副作用的最终削减面（executor 凭据最小化、push 后 generation 标记丢弃） | 本 V1 只承认 fencing 盲区并依赖尽快 terminate；凭据面收敛属 executor 侧改造，**未纳入 Phase 0–5** | 长期 | 另立任务 |
| OQ-7 | `AI_PM_WORKSPACE_ROOT` 卷的备份/快照策略 | 拓扑已定义「卷丢失 → 活跃 Run 全部人工处置」的安全降级；备份策略属运维决策 | Phase 2 运维手册 | Phase 2 部署前 |
| OQ-8 | 内网 origin 与 GitHub 双远端的同步策略（当前 origin 落后 github 2 个政策提交，已本地合并） | 待用户/团队定夺推送目标 | 仓库协作流程 | 近期（非本设计阻塞） |

## D. 政策合规自查（`engineering-lifecycle-policy.md` §3/§4）

- ✅ 事务完整性与并发安全：Phase 0 全部走现有 `claim_core` 事务模板/锁序，不新开旁路。
- ✅ 唯一执行权与安全进程/工作区生命周期：ADR-SEM-04/05 即为此设立。
- ✅ 凭据与权限安全：token 明文一次性下发、库内 hash、卷内无凭据（`deployment-topology.md` §6）。
- ✅ 关键状态迁移可审计：HIR/回收/接管全链路审计行（`runner-supervision-and-handoff.md` §4.2 ⑧）。
- ✅ 空库完整初始化：Phase 2/3 的 Schema 增量走迁移链，验收含空库初始化。
- ✅ 本政策不授权实施：本文档仅为设计，实施待架构复审批准。
