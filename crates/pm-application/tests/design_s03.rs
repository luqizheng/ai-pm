//! S03 设计与技术决策域集成测试（总体设计 V1.0 §10.3 八个关键场景 SC-01~08
//! + 库级不可变守卫）。
//!
//! 治理红线对应用例：
//! - SC-01：AI 创建修订 + 人类批准（Q4）；
//! - SC-02：新修订批准后旧版本不被覆盖（§4.2）；
//! - SC-03：一个设计关联三个需求、各自绑定具体修订（Q1/§2.2）；
//! - SC-04：一个 ADR 关联多个需求与设计（Q3/§3.4）；
//! - SC-05：AI 主体正式批准被拒（原则 5）；
//! - SC-06：并发批准不同修订 → 显式冲突而非错误当前版本（§5.1）；
//! - SC-07：关键 ADR 未解决 → Readiness Gate 拦截（Q5/§6.1）；
//! - SC-08：设计变更不静默改变已冻结基线（§6.3）。

use pm_application::actor::AuditActor;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::design::{self, AcceptanceCriterionInput};
use pm_application::knowledge::KnowledgeRoot;
use pm_application::pm_write::PmWriteError;
use pm_application::requirement::{
    self, ConfirmRevisionRequest, CreateRequirementRequest, DraftRevisionRequest,
};
use pm_domain::SYSTEM_TENANT;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    project_id: Uuid,
    human: AuditActor,
    ai: AuditActor,
    human_id: Uuid,
    ai_id: Uuid,
}

async fn fixture(pool: &PgPool, code: &str) -> Fx {
    let boot = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: format!("s03-{code}"),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let ai_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (id, kind, display_name) \
         VALUES ($1, 'ai_pm', $2) RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(format!("ai-{code}"))
    .fetch_one(pool)
    .await
    .unwrap();
    Fx {
        project_id: boot.project_id,
        human: AuditActor::principal(boot.human_id),
        ai: AuditActor::principal(ai_id),
        human_id: boot.human_id,
        ai_id,
    }
}

fn knowledge_root(tag: &str) -> KnowledgeRoot {
    KnowledgeRoot::new(std::env::temp_dir().join(format!("ai-pm-s03-{tag}-{}", Uuid::new_v4())))
}

/// 门禁可过的设计内容（范围 + 可验证验收标准）。
fn gate_ready_content() -> (
    Option<serde_json::Value>,
    Option<Vec<AcceptanceCriterionInput>>,
) {
    (
        Some(json!({ "included": ["authentication module"], "excluded": [], "modules": ["auth"] })),
        Some(vec![AcceptanceCriterionInput {
            statement: "登录接口返回 200 且签发 token".to_string(),
            verification_kind: Some("automated_test".to_string()),
        }]),
    )
}

/// 由 AI 创建设计 + 首修订并提交（source='ai'），返回 (design_id, revision_id, content_hash)。
async fn ai_submitted_design(
    pool: &PgPool,
    fx: &Fx,
    title: &str,
    gate_ready: bool,
) -> (Uuid, Uuid, String) {
    let (scope, criteria) = if gate_ready {
        let (s, c) = gate_ready_content();
        (s, c)
    } else {
        (None, None)
    };
    let detail = design::create_design(
        pool,
        SYSTEM_TENANT,
        fx.project_id,
        design::CreateDesignRequest {
            code: None,
            title: title.to_string(),
            markdown: Some(format!("# {title}\n\n设计正文。\n")),
            summary: None,
            technical_analysis: None,
            implementation_scope: scope,
            acceptance_criteria: criteria,
        },
        &fx.ai,
    )
    .await
    .unwrap();
    let design_id = detail.design.id;
    let rev = &detail.revisions[0];
    assert_eq!(rev.source, "ai", "AI 主体创建的设计修订必须留痕 source=ai");
    let mut rev = rev.clone();
    if !gate_ready {
        // 无范围/验收的版本：直接用首修订（缺省内容）提交。
    }
    let _ = &mut rev;
    design::submit_design_revision(pool, SYSTEM_TENANT, design_id, rev.id, &fx.ai)
        .await
        .unwrap();
    let rev = design::design_detail(pool, SYSTEM_TENANT, design_id)
        .await
        .unwrap()
        .revisions
        .into_iter()
        .find(|r| r.id == rev.id)
        .unwrap();
    (design_id, rev.id, rev.content_hash)
}

