# AI-PM TaskReady 消费幂等：问题定性与解决方案（V1）

> 面向：技术评审方 / 实施方
> 日期：2026-11-01
> 状态：**已实施并验证（2026-11-01）**：迁移 `0012` + handler 事件级幂等 + `23505` 冲突降级 + 单一真源 + 真实 PG 测试全绿；D1（`0011`）已落库，本方案基于其成果实施
> 关联文档：`docs/ai-pm-slice-d-execution-pipeline-v1.md`（D1 设计依据）、`docs/ai-pm-outbox-wiring-review.md`（Slice C 接线评审）、`docs/ai-pm-task-state-machine-v1.md` §20
> 结论摘要：**TaskReady 的幂等必须拆成两项独立要求——「任务级 active Run 唯一性」（已实现）与「事件级幂等」（缺失）。**补上事件级幂等、唯一冲突降级、崩溃恢复测试后，四种场景（重复投递 / 主动重执行 / 旧事件 replay / 并发消费）才全部闭合。

---

## 〇、一句话结论

TaskReady 的落点是「为任务创建一条 `queued` Run」。当前实现已用**任务行锁 + 活跃 Run 部分唯一索引**保证了「不会并发建两条活跃 Run」；但它**只做了任务级判据，缺事件级幂等**，导致「旧事件 replay / 消费动作已提交但事件未确认时崩溃」会**误建一次新的执行**。补齐这两项，并用真实数据库测试固化，才算可靠。

---

## 一、前因：这件事从哪来

1. **Outbox 规格要求**（`ai-pm-task-state-machine-v1.md` §20）：业务状态变更与事件写入同一事务；事件至少一次投递，**消费者必须幂等**。
2. **③-D 落地**（`bd7e4ef`）：注册 `TaskReadyHandler`，复用 `delivery.task_run` 建 `queued` Run，让 Outbox 首版在生产真正生效。
3. **外部评审提出 7 点**，其中第 1 点为「TaskReady 幂等不可靠」。评审基于接线稿的**文字方案**（只写「查 active Run → 建 Run」，未写行锁），因此高估了并发风险、并**未读到实现已在同事务加了任务行锁**。
4. **D1 设计依据 v1 已裁决**：活跃 Run 收敛为 `queued/claimed/running`（B1）、锁顺序 `run → task` 且新建 Run 的 INSERT 必须在取 task 行锁之后（B2）、创建与计数共享封装（B3-6）。
5. **本仓裁决**：先修 TaskReady 幂等可靠性，再回知识空间 V2；迁移号 `0011` 归 D1，幂等的 Schema 改动走 `0012`。

**为什么必须现在修**：`TaskReady` 是「业务任务 → 执行实例」的唯一入口。入口幂等不可靠，D2（上下文下发）之后每一次「重执行 / replay」都会放大为重复执行或错误跳过，缺陷会写进后续切片。

---

## 二、问题定义：幂等不可靠的四种场景

| 场景 | 期望行为 | 当前实现行为 | 判定 |
| --- | --- | --- | --- |
| 同一 TaskReady 重复投递（at-least-once 重投同一事件行） | 不重复创建 Run | 已有活跃 Run → 跳过 ✓ | **正确** |
| 两个 Worker 并发消费同一事件 | 最终只有一个活跃 Run | 任务行锁串行化 + 部分唯一索引兜底 ✓ | **正确** |
| 任务失败后用户主动重新执行 | 允许创建新 Run | 无独立语义（现只能靠改 Task 状态回 `ready` 触发，撞状态机） | **缺失（语义层）** |
| 旧事件被人工 Replay | 不应意外变成一次新的执行 | 若旧 Run 已终态、任务又回到 `ready` → **会新建 Run** | **错误（真实缺口）** |
| 消费动作已提交、事件确认前 Worker 崩溃 | 重试不得重复建 Run | 依赖 handler 幂等；因缺事件级标识，**存在误建风险** | **风险（真实缺口）** |

