# 执行安全：Lease、进程身份、Task Barrier 与安全重派

> **Authority-boundary revision (2026-11-25):** The Task Barrier/HIR/Run safety model described here is AI-PM business authority, not SEM-owned state. SEM may report process/runtime evidence, but AI-PM decides business disposition, retry, barrier and task state. Reconcile the SEM-facing responsibilities against [authority-boundary-decision.md](./authority-boundary-decision.md); this document is not an SEM implementation contract.


- 状态：SEM-02E 候选规范；Review Pending。
- 权威基线：[SEM-02E](./sem-02e-architecture-convergence.md)。
- 核心公理：**Lease expiry ≠ process death。Task Barrier 是新 Run/Generation 无法绕过的执行安全约束。**

## 1. 当前代码风险（历史审计锚点，实施前必须按最新 HEAD 复核）

此前审查识别两条危险路径：
- B3-2：claimed + lease expired + stage started 时，旧逻辑可能直接回收并创建新 Run，未获得原进程终止证据。
- B06-D3：running + lease expired >1h 时，旧逻辑可能直接回队；该路径绕过普通 claim 候选过滤，也未验证 Runner 项目和进程终止事实。

这些引用描述被审查的代码快照，不应被当成当前已修复事实。Phase 0 必须逐个调用点核验并统一改造。

## 2. 终止证据分类

| 类别 | 事实 | 自动释放 Task Barrier / 安全重派 |
|---|---|---|
| E1 | SEM 对精确本机进程身份取得 OS exit/wait 结果 | 允许，需 Platform 事务复核 |
| E2 | 本机 OS 进程查询确认精确 process identity 已不存在，且排除 PID reuse | 允许，需 Platform 事务复核 |
| E3 | 已认证 Runner Supervisor 对已登记精确 process identity 观察到退出并上报 | 允许，需 Platform 事务复核 |
| E4 | Runner/Worker 心跳或连接超过宽限期无响应 | 不允许；隔离并建/关联 HIR |
| E5 | 身份不完整、卷/日志/PID 证据丢失、观察矛盾或无法确认 | 不允许；Uncertain + Barrier + HIR |
| E6 | 有权限人类明确接受旧进程仍可能运行的残余风险 | 不释放 Barrier；仅授权一个精确绑定 successor |

E6 是人工风险授权，不是终止证据，不得列为 E1/E2/E3 子类。

## 3. Task 级 Execution Barrier

Barrier 以 Task 为作用域，至少关联 task_id、origin_run_id、origin_generation、state、evidence、HIR 和审计信息。一个 Task 同时最多一个未释放 Barrier。Run+Generation 是证据来源身份，不是 Barrier 唯一作用域。

- E1/E2/E3：在 Platform 锁定 Task/Run 并重新验证后，释放 Barrier。
- E4/E5：保留 Barrier，Lease 可自然过期，创建/关联 HIR。
- E6：Barrier 进入 risk_accepted_unconfirmed；旧执行可能仍运行，不能标记 released。
- HIR closed、Interaction continued、Cleanup completed、Runner offline、Workspace missing、RabbitMQ ACK 均不能释放 Barrier。

所有新 Run 行创建、Claim、retry/requeue、B3-1/B3-2、B06-D3、orphan reconciliation、handoff 和 E6 successor 必须检查 Task Barrier。

## 4. Phase 0 周期性 Orphan Scanner

Phase 0 在 Platform 侧实现 DB-only 周期扫描，不依赖 Worker 重启、SEM 或 Session Manager。扫描 claimed/running、Lease 过期并超过宽限期、Interaction shield=none、无 E1–E3 的候选。必须覆盖 Runner 项目，不得因 claim 候选过滤而漏扫。

单个 Platform DB 事务内：
1. 按既定锁序锁定 Project → candidate Run → Task；
2. 重验状态、generation、lease、shield、Barrier 和已有证据；
3. 创建/复用 Task Barrier；
4. 创建/关联 HIR；
5. 写审计、`integration.notification` Feed 行与 `integration.notification_recipient` 收件人 Inbox 行；
6. 不把 Run 回 queued，不创建可执行 successor。

扫描必须周期执行、幂等、并发安全、可观测。启动时 reconcile 只能是额外触发器。B06-D3 合并到此机制，不再保留独立回队旁路。

## 5. 统一 Dispatchability

Platform 提供唯一事务内谓词，至少检查 Task Barrier、Interaction shield、Run 状态、Lease/Generation、设备/项目授权、并发约束和 operation 特定限制。所有 Claim/recovery/requeue/new Run/handoff 路径复用它。不能仅在 claimed candidate 阶段检查；queued claim 和 Run 行创建也必须检查 Task Barrier。

## 6. Process Identity 与迟到证据

一个 Run+Generation 可以有多个顺序启动的进程身份。每个身份含 process_sequence、boot_id、PID + OS creation identity、Harness kind、launch reason、started_at、exit observation 和可选 command_id。

Interaction continue 会启动新进程并登记新序列。旧进程的迟到退出证据只能关闭旧 process observation，不能将新进程或整个 Run 判定为已退出。Run+Generation 只有在所有受监督进程均有可信终止结论后才能进入可安全回收状态。

## 7. Workspace 与卷丢失

Workspace/Worktree/Git Mirror 不做备份。卷丢失、PID 文件丢失、logs 丢失不证明进程死亡。若 Run 曾 stage=started 且无法重建 OS process identity，必须 E5/Uncertain，禁止 Done{Lost} 后自动重派。新 successor 使用独立 Workspace；旧 Workspace 只有在进程终止证据或受控隔离条件满足后才清理。

## 8. 外部副作用与凭据

Generation fencing 只能保护 Platform DB 写入，不能撤销进程内 Git/SSH/API 凭据或阻止已经发出的外部请求。Phase 0 必须审计凭据注入/继承、最小权限、TTL、撤销、日志脱敏、临时文件权限及旧进程仍运行时的外部副作用。E6 风险展示必须包含 Git push、外部 API、LLM 费用等残余风险。

## 9. RabbitMQ 边界

RabbitMQ 是环境中已存在的独立单实例服务，没有集群。执行安全不依赖其可用性；消息 ACK 不构成业务提交、进程退出、E3 或 Barrier 释放。若未来用于非权威唤醒，DB polling/Outbox retry 必须可独立恢复。

## 10. Phase 0 验收

- 活进程 + Lease 过期：任何路径都不得派发第二进程。
- Worker 不重启时周期 scanner 仍能发现孤儿。
- Runner 项目不被排除在 scanner 之外。
- 新 Run 行无法绕过旧 Task Barrier。
- Interaction continue 后旧进程迟到退出证据不能关闭新进程。
- 卷/PID/log 丢失 → E5，不得 Done{Lost} 重派。
- E6 仅授权唯一 successor，旧 Barrier 保持 risk_accepted_unconfirmed。
- 并发 claim/requeue/orphan scan 通过数据库约束和锁序保持唯一执行权。
