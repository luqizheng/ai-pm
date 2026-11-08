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
//! | 4 | LLM 增强（P2-13/D-1=A，wiremock provider）：digest 尾注翻转为 LLM、pc 三区正文、analysis_mode=llm、白名单证据、ok 记账×2 |
//! | 5 | LLM 失败（D-3=A，wiremock 500）：全链仍 COMPLETED，产物回退纯派生，analysis_mode=derived_fallback，failed 记账×2 |

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
    worker_with_model(pool, None)
}

fn worker_with_model(
    pool: &PgPool,
    snapshot_model: Option<
        std::sync::Arc<dyn pm_application::snapshot_analysis::SnapshotAnalysisModel>,
    >,
) -> OutboxWorker {
    register_all_handlers(
        OutboxWorker::new(pool.clone(), "kbpc-test-worker".to_string()),
        pool,
        snapshot_model,
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
    let pc_result = pc_result.expect("real derivation pins analysis source");
    assert_eq!(pc_result["target_domain"], "pc");
    assert_eq!(pc_result["target_type"], "source", "KP-1: pc_analysis_ref anchors pc.source");
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

    // evidence：unverified/medium，document_section 判别器（槽位 = section_anchor，
    // 归因 JSON 归档 description——01B §9.3 C-4 形态）。
    let evidence: Vec<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT verification_status, confidence, section_anchor, description \
           FROM pc.evidence WHERE project_id = $1",
    )
    .bind(project)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(evidence.len(), 5);
    assert!(evidence
        .iter()
        .all(|(v, c, _, _)| v == "unverified" && c == "medium"));
    assert!(evidence
        .iter()
        .all(|(_, _, anchor, desc)| anchor.is_some()
            && desc
                .as_deref()
                .unwrap_or_default()
                .contains(&intake_id.to_string())));

    // source：kb_document 通道（C-4 收窄 CHECK）+ 四元组钉 KB 文档修订。
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
    let source_kind: String =
        sqlx::query_scalar("SELECT source_kind FROM pc.source WHERE project_id = $1 LIMIT 1")
            .bind(project)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(source_kind, "kb_document");

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

// ── LLM 增强用例（P2-13/D-1=A 起笔切片；wiremock provider + 0062 mode）──────

/// 与 pm-ai 测试同款的 enabled provider 行（`openai_compatible` 免凭证行，
/// endpoint 直指 wiremock；timeout 30s 容纳测试节奏）。
async fn seed_enabled_provider(pool: &PgPool, project_id: Uuid, endpoint: &str) -> Uuid {
    let principal_id = Uuid::new_v4();
    sqlx::query("INSERT INTO core.principal (id, kind, display_name) VALUES ($1, 'ai_pm', 'Seed')")
        .bind(principal_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query_scalar(
        "INSERT INTO ai.team_llm_provider \
         (id, tenant_id, name, provider_type, endpoint, configuration, status, timeout_secs, created_by) \
         VALUES ($1, (SELECT tenant_id FROM core.project WHERE id = $2), 'fixture-provider', \
                 'openai_compatible', $3, '{\"model\": \"test-model\"}', 'enabled', 30, $4) \
         RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(endpoint)
    .bind(principal_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 五骨架的 LLM 响应体（object_key 与 `plan_cognition_objects` 的派生键一致；
/// 含一个编造引用 made-up-ref，断言实现侧白名单过滤）。
fn llm_objects_body(repository_id: Uuid) -> serde_json::Value {
    let zone = |slot: &str| {
        format!("### 事实\nfixture 事实（{slot}）\n### 推断\nfixture 推断\n### 不确定\n无")
    };
    serde_json::json!({
        "objects": [
            {"object_key": format!("repo:{repository_id}:snapshot"), "summary": "LLM 摘要：演示仓库",
             "content": zone("snapshot"), "evidence_refs": ["README.md", "made-up-ref"]},
            {"object_key": format!("repo:{repository_id}:tech_stack"), "summary": "LLM 摘要：Markdown",
             "content": zone("tech_stack")},
            {"object_key": format!("repo:{repository_id}:activity"), "summary": "LLM 摘要：交付",
             "content": zone("activity")},
            {"object_key": format!("repo:{repository_id}:module:README.md"), "summary": "LLM 摘要：入口",
             "content": zone("module")},
            {"object_key": format!("repo:{repository_id}:module:docs/design.md"), "summary": "LLM 摘要：设计",
             "content": zone("module-docs")},
        ]
    })
}

async fn llm_call_records(pool: &PgPool, project_id: Uuid) -> Vec<(String, String)> {
    sqlx::query_as(
        "SELECT purpose, status FROM ai.llm_call_record WHERE project_id = $1 ORDER BY created_at",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .unwrap()
}

fn auto_model(
    pool: &PgPool,
) -> std::sync::Arc<dyn pm_application::snapshot_analysis::SnapshotAnalysisModel> {
    std::sync::Arc::new(pm_ai::snapshot_model::LlmSnapshotAnalysisModel::new(
        pool.clone(),
        pm_ai::snapshot_model::SnapshotModelMode::Auto,
    ))
}

// ── 用例 4：LLM 增强成功路径（digest 尾注翻转 + 三区正文 + 白名单证据 + 记账）──

#[sqlx::test(migrations = "../../migrations")]
async fn llm_enrichment_marks_llm_mode_across_kb_and_pc(pool: PgPool) {
    seed_principal(&pool).await;
    let project = seed_project(&pool, "CP1-D").await;
    let repo = init_repo("feat: initial");
    let repository_id = seed_repository(&pool, project, &repo.url).await;

    let server = wiremock::MockServer::start().await;
    seed_enabled_provider(&pool, project, &server.uri()).await;
    // PC 请求带 <skeletons> 标记（挂载顺序即匹配顺序：先 PC 后 KB 兜底）。
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/chat/completions"))
        .and(wiremock::matchers::body_string_contains("<skeletons>"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": llm_objects_body(repository_id).to_string()}}]
            })),
        )
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/chat/completions"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content":
                    "{\"digest_markdown\": \"这是 fixture 仓库的 LLM 叙述摘要。\"}"}}]
            })),
        )
        .mount(&server)
        .await;

    verify(&pool, repository_id).await;
    let intake_id = latest_snapshot_intake(&pool, repository_id).await;
    drain(&worker_with_model(&pool, Some(auto_model(&pool)))).await;

    // KB：digest 为 LLM 拼装版（尾注翻转），analysis_mode=llm。
    let (_, kb_status, kb_result) = task_of_type(&pool, intake_id, "kb_analyze").await;
    assert_eq!(kb_status, "COMPLETED");
    let revision_id: Uuid = Uuid::parse_str(
        kb_result.expect("real analysis")["target_revision_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let (content, change_summary, mode): (String, String, String) = sqlx::query_as(
        "SELECT content, change_summary, analysis_mode FROM kb.document_revision WHERE id = $1",
    )
    .bind(revision_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(mode, "llm");
    assert!(content.contains("## 摘要（LLM 分析）"));
    assert!(content.contains("这是 fixture 仓库的 LLM 叙述摘要。"));
    assert!(content.contains("model=test-model"));
    assert!(content.contains("技术栈构成"), "structured sections kept");
    assert!(!content.contains("无 LLM 参与"));
    assert!(change_summary.contains("LLM 增强"));

    // PC：五对象全部 LLM 三区正文（骨架形态不变），analysis_mode=llm；
    // 正文权威 = pc.document_revision（KP-1 收敛，偏差 a 暂存 PG）。
    let (_, pc_status, pc_result) = task_of_type(&pool, intake_id, "pc_analyze").await;
    assert_eq!(pc_status, "COMPLETED");
    assert_eq!(pc_result.expect("real derivation")["target_domain"], "pc");
    let revisions: Vec<(String, String)> = sqlx::query_as(
        "SELECT cr.analysis_mode, dr.content FROM pc.cognition_revision cr \
           JOIN pc.cognition_object o ON o.id = cr.object_id \
           JOIN pc.document_revision dr ON dr.id = cr.document_revision_id \
          WHERE o.project_id = $1",
    )
    .bind(project)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(revisions.len(), 5);
    assert!(revisions.iter().all(|(m, _)| m == "llm"));
    assert!(revisions.iter().all(|(_, c)| {
        c.contains("### 事实") && c.contains("### 推断") && c.contains("### 不确定")
    }));

    // evidence：description 归档 JSON 记 mode 与白名单过滤后的 llm_evidence_refs
    // （made-up-ref 必须被实现侧丢弃；01B §9.3：无 JSONB locator，TEXT 归档）。
    let description: String = sqlx::query_scalar(
        "SELECT description FROM pc.evidence \
          WHERE project_id = $1 AND section_anchor = 'snapshot'",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    let locator: serde_json::Value = serde_json::from_str(&description)
        .expect("legacy attribution JSON archived in description");
    assert_eq!(locator["analysis_mode"], "llm");
    let refs = locator["llm_evidence_refs"].as_array().unwrap();
    assert!(refs.iter().all(|r| r.as_str() != Some("made-up-ref")));
    assert!(
        refs.iter().any(|r| r.as_str() == Some("README.md")),
        "allowlisted refs survive"
    );

    // 记账：kb/pc 各一条 ok。
    assert_eq!(
        llm_call_records(&pool, project).await,
        vec![
            ("kb_snapshot_analysis".to_string(), "ok".to_string()),
            ("pc_snapshot_analysis".to_string(), "ok".to_string()),
        ]
    );
    assert_eq!(derived_status(&pool, intake_id).await, "Completed");
}

// ── 用例 5：LLM 失败回退（D-3=A：全链不碎，产物=纯派生，mode=fallback）──────

#[sqlx::test(migrations = "../../migrations")]
async fn llm_failure_falls_back_to_derived_and_records_failed(pool: PgPool) {
    seed_principal(&pool).await;
    let project = seed_project(&pool, "CP1-E").await;
    let repo = init_repo("feat: initial");
    let repository_id = seed_repository(&pool, project, &repo.url).await;

    let server = wiremock::MockServer::start().await;
    seed_enabled_provider(&pool, project, &server.uri()).await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/chat/completions"))
        .respond_with(wiremock::ResponseTemplate::new(500))
        .mount(&server)
        .await;

    verify(&pool, repository_id).await;
    let intake_id = latest_snapshot_intake(&pool, repository_id).await;
    drain(&worker_with_model(&pool, Some(auto_model(&pool)))).await;

    // 全链收口不因 LLM 抖动而碎（D-3=A 核心断言）。
    let (_, kb_status, _) = task_of_type(&pool, intake_id, "kb_analyze").await;
    assert_eq!(kb_status, "COMPLETED");
    let (_, pc_status, _) = task_of_type(&pool, intake_id, "pc_analyze").await;
    assert_eq!(pc_status, "COMPLETED");
    assert_eq!(derived_status(&pool, intake_id).await, "Completed");

    // KB：回退产物即纯派生（尾注「无 LLM 参与」），mode=derived_fallback。
    let (content, mode): (String, String) = sqlx::query_as(
        "SELECT content, analysis_mode FROM kb.document_revision \
           WHERE document_id = (SELECT id FROM kb.document WHERE project_id = $1) \
         ORDER BY revision_no DESC LIMIT 1",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(mode, "derived_fallback");
    assert!(content.contains("无 LLM 参与"));
    assert!(!content.contains("## 摘要（LLM 分析）"));

    // PC：五对象回退派生正文，mode=derived_fallback。
    let modes: Vec<(String,)> = sqlx::query_as(
        "SELECT cr.analysis_mode FROM pc.cognition_revision cr \
           JOIN pc.cognition_object o ON o.id = cr.object_id \
          WHERE o.project_id = $1",
    )
    .bind(project)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(modes.len(), 5);
    assert!(modes.iter().all(|(m,)| m == "derived_fallback"));

    // 记账：kb/pc 各一条 failed（DEC-LLM-004.7：失败也记账）。
    assert_eq!(
        llm_call_records(&pool, project).await,
        vec![
            ("kb_snapshot_analysis".to_string(), "failed".to_string()),
            ("pc_snapshot_analysis".to_string(), "failed".to_string()),
        ]
    );
}