> 说明：第三行「主动重新执行」是**语义缺失**（属 D1 后续的领域命令议题）；第四、五行是**幂等机制缺口**，本方案重点解决。

---

## 三、根因分析

| # | 根因 | 说明 |
| --- | --- | --- |
| R1 | **把「事件级幂等」与「任务级 active Run 唯一性」合并成一个判据** | 现判据是「任务 `ready` 且无活跃 Run」。它只能保证「同一时刻只有一条活跃 Run」，无法表达「**这个事件**是否已经产生过副作用」。当副作用对象（Run）已离开活跃集合，判据就失效 |
| R2 | **消费动作与事件确认不在同一事务** | `dispatch_once` 三步：`claim` → `handler.handle()`（handler 自开事务并 commit）→ `outbox::complete()`（另一条独立 SQL）。中间崩溃 → 事件被 redeliver → 必须靠幂等兜住 |
| R3 | **唯一冲突的语义未定义** | `uq_task_run_active` 冲突是「并发/重复」的**预期结果**，但 handler 会把 `sqlx::Error` 直接上抛 → 事件被判失败、退避甚至死信（即评审担心的「Outbox 反复重试」） |
| R4 | **活跃 Run 集合多源手写** | 集合同时出现在 handler SQL 字符串、迁移谓词、`run_dispatch` 常量三处，任一处漂移都会破坏幂等，且无机制强制一致 |
| R5 | **跳过分支静默** | 「已有活跃 Run 则跳过」无审计，排障时无法解释「为何没有建 Run」 |

---

## 四、现状核查（实现事实）

以当前分支（`bb47d30` 基线；主工作树另含未提交的 D1 实现）为参照：

| 事实 | 落点 |
| --- | --- |
| handler 在同一事务内先锁任务行 | `crates/pm-infrastructure/src/outbox_handlers.rs:43-53`（`SELECT … FROM delivery.task … FOR UPDATE`） |
| 资格校验：`status='ready'` 且 `assignee_id` 非空 | 同上 `:66-71` |
| 任务级幂等判据：存在活跃 Run 则跳过 | 同上 `:73-85` |
| 建 Run + 递增 `attempt_no` / `attempt_count`（同事务） | 同上 `:87-108`；D1 版抽为 `pm_application::run_dispatch::insert_task_run` |
| 消费与确认分离 | `outbox_worker.rs:83-119`（`claim` → `handle` → `complete`） |
| 事件携带 `idempotency_key` **但 handler 未使用** | `outbox.rs:30-40` 定义；`outbox_handlers.rs` 全文无引用 |
| 部分唯一索引（D1 版） | `migrations/0011_run_dispatch.sql:82-83`（`uq_task_run_active ON task_run(task_id) WHERE status IN (...)`） |
| 已有测试 | `crates/pm-infrastructure/tests/outbox_handlers.rs`（正常 / 重复 / 并发 / 过期 / 无指派人 / malformed / 端到端） |

**结论**：任务级唯一性**已实现**；事件级幂等、冲突降级、崩溃恢复测试**缺失**。

---

## 五、解决方案（分层）

> 原则：**任务级与事件级两项独立要求，各自有独立防线，不互相代替。**

### L0 任务级 active Run 唯一性（保留现状）

- 机制：任务行锁（`delivery.task FOR UPDATE`）+ 部分唯一索引（`uq_task_run_active`，谓词 `queued/claimed/running`）。
- 作用：保证「同一任务同一时刻至多一条活跃 Run」。
- 改动：**无**（沿用 D1）。

### L1 事件级幂等（新增·核心）

- 机制：为每条 Run 记录「由哪个事件创建」——`delivery.task_run.source_event_id`，并对非空值建**唯一索引**。
- 语义：**一个 outbox 事件至多创建一条 Run**。重复投递同一事件行（含 lease 过期后的 redeliver、旧事件人工 replay）时，第二次 INSERT 撞唯一约束 → 视为幂等成功。
- 与 L0 的分工：
  - 同一事件重复投递 → L1 兜（即使旧 Run 已终态、任务又 ready，也不会新建）。
  - 不同事件的重复投递 / 并发 → L0 兜（任务级 active Run 唯一性）。