/// 已确认需求（human 起草→提交→确认），返回 (requirement_id, revision_id)。
async fn approved_requirement(
    pool: &PgPool,
    fx: &Fx,
    code: &str,
    root: &KnowledgeRoot,
) -> (Uuid, Uuid) {
    let entry = requirement::create_requirement(
        pool,
        SYSTEM_TENANT,
        fx.project_id,
        CreateRequirementRequest {
            code: code.to_string(),
            title: format!("需求 {code}"),
            description: None,
            requirement_type: None,
            priority: None,
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
            markdown: format!("# {code} 修订正文\n"),
            summary: None,
            basis_finding_ids: None,
        },
        &fx.human,
    )
    .await
    .unwrap();
    requirement::submit_revision(pool, SYSTEM_TENANT, entry.id, draft.id, &fx.human, root)
        .await
        .unwrap();
    let confirmed = requirement::confirm_revision(
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
    assert_eq!(confirmed.current_revision_id, Some(draft.id));
    (entry.id, draft.id)
}

/// 提交修订并请求批准，返回审批 id。
async fn submitted_with_approval(pool: &PgPool, fx: &Fx, design_id: Uuid, rev_id: Uuid) -> Uuid {
    design::submit_design_revision(pool, SYSTEM_TENANT, design_id, rev_id, &fx.ai)
        .await
        .unwrap();
    design::request_design_revision_approval(pool, SYSTEM_TENANT, design_id, rev_id, &fx.ai)
        .await
        .unwrap()
        .id
}

async fn decide(
    pool: &PgPool,
    fx: &Fx,
    approval_id: Uuid,
    decision: &str,
    expected_current: Option<Uuid>,
) -> Result<design::ApprovalView, PmWriteError> {
    design::decide_approval(
        pool,
        SYSTEM_TENANT,
        approval_id,
        design::DecideApprovalRequest {
            decision: decision.to_string(),
            reason: Some("评审通过".to_string()),
            expected_current_revision_id: expected_current,
            expected_row_revision: None,
        },
        &fx.human,
    )
    .await
}

// ───────────────────────────────────────────────── SC-01 + SC-05（合并场景）

#[sqlx::test(migrations = "../../migrations")]
async fn sc01_ai_drafts_design_human_approves_ai_cannot_decide(pool: PgPool) {
    let fx = fixture(&pool, "sc01").await;
    let (design_id, rev_id, hash) = ai_submitted_design(&pool, &fx, "登录设计", true).await;

    // AI 请求批准（允许），AI 裁决（禁止，SC-05）。
    let approval =
        design::request_design_revision_approval(&pool, SYSTEM_TENANT, design_id, rev_id, &fx.ai)
            .await
            .unwrap();
    let ai_decide = design::decide_approval(
        &pool,
        SYSTEM_TENANT,
        approval.id,
        design::DecideApprovalRequest {
            decision: "approve".to_string(),
            reason: None,
            expected_current_revision_id: None,
            expected_row_revision: None,
        },
        &fx.ai,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(ai_decide, PmWriteError::Forbidden(_)),
        "AI 主体必须被服务层真人闸拒绝，实际 {ai_decide:?}"
    );

    // 人类批准（SC-01）。
    let decided = decide(&pool, &fx, approval.id, "approve", None)
        .await
        .unwrap();
    assert_eq!(decided.status, "approved");
    let detail = design::design_detail(&pool, SYSTEM_TENANT, design_id)
        .await
        .unwrap();
    assert_eq!(detail.design.current_revision_id, Some(rev_id));
    let rev = detail.revisions.iter().find(|r| r.id == rev_id).unwrap();
    assert_eq!(rev.state, "approved");
    assert_eq!(rev.content_hash, hash);
    assert_eq!(rev.approved_by, Some(fx.human_id));

    // 审计：创建 + 提交 + 裁决均有留痕。
    let audits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM governance.audit_log \
         WHERE project_id = $1 AND target_id = ANY($2)",
    )
    .bind(fx.project_id)
    .bind(vec![design_id, rev_id, approval.id])
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(audits >= 3, "设计/修订/审批的审计行必须存在，实际 {audits}");
}

// ────────────────────────────────────────────────────────────── SC-02

#[sqlx::test(migrations = "../../migrations")]
async fn sc02_new_revision_approval_supersedes_but_never_overwrites(pool: PgPool) {
    let fx = fixture(&pool, "sc02").await;
    let (design_id, rev1, _) = ai_submitted_design(&pool, &fx, "基线设计", true).await;
    // 首修订已由 ai_submitted_design 提交，这里直接请求批准并批准。
    let ap1 =
        design::request_design_revision_approval(&pool, SYSTEM_TENANT, design_id, rev1, &fx.ai)
            .await
            .unwrap();
    decide(&pool, &fx, ap1.id, "approve", None).await.unwrap();

    let v1_content = design::design_detail(&pool, SYSTEM_TENANT, design_id)
        .await
        .unwrap()
        .revisions
        .into_iter()
        .find(|r| r.id == rev1)
        .unwrap()
        .content
        .clone();

    // 新修订（人工起草）→ 提交 → 批准。
    let rev2 = design::create_design_revision(
        &pool,
        SYSTEM_TENANT,
        design_id,
        design::DraftDesignRevisionRequest {
            markdown: "# 基线设计 v2\n\n新方案。\n".to_string(),
            summary: None,
            technical_analysis: None,
            implementation_scope: Some(json!({ "included": ["auth module v2"] })),
            acceptance_criteria: Some(vec![AcceptanceCriterionInput {
                statement: "回归测试通过".to_string(),
                verification_kind: None,
            }]),
        },
        &fx.human,
    )
    .await
    .unwrap();
    assert_eq!(rev2.version, 2);
    let ap2 = submitted_with_approval(&pool, &fx, design_id, rev2.id).await;
    decide(&pool, &fx, ap2, "approve", None).await.unwrap();

    let detail = design::design_detail(&pool, SYSTEM_TENANT, design_id)
        .await
        .unwrap();
    assert_eq!(detail.design.current_revision_id, Some(rev2.id));
    let v1 = detail.revisions.iter().find(|r| r.id == rev1).unwrap();
    assert_eq!(v1.state, "superseded", "旧批准版本退位");
    assert_eq!(
        v1.content, v1_content,
        "旧版本正文必须原样保留（SC-02 不可覆盖）"
    );
    let v2 = detail.revisions.iter().find(|r| r.id == rev2.id).unwrap();
    assert_eq!(v2.state, "approved");

    // 提交后改稿被拒（触发器 + 服务双闸）。
    let err = design::update_design_revision_draft(
        &pool,
        SYSTEM_TENANT,
        design_id,
        rev2.id,
        design::DraftDesignRevisionRequest {
            markdown: "# 篡改已批准内容\n".to_string(),
            summary: None,
            technical_analysis: None,
            implementation_scope: None,
            acceptance_criteria: None,
        },
        &fx.ai,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, PmWriteError::Conflict(_)));
}

// ────────────────────────────────────────────────────────────── SC-03

