# 任务状态机 V1 · Slice B（持久层 + Outbox）实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 Slice A 的纯域模型落到 PostgreSQL：**一个** `0010` 迁移（Run、等待、取代关系、任务级策略覆盖、Outbox 五组表 + `delivery.task` 新列 + 13 值 status 约束 + 默认值），并交付首版最小可靠 Outbox（同事务写入、租约领取、退避重试、死信与人工重放）。

**Architecture:** **新系统，库里没有存量业务数据，开发库可以直接 drop 重建**，因此迁移文件里**不含任何回填语句**：`0010` 一次性建全表、加列、改默认值、加 CHECK。开发库按「drop database → 重新 migrate → 重放 `scripts/seed-demo.sql`」处置。Outbox 落在 `pm-infrastructure`（`outbox.rs` 负责持久化状态机，`outbox_worker.rs` 负责注册表与分发循环），`apps/pm-worker` 只做进程接线，不承载业务规则。

**Tech Stack:** Rust 1.99 / sqlx 0.8（`runtime-tokio` + `postgres` + `migrate`，无 `query!` 离线模式）/ PostgreSQL 16（`aipm-pg-dev`，`localhost:5433`）/ async-trait（workspace 既有依赖，零新增 crate）。

**Spec:** [docs/tasks/ai-pm-task-state-machine-v1.md](docs/tasks/ai-pm-task-state-machine-v1.md)（V12）——§九 M4（状态存 lowercase snake_case）、§11.5（取代关系模型）、§12.4（等待上下文分离存储 + 历史保留）、§19.4/§19.5（Run 失败记录与重试计数）、§20.2–§20.6（Outbox 首版范围、结构与验收标准）、§21.1（DB 存 lowercase snake_case TEXT）；关联 [docs/architecture/ai-pm-multi-repo-model-v1.md](docs/architecture/ai-pm-multi-repo-model-v1.md) V1.1（§7.4 聚合口径已统一到状态机稿 §十三）。**§九 M1–M3（legacy 回填口径）不适用**：无存量数据可迁。

## Global Constraints

- **13 值 status 全集**（§12.4，顺序即权威序）：`draft, ready, claimed, running, awaiting_decision, blocked, submitted, reviewing, completed, changes_requested, failed, cancelled, superseded`。DB 存 lowercase snake_case TEXT（§21.1 第 2 条）。
- **终态四值** `completed / cancelled / failed / superseded` 零出边——DB 层不为此建约束（迁移矩阵属域层 Slice A），本 Slice 只保证存储不吞值。
- **等待四原因**（§12.4）：`needs_info, needs_decision, needs_approval, external_dependency`；同一任务同时只允许一条**活跃**等待记录，历史记录保留不删除。
- **Run 四值结局**（§19.5）：`succeeded, failed, timed_out, cancelled`；Run 自身状态机：`queued, running, succeeded, failed, timed_out, cancelled`。
- **失败七类别**（§19.5）：`infrastructure, execution, validation, permission, policy_violation, missing_dependency, unknown`。
- **重试默认值**（§19.4）：`max_run_attempts = 3`（含首次）、`max_consecutive_failures = 3`、`run_timeout = 30 分钟`、`task_deadline = 2 小时`——本 Slice 只落**存储位**（`attempt_count` / `consecutive_failure_count` / `retry_exhausted` / `last_failure_category`），判定逻辑留 Slice C。
- **失败记录不可变**（§19.5）：`task_run_failure` 每 Run 至多一条，以 `run_id` 为主键；`BEFORE UPDATE OR DELETE` 触发器直接抛异常；重试**新建 Run**，绝不覆盖旧记录。
- **Outbox 四状态**（§20.4）：`pending, processing, delivered, dead_letter`；必须预留 `idempotency_key`（全局唯一）、`locked_until`、`schema_version`。
- **Outbox 默认参数**（本计划钉死，实施时集中为常量）：`lease = 60s`、`max_attempts = 5`、`backoff_base = 5s`（`next_attempt_at = now() + base * 2^attempts`）、空闲轮询 `100ms`。
- **同事务写入**（§20.6 第 1 条）：`append` 只接受 `&mut Transaction`，不接受 `&PgPool`——从签名上杜绝「业务提交了、事件丢了」。
- **至少一次投递**（§20.6 第 3 条）：消费者必须幂等；DB 侧以 `idempotency_key` 唯一约束保证同一语义事件只落一行。
- **迁移文件一经提交不可改写**：sqlx 会校验已应用迁移的 checksum；任何后续变更一律新建 `0011+`。
- **无数据回填**：库内没有存量业务数据，**开发库可随时 drop 重建**，`0010` 只含 DDL + 默认值 + 约束；开发库按「drop → migrate → 重放 `scripts/seed-demo.sql`」处置（见 Task 1 Step 6）。
- 既有约束不变：无新增 crate 依赖、每个任务 TDD（先失败测试）、每任务结束即 commit（只落本地）。

## Decision Gate（实施前需需求方确认）

无存量数据后，原 D1–D3（legacy 回填口径）**不再存在**。只剩一项设计取舍：

| # | 待确认 | 推荐默认 | 理由 |
| --- | --- | --- | --- |
| D4 | Outbox 遇到没有注册 handler 的事件类型 | 直接 `dead_letter`，原因 `no handler registered for <type>` | 静默标记 `delivered` 会让事件永久丢失；首版六个事件类型的真实 handler 依赖 Slice C/D 的调度与知识回写 |

## Review Focus

1. **演示数据/开发库残留旧值**：新系统虽无业务数据，但 dev 库可能有 `seed-demo.sql` 灌入的 `backlog`/`in_review` 行，`0010` 的 CHECK 会让这些行在下次写入时被拒。Task 1 的 Step 6 直接 drop 重建开发库并以新值重放种子，再让 `pm-api` 启动跑迁移。
2. **CHECK 漏值导致线上 500**：13 值必须逐值插入成功。Task 1 用一条循环测试覆盖全部 13 个状态，另加 `backlog`/`in_progress`/`done`/`in_review`/`needs_info` 五个旧值各一条拒绝断言。
3. **约束切换与写路径不同步**：`create_task`、`CreateTask` 命令处理器、`seed-demo.sql` 仍在写 `'backlog'`；Task 1 Step 4 必须同 commit 改完，否则新数据库上创建任务直接 500。
4. **Outbox 事件在业务提交后丢失**：`append` 若接受 `&PgPool` 就会有人在事务外调用。Task 2 的测试必须在一个**回滚**事务里 append 后断言查不到，在**提交**事务里 append 后断言可查。
5. **重复投递变成重复执行**：同一 `idempotency_key` 二次 append 必须被唯一约束拒绝；租约过期后 `claim` 会重投**同一行**（`id` 不变），测试用 `lease_secs = 0` 复现。

## 文件结构

