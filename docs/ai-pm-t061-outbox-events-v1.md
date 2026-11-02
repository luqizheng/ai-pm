# T-061 · Outbox 业务事件覆盖与死信重放（P8）· 设计稿 V1.1

> **状态**：V1 评审后修订稿，待用户二次确认（V1 于 2026-11-02 评审：架构方向通过，设计稿修改后再确认）
> **任务**：`todo.md` §9 T-061（2026-11-02 领用，执行者：Trae）
> **上位依据**：`target.md` §P8「Outbox 与可靠异步处理」；`docs/ai-pm-task-state-machine-v1.md` §20.5 / §20.6（首版条款与验收标准）；`0010_task_state_machine.sql:123-144`（`integration.outbox_event` 已有 `attempts` / `dead_letter` / `locked_until` / `idempotency_key`）
> **同批已落地（T-043 搭车）**：`DeliverySubmitted` / `ReviewCompleted` / `KnowledgeBaseWriteRequested` 生产者
> **实施约束**：`AGENTS.md` 规则 1 —— 本项目无存量数据，**需要改契约就直接改契约**，不写迁移脚本 / 双写 / 兼容分支 / 回退开关

---

## 〇、V1.1 修订记录（评审核对 + 落点对照）

V1 设计评审结论：**有条件通过，暂不进入实施**。本版逐条收编评审意见，未推翻 V1 架构方向（三层结构、复用现有 Outbox、不新增消息队列、不新增 Schema 均保留）。

| 评审项 | 级别 | 意见摘要 | V1.1 落点 |
|---|---|---|---|
| P0-1 | 必须 | `RunCompleted` 语义前后矛盾（§2「无消费者」vs §5.6 注册 handler）——统一为**有消费者的事实通知事件**（Fact Notification） | §1.2 / §2 / §5.5 / §5.6 / §8 / §11 全部统一 |
| P0-2 | 必须 | `WHERE NOT EXISTS` 不等于并发安全的唯一性——两 Worker 并发同查同插会双写；改用**事务级 advisory lock** 包住「检查 + 写入」，审计与通知同事务原子提交 | §5.6 重写幂等方案 + 事务边界图；E-04 增加并发用例 |
| P0-3 | 必须 | replay 只有 API 层 `require_project_visible` 不能证明底层隔离——契约改 `replay(pool, project_id, event_id)`，**原子 UPDATE 同时校验项目归属**；跨项目与不存在**统一 404**（防事件 ID 探测） | §4.5 / §8.1 重写 |
| P0-4 | 必须 | `AI_PM_OUTBOX_EVENTS` 可绕过 `has_handler` 制造悬空事件——目录优先，环境变量不得突破 `has_handler`；**拆分 `has_handler` 与 `enabled_by_default`** | §6.1 重写 + 既有测试影响标注 |
| P1-1 | 建议 | todo 验收标准「四条事件均有生产端与消费端」与实际裁定不一致——改 5 条 | §11 重写 |
| P1-2 | 建议 | `failure_rate` 命名不准确——实为**当前终态死信占比**（terminal_failure_ratio），非历史失败率 | §4.2 补语义标注 |
| P1-3 | 建议 | 死信列表 `total` 必须是**项目全部死信数**（独立 COUNT），不是 limit 内条数 | §4.4 明确口径 |
| P1-4 | 建议 | 「运维权限」措辞与 Q2 裁定（项目成员可见性）不统一——统一为「项目成员级别运维能力」；重放须记审计 | §2 / §4.5 / §8 统一 + 重放审计要求 |
| §4.1 | 细节 | `read_at` 存在不能证明有清理机制——修正该推论 | §10 风险 #4 重写 |
| §4.2 | 细节 | 审计与通知必须原子提交，不得「审计成功通知失败仍标记 delivered」 | §5.6 事务边界图（与 P0-2 合并） |
| 测试补充 | — | 并发重复消费 / 跨项目重放攻击 / env 绕过阻断 / 原子回滚 / replay 后重复消费 | E-04 / E-09 / E-17 ~ E-21 |
| 架构建议 | — | `EventCatalog` 应成为整个 Outbox 的**契约中心** | §6.3 新增六问 |

评审确认保留、本版不动的裁定：`RunCompleted` 在终态事务中产生、幂等键用 `run_id`、`RunRequested` 不做、`ReviewCompleted` / `KnowledgeBaseWriteRequested` 暂不造 handler、单条死信重放、重放不等待消费完成、`failure_rate` 不统计 `pending`、`processing` 纳入积压年龄、Worker 启动目录校验、不新增消息队列、不新增 Schema。

---

## 一、现状盘点（实测，非推断）

### 1.1 已有能力

