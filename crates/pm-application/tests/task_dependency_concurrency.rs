//! T-025 Task Dependency 并发正确性（P0 修复验证）：真实 PostgreSQL、双独立
//! 连接、定向屏障。覆盖任务书 `docs/ai-pm-t025-task-dependency-hardening-v1.md`
//! §九 的 C59 / C60 / C64 / C65 与项目级图锁机制本身。
//!
//! 方法论（任务书 §七 硬要求）：并发场景要么用两个独立连接 + 显式步骤屏障构造
//! 确定性交错（`graph_lock_*`），要么并发调用黑盒入口后断言「任何交错下的同一
//! 最终态」（C59–C65，多轮 + 双向启动顺序）——不做随机碰撞式断言。
//!
//! 修复语义：建边 / 删边 / 终态迁移共用项目级 `pg_advisory_xact_lock`（键由
//! `project_id` 派生），且取锁点先于事务内任何行锁——并发图写与终态解锁传播
//! 对同一项目全序化，后到者必然看到先到者已提交的边或状态。

use std::sync::Arc;

use pm_application::actor::AuditActor;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::command_executor::{
    ActorKind, CommandSource, ExecutionContext, PgCommandExecutor,
};
use pm_application::pm_write::{update_task, UpdateTaskRequest};
use pm_application::task_dependency::{add_dependency, remove_dependency, DependencyError};
use pm_application::PolicyEngine;
use pm_domain::{ProjectCommand, TaskStatus, SYSTEM_TENANT};
use sqlx::{Connection, PgPool, Postgres};
use uuid::Uuid;

struct Fx {
    project_id: Uuid,
    actor_id: Uuid,
}

async fn seed_project(pool: &PgPool, code: &str) -> Fx {
    let o = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    Fx {
        project_id: o.project_id,
        actor_id: o.human_id,
    }
}

