//! 拍②的**只读事实采集**：把管线所需的输入从库里读出来，产物存在性由镜像仓
//! 只读探测补齐（D5 §4「全只读输入」）。
//!
//! 采集与判定分离：本模块只产出 [`ReviewInput`]，`pipeline::evaluate` 是纯函数。
//! 这样「同一输入必得同一输出」可被机械验证，也让管线不需要数据库或工作区。

use std::path::Path;

use serde_json::Value;
use sqlx::{PgPool, Row, Transaction};
use uuid::Uuid;

use crate::git_access::{self, GitError};
use crate::review::pipeline::{ArtifactProbe, CriterionSnapshot, ReviewInput, TestEvidence};
use crate::review::queue::{read_delivery_state, ReviewError};

/// git 探测的超时（与 D4 一致）。
const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// 一条 review run 的采集结果。
#[derive(Debug, Clone)]
pub struct CollectedReview {
    pub task_id: Uuid,
    pub project_id: Uuid,
    pub delivery_run_id: Uuid,
    pub input_manifest_hash: Option<String>,
    pub reviewed_manifest_hash: Option<String>,
    pub review_policy_version: Option<String>,
    pub input: ReviewInput,
}

/// 采集一条 review run 的全部只读输入。
pub async fn collect_review_input(
    pool: &PgPool,
    review_run_id: Uuid,
) -> Result<CollectedReview, ReviewError> {
    let run = sqlx::query(
        "SELECT project_id, task_id, input_manifest_hash, reviewed_manifest_hash, \
                review_policy_version \
           FROM delivery.task_run WHERE id = $1 AND run_kind = 'review'",
    )
    .bind(review_run_id)
    .fetch_optional(pool)
    .await?;
    let Some(run) = run else {
        return Err(ReviewError::Transition(format!(
            "review run {review_run_id} not found"
        )));
    };
    let project_id: Uuid = run.get("project_id");
    let task_id: Uuid = run.get("task_id");

    // 被审交付 run：由锚定哈希回指「最新 delivery run」。
    let delivery_run_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM delivery.task_run \
          WHERE task_id = $1 AND project_id = $2 AND run_kind = 'delivery' \
          ORDER BY attempt_no DESC, created_at DESC LIMIT 1",
    )
    .bind(task_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    let Some(delivery_run_id) = delivery_run_id else {
        return Err(ReviewError::Transition(
            "no delivery run for the reviewed task".to_string(),
        ));
    };

    let delivery_state = {
        let mut tx = pool.begin().await?;
        let state = read_delivery_state(&mut tx, project_id, task_id).await?;
        tx.commit().await?;
        state
    };
    let integration_status: String = sqlx::query_scalar(
        "SELECT integration_status FROM delivery.task_run WHERE id = $1",
    )
    .bind(delivery_run_id)
    .fetch_one(pool)
    .await?;

    let criteria = load_criterion_snapshots(pool, project_id, task_id).await?;
    let tests = load_tests(pool, delivery_run_id).await?;
    let deliverable_paths = load_deliverable_paths(pool, task_id).await?;
    let artifacts = probe_artifacts(pool, project_id, delivery_run_id, &deliverable_paths).await?;

    Ok(CollectedReview {
        task_id,
        project_id,
        delivery_run_id,
        input_manifest_hash: run.get("input_manifest_hash"),
        reviewed_manifest_hash: run.get("reviewed_manifest_hash"),
        review_policy_version: run.get("review_policy_version"),
        input: ReviewInput {
            delivery_state,
            integration_status,
            criteria,
            tests,
            artifacts,
            deliverable_paths,
        },
    })
}

async fn load_criterion_snapshots(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
) -> Result<Vec<CriterionSnapshot>, ReviewError> {
    let rows = sqlx::query(
        "SELECT id, ordinal, text, verification_kind \
           FROM delivery.task_acceptance_criterion \
          WHERE project_id = $1 AND task_id = $2 ORDER BY ordinal",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| CriterionSnapshot {
            criterion_id: row.get("id"),
            ordinal: row.get("ordinal"),
            text: row.get("text"),
            verification_kind: row.get("verification_kind"),
        })
        .collect())
}