| 动作 | 路径 | 职责 |
| --- | --- | --- |
| Create | `migrations/0010_task_state_machine.sql` | 建表（`task_run` / `task_run_failure` 含不可变触发器 / `task_supersession` / `task_waiting` / `task_policy_override` / `outbox_event`）+ `delivery.task` 新列 + `status` 默认值 `draft` + 13 值 CHECK + 失败类别 CHECK（**不含回填语句**） |
| Create | `crates/pm-infrastructure/src/outbox.rs` | Outbox 持久化状态机：`append` / `claim` / `complete` / `fail` / `dead_letter` / `replay` / `list_dead_letters` |
| Create | `crates/pm-infrastructure/src/outbox_worker.rs` | `OutboxHandler` 端口 + 事件类型注册表 + `dispatch_once` 单步分发 |
| Modify | `crates/pm-infrastructure/src/lib.rs` | 导出 `outbox` / `outbox_worker` 及其公开类型 |
| Modify | `crates/pm-infrastructure/Cargo.toml` | 增加 `serde_json` / `chrono` / `async-trait` / `thiserror` / `tracing`（均为 workspace 既有） |
| Modify | `crates/pm-application/src/pm_write.rs:16-17,263,290-294` | `TASK_STATUSES` → 13 值；`create_task` 落 `draft`；文档注释同步 |
| Modify | `crates/pm-application/src/command_executor.rs:455-490` | `CreateTask` 处理器与补偿快照改 `draft` |
| Modify | `apps/pm-worker/{Cargo.toml,src/main.rs}` | Outbox Worker 进程接线（替换空壳 `println!`） |
| Modify | 既有测试与 `scripts/seed-demo.sql` | 旧状态字面量机械替换（清单见 Task 1 Step 4） |
| Create | `crates/pm-infrastructure/tests/migration_0009.rs` | 新表/新列/约束/触发器/默认值 |
| Create | `crates/pm-infrastructure/tests/outbox.rs` | 写入/领取/完成/失败/死信/重放 |
| Create | `crates/pm-infrastructure/tests/outbox_worker.rs` | 注册表分发、未知类型、空闲 |

---

### Task 1 — 0010：表结构、状态词表与写路径切换

**Files:**
- Create: `migrations/0010_task_state_machine.sql`
- Create: `crates/pm-infrastructure/tests/migration_0009.rs`
- Modify: `crates/pm-application/src/pm_write.rs:16-17,263,290-294`
- Modify: `crates/pm-application/src/command_executor.rs:455-490`
- Modify: 既有测试与 `scripts/seed-demo.sql`（Step 4 清单）

**Interfaces:**
- Consumes: 既有 schema（`delivery.task(project_id, id)` 唯一键、`core.principal`、`core.project`）；`sqlx::test(migrations = "../../migrations")` 既有惯例。
- Produces: 表 `delivery.task_run` / `delivery.task_run_failure` / `delivery.task_supersession` / `delivery.task_waiting` / `delivery.task_policy_override` / `integration.outbox_event`；`delivery.task` 新列 `superseded_by_task_id, attempt_count, consecutive_failure_count, last_failure_category, retry_exhausted`；`delivery.task.status` 默认值 `draft`、约束 `task_status_valid`（13 值）与 `task_failure_category_valid`（7 值或 NULL）；`pm_write::TASK_STATUSES` 变为 13 值常量（Slice C 的 PATCH 禁改 status 仍另行实施）。Task 2/3/4 只依赖这些名字。

- [x] **Step 1: 写失败测试**

