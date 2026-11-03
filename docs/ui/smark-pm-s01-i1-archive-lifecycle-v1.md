# I1 · 项目归档生命周期设计稿 V1.0（最终定稿）

> 文档性质：实施前设计基线
> 设计范围：Project 归档、项目活动门禁、Run 生命周期联动、Executor 清理
> 状态：**最终设计定稿，待拆分实施任务**
> 设计原则：先关闭项目活动入口，再撤销已有执行凭据，最后完成进程资源清理
> 上位约束：T-032 D3 Run 取消协议
> 日期：2026-11-03

> **编者按**：本稿吸收了本轮代码核验反馈（`smark-pm-s01-v1-3-revision-and-i1-verification.md`）的全部结论——§1 的代码事实（含 initializing 卡死、queued 无取消路径、cancel_run 无外部入口、清理 1 小时窗口等）均与实际代码核验一致；并发方案采用「Project 行门闩 + 统一锁顺序 + 两阶段 claim」；新增 Executor 启动握手协议（§8）为本稿新设计。

---

## 0. 设计目标与边界

### 0.1 设计目标

I1 为 Project 建立统一的归档生命周期。

归档后必须保证：

1. 项目不能再产生新的业务活动。
2. 所有 Run 创建入口均不能绕过项目归档状态检查。
3. Worker 不能再领取归档项目的 Run。
4. 归档前已经创建的 queued Run 必须被取消。
5. 归档前已经 claim 或 running 的 Run 必须撤销执行凭据。
6. Executor 进程必须进入可追踪、可重试的资源清理流程。
7. 尚未消费的 Outbox 消息不能在归档后重新创建 Run。
8. 死信重放不能绕过归档门禁。
9. 归档操作必须有完整审计记录。
10. 历史 Task、Run、交付记录、知识记录仍可查询。

### 0.2 不在本设计范围内

本设计不负责：

- Project 删除及物理数据清理。
- Project 恢复归档（如未来需要恢复，应另行设计恢复协议）。
- Task、Repository、Knowledge 等领域自身的完整生命周期。
- Run 的一般性取消功能重构。
- Executor 执行引擎整体重构。
- 项目归档后的历史数据迁移。

### 0.3 核心定义

归档是不可逆的项目生命周期转换：

`active → archived`

归档不是暂停项目，也不是仅仅隐藏项目。

归档之后，除明确允许的管理、审计、历史读取和资源清理操作外，所有项目级业务写操作均应被拒绝。

---

## 1. 现有代码事实与设计依据

本节区分已经核验的事实与新增设计。

### 1.1 Project 状态

**已核验事实：**

- 现有项目状态存储在 `core.project.status`。
- 全仓检索未发现其他业务消费者读取该状态作为活动门禁。
- 当前 `core.project.status` 没有 CHECK 约束保证完整状态枚举。
- 项目导入流程会写入 `planning → initializing`，applied 后返回 `planning`。
- `cancel_import` 不会回滚 `initializing`，可能导致项目停留在中间态。

相关代码：

- `lifecycle.rs:127`
- `advance.rs:73`

**设计裁决：**

归档采用直接终态转换：

`active → archived`

不新增 `archiving` 中间态。

不复制现有 `initializing` 状态缺乏完整回滚路径的问题。

实施前必须核实当前项目状态的实际合法值，并明确哪些状态允许归档。不得未经核验直接假设所有项目均为 `active`。

### 1.2 Run 创建入口

已核验存在以下创建入口：

| 入口 | 代码位置 | 当前缺口 |
|---|---|---|
| TaskReady → Delivery Run | `pm-infrastructure/outbox_handlers.rs:41-129` | 无 Project 状态谓词 |
| DeliverySubmitted → Review Run | `pm-application/review/queue.rs:294` | 无 Project 状态谓词 |
| 共享 Run 创建 | `pm-application/run_dispatch/port.rs:30` | 只通过 Project 子查询取得 tenant_id，没有状态门禁 |

TaskReady 当前会锁 Task 行、检查 Task 状态和 assignee，并通过活跃 Run 计数、唯一索引和 23505 降级处理幂等。

但这些机制不能阻止归档后创建新 Run。

**设计裁决：**

所有 Run 创建入口必须统一经过 Project 活动门禁。

不得只在 UI、MCP 或某一个 Outbox Handler 中增加状态检查。

### 1.3 Worker claim

已核验：

