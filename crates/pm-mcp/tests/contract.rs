//! Slice D2 MCP 层契约测试
//! （`docs/ai-pm-slice-d2-mcp-context-v1.md` §七：C3/C4/C5/C6/C11/C18/C24/C25）。
//!
//! 这一层只验证**工具形状与错误契约**：字段值来自真实库，`ToolError` 映射到
//! `isError` 文本；DTO 正确性由 pm-application 的 `run_context.rs` 覆盖。

#![allow(dead_code)]

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::pm_write::{
    update_task, AcceptanceCriterionInput, DeliverableInput, KnowledgeRefInput, UpdateTaskRequest,
};
use pm_domain::SYSTEM_TENANT;
use pm_mcp::tools::{
    claim_task, get_run_context, report_progress, submit_result, submit_result_with, ClaimTaskArgs,
    KnowledgeProposalInput, ReportProgressArgs, RunContextArgs, SubmitResultArgs,
    SubmittedRepository, ToolError,
};
use serde::Deserialize;
use serde_json::Value;
use sqlx::PgPool;
use std::path::Path;
use uuid::Uuid;

/// T-088：直调工具函数用的测试调用人（SYSTEM_TENANT 成员；设备声明缺席）。
fn test_caller(human_id: Uuid) -> pm_application::caller::CallerContext {
    pm_application::caller::CallerContext::new(
        pm_application::caller::HumanActor {
            principal_id: human_id,
            tenant_id: SYSTEM_TENANT,
            identity_id: None,
            client: Some("test".into()),
            auth_mode: "api_key".into(),
        },
        pm_application::caller::DeviceClaim::absent(),
    )
}

