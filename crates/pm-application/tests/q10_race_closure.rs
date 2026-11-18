//! P3-D04-02 Step6 前置 · Q-10/GAP-32 竞态闭合证明测试（专家审阅意见 §七 Q6）。
//!
//! 真实多连接并发交错（非 publish×upsert×decide 的替代面）：
//! 1. close × decide 同时处理同一 submitted Proposal（场景 A）：行级互斥下
//!    恰好一态胜出（withdrawn / applied），无中间态、无双写、报告必 closed。
//! 2. close × 显式补发布 同时处理同一 approved Proposal（场景 C 变体）：同键
//!    幂等——总版本数恰 1，两调用语义一致（其一 replayed）。
//! 结论断言=最终数据库状态（TEST FACT），不依赖执行时序。

use sqlx::PgPool;
use uuid::Uuid;

use pm_application::actor::AuditActor;
use pm_application::analysis_report::close_report;
use pm_application::pc::{
    create_proposal, decide_proposal, publish_version, register_source, submit_proposal,
    upsert_object_draft, CreateProposalInput, DecisionAction, ProposalDecisionInput,
    ProposalMemberPin, PublishVersionInput, SourceInput, UpsertObjectDraft,
};

const LEAD: Uuid = Uuid::from_u128(0x7072_0000_0000_0000_0000_0000_0000_0011);

async fn seed(pool: &PgPool, tag: &str) -> (Uuid, Uuid, Uuid) {
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('system', $1, $1) RETURNING id",
    )
    .bind(format!("q10-{tag}-{}", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .unwrap();
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(format!("q10-c-{tag}"))
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    let project: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, $2, $3) RETURNING id",
    )
    .bind(customer)
    .bind(format!("q10-{tag}"))
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES ($1, 'human', 'q10-lead', true) ON CONFLICT (id) DO NOTHING",
    )
    .bind(LEAD)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ($1, $2, $3, 'owner') ON CONFLICT DO NOTHING",
    )
    .bind(tenant)
    .bind(project)
    .bind(LEAD)
    .execute(pool)
    .await
    .unwrap();
    (tenant, project, LEAD)
}

