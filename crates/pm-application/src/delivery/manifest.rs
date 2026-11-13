//! D4 的交付清单读模型（`docs/slice-d/ai-pm-slice-d4-delivery-v1.md` §2.3）。
//!
//! 清单只陈述事实：**不改状态机**。`delivery_state` 是视图列，永不成为任务状态
//! 字段；`delivered` 仅表示仓库交付契约完备，**不代表任务已验收或已完成**——
//! 最终裁决归 D5。

use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

/// 一次提交的交付清单（Accepted 的返回值）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryManifest {
    pub run_id: Uuid,
    pub task_id: Uuid,
    /// Run 级集成事实（`passed` / `failed` / `not_run`）。
    pub integration_status: String,
    /// 落库的 commit 行数。
    pub commit_count: usize,
    /// 执行端提出的知识建议条数（**只记数不落提案**——回写归 D5）。
    pub knowledge_proposal_count: usize,
    pub repositories: Vec<ManifestEntry>,
}

/// 清单里的一仓。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub repository_id: Uuid,
    pub repository_name: String,
    pub write_access: bool,
    /// 领取快照的基线（基线事实 = `task_run_repository.base_commit`）。
    pub base_commit: String,
    pub commit_sha: String,
    pub changed_files: i32,
    pub branch: Option<String>,
    pub commit_author: Option<String>,
    pub commit_authored_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 测试证据（`{submitted_at, tests:[...]}`）。
    pub test_evidence: serde_json::Value,
}

/// 读取某个 Run 已落库的交付清单。
pub async fn read_manifest(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    run_id: Uuid,
) -> Result<DeliveryManifest, crate::delivery::DeliveryError> {
    let run = sqlx::query(
        "SELECT r.task_id, r.integration_status FROM delivery.task_run r WHERE r.id = $1",
    )
    .bind(run_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| {
        crate::delivery::DeliveryError::LeaseInvalid("run not found in this project".into())
    })?;
    let task_id: Uuid = run.get("task_id");
    let integration_status: String = run.get("integration_status");

    let rows = sqlx::query(
        "SELECT c.repository_id, pr.name, c.commit_sha, c.changed_files, c.branch, \
                c.commit_author, c.commit_authored_at, c.test_evidence, \
                coalesce(trr.base_commit, '') AS base_commit \
           FROM delivery.task_repository_commit c \
           JOIN integration.project_repository pr \
             ON pr.id = c.repository_id AND pr.project_id = c.project_id \
           LEFT JOIN delivery.task_run_repository trr \
             ON trr.run_id = c.task_run_id AND trr.repository_id = c.repository_id \
          WHERE c.task_run_id = $1 \
          ORDER BY c.repository_id",
    )
    .bind(run_id)
    .fetch_all(&mut **tx)
    .await?;

    let repositories = rows
        .into_iter()
        .map(|row| ManifestEntry {
            repository_id: row.get("repository_id"),
            repository_name: row.get("name"),
            write_access: true,
            base_commit: row.get("base_commit"),
            commit_sha: row.get("commit_sha"),
            changed_files: row.get("changed_files"),
            branch: row.get("branch"),
            commit_author: row.get("commit_author"),
            commit_authored_at: row.get("commit_authored_at"),
            test_evidence: row.get("test_evidence"),
        })
        .collect::<Vec<_>>();

    Ok(DeliveryManifest {
        run_id,
        task_id,
        integration_status,
        commit_count: repositories.len(),
        knowledge_proposal_count: 0,
        repositories,
    })
}

/// 交付清单视图的一行（`delivery.v_task_delivery_manifest` 的只读投影）。
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ManifestRow {
    pub task_id: Uuid,
    pub project_id: Uuid,
    pub repository_id: Uuid,
    pub repository_name: String,
    pub write_access: bool,
    pub delivery_required: bool,
    pub latest_run_id: Option<Uuid>,
    pub latest_attempt_no: Option<i32>,
    pub integration_status: Option<String>,
    pub latest_submitted_run_id: Option<Uuid>,
    pub base_commit: Option<String>,
    pub commit_sha: Option<String>,
    pub changed_files: Option<i32>,
    pub test_evidence: Option<serde_json::Value>,
    pub branch: Option<String>,
    pub commit_author: Option<String>,
    pub commit_authored_at: Option<chrono::DateTime<chrono::Utc>>,
    pub expected_delivery_count: i32,
    pub actual_delivery_count: i32,
    pub delivery_state: String,
}

/// 按任务读取交付清单视图（只读）。
pub async fn list_manifest(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    task_id: Uuid,
) -> Result<Vec<ManifestRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT task_id, project_id, repository_id, repository_name, write_access, \
                delivery_required, latest_run_id, latest_attempt_no, integration_status, \
                latest_submitted_run_id, base_commit, commit_sha, changed_files, \
                test_evidence, branch, commit_author, commit_authored_at, \
                expected_delivery_count, actual_delivery_count, delivery_state \
           FROM delivery.v_task_delivery_manifest \
          WHERE project_id = $1 AND task_id = $2 \
          ORDER BY repository_id",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_all(pool)
    .await
}

