//! D5 · Review / 失败判定 / 知识回写验收（`docs/slice-d/ai-pm-slice-d5-review-v1.md` §10。
//!
//! 用真实 git 工作区 + 真实 PostgreSQL：先经 D4 提交管线把任务推进到 `submitted`，
//! 再驱动 D5 的两拍编排（`enqueue_review` → `review_step`），断言 §5.1 矩阵出口。
//!
//! 覆盖映射（`todo.md` T-043 的两条闭环必验 + 关键不变量）：
//!
//! | 用例 | 断言 |
//! |---|---|
//! | C99  | 活跃 review run 不挡 delivery run；同一 task 第二个活跃 review run 被拒 |
//! | C100 | delivered → approved → completed（+ draft 提案 + 两事件） |
//! | C101 | 强制人工审批 → 停 `reviewing` + pending 审批行 + `task_waiting` |
//! | C102 | approve → completed；reject → changes_requested（`decision_reason` 转 finding） |
//! | C103 | `integration_status=failed` → changes_requested；人工 approve 被拒 |
//! | C104 | `not_run` + 自动化条目 → changes_requested（`evidence_missing`） |
//! | C105 | `not_run` + 无自动化条目 → inconclusive 降级人工（不当 approved） |
//! | C107 | `manual` 条目不预插裁决行，审批裁决事务内 INSERT 终值行 |
//! | C111 | review run 失败不改 delivery run，重试 run `run_kind='review'` |
//! | C114 | 失败判定作业对 review run 零命中、不建 delivery run |
//! | C119 | 审计链 `review.queued` / `review.completed` / `review.changes_requested` 齐备 |
//! | C123 | 不可变触发器拒绝 UPDATE（finding / verdict / review 报告列） |
//! | C124 | `enqueue_review` 重复调用幂等（单 run、单审计） |
//! | C131 | R1 单活跃 review 唯一索引 |
//! | C136 | review 恢复链零 delivery run 创建 |

use std::path::{Path, PathBuf};
use std::time::Duration;

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::delivery::{
    submit_delivery_with, DeliveryOptions, SubmitResultRequest, SubmittedRepository, SubmittedTest,
};
use pm_application::review::{
    decide_review_approval, enqueue_review, judge_delivery_failures, review_step, AdvanceOutcome,
    ApprovalOutcome, EnqueueOutcome, ReviewConfig, ReviewDecisionRequest, ReviewStep,
};
use pm_application::run_context::{report_progress, ProgressReport};
use pm_application::run_dispatch::{claim_task_run, workspace_path_for, RunDispatchConfig};
use sqlx::PgPool;
use uuid::Uuid;

use pm_domain::SYSTEM_TENANT;

// ---------------------------------------------------------------------------
// git 工作区夹具（与 D4 用例同款）
// ---------------------------------------------------------------------------

fn git(root: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .status()
        .expect("git must be installed for the D5 review tests");
    assert!(status.success(), "git {args:?} failed in {root:?}");
}

fn head_sha(root: &Path) -> String {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .expect("git rev-parse");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn init_workspace(base: &Path, run_id: Uuid, workspace_path: &str) -> (PathBuf, String) {
    let root = base.join(run_id.to_string()).join(workspace_path);
    std::fs::create_dir_all(&root).expect("create workspace");
    git(&root, &["init", "-q", "-b", "main"]);
    std::fs::write(root.join("README.md"), "# base\n").expect("write base file");
    git(&root, &["add", "."]);
    git(
        &root,
        &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "base"],
    );
    let sha = head_sha(&root);
    (root, sha)
}

fn commit_change(root: &Path, files: &[&str]) -> String {
    for file in files {
        std::fs::write(root.join(file), "changed\n").expect("write changed file");
    }
    git(root, &["add", "."]);
    git(
        root,
        &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "change"],
    );
    head_sha(root)
}