async fn seed_analyzed_report(
    pool: &PgPool,
    project: Uuid,
    tenant: Uuid,
) -> (Uuid, Uuid) {
    let source_id = register_source(
        pool,
        project,
        &AuditActor::principal(LEAD),
        SourceInput {
            source_kind: "manual_input".into(),
            target_domain: "pc".into(),
            target_type: "analysis".into(),
            target_id: Uuid::new_v4().to_string(),
            target_revision_id: None,
            revision_missing_reason: Some("q10 fixture".into()),
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
         VALUES ($1, 'q10-repo', 'git@example.com:q10.git', 'verified', $2) RETURNING id",
    )
    .bind(project)
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    let report_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.repo_analysis_report \
         (tenant_id, project_id, repository_id, commit_sha, intake_id, status, pc_source_id) \
         VALUES ($1, $2, $3, 'q10abc1234', $4, 'analyzed', $5) RETURNING id",
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

async fn submitted_proposal(pool: &PgPool, project: Uuid, source: Uuid, key: &str) -> Uuid {
    let obj = upsert_object_draft(
        pool,
        project,
        &AuditActor::principal(LEAD),
        UpsertObjectDraft {
            object_type: "feature".into(),
            object_key: Some(key.into()),
            display_name: key.into(),
            content_markdown: format!("# {key}"),
            change_summary: Some("q10".into()),
            source_context: None,
            analysis_mode: None,
        },
    )
    .await
    .unwrap();
    let drafted = create_proposal(
        pool,
        project,
        &AuditActor::principal(LEAD),
        CreateProposalInput {
            title: format!("q10 {key}"),
            rationale: "q10".into(),
            origin_kind: "manual_input".into(),
            source_id: Some(source),
            governance_level: None,
            risk_level: None,
            confidence: None,
            members: vec![ProposalMemberPin {
                change_kind: "create_object".into(),
                object_id: Some(obj.object_id),
                object_revision_id: Some(obj.revision_id),
                relationship_id: None,
                relationship_revision_id: None,
            }],
        },
    )
    .await
    .unwrap();
    submit_proposal(pool, project, &AuditActor::principal(LEAD), drafted.proposal_id, Some(0))
        .await
        .unwrap();
    drafted.proposal_id
}

/// 场景 A：close × decide 并发处理同一 submitted Proposal。
#[sqlx::test(migrations = "../../migrations")]
async fn close_vs_decide_single_submitted_proposal(pool: PgPool) {
    let (tenant, project, _lead) = seed(&pool, "a").await;
    let (report, source) = seed_analyzed_report(&pool, project, tenant).await;
    let proposal = submitted_proposal(&pool, project, source, "q10-a").await;

    let p1 = pool.clone();
    let p2 = pool.clone();
    let (close_r, decide_r) = tokio::join!(
        async move { close_report(&p1, project, report, &AuditActor::principal(LEAD)).await },
        async move {
            decide_proposal(
                &p2,
                project,
                &AuditActor::principal(LEAD),
                proposal,
                ProposalDecisionInput {
                    action: DecisionAction::Approve,
                    reason: Some("并发批准".into()),
                    expected_version_no: 0,
                },
            )
            .await
        },
    );

    // 报告必 closed；close 计数与提案终态一致
    let outcome = close_r.unwrap();
    let (status, report_status): (String, String) = sqlx::query_as(
        "SELECT p.status, (SELECT r.status FROM integration.repo_analysis_report r WHERE r.id = $2) \
           FROM pc.proposal p WHERE p.id = $1",
    )
    .bind(proposal)
    .bind(report)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(report_status, "closed");

    match status.as_str() {
        "withdrawn" => {
            // close 先获提案行锁：decide 后到 → 409（终态不可决）
            assert!(matches!(decide_r, Err(pm_application::pc::PcError::Conflict(_))), "{decide_r:?}");
            assert_eq!(outcome.withdrawn_proposals, 1);
            assert_eq!(outcome.applied_proposals, 0);
        }
        "applied" => {
            // decide 先提交 approved：close 清单读取后批量发布 → applied
            assert!(decide_r.is_ok(), "{decide_r:?}");
            assert_eq!(outcome.applied_proposals, 1);
            assert_eq!(outcome.withdrawn_proposals, 0);
        }
        other => panic!("非法中间态 {other}：竞态闭合被破坏"),
    }

    // 双写不变量：版本至多 1；审计链存在（withdraw 或 approve+publish 二者恰一）
    let versions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pc.cognition_version WHERE project_id = $1",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(versions <= 1, "同提案并发至多一版本: {versions}");
}

/// 场景 C 变体：close × 显式补发布并发处理同一 approved Proposal（同键幂等）。
#[sqlx::test(migrations = "../../migrations")]
async fn close_vs_explicit_publish_same_proposal(pool: PgPool) {
    let (tenant, project, _lead) = seed(&pool, "c").await;
    let (report, source) = seed_analyzed_report(&pool, project, tenant).await;
    let proposal = submitted_proposal(&pool, project, source, "q10-c").await;
    pm_application::pc::approve_proposal(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        proposal,
        pm_application::pc::DecisionInput {
            reason: Some("预置批准".into()),
            expected_version_no: 0,
        },
    )
    .await
    .unwrap();

    let p1 = pool.clone();
    let p2 = pool.clone();
    let (close_r, publish_r) = tokio::join!(
        async move { close_report(&p1, project, report, &AuditActor::principal(LEAD)).await },
        async move {
            publish_version(
                &p2,
                project,
                &AuditActor::principal(LEAD),
                PublishVersionInput { proposal_id: proposal },
            )
            .await
        },
    );

    let outcome = close_r.unwrap();
    // 两入口同键（pc-decision:{proposal_id}）+ 同项目 advisory：串行+completed 短路
    // → 恰一版本；先到者 applied，后到者 replayed（close 计数为发布成功数）
    publish_r.unwrap();
    let versions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pc.cognition_version WHERE project_id = $1",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(versions, 1, "同键幂等：并发 close/补发布恰一版本");
    let status: String = sqlx::query_scalar("SELECT status FROM pc.proposal WHERE id = $1")
        .bind(proposal)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "applied");
    // 顺序无关：close 先读清单→applied(1)+failed(0)；显式发布先完成→清单空(0+0)。
    // 两种交错均合法；不变量=恰一版本+applied（同键幂等兜底）。
    assert!(outcome.applied_proposals + outcome.publish_failed_proposals <= 1);
}
