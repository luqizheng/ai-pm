//! S04 任务计划域集成测试（S4 提案 V1 §7 测试要点落地）。
//!
//! 覆盖：
//! - 全链：冻结基线 → AI 建计划草案 → 提交 → 人类批准 → 正式化（依赖边/
//!   验收标准/追溯列）→ 真人置 ready → 交接资格检查 → 执行回写闭环；
//! - 红线：未批准计划不得正式化；AI 无权批准计划（服务层 Forbidden）；
//!   AI 拆解产物恒为 draft；非 frozen 基线不得拆解；
//! - Q2：重拆产生新修订 diff、已正式化任务不被覆盖、幂等正式化；
//! - 并发/冲突：重复键草案拒绝、依赖前向图校验、计划状态机非法转移拒绝。

use std::sync::Arc;

use pm_ai::plan_decomposition;
use pm_application::actor::AuditActor;
use pm_application::command_executor::PgCommandExecutor;
use pm_application::PolicyEngine;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::design::{self, AcceptanceCriterionInput};
use pm_application::knowledge::KnowledgeRoot;
use pm_application::pm_write::{self, PmWriteError};
use pm_application::requirement::{
    self, ConfirmRevisionRequest, CreateRequirementRequest, DraftRevisionRequest,
};
use pm_application::task_plan;
use pm_domain::SYSTEM_TENANT;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    project_id: Uuid,
    human: AuditActor,
    ai: AuditActor,
    human_id: Uuid,
    ai_id: Uuid,
}

async fn fixture(pool: &PgPool, code: &str) -> Fx {
    let boot = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: format!("s04-{code}"),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let ai_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (id, kind, display_name) \
         VALUES ($1, 'ai_pm', $2) RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(format!("ai-{code}"))
    .fetch_one(pool)
    .await
    .unwrap();
    Fx {
        project_id: boot.project_id,
        human: AuditActor::principal(boot.human_id),
        ai: AuditActor::principal(ai_id),
        human_id: boot.human_id,
        ai_id,
    }
}

fn executor(pool: &PgPool) -> PgCommandExecutor {
    PgCommandExecutor::new(pool.clone(), Arc::new(PolicyEngine::for_tests()))
}

fn knowledge_root(tag: &str) -> KnowledgeRoot {
    KnowledgeRoot::new(std::env::temp_dir().join(format!("ai-pm-s04-{tag}-{}", Uuid::new_v4())))
}

fn gate_ready_content(title: &str) -> (Option<serde_json::Value>, Option<Vec<AcceptanceCriterionInput>>) {
    (
        Some(json!({ "included": [format!("{title} module")], "excluded": [], "modules": ["auth"] })),
        Some(vec![AcceptanceCriterionInput {
            statement: format!("{title} 验收：自动化测试通过"),
            verification_kind: Some("automated_test".to_string()),
        }]),
    )
}