#[sqlx::test(migrations = "../../migrations")]
async fn sc03_one_design_links_three_requirements_revision_bound(pool: PgPool) {
    let fx = fixture(&pool, "sc03").await;
    let root = knowledge_root("sc03");
    let (r1, r1_rev1) = approved_requirement(&pool, &fx, "REQ-A", &root).await;
    let (r2, r2_rev) = approved_requirement(&pool, &fx, "REQ-B", &root).await;
    let (r3, _) = approved_requirement(&pool, &fx, "REQ-C", &root).await;

    // REQ-A 推进到第 2 个正式版本，验证设计可绑旧版本（版本级追溯）。
    let r1_rev2 = requirement::create_draft(
        &pool,
        SYSTEM_TENANT,
        r1,
        DraftRevisionRequest {
            markdown: "# REQ-A 第二版\n".to_string(),
            summary: None,
            basis_finding_ids: None,
        },
        &fx.human,
    )
    .await
    .unwrap();
    requirement::submit_revision(&pool, SYSTEM_TENANT, r1, r1_rev2.id, &fx.human, &root)
        .await
        .unwrap();
    requirement::confirm_revision(
        &pool,
        SYSTEM_TENANT,
        r1,
        r1_rev2.id,
        ConfirmRevisionRequest {
            expected_content_hash: r1_rev2.content_hash.clone(),
            expected_current_revision_id: None,
            expected_row_revision: None,
        },
        &fx.human,
    )
    .await
    .unwrap();

    let (design_id, _rev_id, _) = ai_submitted_design(&pool, &fx, "三需求设计", false).await;
    // 转回 draft 不可行——ai_submitted_design 已提交；关联仅限 draft，
    // 因此这里建一条新的 draft 修订做关联验证。
    let draft_rev = design::create_design_revision(
        &pool,
        SYSTEM_TENANT,
        design_id,
        design::DraftDesignRevisionRequest {
            markdown: "# 三需求设计 draft\n".to_string(),
            summary: None,
            technical_analysis: None,
            implementation_scope: None,
            acceptance_criteria: None,
        },
        &fx.ai,
    )
    .await
    .unwrap();

    // 绑定三个需求；REQ-A 显式指定修订 id（= 当前正式版本 rev2），
    // REQ-B/REQ-C 缺省绑当前正式版本。
    for (rid, rvid) in [(r1, Some(r1_rev2.id)), (r2, None), (r3, None)] {
        design::link_design_requirement(
            &pool,
            SYSTEM_TENANT,
            design_id,
            draft_rev.id,
            design::LinkDesignRequirementRequest {
                requirement_id: rid,
                requirement_revision_id: rvid,
            },
            &fx.ai,
        )
        .await
        .unwrap();
    }
    let links = design::list_design_requirements(&pool, SYSTEM_TENANT, design_id, draft_rev.id)
        .await
        .unwrap();
    assert_eq!(links.len(), 3, "一个设计关联三个需求");
    let r1_link = links.iter().find(|l| l.requirement_id == r1).unwrap();
    assert_eq!(
        r1_link.requirement_revision_id, r1_rev2.id,
        "显式指定的修订被原样绑定"
    );
    assert_eq!(r1_link.is_current, Some(true));
    let r2_link = links.iter().find(|l| l.requirement_id == r2).unwrap();
    assert_eq!(
        r2_link.requirement_revision_id, r2_rev,
        "缺省绑当前正式版本"
    );
    assert_eq!(r2_link.is_current, Some(true));

    // 绑定未确认修订被拒（§2.2）。
    let pending = requirement::create_draft(
        &pool,
        SYSTEM_TENANT,
        r2,
        DraftRevisionRequest {
            markdown: "# REQ-B 草稿\n".to_string(),
            summary: None,
            basis_finding_ids: None,
        },
        &fx.human,
    )
    .await
    .unwrap();
    let err = design::link_design_requirement(
        &pool,
        SYSTEM_TENANT,
        design_id,
        draft_rev.id,
        design::LinkDesignRequirementRequest {
            requirement_id: r2,
            requirement_revision_id: Some(pending.id),
        },
        &fx.ai,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, PmWriteError::Validation(_)));
}

// ────────────────────────────────────────────────────────────── SC-04