| 能力 | 位置 | 状态 |
|---|---|---|
| 事件表 | `integration.outbox_event` | ✅ `0010` 建表；`0019` 补 `tenant_id` + 组合外键 + `UNIQUE (tenant_id, idempotency_key)` |
| 同事务写入 | `pm_application::outbox::PgOutboxAppend` | ✅ `append` 只接受 `&mut Transaction` —— 「状态已变、事件没写」不可表示 |
| Worker 分发 | `pm_infrastructure::outbox_worker::OutboxWorker` | ✅ 领取 → 路由 → 结算；未注册事件**明确死信**（不假装消费） |
| 重试 + 退避 | `pm_infrastructure::outbox::fail` | ✅ 指数退避 `base * 2^attempts`，达 `max_attempts` 转 `dead_letter` |
| 死信查询 | `outbox::list_dead_letters` | ⚠️ **基础设施层已实现，但无任何 API 暴露**，且 SQL 无 `project_id` 过滤（§8.1） |
| 死信重放 | `outbox::replay` | ⚠️ **已实现但无 API 暴露**，且未校验项目归属（P0-3 一并修） |
| 幂等坍缩 | `PgOutboxAppend` 的 `ON CONFLICT (tenant_id, idempotency_key) DO NOTHING` | ✅ 写入时去重 |

### 1.2 六类首批事件的真实状态（`target.md` §P8 清单）

| 事件 | 生产者 | 消费者 | 判定 |
|---|---|---|---|
| `TaskReady` | ✅ `task_dependency.rs:457`（解锁同事务） | ✅ `TaskReadyHandler` → 建 queued Run | **完整** |
| `DeliverySubmitted` | ✅ `delivery/mod.rs:762`（提交同事务） | ✅ `DeliverySubmittedHandler` → D5 拍① | **完整** |
| `ReviewCompleted` | ✅ `review/decide.rs:347` / `:408` | ❌ 无（V1 纯通知） | 生产者有，**无消费者 → 默认禁产**（§6.1） |
| `KnowledgeBaseWriteRequested` | ✅ `review/decide.rs:423` | ❌ 无 | 生产者有，**无消费者 → 默认禁产**（§6.1） |
| `RunCompleted` | ❌ **无 → 本次新增** | ⚠️ **本次新增记录型 handler**（`RunCompletedHandler`） | **本次补齐**（语义见 §5.6） |
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

**未设置该变量时，`ReviewCompleted` / `KnowledgeBaseWriteRequested` 默认不产生**（V1 不注册 handler，避免悬空事件进死信）。测试靠 `set_var` 打开——**这正是一个漏洞**：环境变量分支完全绕过默认清单，管理员配置一个无 handler 的事件类型即可制造悬空事件（P0-4，§6.1 修复）。同时它意味着：**监控端点若要统计这几类事件，必须能区分「没产生」与「产生了但没消费」** —— §四的统计口径据此设计。

---

## 二、范围裁定（2026-11-02 用户确认 + V1.1 评审修订）

| 决策点 | 裁定 | 理由 |
|---|---|---|
| `RunCompleted` 语义 | **有消费者的事实通知事件（Fact Notification）**：生产者 = Run 终态事务；消费者 = `RunCompletedHandler`；消费动作 = 记审计事实 + 生成执行完成通知；**不改 Run 状态、不触发 D5 失败判定、不重新调度 Task** | Run 终态的状态推进已由 D5 失败判定作业扫描 `delivery.task_run` 驱动；再造一个业务状态消费者会与失败判定**抢状态**。它的职责是**记录不可逆业务事实**（某次 Run 已结束）并接入既有通知通道，与 `TaskReady` / `DeliverySubmitted` 的业务职责不混淆 |
| 死信重放形态 | **REST 端点 + 项目成员级运维能力**（V1 取舍：沿用 `require_project_visible`，**不新建运维角色 / 权限码体系**；重放须记审计，见 §4.5） | 复用已存在的 `outbox::list_dead_letters` / `outbox::replay`；走现有 `require_project_visible` 租户隔离；排障是人用场景。**明确**：V1 语义是「项目成员可操作本项目死信」，不是「独立运维角色授权」 |
| 事件监控形态 | **REST 只读端点** | 不引入 Prometheus 依赖；与现有 `/v1/pm/*` 契约一致 |

### 2.1 不做（YAGNI）

