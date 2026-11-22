//! P0 外部 Commit 采纳闭环集成测试（迁移 0054）。
//!
//! 覆盖：发现（锚点/去重/force push 重建锚）→ 分析（派生回退/失败重试/
//! LLM 输出白名单）→ 澄清（答复推进/not_needed 显式记录）→ 采纳（人类门/
//! 提案管线/状态机约束）→ 关闭（终态门）。
//! LLM 用测试桩（trait 实现）；LLM 线协议与降级语义在 pm-ai 单测覆盖。

use std::path::PathBuf;

use sqlx::PgPool;
use uuid::Uuid;

use pm_application::actor::AuditActor;
use pm_application::external_commit::{
    analyze_external_commit, answer_clarification, close_adoption, decide_adoption,
    discover_external_commits, get_external_commit, read_commit_diff, AdoptionDecisionRequest,
    CommitAnalysis, CommitAnalysisInput, CommitAnalysisModel, CommitAnalysisOutput,
    DeviationJudgment, ExternalCommitError, KnowledgeUpdateSuggestion,
};
use pm_application::knowledge::KnowledgeRoot;
use pm_application::repo_facts::pm_ai_port::ModelError;

// ── 夹具 ──────────────────────────────────────────────────────────────────

/// 本地 file:// 仓库夹具：初始化 + N 个提交，返回 URL 与仓库根。
fn init_repo(name: &str, commits: &[(&str, &str)]) -> (String, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "ai-pm-ec-test-{name}-{}",
        Uuid::new_v4()
    ));
    let path = root.join("repo");
    std::fs::create_dir_all(&path).unwrap();
    let run = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(&path)
            .env("GIT_AUTHOR_NAME", "External Dev")
            .env("GIT_AUTHOR_EMAIL", "dev@example.com")
            .env("GIT_COMMITTER_NAME", "External Dev")
            .env("GIT_COMMITTER_EMAIL", "dev@example.com")
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    };
    run(&["init", "-q", "-b", "main"]);
    for (file, message) in commits {
        std::fs::write(path.join(file), format!("# {file}\n\ncontent of {file}\n")).unwrap();
        run(&["add", "."]);
        run(&["-c", "commit.gpgsign=false", "commit", "-q", "-m", message]);
    }
    let url = format!("file://{}", path.to_string_lossy().replace('\\', "/"));
    (url, path)
}

/// 额外提交（不动已有文件，保持增量语义清晰）。
fn commit_change(repo_root: &PathBuf, file: &str, message: &str) {
    let target = repo_root.join(file);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(target, format!("# {file}\n\nupdated\n")).unwrap();
    let run = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(repo_root)
            .env("GIT_AUTHOR_NAME", "External Dev")
            .env("GIT_AUTHOR_EMAIL", "dev@example.com")
            .env("GIT_COMMITTER_NAME", "External Dev")
            .env("GIT_COMMITTER_EMAIL", "dev@example.com")
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    };
    run(&["add", "."]);
    run(&["-c", "commit.gpgsign=false", "commit", "-q", "-m", message]);
}

async fn seed_human_principal(pool: &PgPool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO core.principal (id, kind, display_name) VALUES ($1, 'human', '产品经理')")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn seed_ai_principal(pool: &PgPool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO core.principal (id, kind, display_name) VALUES ($1, 'ai_pm', 'Agent')")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    id
}

struct Fixture {
    project_id: Uuid,
    repository_id: Uuid,
    repo_root: PathBuf,
    human: AuditActor,
    /// 项目知识库根（每用例独立临时目录，正文权威在 Git）。
    knowledge_root: KnowledgeRoot,
}

