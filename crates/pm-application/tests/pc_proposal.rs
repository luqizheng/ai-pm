//! KP-2 · PC-01C 确认闭环服务层集成测试（pm_application::pc 提案/审批命令；真实 DB）。
//!
//! 断言面：
//! 1. 全链：draft → 提案（F-05 端点并入）→ 提交（pending 审批锚定）→ 审批 → 发布 → 版本；
//!    第二轮经 P5 编排（审批 + 发布）替换携带修订；决策幂等；P4 读面。
//! 2. 门禁矩阵：AI/服务主体 403、非成员 403、普通成员 403、显式 Approver 同权（A3）、
//!    L3 冻结（submit/approve 双落点）403、请求侧 CAS 409、锚定篡改 P0001、pending 重复 23505。
//! 3. 起草校验：origin/成员维度/同身份/归属/R7 等级下限/生命周期。
//! 4. F-05 冲突与闭合预检：端点修订冲突拒绝；携带关系端点改钉须同提案处置该关系。
//! 5. 驳回与撤回：rejected 终态、撤回权限、撤回后裁决 409；AI 可提案不可确认。
//! 6. 分析起草：未覆盖修订 → AI 提案（submitted + change_proposed 事件）；同修订集幂等
//!    NO_CHANGE；人工驳回仍覆盖；failed 释放覆盖。
//! 7. Profile/Vision 单例骨架：项目创建即建 identity 态两单例（PC-D-06）；可重入；
//!    单例唯一索引兜底；骨架可被草稿填充后进入确认链。

use sqlx::PgPool;
use uuid::Uuid;

use pm_application::actor::AuditActor;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::pc::{
    analysis_outcome_key, approve_proposal, canonical_proposal_hash, create_proposal,
    create_relationship_draft, decide_proposal, ensure_product_singletons, load_proposal,
    propose_from_analysis, publish_version, register_evidence, register_source, reject_proposal,
    submit_proposal, upsert_object_draft, withdraw_proposal, AnalysisProposalInput,
    AnalysisProposalOutcome, CreateProposalInput, DecisionAction, DecisionInput, EvidenceInput,
    EvidenceSubject, MemberKind, ObjectUpsertReceipt, PcEndpoint, PcError, ProposalDecisionInput,
    ProposalMemberPin, PublishVersionInput, RelationshipDraftInput, SourceInput, UpsertObjectDraft,
    VersionMemberPin,
};

const LEAD: Uuid = Uuid::from_u128(0x7272_0000_0000_0000_0000_0000_0000_0001);
const AI_BOT: Uuid = Uuid::from_u128(0x7272_0000_0000_0000_0000_0000_0000_0002);
const OUTSIDER: Uuid = Uuid::from_u128(0x7272_0000_0000_0000_0000_0000_0000_0003);
const MEMBER: Uuid = Uuid::from_u128(0x7272_0000_0000_0000_0000_0000_0000_0004);
const APPROVER: Uuid = Uuid::from_u128(0x7272_0000_0000_0000_0000_0000_0000_0005);
/// 迁移种子 `ai-pm-dispatch`（kind='service'）——分析链起草主体。
const DISPATCH: Uuid = Uuid::from_u128(1);

