# SEM V1 Migration Roadmap — SEM-02E

> **HISTORICAL — SUPERSEDED, NOT AN IMPLEMENTATION CONTRACT (2026-11-25):** This AI-PM-side SEM design copy records the former Platform Claim/Lease/Run/Task/HIR/Barrier control-plane proposal. It is retained for source-audit and migration traceability only. Do not implement its SEM business orchestration, database ownership, state machines or API flows. The current bilateral boundary and active SEM protocol drafts are indexed in [the AI-PM SEM integration review](./README.md) and maintained canonically in [luqizheng/smark-sem](https://github.com/luqizheng/smark-sem).


- 状态：**Design In Progress — Review Pending**。
- 唯一 Phase 定义：[SEM-02E §14](./sem-02e-architecture-convergence.md#14-phase-0–5-统一路线)。
- 本文为候选实施路线，不授权启动实施。

## Phase 0 — Safety + Minimum Human Intervention

物理模型和协议候选详见 [SEM-02F](./sem-02f-executable-physical-and-protocol-contracts.md)。孤儿扫描属于 Platform DB 责任，不属于 SEM 服务，也不得依赖 RabbitMQ。

**目标：** 在独立 SEM 服务上线前，先保证 Platform 侧任何执行路径都不会因 Lease 过期、Worker 重启或新 Run 行而产生双重执行，并具备最小人工处置闭环。

交付：
1. Task 级 Execution Barrier Migration；唯一 active Barrier 约束。
2. 复用并扩展 `governance.human_intervention`、`governance.human_consultation`、`integration.notification`；实施前完成完整 schema/consumer/routes 对账。
3. Platform Worker 内周期 DB-only orphan scanner（启动时立即执行仅作优化）：claimed/running + expired lease + grace + shield none + 无 E1–E3；按 Project→Run→Task 锁顺序重验，并在同一事务创建 Task Barrier、HIR、Feed notification + recipient Inbox rows、audit；B06-D3 直接回队删除并并入 scanner。
4. 唯一事务内 dispatchability predicate 覆盖所有 Run 创建、claim、recovery、retry、requeue、handoff 和 E6 successor。
5. 修复 Done{Lost} 卷丢失误判；已 started 且证据丢失进入 E5。
6. Interaction terminate/continue 双 adapter：Runner 与 SEM-local CodexExecutor；命令 receipt、result、E3、continuation consumed 分离。
7. SEM-local awaiting-interaction/awaiting-continue 等待期在 Worker restart 后可恢复；shielded Run 不得被 recover_lost 或终态扫描误处理。
8. HIR 最小闭环：创建、查询、责任人、通知、证据、受控处置、审计。
9. 凭据注入/继承/撤销/日志脱敏只读盘点与必要安全修复。
10. Platform Process Registry、active_process_sequence 与 process-bound Lease Renewal fence（Phase 0）；当前 Runner MCP 仅作临时迁移适配，Phase 1 切换至 SEM WSS。
11. 全新空库 Migration、并发锁、唯一约束、故障注入验收。

不包含：SEM 独立主机/WSS 完整部署、Harness Native Resume、跨 Runner handoff UI。

## Phase 1 — SEM Control Plane + Runner E3 + Detached Recovery

交付：
- Platform–SEM HTTPS API、service identity、Outbox/Inbox、幂等 Claim；
- Runner–SEM WSS、一次性 Ticket、connection epoch；
- Runner process evidence spool 与 exit/reconciled/reconnect_sync；
- Runner process spool / E3 evidence 转发；
- 将 Phase 0 临时 Runner MCP process-control adapter 切换至 SEM WSS + Platform HTTPS；
- Detached → Reconciling → Active/Terminated/Quarantined；
- 删除 Runner MCP Claim/command poll/ACK/recovery 控制旁路；
- SEM DB 私有持久化和资源快照/预留。

## Phase 2 — Independent Deployment + Full Interaction Acceptance

SEM 作为独立服务部署在独立 Docker 主机，使用自己的数据库和运行期卷。验收必须包含普通执行、Interaction terminate→回复→continue、SEM-local adapter、Runner adapter、通知/HIR、进程退出证据、Runner/SEM 重启和网络分区。不得只以一条普通任务成功作为通过标准。

## Phase 3 — Session Continuity

完成 Session Manager 逻辑模块和真实 Harness Session 采集。区分 Process Reattachment、Interaction Continuation（Fresh Session）与 Harness Native Resume。Native Resume 默认 false；SEM 不读取远端 Runner 的 CODEX_HOME。

## Phase 4 — Advanced HIR / Cross-Runner Handoff / E6

扩展高级 HIR、supervised 选择、跨 Runner handoff 和 E6 风险授权。基础 HIR/通知/处置必须已在 Phase 0 可用。E6 必须使用正式 Human Consultation 决定和唯一 successor；旧 Barrier 不释放。

## Phase 5 — Multi-Runner Chaos Acceptance

真实多 Runner、多 OS、网络分区、进程身份复用、迟到退出证据、SEM/Platform/RabbitMQ 服务重启、资源压力、撤权和无双重执行混沌验收。

## 全局约束

- RabbitMQ 已存在，为独立单实例服务、无集群。任何 Phase 均不得把它作为 Claim、Lease、Barrier、HIR、E3 或恢复的安全关键依赖。非权威 wake-up 可选项需另行批准。
- Workspace/Worktree/Git Mirror 不做备份；卷丢失不证明进程死亡。
- SEM 不直连 Platform DB；Platform 与 SEM 各自独立数据库。
- Phase 0 未经用户明确授权不得启动。架构评审通过不等于实施授权。


## Process Sequence phase dependency

Platform-side process registry, active sequence pointer and process-bound Lease Renewal must be delivered in Phase 0, alongside the Interaction dual adapters. Phase 0 may use a temporary adapter over the current Runner→Platform MCP control path solely to keep the existing Runner functional during migration. Phase 1 moves that control traffic to Runner→SEM WSS→Platform HTTPS and removes the temporary direct Runner MCP control path. The temporary route must have an explicit removal test and must never remain as a second permanent control plane.