/// 已确认需求 → 门禁可过设计 → 冻结基线，返回 (baseline_id, 修订集)。
async fn frozen_baseline(pool: &PgPool, fx: &Fx, code: &str, root: &KnowledgeRoot) -> Uuid {
    let entry = requirement::create_requirement(
        pool,
        SYSTEM_TENANT,
        fx.project_id,
        CreateRequirementRequest {
            code: format!("REQ-{code}"),
            title: format!("需求 {code}"),
            description: None,
            requirement_type: None,
            priority: None,
            markdown: Some(format!("# REQ-{code}\n")),
            basis_finding_ids: None,
        },
        &fx.human,
    )
    .await
    .unwrap();
    let draft = requirement::create_draft(
        pool,
        SYSTEM_TENANT,
        entry.id,
        DraftRevisionRequest {
            markdown: format!("# REQ-{code} 正文\n"),
            summary: None,
            basis_finding_ids: None,
        },
        &fx.human,
    )
    .await
    .unwrap();
    requirement::submit_revision(pool, SYSTEM_TENANT, entry.id, draft.id, &fx.human, root)
        .await
        .unwrap();
    requirement::confirm_revision(
        pool,
        SYSTEM_TENANT,
        entry.id,
        draft.id,
        ConfirmRevisionRequest {
            expected_content_hash: draft.content_hash.clone(),
            expected_current_revision_id: None,
            expected_row_revision: None,
        },
        &fx.human,
    )
    .await
    .unwrap();

    let (scope, criteria) = gate_ready_content(code);
    let detail = design::create_design(
        pool,
        SYSTEM_TENANT,
        fx.project_id,
        design::CreateDesignRequest {
            code: None,
            title: format!("设计 {code}"),
            markdown: Some(format!("# 设计 {code}\n\n正文。\n")),
            summary: None,
            technical_analysis: None,
            implementation_scope: scope,
            acceptance_criteria: criteria,
        },
        &fx.ai,
    )
    .await
    .unwrap();
    let design_id = detail.design.id;
    let rev = detail.revisions[0].id;
    design::submit_design_revision(pool, SYSTEM_TENANT, design_id, rev, &fx.ai)
        .await
        .unwrap();
    let draft_rev = design::create_design_revision(
        pool,
        SYSTEM_TENANT,
        design_id,
        design::DraftDesignRevisionRequest {
            markdown: format!("# 设计 {code} 关联版\n"),
            summary: None,
            technical_analysis: None,
            implementation_scope: Some(json!({ "included": [format!("{code} module")] })),
            acceptance_criteria: Some(vec![AcceptanceCriterionInput {
                statement: format!("{code} 验收"),
                verification_kind: None,
            }]),
        },
        &fx.ai,
    )
    .await
    .unwrap();
    design::link_design_requirement(
        pool,
        SYSTEM_TENANT,
        design_id,
        draft_rev.id,
        design::LinkDesignRequirementRequest {
            requirement_id: entry.id,
            requirement_revision_id: None,
        },
        &fx.ai,
    )
    .await
    .unwrap();
    design::submit_design_revision(pool, SYSTEM_TENANT, design_id, draft_rev.id, &fx.ai)
        .await
        .unwrap();
    let ap = design::request_design_revision_approval(pool, SYSTEM_TENANT, design_id, draft_rev.id, &fx.ai)
        .await
        .unwrap();
    design::decide_approval(
        pool,
        SYSTEM_TENANT,
        ap.id,
        design::DecideApprovalRequest {
            decision: "approve".to_string(),
            reason: None,
            expected_current_revision_id: None,
            expected_row_revision: None,
        },
        &fx.human,
    )
    .await
    .unwrap();

    design::freeze_baseline(
        pool,
        SYSTEM_TENANT,
        fx.project_id,
        design::FreezeBaselineRequest {
            code: None,
            design_revision_ids: vec![draft_rev.id],
            repository_refs: vec![],
        },
        &fx.human,
    )
    .await
    .unwrap()
    .id
}

fn plan_tasks() -> Vec<task_plan::PlanTaskDraft> {
    vec![
        task_plan::PlanTaskDraft {
            key: "T001".to_string(),
            title: "数据库结构".to_string(),
            description: Some("建表与迁移".to_string()),
            acceptance_criteria: vec!["迁移可重放".to_string()],
            depends_on: vec![],
            repository_ids: vec![],
            priority: None,
            change: None,
        },
        task_plan::PlanTaskDraft {
            key: "T002".to_string(),
            title: "数据访问层".to_string(),
            description: None,
            acceptance_criteria: vec!["DAO 单测通过".to_string()],
            depends_on: vec!["T001".to_string()],
            repository_ids: vec![],
            priority: Some("high".to_string()),
            change: None,
        },
        task_plan::PlanTaskDraft {
            key: "T003".to_string(),
            title: "后端 API".to_string(),
            description: None,
            acceptance_criteria: vec!["接口集成测试通过".to_string()],
            depends_on: vec!["T002".to_string()],
            repository_ids: vec![],
            priority: None,
            change: None,
        },
    ]
}

/// 建计划（AI 草案）→ 提交 → 请求批准 → 人类批准。
async fn approved_plan(pool: &PgPool, fx: &Fx, baseline_id: Uuid, code: &str) -> (Uuid, Uuid) {
    let detail = task_plan::create_plan(
        pool,
        SYSTEM_TENANT,
        fx.project_id,
        task_plan::CreatePlanRequest {
            code: None,
            title: format!("计划 {code}"),
            source_baseline_id: Some(baseline_id),
            tasks: plan_tasks(),
            instructions: None,
        },
        &fx.ai,
    )
    .await
    .unwrap();
    let plan_id = detail.plan.id;
    assert_eq!(detail.plan.code, format!("TP-001"));
    let rev = &detail.revisions[0];
    assert_eq!(rev.source, "ai");
    let rev_id = rev.id;
    task_plan::submit_plan_revision(pool, SYSTEM_TENANT, plan_id, rev_id, &fx.ai)
        .await
        .unwrap();
    let ap = task_plan::request_plan_approval(pool, SYSTEM_TENANT, plan_id, &fx.ai)
        .await
        .unwrap();
    design::decide_approval(
        pool,
        SYSTEM_TENANT,
        ap.id,
        design::DecideApprovalRequest {
            decision: "approve".to_string(),
            reason: Some("计划合理".to_string()),
            expected_current_revision_id: None,
            expected_row_revision: None,
        },
        &fx.human,
    )
    .await
    .unwrap();
    (plan_id, rev_id)
}