### L2 唯一冲突降级为幂等成功（新增）

- handler 捕获 `source_event_id` 唯一约束冲突（SQLSTATE `23505`）→ 确认消费（`Ok(())`），**不判失败、不进死信**。
- 同理，`uq_task_run_active` 冲突也按幂等成功处理（并发下先到者已建）。

### L3 锁顺序硬不变量（沿用 D1 并显式化）

- 任何新建 Run 的 `INSERT` 必须在取得 `delivery.task` 行锁**之后**；顺序固定 `run(FOR UPDATE SKIP LOCKED) → task(FOR UPDATE) → INSERT run`。
- 原因：PostgreSQL 唯一索引冲突是「等待」而非立即失败；锁序反转会形成真死锁。

### L4 崩溃恢复语义（依赖 L1）

- 消费动作已提交、`outbox.complete()` 未执行时崩溃 → 事件 lease 过期被 redeliver → L1（`source_event_id` 唯一）+ L0（活跃 Run）共同保证不重复建 Run。

### L5 单一真源与可观测（治理）

- 活跃 Run 集合收敛到 `run_dispatch::ACTIVE_RUN_STATUSES_SQL` / `ACTIVE_RUN_STATUSES`，handler 与迁移谓词复用；新增集成测试断言「索引谓词 == 常量集合」。
- 跳过分支写审计或计数（reason: `active_run_exists` / `task_not_ready` / `no_assignee`）。

---

## 六、数据模型最小改动（迁移 `0012`）

> 迁移号归属：`0011` 属 D1。本文的 Schema 改动走 **`0012`**，与 D1 互不冲突。若 D1 的 `0011` 尚未入库，实施前必须先将其落地（或由 D1 线吸收本改动），不得抢号。

```sql
-- 0012_task_run_event_idempotency: 为 Run 绑定创建它的事件，实现事件级幂等。
-- 依据：docs/ai-pm-taskready-idempotency-v1.md §五 L1。

-- 1) 事件来源列（可空：恢复路径新建的 Run 无对应事件）。
ALTER TABLE delivery.task_run
    ADD COLUMN IF NOT EXISTS source_event_id UUID;

-- 2) 事件级唯一：一个 outbox 事件至多创建一条 Run。
--    部分唯一索引：仅约束非空值，允许恢复路径/历史 Run 为 NULL。
CREATE UNIQUE INDEX IF NOT EXISTS uq_task_run_source_event
    ON delivery.task_run(source_event_id)
    WHERE source_event_id IS NOT NULL;

-- 3) 存量核查（若历史已存在同一事件的重复 Run，显式失败并输出冲突行，
--    与 0011 的 S6 一致，不得静默删除）。
DO $$
DECLARE r RECORD;
BEGIN
    FOR r IN
        SELECT source_event_id, count(*) AS n
          FROM delivery.task_run
         WHERE source_event_id IS NOT NULL
         GROUP BY source_event_id
        HAVING count(*) > 1
    LOOP
        RAISE EXCEPTION
            '0012: event_id=% already created % runs; resolve before indexing',
            r.source_event_id, r.n;
    END LOOP;
END $$;
```

> 可选：`source_event_id` 加外键到 `integration.outbox_event(id)`。若不希望跨 schema 强依赖，可仅保留列 + 唯一索引，由 handler 保证写入来源。

---

## 七、实现落点与代码片段

### 7.1 共享封装扩展（`crates/pm-application/src/run_dispatch.rs`）

`insert_task_run` 增加 `source_event_id` 参数（B3-6：创建与计数只此一处）：

