# T-061 · Outbox 业务事件覆盖与死信重放（P8）· 设计稿 V1

> **状态**：待用户确认（§0.1 brainstorming 已完成首轮裁定，2026-11-02）
> **任务**：`todo.md` §9 T-061（2026-11-02 领用，执行者：Trae）
> **上位依据**：`target.md` §P8「Outbox 与可靠异步处理」；`docs/ai-pm-task-state-machine-v1.md` §20.5 / §20.6（首版条款与验收标准）；`0010_task_state_machine.sql:123-144`（`integration.outbox_event` 已有 `attempts` / `dead_letter` / `locked_until` / `idempotency_key`）
> **同批已落地（T-043 搭车）**：`DeliverySubmitted` / `ReviewCompleted` / `KnowledgeBaseWriteRequested` 生产者
> **实施约束**：`AGENTS.md` 规则 1 —— 本项目无存量数据，**需要改契约就直接改契约**，不写迁移脚本 / 双写 / 兼容分支 / 回退开关

---

## 一、现状盘点（实测，非推断）

### 1.1 已有能力

| 能力 | 位置 | 状态 |
|---|---|---|
| 事件表 | `integration.outbox_event` | ✅ `0010` 建表；`0019` 补 `tenant_id` + 组合外键 + `UNIQUE (tenant_id, idempotency_key)` |
| 同事务写入 | `pm_application::outbox::PgOutboxAppend` | ✅ `append` 只接受 `&mut Transaction` —— 「状态已变、事件没写」不可表示 |
| Worker 分发 | `pm_infrastructure::outbox_worker::OutboxWorker` | ✅ 领取 → 路由 → 结算；未注册事件**明确死信**（不假装消费） |
| 重试 + 退避 | `pm_infrastructure::outbox::fail` | ✅ 指数退避 `base * 2^attempts`，达 `max_attempts` 转 `dead_letter` |
| 死信查询 | `outbox::list_dead_letters` | ⚠️ **基础设施层已实现，但无任何 API 暴露** |
| 死信重放 | `outbox::replay` | ⚠️ **同上——已实现但无 API 暴露** |
| 幂等坍缩 | `PgOutboxAppend` 的 `ON CONFLICT (tenant_id, idempotency_key) DO NOTHING` | ✅ 写入时去重 |

### 1.2 六类首批事件的真实状态（`target.md` §P8 清单）

| 事件 | 生产者 | 消费者 | 判定 |
|---|---|---|---|
| `TaskReady` | ✅ `task_dependency.rs:457`（解锁同事务） | ✅ `TaskReadyHandler` → 建 queued Run | **完整** |
| `DeliverySubmitted` | ✅ `delivery/mod.rs:762`（提交同事务） | ✅ `DeliverySubmittedHandler` → D5 拍① | **完整** |
| `ReviewCompleted` | ✅ `review/decide.rs:347` / `:408` | ❌ 无（V1 纯通知） | 生产者有，**无消费者** |
| `KnowledgeBaseWriteRequested` | ✅ `review/decide.rs:423` | ❌ 无 | 生产者有，**无消费者** |
| `RunCompleted` | ❌ **无** | ❌ 无 | **完全缺失** |
| `RunRequested` | ❌ 无 | ❌ 无 | **无业务语义**——建 Run 的动作已由 `TaskReady` handler 承担，再发一条是重复。**按 `target.md`「不为凑数造 Handler」不做** |

### 1.3 三个真实缺口（todo.md §9 明列）

1. **`RunCompleted` 生产者** —— Run 终态（`finish_run` 进程退出 / `record_termination` 终止）当前只写审计，不发事件。
2. **死信重放接口** —— `outbox::list_dead_letters` / `outbox::replay` 在基础设施层躺着，**没有任何 HTTP 入口**。运维发现死信只能直连 DB。
3. **事件监控指标** —— 无任何计数 / 积压 / 失败率视图。

### 1.4 一个被忽略的事实

`event_is_enabled()`（`pm_application::outbox.rs:86`）读环境变量 `AI_PM_OUTBOX_EVENTS`：