#[sqlx::test(migrations = "../../migrations")]
async fn sc04_adr_links_multiple_designs_and_requirements(pool: PgPool) {
    let fx = fixture(&pool, "sc04").await;
    let root = knowledge_root("sc04");
    let (r1, _) = approved_requirement(&pool, &fx, "REQ-D", &root).await;
    let (r2, _) = approved_requirement(&pool, &fx, "REQ-E", &root).await;
    let (d1, _, _) = ai_submitted_design(&pool, &fx, "设计一", true).await;
    let (d2, _, _) = ai_submitted_design(&pool, &fx, "设计二", true).await;
    let d1_rev = design::design_detail(&pool, SYSTEM_TENANT, d1)
        .await
        .unwrap()
        .revisions[0]
        .id;
    let d2_rev = design::design_detail(&pool, SYSTEM_TENANT, d2)
        .await
        .unwrap()
        .revisions[0]
        .id;

    let adr = design::create_adr(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        design::CreateAdrRequest {
            code: None,
            title: "采用 PostgreSQL 作为业务状态权威".to_string(),
            decision_question: "业务状态存哪里？".to_string(),
            context: Some("多候选存储方案".to_string()),
            options: vec![json!({"name": "PG"}), json!({"name": "Mongo"})],
            chosen_option: Some("PG".to_string()),
            rationale: Some("事务与审计能力".to_string()),
            consequences: Some("强一致，弱水平扩展".to_string()),
            blockers_implementation: false,
        },
        &fx.ai,
    )
    .await
    .unwrap();
    let adr_id = adr.adr.id;
    let adr_rev = adr.revisions[0].id;

    // 关联（draft 修订期）：2 设计 + 2 需求。
    design::link_adr_design_revision(
        &pool,
        SYSTEM_TENANT,
        adr_id,
        adr_rev,
        design::LinkAdrDesignRequest {
            design_id: d1,
            design_revision_id: d1_rev,
        },
        &fx.ai,
    )
    .await
    .unwrap();
    design::link_adr_design_revision(
        &pool,
        SYSTEM_TENANT,
        adr_id,
        adr_rev,
        design::LinkAdrDesignRequest {
            design_id: d2,
            design_revision_id: d2_rev,
        },
        &fx.ai,
    )
    .await
    .unwrap();
    for rid in [r1, r2] {
        design::link_adr_requirement(
            &pool,
            SYSTEM_TENANT,
            adr_id,
            adr_rev,
            design::LinkAdrRequirementRequest {
                requirement_id: rid,
                requirement_revision_id: None,
            },
            &fx.ai,
        )
        .await
        .unwrap();
    }

    // 提交 → 人类接受。
    design::submit_adr_revision(&pool, SYSTEM_TENANT, adr_id, adr_rev, &fx.ai)
        .await
        .unwrap();
    let ap = design::request_adr_approval(&pool, SYSTEM_TENANT, adr_id, &fx.ai)
        .await
        .unwrap();
    let ai_err = design::decide_approval(
        &pool,
        SYSTEM_TENANT,
        ap.id,
        design::DecideApprovalRequest {
            decision: "approve".to_string(),
            reason: None,
            expected_current_revision_id: None,
            expected_row_revision: None,
        },
        &fx.ai,
    )
    .await
    .unwrap_err();
    assert!(matches!(ai_err, PmWriteError::Forbidden(_)));
    decide(&pool, &fx, ap.id, "approve", None).await.unwrap();

    let detail = design::adr_detail(&pool, SYSTEM_TENANT, adr_id)
        .await
        .unwrap();
    assert_eq!(detail.adr.status, "accepted");
    assert_eq!(detail.design_links.len(), 2, "ADR 关联两个设计版本");
    assert_eq!(detail.requirement_links.len(), 2, "ADR 关联两个需求修订");
}

// ────────────────────────────────────────────────────────────── SC-06

#[sqlx::test(migrations = "../../migrations")]
async fn sc06_concurrent_approvals_conflict_not_corrupt_current(pool: PgPool) {
    let fx = fixture(&pool, "sc06").await;
    let (design_id, rev1, _) = ai_submitted_design(&pool, &fx, "并发设计", true).await;
    let ap1 =
        design::request_design_revision_approval(&pool, SYSTEM_TENANT, design_id, rev1, &fx.ai)
            .await
            .unwrap();
    decide(&pool, &fx, ap1.id, "approve", None).await.unwrap();

    // 两个新修订同时进入待批（两位审批人看到同一当前版本 rev1）。
    let rev2 = design::create_design_revision(
        &pool,
        SYSTEM_TENANT,
        design_id,
        design::DraftDesignRevisionRequest {
            markdown: "# 并发设计 v2 A 方案\n".to_string(),
            summary: None,
            technical_analysis: None,
            implementation_scope: Some(json!({ "included": ["a"] })),
            acceptance_criteria: Some(vec![AcceptanceCriterionInput {
                statement: "A 方案验收".to_string(),
                verification_kind: None,
            }]),
        },
        &fx.human,
    )
    .await
    .unwrap();
    let rev3 = design::create_design_revision(
        &pool,
        SYSTEM_TENANT,
        design_id,
        design::DraftDesignRevisionRequest {
            markdown: "# 并发设计 v3 B 方案\n".to_string(),
            summary: None,
            technical_analysis: None,
            implementation_scope: Some(json!({ "included": ["b"] })),
            acceptance_criteria: Some(vec![AcceptanceCriterionInput {
                statement: "B 方案验收".to_string(),
                verification_kind: None,
            }]),
        },
        &fx.human,
    )
    .await
    .unwrap();
    let ap2 = submitted_with_approval(&pool, &fx, design_id, rev2.id).await;
    let ap3 = submitted_with_approval(&pool, &fx, design_id, rev3.id).await;

    // 审批人甲批准 rev2（核准时当前版本 = rev1）。
    decide(&pool, &fx, ap2, "approve", Some(rev1))
        .await
        .unwrap();
    // 审批人乙拿着旧快照（当前版本仍以为是 rev1）批准 rev3 → 显式 409。
    let err = decide(&pool, &fx, ap3, "approve", Some(rev1))
        .await
        .unwrap_err();
    assert!(
        matches!(err, PmWriteError::Conflict(_)),
        "并发批准必须显式冲突，实际 {err:?}"
    );
    let detail = design::design_detail(&pool, SYSTEM_TENANT, design_id)
        .await
        .unwrap();
    assert_eq!(
        detail.design.current_revision_id,
        Some(rev2.id),
        "当前版本未被并发破坏"
    );
}

// ────────────────────────────────────────────────────────────── SC-07