async fn seed_task(pool: &PgPool, project_id: Uuid, status: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, tenant_id) \
         VALUES ($1, 't', $2, 1, (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(status.to_string())
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn task_status(pool: &PgPool, task_id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM delivery.task WHERE id = $1")
        .bind(task_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn task_ready_events(pool: &PgPool, project_id: Uuid, task_id: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM integration.outbox_event \
          WHERE project_id = $1 AND event_type = 'TaskReady' AND aggregate_id = $2",
    )
    .bind(project_id)
    .bind(task_id.to_string())
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn edge_count(pool: &PgPool, project_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM delivery.task_dependency WHERE project_id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// `add_dependency` 的检环 SQL（与实现同款），供屏障测试直接断言边可见性。
async fn cycle_exists<'e, E>(e: E, project_id: Uuid, task_id: Uuid, upstream: Uuid) -> bool
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar(
        "WITH RECURSIVE ancestors AS ( \
             SELECT $2::uuid AS id \
             UNION \
             SELECT d.depends_on_task_id \
               FROM delivery.task_dependency d \
               JOIN ancestors a ON d.task_id = a.id \
              WHERE d.project_id = $1) \
         SELECT EXISTS (SELECT 1 FROM ancestors WHERE id = $3)",
    )
    .bind(project_id)
    .bind(upstream)
    .bind(task_id)
    .fetch_one(e)
    .await
    .unwrap()
}

/// 命令通道把任务迁到 `completed`（终态迁移入口，即修复后取图锁的路径）。
async fn complete_task(pool: PgPool, project_id: Uuid, actor_id: Uuid, task_id: Uuid) {
    PgCommandExecutor::new(pool, Arc::new(PolicyEngine::for_tests()))
        .execute(
            ProjectCommand::UpdateTaskStatus {
                task_id,
                expected_revision: 1,
                new_status: TaskStatus::Completed,
            },
            ExecutionContext {
                project_id,
                actor: pm_application::actor::AuditActor::principal(actor_id),
                actor_kind: ActorKind::Human,
                source: CommandSource::Bootstrap,
            },
        )
        .await
        .expect("transition to completed must succeed");
}

// ------------------------------------------------------------- 图锁机制本身

/// 屏障式定向验证：图锁互斥 + MVCC 隔离 + 提交后可见。
///
/// 正是这三个事实的组合关闭了评审 §2.1/§2.2 的两个竞态窗口：未提交的边不再
/// 参与判定（互斥推迟了后到者），已提交的边必然被后到者看到（全序化）。
#[sqlx::test(migrations = "../../migrations")]
async fn graph_lock_serializes_writers_and_exposes_committed_edges(pool: PgPool) {
    let fx = seed_project(&pool, "TDC-LK").await;
    let a = seed_task(&pool, fx.project_id, "draft").await;
    let b = seed_task(&pool, fx.project_id, "draft").await;

    // 连接 1：取图锁 → 插边（不提交）。
    let mut conn1 = pool.acquire().await.unwrap();
    let mut tx1 = conn1.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('task-graph:' || $1::text, 0))")
        .bind(fx.project_id.to_string())
        .execute(&mut *tx1)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_dependency \
         (tenant_id, project_id, task_id, depends_on_task_id, created_by) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3, $4)",
    )
    .bind(fx.project_id)
    .bind(a)
    .bind(b)
    .bind(fx.actor_id)
    .execute(&mut *tx1)
    .await
    .unwrap();

    // 屏障点（连接 2，独立连接）：图锁被占（互斥）且未提交的边不可见。
    let mut conn2 = pool.acquire().await.unwrap();
    {
        let mut tx2 = conn2.begin().await.unwrap();
        let held: bool = sqlx::query_scalar(
            "SELECT pg_try_advisory_xact_lock(hashtextextended('task-graph:' || $1::text, 0))",
        )
        .bind(fx.project_id.to_string())
        .fetch_one(&mut *tx2)
        .await
        .unwrap();
        assert!(!held, "graph lock must be held by the uncommitted writer");
        assert!(
            // 检环方向与 C59 同款：从 upstream=a 沿边回到 task=b（边 A 依赖 B）。
            !cycle_exists(&mut *tx2, fx.project_id, b, a).await,
            "uncommitted edge must be invisible to a concurrent graph writer"
        );
        tx2.rollback().await.unwrap();
    }

    // 放行（连接 1 提交）：后到的图写者必须能拿到锁，且看到已提交的边。
    tx1.commit().await.unwrap();
    {
        let mut tx3 = conn2.begin().await.unwrap();
        let held: bool = sqlx::query_scalar(
            "SELECT pg_try_advisory_xact_lock(hashtextextended('task-graph:' || $1::text, 0))",
        )
        .bind(fx.project_id.to_string())
        .fetch_one(&mut *tx3)
        .await
        .unwrap();
        assert!(
            held,
            "graph lock must be free after the first writer commits"
        );
        assert!(
            cycle_exists(&mut *tx3, fx.project_id, b, a).await,
            "committed edge must be visible to the next graph writer"
        );
        tx3.rollback().await.unwrap();
    }
}

/// 跨项目互不阻塞：不同项目的图锁彼此独立（任务书 §一 要求）。
#[sqlx::test(migrations = "../../migrations")]
async fn graph_locks_of_different_projects_do_not_block(pool: PgPool) {
    let fx1 = seed_project(&pool, "TDC-LK-P1").await;
    let fx2 = seed_project(&pool, "TDC-LK-P2").await;

    let mut conn1 = pool.acquire().await.unwrap();
    let mut tx1 = conn1.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('task-graph:' || $1::text, 0))")
        .bind(fx1.project_id.to_string())
        .execute(&mut *tx1)
        .await
        .unwrap();

    let mut conn2 = pool.acquire().await.unwrap();
    let mut tx2 = conn2.begin().await.unwrap();
    let held: bool = sqlx::query_scalar(
        "SELECT pg_try_advisory_xact_lock(hashtextextended('task-graph:' || $1::text, 0))",
    )
    .bind(fx2.project_id.to_string())
    .fetch_one(&mut *tx2)
    .await
    .unwrap();
    assert!(held, "a different project's graph lock must not be blocked");
}

// --------------------------------------------------------------- C59