- `claim_core` 位于 `run_dispatch/claim.rs:258-300`。
- 当前候选 SQL 主要筛选 Run 状态、run_kind 和时间窗。
- 使用 `FOR UPDATE OF r SKIP LOCKED`。
- 随后锁 Task 并检查 Task 状态。
- 当前没有联结 `core.project`。
- MCP `claim_task` 通过 `claim_task_run_as` 复用同一个 `claim_core`。
- MCP 前置 `authorize` 是真人成员授权门，不是 Project 状态门。

**设计裁决：**

Project 活动门禁必须进入共享 `claim_core`，不能只在 Worker 调度循环中检查。

### 1.4 Outbox

已核验：

- Outbox 使用 `FOR UPDATE SKIP LOCKED` 领取。
- 投递采用 at-least-once。
- 失败会退避，达到 max_attempts 后进入死信。
- 归档后未消费的 TaskReady 消息仍可能创建 Delivery Run。
- 归档后未消费的 DeliverySubmitted 消息仍可能创建 Review Run。
- 死信重放同样没有 Project 状态门禁。

**设计裁决：**

Outbox Handler 必须执行 Project 活动门禁。

归档后的业务消息应被视为合法跳过，而不是永久重试错误。

### 1.5 T-032 D3 Run 取消协议

已核验：

`run_executor.rs::cancel_run` 遵循 T-032 D3：

**先撤销凭据，再终止进程。**

`record_termination` 使用条件 UPDATE：

`status IN ('claimed', 'running')`

单事务提交内容包括：

- Run 终态更新。
- Lease 三件套清理：owner、expires、token_hash。
- Failure 记录（适用时）。
- `audit_run`。
- `RunCompleted` 事件，幂等键为 `run_completed:<run_id>`。

条件 UPDATE 返回 None 时，不产生第二次副作用。

`finish_run`、`recover_lost` 与其共享终态竞争谓词。

`run_context.rs::verify_row` 每次调用都会校验：

- token hash；
- generation；
- Run status；
- lease；
- deadline。

取消落库后，后续 MCP 请求会被拒绝。

其他事实：

- cancelled 终态不插入 failure 行，这是正确行为。
- `cancel_run` 当前没有正式外部业务入口。
- 当前取消原因硬编码四值：`user_cancel`、`timeout`、`agent_heartbeat_lost`、`progress_stalled`。
- 当前 `cancel_run` 只处理 claimed/running。
- queued Run 没有通用取消路径。
- `run_dispatch/lease.rs:171` 存在另一个内部取消实现，但没有并发谓词，不适合归档使用。

**设计裁决：**

归档复用 T-032 D3 凭据撤销协议，不另造一套 claimed/running 取消语义。

新增取消原因：

`project_archived`

queued Run 使用独立的条件取消路径。

### 1.6 Executor 清理

已核验：

- `cleanup_finished_processes` 仅清理 `finished_at > now()-1h` 的终态进程。
- 单次清理 LIMIT 64。
- 超过一小时的终态残留进程没有完整后续清理路径。
- `reconcile_orphans` 只扫描 claimed/running。

**设计裁决：**

不能将当前一小时清理窗口视为完整兜底。

归档必须为进程清理建立可持久化、可重试、可告警的机制。

---

## 2. Project 生命周期设计

### 2.1 状态转换

归档只允许：

`active → archived`

`archived` 为终态。

归档操作应满足：

- 重复归档具备幂等性。
- 已归档项目再次归档不得重复生成副作用。
- 非法状态不得直接归档。
- 归档失败时，不得出现 Project 已归档但审计记录缺失的情况。

### 2.2 不增加 archiving 状态

本设计明确不引入 `archiving`。

原因：

1. 归档的首要目标是立即关闭活动入口。
2. 现有 Project 状态消费者尚未形成完整生命周期协议。
3. 现有 `initializing` 已经出现取消后无法回滚的问题。
4. 归档后的 Run 清理可以通过持久化后台工作完成，不需要让 Project 停留在一个含义不清的中间状态。

因此，Project 的归档状态转换必须在短事务内完成。

### 2.3 归档后的允许操作

允许：

- 查询 Project 历史信息。
- 查询历史 Task、Run 和交付记录。
- 查询历史 Knowledge。
- 查询审计日志。
- 查询归档清理进度。
- 执行归档资源清理。
- 管理员查看归档失败原因。

禁止：

- 创建 Task。
- 修改 Task 业务内容或状态。
- 创建新的 Run。
- 创建 Review Run。
- 创建新的 Delivery Run。
- 发起新的 Repository 分析。
- 修改 Project Goal。
- 写入新的项目业务 Knowledge。
- 修改项目 Repository 业务配置。
- 通过 Outbox 重放重新触发项目业务活动。
- 通过 MCP 绕过以上限制。

