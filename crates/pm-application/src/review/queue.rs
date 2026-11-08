//! 拍①：`DeliverySubmitted` 事件 handler 的轻拍编排（D5 §3.1）。
//!
//! 只做三件事，全部在**单事务**内：
//!
//! 1. **幂等判据前置**（§3.4 场景 1）：已存在锚定同一 `(task_id,
//!    input_manifest_hash)` 的 review run（含重试 run）→ 幂等返回，不建 run、
//!    不重复审计、不重复迁移。
//! 2. **B1 守卫**：回读 `v_task_delivery_manifest`，只有 `delivery_state='delivered'`
//!    才继续；`partial` 停 `submitted` 等执行端同 run 补交（D4 幂等语义支持追加）。
//! 3. 通过 → `submitted→reviewing`（经 D1 受守卫通道）+ 创建 `queued` review run
//!    （`run_kind='review'`、`role_id='reviewer'`、`executor_kind='platform'`、
//!    双哈希 + 策略版本落行）+ 审计 `review.queued`。
//!
//! 「慢操作」不在这里：确定性校验管线归拍②（worker review 领取循环）。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row, Transaction};
use uuid::Uuid;

use pm_domain::TaskStatus;

use crate::command_executor::{apply_task_transition, system_context, TransitionFacts};
use crate::review::anchor::{
    review_policy_version, reviewed_manifest_hash, CriterionAnchor, RepositoryAnchor,
};

/// Reviewer Run 的角色标识（`chk_review_role_present` 要求 review 行必须带角色）。
pub const REVIEWER_ROLE: &str = "reviewer";

/// 平台执行器标识（review run 的唯一合法 `executor_kind`）。
pub const PLATFORM_EXECUTOR: &str = "platform";

/// 拍①的结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum EnqueueOutcome {
    /// 已建 review run（或已有同锚定 run → 幂等命中）。
    Queued {
        review_run_id: Uuid,
        input_manifest_hash: String,
        already_queued: bool,
    },
    /// B1 守卫拒绝：交付对账不是 `delivered`（停 `submitted` 等补交）。
    NotDelivered { delivery_state: String },
    /// 任务不在 `submitted`（过期事件）。
    TaskNotSubmitted { status: String },
    /// 任务或交付 run 不存在（过期事件）。
    NotFound,
    /// 锚定无法计算（缺少基线快照 / 交付 commit）。
    AnchorUnavailable { reason: String },
}

#[derive(Debug, thiserror::Error)]
pub enum ReviewError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("state transition refused: {0}")]
    Transition(String),
}

impl ReviewError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Db(_) => "db_error",
            Self::Transition(_) => "transition_refused",
        }
    }
}

/// 被审交付的锚定快照（拍①计算的输入）。
#[derive(Debug, Clone)]
pub struct ReviewedDelivery {
    pub delivery_run_id: Uuid,
    pub reviewed_manifest_hash: String,
    pub input_manifest_hash: String,
}

/// 读被审交付快照：最新 delivery run 的逐仓 `(base_commit, commit_sha)` +
/// 当前验收条目。**只读**，不改任何事实。
pub async fn load_reviewed_delivery(
    tx: &mut Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    delivery_run_id: Uuid,
) -> Result<ReviewedDelivery, ReviewError> {
    let rows = sqlx::query(
        "SELECT c.repository_id, c.commit_sha, trr.base_commit \
           FROM delivery.task_repository_commit c \
           JOIN delivery.task_run_repository trr \
             ON trr.run_id = c.task_run_id AND trr.repository_id = c.repository_id \
          WHERE c.task_run_id = $1 AND c.project_id = $2 \
          ORDER BY c.repository_id",
    )
    .bind(delivery_run_id)
    .bind(project_id)
    .fetch_all(&mut **tx)
    .await?;
    let repositories: Vec<RepositoryAnchor> = rows
        .into_iter()
        .map(|row| RepositoryAnchor {
            repository_id: row.get("repository_id"),
            base_commit: row.get("base_commit"),
            commit_sha: row.get("commit_sha"),
        })
        .collect();
    let reviewed = reviewed_manifest_hash(delivery_run_id, &repositories);

    let criteria = load_criteria(tx, project_id, task_id).await?;
    let input = crate::review::anchor::input_manifest_hash(&reviewed, &criteria);
    Ok(ReviewedDelivery {
        delivery_run_id,
        reviewed_manifest_hash: reviewed,
        input_manifest_hash: input,
    })
}