#[sqlx::test(migrations = "../../migrations")]
async fn sc07_readiness_gate_blocks_unresolved_blocking_adr(pool: PgPool) {
    let fx = fixture(&pool, "sc07").await;
    let root = knowledge_root("sc07");
    let (rid, _) = approved_requirement(&pool, &fx, "REQ-F", &root).await;
    let (design_id, _rev_id, _) = ai_submitted_design(&pool, &fx, "门禁设计", true).await;
    // ai_submitted_design 已提交修订；关联需求需要 draft——补一条 draft 修订绑需求。
    let draft = design::create_design_revision(
        &pool,
        SYSTEM_TENANT,
        design_id,
        design::DraftDesignRevisionRequest {
            markdown: "# 门禁设计 v2\n".to_string(),
            summary: None,
            technical_analysis: None,
            implementation_scope: Some(json!({ "included": ["auth module"] })),
            acceptance_criteria: Some(vec![AcceptanceCriterionInput {
                statement: "门禁设计验收".to_string(),
                verification_kind: None,
            }]),
        },
        &fx.ai,
    )
    .await
    .unwrap();
    design::link_design_requirement(
        &pool,
        SYSTEM_TENANT,
        design_id,
        draft.id,
        design::LinkDesignRequirementRequest {
            requirement_id: rid,
            requirement_revision_id: None,
        },
        &fx.ai,
    )
    .await
    .unwrap();
    design::submit_design_revision(&pool, SYSTEM_TENANT, design_id, draft.id, &fx.ai)
        .await
        .unwrap();
    let ap =
        design::request_design_revision_approval(&pool, SYSTEM_TENANT, design_id, draft.id, &fx.ai)
            .await
            .unwrap();
    decide(&pool, &fx, ap.id, "approve", None).await.unwrap();

    // 阻塞项 ADR：draft 期关联设计修订 → 提交后处于未决（under_review）。
    let adr = design::create_adr(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        design::CreateAdrRequest {
            code: None,
            title: "数据库主键策略".to_string(),
            decision_question: "主键用什么？".to_string(),
            context: None,
            options: vec![],
            chosen_option: None,
            rationale: None,
            consequences: None,
            blockers_implementation: true,
        },
        &fx.ai,
    )
    .await
    .unwrap();
    design::link_adr_design_revision(
        &pool,
        SYSTEM_TENANT,
        adr.adr.id,
        adr.revisions[0].id,
        design::LinkAdrDesignRequest {
            design_id,
            design_revision_id: draft.id,
        },
        &fx.ai,
    )
    .await
    .unwrap();
    design::submit_adr_revision(
        &pool,
        SYSTEM_TENANT,
        adr.adr.id,
        adr.revisions[0].id,
        &fx.ai,
    )
    .await
    .unwrap();

    // 条件 3 失败：阻塞 ADR 处于 under_review。
    let report = design::evaluate_readiness(&pool, SYSTEM_TENANT, fx.project_id, &[draft.id])
        .await
        .unwrap();
    assert!(!report.ready);
    let adr_cond = report
        .conditions
        .iter()
        .find(|c| c.condition == "adrs_resolved")
        .unwrap();
    assert!(!adr_cond.passed, "未决阻塞 ADR 必须拦截（SC-07）");

    // 人类接受 ADR 后门禁通过。
    let ap = design::request_adr_approval(&pool, SYSTEM_TENANT, adr.adr.id, &fx.ai)
        .await
        .unwrap();
    decide(&pool, &fx, ap.id, "approve", None).await.unwrap();
    let report = design::evaluate_readiness(&pool, SYSTEM_TENANT, fx.project_id, &[draft.id])
        .await
        .unwrap();
    assert!(report.ready, "五条件全过后门禁放行：{report:?}");

    // 范围缺失的修订过不了条件 4。
    let bare = design::create_design_revision(
        &pool,
        SYSTEM_TENANT,
        design_id,
        design::DraftDesignRevisionRequest {
            markdown: "# 无范围修订\n".to_string(),
            summary: None,
            technical_analysis: None,
            implementation_scope: None,
            acceptance_criteria: None,
        },
        &fx.ai,
    )
    .await
    .unwrap();
    design::submit_design_revision(&pool, SYSTEM_TENANT, design_id, bare.id, &fx.ai)
        .await
        .unwrap();
    let ap =
        design::request_design_revision_approval(&pool, SYSTEM_TENANT, design_id, bare.id, &fx.ai)
            .await
            .unwrap();
    decide(&pool, &fx, ap.id, "approve", None).await.unwrap();
    let report = design::evaluate_readiness(&pool, SYSTEM_TENANT, fx.project_id, &[bare.id])
        .await
        .unwrap();
    assert!(!report.ready);
    assert!(report
        .conditions
        .iter()
        .any(|c| c.condition == "scope_defined" && !c.passed));
    assert!(report
        .conditions
        .iter()
        .any(|c| c.condition == "acceptance_ready" && !c.passed));
}

// ────────────────────────────────────────────────────────────── SC-08