具体写操作应按领域逐项接入 Project 活动门禁。

不能仅凭 UI 隐藏按钮实现禁止。

---

## 3. 并发控制总原则

### 3.1 Project 行是活动门闩

Project 行是项目级活动是否允许继续的权威判断点。

所有会创建项目活动或领取项目 Run 的事务，必须与归档事务通过同一 Project 行锁串行化。

统一锁顺序：

`core.project → delivery.task_run → delivery.task → 其他关联记录`

不得在已经持有 Task 锁或 Run 锁后，再反向申请 Project 锁。

### 3.2 为什么不能简单追加 Project 锁

现有实现中：

- TaskReady 创建 Run 先锁 Task。
- Worker claim 先锁 Run，再锁 Task。

如果直接在原有事务末尾追加 Project 锁，可能出现：

- 事务 A 持有 Task，等待 Project。
- 事务 B 持有 Project，等待 Task。

形成锁等待环。

因此必须调整事务获取锁的顺序，而不是只增加一个状态查询。

### 3.3 通用门禁协议

所有项目活动事务遵循：

1. 通过非锁定只读查询取得 `project_id`。
2. 锁定对应 `core.project` 行。
3. 检查 Project 状态为允许活动的状态。
4. 再锁定后续 Run、Task 等业务行。
5. 重新验证前置查询得到的关联关系。
6. 在同一事务中执行写操作。
7. 提交事务。

第 5 步不可省略。

初始只读查询只用于发现 Project，不代表获得了任何写入授权。

---

## 4. Run 创建协议

### 4.1 TaskReady → Delivery Run

修改 `pm-infrastructure/outbox_handlers.rs`。

事务流程：

1. 根据 Task ID 只读取得 `project_id`。
2. 锁定 `core.project`。
3. 检查 Project 为 active。
4. 锁定 Task。
5. 重新检查 Task 的 `project_id`。
6. 检查 Task 仍为 ready。
7. 检查 assignee。
8. 执行现有活跃 Run 幂等检查。
9. 插入 Delivery Run。
10. 提交事务。

归档事务与该流程竞争同一 Project 行锁。

如果创建事务先取得 Project 锁并成功提交，归档事务随后归档；该 Run 属于归档前已存在的 Run，后续进入归档清理。

如果归档事务先取得 Project 锁并提交，创建事务随后检查到 archived，必须合法跳过。

### 4.2 DeliverySubmitted → Review Run

修改 `pm-application/review/queue.rs`。

流程：

1. 只读取得 Task 对应的 `project_id`。
2. 锁 Project。
3. 检查 Project active。
4. 锁定 Task。
5. 重新验证 Task 与 Project 的关联。
6. 执行现有 Review Run 创建逻辑。
7. 提交。

不得绕过共享 Project 活动门禁。

### 4.3 共享 Run 创建封装

修改 `insert_task_run` 或其上游事务接口。

共享创建封装必须确保调用方已经在同一事务内取得 Project 行锁并通过状态检查。

不能仅在 `insert_task_run` 内做一次不带锁的 Project 状态查询。

如果封装无法可靠保证调用方持锁，应调整接口，使其不能被不符合协议的调用方直接使用。

### 4.4 创建失败语义

归档状态导致的 Run 创建跳过是合法业务结果。

不得将其转化为：

- 无限重试；
- Outbox 死信；
- 重复创建；
- 业务失败告警。

应有明确、可观测的跳过原因，例如 `project_archived`。

---

## 5. Worker claim 协议

### 5.1 设计目标

保证：

- 归档后不能领取新的 Run。
- 归档前已经成功 claim 的 Run 可以被归档取消。
- Worker 和 MCP 使用相同规则。
- 不因新增 Project 锁引入反向锁顺序。
- `SKIP LOCKED` 语义得到保留。

### 5.2 两阶段候选方案

调整 `claim_core`：

**阶段 A：只读发现候选 Project**

只读查询可能符合 claim 条件的 Run，取得候选 `project_id`。

这一步不锁 Run，不代表获得 claim 权限。

**阶段 B：按统一锁顺序执行 claim**

对候选 Project：

1. 锁定 `core.project`。
2. 检查 Project active。
3. 在该 Project 范围内重新查询候选 Run。
4. 对 Run 使用 `FOR UPDATE SKIP LOCKED`。
5. 锁定 Task。
6. 重新验证 Task 状态、关联关系和 Run claim 资格。
7. 执行 claim。
8. 提交事务。