```rust
Err(_) => matches!(event_type, "TaskReady" | "DeliverySubmitted"),
```

**未设置该变量时，`ReviewCompleted` / `KnowledgeBaseWriteRequested` 默认不产生**（V1 不注册 handler，避免悬空事件进死信）。测试靠 `set_var` 打开。这不是 bug，但它意味着：**监控端点若要统计这几类事件，必须能区分「没产生」与「产生了但没消费」** —— §四的统计口径据此设计。

---

## 二、范围裁定（2026-11-02 用户确认）

| 决策点 | 裁定 | 理由 |
|---|---|---|
| `RunCompleted` 语义 | **Run 终态即发事件，无消费者** | Run 终态已由 D5 失败判定作业扫描 `delivery.task_run` 驱动，再造一个消费者改状态会与失败判定**抢状态**。做成纯通知事件，与 `ReviewCompleted` 同构：生产 + 监控，V1 不注册 handler。 |
| 死信重放形态 | **REST 端点 + 运维权限** | 复用已存在的 `outbox::list_dead_letters` / `outbox::replay`；走现有 `require_project_visible` 租户隔离；运维排障是人用场景。 |
| 事件监控形态 | **REST 只读端点** | 不引入 Prometheus 依赖；与现有 `/v1/pm/*` 契约一致。 |

### 2.1 不做（YAGNI）

- ❌ 不做 `RunRequested`（无业务语义——建 Run 已由 `TaskReady` handler 承担）。
- ❌ 不为 `ReviewCompleted` / `KnowledgeBaseWriteRequested` 造消费者（V1 无落点；按 `target.md`「不为凑数造 Handler」）。
- ❌ 不引入消息队列 / 事件总线 / 事件溯源（§20.6 第 7 条）。
- ❌ 不做 Prometheus `/metrics`、不做前端运维页（T-070 已在做执行中心）。
- ❌ 不做批量重放（一条一次，YAGNI）。
- ❌ 不做死信事件的「编辑 payload 后重放」（会破坏事件溯源）。

---

## 三、架构：三层，各司其职

```
┌─────────────────────────────────────────────────────────────┐
│ 生产层（应用层）                                             │
│   finish_run / record_termination  ──┐                       │
│   review/decide.rs (已完成)         ──┼─→ PgOutboxAppend    │
│   delivery/mod.rs (已完成)          ──┘   （同事务写入）      │
├─────────────────────────────────────────────────────────────┤
│ 分发层（基础设施层）                                          │
│   OutboxWorker::dispatch_once                                 │
│     claim → 路由 handler → complete / fail(退避) / dead_letter│
│   未注册事件类型 → 明确死信（可 replay，绝不假装消费）          │
├─────────────────────────────────────────────────────────────┤
│ 运维层（本次新增）                                            │
│   GET  /v1/pm/outbox/dead-letters    死信列表 + 失败原因       │
│   POST /v1/pm/outbox/events/{id}/replay  幂等重放             │
│   GET  /v1/pm/outbox/stats           计数 / 积压 / 失败率       │
└─────────────────────────────────────────────────────────────┘
```

**关键边界**：运维层**只调基础设施层已有的 `outbox::list_dead_letters` / `outbox::replay`**，不改 outbox 状态机。生产层与运维层之间没有耦合——`RunCompleted` 生产者不需要知道运维端点的存在。

---

## 四、监控指标设计（§一.1 的三个口径问题）

### 4.1 端点契约

```
GET /v1/pm/outbox/stats?project_id=<uuid>
```

**权限**：与现有 `/v1/pm/*` 一致——`require_project_visible`（同租户 + 项目成员）。`project_id` **必填**（跨项目的运维视图没有意义，且必须落到一个租户边界内）。

**响应**：

```json
{
  "by_status": { "pending": 3, "processing": 1, "delivered": 42, "dead_letter": 0 },
  "by_event_type": [
    { "event_type": "TaskReady", "pending": 1, "processing": 0, "delivered": 20, "dead_letter": 0 },
    { "event_type": "RunCompleted", "pending": 2, "processing": 1, "delivered": 22, "dead_letter": 0 }
  ],
  "failure_rate": 0.0,
  "oldest_pending_age_secs": 12,
  "dead_letter_count": 0
}
```

