//! 审阅流 v2 · D-4 身份综合提案挂报告（pm_application::identity 服务层集成测试）。
//!
//! 断言面：
//! 1. 无 analyzed 报告 → apply 拒绝（Conflict "no open analysis report"）。
//! 2. 有 analyzed 报告 → 画像/愿景提案挂报告 pc_source_id（报告详情可见、
//!    close 批量发布的供给面），submitted 等人工审阅。

use sqlx::PgPool;
use uuid::Uuid;

use pm_application::actor::AuditActor;
use pm_application::identity::{apply_identity_drafts, IdentityDrafts, IdentityError};
use pm_application::pc::{register_source, SourceInput};

const LEAD: Uuid = Uuid::from_u128(0x7273_0000_0000_0000_0000_0000_0000_0001);

async fn seed_project(pool: &PgPool, code: &str) -> (Uuid, Uuid, Uuid) {
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES ($1, 'human', 'ident-lead', true) ON CONFLICT (id) DO NOTHING",
    )
    .bind(LEAD)
    .execute(pool)
    .await
    .unwrap();
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('system', $1, $1) RETURNING id",
    )
    .bind(format!("ident-{code}-{}", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .unwrap();
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(format!("ident-c-{code}"))
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    let project: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, $2, $3) RETURNING id",
    )
    .bind(customer)
    .bind(code)
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ($1, $2, $3, 'owner')",
    )
    .bind(tenant)
    .bind(project)
    .bind(LEAD)
    .execute(pool)
    .await
    .unwrap();
    (tenant, project, LEAD)
}

/// analyzed 报告夹具：pc.source + project_repository + repo_analysis_report。
async fn seed_analyzed_report(
    pool: &PgPool,
    project: Uuid,
    tenant: Uuid,
    who: Uuid,
) -> (Uuid, Uuid) {
    let source_id = register_source(
        pool,
        project,
        &AuditActor::principal(who),
        SourceInput {
            source_kind: "manual_input".into(),
            target_domain: "pc".into(),
            target_type: "analysis".into(),
            target_id: Uuid::new_v4().to_string(),
            target_revision_id: None,
            revision_missing_reason: Some("identity test fixture".into()),
            section_anchor: None,
            start_line: None,
            locator: None,
            description: None,
            source_content_hash: None,
        },
    )
    .await
    .unwrap();
    let repo_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.project_repository \
         (project_id, name, url, status, tenant_id) \
         VALUES ($1, 'fixture-repo', 'git@example.com:fixture.git', 'verified', $2) \
         RETURNING id",
    )
    .bind(project)
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    let report_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.repo_analysis_report \
         (tenant_id, project_id, repository_id, commit_sha, intake_id, status, pc_source_id) \
         VALUES ($1, $2, $3, 'abc1234def567', $4, 'analyzed', $5) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(repo_id)
    .bind(Uuid::new_v4())
    .bind(source_id)
    .fetch_one(pool)
    .await
    .unwrap();
    (report_id, source_id)
}

fn drafts() -> IdentityDrafts {
    IdentityDrafts {
        product_profile_markdown: "# 产品画像\n\n测试画像正文".into(),
        product_vision_markdown: "# 产品愿景\n\n测试愿景正文".into(),
        project_goal_markdown: "# 项目目标".into(),
        uncertainty_notes: vec![],
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn apply_requires_open_report(pool: PgPool) {
    let (_, project, who) = seed_project(&pool, "ident-n1").await;
    let err = apply_identity_drafts(
        &pool,
        None,
        project,
        &AuditActor::principal(who),
        &drafts(),
        Uuid::new_v4(),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, IdentityError::Conflict(ref m) if m.contains("no open analysis report")),
        "{err:?}"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn apply_attaches_proposals_to_open_report(pool: PgPool) {
    let (tenant, project, who) = seed_project(&pool, "ident-a1").await;
    let (_report_id, report_source) = seed_analyzed_report(&pool, project, tenant, who).await;

    let outcome = apply_identity_drafts(
        &pool,
        None,
        project,
        &AuditActor::principal(who),
        &drafts(),
        Uuid::new_v4(),
    )
    .await
    .unwrap();
    let profile = outcome.profile_proposal_id.expect("profile proposal");
    let vision = outcome.vision_proposal_id.expect("vision proposal");

    // 两提案挂报告 pc_source_id（报告详情审阅面 + close 批量发布供给）
    let (n, statuses): (i64, Vec<String>) = sqlx::query_as(
        "SELECT count(*), array_agg(status ORDER BY created_at) FROM pc.proposal \
          WHERE project_id = $1 AND source_id = $2",
    )
    .bind(project)
    .bind(report_source)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 2, "both proposals attached to the report source");
    assert!(statuses.iter().all(|s| s == "submitted"), "{statuses:?}");
}

// ── GAP-02（Batch4a 统一授权实施）：同轮重跑零重复提案 + 异意图 409 ──────────

#[sqlx::test(migrations = "../../migrations")]
async fn same_round_replay_is_idempotent(pool: PgPool) {
    let (tenant, project, who) = seed_project(&pool, "ident-r1").await;
    let (_report_id, _report_source) = seed_analyzed_report(&pool, project, tenant, who).await;
    let round = Uuid::new_v4();

    let first = apply_identity_drafts(
        &pool,
        None,
        project,
        &AuditActor::principal(who),
        &drafts(),
        round,
    )
    .await
    .unwrap();
    let second = apply_identity_drafts(
        &pool,
        None,
        project,
        &AuditActor::principal(who),
        &drafts(),
        round,
    )
    .await
    .unwrap();
    assert_eq!(
        first.profile_proposal_id, second.profile_proposal_id,
        "同轮重跑 profile 幂等返回原提案"
    );
    assert_eq!(first.vision_proposal_id, second.vision_proposal_id);

    // 零重复：同源提案仍 2 条；账本两行（profile/vision）completed
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM pc.proposal WHERE project_id = $1")
        .bind(project)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 2, "同轮重跑零重复提案");
    let ops: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pc.write_operation \
          WHERE project_id = $1 AND command = 'pc_propose_identity' AND status = 'completed'",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ops, 2, "slot 异键各自成立（profile/vision 两行）");
}

#[sqlx::test(migrations = "../../migrations")]
async fn same_round_different_drafts_rejected(pool: PgPool) {
    let (tenant, project, who) = seed_project(&pool, "ident-r2").await;
    let (_report_id, _report_source) = seed_analyzed_report(&pool, project, tenant, who).await;
    let round = Uuid::new_v4();

    apply_identity_drafts(
        &pool,
        None,
        project,
        &AuditActor::principal(who),
        &drafts(),
        round,
    )
    .await
    .unwrap();
    let tampered = IdentityDrafts {
        product_profile_markdown: "# 产品画像\n\n被篡改的正文".into(),
        ..drafts()
    };
    let err = apply_identity_drafts(
        &pool,
        None,
        project,
        &AuditActor::principal(who),
        &tampered,
        round,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, IdentityError::Conflict(ref m) if m.contains("fingerprint mismatch")),
        "同轮异草案必须指纹失配 409: {err:?}"
    );
}