阶段 B 必须重新查询候选 Run，不能直接使用阶段 A 的结果完成 claim。

如果 Project 已归档，跳过该 Project 的候选 Run，继续寻找其他 Project。

### 5.3 MCP 与 Worker 一致性

MCP：

`claim_task → claim_task_run_as → claim_core`

继续复用同一个 `claim_core`。

真人成员 `authorize` 仍然是调用人权限门，不替代 Project 活动门禁。

### 5.4 事务要求

程序员必须根据实际 SQL 验证：

- 所有 claim 分支都先锁 Project。
- 不存在先锁 Run 再锁 Project 的分支。
- 阶段 A 不会持有跨事务锁。
- 阶段 B 的候选查询能处理候选 Run 被其他 Worker 抢占的情况。
- 多 Project 候选遍历不会产生饥饿。
- `SKIP LOCKED` 仍能正常工作。

不能仅凭流程图宣称该方案已经不存在死锁。

---

## 6. 归档事务

### 6.1 事务职责

归档事务只负责关闭活动入口和持久化后续处理任务。

不得在 Project 行锁下等待 Executor 进程终止。

### 6.2 事务步骤

单个短事务：

1. 校验调用人具备 Project 归档权限。
2. 锁定 `core.project`。
3. 检查当前状态。
4. 如果已经 archived，返回幂等结果。
5. 校验当前状态允许归档。
6. 更新 Project 状态为 archived。
7. 写入 `governance.audit_log`。
8. 持久化归档后续处理任务。
9. 提交事务。

第 6、7、8 步必须处于同一个事务。

如果审计或后续处理任务写入失败，整个归档事务回滚。

### 6.3 后续处理任务

建议建立明确的持久化归档处理记录。

逻辑上至少包含：

- project_id；
- archive_id 或幂等标识；
- 状态；
- 创建时间；
- 开始时间；
- 完成时间；
- 最后错误；
- 重试次数。

状态建议：

`pending → processing → completed`

失败可进入可重试状态，并记录下一次执行时间。

实际可以复用现有持久化任务设施，但必须证明：

- 归档事务提交后工作一定可恢复；
- Worker 重启后可以继续；
- 重复消费不会产生重复副作用；
- 死信后有明确恢复或人工处理路径。

不得只依赖进程内队列。

### 6.4 归档事务不做的事情

归档事务不：

- 遍历所有 Run 并逐个取消；
- 等待 Executor 退出；
- 杀死子进程；
- 执行长时间清理；
- 等待所有后台工作完成。

这些工作由持久化后续处理流程完成。

---

## 7. Run 归档取消协议

### 7.1 Run 状态分类

归档处理必须区分：

| Run 状态 | 处理方式 |
|---|---|
| queued | 新增专用条件取消路径 |
| claimed | 复用 T-032 D3 凭据撤销协议 |
| running | 复用 T-032 D3 凭据撤销协议 |
| completed | 保持不变 |
| failed | 保持不变 |
| cancelled | 保持不变 |

不得重写已经完成的历史 Run。

### 7.2 queued Run

当前 T-032 D3 `cancel_run` 不支持 queued。

因此新增归档专用 queued 取消路径。

条件更新：

`status = 'queued' → 'cancelled'`

同时写入：

`release_reason = 'project_archived'`

要求：

- 使用条件 UPDATE。
- 与 claim 竞争时先到先得。
- 不创建 failure 行。
- 保留 Run 历史。
- 写入必要审计。
- 重复执行无额外副作用。

如果 claim 已经成功将 Run 从 queued 转为 claimed，queued 取消更新不得覆盖它，必须交给 D3 取消流程处理。

### 7.3 claimed/running Run

复用 T-032 D3。

扩展取消原因：

`project_archived`

取消时：

1. 条件更新 Run 终态。
2. 清除 lease owner。
3. 清除 lease expires。
4. 清除 lease token hash。
5. 写入审计。
6. 产生幂等 RunCompleted 事件。
7. 提交事务。
8. 后续执行进程终止。

必须保持：

**先撤销凭据，再终止进程。**

不得等待进程终止成功才提交 Run 终态。

进程终止失败不得回滚 Run 终态。

### 7.4 并发竞争

归档、claim、finish、recover_lost、cancel 可能并发。

统一依靠：

- Project 行锁；
- Run 条件 UPDATE；
- D3 终态竞争谓词；
- lease 凭据撤销；