```rust
// crates/pm-infrastructure/tests/migration_0009.rs
use sqlx::PgPool;
use uuid::Uuid;

async fn seed_task(pool: &PgPool, status: Option<&str>) -> (Uuid, Uuid) {
    let customer_id: Uuid =
        sqlx::query_scalar("INSERT INTO core.customer (name) VALUES ('m9-cust') RETURNING id")
            .fetch_one(pool).await.unwrap();
    let project_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name) VALUES ($1, 'M9-1', 'm9') RETURNING id",
    ).bind(customer_id).fetch_one(pool).await.unwrap();
    let task_id: Uuid = match status {
        Some(s) => sqlx::query_scalar(
            "INSERT INTO delivery.task (project_id, title, status) VALUES ($1, 't', $2) RETURNING id",
        ).bind(project_id).bind(s).fetch_one(pool).await.unwrap(),
        None => sqlx::query_scalar(
            "INSERT INTO delivery.task (project_id, title) VALUES ($1, 't') RETURNING id",
        ).bind(project_id).fetch_one(pool).await.unwrap(),
    };
    (project_id, task_id)
}

#[sqlx::test(migrations = "../../migrations")]
async fn task_row_carries_run_budget_columns(pool: PgPool) {
    let (project_id, task_id) = seed_task(&pool, None).await;
    let row: (i32, i32, Option<String>, bool) = sqlx::query_as(
        "SELECT attempt_count, consecutive_failure_count, last_failure_category, retry_exhausted \
         FROM delivery.task WHERE project_id = $1 AND id = $2",
    ).bind(project_id).bind(task_id).fetch_one(&pool).await.unwrap();
    assert_eq!(row, (0, 0, None, false));
    assert!(
        sqlx::query("SELECT superseded_by_task_id FROM delivery.task WHERE id = $1")
            .execute(&pool).await.is_ok(),
        "superseded_by_task_id column must exist"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn task_run_attempt_numbers_are_unique_per_task(pool: PgPool) {
    let (project_id, task_id) = seed_task(&pool, None).await;
    sqlx::query(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no) VALUES ($1, $2, 1)",
    ).bind(project_id).bind(task_id).execute(&pool).await.unwrap();
    let dup = sqlx::query(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no) VALUES ($1, $2, 1)",
    ).bind(project_id).bind(task_id).execute(&pool).await;
    assert!(dup.is_err(), "second attempt_no = 1 must violate UNIQUE (task_id, attempt_no)");
}

#[sqlx::test(migrations = "../../migrations")]
async fn task_supersession_rejects_self_and_duplicate_edges(pool: PgPool) {
    let (project_id, task_id) = seed_task(&pool, None).await;
    let (_, new_id) = seed_task(&pool, None).await;
    let actor: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('human', 'u') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let self_edge = sqlx::query(
        "INSERT INTO delivery.task_supersession (project_id, old_task_id, new_task_id, reason, created_by) \
         VALUES ($1, $2, $2, 'self', $3)",
    ).bind(project_id).bind(task_id).bind(actor).execute(&pool).await;
    assert!(self_edge.is_err(), "old_task_id = new_task_id must be rejected");
    for _ in 0..2 {
        let _ = sqlx::query(
            "INSERT INTO delivery.task_supersession (project_id, old_task_id, new_task_id, reason, created_by) \
             VALUES ($1, $2, $3, 'replaced', $4)",
        ).bind(project_id).bind(task_id).bind(new_id).bind(actor).execute(&pool).await;
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_supersession WHERE old_task_id = $1",
    ).bind(task_id).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1, "duplicate edges must collapse into one row");
}

#[sqlx::test(migrations = "../../migrations")]
async fn task_waiting_allows_one_active_row_per_task(pool: PgPool) {
    let (project_id, task_id) = seed_task(&pool, None).await;
    let insert = "INSERT INTO delivery.task_waiting (project_id, task_id, reason, questions) \
                  VALUES ($1, $2, 'needs_info', '[\\"which cache?\\"]')";
    sqlx::query(insert).bind(project_id).bind(task_id).execute(&pool).await.unwrap();
    let second = sqlx::query(insert).bind(project_id).bind(task_id).execute(&pool).await;
    assert!(second.is_err(), "a task may have only one active waiting row");
    sqlx::query(
        "UPDATE delivery.task_waiting SET ended_at = now() WHERE task_id = $1 AND ended_at IS NULL",
    ).bind(task_id).execute(&pool).await.unwrap();
    assert!(
        sqlx::query(insert).bind(project_id).bind(task_id).execute(&pool).await.is_ok(),
        "after closing the active row a new one must be allowed (history is kept)"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn task_run_failure_is_append_only(pool: PgPool) {
    let (project_id, task_id) = seed_task(&pool, None).await;
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status) \
         VALUES ($1, $2, 1, 'failed') RETURNING id",
    ).bind(project_id).bind(task_id).fetch_one(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_run_failure (run_id, project_id, category, retryable, reason_code) \
         VALUES ($1, $2, 'execution', true, 'exit_code_1')",
    ).bind(run_id).bind(project_id).execute(&pool).await.unwrap();
    let update = sqlx::query("UPDATE delivery.task_run_failure SET retryable = false WHERE run_id = $1")
        .bind(run_id).execute(&pool).await;
    assert!(update.is_err(), "failure records must be immutable");
    let delete = sqlx::query("DELETE FROM delivery.task_run_failure WHERE run_id = $1")
        .bind(run_id).execute(&pool).await;
    assert!(delete.is_err(), "failure records must not be deletable");
}

#[sqlx::test(migrations = "../../migrations")]
async fn outbox_defaults_to_pending_and_rejects_duplicate_idempotency_key(pool: PgPool) {
    let (project_id, _) = seed_task(&pool, None).await;
    let insert = "INSERT INTO integration.outbox_event \
                  (project_id, event_type, aggregate_type, aggregate_id, payload, idempotency_key) \
                  VALUES ($1, 'TaskReady', 'Task', 'agg-1', '{}', 'k-1')";
    let id: Uuid = sqlx::query_scalar(insert).bind(project_id).fetch_one(&pool).await.unwrap();
    let row: (String, i32, i32, i32) = sqlx::query_as(
        "SELECT status, attempts, max_attempts, schema_version FROM integration.outbox_event WHERE id = $1",
    ).bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(row, ("pending".to_string(), 0, 5, 1));
    assert!(
        sqlx::query(insert).bind(project_id).execute(&pool).await.is_err(),
        "idempotency_key must be globally unique"
    );
}

async fn seed_project_only(pool: &PgPool) -> Uuid {
    let customer_id: Uuid =
        sqlx::query_scalar("INSERT INTO core.customer (name) VALUES ('m9c') RETURNING id")
            .fetch_one(pool).await.unwrap();
    sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name) VALUES ($1, 'M9-C', 'm9c') RETURNING id",
    ).bind(customer_id).fetch_one(pool).await.unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn constraint_accepts_all_thirteen_values_and_rejects_legacy_ones(pool: PgPool) {
    let project_id = seed_project_only(&pool).await;

    for status in [
        "draft", "ready", "claimed", "running", "awaiting_decision", "blocked",
        "submitted", "reviewing", "completed", "changes_requested", "failed",
        "cancelled", "superseded",
    ] {
        sqlx::query("INSERT INTO delivery.task (project_id, title, status) VALUES ($1, $2, $3)")
            .bind(project_id).bind(format!("ok {status}")).bind(status)
            .execute(&pool).await.unwrap_or_else(|e| panic!("{status} must be accepted: {e}"));
    }
    for legacy in ["backlog", "in_progress", "in_review", "done", "needs_info"] {
        let err = sqlx::query(
            "INSERT INTO delivery.task (project_id, title, status) VALUES ($1, $2, $3)",
        ).bind(project_id).bind(format!("legacy {legacy}")).bind(legacy)
         .execute(&pool).await;
        assert!(err.is_err(), "{legacy} must be rejected by the 13-state CHECK");
    }

    let default_status: String = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title) VALUES ($1, 'default') RETURNING status",
    ).bind(project_id).fetch_one(&pool).await.unwrap();
    assert_eq!(default_status, "draft");
}

#[sqlx::test(migrations = "../../migrations")]
async fn failure_category_constraint_accepts_seven_values(pool: PgPool) {
    let project_id = seed_project_only(&pool).await;
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status) VALUES ($1, 'cat', 'failed') RETURNING id",
    ).bind(project_id).fetch_one(&pool).await.unwrap();
    for category in [
        "infrastructure", "execution", "validation", "permission",
        "policy_violation", "missing_dependency", "unknown",
    ] {
        sqlx::query("UPDATE delivery.task SET last_failure_category = $2 WHERE id = $1")
            .bind(task_id).bind(category).execute(&pool).await.unwrap();
    }
    assert!(
        sqlx::query("UPDATE delivery.task SET last_failure_category = 'bogus' WHERE id = $1")
            .bind(task_id).execute(&pool).await.is_err(),
        "unknown failure categories must be rejected"
    );
}
```

- [x] **Step 2: 跑测试确认失败**

Run: `bash scripts/cargo-test.sh -p pm-infrastructure`
Expected: FAIL —— `relation "delivery.task_run" does not exist`；且 `status='backlog'` 仍可插入、`last_failure_category='bogus'` 被接受、默认值仍是 `backlog`。

- [x] **Step 3: 写 `migrations/0010_task_state_machine.sql`**

按以下顺序写全（列名、类型、约束与索引逐字照录；**最后一个语句块是默认值与 CHECK，紧跟在建表之后，文件内没有任何 UPDATE/INSERT 回填**）：