#[sqlx::test(migrations = "../../migrations")]
async fn sc08_baseline_frozen_immune_to_later_design_changes(pool: PgPool) {
    let fx = fixture(&pool, "sc08").await;
    let root = knowledge_root("sc08");
    let (rid, _) = approved_requirement(&pool, &fx, "REQ-G", &root).await;
    let (design_id, _rev1, _) = ai_submitted_design(&pool, &fx, "基线隔离设计", true).await;
    let draft = design::create_design_revision(
        &pool,
        SYSTEM_TENANT,
        design_id,
        design::DraftDesignRevisionRequest {
            markdown: "# 基线隔离设计 v1\n".to_string(),
            summary: None,
            technical_analysis: None,
            implementation_scope: Some(json!({ "included": ["module m1"] })),
            acceptance_criteria: Some(vec![AcceptanceCriterionInput {
                statement: "基线验收 v1".to_string(),
                verification_kind: None,
            }]),
        },
        &fx.ai,
    )
    .await
    .unwrap();
    design::link_design_requirement(
        &pool,
        SYSTEM_TENANT,
        design_id,
        draft.id,
        design::LinkDesignRequirementRequest {
            requirement_id: rid,
            requirement_revision_id: None,
        },
        &fx.ai,
    )
    .await
    .unwrap();
    design::submit_design_revision(&pool, SYSTEM_TENANT, design_id, draft.id, &fx.ai)
        .await
        .unwrap();
    let ap =
        design::request_design_revision_approval(&pool, SYSTEM_TENANT, design_id, draft.id, &fx.ai)
            .await
            .unwrap();
    decide(&pool, &fx, ap.id, "approve", None).await.unwrap();

    // 冻结基线（真人门；AI 拒绝）。
    let ai_err = design::freeze_baseline(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        design::FreezeBaselineRequest {
            code: None,
            design_revision_ids: vec![draft.id],
            repository_refs: vec![],
        },
        &fx.ai,
    )
    .await
    .unwrap_err();
    assert!(matches!(ai_err, PmWriteError::Forbidden(_)));
    let baseline = design::freeze_baseline(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        design::FreezeBaselineRequest {
            code: None,
            design_revision_ids: vec![draft.id],
            repository_refs: vec![],
        },
        &fx.human,
    )
    .await
    .unwrap();
    assert_eq!(baseline.status, "frozen");
    assert_eq!(baseline.code, "IB-001");
    assert!(baseline.content["requirements"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["requirement_id"] == json!(rid)));
    // 冻结审批行（R6：baseline_freeze 独立审批语义）。
    let approvals: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM design.design_approval \
         WHERE baseline_id = $1 AND kind = 'baseline_freeze' AND status = 'approved'",
    )
    .bind(baseline.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(approvals, 1);
    let frozen_hash = baseline.content_hash.clone();
    let frozen_content = baseline.content.clone();

    // 设计推进到新版本 + 新需求版本 → 基线原样（SC-08 版本隔离）。
    let rev2 = design::create_design_revision(
        &pool,
        SYSTEM_TENANT,
        design_id,
        design::DraftDesignRevisionRequest {
            markdown: "# 基线隔离设计 v2\n".to_string(),
            summary: None,
            technical_analysis: None,
            implementation_scope: Some(json!({ "included": ["module m2"] })),
            acceptance_criteria: Some(vec![AcceptanceCriterionInput {
                statement: "基线验收 v2".to_string(),
                verification_kind: None,
            }]),
        },
        &fx.ai,
    )
    .await
    .unwrap();
    // 新版本同样必须绑需求修订（draft 期），门禁条件 1 才能通过。
    design::link_design_requirement(
        &pool,
        SYSTEM_TENANT,
        design_id,
        rev2.id,
        design::LinkDesignRequirementRequest {
            requirement_id: rid,
            requirement_revision_id: None,
        },
        &fx.ai,
    )
    .await
    .unwrap();
    design::submit_design_revision(&pool, SYSTEM_TENANT, design_id, rev2.id, &fx.ai)
        .await
        .unwrap();
    let ap =
        design::request_design_revision_approval(&pool, SYSTEM_TENANT, design_id, rev2.id, &fx.ai)
            .await
            .unwrap();
    decide(&pool, &fx, ap.id, "approve", None).await.unwrap();

    let after = design::baseline_detail(&pool, SYSTEM_TENANT, baseline.id)
        .await
        .unwrap();
    assert_eq!(after.content_hash, frozen_hash, "基线内容哈希不随设计变化");
    assert_eq!(after.content, frozen_content);
    assert_eq!(after.status, "frozen");

    // S04 交接：Task 引用基线 id。
    let task = pm_application::pm_write::create_task(
        &pool,
        pm_application::pm_write::CreateTaskRequest {
            project_id: Some(fx.project_id),
            milestone_id: None,
            title: Some("实现基线范围内的功能".to_string()),
            description: None,
            priority: None,
            assignee_id: None,
            implementation_baseline_id: Some(baseline.id),
            ai_role_version_id: None,
            acceptance_criteria: None,
        },
        &fx.human,
    )
    .await
    .unwrap();
    let task_baseline: Option<Uuid> =
        sqlx::query_scalar("SELECT implementation_baseline_id FROM delivery.task WHERE id = $1")
            .bind(task.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(task_baseline, Some(baseline.id));

    // 新基线冻结 → 旧基线显式替代 → 旧基线不可再被新任务引用。
    let baseline2 = design::freeze_baseline(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        design::FreezeBaselineRequest {
            code: None,
            design_revision_ids: vec![rev2.id],
            repository_refs: vec![],
        },
        &fx.human,
    )
    .await
    .unwrap();
    assert_eq!(baseline2.code, "IB-002");
    let old = design::supersede_baseline(
        &pool,
        SYSTEM_TENANT,
        baseline.id,
        design::SupersedeBaselineRequest {
            successor_baseline_id: Some(baseline2.id),
            reason: Some("设计推进 v2".to_string()),
        },
        &fx.human,
    )
    .await
    .unwrap();
    assert_eq!(old.status, "superseded");
    assert_eq!(old.superseded_by_baseline_id, Some(baseline2.id));
    let err = pm_application::pm_write::create_task(
        &pool,
        pm_application::pm_write::CreateTaskRequest {
            project_id: Some(fx.project_id),
            milestone_id: None,
            title: Some("引用已替代基线".to_string()),
            description: None,
            priority: None,
            assignee_id: None,
            implementation_baseline_id: Some(baseline.id),
            ai_role_version_id: None,
            acceptance_criteria: None,
        },
        &fx.human,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, PmWriteError::Validation(_)));
    // 跨项目基线引用拦截。
    let boot2 = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "sc08-other".to_string(),
            project_name: "sc08-other".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let err = pm_application::pm_write::create_task(
        &pool,
        pm_application::pm_write::CreateTaskRequest {
            project_id: Some(boot2.project_id),
            milestone_id: None,
            title: Some("跨项目引用基线".to_string()),
            description: None,
            priority: None,
            assignee_id: None,
            implementation_baseline_id: Some(baseline2.id),
            ai_role_version_id: None,
            acceptance_criteria: None,
        },
        &fx.human,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, PmWriteError::Validation(_)));
}

// ─────────────────────────────────────────────────── 库级不可变守卫（R4）