保证只有一个终态转换能够产生有效副作用。

不得采用先读取状态、再无条件 UPDATE 的方式处理归档取消。

---

## 8. Executor 启动竞态与进程登记协议

### 8.1 问题

Worker claim 事务提交后，到 Executor 子进程真正启动之间存在时间窗口。

可能发生：

1. Worker claim 成功。
2. Claim 事务提交。
3. Project 被归档。
4. Run 被取消，lease 凭据被撤销。
5. Worker 仍然启动刚才准备的子进程。

`run_context.verify_row` 能阻止取消后的 MCP 调用，但不能单独保证本地子进程不会启动或执行本地副作用。

因此，仅增加 MCP 凭据校验不够。

### 8.2 设计裁决

采用**持久化启动意图 + 进程登记 + 启动门闩**协议。

核心原则：

子进程在获得有效启动授权之前，不得开始实际 Agent 工作。

### 8.3 启动流程

**阶段 A：登记启动意图**

Worker 在准备启动 Executor 前，为本次启动创建唯一 session_id，并持久化启动意图。

启动意图至少关联：

- run_id；
- session_id；
- generation；
- 启动状态；
- 创建时间。

同一个 Run 的启动意图必须具备幂等性。

**阶段 B：启动受控子进程**

Worker 创建子进程。

子进程启动后必须停留在等待状态，不得立即开始 Agent 工作。

**阶段 C：登记进程身份**

Worker 将 PID、进程启动标识、session_id 等必要信息持久化。

登记必须采用条件更新，确认：

- Run 仍然处于允许启动的状态；
- generation 未变化；
- session_id 仍然有效；
- 启动意图未被取消；
- Project 仍允许该启动继续。

登记失败时，Worker 必须终止刚创建的子进程。

**阶段 D：启动授权**

只有进程身份登记成功且启动授权条件仍成立，Worker 才能释放启动门闩。

如果归档在启动意图登记后、进程登记前发生：

- 归档处理将启动意图标记为取消；
- 后续 PID 登记必须失败；
- Worker 发现失败后立即终止子进程。

如果归档在 PID 登记后发生：

- 归档处理可以根据持久化进程身份定位进程；
- 先撤销 Run 凭据；
- 再终止进程。

### 8.4 启动协议验收要求

必须覆盖以下竞态：

1. 归档发生在启动意图创建前。
2. 归档发生在启动意图创建后、spawn 前。
3. 归档发生在 spawn 后、PID 登记前。
4. 归档发生在 PID 登记后、启动门闩释放前。
5. 归档发生在 Executor 已经开始执行后。
6. Worker 在任意阶段崩溃后重启。

必须证明：

- 不会出现无法追踪的迟到子进程。
- 未取得启动授权的子进程不能开始实际工作。
- 归档后已撤销凭据的进程不能继续通过 MCP 执行平台操作。
- Worker 重启后能够发现未完成的启动意图并处理。

如果现有 `executor_session` 已能承载上述信息，可以扩展；否则需要新增持久化结构。

不能把进程 PID 只保存在 Worker 内存中。

---

## 9. Executor 进程清理

### 9.1 清理原则

Run 终态与进程资源清理是两个独立生命周期。

Run 取消成功不等于进程已经退出。

进程退出失败不允许恢复 Run 的执行凭据。

### 9.2 持久化清理状态

必须存在持久化清理记录，或者证明现有 Executor Session / Outbox 结构可以完整承担同等职责。

至少具备：

- run_id；
- session_id；
- 进程身份；
- 清理状态；
- 尝试次数；
- 下次重试时间；
- 最后错误；
- 完成时间。

建议状态：

`pending → cleaning → completed`

失败时进入可重试状态。

达到重试阈值后进入需要人工处理的状态，并持续可查询。

### 9.3 清理 Worker

清理 Worker 应：

1. 查询待清理记录。
2. 获取清理任务的并发执行权。
3. 根据进程身份检查进程是否存在。
4. 终止仍然存在的目标进程。
5. 确认进程已退出。
6. 更新清理记录为 completed。
7. 记录失败原因并安排重试。

必须防止 PID 被操作系统复用后误杀其他进程。

因此，进程身份不能只依赖裸 PID，应结合进程启动时间或等价的稳定身份信息。

### 9.4 超过一小时的残留进程

现有 `cleanup_finished_processes` 的一小时窗口和 LIMIT 64 不能作为完整清理保障。

必须补充：

