//! DK-E：`kb.analysis.completed` → 自动开 `INITIAL_ONBOARDING` 访谈
//! （设计稿 §8.2 场景 2 首段）。
//!
//! 覆盖：开关 off 零动作；事件产物建会话 + `unknowns → questions`
//! （`why_blocking` 必填）；至少一次投递的幂等；已完成会话不复活。

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::discovery::{
    analysis_unknown_questions, ensure_onboarding_from_analysis, AutoOnboardingOutcome,
    ONBOARDING_SKIP_COMPLETED, ONBOARDING_SKIP_DISABLED,
};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    project_id: Uuid,
    revision_id: Uuid,
}

/// 项目 + 一份仓库快照修订（`analysis_mode` 可指定）+ 完成的审计行。
async fn seed(pool: &PgPool, analysis_mode: &str, truncated: bool) -> Fx {
    let project_id = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: "DKON".to_string(),
            project_name: "dk-onboarding".to_string(),
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
         (tenant_id, document_id, revision_no, source_commit_sha, content_hash, content, analysis_mode) \
         VALUES ($1, $2, 1, 'abc123', 'hash', '# snapshot', $3) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .bind(document_id)
    .bind(analysis_mode)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (project_id, actor_id, action, target_type, target_id, after_data, source_kind, tenant_id) \
         VALUES ($1, $2, 'kb_analysis_completed', 'kb_document_revision', $3, $4, 'kb_pc_analysis', $5)",
    )
    .bind(project_id)
    .bind(pm_application::run_dispatch::DISPATCH_SERVICE_PRINCIPAL)
    .bind(revision_id)
    .bind(serde_json::json!({ "truncated": truncated, "analysis_mode": analysis_mode }))
    .bind(SYSTEM_TENANT)
    .execute(pool)
    .await
    .unwrap();
    Fx {
        project_id,
        revision_id,
    }
}

async fn onboarding_counts(pool: &PgPool, project_id: Uuid) -> (i64, i64) {
    let sessions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM discovery.discovery_session \
         WHERE project_id = $1 AND kind = 'INITIAL_ONBOARDING'",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let questions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM discovery.discovery_question q \
         JOIN discovery.discovery_session s ON s.id = q.session_id \
         WHERE s.project_id = $1 AND s.kind = 'INITIAL_ONBOARDING'",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    (sessions, questions)
}

#[test]
fn unknowns_are_derived_from_analysis_confidence_and_coverage() {
    // 干净 LLM 分析：没有未知点（问题交给编排器按槽位缺口生成）。
    assert!(analysis_unknown_questions("llm", false).is_empty());
    // 低置信：一条；触顶：再加一条。
    let low = analysis_unknown_questions("derived", false);
    assert_eq!(low.len(), 1);
    assert!(!low[0].why_blocking.trim().is_empty());
    let both = analysis_unknown_questions("derived_fallback", true);
    assert_eq!(both.len(), 2);
    assert!(both.iter().all(|q| !q.question.trim().is_empty()));
    assert!(both.iter().all(|q| !q.why_blocking.trim().is_empty()));
}

/// 开关 off：零动作（不建会话、不写问题）——与编排器同一个开关。
#[sqlx::test(migrations = "../../migrations")]
async fn switch_off_writes_nothing(pool: PgPool) {
    let fx = seed(&pool, "derived", true).await;
    let outcome = ensure_onboarding_from_analysis(&pool, fx.project_id, fx.revision_id, false)
        .await
        .unwrap();
    assert_eq!(
        outcome,
        AutoOnboardingOutcome {
            session_id: None,
            created_session: false,
            questions_added: 0,
            skipped: Some(ONBOARDING_SKIP_DISABLED),
        }
    );
    assert_eq!(onboarding_counts(&pool, fx.project_id).await, (0, 0));
}