```rust
pub async fn insert_task_run(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    source_event_id: Option<Uuid>,   // 新增：事件级幂等绑定
) -> Result<CreatedRun, sqlx::Error> {
    let attempt_no: i32 = sqlx::query_scalar(
        "SELECT coalesce(max(attempt_no), 0) + 1 FROM delivery.task_run WHERE task_id = $1",
    ).bind(task_id).fetch_one(&mut **tx).await?;

    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run \
         (project_id, task_id, attempt_no, status, stage, next_dispatch_at, source_event_id) \
         VALUES ($1, $2, $3, 'queued', 'dispatch', now(), $4) RETURNING id",
    ).bind(project_id).bind(task_id).bind(attempt_no).bind(source_event_id)
     .fetch_one(&mut **tx).await?;

    sqlx::query("UPDATE delivery.task SET attempt_count = attempt_count + 1 WHERE id = $1")
        .bind(task_id).execute(&mut **tx).await?;

    Ok(CreatedRun { run_id, attempt_no })
}
```

> 调用点：`claim_once` 的恢复路径传 `None`（不是事件触发）。

### 7.2 handler 改造（`crates/pm-infrastructure/src/outbox_handlers.rs`）

```rust
// 已是：同一事务内 SELECT delivery.task … FOR UPDATE（保持）
// 已是：status='ready' 且 assignee 非空（保持）
// 已是：活跃 Run 判据（保持，但集合改为复用常量）
let active: i64 = sqlx::query_scalar(&format!(
    "SELECT count(*) FROM delivery.task_run \
     WHERE task_id = $1 AND status IN ({})",   // ← 复用 ACTIVE_RUN_STATUSES_SQL
    pm_application::run_dispatch::ACTIVE_RUN_STATUSES_SQL,
)).bind(task_id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
if active > 0 {
    // L5：写审计/计数（reason=active_run_exists），不再静默
    tx.commit().await.map_err(|e| e.to_string())?;
    return Ok(());
}

// L1：把事件 id 绑定进 Run；L2：唯一冲突降级为幂等成功
match pm_application::run_dispatch::insert_task_run(&mut tx, event.project_id, task_id, Some(event.id)).await {
    Ok(_) => {}
    Err(sqlx::Error::Database(db)) if db.code().as_deref() == Some("23505") => {
        // 同一事件已创建 Run（重投/replay）或并发先到者已建 Run → 幂等成功
        tx.rollback().await.map_err(|e| e.to_string())?;
        return Ok(());
    }
    Err(e) => return Err(e.to_string()),
}
tx.commit().await.map_err(|e| e.to_string())?;
Ok(())
```

> 注意：冲突时整个 `attempt_count` 递增随事务回滚，不会产生「计数已加、Run 未建」的半更新。

### 7.3 迁移谓词单一真源（治理）

- 新增集成测试断言：`pg_get_indexdef('uq_task_run_active')` 的谓词集合 == `ACTIVE_RUN_STATUSES`。
- 迁移文件加锚点注释指向常量位置。

---

## 八、测试矩阵（真实 PostgreSQL，不 Mock 数据访问）

| ID | 场景 | 断言 |
| --- | --- | --- |
| I1 | 正常消费 | 恰 1 条 `queued` Run；task 仍 `ready`；`attempt_count=1` |
| I2 | 同一事件重复 handler 调用 | 仍 1 条 Run |
| I3 | 两个并发消费同一事件 | 恰 1 条 Run；无死锁 |
| I4 | **旧事件 replay（本方案核心）**：Run 已终态、任务重新 `ready`，replay 同一事件 | **不新建 Run**；`source_event_id` 唯一 |
| I5 | **崩溃恢复**：handler 已 commit、`complete` 前"崩溃"（不调用 complete），事件 redeliver | 重试不重复建 Run |
| I6 | 两个独立合法执行请求（不同 `source_event_id`） | 按业务规则分别建 Run（当前语义下：第二个因 active Run 跳过，语义待 §十一 S4 定义） |
| I7 | 唯一冲突降级 | 并发冲突不产生死信、事件为 `Delivered` |
| I8 | 引用不存在的 task / 非 `ready` / 无指派人 | 无 Run；无副作用 |
| I9 | 事务失败注入 | task 无部分更新、计数不变、无 Run |
| I10 | 谓词一致性 | 唯一索引谓词 == `ACTIVE_RUN_STATUSES` |