- 全量可恢复扫描；
- 超期清理告警；
- 多次失败后的人工处理入口；
- Worker 重启后的恢复能力。

### 9.5 归档完成定义

Project 归档状态可以在归档事务提交后立即生效。

但归档后台处理必须单独展示进度。

至少区分：

- 已关闭活动入口；
- Run 凭据撤销处理中；
- Executor 清理处理中；
- 清理完成；
- 清理异常，待处理。

不能把 Project archived 直接解释成所有子进程已经退出。

---

## 10. Outbox 归档门禁

### 10.1 业务事件

以下事件在 Project 已归档时必须合法跳过：

- TaskReady → Delivery Run。
- DeliverySubmitted → Review Run。

Handler 必须在创建 Run 前执行统一 Project 活动门禁。

### 10.2 合法跳过语义

归档导致的跳过：

- 不创建 Run；
- 不产生业务重试；
- 不进入死信；
- 不重复产生副作用；
- 保留必要的可观测记录。

Handler 必须向 Outbox Worker 返回可被识别的成功处理结果，使消息正常 ACK。

不能把 `project_archived` 当成普通异常抛出，导致 at-least-once 重复投递。

### 10.3 非业务清理事件

归档后仍必须允许：

- Run 取消处理；
- Executor 终止处理；
- Executor 清理重试；
- 审计和归档状态更新。

这些事件属于归档管理和资源回收，不受普通 Project 业务写门禁阻断。

### 10.4 死信重放

死信重放不能绕过 Handler 的 Project 活动门禁。

归档后重放 TaskReady 或 DeliverySubmitted，仍必须合法跳过。

---

## 11. 审计设计

### 11.1 审计存储

复用：

`governance.audit_log`

现有 `project_import_applied` 已提供 actor 五列和 after_data 的审计先例。

### 11.2 归档审计事件

建议 action：

`project_archived`

记录至少包括：

- tenant_id；
- project_id；
- actor 身份；
- 操作时间；
- 原状态；
- 新状态；
- archive_id 或幂等标识；
- 归档原因（如果有）。

### 11.3 Run 取消审计

Run 取消继续沿用 D3 的 `audit_run`。

取消原因：

`project_archived`

queued Run 的专用取消路径也必须有可追踪的审计记录。

### 11.4 清理审计

清理过程应能追踪：

- 清理开始；
- 清理成功；
- 清理失败；
- 重试；
- 最终需要人工介入。

不要求每次低层轮询都写审计日志，但必须能够还原完整处理结果。

---

## 12. 幂等性与故障恢复

### 12.1 Project 归档

同一 Project 重复归档：

- 不重复生成归档副作用；
- 不重复创建互相独立的归档处理任务；
- 返回稳定的归档结果。

### 12.2 Run 取消

同一 Run 重复取消：

- 只允许一次有效终态转换；
- 不重复插入 failure；
- 不重复产生有效终态事件；
- 不恢复 lease 凭据。

### 12.3 Executor 清理

同一 session 重复清理：

- 目标进程已退出时视为清理成功；
- 不因重复执行造成额外副作用；
- 不误杀 PID 复用后的其他进程。

### 12.4 Worker 崩溃

以下位置崩溃后都必须可恢复：

- 归档事务提交后；
- Run 取消事务提交后；
- 启动意图创建后；
- 子进程 spawn 后；
- PID 登记后；
- 进程终止后、清理状态更新前。

恢复逻辑不得依赖原 Worker 进程仍然存在。

---

## 13. 权限与安全边界

### 13.1 归档权限

归档必须由有权限的真人调用。

Agent 不得自行归档 Project。

具体角色与权限映射沿用现有授权体系，不在 I1 中新建平行权限模型。

### 13.2 MCP

MCP 入口必须遵守 Project 活动门禁。

即使调用人已经通过 T-051 真人成员授权，也不能在 archived Project 上继续发起业务写操作。

### 13.3 Worker

Worker 的内部身份不能成为绕过 Project 门禁的理由。

所有 Run 创建与 claim 都必须执行相同的服务端规则。

### 13.4 管理员例外

管理员可以执行归档管理和资源清理操作，但不能借此重新激活归档项目的普通业务活动。

---

## 14. 数据库与接口实施要求

本节只规定设计要求，不预先断言新增字段或表已经存在。

### 14.1 Project 状态

实施前必须核实：

- 当前所有合法 Project 状态；
- 状态写入入口；
- 状态读取入口；
- 哪些状态允许归档；
- 是否存在不经过统一服务的写路径。