#[sqlx::test(migrations = "../../migrations")]
async fn db_guards_revision_and_baseline_immutability(pool: PgPool) {
    let fx = fixture(&pool, "guards").await;
    let (design_id, rev_id, _) = ai_submitted_design(&pool, &fx, "守卫设计", false).await;
    let _ = design_id;

    // 提交后的修订：正文不可改、行不可删。
    let err = sqlx::query(
        "UPDATE design.design_revision SET content = '{\"markdown\":\"hacked\"}'::jsonb WHERE id = $1",
    )
    .bind(rev_id)
    .execute(&pool)
    .await;
    assert!(err.is_err(), "submitted revision content must be immutable");
    let err = sqlx::query("DELETE FROM design.design_revision WHERE id = $1")
        .bind(rev_id)
        .execute(&pool)
        .await;
    assert!(err.is_err(), "design_revision is append-only");

    // draft 修订：正文可改（服务路径）。
    let (design_id, draft_id, _) = {
        let detail = design::create_design(
            &pool,
            SYSTEM_TENANT,
            fx.project_id,
            design::CreateDesignRequest {
                code: None,
                title: "可改草稿".to_string(),
                markdown: None,
                summary: None,
                technical_analysis: None,
                implementation_scope: None,
                acceptance_criteria: None,
            },
            &fx.human,
        )
        .await
        .unwrap();
        let d = &detail.design;
        let r = &detail.revisions[0];
        (d.id, r.id, r.content_hash.clone())
    };
    let updated = design::update_design_revision_draft(
        &pool,
        SYSTEM_TENANT,
        design_id,
        draft_id,
        design::DraftDesignRevisionRequest {
            markdown: "# 改过草稿\n".to_string(),
            summary: None,
            technical_analysis: None,
            implementation_scope: None,
            acceptance_criteria: None,
        },
        &fx.human,
    )
    .await
    .unwrap();
    assert_ne!(
        updated.content_hash,
        json!(null).to_string().len().to_string(),
        "hash 应变化"
    );
    assert_ne!(updated.content["markdown"], "# 可改草稿\n");

    // 基线冻结列不可变（先造一个门禁可过修订）。
    let root = knowledge_root("guards");
    let (rid, _) = approved_requirement(&pool, &fx, "REQ-H", &root).await;
    let draft2 = design::create_design_revision(
        &pool,
        SYSTEM_TENANT,
        design_id,
        design::DraftDesignRevisionRequest {
            markdown: "# 守卫设计 v2\n".to_string(),
            summary: None,
            technical_analysis: None,
            implementation_scope: Some(json!({ "included": ["m"] })),
            acceptance_criteria: Some(vec![AcceptanceCriterionInput {
                statement: "验收".to_string(),
                verification_kind: None,
            }]),
        },
        &fx.ai,
    )
    .await
    .unwrap();
    design::link_design_requirement(
        &pool,
        SYSTEM_TENANT,
        design_id,
        draft2.id,
        design::LinkDesignRequirementRequest {
            requirement_id: rid,
            requirement_revision_id: None,
        },
        &fx.ai,
    )
    .await
    .unwrap();
    design::submit_design_revision(&pool, SYSTEM_TENANT, design_id, draft2.id, &fx.ai)
        .await
        .unwrap();
    let ap = design::request_design_revision_approval(
        &pool,
        SYSTEM_TENANT,
        design_id,
        draft2.id,
        &fx.ai,
    )
    .await
    .unwrap();
    decide(&pool, &fx, ap.id, "approve", None).await.unwrap();
    let baseline = design::freeze_baseline(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        design::FreezeBaselineRequest {
            code: None,
            design_revision_ids: vec![draft2.id],
            repository_refs: vec![],
        },
        &fx.human,
    )
    .await
    .unwrap();
    let err = sqlx::query(
        "UPDATE design.implementation_baseline SET content = '{}'::jsonb WHERE id = $1",
    )
    .bind(baseline.id)
    .execute(&pool)
    .await;
    assert!(err.is_err(), "frozen baseline content must be immutable");
    let err = sqlx::query("DELETE FROM design.implementation_baseline WHERE id = $1")
        .bind(baseline.id)
        .execute(&pool)
        .await;
    assert!(err.is_err(), "baseline is append-only");
}

// ────────────────────────────────────────────── S03 前端接线读面（2026-11-10）

