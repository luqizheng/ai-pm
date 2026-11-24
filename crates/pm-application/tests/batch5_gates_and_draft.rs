//! P3-D04-02 Step5 Batch5 · GAP-06/10/11/21/29 测试（真实 DB）。
//!
//! 断言面：
//! 1. GAP-06：proposal 状态转换 DB 兜底触发器——非法转换直写 DB 拒绝（P0001）；
//!    同态更新放行；合法服务转换不受影响（回归承载）。
//! 2. GAP-10：draft 修改入口+重锚——成员增删后 submit 不触发 fail-safe 409；
//!    非 draft 409；锚列更新+审计。
//! 3. GAP-11：草稿物化同事务审计 `{domain}_object.draft_upsert`（每次 upsert）。
//! 4. GAP-29：PC 侧冲突门禁（R-06 pc.conflict=Legacy：直写种子测门禁代码）——
//!    submit/approve 403；resolved 解冻。
//! 5. GAP-21：对账差分只读查询零差分（含一次发布后的自洽）。

use sqlx::PgPool;
use uuid::Uuid;

use pm_application::actor::AuditActor;
use pm_application::pc::{
    approve_proposal, create_proposal, publish_version, submit_proposal, update_proposal_draft,
    upsert_object_draft, CreateProposalInput, DecisionInput, ProposalMemberPin,
    PublishVersionInput, UpsertObjectDraft,
};

const LEAD: Uuid = Uuid::from_u128(0x7072_0000_0000_0000_0000_0000_0000_0005);

async fn seed(pool: &PgPool, tag: &str) -> (Uuid, Uuid, Uuid) {
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('system', $1, $1) RETURNING id",
    )
    .bind(format!("b5-{tag}-{}", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .unwrap();
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(format!("b5-c-{tag}"))
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    let project: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, $2, $3) RETURNING id",
    )
    .bind(customer)
    .bind(format!("b5-{tag}"))
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES ($1, 'human', 'b5-lead', true) ON CONFLICT (id) DO NOTHING",
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

async fn draft_obj(
    pool: &PgPool,
    project: Uuid,
    key: &str,
) -> pm_application::pc::ObjectUpsertReceipt {
    upsert_object_draft(
        pool,
        project,
        &AuditActor::principal(LEAD),
        UpsertObjectDraft {
            object_type: "feature".into(),
            object_key: Some(key.into()),
            display_name: key.into(),
            content_markdown: format!("# {key}"),
            change_summary: Some("b5 draft".into()),
            source_context: None,
            analysis_mode: None,
        },
    )
    .await
    .unwrap()
}

fn one_member(receipt: &pm_application::pc::ObjectUpsertReceipt) -> Vec<ProposalMemberPin> {
    vec![ProposalMemberPin {
        change_kind: "create_object".into(),
        object_id: Some(receipt.object_id),
        object_revision_id: Some(receipt.revision_id),
        relationship_id: None,
        relationship_revision_id: None,
    }]
}

