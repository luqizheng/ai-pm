//! 冷启动导入的集成测试：启动 → Fake 模型分析 → 草案 → 4 条提案 → 审批执行。
//!
//! 全程真实 PostgreSQL（`#[sqlx::test]`）与真实 Git 仓库；不访问网络。

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use pm_ai::repo_model::FakeRepoAnalysisModel;
use pm_application::bootstrap::{BootstrapRequest, bootstrap_project_with_status};
use pm_application::command_executor::PgCommandExecutor;
use pm_application::project_import::{
    ApplyProject, ApplyRequest, StartImportRequest, ground_draft, propose_import, run_import_job,
    start_import,
};
use pm_application::repo_facts::{CommitFact, DocContent, HotFile, LanguageStat, RepoFacts};
use pm_application::supervision::{DecideRequest, Decision, decide_approval};
use pm_application::PolicyEngine;
use pm_domain::import::{
    AssigneeRole, Evidence, EvidenceKind, OrgChart, ProjectDraft, RequirementDraft, TaskDraft,
};
use pm_domain::proposal::Confidence;
use sqlx::PgPool;
use uuid::Uuid;
use pm_domain::SYSTEM_TENANT;

struct Repo {
    root: PathBuf,
    url: String,
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn init_repo() -> Repo {
    let root = std::env::temp_dir().join(format!("ai-pm-import-{}", Uuid::new_v4()));
    let path = root.join("repo");
    std::fs::create_dir_all(&path).unwrap();
    let run = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(&path)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    };
    run(&["init", "-q", "-b", "main"]);
    std::fs::write(path.join("README.md"), "# 世界地图编辑器\n\nTODO: docs\n").unwrap();
    std::fs::create_dir_all(path.join("docs")).unwrap();
    std::fs::write(path.join("docs/design.md"), "# 地形设计\n").unwrap();
    run(&["add", "-A"]);
    run(&["-c", "commit.gpgsign=false", "commit", "-q", "-m", "feat(core): terrain"]);
    std::fs::write(path.join("docs/design.md"), "# 地形设计 v2\n").unwrap();
    run(&["add", "-A"]);
    run(&["-c", "commit.gpgsign=false", "commit", "-q", "-m", "fix: docs"]);
    let url = format!("file://{}", path.to_string_lossy().replace('\\', "/"));
    Repo { root, url }
}

async fn seed_project_with_repo(pool: &PgPool, code: &str) -> (Uuid, Uuid, Repo) {
    let outcome = bootstrap_project_with_status(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
        Some("initializing"),
    )
    .await
    .unwrap();
    let repo = init_repo();
    let created = pm_application::repository::create_repository(
        pool,
        outcome.project_id,
        &pm_application::actor::AuditActor::principal(outcome.human_id),
        pm_application::repository::CreateRepositoryRequest {
            name: "主仓库".to_string(),
            url: "git@example.invalid:team/demo.git".to_string(),
            role: Some("main".to_string()),
            default_branch: Some("main".to_string()),
            description: None,
        },
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE integration.project_repository SET url = $2, status = 'verified', \
         default_branch = 'main' WHERE id = $1",
    )
    .bind(created.id)
    .bind(&repo.url)
    .execute(pool)
    .await
    .unwrap();
    (outcome.project_id, created.id, repo)
}

async fn prepare_draft(pool: &PgPool, project_id: Uuid) -> (Uuid, ProjectDraft) {
    // N-3：`repository_ids` 必填——导入集合由调用方明确指定。
    let repository_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM integration.project_repository WHERE project_id = $1 LIMIT 1",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let started = start_import(
        pool,
        project_id,
        &pm_application::actor::AuditActor::dispatch(),
        &StartImportRequest {
            repository_ids: vec![repository_id],
            idempotency_key: None,
        },
    )
    .await
    .unwrap();
    let model = FakeRepoAnalysisModel;
    run_import_job(pool, &model, started.import_id).await.unwrap();
    let detail = pm_application::project_import::get_import(pool, project_id, started.import_id)
        .await
        .unwrap();
    assert_eq!(detail.status, "draft_ready");
    let draft = detail.draft.unwrap();
    assert!(!draft.requirements.is_empty());
    assert!(!draft.tasks.is_empty());
    assert!(detail.dropped_items >= 0);
    (started.import_id, draft)
}