/// 验收条目快照（按 ordinal 升序）。
pub(crate) async fn load_criteria(
    tx: &mut Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    task_id: Uuid,
) -> Result<Vec<CriterionAnchor>, ReviewError> {
    let rows = sqlx::query(
        "SELECT ordinal, text, verification_kind \
           FROM delivery.task_acceptance_criterion \
          WHERE project_id = $1 AND task_id = $2 ORDER BY ordinal",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| CriterionAnchor {
            ordinal: row.get("ordinal"),
            text: row.get("text"),
            verification_kind: row.get("verification_kind"),
        })
        .collect())
}

/// 拍①：为一次交付创建 review run。
///
/// `SubtaskStatus` 任务行与交付视图的回读全部在同一事务内完成，保证「判据看到
/// 的事实」与「落库的事实」不会在两者之间漂移（行锁串行化同任务的并发消费）。
pub async fn enqueue_review(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
) -> Result<EnqueueOutcome, ReviewError> {
    let mut tx = pool.begin().await?;

    // I1-A 归档守卫 + I1-B 锁序：**先锁 Project 行**，再锁 Task、最后建 Run——
    // 与 TaskReady/Worker claim 同为 `project → run → task`（§17.1 裁决 3）。
    // 创建与归档在同一把行锁上串行：归档先拿锁 → 这里看到 archived 合法跳过；
    // 创建先拿锁 → 归档随后看到新建的 queued run 并条件取消（I1-012/I1-013）。
    match crate::project_gate::lock_project_activity_tx(&mut tx, project_id).await {
        Ok(crate::project_gate::ProjectActivity::Archived) => {
            tx.commit().await?;
            return Ok(EnqueueOutcome::NotFound);
        }
        Ok(crate::project_gate::ProjectActivity::Active) => {}
        Err(crate::project_gate::GateError::NotFound) => {
            tx.commit().await?;
            return Ok(EnqueueOutcome::NotFound);
        }
        Err(err) => return Err(ReviewError::Db(sqlx::Error::Protocol(err.to_string()))),
    }

    // 锁任务行（本路径的 run 由本事务创建，故 task 在 run 之前）。
    let task = sqlx::query(
        "SELECT status FROM delivery.task WHERE id = $1 AND project_id = $2 FOR UPDATE",
    )
    .bind(task_id)
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(task) = task else {
        tx.commit().await?;
        return Ok(EnqueueOutcome::NotFound);
    };
    let status: String = task.get("status");

    // 最新 delivery run（与 D4 视图「最新 Run」口径一致：attempt_no 最大）。
    let latest: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM delivery.task_run \
          WHERE task_id = $1 AND project_id = $2 AND run_kind = 'delivery' \
          ORDER BY attempt_no DESC, created_at DESC LIMIT 1",
    )
    .bind(task_id)
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(delivery_run_id) = latest else {
        tx.commit().await?;
        return Ok(EnqueueOutcome::TaskNotSubmitted { status });
    };

    // 锚定计算（需要基线快照 + 交付 commit；`delivered` 时两者必然齐备）。
    let reviewed = match load_reviewed_delivery(&mut tx, project_id, task_id, delivery_run_id).await
    {
        Ok(reviewed) if !reviewed.reviewed_manifest_hash.is_empty() => reviewed,
        Ok(_) => {
            tx.commit().await?;
            return Ok(EnqueueOutcome::AnchorUnavailable {
                reason: "delivery run has no commit rows".to_string(),
            });
        }
        Err(ReviewError::Db(error)) => return Err(ReviewError::Db(error)),
        Err(ReviewError::Transition(reason)) => {
            tx.commit().await?;
            return Ok(EnqueueOutcome::AnchorUnavailable { reason });
        }
    };

    // 幂等判据**先于状态检查**（§3.1 拍① P0-5）：同一份锚定已有 review run
    // （含重试 run）→ 幂等返回。放在状态检查之后会让「重复消费」被误判为
    // `TaskNotSubmitted`（任务此刻已是 `reviewing`），幂等就永远不可达。
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM delivery.task_run \
          WHERE task_id = $1 AND project_id = $2 AND run_kind = 'review' \
            AND input_manifest_hash = $3 \
          ORDER BY attempt_no DESC LIMIT 1",
    )
    .bind(task_id)
    .bind(project_id)
    .bind(&reviewed.input_manifest_hash)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(review_run_id) = existing {
        tx.commit().await?;
        return Ok(EnqueueOutcome::Queued {
            review_run_id,
            input_manifest_hash: reviewed.input_manifest_hash,
            already_queued: true,
        });
    }

    // 任务必须仍停在 `submitted`（否则是过期事件，确认消费但不产生副作用）。
    if status != "submitted" {
        tx.commit().await?;
        return Ok(EnqueueOutcome::TaskNotSubmitted { status });
    }

    // B1 守卫：交付对账必须 `delivered`。
    let delivery_state = read_delivery_state(&mut tx, project_id, task_id).await?;
    if delivery_state != "delivered" {
        tx.commit().await?;
        return Ok(EnqueueOutcome::NotDelivered { delivery_state });
    }

    // 状态推进：submitted → reviewing（D1 受守卫通道）。
    let ctx = system_context(&mut tx, project_id)
        .await
        .map_err(|e| ReviewError::Transition(e.to_string()))?;
    apply_task_transition(
        &mut tx,
        task_id,
        None,
        TaskStatus::Reviewing,
        &TransitionFacts::for_dispatch(),
        &ctx,
    )
    .await
    .map_err(|e| ReviewError::Transition(e.to_string()))?;

    // 建 review run（queued）。attempt_no 按 kind 分轨。
    let review_run_id =
        insert_review_run(&mut tx, project_id, task_id, delivery_run_id, &reviewed).await?;

    audit_review(
        &mut tx,
        project_id,
        "review.queued",
        review_run_id,
        serde_json::json!({ "delivery_run_id": delivery_run_id }),
    )
    .await?;

    tx.commit().await?;
    Ok(EnqueueOutcome::Queued {
        review_run_id,
        input_manifest_hash: reviewed.input_manifest_hash,
        already_queued: false,
    })
}