```sql
-- 0010_task_state_machine: Run / 等待 / 取代关系 / 任务级策略覆盖 / Outbox
--                          + 13 值 status 约束与默认值（新系统，无回填）
-- 规格：docs/tasks/ai-pm-task-state-machine-v1.md §11.5 §12.4 §19.5 §20.4 §21.1

ALTER TABLE delivery.task
    ADD COLUMN superseded_by_task_id UUID,
    ADD COLUMN attempt_count INT NOT NULL DEFAULT 0,
    ADD COLUMN consecutive_failure_count INT NOT NULL DEFAULT 0,
    ADD COLUMN last_failure_category TEXT,
    ADD COLUMN retry_exhausted BOOLEAN NOT NULL DEFAULT false;

ALTER TABLE delivery.task
    ADD CONSTRAINT fk_task_superseded_by
    FOREIGN KEY (project_id, superseded_by_task_id)
    REFERENCES delivery.task(project_id, id);

CREATE TABLE delivery.task_run (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL,
    task_id UUID NOT NULL,
    attempt_no INT NOT NULL CHECK (attempt_no >= 1),
    stage TEXT,                       -- Q9.1 未裁阶段清单，先留空
    status TEXT NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued', 'running', 'succeeded', 'failed', 'timed_out', 'cancelled')),
    outcome TEXT
        CHECK (outcome IN ('succeeded', 'failed', 'timed_out', 'cancelled')),
    timeout_seconds INT,
    policy_snapshot JSONB,
    policy_version INT,
    override_version BIGINT,
    policy_hash TEXT,
    evaluated_at TIMESTAMPTZ,
    repository_policy_snapshots JSONB,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (task_id, attempt_no),
    FOREIGN KEY (project_id, task_id) REFERENCES delivery.task(project_id, id)
);
CREATE INDEX idx_task_run_task_status ON delivery.task_run(task_id, status);
CREATE INDEX idx_task_run_active ON delivery.task_run(task_id)
    WHERE status IN ('queued', 'running');

CREATE TABLE delivery.task_run_failure (
    run_id UUID PRIMARY KEY REFERENCES delivery.task_run(id),
    project_id UUID NOT NULL,
    category TEXT NOT NULL CHECK (category IN (
        'infrastructure', 'execution', 'validation', 'permission',
        'policy_violation', 'missing_dependency', 'unknown')),
    retryable BOOLEAN NOT NULL,
    reason_code TEXT NOT NULL,
    exit_code INT,
    timeout_seconds INT,
    evidence_ref TEXT,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE OR REPLACE FUNCTION delivery.reject_mutation_of_failure_record()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'delivery.task_run_failure is append-only';
END
$$;

CREATE TRIGGER task_run_failure_immutable
    BEFORE UPDATE OR DELETE ON delivery.task_run_failure
    FOR EACH ROW EXECUTE FUNCTION delivery.reject_mutation_of_failure_record();

CREATE TABLE delivery.task_supersession (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL,
    old_task_id UUID NOT NULL,
    new_task_id UUID NOT NULL,
    reason TEXT NOT NULL CHECK (length(btrim(reason)) > 0),
    created_by UUID NOT NULL REFERENCES core.principal(id),
    approval_request_id UUID REFERENCES governance.approval_request(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (old_task_id, new_task_id),
    CHECK (old_task_id <> new_task_id),
    FOREIGN KEY (project_id, old_task_id) REFERENCES delivery.task(project_id, id),
    FOREIGN KEY (project_id, new_task_id) REFERENCES delivery.task(project_id, id)
);
CREATE INDEX idx_task_supersession_new ON delivery.task_supersession(new_task_id);

CREATE TABLE delivery.task_waiting (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL,
    task_id UUID NOT NULL,
    reason TEXT NOT NULL CHECK (reason IN (
        'needs_info', 'needs_decision', 'needs_approval', 'external_dependency')),
    questions JSONB NOT NULL DEFAULT '[]',
    requested_from UUID REFERENCES core.principal(id),
    since TIMESTAMPTZ NOT NULL DEFAULT now(),
    deadline TIMESTAMPTZ,
    ended_at TIMESTAMPTZ,
    ended_by UUID REFERENCES core.principal(id),
    resolution_note TEXT,
    FOREIGN KEY (project_id, task_id) REFERENCES delivery.task(project_id, id)
);
CREATE UNIQUE INDEX uq_task_waiting_active ON delivery.task_waiting(task_id)
    WHERE ended_at IS NULL;

CREATE TABLE delivery.task_policy_override (
    task_id UUID PRIMARY KEY,
    project_id UUID NOT NULL,
    auto_accept BOOLEAN,
    auto_merge BOOLEAN,
    auto_publish_knowledge BOOLEAN,
    require_human_approval BOOLEAN,
    additional_checks JSONB NOT NULL DEFAULT '[]',
    updated_by UUID NOT NULL REFERENCES core.principal(id),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (project_id, task_id) REFERENCES delivery.task(project_id, id)
);

CREATE TABLE integration.outbox_event (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES core.project(id),
    event_type TEXT NOT NULL,
    aggregate_type TEXT NOT NULL,
    aggregate_id TEXT NOT NULL,
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'processing', 'delivered', 'dead_letter')),
    attempts INT NOT NULL DEFAULT 0,
    max_attempts INT NOT NULL DEFAULT 5,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    locked_until TIMESTAMPTZ,
    lease_owner TEXT,
    last_error TEXT,
    idempotency_key TEXT NOT NULL UNIQUE,
    schema_version INT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    delivered_at TIMESTAMPTZ
);
CREATE INDEX idx_outbox_claim ON integration.outbox_event(status, next_attempt_at)
    WHERE status IN ('pending', 'processing');

ALTER TABLE delivery.task ALTER COLUMN status SET DEFAULT 'draft';

ALTER TABLE delivery.task
    ADD CONSTRAINT task_status_valid CHECK (status IN (
        'draft', 'ready', 'claimed', 'running', 'awaiting_decision', 'blocked',
        'submitted', 'reviewing', 'completed', 'changes_requested', 'failed',
        'cancelled', 'superseded')),
    ADD CONSTRAINT task_failure_category_valid CHECK (
        last_failure_category IS NULL OR last_failure_category IN (
            'infrastructure', 'execution', 'validation', 'permission',
            'policy_violation', 'missing_dependency', 'unknown'));
```

- [x] **Step 4: 同 commit 改完旧状态字面量**

CHECK 一旦生效，任何还在写 `'backlog'` 的路径都会失败。**只改字面量与文档注释，不改行为**：

| 文件 | 位置 | 改动 |
| --- | --- | --- |
| `crates/pm-application/src/pm_write.rs` | L16-17 | `TASK_STATUSES` → 13 值数组 |
| 同上 | L263 注释、L293 | `status 固定 backlog` → `draft`；SQL 里 `'backlog'` → `'draft'` |
| `crates/pm-application/src/command_executor.rs` | L455 注释、L476 | `CreateTask` 落 `draft` |
| 同上 | L488 | 补偿快照 `"status": "backlog"` → `"draft"` |
| `scripts/seed-demo.sql` | L65, L67 | `in_review` → `reviewing`；`backlog` → `draft` |
| `crates/pm-application/tests/command_executor.rs` | L169, L181, L257, L317, L612 | `backlog` → `draft` |
| 同上 | L359 | 保持 `running`（已是新值） |
| `crates/pm-application/tests/bootstrap_read.rs` | L249 | `backlog` → `draft` |
| `crates/pm-application/tests/e2e_scenarios.rs` | L151 seed、L1070、L1103、L1234 | seed `in_progress` → `running`；对应断言同步 |
| `crates/pm-application/tests/supervision.rs` | L135 seed、L297 | seed `in_progress` → `running`；断言同步 |
| `crates/pm-ai/tests/orchestrator.rs` | L264 | `backlog` → `draft` |
| `crates/pm-mcp/tests/tools.rs` | L268 | `backlog` → `draft` |
| `apps/pm-api/tests/pm_writes.rs` | L269, L313 | `backlog` → `draft` |
| 同上 | L373, L383 | PATCH 值 `in_progress` → `running` |
| 同上 | L395, L406, L407, L419 | PATCH 值 `done` → `completed` |
| `apps/pm-api/tests/pm_lists.rs` | L28 | seed `'backlog'` → `'draft'` |

- [x] **Step 5: 跑全量测试确认通过**

Run: `bash scripts/cargo-test.sh`
Expected: PASS —— nextest 全量绿（含 `migration_0009` 的 8 个用例与改写后的既有套件），doctest 绿。

- [x] **Step 6: drop 重建开发库并重放种子**

新系统无业务数据，dev 库**直接 drop 重建**，不做任何数据搬运：