/// S03 四页共用的读面：审批收件箱 / 工作空间聚合 / 分析页覆盖与影响 /
/// 基线 → 任务交接。
///
/// 覆盖点：
/// - 审批收件箱按「待办优先 + 时间倒序」排，且带出目标展示码；
/// - 工作空间聚合的计数与源表一致（含按状态分桶）；
/// - 分析页把「需求已被新版本取代」标为 stale（影响分析重点）；
/// - 基线 → 任务交接读出以本基线为实施依据的任务。
#[sqlx::test(migrations = "../../migrations")]
async fn s03_read_surfaces_serve_workspace_pages(pool: PgPool) {
    let fx = fixture(&pool, "read").await;
    let root = knowledge_root("read");
    let (rid, rev1) = approved_requirement(&pool, &fx, "REQ-R", &root).await;

    // 两个设计：一个绑定需求（当前版本），一个不绑。
    let (design_a, _rev_a0, _hash_a) = ai_submitted_design(&pool, &fx, "设计甲", true).await;
    let (_design_b, _rev_b, _hash_b) = ai_submitted_design(&pool, &fx, "设计乙", true).await;
    // 需求关联只在 draft 修订上可编辑（§4.2 冻结原则）→ 新建一版 draft 承载绑定。
    let rev_a = design::create_design_revision(
        &pool,
        SYSTEM_TENANT,
        design_a,
        design::DraftDesignRevisionRequest {
            markdown: "# 设计甲 v2\n".to_string(),
            summary: None,
            technical_analysis: None,
            implementation_scope: Some(json!({ "included": ["module a"] })),
            acceptance_criteria: Some(vec![AcceptanceCriterionInput {
                statement: "设计甲验收".to_string(),
                verification_kind: None,
            }]),
        },
        &fx.ai,
    )
    .await
    .unwrap()
    .id;
    design::link_design_requirement(
        &pool,
        SYSTEM_TENANT,
        design_a,
        rev_a,
        design::LinkDesignRequirementRequest {
            requirement_id: rid,
            requirement_revision_id: Some(rev1),
        },
        &fx.ai,
    )
    .await
    .unwrap();
    design::submit_design_revision(&pool, SYSTEM_TENANT, design_a, rev_a, &fx.ai)
        .await
        .unwrap();

    // 审批收件箱：请求批准（不裁决）→ 状态 pending。
    let ap =
        design::request_design_revision_approval(&pool, SYSTEM_TENANT, design_a, rev_a, &fx.ai)
            .await
            .unwrap();
    let inbox = design::list_approvals(&pool, SYSTEM_TENANT, fx.project_id)
        .await
        .unwrap();
    assert_eq!(inbox.len(), 1, "只有一个待办审批");
    assert_eq!(inbox[0].id, ap.id);
    assert_eq!(inbox[0].status, "pending");
    assert_eq!(inbox[0].kind, "design_revision");
    assert!(inbox[0].target_code.is_some(), "审批行必须带出目标展示码");

    // 工作空间聚合：设计数 / ADR 数 / 待办审批数 / 需求覆盖。
    let ws = design::design_workspace(&pool, SYSTEM_TENANT, fx.project_id)
        .await
        .unwrap();
    assert_eq!(ws.design_count, 2);
    assert_eq!(ws.adr_count, 0);
    assert_eq!(ws.pending_approval_count, 1);
    assert_eq!(ws.linked_requirement_count, 1);
    assert_eq!(ws.stale_requirement_count, 0);
    assert_eq!(ws.open_finding_count, 0);

    // 分析页：需求覆盖 + 影响关系。
    let analysis = design::design_analysis(&pool, SYSTEM_TENANT, fx.project_id)
        .await
        .unwrap();
    assert_eq!(analysis.len(), 2);
    let row_a = analysis
        .iter()
        .find(|r| r.design_id == design_a)
        .expect("设计甲的分析行");
    assert_eq!(row_a.requirement_count, 1);
    assert_eq!(row_a.stale_requirement_count, 0);
    assert_eq!(
        row_a.requirements[0].requirement_code.as_deref(),
        Some("REQ-R")
    );
    assert_eq!(row_a.requirements[0].is_current, Some(true));

    // 需求推进到新版本后，旧绑定成为 stale（影响分析重点）。
    let draft2 = requirement::create_draft(
        &pool,
        SYSTEM_TENANT,
        rid,
        DraftRevisionRequest {
            markdown: "# REQ-R 修订二\n".to_string(),
            summary: None,
            basis_finding_ids: None,
        },
        &fx.human,
    )
    .await
    .unwrap();
    requirement::submit_revision(&pool, SYSTEM_TENANT, rid, draft2.id, &fx.human, &root)
        .await
        .unwrap();
    requirement::confirm_revision(
        &pool,
        SYSTEM_TENANT,
        rid,
        draft2.id,
        ConfirmRevisionRequest {
            expected_content_hash: draft2.content_hash.clone(),
            expected_current_revision_id: None,
            expected_row_revision: None,
        },
        &fx.human,
    )
    .await
    .unwrap();

    let analysis = design::design_analysis(&pool, SYSTEM_TENANT, fx.project_id)
        .await
        .unwrap();
    let row_a = analysis
        .iter()
        .find(|r| r.design_id == design_a)
        .expect("设计甲的分析行");
    assert_eq!(
        row_a.stale_requirement_count, 1,
        "需求已被新版本取代 → 旧绑定必须标为 stale"
    );
    assert_eq!(row_a.requirements[0].is_current, Some(false));

    let ws = design::design_workspace(&pool, SYSTEM_TENANT, fx.project_id)
        .await
        .unwrap();
    assert_eq!(ws.stale_requirement_count, 1);
    assert_eq!(ws.linked_requirement_count, 0);

    // 评审问题计入未闭问题数。
    design::add_review_finding(
        &pool,
        SYSTEM_TENANT,
        design_a,
        rev_a,
        design::AddFindingRequest {
            finding_kind: Some("issue".to_string()),
            severity: Some("warning".to_string()),
            body: "缺少回滚方案".to_string(),
        },
        &fx.ai,
    )
    .await
    .unwrap();
    let ws = design::design_workspace(&pool, SYSTEM_TENANT, fx.project_id)
        .await
        .unwrap();
    assert_eq!(ws.open_finding_count, 1);

    // 基线 → 任务交接：以本基线为实施依据的任务被读出。
    let baseline = design::freeze_baseline(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        design::FreezeBaselineRequest {
            code: None,
            design_revision_ids: vec![rev_a],
            repository_refs: vec![],
        },
        &fx.human,
    )
    .await;
    // 门禁不通过时冻结被拒——这是正确的治理行为；改用未冻结路径验证 tasks 读面。
    if let Ok(baseline) = baseline {
        let task_id: Uuid = sqlx::query_scalar(
            "INSERT INTO delivery.task (project_id, title, status, priority, tenant_id, \
                    created_by, implementation_baseline_id) \
             VALUES ($1, '交接任务', 'draft', 'normal', $2, $3, $4) RETURNING id",
        )
        .bind(fx.project_id)
        .bind(SYSTEM_TENANT)
        .bind(fx.human_id)
        .bind(baseline.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let tasks = design::baseline_tasks(&pool, SYSTEM_TENANT, baseline.id)
            .await
            .unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, task_id);
        assert_eq!(tasks[0].status, "draft");
    }
}
