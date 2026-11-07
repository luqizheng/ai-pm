//! Checkpoint-1 · 仓库快照全链路集成测试（verify 触发链 → KB 分析 → 认知信号 →
//! RT 路由 → PC 结构化派生 → 父 Intake 收口 + 幂等矩阵）。
//!
//! 驱动方式与 intake_flows.rs 同款：服务层直调（`verify_repository` /
//! `pm_application::intake`）+ `register_all_handlers` 装配的 OutboxWorker
//! 逐事件 `dispatch_once` 消费——真实 DB + 真实本地 git 仓库（`file://` 夹具，
//! ls-remote/clone 匿名可达；HTTP 面另见 apps/pm-api/tests/intake.rs）。
//!
//! | 用例 | 断言焦点 |
//! |---|---|
//! | 1 | verify（HEAD 前进）自动受理 intake → kb 修订落库 → 信号 → pc 5 对象 draft → 父 COMPLETED |
//! | 2 | 同 HEAD 重复 verify：intake 重放零增量（repo_verify 幂等键） |
//! | 3 | 新 commit 再 verify：kb revision_no=2；pc 对象身份稳定、内容寻址去重（tech_stack 不追加修订）；evidence 追加 |

use std::path::PathBuf;
use std::process::Command;

use sqlx::{PgPool, Row};
use uuid::Uuid;

use pm_application::actor::AuditActor;
use pm_application::repository::verify_repository;
use pm_domain::SYSTEM_TENANT;
use pm_infrastructure::outbox_handlers::register_all_handlers;
use pm_infrastructure::outbox_worker::{DispatchOutcome, OutboxWorker};

const SEED_PRINCIPAL: Uuid = Uuid::from_u128(0x1111_1111_1111_1111_1111_1111_1111_1111);

// ── 夹具：真实本地 git 仓库 ─────────────────────────────────────────────────

struct LocalRepo {
    root: PathBuf,
    url: String,
}

impl Drop for LocalRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn git_run(path: &PathBuf, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(path)
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.com")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.com")
        .status()
        .expect("git must be on PATH");
    assert!(status.success(), "git {args:?} failed");
}

fn init_repo(initial_subject: &str) -> LocalRepo {
    let root = std::env::temp_dir().join(format!("ai-pm-kbpc-e2e-{}", Uuid::new_v4()));
    let path = root.join("repo");
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(
        path.join("README.md"),
        "# demo repo\n\ncheckpoint fixture\n",
    )
    .unwrap();
    std::fs::create_dir_all(path.join("docs")).unwrap();
    std::fs::write(path.join("docs/design.md"), "# Design\n\nfixture doc\n").unwrap();
    git_run(&path, &["init", "-q", "-b", "main"]);
    git_run(&path, &["add", "."]);
    git_run(
        &path,
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            initial_subject,
        ],
    );
    let url = format!("file://{}", path.to_string_lossy().replace('\\', "/"));
    LocalRepo { root, url }
}

fn append_commit(repo: &LocalRepo, subject: &str) {
    let path = repo.root.join("repo");
    std::fs::write(
        path.join("docs/design.md"),
        format!("# Design\n\nupdated: {subject}\n"),
    )
    .unwrap();
    git_run(&path, &["add", "."]);
    git_run(
        &path,
        &["-c", "commit.gpgsign=false", "commit", "-q", "-m", subject],
    );
}

// ── 种子 ────────────────────────────────────────────────────────────────────

async fn seed_principal(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES ($1, 'human', 'seed', true) ON CONFLICT (id) DO NOTHING",
    )
    .bind(SEED_PRINCIPAL)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_project(pool: &PgPool, code: &str) -> Uuid {
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(format!("c-{code}"))
    .bind(SYSTEM_TENANT)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, $2, $3) RETURNING id",
    )
    .bind(customer)
    .bind(code)
    .bind(SYSTEM_TENANT)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 直插仓库行（`file://` 绕过 REST URL 白名单——与 imports_e2e 同款偏差）。
