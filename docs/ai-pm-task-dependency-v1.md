# Task 依赖与解锁 v1（T-023 设计稿）

> 状态：**设计稿，待用户过稿**（过稿后才可实施 T-024）
> 日期：2026-11-01
> 一句话：用独立边表 `delivery.task_dependency` 表达「任务 → 前置任务」，写边时用递归 CTE 拒绝环，上游进入终态时在同一事务里解锁下游并投递 `TaskReady`。
> 上位依据：`target.md` §P1「Task 依赖」、`docs/ai-pm-task-state-machine-v1.md`（13 态与 B3 `Blocked→Ready`）、`docs/ai-pm-slice-d-execution-pipeline-v1.md`（D1 领取与 outbox）、`todo.md` T-023。
> 用户已裁定四项：独立边表 / 写边时检环 / 禁止跨项目 / 终态发 `TaskReady`。

---

## 1. 问题与实测现状

| # | 事实 | 证据 |
|---|---|---|
| F1 | 全库**零依赖载体**：没有 `task_dependency` / `depends_on` / `blocked_by` 任何表或列 | `grep -r "task_dependency\|depends_on\|blocked_by" migrations crates` 无命中 |
| F2 | `delivery.task_waiting` 只表达「这个任务此刻在等什么原因」，不是可持久化的任务间关系 | `0010_task_state_machine.sql:90-107`：`reason IN (needs_info, needs_decision, needs_approval, external_dependency)`，同一任务只允许一条活跃行 |
| F3 | 调度侧只按 Run 状态选候选，不检查任务依赖 | `run_dispatch/claim.rs:230-262`（候选 = `queued` 到期 / `claimed` 租约过期） |
| F4 | 消费侧只有 `TaskReady` 一个 handler，创建流程不关心依赖 | `pm-infrastructure/src/outbox_handlers.rs:39-124` |
| F5 | 进入 `ready` 时命令通道已经会投递 `TaskReady`（幂等键 `task:<id>:TaskReady:rev<revision>`） | `command_executor/executor.rs:229-238`、`command_executor/guards.rs:9-15` |
| F6 | `Blocked → Ready` 是合法边；`Ready → Blocked` **不合法** | `pm-domain/src/task_transition.rs:41`、边表 36-57 |

结论：依赖关系需要新载体；调度侧要一个「依赖满足」判定；解锁必须复用既有 outbox 幂等通道，不能旁路插 Run。

---

## 2. 四项裁定（用户已确认）

| # | 决策 | 结论 |
|---|---|---|
| D1 | 载体 | 独立边表 `delivery.task_dependency`；`task_waiting` 维持原语义 |
| D2 | 环检测 | 写边事务内递归 CTE 检测，有环 → `409` |
| D3 | 跨项目 | 禁止；同一 `project_id` 由两条复合外键保证，API 预检给 `409` |
| D4 | 解锁 | 上游进入终态时，同事务把满足依赖的 `blocked` 下游迁到 `ready`，并写 `TaskReady` 事件 |

补充裁定（设计需要，明确记录为「本稿决定」）：

- **D5 状态语义**：前置「已满足」= 上游状态 ∈ `{completed, cancelled, superseded}`。三者都是终态，`cancelled/superseded` 若不算满足会让下游永久阻塞。
- **D6 建边时机**：只允许给 `draft` / `blocked` 任务加边；`ready` 及之后的状态返回 `409 task_not_pending`。理由：`Ready → Blocked` 不是合法边，已就绪任务再加未满足依赖无法回退。
- **D7 建边不自动改状态**：`draft` 任务加边后仍是 `draft`；是否进 `ready` 由用户/命令通道决定。`blocked` 任务加边后若依赖已全满足则立即解锁（见 §5）。
- **D8 删边不级联改状态**：删边后同样做一次解锁判定（可能使 `blocked` 下游变为 `ready`）。

---

## 3. 数据模型