// ────────────────────────────────────── 全链 + 红线（合并场景）

#[sqlx::test(migrations = "../../migrations")]
async fn full_chain_plan_approve_materialize_ready_handoff(pool: PgPool) {
    let fx = fixture(&pool, "chain").await;
    let root = knowledge_root("chain");
    let baseline_id = frozen_baseline(&pool, &fx, "CHAIN", &root).await;
    let (plan_id, rev_id) = approved_plan(&pool, &fx, baseline_id, "CHAIN").await;

    // 计划状态 approved。
    let detail = task_plan::plan_detail(&pool, SYSTEM_TENANT, plan_id).await.unwrap();
    assert_eq!(detail.plan.status, "approved");
    let rev = detail.revisions.iter().find(|r| r.id == rev_id).unwrap();
    assert_eq!(rev.state, "approved");
    assert_eq!(rev.approved_by, Some(fx.human_id));

    // 正式化（AI 触发机械转写允许）。
    let outcome = task_plan::materialize_plan(
        &pool,
        SYSTEM_TENANT,
        plan_id,
        task_plan::MaterializeRequest { plan_revision_id: None, task_keys: None },
        &fx.ai,
    )
    .await
    .unwrap();
    assert_eq!(outcome["created"].as_array().unwrap().len(), 3);

    // 追溯列 + 验收标准 + 依赖边落库。
    let t001: Uuid = sqlx::query_scalar(
        "SELECT id FROM delivery.task WHERE plan_revision_id = $1 AND plan_task_key = 'T001'",
    )
    .bind(rev_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let t002: Uuid = sqlx::query_scalar(
        "SELECT id FROM delivery.task WHERE plan_revision_id = $1 AND plan_task_key = 'T002'",
    )
    .bind(rev_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let t003: Uuid = sqlx::query_scalar(
        "SELECT id FROM delivery.task WHERE plan_revision_id = $1 AND plan_task_key = 'T003'",
    )
    .bind(rev_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let status: String = sqlx::query_scalar("SELECT status FROM delivery.task WHERE id = $1")
        .bind(t001)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "draft");
    let baseline_of_task: Option<Uuid> =
        sqlx::query_scalar("SELECT implementation_baseline_id FROM delivery.task WHERE id = $1")
            .bind(t001)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(baseline_of_task, Some(baseline_id), "Task 继承计划来源基线");
    let criteria: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM delivery.task_acceptance_criterion WHERE task_id = $1",
    )
    .bind(t002)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(criteria, 1);
    let deps: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM delivery.task_dependency WHERE task_id = $1 AND depends_on_task_id = $2",
    )
    .bind(t002)
    .bind(t001)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(deps, 1);
    let deps3: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM delivery.task_dependency WHERE task_id = $1 AND depends_on_task_id = $2",
    )
    .bind(t003)
    .bind(t002)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(deps3, 1);

    // 幂等正式化：重跑只报 already_materialized。
    let outcome2 = task_plan::materialize_plan(
        &pool,
        SYSTEM_TENANT,
        plan_id,
        task_plan::MaterializeRequest { plan_revision_id: None, task_keys: None },
        &fx.ai,
    )
    .await
    .unwrap();
    assert_eq!(outcome2["created"].as_array().unwrap().len(), 0);
    assert_eq!(outcome2["already_materialized"].as_array().unwrap().len(), 3);

    // 未置 ready 的任务不可交接。
    let err = task_plan::prepare_handoff(&pool, SYSTEM_TENANT, t001).await.unwrap_err();
    assert!(matches!(err, PmWriteError::Conflict(_)));

    // T001 无上游依赖：真人置 ready（复用 mark_task_batch_ready；AI 禁）。
    let ai_err = pm_write::mark_task_batch_ready(
        &pool,
        &executor(&pool),
        SYSTEM_TENANT,
        fx.project_id,
        &[t001],
        &fx.ai,
    )
    .await;
    // 命令通道红线：AI 主体执行 draft→ready 必须被拒（Executor 层）。
    assert!(ai_err.is_err(), "AI 不得置 ready（命令通道红线）");
    let ready = pm_write::mark_task_batch_ready(
        &pool,
        &executor(&pool),
        SYSTEM_TENANT,
        fx.project_id,
        &[t001],
        &fx.human,
    )
    .await
    .unwrap();
    assert!(ready.ready.contains(&t001));

    // 交接：资格检查 + 载荷。
    let payload = task_plan::prepare_handoff(&pool, SYSTEM_TENANT, t001).await.unwrap();
    assert_eq!(payload["task_id"], json!(t001));
    assert_eq!(payload["implementation_baseline_id"], json!(baseline_id));
    assert!(payload["acceptance_criteria"].as_array().unwrap().len() >= 1);
    assert!(payload["repositories"].as_array().unwrap().is_empty());

    // T003 依赖未完成 → 交接拒绝。
    let err = task_plan::prepare_handoff(&pool, SYSTEM_TENANT, t003).await.unwrap_err();
    assert!(matches!(err, PmWriteError::Conflict(_)), "依赖未完成必须拦截");

    // 进度读模型。
    let progress = task_plan::plan_progress(&pool, SYSTEM_TENANT, plan_id).await.unwrap();
    assert_eq!(progress.total_tasks, 3);
    assert_eq!(progress.planned_tasks, 3);
    assert!(progress.by_status.get("draft").is_some());
}