| 字段 | 口径 |
|---|---|
| `by_status` | 该项目下按 `status` 分组的行数（四值全覆盖，缺者为 0） |
| `by_event_type` | 按 `event_type` × `status` 的交叉计数；**只列出有行的类型** |
| `failure_rate` | `dead_letter / (delivered + dead_letter)`，分母为 0 时取 `0.0`（不是 `NaN`、不是 `1.0`——**没有事件不是「全部失败」**） |
| `oldest_pending_age_secs` | `pending` + `processing` 两类里**最老**那条的年龄（秒）；无积压时为 `0` |
| `dead_letter_count` | `dead_letter` 行数（与 `by_status.dead_letter` 冗余，但让运维一眼看到告警值） |

### 4.2 为什么 `failure_rate` 不含 `pending`

`pending` 是**还没处理**，不是**处理失败**。把它算进失败率会让「刚产生的正常事件」被计为失败——这是最常见的监控误报。失败率只衡量「已经走完生命周期的事件里，失败的比例」。

### 4.3 为什么 `oldest_pending_age_secs` 要算 `processing`

`processing` 行可能是**卡住的**：worker 崩溃后租约过期的行会留在 `processing` 直到被重新领取。积压年龄取 `pending` 与 `processing` 的共同最老值，才能暴露「worker 死了没人捞」。

### 4.4 死信列表端点

```
GET /v1/pm/outbox/dead-letters?project_id=<uuid>&limit=50
```

复用 `outbox::list_dead_letters(pool, limit)`。**注意一个既存缺陷**：`list_dead_letters` 的 SQL **没有 `project_id` 过滤**——它返回全库死信。

> **本次修复**：`list_dead_letters` 增加 `project_id` 参数与 `WHERE project_id = $1` 过滤。
> 依据 `AGENTS.md` 规则 1（无存量数据，直接改契约，不留兼容分支）。

响应：`{ "items": [...], "total": N }`，每项含 `id` / `event_type` / `aggregate_type` / `aggregate_id` / `attempts` / `last_error` / `created_at`。

### 4.5 重放端点

```
POST /v1/pm/outbox/events/{event_id}/replay?project_id=<uuid>
```

**语义**（复用 `outbox::replay`）：

| 前置 | 结果 |
|---|---|
| 事件不存在 | 404 `outbox_event_not_found` |
| 事件存在但不是 `dead_letter` | 409 `outbox_event_not_dead_letter`（活事件仍在飞行中，重放会重复投递） |
| 事件是 `dead_letter` | 200 `{ "replayed": true, "event_id": "..." }` |

**幂等性**（todo 验收项「死信重放幂等」）：`replay` 把 `status` 置 `pending`、`attempts` 归零、清租约。**第二次对同一事件 replay 会返回 409**（因为它已经不是 `dead_letter` 了）——这是**正确的幂等**：不会产生第二条投递。

> **这里有一个真实的边界问题**：`replay` 归零 `attempts` 意味着重放后的事件又有完整 5 次重试预算。如果处理器的 bug 没修，重放会再死信一次——这是**期望行为**（不是无限重试，是「再给 5 次」）。

**重放不保证消费成功**：replay 只是把事件放回队列，**真正的消费由 worker 的下一次 `dispatch_once` 完成**。端点不等待消费完成——那会让 HTTP 请求挂起在 worker 调度节奏上。

---

## 五、`RunCompleted` 生产者设计

### 5.1 落点：两个终态路径

| 路径 | 函数 | 终态 |
|---|---|---|
| 进程自然退出 | `run_executor::finish_run` | `succeeded` / `failed` |
| 主动终止 | `run_executor::record_termination` | `timed_out` / `cancelled` / `failed` |

两者都**已经在同事务里**写审计（`audit_run`）。事件与终态、审计**同事务**追加。