fn temp_base(label: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!("ai-pm-d5-{label}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&base).expect("create workspace base");
    base
}

// ---------------------------------------------------------------------------
// 数据库夹具
// ---------------------------------------------------------------------------

struct Fx {
    project_id: Uuid,
    task_id: Uuid,
    delivery_run_id: Uuid,
    repository_id: Uuid,
    base_sha: String,
    base: PathBuf,
    workspace: PathBuf,
    human_id: Uuid,
}

fn dispatch_cfg() -> RunDispatchConfig {
    RunDispatchConfig::from_env()
}

/// 建项目 + 任务 + 一个可写仓 + Run，推进到 `running` 并经 D4 提交交付。
///
/// `tests_passed` 决定测试证据是否全绿；`integration_status` 决定集成事实。
async fn seed_delivered(
    pool: &PgPool,
    code: &str,
    tests_passed: bool,
    integration_status: &str,
) -> Fx {
    let outcome = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let agent: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('dev_agent', 'dev') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, assignee_id, tenant_id) \
         VALUES ($1, 'review me', 'ready', 1, $2, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(outcome.project_id)
    .bind(agent)
    .fetch_one(pool)
    .await
    .unwrap();
    let delivery_run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 1, 'queued', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(outcome.project_id)
    .bind(task_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let base = temp_base(code);
    let repository_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.project_repository (project_id, name, url, status, head_commit_sha, tenant_id) \
         VALUES ($1, 'svc', $2, 'verified', 'headsha', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(outcome.project_id)
    .bind(format!("git@host:{code}-svc"))
    .fetch_one(pool)
    .await
    .unwrap();
    let workspace_path = workspace_path_for(repository_id, "svc");
    let (workspace, base_sha) = init_workspace(&base, delivery_run_id, &workspace_path);
    sqlx::query(
        "INSERT INTO delivery.task_repository \
         (task_id, project_id, repository_id, write_access, base_ref, base_commit, tenant_id) \
         VALUES ($1, $2, $3, true, 'main', $4, \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(task_id)
    .bind(outcome.project_id)
    .bind(repository_id)
    .bind(&base_sha)
    .execute(pool)
    .await
    .unwrap();

    // 领取 → started → running。
    let claimed = claim_task_run(
        pool,
        outcome.project_id,
        task_id,
        "codex",
        Some("1.2.0"),
        &dispatch_cfg(),
    )
    .await
    .unwrap();
    report_progress(
        pool,
        outcome.project_id,
        delivery_run_id,
        &claimed.lease_token,
        ProgressReport {
            stage: "started",
            note: None,
            blocked: false,
        },
        &dispatch_cfg(),
        claimed.lease_generation,
    )
    .await
    .unwrap();

    // D4 提交：落 commit 行 + 任务 submitted + DeliverySubmitted 事件。
    let commit_sha = commit_change(&workspace, &["work.txt"]);
    let request = SubmitResultRequest {
        project_id: outcome.project_id,
        run_id: delivery_run_id,
        lease_token: claimed.lease_token.clone(),
        lease_generation: claimed.lease_generation,
        integration_status: Some(integration_status.to_string()),
        repositories: vec![SubmittedRepository {
            repository_id,
            base_commit: base_sha.clone(),
            commit_sha,
            changed_files: 1,
            tests: vec![SubmittedTest {
                name: "cargo test".to_string(),
                passed: tests_passed,
                evidence_ref: Some("log://cargo-test".to_string()),
            }],
        }],
        summary: "done".to_string(),
        knowledge_proposals: vec![],
        caller: None,
    };
    let options = DeliveryOptions {
        workspace_base: base.clone(),
        git_timeout: Duration::from_secs(30),
    };
    submit_delivery_with(pool, &request, &options)
        .await
        .unwrap();

    Fx {
        project_id: outcome.project_id,
        task_id,
        delivery_run_id,
        repository_id,
        base_sha,
        base,
        workspace,
        human_id: outcome.human_id,
    }
}

async fn insert_criterion(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    ordinal: i32,
    kind: &str,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task_acceptance_criterion \
         (project_id, task_id, ordinal, text, verification_kind, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(task_id)
    .bind(ordinal)
    .bind(format!("criterion {ordinal}"))
    .bind(kind)
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

async fn delivery_run_status(pool: &PgPool, run_id: Uuid) -> (String, Option<String>) {
    sqlx::query_as("SELECT status, outcome FROM delivery.task_run WHERE id = $1")
        .bind(run_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn audit_count(pool: &PgPool, action: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM governance.audit_log WHERE action = $1")
        .bind(action)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn outbox() -> pm_application::outbox::PgOutboxAppend {
    pm_application::outbox::PgOutboxAppend
}

/// 让 D5 的领域通知事件在测试里可见。
///
/// T-061（P0-4）之后 `AI_PM_OUTBOX_EVENTS` **不能**突破事件目录的
/// `has_handler` 限制——`ReviewCompleted` / `KnowledgeBaseWriteRequested`
/// 没有消费 handler，因此**任何开关配置都无法让它们产生**（否则会产生
/// 悬空事件并被 Worker 死信）。这两个事件的断言已相应改为「不产生」。
fn enable_notification_events() {
    std::env::set_var(
        "AI_PM_OUTBOX_EVENTS",
        "TaskReady,DeliverySubmitted,ReviewCompleted,KnowledgeBaseWriteRequested",
    );
}

// ---------------------------------------------------------------------------
// C99 / C131：索引对偶
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c99_c131_active_review_does_not_block_delivery_and_is_unique_per_task(pool: PgPool) {
    let fx = seed_delivered(&pool, "D5-C99", true, "passed").await;
    // 原 delivery run 先落终态（否则它自己就是活跃 delivery run，新行本应被拒）。
    sqlx::query(
        "UPDATE delivery.task_run SET status = 'succeeded', outcome = 'succeeded' WHERE id = $1",
    )
    .bind(fx.delivery_run_id)
    .execute(&pool)
    .await
    .unwrap();
    let review_run: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run \
         (project_id, task_id, attempt_no, status, run_kind, role_id, executor_kind, \
          input_manifest_hash, reviewed_manifest_hash, review_policy_version, tenant_id) \
         VALUES ($1, $2, 1, 'queued', 'review', 'reviewer', 'platform', 'in-hash', 'rev-hash', 'pol', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(fx.project_id)
    .bind(fx.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 活跃 review run 存在时，delivery run 仍可创建（旧约束收窄的直接证据）。
    let second_delivery: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, run_kind, tenant_id) \
         VALUES ($1, $2, 2, 'queued', 'delivery', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(fx.project_id)
    .bind(fx.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_ne!(second_delivery, review_run);

    // 第二个活跃 review run 被 uq_task_review_active 拒绝（R1 / C131）。
    let error = sqlx::query(
        "INSERT INTO delivery.task_run \
         (project_id, task_id, attempt_no, status, run_kind, role_id, executor_kind, \
          input_manifest_hash, reviewed_manifest_hash, review_policy_version, tenant_id) \
         VALUES ($1, $2, 2, 'queued', 'review', 'reviewer', 'platform', 'i2', 'r2', 'pol', \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(fx.project_id)
    .bind(fx.task_id)
    .execute(&pool)
    .await
    .expect_err("a second active review run must be refused");
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("23505")
    );
}

// ---------------------------------------------------------------------------
// C100：自动闭环
// ---------------------------------------------------------------------------

/// 显式授权 auto_accept（DEF-P0-01 默认拒绝后，**自动闭环必须显式配置**）。
///
/// 能力类字段在**所有适用层**都须显式允许；此处任务无 task_type，适用层为
/// organization + project。
async fn allow_auto_accept(pool: &PgPool, project_id: Uuid, actor: Uuid) {
    let tenant_id: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await
        .unwrap();
    for (scope, key) in [
        ("organization", "*".to_string()),
        ("project", project_id.to_string()),
    ] {
        sqlx::query(
            "INSERT INTO delivery.policy_scope              (scope, scope_key, require_human_approval, auto_accept, auto_merge,               auto_publish_knowledge, updated_by, tenant_id)              VALUES ($1, $2, false, true, true, true, $3, $4)              ON CONFLICT (tenant_id, scope, scope_key) DO UPDATE SET                 auto_accept = true, auto_merge = true, auto_publish_knowledge = true",
        )
        .bind(scope)
        .bind(&key)
        .bind(actor)
        .bind(tenant_id)
        .execute(pool)
        .await
        .unwrap();
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn c100_delivered_auto_accept_completes_with_proposal_and_events(pool: PgPool) {
    enable_notification_events();
    let fx = seed_delivered(&pool, "D5-C100", true, "passed").await;
    insert_criterion(&pool, fx.project_id, fx.task_id, 10, "automated_test").await;
    // DEF-P0-01 默认拒绝：自动闭环需**显式授权**（无配置即不得自动放行）。
    allow_auto_accept(&pool, fx.project_id, fx.human_id).await;

    let queued = enqueue_review(&pool, fx.project_id, fx.task_id)
        .await
        .unwrap();
    let EnqueueOutcome::Queued {
        review_run_id,
        already_queued,
        ..
    } = queued
    else {
        panic!("delivered delivery must enqueue a review run, got {queued:?}");
    };
    assert!(!already_queued);
    assert_eq!(task_status(&pool, fx.task_id).await, "reviewing");

    let step = review_step(&pool, &outbox(), &ReviewConfig::from_env())
        .await
        .unwrap();
    let ReviewStep::Reviewed {
        verdict, advance, ..
    } = step
    else {
        panic!("the queued review run must be executed, got {step:?}");
    };
    assert_eq!(verdict, "approved");
    let AdvanceOutcome::Completed { proposal_id } = advance else {
        panic!("auto_accept must complete the task, got {advance:?}");
    };
    let proposal_id =
        proposal_id.expect("a draft proposal must be created in the same transaction");

    assert_eq!(task_status(&pool, fx.task_id).await, "completed");
    let (verdict_col, run_status): (Option<String>, String) =
        sqlx::query_as("SELECT review_verdict, status FROM delivery.task_run WHERE id = $1")
            .bind(review_run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(verdict_col.as_deref(), Some("approved"));
    assert_eq!(run_status, "succeeded");

    // 提案：`draft` 初始态，与 completed 同事务落定。
    let (doc_type, status, doc_key): (String, String, String) =
        sqlx::query_as("SELECT doc_type, status, doc_key FROM ai.document_proposal WHERE id = $1")
            .bind(proposal_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(doc_type, "knowledge");
    assert_eq!(status, "draft");
    assert_eq!(doc_key, format!("task-{}", fx.task_id));

    // D5 的两个领域通知事件**不产生**：T-061（P0-4）之后事件目录是单一真源，
    // 无消费 handler 的事件类型**禁止生产**——`AI_PM_OUTBOX_EVENTS` 也不能突破。
    // 否则会产生悬空事件，随后被 Worker 死信（假死信污染运维面板）。
    let events: Vec<String> = sqlx::query_scalar(
        "SELECT event_type FROM integration.outbox_event WHERE project_id = $1 ORDER BY event_type",
    )
    .bind(fx.project_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(
        !events.iter().any(|e| e == "ReviewCompleted"),
        "an event without a handler must not be produced: {events:?}"
    );
    assert!(
        !events.iter().any(|e| e == "KnowledgeBaseWriteRequested"),
        "an event without a handler must not be produced: {events:?}"
    );
    assert_eq!(audit_count(&pool, "review.completed").await, 1);
}

// ---------------------------------------------------------------------------
// C102：默认拒绝（DEF-P0-01）——无任何策略授权时不得自动放行
// ---------------------------------------------------------------------------

/// 未显式授权 auto_accept（无任何 policy_scope 行）→ 交付评审**不得**自动完成，
/// 必须停在人工审批门（auto_accept_gate）。这是 DEF-P0-01 在业务层的断言：
/// 「没有明确授权 = 不允许」。
#[sqlx::test(migrations = "../../migrations")]
async fn c102_default_deny_stops_at_approval_gate_without_explicit_authorization(pool: PgPool) {
    let fx = seed_delivered(&pool, "D5-C102", true, "passed").await;
    insert_criterion(&pool, fx.project_id, fx.task_id, 10, "automated_test").await;

    // **不**调用 allow_auto_accept —— 没有任何显式授权。
    enqueue_review(&pool, fx.project_id, fx.task_id)
        .await
        .unwrap();
    let step = review_step(&pool, &outbox(), &ReviewConfig::from_env())
        .await
        .unwrap();
    let ReviewStep::Reviewed {
        verdict, advance, ..
    } = step
    else {
        panic!("expected a reviewed step, got {step:?}");
    };
    assert_eq!(verdict, "approved");
    let AdvanceOutcome::AwaitingApproval { approval_id, kind } = advance else {
        panic!("default deny must stop at the approval gate, got {advance:?}");
    };
    assert_eq!(kind, "auto_accept_gate");

    // 任务停在 reviewing（未完成）+ 落 pending 审批行。
    assert_eq!(task_status(&pool, fx.task_id).await, "reviewing");
    let status: String =
        sqlx::query_scalar("SELECT status FROM governance.review_approval WHERE id = $1")
            .bind(approval_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "pending");
}

// ---------------------------------------------------------------------------
// C101：强制人工审批
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c101_require_human_approval_stops_at_a_pending_approval(pool: PgPool) {
    let fx = seed_delivered(&pool, "D5-C101", true, "passed").await;
    insert_criterion(&pool, fx.project_id, fx.task_id, 10, "automated_test").await;
    sqlx::query(
        "INSERT INTO delivery.policy_scope \
         (scope, scope_key, require_human_approval, auto_accept, updated_by, tenant_id) \
         VALUES ('organization', '*', true, true, $1, $2)",
    )
    .bind(fx.human_id)
    .bind(
        sqlx::query_scalar::<_, Uuid>("SELECT tenant_id FROM core.project WHERE id = $1")
            .bind(fx.project_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
    )
    .execute(&pool)
    .await
    .unwrap();

    enqueue_review(&pool, fx.project_id, fx.task_id)
        .await
        .unwrap();
    let step = review_step(&pool, &outbox(), &ReviewConfig::from_env())
        .await
        .unwrap();
    let ReviewStep::Reviewed {
        verdict, advance, ..
    } = step
    else {
        panic!("expected a reviewed step, got {step:?}");
    };
    assert_eq!(verdict, "approved");
    let AdvanceOutcome::AwaitingApproval { approval_id, kind } = advance else {
        panic!("a forced approval must wait, got {advance:?}");
    };
    assert_eq!(kind, "auto_accept_gate");

    // 停 `reviewing`（不完成）+ pending 审批行 + task_waiting（同事务）。
    assert_eq!(task_status(&pool, fx.task_id).await, "reviewing");
    let (status, delivery_run_id, hash): (String, Uuid, String) = sqlx::query_as(
        "SELECT status, delivery_run_id, delivery_manifest_hash \
           FROM governance.review_approval WHERE id = $1",
    )
    .bind(approval_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "pending");
    assert_eq!(delivery_run_id, fx.delivery_run_id);
    assert!(!hash.is_empty());
    let waiting: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_waiting WHERE task_id = $1 AND ended_at IS NULL",
    )
    .bind(fx.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(waiting, 1);
    let proposals: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ai.document_proposal WHERE project_id = $1")
            .bind(fx.project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(proposals, 0, "no proposal before completion");
}

// ---------------------------------------------------------------------------
// C102：审批裁决单事务（通过 / 驳回）
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c102_approve_completes_and_reject_requests_changes(pool: PgPool) {
    // ── approve ──────────────────────────────────────────────────────
    let fx = seed_delivered(&pool, "D5-C102A", true, "passed").await;
    insert_criterion(&pool, fx.project_id, fx.task_id, 10, "manual").await;
    enqueue_review(&pool, fx.project_id, fx.task_id)
        .await
        .unwrap();
    let step = review_step(&pool, &outbox(), &ReviewConfig::from_env())
        .await
        .unwrap();
    let ReviewStep::Reviewed {
        advance: AdvanceOutcome::AwaitingApproval { approval_id, .. },
        ..
    } = step
    else {
        panic!("a manual criterion must wait for approval, got {step:?}");
    };
    let decision = decide_review_approval(
        &pool,
        &outbox(),
        &ReviewDecisionRequest {
            project_id: fx.project_id,
            approval_id,
            decision: "approved".to_string(),
            decided_by: fx.human_id,
            decision_reason: Some("looks good".to_string()),
        },
    )
    .await
    .unwrap();
    let ApprovalOutcome::Completed { proposal_id, .. } = decision else {
        panic!("approve must complete, got {decision:?}");
    };
    assert!(proposal_id.is_some());
    assert_eq!(task_status(&pool, fx.task_id).await, "completed");
    let decided: (String, Option<Uuid>, bool) = sqlx::query_as(
        "SELECT status, decided_by, decided_at IS NOT NULL FROM governance.review_approval WHERE id = $1",
    )
    .bind(approval_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(decided.0, "approved");
    assert_eq!(decided.1, Some(fx.human_id));
    assert!(decided.2);
    // 人工裁决的终值行在裁决事务内 INSERT（`manual` 条目不预插）。
    let manual_verdict: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.acceptance_criterion_verdict WHERE verdict = 'passed' \
           AND decided_by = $1",
    )
    .bind(fx.human_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(manual_verdict, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM delivery.task_waiting WHERE task_id = $1 AND ended_at IS NULL"
        )
        .bind(fx.task_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0,
        "the waiting row must be closed by the decision"
    );

    // ── reject ───────────────────────────────────────────────────────
    let fx2 = seed_delivered(&pool, "D5-C102B", true, "passed").await;
    insert_criterion(&pool, fx2.project_id, fx2.task_id, 10, "manual").await;
    enqueue_review(&pool, fx2.project_id, fx2.task_id)
        .await
        .unwrap();
    let step = review_step(&pool, &outbox(), &ReviewConfig::from_env())
        .await
        .unwrap();
    let ReviewStep::Reviewed {
        advance: AdvanceOutcome::AwaitingApproval { approval_id, .. },
        ..
    } = step
    else {
        panic!("expected waiting approval, got {step:?}");
    };
    let decision = decide_review_approval(
        &pool,
        &outbox(),
        &ReviewDecisionRequest {
            project_id: fx2.project_id,
            approval_id,
            decision: "rejected".to_string(),
            decided_by: fx2.human_id,
            decision_reason: Some("missing docs".to_string()),
        },
    )
    .await
    .unwrap();
    assert!(matches!(decision, ApprovalOutcome::ChangesRequested { .. }));
    assert_eq!(task_status(&pool, fx2.task_id).await, "changes_requested");
    let findings: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.review_finding WHERE finding_kind = 'approval_rejected'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(findings, 1, "the rejection reason must become a finding");
}

// ---------------------------------------------------------------------------
// C103 / C104 / C105：verdict 来源行
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c103_failed_integration_is_changes_requested_and_human_approve_is_refused(pool: PgPool) {
    let fx = seed_delivered(&pool, "D5-C103", true, "failed").await;
    insert_criterion(&pool, fx.project_id, fx.task_id, 10, "automated_test").await;
    enqueue_review(&pool, fx.project_id, fx.task_id)
        .await
        .unwrap();
    let step = review_step(&pool, &outbox(), &ReviewConfig::from_env())
        .await
        .unwrap();
    let ReviewStep::Reviewed {
        verdict, advance, ..
    } = step
    else {
        panic!("expected reviewed, got {step:?}");
    };
    assert_eq!(verdict, "changes_requested");
    assert!(matches!(advance, AdvanceOutcome::ChangesRequested));
    assert_eq!(task_status(&pool, fx.task_id).await, "changes_requested");

    // 人工通道也不可越（硬禁令）：审批行不存在 → 直接构造一条 pending 审批行并尝试 approve。
    let review_run_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM delivery.task_run WHERE task_id = $1 AND run_kind = 'review'",
    )
    .bind(fx.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let approval_id: Uuid = sqlx::query_scalar(
        "INSERT INTO governance.review_approval \
         (project_id, task_id, review_run_id, delivery_run_id, delivery_manifest_hash, kind, \
          status, tenant_id) \
         SELECT project_id, task_id, id, $3, reviewed_manifest_hash, 'manual_review', 'pending', tenant_id \
           FROM delivery.task_run WHERE id = $2 RETURNING id",
    )
    .bind(fx.project_id)
    .bind(review_run_id)
    .bind(fx.delivery_run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let error = decide_review_approval(
        &pool,
        &outbox(),
        &ReviewDecisionRequest {
            project_id: fx.project_id,
            approval_id,
            decision: "approved".to_string(),
            decided_by: fx.human_id,
            decision_reason: None,
        },
    )
    .await
    .expect_err("integration_status=failed must refuse approval");
    assert_eq!(error.code(), "completion_gate_refused");
    assert_eq!(task_status(&pool, fx.task_id).await, "changes_requested");
}

#[sqlx::test(migrations = "../../migrations")]
async fn c104_not_run_with_automated_criterion_requests_changes(pool: PgPool) {
    let fx = seed_delivered(&pool, "D5-C104", true, "not_run").await;
    insert_criterion(&pool, fx.project_id, fx.task_id, 10, "automated_test").await;
    enqueue_review(&pool, fx.project_id, fx.task_id)
        .await
        .unwrap();
    let step = review_step(&pool, &outbox(), &ReviewConfig::from_env())
        .await
        .unwrap();
    let ReviewStep::Reviewed { verdict, .. } = step else {
        panic!("expected reviewed, got {step:?}");
    };
    assert_eq!(verdict, "changes_requested");
    assert_eq!(task_status(&pool, fx.task_id).await, "changes_requested");
    let finding: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.review_finding WHERE finding_kind = 'evidence_missing'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(finding >= 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn c105_not_run_without_automated_criterion_is_inconclusive_and_waits(pool: PgPool) {
    let fx = seed_delivered(&pool, "D5-C105", true, "not_run").await;
    insert_criterion(&pool, fx.project_id, fx.task_id, 10, "manual").await;
    enqueue_review(&pool, fx.project_id, fx.task_id)
        .await
        .unwrap();
    let step = review_step(&pool, &outbox(), &ReviewConfig::from_env())
        .await
        .unwrap();
    let ReviewStep::Reviewed {
        verdict, advance, ..
    } = step
    else {
        panic!("expected reviewed, got {step:?}");
    };
    assert_eq!(verdict, "inconclusive");
    let AdvanceOutcome::AwaitingApproval { kind, .. } = advance else {
        panic!("inconclusive must degrade to a manual approval, got {advance:?}");
    };
    assert_eq!(kind, "manual_review");
    assert_eq!(task_status(&pool, fx.task_id).await, "reviewing");
    // Inconclusive 不当 approved（§5.1）。
    let stored: Option<String> = sqlx::query_scalar(
        "SELECT review_verdict FROM delivery.task_run WHERE task_id = $1 AND run_kind = 'review'",
    )
    .bind(fx.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored.as_deref(), Some("inconclusive"));
}

// ---------------------------------------------------------------------------
// C107：manual 条目不预插裁决行
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c107_manual_criteria_get_no_pipeline_verdict_row(pool: PgPool) {
    let fx = seed_delivered(&pool, "D5-C107", true, "passed").await;
    let automated = insert_criterion(&pool, fx.project_id, fx.task_id, 10, "automated_test").await;
    let manual = insert_criterion(&pool, fx.project_id, fx.task_id, 20, "manual").await;
    enqueue_review(&pool, fx.project_id, fx.task_id)
        .await
        .unwrap();
    review_step(&pool, &outbox(), &ReviewConfig::from_env())
        .await
        .unwrap();

    let rows: Vec<Uuid> = sqlx::query_scalar(
        "SELECT criterion_id FROM delivery.acceptance_criterion_verdict WHERE review_run_id IN \
           (SELECT id FROM delivery.task_run WHERE task_id = $1 AND run_kind = 'review')",
    )
    .bind(fx.task_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(
        rows.contains(&automated),
        "deterministic criteria land a row"
    );
    assert!(
        !rows.contains(&manual),
        "manual criteria must not pre-insert verdicts"
    );
    // finding 仍然锚定 manual 条目（待人工可见）。
    let anchored: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.review_finding WHERE criterion_id = $1 AND finding_kind = 'awaiting_manual'",
    )
    .bind(manual)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(anchored, 1);
}

// ---------------------------------------------------------------------------
// C111 / C136：review 失败隔离
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c111_c136_review_failure_touches_neither_delivery_run_nor_creates_one(pool: PgPool) {
    let fx = seed_delivered(&pool, "D5-C111", true, "passed").await;
    insert_criterion(&pool, fx.project_id, fx.task_id, 10, "automated_test").await;
    enqueue_review(&pool, fx.project_id, fx.task_id)
        .await
        .unwrap();

    // 构造锚定漂移（§3.3 / T-052 场景）：拍①之后新增一条验收条目 → 输入锚定
    // 与 run 行记录的哈希不再一致 → verdict 落库前复核失败。
    insert_criterion(&pool, fx.project_id, fx.task_id, 20, "manual").await;

    let step = review_step(&pool, &outbox(), &ReviewConfig::from_env())
        .await
        .unwrap();
    let ReviewStep::Failed {
        run_id,
        retry_run_id,
    } = step
    else {
        panic!("anchor drift must fail the review run, got {step:?}");
    };
    assert!(
        retry_run_id.is_some(),
        "a retry review run must be scheduled"
    );

    let failed_status: String =
        sqlx::query_scalar("SELECT status FROM delivery.task_run WHERE id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(failed_status, "failed");

    // 重试 run 的 run_kind 恒为 review；delivery 行数不增加（C136）。
    let retry_kind: String =
        sqlx::query_scalar("SELECT run_kind FROM delivery.task_run WHERE id = $1")
            .bind(retry_run_id.unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(retry_kind, "review");
    let delivery_runs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_run WHERE task_id = $1 AND run_kind = 'delivery'",
    )
    .bind(fx.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        delivery_runs, 1,
        "review recovery must never create a delivery run"
    );

    // delivery run status/outcome 与任务状态全程不变（C111）。
    let (status, outcome) = delivery_run_status(&pool, fx.delivery_run_id).await;
    assert_eq!(
        status, "running",
        "the delivery run is untouched by the review failure"
    );
    assert_eq!(outcome, None);
    assert_eq!(task_status(&pool, fx.task_id).await, "reviewing");
    assert_eq!(
        sqlx::query_scalar::<_, i32>(
            "SELECT consecutive_failure_count FROM delivery.task WHERE id = $1"
        )
        .bind(fx.task_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0,
        "review failure must not count as an execution failure (R7)"
    );
}

// ---------------------------------------------------------------------------
// C114：失败判定作业对 review run 零命中
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c114_failure_judge_never_touches_review_runs(pool: PgPool) {
    let fx = seed_delivered(&pool, "D5-C114", true, "passed").await;
    // 任务回到 running，并造一条终态 review run（failed）。
    sqlx::query("UPDATE delivery.task SET status = 'running' WHERE id = $1")
        .bind(fx.task_id)
        .execute(&pool)
        .await
        .unwrap();
    let review_run: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run \
         (project_id, task_id, attempt_no, status, outcome, run_kind, role_id, executor_kind, \
          input_manifest_hash, reviewed_manifest_hash, review_policy_version, finished_at, tenant_id) \
         VALUES ($1, $2, 1, 'failed', 'failed', 'review', 'reviewer', 'platform', \
                 'i', 'r', 'pol', now() - make_interval(secs => 60), \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(fx.project_id)
    .bind(fx.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let report = judge_delivery_failures(&pool, &dispatch_cfg())
        .await
        .unwrap();
    assert_eq!(
        report.judged, 0,
        "the scan must not hit review runs (structural predicate)"
    );
    assert_eq!(report.retries, 0);
    assert_eq!(report.task_failures, 0);
    let delivery_runs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_run WHERE task_id = $1 AND run_kind = 'delivery'",
    )
    .bind(fx.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        delivery_runs, 1,
        "no delivery run may be created from a review failure"
    );
    let untouched: String =
        sqlx::query_scalar("SELECT status FROM delivery.task_run WHERE id = $1")
            .bind(review_run)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(untouched, "failed");
}

// ---------------------------------------------------------------------------
// C119 / C124：审计链与幂等
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c119_c124_audit_chain_and_idempotent_enqueue(pool: PgPool) {
    let fx = seed_delivered(&pool, "D5-C119", true, "passed").await;
    insert_criterion(&pool, fx.project_id, fx.task_id, 10, "automated_test").await;

    let first = enqueue_review(&pool, fx.project_id, fx.task_id)
        .await
        .unwrap();
    let EnqueueOutcome::Queued { review_run_id, .. } = first else {
        panic!("expected queued, got {first:?}");
    };
    let second = enqueue_review(&pool, fx.project_id, fx.task_id)
        .await
        .unwrap();
    let EnqueueOutcome::Queued {
        review_run_id: again,
        already_queued,
        ..
    } = second
    else {
        panic!("a repeat must be idempotent, got {second:?}");
    };
    assert!(already_queued);
    assert_eq!(again, review_run_id);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM delivery.task_run WHERE task_id = $1 AND run_kind = 'review'"
        )
        .bind(fx.task_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(audit_count(&pool, "review.queued").await, 1);

    review_step(&pool, &outbox(), &ReviewConfig::from_env())
        .await
        .unwrap();
    assert_eq!(audit_count(&pool, "review.queued").await, 1);
    assert_eq!(audit_count(&pool, "review.completed").await, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn c119_changes_requested_audit_is_distinct(pool: PgPool) {
    // `integration_status=failed` 才是 changes_requested 的来源（L2 硬禁令）；
    // 「passed 但 tests 有失败」属 L4 矛盾 → inconclusive（C106），不是本用例。
    let fx = seed_delivered(&pool, "D5-C119B", true, "failed").await;
    insert_criterion(&pool, fx.project_id, fx.task_id, 10, "automated_test").await;
    enqueue_review(&pool, fx.project_id, fx.task_id)
        .await
        .unwrap();
    review_step(&pool, &outbox(), &ReviewConfig::from_env())
        .await
        .unwrap();
    assert_eq!(audit_count(&pool, "review.changes_requested").await, 1);
    assert_eq!(audit_count(&pool, "review.completed").await, 0);
    assert_eq!(task_status(&pool, fx.task_id).await, "changes_requested");
}

// ---------------------------------------------------------------------------
// C106：声明与证据矛盾 → inconclusive
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c106_passed_with_a_failed_test_is_inconclusive(pool: PgPool) {
    let fx = seed_delivered(&pool, "D5-C106", false, "passed").await;
    insert_criterion(&pool, fx.project_id, fx.task_id, 10, "automated_test").await;
    enqueue_review(&pool, fx.project_id, fx.task_id)
        .await
        .unwrap();
    let step = review_step(&pool, &outbox(), &ReviewConfig::from_env())
        .await
        .unwrap();
    let ReviewStep::Reviewed { verdict, .. } = step else {
        panic!("expected reviewed, got {step:?}");
    };
    assert_eq!(verdict, "inconclusive");
    let conflict: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.review_finding WHERE finding_kind = 'evidence_conflict'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(conflict, 1);
    assert_eq!(task_status(&pool, fx.task_id).await, "reviewing");
}

// ---------------------------------------------------------------------------
// C123：不可变触发器
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../migrations")]
async fn c123_immutability_triggers_reject_updates(pool: PgPool) {
    // 用会产出 finding 的夹具：`integration_status=failed` → blocker finding 必须存在
    // （否则「UPDATE 0 行也返回 Ok」会让 append-only 断言假通过）。
    let fx = seed_delivered(&pool, "D5-C123", true, "failed").await;
    insert_criterion(&pool, fx.project_id, fx.task_id, 10, "automated_test").await;
    enqueue_review(&pool, fx.project_id, fx.task_id)
        .await
        .unwrap();
    review_step(&pool, &outbox(), &ReviewConfig::from_env())
        .await
        .unwrap();

    let review_run: Uuid = sqlx::query_scalar(
        "SELECT id FROM delivery.task_run WHERE task_id = $1 AND run_kind = 'review'",
    )
    .bind(fx.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // review 报告列 once-written（verdict 非 NULL 后禁改）。
    let verdict_update =
        sqlx::query("UPDATE delivery.task_run SET review_verdict = 'inconclusive' WHERE id = $1")
            .bind(review_run)
            .execute(&pool)
            .await;
    assert!(
        verdict_update.is_err(),
        "a decided verdict must be immutable"
    );

    let role_update = sqlx::query("UPDATE delivery.task_run SET role_id = 'other' WHERE id = $1")
        .bind(review_run)
        .execute(&pool)
        .await;
    assert!(
        role_update.is_err(),
        "role_id is once-written on review runs"
    );

    // finding / verdict 全表禁 UPDATE。
    let finding_update = sqlx::query(
        "UPDATE delivery.review_finding SET message = 'rewritten' WHERE review_run_id = $1",
    )
    .bind(review_run)
    .execute(&pool)
    .await;
    assert!(finding_update.is_err(), "review findings are append-only");
    let verdict_row = sqlx::query(
        "UPDATE delivery.acceptance_criterion_verdict SET verdict = 'failed' WHERE review_run_id = $1",
    )
    .bind(review_run)
    .execute(&pool)
    .await;
    assert!(verdict_row.is_err(), "criterion verdicts are append-only");

    // delivery 行的常规更新不受影响（触发器按 OLD.run_kind 判定）。
    let delivery_update =
        sqlx::query("UPDATE delivery.task_run SET integration_status = 'passed' WHERE id = $1")
            .bind(fx.delivery_run_id)
            .execute(&pool)
            .await;
    assert!(
        delivery_update.is_ok(),
        "delivery rows keep their D3/D4 update paths"
    );
    let _ = (&fx.repository_id, &fx.base_sha, &fx.base, &fx.workspace);
}