/// C59：两个事务同时建立反向依赖——至多一个成功，不能形成环。
#[sqlx::test(migrations = "../../migrations")]
async fn c59_concurrent_reverse_edges_cannot_form_a_cycle(pool: PgPool) {
    for round in 0..12u32 {
        let fx = seed_project(&pool, &format!("TDC59-R{round}")).await;
        let a = seed_task(&pool, fx.project_id, "draft").await;
        let b = seed_task(&pool, fx.project_id, "draft").await;

        let p1 = pool.clone();
        let (aid1, a1, b1) = (fx.actor_id, a, b);
        let e1 = async move { add_dependency(&p1, a1, b1, &AuditActor::principal(aid1)).await };
        let p2 = pool.clone();
        let (aid2, a2, b2) = (fx.actor_id, a, b);
        let e2 = async move { add_dependency(&p2, b2, a2, &AuditActor::principal(aid2)).await };
        let (r1, r2) = tokio::join!(e1, e2);

        let ok_count = usize::from(r1.is_ok()) + usize::from(r2.is_ok());
        assert_eq!(
            ok_count, 1,
            "round {round}: exactly one reverse edge must succeed (r1={r1:?} r2={r2:?})"
        );
        for r in [&r1, &r2] {
            if let Err(e) = r {
                assert!(
                    matches!(e, DependencyError::Cycle),
                    "round {round}: the losing edge must be rejected as Cycle, got {e:?}"
                );
            }
        }
        assert_eq!(
            edge_count(&pool, fx.project_id).await,
            1,
            "round {round}: the dependency graph must not contain a cycle"
        );
    }
}

// --------------------------------------------------------------- C60

/// C60：建边与上游完成并发发生——任何交错下，下游最终必须解锁。
#[sqlx::test(migrations = "../../migrations")]
async fn c60_edge_insertion_racing_with_upstream_completion_still_unlocks(pool: PgPool) {
    for round in 0..4u32 {
        for edge_first in [true, false] {
            let fx = seed_project(&pool, &format!("TDC60-R{round}-{edge_first}")).await;
            let upstream = seed_task(&pool, fx.project_id, "reviewing").await;
            let downstream = seed_task(&pool, fx.project_id, "blocked").await;

            let p1 = pool.clone();
            let (down1, up1, aid1) = (downstream, upstream, fx.actor_id);
            let edge =
                async move { add_dependency(&p1, down1, up1, &AuditActor::principal(aid1)).await };
            let p2 = pool.clone();
            let (up2, proj2, aid2) = (upstream, fx.project_id, fx.actor_id);
            let done = async move {
                complete_task(p2, proj2, aid2, up2).await;
            };

            // 启动顺序偏置只影响交错方向；断言只依赖修复保证的最终态。
            let (r_edge, _) = if edge_first {
                tokio::join!(edge, done)
            } else {
                let (a, b) = tokio::join!(done, edge);
                (b, a)
            };
            r_edge.expect("edge insertion must succeed under any interleaving");

            assert_eq!(
                task_status(&pool, downstream).await,
                "ready",
                "round {round} edge_first={edge_first}: the downstream must unlock"
            );
            assert_eq!(
                task_ready_events(&pool, fx.project_id, downstream).await,
                1,
                "round {round} edge_first={edge_first}: exactly one TaskReady"
            );
        }
    }
}

// --------------------------------------------------------------- C64

/// C64：多个上游并发完成——只产生一次有效解锁。
#[sqlx::test(migrations = "../../migrations")]
async fn c64_concurrent_completion_of_multiple_upstreams_unlocks_once(pool: PgPool) {
    for round in 0..5u32 {
        let fx = seed_project(&pool, &format!("TDC64-R{round}")).await;
        let u1 = seed_task(&pool, fx.project_id, "reviewing").await;
        let u2 = seed_task(&pool, fx.project_id, "reviewing").await;
        let downstream = seed_task(&pool, fx.project_id, "blocked").await;
        add_dependency(&pool, downstream, u1, &AuditActor::principal(fx.actor_id))
            .await
            .unwrap();
        add_dependency(&pool, downstream, u2, &AuditActor::principal(fx.actor_id))
            .await
            .unwrap();

        let p1 = pool.clone();
        let (u1c, proj1, aid1) = (u1, fx.project_id, fx.actor_id);
        let c1 = async move { complete_task(p1, proj1, aid1, u1c).await };
        let p2 = pool.clone();
        let (u2c, proj2, aid2) = (u2, fx.project_id, fx.actor_id);
        let c2 = async move { complete_task(p2, proj2, aid2, u2c).await };
        tokio::join!(c1, c2);

        assert_eq!(
            task_status(&pool, u1).await,
            "completed",
            "round {round}: both upstreams must end completed"
        );
        assert_eq!(task_status(&pool, u2).await, "completed");
        assert_eq!(
            task_status(&pool, downstream).await,
            "ready",
            "round {round}: the downstream must unlock exactly once at the end"
        );
        assert_eq!(
            task_ready_events(&pool, fx.project_id, downstream).await,
            1,
            "round {round}: concurrent completions must collapse to one TaskReady"
        );
    }
}

