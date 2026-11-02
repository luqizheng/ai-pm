//! `GET /v1/pm/acceptance-overview` 与任务级验收证据端点的集成测试
//! （T-070 / `docs/ai-pm-delivery-acceptance-workbench-v1.md` §四 / §5.1 / §5.3）。
//!
//! 判定规则、Q1「待补充不被过滤」、徽标互斥、动作区状态与 404 边界全部钉在
//! 真实数据库上；判定逻辑本身在后端（前端无资格推导）。

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

mod common;

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

async fn body_json(response: axum::response::Response) -> (StatusCode, serde_json::Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

async fn seed_project(pool: &PgPool, code: &str) -> Uuid {
    let project_id = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap()
    .project_id;
    common::join_dev(pool, project_id).await;
    project_id
}

async fn seed_task(
    pool: &PgPool,
    project_id: Uuid,
    title: &str,
    status: &str,
    created_minutes_ago: i32,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, tenant_id, created_at) \
         VALUES ($1, $2, $3, (SELECT tenant_id FROM core.project WHERE id = $1), \
                 now() - make_interval(mins => $4::int)) RETURNING id",
    )
    .bind(project_id)
    .bind(title)
    .bind(status)
    .bind(created_minutes_ago)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_repo(pool: &PgPool, project_id: Uuid, name: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO integration.project_repository (project_id, name, url, tenant_id) \
         VALUES ($1, $2, $3, (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(name)
    .bind(format!("ssh://git@example.com/{name}.git"))
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_task_repo(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    repo_id: Uuid,
    write_access: bool,
) {
    sqlx::query(
        "INSERT INTO delivery.task_repository (task_id, project_id, repository_id, write_access, tenant_id) \
         VALUES ($1, $2, $3, $4, (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(task_id)
    .bind(project_id)
    .bind(repo_id)
    .bind(write_access)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_delivery_run(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    attempt_no: i32,
    integration_status: &str,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task_run \
             (project_id, task_id, attempt_no, run_kind, status, outcome, integration_status, \
              started_at, finished_at, tenant_id) \
         VALUES ($1, $2, $3, 'delivery', 'succeeded', 'succeeded', $4, now(), now(), \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(attempt_no)
    .bind(integration_status)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_review_run(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    attempt_no: i32,
    review_verdict: &str,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task_run \
             (project_id, task_id, attempt_no, run_kind, status, review_verdict, \
              role_id, review_policy_version, executor_kind, finished_at, tenant_id) \
         VALUES ($1, $2, $3, 'review', 'succeeded', $4, \
                 'reviewer', 'v1', 'platform', now(), \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(attempt_no)
    .bind(review_verdict)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_run_repo(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
    repo_id: Uuid,
    base_commit: &str,
) {
    sqlx::query(
        "INSERT INTO delivery.task_run_repository \
             (run_id, project_id, repository_id, base_ref, base_commit, write_access, \
              workspace_path, tenant_id) \
         VALUES ($1, $2, $3, 'main', $4, true, $5, \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(run_id)
    .bind(project_id)
    .bind(repo_id)
    .bind(base_commit)
    .bind(format!("/ws/{run_id}"))
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_commit(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
    repo_id: Uuid,
    sha: &str,
) {
    sqlx::query(
        "INSERT INTO delivery.task_repository_commit \
             (tenant_id, project_id, task_run_id, repository_id, commit_sha, branch) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3, $4, 'feature/x')",
    )
    .bind(project_id)
    .bind(run_id)
    .bind(repo_id)
    .bind(sha)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_finding(
    pool: &PgPool,
    project_id: Uuid,
    review_run_id: Uuid,
    severity: &str,
    kind: &str,
    message: &str,
    code_location: Option<&str>,
) {
    sqlx::query(
        "INSERT INTO delivery.review_finding \
             (tenant_id, project_id, review_run_id, finding_kind, severity, message, code_location) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3, $4, $5, $6)",
    )
    .bind(project_id)
    .bind(review_run_id)
    .bind(kind)
    .bind(severity)
    .bind(message)
    .bind(code_location)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_criterion(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    ordinal: i32,
    text: &str,
    verification_kind: &str,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task_acceptance_criterion \
             (project_id, task_id, ordinal, text, verification_kind, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(ordinal)
    .bind(text)
    .bind(verification_kind)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_verdict(
    pool: &PgPool,
    project_id: Uuid,
    criterion_id: Uuid,
    review_run_id: Uuid,
    verdict: &str,
) {
    sqlx::query(
        "INSERT INTO delivery.acceptance_criterion_verdict \
             (tenant_id, project_id, criterion_id, review_run_id, verdict, evidence) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3, $4, '{}'::jsonb)",
    )
    .bind(project_id)
    .bind(criterion_id)
    .bind(review_run_id)
    .bind(verdict)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_approval(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    review_run_id: Uuid,
    delivery_run_id: Uuid,
    status: &str,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO governance.review_approval \
             (tenant_id, project_id, task_id, review_run_id, delivery_run_id, \
              delivery_manifest_hash, kind, status) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3, $4, \
                 'manifest-hash', 'manual_review', $5) RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(review_run_id)
    .bind(delivery_run_id)
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 种子：一个项目里 §四 的全部徽标场景 + 排除态任务 + 第二个不可见项目。
async fn seed_workspace(pool: &PgPool) -> (Uuid, Uuid, Uuid, Uuid, Uuid, Uuid, Uuid, Uuid) {
    let project_id = seed_project(pool, "ACC-1").await;
    let repo_a = seed_repo(pool, project_id, "repo-a").await;
    let repo_b = seed_repo(pool, project_id, "repo-b").await;

    // blocked（blocker finding + 已 rejected 审批 → decided）。
    let blocked_task = seed_task(pool, project_id, "blocked task", "reviewing", 9).await;
    seed_task_repo(pool, project_id, blocked_task, repo_a, true).await;
    seed_task_repo(pool, project_id, blocked_task, repo_b, true).await;
    let blocked_delivery =
        seed_delivery_run(pool, project_id, blocked_task, 1, "passed").await;
    seed_run_repo(pool, project_id, blocked_delivery, repo_a, "aaaaaaaaaa").await;
    seed_run_repo(pool, project_id, blocked_delivery, repo_b, "bbbbbbbbbb").await;
    seed_commit(pool, project_id, blocked_delivery, repo_a, "ca1").await;
    seed_commit(pool, project_id, blocked_delivery, repo_b, "cb1").await;
    let blocked_review = seed_review_run(pool, project_id, blocked_task, 1, "changes_requested").await;
    seed_finding(pool, project_id, blocked_review, "blocker", "contract_violation", "缺少变更清单", Some("repo-a/src/lib.rs:1-10")).await;
    seed_finding(pool, project_id, blocked_review, "warning", "style", "命名不一致", None).await;
    seed_finding(pool, project_id, blocked_review, "info", "note", "仅供参考", None).await;
    seed_approval(pool, project_id, blocked_task, blocked_review, blocked_delivery, "rejected").await;

    // blocked（集成失败，无审查记录 → none）。
    let int_failed_task = seed_task(pool, project_id, "integration failed", "submitted", 8).await;
    seed_task_repo(pool, project_id, int_failed_task, repo_a, true).await;
    let int_failed_delivery =
        seed_delivery_run(pool, project_id, int_failed_task, 1, "failed").await;
    seed_run_repo(pool, project_id, int_failed_delivery, repo_a, "cccccccccc").await;
    seed_commit(pool, project_id, int_failed_delivery, repo_a, "cc1").await;

    // missing（部分交付，集成通过）。
    let partial_task = seed_task(pool, project_id, "partial delivery", "submitted", 7).await;
    seed_task_repo(pool, project_id, partial_task, repo_a, true).await;
    seed_task_repo(pool, project_id, partial_task, repo_b, true).await;
    let partial_delivery = seed_delivery_run(pool, project_id, partial_task, 1, "passed").await;
    seed_run_repo(pool, project_id, partial_delivery, repo_a, "dddddddddd").await;
    seed_run_repo(pool, project_id, partial_delivery, repo_b, "eeeeeeeeee").await;
    seed_commit(pool, project_id, partial_delivery, repo_a, "cd1").await;

    // missing（零交付：无仓、无 run —— Q1「待补充」不被过滤）。
    let bare_task = seed_task(pool, project_id, "bare task", "submitted", 6).await;

    // eligible（交付齐 + 集成通过 + 无 blocker + pending 审批 → awaiting_decision）。
    let eligible_task = seed_task(pool, project_id, "eligible task", "submitted", 10).await;
    seed_task_repo(pool, project_id, eligible_task, repo_a, true).await;
    seed_task_repo(pool, project_id, eligible_task, repo_b, true).await;
    let eligible_delivery =
        seed_delivery_run(pool, project_id, eligible_task, 1, "passed").await;
    seed_run_repo(pool, project_id, eligible_delivery, repo_a, "ffffffffff").await;
    seed_run_repo(pool, project_id, eligible_delivery, repo_b, "1111111111").await;
    seed_commit(pool, project_id, eligible_delivery, repo_a, "ea1").await;
    seed_commit(pool, project_id, eligible_delivery, repo_b, "eb1").await;
    let eligible_review = seed_review_run(pool, project_id, eligible_task, 1, "approved").await;
    seed_finding(pool, project_id, eligible_review, "info", "note", "一切正常", None).await;
    let criterion_1 = seed_criterion(pool, project_id, eligible_task, 1, "接口测试通过", "automated_test").await;
    let _criterion_2 = seed_criterion(pool, project_id, eligible_task, 2, "人工确认文档", "manual").await;
    seed_verdict(pool, project_id, criterion_1, eligible_review, "passed").await;
    let pending_approval =
        seed_approval(pool, project_id, eligible_task, eligible_review, eligible_delivery, "pending").await;

    // 排除态：completed 不属于待验收集合。
    seed_task(pool, project_id, "done task", "completed", 5).await;

    // 第二个项目：dev principal 非成员 → 404。
    let other_project = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: "ACC-OTHER".to_string(),
            project_name: "ACC-OTHER".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap()
    .project_id;
    let other_task = seed_task(pool, other_project, "hidden task", "submitted", 4).await;

    (
        project_id,
        other_task,
        blocked_task,
        int_failed_task,
        partial_task,
        bare_task,
        eligible_task,
        pending_approval,
    )
}

#[sqlx::test(migrations = "../../migrations")]
async fn overview_counts_badges_and_review_status(pool: PgPool) {
    let (project_id, _hidden, blocked, int_failed, partial, bare, eligible, pending_approval) =
        seed_workspace(&pool).await;

    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(get(&format!(
                "/v1/pm/acceptance-overview?project_id={project_id}"
            )))
            .await
            .unwrap(),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["project_id"], serde_json::json!(project_id));
    // 三指标：pending=任务生命周期口径（completed 不算）；blocked=2；eligible=1。
    assert_eq!(
        body["counts"],
        serde_json::json!({ "pending": 5, "eligible": 1, "blocked": 2 }),
        "body: {body}"
    );
    let items = body["tasks"].as_array().unwrap();
    assert_eq!(items.len(), 5);
    assert_eq!(body["total"], 5);

    // 排序：blocked 优先（组内 created_at DESC），其次 missing，eligible 靠后。
    let titles: Vec<&str> = items
        .iter()
        .map(|t| t["title"].as_str().unwrap())
        .collect();
    assert_eq!(
        titles,
        vec!["integration failed", "blocked task", "bare task", "partial delivery", "eligible task"],
        "body: {body}"
    );

    // 徽标互斥 + 后端判定（前端不推导）。
    let badges: Vec<&str> = items
        .iter()
        .map(|t| t["badge"].as_str().unwrap())
        .collect();
    assert_eq!(badges, vec!["blocked", "blocked", "missing", "missing", "eligible"]);

    // blocked #1：集成失败即阻塞，无需审查记录（组内 created_at DESC，新者在前）。
    assert_eq!(items[0]["task_id"], serde_json::json!(int_failed));
    assert_eq!(items[0]["delivery_summary"]["integration_status"], "failed");
    assert_eq!(items[0]["review_status"], "none");

    // blocked #2：blocker 计数来自最新成功 review run；审批已决 → decided。
    assert_eq!(items[1]["task_id"], serde_json::json!(blocked));
    assert_eq!(items[1]["blocker_count"], 1);
    assert_eq!(items[1]["review_status"], "decided");
    assert!(items[1]["pending_approval_id"].is_null());
    assert_eq!(
        items[1]["delivery_summary"],
        serde_json::json!({
            "expected_repos": 2, "delivered_repos": 2, "missing_repos": [],
            "integration_status": "passed"
        })
    );

    // missing #1（Q1）：零交付任务不被过滤，标待补充。
    assert_eq!(items[2]["task_id"], serde_json::json!(bare));
    assert_eq!(
        items[2]["delivery_summary"],
        serde_json::json!({
            "expected_repos": 0, "delivered_repos": 0, "missing_repos": [],
            "integration_status": serde_json::Value::Null
        })
    );
    assert!(items[2]["latest_delivery_run_id"].is_null());

    // missing #2：部分交付，缺仓名可见。
    assert_eq!(items[3]["task_id"], serde_json::json!(partial));
    assert_eq!(
        items[3]["delivery_summary"]["missing_repos"],
        serde_json::json!(["repo-b"])
    );
    assert_eq!(items[3]["delivery_summary"]["delivered_repos"], 1);

    // eligible：动作区状态 + pending 审批 id 直接可点。
    assert_eq!(items[4]["task_id"], serde_json::json!(eligible));
    assert_eq!(items[4]["review_status"], "awaiting_decision");
    assert_eq!(items[4]["pending_approval_id"], serde_json::json!(pending_approval));
    assert_eq!(items[4]["blocker_count"], 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn evidence_endpoints_findings_verdicts_and_approvals(pool: PgPool) {
    let (project_id, _hidden, blocked, _int_failed, _partial, _bare, eligible, _pending) =
        seed_workspace(&pool).await;

    // findings：blocker 置顶，携带所属 review run。
    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(get(&format!(
                "/v1/pm/tasks/{blocked}/review-findings"
            )))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let items = body["items"].as_array().unwrap();
    let severities: Vec<&str> = items
        .iter()
        .map(|f| f["severity"].as_str().unwrap())
        .collect();
    assert_eq!(severities, vec!["blocker", "warning", "info"], "body: {body}");
    assert_eq!(items[0]["kind"], "contract_violation");
    assert_eq!(items[0]["code_location"], "repo-a/src/lib.rs:1-10");
    assert_eq!(items[0]["review_attempt_no"], 1);
    assert_eq!(items[0]["review_run_status"], "succeeded");

    // criterion-verdicts：逐标准最新一次成功 review run 的裁决；未覆盖的 evaluated=false。
    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(get(&format!(
                "/v1/pm/tasks/{eligible}/criterion-verdicts"
            )))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2, "body: {body}");
    assert_eq!(items[0]["ordinal"], 1);
    assert_eq!(items[0]["text"], "接口测试通过");
    assert_eq!(items[0]["evaluated"], true);
    assert_eq!(items[0]["verdict"], "passed");
    assert_eq!(items[1]["ordinal"], 2);
    assert_eq!(items[1]["evaluated"], false, "不造 awaiting_manual 假值");
    assert!(items[1]["verdict"].is_null());

    // review-approvals（task 级投影，含 pending 与已决）。
    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(get(&format!(
                "/v1/pm/tasks/{eligible}/review-approvals"
            )))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["status"], "pending");
    assert_eq!(items[0]["task_title"], "eligible task");

    let (status, body) = body_json(
        router(pool.clone())
            .oneshot(get(&format!(
                "/v1/pm/tasks/{blocked}/review-approvals"
            )))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["items"][0]["status"], "rejected");
    assert_eq!(body["items"][0]["decision_reason"], serde_json::Value::Null);
}

#[sqlx::test(migrations = "../../migrations")]
async fn overview_and_evidence_enforce_visibility_boundary(pool: PgPool) {
    let (project_id, hidden_task, blocked, _i, _p, _b, _e, _pa) = seed_workspace(&pool).await;

    // 缺 project_id → 400。
    let (status, _) = body_json(
        router(pool.clone())
            .oneshot(get("/v1/pm/acceptance-overview"))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 未知项目（同租户非成员/不存在）→ 404，不泄露存在性。
    let unknown = Uuid::new_v4();
    let (status, _) = body_json(
        router(pool.clone())
            .oneshot(get(&format!(
                "/v1/pm/acceptance-overview?project_id={unknown}"
            )))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // 跨项目任务 → 404；未知任务 → 404。
    for task_id in [hidden_task, Uuid::new_v4()] {
        for path in [
            format!("/v1/pm/tasks/{task_id}/review-findings"),
            format!("/v1/pm/tasks/{task_id}/criterion-verdicts"),
            format!("/v1/pm/tasks/{task_id}/review-approvals"),
        ] {
            let (status, _) = body_json(
                router(pool.clone())
                    .oneshot(get(&path))
                    .await
                    .unwrap(),
            )
            .await;
            assert_eq!(status, StatusCode::NOT_FOUND, "path: {path}");
        }
    }

    // 端点连通性再确认（可见任务 → 200）。
    let (status, _) = body_json(
        router(pool.clone())
            .oneshot(get(&format!(
                "/v1/pm/tasks/{blocked}/review-findings"
            )))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _ = project_id;
}
