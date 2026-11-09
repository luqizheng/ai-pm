//! WP-0 · 内容管线接缝：KB 文档修订确认（draft → published）集成测试。
//!
//! 覆盖（WP-0 裁决验收七例 + 补充）：
//! - 真人（`auth_mode=interactive`）确认 → draft→published + 确认人/时间落库；
//! - **同事务 outbox**：`kb.document_revision.published` 入 `integration.outbox_event`
//!   （类型 + 幂等键 `kb_document_revision_published:{revision_id}` + payload 四元组引用）；
//! - 审计四身份列全写（`source_kind='kb_service'`）；
//! - 幂等：已发布再次确认 → 200 且**不改原确认人与确认时间**，**不重复发事件**；
//! - AI 主体 403；`api_key` 模式 403（可信人类交互门）；非 interactive 的真人 403；
//! - Revision 不属于该 Document → 404；跨项目 Revision → 404（防枚举）；
//! - rejected 修订 → 409（不删生命周期语义，只是不交付操作能力）。

use pm_application::actor::AuditActor;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::kb_service::document::{self, ConfirmOutcome};
use pm_application::kb_service::KbServiceError;
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    project_id: Uuid,
    tenant_id: Uuid,
    /// 真人 + 可信交互上下文（`auth_mode=interactive`）。
    human: AuditActor,
    /// 另一位真人（用于验证幂等重确认**不改原确认人**）。
    human2: AuditActor,
    /// AI 主体（无认证身份）。
    ai: AuditActor,
    /// 真人主体但 `auth_mode=api_key`（Agent/Service 形态）。
    api_key: AuditActor,
}