> 优先级最高：I4（旧事件 replay）与 I5（崩溃恢复）——它们是本方案要修的两处真实缺口。

---

## 九、验收标准

1. I1–I10 全绿（真实 PG）。
2. 「同一事件至多一条 Run」由**数据库唯一索引**保证，不依赖应用层判断。
3. 并发冲突、重复/replay **不进死信**（事件为 `Delivered`）。
4. 任务级与事件级两项防线各自可独立测试（去掉任一，另一对应场景应失败）。
5. 后端全量测试基线全绿、`cargo build` 无新 warning、迁移可重入。

---

## 十、风险与阻塞前置

| # | 项 | 影响 | 处置 |
| --- | --- | --- | --- |
| **前置 1**（**已解决**） | D1（`0011`）已于 2026-11-01 落库（`92c1f26`）；本方案基于其成果实施，未抢号 | — | 已闭环 |
| 风险 1 | 并行会话正改 `outbox_handlers.rs` | 同文件冲突 | 本改动在 D1 成果之上叠加；避免两条线同时改该文件 |
| 风险 2 | `source_event_id` 外键跨 schema | 依赖方向/DDL 耦合 | 默认只加列 + 唯一索引，不加强外键 |
| 风险 3 | 「主动重执行」语义未定 | 与「事件级幂等」边界模糊 | 明确：重执行 = **新事件**（新 id）；replay = **同事件**（不得新建）。语义细化见 §十一 S4 |

---

## 十一、落地顺序

> 实施进度（2026-11-01）：**S0–S3 已完成并全绿**（迁移 `0012` + 事件级幂等 + `23505` 降级 + 单一真源 + 测试）；S4（主动重执行语义）/ S5（文档口径修正）待办。

| 阶段 | 内容 | 验收 |
| --- | --- | --- |
| **S0** | 确认 `0011`（D1）落库并合并；确定本改动基于其成果 | 基线含 `uq_task_run_active` 与 `run_dispatch` |
| **S1** | 迁移 `0012`：`source_event_id` + 唯一索引 + 存量核查 | 迁移可重入；`\d` 校验 |
| **S2** | `insert_task_run` 扩展 + handler 事件级幂等 + 23505 降级 | I2/I4/I7 绿 |
| **S3** | 崩溃恢复测试 + 谓词一致性测试 + 跳过审计 | I5/I10 绿 |
| **S4** | 「主动重执行」领域命令/语义（区分任务变为可执行 vs 请求新执行实例） | 语义文档 + 测试 |
| **S5** | 文档口径修正（事件开关策略、Delivered 语义） | 与实现一致 |

> S5 与本文可分开发布：**纯文档一组、实现一组**（遵循本仓「docs 与实现分离提交」的裁决）。

---

## 十二、与 D1 / `0011` 的关系

- 本方案**不推翻 D1**，是其幂等能力的**补强**：D1 提供「任务级唯一性 + 锁序 + 共享封装」，本方案增加「事件级幂等 + 冲突降级 + 崩溃恢复测试」。
- 迁移分工：`0011`（D1：`claimed` 状态、租约列、`uq_task_run_active`、多仓表）；`0012`（本方案：`source_event_id` 唯一）。
- 若 D1 尚未提交，**两个选择**：
  1. 等 D1 落库后，本改动基于其成果实施（推荐，冲突最小）；
  2. 由 D1 线直接在 `0011` 内吸收 `source_event_id`（一次迁移完成，需 D1 线同意扩围）。
- **不得**在缺 `0011` 的基线上自行补 `claimed`/唯一索引，会与 D1 重复并抢号。