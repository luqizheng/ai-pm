//! DK-E：`kb.analysis.completed` 消费端接线验（handler → 自动开访谈）。
//!
//! 应用层行为已在 `pm-application/tests/discovery_auto_onboarding.rs` 钉住；
//! 本文件只验「同一个 outbox 事件真的会驱动它」与「开关关掉时事件仍完成回写、
//! 但不建会话」（任务完成回写不受开访谈失败影响）。

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_domain::SYSTEM_TENANT;
use pm_infrastructure::kb_pc_handlers::KbAnalysisCompletedHandler;
use pm_infrastructure::outbox::ClaimedOutboxEvent;
use pm_infrastructure::outbox_worker::OutboxHandler;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

async fn seed(pool: &PgPool) -> (Uuid, Uuid) {
    let project_id = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: "DKH".to_string(),
            project_name: "dk-handler".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap()
    .project_id;
    let document_id: Uuid = sqlx::query_scalar(
        "INSERT INTO kb.document (tenant_id, project_id, doc_key, doc_type, canonical_path) \
         VALUES ($1, $2, 'repo-snapshot', 'knowledge', 'repos/x/repository-snapshot.md') \
         RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO kb.document_revision \
         (tenant_id, document_id, revision_no, content_hash, content, analysis_mode) \
         VALUES ($1, $2, 1, 'hash', '# snapshot', 'derived_fallback') RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .bind(document_id)
    .fetch_one(pool)
    .await
    .unwrap();
    (project_id, revision_id)
}

fn event(project_id: Uuid, revision_id: Uuid) -> ClaimedOutboxEvent {
    ClaimedOutboxEvent {
        id: Uuid::new_v4(),
        project_id,
        event_type: "kb.analysis.completed".to_string(),
        aggregate_type: "intake_task".to_string(),
        aggregate_id: Uuid::new_v4().to_string(),
        payload: json!({
            "document_ref": {
                "target_domain": "knowledge_base",
                "target_type": "document",
                "target_id": Uuid::new_v4().to_string(),
                "target_revision_id": revision_id.to_string(),
            },
            "conclusion": "COMPLETED",
            "task_ref": {
                "target_domain": "requirement",
                "target_type": "intake_task",
                "target_id": Uuid::new_v4().to_string(),
                "target_revision_id": null,
            },
        }),
        attempts: 0,
        schema_version: 1,
        idempotency_key: format!("kb_analysis:{revision_id}"),
    }
}

async fn onboarding_sessions(pool: &PgPool, project_id: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM discovery.discovery_session \
         WHERE project_id = $1 AND kind = 'INITIAL_ONBOARDING'",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 开关 on：事件消费后自动开访谈；开关 off：同一事件零会话。
#[sqlx::test(migrations = "../../migrations")]
async fn the_analysis_completed_event_opens_the_interview_only_when_switched_on(
    pool: PgPool,
) {
    let (project_id, revision_id) = seed(&pool).await;

    let off = KbAnalysisCompletedHandler::with_auto_onboarding(pool.clone(), false);
    off.handle(&event(project_id, revision_id)).await.unwrap();
    assert_eq!(onboarding_sessions(&pool, project_id).await, 0);

    let on = KbAnalysisCompletedHandler::with_auto_onboarding(pool.clone(), true);
    on.handle(&event(project_id, revision_id)).await.unwrap();
    assert_eq!(onboarding_sessions(&pool, project_id).await, 1);

    let questions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM discovery.discovery_question q \
         JOIN discovery.discovery_session s ON s.id = q.session_id \
         WHERE s.project_id = $1 AND s.kind = 'INITIAL_ONBOARDING'",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(questions, 1, "derived_fallback → one low-confidence question");
}