/// 逐仓测试证据（`test_evidence.tests[]`，D4 原样落库）。
async fn load_tests(pool: &PgPool, delivery_run_id: Uuid) -> Result<Vec<TestEvidence>, ReviewError> {
    let rows = sqlx::query(
        "SELECT repository_id, test_evidence FROM delivery.task_repository_commit \
          WHERE task_run_id = $1 ORDER BY repository_id",
    )
    .bind(delivery_run_id)
    .fetch_all(pool)
    .await?;
    let mut out = Vec::new();
    for row in rows {
        let repository_id: Uuid = row.get("repository_id");
        let evidence: Value = row.get("test_evidence");
        let Some(list) = evidence.get("tests").and_then(Value::as_array) else {
            continue;
        };
        for test in list {
            let name = test
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if name.is_empty() {
                continue;
            }
            out.push(TestEvidence {
                repository_id,
                name,
                passed: test
                    .get("passed")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                evidence_ref: test
                    .get("evidence_ref")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            });
        }
    }
    Ok(out)
}

async fn load_deliverable_paths(pool: &PgPool, task_id: Uuid) -> Result<Vec<String>, ReviewError> {
    let deliverables: Value =
        sqlx::query_scalar("SELECT deliverables FROM delivery.task WHERE id = $1")
            .bind(task_id)
            .fetch_one(pool)
            .await?;
    Ok(deliverables
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("path").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default())
}

/// 产物存在性探测：在**镜像仓**里按 `commit_sha:path` 取对象（只读）。
///
/// 镜像仓不存在（尚未导入 / 已清理）时一律视为不存在——fail-closed：没有证据
/// 就不给通过。
async fn probe_artifacts(
    pool: &PgPool,
    project_id: Uuid,
    delivery_run_id: Uuid,
    deliverable_paths: &[String],
) -> Result<Vec<ArtifactProbe>, ReviewError> {
    if deliverable_paths.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query(
        "SELECT c.repository_id, c.commit_sha \
           FROM delivery.task_repository_commit c \
          WHERE c.task_run_id = $1 AND c.project_id = $2 \
          ORDER BY c.repository_id",
    )
    .bind(delivery_run_id)
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    let mut probes = Vec::new();
    for row in rows {
        let repository_id: Uuid = row.get("repository_id");
        let commit_sha: String = row.get("commit_sha");
        let mirror = crate::repo_facts::repo_cache_dir(repository_id);
        for path in deliverable_paths {
            let exists = object_exists(&mirror, &commit_sha, path).await;
            probes.push(ArtifactProbe {
                repository_id,
                path: path.clone(),
                exists,
            });
        }
    }
    Ok(probes)
}

/// `git -C <mirror> cat-file -e <sha>:<path>`：退出码 0 即存在。
async fn object_exists(mirror: &Path, commit_sha: &str, path: &str) -> bool {
    if !mirror.exists() || path.is_empty() || commit_sha.is_empty() {
        return false;
    }
    let spec = format!("{commit_sha}:{path}");
    match git_access::git_run(mirror, &["cat-file", "-e", &spec], PROBE_TIMEOUT).await {
        Ok(output) => output.ok(),
        Err(GitError::Timeout) | Err(_) => false,
    }
}

/// 锚定漂移复核（§3.3）：verdict 落库前回读现状重算双哈希，与 run 行比对。
/// 返回 `None` 表示一致（可以落 verdict），`Some(reason)` 表示漂移。
pub async fn detect_anchor_drift(
    tx: &mut Transaction<'static, sqlx::Postgres>,
    collected: &CollectedReview,
    review_run_id: Uuid,
) -> Result<Option<String>, ReviewError> {
    let current = crate::review::queue::load_reviewed_delivery(
        tx,
        collected.project_id,
        collected.task_id,
        collected.delivery_run_id,
    )
    .await?;
    if Some(&current.reviewed_manifest_hash) != collected.reviewed_manifest_hash.as_ref() {
        return Ok(Some("reviewed_manifest_hash drifted".to_string()));
    }
    if Some(&current.input_manifest_hash) != collected.input_manifest_hash.as_ref() {
        return Ok(Some("input_manifest_hash drifted".to_string()));
    }
    let policy = crate::review::anchor::review_policy_version();
    if Some(&policy) != collected.review_policy_version.as_ref() {
        return Ok(Some(format!(
            "review_policy_version changed ({policy} != {:?})",
            collected.review_policy_version
        )));
    }
    let _ = review_run_id;
    Ok(None)
}

