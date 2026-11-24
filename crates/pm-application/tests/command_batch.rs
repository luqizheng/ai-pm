//! 冷启动批量命令（CreateMilestones / CreateRequirements / CreateTasks /
//! WriteDocuments）的执行器集成测试。

use std::sync::Arc;

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::command_executor::{
    ActorKind, CommandSource, ExecutionContext, PgCommandExecutor,
};
use pm_application::PolicyEngine;
use pm_domain::import::{AssigneeRole, DocumentDraft, MilestoneDraft, RequirementDraft, TaskDraft};
use pm_domain::proposal::Confidence;
use pm_domain::ProjectCommand;
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

struct Fixture {
    project_id: Uuid,
    human_id: Uuid,
}

async fn seed(pool: &PgPool, code: &str) -> Fixture {
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
    Fixture {
        project_id: outcome.project_id,
        human_id: outcome.human_id,
    }
}

fn executor(pool: &PgPool) -> PgCommandExecutor {
    PgCommandExecutor::new(pool.clone(), Arc::new(PolicyEngine::for_tests()))
}

fn ctx(fx: &Fixture) -> ExecutionContext {
    // The four batch commands are red lines (`project_owner` in the policy):
    // only a human coming through the red-line approval flow may execute them.
    ExecutionContext {
        project_id: fx.project_id,
        actor: pm_application::actor::AuditActor::principal(fx.human_id),
        actor_kind: ActorKind::Human,
        source: CommandSource::RedlineApproval {
            approval_id: Uuid::new_v4(),
        },
    }
}

fn milestone(key: &str, name: &str) -> MilestoneDraft {
    MilestoneDraft {
        key: key.to_string(),
        name: name.to_string(),
        planned_at: None,
    }
}

fn requirement(key: &str, code: &str) -> RequirementDraft {
    RequirementDraft {
        key: key.to_string(),
        code: code.to_string(),
        title: format!("{code} title"),
        markdown: format!("# {code}"),
        priority: "high".to_string(),
        confidence: Confidence::High,
        evidence: Vec::new(),
    }
}

