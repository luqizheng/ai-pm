//! P3-11 T-1/T-2/T-3 阶段一 · projc 正式链集成测试（提案/审批/发布/关系/冲突；
//! 真实 DB）。真源 = pc_proposal.rs 同款断言口径，域内改名 projc_*。
//!
//! 断言面：
//! 1. 提案全生命周期（create → submit → approve → publish → applied；reject；
//!    withdraw）。
//! 2. 正式写入门禁：AI/服务主体恒 403；非授权真人 403（Lead OR approver_grant）。
//! 3. 发布幂等重放同 receipt；CAS 失配 409 + 提案内置 failed（账本/审计留痕）。
//! 4. L3 冻结：未解决冲突 → submit/approve/publish 403；resolved 解冻。
//! 5. 关系草稿：端点类型合法 / 跨域 `domain.type` ext 键 / 自引用 / 活跃唯一；
//!    version_member 端点闭合触发器（DEFERRED 兜底）。
//! 6. conflict 服务生命周期（人工 create/resolve，库层状态机兜底）。
//! 7. propose_from_analysis 候选剔除口径（版本已含 / 有效提案覆盖；rejected
//!    亦阻断，failed/expired 放行）。

use sqlx::PgPool;
use uuid::Uuid;

use pm_application::actor::AuditActor;
use pm_application::projc::{
    approve_proposal, create_conflict, create_proposal, create_relationship_draft,
    propose_from_analysis, publish_version, register_evidence, register_source,
    resolve_conflict, reject_proposal, submit_proposal, upsert_object_draft, withdraw_proposal,
    AnalysisProposalInput, AnalysisProposalOutcome, ConflictInput, CreateProposalInput,
    EvidenceInput, EvidenceSubject, ProjcEndpoint, ProjcError, PublishVersionInput,
    RelationshipDraftInput, ResolveAction, ResolveInput, SourceInput, UpsertObjectDraft,
};

const LEAD: Uuid = Uuid::from_u128(0x7072_0000_0000_0000_0000_0000_0000_0001);
const MEMBER: Uuid = Uuid::from_u128(0x7072_0000_0000_0000_0000_0000_0000_0002);
/// 迁移种子 `ai-pm-dispatch`（kind='service'）——分析链起草主体。
const DISPATCH: Uuid = Uuid::from_u128(1);