// --------------------------------------------------------------- C65

/// C65：删除依赖与上游完成并发——不发生漏解锁或错误状态迁移。
#[sqlx::test(migrations = "../../migrations")]
async fn c65_edge_removal_racing_with_upstream_completion_still_unlocks(pool: PgPool) {
    for round in 0..4u32 {
        for remove_first in [true, false] {
            let fx = seed_project(&pool, &format!("TDC65-R{round}-{remove_first}")).await;
            let upstream = seed_task(&pool, fx.project_id, "reviewing").await;
            let done = seed_task(&pool, fx.project_id, "completed").await;
            let downstream = seed_task(&pool, fx.project_id, "blocked").await;
            add_dependency(
                &pool,
                downstream,
                upstream,
                &AuditActor::principal(fx.actor_id),
            )
            .await
            .unwrap();
            add_dependency(&pool, downstream, done, &AuditActor::principal(fx.actor_id))
                .await
                .unwrap();
            assert_eq!(task_status(&pool, downstream).await, "blocked");

            let p1 = pool.clone();
            let (down1, up1, aid1) = (downstream, upstream, fx.actor_id);
            let rm = async move {
                remove_dependency(&p1, down1, up1, &AuditActor::principal(aid1)).await
            };
            let p2 = pool.clone();
            let (up2, proj2, aid2) = (upstream, fx.project_id, fx.actor_id);
            let fin = async move { complete_task(p2, proj2, aid2, up2).await };

            let (r_rm, _) = if remove_first {
                tokio::join!(rm, fin)
            } else {
                let (a, b) = tokio::join!(fin, rm);
                (b, a)
            };
            let removed = r_rm.expect("edge removal must succeed under any interleaving");
            assert!(removed);

            assert_eq!(
                task_status(&pool, downstream).await,
                "ready",
                "round {round} remove_first={remove_first}: no permanent block is allowed"
            );
            assert_eq!(
                task_ready_events(&pool, fx.project_id, downstream).await,
                1,
                "round {round} remove_first={remove_first}: exactly one TaskReady"
            );
            assert_eq!(
                task_status(&pool, upstream).await,
                "completed",
                "round {round}: the upstream must not be dragged into a wrong state"
            );
        }
    }
}

// ------------------------------------------------- C68 / UI 直写路径的图锁覆盖

/// UI 直写终态（`pm_write::update_task`）的最小请求：只改 status + revision。
async fn ui_complete(pool: &PgPool, task_id: Uuid, actor_id: Uuid, revision: i64) {
    update_task(
        pool,
        task_id,
        UpdateTaskRequest {
            revision: Some(revision),
            title: Some("ui write".to_string()),
            description: None,
            // T-025 遗留一落地后的约束：UI 直写必须走合法边且不缺守卫证据。
            // `reviewing → completed` 的 E2 证据（验收完备）只有 D5 判定器/命令
            // 通道能提供——UI 直写该边被拒是正确行为。图锁覆盖改用 UI 可达的
            // 终态边 `ready → cancelled`（guard-free）。
            status: Some("cancelled".to_string()),
            priority: Some("normal".to_string()),
            assignee_id: None,
            goal: None,
            constraints: None,
            deliverables: None,
            acceptance_criteria: None,
            knowledge_refs: None,
        },
        &AuditActor::principal(actor_id),
    )
    .await
    .expect("the UI direct write must succeed");
}

/// C68（T-025 §五）：UI 直写终态必须触发与命令通道一致的依赖解锁副作用。
#[sqlx::test(migrations = "../../migrations")]
async fn c68_ui_direct_write_to_terminal_unlocks_downstream(pool: PgPool) {
    let fx = seed_project(&pool, "TDC68").await;
    let upstream = seed_task(&pool, fx.project_id, "ready").await;
    let downstream = seed_task(&pool, fx.project_id, "blocked").await;
    add_dependency(
        &pool,
        downstream,
        upstream,
        &AuditActor::principal(fx.actor_id),
    )
    .await
    .unwrap();

    ui_complete(&pool, upstream, fx.actor_id, 1).await;

    assert_eq!(task_status(&pool, upstream).await, "cancelled");
    assert_eq!(task_status(&pool, downstream).await, "ready");
    assert_eq!(
        task_ready_events(&pool, fx.project_id, downstream).await,
        1,
        "the UI path must produce exactly one TaskReady"
    );
}