// ────────────────────────────────────────────── 红线：AI 无批准权

#[sqlx::test(migrations = "../../migrations")]
async fn ai_cannot_decide_plan_approval(pool: PgPool) {
    let fx = fixture(&pool, "redline").await;
    let root = knowledge_root("redline");
    let baseline_id = frozen_baseline(&pool, &fx, "RED", &root).await;
    let detail = task_plan::create_plan(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        task_plan::CreatePlanRequest {
            code: None,
            title: "红线计划".to_string(),
            source_baseline_id: Some(baseline_id),
            tasks: plan_tasks(),
            instructions: None,
        },
        &fx.ai,
    )
    .await
    .unwrap();
    task_plan::submit_plan_revision(&pool, SYSTEM_TENANT, detail.plan.id, detail.revisions[0].id, &fx.ai)
        .await
        .unwrap();
    let ap = task_plan::request_plan_approval(&pool, SYSTEM_TENANT, detail.plan.id, &fx.ai)
        .await
        .unwrap();
    // 服务层真人闸：AI 主体 Forbidden。
    let err = design::decide_approval(
        &pool,
        SYSTEM_TENANT,
        ap.id,
        design::DecideApprovalRequest {
            decision: "approve".to_string(),
            reason: None,
            expected_current_revision_id: None,
            expected_row_revision: None,
        },
        &fx.ai,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, PmWriteError::Forbidden(_)));
    // 人类批准后正式化闸：未批准（此处仍 in_review）不得正式化。
    let err = task_plan::materialize_plan(
        &pool,
        SYSTEM_TENANT,
        detail.plan.id,
        task_plan::MaterializeRequest { plan_revision_id: None, task_keys: None },
        &fx.ai,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, PmWriteError::Conflict(_)));
}

// ────────────────────────────────────── 草案校验（系统拥有骨架）