async fn seed(pool: &PgPool, tag: &str, commits: &[(&str, &str)]) -> Fixture {
    let customer_id = Uuid::new_v4();
    sqlx::query("INSERT INTO core.customer (id, name, tenant_id) VALUES ($1, 'Acme', $2)")
        .bind(customer_id)
        .bind(pm_domain::SYSTEM_TENANT)
        .execute(pool)
        .await
        .unwrap();
    let project_id = Uuid::new_v4();
    let code = format!(
        "EC-{}",
        project_id.simple().to_string()[..8].to_uppercase()
    );
    sqlx::query(
        "INSERT INTO core.project (id, customer_id, code, name, tenant_id) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(project_id)
    .bind(customer_id)
    .bind(&code)
    .bind(format!("外部提交测试 {tag}"))
    .bind(pm_domain::SYSTEM_TENANT)
    .execute(pool)
    .await
    .unwrap();

    let human_id = seed_human_principal(pool).await;
    let human = AuditActor::principal(human_id);

    let (url, repo_root) = init_repo(tag, commits);
    let repository_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO integration.project_repository \
         (id, project_id, name, url, status, default_branch, tenant_id) \
         VALUES ($1, $2, 'origin', $3, 'verified', 'main', \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(repository_id)
    .bind(project_id)
    .bind(&url)
    .execute(pool)
    .await
    .unwrap();

    Fixture {
        project_id,
        repository_id,
        repo_root,
        human,
        knowledge_root: KnowledgeRoot::new(
            std::env::temp_dir().join(format!("ai-pm-ec-root-{}", Uuid::new_v4())),
        ),
    }
}

/// 既有知识文档（doc_key 白名单成员）。
async fn seed_known_doc(pool: &PgPool, project_id: Uuid, doc_key: &str) {
    sqlx::query(
        "INSERT INTO ai.project_document (id, project_id, doc_key, doc_type, canonical_path, tenant_id) \
         VALUES ($1, $2, $3, 'knowledge', $4, \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(doc_key)
    .bind(format!("projects/EC/knowledge/{doc_key}.md"))
    .execute(pool)
    .await
    .unwrap();
}

// ── 分析模型桩 ────────────────────────────────────────────────────────────

/// 恒返回 None（派生回退路径）。
struct DerivedStub;

#[async_trait::async_trait]
impl CommitAnalysisModel for DerivedStub {
    async fn analyze_commit(
        &self,
        _input: &CommitAnalysisInput<'_>,
    ) -> Result<Option<CommitAnalysisOutput>, ModelError> {
        Ok(None)
    }
}

/// 恒失败（失败可见 + 重试路径）。
struct FailingStub;

#[async_trait::async_trait]
impl CommitAnalysisModel for FailingStub {
    async fn analyze_commit(
        &self,
        _input: &CommitAnalysisInput<'_>,
    ) -> Result<Option<CommitAnalysisOutput>, ModelError> {
        Err(ModelError::Unavailable("provider down".to_string()))
    }
}

/// 带拟稿与澄清问题的输出（澄清 + 采纳主链）。
struct FullStub {
    doc_key: String,
}

fn full_output(doc_key: &str) -> CommitAnalysisOutput {
    CommitAnalysisOutput {
        analysis: CommitAnalysis {
            summary: "实现登录页面并同步设计文档".to_string(),
            analysis_mode: "llm".to_string(),
            related_objects: vec![pm_application::external_commit::RelatedObject {
                domain: "requirement".to_string(),
                id: "not-a-candidate-id".to_string(),
                title: "编造对象会被白名单丢弃".to_string(),
                relation: "实现".to_string(),
            }],
            deviation: Some(DeviationJudgment {
                detected: false,
                note: "与既有需求一致".to_string(),
            }),
            knowledge_updates: vec![KnowledgeUpdateSuggestion {
                doc_key: doc_key.to_string(),
                doc_type: "knowledge".to_string(),
                title: doc_key.to_string(),
                summary: "补充登录页说明".to_string(),
                proposed_markdown: "# 登录设计\n\n## 新增登录页\n\n按外部提交同步。".to_string(),
            }],
        },
        clarification_questions: vec!["登录态存储方案是否沿用既有约定？".to_string()],
    }
}

#[async_trait::async_trait]
impl CommitAnalysisModel for FullStub {
    async fn analyze_commit(
        &self,
        _input: &CommitAnalysisInput<'_>,
    ) -> Result<Option<CommitAnalysisOutput>, ModelError> {
        Ok(Some(full_output(&self.doc_key)))
    }
}

/// 推进到 awaiting_adoption 的捷径：建锚 → 新提交 → 发现 → 分析 → 人类答复。
async fn reach_awaiting_adoption(pool: &PgPool, fx: &Fixture, doc_key: &str) -> Uuid {
    // 首扫只建锚（历史不进队列）。
    discover_external_commits(pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();
    // 外部新提交 → 扫描发现。
    commit_change(&fx.repo_root, "feature.md", "feat: login page");
    let outcome = discover_external_commits(pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();
    assert_eq!(
        outcome.discovered_commits, 1,
        "fixture must discover exactly one commit"
    );
    let listed = pm_application::external_commit::list_external_commits(
        pool,
        fx.project_id,
        None,
    )
    .await
    .unwrap();
    let commit_id: Uuid = listed[0]["id"].as_str().unwrap().parse().unwrap();

    let stub = FullStub {
        doc_key: doc_key.to_string(),
    };
    let analyzed = analyze_external_commit(
        pool,
        &fx.knowledge_root,
        &stub,
        commit_id,
        &fx.human,
    )
    .await
    .unwrap();
    assert_eq!(analyzed.status, "analyzed");

    let (_, clarifications, _) = get_external_commit(pool, commit_id).await.unwrap();
    assert_eq!(clarifications.len(), 1);
    for clarification in &clarifications {
        answer_clarification(pool, commit_id, clarification.id, &fx.human, "沿用既有约定")
            .await
            .unwrap();
    }
    commit_id
}

// ── 1. 发现 ───────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn first_scan_builds_anchor_without_flooding_history(pool: PgPool) {
    let fx = seed(&pool, "anchor", &[("a.md", "init a"), ("b.md", "init b")]).await;

    let outcome = discover_external_commits(&pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();
    assert_eq!(outcome.scanned_repositories, 1);
    let scan = &outcome.repositories[0];
    assert_eq!(scan.discovered, 0, "first scan must only build the anchor");
    assert!(scan.remote_head.is_some());
    assert!(scan.note.as_deref().unwrap_or_default().contains("锚点"));

    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM integration.external_commit")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 0, "pre-existing history must not flood the adoption queue");

    // 新提交 → 下次扫描发现。
    commit_change(&fx.repo_root, "c.md", "feat: add login page");
    let outcome = discover_external_commits(&pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();
    assert_eq!(outcome.repositories[0].discovered, 1);

    let listed = pm_application::external_commit::list_external_commits(
        &pool,
        fx.project_id,
        None,
    )
    .await
    .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["status"], "discovered");
    assert_eq!(listed[0]["author_name"], "External Dev");
    assert_eq!(listed[0]["subject"], "feat: add login page");
}

#[sqlx::test(migrations = "../../migrations")]
async fn repeated_scans_dedupe_by_repository_and_sha(pool: PgPool) {
    let fx = seed(&pool, "dedupe", &[("a.md", "init")]).await;
    // 首扫建锚（历史不进队列）。
    discover_external_commits(&pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();
    // 推两个提交再扫 → 发现 2。
    commit_change(&fx.repo_root, "b.md", "feat: add b");
    commit_change(&fx.repo_root, "c.md", "feat: add c");

    let first = discover_external_commits(&pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();
    assert_eq!(first.repositories[0].discovered, 2);

    let second = discover_external_commits(&pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();
    assert_eq!(
        second.repositories[0].discovered,
        0,
        "HEAD 未前进的重复扫描必须零新增"
    );

    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM integration.external_commit")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 2, "UNIQUE(repository_id, commit_sha) 防重复发现");

    // 去重后备证明：直接重插同一 sha 被 ON CONFLICT 吸收（唯一约束在库层）。
    let sha: String =
        sqlx::query_scalar("SELECT commit_sha FROM integration.external_commit LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let conflict = sqlx::query(
        "INSERT INTO integration.external_commit \
         (tenant_id, project_id, repository_id, commit_sha, branch, author_name, committed_at, subject) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3, 'main', 'x', now(), 'dup') \
         ON CONFLICT (repository_id, commit_sha) DO NOTHING",
    )
    .bind(fx.project_id)
    .bind(fx.repository_id)
    .bind(&sha)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(conflict.rows_affected(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn force_push_rebuilds_anchor_instead_of_guessing_range(pool: PgPool) {
    let fx = seed(&pool, "forcepush", &[("a.md", "init")]).await;
    discover_external_commits(&pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();

    // force push：amend 产生历史重写。
    let run = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(&fx.repo_root)
            .env("GIT_AUTHOR_NAME", "External Dev")
            .env("GIT_COMMITTER_NAME", "External Dev")
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    };
    std::fs::write(fx.repo_root.join("a.md"), "# rewritten\n").unwrap();
    run(&["add", "."]);
    run(&[
        "-c",
        "commit.gpgsign=false",
        "commit",
        "-q",
        "--amend",
        "-m",
        "init (rewritten)",
    ]);

    let outcome = discover_external_commits(&pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();
    let scan = &outcome.repositories[0];
    assert_eq!(scan.discovered, 0, "历史重写不得猜区间");
    assert!(scan.note.as_deref().unwrap_or_default().contains("重建锚点"));
}

// ── 2. 分析 ───────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn analyze_without_provider_falls_back_to_derived_and_records_not_needed(pool: PgPool) {
    let fx = seed(&pool, "derived", &[("a.md", "init")]).await;
    discover_external_commits(&pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();
    commit_change(&fx.repo_root, "docs/guide.md", "docs: update guide");
    discover_external_commits(&pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();

    let listed =
        pm_application::external_commit::list_external_commits(&pool, fx.project_id, None)
            .await
            .unwrap();
    let commit_id: Uuid = listed[0]["id"].as_str().unwrap().parse().unwrap();

    let outcome = analyze_external_commit(
        &pool,
        &fx.knowledge_root,
        &DerivedStub,
        commit_id,
        &fx.human,
    )
    .await
    .unwrap();
    assert_eq!(outcome.status, "awaiting_adoption", "派生模式零澄清 → 直接待采纳");
    assert_eq!(outcome.analysis_mode.as_deref(), Some("derived"));
    assert_eq!(outcome.clarification_state.as_deref(), Some("not_needed"));

    let (row, clarifications, _) = get_external_commit(&pool, commit_id).await.unwrap();
    assert!(clarifications.is_empty());
    assert_eq!(row.clarification_state, "not_needed", "无需澄清必须显式记录");
    let analysis = row.analysis.unwrap();
    assert_eq!(analysis["analysis_mode"], "derived");
    assert!(analysis["knowledge_updates"].as_array().unwrap().is_empty());
    assert_eq!(row.status, "awaiting_adoption");
}

#[sqlx::test(migrations = "../../migrations")]
async fn analysis_failure_is_visible_and_retry_recovers(pool: PgPool) {
    let fx = seed(&pool, "retry", &[("a.md", "init")]).await;
    discover_external_commits(&pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();
    commit_change(&fx.repo_root, "b.md", "feat: b");
    discover_external_commits(&pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();
    let listed =
        pm_application::external_commit::list_external_commits(&pool, fx.project_id, None)
            .await
            .unwrap();
    let commit_id: Uuid = listed[0]["id"].as_str().unwrap().parse().unwrap();

    let failed = analyze_external_commit(
        &pool,
        &fx.knowledge_root,
        &FailingStub,
        commit_id,
        &fx.human,
    )
    .await
    .unwrap();
    assert_eq!(failed.status, "failed");
    assert!(failed.error.as_deref().unwrap_or_default().contains("provider down"));

    let (row, _, _) = get_external_commit(&pool, commit_id).await.unwrap();
    assert_eq!(row.status, "failed");
    assert!(row.last_error.as_deref().unwrap_or_default().contains("provider down"));
    assert_eq!(row.attempts, 1);

    // 重试成功。
    let recovered = analyze_external_commit(
        &pool,
        &fx.knowledge_root,
        &DerivedStub,
        commit_id,
        &fx.human,
    )
    .await
    .unwrap();
    assert_eq!(recovered.status, "awaiting_adoption");
    let (row, _, _) = get_external_commit(&pool, commit_id).await.unwrap();
    assert_eq!(row.attempts, 2, "重试必须累加 attempts");
    assert!(row.last_error.is_none(), "成功重试必须清除 last_error");
}

#[sqlx::test(migrations = "../../migrations")]
async fn llm_output_is_sanitized_and_questions_become_clarifications(pool: PgPool) {
    let fx = seed(&pool, "sanitize", &[("a.md", "init")]).await;
    seed_known_doc(&pool, fx.project_id, "login-design").await;
    discover_external_commits(&pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();
    commit_change(&fx.repo_root, "b.md", "feat: login");
    discover_external_commits(&pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();
    let listed =
        pm_application::external_commit::list_external_commits(&pool, fx.project_id, None)
            .await
            .unwrap();
    let commit_id: Uuid = listed[0]["id"].as_str().unwrap().parse().unwrap();

    let stub = FullStub {
        doc_key: "login-design".to_string(),
    };
    let outcome = analyze_external_commit(
        &pool,
        &fx.knowledge_root,
        &stub,
        commit_id,
        &fx.human,
    )
    .await
    .unwrap();
    assert_eq!(outcome.status, "analyzed", "有澄清问题 → analyzed + pending");
    assert_eq!(outcome.clarification_state.as_deref(), Some("pending"));

    let (row, clarifications, _) = get_external_commit(&pool, commit_id).await.unwrap();
    assert_eq!(clarifications.len(), 1);
    assert_eq!(row.clarification_state, "pending");
    // 白名单：编造的 related id 被丢弃。
    let analysis = row.analysis.unwrap();
    assert!(
        analysis["related_objects"].as_array().unwrap().is_empty(),
        "编造候选外的 id 必须被白名单丢弃"
    );
    // doc_type 由服务端白名单回填。
    assert_eq!(analysis["knowledge_updates"][0]["doc_type"], "knowledge");
}

// ── 3. 澄清 ───────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn answering_all_clarifications_promotes_to_awaiting_adoption(pool: PgPool) {
    let fx = seed(&pool, "clarify", &[("a.md", "init")]).await;
    seed_known_doc(&pool, fx.project_id, "login-design").await;
    discover_external_commits(&pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();
    commit_change(&fx.repo_root, "b.md", "feat: login");
    discover_external_commits(&pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();
    let listed =
        pm_application::external_commit::list_external_commits(&pool, fx.project_id, None)
            .await
            .unwrap();
    let commit_id: Uuid = listed[0]["id"].as_str().unwrap().parse().unwrap();
    let stub = FullStub {
        doc_key: "login-design".to_string(),
    };
    analyze_external_commit(
        &pool,
        &fx.knowledge_root,
        &stub,
        commit_id,
        &fx.human,
    )
    .await
    .unwrap();

    // AI 主体不得答复（人类面通道执法）。
    let ai_id = seed_ai_principal(&pool).await;
    let ai = AuditActor::principal(ai_id);
    let (_, clarifications, _) = get_external_commit(&pool, commit_id).await.unwrap();
    let err = answer_clarification(&pool, commit_id, clarifications[0].id, &ai, "机器人答")
        .await
        .unwrap_err();
    assert!(matches!(err, ExternalCommitError::Forbidden(_)), "{err:?}");

    // 空答复拒绝。
    let err = answer_clarification(&pool, commit_id, clarifications[0].id, &fx.human, "  ")
        .await
        .unwrap_err();
    assert!(matches!(err, ExternalCommitError::Validation(_)));

    // 人类答复 → resolved + awaiting_adoption。
    let answered =
        answer_clarification(&pool, commit_id, clarifications[0].id, &fx.human, "沿用既有约定")
            .await
            .unwrap();
    assert_eq!(answered.status, "answered");
    let (row, clarifications, _) = get_external_commit(&pool, commit_id).await.unwrap();
    assert_eq!(row.clarification_state, "resolved");
    assert_eq!(row.status, "awaiting_adoption");
    assert!(clarifications.iter().all(|c| c.status == "answered"));
    assert!(clarifications[0].answered_at.is_some());
}

// ── 4. 采纳 ───────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn adopt_requires_analysis_and_human_principal(pool: PgPool) {
    let fx = seed(&pool, "guard", &[("a.md", "init")]).await;
    discover_external_commits(&pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();
    commit_change(&fx.repo_root, "b.md", "feat: b");
    discover_external_commits(&pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();
    let listed =
        pm_application::external_commit::list_external_commits(&pool, fx.project_id, None)
            .await
            .unwrap();
    let commit_id: Uuid = listed[0]["id"].as_str().unwrap().parse().unwrap();

    // 发现态直接决定 → 409（发现 ≠ 采纳；CHECK 约束同层兜底）。
    let err = decide_adoption(
        &pool,
        &fx.knowledge_root,
        commit_id,
        &fx.human,
        AdoptionDecisionRequest {
            decision: "approved".to_string(),
            note: None,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ExternalCommitError::Conflict(_)), "{err:?}");

    // 推进到 awaiting_adoption 后，AI 主体仍不得决定。
    let stub = DerivedStub;
    analyze_external_commit(
        &pool,
        &fx.knowledge_root,
        &stub,
        commit_id,
        &fx.human,
    )
    .await
    .unwrap();
    let ai_id = seed_ai_principal(&pool).await;
    let ai = AuditActor::principal(ai_id);
    let err = decide_adoption(
        &pool,
        &fx.knowledge_root,
        commit_id,
        &ai,
        AdoptionDecisionRequest {
            decision: "approved".to_string(),
            note: None,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ExternalCommitError::Forbidden(_)), "{err:?}");
}

#[sqlx::test(migrations = "../../migrations")]
async fn approval_submits_knowledge_proposal_and_close_backfills_revision(pool: PgPool) {
    let fx = seed(&pool, "adopt", &[("a.md", "init")]).await;
    seed_known_doc(&pool, fx.project_id, "login-design").await;
    let commit_id = reach_awaiting_adoption(&pool, &fx, "login-design").await;

    let outcome = decide_adoption(
        &pool,
        &fx.knowledge_root,
        commit_id,
        &fx.human,
        AdoptionDecisionRequest {
            decision: "approved".to_string(),
            note: Some("采纳登录页同步".to_string()),
        },
    )
    .await
    .unwrap();
    assert_eq!(outcome.status, "adopted");
    assert_eq!(outcome.adoption_decision, "approved");

    let proposals = outcome.knowledge_proposals.as_array().unwrap();
    assert_eq!(proposals.len(), 1);
    let proposal_id: Uuid = proposals[0]["proposal_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(proposals[0]["outcome"], "committed", "过质量门的常规修订直接提交");

    // 提案已落正式修订（Markdown+Git 权威正文经既有管线写入；DB 只存修订账目，
    // 正文按 canonical_path 落在知识库 Git 工作树）。
    let revision_id: Uuid = sqlx::query_scalar(
        "SELECT resulting_revision_id FROM ai.document_proposal WHERE id = $1",
    )
    .bind(proposal_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    // 提案行的 canonical_path 是 submit 时按 project_code 规范化的真实落盘路径
    //（夹具种子行用的是占位路径，不能作为读盘依据）。
    let (git_sha, canonical_path): (String, String) = sqlx::query_as(
        "SELECT r.git_commit_sha, p.canonical_path          FROM ai.document_revision r          JOIN ai.document_proposal p ON p.resulting_revision_id = r.id          WHERE r.id = $1",
    )
    .bind(revision_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!git_sha.is_empty(), "修订必须引用 Git sha");
    let md_path = fx.knowledge_root.path().join(&canonical_path);
    eprintln!(
        "DEBUG md_path={} exists={} root_exists={:?}",
        md_path.display(),
        md_path.exists(),
        std::fs::read_dir(fx.knowledge_root.path()).map(|entries| entries.count()).unwrap_or(0)
    );
    let markdown = std::fs::read_to_string(&md_path).unwrap();
    assert!(markdown.contains("source_events:"), "缺省来源尾注必须由采纳路径补齐");
    assert!(markdown.contains("# 登录设计"), "拟稿正文必须真实写入权威正文");

    // 提案未关闭前 close 必须可行（本例已 committed）→ 回填修订引用并关闭。
    let closed = close_adoption(&pool, commit_id, &fx.human).await.unwrap();
    assert_eq!(closed.status, "closed");
    let (row, _, _) = get_external_commit(&pool, commit_id).await.unwrap();
    assert_eq!(row.status, "closed");
    assert_eq!(row.adoption_decision.as_deref(), Some("approved"));
    let revisions = row.adopted_revision_ids.as_array().unwrap();
    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0].as_str().unwrap(), revision_id.to_string());

    // 幂等：重复 approve 不可（状态已 closed）。
    let err = decide_adoption(
        &pool,
        &fx.knowledge_root,
        commit_id,
        &fx.human,
        AdoptionDecisionRequest {
            decision: "approved".to_string(),
            note: None,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ExternalCommitError::Conflict(_)));
}

#[sqlx::test(migrations = "../../migrations")]
async fn close_is_blocked_while_knowledge_proposal_is_pending(pool: PgPool) {
    let fx = seed(&pool, "closegate", &[("a.md", "init")]).await;
    seed_known_doc(&pool, fx.project_id, "login-design").await;
    let commit_id = reach_awaiting_adoption(&pool, &fx, "login-design").await;

    // 把提案推向红线待确认（模拟人类面未完成：直接改状态等价于 redline 场景；
    // 红线确认全链在 knowledge 管线自有测试覆盖）。
    let outcome = decide_adoption(
        &pool,
        &fx.knowledge_root,
        commit_id,
        &fx.human,
        AdoptionDecisionRequest {
            decision: "approved".to_string(),
            note: None,
        },
    )
    .await
    .unwrap();
    let proposal_id: Uuid = outcome.knowledge_proposals.as_array().unwrap()[0]["proposal_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    sqlx::query("UPDATE ai.document_proposal SET status = 'redline_pending' WHERE id = $1")
        .bind(proposal_id)
        .execute(&pool)
        .await
        .unwrap();

    let err = close_adoption(&pool, commit_id, &fx.human).await.unwrap_err();
    let ExternalCommitError::Conflict(message) = &err else {
        panic!("expected conflict, got {err:?}");
    };
    assert!(message.contains("redline_pending"), "{message}");

    // 提案落到终态（等价于人类在知识审核面完成确认/拒绝；红线确认全链在
    // knowledge 管线自有测试覆盖）→ close 通过。
    sqlx::query("UPDATE ai.document_proposal SET status = 'rejected' WHERE id = $1")
        .bind(proposal_id)
        .execute(&pool)
        .await
        .unwrap();

    let closed = close_adoption(&pool, commit_id, &fx.human).await.unwrap();
    assert_eq!(closed.status, "closed");
    let (row, _, _) = get_external_commit(&pool, commit_id).await.unwrap();
    assert_eq!(row.status, "closed");
    assert!(
        row.adopted_revision_ids.as_array().unwrap().is_empty(),
        "被拒绝的提案不产生修订引用"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn no_update_and_reject_close_explicitly(pool: PgPool) {
    let fx = seed(&pool, "noupdate", &[("a.md", "init")]).await;
    discover_external_commits(&pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();
    commit_change(&fx.repo_root, "b.md", "chore: typo");
    discover_external_commits(&pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();
    let listed =
        pm_application::external_commit::list_external_commits(&pool, fx.project_id, None)
            .await
            .unwrap();
    let commit_id: Uuid = listed[0]["id"].as_str().unwrap().parse().unwrap();
    analyze_external_commit(
        &pool,
        &fx.knowledge_root,
        &DerivedStub,
        commit_id,
        &fx.human,
    )
    .await
    .unwrap();

    let outcome = decide_adoption(
        &pool,
        &fx.knowledge_root,
        commit_id,
        &fx.human,
        AdoptionDecisionRequest {
            decision: "no_update".to_string(),
            note: Some("纯笔误，无需知识更新".to_string()),
        },
    )
    .await
    .unwrap();
    assert_eq!(outcome.status, "closed");
    assert_eq!(outcome.adoption_decision, "no_update");
    let (row, _, _) = get_external_commit(&pool, commit_id).await.unwrap();
    assert_eq!(row.adoption_decision.as_deref(), Some("no_update"));
    assert!(row.adopted_at.is_some(), "显式无需更新也必须落决定人与时间");

    // rejected 路径独立成行。
    commit_change(&fx.repo_root, "c.md", "feat: unrelated");
    discover_external_commits(&pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();
    let listed = pm_application::external_commit::list_external_commits(
        &pool,
        fx.project_id,
        Some("discovered"),
    )
    .await
    .unwrap();
    let commit_id: Uuid = listed[0]["id"].as_str().unwrap().parse().unwrap();
    analyze_external_commit(
        &pool,
        &fx.knowledge_root,
        &DerivedStub,
        commit_id,
        &fx.human,
    )
    .await
    .unwrap();
    let outcome = decide_adoption(
        &pool,
        &fx.knowledge_root,
        commit_id,
        &fx.human,
        AdoptionDecisionRequest {
            decision: "rejected".to_string(),
            note: Some("与项目无关".to_string()),
        },
    )
    .await
    .unwrap();
    assert_eq!(outcome.status, "closed");
    assert_eq!(outcome.adoption_decision, "rejected");

    // 非法决定值拒绝。
    commit_change(&fx.repo_root, "d.md", "feat: d");
    discover_external_commits(&pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();
    let listed = pm_application::external_commit::list_external_commits(
        &pool,
        fx.project_id,
        Some("discovered"),
    )
    .await
    .unwrap();
    let commit_id: Uuid = listed[0]["id"].as_str().unwrap().parse().unwrap();
    analyze_external_commit(
        &pool,
        &fx.knowledge_root,
        &DerivedStub,
        commit_id,
        &fx.human,
    )
    .await
    .unwrap();
    let err = decide_adoption(
        &pool,
        &fx.knowledge_root,
        commit_id,
        &fx.human,
        AdoptionDecisionRequest {
            decision: "auto_approve".to_string(),
            note: None,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ExternalCommitError::Validation(_)));
}

// ── 5. 差异读取 ───────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn diff_view_reads_the_cached_clone(pool: PgPool) {
    let fx = seed(&pool, "diff", &[("a.md", "init")]).await;
    discover_external_commits(&pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();
    commit_change(&fx.repo_root, "b.md", "feat: b with body");
    discover_external_commits(&pool, fx.project_id, &fx.human, None)
        .await
        .unwrap();
    let listed =
        pm_application::external_commit::list_external_commits(&pool, fx.project_id, None)
            .await
            .unwrap();
    let commit_id: Uuid = listed[0]["id"].as_str().unwrap().parse().unwrap();

    let (diff, truncated, sha) = read_commit_diff(&pool, commit_id).await.unwrap();
    assert!(!truncated);
    assert_eq!(sha, listed[0]["commit_sha"].as_str().unwrap());
    assert!(diff.contains("diff --git"), "必须返回 patch 形态的变更差异");
    assert!(diff.contains("b.md"));
}
