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
use pm_mcp::tools::{
    claim_task, get_run_context, report_progress, submit_result, ClaimTaskArgs, KnowledgeProposalInput,
    ReportProgressArgs, RunContextArgs, SubmitResultArgs, SubmittedRepository, ToolError,
};
use serde::Deserialize;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;
use pm_domain::SYSTEM_TENANT;

struct Fx {
    project_id: Uuid,
    task_id: Uuid,
    agent_id: Uuid,
    document_id: Uuid,
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
    acceptance_criteria: Vec<CriterionMirror>,
    repositories: Vec<RepositoryMirror>,
    knowledge_refs: Vec<KnowledgeRefMirror>,
    policy: PolicyMirror,
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
    };
    let value = get_run_context(&pool, args).await.unwrap();

    // C4：schema 校验（deny_unknown_fields 同时保证未知字段为 0）。
    let mirror: ContractMirror = serde_json::from_value(value.clone())
        .unwrap_or_else(|err| panic!("contract shape drifted: {err}: {value}"));
    assert_eq!(mirror.contract_version, 1);
    assert_eq!(mirror.run.timeout_seconds, Some(1800));
    assert_eq!(mirror.task.goal.as_deref(), Some("确定 Bevy 2D 地形高度晕渲方案"));
    assert_eq!(mirror.acceptance_criteria.len(), 2);
    assert_eq!(mirror.acceptance_criteria[0].ordinal, 10, "C25: ordinal ascending");
    assert_eq!(mirror.acceptance_criteria[1].ordinal, 20);
    assert_eq!(mirror.repositories.len(), 1);
    assert_eq!(mirror.repositories[0].name, "world-view");
    assert_eq!(mirror.knowledge_refs.len(), 1);
    assert_eq!(
        mirror.knowledge_refs[0].document_id,
        fx.document_id
    );
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
        },
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
        },
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
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Forbidden(_)));
    let text = err.to_string();
    assert!(!text.contains(&run_id.to_string()));
    assert!(!text.contains(&lease_token));
    assert!(!text.contains("mcp:codex"));

    // 租约过期 → LeaseExpired。
    sqlx::query("UPDATE delivery.task_run SET lease_expires_at = now() - interval '1 second' WHERE id = $1")
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
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::LeaseExpired(_)));
}

#[sqlx::test(migrations = "../../migrations")]
async fn c11_submit_result_is_shape_first_and_touches_no_table(pool: PgPool) {
    let fx = seed(&pool, "D2-MCP-C11", true).await;
    let claimed = claim(&pool, &fx).await;
    let run_id: Uuid = claimed["run_id"].as_str().unwrap().parse().unwrap();

    async fn counts(pool: &PgPool) -> (i64, i64, i64, i64, i64) {
        let mut out = (0i64, 0i64, 0i64, 0i64, 0i64);
        for (index, table) in [
            "delivery.task_run",
            "delivery.task_run_repository",
            "governance.audit_log",
            "integration.inbound_event",
            "delivery.task_run_failure",
        ]
        .into_iter()
        .enumerate()
        {
            let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
                .fetch_one(pool)
                .await
                .unwrap();
            match index {
                0 => out.0 = count,
                1 => out.1 = count,
                2 => out.2 = count,
                3 => out.3 = count,
                _ => out.4 = count,
            }
        }
        out
    }

    let before_counts = counts(&pool).await;
    let before_task: (String, i64, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT t.status, t.revision, r.lease_owner, r.lease_token_hash \
           FROM delivery.task t JOIN delivery.task_run r ON r.task_id = t.id WHERE t.id = $1",
    )
    .bind(fx.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let err = submit_result(SubmitResultArgs {
        project_id: fx.project_id,
        run_id,
        lease_token: claimed["lease_token"].as_str().unwrap().to_string(),
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
    })
    .unwrap_err();
    assert!(matches!(err, ToolError::Unsupported(_)));
    let text = err.to_string();
    assert!(text.contains(r#""implemented_in":"D4""#), "{text}");
    assert!(text.contains(r#""schema":"draft""#), "{text}");

    assert_eq!(counts(&pool).await, before_counts, "shape-first touches no table");
    let after_task: (String, i64, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT t.status, t.revision, r.lease_owner, r.lease_token_hash \
           FROM delivery.task t JOIN delivery.task_run r ON r.task_id = t.id WHERE t.id = $1",
    )
    .bind(fx.task_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after_task, before_task, "submit_result did not mutate task/lease state");
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
            stage: "started".to_string(),
            note: Some("located the renderer".to_string()),
            blocked: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(value["task_status"], "running");
    assert_eq!(value["status"], "running");
    assert_eq!(value["stage"], "started");
    assert!(value["lease_expires_at"].is_string());
}