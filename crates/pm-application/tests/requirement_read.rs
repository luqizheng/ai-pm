//! S02 · 需求域读面集成测试（评审队列 / 需求基线 / 变更影响分析）。
//!
//! 覆盖点：
//! - 评审队列只列 pending_confirmation 修订，带出前一正式版本（供并排比较）；
//! - 需求基线 = 已确认需求的当前正式修订集合（派生视图，非存储实体）；
//! - 影响分析列出谁绑定了本需求、绑的是哪一版、是否已过期。

use pm_application::actor::AuditActor;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::knowledge::KnowledgeRoot;
use pm_application::requirement::{
    self, ConfirmRevisionRequest, CreateRequirementRequest, DraftRevisionRequest,
};
use pm_application::requirement_read;
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    project_id: Uuid,
    human: AuditActor,
    human_id: Uuid,
    root: KnowledgeRoot,
}

async fn fixture(pool: &PgPool, code: &str) -> Fx {
    let boot = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: format!("s02-{code}"),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    Fx {
        project_id: boot.project_id,
        human: AuditActor::principal(boot.human_id),
        human_id: boot.human_id,
        root: KnowledgeRoot::new(
            std::env::temp_dir().join(format!("ai-pm-s02-{code}-{}", Uuid::new_v4())),
        ),
    }
}

/// 建需求 + 一版正文；`confirm` 决定是否走完确认。
async fn seed_requirement(pool: &PgPool, fx: &Fx, code: &str, confirm: bool) -> (Uuid, Uuid) {
    let entry = requirement::create_requirement(
        pool,
        SYSTEM_TENANT,
        fx.project_id,
        CreateRequirementRequest {
            code: code.to_string(),
            title: format!("需求 {code}"),
            description: None,
            requirement_type: Some("functional".to_string()),
            priority: Some("high".to_string()),
            markdown: Some(format!("# {code}\n")),
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
            markdown: format!("# {code} 正文\n"),
            summary: None,
            basis_finding_ids: None,
        },
        &fx.human,
    )
    .await
    .unwrap();
    requirement::submit_revision(pool, SYSTEM_TENANT, entry.id, draft.id, &fx.human, &fx.root)
        .await
        .unwrap();
    if confirm {
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
    }
    (entry.id, draft.id)
}

#[sqlx::test(migrations = "../../migrations")]
async fn review_queue_lists_pending_revisions_only(pool: PgPool) {
    let fx = fixture(&pool, "queue").await;
    let (confirmed_id, _) = seed_requirement(&pool, &fx, "REQ-A", true).await;
    let (pending_id, pending_rev) = seed_requirement(&pool, &fx, "REQ-B", false).await;

    let queue = requirement_read::project_review_queue(&pool, SYSTEM_TENANT, fx.project_id)
        .await
        .unwrap();

    assert_eq!(queue.proposals.len(), 1, "只有待确认修订进队列");
    assert_eq!(queue.proposals[0].requirement_id, pending_id);
    assert_eq!(queue.proposals[0].revision_id, pending_rev);
    assert_eq!(queue.proposals[0].state, "pending_confirmation");
    assert_eq!(
        queue.proposals[0].requirement_code.as_deref(),
        Some("REQ-B")
    );
    // 已确认的那条不在队列里。
    assert!(queue
        .proposals
        .iter()
        .all(|p| p.requirement_id != confirmed_id));
}

#[sqlx::test(migrations = "../../migrations")]
async fn requirement_baseline_lists_confirmed_current_revisions(pool: PgPool) {
    let fx = fixture(&pool, "base").await;
    let (confirmed_id, confirmed_rev) = seed_requirement(&pool, &fx, "REQ-C", true).await;
    seed_requirement(&pool, &fx, "REQ-D", false).await;

    let baseline =
        requirement_read::project_requirement_baseline(&pool, SYSTEM_TENANT, fx.project_id)
            .await
            .unwrap();

    assert_eq!(baseline.total_count, 2);
    assert_eq!(baseline.confirmed_count, 1);
    assert_eq!(baseline.unconfirmed_count, 1);

    let confirmed_row = baseline
        .items
        .iter()
        .find(|item| item.requirement_id == confirmed_id)
        .expect("已确认需求");
    assert_eq!(confirmed_row.revision_id, Some(confirmed_rev));
    assert_eq!(confirmed_row.state.as_deref(), Some("approved"));
    assert!(confirmed_row.confirmed_at.is_some(), "确认时间");

    let pending_row = baseline
        .items
        .iter()
        .find(|item| item.code.as_deref() == Some("REQ-D"))
        .expect("未确认需求也在基线表里（状态可辨）");
    assert_ne!(pending_row.state.as_deref(), Some("approved"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn requirement_impact_reports_design_and_task_bindings(pool: PgPool) {
    let fx = fixture(&pool, "impact").await;
    let (requirement_id, revision_id) = seed_requirement(&pool, &fx, "REQ-E", true).await;

    // 设计绑定本需求当前修订。
    sqlx::query(
        "INSERT INTO design.design (tenant_id, project_id, code, title, created_by) \
         VALUES ($1, $2, 'TD-001', '设计甲', $3)",
    )
    .bind(SYSTEM_TENANT)
    .bind(fx.project_id)
    .bind(fx.human_id)
    .execute(&pool)
    .await
    .unwrap();
    let design_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM design.design WHERE project_id = $1 AND code = 'TD-001'",
    )
    .bind(fx.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let design_rev: Uuid = sqlx::query_scalar(
        "INSERT INTO design.design_revision \
         (tenant_id, project_id, design_id, version, content, state, content_hash, created_by) \
         VALUES ($1, $2, $3, 1, '{}'::jsonb, 'draft', 'h1', $4) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .bind(fx.project_id)
    .bind(design_id)
    .bind(fx.human_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO design.design_requirement \
         (design_revision_id, design_id, requirement_id, requirement_revision_id, \
          tenant_id, project_id, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(design_rev)
    .bind(design_id)
    .bind(requirement_id)
    .bind(revision_id)
    .bind(SYSTEM_TENANT)
    .bind(fx.project_id)
    .bind(fx.human_id)
    .execute(&pool)
    .await
    .unwrap();

    // 任务绑定同一修订。
    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, priority, revision, tenant_id) \
         VALUES ($1, '任务甲', 'draft', 'normal', 1, $2) RETURNING id",
    )
    .bind(fx.project_id)
    .bind(SYSTEM_TENANT)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_requirement \
         (project_id, task_id, requirement_id, revision_id, tenant_id) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(fx.project_id)
    .bind(task_id)
    .bind(requirement_id)
    .bind(revision_id)
    .bind(SYSTEM_TENANT)
    .execute(&pool)
    .await
    .unwrap();

    let impact = requirement_read::requirement_impact(&pool, SYSTEM_TENANT, requirement_id)
        .await
        .unwrap();
    assert_eq!(impact.designs.len(), 1, "设计绑定");
    assert_eq!(impact.tasks.len(), 1, "任务绑定");
    assert_eq!(impact.designs[0].code.as_deref(), Some("TD-001"));
    assert_eq!(impact.tasks[0].title.as_deref(), Some("任务甲"));
    // 绑定的正是当前正式版本 → 无过期绑定。
    assert_eq!(impact.stale_binding_count, 0);
    assert_eq!(impact.designs[0].is_current, Some(true));
}