### 5.2 为什么不放在 `run_dispatch::claim_and_start` 里

领取路径不产生终态。终态只在上面两处。

### 5.3 幂等键

```
run_completed:<run_id>
```

**以 run_id 钉死**——一个 Run 只可能有一次终态（条件 UPDATE `WHERE status IN ('claimed','running')` 保证先到先得，第二个到达者 `updated` 为 `None`，直接返回，**不写事件**）。

这与 `DeliverySubmitted` 的 `delivery_submitted:<run_id>` 同构。

### 5.4 载荷

```json
{
  "run_id": "<uuid>",
  "task_id": "<uuid>",
  "outcome": "succeeded",
  "status": "succeeded",
  "attempt_no": 1
}
```

`aggregate_type = "TaskRun"`，`aggregate_id = <run_id>`。

### 5.5 事件开关

`event_is_enabled("RunCompleted")` 需加入默认开启清单。理由与 `TaskReady` / `DeliverySubmitted` 不同：

- `TaskReady` / `DeliverySubmitted` **有 handler**，默认开是必需的（不开就断链）。
- `RunCompleted` **无 handler** —— 默认开会产生「无人消费」的事件。

**裁定**：`RunCompleted` **默认开启**，但**必须同时注册一个 handler**（见 §5.6）——否则违反「不产生悬空事件」原则。

### 5.6 `RunCompleted` 的消费者：让事件不是悬空的

问题：V1 没有业务落点。但**没有 handler 的事件会被 Worker 死信**（`OutboxWorker::dispatch_once` 的 `None` 分支 → `dead_letter` + "no handler registered"）。这会让监控面板上出现一堆死信，**污染死信列表**（运维排障时真假难辨）。

三个方案：

| 方案 | 做法 | 评价 |
|---|---|---|
| A. 不发 `RunCompleted` | 只补死信 + 监控 | ❌ todo 明列 `RunCompleted` 是验收项 |
| B. 发 + 注册「记录型」handler | handler 只做**幂等的日志 + 计数**，不改任何业务状态 | ✅ **推荐** |
| C. 发 + 不注册 handler | 依赖 `event_is_enabled` 默认关 | ⚠️ 默认关 = 事件基本不产生，等于没做；默认开 = 死信堆积 |

**裁定 B**：`RunCompletedHandler` 的职责是「**记录这次执行结束的事实**」——写一条 `governance.audit_log`（`action = "run_completed_event"`）+ 一条 `integration.notification`（`kind = "run_completed"`，让前端通知流能看到执行结束）。

**为什么这是真实业务语义而非凑数**：前端 `workspace/:projectId/execution` 执行中心（T-070）需要展示「Run 结束」的事实。`integration.notification` 已经是既有的通知通道（`/api/v1/projects/{id}/notifications` 读它），T-070 的执行中心可以直接消费。

**幂等性**：`audit_log` 与 `notification` 的插入需幂等——用 `run_id` 作幂等锚点。`governance.audit_log` 无唯一约束，靠条件 INSERT：

```sql
INSERT INTO governance.audit_log
    (project_id, actor_id, action, target_type, target_id, after_data, source_kind, tenant_id)
SELECT $1, $2, 'run_completed_event', 'task_run', $3, $4, 'run_dispatch',
       (SELECT tenant_id FROM core.project WHERE id = $1)
WHERE NOT EXISTS (
    SELECT 1 FROM governance.audit_log
     WHERE target_id = $3 AND action = 'run_completed_event' AND project_id = $1
)
```

`integration.notification` 的 `source_proposal_id` 是 `UUID UNIQUE` 且必须为 `NULL`（`notify` 现有实现已经是 `NULL`），幂等靠 payload 里的 `run_id` + 条件 INSERT：

```sql
INSERT INTO integration.notification (project_id, source_proposal_id, kind, payload, tenant_id)
SELECT $1, NULL, 'run_completed', $2,
       (SELECT tenant_id FROM core.project WHERE id = $1)
WHERE NOT EXISTS (
    SELECT 1 FROM integration.notification
     WHERE project_id = $1 AND kind = 'run_completed'
       AND payload->>'run_id' = $3
)
```