/// 直写路径必须与建边/终态迁移共用同一把项目级图锁（否则 P0-2 在这条路径复发）。
/// 定向验证（非随机碰撞）：另一事务持图锁期间，直写终态必须等锁，且可在
/// `pg_locks` 观察到「advisory 锁未授予」；放行后直写完成并触发解锁。
#[sqlx::test(migrations = "../../migrations")]
async fn ui_terminal_write_waits_for_the_project_graph_lock(pool: PgPool) {
    let fx = seed_project(&pool, "TDC68LK").await;
    let upstream = seed_task(&pool, fx.project_id, "ready").await;
    let downstream = seed_task(&pool, fx.project_id, "blocked").await;
    add_dependency(
        &pool,
        downstream,
        upstream,
        &AuditActor::principal(fx.actor_id),
    )
    .await
    .unwrap();

    // 连接 1：像并发图写者一样持锁不放。
    let mut conn1 = pool.acquire().await.unwrap();
    let mut tx1 = conn1.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('task-graph:' || $1::text, 0))")
        .bind(fx.project_id.to_string())
        .execute(&mut *tx1)
        .await
        .unwrap();

    let writer = {
        let pool = pool.clone();
        let (task, actor) = (upstream, fx.actor_id);
        tokio::spawn(async move { ui_complete(&pool, task, actor, 1).await })
    };

    // 屏障点：等到直写确实开始排队（advisory 锁出现未授予记录，与键无关——
    // 该测试库由本用例独占）。
    let mut observed = false;
    for _ in 0..200 {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_locks WHERE locktype = 'advisory' AND NOT granted",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        if waiting > 0 {
            observed = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(
        observed,
        "the UI terminal write must wait on the project graph lock"
    );
    assert_eq!(
        task_status(&pool, upstream).await,
        "ready",
        "no task row may change before the graph lock is granted"
    );

    // 放行：直写完成并触发解锁。
    tx1.commit().await.unwrap();
    writer.await.unwrap();

    assert_eq!(task_status(&pool, upstream).await, "cancelled");
    assert_eq!(task_status(&pool, downstream).await, "ready");
    assert_eq!(
        task_ready_events(&pool, fx.project_id, downstream).await,
        1,
        "exactly one TaskReady after the lock is released"
    );
}
// ------------------------- T-025 §10.9 遗留收口（2026-11-03） -------------------------

/// 遗留一：UI 直写非法边被状态机拒绝（`reviewing → completed` 缺 E2 证据；
/// `draft → running` 不在合法边表）——不再能绕过 13 态状态机。
#[sqlx::test(migrations = "../../migrations")]
async fn t025_ui_direct_write_rejects_illegal_transitions(pool: PgPool) {
    let fx = seed_project(&pool, "TDC-T090A").await;
    let reviewing = seed_task(&pool, fx.project_id, "reviewing").await;
    let draft = seed_task(&pool, fx.project_id, "draft").await;

    // reviewing → completed：边合法但缺 E2 验收证据（只有 D5 判定器/命令通道能给）
    let err = update_task(
        &pool,
        reviewing,
        UpdateTaskRequest {
            revision: Some(1),
            title: Some("x".to_string()),
            description: None,
            status: Some("completed".to_string()),
            priority: Some("normal".to_string()),
            assignee_id: None,
            goal: None,
            constraints: None,
            deliverables: None,
            acceptance_criteria: None,
            knowledge_refs: None,
        },
        &AuditActor::principal(fx.actor_id),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, pm_application::pm_write::PmWriteError::Validation(_)),
        "got {err:?}"
    );

    // draft → running：不在合法边表
    let err = update_task(
        &pool,
        draft,
        UpdateTaskRequest {
            revision: Some(1),
            title: Some("x".to_string()),
            description: None,
            status: Some("running".to_string()),
            priority: Some("normal".to_string()),
            assignee_id: None,
            goal: None,
            constraints: None,
            deliverables: None,
            acceptance_criteria: None,
            knowledge_refs: None,
        },
        &AuditActor::principal(fx.actor_id),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, pm_application::pm_write::PmWriteError::Validation(_)),
        "got {err:?}"
    );
}