/// 项目级交付聚合的一个任务卷（S01-W60 / 缺陷 D-13）。
#[derive(Debug, Clone, Serialize)]
pub struct TaskDeliveryRollup {
    pub task_id: Uuid,
    pub task_title: String,
    pub task_status: String,
    /// 待处理 Review 审批数（总体计划 §9.2「Review 状态」）。
    pub pending_review_approvals: i64,
    /// 逐仓交付契约（与任务级 `/delivery` 同一视图行）。
    pub repositories: Vec<ManifestRow>,
    /// 任务级卷起状态：`delivered` / `partial` / `missing` / `no_contract`。
    pub delivery_state: String,
    pub expected_delivery_count: i32,
    pub actual_delivery_count: i32,
}

/// 项目级交付聚合（总体计划 §9.1 W60 / §15.E「项目级交付可查」）。
///
/// 读模型：只读 `delivery.v_task_delivery_manifest` 视图 + `delivery.task` +
/// pending 审批计数，**不拥有业务状态、不重复维护领域状态**（I5 §9.3 同口径）。
///
/// 任务级卷起规则（与视图的逐仓 `delivery_state` 同源）：
/// - 契约仓数 = 0 → `no_contract`（无可交付契约，不算阻塞）；
/// - 全部 write-access 契约仓 `delivered` → `delivered`；
/// - 一个都没有 → `missing`；
/// - 其余 → `partial`。
pub async fn list_project_delivery(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<Vec<TaskDeliveryRollup>, sqlx::Error> {
    // 任务清单 + pending 审批数。
    let tasks: Vec<(Uuid, String, String, i64)> = sqlx::query_as(
        "SELECT t.id, t.title, t.status, \
                (SELECT count(*) FROM governance.review_approval a \
                  WHERE a.task_id = t.id AND a.status = 'pending') \
           FROM delivery.task t \
          WHERE t.project_id = $1 AND t.tenant_id = $2 \
          ORDER BY t.created_at DESC, t.id",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;

    // 全项目逐仓清单行（按任务逐仓；经 `delivery.task` 补租户谓词——视图本身不带 tenant）。
    let rows: Vec<ManifestRow> = sqlx::query_as(
        "SELECT m.task_id, m.project_id, m.repository_id, m.repository_name, m.write_access, \
                m.delivery_required, m.latest_run_id, m.latest_attempt_no, m.integration_status, \
                m.latest_submitted_run_id, m.base_commit, m.commit_sha, m.changed_files, \
                m.test_evidence, m.branch, m.commit_author, m.commit_authored_at, \
                m.expected_delivery_count, m.actual_delivery_count, m.delivery_state \
           FROM delivery.v_task_delivery_manifest m \
           JOIN delivery.task t ON t.id = m.task_id \
          WHERE m.project_id = $1 AND t.tenant_id = $2 \
          ORDER BY m.task_id, m.repository_id",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;

    let mut by_task: std::collections::HashMap<Uuid, Vec<ManifestRow>> =
        std::collections::HashMap::new();
    for row in rows {
        by_task.entry(row.task_id).or_default().push(row);
    }

    let items = tasks
        .into_iter()
        .map(|(task_id, title, status, pending)| {
            let repos = by_task.remove(&task_id).unwrap_or_default();
            let expected = repos
                .iter()
                .filter(|r| r.delivery_required)
                .map(|r| r.expected_delivery_count)
                .max()
                .unwrap_or(0);
            let actual = repos
                .iter()
                .filter(|r| r.delivery_required)
                .map(|r| r.actual_delivery_count)
                .max()
                .unwrap_or(0);
            let delivery_states: Vec<&str> = repos
                .iter()
                .filter(|r| r.delivery_required)
                .map(|r| r.delivery_state.as_str())
                .collect();
            // 视图的逐仓 `delivery_state` 已是任务级口径（同一任务的各行共享
            // expected/actual 计数）：`delivered` / `partial` / `never_submitted`。
            // 任务级卷起 = 对该组状态归类，而不是对每仓再求交集。
            let delivery_state = if delivery_states.is_empty() {
                "no_contract".to_string()
            } else if delivery_states.iter().all(|s| *s == "delivered") {
                "delivered".to_string()
            } else if delivery_states.contains(&"partial") {
                "partial".to_string()
            } else {
                "missing".to_string()
            };
            TaskDeliveryRollup {
                task_id,
                task_title: title,
                task_status: status,
                pending_review_approvals: pending,
                repositories: repos,
                delivery_state,
                expected_delivery_count: expected,
                actual_delivery_count: actual,
            }
        })
        .collect();

    Ok(items)
}