```bash
# 1) 停掉连库进程（pm-api / ai-pm-worker / pm-worker）
# 2) 重建库（Postgres 16 的 WITH (FORCE) 会断开残留连接）
docker exec aipm-pg-dev psql -U aipm -d postgres \
  -c "DROP DATABASE IF EXISTS aipm WITH (FORCE)" -c "CREATE DATABASE aipm"
# 3) 启动一次 pm-api 跑迁移 0001–0010，再灌演示数据
cargo run -p pm-api   # 迁移完成后 Ctrl+C
docker exec -i aipm-pg-dev psql -U aipm -d aipm < scripts/seed-demo.sql
```

Expected: `seed-demo.sql` 无约束错误；`SELECT status, count(*) FROM delivery.task GROUP BY 1` 只出现 13 值之一。

- [x] **Step 7: Commit**

```bash
git add migrations/0010_task_state_machine.sql crates/pm-infrastructure/tests/migration_0009.rs \
        crates/pm-application apps scripts/seed-demo.sql crates/pm-ai/tests/orchestrator.rs \
        crates/pm-mcp/tests/tools.rs
git commit -m "feat(db): task run, waiting, supersession, policy override and outbox schema with the 13-state constraint"
```

**Task 1 完成证据（2026-10-31）**：`migration_0009` 8/8 绿；全量 `258 tests run: 258 passed, 0 skipped`（nextest + doctest）；dev 库已 `pg_dump` 备份 → drop/重建 → `_sqlx_migrations` 10 条 → 重放 `seed-demo.sql`（任务状态只出现 `blocked/completed/draft/reviewing/running`）。因导入特性先落地了 `0009_repo_import.sql`，本迁移顺延为 `0010`。

---

### Task 2 — Outbox：事务内写入 + 租约领取 + 退避重试

**Files:**
- Create: `crates/pm-infrastructure/src/outbox.rs`
- Create: `crates/pm-infrastructure/tests/outbox.rs`
- Modify: `crates/pm-infrastructure/src/lib.rs`、`crates/pm-infrastructure/Cargo.toml`

**Interfaces:**
- Consumes: Task 1 的 `integration.outbox_event`。
- Produces（Task 3/4 直接调用，名字与签名不变）：
  - `pub struct NewOutboxEvent<'a> { pub project_id: uuid::Uuid, pub event_type: &'a str, pub aggregate_type: &'a str, pub aggregate_id: &'a str, pub payload: serde_json::Value, pub idempotency_key: &'a str }`
  - `pub struct ClaimedOutboxEvent { pub id: uuid::Uuid, pub project_id: uuid::Uuid, pub event_type: String, pub aggregate_type: String, pub aggregate_id: String, pub payload: serde_json::Value, pub attempts: i32, pub schema_version: i32, pub idempotency_key: String }`
  - `#[derive(Debug, PartialEq, Eq)] pub enum OutboxFailOutcome { Retried, DeadLettered }`
  - `pub async fn append(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, event: NewOutboxEvent<'_>) -> Result<uuid::Uuid, sqlx::Error>`
  - `pub async fn claim(pool: &sqlx::PgPool, worker_id: &str, lease_secs: i64) -> Option<ClaimedOutboxEvent>`
  - `pub async fn complete(pool: &sqlx::PgPool, id: uuid::Uuid) -> Result<(), sqlx::Error>`
  - `pub async fn fail(pool: &sqlx::PgPool, id: uuid::Uuid, error: &str, max_attempts: i32, backoff_base_secs: i64) -> Result<OutboxFailOutcome, sqlx::Error>`
  - 常量 `pub const OUTBOX_MAX_ATTEMPTS: i32 = 5; pub const OUTBOX_BACKOFF_BASE_SECS: i64 = 5; pub const OUTBOX_LEASE_SECS: i64 = 60;`

- [x] **Step 1: 写失败测试**

```rust
// crates/pm-infrastructure/tests/outbox.rs
use pm_infrastructure::outbox::{self, NewOutboxEvent, OutboxFailOutcome};
use sqlx::PgPool;
use uuid::Uuid;

async fn seed_project(pool: &PgPool) -> Uuid {
    let customer_id: Uuid =
        sqlx::query_scalar("INSERT INTO core.customer (name) VALUES ('ob') RETURNING id")
            .fetch_one(pool).await.unwrap();
    sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name) VALUES ($1, 'OB-1', 'ob') RETURNING id",
    ).bind(customer_id).fetch_one(pool).await.unwrap()
}

fn event<'a>(project_id: Uuid, key: &'a str) -> NewOutboxEvent<'a> {
    NewOutboxEvent {
        project_id,
        event_type: "TaskReady",
        aggregate_type: "Task",
        aggregate_id: "task-1",
        payload: serde_json::json!({ "task_id": "task-1" }),
        idempotency_key: key,
    }
}

/// 事务内写入：提交才可见，回滚即随事务消失（§20.6 第 1 条）。
#[sqlx::test(migrations = "../../migrations")]
async fn append_is_visible_only_after_commit(pool: PgPool) {
    let project_id = seed_project(&pool).await;

    let mut tx = pool.begin().await.unwrap();
    let rolled_back = outbox::append(&mut tx, event(project_id, "k-rollback")).await.unwrap();
    tx.rollback().await.unwrap();
    let exists: i64 =
        sqlx::query_scalar("SELECT count(*) FROM integration.outbox_event WHERE id = $1")
            .bind(rolled_back).fetch_one(&pool).await.unwrap();
    assert_eq!(exists, 0, "an event written in a rolled-back tx must not exist");

    let mut tx = pool.begin().await.unwrap();
    outbox::append(&mut tx, event(project_id, "k-commit")).await.unwrap();
    tx.commit().await.unwrap();
    let committed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.outbox_event WHERE idempotency_key = 'k-commit'",
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(committed, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn claim_leases_the_event_and_bumps_attempts(pool: PgPool) {
    let project_id = seed_project(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    outbox::append(&mut tx, event(project_id, "k-claim")).await.unwrap();
    tx.commit().await.unwrap();

    let claimed = outbox::claim(&pool, "worker-a", 60).await.expect("one pending event");
    assert_eq!(claimed.event_type, "TaskReady");
    assert_eq!(claimed.attempts, 1, "claiming counts the attempt");
    assert!(
        outbox::claim(&pool, "worker-b", 60).await.is_none(),
        "a live lease must not be handed to a second worker"
    );
}

/// 租约过期 = Worker 崩溃恢复路径：同一行被重投，id 不变（至少一次）。
#[sqlx::test(migrations = "../../migrations")]
async fn expired_lease_is_reclaimed_as_the_same_row(pool: PgPool) {
    let project_id = seed_project(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let id = outbox::append(&mut tx, event(project_id, "k-crash")).await.unwrap();
    tx.commit().await.unwrap();

    let first = outbox::claim(&pool, "worker-a", 0).await.expect("claim with zero lease");
    assert_eq!(first.id, id);
    let second = outbox::claim(&pool, "worker-b", 60).await.expect("expired lease is reclaimable");
    assert_eq!(second.id, first.id, "redelivery must reuse the event row");
    assert_eq!(second.attempts, 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn complete_marks_delivered(pool: PgPool) {
    let project_id = seed_project(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let id = outbox::append(&mut tx, event(project_id, "k-done")).await.unwrap();
    tx.commit().await.unwrap();
    outbox::claim(&pool, "worker-a", 60).await.unwrap();
    outbox::complete(&pool, id).await.unwrap();
    let row: (String, bool) = sqlx::query_as(
        "SELECT status, delivered_at IS NOT NULL FROM integration.outbox_event WHERE id = $1",
    ).bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(row, ("delivered".to_string(), true));
}

#[sqlx::test(migrations = "../../migrations")]
async fn fail_retries_with_backoff_until_the_ceiling(pool: PgPool) {
    let project_id = seed_project(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let id = outbox::append(&mut tx, event(project_id, "k-retry")).await.unwrap();
    tx.commit().await.unwrap();

    outbox::claim(&pool, "worker-a", 60).await.unwrap();
    assert_eq!(
        outbox::fail(&pool, id, "boom", 2, 5).await.unwrap(),
        OutboxFailOutcome::Retried
    );
    let row: (String, i32, String) = sqlx::query_as(
        "SELECT status, attempts, last_error FROM integration.outbox_event WHERE id = $1",
    ).bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(row, ("pending".to_string(), 1, "boom".to_string()));

    // 退避后到期 → 再领取一次（attempts 由 claim 自增到 2）
    sqlx::query("UPDATE integration.outbox_event SET next_attempt_at = now() WHERE id = $1")
        .bind(id).execute(&pool).await.unwrap();
    let second = outbox::claim(&pool, "worker-a", 60).await.expect("backoff elapsed");
    assert_eq!(second.attempts, 2);
    assert_eq!(
        outbox::fail(&pool, id, "boom again", 2, 5).await.unwrap(),
        OutboxFailOutcome::DeadLettered,
        "attempts >= max_attempts must dead-letter"
    );
}
```