- ❌ 不做 `RunRequested`（无业务语义——建 Run 已由 `TaskReady` handler 承担）。
- ❌ 不为 `ReviewCompleted` / `KnowledgeBaseWriteRequested` 造消费者（V1 无落点；按 `target.md`「不为凑数造 Handler」）。**默认禁产**（§6.1），环境变量也不得开启（P0-4）。
- ❌ 不引入消息队列 / 事件总线 / 事件溯源（§20.6 第 7 条）。
- ❌ 不做 Prometheus `/metrics`、不做前端运维页（T-070 已在做执行中心）。
- ❌ 不做批量重放（一条一次，YAGNI）。
- ❌ 不做死信事件的「编辑 payload 后重放」（会破坏事件溯源）。
- ❌ 不做通知清理 / 过期归档（§10 #4：与死信行清理一样，保留与清理策略另行管理，本条只负责**可靠写入**）。

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
│   POST /v1/pm/outbox/events/{id}/replay  幂等重放 + 审计       │
│   GET  /v1/pm/outbox/stats           计数 / 积压 / 失败率       │
└─────────────────────────────────────────────────────────────┘
```

**关键边界**：运维层**只调基础设施层**，不改 outbox 状态机；底层查询与重放**必须带 `project_id` 条件**（P0-3：隔离在 SQL 层保证，不依赖 API 层调用纪律）。生产层与运维层之间没有耦合——`RunCompleted` 生产者不需要知道运维端点的存在。

---

## 四、监控指标与运维端点设计

### 4.1 stats 端点契约

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
| `failure_rate` | `dead_letter / (delivered + dead_letter)`，分母为 0 时取 `0.0`（不是 `NaN`、不是 `1.0`——**没有事件不是「全部失败」**）。**语义标注（P1-2）**：这是**当前终态死信占比**（terminal_failure_ratio），**不是历史失败率，也不是时间窗口失败率**——replay 会把 `dead_letter → pending` 并将 `attempts` 归零，历史失败事实会从当前状态统计中消失。执行中心等下游消费者不得把它当作系统历史可靠性指标 |
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

复用 `outbox::list_dead_letters`，**增加 `project_id` 参数与 `WHERE status = 'dead_letter' AND project_id = $1` 过滤**（修复既存缺陷，见 §8.1）。

响应：`{ "items": [...], "total": N }`，每项含 `id` / `event_type` / `aggregate_type` / `aggregate_id` / `attempts` / `last_error` / `created_at`。

**`total` 口径（P1-3）**：`total` = **该项目死信总数**（不带 limit 的独立 `COUNT` 查询），不是「本次返回的条数」。实现为两条查询——分页查询 + 独立 COUNT——**两条都必须带相同的 `project_id` 条件**。理由：运维需要知道「一共积压了多少死信」才能判断是否需要继续排障，limit 截断的条数没有这个信息量。

### 4.5 重放端点（P0-3 重写：隔离在 SQL 层保证）

```
POST /v1/pm/outbox/events/{event_id}/replay?project_id=<uuid>
```

**底层契约改为显式带项目归属**（`AGENTS.md` 规则 1：直接改契约，不留兼容分支）：

```rust
pub async fn replay(
    pool: &sqlx::PgPool,
    project_id: uuid::Uuid,
    event_id: uuid::Uuid,
) -> Result<ReplayOutcome, sqlx::Error>
```

**原子 UPDATE 同时校验项目归属与状态**（单条 SQL，先到先得，无 TOCTOU 窗口）：

```sql
UPDATE integration.outbox_event
   SET status = 'pending', attempts = 0,
       locked_until = NULL, lease_owner = NULL
 WHERE id = $1 AND project_id = $2 AND status = 'dead_letter'
RETURNING id
```

- `RETURNING` 有行 → 重放成功。
- 0 行 → 需区分三种情况，**但对外的错误分类有刻意设计**：

| 实际情况 | 对外结果 | 理由 |
|---|---|---|
| 事件不存在 | 404 `outbox_event_not_found` | — |
| 事件存在但属于**其他项目** | **404 `outbox_event_not_found`**（与不存在统一） | 防事件 ID 探测：有权限的用户拿别项目的 `event_id` 探测时，得不到「存在但无权」的信息 |
| 事件属于本项目但不是 `dead_letter` | 409 `outbox_event_not_dead_letter` | 活事件仍在飞行中，重放会重复投递 |
| 事件属于本项目且是 `dead_letter` | 200 `{ "replayed": true, "event_id": "..." }` | — |

0 行时的区分查询同样以 `id = $event_id AND project_id = $project_id` 为条件——跨项目的行视同不存在，不回读其任何字段。

**重放审计（P1-4）**：重放是有副作用的管理操作，端点层（`apps/pm-api/src/outbox_ops.rs`）在每次调用后写一条 `governance.audit_log`：`action = 'outbox_dead_letter_replay'`，`actor_id` = 调用者 principal（API 层可得），`project_id` / `target_id`（= event_id）/ `after_data`（含操作结果：`replayed` / `not_found` / `not_dead_letter`）。**成功与失败均记**——失败的探测尝试本身就是排障信息。

**幂等性**（todo 验收项「死信重放幂等」）：`replay` 把 `status` 置 `pending`、`attempts` 归零、清租约。**第二次对同一事件 replay 会返回 409**（它已不是 `dead_letter`）——这是**正确的幂等**：不会产生第二条投递。E-21 进一步验证「replay 后事件被消费时，`RunCompletedHandler` 的幂等锚点保证不产生第二份通知/审计」。

> **边界（保留 V1 裁定）**：`replay` 归零 `attempts` 意味着重放后的事件又有完整 5 次重试预算。如果处理器的 bug 没修，重放会再死信一次——这是**期望行为**（不是无限重试，是「再给 5 次」）。

**重放不保证消费成功**：replay 只是把事件放回队列，**真正的消费由 worker 的下一次 `dispatch_once` 完成**。端点不等待消费完成——那会让 HTTP 请求挂起在 worker 调度节奏上。

---

## 五、`RunCompleted` 生产者与消费者设计

### 5.1 生产落点：两个终态路径

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

这与 `DeliverySubmitted` 的 `delivery_submitted:<run_id>` 同构。**run_id 同时是生产端幂等锚点和消费端幂等锚点**（§5.6），是本事件的唯一幂等坐标。

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

`event_is_enabled("RunCompleted")` 默认开启。理由：它**有消费者**（`RunCompletedHandler`，记录型）——「默认开」不违反「不产生悬空事件」原则。事件是否允许生产由**目录**决定（§6.1），`has_handler = true` 是开启的前置条件。

### 5.6 消费者：`RunCompletedHandler`（记录型，P0-1 定稿语义）

**语义（统一表述，全文以此为准）**：`RunCompleted` 是**有消费者的事实通知事件**——

- **生产者**：Run 终态事务（`finish_run` / `record_termination`）。
- **消费者**：`RunCompletedHandler`。
- **消费动作**：记录审计事实（`governance.audit_log`）+ 生成执行完成通知（`integration.notification`）。
- **不做**：不修改 Run 状态、不触发 D5 失败判定、不重新调度 Task。

它记录的是一个**不可逆的业务事实**：某次 Run 已经结束。前端 `workspace/:projectId/execution` 执行中心（T-070）需要展示「Run 结束」的事实；`integration.notification` 已经是既有通知通道（`/api/v1/projects/{id}/notifications` 读它），T-070 可直接消费。

#### 5.6.1 并发安全的幂等（P0-2：advisory lock，不再依赖裸 `WHERE NOT EXISTS`）

**问题**：Outbox 是至少一次投递——租约过期、重复投递会让两个 Worker 同时处理同一 `RunCompleted`。`WHERE NOT EXISTS` 在「先查后插」之间存在竞态窗口：A、B 都查到不存在 → 都插入 → 双份审计/通知。**`NOT EXISTS` 不等于并发安全的唯一性保证**（`governance.audit_log` 无唯一约束，数据库不会兜底）。

**方案**：事务级 advisory lock 以 `run_id` 为键，串行化「检查 + 写入」窗口——**不新增 Schema、不加唯一约束**（评审倾向；若未来加唯一约束 + `ON CONFLICT DO NOTHING` 亦可，V1 不做）。

**事务边界（评审 §4.2 要求，两项写入原子）**：

```
Worker 领取 RunCompleted
        │
        ▼