/// 建一条 `queued` review run（`executor_kind='platform'`，凭据列全 NULL）。
pub(crate) async fn insert_review_run(
    tx: &mut Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    delivery_run_id: Uuid,
    reviewed: &ReviewedDelivery,
) -> Result<Uuid, ReviewError> {
    let attempt_no: i32 = sqlx::query_scalar(
        "SELECT coalesce(max(attempt_no), 0) + 1 FROM delivery.task_run \
          WHERE task_id = $1 AND run_kind = 'review'",
    )
    .bind(task_id)
    .fetch_one(&mut **tx)
    .await?;
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run \
         (project_id, task_id, attempt_no, status, run_kind, role_id, executor_kind, \
          input_manifest_hash, reviewed_manifest_hash, review_policy_version, next_dispatch_at, \
          execution_mode, tenant_id) \
         VALUES ($1, $2, $3, 'queued', 'review', $4, $5, $6, $7, $8, now(), 'spawn', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(attempt_no)
    .bind(REVIEWER_ROLE)
    .bind(PLATFORM_EXECUTOR)
    .bind(&reviewed.input_manifest_hash)
    .bind(&reviewed.reviewed_manifest_hash)
    .bind(review_policy_version())
    .fetch_one(&mut **tx)
    .await?;
    // 被审 delivery run 通过 `rev` 语义锚定：审计与审批表都引用它。
    let _ = delivery_run_id;
    Ok(run_id)
}

/// 交付对账状态（读视图，按 task 取任一行的 `delivery_state`——同一 task 的所有
/// 行取值相同，视图按 task 计算）。
pub(crate) async fn read_delivery_state(
    tx: &mut Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    task_id: Uuid,
) -> Result<String, ReviewError> {
    let state: Option<String> = sqlx::query_scalar(
        "SELECT delivery_state FROM delivery.v_task_delivery_manifest \
          WHERE project_id = $1 AND task_id = $2 LIMIT 1",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(state.unwrap_or_else(|| "never_submitted".to_string()))
}

/// 这些 actor 归因事实，供审计 payload 复用。
#[derive(Debug, Clone, Default)]
pub struct ReviewFacts {
    pub delivery_run_id: Option<Uuid>,
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// 写一条 review 相关审计（actor 固定 service principal——审查是平台动作，
/// 真人三列与设备归因为 NULL）。
pub(crate) async fn audit_review(
    tx: &mut Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    action: &str,
    review_run_id: Uuid,
    after: serde_json::Value,
) -> Result<(), ReviewError> {
    let actor = crate::actor::AuditActor::dispatch();
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (project_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
          action, target_type, target_id, after_data, source_kind, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'task_run', $8, $9, 'review', \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(actor.principal_id)
    .bind(actor.identity_id)
    .bind(&actor.client)
    .bind(&actor.auth_mode)
    .bind(actor.claimed_device_id)
    .bind(action)
    .bind(review_run_id)
    .bind(after)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
