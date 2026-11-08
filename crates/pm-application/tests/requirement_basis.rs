//! DK-D 验收 · basis_finding_ids 边界 + 一键物化（设计稿 §3.3 / D-DK-5）。
//!
//! 两条硬边界的 **整条拒绝**语义（不是"过滤掉坏的那一条"）：
//! 引用未确认 finding / 引用不存在 finding → 零需求、零修订落库。
//!
//! 物化面（人类半步）：`requirement_finding` 逐条 + `source_input(kind='discovery')`
//! 一条，幂等；超出清单的 id 直接拒绝（不猜意图）。

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::discovery;
use pm_application::pm_write::PmWriteError;
use pm_application::requirement::{
    self, CreateRequirementRequest, DraftRevisionRequest, RequirementEntry, RevisionView,
};
use pm_application::requirement_traceability::{self, MaterializeBasisRequest};
use pm_application::actor::AuditActor;
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    project_id: Uuid,
    human_id: Uuid,
    session_id: Uuid,
    other_session_id: Uuid,
}

async fn fixture(pool: &PgPool, code: &str) -> Fx {
    let boot = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: format!("basis-{code}"),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let session = discovery::create_session(
        pool,
        SYSTEM_TENANT,
        boot.project_id,
        "WEB",
        "TASK_DISCOVERY",
        boot.human_id,
    )
    .await
    .unwrap();
    let other_session = discovery::create_session(
        pool,
        SYSTEM_TENANT,
        boot.project_id,
        "WEB",
        "UPDATE",
        boot.human_id,
    )
    .await
    .unwrap();
    Fx {
        project_id: boot.project_id,
        human_id: boot.human_id,
        session_id: session.id,
        other_session_id: other_session.id,
    }
}

