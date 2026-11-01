//! D4 的交付清单读模型（`docs/ai-pm-slice-d4-delivery-v1.md` §2.3）。
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