`actor_id` 用 `DISPATCH_SERVICE_PRINCIPAL`（`00000000-0000-0000-0000-000000000001`，`0015` 已种子化为 `service` principal）——Run 结束是**平台事实**，不是人的动作。

---

## 六、幂等与「不产生悬空事件」的总账

### 6.1 事件类型注册表（单一真源）

现状：事件类型散落在 `event_is_enabled` 的 match、Worker 的 `.with_handler` 调用、各生产点的字符串字面量里。**三处不同步就会出「产生了但没人消费」或「消费了但没人产生」。**

**裁定**：新增 `pm_application::outbox::EventCatalog`：

```rust
/// 事件类型的**单一真源**：哪些类型会被生产、哪些有消费者、默认是否开启。
pub struct EventSpec {
    pub event_type: &'static str,
    /// 是否有真实消费 handler。`false` 的类型默认**不产生**（§20.6「不假装消费」）。
    pub has_handler: bool,
    /// 生产者所在位置（文档用）。
    pub producer: &'static str,
}

pub const CATALOG: &[EventSpec] = &[
    EventSpec { event_type: "TaskReady",             has_handler: true,  producer: "task_dependency.rs" },
    EventSpec { event_type: "DeliverySubmitted",     has_handler: true,  producer: "delivery/mod.rs" },
    EventSpec { event_type: "ReviewCompleted",       has_handler: false, producer: "review/decide.rs" },
    EventSpec { event_type: "KnowledgeBaseWriteRequested", has_handler: false, producer: "review/decide.rs" },
    EventSpec { event_type: "RunCompleted",          has_handler: true,  producer: "run_executor.rs" },
];
```

`event_is_enabled` 改为查 `CATALOG`：

```rust
pub fn event_is_enabled(event_type: &str) -> bool {
    if let Ok(value) = std::env::var("AI_PM_OUTBOX_EVENTS") {
        return value.split(',').map(str::trim).any(|item| item == event_type);
    }
    CATALOG.iter()
        .find(|spec| spec.event_type == event_type)
        .map(|spec| spec.has_handler)
        .unwrap_or(false)
}
```

**这改变了既有行为**：`ReviewCompleted` / `KnowledgeBaseWriteRequested` 在未设置 `AI_PM_OUTBOX_EVENTS` 时**仍然默认关闭**（`has_handler = false`），与现状一致；但 `RunCompleted` 因为**有** handler 所以默认开启。**不设环境变量时的行为对既有事件零变化**（`TaskReady` / `DeliverySubmitted` 都是 `has_handler = true`）。

### 6.2 悬空事件检测（Worker 侧兜底）

`OutboxWorker` 启动时校验：注册的 handler 类型都在 `CATALOG` 里；`CATALOG` 里 `has_handler = true` 的类型都注册了 handler。**不匹配 → 启动即 panic**（fail fast，不是运行期静默）。

理由：Worker 的 `None` 分支会把未知事件死信。启动时校验能把「配错了」变成启动失败，而不是运行期积累死信。

---

## 七、测试矩阵（todo §9 验收：「四条事件均有生产端与消费端；重复投递不产生副作用；死信重放幂等；监控页或指标可读」）