fn apply_request(draft: &ProjectDraft, revision: i64) -> ApplyRequest {
    ApplyRequest {
        draft_revision: revision,
        project: ApplyProject {
            code: "IMPORT-E2E".to_string(),
            name: "导入项目".to_string(),
            description: String::new(),
        },
        milestone_keys: draft.milestones.iter().map(|m| m.key.clone()).collect(),
        requirement_keys: draft.requirements.iter().map(|r| r.key.clone()).collect(),
        task_keys: draft.tasks.iter().map(|t| t.key.clone()).collect(),
        write_knowledge_documents: true,
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn import_creates_four_proposals_without_delivery_writes(pool: PgPool) {
    let (project_id, _, _repo) = seed_project_with_repo(&pool, "IMP-1").await;
    let (import_id, draft) = prepare_draft(&pool, project_id).await;
    let detail = pm_application::project_import::get_import(&pool, project_id, import_id)
        .await
        .unwrap();
    let req = apply_request(&draft, detail.draft_revision);
    let proposed = propose_import(&pool, project_id, import_id, req.clone())
        .await
        .unwrap();
    assert_eq!(proposed.outcome, "proposed");
    assert_eq!(proposed.proposals.len(), 4);
    assert_eq!(proposed.proposals[0].kind, "create_milestones");
    assert_eq!(proposed.proposals[1].kind, "create_requirements");
    assert_eq!(proposed.proposals[2].kind, "create_tasks");
    assert_eq!(proposed.proposals[3].kind, "write_documents");

    let approvals: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.approval_request \
         WHERE project_id = $1 AND required_role = 'project_owner' AND status = 'pending'",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(approvals, 4);

    for table in ["requirement", "milestone", "task"] {
        let count: i64 = sqlx::query_scalar(&format!(
            "SELECT count(*) FROM delivery.{table} WHERE project_id = $1"
        ))
        .bind(project_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0, "delivery.{table} must stay empty before approval");
    }

    // 重复 apply 在 proposed 状态返回同一组提案。
    let repeated = propose_import(&pool, project_id, import_id, req)
        .await
        .unwrap();
    assert_eq!(repeated.proposals.len(), 4);
    let ids: Vec<Uuid> = repeated.proposals.iter().map(|p| p.proposal_id).collect();
    let originals: Vec<Uuid> = proposed.proposals.iter().map(|p| p.proposal_id).collect();
    assert_eq!(ids, originals);
}

/// 台账 G-20：导入草案就绪后 Worker 自动开 `source=SYSTEM` 认知会话，
/// unknowns 映射为质询（Discovery Worker 分析作业）。
#[sqlx::test(migrations = "../../migrations")]
async fn draft_ready_auto_starts_a_system_discovery_session(pool: PgPool) {
    let (project_id, _repository_id, _repo) = seed_project_with_repo(&pool, "AUTODISC").await;
    let (import_id, _draft) = prepare_draft(&pool, project_id).await;

    let (session_id, source, question_count): (Uuid, String, i64) = sqlx::query_as(
        "SELECT s.id, s.source, \
                (SELECT count(*) FROM discovery.discovery_question q WHERE q.session_id = s.id) \
         FROM integration.repo_import i \
         JOIN discovery.discovery_session s ON s.id = i.discovery_session_id \
         WHERE i.id = $1",
    )
    .bind(import_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(source, "SYSTEM", "the worker session must be SYSTEM-sourced");
    // unknowns→质询的映射由 Web 向导测试覆盖（web_session_is_created_with_unknowns_as_questions）；
    // Fake 模型仅在材料截断时产 unknowns，本 fixture 无截断 → 0 质询是合法输出。
    assert_eq!(question_count, 0, "no unknowns in this fixture, no questions expected");
    let _ = session_id;

    // 幂等：重跑 job 不再另开会话。
    run_import_job(&pool, &FakeRepoAnalysisModel, import_id).await.unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM discovery.discovery_session s \
         JOIN integration.repo_import i ON i.discovery_session_id = s.id WHERE i.id = $1",
    )
    .bind(import_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn apply_requires_revision_and_selection(pool: PgPool) {
    let (project_id, _, _repo) = seed_project_with_repo(&pool, "IMP-2").await;
    let (import_id, draft) = prepare_draft(&pool, project_id).await;
    let detail = pm_application::project_import::get_import(&pool, project_id, import_id)
        .await
        .unwrap();
    let mut req = apply_request(&draft, detail.draft_revision);
    req.draft_revision += 1;
    assert!(matches!(
        propose_import(&pool, project_id, import_id, req).await,
        Err(pm_application::project_import::ImportError::StaleDraft)
    ));

    let empty = ApplyRequest {
        draft_revision: detail.draft_revision,
        project: ApplyProject {
            code: "IMPORT-EMPTY".to_string(),
            name: "空选择".to_string(),
            description: String::new(),
        },
        milestone_keys: Vec::new(),
        requirement_keys: Vec::new(),
        task_keys: Vec::new(),
        write_knowledge_documents: false,
    };
    assert!(matches!(
        propose_import(&pool, project_id, import_id, empty).await,
        Err(pm_application::project_import::ImportError::NoSelection)
    ));
}

#[sqlx::test(migrations = "../../migrations")]
async fn approvals_execute_import_and_project_becomes_planning(pool: PgPool) {
    let knowledge = std::env::temp_dir().join(format!("ai-pm-import-docs-{}", Uuid::new_v4()));
    std::env::set_var("AI_PM_KNOWLEDGE_ROOT", &knowledge);
    let (project_id, _, _repo) = seed_project_with_repo(&pool, "IMP-3").await;
    let (import_id, draft) = prepare_draft(&pool, project_id).await;
    let detail = pm_application::project_import::get_import(&pool, project_id, import_id)
        .await
        .unwrap();
    let proposed = propose_import(
        &pool,
        project_id,
        import_id,
        apply_request(&draft, detail.draft_revision),
    )
    .await
    .unwrap();

    let owner: Uuid = sqlx::query_scalar(
        "SELECT principal_id FROM core.project_member WHERE project_id = $1 AND role = 'owner'",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let executor = PgCommandExecutor::new(pool.clone(), Arc::new(PolicyEngine::for_tests()));
    for proposal in &proposed.proposals {
        decide_approval(
            &pool,
            &executor,
            DecideRequest {
                approval_id: proposal.approval_id,
                actor_id: owner,
                decision: Decision::Approve,
                reason: "导入验收".to_string(),
                actor: None,
            },
        )
        .await
        .unwrap();
    }

    let status: String =
        sqlx::query_scalar("SELECT status FROM integration.repo_import WHERE id = $1")
            .bind(import_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "applied");
    let project_status: String = sqlx::query_scalar("SELECT status FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(project_status, "planning");

    let requirements: Vec<(String, String)> = sqlx::query_as(
        "SELECT code, status FROM delivery.requirement WHERE project_id = $1 ORDER BY code",
    )
    .bind(project_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(requirements.len(), draft.requirements.len());
    assert!(requirements.iter().all(|(_, status)| status == "draft"));

    let unassigned: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task WHERE project_id = $1 AND assignee_id IS NULL",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(unassigned, 0, "all imported tasks must resolve an agent");

    let docs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.project_document WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(docs as usize, draft.requirements.len() + 1);

    let audit: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
         WHERE project_id = $1 AND action = 'project_import_applied'",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit, 1);

    // 导入后 owner 仍可继续批量建需求：批量命令是红线，必须由人经审批通道执行。
    executor
        .execute(
            pm_domain::ProjectCommand::CreateRequirements {
                requirements: vec![RequirementDraft {
                    key: "R9".to_string(),
                    code: "REQ-900".to_string(),
                    title: "导入后追加需求".to_string(),
                    markdown: "# 追加\n".to_string(),
                    priority: "normal".to_string(),
                    confidence: Confidence::High,
                    evidence: Vec::new(),
                }],
            },
            pm_application::command_executor::ExecutionContext {
                project_id,
                actor: pm_application::actor::AuditActor::principal(owner),
                actor_kind: pm_application::command_executor::ActorKind::Human,
                source: pm_application::command_executor::CommandSource::RedlineApproval {
                    approval_id: Uuid::new_v4(),
                },
            },
        )
        .await
        .unwrap();
    let after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.requirement WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after as usize, draft.requirements.len() + 1);

    let _ = std::fs::remove_dir_all(knowledge);
}

#[sqlx::test(migrations = "../../migrations")]
async fn rejecting_one_approval_marks_import_rejected(pool: PgPool) {
    let (project_id, _, _repo) = seed_project_with_repo(&pool, "IMP-4").await;
    let (import_id, draft) = prepare_draft(&pool, project_id).await;
    let detail = pm_application::project_import::get_import(&pool, project_id, import_id)
        .await
        .unwrap();
    let proposed = propose_import(
        &pool,
        project_id,
        import_id,
        apply_request(&draft, detail.draft_revision),
    )
    .await
    .unwrap();
    let owner: Uuid = sqlx::query_scalar(
        "SELECT principal_id FROM core.project_member WHERE project_id = $1 AND role = 'owner'",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let executor = PgCommandExecutor::new(pool.clone(), Arc::new(PolicyEngine::for_tests()));
    decide_approval(
        &pool,
        &executor,
        DecideRequest {
            approval_id: proposed.proposals[0].approval_id,
            actor_id: owner,
            decision: Decision::Approve,
            reason: "ok".to_string(),
            actor: None,
        },
    )
    .await
    .unwrap();
    decide_approval(
        &pool,
        &executor,
        DecideRequest {
            approval_id: proposed.proposals[1].approval_id,
            actor_id: owner,
            decision: Decision::Reject,
            reason: "不需要".to_string(),
            actor: None,
        },
    )
    .await
    .unwrap();

    let status: String =
        sqlx::query_scalar("SELECT status FROM integration.repo_import WHERE id = $1")
            .bind(import_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "rejected");
    let project_status: String =
        sqlx::query_scalar("SELECT status FROM core.project WHERE id = $1")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(project_status, "initializing");
    // 已批准的里程碑不回滚。
    let milestones: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.milestone WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(milestones as usize, draft.milestones.len());
}

#[test]
fn grounding_drops_fabricated_evidence() {
    let repo_id = "repo-1".to_string();
    let facts = RepoFacts {
        head_commit_sha: "a".repeat(40),
        default_branch: "main".to_string(),
        commits: vec![CommitFact {
            sha: "a".repeat(40),
            author: "T".to_string(),
            authored_at: "2026-10-31T00:00:00Z".to_string(),
            subject: "feat: x".to_string(),
        }],
        hot_files: vec![HotFile { path: "a.rs".to_string(), touches: 1 }],
        languages: vec![LanguageStat { extension: "rs".to_string(), files: 1 }],
        file_count: 1,
        docs: vec![DocContent {
            path: "README.md".to_string(),
            bytes: 5,
            markdown: "# x\n".to_string(),
        }],
        truncated: false,
        size_bytes: 1,
    };
    let materials = vec![pm_application::repo_facts::RepoMaterial {
        repository_id: repo_id.clone(),
        facts,
    }];
    let good = Evidence {
        kind: EvidenceKind::Doc,
        repo_id: repo_id.clone(),
        r#ref: "README.md".to_string(),
        detail: String::new(),
    };
    let fabricated = Evidence {
        kind: EvidenceKind::Commit,
        repo_id: repo_id.clone(),
        r#ref: "deadbeef".to_string(),
        detail: String::new(),
    };
    let draft = ProjectDraft {
        requirements: vec![
            RequirementDraft {
                key: "R1".to_string(),
                code: "REQ-001".to_string(),
                title: "真实".to_string(),
                markdown: String::new(),
                priority: "high".to_string(),
                confidence: Confidence::High,
                evidence: vec![good],
            },
            RequirementDraft {
                key: "R2".to_string(),
                code: "REQ-002".to_string(),
                title: "编造".to_string(),
                markdown: String::new(),
                priority: "high".to_string(),
                confidence: Confidence::High,
                evidence: vec![fabricated],
            },
        ],
        tasks: vec![TaskDraft {
            key: "T1".to_string(),
            title: "编造任务".to_string(),
            description: String::new(),
            priority: "normal".to_string(),
            milestone_key: None,
            requirement_key: None,
            assignee_role: Some(AssigneeRole::Developer),
            confidence: Confidence::Medium,
            evidence: vec![],
        }],
        ..ProjectDraft::default()
    };
    let (grounded, dropped) = ground_draft(&draft, &materials);
    assert_eq!(dropped, 2);
    assert_eq!(grounded.requirements.len(), 1);
    assert!(grounded.tasks.is_empty());
    assert_eq!(grounded.requirements[0].code, "REQ-001");
}

/// 造一个**克隆必然失败**的仓库（URL 指向不存在的本地路径）。
async fn seed_broken_repo(pool: &PgPool, project_id: Uuid) -> Uuid {
    let actor_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) \
         VALUES ('human', 'broken-repo-owner', true) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let created = pm_application::repository::create_repository(
        pool,
        project_id,
        &pm_application::actor::AuditActor::principal(actor_id),
        pm_application::repository::CreateRepositoryRequest {
            name: "坏仓库".to_string(),
            // 端口 1 → 连接被立即拒绝，克隆快速失败（不依赖 DNS 或超时）。
            url: "ssh://git@127.0.0.1:1/team/missing.git".to_string(),
            role: Some("main".to_string()),
            default_branch: Some("main".to_string()),
            description: None,
        },
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE integration.project_repository SET status = 'verified' WHERE id = $1",
    )
    .bind(created.id)
    .execute(pool)
    .await
    .unwrap();
    created.id
}

/// 基线 §3.1 事实 #5 / 场景稿 §6：一个仓库失败**不中止**其余仓库。
#[sqlx::test(migrations = "../../migrations")]
async fn one_repository_failure_does_not_abort_the_others(pool: PgPool) {
    let (project_id, good_repo, _repo) = seed_project_with_repo(&pool, "PARTIAL-OK").await;
    let bad_repo = seed_broken_repo(&pool, project_id).await;

    let started = start_import(
        &pool,
        project_id,
        &pm_application::actor::AuditActor::dispatch(),
        &StartImportRequest {
            repository_ids: vec![good_repo, bad_repo],
            idempotency_key: None,
        },
    )
    .await
    .unwrap();

    run_import_job(&pool, &FakeRepoAnalysisModel, started.import_id)
        .await
        .unwrap();

    let status: String =
        sqlx::query_scalar("SELECT status FROM integration.repo_import WHERE id = $1")
            .bind(started.import_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "draft_ready", "a failing repository must not abort the import");

    // 失败仓留 failed Attempt；成功仓留 succeeded Attempt（R-4-1）。
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT repository_id, status FROM integration.repo_import_source_attempt \
         WHERE import_id = $1 ORDER BY repository_id",
    )
    .bind(started.import_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    let failed = rows
        .iter()
        .find(|(repo, _)| *repo == bad_repo)
        .map(|(_, s)| s.clone());
    let ok = rows
        .iter()
        .find(|(repo, _)| *repo == good_repo)
        .map(|(_, s)| s.clone());
    assert_eq!(
        failed.as_deref(),
        Some("failed"),
        "the bad repository must be recorded as failed"
    );
    assert_eq!(
        ok.as_deref(),
        Some("succeeded"),
        "the good repository must still be analyzed"
    );
}

/// 基线 §3.1 事实 #7：取消后 Worker 的阶段写入不得覆盖 cancelled。
#[sqlx::test(migrations = "../../migrations")]
async fn cancelled_import_is_not_resurrected_by_the_worker(pool: PgPool) {
    let (project_id, repository_id, _repo) = seed_project_with_repo(&pool, "CANCEL-OK").await;
    let started = start_import(
        &pool,
        project_id,
        &pm_application::actor::AuditActor::dispatch(),
        &StartImportRequest {
            repository_ids: vec![repository_id],
            idempotency_key: None,
        },
    )
    .await
    .unwrap();

    pm_application::project_import::cancel_import(
        &pool,
        project_id,
        started.import_id,
        &pm_application::actor::AuditActor::dispatch(),
        Some("user_abort"),
    )
    .await
    .unwrap();

    // Worker 继续跑：状态必须仍是 cancelled。
    let _ = run_import_job(&pool, &FakeRepoAnalysisModel, started.import_id).await;

    let status: String =
        sqlx::query_scalar("SELECT status FROM integration.repo_import WHERE id = $1")
            .bind(started.import_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        status, "cancelled",
        "the worker must not resurrect a cancelled import"
    );
}

/// 导入失败必须释放初始化名额（否则该 Project 再也发不起导入）。
#[sqlx::test(migrations = "../../migrations")]
async fn failed_import_releases_the_initialization_slot(pool: PgPool) {
    let (project_id, _repo_id, _repo) = seed_project_with_repo(&pool, "FAILSLOT").await;
    let bad_repo = seed_broken_repo(&pool, project_id).await;

    let started = start_import(
        &pool,
        project_id,
        &pm_application::actor::AuditActor::dispatch(),
        &StartImportRequest {
            repository_ids: vec![bad_repo],
            idempotency_key: None,
        },
    )
    .await
    .unwrap();
    run_import_job(&pool, &FakeRepoAnalysisModel, started.import_id)
        .await
        .unwrap();

    let status: String =
        sqlx::query_scalar("SELECT status FROM integration.repo_import WHERE id = $1")
            .bind(started.import_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "failed");

    let active: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.project_initialization_operation \
         WHERE import_id = $1 AND status = 'active'",
    )
    .bind(started.import_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(active, 0, "a failed import must release the initialization slot");

    // 名额释放后可以再次发起导入。
    let again = start_import(
        &pool,
        project_id,
        &pm_application::actor::AuditActor::dispatch(),
        &StartImportRequest {
            repository_ids: vec![bad_repo],
            idempotency_key: None,
        },
    )
    .await
    .unwrap();
    assert_ne!(again.import_id, started.import_id);
}

#[test]
fn all_fabricated_draft_is_ungrounded() {
    let (grounded, dropped) = ground_draft(&ProjectDraft::default(), &[]);
    assert_eq!(dropped, 0);
    assert!(grounded.requirements.is_empty());
    assert!(grounded.tasks.is_empty());
    assert_eq!(OrgChart::p0().roles.len(), 4);
}