开启数据库事务
        │
        ├── SELECT pg_advisory_xact_lock(hashtextextended(<run_id>, 0))
        │      （事务级锁：事务结束自动释放，无需手工归还；
        │        后到的并发 Worker 在此排队）
        │
        ├── 条件 INSERT audit_log（WHERE NOT EXISTS ... run_id）
        │      └── 后到者观察到已存在 → 0 行 → 幂等跳过
        │
        ├── 条件 INSERT notification（WHERE NOT EXISTS ... payload->>'run_id'）
        │      └── 同上
        │
        ▼
提交事务
```

**不变式**：

1. 锁必须覆盖**整个**「幂等检查 + 两处写入」过程。
2. 审计插入与通知插入**同一事务**——任一失败则两项一起回滚，事件**不标记 delivered**（走 `fail()` 退避重试，而不是「半成功」）。E-20 验证。
3. 事务成功 → worker 才 `complete()`；事务失败 → `fail()`。**不存在「审计成功、通知失败、却 delivered」的状态**——该状态在数据库层面不可表示（单事务）。
4. 并发重复消费（两连接同时 handle 同一事件）最终只产生一份审计 + 一份通知。E-17 验证。

**SQL**（与 V1 相同的条件 INSERT，但置于锁内、同一事务）：

```sql
-- 事务开始后第一步：以 run_id 为键取事务级 advisory lock
SELECT pg_advisory_xact_lock(hashtextextended($run_id::text, 0));

-- 审计（幂等锚点：project_id + action + target_id = run_id）
INSERT INTO governance.audit_log
    (project_id, actor_id, action, target_type, target_id, after_data, source_kind, tenant_id)
SELECT $1, $2, 'run_completed_event', 'task_run', $3, $4, 'run_dispatch',
       (SELECT tenant_id FROM core.project WHERE id = $1)
WHERE NOT EXISTS (
    SELECT 1 FROM governance.audit_log
     WHERE target_id = $3 AND action = 'run_completed_event' AND project_id = $1
);

-- 通知（幂等锚点：project_id + kind + payload->>'run_id'）
INSERT INTO integration.notification (project_id, source_proposal_id, kind, payload, tenant_id)
SELECT $1, NULL, 'run_completed', $2,
       (SELECT tenant_id FROM core.project WHERE id = $1)
WHERE NOT EXISTS (
    SELECT 1 FROM integration.notification
     WHERE project_id = $1 AND kind = 'run_completed'
       AND payload->>'run_id' = $3
);
```

`actor_id` 用 `DISPATCH_SERVICE_PRINCIPAL`（`00000000-0000-0000-0000-000000000001`，`0015` 已种子化为 `service` principal）——Run 结束是**平台事实**，不是人的动作。

`integration.notification.source_proposal_id` 是 `UUID UNIQUE`，插入时必须为 `NULL`（`notify` 现有实现已是 `NULL`）。

#### 5.6.2 为什么记录型 handler 不是「凑数」

`target.md` 禁止的是「为凑齐六种事件而提前创建**没有真实业务语义**的 Handler」。`RunCompletedHandler` 有明确消费方（通知流 + 审计检索 + T-070 执行中心），且其幂等键、事务边界、原子性均有真实约束——与「空 handler 假装消费」有本质区别。

---

## 六、`EventCatalog`：事件类型的单一真源（P0-4 重写）

### 6.1 目录优先，环境变量不得突破目录

现状问题：事件类型散落在 `event_is_enabled` 的 match、Worker 的 `.with_handler` 调用、各生产点的字符串字面量里，**三处不同步**就会出「产生了但没人消费」或「消费了但没人产生」。更糟的是 V1 的 `event_is_enabled` 里环境变量分支**直接 return**——设置 `AI_PM_OUTBOX_EVENTS=ReviewCompleted` 即可制造无 handler 事件，随后 Worker 将其死信。这与「不产生悬空事件」原则直接矛盾（P0-4）。

**裁定**：新增 `pm_application::outbox::EventCatalog`，**拆分两个概念**（评审要求）：

- `has_handler`：是否存在真实消费者（事实陈述，不是开关）。
- `enabled_by_default`：目录层默认生产策略。

```rust
/// 事件类型的**单一真源**：哪些类型存在、谁生产、谁消费、默认是否生产。
pub struct EventSpec {
    pub event_type: &'static str,
    /// 是否存在真实消费 handler。`false` 的类型**禁止生产**（§20.6「不假装消费」
    /// ——生产了也没人消费，只会变死信污染运维面板）。
    pub has_handler: bool,
    /// 默认是否生产。环境变量只能在 `has_handler = true` 的集合内收窄，
    /// 不能突破 `has_handler` 限制。
    pub enabled_by_default: bool,
    /// 生产者所在位置（文档用）。
    pub producer: &'static str,
}