| # | 用例 | 落点 |
|---|---|---|
| E-01 | `RunCompleted` 事件在 Run 终态**同事务**产生 | `pm-application/tests/outbox_events.rs` |
| E-02 | 同一 Run 的终态只产生**一条** `RunCompleted`（并发收尾坍缩） | 同上 |
| E-03 | `RunCompleted` 的幂等键是 `run_completed:<run_id>` | 同上 |
| E-04 | `RunCompletedHandler` 重复消费**不产生第二份**通知/审计 | `pm-infrastructure/tests/outbox_handlers.rs` |
| E-05 | 死信列表**只返回本项目**的死信（跨项目不可见） | `pm-application/tests/outbox_ops.rs` |
| E-06 | 死信重放把事件放回 `pending` | 同上 |
| E-07 | 死信重放**幂等**：第二次 replay 返回 409（不产生第二条投递） | 同上 |
| E-08 | 对非死信事件 replay 返回 409 | 同上 |
| E-09 | 对不存在事件 replay 返回 404 | 同上 |
| E-10 | 监控端点返回四个状态计数（缺者为 0） | `apps/pm-api/tests/outbox_ops.rs` |
| E-11 | `failure_rate` 分母为 0 时是 `0.0`（不是 `NaN` / `1.0`） | 同上 |
| E-12 | `failure_rate` **不含** `pending` | 同上 |
| E-13 | `oldest_pending_age_secs` 取 `pending` + `processing` 的共同最老 | 同上 |
| E-14 | 监控端点跨租户不可见（404） | 同上 |
| E-15 | `EventCatalog` 与 Worker 注册表**一致**（Worker 启动即校验） | `pm-infrastructure/tests/outbox_worker.rs` |
| E-16 | 死信重放后事件**被真的消费掉**（端到端：replay → dispatch_once → delivered） | `pm-application/tests/outbox_ops.rs` |

---

## 八、裁定记录（2026-11-02 用户确认）

| # | 决策点 | 裁定 | 理由 |
|---|---|---|---|
| Q1 | `RunCompleted` 无业务消费者怎么办 | **记录型 handler**（写 `audit_log` + `notification`，不改业务状态） | 避免「无 handler → 死信」污染死信列表；`integration.notification` 是既有通知通道，T-070 执行中心可直接消费 |
| Q2 | 死信重放端点的权限模型 | **沿用 `require_project_visible`**（同租户 + 项目成员） | 项目内**没有**权限码体系（`grep permission migrations/` 只命中状态机 category 枚举）；权限码只在 MCP 层（`PM_MCP_READ` / `PM_MCP_DOCUMENT_WRITE`）。不新造体系 = YAGNI |
| Q3 | 监控端点的作用域 | **仅单项目**（`project_id` 必填） | 租户级聚合会跨租户边界，与现有 `/v1/pm/*` 契约不一致；运维排障实际总是先定位到项目 |
| Q4 | 死信重放是否跨项目批量 | **否**——一条一次 | YAGNI（§二） |

### 8.1 死信列表的既存缺陷（本次一并修）

`outbox::list_dead_letters` 的 SQL **没有 `project_id` 过滤**——直接暴露到 API 会让调用方看到**别的项目**的死信（含 `last_error` 文本，可能带内部细节）。

**修法**（`AGENTS.md` 规则 1：直接改契约，不留兼容分支）：

```rust
pub async fn list_dead_letters(
    pool: &sqlx::PgPool,
    project_id: uuid::Uuid,
    limit: i64,
) -> Result<Vec<DeadLetterRow>, sqlx::Error>
```

SQL 加 `WHERE status = 'dead_letter' AND project_id = $1`。既有测试（`pm-infrastructure/tests/outbox.rs:132`）同步改。

---

## 九、文件级实施清单