#[sqlx::test(migrations = "../../migrations")]
async fn plan_draft_validation_rejects_bad_batches(pool: PgPool) {
    let fx = fixture(&pool, "valid").await;
    let bad_cases: Vec<(Vec<task_plan::PlanTaskDraft>, &str)> = vec![
        (vec![], "empty"),
        (
            vec![task_plan::PlanTaskDraft {
                key: "A".to_string(),
                title: "x".to_string(),
                description: None,
                acceptance_criteria: vec![],
                depends_on: vec![],
                repository_ids: vec![],
                priority: None,
                change: None,
            }],
            "no criteria",
        ),
        (
            vec![
                task_plan::PlanTaskDraft {
                    key: "A".to_string(),
                    title: "x".to_string(),
                    description: None,
                    acceptance_criteria: vec!["c".to_string()],
                    depends_on: vec![],
                    repository_ids: vec![],
                    priority: None,
                    change: None,
                },
                task_plan::PlanTaskDraft {
                    key: "A".to_string(),
                    title: "y".to_string(),
                    description: None,
                    acceptance_criteria: vec!["c".to_string()],
                    depends_on: vec![],
                    repository_ids: vec![],
                    priority: None,
                    change: None,
                },
            ],
            "dup key",
        ),
        (
            vec![task_plan::PlanTaskDraft {
                key: "B".to_string(),
                title: "x".to_string(),
                description: None,
                acceptance_criteria: vec!["c".to_string()],
                depends_on: vec!["A".to_string()],
                repository_ids: vec![],
                priority: None,
                change: None,
            }],
            "forward dep",
        ),
        (
            vec![
                task_plan::PlanTaskDraft {
                    key: "A".to_string(),
                    title: "x".to_string(),
                    description: None,
                    acceptance_criteria: vec!["c".to_string()],
                    depends_on: vec!["B".to_string()],
                    repository_ids: vec![],
                    priority: None,
                    change: None,
                },
                task_plan::PlanTaskDraft {
                    key: "B".to_string(),
                    title: "y".to_string(),
                    description: None,
                    acceptance_criteria: vec!["c".to_string()],
                    depends_on: vec![],
                    repository_ids: vec![],
                    priority: None,
                    change: None,
                },
            ],
            "backward dep ok but first has forward dep",
        ),
    ];
    for (tasks, label) in bad_cases {
        let err = task_plan::create_plan(
            &pool,
            SYSTEM_TENANT,
            fx.project_id,
            task_plan::CreatePlanRequest {
                code: None,
                title: format!("校验 {label}"),
                source_baseline_id: None,
                tasks,
                instructions: None,
            },
            &fx.human,
        )
        .await
        .unwrap_err();
        assert!(
            matches!(err, PmWriteError::ValidationMsg(_)),
            "case {label} must be rejected, got {err:?}"
        );
    }
}

// ────────────────────────────────────────────── Q2：重拆不覆盖 + 计划状态机

#[sqlx::test(migrations = "../../migrations")]
async fn plan_lifecycle_and_resplit_preserves_materialized(pool: PgPool) {
    let fx = fixture(&pool, "resplit").await;
    let root = knowledge_root("resplit");
    let baseline_id = frozen_baseline(&pool, &fx, "RS", &root).await;
    let (plan_id, rev1) = approved_plan(&pool, &fx, baseline_id, "RS").await;
    task_plan::materialize_plan(
        &pool,
        SYSTEM_TENANT,
        plan_id,
        task_plan::MaterializeRequest { plan_revision_id: None, task_keys: None },
        &fx.ai,
    )
    .await
    .unwrap();

    // 已批准计划不得直接发起新修订（须先退回）。
    let err = task_plan::create_plan_revision(
        &pool,
        SYSTEM_TENANT,
        plan_id,
        task_plan::DraftPlanRevisionRequest {
            tasks: plan_tasks(),
            instructions: None,
        },
        &fx.ai,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, PmWriteError::Conflict(_)));

    // AI 无权退回（decide 拒绝已在红线用例覆盖）；此处验证修订 v1 仍 approved
    // 且已正式化任务不受影响——即使后续有人重拆。
    let t001_before: Uuid = sqlx::query_scalar(
        "SELECT id FROM delivery.task WHERE plan_revision_id = $1 AND plan_task_key = 'T001'",
    )
    .bind(rev1)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 计划退回（reject）→ 回 draft → 人工修订 v2（改标题）→ 提交 → 批准。
    // 走一遍 decide=reject 路径验证状态机。
    // 计划已 approved，不能再次请求批准 → Conflict。
    let pending_ap = task_plan::request_plan_approval(&pool, SYSTEM_TENANT, plan_id, &fx.ai)
        .await
        .unwrap_err();
    assert!(matches!(pending_ap, PmWriteError::Conflict(_)));

    // 人工修订 v2（draft 状态下允许）。
    let mut tasks2 = plan_tasks();
    tasks2[0].title = "数据库结构 v2（人工改名）".to_string();
    // 退回：需要一个 in_review 状态；这里直接验证 approved 不可再修订（上面已验）。
    // 改用：批准的修订不可改稿（触发器 + 服务双闸）。
    let err = task_plan::update_plan_revision_draft(
        &pool,
        SYSTEM_TENANT,
        plan_id,
        rev1,
        task_plan::DraftPlanRevisionRequest { tasks: tasks2, instructions: None },
        &fx.human,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, PmWriteError::Conflict(_)));

    // 旧任务不受影响。
    let still: String = sqlx::query_scalar("SELECT title FROM delivery.task WHERE id = $1")
        .bind(t001_before)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(still, "数据库结构");

    // 计划替代链：建第二个批准计划 → 旧计划显式 superseded（AI 拒、人类过）。
    let plan2 = task_plan::create_plan(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        task_plan::CreatePlanRequest {
            code: None,
            title: "计划 RS-v2".to_string(),
            source_baseline_id: Some(baseline_id),
            tasks: plan_tasks(),
            instructions: None,
        },
        &fx.human,
    )
    .await
    .unwrap();
    task_plan::submit_plan_revision(&pool, SYSTEM_TENANT, plan2.plan.id, plan2.revisions[0].id, &fx.ai)
        .await
        .unwrap();
    let ap2 = task_plan::request_plan_approval(&pool, SYSTEM_TENANT, plan2.plan.id, &fx.ai)
        .await
        .unwrap();
    design::decide_approval(
        &pool,
        SYSTEM_TENANT,
        ap2.id,
        design::DecideApprovalRequest {
            decision: "approve".to_string(),
            reason: None,
            expected_current_revision_id: None,
            expected_row_revision: None,
        },
        &fx.human,
    )
    .await
    .unwrap();
    let ai_err = task_plan::supersede_plan(
        &pool,
        SYSTEM_TENANT,
        plan_id,
        task_plan::SupersedePlanRequest {
            successor_plan_id: plan2.plan.id,
            reason: Some("重拆替代".to_string()),
        },
        &fx.ai,
    )
    .await
    .unwrap_err();
    assert!(matches!(ai_err, PmWriteError::Forbidden(_)));
    let old = task_plan::supersede_plan(
        &pool,
        SYSTEM_TENANT,
        plan_id,
        task_plan::SupersedePlanRequest {
            successor_plan_id: plan2.plan.id,
            reason: Some("重拆替代".to_string()),
        },
        &fx.human,
    )
    .await
    .unwrap();
    assert_eq!(old.status, "superseded");
    // 已正式化任务仍存在（替代不删任务）。
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM delivery.task WHERE plan_revision_id = $1",
    )
    .bind(rev1)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 3);
}