pub const CATALOG: &[EventSpec] = &[
    EventSpec { event_type: "TaskReady",                  has_handler: true,  enabled_by_default: true,  producer: "task_dependency.rs" },
    EventSpec { event_type: "DeliverySubmitted",          has_handler: true,  enabled_by_default: true,  producer: "delivery/mod.rs" },
    EventSpec { event_type: "ReviewCompleted",            has_handler: false, enabled_by_default: false, producer: "review/decide.rs" },
    EventSpec { event_type: "KnowledgeBaseWriteRequested", has_handler: false, enabled_by_default: false, producer: "review/decide.rs" },
    EventSpec { event_type: "RunCompleted",               has_handler: true,  enabled_by_default: true,  producer: "run_executor.rs" },
];
```

`event_is_enabled` 改为**目录优先**——未登记或 `has_handler = false` 一律 `false`，环境变量只在 `has_handler = true` 的前提下生效：

```rust
pub fn event_is_enabled(event_type: &str) -> bool {
    // 1. 目录优先：未登记的事件类型一律不生产
    let Some(spec) = CATALOG.iter().find(|spec| spec.event_type == event_type) else {
        return false;
    };
    // 2. 无消费者的事件禁止生产——环境变量不能绕过
    if !spec.has_handler {
        return false;
    }
    // 3. 环境变量：在 has_handler = true 的集合内显式选择
    if let Ok(value) = std::env::var("AI_PM_OUTBOX_EVENTS") {
        return value.split(',').map(str::trim).any(|item| item == event_type);
    }
    // 4. 默认策略
    spec.enabled_by_default
}
```

**对既有行为的影响**：

- `TaskReady` / `DeliverySubmitted` / `RunCompleted`（`has_handler = true`）：未设环境变量时按 `enabled_by_default` 开启——与现状一致（前两者）或为本次新增（RunCompleted）。
- `ReviewCompleted` / `KnowledgeBaseWriteRequested`（`has_handler = false`）：**默认关闭，且环境变量也无法开启**——这是相对现状的行为收紧（现状可经 env 打开）。
- **⚠ 既有测试影响（实施时必须同步改）**：`pm-application/tests/review.rs:305` 的 `enable_notification_events()` 用 `AI_PM_OUTBOX_EVENTS="TaskReady,DeliverySubmitted,ReviewCompleted,KnowledgeBaseWriteRequested"` 打开通知事件。P0-4 修复后 `ReviewCompleted` / `KnowledgeBaseWriteRequested` 无法经 env 启用，依赖它们的断言（T-043 通知事件用例）**会失败**。修法：这些用例改为断言「这两类事件默认不产生（目录禁止）」，通知断言保留给 `has_handler = true` 的事件；测试辅助函数同步收敛。**不做任何让目录开口子的兼容分支**。

### 6.2 悬空事件检测（Worker 侧兜底）

`OutboxWorker` 启动时校验：注册的 handler 类型都在 `CATALOG` 里；`CATALOG` 里 `has_handler = true` 的类型都注册了 handler。**不匹配 → 启动即 panic**（fail fast，不是运行期静默）。

理由：Worker 的 `None` 分支会把未知事件死信。启动时校验能把「配错了」变成启动失败，而不是运行期积累死信。

**补充（实施期发现的部署事实）**：`pm-worker` 与 `ai-pm-worker` 在同一 compose 中竞争**同一张** outbox 表。`pm-worker` 目前是零 handler 空壳——它抢到的事件会被无条件死信，**制造假的「no handler」死信**。装配方案：抽 `register_all_handlers(worker, pool)` 共享装配函数，两个 worker 的 main 都调用——两者注册同一套 handler，谁抢到都能正确消费。目录校验（§6.2）保证「注册表 = 目录」之后，这套装配就是自洽的。

### 6.3 `EventCatalog` 的定位：整个系统 Outbox 的契约中心（评审架构建议）

`EventCatalog` 不只是 Worker 的注册校验表——它是**整个系统 Outbox 事件的契约中心**。`CATALOG` 是编译期常量，V1 承载四个字段；随着 AI Agent、Task、Run、Delivery、Review 等业务事件逐渐增加，**未来每新增一种事件，都必须先在目录里回答六个问题**：

| # | 问题 | V1 承载方式 |
|---|---|---|
| 1 | 谁生产？ | `producer` 字段 |
| 2 | 谁消费？ | `has_handler` 字段（+ Worker 注册表，启动校验一致） |
| 3 | 默认是否启用？ | `enabled_by_default` 字段 |
| 4 | 消费失败如何处理？ | 既有通用策略：退避重试 → 死信（不假装消费） |
| 5 | 是否允许重放？ | 既有通用策略：死信一律可 replay（本设计 §4.5） |
| 6 | 幂等锚点是什么？ | V1 未入目录（各 handler 自带：`TaskReady` 建幂等 Run、`DeliverySubmitted` 拍①幂等、`RunCompleted` 的 run_id）。**V1 不为问题 6 加字段**（YAGNI）——但设计稿记录：当出现第二种幂等模式时，再评估是否升格为目录字段 |

目的：避免 Outbox 随业务膨胀成「无人能说清楚事件生命周期」的地方。V1 的字段集是这六问的最小可用投影，**不预建**问题 4/5/6 的目录化机制。

---

## 七、测试矩阵

todo §9 原验收句「四条事件均有生产端与消费端」按 P1-1 修订为 §十一 的 5 条；测试矩阵对应扩充（**加粗**为 V1.1 新增/重写）。

| # | 用例 | 落点 |
|---|---|---|
| E-01 | `RunCompleted` 事件在 Run 终态**同事务**产生 | `pm-application/tests/outbox_events.rs` |
| E-02 | 同一 Run 的终态只产生**一条** `RunCompleted`（并发收尾坍缩） | 同上 |
| E-03 | `RunCompleted` 的幂等键是 `run_completed:<run_id>` | 同上 |
| E-04 | `RunCompletedHandler` 顺序重复消费**不产生第二份**通知/审计 | `pm-infrastructure/tests/outbox_handlers.rs` |
| **E-17** | **`RunCompletedHandler` 并发重复消费**：两连接同时 handle 同一事件（模拟租约过期重复投递），最终**仅一份**审计 + 一份通知（P0-2） | 同上 |
| **E-20** | **原子回滚**：审计插入成功、通知插入失败（如注入约束冲突）→ **事务整体回滚**，事件不标记 delivered，audit_log 无残留（§5.6.1 不变式 3） | 同上 |
| **E-19** | **环境变量绕过阻断**：设置 `AI_PM_OUTBOX_EVENTS=ReviewCompleted` 后 `event_is_enabled("ReviewCompleted")` 仍为 `false`；未登记类型亦 `false`（P0-4） | `pm-application/tests/outbox_events.rs` |
| E-05 | 死信列表**只返回本项目**的死信（跨项目不可见） | `pm-application/tests/outbox_ops.rs` |
| **E-18a** | **死信列表 `total` = 项目全部死信数**（limit 截断后仍返回全量 COUNT）（P1-3） | 同上 |
| E-06 | 死信重放把事件放回 `pending` | 同上 |
| E-07 | 死信重放**幂等**：第二次 replay 返回 409（不产生第二条投递） | 同上 |
| E-08 | 对非死信事件 replay 返回 409 | 同上 |
| E-09 | 对**不存在或跨项目**事件 replay 均**返回 404**（统一口径，防探测）（P0-3） | 同上 |
| **E-18b** | **跨项目 event_id 重放攻击**：用户对项目 A 有权限，构造项目 B 的死信 event_id + 项目 A 的 project_id 调 replay → 404，且项目 B 事件**未被改动**（P0-3） | `apps/pm-api/tests/outbox_ops.rs` |
| E-10 | 监控端点返回四个状态计数（缺者为 0） | 同上 |
| E-11 | `failure_rate` 分母为 0 时是 `0.0`（不是 `NaN` / `1.0`） | 同上 |
| E-12 | `failure_rate` **不含** `pending` | 同上 |
| E-13 | `oldest_pending_age_secs` 取 `pending` + `processing` 的共同最老 | 同上 |
| E-14 | 监控端点跨租户不可见（404） | 同上 |
| E-15 | `EventCatalog` 与 Worker 注册表**一致**（Worker 启动即校验） | `pm-infrastructure/tests/outbox_worker.rs` |
| E-16 | 死信重放后事件**被真的消费掉**（端到端：replay → dispatch_once → delivered） | `pm-application/tests/outbox_ops.rs` |
| **E-21** | **replay 后重复消费不产生第二份通知**：replay → 两次 dispatch（模拟重复投递）→ 仍只有一份 `RunCompleted` 通知/审计 | 同上 |
| **E-22** | **重放审计**：replay 成功与失败（404/409）均写入 `governance.audit_log`（含操作者、project_id、event_id、结果）（P1-4） | `apps/pm-api/tests/outbox_ops.rs` |

---

## 八、裁定记录（2026-11-02 用户确认 + V1.1 修订）

| # | 决策点 | 裁定 | 理由 |
|---|---|---|---|
| Q1 | `RunCompleted` 无业务状态消费者怎么办 | **记录型 handler**（写 `audit_log` + `notification`，不改业务状态）——语义定稿为「**有消费者的事实通知事件**」（P0-1） | 避免「无 handler → 死信」污染死信列表；`integration.notification` 是既有通知通道，T-070 执行中心可直接消费；Run 状态推进权留在 D5 失败判定作业，不争夺 |
| Q2 | 死信重放端点的权限模型 | **沿用 `require_project_visible`**（同租户 + 项目成员）——**措辞统一**：V1 是「项目成员级别的死信运维能力」，**不是**独立运维角色授权（项目内无权限码体系；`grep permission migrations/` 只命中状态机 category 枚举，权限码只在 MCP 层。不新造体系 = YAGNI）。**配套**：隔离必须在底层 SQL 层二次保证（P0-3），重放记审计（P1-4） | API 层鉴权 + SQL 层项目归属校验，双层防线；操作留痕弥补无独立运维角色的追溯需要 |
| Q3 | 监控端点的作用域 | **仅单项目**（`project_id` 必填） | 租户级聚合会跨租户边界，与现有 `/v1/pm/*` 契约不一致；运维排障实际总是先定位到项目 |
| Q4 | 死信重放是否跨项目批量 | **否**——一条一次 | YAGNI（§二） |
| Q5 | `RunCompletedHandler` 并发幂等机制 | **事务级 `pg_advisory_xact_lock`（键 = run_id）+ 同事务条件 INSERT**（P0-2） | `WHERE NOT EXISTS` 有检查-插入竞态窗口，与至少一次投递冲突；advisory lock 不新增 Schema、不加唯一约束，事务结束自动释放 |
| Q6 | 无 handler 事件能否经环境变量开启 | **否**（P0-4） | 目录优先；`has_handler = false` 一律禁产，env 只能在 `has_handler = true` 集合内收窄——「不产生悬空事件」不可配置突破 |

### 8.1 死信查询与重放的既存隔离缺陷（本次一并修，P0-3 扩大范围）

V1 只发现 `list_dead_letters` 缺 `project_id` 过滤；评审指出 **`replay` 有相同性质的问题**——仅在 API 层调用 `require_project_visible` 不能证明底层只操作该事件。两处同修（`AGENTS.md` 规则 1：直接改契约，不留兼容分支）：

```rust
// 查询：加项目过滤
pub async fn list_dead_letters(
    pool: &sqlx::PgPool,
    project_id: uuid::Uuid,
    limit: i64,
) -> Result<Vec<DeadLetterRow>, sqlx::Error>   // total 由调用方经独立 COUNT 获取（P1-3）

// 重放：契约显式带项目归属 + 原子 UPDATE（§4.5）
pub async fn replay(
    pool: &sqlx::PgPool,
    project_id: uuid::Uuid,
    event_id: uuid::Uuid,
) -> Result<ReplayOutcome, sqlx::Error>
```

既有测试（`pm-infrastructure/tests/outbox.rs:132`）同步改。

---

## 九、文件级实施清单

| 文件 | 动作 | 要点 |
|---|---|---|
| `crates/pm-application/src/outbox.rs` | 改 | 新增 `EventSpec`（含 `has_handler` / `enabled_by_default` 两字段）/ `CATALOG`；`event_is_enabled` 改**目录优先**（env 不得突破 `has_handler`，§6.1） |
| `crates/pm-application/src/run_executor.rs` | 改 | `finish_run` / `record_termination` 同事务发 `RunCompleted`（幂等键 `run_completed:<run_id>`） |
| `crates/pm-application/src/outbox_ops.rs` | 新建 | 运维服务层：`dead_letters(pool, project_id, limit)` + 独立 `dead_letter_total(pool, project_id)` / `replay(pool, project_id, event_id)` / `stats(pool, project_id)` |
| `crates/pm-infrastructure/src/outbox.rs` | 改 | `list_dead_letters` 加 `project_id` 过滤；`replay` 契约改 `(pool, project_id, event_id)` + **原子 UPDATE**（`WHERE id AND project_id AND status='dead_letter'`，§4.5） |
| `crates/pm-infrastructure/src/outbox_handlers.rs` | 改 | 新增 `RunCompletedHandler`（**advisory lock 幂等**，§5.6.1）+ `register_all_handlers(worker, pool)` 共享装配 |
| `crates/pm-infrastructure/src/outbox_worker.rs` | 改 | 启动时校验「注册表 vs `CATALOG`」一致，不匹配即 panic（fail fast） |
| `apps/ai-pm-worker/src/main.rs` | 改 | 改用 `register_all_handlers`（注册 `TaskReady` / `DeliverySubmitted` / `RunCompleted`） |
| `apps/pm-worker/src/main.rs` | 改 | 同上——消除「零 handler 空壳抢事件 → 假死信」（§6.2 补充） |
| `apps/pm-api/src/outbox_ops.rs` | 新建 | 三个端点 + **重放审计**（§4.5）：`GET /v1/pm/outbox/dead-letters` / `POST /v1/pm/outbox/events/{id}/replay` / `GET /v1/pm/outbox/stats` |
| `apps/pm-api/src/routes.rs` | 改 | 挂载 `outbox_ops::router()` |
| `crates/pm-application/tests/outbox_events.rs` | 新建 | E-01 ~ E-03 + E-19（目录/env 语义） |
| `crates/pm-application/tests/outbox_ops.rs` | 新建 | E-05 ~ E-09 + E-16 + E-18a + E-21 |
| `apps/pm-api/tests/outbox_ops.rs` | 新建 | E-10 ~ E-14 + E-18b + E-22（HTTP 契约） |
| `crates/pm-infrastructure/tests/outbox_handlers.rs` | 新建 | E-04 + E-17（并发）+ E-20（原子回滚） |
| `crates/pm-infrastructure/tests/outbox_worker.rs` | 改 | E-15（目录一致性校验） |
| `crates/pm-infrastructure/tests/outbox.rs` | 改 | `list_dead_letters` / `replay` 签名同步改 |
| `crates/pm-application/tests/review.rs` | 改 | **P0-4 连带**：`enable_notification_events()` 不再能开启 `ReviewCompleted` / `KnowledgeBaseWriteRequested`；相关 T-043 断言改为「默认不产生」（§6.1 影响标注） |

**不需要新迁移**：`integration.outbox_event` 已有 `attempts` / `dead_letter` / `locked_until` / `idempotency_key` / `tenant_id`；`integration.notification` 已有 `kind` / `payload`；`governance.audit_log` 无唯一约束 → 幂等靠 advisory lock 而非 DDL。**这是本条的重要结论**——T-061 **不动 schema**（P0-2 的 advisory lock 方案正是为此选型）。

> **待落地时确认**：`integration.notification.source_proposal_id` 是 `UUID UNIQUE`。`RunCompletedHandler` 插入通知时该列必须为 `NULL`（`notify` 现有实现已经是 `NULL`），否则会撞唯一约束。

---

## 十、风险与已知边界

| # | 风险 | 处置 |
|---|---|---|
| 1 | `list_dead_letters` / `replay` 加 `project_id` 是**破坏性签名变更** | 依据 `AGENTS.md` 规则 1 直接改；全仓只有少量测试调用 + 本次新增的 API |
| 2 | `RunCompleted` 事件量可能很大（每个 Run 一条） | 事件表只增不删是既有设计；`delivered` 行会持续累积——**本条不做清理**（清理策略归本条之外的运维议题，或后续切片） |
| 3 | `RunCompletedHandler` 的并发幂等靠什么（V1 的 `WHERE NOT EXISTS` 有竞态） | **改**：事务级 `pg_advisory_xact_lock(hashtextextended(run_id, 0))` 包住「检查 + 两处插入」，事务结束自动释放；并发重复消费由 E-17 验证（§5.6.1） |
| 4 | `notification` 表会因 `RunCompleted` 增长 | 同 #2；**V1.1 修正措辞**：`read_at` 字段的存在只能证明有已读状态，**不能证明存在定期清理任务**——不将 `read_at` 当作数据自动清理机制。**本次不做清理**；通知保留与清理策略另行管理。T-061 对通知的职责边界 = **可靠写入** |
| 5 | Worker 启动即 panic 会不会太激进 | 只在**注册表与目录不一致**时 panic（配置错误）；正常运行路径不受影响 |
| 6 | P0-4 行为收紧影响既有 T-043 测试 | 实施清单已列 `review.rs` 同步改（断言「默认不产生」）；**不为旧测试给目录开兼容口子**（`AGENTS.md` 规则 1 判据：「以前是这样」不是存在理由） |
| 7 | advisory lock 的键碰撞 | `hashtextextended` 是 64 位 hash，两个不同 run_id 碰撞仅导致**多余串行化**（不会错误跳过写入——条件 INSERT 的判定键仍是 run_id 本身），正确性不受影响 |

---

## 十一、验收标准（P1-1：替换 todo.md 原句「四条事件均有生产端与消费端」）

todo.md §9 原验收句与实际裁定不符（五种事件里两种明确无消费者），按评审修订为以下 5 条：

| # | 验收标准 | 对应验证 |
|---|---|---|
| 1 | 所有**默认启用**的事件都有生产者和消费者 | `TaskReady` / `DeliverySubmitted` / `RunCompleted`（§1.2 / §6.1 目录） |
| 2 | 没有消费者的事件**默认禁止生产**（环境变量亦不可开启） | `ReviewCompleted` / `KnowledgeBaseWriteRequested`（E-19） |
| 3 | 所有已启用事件都能通过**目录验证**（Worker 启动一致性校验） | E-15 |
| 4 | `RunCompleted` 有**记录型 Handler**（事实通知事件：审计 + 通知，不改业务状态） | E-01 ~ E-04 / E-17 / E-20 |
| 5 | 重复投递**不产生重复业务副作用** | E-04 / E-17 / E-20 / E-21 + 既有 `TaskReady` / `DeliverySubmitted` 幂等用例 |

todo §9 其余验收项不变：

| todo 验收项 | 对应 |
|---|---|
| 死信重放幂等 | E-07（第二次 replay 409）、E-16（replay → 真的被消费）、E-09/E-18b（隔离与防探测）、E-22（重放审计） |
| 监控页或指标可读 | E-10 ~ E-13（`GET /v1/pm/outbox/stats`） |

**通用验收命令**（todo.md §0.2）：

```bash
DATABASE_URL=postgres://aipm:aipm@127.0.0.1:5433/aipm cargo test --workspace
cargo check --workspace --all-targets
```