| 文件 | 动作 | 要点 |
|---|---|---|
| `crates/pm-application/src/outbox.rs` | 改 | 新增 `EventSpec` / `CATALOG`；`event_is_enabled` 改查目录；`CATALOG` 里加 `RunCompleted`（`has_handler = true`） |
| `crates/pm-application/src/run_executor.rs` | 改 | `finish_run` / `record_termination` 同事务发 `RunCompleted`（幂等键 `run_completed:<run_id>`） |
| `crates/pm-application/src/outbox_ops.rs` | 新建 | 运维服务层：`dead_letters(pool, project_id, limit)` / `replay(pool, project_id, event_id)` / `stats(pool, project_id)` |
| `crates/pm-infrastructure/src/outbox.rs` | 改 | `list_dead_letters` 加 `project_id` 过滤 |
| `crates/pm-infrastructure/src/outbox_handlers.rs` | 改 | 新增 `RunCompletedHandler`（幂等写 `audit_log` + `notification`） |
| `crates/pm-infrastructure/src/outbox_worker.rs` | 改 | 启动时校验「注册表 vs `CATALOG`」一致，不匹配即 panic（fail fast） |
| `apps/ai-pm-worker/src/main.rs` | 改 | 注册 `RunCompletedHandler` |
| `apps/pm-api/src/outbox_ops.rs` | 新建 | 三个端点：`GET /v1/pm/outbox/dead-letters` / `POST /v1/pm/outbox/events/{id}/replay` / `GET /v1/pm/outbox/stats` |
| `apps/pm-api/src/routes.rs` | 改 | 挂载 `outbox_ops::router()` |
| `crates/pm-application/tests/outbox_events.rs` | 新建 | E-01 ~ E-04（RunCompleted 生产 + 幂等 + handler） |
| `crates/pm-application/tests/outbox_ops.rs` | 新建 | E-05 ~ E-09 + E-16（运维服务层） |
| `apps/pm-api/tests/outbox_ops.rs` | 新建 | E-10 ~ E-14（HTTP 契约） |
| `crates/pm-infrastructure/tests/outbox_worker.rs` | 改 | E-15（目录一致性校验） |
| `crates/pm-infrastructure/tests/outbox.rs` | 改 | `list_dead_letters` 加 `project_id` 参数 |

**不需要新迁移**：`integration.outbox_event` 已有 `attempts` / `dead_letter` / `locked_until` / `idempotency_key` / `tenant_id`；`integration.notification` 已有 `kind` / `payload`。**这是本条的重要结论**——T-061 **不动 schema**。

> **待落地时确认**：`integration.notification.source_proposal_id` 是 `UUID UNIQUE`。`RunCompletedHandler` 插入通知时该列必须为 `NULL`（`notify` 现有实现已经是 `NULL`），否则会撞唯一约束。

---

## 十、风险与已知边界

| # | 风险 | 处置 |
|---|---|---|
| 1 | `list_dead_letters` 加 `project_id` 是**破坏性签名变更** | 依据 `AGENTS.md` 规则 1 直接改；全仓只有 1 处调用（测试）+ 本次新增的 API |
| 2 | `RunCompleted` 事件量可能很大（每个 Run 一条） | 事件表只增不删是既有设计；`delivered` 行会持续累积——**本条不做清理**（清理策略归 T-061 之外的运维议题，或后续切片） |
| 3 | `RunCompletedHandler` 的 `audit_log` 幂等靠什么 | `governance.audit_log` 无唯一约束 → 用 `WHERE NOT EXISTS (SELECT 1 FROM ... WHERE target_id = <run_id> AND action = 'run_completed_event')` 条件 INSERT（落在 `RunCompletedHandler` 的事务里） |
| 4 | `notification` 表会因 `RunCompleted` 增长 | 同 #2；`read_at` 字段已存在，说明通知表本就预期被消费方清理 |
| 5 | Worker 启动即 panic 会不会太激进 | 只在**注册表与目录不一致**时 panic（配置错误）；正常运行路径不受影响 |

---

## 十一、验收标准（对齐 todo.md §9 T-061）

| todo 验收项 | 对应 |
|---|---|
| 四条事件均有生产端与消费端 | `TaskReady` ✅ / `DeliverySubmitted` ✅ / `ReviewCompleted` ✅（生产）→ `KnowledgeBaseWriteRequested` ✅（生产）；`RunCompleted` ✅（生产 + 记录型 handler） |
| 重复投递不产生副作用 | E-04（`RunCompletedHandler` 幂等）、既有 `TaskReady` / `DeliverySubmitted` 幂等用例 |
| 死信重放幂等 | E-07（第二次 replay 409）、E-16（replay → 真的被消费） |
| 监控页或指标可读 | E-10 ~ E-13（`GET /v1/pm/outbox/stats`） |

**通用验收命令**（todo.md §0.2）：

```bash
DATABASE_URL=postgres://aipm:aipm@127.0.0.1:5433/aipm cargo test --workspace
cargo check --workspace --all-targets
```

