//! Outbox 事件的生产 handler（spec §20.5 / §20.6）。
//!
//! 首版只注册一个有真实落点的事件：`TaskReady` → 为本任务创建一条
//! `delivery.task_run(status='queued')`，即「等待调度」的最小真实形态。
//! `queued` 只表示待执行，**不**把任务标记为 Running——执行器在 Slice D 领取。
//!
//! 幂等与并发：在**同一事务**里先对 `delivery.task` 行加锁（`FOR UPDATE`），再
//! 依次校验状态、执行资格与是否已有有效 Run，最后建 Run 并递增计数。行锁把同一
//! 任务的并发消费串行化；「已有 queued/running Run 则跳过」让至少一次投递下的重复
//! 消费坍缩为一次创建；任务已不在可调度态（过期事件）则确认消费但不产生副作用。

use std::sync::Arc;

use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::outbox::ClaimedOutboxEvent;
use crate::outbox_worker::{OutboxHandler, OutboxWorker};

/// `TaskReady` 的消费 handler：为可调度的任务创建一条待执行 Run。
pub struct TaskReadyHandler {
    pool: PgPool,
}

impl TaskReadyHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn task_id_of(event: &ClaimedOutboxEvent) -> Option<Uuid> {
    event
        .payload
        .get("task_id")
        .and_then(|value| value.as_str())
        .and_then(|text| Uuid::parse_str(text).ok())
        .or_else(|| Uuid::parse_str(&event.aggregate_id).ok())
}