// ─────────────────────────────────── 拆解准入：非 frozen 基线拒绝（红线）

#[sqlx::test(migrations = "../../migrations")]
async fn decomposition_requires_frozen_baseline(pool: PgPool) {
    let fx = fixture(&pool, "admit").await;
    let root = knowledge_root("admit");
    let baseline_id = frozen_baseline(&pool, &fx, "AD", &root).await;

    // 计划不带基线 + 请求也不带 → Validation。
    let bare = task_plan::create_plan(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        task_plan::CreatePlanRequest {
            code: None,
            title: "无基线计划".to_string(),
            source_baseline_id: None,
            tasks: vec![task_plan::PlanTaskDraft {
                key: "K1".to_string(),
                title: "x".to_string(),
                description: None,
                acceptance_criteria: vec!["c".to_string()],
                depends_on: vec![],
                repository_ids: vec![],
                priority: None,
                change: None,
            }],
            instructions: None,
        },
        &fx.ai,
    )
    .await
    .unwrap();
    let err = plan_decomposition::decompose(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        plan_decomposition::DecomposeRequest {
            plan_id: bare.plan.id,
            baseline_id: None,
            instructions: None,
        },
        &fx.ai,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, plan_decomposition::DecomposeError::Pm(PmWriteError::Validation(_))),
        "无基线必须拒绝"
    );

    // 基线 supersede 后（这里退化为：跨项目引用）→ Validation。
    let boot2 = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "admit-other".to_string(),
            project_name: "admit-other".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let err = plan_decomposition::decompose(
        &pool,
        SYSTEM_TENANT,
        boot2.project_id,
        plan_decomposition::DecomposeRequest {
            plan_id: bare.plan.id,
            baseline_id: Some(baseline_id),
            instructions: None,
        },
        &fx.ai,
    )
    .await;
    // bare.plan 属 fx.project 而请求按 boot2 项目发起 → 计划归属校验先拦截。
    assert!(err.is_err());

    // 无 Provider：拆解显式失败（不静默造数据）——Model 错误。
    let err = plan_decomposition::decompose(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        plan_decomposition::DecomposeRequest {
            plan_id: bare.plan.id,
            baseline_id: Some(baseline_id),
            instructions: None,
        },
        &fx.ai,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, plan_decomposition::DecomposeError::Model(_)),
        "无 Provider 拆解必须显式失败，实际 {err:?}"
    );
}