async fn seed_finding(
    pool: &PgPool,
    fx: &Fx,
    session_id: Uuid,
    content: &str,
    verification_status: &str,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO discovery.discovery_finding \
         (tenant_id, session_id, project_id, kind, content, verification_status) \
         VALUES ($1, $2, $3, 'user_confirmed', $4, $5) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .bind(session_id)
    .bind(fx.project_id)
    .bind(content)
    .bind(verification_status)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn create_request(basis: Option<Vec<Uuid>>) -> CreateRequirementRequest {
    CreateRequirementRequest {
        code: "FR-BASIS".to_string(),
        title: "手机号登录".to_string(),
        description: Some("支持手机号+验证码登录".to_string()),
        requirement_type: None,
        priority: None,
        markdown: Some("# 手机号登录\n\n用户可用手机号登录。\n".to_string()),
        basis_finding_ids: basis,
    }
}

async fn create(
    pool: &PgPool,
    fx: &Fx,
    basis: Option<Vec<Uuid>>,
) -> Result<RequirementEntry, PmWriteError> {
    requirement::create_requirement(
        pool,
        SYSTEM_TENANT,
        fx.project_id,
        create_request(basis),
        &AuditActor::principal(fx.human_id),
    )
    .await
}

async fn counts(pool: &PgPool, project_id: Uuid) -> (i64, i64) {
    let requirements: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.requirement WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(pool)
            .await
            .unwrap();
    let revisions: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery.requirement_revision WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(pool)
            .await
            .unwrap();
    (requirements, revisions)
}

// ─────────────────────────────────────────────────── 正路：字段落库 + 透出

#[sqlx::test(migrations = "../../migrations")]
async fn basis_rides_inside_content_and_is_exposed(pool: PgPool) {
    let fx = fixture(&pool, "BASIS-OK").await;
    let first = seed_finding(&pool, &fx, fx.session_id, "需要手机号登录", "CONFIRMED").await;
    let second = seed_finding(&pool, &fx, fx.session_id, "验证码 60s 有效", "CONFIRMED").await;

    let entry = create(&pool, &fx, Some(vec![first, second, first])).await.unwrap();
    let detail = requirement::requirement_detail(&pool, SYSTEM_TENANT, entry.id)
        .await
        .unwrap();
    let revision = &detail.revisions[0];
    assert_eq!(
        revision.basis_finding_ids,
        Some(serde_json::json!([first, second])),
        "去重保序后写进 content 子键"
    );
    assert_eq!(
        revision.content["markdown"], revision.markdown.clone().unwrap(),
        "basis 与 markdown/summary 并列，不替换正文"
    );

    // create_draft 同样携带标记（AI 后续修订也走这条路）。
    let drafted: RevisionView = requirement::create_draft(
        &pool,
        SYSTEM_TENANT,
        entry.id,
        DraftRevisionRequest {
            markdown: "# 手机号登录 v2\n".to_string(),
            summary: None,
            basis_finding_ids: Some(vec![first]),
        },
        &AuditActor::principal(fx.human_id),
    )
    .await
    .unwrap();
    assert_eq!(
        drafted.basis_finding_ids,
        Some(serde_json::json!([first]))
    );
}

// ─────────────────────────────────────────────────── 硬边界：整条拒绝

#[sqlx::test(migrations = "../../migrations")]
async fn unconfirmed_basis_finding_rejects_the_whole_draft(pool: PgPool) {
    let fx = fixture(&pool, "BASIS-PENDING").await;
    let confirmed = seed_finding(&pool, &fx, fx.session_id, "需要手机号登录", "CONFIRMED").await;
    let pending = seed_finding(&pool, &fx, fx.session_id, "可能会员体系", "PENDING").await;

    let error = create(&pool, &fx, Some(vec![confirmed, pending]))
        .await
        .unwrap_err();
    assert!(matches!(error, PmWriteError::Validation(_)), "got {error:?}");
    assert_eq!(
        counts(&pool, fx.project_id).await,
        (0, 0),
        "整条拒绝：不留带病提案（连合法的那一条也不落）"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn unknown_basis_finding_rejects_the_whole_draft(pool: PgPool) {
    let fx = fixture(&pool, "BASIS-GHOST").await;
    let confirmed = seed_finding(&pool, &fx, fx.session_id, "需要手机号登录", "CONFIRMED").await;

    let error = create(&pool, &fx, Some(vec![confirmed, Uuid::new_v4()]))
        .await
        .unwrap_err();
    assert!(matches!(error, PmWriteError::Validation(_)), "got {error:?}");
    assert_eq!(counts(&pool, fx.project_id).await, (0, 0));
}

#[sqlx::test(migrations = "../../migrations")]
async fn basis_spanning_two_sessions_is_rejected(pool: PgPool) {
    let fx = fixture(&pool, "BASIS-SESS").await;
    let first = seed_finding(&pool, &fx, fx.session_id, "需要手机号登录", "CONFIRMED").await;
    let second = seed_finding(&pool, &fx, fx.other_session_id, "验证码 60s", "CONFIRMED").await;

    let error = create(&pool, &fx, Some(vec![first, second])).await.unwrap_err();
    let message = format!("{error:?}");
    assert!(
        message.contains("multiple discovery sessions"),
        "basis must come from one session: {message}"
    );
    assert_eq!(counts(&pool, fx.project_id).await, (0, 0));
}

// ─────────────────────────────────────────────────── update_draft 语义

#[sqlx::test(migrations = "../../migrations")]
async fn update_draft_preserves_basis_unless_explicitly_cleared(pool: PgPool) {
    let fx = fixture(&pool, "BASIS-UPD").await;
    let finding = seed_finding(&pool, &fx, fx.session_id, "需要手机号登录", "CONFIRMED").await;
    let entry = create(&pool, &fx, Some(vec![finding])).await.unwrap();
    let revision = entry.latest_revision_id.unwrap();
    let actor = AuditActor::principal(fx.human_id);

    // 不带 basis 的更新：必须保留（content 是全量重建的）。
    let kept = requirement::update_draft(
        &pool,
        SYSTEM_TENANT,
        entry.id,
        revision,
        DraftRevisionRequest {
            markdown: "# 改过的正文\n".to_string(),
            summary: None,
            basis_finding_ids: None,
        },
        &actor,
    )
    .await
    .unwrap();
    assert_eq!(kept.basis_finding_ids, Some(serde_json::json!([finding])));

    // 显式空数组：清空标记。
    let cleared = requirement::update_draft(
        &pool,
        SYSTEM_TENANT,
        entry.id,
        revision,
        DraftRevisionRequest {
            markdown: "# 再改\n".to_string(),
            summary: None,
            basis_finding_ids: Some(vec![]),
        },
        &actor,
    )
    .await
    .unwrap();
    assert!(cleared.basis_finding_ids.is_none(), "explicit empty clears it");
}

// ─────────────────────────────────────────────────── 一键物化（人类半步）

#[sqlx::test(migrations = "../../migrations")]
async fn materialize_basis_links_findings_and_discovery_source(pool: PgPool) {
    let fx = fixture(&pool, "MAT-OK").await;
    let first = seed_finding(&pool, &fx, fx.session_id, "需要手机号登录", "CONFIRMED").await;
    let second = seed_finding(&pool, &fx, fx.session_id, "验证码 60s 有效", "CONFIRMED").await;
    let entry = create(&pool, &fx, Some(vec![first, second])).await.unwrap();
    let actor = AuditActor::principal(fx.human_id);

    let outcome = requirement_traceability::materialize_basis(
        &pool,
        SYSTEM_TENANT,
        entry.id,
        MaterializeBasisRequest {
            finding_ids: None,
            register_source: None,
            source_title: None,
        },
        &actor,
    )
    .await
    .unwrap();
    assert_eq!(outcome.basis_finding_ids, vec![first, second]);
    assert_eq!(outcome.linked_findings, vec![first, second]);
    assert!(outcome.already_linked_findings.is_empty());
    assert!(outcome.source_linked);
    let source_input_id = outcome.source_input_id.expect("discovery source registered");

    // 正式溯源关联真的落了两行。
    let linked: Vec<Uuid> = sqlx::query_scalar(
        "SELECT finding_id FROM delivery.requirement_finding \
          WHERE requirement_id = $1 ORDER BY created_at, finding_id",
    )
    .bind(entry.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(linked.len(), 2);

    // 来源材料 = 一个访谈会话一条，kind='discovery'，source_ref=session。
    let (kind, source_ref): (String, Option<String>) = sqlx::query_as(
        "SELECT input_kind, source_ref FROM source.source_input WHERE id = $1",
    )
    .bind(source_input_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(kind, "discovery");
    assert_eq!(source_ref.as_deref(), Some(fx.session_id.to_string().as_str()));
    let linked_sources: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.requirement_source_input \
          WHERE requirement_id = $1 AND input_id = $2",
    )
    .bind(entry.id)
    .bind(source_input_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(linked_sources, 1);

    // 幂等：再点一次 → 全部 already_*、不重复建来源材料。
    let again = requirement_traceability::materialize_basis(
        &pool,
        SYSTEM_TENANT,
        entry.id,
        MaterializeBasisRequest {
            finding_ids: None,
            register_source: None,
            source_title: None,
        },
        &actor,
    )
    .await
    .unwrap();
    assert!(again.linked_findings.is_empty());
    assert_eq!(again.already_linked_findings.len(), 2);
    assert_eq!(again.source_input_id, Some(source_input_id));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM source.source_input WHERE project_id = $1 AND input_kind = 'discovery'"
        )
        .bind(fx.project_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn materialize_basis_rejects_ids_outside_the_basis(pool: PgPool) {
    let fx = fixture(&pool, "MAT-OUT").await;
    let inside = seed_finding(&pool, &fx, fx.session_id, "需要手机号登录", "CONFIRMED").await;
    let outside = seed_finding(&pool, &fx, fx.session_id, "别的已确认事实", "CONFIRMED").await;
    let entry = create(&pool, &fx, Some(vec![inside])).await.unwrap();

    let error = requirement_traceability::materialize_basis(
        &pool,
        SYSTEM_TENANT,
        entry.id,
        MaterializeBasisRequest {
            finding_ids: Some(vec![inside, outside]),
            register_source: Some(false),
            source_title: None,
        },
        &AuditActor::principal(fx.human_id),
    )
    .await
    .unwrap_err();
    assert!(matches!(error, PmWriteError::Validation(_)), "got {error:?}");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM delivery.requirement_finding WHERE requirement_id = $1"
        )
        .bind(entry.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0,
        "校验先于写入：拒绝时一行都不落"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn materialize_basis_requires_a_basis_list(pool: PgPool) {
    let fx = fixture(&pool, "MAT-NONE").await;
    let entry = create(&pool, &fx, None).await.unwrap();

    let error = requirement_traceability::materialize_basis(
        &pool,
        SYSTEM_TENANT,
        entry.id,
        MaterializeBasisRequest::default_like(),
        &AuditActor::principal(fx.human_id),
    )
    .await
    .unwrap_err();
    assert!(matches!(error, PmWriteError::Validation(_)), "got {error:?}");
}

/// 便于构造"全缺省"请求（`MaterializeBasisRequest` 没有 `Default`：请求体
/// 反序列化语义靠 `#[serde(default)]`，这里手写一个等价物）。
trait DefaultLike {
    fn default_like() -> Self;
}

impl DefaultLike for MaterializeBasisRequest {
    fn default_like() -> Self {
        Self {
            finding_ids: None,
            register_source: None,
            source_title: None,
        }
    }
}