- [x] **Step 2: 跑测试确认失败**

Run: `bash scripts/cargo-test.sh -p pm-infrastructure`
Expected: FAIL —— `unresolved import pm_infrastructure::outbox`。

- [x] **Step 3: 实现 `outbox.rs` + 导出 + 依赖**

`append` 用 `sqlx::query_scalar("INSERT ... RETURNING id")` 在传入的 `tx` 上执行（签名强制事务内）。`claim` 采用与 `job_queue::claim` 同构的单条 CTE：候选 `(status = 'pending' AND next_attempt_at <= now()) OR (status = 'processing' AND locked_until < now())`，`FOR UPDATE SKIP LOCKED ORDER BY next_attempt_at LIMIT 1`，随后 `UPDATE SET status='processing', attempts = attempts + 1, locked_until = now() + make_interval(secs => $2::float8), lease_owner = $3`。`complete` 置 `delivered` + `delivered_at = now()` + 清租约。`fail` 的 `CASE`：`attempts >= max_attempts` → `dead_letter`（并写 `last_error`）；否则 `pending` + `next_attempt_at = now() + backoff_base_secs * power(2.0, attempts)`。全部沿用 `job_queue` 的 `map(|_| ())` / `ok()?` 风格，不引入新错误类型。

`lib.rs` 增 `pub mod outbox;`；`Cargo.toml` 增 `serde_json`/`chrono`（dev-dep 已有同名项则可提升到 `[dependencies]`，避免重复声明）。

- [x] **Step 4: 跑测试确认通过**

Run: `bash scripts/cargo-test.sh -p pm-infrastructure`
Expected: PASS（5 个 outbox 用例全绿）。

- [x] **Step 5: Commit**

```bash
git add crates/pm-infrastructure/src/outbox.rs crates/pm-infrastructure/src/lib.rs \
        crates/pm-infrastructure/Cargo.toml crates/pm-infrastructure/tests/outbox.rs
git commit -m "feat(outbox): transactional append with lease-based claim and retry"
```

**Task 2 完成证据（2026-10-31）**：红→绿留痕——首跑 `E0432: unresolved import pm_infrastructure::outbox`；实现后 `bash scripts/cargo-test.sh -p pm-infrastructure` **24/24 绿**（新增 outbox 5 用例：回滚事务内 append 不可见 / 提交后可见、claim 租约领取且 attempts=1、租约过期同 id 重投 attempts=2、complete 置 `delivered`+`delivered_at`、fail 按 `attempts >= max_attempts` 退避重试→触顶死信），既有 job_queue / migration_0009 / schema 套件无回归。commit `1263c91`（`Cargo.lock` 随 serde_json/chrono 提升为正式依赖一并入库）。

---

### Task 3 — 死信查询与人工重放

**Files:**
- Modify: `crates/pm-infrastructure/src/outbox.rs`
- Modify: `crates/pm-infrastructure/tests/outbox.rs`

**Interfaces:**
- Consumes: Task 2 的 `fail` / `claim`。
- Produces：
  - `pub async fn dead_letter(pool: &sqlx::PgPool, id: uuid::Uuid, reason: &str) -> Result<(), sqlx::Error>`
  - `pub async fn replay(pool: &sqlx::PgPool, id: uuid::Uuid) -> Result<(), ReplayError>`
  - `pub async fn list_dead_letters(pool: &sqlx::PgPool, limit: i64) -> Result<Vec<DeadLetterRow>, sqlx::Error>`
  - `pub struct DeadLetterRow { pub id: uuid::Uuid, pub event_type: String, pub aggregate_type: String, pub aggregate_id: String, pub attempts: i32, pub last_error: Option<String>, pub created_at: chrono::DateTime<chrono::Utc> }`
  - `pub enum ReplayError { NotFound, NotDeadLetter }`（`thiserror::Error`，`#[error("outbox event not found")]` / `#[error("only dead-letter events can be replayed")]`）
  - 重放语义：`status = 'pending'`、`attempts = 0`、`next_attempt_at = now()`、清 `locked_until`/`lease_owner`；`last_error` 保留供追溯。

- [x] **Step 1: 写失败测试**

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn dead_letter_is_listed_with_its_reason_and_can_be_replayed(pool: PgPool) {
    let project_id = seed_project(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let id = outbox::append(&mut tx, event(project_id, "k-dead")).await.unwrap();
    tx.commit().await.unwrap();
    outbox::dead_letter(&pool, id, "no handler registered for TaskReady").await.unwrap();

    let rows = outbox::list_dead_letters(&pool, 10).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, id);
    assert_eq!(
        rows[0].last_error.as_deref(),
        Some("no handler registered for TaskReady")
    );

    outbox::replay(&pool, id).await.expect("dead-letter replay is allowed");
    let reclaimed = outbox::claim(&pool, "worker-a", 60).await.expect("replayed event is claimable");
    assert_eq!(reclaimed.id, id);
    assert_eq!(reclaimed.attempts, 1, "replay resets attempts");
}