/// 项目 + 团队：LEAD=owner、MEMBER=普通成员、APPROVER=成员 + 显式授权；
/// OUTSIDER 与 AI_BOT 不入项目。返回 (tenant, project)。
async fn seed_project_with_team(pool: &PgPool, code: &str) -> (Uuid, Uuid) {
    for (id, kind, name) in [
        (LEAD, "human", "kp2-lead"),
        (AI_BOT, "dev_agent", "kp2-ai"),
        (OUTSIDER, "human", "kp2-outsider"),
        (MEMBER, "human", "kp2-member"),
        (APPROVER, "human", "kp2-approver"),
    ] {
        sqlx::query(
            "INSERT INTO core.principal (id, kind, display_name, is_active) \
             VALUES ($1, $2, $3, true) ON CONFLICT (id) DO NOTHING",
        )
        .bind(id)
        .bind(kind)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    }
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('system', $1, $1) RETURNING id",
    )
    .bind(format!("kp2-{code}-{}", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .unwrap();
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(format!("kp2-c-{code}"))
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
    for (principal, role) in [(LEAD, "owner"), (MEMBER, "member"), (APPROVER, "member")] {
        sqlx::query(
            "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(tenant)
        .bind(project)
        .bind(principal)
        .bind(role)
        .execute(pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO core.approver_grant (tenant_id, project_id, principal_id, granted_by) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(tenant)
    .bind(project)
    .bind(APPROVER)
    .bind(LEAD)
    .execute(pool)
    .await
    .unwrap();
    (tenant, project)
}

fn draft(object_type: &str, key: &str, name: &str, content: &str) -> UpsertObjectDraft {
    UpsertObjectDraft {
        object_type: object_type.to_string(),
        object_key: Some(key.to_string()),
        display_name: name.to_string(),
        content_markdown: content.to_string(),
        change_summary: Some("kp2 draft".into()),
        source_context: None,
        analysis_mode: None,
    }
}

async fn upsert(
    pool: &PgPool,
    project: Uuid,
    actor: &AuditActor,
    input: UpsertObjectDraft,
) -> ObjectUpsertReceipt {
    upsert_object_draft(pool, project, actor, input)
        .await
        .expect("upsert draft")
}

fn obj_member(change_kind: &str, receipt: &ObjectUpsertReceipt) -> ProposalMemberPin {
    ProposalMemberPin {
        change_kind: change_kind.to_string(),
        object_id: Some(receipt.object_id),
        object_revision_id: Some(receipt.revision_id),
        relationship_id: None,
        relationship_revision_id: None,
    }
}

fn rel_member(change_kind: &str, relationship_id: Uuid, revision_id: Uuid) -> ProposalMemberPin {
    ProposalMemberPin {
        change_kind: change_kind.to_string(),
        object_id: None,
        object_revision_id: None,
        relationship_id: Some(relationship_id),
        relationship_revision_id: Some(revision_id),
    }
}

fn proposal_input(title: &str, members: Vec<ProposalMemberPin>) -> CreateProposalInput {
    CreateProposalInput {
        title: title.to_string(),
        rationale: "kp2 test rationale".into(),
        origin_kind: "manual_input".into(),
        source_id: None,
        governance_level: None,
        risk_level: None,
        confidence: None,
        members,
    }
}

fn decision(expected_version_no: i64, reason: Option<&str>) -> DecisionInput {
    DecisionInput {
        reason: reason.map(str::to_string),
        expected_version_no,
    }
}

fn approve(expected_version_no: i64) -> ProposalDecisionInput {
    ProposalDecisionInput {
        action: DecisionAction::Approve,
        reason: None,
        expected_version_no,
    }
}

fn pin(kind: MemberKind, identity_id: Uuid, revision_id: Uuid) -> VersionMemberPin {
    VersionMemberPin {
        kind,
        identity_id,
        revision_id,
    }
}

fn db_code(err: &sqlx::Error) -> String {
    err.as_database_error()
        .and_then(|e| e.code())
        .map(|c| c.to_string())
        .unwrap_or_default()
}

async fn uses_relationship(
    pool: &PgPool,
    project: Uuid,
    actor: &AuditActor,
    scenario: &ObjectUpsertReceipt,
    feature: &ObjectUpsertReceipt,
) -> (Uuid, Uuid) {
    create_relationship_draft(
        pool,
        project,
        actor,
        RelationshipDraftInput {
            relationship_type: "uses".into(),
            source: PcEndpoint::Object {
                object_id: scenario.object_id,
                revision_id: Some(scenario.revision_id),
            },
            target: PcEndpoint::Object {
                object_id: feature.object_id,
                revision_id: Some(feature.revision_id),
            },
            properties: serde_json::json!({}),
            change_summary: None,
        },
    )
    .await
    .expect("relationship draft")
}

/// 版本对象成员 (object_id, revision_id) 与关系成员 (relationship_id, revision_id)。
async fn version_members(
    pool: &PgPool,
    version_id: Uuid,
) -> (Vec<(Uuid, Uuid)>, Vec<(Uuid, Uuid)>) {
    let objects = sqlx::query_as(
        "SELECT object_id, object_revision_id FROM pc.version_member \
          WHERE version_id = $1 AND object_id IS NOT NULL",
    )
    .bind(version_id)
    .fetch_all(pool)
    .await
    .unwrap();
    let relationships = sqlx::query_as(
        "SELECT relationship_id, relationship_revision_id FROM pc.version_member \
          WHERE version_id = $1 AND relationship_id IS NOT NULL",
    )
    .bind(version_id)
    .fetch_all(pool)
    .await
    .unwrap();
    (objects, relationships)
}

async fn audit_count(pool: &PgPool, project: Uuid, action: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log WHERE project_id = $1 AND action = $2",
    )
    .bind(project)
    .bind(action)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn proposal_status(pool: &PgPool, proposal_id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM pc.proposal WHERE id = $1")
        .bind(proposal_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn conflict_on_object(pool: &PgPool, tenant: Uuid, project: Uuid, object_id: Uuid) -> Uuid {
    let conflict: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.conflict \
         (tenant_id, project_id, conflict_type, severity, description, status, created_by) \
         VALUES ($1, $2, 'scope_conflict', 'major', 'kp2 freeze', 'detected', $3) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(LEAD)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO pc.conflict_object (tenant_id, project_id, conflict_id, object_id) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(tenant)
    .bind(project)
    .bind(conflict)
    .bind(object_id)
    .execute(pool)
    .await
    .unwrap();
    conflict
}

// ══════════════════════════════════════════════════════════════════════════
// 1. 全链
// ══════════════════════════════════════════════════════════════════════════

#[sqlx::test(migrations = "../../migrations")]
async fn confirmation_loop_full_chain(pool: PgPool) {
    let (_, project) = seed_project_with_team(&pool, "kp2-chain").await;
    let lead = AuditActor::principal(LEAD);

    let a = upsert(&pool, project, &lead, draft("feature", "fa", "A", "# A1")).await;
    let b = upsert(&pool, project, &lead, draft("scenario", "sb", "B", "# B1")).await;
    let c = upsert(&pool, project, &lead, draft("feature", "fc", "C", "# C1")).await;
    let (rel, rr) = uses_relationship(&pool, project, &lead, &b, &a).await;

    // ① 起草：显式成员 A / R / C，关系端点 B 由 F-05 自动并入
    let drafted = create_proposal(
        &pool,
        project,
        &lead,
        proposal_input(
            "首版本认知",
            vec![
                obj_member("create_object", &a),
                rel_member("create_relationship", rel, rr),
                obj_member("create_object", &c),
            ],
        ),
    )
    .await
    .unwrap();
    assert_eq!(drafted.status, "draft");
    assert_eq!(drafted.governance_level, "L1");
    assert_eq!(drafted.expected_version_no, 0);
    assert_eq!(drafted.member_count, 4, "endpoint B auto-merged (F-05)");
    let merged_kind: String = sqlx::query_scalar(
        "SELECT change_kind FROM pc.proposal_member WHERE proposal_id = $1 AND object_id = $2",
    )
    .bind(drafted.proposal_id)
    .bind(b.object_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(merged_kind, "update_object");
    let expected_hash = canonical_proposal_hash(
        "首版本认知",
        "kp2 test rationale",
        "manual_input",
        "L1",
        0,
        &[
            pin(MemberKind::Object, a.object_id, a.revision_id),
            pin(MemberKind::Object, b.object_id, b.revision_id),
            pin(MemberKind::Object, c.object_id, c.revision_id),
            pin(MemberKind::Relationship, rel, rr),
        ],
    );
    assert_eq!(
        drafted.proposal_content_hash, expected_hash,
        "content hash covers the auto-merged endpoint (T-29)"
    );

    // ② 提交：pending 审批行锚定 content_hash + expected_version_no
    let submitted = submit_proposal(&pool, project, &lead, drafted.proposal_id, Some(0))
        .await
        .unwrap();
    assert_eq!(submitted.status, "submitted");
    let (approval_status, approval_hash, approval_expected): (String, String, i64) =
        sqlx::query_as(
            "SELECT status, proposal_content_hash, expected_version_no FROM pc.approval \
              WHERE proposal_id = $1",
        )
        .bind(drafted.proposal_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(approval_status, "pending");
    assert_eq!(approval_hash, expected_hash);
    assert_eq!(approval_expected, 0);

    // ③ 审批（真人 Lead）
    let approval = approve_proposal(
        &pool,
        project,
        &lead,
        drafted.proposal_id,
        decision(0, Some("认知准确")),
    )
    .await
    .unwrap();
    assert_eq!(approval.status, "approved");
    let (status, decided_by, reason): (String, Option<Uuid>, Option<String>) =
        sqlx::query_as("SELECT status, decided_by, decision_reason FROM pc.approval WHERE id = $1")
            .bind(approval.approval_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "approved");
    assert_eq!(decided_by, Some(LEAD));
    assert_eq!(reason.as_deref(), Some("认知准确"));
    assert_eq!(
        proposal_status(&pool, drafted.proposal_id).await,
        "approved"
    );

    // ④ 发布 → 版本 1（成员 4：A/B/C + R）
    let v1 = publish_version(
        &pool,
        project,
        &lead,
        PublishVersionInput {
            proposal_id: drafted.proposal_id,
            operation_id: "kp2-chain-1".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(v1.version_no, 1);
    assert_eq!(v1.member_count, 4);
    assert_eq!(proposal_status(&pool, drafted.proposal_id).await, "applied");
    let active: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pc.cognition_object WHERE project_id = $1 AND lifecycle_status = 'active'",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(active, 3, "A/B/C formalized");
    for action in [
        "pc_proposal.create",
        "pc_proposal.submit",
        "pc_proposal.approve",
        "pc_version.publish",
    ] {
        assert_eq!(
            audit_count(&pool, project, action).await,
            1,
            "audit {action}"
        );
    }
    let actor_audit: (Uuid, String) = sqlx::query_as(
        "SELECT actor_id, source_kind FROM governance.audit_log \
          WHERE project_id = $1 AND action = 'pc_proposal.approve'",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(actor_audit, (LEAD, "pc_proposal".to_string()));

    // ⑤ 第二轮（P5 编排 = 审批 + 发布）：C@2 替换携带的 C@1
    let c2 = upsert(&pool, project, &lead, draft("feature", "fc", "C", "# C2")).await;
    let p2 = create_proposal(
        &pool,
        project,
        &lead,
        proposal_input("C 更新", vec![obj_member("update_object", &c2)]),
    )
    .await
    .unwrap();
    assert_eq!(p2.expected_version_no, 1);
    submit_proposal(&pool, project, &lead, p2.proposal_id, Some(1))
        .await
        .unwrap();
    // ⑤' 决定暂记：approve → approved（无版本）；正式版本经发布入口产出
    let outcome = decide_proposal(&pool, project, &lead, p2.proposal_id, approve(1))
        .await
        .unwrap();
    assert_eq!(outcome.status, "approved");
    assert!(outcome.published_version.is_none());
    // 同态重放幂等（发布前）：不产生新版本、不重复审计
    let again = decide_proposal(&pool, project, &lead, p2.proposal_id, approve(1))
        .await
        .unwrap();
    assert_eq!(again.status, "approved");
    assert!(again.published_version.is_none());
    let v2 = publish_version(
        &pool,
        project,
        &lead,
        PublishVersionInput {
            proposal_id: p2.proposal_id,
            operation_id: format!("pc-decision:{}", p2.proposal_id),
        },
    )
    .await
    .unwrap();
    assert_eq!(v2.version_no, 2);
    assert!(!v2.replayed);
    let (objects, relationships) = version_members(&pool, v2.version_id).await;
    assert_eq!(objects.len(), 3, "one revision per identity: {objects:?}");
    assert!(objects.contains(&(c2.object_id, c2.revision_id)));
    assert!(!objects.contains(&(c.object_id, c.revision_id)));
    assert_eq!(relationships, vec![(rel, rr)], "relationship carried");

    let versions: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pc.cognition_version WHERE project_id = $1")
            .bind(project)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(versions, 2);

    // ⑥ P4 读面：提案头 + 成员展示投影 + 审批历史 + 已发布版本
    let view = load_proposal(&pool, project, drafted.proposal_id)
        .await
        .unwrap();
    assert_eq!(view.head.status, "applied");
    assert_eq!(view.head.proposal_content_hash, expected_hash);
    assert_eq!(view.members.len(), 4);
    assert!(view
        .members
        .iter()
        .any(|m| m.display_name.as_deref() == Some("A") && m.object_revision_no == Some(1)));
    assert!(view
        .members
        .iter()
        .any(|m| m.relationship_type.as_deref() == Some("uses")));
    assert_eq!(view.approvals.len(), 1);
    assert_eq!(view.approvals[0].status, "approved");
    assert_eq!(
        view.approvals[0].decided_by_name.as_deref(),
        Some("kp2-lead")
    );
    assert_eq!(view.published_version.unwrap().version_no, 1);
    assert!(matches!(
        load_proposal(&pool, project, Uuid::new_v4()).await,
        Err(PcError::NotFound(_))
    ));

    // ⑦ 历史版本不可覆盖（T-12/T-23）：版本头锚定列改写、版本删除均被库层拒绝；
    // 新版本发布后旧版本成员原样保留
    let err =
        sqlx::query("UPDATE pc.cognition_version SET snapshot_hash = 'tampered' WHERE id = $1")
            .bind(v1.version_id)
            .execute(&pool)
            .await
            .unwrap_err();
    assert_eq!(db_code(&err), "P0001", "version anchor: {err}");
    let err = sqlx::query("DELETE FROM pc.cognition_version WHERE id = $1")
        .bind(v1.version_id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert_eq!(db_code(&err), "P0001", "version delete: {err}");
    let (v1_objects, v1_relationships) = version_members(&pool, v1.version_id).await;
    assert_eq!(v1_objects.len(), 3);
    assert!(
        v1_objects.contains(&(c.object_id, c.revision_id)),
        "v1 keeps C@1"
    );
    assert_eq!(v1_relationships, vec![(rel, rr)]);
}

// ══════════════════════════════════════════════════════════════════════════
// 2. 门禁矩阵（fail-closed）
// ══════════════════════════════════════════════════════════════════════════

#[sqlx::test(migrations = "../../migrations")]
async fn decision_gates_fail_closed(pool: PgPool) {
    let (tenant, project) = seed_project_with_team(&pool, "kp2-gates").await;
    let lead = AuditActor::principal(LEAD);

    let a = upsert(&pool, project, &lead, draft("feature", "ga", "A", "# A1")).await;
    let p = create_proposal(
        &pool,
        project,
        &lead,
        proposal_input("gates", vec![obj_member("create_object", &a)]),
    )
    .await
    .unwrap();
    submit_proposal(&pool, project, &lead, p.proposal_id, None)
        .await
        .unwrap();

    // AI / 服务主体 / 非成员真人 / 普通成员 → 403（approve、reject、P5 同门）
    for (actor, label) in [
        (AuditActor::principal(AI_BOT), "dev_agent"),
        (AuditActor::dispatch(), "service"),
        (AuditActor::principal(OUTSIDER), "outsider"),
        (AuditActor::principal(MEMBER), "plain member"),
    ] {
        let r = approve_proposal(&pool, project, &actor, p.proposal_id, decision(0, None)).await;
        assert!(
            matches!(r, Err(PcError::Forbidden(_))),
            "{label} approve: {r:?}"
        );
        let r = reject_proposal(&pool, project, &actor, p.proposal_id, decision(0, None)).await;
        assert!(
            matches!(r, Err(PcError::Forbidden(_))),
            "{label} reject: {r:?}"
        );
        let r = decide_proposal(&pool, project, &actor, p.proposal_id, approve(0)).await;
        assert!(
            matches!(r, Err(PcError::Forbidden(_))),
            "{label} decide: {r:?}"
        );
    }
    assert_eq!(proposal_status(&pool, p.proposal_id).await, "submitted");

    // 请求侧 CAS：调用方基线与提案锚定不符 → 409
    let r = approve_proposal(&pool, project, &lead, p.proposal_id, decision(7, None)).await;
    assert!(matches!(r, Err(PcError::Conflict(_))), "stale CAS: {r:?}");

    // L3 冻结（approve 侧）→ 403；解冻后显式 Approver 同权通过（A3 / Q-PC-08）
    let conflict = conflict_on_object(&pool, tenant, project, a.object_id).await;
    let r = approve_proposal(&pool, project, &lead, p.proposal_id, decision(0, None)).await;
    assert!(
        matches!(r, Err(PcError::Forbidden(_))),
        "frozen approve: {r:?}"
    );
    sqlx::query(
        "UPDATE pc.conflict SET status = 'resolved', resolved_by = $2, resolved_at = now() \
          WHERE id = $1",
    )
    .bind(conflict)
    .bind(LEAD)
    .execute(&pool)
    .await
    .unwrap();
    let approved = approve_proposal(
        &pool,
        project,
        &AuditActor::principal(APPROVER),
        p.proposal_id,
        decision(0, Some("Approver 同权")),
    )
    .await
    .unwrap();
    assert_eq!(approved.status, "approved");

    // 审阅流 v2：同态重放幂等；反向重决互改（报告 close 前决定可反复修改）
    let r = approve_proposal(&pool, project, &lead, p.proposal_id, decision(0, None)).await;
    assert!(r.is_ok(), "idempotent re-approve: {r:?}");
    let r = reject_proposal(&pool, project, &lead, p.proposal_id, decision(0, None)).await;
    assert!(r.is_ok(), "re-decide approve→reject: {r:?}");
    assert_eq!(proposal_status(&pool, p.proposal_id).await, "rejected");
    let r = approve_proposal(&pool, project, &lead, p.proposal_id, decision(0, None)).await;
    assert!(r.is_ok(), "re-decide reject→approve: {r:?}");
    assert_eq!(proposal_status(&pool, p.proposal_id).await, "approved");

    // 锚定篡改：库层触发器拒绝（P0001）
    let err = sqlx::query("UPDATE pc.proposal SET title = 'tampered' WHERE id = $1")
        .bind(p.proposal_id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert_eq!(db_code(&err), "P0001", "proposal anchor: {err}");
    let err =
        sqlx::query("UPDATE pc.approval SET proposal_content_hash = 'tampered' WHERE id = $1")
            .bind(approved.approval_id)
            .execute(&pool)
            .await
            .unwrap_err();
    assert_eq!(db_code(&err), "P0001", "approval anchor: {err}");
    let err = sqlx::query("UPDATE pc.approval SET status = 'rejected' WHERE id = $1")
        .bind(approved.approval_id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert_eq!(
        db_code(&err),
        "P0001",
        "decided approval is immutable: {err}"
    );

    // pending 重复：uq_pc_approval_pending（23505）
    let b = upsert(&pool, project, &lead, draft("feature", "gb", "B", "# B1")).await;
    let p2 = create_proposal(
        &pool,
        project,
        &lead,
        proposal_input("pending dup", vec![obj_member("create_object", &b)]),
    )
    .await
    .unwrap();
    submit_proposal(&pool, project, &lead, p2.proposal_id, Some(0))
        .await
        .unwrap();
    let err = sqlx::query(
        "INSERT INTO pc.approval \
         (tenant_id, project_id, proposal_id, proposal_content_hash, expected_version_no) \
         VALUES ($1, $2, $3, 'dup', 0)",
    )
    .bind(tenant)
    .bind(project)
    .bind(p2.proposal_id)
    .execute(&pool)
    .await
    .unwrap_err();
    assert_eq!(db_code(&err), "23505", "second pending approval: {err}");
    // 重复提交 → 409
    let r = submit_proposal(&pool, project, &lead, p2.proposal_id, None).await;
    assert!(matches!(r, Err(PcError::Conflict(_))));

    // L3 冻结（submit 侧，§10.3）→ 403
    let c = upsert(&pool, project, &lead, draft("feature", "gc", "C", "# C1")).await;
    let p3 = create_proposal(
        &pool,
        project,
        &lead,
        proposal_input("frozen submit", vec![obj_member("create_object", &c)]),
    )
    .await
    .unwrap();
    conflict_on_object(&pool, tenant, project, c.object_id).await;
    let r = submit_proposal(&pool, project, &lead, p3.proposal_id, None).await;
    assert!(
        matches!(r, Err(PcError::Forbidden(_))),
        "frozen submit: {r:?}"
    );
    assert_eq!(proposal_status(&pool, p3.proposal_id).await, "draft");
}

// ══════════════════════════════════════════════════════════════════════════
// 3. 起草校验
// ══════════════════════════════════════════════════════════════════════════

#[sqlx::test(migrations = "../../migrations")]
async fn drafting_validates_members_and_levels(pool: PgPool) {
    let (_, project) = seed_project_with_team(&pool, "kp2-val").await;
    let lead = AuditActor::principal(LEAD);
    let a1 = upsert(&pool, project, &lead, draft("feature", "va", "A", "# A1")).await;
    let a2 = upsert(&pool, project, &lead, draft("feature", "va", "A", "# A2")).await;
    let other = upsert(&pool, project, &lead, draft("feature", "vo", "O", "# O1")).await;
    let rule = upsert(
        &pool,
        project,
        &lead,
        draft("business_rule", "vr", "R", "# R1"),
    )
    .await;

    let expect_invalid = |r: Result<_, PcError>, label: &str| match r {
        Err(PcError::Invalid(_)) => {}
        other => panic!("{label}: expected Invalid, got {other:?}"),
    };

    let mut bad_origin = proposal_input("t", vec![obj_member("create_object", &a2)]);
    bad_origin.origin_kind = "robot".into();
    expect_invalid(
        create_proposal(&pool, project, &lead, bad_origin).await,
        "origin_kind",
    );
    expect_invalid(
        create_proposal(&pool, project, &lead, proposal_input("t", vec![])).await,
        "empty members",
    );
    let mut mixed = obj_member("create_object", &a2);
    mixed.relationship_id = Some(Uuid::new_v4());
    expect_invalid(
        create_proposal(&pool, project, &lead, proposal_input("t", vec![mixed])).await,
        "mixed dimensions",
    );
    expect_invalid(
        create_proposal(
            &pool,
            project,
            &lead,
            proposal_input("t", vec![obj_member("rename_object", &a2)]),
        )
        .await,
        "unknown change_kind",
    );
    expect_invalid(
        create_proposal(
            &pool,
            project,
            &lead,
            proposal_input(
                "t",
                vec![
                    obj_member("create_object", &a1),
                    obj_member("update_object", &a2),
                ],
            ),
        )
        .await,
        "two revisions of one identity",
    );
    // 修订不属于对象 → NotFound
    let foreign = ProposalMemberPin {
        change_kind: "create_object".into(),
        object_id: Some(other.object_id),
        object_revision_id: Some(a2.revision_id),
        relationship_id: None,
        relationship_revision_id: None,
    };
    let r = create_proposal(&pool, project, &lead, proposal_input("t", vec![foreign])).await;
    assert!(matches!(r, Err(PcError::NotFound(_))), "{r:?}");

    // R7/F-07：business_rule 固定 L2——显式 L1 拒绝，缺省推导 L2
    let mut low = proposal_input("rule", vec![obj_member("create_object", &rule)]);
    low.governance_level = Some("L1".into());
    expect_invalid(
        create_proposal(&pool, project, &lead, low).await,
        "L1 floor",
    );
    let derived = create_proposal(
        &pool,
        project,
        &lead,
        proposal_input("rule", vec![obj_member("create_object", &rule)]),
    )
    .await
    .unwrap();
    assert_eq!(derived.governance_level, "L2");
    let mut raised = proposal_input("raised", vec![obj_member("create_object", &a2)]);
    raised.governance_level = Some("L2".into());
    assert_eq!(
        create_proposal(&pool, project, &lead, raised)
            .await
            .unwrap()
            .governance_level,
        "L2",
        "raising above the floor is allowed"
    );

    // 生命周期：未正式化对象不可废弃
    expect_invalid(
        create_proposal(
            &pool,
            project,
            &lead,
            proposal_input("t", vec![obj_member("deprecate_object", &a2)]),
        )
        .await,
        "deprecate draft",
    );
    // 未知来源 → NotFound
    let mut sourced = proposal_input("t", vec![obj_member("create_object", &other)]);
    sourced.source_id = Some(Uuid::new_v4());
    let r = create_proposal(&pool, project, &lead, sourced).await;
    assert!(matches!(r, Err(PcError::NotFound(_))), "{r:?}");
}

// ══════════════════════════════════════════════════════════════════════════
// 4. F-05 冲突与闭合预检
// ══════════════════════════════════════════════════════════════════════════

#[sqlx::test(migrations = "../../migrations")]
async fn endpoint_merge_conflict_and_projected_closure(pool: PgPool) {
    let (_, project) = seed_project_with_team(&pool, "kp2-close").await;
    let lead = AuditActor::principal(LEAD);
    let a = upsert(&pool, project, &lead, draft("feature", "ea", "A", "# A1")).await;
    let b = upsert(&pool, project, &lead, draft("scenario", "eb", "B", "# B1")).await;
    let (rel, rr) = uses_relationship(&pool, project, &lead, &b, &a).await;
    let a2 = upsert(&pool, project, &lead, draft("feature", "ea", "A", "# A2")).await;

    // F-05 冲突：关系端点钉 A@1，提案另钉 A@2 → 拒绝
    let r = create_proposal(
        &pool,
        project,
        &lead,
        proposal_input(
            "conflicting endpoint",
            vec![
                rel_member("create_relationship", rel, rr),
                obj_member("update_object", &a2),
            ],
        ),
    )
    .await;
    assert!(matches!(r, Err(PcError::Invalid(_))), "{r:?}");

    // 只提关系：端点 A@1 / B@1 自动并入 → 发布版本 1（3 成员）
    let p1 = create_proposal(
        &pool,
        project,
        &lead,
        proposal_input(
            "relationship only",
            vec![rel_member("create_relationship", rel, rr)],
        ),
    )
    .await
    .unwrap();
    assert_eq!(p1.member_count, 3);
    submit_proposal(&pool, project, &lead, p1.proposal_id, Some(0))
        .await
        .unwrap();
    decide_proposal(&pool, project, &lead, p1.proposal_id, approve(0))
        .await
        .unwrap();
    let v1 = publish_version(
        &pool,
        project,
        &lead,
        PublishVersionInput {
            proposal_id: p1.proposal_id,
            operation_id: format!("pc-decision:{}", p1.proposal_id),
        },
    )
    .await
    .unwrap();
    assert_eq!(v1.member_count, 3);

    // 闭合预检：携带关系 R 钉 A@1，单独改钉 A@2 → 拒绝（否则发布闭合断言必败）
    let r = create_proposal(
        &pool,
        project,
        &lead,
        proposal_input("dangling", vec![obj_member("update_object", &a2)]),
    )
    .await;
    assert!(matches!(r, Err(PcError::Invalid(_))), "{r:?}");
    // 已正式化对象不可再 create
    let r = create_proposal(
        &pool,
        project,
        &lead,
        proposal_input("recreate", vec![obj_member("create_object", &a)]),
    )
    .await;
    assert!(matches!(r, Err(PcError::Invalid(_))), "{r:?}");

    // 同提案废弃 R + 更新 A@2 → 通过；版本 2 = {A@2, B@1}，R 废弃离开版本
    let p2 = create_proposal(
        &pool,
        project,
        &lead,
        proposal_input(
            "retire relationship",
            vec![
                obj_member("update_object", &a2),
                rel_member("deprecate_relationship", rel, rr),
            ],
        ),
    )
    .await
    .unwrap();
    submit_proposal(&pool, project, &lead, p2.proposal_id, Some(1))
        .await
        .unwrap();
    decide_proposal(&pool, project, &lead, p2.proposal_id, approve(1))
        .await
        .unwrap();
    let v2 = publish_version(
        &pool,
        project,
        &lead,
        PublishVersionInput {
            proposal_id: p2.proposal_id,
            operation_id: format!("pc-decision:{}", p2.proposal_id),
        },
    )
    .await
    .unwrap();
    let (objects, relationships) = version_members(&pool, v2.version_id).await;
    assert_eq!(objects.len(), 2, "{objects:?}");
    assert!(objects.contains(&(a2.object_id, a2.revision_id)));
    assert!(objects.contains(&(b.object_id, b.revision_id)));
    assert!(
        relationships.is_empty(),
        "deprecated relationship leaves the version"
    );
    let rel_status: String =
        sqlx::query_scalar("SELECT lifecycle_status FROM pc.relationship WHERE id = $1")
            .bind(rel)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rel_status, "deprecated");
}

// ══════════════════════════════════════════════════════════════════════════
// 5. 驳回与撤回
// ══════════════════════════════════════════════════════════════════════════

#[sqlx::test(migrations = "../../migrations")]
async fn reject_and_withdraw_paths(pool: PgPool) {
    let (_, project) = seed_project_with_team(&pool, "kp2-rw").await;
    let lead = AuditActor::principal(LEAD);
    let ai = AuditActor::principal(AI_BOT);
    let a = upsert(&pool, project, &lead, draft("feature", "ra", "A", "# A1")).await;
    let b = upsert(&pool, project, &lead, draft("feature", "rb", "B", "# B1")).await;
    let c = upsert(&pool, project, &lead, draft("feature", "rc", "C", "# C1")).await;

    // AI 可提案（create + submit），不可确认（Q2-C）
    let p1 = create_proposal(
        &pool,
        project,
        &ai,
        proposal_input("AI 提议", vec![obj_member("create_object", &a)]),
    )
    .await
    .unwrap();
    submit_proposal(&pool, project, &ai, p1.proposal_id, Some(0))
        .await
        .unwrap();
    let r = approve_proposal(&pool, project, &ai, p1.proposal_id, decision(0, None)).await;
    assert!(matches!(r, Err(PcError::Forbidden(_))));

    // 驳回：终态 + 审批事实带理由
    let rejected = reject_proposal(
        &pool,
        project,
        &lead,
        p1.proposal_id,
        decision(0, Some("不符合产品定位")),
    )
    .await
    .unwrap();
    assert_eq!(rejected.status, "rejected");
    let (status, reason): (String, Option<String>) =
        sqlx::query_as("SELECT status, decision_reason FROM pc.approval WHERE id = $1")
            .bind(rejected.approval_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "rejected");
    assert_eq!(reason.as_deref(), Some("不符合产品定位"));
    assert_eq!(proposal_status(&pool, p1.proposal_id).await, "rejected");
    // 审阅流 v2：rejected 仍可互改（同态重放幂等；approve 反转回 approved）
    let r = reject_proposal(&pool, project, &lead, p1.proposal_id, decision(0, None)).await;
    assert!(r.is_ok(), "idempotent re-reject: {r:?}");
    let reapproved = decide_proposal(&pool, project, &lead, p1.proposal_id, approve(0))
        .await
        .unwrap();
    assert_eq!(reapproved.status, "approved");
    assert_eq!(proposal_status(&pool, p1.proposal_id).await, "approved");

    // 撤回：本人撤回 draft
    let p2 = create_proposal(
        &pool,
        project,
        &lead,
        proposal_input("draft to withdraw", vec![obj_member("create_object", &b)]),
    )
    .await
    .unwrap();
    let withdrawn = withdraw_proposal(&pool, project, &lead, p2.proposal_id, None)
        .await
        .unwrap();
    assert_eq!(withdrawn.status, "withdrawn");

    // 非本人且非授权 → 403；授权人撤回 submitted（pending 审批行原样保留、不可再裁决）
    let p3 = create_proposal(
        &pool,
        project,
        &ai,
        proposal_input(
            "submitted to withdraw",
            vec![obj_member("create_object", &c)],
        ),
    )
    .await
    .unwrap();
    submit_proposal(&pool, project, &ai, p3.proposal_id, None)
        .await
        .unwrap();
    let r = withdraw_proposal(
        &pool,
        project,
        &AuditActor::principal(OUTSIDER),
        p3.proposal_id,
        None,
    )
    .await;
    assert!(matches!(r, Err(PcError::Forbidden(_))), "{r:?}");
    withdraw_proposal(
        &pool,
        project,
        &lead,
        p3.proposal_id,
        Some("重复提案".into()),
    )
    .await
    .unwrap();
    assert_eq!(proposal_status(&pool, p3.proposal_id).await, "withdrawn");
    let pending: String =
        sqlx::query_scalar("SELECT status FROM pc.approval WHERE proposal_id = $1")
            .bind(p3.proposal_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(pending, "pending");
    assert!(matches!(
        approve_proposal(&pool, project, &lead, p3.proposal_id, decision(0, None)).await,
        Err(PcError::Conflict(_))
    ));
    assert!(matches!(
        withdraw_proposal(&pool, project, &lead, p3.proposal_id, None).await,
        Err(PcError::Conflict(_))
    ));
    assert_eq!(audit_count(&pool, project, "pc_proposal.reject").await, 1);
    assert_eq!(audit_count(&pool, project, "pc_proposal.withdraw").await, 2);
}

// ══════════════════════════════════════════════════════════════════════════
// 6. 分析起草（AI 提议、人工确认）
// ══════════════════════════════════════════════════════════════════════════

/// 模拟一次分析运行：来源 + 对每个修订登记证据（与分析链同构：一次分析一条来源）。
async fn analysis_run(pool: &PgPool, project: Uuid, revisions: &[Uuid]) -> Uuid {
    let dispatch = AuditActor::dispatch();
    let source_id = register_source(
        pool,
        project,
        &dispatch,
        SourceInput {
            source_kind: "manual_input".into(),
            target_domain: "pc".into(),
            target_type: "analysis".into(),
            target_id: Uuid::new_v4().to_string(),
            target_revision_id: None,
            revision_missing_reason: Some("kp2 simulated analysis".into()),
            section_anchor: None,
            start_line: None,
            locator: None,
            description: None,
            source_content_hash: None,
        },
    )
    .await
    .unwrap();
    for revision_id in revisions {
        register_evidence(
            pool,
            project,
            &dispatch,
            EvidenceInput {
                source_id,
                subject: EvidenceSubject::ObjectRevision(*revision_id),
                evidence_type: "system_observation".into(),
                target_domain: None,
                target_type: None,
                target_id: None,
                target_revision_id: None,
                file_path: None,
                symbol: None,
                code_range: None,
                commit_sha: None,
                section_anchor: None,
                start_line: None,
                end_line: None,
                excerpt: None,
                description: Some("kp2 analysis evidence".into()),
                evidence_hash: "kp2".into(),
                confidence: None,
            },
        )
        .await
        .unwrap();
    }
    source_id
}

async fn analyze(pool: &PgPool, project: Uuid, source_id: Uuid) -> AnalysisProposalOutcome {
    propose_from_analysis(
        pool,
        project,
        AnalysisProposalInput {
            source_id,
            task_id: Uuid::new_v4(),
            title: "仓库快照认知更新".into(),
            rationale: "AI 起草，待人工确认".into(),
        },
    )
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn analysis_drafts_uncovered_revisions_once(pool: PgPool) {
    let (_, project) = seed_project_with_team(&pool, "kp2-ana").await;
    let dispatch = AuditActor::dispatch();
    let lead = AuditActor::principal(LEAD);
    let x = upsert(
        &pool,
        project,
        &dispatch,
        draft("feature", "an.x", "X", "# X1"),
    )
    .await;
    let y = upsert(
        &pool,
        project,
        &dispatch,
        draft("business_capability", "an.y", "Y", "# Y1"),
    )
    .await;

    // 首轮：两修订未覆盖 → AI 提案（submitted，pending 人工审批）+ change_proposed 事件
    let source1 = analysis_run(&pool, project, &[x.revision_id, y.revision_id]).await;
    let task = Uuid::new_v4();
    let outcome = propose_from_analysis(
        &pool,
        project,
        AnalysisProposalInput {
            source_id: source1,
            task_id: task,
            title: "仓库快照认知更新".into(),
            rationale: "AI 起草，待人工确认".into(),
        },
    )
    .await
    .unwrap();
    let AnalysisProposalOutcome::Proposed(receipt) = outcome else {
        panic!("uncovered revisions must be proposed");
    };
    assert_eq!(receipt.status, "submitted");
    assert_eq!(receipt.member_count, 2);
    let (origin, created_by, source): (String, Uuid, Option<Uuid>) =
        sqlx::query_as("SELECT origin_kind, created_by, source_id FROM pc.proposal WHERE id = $1")
            .bind(receipt.proposal_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(origin, "ai_agent");
    assert_eq!(created_by, DISPATCH);
    assert_eq!(source, Some(source1));
    let (event_type, payload): (String, serde_json::Value) = sqlx::query_as(
        "SELECT event_type, payload FROM integration.outbox_event \
          WHERE project_id = $1 AND idempotency_key = $2",
    )
    .bind(project)
    .bind(analysis_outcome_key(task))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event_type, "pc.change_proposed");
    assert_eq!(
        payload["proposal_ref"]["target_id"],
        receipt.proposal_id.to_string()
    );
    assert_eq!(payload["task_ref"]["target_id"], task.to_string());
    assert_eq!(payload["members"].as_array().unwrap().len(), 2);
    assert_eq!(payload["content_hash"], receipt.proposal_content_hash);

    // 重投递（同来源）与重分析（新来源钉同修订）→ NO_CHANGE（同一修订集不重复提案）
    assert!(matches!(
        analyze(&pool, project, source1).await,
        AnalysisProposalOutcome::NoChange
    ));
    let same = upsert(
        &pool,
        project,
        &dispatch,
        draft("feature", "an.x", "X", "# X1"),
    )
    .await;
    assert!(!same.revision_created);
    let source2 = analysis_run(&pool, project, &[x.revision_id, y.revision_id]).await;
    assert!(matches!(
        analyze(&pool, project, source2).await,
        AnalysisProposalOutcome::NoChange
    ));

    // AI 无确认权；人工驳回后同内容仍视为已裁决（不重复打扰）
    let r = approve_proposal(
        &pool,
        project,
        &dispatch,
        receipt.proposal_id,
        decision(0, None),
    )
    .await;
    assert!(matches!(r, Err(PcError::Forbidden(_))));
    reject_proposal(
        &pool,
        project,
        &lead,
        receipt.proposal_id,
        decision(0, Some("待补证据")),
    )
    .await
    .unwrap();
    assert!(matches!(
        analyze(&pool, project, source2).await,
        AnalysisProposalOutcome::NoChange
    ));

    // 内容变化 → 新修订入链（仅 X@2；Y@1 已被人工裁决）
    let x2 = upsert(
        &pool,
        project,
        &dispatch,
        draft("feature", "an.x", "X", "# X2"),
    )
    .await;
    assert!(x2.revision_created);
    let source3 = analysis_run(&pool, project, &[x2.revision_id, y.revision_id]).await;
    let AnalysisProposalOutcome::Proposed(second) = analyze(&pool, project, source3).await else {
        panic!("new revision must be proposed");
    };
    assert_eq!(second.member_count, 1);
    let pinned: Uuid = sqlx::query_scalar(
        "SELECT object_revision_id FROM pc.proposal_member WHERE proposal_id = $1",
    )
    .bind(second.proposal_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pinned, x2.revision_id);

    // failed（如发布 CAS 失配）释放覆盖：修订回到候选池待重新提案
    sqlx::query("UPDATE pc.proposal SET status = 'failed' WHERE id = $1")
        .bind(second.proposal_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        analyze(&pool, project, source3).await,
        AnalysisProposalOutcome::Proposed(_)
    ));
}

// ══════════════════════════════════════════════════════════════════════════
// 7. Profile / Vision 单例骨架（PC-D-06 初始化规则）
// ══════════════════════════════════════════════════════════════════════════

/// 单例骨架行投影（type, key, display_name, lifecycle, current_revision, created_by）。
type SingletonRow = (String, Option<String>, String, String, Option<Uuid>, Uuid);

#[sqlx::test(migrations = "../../migrations")]
async fn project_bootstrap_seeds_product_singletons(pool: PgPool) {
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('system', $1, $1) RETURNING id",
    )
    .bind(format!("kp2-singleton-{}", Uuid::new_v4().simple()))
    .fetch_one(&pool)
    .await
    .unwrap();
    let outcome = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "KP2-SINGLE".into(),
            project_name: "北极星".into(),
            tenant_id: tenant,
        },
    )
    .await
    .unwrap();
    let project = outcome.project_id;

    // 项目创建即建两单例：identity 态、无修订、无版本成员（「未确认」与「确认为空」可区分）
    let singletons: Vec<SingletonRow> = sqlx::query_as(
        "SELECT object_type, object_key, display_name, lifecycle_status, \
                current_revision_id, created_by \
           FROM pc.cognition_object WHERE project_id = $1 ORDER BY object_type",
    )
    .bind(project)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(singletons.len(), 2, "{singletons:?}");
    assert_eq!(singletons[0].0, "product_profile");
    assert_eq!(singletons[0].1.as_deref(), Some("product-profile"));
    assert_eq!(singletons[0].2, "北极星");
    assert_eq!(singletons[1].0, "product_vision");
    assert_eq!(singletons[1].1.as_deref(), Some("product-vision"));
    assert!(singletons
        .iter()
        .all(|(_, _, _, lifecycle, revision, _)| lifecycle == "identity" && revision.is_none()));
    assert!(
        singletons.iter().all(|s| s.5 == outcome.human_id),
        "skeletons are attributed to the project owner"
    );

    // 可重入：存量补骨架零新增
    let lead = AuditActor::principal(outcome.human_id);
    assert_eq!(
        ensure_product_singletons(&pool, project, &lead)
            .await
            .unwrap(),
        0
    );

    // 单例唯一索引兜底：同项目第二个活跃 product_profile → 23505
    let err = sqlx::query(
        "INSERT INTO pc.cognition_object \
         (tenant_id, project_id, object_type, object_key, display_name, lifecycle_status, \
          created_by, updated_by) \
         VALUES ($1, $2, 'product_profile', 'profile-dup', 'dup', 'identity', $3, $3)",
    )
    .bind(tenant)
    .bind(project)
    .bind(outcome.human_id)
    .execute(&pool)
    .await
    .unwrap_err();
    assert_eq!(db_code(&err), "23505", "{err}");

    // 骨架可被草稿填充（identity → draft 首修订）并进入确认链；Vision 固定 L2
    let vision = upsert(
        &pool,
        project,
        &lead,
        UpsertObjectDraft {
            object_type: "product_vision".into(),
            object_key: Some("product-vision".into()),
            display_name: "北极星 产品愿景".into(),
            content_markdown: "## Vision Statement\n让交付可确认".into(),
            change_summary: Some("首版愿景".into()),
            source_context: None,
            analysis_mode: None,
        },
    )
    .await;
    assert_eq!(vision.revision_no, 1);
    let (skeleton_id, lifecycle): (Uuid, String) = sqlx::query_as(
        "SELECT id, lifecycle_status FROM pc.cognition_object \
          WHERE project_id = $1 AND object_type = 'product_vision'",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        vision.object_id, skeleton_id,
        "draft fills the skeleton identity"
    );
    assert_eq!(lifecycle, "draft");
    let proposal = create_proposal(
        &pool,
        project,
        &lead,
        proposal_input("愿景确认", vec![obj_member("create_object", &vision)]),
    )
    .await
    .unwrap();
    assert_eq!(
        proposal.governance_level, "L2",
        "product_vision is fixed L2"
    );
}

// ══════════════════════════════════════════════════════════════════════════
// 8. 决定暂记与发布解耦（审阅流 v2：close 前可改；发布独立幂等，CAS 失配 → failed）
// ══════════════════════════════════════════════════════════════════════════

#[sqlx::test(migrations = "../../migrations")]
async fn decide_is_provisional_and_publish_is_idempotent(pool: PgPool) {
    let (tenant, project) = seed_project_with_team(&pool, "kp2-atomic").await;
    let lead = AuditActor::principal(LEAD);
    let a = upsert(&pool, project, &lead, draft("feature", "ta", "A", "# A1")).await;
    let b = upsert(&pool, project, &lead, draft("feature", "tb", "B", "# B1")).await;
    let p = create_proposal(
        &pool,
        project,
        &lead,
        proposal_input("atomic", vec![obj_member("create_object", &a)]),
    )
    .await
    .unwrap();
    submit_proposal(&pool, project, &lead, p.proposal_id, Some(0))
        .await
        .unwrap();
    let stale = create_proposal(
        &pool,
        project,
        &lead,
        proposal_input("stale", vec![obj_member("create_object", &b)]),
    )
    .await
    .unwrap();
    submit_proposal(&pool, project, &lead, stale.proposal_id, Some(0))
        .await
        .unwrap();

    // 决定暂记：approve → approved（无版本）；同态重放幂等不重复审计
    let outcome = decide_proposal(&pool, project, &lead, p.proposal_id, approve(0))
        .await
        .unwrap();
    assert_eq!(outcome.status, "approved");
    assert!(outcome.published_version.is_none());
    assert_eq!(audit_count(&pool, project, "pc_proposal.approve").await, 1);
    let again = decide_proposal(&pool, project, &lead, p.proposal_id, approve(0))
        .await
        .unwrap();
    assert_eq!(again.status, "approved");
    assert_eq!(audit_count(&pool, project, "pc_proposal.approve").await, 1);

    // 发布独立：注入发布尾段失败（同幂等键账本行已终态）→ 发布失败，决定事实保留
    sqlx::query(
        "INSERT INTO pc.write_operation \
         (tenant_id, project_id, operation_id, command, status, expected_content_hash, created_by) \
         VALUES ($1, $2, $3, 'pc_publish_version', 'abandoned', 'injected', $4)",
    )
    .bind(tenant)
    .bind(project)
    .bind(format!("pc-decision:{}", p.proposal_id))
    .bind(LEAD)
    .execute(&pool)
    .await
    .unwrap();
    let r = publish_version(
        &pool,
        project,
        &lead,
        PublishVersionInput {
            proposal_id: p.proposal_id,
            operation_id: format!("pc-decision:{}", p.proposal_id),
        },
    )
    .await;
    assert!(matches!(r, Err(PcError::Db(_))), "{r:?}");
    assert_eq!(proposal_status(&pool, p.proposal_id).await, "approved");
    let versions: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pc.cognition_version WHERE project_id = $1")
            .bind(project)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(versions, 0);

    // 故障消除后重试：发布幂等键复用 → applied v1
    sqlx::query("DELETE FROM pc.write_operation WHERE project_id = $1")
        .bind(project)
        .execute(&pool)
        .await
        .unwrap();
    let receipt = publish_version(
        &pool,
        project,
        &lead,
        PublishVersionInput {
            proposal_id: p.proposal_id,
            operation_id: format!("pc-decision:{}", p.proposal_id),
        },
    )
    .await
    .unwrap();
    assert_eq!(receipt.version_no, 1);
    assert_eq!(proposal_status(&pool, p.proposal_id).await, "applied");

    // 基线已移动的提案：决定仍可暂记（CAS 只对提案基线），发布入口 CAS 失配 → failed
    let outcome2 = decide_proposal(&pool, project, &lead, stale.proposal_id, approve(0))
        .await
        .unwrap();
    assert_eq!(outcome2.status, "approved");
    let r = publish_version(
        &pool,
        project,
        &lead,
        PublishVersionInput {
            proposal_id: stale.proposal_id,
            operation_id: format!("pc-decision:{}", stale.proposal_id),
        },
    )
    .await;
    assert!(matches!(r, Err(PcError::Conflict(_))), "{r:?}");
    assert_eq!(proposal_status(&pool, stale.proposal_id).await, "failed");
    assert_eq!(
        audit_count(&pool, project, "pc_version.publish_failed").await,
        1
    );
}