/// GAP-11 + GAP-10：草稿物化审计 + 重锚提交。
#[sqlx::test(migrations = "../../migrations")]
async fn draft_upsert_audited_and_reanchor_allows_submit(pool: PgPool) {
    let (_t, project, _lead) = seed(&pool, "g11").await;
    let obj = draft_obj(&pool, project, "b5-a").await;

    // GAP-11：首次物化审计落行
    let audit_pc: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
          WHERE project_id = $1 AND action = 'pc_object.draft_upsert'",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(audit_pc >= 1, "草稿物化必须有同事务审计行");

    // draft 提案 → GAP-10 修改成员 → 锚列更新 → submit 不触发 fail-safe 409
    let b = draft_obj(&pool, project, "b5-b").await;
    let proposal = create_proposal(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        CreateProposalInput {
            title: "重锚验证".into(),
            rationale: "b5".into(),
            origin_kind: "manual_input".into(),
            source_id: None,
            governance_level: None,
            risk_level: None,
            confidence: None,
            members: one_member(&obj),
        },
    )
    .await
    .unwrap();
    let old_hash: String =
        sqlx::query_scalar("SELECT proposal_content_hash FROM pc.proposal WHERE id = $1")
            .bind(proposal.proposal_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    // 非成员对象加入 → 成员增删重锚
    let updated = update_proposal_draft(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        proposal.proposal_id,
        one_member(&b),
    )
    .await
    .unwrap();
    assert_eq!(updated.status, "draft");
    assert_ne!(updated.proposal_content_hash, old_hash, "重锚产生新内容锚");
    assert_eq!(updated.member_count, 1);

    let update_audit: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
          WHERE project_id = $1 AND action = 'pc_proposal.draft_update'",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(update_audit >= 1, "重锚同事务审计");

    // submit 成功（重锚后 hash 三方一致，无 fail-safe 409）
    let submitted = submit_proposal(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        proposal.proposal_id,
        Some(0),
    )
    .await
    .unwrap();
    assert_eq!(submitted.status, "submitted");

    // 非 draft 修改 → 409
    let r = update_proposal_draft(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        proposal.proposal_id,
        one_member(&obj),
    )
    .await;
    assert!(
        matches!(r, Err(pm_application::pc::PcError::Conflict(_))),
        "非 draft 修改必须 409: {r:?}"
    );
}

/// GAP-06：proposal 状态转换 DB 兜底触发器（直写非法转换 P0001；同态放行）。
#[sqlx::test(migrations = "../../migrations")]
async fn illegal_transition_rejected_by_db_guard(pool: PgPool) {
    let (_t, project, _lead) = seed(&pool, "g06").await;
    let obj = draft_obj(&pool, project, "b5-guard").await;
    let proposal = create_proposal(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        CreateProposalInput {
            title: "守卫验证".into(),
            rationale: "b5".into(),
            origin_kind: "manual_input".into(),
            source_id: None,
            governance_level: None,
            risk_level: None,
            confidence: None,
            members: one_member(&obj),
        },
    )
    .await
    .unwrap();

    // 非法转换：draft → applied / rejected / approved（直写 DB 触发器拒绝）
    for illegal in ["applied", "rejected", "approved", "expired", "failed"] {
        let r = sqlx::query("UPDATE pc.proposal SET status = $2 WHERE id = $1")
            .bind(proposal.proposal_id)
            .bind(illegal)
            .execute(&pool)
            .await;
        assert!(r.is_err(), "draft→{illegal} 必须被 DB 触发器拒绝");
    }

    // 同态放行（非状态列更新不受影响）
    sqlx::query("UPDATE pc.proposal SET updated_at = now() WHERE id = $1")
        .bind(proposal.proposal_id)
        .execute(&pool)
        .await
        .unwrap();

    // 合法服务转换不受影响：draft → submitted（T-02）
    let submitted = submit_proposal(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        proposal.proposal_id,
        Some(0),
    )
    .await
    .unwrap();
    assert_eq!(submitted.status, "submitted");
}

/// GAP-29：PC 侧冲突门禁（R-06：pc.conflict=Legacy，直写种子测门禁代码）。
#[sqlx::test(migrations = "../../migrations")]
async fn pc_conflict_gates_submit_and_approve(pool: PgPool) {
    let (tenant, project, _lead) = seed(&pool, "g29").await;
    let obj = draft_obj(&pool, project, "b5-conf").await;

    // 直写 pc.conflict（Legacy 表；门禁代码=测试对象）
    let conflict: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.conflict \
         (tenant_id, project_id, conflict_type, severity, description, detection_source, status, created_by) \
         VALUES ($1, $2, 'content_contradiction', 'major', 'PC 门禁用例', 'manual', 'detected', $3) \
         RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(LEAD)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO pc.conflict_object (tenant_id, project_id, conflict_id, object_id) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(tenant)
    .bind(project)
    .bind(conflict)
    .bind(obj.object_id)
    .execute(&pool)
    .await
    .unwrap();

    let proposal = create_proposal(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        CreateProposalInput {
            title: "冻结下起草".into(),
            rationale: "b5".into(),
            origin_kind: "manual_input".into(),
            source_id: None,
            governance_level: None,
            risk_level: None,
            confidence: None,
            members: one_member(&obj),
        },
    )
    .await
    .unwrap();

    // 三落点之 submit/approve：L3 冻结 403（publish 侧已由 pc_model 冻结用例承载）
    let frozen_submit = submit_proposal(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        proposal.proposal_id,
        Some(0),
    )
    .await;
    assert!(
        matches!(
            frozen_submit,
            Err(pm_application::pc::PcError::Forbidden(_))
        ),
        "冻结下 submit 必须 403: {frozen_submit:?}"
    );
    // approve 需 submitted——先手工置 submitted（合法转换经 DB）再验证冻结
    // ——approve 的 L3 前置在 submitted 状态才可达；此处直接验证 submit 侧即可，
    // approve 侧已由 decision_gates_fail_closed（frozen approve 403）承载。
    let _ = approve_proposal;

    // 解冻（resolved）→ submit 放行
    sqlx::query(
        "UPDATE pc.conflict SET status = 'resolved', resolved_by = $2, resolved_at = now() \
          WHERE id = $1",
    )
    .bind(conflict)
    .bind(LEAD)
    .execute(&pool)
    .await
    .unwrap();
    let submitted = submit_proposal(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        proposal.proposal_id,
        Some(0),
    )
    .await
    .unwrap();
    assert_eq!(submitted.status, "submitted");
}

/// GAP-21：对账差分只读查询（发布后自洽=零差分；孤儿账本=差分行）。
#[sqlx::test(migrations = "../../migrations")]
async fn reconciliation_diff_is_empty_on_consistent_state(pool: PgPool) {
    let (_t, project, _lead) = seed(&pool, "g21").await;
    let obj = draft_obj(&pool, project, "b5-rec").await;

    // 服务路径构造 approved 提案（合法转换链 draft→submitted→approved；0034 守卫下
    // 不再允许直写跳态）
    let proposal = create_proposal(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        CreateProposalInput {
            title: "对账自洽".into(),
            rationale: "b5".into(),
            origin_kind: "manual_input".into(),
            source_id: None,
            governance_level: None,
            risk_level: None,
            confidence: None,
            members: one_member(&obj),
        },
    )
    .await
    .unwrap();
    submit_proposal(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        proposal.proposal_id,
        Some(0),
    )
    .await
    .unwrap();
    approve_proposal(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        proposal.proposal_id,
        DecisionInput {
            reason: Some("对账验证批准".into()),
            expected_version_no: 0,
        },
    )
    .await
    .unwrap();
    let proposal_id = proposal.proposal_id;

    let v1 = publish_version(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        PublishVersionInput { proposal_id },
    )
    .await
    .unwrap();
    assert!(!v1.replayed);

    // ①② 差分：completed 发布行 × version 双向=零差分
    let orphans: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pc.write_operation wo \
          WHERE wo.status = 'completed' AND wo.command = 'pc_publish_version' \
            AND NOT EXISTS (SELECT 1 FROM pc.cognition_version v WHERE v.id = wo.version_id)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let unledgered: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pc.cognition_version v \
          WHERE NOT EXISTS (SELECT 1 FROM pc.write_operation wo \
            WHERE wo.status = 'completed' AND wo.command = 'pc_publish_version' \
              AND wo.version_id = v.id)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((orphans, unledgered), (0, 0), "同事务模型零差分");
}