#[sqlx::test(migrations = "../../migrations")]
async fn replay_rejects_missing_and_live_events(pool: PgPool) {
    let project_id = seed_project(&pool).await;
    assert!(matches!(
        outbox::replay(&pool, Uuid::new_v4()).await,
        Err(outbox::ReplayError::NotFound)
    ));
    let mut tx = pool.begin().await.unwrap();
    let id = outbox::append(&mut tx, event(project_id, "k-live")).await.unwrap();
    tx.commit().await.unwrap();
    assert!(matches!(
        outbox::replay(&pool, id).await,
        Err(outbox::ReplayError::NotDeadLetter)
    ));
}
```

- [x] **Step 2: 跑测试确认失败**

Run: `bash scripts/cargo-test.sh -p pm-infrastructure`
Expected: FAIL —— `outbox::dead_letter` / `list_dead_letters` / `replay` 未定义。

- [x] **Step 3: 实现三个函数**

`dead_letter`：`UPDATE ... SET status = 'dead_letter', last_error = $2, locked_until = NULL, lease_owner = NULL WHERE id = $1`（幂等，重复调用不报错）。`replay`：先查 `status`，缺失 → `NotFound`，非 `dead_letter` → `NotDeadLetter`，否则一条 `UPDATE` 完成重置。`list_dead_letters`：`SELECT ... WHERE status = 'dead_letter' ORDER BY created_at DESC LIMIT $1`。

- [x] **Step 4: 跑测试确认通过**

Run: `bash scripts/cargo-test.sh -p pm-infrastructure`
Expected: PASS（新增 2 个用例 + Task 2 五个用例全绿）。

- [x] **Step 5: Commit**

```bash
git add crates/pm-infrastructure/src/outbox.rs crates/pm-infrastructure/tests/outbox.rs
git commit -m "feat(outbox): dead letter listing and manual replay"
```

**Task 3 完成证据（2026-10-31）**：红→绿留痕——首跑 `E0425`/`E0433`（`replay`/`dead_letter` 未定义）；实现后 `-p pm-infrastructure` **26/26 绿**（新增 2 用例：死信列表带 reason 且 replay 后同 id 领取 attempts=1、replay 拒缺失行 `NotFound` 与活事件 `NotDeadLetter`）。commit `ba76485`（`thiserror` 随 Cargo.lock 入库）。

---

### Task 4 — Worker：注册表分发与进程接线

**Files:**
- Create: `crates/pm-infrastructure/src/outbox_worker.rs`
- Create: `crates/pm-infrastructure/tests/outbox_worker.rs`
- Modify: `crates/pm-infrastructure/src/lib.rs`、`crates/pm-infrastructure/Cargo.toml`（`async-trait` / `tracing`）
- Modify: `apps/pm-worker/{Cargo.toml,src/main.rs}`

**Interfaces:**
- Consumes: Task 2/3 的 `outbox::{append, claim, complete, fail, dead_letter}` 与 `ClaimedOutboxEvent`。
- Produces：
  - `#[async_trait::async_trait] pub trait OutboxHandler: Send + Sync { async fn handle(&self, event: &ClaimedOutboxEvent) -> Result<(), String>; }`
  - `#[derive(Debug, PartialEq, Eq)] pub enum DispatchOutcome { Delivered, Retried, DeadLettered, Idle }`
  - `pub struct OutboxWorker { /* pool, worker_id, handlers, lease_secs, max_attempts, backoff_base_secs */ }`
  - `impl OutboxWorker { pub fn new(pool: sqlx::PgPool, worker_id: String) -> Self; pub fn with_handler(self, event_type: &'static str, handler: std::sync::Arc<dyn OutboxHandler>) -> Self; pub async fn dispatch_once(&self) -> Result<DispatchOutcome, sqlx::Error>; pub async fn run(self) -> std::convert::Infallible; }`
  - 常量 `pub const OUTBOX_IDLE_POLL_MS: u64 = 100;`

- [x] **Step 1: 写失败测试**

```rust
// crates/pm-infrastructure/tests/outbox_worker.rs
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use pm_infrastructure::outbox::{self, NewOutboxEvent};
use pm_infrastructure::outbox_worker::{DispatchOutcome, OutboxHandler, OutboxWorker};
use sqlx::PgPool;
use uuid::Uuid;

struct CountingHandler { calls: AtomicUsize, fail: bool }

#[async_trait::async_trait]
impl OutboxHandler for CountingHandler {
    async fn handle(&self, _event: &outbox::ClaimedOutboxEvent) -> Result<(), String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail { Err("handler exploded".to_string()) } else { Ok(()) }
    }
}

async fn seed_event(pool: &PgPool, key: &str) -> Uuid {
    let customer_id: Uuid =
        sqlx::query_scalar("INSERT INTO core.customer (name) VALUES ('ow') RETURNING id")
            .fetch_one(pool).await.unwrap();
    let project_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name) VALUES ($1, $2, 'ow') RETURNING id",
    ).bind(customer_id).bind(format!("OW-{key}")).fetch_one(pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    let id = outbox::append(&mut tx, NewOutboxEvent {
        project_id,
        event_type: "TaskReady",
        aggregate_type: "Task",
        aggregate_id: "task-1",
        payload: serde_json::json!({}),
        idempotency_key: key,
    }).await.unwrap();
    tx.commit().await.unwrap();
    id
}

#[sqlx::test(migrations = "../../migrations")]
async fn dispatch_delivers_when_the_handler_succeeds(pool: PgPool) {
    seed_event(&pool, "k-ok").await;
    let handler = Arc::new(CountingHandler { calls: AtomicUsize::new(0), fail: false });
    let worker = OutboxWorker::new(pool.clone(), "w".to_string())
        .with_handler("TaskReady", handler.clone());
    assert_eq!(worker.dispatch_once().await.unwrap(), DispatchOutcome::Delivered);
    assert_eq!(handler.calls.load(Ordering::SeqCst), 1);
    assert!(worker.dispatch_once().await.unwrap() == DispatchOutcome::Idle);
}

#[sqlx::test(migrations = "../../migrations")]
async fn handler_failure_hits_the_retry_ceiling_then_dead_letters(pool: PgPool) {
    let id = seed_event(&pool, "k-fail").await;
    let handler = Arc::new(CountingHandler { calls: AtomicUsize::new(0), fail: true });
    let worker = OutboxWorker::new(pool.clone(), "w".to_string())
        .with_handler("TaskReady", handler);
    // 首次失败 → 重试排队
    assert_eq!(worker.dispatch_once().await.unwrap(), DispatchOutcome::Retried);
    // 让退避立即可到期，第二次失败即触顶 → 死信
    sqlx::query("UPDATE integration.outbox_event SET next_attempt_at = now() WHERE id = $1")
        .bind(id).execute(&pool).await.unwrap();
    assert_eq!(worker.dispatch_once().await.unwrap(), DispatchOutcome::DeadLettered);
    let status: String = sqlx::query_scalar("SELECT status FROM integration.outbox_event WHERE id = $1")
        .bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(status, "dead_letter");
}

#[sqlx::test(migrations = "../../migrations")]
async fn unregistered_event_type_goes_to_dead_letter(pool: PgPool) {
    let id = seed_event(&pool, "k-unreg").await;
    let worker = OutboxWorker::new(pool.clone(), "w".to_string());
    assert_eq!(worker.dispatch_once().await.unwrap(), DispatchOutcome::DeadLettered);
    let reason: Option<String> = sqlx::query_scalar(
        "SELECT last_error FROM integration.outbox_event WHERE id = $1",
    ).bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(
        reason.as_deref(),
        Some("no handler registered for TaskReady"),
        "an unhandled event must be visible for replay, never silently dropped"
    );
}
```