补充数据库约束时必须兼容现有合法状态，不得简单添加未经核验的枚举 CHECK。

### 14.2 Run 取消原因

扩展 D3 允许的 reason_code：

`project_archived`

保持既有原因不变。

### 14.3 queued Run 取消

新增条件取消逻辑。

不得复用 `run_dispatch/lease.rs:171` 中无条件 UPDATE 的内部取消实现。

### 14.4 归档处理记录

实现时选择：

- 扩展已有持久化执行任务设施；或
- 新增归档处理表。

必须满足第 6 节和第 12 节要求。

### 14.5 Executor Session

先核实现有 Session 的字段、生命周期、登记时机和取消定位能力。

若不能满足第 8、9 节要求，必须扩展或新增结构。

不得为了复用现有表而降低启动握手和清理可靠性。

---

## 15. 验收矩阵

### 15.1 Project 生命周期

| 编号 | 验收项 | 预期结果 |
|---|---|---|
| I1-001 | active 项目归档 | 状态变为 archived |
| I1-002 | 重复归档 | 幂等，不产生重复副作用 |
| I1-003 | 非法状态归档 | 拒绝 |
| I1-004 | 归档审计失败 | 归档事务整体回滚 |
| I1-005 | 后续处理任务写入失败 | 归档事务整体回滚 |
| I1-006 | archived 项目历史读取 | 正常 |

### 15.2 Run 创建与 claim

| 编号 | 验收项 | 预期结果 |
|---|---|---|
| I1-007 | 归档后 TaskReady | 不创建 Delivery Run |
| I1-008 | 归档后 DeliverySubmitted | 不创建 Review Run |
| I1-009 | 归档后共享 Run 创建 | 拒绝 |
| I1-010 | 归档后 Worker claim | 不领取该项目 Run |
| I1-011 | 归档后 MCP claim | 不领取该项目 Run |
| I1-012 | 创建事务先取得 Project 锁 | 创建完成后归档处理该 Run |
| I1-013 | 归档事务先取得 Project 锁 | 后续创建合法跳过 |
| I1-014 | 多 Project 并发 claim | 不产生跨 Project 锁死 |

### 15.3 Run 取消

| 编号 | 验收项 | 预期结果 |
|---|---|---|
| I1-015 | queued Run 归档 | 条件取消为 cancelled |
| I1-016 | claimed Run 归档 | 先撤销凭据，再终止进程 |
| I1-017 | running Run 归档 | 先撤销凭据，再终止进程 |
| I1-018 | completed Run 归档 | 保持 completed |
| I1-019 | failed Run 归档 | 保持 failed |
| I1-020 | 已 cancelled Run 归档 | 无重复副作用 |
| I1-021 | cancel 与 finish 并发 | 只有一个终态转换成功 |
| I1-022 | cancel 与 recover_lost 并发 | 只有一个终态转换成功 |
| I1-023 | cancelled Run | 不产生 failure 行 |
| I1-024 | 归档取消原因 | 为 project_archived |

### 15.4 Executor 启动和清理

| 编号 | 验收项 | 预期结果 |
|---|---|---|
| I1-025 | 启动前归档 | 不启动实际 Agent 工作 |
| I1-026 | spawn 后、PID 登记前归档 | 登记失败并终止子进程 |
| I1-027 | PID 登记后归档 | 能定位并终止子进程 |
| I1-028 | 启动门闩前归档 | 子进程不能开始实际工作 |
| I1-029 | Worker 在启动流程中崩溃 | 重启后能恢复启动意图 |
| I1-030 | Executor 终止失败 | Run 终态不回滚 |
| I1-031 | 终态进程超过一小时 | 仍能被发现并清理 |
| I1-032 | 单次清理超过 LIMIT | 后续批次继续处理 |
| I1-033 | 清理 Worker 崩溃 | 清理任务可恢复 |
| I1-034 | PID 被复用 | 不误杀其他进程 |

### 15.5 Outbox 与故障恢复

| 编号 | 验收项 | 预期结果 |
|---|---|---|
| I1-035 | 归档后消费 TaskReady | 合法跳过并 ACK |
| I1-036 | 归档后消费 DeliverySubmitted | 合法跳过并 ACK |
| I1-037 | 归档后重放死信 | 不创建新 Run |
| I1-038 | 重复消费归档清理任务 | 幂等 |
| I1-039 | 归档事务提交后 Worker 重启 | 后续处理继续 |
| I1-040 | 清理失败达到重试阈值 | 可查询、告警、人工处理 |

