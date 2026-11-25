# SEM V1 State Machines — SEM-02E

> **Authority-boundary revision (2026-11-25):** This file's Run/Lease/Barrier/Task state machines describe AI-PM-owned business state, not SEM-local state. SEM must not implement or mirror these state machines. SEM has a separate infrastructure Execution lifecycle only. See [authority-boundary-decision.md](./authority-boundary-decision.md).


- 状态：候选规范，Review Pending。
- 跨实体权威定义：[SEM-02E](./sem-02e-architecture-convergence.md)。
- 本文件是状态域唯一登记处；其他文档引用，不得另造状态枚举。

## 1. Run / Lease

- queued → claimed：仅 Platform 原子 Claim 成功。
- claimed → running：执行端启动并登记 Process Identity 成功。
- running → terminal：仅 Platform guarded transition。
- Lease owned → renewed：匹配当前 owner、token proof、generation 和活动 process identity sequence。
- Lease owned → expired：时间到期；仅触发扫描，不证明进程死亡。
- expired + E1/E2/E3：Platform 复核后可释放 Barrier 并安全回收。
- expired + E4/E5：Task Barrier + HIR；不回队、不自动重派。

## 2. Execution Barrier（Task 作用域）

状态：
- active：阻止该 Task 的所有新执行。
- risk_accepted_unconfirmed：E6 已授权唯一 successor，但旧进程仍可能运行，Barrier 仍有效。
- released：仅由 E1/E2/E3 证据经 Platform 事务释放。

禁止：
- HIR closed → released；
- Interaction continued → released；
- lease expired → released；
- Runner offline / Workspace missing / RabbitMQ ACK → released。

一个 Task 同时最多一个 active 或 risk_accepted_unconfirmed Barrier。新 Run 行不能绕过。

## 3. Evidence

| 类别 | 含义 | 转移 |
|---|---|---|
| E1 | 本机 OS wait/exit result，绑定精确 Process Identity | 可申请 Barrier release |
| E2 | OS 证实精确 process identity 不存在且排除 PID reuse | 可申请 Barrier release |
| E3 | 认证 Runner 对已登记进程身份观察到 OS exit | 可申请 Barrier release |
| E4 | 心跳/连接失联超过宽限期 | 只隔离，关联 HIR |
| E5 | Uncertain / identity mismatch / evidence lost | 隔离、告警、HIR |
| E6 | 人类风险接受 | 不释放 Barrier；只授权一个 successor |

E6 不是进程退出证据，不能进入 E1/E2/E3 的证据聚合。

## 4. Process Identity Sequence

Run+Generation 内进程身份按 process_sequence 递增：
- initial_start；
- interaction_continue；
- recovery_restart（仅在明确安全条件下）；
- cleanup / shutdown 等派生动作。

状态：
- registered → active → exited_observed；
- registered/active → unknown（证据缺失或冲突）。

Interaction continue 的 consumed 回执必须登记新 sequence + process identity。旧 sequence 的迟到 exit 只改变旧 identity 状态，不能终结新 sequence。Run 级终止需确认所有已登记且受监督的进程均已退出。

## 5. Runner Detached / Reconnect

Active → Detached → Reconciling → Active / Terminated / Quarantined。

- Active：当前连接、Lease、Generation 与 OS 监督进程一致。
- Detached：连接中断；SEM 持久化执行侧状态。
- Reconciling：同设备重新认证、boot identity、process sequence、spool 和 Platform Lease/Generation 对账。
- Active：精确进程仍活且当前 Lease/Generation 有效，恢复监督。
- Terminated：E1/E2/E3 证明所有相关进程退出。
- Quarantined：E4/E5、身份不匹配或证据不足；Task Barrier + HIR。

Detached/Reconciling 不属于 Interaction scheduling_shield，不增加 CHECK 枚举；SEM 不直连 Platform DB。

## 6. HIR（唯一状态域）

HIR 状态：
- open：已创建、待分派；
- assigned：已指定责任人；
- awaiting_user：等待明确人工动作/决定；
- in_progress：人工处置进行中；
- resolved：处置目标已达成，但尚待关闭核验；
- closed：人工工作项关闭；
- cancelled：经授权取消且原因审计。

允许转移：
- open → assigned / awaiting_user / cancelled；
- assigned → awaiting_user / in_progress / cancelled；
- awaiting_user → assigned / in_progress / cancelled；
- in_progress → awaiting_user / resolved / cancelled；
- resolved → closed / in_progress；
- closed →（仅显式 reopen 操作）open；
- cancelled →（仅显式 reopen 操作）open。

Notification delivery 状态不属于 HIR 状态。HIR 状态变化不隐式改变 Barrier。

## 7. Human Consultation / E6

Human Consultation 复用既有实体的四态与决定四元组、supersede 语义。E6 是其一种 decision type，必须绑定：
- HIR ID；
- Task Barrier ID；
- Task ID；
- operator principal；
- decision timestamp；
- reason + residual risk summary + mitigations；
- 唯一 successor_run_id。

E6 授权事务必须唯一分配 successor；重复或第二个 successor 请求拒绝。授权后 Barrier → risk_accepted_unconfirmed，不能 released。

## 8. Interaction Continuation

Interaction lifecycle 与 HIR 独立。continue:
1. 保存用户回复和 checkpoint；
2. 创建 continuation operation；
3. 执行端注入回复；
4. 启动 Fresh Session / 新进程；
5. 收到 continuation.consumed，登记 process sequence；
6. 恢复正常监督。

Native Resume 默认 false；不得把 Fresh Session continuation 伪装成原生 Session Resume。

## 9. Notification delivery

- `integration.notification` remains the project-level Feed; its existing row-level `read_at` meaning is unchanged.
- `integration.notification_recipient` is the per-principal Inbox membership and delivery-state ledger. One parent notification may have multiple recipient rows.
- A NULL recipient is allowed only for an explicit `unassigned` queue row; it is visible only to authorized project operators and must raise an alert.
- V1 in-app availability is committed with the parent notification in the same Platform transaction as HIR/Interaction. No RabbitMQ delivery is required for visibility.
- Per-recipient `read_at` is separate from the parent Feed `read_at`. Delivery state is not HIR or Interaction state.
- Recipient resolution: active Task assignee first; otherwise active project owner members; if none, create the unassigned row.
- Notification failure cannot silently discard the human work item. External channel retries, if added later, remain non-authoritative and separately tracked.