### 3.1 DDL 草案（`migrations/0021_task_dependency.sql`）

```sql
-- 0021_task_dependency: Task 前置依赖（T-023/D1）
-- 依据：docs/ai-pm-task-dependency-v1.md §3
-- 原则：forward-only；新表；同项目由复合外键保证。

CREATE TABLE delivery.task_dependency (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id           UUID NOT NULL,
    project_id          UUID NOT NULL,
    task_id             UUID NOT NULL,          -- 下游（依赖方）
    depends_on_task_id  UUID NOT NULL,          -- 上游（被依赖方）
    created_by          UUID NOT NULL REFERENCES core.principal(id),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),

    UNIQUE (task_id, depends_on_task_id),
    UNIQUE (tenant_id, id),
    CHECK (task_id <> depends_on_task_id),

    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    -- 两条外键共用同一个 project_id：跨项目边在库层不可表示（D3）
    FOREIGN KEY (project_id, task_id)
        REFERENCES delivery.task (project_id, id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, depends_on_task_id)
        REFERENCES delivery.task (project_id, id) ON DELETE CASCADE
);

CREATE INDEX idx_task_dependency_upstream
    ON delivery.task_dependency (project_id, depends_on_task_id);
```

要点：

- `(tenant_id, id)` 唯一键满足 T-021b 的组合外键目标要求（ADR-0001 I5）。
- 边表自身是租户业务表：`tenant_id NOT NULL` + `(tenant_id, project_id) → core.project`。
- `ON DELETE CASCADE` 与 `task_acceptance_criterion` 同款；任务删除（当前无入口）时不留悬空边。

### 3.2 不变量

1. 同一对 `(task_id, depends_on_task_id)` 至多一条边。
2. 边两端必须同项目（库层保证）。
3. 不允许自依赖（`CHECK`）。
4. 不允许有向环（应用层写时保证，见 §4.2）。

---

## 4. 写边/删边接口

### 4.1 端点

| 方法 | 路径 | 语义 |
|---|---|---|
| GET | `/v1/pm/tasks/{task_id}/dependencies` | 返回 `{task_id, upstream: [...], downstream: [...]}`，元素为 `{id, title, status}` |
| POST | `/v1/pm/tasks/{task_id}/dependencies` | body `{depends_on_task_id}`；为 `task_id` 增加一条**指向前置**的边 |
| DELETE | `/v1/pm/tasks/{task_id}/dependencies/{depends_on_task_id}` | 删除边；幂等（不存在也返回 204） |

- 可见性：三条端点都先走 T-022 的 `project_visible`（成员才可见）；不可见即 `404`，不泄露存在性。
- 审计：`governance.audit_log` 写 `action = 'task.dependency.add' / 'task.dependency.remove'`，`target_type='task'`，`target_id = task_id`，`after_data` 带 `depends_on_task_id`。
- 事务：建边/删边各自单事务；写审计与解锁判定同事务。

### 4.2 建边校验顺序（同一事务）

1. 锁下游任务行（`FOR UPDATE`）→ 不存在/不可见 `404`。
2. 白名单校验 `depends_on_task_id`（同项目、存在、可见）→ 不满足 `404`。
3. 自依赖 → `409 SELF_DEPENDENCY`。
4. 下游状态必须 ∈ `{draft, blocked}` → 否则 `409 TASK_NOT_PENDING`。
5. 重复边（唯一键预检）→ `409 DUPLICATE_DEPENDENCY`。
6. **环检测**（递归 CTE）：若「新上游」沿现有边能回到 `task_id`，则拒绝 `409 DEPENDENCY_CYCLE`。
7. 插边 + 审计 + 解锁判定（D7/D8）。

环检测 SQL：

```sql
WITH RECURSIVE ancestors AS (
    SELECT $upstream::uuid AS id
    UNION
    SELECT d.depends_on_task_id
      FROM delivery.task_dependency d
      JOIN ancestors a ON d.task_id = a.id
     WHERE d.project_id = $project
)
SELECT EXISTS (SELECT 1 FROM ancestors WHERE id = $task);
```