/// 事件产物：建 `SYSTEM` 来源的 `INITIAL_ONBOARDING` 会话 + unknowns→questions
/// （why_blocking 必填）；重投同一事件不重复。
#[sqlx::test(migrations = "../../migrations")]
async fn analysis_event_opens_interview_with_questions(pool: PgPool) {
    let fx = seed(&pool, "derived_fallback", true).await;
    let outcome = ensure_onboarding_from_analysis(&pool, fx.project_id, fx.revision_id, true)
        .await
        .unwrap();
    assert!(outcome.created_session);
    assert_eq!(outcome.questions_added, 2);
    assert_eq!(outcome.skipped, None);
    let session_id = outcome.session_id.unwrap();

    let (source, kind, status, created_by): (String, String, String, Uuid) = sqlx::query_as(
        "SELECT source, kind, status, created_by FROM discovery.discovery_session WHERE id = $1",
    )
    .bind(session_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(source, "SYSTEM");
    assert_eq!(kind, "INITIAL_ONBOARDING");
    assert_eq!(status, "IN_PROGRESS");
    assert_eq!(
        created_by,
        pm_application::run_dispatch::DISPATCH_SERVICE_PRINCIPAL,
        "system-opened interview is owned by the dispatch principal"
    );

    let rows: Vec<(String, Option<String>, String)> = sqlx::query_as(
        "SELECT question, why_blocking, status FROM discovery.discovery_question \
         WHERE session_id = $1 ORDER BY created_at, id",
    )
    .bind(session_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2);
    for (question, why_blocking, status) in &rows {
        assert!(!question.trim().is_empty());
        assert!(
            why_blocking
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
            "why_blocking is mandatory"
        );
        assert_eq!(status, "open");
    }

    // 至少一次投递：同一事件重放 → 复用会话、零新增问题。
    let replay = ensure_onboarding_from_analysis(&pool, fx.project_id, fx.revision_id, true)
        .await
        .unwrap();
    assert_eq!(replay.session_id, Some(session_id));
    assert!(!replay.created_session);
    assert_eq!(replay.questions_added, 0);
    assert_eq!(onboarding_counts(&pool, fx.project_id).await, (1, 2));
}

/// 已有进行中的访谈：复用（不建第二条会话），只补缺失的问题。
#[sqlx::test(migrations = "../../migrations")]
async fn existing_interview_is_reused(pool: PgPool) {
    let fx = seed(&pool, "derived", false).await;
    let human: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('human', 'owner') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let existing = pm_application::discovery::create_session(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        "WEB",
        "INITIAL_ONBOARDING",
        human,
    )
    .await
    .unwrap();
    let outcome = ensure_onboarding_from_analysis(&pool, fx.project_id, fx.revision_id, true)
        .await
        .unwrap();
    assert_eq!(outcome.session_id, Some(existing.id));
    assert!(!outcome.created_session);
    assert_eq!(outcome.questions_added, 1);
    assert_eq!(onboarding_counts(&pool, fx.project_id).await, (1, 1));
}

/// 最近一条访谈 `COMPLETED`：不复活（零会话、零问题）——新意图应显式重开。
#[sqlx::test(migrations = "../../migrations")]
async fn completed_interview_is_not_resurrected(pool: PgPool) {
    let fx = seed(&pool, "derived", true).await;
    let human: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('human', 'owner') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let existing = pm_application::discovery::create_session(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        "WEB",
        "INITIAL_ONBOARDING",
        human,
    )
    .await
    .unwrap();
    sqlx::query("UPDATE discovery.discovery_session SET status = 'COMPLETED' WHERE id = $1")
        .bind(existing.id)
        .execute(&pool)
        .await
        .unwrap();
    let outcome = ensure_onboarding_from_analysis(&pool, fx.project_id, fx.revision_id, true)
        .await
        .unwrap();
    assert_eq!(
        outcome,
        AutoOnboardingOutcome {
            session_id: None,
            created_session: false,
            questions_added: 0,
            skipped: Some(ONBOARDING_SKIP_COMPLETED),
        }
    );
    assert_eq!(onboarding_counts(&pool, fx.project_id).await, (1, 0));
}