#[async_trait::async_trait]
impl OutboxHandler for TaskReadyHandler {
    async fn handle(&self, event: &ClaimedOutboxEvent) -> Result<(), String> {
        let task_id = task_id_of(event)
            .ok_or_else(|| "TaskReady payload is missing a task_id".to_string())?;
        let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;

        let row = sqlx::query(
            "SELECT status, assignee_id FROM delivery.task \
             WHERE id = $1 AND project_id = $2 FOR UPDATE",
        )
        .bind(task_id)
        .bind(event.project_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        let (status, assignee_id) = match row {
            Some(row) => (
                row.get::<String, _>("status"),
                row.get::<Option<Uuid>, _>("assignee_id"),
            ),
            // 任务已不存在：过期事件，确认消费。
            None => {
                tx.commit().await.map_err(|e| e.to_string())?;
                return Ok(());
            }
        };

        // 执行资格：只有仍处于可调度态（ready）且已指派 Agent 的任务才建 Run。
        // 状态已变化（过期事件）或未指派，则确认消费但不产生副作用（留痕，不静默）。
        if status != "ready" || assignee_id.is_none() {
            let reason = if status != "ready" {
                "task_not_ready"
            } else {
                "no_assignee"
            };
            tracing::info!(%task_id, reason, "TaskReady consumed without scheduling");
            tx.commit().await.map_err(|e| e.to_string())?;
            return Ok(());
        }

        // 任务级幂等（L0）：已有活跃 Run 则不再新建。三处口径必须一致：
        // 状态集合复用 run_dispatch 的单一真源（ACTIVE_RUN_STATUSES_SQL），
        // 且**必须带 `run_kind = 'delivery'` 谓词**——收窄后的 uq_task_run_active
        // 只约束 delivery，若计数不带谓词，活跃 review run 会挡住 delivery run。
        let active: i64 = sqlx::query_scalar(&format!(
            "SELECT count(*) FROM delivery.task_run \
             WHERE task_id = $1 AND {} AND status IN ({})",
            pm_application::run_dispatch::DELIVERY_RUN_KIND_SQL,
            pm_application::run_dispatch::ACTIVE_RUN_STATUSES_SQL
        ))
        .bind(task_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        if active > 0 {
            tracing::info!(%task_id, reason = "active_run_exists", "TaskReady consumed: active run exists");
            tx.commit().await.map_err(|e| e.to_string())?;
            return Ok(());
        }

        // L1 事件级幂等 + B3-6 共享封装：把创建它的事件 id 绑定进 Run（此处已持有
        // task 行锁，满足 B2 的 INSERT 顺序要求）。
        match pm_application::run_dispatch::insert_task_run(
            &mut tx,
            event.project_id,
            task_id,
            Some(event.id),
        )
        .await
        {
            Ok(_) => {}
            // L2 冲突降级：uq_task_run_source_event（同一事件已建 Run，如旧事件 replay
            // 而旧 Run 已终态）或 uq_task_run_active（并发先到者已建）冲突 → 幂等成功
            // 确认消费，**不进死信**。冲突时整个事务回滚，计数不会半更新。
            Err(sqlx::Error::Database(db)) if db.code().as_deref() == Some("23505") => {
                tracing::info!(
                    %task_id,
                    event_id = %event.id,
                    "TaskReady consumed: idempotent conflict, run already exists"
                );
                tx.rollback().await.map_err(|e| e.to_string())?;
                return Ok(());
            }
            Err(e) => return Err(e.to_string()),
        }

        tx.commit().await.map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// `DeliverySubmitted` 的消费 handler（D5 拍①）：推进 `submitted→reviewing` 并为
/// 本次交付创建一条 queued review run。
///
/// 幂等由 D5 应用层判据承担（存在锚定 `(task_id, input_manifest_hash)` 的任意
/// review run → 幂等返回），因此重复消费坍缩为一次创建；`23505`（并发先到者已建，
/// `uq_task_review_active` / 锚定冲突）同样降级为幂等成功，**不进死信**。
pub struct DeliverySubmittedHandler {
    pool: PgPool,
}

impl DeliverySubmittedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl OutboxHandler for DeliverySubmittedHandler {
    async fn handle(&self, event: &ClaimedOutboxEvent) -> Result<(), String> {
        let task_id = task_id_of(event)
            .ok_or_else(|| "DeliverySubmitted payload is missing a task_id".to_string())?;
        match pm_application::review::enqueue_review(&self.pool, event.project_id, task_id).await {
            Ok(outcome) => {
                tracing::info!(%task_id, ?outcome, "DeliverySubmitted consumed");
                Ok(())
            }
            Err(pm_application::review::ReviewError::Db(sqlx::Error::Database(db)))
                if db.code().as_deref() == Some("23505") =>
            {
                tracing::info!(
                    %task_id,
                    "DeliverySubmitted consumed: idempotent conflict, review run already exists"
                );
                Ok(())
            }
            Err(error) => Err(error.to_string()),
        }
    }
}

/// `RunCompleted` 的消费 handler（**记录型**事实通知，T-061 §5.6）。
///
/// 语义：记录「某次 Run 已经结束」这一不可逆业务事实——写一条
/// `governance.audit_log` + 一条 `integration.notification`。
/// **不改 Run 状态、不触发 D5 失败判定、不重新调度 Task**（状态推进权归 D5
/// 失败判定作业，本 handler 不与之争夺）。
///
/// 并发幂等（P0-2）：Outbox 是至少一次投递——租约过期、重复投递会让两个
/// Worker 同时处理同一事件。裸 `WHERE NOT EXISTS` 有「检查—插入」竞态窗口
/// （A、B 同查不存在 → 同插 → 双份），因此用**事务级 advisory lock**（键 =
/// run_id）串行化整个「检查 + 写入」过程。审计与通知**同一事务**原子提交：
/// 任一失败则两项一起回滚，事件不标记 delivered。
pub struct RunCompletedHandler {
    pool: PgPool,
}

impl RunCompletedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// 从载荷取 run_id（与生产端同构：payload.run_id 优先，回退 aggregate_id）。
fn run_id_of(event: &ClaimedOutboxEvent) -> Option<Uuid> {
    event
        .payload
        .get("run_id")
        .and_then(|value| value.as_str())
        .and_then(|text| Uuid::parse_str(text).ok())
        .or_else(|| Uuid::parse_str(&event.aggregate_id).ok())
}

#[async_trait::async_trait]
impl OutboxHandler for RunCompletedHandler {
    async fn handle(&self, event: &ClaimedOutboxEvent) -> Result<(), String> {
        let run_id = run_id_of(event)
            .ok_or_else(|| "RunCompleted payload is missing a run_id".to_string())?;
        let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;

        // ① 事务级 advisory lock：键 = run_id。事务结束自动释放，无需手工归还；
        //    后到的并发 Worker 在此排队，因此「检查 + 写入」窗口被串行化。
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::text, 0))")
            .bind(run_id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

        // ② 审计事实（幂等锚点：project_id + action + target_id = run_id）。
        //    `governance.audit_log` 无唯一约束 → 靠锁内的条件 INSERT 保证唯一。
        //    T-088：平台动作，真人三列 NULL；`claimed_device_id` 与 Run 领取时落下的
        //    `task_run.executor_id` **同源同值**（设备归因在 claim 时已校验）。
        sqlx::query(
            "INSERT INTO governance.audit_log \
             (project_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
              action, target_type, target_id, after_data, source_kind, tenant_id) \
             SELECT $1, $2, NULL, NULL, NULL, \
                    (SELECT r.executor_id FROM delivery.task_run r \
                      WHERE r.id = $3 AND EXISTS (SELECT 1 FROM devenv.device d WHERE d.id = r.executor_id)), \
                    'run_completed_event', 'task_run', $3, $4, 'run_dispatch', \
                    (SELECT tenant_id FROM core.project WHERE id = $1) \
             WHERE NOT EXISTS ( \
                 SELECT 1 FROM governance.audit_log \
                  WHERE target_id = $3 AND action = 'run_completed_event' AND project_id = $1)",
        )
        .bind(event.project_id)
        .bind(pm_application::run_dispatch::DISPATCH_SERVICE_PRINCIPAL)
        .bind(run_id)
        .bind(event.payload.clone())
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        // ③ 执行完成通知（幂等锚点：project_id + kind + payload->>'run_id'）。
        //    `source_proposal_id` 是 UUID UNIQUE 且此处必须为 NULL（`notify` 现有
        //    实现同样是 NULL）——本通知不来自任何提案。
        sqlx::query(
            "INSERT INTO integration.notification \
             (project_id, source_proposal_id, kind, payload, tenant_id) \
             SELECT $1, NULL, 'run_completed', $2, \
                    (SELECT tenant_id FROM core.project WHERE id = $1) \
             WHERE NOT EXISTS ( \
                 SELECT 1 FROM integration.notification \
                  WHERE project_id = $1 AND kind = 'run_completed' \
                    AND payload->>'run_id' = $3)",
        )
        .bind(event.project_id)
        .bind(event.payload.clone())
        .bind(run_id.to_string())
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        // ④ 审计与通知同事务提交：不可能出现「审计成功、通知失败、却 delivered」。
        tx.commit().await.map_err(|e| e.to_string())?;
        tracing::info!(%run_id, "RunCompleted consumed: audit + notification recorded");
        Ok(())
    }
}

/// **唯一的生产 Handler 装配入口**（T-061 §6.2.1）。
///
/// `pm-worker` 与 `ai-pm-worker` 共享同一张 outbox 表——任一实例抢到的事件都
/// 必须能被正确消费。零 Handler 空壳参与共享队列领取，会把另一 Worker **可正常
/// 处理**的事件经重试误判为死信（**假死信**，非业务失败）。
///
/// 两个 Worker 的运行配置、进程职责与生命周期各自独立；**统一的是 Handler
/// 注册能力**。装配集合必须覆盖 `EventCatalog` 中 `has_handler = true` 的全集
/// ——由 `OutboxWorker::assert_catalog_consistent` 在启动时校验。
///
/// 并发边界：「集合一致」不等于「并发安全」——原子领取（`SKIP LOCKED` + 租约）
/// 与各 Handler 的幂等锚点另行保证（`RunCompletedHandler` 见其 advisory lock）。
pub fn register_all_handlers(worker: OutboxWorker, pool: &PgPool) -> OutboxWorker {
    worker
        .with_handler("TaskReady", Arc::new(TaskReadyHandler::new(pool.clone())))
        .with_handler(
            "DeliverySubmitted",
            Arc::new(DeliverySubmittedHandler::new(pool.clone())),
        )
        .with_handler("RunCompleted", Arc::new(RunCompletedHandler::new(pool.clone())))
}