---

## 16. 实施任务拆分建议

I1 应拆为以下实施任务，按依赖关系推进。

### I1-A：Project 活动门禁基础设施

范围：

- 核实 Project 合法状态。
- 统一 Project 行锁与活动状态检查。
- 明确事务锁顺序。
- 为业务写入口提供统一门禁能力。

验收：I1-001～006 的基础条件，以及锁顺序相关测试。

### I1-B：Run 创建入口改造

范围：

- TaskReady → Delivery Run。
- DeliverySubmitted → Review Run。
- 共享 Run 创建封装。
- Outbox 合法跳过语义。

验收：I1-007～009、I1-012～013、I1-035～036。

### I1-C：Worker claim 改造

范围：

- 两阶段候选发现与 Project 锁定。
- Run、Task 锁顺序调整。
- MCP 与 Worker 共享门禁。
- 多 Project 并发 claim。

验收：I1-010～014。

### I1-D：归档事务与 Run 取消

范围：

- 归档权限。
- Project 状态转换。
- `governance.audit_log`。
- 归档后续处理任务。
- queued Run 专用条件取消。
- claimed/running D3 取消原因扩展。

验收：I1-001～006、I1-015～024。

### I1-E：Executor 启动握手与持久化清理

范围：

- 启动意图。
- 子进程等待门闩。
- 进程身份登记。
- 归档与启动竞态处理。
- 清理状态持久化。
- 重试、告警和人工处理。

验收：I1-025～034、I1-040。

### I1-F：全链路回归

范围：

- Outbox 重放。
- Worker 重启。
- 多 Worker 并发。
- Run 终态竞争。
- 归档期间的业务写入。
- 历史数据只读。

验收：I1-035～040 及全部回归测试。

---

## 17. 实施边界与最终裁决

### 17.1 必须遵守的设计裁决

1. Project 归档采用 `active → archived`，不增加 `archiving` 中间态。
2. Project 行是项目级活动门闩。
3. 统一锁顺序为 Project → Run → Task → 其他关联记录。
4. Run 创建、Worker claim、Outbox Handler 必须统一执行 Project 活动门禁。
5. 归档事务只关闭入口、写审计、持久化后续工作。
6. queued Run 使用独立条件取消路径。
7. claimed/running Run 复用 T-032 D3。
8. 取消先撤销凭据，再终止进程。
9. 终止失败不回滚 Run 终态。
10. Executor 启动必须具备持久化启动意图和进程登记握手。
11. Executor 清理必须可持久化、可重试、可恢复。
12. Outbox 归档后合法跳过，不进入无限重试或死信。
13. 归档后历史读取继续允许。
14. Agent 不得自行归档 Project。

### 17.2 不允许的简化

以下做法不符合本设计：

- 只在 UI 隐藏新建按钮。
- 只在 API 层做一次不带锁的 Project 状态查询。
- 只在 TaskReady Handler 增加状态检查。
- 只在 Worker 外层检查 Project 状态。
- 在持有 Task 或 Run 锁后再追加 Project 锁。
- 只取消 queued Run，不处理 claimed/running。
- 先杀进程，再撤销 Run 凭据。
- 依赖进程内队列完成归档清理。
- 认为 MCP 请求被拒绝就代表子进程已经停止。
- 认为现有一小时清理窗口可以覆盖所有残留进程。
- 把归档后的业务事件当成普通异常反复重试。

### 17.3 最终验收原则

I1 不能以「Project 状态已经变成 archived」作为全部完成标准。

必须分别证明：

1. **入口关闭**：不存在任何绕过 Project 门禁的新活动入口。
2. **并发安全**：归档、创建、claim、取消和完成之间不存在错误状态覆盖或锁顺序死锁。
3. **凭据撤销**：归档取消后旧 Run 凭据立即失效。
4. **进程可控**：所有迟到或残留 Executor 都能被定位、终止和恢复清理。
5. **消息安全**：归档前产生、归档后消费的业务消息不能重新创建 Run。
6. **审计完整**：归档及其后续处理过程可追溯。
7. **故障可恢复**：Worker 重启、重复投递、进程终止失败均不会使项目重新开放活动入口。

**最终结论：I1 的领域语义、并发控制原则、Run 取消协议、Executor 启动与清理要求、Outbox 处理规则和验收标准至此定稿。**

具体数据库结构、事务 SQL 和进程登记实现属于实施设计细节，必须遵守本稿约束，并通过对应验收测试；不得以实现便利为由改变上述生命周期语义。