- [x] **Step 2: 跑测试确认失败**

Run: `bash scripts/cargo-test.sh -p pm-infrastructure`
Expected: FAIL —— module `outbox_worker` 未定义。

- [x] **Step 3: 实现注册表与 `dispatch_once`，并接线 `apps/pm-worker`**

`dispatch_once`：`claim` 为空 → `Idle`；命中注册表 → `handler.handle(&event)`，`Ok` → `outbox::complete` → `Delivered`，`Err(msg)` → `outbox::fail(..., OUTBOX_MAX_ATTEMPTS, OUTBOX_BACKOFF_BASE_SECS)`，按返回映射 `Retried`/`DeadLettered`；未命中注册表 → `outbox::dead_letter(pool, id, &format!("no handler registered for {event_type}"))` → `DeadLettered`。`run`：`loop { match self.dispatch_once().await { Ok(Idle) => sleep(IDLE_POLL_MS), Ok(_) => continue, Err(e) => { tracing::warn!(...); sleep(IDLE_POLL_MS) } } }`。

`apps/pm-worker/src/main.rs`：读 `DATABASE_URL`（缺失即 `expect`，与 `ai-pm-worker` 同风格）、`PgPoolOptions::max_connections(5)`、构造 `OutboxWorker`、**首版不注册任何 handler**（注释指向 Slice C/D 的 MCP 调度与知识回写），`tracing::info!` 后进入 `run().await`。`apps/pm-worker/Cargo.toml` 增 `sqlx`/`uuid`/`pm-infrastructure`。

- [x] **Step 4: 跑测试确认通过**

Run: `bash scripts/cargo-test.sh`
Expected: PASS —— 全量绿；`cargo build` 亦通过（`pm-worker` 由 workspace 编译）。

- [x] **Step 5: Commit**

```bash
git add crates/pm-infrastructure/src/outbox_worker.rs crates/pm-infrastructure/src/lib.rs \
        crates/pm-infrastructure/Cargo.toml crates/pm-infrastructure/tests/outbox_worker.rs \
        apps/pm-worker
git commit -m "feat(worker): dispatch outbox events with a handler registry"
```

**Task 4 完成证据（2026-10-31）**：**偏差留痕**——plan 字面指引（`fail` 传 `OUTBOX_MAX_ATTEMPTS=5`、`new()` 无预算入口）与测试「第二次失败即触顶」矛盾，首跑按字面实现留红证据：`handler_failure_hits_the_retry_ceiling_then_dead_letters` 断言 `left: Retried, right: DeadLettered`（其余 28/29 绿）。最小修正：`OutboxWorker` 增 `with_retry_limits(max_attempts, backoff_base_secs)` builder，生产默认仍为常量 5/5，测试用例显式 `.with_retry_limits(2, 5)` 钉死触顶路径。修正后 `-p pm-infrastructure` **29/29 绿**；全量 `bash scripts/cargo-test.sh` **268 passed**（较 Task 1 基线 258 增 10）+ 6 组 doctest ok + `cargo build` 10.9s 通过。commit `84805b0`。另两处必要补充：`outbox_worker` 的 `run()` 需 `tokio`（文件结构表未列，实现指引 sleep 必需）；库内引用修正为 `crate::outbox`（首跑 `E0433` 留痕）。

---

## 风险登记

| # | 风险 | 处置 |
| --- | --- | --- |
| R1 | 迁移**不可回滚**（Postgres DDL 虽可事务回滚，但已发布的迁移文件不能改） | `0010` 一经提交禁改，后续变更新建 `0011+`（改已应用文件会触发 sqlx checksum 校验失败，直接拒绝启动） |
| R2 | CHECK 收紧与写路径不同步 → 创建任务 500 | Task 1 Step 4 把 `pm_write` / `command_executor` / `seed-demo.sql` / 全部测试字面量放同一 commit；Step 5 跑全量而非单包 |
| R3 | 开发库里残留 `seed-demo.sql` 灌入的旧状态行，`0010` 之后被拒写 | Task 1 Step 6 已 drop 重建开发库并重放**已改值**的种子；新系统无业务数据，重建不丢东西 |
| R4 | 全量测试需要改写十余处断言，机械替换可能改变语义 | 逐处对照「种子值 / 执行后值 / 补偿快照值」三类；补偿回滚断言必须等于**种子**值（如 seed 改成 `running` 后，回滚后仍是 `running`） |
| R5 | PM 进程仍在运行 `pm-api.exe` 时重建会锁文件 | 用 `CARGO_TARGET_DIR` 或先停服务；`scripts/cargo-test.sh` 使用独立临时目录，不额外占用工作区 |
| R6 | Outbox 首版无真实 handler，事件一律死信 | **D4 的既定口径**：不静默 delivered；Slice C/D 注册真实 handler 后，对历史死信可用 `replay` 重放 |

## Self-Review 记录

- **规格覆盖**：§九 M4（lowercase snake_case 存储）→ Task 1；§11.5（双向关系 + 多后继 + 自取代/重复边约束）→ Task 1；§12.4（等待上下文分离 + 历史保留 + 单活跃）→ Task 1；§19.4/19.5（计数位 + 不可变失败记录 + Run 快照字段）→ Task 1；§20.2–20.6（同事务写入 / 最小 Worker / 至少一次 / 死信与重放 / 幂等）→ Task 2/3/4；§21.1（lowercase snake_case）→ 全部 DDL 与枚举字符串。**§九 M1–M3（legacy 回填）不适用**：新系统无存量数据。
- **明确不在本计划**：`completion_evaluator`、风险评估器、策略合成引擎、Reviewer 编排、任务类型/项目策略表、`PATCH` 禁改 status、前端徽标（均为 Slice C/D 或详情页稿待裁项）。Task 1 只落 `task_policy_override` **存储**，不落解析器。
- **类型一致性**：`ClaimedOutboxEvent` 字段在 Task 2 定义、Task 3/4 复用；`OutboxFailOutcome` 只在 Task 2 内部与 Task 4 映射使用；`ReplayError` 仅 Task 3 暴露。`NewOutboxEvent` 全字段与 `integration.outbox_event` 列一一对应（`schema_version` 走默认值 1，不在结构体里重复）。
- **Review Focus 五项**均有钉死测试：dev 库残留旧值（Task 1 Step 6 drop 重建 + 种子重放）、CHECK 漏值（Task 1 13 值循环 + 5 个旧值拒绝 + 失败类别 7 值）、写路径不同步（Task 1 Step 4 清单 + 全量跑）、事务外丢失（Task 2 `append_is_visible_only_after_commit`）、重复投递（Task 2 `expired_lease_is_reclaimed_as_the_same_row` + 唯一键断言）。
- **比例检查**：计划 1 个迁移文件 + 2 个测试文件（迁移/Outbox）+ 2 个实现文件 + 1 处进程接线；无「TBD」「适当处理」类空步，无超出规格的抽象。
- **依赖顺序**：Task 2 依赖 Task 1 的 `outbox_event`；Task 3 依赖 Task 2 的 `fail`；Task 4 依赖 Task 2/3。任务内 TDD 红→绿，逐任务 commit。