### 4.3 错误码

| code | HTTP | 触发 |
|---|---|---|
| `task_not_found` | 404 | 任务不存在 / 非成员 |
| `SELF_DEPENDENCY` | 409 | `task_id == depends_on_task_id` |
| `TASK_NOT_PENDING` | 409 | 下游状态不在 `draft/blocked` |
| `DUPLICATE_DEPENDENCY` | 409 | 边已存在（含并发唯一键冲突） |
| `DEPENDENCY_CYCLE` | 409 | 递归 CTE 命中 |

---

## 5. 依赖满足判定与解锁

### 5.1 单一判定函数

应用层提供**唯一**判定：

```rust
/// 任务的全部前置是否都已满足（D5：completed / cancelled / superseded）。
pub(crate) async fn dependencies_satisfied(
    tx: &mut Transaction<'_, Postgres>, project_id: Uuid, task_id: Uuid,
) -> Result<bool, sqlx::Error>;
```

SQL：

```sql
SELECT NOT EXISTS (
    SELECT 1 FROM delivery.task_dependency d
      JOIN delivery.task u ON u.id = d.depends_on_task_id
     WHERE d.project_id = $1 AND d.task_id = $2
       AND u.status NOT IN ('completed', 'cancelled', 'superseded')
);
```

调度、解锁、建边/删边后的判定全部复用它，禁止各写一套谓词。

### 5.2 领取判定（`claim_core`）

候选 Run 锁住之后（`claim.rs` 候选选择与 fencing 之间）插入：

1. `dependencies_satisfied(&mut tx, project_id, task_id)`。
2. 若为 `false`：把该 Run 释放回 `queued`，`release_reason = 'dependency_unmet'`，`dispatch_attempts + 1`，指数退避（复用现有 `release_back`），返回 `ClaimKind::Skipped`。
3. Model A（指定 task 领取）与 Model B（worker 扫描）走**同一条**判定，不复制逻辑。

> 注意：Unmet 依赖不把 Run 判失败、不改任务状态、不写 `task_run_failure`——它只是「此刻还不能执行」。

### 5.3 解锁触发

在**唯一受守卫的状态迁移入口** `apply_task_transition`（`command_executor/transition.rs`）末尾追加：

- 目标状态 ∈ `{completed, cancelled, superseded}`（D5 的终态集合）时，调用 `unlock_downstream(tx, ctx.project_id, 上游 task_id)`。

`unlock_downstream` 的行为（同一事务）：

1. 查询所有 `depends_on_task_id = 上游` 的下游任务，`ORDER BY task_id` 加锁（固定顺序防死锁）。
2. 对每个下游：
   - 若 `status = 'ready'`：**什么都不做**。该任务进 `ready` 时已经投递过 `TaskReady`，`queued` Run 仍在；依赖满足后领取自然通过。
   - 若 `status = 'blocked'` 且 `dependencies_satisfied`：调用 `apply_task_transition(downstream, Ready, for_dispatch, system_context)`（`Blocked→Ready` 合法），随后用 `PgOutboxAppend` 在**同一事务**写 `TaskReady`。
   - 其他状态（`draft/claimed/running/…`）：不动作。
3. 幂等键：`task:<downstream>:TaskReady:rev<resulting_revision>`，与命令通道约定一致；重复解锁不会产生第二条事件。

> 是否需要在解锁时额外投递 `TaskReady` 给已 `ready` 的下游？**不需要**，也不再重复投递——避免同一语义事件出现两个键。
>
> 为什么不在 `TaskReadyHandler` 里加依赖判定？现有 handler 的语义是「排队」，执行资格由领取侧负责（D1 既有裁定）。若 handler 判依赖，`ready` 且依赖未满足的任务会**永远**拿不到 Run，解锁侧必须再补一条「补投递」逻辑，复杂度高于在领取侧跳过。