async fn seed_principals(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES ($1, 'human', 'projc-lead', true), ($2, 'human', 'projc-member', true) \
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(LEAD)
    .bind(MEMBER)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_project_with_lead(pool: &PgPool, code: &str) -> Uuid {
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('system', $1, $1) RETURNING id",
    )
    .bind(format!("pjc-{code}-{}", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .unwrap();
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(format!("pjc-c-{code}"))
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
         VALUES ($1, $2, $3, 'owner'), ($1, $2, $4, 'member')",
    )
    .bind(tenant)
    .bind(project)
    .bind(LEAD)
    .bind(MEMBER)
    .execute(pool)
    .await
    .unwrap();
    project
}

fn draft(
    object_type: &str,
    object_key: Option<&str>,
    name: &str,
    content: &str,
) -> UpsertObjectDraft {
    UpsertObjectDraft {
        object_type: object_type.to_string(),
        object_key: object_key.map(str::to_string),
        display_name: name.to_string(),
        content_markdown: content.to_string(),
        change_summary: Some("test draft".into()),
        source_context: Some("service test".into()),
        analysis_mode: None,
    }
}

/// 单成员提案夹具（create_object，指定对象修订）。
fn one_object_proposal(
    title: &str,
    source_id: Option<Uuid>,
    object_id: Uuid,
    revision_id: Uuid,
) -> CreateProposalInput {
    CreateProposalInput {
        title: title.to_string(),
        rationale: "test".into(),
        origin_kind: "manual_input".into(),
        source_id,
        governance_level: None,
        risk_level: None,
        confidence: None,
        members: vec![pm_application::projc::ProposalMemberPin {
            change_kind: "create_object".into(),
            object_id: Some(object_id),
            object_revision_id: Some(revision_id),
            relationship_id: None,
            relationship_revision_id: None,
        }],
    }
}

/// 临时来源夹具（manual_input，无对象探测）。
async fn seed_manual_source(pool: &PgPool, project: Uuid, actor: &AuditActor) -> Uuid {
    register_source(
        pool,
        project,
        actor,
        SourceInput {
            source_kind: "manual_input".into(),
            target_domain: "knowledge_base".into(),
            target_type: "document".into(),
            target_id: Uuid::new_v4().to_string(),
            target_revision_id: None,
            revision_missing_reason: Some("fixture".into()),
            section_anchor: None,
            start_line: None,
            locator: None,
            description: None,
            source_content_hash: None,
        },
    )
    .await
    .unwrap()
}

// ── 1. 提案全生命周期 ────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn proposal_full_lifecycle_reject_and_withdraw(pool: PgPool) {
    seed_principals(&pool).await;
    let project = seed_project_with_lead(&pool, "pj-l1").await;
    let lead = AuditActor::principal(LEAD);

    let obj = upsert_object_draft(&pool, project, &lead, draft("repository", Some("r1"), "仓库", "c1"))
        .await
        .unwrap();
    let proposal = create_proposal(
        &pool,
        project,
        &lead,
        one_object_proposal("仓库认知 v1", None, obj.object_id, obj.revision_id),
    )
    .await
    .unwrap();
    assert_eq!(proposal.status, "draft");
    assert_eq!(proposal.expected_version_no, 0);

    // reject 终态：submitted → rejected，再 submit 409
    let submitted = submit_proposal(&pool, project, &lead, proposal.proposal_id, None)
        .await
        .unwrap();
    assert_eq!(submitted.status, "submitted");
    let rejected = reject_proposal(
        &pool,
        project,
        &lead,
        proposal.proposal_id,
        pm_application::projc::DecisionInput {
            reason: Some("内容不成立".into()),
            expected_version_no: 0,
        },
    )
    .await
    .unwrap();
    assert_eq!(rejected.status, "rejected");
    let resubmit = submit_proposal(&pool, project, &lead, proposal.proposal_id, None).await;
    assert!(matches!(resubmit, Err(ProjcError::Conflict(_))), "{resubmit:?}");

    // withdraw：draft/submitted → withdrawn
    let obj2 = upsert_object_draft(&pool, project, &lead, draft("repository", Some("r2"), "仓库2", "c2"))
        .await
        .unwrap();
    let p2 = create_proposal(
        &pool,
        project,
        &lead,
        one_object_proposal("仓库认知 v2", None, obj2.object_id, obj2.revision_id),
    )
    .await
    .unwrap();
    submit_proposal(&pool, project, &lead, p2.proposal_id, None)
        .await
        .unwrap();
    // 非提案人撤回：member（非 Lead 无授权）→ 403
    let member_withdraw = withdraw_proposal(
        &pool,
        project,
        &AuditActor::principal(MEMBER),
        p2.proposal_id,
        None,
    )
    .await;
    assert!(
        matches!(member_withdraw, Err(ProjcError::Forbidden(_))),
        "{member_withdraw:?}"
    );
    // 提案人本人撤回 → withdrawn；再 withdraw 409
    let withdrawn = withdraw_proposal(&pool, project, &lead, p2.proposal_id, Some("改期".to_string()))
        .await
        .unwrap();
    assert_eq!(withdrawn.status, "withdrawn");
    let again = withdraw_proposal(&pool, project, &lead, p2.proposal_id, None).await;
    assert!(matches!(again, Err(ProjcError::Conflict(_))), "{again:?}");

    // approve → publish → applied：全链绿灯路径
    let obj3 = upsert_object_draft(&pool, project, &lead, draft("tech_stack", Some("t1"), "技术栈", "rust"))
        .await
        .unwrap();
    let p3 = create_proposal(
        &pool,
        project,
        &lead,
        one_object_proposal("技术栈入正式", None, obj3.object_id, obj3.revision_id),
    )
    .await
    .unwrap();
    submit_proposal(&pool, project, &lead, p3.proposal_id, None)
        .await
        .unwrap();
    let approved = approve_proposal(
        &pool,
        project,
        &lead,
        p3.proposal_id,
        pm_application::projc::DecisionInput {
            reason: None,
            expected_version_no: 0,
        },
    )
    .await
    .unwrap();
    assert_eq!(approved.status, "approved");
    let receipt = publish_version(
        &pool,
        project,
        &lead,
        PublishVersionInput {
            proposal_id: p3.proposal_id,
            operation_id: format!("test-decision:{}", p3.proposal_id),
        },
    )
    .await
    .unwrap();
    assert_eq!(receipt.version_no, 1);
    assert_eq!(receipt.member_count, 1);
    assert!(!receipt.replayed);

    let (p_status,): (String,) = sqlx::query_as(
        "SELECT status FROM projc.proposal WHERE id = $1",
    )
    .bind(p3.proposal_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(p_status, "applied");
    let (lifecycle,): (String,) = sqlx::query_as(
        "SELECT lifecycle_status FROM projc.cognition_object WHERE id = $1",
    )
    .bind(obj3.object_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(lifecycle, "active", "首次入正式版本 = active");

    // applied 后再发布 409（提案终态）
    let republish = publish_version(
        &pool,
        project,
        &lead,
        PublishVersionInput {
            proposal_id: p3.proposal_id,
            operation_id: "test-decision:again".into(),
        },
    )
    .await;
    assert!(matches!(republish, Err(ProjcError::Conflict(_))), "{republish:?}");

    // 审计口径：projc_ 前缀（create/submit/approve/publish）
    let actions: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT action FROM governance.audit_log \
          WHERE action LIKE 'projc_%' ORDER BY action",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    for expected in [
        "projc_proposal.create",
        "projc_proposal.submit",
        "projc_proposal.approve",
        "projc_version.publish",
    ] {
        assert!(actions.iter().any(|a| a.0 == expected), "missing {expected}");
    }
}

// ── 2. 正式写入门禁（AI/服务主体 + 非授权真人）──────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn formal_writes_fail_closed_for_ai_and_unauthorized(pool: PgPool) {
    seed_principals(&pool).await;
    let project = seed_project_with_lead(&pool, "pj-g1").await;
    let lead = AuditActor::principal(LEAD);
    let ai = AuditActor::principal(DISPATCH);

    // 提案面 AI 可达（create draft）；草稿面 AI 可达
    let obj = upsert_object_draft(&pool, project, &ai, draft("repository", Some("r1"), "仓库", "c1"))
        .await
        .unwrap();
    let proposal = create_proposal(
        &pool,
        project,
        &ai,
        one_object_proposal("AI 起草", None, obj.object_id, obj.revision_id),
    )
    .await
    .unwrap();
    submit_proposal(&pool, project, &ai, proposal.proposal_id, None)
        .await
        .unwrap();

    // AI 主体 approve → 403（真人门）
    let ai_approve = approve_proposal(
        &pool,
        project,
        &ai,
        proposal.proposal_id,
        pm_application::projc::DecisionInput {
            reason: None,
            expected_version_no: 0,
        },
    )
    .await;
    assert!(matches!(ai_approve, Err(ProjcError::Forbidden(_))), "{ai_approve:?}");

    // 非授权真人（member 无 grant）approve → 403（A3 闸）
    let member_approve = approve_proposal(
        &pool,
        project,
        &AuditActor::principal(MEMBER),
        proposal.proposal_id,
        pm_application::projc::DecisionInput {
            reason: None,
            expected_version_no: 0,
        },
    )
    .await;
    assert!(
        matches!(member_approve, Err(ProjcError::Forbidden(_))),
        "{member_approve:?}"
    );

    // Lead approve → approved；AI publish → 403
    approve_proposal(
        &pool,
        project,
        &lead,
        proposal.proposal_id,
        pm_application::projc::DecisionInput {
            reason: None,
            expected_version_no: 0,
        },
    )
    .await
    .unwrap();
    let ai_publish = publish_version(
        &pool,
        project,
        &ai,
        PublishVersionInput {
            proposal_id: proposal.proposal_id,
            operation_id: "gate-test".into(),
        },
    )
    .await;
    assert!(matches!(ai_publish, Err(ProjcError::Forbidden(_))), "{ai_publish:?}");

    // 显式 Approver 授权行放行非 Lead 真人（grant → approve 幂等重放 OK）
    let tenant: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.approver_grant (tenant_id, project_id, principal_id, granted_by) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(tenant)
    .bind(project)
    .bind(MEMBER)
    .bind(LEAD)
    .execute(&pool)
    .await
    .unwrap();
    let member_replay = approve_proposal(
        &pool,
        project,
        &AuditActor::principal(MEMBER),
        proposal.proposal_id,
        pm_application::projc::DecisionInput {
            reason: None,
            expected_version_no: 0,
        },
    )
    .await
    .unwrap();
    assert_eq!(member_replay.status, "approved", "同态重放返回既有审批事实");
    // 授权真人发布 → 成功
    let receipt = publish_version(
        &pool,
        project,
        &AuditActor::principal(MEMBER),
        PublishVersionInput {
            proposal_id: proposal.proposal_id,
            operation_id: "gate-test-2".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(receipt.version_no, 1);
}

// ── 3. 发布幂等重放 + CAS 失配 ──────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn publish_replays_receipt_and_cas_mismatch_fails_proposal(pool: PgPool) {
    seed_principals(&pool).await;
    let project = seed_project_with_lead(&pool, "pj-c1").await;
    let lead = AuditActor::principal(LEAD);

    let obj = upsert_object_draft(&pool, project, &lead, draft("repository", Some("r1"), "仓库", "c1"))
        .await
        .unwrap();
    let p1 = create_proposal(
        &pool,
        project,
        &lead,
        one_object_proposal("首个版本", None, obj.object_id, obj.revision_id),
    )
    .await
    .unwrap();
    submit_proposal(&pool, project, &lead, p1.proposal_id, None)
        .await
        .unwrap();

    // p2 与 p1 同期起草（基线同为 0）——p1 发布后 p2 的基线即过期
    let obj2 = upsert_object_draft(&pool, project, &lead, draft("tech_stack", Some("t1"), "栈", "rust"))
        .await
        .unwrap();
    let p2 = create_proposal(
        &pool,
        project,
        &lead,
        one_object_proposal("过期基线", None, obj2.object_id, obj2.revision_id),
    )
    .await
    .unwrap();
    submit_proposal(&pool, project, &lead, p2.proposal_id, None)
        .await
        .unwrap();

    approve_proposal(
        &pool,
        project,
        &lead,
        p1.proposal_id,
        pm_application::projc::DecisionInput {
            reason: None,
            expected_version_no: 0,
        },
    )
    .await
    .unwrap();

    // 幂等重放：同 operation_id 第二次 → replayed=true，同 version
    let op_id = "projc-decision:fixture-1";
    let first = publish_version(
        &pool,
        project,
        &lead,
        PublishVersionInput {
            proposal_id: p1.proposal_id,
            operation_id: op_id.into(),
        },
    )
    .await
    .unwrap();
    let replay = publish_version(
        &pool,
        project,
        &lead,
        PublishVersionInput {
            proposal_id: p1.proposal_id,
            operation_id: op_id.into(),
        },
    )
    .await
    .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.version_id, first.version_id);
    assert_eq!(replay.version_no, first.version_no);
    assert_eq!(replay.snapshot_hash, first.snapshot_hash);
    // 账本单行 + completed（无重复正式版本）
    let tenant: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project)
        .fetch_one(&pool)
        .await
        .unwrap();
    let (op_rows, versions): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM projc.write_operation \
                  WHERE tenant_id = $1 AND project_id = $2 AND operation_id = $3), \
                (SELECT count(*) FROM projc.cognition_version \
                  WHERE tenant_id = $1 AND project_id = $2)",
    )
    .bind(tenant)
    .bind(project)
    .bind(op_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(op_rows, 1);
    assert_eq!(versions, 1);

    // CAS 失配：p2 基线 0，但现行版本已到 1 → 409 + 提案内置 failed + 审计留痕
    approve_proposal(
        &pool,
        project,
        &lead,
        p2.proposal_id,
        pm_application::projc::DecisionInput {
            reason: None,
            expected_version_no: 0,
        },
    )
    .await
    .unwrap();
    let cas = publish_version(
        &pool,
        project,
        &lead,
        PublishVersionInput {
            proposal_id: p2.proposal_id,
            operation_id: "projc-decision:fixture-2".into(),
        },
    )
    .await;
    assert!(matches!(cas, Err(ProjcError::Conflict(_))), "{cas:?}");
    let (p2_status,): (String,) = sqlx::query_as(
        "SELECT status FROM projc.proposal WHERE id = $1",
    )
    .bind(p2.proposal_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(p2_status, "failed", "CAS 失配提案内置 failed");
    let (op_status,): (String,) = sqlx::query_as(
        "SELECT status FROM projc.write_operation \
          WHERE tenant_id = (SELECT tenant_id FROM core.project WHERE id = $1) \
            AND project_id = $1 AND operation_id = 'projc-decision:fixture-2'",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(op_status, "failed");
    let (audit_rows,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM governance.audit_log \
          WHERE action = 'projc_version.publish_failed' AND source_ref = $1",
    )
    .bind(p2.proposal_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(audit_rows >= 1, "publish_failed audit trail");
}

// ── 4. L3 冻结与解冻 ────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn conflict_freezes_and_resolves(pool: PgPool) {
    seed_principals(&pool).await;
    let project = seed_project_with_lead(&pool, "pj-f1").await;
    let lead = AuditActor::principal(LEAD);
    let ai = AuditActor::principal(DISPATCH);

    let obj = upsert_object_draft(&pool, project, &lead, draft("repository", Some("r1"), "仓库", "c1"))
        .await
        .unwrap();
    let conflict = create_conflict(
        &pool,
        project,
        &lead,
        ConflictInput {
            conflict_type: "content_contradiction".into(),
            severity: "major".into(),
            description: "仓库快照与口头结论矛盾".into(),
            detection_source: Some("manual".into()),
            objects: vec![obj.object_id],
            relationships: vec![],
        },
    )
    .await
    .unwrap();

    // AI 登记冲突 → 403（人工治理动作）
    let ai_conflict = create_conflict(
        &pool,
        project,
        &ai,
        ConflictInput {
            conflict_type: "scope_conflict".into(),
            severity: "minor".into(),
            description: "x".into(),
            detection_source: None,
            objects: vec![obj.object_id],
            relationships: vec![],
        },
    )
    .await;
    assert!(matches!(ai_conflict, Err(ProjcError::Forbidden(_))), "{ai_conflict:?}");

    // 冻结：submit / approve / publish 均 403
    let proposal = create_proposal(
        &pool,
        project,
        &lead,
        one_object_proposal("冻结下起草", None, obj.object_id, obj.revision_id),
    )
    .await
    .unwrap();
    let frozen_submit = submit_proposal(&pool, project, &lead, proposal.proposal_id, None).await;
    assert!(matches!(frozen_submit, Err(ProjcError::Forbidden(_))), "{frozen_submit:?}");

    resolve_conflict(
        &pool,
        project,
        &lead,
        conflict,
        ResolveInput {
            action: ResolveAction::Resolve,
            resolution_note: Some("以仓库为准".into()),
        },
    )
    .await
    .unwrap();

    // 解冻后：submit → approve → publish 绿灯
    submit_proposal(&pool, project, &lead, proposal.proposal_id, None)
        .await
        .unwrap();
    approve_proposal(
        &pool,
        project,
        &lead,
        proposal.proposal_id,
        pm_application::projc::DecisionInput {
            reason: None,
            expected_version_no: 0,
        },
    )
    .await
    .unwrap();
    publish_version(
        &pool,
        project,
        &lead,
        PublishVersionInput {
            proposal_id: proposal.proposal_id,
            operation_id: "frozen-test".into(),
        },
    )
    .await
    .unwrap();

    // reopen 重新冻结 → 新提案 submit 403；resolve 需 note（服务层校验）
    let reopened = resolve_conflict(
        &pool,
        project,
        &lead,
        conflict,
        ResolveInput {
            action: ResolveAction::Reopen,
            resolution_note: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(reopened, "reopened");
    // 重开期起草：对象已被 p1 发布为 active → update_object 提案，目标仍被冻结
    let obj_v2 = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("repository", Some("r1"), "仓库", "c1-updated"),
    )
    .await
    .unwrap();
    let p2 = create_proposal(
        &pool,
        project,
        &lead,
        CreateProposalInput {
            title: "重开期起草".into(),
            rationale: "test".into(),
            origin_kind: "manual_input".into(),
            source_id: None,
            governance_level: None,
            risk_level: None,
            confidence: None,
            members: vec![pm_application::projc::ProposalMemberPin {
                change_kind: "update_object".into(),
                object_id: Some(obj.object_id),
                object_revision_id: Some(obj_v2.revision_id),
                relationship_id: None,
                relationship_revision_id: None,
            }],
        },
    )
    .await
    .unwrap();
    let refrozen = submit_proposal(&pool, project, &lead, p2.proposal_id, None).await;
    assert!(matches!(refrozen, Err(ProjcError::Forbidden(_))), "{refrozen:?}");
    let no_note = resolve_conflict(
        &pool,
        project,
        &lead,
        conflict,
        ResolveInput {
            action: ResolveAction::Dismiss,
            resolution_note: None,
        },
    )
    .await;
    assert!(matches!(no_note, Err(ProjcError::Invalid(_))), "{no_note:?}");
    let dismissed = resolve_conflict(
        &pool,
        project,
        &lead,
        conflict,
        ResolveInput {
            action: ResolveAction::Dismiss,
            resolution_note: Some("不成立".into()),
        },
    )
    .await
    .unwrap();
    assert_eq!(dismissed, "dismissed");
    submit_proposal(&pool, project, &lead, p2.proposal_id, None)
        .await
        .unwrap();
}

// ── 5. 关系草稿 + 端点闭合触发器 ────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn relationship_endpoints_types_ext_keys_and_closure(pool: PgPool) {
    seed_principals(&pool).await;
    let project = seed_project_with_lead(&pool, "pj-r1").await;
    let lead = AuditActor::principal(LEAD);

    let module = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("repository_module", Some("repo:x:module:src"), "src", "src/"),
    )
    .await
    .unwrap();
    let other = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("repository_module", Some("repo:x:module:lib"), "lib", "lib/"),
    )
    .await
    .unwrap();

    // 跨域 ext 键（0007 形态 `domain.type`）：implements 目标端 = pc.feature_module
    // （DomainRef 契约引用 PC 对象身份，不构成跨域授权——pc 通道 V1 无存在性探测）。
    let ext = || ProjcEndpoint::External {
        domain: "pc".into(),
        target_type: "feature_module".into(),
        target_id: Uuid::new_v4().to_string(),
        revision_id: None,
    };
    let inner = |object_id: Uuid| ProjcEndpoint::Object {
        object_id,
        revision_id: None,
    };

    // implements：repository_module → pc.feature_module（跨域 ext 键）→ 草稿 OK
    let ext_target = ext();
    let (rel_id, rr_id) = create_relationship_draft(
        &pool,
        project,
        &lead,
        RelationshipDraftInput {
            relationship_type: "implements".into(),
            source: inner(module.object_id),
            target: ext_target.clone(),
            properties: serde_json::json!({ "confidence": "high" }),
            change_summary: Some("src 实现结算模块".into()),
        },
    )
    .await
    .unwrap();

    // 端点类型不合法：depends_on 目标端只收 repository_module（pc.feature_module 拒）
    let bad_type = create_relationship_draft(
        &pool,
        project,
        &lead,
        RelationshipDraftInput {
            relationship_type: "depends_on".into(),
            source: inner(module.object_id),
            target: ext(),
            properties: serde_json::json!({}),
            change_summary: None,
        },
    )
    .await;
    assert!(matches!(bad_type, Err(ProjcError::Invalid(_))), "{bad_type:?}");

    // 自引用拒绝（allow_self_reference=false）
    let self_ref = create_relationship_draft(
        &pool,
        project,
        &lead,
        RelationshipDraftInput {
            relationship_type: "depends_on".into(),
            source: inner(module.object_id),
            target: inner(module.object_id),
            properties: serde_json::json!({}),
            change_summary: None,
        },
    )
    .await;
    assert!(matches!(self_ref, Err(ProjcError::Invalid(_))), "{self_ref:?}");

    // 活跃唯一：同类型同端点重复 → Conflict（同 ext 四元组定位）
    let dup = create_relationship_draft(
        &pool,
        project,
        &lead,
        RelationshipDraftInput {
            relationship_type: "implements".into(),
            source: inner(module.object_id),
            target: ext_target.clone(),
            properties: serde_json::json!({}),
            change_summary: None,
        },
    )
    .await;
    assert!(matches!(dup, Err(ProjcError::Conflict(_))), "{dup:?}");

    // 未知关系类型 → Invalid（独立字典；pc 类型不可用）
    let unknown = create_relationship_draft(
        &pool,
        project,
        &lead,
        RelationshipDraftInput {
            relationship_type: "capability-realized-by-requirement".into(),
            source: inner(module.object_id),
            target: inner(other.object_id),
            properties: serde_json::json!({}),
            change_summary: None,
        },
    )
    .await;
    assert!(matches!(unknown, Err(ProjcError::Invalid(_))), "{unknown:?}");

    // 端点闭合：create_relationship 提案发布 → 版本自动并入端点成员（F-05）
    let proposal = create_proposal(
        &pool,
        project,
        &lead,
        CreateProposalInput {
            title: "关系入正式".into(),
            rationale: "test".into(),
            origin_kind: "manual_input".into(),
            source_id: None,
            governance_level: None,
            risk_level: None,
            confidence: None,
            members: vec![pm_application::projc::ProposalMemberPin {
                change_kind: "create_relationship".into(),
                object_id: None,
                object_revision_id: None,
                relationship_id: Some(rel_id),
                relationship_revision_id: Some(rr_id),
            }],
        },
    )
    .await
    .unwrap();
    submit_proposal(&pool, project, &lead, proposal.proposal_id, None)
        .await
        .unwrap();
    approve_proposal(
        &pool,
        project,
        &lead,
        proposal.proposal_id,
        pm_application::projc::DecisionInput {
            reason: None,
            expected_version_no: 0,
        },
    )
    .await
    .unwrap();
    let receipt = publish_version(
        &pool,
        project,
        &lead,
        PublishVersionInput {
            proposal_id: proposal.proposal_id,
            operation_id: "closure-test".into(),
        },
    )
    .await
    .unwrap();
    // 成员 = 关系 + ProjC 内端点（F-05 并入；ext 目标端无内部端点对象）
    assert_eq!(receipt.member_count, 2, "relationship + source endpoint");

    // 触发器兜底：直插「端点不在版本内」的关系成员 → 闭合触发器在语句提交时报错
    // （新关系 target=other 未随版本发布 → 端点缺失；DEFERRABLE 兜底于提交点）
    let (rel2_id, rel2_rr) = create_relationship_draft(
        &pool,
        project,
        &lead,
        RelationshipDraftInput {
            relationship_type: "depends_on".into(),
            source: inner(module.object_id),
            target: inner(other.object_id),
            properties: serde_json::json!({}),
            change_summary: None,
        },
    )
    .await
    .unwrap();
    let violation = sqlx::query(
        "INSERT INTO projc.version_member \
         (tenant_id, project_id, version_id, relationship_id, relationship_revision_id) \
         SELECT tenant_id, project_id, $1, $2, $3 FROM projc.relationship WHERE id = $2",
    )
    .bind(receipt.version_id)
    .bind(rel2_id)
    .bind(rel2_rr)
    .execute(&pool)
    .await;
    assert!(
        violation.is_err(),
        "endpoint closure trigger must reject the insert: {violation:?}"
    );
}