fn task(
    key: &str,
    title: &str,
    milestone: Option<&str>,
    req: Option<&str>,
    role: Option<AssigneeRole>,
) -> TaskDraft {
    TaskDraft {
        key: key.to_string(),
        title: title.to_string(),
        description: String::new(),
        priority: "normal".to_string(),
        milestone_key: milestone.map(str::to_string),
        requirement_key: req.map(str::to_string),
        assignee_role: role,
        confidence: Confidence::High,
        evidence: Vec::new(),
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_milestones_batch_is_idempotent(pool: PgPool) {
    let fx = seed(&pool, "BATCH-M").await;
    let executor = executor(&pool);
    let command = ProjectCommand::CreateMilestones {
        milestones: vec![milestone("M1", "核心地形"), milestone("M2", "编辑器")],
    };
    executor.execute(command.clone(), ctx(&fx)).await.unwrap();
    executor.execute(command, ctx(&fx)).await.unwrap();

    let names: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM delivery.milestone WHERE project_id = $1 ORDER BY name",
    )
    .bind(fx.project_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(names.len(), 2);
    assert!(names.iter().any(|n| n == "核心地形"), "{names:?}");
    assert!(names.iter().any(|n| n == "编辑器"), "{names:?}");

    let audit: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT after_data FROM governance.audit_log \
         WHERE project_id = $1 AND action = 'create_milestones' ORDER BY created_at DESC LIMIT 1",
    )
    .bind(fx.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit.unwrap()["items"].as_array().unwrap().len(), 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_requirements_batch_writes_draft_revisions(pool: PgPool) {
    let fx = seed(&pool, "BATCH-R").await;
    let executor = executor(&pool);
    executor
        .execute(
            ProjectCommand::CreateRequirements {
                requirements: vec![requirement("R1", "REQ-001"), requirement("R2", "REQ-002")],
            },
            ctx(&fx),
        )
        .await
        .unwrap();

    // 0039/S02D-04：requirement 级 draft 态废除——导入建行即 active。
    // S02-I2 §6.6 债务清偿：导入不再自动设 current 指针（「current 非空 ⇒
    // approved + Confirmation」不变量成立）；「不自动批准」= current IS NULL
    // 且 v1 修订仍为 draft（投影绑定随人类 submit 发生，S02-I2）。
    let rows: Vec<(String, String, Option<Uuid>, String)> = sqlx::query_as(
        "SELECT r.code, r.lifecycle_status, r.current_revision_id, \
                (SELECT v.state FROM delivery.requirement_revision v \
                  WHERE v.requirement_id = r.id AND v.version = 1) AS v1_state \
         FROM delivery.requirement r WHERE r.project_id = $1 ORDER BY r.code",
    )
    .bind(fx.project_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2);
    for (code, lifecycle_status, current, v1_state) in rows {
        assert_eq!(lifecycle_status, "active", "{code}");
        assert!(
            current.is_none(),
            "{code}: import must not move the pointer"
        );
        assert_eq!(v1_state, "draft", "{code}");
    }

    // 幂等：同 code 再来一次不新增行。
    executor
        .execute(
            ProjectCommand::CreateRequirements {
                requirements: vec![requirement("R1", "REQ-001")],
            },
            ctx(&fx),
        )
        .await
        .unwrap();
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.requirement WHERE project_id = $1")
            .bind(fx.project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_tasks_resolves_links_and_agents(pool: PgPool) {
    let fx = seed(&pool, "BATCH-T").await;
    // "未解析角色" 需要真的没有对应成员：bootstrap 默认会编制 architect，
    // 所以先移除其项目授权行，才能覆盖 findings 里的 assignee_unresolved 分支。
    sqlx::query(
        "DELETE FROM ai.project_agent_member WHERE project_id = $1 AND role_kind = 'architect'",
    )
    .bind(fx.project_id)
    .execute(&pool)
    .await
    .unwrap();
    let executor = executor(&pool);
    executor
        .execute(
            ProjectCommand::CreateMilestones {
                milestones: vec![milestone("M1", "核心地形")],
            },
            ctx(&fx),
        )
        .await
        .unwrap();
    executor
        .execute(
            ProjectCommand::CreateRequirements {
                requirements: vec![requirement("R1", "REQ-001")],
            },
            ctx(&fx),
        )
        .await
        .unwrap();
    executor
        .execute(
            ProjectCommand::CreateTasks {
                tasks: vec![
                    task(
                        "T1",
                        "实现加载器",
                        Some("核心地形"),
                        Some("REQ-001"),
                        Some(AssigneeRole::Developer),
                    ),
                    task(
                        "T2",
                        "未知角色任务",
                        None,
                        None,
                        Some(AssigneeRole::Architect),
                    ),
                ],
            },
            ctx(&fx),
        )
        .await
        .unwrap();

    let developer_id: Uuid = sqlx::query_scalar(
        "SELECT principal_id FROM ai.project_agent_member WHERE project_id = $1 AND role_kind = 'developer'",
    )
    .bind(fx.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (milestone_id, assignee_id): (Option<Uuid>, Option<Uuid>) = sqlx::query_as(
        "SELECT milestone_id, assignee_id FROM delivery.task \
         WHERE project_id = $1 AND title = '实现加载器'",
    )
    .bind(fx.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(milestone_id.is_some());
    assert_eq!(assignee_id, Some(developer_id));

    // S02-I2 §6.6 债务清偿 + I1-D07：导入需求未确认（current NULL）→
    // 不建立执行依据，findings 显式 requirement_unresolved；确认后（I4 改绑）
    // 才可链接。反向覆盖：SQL 造已确认需求 → create_tasks 仍正常链接。
    let linked: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_requirement tr \
         JOIN delivery.task t ON t.id = tr.task_id WHERE t.title = '实现加载器'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        linked, 0,
        "unconfirmed import must not create an execution basis"
    );

    let confirmed_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement (project_id, code, title, lifecycle_status, tenant_id) \
         VALUES ($1, 'REQ-OK', 'confirmed', 'active', \
                 (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(fx.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let confirmed_rev: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement_revision \
         (requirement_id, project_id, version, content, state, content_hash, created_by, tenant_id) \
         VALUES ($1, $2, 1, '{}', 'approved', 'hash', $3, \
                 (SELECT tenant_id FROM core.project WHERE id = $2)) RETURNING id",
    )
    .bind(confirmed_id)
    .bind(fx.project_id)
    .bind(fx.human_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE delivery.requirement SET current_revision_id = $2 WHERE id = $1")
        .bind(confirmed_id)
        .bind(confirmed_rev)
        .execute(&pool)
        .await
        .unwrap();
    executor
        .execute(
            ProjectCommand::CreateTasks {
                tasks: vec![task(
                    "T3",
                    "已确认依据",
                    None,
                    Some("REQ-OK"),
                    Some(AssigneeRole::Developer),
                )],
            },
            ctx(&fx),
        )
        .await
        .unwrap();
    let linked_confirmed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_requirement tr \
         JOIN delivery.task t ON t.id = tr.task_id WHERE t.title = '已确认依据'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(linked_confirmed, 1, "confirmed requirement still links");

    let unknown_assignee: Option<Uuid> = sqlx::query_scalar(
        "SELECT assignee_id FROM delivery.task WHERE project_id = $1 AND title = '未知角色任务'",
    )
    .bind(fx.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(unknown_assignee.is_none());

    let audit: serde_json::Value = sqlx::query_scalar(
        "SELECT after_data FROM governance.audit_log \
         WHERE project_id = $1 AND action = 'create_tasks'",
    )
    .bind(fx.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let findings = audit["findings"].as_array().unwrap();
    assert!(findings.iter().any(|f| f["kind"] == "assignee_unresolved"));
    assert!(
        findings
            .iter()
            .any(|f| f["kind"] == "requirement_unconfirmed"),
        "unconfirmed import must surface requirement_unresolved: {findings:?}"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn write_documents_batch_commits_and_compensates(pool: PgPool) {
    let root = std::env::temp_dir().join(format!("ai-pm-batch-docs-{}", Uuid::new_v4()));
    std::env::set_var("AI_PM_KNOWLEDGE_ROOT", &root);
    let fx = seed(&pool, "BATCH-D").await;
    let executor = executor(&pool);

    executor
        .execute(
            ProjectCommand::WriteDocuments {
                documents: vec![
                    DocumentDraft {
                        doc_key: "architecture".to_string(),
                        doc_type: "architecture".to_string(),
                        markdown: "# 架构\n".to_string(),
                        change_summary: None,
                    },
                    DocumentDraft {
                        doc_key: "decision".to_string(),
                        doc_type: "decision".to_string(),
                        markdown: "# 决策\n".to_string(),
                        change_summary: None,
                    },
                ],
            },
            ctx(&fx),
        )
        .await
        .unwrap();

    let docs: Vec<(String, String)> = sqlx::query_as(
        "SELECT d.doc_key, d.canonical_path FROM ai.project_document d WHERE d.project_id = $1",
    )
    .bind(fx.project_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(docs.len(), 2);
    for (_, path) in &docs {
        assert!(root.join(path).exists(), "missing file {path}");
    }
    let revisions: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ai.document_revision WHERE project_id = $1")
            .bind(fx.project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(revisions, 2);

    // 第二篇非法类型 → 第一篇被补偿删除（只保留此前已存在的文档）。
    let err = executor
        .execute(
            ProjectCommand::WriteDocuments {
                documents: vec![
                    DocumentDraft {
                        doc_key: "fresh".to_string(),
                        doc_type: "architecture".to_string(),
                        markdown: "# fresh\n".to_string(),
                        change_summary: None,
                    },
                    DocumentDraft {
                        doc_key: "bad".to_string(),
                        doc_type: "unknown".to_string(),
                        markdown: "# bad\n".to_string(),
                        change_summary: None,
                    },
                ],
            },
            ctx(&fx),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, pm_domain::CommandError::Validation(_)),
        "{err:?}"
    );
    let fresh: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM ai.project_document WHERE project_id = $1 AND doc_key = 'fresh'",
    )
    .bind(fx.project_id)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(fresh.is_none(), "fresh document must be compensated");
    let remaining: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ai.project_document WHERE project_id = $1")
            .bind(fx.project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(remaining, 2);

    let _ = std::fs::remove_dir_all(root);
}