### 5.4 解锁后的可观测性

- 解锁迁移写 `audit_log`（`update_task_status`，actor = `ai-pm-dispatch` service principal，`source = System`），与被锁 Run 的领取审计可串联。
- `TaskReady` 事件进入既有 outbox 监控视图（T-061 完善），本切片只保证事件真实产生。

---

## 6. 状态图（新增部分）

```mermaid
stateDiagram-v2
    [*] --> Draft
    Draft --> Ready: 契约确认
    Draft --> Draft: 加边（不自动进 ready）
    Blocked --> Ready: 依赖满足（解锁）
    Ready --> Claimed: 领取（依赖已满足）
    Claimed --> Queued: 依赖未满足 → Skip
    Ready --> Blocked: 不合法（库外不可达）
    Completed --> Unlock: 终态触发
    Cancelled --> Unlock: 终态触发
    Superseded --> Unlock: 终态触发
    Unlock --> Ready: blocked 下游全满足
```

（`Queued` 是 Run 状态，不是任务状态；图中用它表达「Run 被释放回队列」。）

---

## 7. 验收矩阵（续 C42 编号）

| # | 场景 | 期望 |
|---|---|---|
| C43 | 给 `draft` 任务建边成功 | 201/200；GET 能看到 upstream/downstream；审计落 `task.dependency.add` |
| C44 | 自依赖 | 409 `SELF_DEPENDENCY`，库内无行 |
| C45 | 两跳成环（A→B，再 B→A） | 409 `DEPENDENCY_CYCLE`，库内无第二行 |
| C46 | 跨项目依赖 | 409；库层外键也会拒绝（双保险） |
| C47 | 重复边 | 409 `DUPLICATE_DEPENDENCY` |
| C48 | 给 `ready` 任务加边 | 409 `TASK_NOT_PENDING` |
| C49 | 前置未完成时领取 | `ClaimKind::Skipped`；Run 回 `queued`；`release_reason='dependency_unmet'`；无失败记录 |
| C50 | 前置 `completed` 后领取 | 正常 `claimed`，签发租约 |
| C51 | 上游 `completed` 解锁 `blocked` 下游 | 下游 `blocked→ready`；同事务产生一条 `TaskReady`；后续领取成功 |
| C52 | 两个上游只完成一个 | 下游仍 `blocked`；无 `TaskReady` |
| C53 | 上游 `cancelled / superseded` | 同样解锁下游（D5） |
| C54 | 重复投递 `TaskReady` | 不产生第二个活跃 Run（既有幂等） |
| C55 | `blocked` 任务加一条已满足前置 | 立即 `blocked→ready` + `TaskReady` |
| C56 | 删除边后依赖全满足 | `blocked` 下游解锁 |
| C57 | 非成员访问依赖端点 | 404（T-022 语义） |
| C58 | 同一事务回滚 | 边、审计、事件全部不落库 |

---

## 8. 实施顺序（T-024）

1. `0021_task_dependency.sql`（§3.1）。
2. `pm-application::task_dependency` 模块：`dependencies_satisfied`、`add_dependency`、`remove_dependency`、`list_dependencies`、`unlock_downstream`。
3. `apply_task_transition` 末尾接入解锁（终态集合）。
4. `claim_core` 接入领取判定与 `release_back(reason='dependency_unmet')`。
5. `pm-api` 三条端点 + T-022 可见性 + 审计。
6. 迁移/用例/文档回填，`cargo test --workspace` 全绿。

---

## 9. 明确不做

- 不做子任务树/工作项分解（P9）。
- 不做甘特排期、关键路径、自动排期。
- 不做跨项目依赖（D3）。
- 不改 13 态状态机取值；不新增任务状态。
- 不做「依赖变更后自动重排 Run」——领取时的动态判定已经覆盖。
