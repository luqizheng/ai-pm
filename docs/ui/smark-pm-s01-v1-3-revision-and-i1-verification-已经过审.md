# S01 基线 V1.3 事实修订与 I1 并发边界核验——给专家的反馈

> 日期：2026-11-03
> 依据：**以实际代码为准**（migrations 全量 + `crates/pm-application` / `crates/pm-infrastructure` / `crates/pm-mcp` / `apps/*` 源码逐行核验，非文档推定）。
> 结论先行：**同意你的判断——T-032 D3 取消语义不再争论；I1 剩余重点是归档门禁与并发一致性。** 三个并发位置的核验答案都是「现状无保护」，另有 4 个设计稿需要修正的事实点。

---

## 一、八项修订已同步进 S01 基线 V1.3 正文

| # | 工作包 | 修订位置与内容 |
|---|---|---|
| 1 | I1 | §1.3/§1.4/§1.6 改为 T-032 D3 口径：先撤凭据（终态+清 `lease_token_hash` 原子落库）→ 后进程终止（资源清理）；验收项改为「不等待进程终止确认；终止失败不回滚终态」 |
| 2 | I2 | §2.4 补：`discovery_finding` 无 version/revision、无 decided_by/decided_at/updated_at；确认人时间权威落点=`governance.audit_log`；F3=kind 与 verification_status 两列一次原子更新 |
| 3 | I3 | §3.5 删除 auto_read/auto_write/auto_commit；改用实际字段 auto_accept / auto_merge / auto_publish_knowledge / require_human_approval / additional_checks，并明确它们**不是 Git 授权的表达** |
| 4 | I3 | §3.3 补：`task_run.policy_version` 为 INT（0010），与 D2.1 SHA-256 前缀内容寻址存在类型/语义缺口，兼容方案在 I3 单独设计 |
| 5 | I3 | §3.6 拆两行：write_access 读取链路已定位=EXISTING；执行期强校验=VERIFY（未证实） |
| 6 | I6 | §5.2 改 project_id 主口径；repo_import 是项目级作业表、仓库关联走 `repo_import_source(import_id, repository_id)`、Commit SHA=`repo_import_source.head_commit_sha` |
| 7 | I6 | §5.5 验收项改为「并发分析由 `repo_import_one_in_progress` 唯一索引在数据库层拒绝」，删除「应用层识别当前作业」口径 |
| 8 | I6 | §5.2 补：仓库连接事实（project_repository.status/head_commit_sha/last_verified_at/last_error_code）与分析作业结果（repo_import/repo_import_source）分属两个数据模型，分别表达 |

另有一项**主动修正**（不在你列的八项内）：V1.2 Q4「代码只写入 planning」**有误**——`project_import` 流程实际写 `planning → initializing`（lifecycle.rs:127），applied 时回 planning（advance.rs:73），且 **cancel_import 不回滚 initializing**（取消导入后项目卡在 initializing）。这是现有唯一项目中间态先例，且自带卡死缺陷——恰好为「不引入 archiving 中间态」提供了反面佐证。已入基线附录 A.4。

---

## 二、你提出的三个并发位置：核验答案（全部「现状无保护」）

### 1. Run 创建事务：无项目状态检查、与归档无共同锁

Run 创建共三个入口，全部不查 `core.project.status`：

| 入口 | 代码位置 | 事务内锁/检查 |
|---|---|---|
| TaskReady → delivery run | `pm-infrastructure/outbox_handlers.rs:41-129`（TaskReadyHandler） | 锁 `delivery.task` 行 FOR UPDATE；查 task.status='ready' + assignee；幂等三处（活跃 run 计数 / `uq_task_run_source_event` / 23505 降级）。**无项目状态谓词** |
| DeliverySubmitted → review run | `pm-application/review/queue.rs:294`（enqueue_review） | 同上，INSERT review run，**无项目状态谓词** |
| 共享创建封装 | `pm-application/run_dispatch/port.rs:30`（insert_task_run，B3-6） | 前置仅要求 task 行锁（B2）；INSERT 里的 core.project 子查询**只取 tenant_id** |

→ 归档若只 UPDATE project 行，与 Run 创建事务**无锁交集**：归档提交后，在途/后到的 TaskReady 事务照样建 Run。全仓检索证实：`core.project.status` 的消费者为零（读路径仅展示；写路径仅 import 流程）。

### 2. Worker claim：数据库层完全不关联项目状态

- `claim_core`（`run_dispatch/claim.rs:258-300`）候选 SQL 只筛 run 状态 + run_kind + 时间窗（`FOR UPDATE OF r SKIP LOCKED`），随后锁 task 行查 task 状态——**全程无 core.project 联结**。
- MCP Model A（`pm-mcp/tools.rs:663 claim_task` → `claim_task_run_as`）走同一 `claim_core` 模板；前置 `authorize` 是**真人成员门（T-051）**，不是项目状态门。
- 调度循环在 `apps/ai-pm-worker/main.rs:110-168`（claim_and_start + poll + cleanup；启动时一次 reconcile_orphans）。