async fn seed_repository(pool: &PgPool, project_id: Uuid, url: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO integration.project_repository \
         (project_id, name, url, role, default_branch, tenant_id) \
         VALUES ($1, 'fixture', $2, 'main', 'main', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(project_id)
    .bind(url)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn verify(pool: &PgPool, repository_id: Uuid) -> pm_application::repository::VerifyOutcome {
    verify_repository(pool, repository_id, &AuditActor::principal(SEED_PRINCIPAL))
        .await
        .unwrap()
}

fn worker(pool: &PgPool) -> OutboxWorker {
    register_all_handlers(
        OutboxWorker::new(pool.clone(), "kbpc-test-worker".to_string()),
        pool,
    )
}

/// 驱动 worker 消费至空闲（上限 80 步防死循环）。
async fn drain(worker: &OutboxWorker) {
    for _ in 0..80 {
        match worker.dispatch_once().await.unwrap() {
            DispatchOutcome::Idle => return,
            _ => continue,
        }
    }
    panic!("outbox queue did not drain within 80 dispatches (handler loop?)");
}

async fn latest_snapshot_intake(pool: &PgPool, repository_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "SELECT id FROM integration.intake_record \
          WHERE idempotency_key LIKE 'repo_verify:' || $1 || ':%' \
          ORDER BY created_at DESC LIMIT 1",
    )
    .bind(repository_id.to_string())
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn task_of_type(
    pool: &PgPool,
    intake_id: Uuid,
    task_type: &str,
) -> (Uuid, String, Option<serde_json::Value>) {
    sqlx::query_as(
        "SELECT id, task_status, result_ref FROM integration.intake_task \
          WHERE intake_id = $1 AND task_type = $2 ORDER BY created_at LIMIT 1",
    )
    .bind(intake_id)
    .bind(task_type)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn derived_status(pool: &PgPool, intake_id: Uuid) -> String {
    let tasks: Vec<(String, String)> =
        sqlx::query("SELECT phase, task_status FROM integration.intake_task WHERE intake_id = $1")
            .bind(intake_id)
            .fetch_all(pool)
            .await
            .unwrap()
            .into_iter()
            .map(|r| {
                (
                    r.get::<String, _>("phase"),
                    r.get::<String, _>("task_status"),
                )
            })
            .collect();
    format!(
        "{:?}",
        pm_application::intake::aggregate_task_status(&tasks)
    )
}

// ── 用例 1：verify → intake → KB 修订 → 信号 → PC 派生 → 父 COMPLETED ───────

#[sqlx::test(migrations = "../../migrations")]
async fn verify_triggers_full_kb_pc_chain(pool: PgPool) {
    seed_principal(&pool).await;
    let project = seed_project(&pool, "CP1-A").await;
    let repo = init_repo("feat: initial");
    let repository_id = seed_repository(&pool, project, &repo.url).await;

    let outcome = verify(&pool, repository_id).await;
    assert_eq!(outcome.status, "verified");
    let head = outcome
        .head_commit_sha
        .clone()
        .expect("verified repo has head");

    // 触发链：intake 自动受理（engineering_artifact/system，幂等键锚 repo+head）。
    let intake_id = latest_snapshot_intake(&pool, repository_id).await;
    let (intake_type, channel, key, content_hash): (String, String, String, Option<String>) =
        sqlx::query_as(
            "SELECT intake_type, channel, idempotency_key, content_hash \
              FROM integration.intake_record WHERE id = $1",
        )
        .bind(intake_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(intake_type, "engineering_artifact");
    assert_eq!(channel, "system");
    assert_eq!(key, format!("repo_verify:{repository_id}:{head}"));
    assert_eq!(content_hash.as_deref(), Some(head.as_str()));

    drain(&worker(&pool)).await;

    // KB：修订落库（draft、锚定 HEAD、digest 正文）。
    let (_, kb_status, kb_result) = task_of_type(&pool, intake_id, "kb_analyze").await;
    assert_eq!(kb_status, "COMPLETED");
    let kb_result = kb_result.expect("real analysis pins document ref");
    assert_eq!(kb_result["target_domain"], "knowledge_base");
    assert_eq!(kb_result["target_type"], "document");
    let revision_id: Uuid =
        Uuid::parse_str(kb_result["target_revision_id"].as_str().unwrap()).unwrap();
    let (revision_no, rev_status, rev_sha, _rev_hash): (i32, String, Option<String>, String) =
        sqlx::query_as(
            "SELECT revision_no, status, source_commit_sha, content_hash \
              FROM kb.document_revision WHERE id = $1",
        )
        .bind(revision_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((revision_no, rev_status.as_str()), (1, "draft"));
    assert_eq!(rev_sha.as_deref(), Some(head.as_str()));
    let content: String =
        sqlx::query_scalar("SELECT content FROM kb.document_revision WHERE id = $1")
            .bind(revision_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(content.contains("# 仓库快照"));
    assert!(content.contains("近期交付"));

    // PC：RT 路由建任务 → 结构化派生 → PROPOSED 完成。
    let (_, pc_status, pc_result) = task_of_type(&pool, intake_id, "pc_analyze").await;
    assert_eq!(pc_status, "COMPLETED");
    let pc_result = pc_result.expect("real derivation pins cognition object");
    assert_eq!(pc_result["target_domain"], "pc");
    assert_eq!(pc_result["target_type"], "cognition_object");
    assert_eq!(derived_status(&pool, intake_id).await, "Completed");

    // 信号面：pc_analyze.signal_ref 记录真实信号（四元组引用，无 service 字段）。
    let signal_ref: serde_json::Value = sqlx::query_scalar(
        "SELECT signal_ref FROM integration.intake_task \
          WHERE intake_id = $1 AND task_type = 'pc_analyze' LIMIT 1",
    )
    .bind(intake_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let signals = signal_ref
        .get("signals")
        .and_then(|v| v.as_array())
        .unwrap();
    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0]["relevance"], "PRODUCT_CANDIDATE");
    assert_eq!(signals[0]["confidence"], "high");
    assert!(
        !signal_ref.to_string().contains("service"),
        "ADR-07: no service field"
    );

    // pc.cognition_object：snapshot + tech_stack + activity + 2 modules = 5（全 draft）。
    let objects: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT object_type, object_key, lifecycle_status FROM pc.cognition_object \
          WHERE project_id = $1 ORDER BY created_at, object_key",
    )
    .bind(project)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(objects.len(), 5, "snapshot+tech_stack+activity+2 modules");
    assert!(objects.iter().all(|(_, _, s)| s == "draft"));
    assert!(objects.iter().any(|(t, _, _)| t == "repository"));
    assert!(objects.iter().any(|(t, _, _)| t == "tech_stack"));
    assert!(objects.iter().any(|(t, _, _)| t == "delivery_activity"));
    assert_eq!(
        objects.iter().filter(|(t, _, _)| *t == "module").count(),
        2,
        "README.md + docs/design.md hot files"
    );

    // evidence：unverified/medium，locator 锚定 intake/task/HEAD。
    let evidence: Vec<(String, String, serde_json::Value)> = sqlx::query_as(
        "SELECT verification_status, confidence, locator FROM pc.evidence \
          WHERE project_id = $1",
    )
    .bind(project)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(evidence.len(), 5);
    assert!(evidence
        .iter()
        .all(|(v, c, _)| v == "unverified" && c == "medium"));
    assert!(evidence
        .iter()
        .all(|(_, _, l)| l.get("intake_id").and_then(|v| v.as_str())
            == Some(intake_id.to_string().as_str())));

    // source：四元组结构化列指向 KB 文档修订。
    let source: (String, String, String, Option<String>) = sqlx::query_as(
        "SELECT target_domain, target_type, target_id, target_revision_id \
          FROM pc.source WHERE project_id = $1 LIMIT 1",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (source.0.as_str(), source.1.as_str()),
        ("knowledge_base", "document")
    );
    assert_eq!(source.3.as_deref(), Some(revision_id.to_string().as_str()));

    // 审计：kb_pc_analysis 两处（KB 完成 + PC 派生）。
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log WHERE source_kind = 'kb_pc_analysis'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 2);
}

// ── 用例 2：同 HEAD 重复 verify 幂等（零增量）──────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn reverify_same_head_is_idempotent(pool: PgPool) {
    seed_principal(&pool).await;
    let project = seed_project(&pool, "CP1-B").await;
    let repo = init_repo("feat: initial");
    let repository_id = seed_repository(&pool, project, &repo.url).await;

    verify(&pool, repository_id).await;
    drain(&worker(&pool)).await;

    // 同 HEAD 再 verify：intake 重放（replayed=true），不建新行。
    verify(&pool, repository_id).await;
    let intake_n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.intake_record \
          WHERE idempotency_key LIKE 'repo_verify:' || $1 || ':%'",
    )
    .bind(repository_id.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(intake_n, 1, "same head must not create a second intake");

    drain(&worker(&pool)).await;
    let revision_n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kb.document_revision r \
           JOIN kb.document d ON d.id = r.document_id \
          WHERE d.project_id = $1",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(revision_n, 1, "content-addressed revision dedup");
    let object_n: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pc.cognition_object WHERE project_id = $1")
            .bind(project)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(object_n, 5, "object_key idempotent upsert");
}

// ── 用例 3：新 commit 再 verify → revision_no=2 + 内容寻址去重 ─────────────

#[sqlx::test(migrations = "../../migrations")]
async fn new_commit_advances_revision_with_content_dedup(pool: PgPool) {
    seed_principal(&pool).await;
    let project = seed_project(&pool, "CP1-C").await;
    let repo = init_repo("feat: initial");
    let repository_id = seed_repository(&pool, project, &repo.url).await;

    verify(&pool, repository_id).await;
    drain(&worker(&pool)).await;

    append_commit(&repo, "feat: second change");
    let outcome = verify(&pool, repository_id).await;
    let head2 = outcome.head_commit_sha.clone().unwrap();
    let intake2 = latest_snapshot_intake(&pool, repository_id).await;
    let key2: String =
        sqlx::query_scalar("SELECT idempotency_key FROM integration.intake_record WHERE id = $1")
            .bind(intake2)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(key2, format!("repo_verify:{repository_id}:{head2}"));

    drain(&worker(&pool)).await;

    // KB：两轮分析 → 两个修订（revision_no 1/2，第二修订锚新 HEAD）。
    let revisions: Vec<(i32, Option<String>)> = sqlx::query_as(
        "SELECT r.revision_no, r.source_commit_sha FROM kb.document_revision r \
           JOIN kb.document d ON d.id = r.document_id \
          WHERE d.project_id = $1 ORDER BY r.revision_no",
    )
    .bind(project)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[1].0, 2);
    assert_eq!(revisions[1].1.as_deref(), Some(head2.as_str()));

    // PC：对象身份稳定（object_key 不变）——snapshot/activity 随 HEAD 变化追加
    // 修订；tech_stack 内容未变 → 内容寻址去重不追加；evidence 每轮追加。
    let snapshot_revisions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pc.cognition_revision cr \
           JOIN pc.cognition_object o ON o.id = cr.object_id \
          WHERE o.project_id = $1 AND o.object_key LIKE '%:snapshot'",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        snapshot_revisions, 2,
        "snapshot content changed → new revision"
    );
    let tech_revisions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pc.cognition_revision cr \
           JOIN pc.cognition_object o ON o.id = cr.object_id \
          WHERE o.project_id = $1 AND o.object_key LIKE '%:tech_stack'",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        tech_revisions, 1,
        "unchanged content must not append revision"
    );
    let evidence_n: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pc.evidence WHERE project_id = $1")
            .bind(project)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(evidence_n, 10, "5 objects × 2 analysis runs");
    assert_eq!(derived_status(&pool, intake2).await, "Completed");
}