async fn principal(pool: &PgPool, kind: &str, name: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ($1, $2) RETURNING id",
    )
    .bind(kind)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn identity(pool: &PgPool, principal_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.principal_identity (provider, tenant_id, subject, principal_id) \
         VALUES ('lmms', $1, $2, $3) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .bind(Uuid::new_v4().to_string())
    .bind(principal_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn interactive_human(pool: &PgPool, name: &str) -> AuditActor {
    let pid = principal(pool, "human", name).await;
    let iid = identity(pool, pid).await;
    AuditActor::identified(pid, iid, Some("web".into()), Some("interactive".into()))
}

async fn seed(pool: &PgPool, code: &str) -> Fx {
    let project_id = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap()
    .project_id;

    let human = interactive_human(pool, "reviewer-a").await;
    let human2 = interactive_human(pool, "reviewer-b").await;
    let ai_pid = principal(pool, "ai_pm", "ai-pm").await;
    let api_pid = principal(pool, "human", "key-user").await;
    let api_iid = identity(pool, api_pid).await;

    Fx {
        project_id,
        tenant_id: SYSTEM_TENANT,
        human,
        human2,
        ai: AuditActor::principal(ai_pid),
        api_key: AuditActor::identified(
            api_pid,
            api_iid,
            Some("codex".into()),
            Some("api_key".into()),
        ),
    }
}

/// 种一篇文档 + 一个修订，返回 `(document_id, revision_id)`。
async fn seed_revision(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    doc_key: &str,
    status: &str,
) -> (Uuid, Uuid) {
    let document_id: Uuid = sqlx::query_scalar(
        "INSERT INTO kb.document (tenant_id, project_id, doc_key, doc_type, canonical_path) \
         VALUES ($1, $2, $3, 'knowledge', $4) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(doc_key)
    .bind(format!("kb/{doc_key}.md"))
    .fetch_one(pool)
    .await
    .unwrap();

    let revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO kb.document_revision \
         (tenant_id, document_id, revision_no, content_hash, status, content, created_by) \
         VALUES ($1, $2, 1, 'hash-1', $3, '# doc', $4) RETURNING id",
    )
    .bind(tenant_id)
    .bind(document_id)
    .bind(status)
    .bind(Uuid::new_v4())
    .fetch_one(pool)
    .await
    .unwrap();

    (document_id, revision_id)
}

async fn outbox_rows(pool: &PgPool, project_id: Uuid) -> Vec<(String, serde_json::Value, String)> {
    sqlx::query_as(
        "SELECT event_type, payload, idempotency_key FROM integration.outbox_event \
          WHERE project_id = $1 AND event_type = 'kb.document_revision.published' \
          ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .unwrap()
}

// ── 正向路径 ───────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn confirm_publishes_writes_confirmer_and_emits_event_in_same_transaction(pool: PgPool) {
    let fx = seed(&pool, "WP0-CONFIRM-OK").await;
    let (document_id, revision_id) =
        seed_revision(&pool, fx.tenant_id, fx.project_id, "doc-a", "draft").await;

    let outcome = document::confirm_revision(
        &pool,
        fx.tenant_id,
        fx.project_id,
        document_id,
        revision_id,
        &fx.human,
    )
    .await
    .unwrap();

    assert_eq!(outcome.status, "published");
    assert!(!outcome.already_published);
    assert_eq!(outcome.confirmed_by, fx.human.principal_id);
    assert_eq!(outcome.revision_no, 1);

    // 库态：published + 确认人 + 确认时间。
    let (status, confirmed_by, published_at): (
        String,
        Option<Uuid>,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as(
        "SELECT status, confirmed_by, published_at FROM kb.document_revision WHERE id = $1",
    )
    .bind(revision_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "published");
    assert_eq!(confirmed_by, Some(fx.human.principal_id));
    assert!(published_at.is_some());

    // 同事务 outbox：类型 + 幂等键 + payload（目录三字段 + 四元组引用）。
    let events = outbox_rows(&pool, fx.project_id).await;
    assert_eq!(events.len(), 1, "confirm must emit exactly one event");
    let (event_type, payload, idempotency_key) = &events[0];
    assert_eq!(event_type, "kb.document_revision.published");
    assert_eq!(
        idempotency_key,
        &format!("kb_document_revision_published:{revision_id}")
    );
    let doc_ref = &payload["document_ref"];
    assert_eq!(doc_ref["target_type"], "document");
    assert_eq!(doc_ref["target_id"], document_id.to_string());
    assert_eq!(doc_ref["target_revision_id"], revision_id.to_string());
    assert_eq!(
        payload["confirmed_by"],
        fx.human.principal_id.to_string(),
        "event payload carries the real human confirmer"
    );
    assert_eq!(payload["revision_no"], 1);

    // 审计：source_kind='kb_service' + 四身份列全写（C174/C180）。
    let (source_kind, actor_auth_mode, actor_client): (String, Option<String>, Option<String>) =
        sqlx::query_as(
            "SELECT source_kind, actor_auth_mode, actor_client FROM governance.audit_log \
              WHERE action = 'kb.document_revision.published' AND target_id = $1",
        )
        .bind(revision_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(source_kind, "kb_service");
    assert_eq!(actor_auth_mode.as_deref(), Some("interactive"));
    assert_eq!(actor_client.as_deref(), Some("web"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn reconfirm_is_idempotent_and_keeps_original_confirmer(pool: PgPool) {
    let fx = seed(&pool, "WP0-CONFIRM-IDEMPOTENT").await;
    let (document_id, revision_id) =
        seed_revision(&pool, fx.tenant_id, fx.project_id, "doc-b", "draft").await;

    let first = document::confirm_revision(
        &pool,
        fx.tenant_id,
        fx.project_id,
        document_id,
        revision_id,
        &fx.human,
    )
    .await
    .unwrap();

    // 另一位真人重确认 → 幂等 no-op。
    let second = document::confirm_revision(
        &pool,
        fx.tenant_id,
        fx.project_id,
        document_id,
        revision_id,
        &fx.human2,
    )
    .await
    .unwrap();

    assert!(second.already_published, "re-confirm must be a no-op");
    assert_eq!(
        second.confirmed_by, first.confirmed_by,
        "re-confirm must not overwrite the original confirmer"
    );
    assert_eq!(
        second.published_at, first.published_at,
        "re-confirm must not overwrite the original publish time"
    );

    // 幂等：不重复发事件。
    assert_eq!(outbox_rows(&pool, fx.project_id).await.len(), 1);
}

// ── 真人门负向 ─────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn api_key_mode_principal_cannot_confirm(pool: PgPool) {
    let fx = seed(&pool, "WP0-CONFIRM-APIKEY").await;
    let (document_id, revision_id) =
        seed_revision(&pool, fx.tenant_id, fx.project_id, "doc-c", "draft").await;

    let err = document::confirm_revision(
        &pool,
        fx.tenant_id,
        fx.project_id,
        document_id,
        revision_id,
        &fx.api_key,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, KbServiceError::Forbidden(_)),
        "api_key mode must be rejected, got {err:?}"
    );
    assert!(outbox_rows(&pool, fx.project_id).await.is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn ai_principal_cannot_confirm(pool: PgPool) {
    let fx = seed(&pool, "WP0-CONFIRM-AI").await;
    let (document_id, revision_id) =
        seed_revision(&pool, fx.tenant_id, fx.project_id, "doc-d", "draft").await;

    let err = document::confirm_revision(
        &pool,
        fx.tenant_id,
        fx.project_id,
        document_id,
        revision_id,
        &fx.ai,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, KbServiceError::Forbidden(_)),
        "AI principal must be rejected, got {err:?}"
    );
    assert!(outbox_rows(&pool, fx.project_id).await.is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn human_without_interactive_auth_mode_cannot_confirm(pool: PgPool) {
    // 真人主体 + 真人身份行，但 auth_mode 缺失（不可信上下文）→ 拒绝。
    let fx = seed(&pool, "WP0-CONFIRM-NOAUTHMODE").await;
    let pid = principal(&pool, "human", "no-auth-mode").await;
    let iid = identity(&pool, pid).await;
    let actor = AuditActor::identified(pid, iid, Some("web".into()), None);

    let (document_id, revision_id) =
        seed_revision(&pool, fx.tenant_id, fx.project_id, "doc-e", "draft").await;
    let err = document::confirm_revision(
        &pool,
        fx.tenant_id,
        fx.project_id,
        document_id,
        revision_id,
        &actor,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, KbServiceError::Forbidden(_)),
        "missing auth_mode must be rejected (fail-closed), got {err:?}"
    );
}

// ── 归属与状态 ─────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn revision_not_belonging_to_document_is_not_found(pool: PgPool) {
    let fx = seed(&pool, "WP0-CONFIRM-WRONGDOC").await;
    let (document_id, _) =
        seed_revision(&pool, fx.tenant_id, fx.project_id, "doc-f1", "draft").await;
    let (_, other_revision) =
        seed_revision(&pool, fx.tenant_id, fx.project_id, "doc-f2", "draft").await;

    let err = document::confirm_revision(
        &pool,
        fx.tenant_id,
        fx.project_id,
        document_id,
        other_revision,
        &fx.human,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, KbServiceError::NotFound(_)),
        "revision of another document must be 404, got {err:?}"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn cross_project_revision_is_not_found(pool: PgPool) {
    let fx = seed(&pool, "WP0-CONFIRM-CROSSPROJECT").await;
    let other = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "WP0-CONFIRM-OTHER".to_string(),
            project_name: "other".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap()
    .project_id;

    let (document_id, revision_id) =
        seed_revision(&pool, fx.tenant_id, other, "doc-g", "draft").await;

    // 用 A 项目的身份去确认 B 项目的修订 → 404（不泄漏存在性）。
    let err = document::confirm_revision(
        &pool,
        fx.tenant_id,
        fx.project_id,
        document_id,
        revision_id,
        &fx.human,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, KbServiceError::NotFound(_)),
        "cross-project revision must be 404, got {err:?}"
    );
    assert!(outbox_rows(&pool, fx.project_id).await.is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn rejected_revision_conflicts_and_keeps_lifecycle_semantics(pool: PgPool) {
    let fx = seed(&pool, "WP0-CONFIRM-REJECTED").await;
    let (document_id, revision_id) =
        seed_revision(&pool, fx.tenant_id, fx.project_id, "doc-h", "rejected").await;

    let err = document::confirm_revision(
        &pool,
        fx.tenant_id,
        fx.project_id,
        document_id,
        revision_id,
        &fx.human,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, KbServiceError::Conflict(_)),
        "rejected revision must be 409, got {err:?}"
    );

    // 库态未被改动（rejected 语义保留）。
    let (status, confirmed_by): (String, Option<Uuid>) =
        sqlx::query_as("SELECT status, confirmed_by FROM kb.document_revision WHERE id = $1")
            .bind(revision_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "rejected");
    assert!(confirmed_by.is_none());
}

// ── 库层守卫（触发器置换后的纪律固化）────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn content_columns_are_immutable_and_delete_is_forbidden(pool: PgPool) {
    let fx = seed(&pool, "WP0-CONFIRM-GUARD").await;
    let (_, revision_id) =
        seed_revision(&pool, fx.tenant_id, fx.project_id, "doc-j", "draft").await;

    // 内容列不可改。
    let err = sqlx::query("UPDATE kb.document_revision SET content = '# tampered' WHERE id = $1")
        .bind(revision_id)
        .execute(&pool)
        .await
        .expect_err("content column must be immutable");
    assert!(err.to_string().contains("content columns are immutable"));

    // DELETE 禁止。
    let err = sqlx::query("DELETE FROM kb.document_revision WHERE id = $1")
        .bind(revision_id)
        .execute(&pool)
        .await
        .expect_err("delete must be forbidden");
    assert!(err.to_string().contains("append-only"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn only_draft_to_published_transition_is_allowed(pool: PgPool) {
    let fx = seed(&pool, "WP0-CONFIRM-TRANSITION").await;
    let (document_id, revision_id) =
        seed_revision(&pool, fx.tenant_id, fx.project_id, "doc-k", "draft").await;

    // draft → rejected 不在白名单（reject 通道后置，本批不实现）。
    let err = sqlx::query("UPDATE kb.document_revision SET status = 'rejected' WHERE id = $1")
        .bind(revision_id)
        .execute(&pool)
        .await
        .expect_err("draft -> rejected must be rejected by the guard");
    assert!(err.to_string().contains("illegal status transition"));

    // 正常发布一次。
    document::confirm_revision(
        &pool,
        fx.tenant_id,
        fx.project_id,
        document_id,
        revision_id,
        &fx.human,
    )
    .await
    .unwrap();

    // published → draft 回退非法。
    let err = sqlx::query("UPDATE kb.document_revision SET status = 'draft' WHERE id = $1")
        .bind(revision_id)
        .execute(&pool)
        .await
        .expect_err("published -> draft must be rejected by the guard");
    assert!(err.to_string().contains("illegal status transition"));

    // published 后改确认人不可（幂等的库层保证）。
    let err = sqlx::query("UPDATE kb.document_revision SET confirmed_by = $2 WHERE id = $1")
        .bind(revision_id)
        .bind(fx.human2.principal_id)
        .execute(&pool)
        .await
        .expect_err("publication metadata must be immutable");
    assert!(err
        .to_string()
        .contains("publication metadata is immutable"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn published_status_requires_confirmer_and_time(pool: PgPool) {
    let fx = seed(&pool, "WP0-CONFIRM-DBCHECK").await;
    let (document_id, revision_id) =
        seed_revision(&pool, fx.tenant_id, fx.project_id, "doc-i", "draft").await;

    // 直写 published 但不给确认人/时间 → 库层 CHECK 拒绝。
    let err = sqlx::query("UPDATE kb.document_revision SET status = 'published' WHERE id = $1")
        .bind(revision_id)
        .execute(&pool)
        .await
        .expect_err("published without confirmer must be rejected by the DB check");
    assert!(err
        .to_string()
        .contains("document_revision_confirm_consistency_check"));

    // 草稿态带确认人 → 同样拒绝（双向等价式）。
    let err2 = sqlx::query(
        "UPDATE kb.document_revision SET confirmed_by = $2, published_at = now() WHERE id = $1",
    )
    .bind(revision_id)
    .bind(fx.human.principal_id)
    .execute(&pool)
    .await
    .expect_err("draft with confirmer must be rejected by the DB check");
    assert!(err2
        .to_string()
        .contains("document_revision_confirm_consistency_check"));

    // 正常路径（应用层）写入的 published 行满足约束。
    let _: ConfirmOutcome = document::confirm_revision(
        &pool,
        fx.tenant_id,
        fx.project_id,
        document_id,
        revision_id,
        &fx.human,
    )
    .await
    .unwrap();
}