### 3. Outbox Handler：已入队执行消息归档后仍会启动执行

- outbox 领取 `FOR UPDATE SKIP LOCKED`、at-least-once、失败退避重试至 max_attempts 死信（`pm-infrastructure/outbox.rs:112` / `outbox_worker.rs:114-140`）。
- 已入队的 **TaskReady** 事件归档后仍会被 TaskReadyHandler 消费并建 delivery run；**DeliverySubmitted** 仍会 enqueue review run；死信恢复路径同样无项目状态检查。
- 即：**归档时刻的「尚未消费」事件 = 归档后的新活动来源**。

**门禁设计含义**（供 I1 阶段 B）：三类入口（Run 创建 ×2、claim ×2 模式、outbox handler ×2）必须在**数据库层**与归档形成共同锁。最小侵入方案：在 insert_task_run / enqueue_review / claim_core / handler 的事务内，对 `core.project` 行 `SELECT ... FOR UPDATE` 并断言非 archived——归档 UPDATE 与创建 SELECT 锁同一行，天然串行化，无竞态窗口。

---

## 三、I1 设计稿 V1.1 需要修正的 4 个点（以代码为准）

| # | 设计稿位置 | 问题 | 修正建议 |
|---|---|---|---|
| P1 | §3.4「已排队、尚未领取：按现有取消协议撤销或取消」 | **不准确**：`cancel_run` 只认 claimed/running（run_executor.rs:347），queued Run 无现成取消路径。`run_dispatch/lease.rs:171` 的内部 cancel_run 是无条件 UPDATE（无并发谓词），仅用于 task_missing/task_terminal | 归档需为 queued Run 补取消路径：条件 UPDATE `queued → cancelled` 带 release_reason='project_archived'，避免与 claim 的 `FOR UPDATE SKIP LOCKED` 竞态（claim 锁 run 行后再断言项目状态即可） |
| P2 | §3.3/§6.2（隐含 cancel_run 可直接复用） | `cancel_run` 目前**无任何外部入口**（仅测试与 claim 内部调用）；`record_termination` 的 reason_code 硬编码四值（user_cancel/timeout/agent_heartbeat_lost/progress_stalled） | 归档是 cancel_run 的第一个正式业务调用方：需扩展 reason（如 project_archived）；cancelled 终态不插 failure 行的现状行为正确，保持 |
| P3 | §3.5「终止失败时保留可重试信息」 | 现状=隐式时间窗轮询：`cleanup_finished_processes` 只清 `finished_at > now()-1h` 且 LIMIT 64（run_executor.rs:455-464）。**超窗终态残留进程无任何后续清理路径**（reconcile_orphans 只扫 claimed/running） | I1 验收补一条：超窗残留告警（或扩窗/显式清理队列表），否则「兜底」在 1 小时后失效 |
| P4 | §2.1/§3.1（项目状态现状描述） | 需补 initializing 事实与 cancel_import 卡死缺陷（见上文「主动修正」） | §3.1 论证从「没有中间态先例」改为「现有唯一中间态（initializing）已带卡死缺陷，更不应再加」 |

## 四、核验通过项（可直接引用为 EXISTING）

- **V1** `record_termination` 单事务：终态 + 清 lease 三件套（owner/expires/token_hash）+ failure 行 + audit_run + RunCompleted 事件（幂等键 `run_completed:<run_id>`）同事务提交（run_executor.rs:693-764）。
- **V2** 并发：条件 UPDATE 返回 None → 空提交返 false，第二到达者零副作用；与 finish_run / recover_lost 共用同一谓词，先到先得。
- **V4** terminate 内部先 try_wait 不杀尸体；凭据与进程分离的窗口内，进程侧 MCP 调用全被拒。
- **P5 撤权即时性已证实**：`run_context.rs::verify_row`（121-153）每次调用校验 token hash + generation + status(claimed/running) + 租约 + deadline——cancel 落库后，后续请求立即 Forbidden。I1 设计稿 §1.3 的这条担忧已被现有实现覆盖，无需新增机制。
- **V10** 归档审计可复用 `governance.audit_log`（先例 action='project_import_applied'，含 actor 五列 + after_data）。

## 五、结论

1. T-032 D3 语义冻结，I1 不再讨论取消协议方向。
2. I1 阶段 A 核验已完成（本反馈即证据矩阵），**门禁缺口=全部三类入口零项目状态检查**，阶段 B 需给出数据库层共同锁方案。
3. I1 设计稿 V1.1 按 P1~P4 修订后可进入阶段 B 定稿。