/// 在工作区里执行 git 命令（D4 校验跑在真实 Git 工作区上）。
fn git(root: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .status()
        .expect("git must be installed for the D4 MCP tests");
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

struct Fx {
    project_id: Uuid,
    task_id: Uuid,
    agent_id: Uuid,
    document_id: Uuid,
    human_id: Uuid,
}

async fn seed(pool: &PgPool, code: &str, with_contract: bool) -> Fx {
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
    let agent_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('dev_agent', 'executor') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, assignee_id, task_type, tenant_id) \
         VALUES ($1, '确定地形渲染架构', 'ready', 1, $2, 'architecture', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(outcome.project_id)
    .bind(agent_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let repo_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.project_repository (project_id, name, url, status, head_commit_sha, tenant_id) \
         VALUES ($1, 'world-view', $2, 'verified', 'headsha', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(outcome.project_id)
    .bind(format!("git@host:{code}"))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_repository (task_id, project_id, repository_id, write_access, base_ref, path_scope, tenant_id) \
         VALUES ($1, $2, $3, true, 'main', ARRAY['crates/terrain/**'], \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(task_id)
    .bind(outcome.project_id)
    .bind(repo_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 1, 'queued', (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(outcome.project_id)
    .bind(task_id)
    .execute(pool)
    .await
    .unwrap();

    let document_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.project_document (project_id, doc_key, doc_type, canonical_path, tenant_id) \
         VALUES ($1, 'architecture.terrain', 'architecture', 'architecture/terrain.md', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(outcome.project_id)
    .fetch_one(pool)
    .await
    .unwrap();

    if with_contract {
        let author: Uuid = sqlx::query_scalar(
            "INSERT INTO core.principal (kind, display_name) VALUES ('ai_pm', 'doc-author') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        for revision_no in [1, 2] {
            sqlx::query(
                "INSERT INTO ai.document_revision \
                 (project_id, document_id, revision_no, git_commit_sha, content_hash, status, created_by, tenant_id) \
                 VALUES ($1, $2, $3, $4, $5, 'published', $6, \
                         (SELECT tenant_id FROM core.project WHERE id = $1))",
            )
            .bind(outcome.project_id)
            .bind(document_id)
            .bind(revision_no)
            .bind(format!("sha-{revision_no}"))
            .bind(format!("hash-{revision_no}"))
            .bind(author)
            .execute(pool)
            .await
            .unwrap();
        }
        update_task(
            pool,
            task_id,
            UpdateTaskRequest {
                revision: Some(1),
                title: Some("确定地形渲染架构".to_string()),
                description: Some("比较 GPU Shader 与 CPU 预计算".to_string()),
                status: Some("ready".to_string()),
                priority: Some("normal".to_string()),
                assignee_id: Some(agent_id),
                goal: Some(Some("确定 Bevy 2D 地形高度晕渲方案".to_string())),
                constraints: Some(serde_json::json!({
                    "language": "Rust", "engine": "Bevy 0.19", "map_scale": "regional"
                })),
                deliverables: Some(vec![DeliverableInput {
                    id: Some("ADR-TERRAIN".to_string()),
                    path: "docs/adr/001-terrain-rendering.md".to_string(),
                }]),
                acceptance_criteria: Some(vec![
                    AcceptanceCriterionInput {
                        ordinal: Some(20),
                        text: "cargo test 全绿".to_string(),
                        verification_kind: Some("automated_test".to_string()),
                    },
                    AcceptanceCriterionInput {
                        ordinal: Some(10),
                        text: "比较 GPU Shader 和 CPU 预计算".to_string(),
                        verification_kind: Some("review".to_string()),
                    },
                ]),
                knowledge_refs: Some(vec![KnowledgeRefInput {
                    document_id,
                    is_required: true,
                }]),
            },
            &pm_application::actor::AuditActor::principal(author),
        )
        .await
        .unwrap();
    }

    Fx {
        project_id: outcome.project_id,
        task_id,
        agent_id,
        document_id,
        human_id: outcome.human_id,
    }
}

async fn claim(pool: &PgPool, fx: &Fx) -> Value {
    claim_task(
        pool,
        ClaimTaskArgs {
            project_id: fx.project_id,
            task_id: fx.task_id,
            client_name: "codex".to_string(),
            client_version: Some("1.2.0".to_string()),
        },
        &test_caller(fx.human_id),
    )
    .await
    .unwrap()
}

/// C4：`#[serde(deny_unknown_fields)]` 镜像契约——未知字段会直接反序列化失败。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContractMirror {
    contract_version: i32,
    run: RunMirror,
    task: TaskMirror,
    /// I4-E：项目正式 Goal 快照（未建立 → null）。
    goal: Option<GoalMirror>,
    acceptance_criteria: Vec<CriterionMirror>,
    repositories: Vec<RepositoryMirror>,
    knowledge_refs: Vec<KnowledgeRefMirror>,
    policy: PolicyMirror,
}

/// I4-E：Goal 快照镜像（身份与版本锚点，正文经知识读取路径获取）。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GoalMirror {
    document_id: Uuid,
    revision_id: Uuid,
    revision_no: i32,
    content_hash: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunMirror {
    run_id: Uuid,
    attempt_no: i32,
    stage: Option<String>,
    timeout_seconds: Option<i32>,
    lease: LeaseMirror,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LeaseMirror {
    expires_at: String,
    heartbeat_interval_secs: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskMirror {
    task_id: Uuid,
    title: String,
    task_type: Option<String>,
    status: String,
    revision: i64,
    priority: String,
    goal: Option<String>,
    description: Option<String>,
    constraints: Value,
    deliverables: Vec<DeliverableMirror>,
    git_branch: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeliverableMirror {
    id: String,
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CriterionMirror {
    id: Uuid,
    ordinal: i32,
    text: String,
    verification_kind: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepositoryMirror {
    repository_id: Uuid,
    name: String,
    base_ref: String,
    base_commit: String,
    write_access: bool,
    workspace_path: String,
    path_scope: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct KnowledgeRefMirror {
    document_id: Uuid,
    doc_key: String,
    doc_type: String,
    canonical_path: String,
    revision_no: Option<i32>,
    is_required: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyMirror {
    approval_required: bool,
    approval_status: String,
    execution_enabled: bool,
    snapshot_scope: String,
}

#[sqlx::test(migrations = "../../migrations")]
async fn c3_c4_c24_c25_contract_shape_and_values(pool: PgPool) {
    let fx = seed(&pool, "D2-MCP-C3", true).await;
    let claimed = claim(&pool, &fx).await;
    let args = RunContextArgs {
        project_id: fx.project_id,
        run_id: claimed["run_id"].as_str().unwrap().parse().unwrap(),
        lease_token: claimed["lease_token"].as_str().unwrap().to_string(),
        lease_generation: claimed["lease_generation"].as_i64().unwrap() as i32,
    };
    let value = get_run_context(&pool, args, &test_caller(fx.human_id))
        .await
        .unwrap();

    // C4：schema 校验（deny_unknown_fields 同时保证未知字段为 0）。
    let mirror: ContractMirror = serde_json::from_value(value.clone())
        .unwrap_or_else(|err| panic!("contract shape drifted: {err}: {value}"));
    assert_eq!(mirror.contract_version, 1);
    assert_eq!(mirror.run.timeout_seconds, Some(1800));
    assert_eq!(
        mirror.task.goal.as_deref(),
        Some("确定 Bevy 2D 地形高度晕渲方案")
    );
    // I4-E：本 fixture 未建立 Goal → 显式 null（缺失是信号，不是错误）。
    assert!(
        mirror.goal.is_none(),
        "goal must be an explicit null when absent"
    );
    assert_eq!(mirror.acceptance_criteria.len(), 2);
    assert_eq!(
        mirror.acceptance_criteria[0].ordinal, 10,
        "C25: ordinal ascending"
    );
    assert_eq!(mirror.acceptance_criteria[1].ordinal, 20);
    assert_eq!(mirror.repositories.len(), 1);
    assert_eq!(mirror.repositories[0].name, "world-view");
    assert_eq!(mirror.knowledge_refs.len(), 1);
    assert_eq!(mirror.knowledge_refs[0].document_id, fx.document_id);
    assert_eq!(
        mirror.knowledge_refs[0].revision_no,
        Some(2),
        "C24: live resolution returns the newest revision"
    );
    assert!(mirror.policy.execution_enabled);
}

#[sqlx::test(migrations = "../../migrations")]
async fn c18_empty_contract_is_still_schema_valid(pool: PgPool) {
    let fx = seed(&pool, "D2-MCP-C18", false).await;
    let claimed = claim(&pool, &fx).await;
    let value = get_run_context(
        &pool,
        RunContextArgs {
            project_id: fx.project_id,
            run_id: claimed["run_id"].as_str().unwrap().parse().unwrap(),
            lease_token: claimed["lease_token"].as_str().unwrap().to_string(),
            lease_generation: claimed["lease_generation"].as_i64().unwrap() as i32,
        },
        &test_caller(fx.human_id),
    )
    .await
    .unwrap();
    assert_eq!(value["acceptance_criteria"], serde_json::json!([]));
    assert_eq!(value["task"]["goal"], Value::Null);
    let _: ContractMirror = serde_json::from_value(value).expect("empty contract is schema-valid");
}

#[sqlx::test(migrations = "../../migrations")]
async fn c5_c6_errors_are_tool_errors_without_leaking_credentials(pool: PgPool) {
    let fx = seed(&pool, "D2-MCP-C6", false).await;
    let claimed = claim(&pool, &fx).await;
    let run_id: Uuid = claimed["run_id"].as_str().unwrap().parse().unwrap();
    let lease_token = claimed["lease_token"].as_str().unwrap().to_string();
    let other = seed(&pool, "D2-MCP-C6-OTHER", false).await;

    // C5：跨项目 → NotFound（不泄露存在性）。
    let err = get_run_context(
        &pool,
        RunContextArgs {
            project_id: other.project_id,
            run_id,
            lease_token: lease_token.clone(),
            lease_generation: 1,
        },
        &test_caller(fx.human_id),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::NotFound(_)));

    // C6：错 token → Forbidden，文本不含 run_id / hash / owner。
    let err = get_run_context(
        &pool,
        RunContextArgs {
            project_id: fx.project_id,
            run_id,
            lease_token: "wrong-token".to_string(),
            lease_generation: 1,
        },
        &test_caller(fx.human_id),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Forbidden(_)));
    let text = err.to_string();
    assert!(!text.contains(&run_id.to_string()));
    assert!(!text.contains(&lease_token));
    assert!(!text.contains("mcp:codex"));

    // 租约过期 → LeaseExpired。
    sqlx::query(
        "UPDATE delivery.task_run SET lease_expires_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(run_id)
    .execute(&pool)
    .await
    .unwrap();
    let err = get_run_context(
        &pool,
        RunContextArgs {
            project_id: fx.project_id,
            run_id,
            lease_token,
            lease_generation: 1,
        },
        &test_caller(fx.human_id),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::LeaseExpired(_)));
}

/// C11（D4 定稿后重写）：`pm_submit_result` **真实落库**——D2 的 shape-first
/// 语义作废。本用例覆盖「上报仓不在任务契约」这条驳回路径：整单驳回、0 行
/// commit、任务状态不动、驳回审计持久。
#[sqlx::test(migrations = "../../migrations")]
async fn c11_submit_result_rejects_a_repository_outside_the_contract(pool: PgPool) {
    let fx = seed(&pool, "D4-MCP-C11", true).await;
    let claimed = claim(&pool, &fx).await;
    let run_id: Uuid = claimed["run_id"].as_str().unwrap().parse().unwrap();

    let before_task: (String, i64) =
        sqlx::query_as("SELECT t.status, t.revision FROM delivery.task t WHERE t.id = $1")
            .bind(fx.task_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    let value = submit_result(
        &pool,
        SubmitResultArgs {
            project_id: fx.project_id,
            run_id,
            lease_token: claimed["lease_token"].as_str().unwrap().to_string(),
            lease_generation: claimed["lease_generation"].as_i64().unwrap() as i32,
            integration_status: None,
            repositories: vec![SubmittedRepository {
                repository_id: Uuid::new_v4(),
                base_commit: "headsha".to_string(),
                commit_sha: "newsha".to_string(),
                changed_files: 12,
                tests: vec![],
            }],
            summary: "done".to_string(),
            knowledge_proposals: vec![KnowledgeProposalInput {
                document_id: fx.document_id,
                change_summary: "update terrain ADR".to_string(),
            }],
        },
        &test_caller(fx.human_id),
    )
    .await
    .unwrap();

    assert_eq!(value["outcome"], "rejected");
    assert_eq!(value["code"], "repo_not_in_contract");
    assert_eq!(value["submitted_repository_count"], 1);

    let commits: i64 = sqlx::query_scalar("SELECT count(*) FROM delivery.task_repository_commit")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        commits, 0,
        "a rejected submission must not land any commit row"
    );

    let after_task: (String, i64) =
        sqlx::query_as("SELECT t.status, t.revision FROM delivery.task t WHERE t.id = $1")
            .bind(fx.task_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        after_task, before_task,
        "a rejected submission must not move the task"
    );

    let rejected_audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log WHERE action = 'delivery.rejected'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rejected_audits, 1, "the rejection must be auditable");
}

/// C84 / C85（D4 定稿）：`pm_submit_result` **真实落库**的端到端路径。
///
/// C84：成功提交 → 交付段落两表落库（`task_repository_commit` + `task_run.
/// integration_status`）、任务 `running→submitted`、`delivery.submitted` 审计。
/// C85：成功提交后重复请求 → 返回原结果（幂等 200），不产生第二份审计。
#[sqlx::test(migrations = "../../migrations")]
async fn c84_c85_submit_result_lands_the_delivery_and_replays(pool: PgPool) {
    let fx = seed(&pool, "D4-MCP-C84", true).await;

    // 交付校验跑在**实际 Git 工作区**上：先造出工作区与基线 commit，再领取
    // Run（领取时的基线快照必须等于工作区的初始 HEAD）。
    let repository_id: Uuid =
        sqlx::query_scalar("SELECT repository_id FROM delivery.task_repository WHERE task_id = $1")
            .bind(fx.task_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let workspace_path: String =
        pm_application::run_dispatch::workspace_path_for(repository_id, "world-view");
    let workspace_root = std::env::temp_dir().join(format!("ai-pm-d4-mcp-{}", Uuid::new_v4()));

    // Run 的 id 在领取后才知道——先领取，再按 run_id 落工作区。
    let claimed = claim(&pool, &fx).await;
    let run_id: Uuid = claimed["run_id"].as_str().unwrap().parse().unwrap();
    let lease_token = claimed["lease_token"].as_str().unwrap().to_string();
    let lease_generation = claimed["lease_generation"].as_i64().unwrap() as i32;

    let worktree = workspace_root
        .join(run_id.to_string())
        .join(&workspace_path);
    std::fs::create_dir_all(&worktree).unwrap();
    git(&worktree, &["init", "-q", "-b", "main"]);
    std::fs::write(worktree.join("README.md"), "# base\n").unwrap();
    git(&worktree, &["add", "."]);
    git(
        &worktree,
        &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "base"],
    );
    let base_commit = head_sha(&worktree);

    // 领取快照的基线是 `task_repository.base_commit`——把它对齐到工作区 HEAD，
    // 否则库层防篡改（基线比对）会先于 git 校验驳回。
    sqlx::query("UPDATE delivery.task_repository SET base_commit = $2 WHERE task_id = $1")
        .bind(fx.task_id)
        .bind(&base_commit)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE delivery.task_run_repository SET base_commit = $2 WHERE run_id = $1")
        .bind(run_id)
        .bind(&base_commit)
        .execute(&pool)
        .await
        .unwrap();

    // 落一个新 commit 作为交付结果。
    std::fs::write(worktree.join("change.md"), "changed\n").unwrap();
    git(&worktree, &["add", "."]);
    git(
        &worktree,
        &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "change"],
    );
    let commit_sha = head_sha(&worktree);

    // 执行器首报 `started` → 任务 `claimed → running`。交付迁移的合法边是
    // `running → submitted`（D1 状态机 E1 守卫）。
    report_progress(
        &pool,
        ReportProgressArgs {
            project_id: fx.project_id,
            run_id,
            lease_token: lease_token.clone(),
            lease_generation,
            stage: "started".to_string(),
            note: None,
            blocked: false,
        },
        &test_caller(fx.human_id),
    )
    .await
    .unwrap();

    let options = pm_application::delivery::DeliveryOptions {
        workspace_base: workspace_root.clone(),
        git_timeout: std::time::Duration::from_secs(30),
    };
    let args = SubmitResultArgs {
        project_id: fx.project_id,
        run_id,
        lease_token: lease_token.clone(),
        lease_generation,
        integration_status: Some("passed".to_string()),
        repositories: vec![SubmittedRepository {
            repository_id,
            base_commit: base_commit.clone(),
            commit_sha: commit_sha.clone(),
            changed_files: 1,
            tests: vec![],
        }],
        summary: "done".to_string(),
        knowledge_proposals: vec![],
    };

    // C84：成功提交 → 真实落库。
    let value = submit_result_with(&pool, args.clone(), &options, &test_caller(fx.human_id))
        .await
        .unwrap();
    assert_eq!(value["outcome"], "accepted");
    assert_eq!(value["integration_status"], "passed");
    assert_eq!(value["commit_count"], 1);

    let commits: i64 = sqlx::query_scalar("SELECT count(*) FROM delivery.task_repository_commit")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(commits, 1, "the accepted submission must land a commit row");

    let stored_integration: String =
        sqlx::query_scalar("SELECT integration_status FROM delivery.task_run WHERE id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored_integration, "passed");

    let task_status: String = sqlx::query_scalar("SELECT status FROM delivery.task WHERE id = $1")
        .bind(fx.task_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        task_status, "submitted",
        "the task must advance to submitted"
    );

    let submitted_audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log WHERE action = 'delivery.submitted'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(submitted_audits, 1);

    // C85：重复请求 → 幂等返回原结果，不产生第二份审计。
    let replay = submit_result_with(&pool, args, &options, &test_caller(fx.human_id))
        .await
        .unwrap();
    assert_eq!(replay["outcome"], "accepted");
    assert_eq!(replay["commit_count"], 1);
    let commits_after: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.task_repository_commit")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        commits_after, 1,
        "the replay must not land a second commit row"
    );
    let audits_after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log WHERE action = 'delivery.submitted'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        audits_after, 1,
        "the replay must not produce a second audit"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn report_progress_tool_renews_and_reports(pool: PgPool) {
    let fx = seed(&pool, "D2-MCP-RP", false).await;
    let claimed = claim(&pool, &fx).await;
    let run_id: Uuid = claimed["run_id"].as_str().unwrap().parse().unwrap();
    let lease_token = claimed["lease_token"].as_str().unwrap().to_string();
    let value = report_progress(
        &pool,
        ReportProgressArgs {
            project_id: fx.project_id,
            run_id,
            lease_token,
            lease_generation: 1,
            stage: "started".to_string(),
            note: Some("located the renderer".to_string()),
            blocked: false,
        },
        &test_caller(fx.human_id),
    )
    .await
    .unwrap();
    assert_eq!(value["task_status"], "running");
    assert_eq!(value["status"], "running");
    assert_eq!(value["stage"], "started");
    assert!(value["lease_expires_at"].is_string());
}