// ── 6. propose_from_analysis 候选剔除口径 ───────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn propose_from_analysis_candidate_exclusion(pool: PgPool) {
    seed_principals(&pool).await;
    let project = seed_project_with_lead(&pool, "pj-p1").await;
    let lead = AuditActor::principal(LEAD);

    let tenant: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project)
        .fetch_one(&pool)
        .await
        .unwrap();
    let kb_doc: Uuid = sqlx::query_scalar(
        "INSERT INTO kb.document (tenant_id, project_id, doc_key, doc_type, canonical_path) \
         VALUES ($1, $2, $3, 'knowledge', $3) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(format!("kb-probe-{}", Uuid::new_v4().simple()))
    .fetch_one(&pool)
    .await
    .unwrap();
    let kb_rev: Uuid = sqlx::query_scalar(
        "INSERT INTO kb.document_revision \
         (tenant_id, document_id, revision_no, content_hash, status, content, created_by, \
          confirmed_by, published_at) \
         VALUES ($1, $2, 1, 'hash-kb', 'published', '# k', $3, $3, now()) RETURNING id",
    )
    .bind(tenant)
    .bind(kb_doc)
    .bind(LEAD)
    .fetch_one(&pool)
    .await
    .unwrap();
    let source = register_source(
        &pool,
        project,
        &lead,
        SourceInput {
            source_kind: "kb_document".into(),
            target_domain: "knowledge_base".into(),
            target_type: "document".into(),
            target_id: kb_doc.to_string(),
            target_revision_id: Some(kb_rev.to_string()),
            revision_missing_reason: None,
            section_anchor: None,
            start_line: None,
            locator: None,
            description: None,
            source_content_hash: None,
        },
    )
    .await
    .unwrap();

    let obj = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("tech_stack", Some("repo:x:tech_stack"), "技术栈", "rust"),
    )
    .await
    .unwrap();
    register_evidence(
        &pool,
        project,
        &lead,
        EvidenceInput {
            source_id: source,
            subject: EvidenceSubject::ObjectRevision(obj.revision_id),
            evidence_type: "document_section".into(),
            target_domain: Some("knowledge_base".into()),
            target_type: Some("document".into()),
            target_id: Some(kb_doc.to_string()),
            target_revision_id: Some(kb_rev.to_string()),
            file_path: None,
            symbol: None,
            code_range: None,
            commit_sha: None,
            section_anchor: Some("tech_stack".into()),
            start_line: None,
            end_line: None,
            excerpt: None,
            description: None,
            evidence_hash: "h1".into(),
            confidence: None,
        },
    )
    .await
    .unwrap();

    let input_for = |task: Uuid| AnalysisProposalInput {
        source_id: source,
        task_id: task,
        title: "仓库认知更新".into(),
        rationale: "test".into(),
    };

    // 首轮：起草+提交（origin_kind=repo_analysis；AI 主体可达）
    let task1 = Uuid::new_v4();
    let first = propose_from_analysis(&pool, project, input_for(task1)).await.unwrap();
    let AnalysisProposalOutcome::Proposed(receipt) = first else {
        panic!("expected Proposed");
    };
    assert_eq!(receipt.member_count, 1);
    let (origin, status): (String, String) = sqlx::query_as(
        "SELECT origin_kind, status FROM projc.proposal WHERE id = $1",
    )
    .bind(receipt.proposal_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(origin, "repo_analysis");
    assert_eq!(status, "submitted");

    // 同修订被有效提案覆盖 → NoChange（draft/submitted 待审即覆盖）
    let rerun = propose_from_analysis(&pool, project, input_for(Uuid::new_v4()))
        .await
        .unwrap();
    assert!(matches!(rerun, AnalysisProposalOutcome::NoChange), "{rerun:?}");

    // approved/applied 已收 → 仍覆盖；发布后版本已含 → 排除（口径①）
    approve_proposal(
        &pool,
        project,
        &lead,
        receipt.proposal_id,
        pm_application::projc::DecisionInput {
            reason: None,
            expected_version_no: 0,
        },
    )
    .await
    .unwrap();
    publish_version(
        &pool,
        project,
        &lead,
        PublishVersionInput {
            proposal_id: receipt.proposal_id,
            operation_id: "candidate-test".into(),
        },
    )
    .await
    .unwrap();
    let after_publish = propose_from_analysis(&pool, project, input_for(Uuid::new_v4()))
        .await
        .unwrap();
    assert!(
        matches!(after_publish, AnalysisProposalOutcome::NoChange),
        "version already contains the pinned revision"
    );

    // 新修订（revision 2，current 指针前移）→ 重新入候选（update_object）
    let obj2 = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("tech_stack", Some("repo:x:tech_stack"), "技术栈", "rust+go"),
    )
    .await
    .unwrap();
    assert!(obj2.revision_created);
    assert_eq!(obj2.revision_no, 2);
    register_evidence(
        &pool,
        project,
        &lead,
        EvidenceInput {
            source_id: source,
            subject: EvidenceSubject::ObjectRevision(obj2.revision_id),
            evidence_type: "document_section".into(),
            target_domain: Some("knowledge_base".into()),
            target_type: Some("document".into()),
            target_id: Some(kb_doc.to_string()),
            target_revision_id: Some(kb_rev.to_string()),
            file_path: None,
            symbol: None,
            code_range: None,
            commit_sha: None,
            section_anchor: Some("tech_stack".into()),
            start_line: None,
            end_line: None,
            excerpt: None,
            description: None,
            evidence_hash: "h2".into(),
            confidence: None,
        },
    )
    .await
    .unwrap();
    let second = propose_from_analysis(&pool, project, input_for(Uuid::new_v4()))
        .await
        .unwrap();
    let AnalysisProposalOutcome::Proposed(receipt2) = second else {
        panic!("expected Proposed for new revision");
    };
    let (change_kind,): (String,) = sqlx::query_as(
        "SELECT change_kind FROM projc.proposal_member WHERE proposal_id = $1",
    )
    .bind(receipt2.proposal_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(change_kind, "update_object", "已正式化对象走 update_object");

    // rejected 亦阻断（已人工裁决同一内容）；failed 放行
    reject_proposal(
        &pool,
        project,
        &lead,
        receipt2.proposal_id,
        pm_application::projc::DecisionInput {
            reason: Some("不需要".into()),
            expected_version_no: 1,
        },
    )
    .await
    .unwrap();
    let after_reject = propose_from_analysis(&pool, project, input_for(Uuid::new_v4()))
        .await
        .unwrap();
    assert!(
        matches!(after_reject, AnalysisProposalOutcome::NoChange),
        "rejected proposal covers the same content"
    );

    // 幂等键：同任务重投递不追加第二种结论（outbox 幂等坍缩）
    let (event_rows,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM integration.outbox_event WHERE idempotency_key = $1",
    )
    .bind(format!("projc_analysis:{task1}"))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event_rows, 1);
}
