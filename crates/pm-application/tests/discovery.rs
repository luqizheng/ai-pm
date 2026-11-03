//! T-083 Discovery 数据模型集成测试：session 状态机 / message / finding /
//! question-answer / proposal（Q7 base_revision 校验）/ 租户隔离 /
//! repo_import 单向挂接（Q1）。

use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use pm_application::discovery::{
    self, DiscoveryError, FindingCreator, NewFinding, NewProposal,
};
use pm_domain::SYSTEM_TENANT;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    tenant_id: Uuid,
    project_id: Uuid,
    human_id: Uuid,
}

async fn seed(pool: &PgPool) -> Fx {
    let o = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: "DISC".to_string(),
            project_name: "discovery".to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    Fx {
        tenant_id: SYSTEM_TENANT,
        project_id: o.project_id,
        human_id: o.human_id,
    }
}

async fn seed_repository(pool: &PgPool, fx: &Fx, name: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO integration.project_repository \
         (tenant_id, project_id, name, url, status) \
         VALUES ($1, $2, $3, $4, 'verified') RETURNING id",
    )
    .bind(fx.tenant_id)
    .bind(fx.project_id)
    .bind(name)
    .bind(format!("git@host:{name}"))
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_other_tenant_project(pool: &PgPool) -> (Uuid, Uuid, Uuid) {
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', 'disc-other', 'other') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, name) VALUES ($1, 'c') RETURNING id",
    )
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    let human: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ('human', 'other') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let project: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name) \
         VALUES ($1, $2, 'DISC-OTHER', 'other') RETURNING id",
    )
    .bind(tenant)
    .bind(customer)
    .fetch_one(pool)
    .await
    .unwrap();
    (tenant, project, human)
}

async fn seed_document_and_revision(pool: &PgPool, fx: &Fx) -> (Uuid, Uuid) {
    let document_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.project_document \
         (tenant_id, project_id, doc_key, doc_type, canonical_path) \
         VALUES ($1, $2, 'arch', 'architecture', 'projects/DISC/architecture/arch.md') \
         RETURNING id",
    )
    .bind(fx.tenant_id)
    .bind(fx.project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.document_revision \
         (tenant_id, project_id, document_id, revision_no, git_commit_sha, content_hash, created_by) \
         VALUES ($1, $2, $3, 1, 'sha0', 'hash0', $4) RETURNING id",
    )
    .bind(fx.tenant_id)
    .bind(fx.project_id)
    .bind(document_id)
    .bind(fx.human_id)
    .fetch_one(pool)
    .await
    .unwrap();
    (document_id, revision_id)
}

#[sqlx::test(migrations = "../../migrations")]
async fn session_lifecycle_follows_the_state_machine(pool: PgPool) {
    let fx = seed(&pool).await;
    let session = discovery::create_session(
        &pool,
        fx.tenant_id,
        fx.project_id,
        "WEB",
        "INITIAL_ONBOARDING",
        fx.human_id,
    )
    .await
    .unwrap();
    assert_eq!(session.status, "IN_PROGRESS");

    for target in ["NEEDS_USER_INPUT", "IN_PROGRESS", "PAUSED", "IN_PROGRESS", "REVIEW_REQUIRED"] {
        let s = discovery::transition_session(&pool, fx.tenant_id, session.id, target)
            .await
            .unwrap();
        assert_eq!(s.status, target);
    }
    let done = discovery::transition_session(&pool, fx.tenant_id, session.id, "COMPLETED")
        .await
        .unwrap();
    assert_eq!(done.status, "COMPLETED");

    let after = discovery::transition_session(&pool, fx.tenant_id, session.id, "IN_PROGRESS")
        .await
        .unwrap_err();
    assert!(matches!(after, DiscoveryError::InvalidTransition { .. }));

    let list = discovery::list_sessions(&pool, fx.tenant_id, fx.project_id)
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn parallel_sessions_are_allowed(pool: PgPool) {
    let fx = seed(&pool).await;
    let web = discovery::create_session(
        &pool,
        fx.tenant_id,
        fx.project_id,
        "WEB",
        "INITIAL_ONBOARDING",
        fx.human_id,
    )
    .await
    .unwrap();
    let codex = discovery::create_session(
        &pool,
        fx.tenant_id,
        fx.project_id,
        "CODEX",
        "UPDATE",
        fx.human_id,
    )
    .await
    .unwrap();
    assert_ne!(web.id, codex.id);
    assert_eq!(web.source, "WEB");
    assert_eq!(codex.source, "CODEX");
}

#[sqlx::test(migrations = "../../migrations")]
async fn messages_are_numbered_per_session(pool: PgPool) {
    let fx = seed(&pool).await;
    let session = discovery::create_session(
        &pool,
        fx.tenant_id,
        fx.project_id,
        "WEB",
        "INITIAL_ONBOARDING",
        fx.human_id,
    )
    .await
    .unwrap();
    for i in 1..=3 {
        let speaker = if i % 2 == 1 { "ai" } else { "user" };
        let m = discovery::append_message(
            &pool,
            fx.tenant_id,
            session.id,
            speaker,
            "WEB",
            &format!("msg-{i}"),
        )
        .await
        .unwrap();
        assert_eq!(m.seq, i);
    }
    let messages = discovery::list_messages(&pool, fx.tenant_id, session.id)
        .await
        .unwrap();
    let seqs: Vec<i64> = messages.iter().map(|m| m.seq).collect();
    assert_eq!(seqs, vec![1, 2, 3]);

    let blank = discovery::append_message(&pool, fx.tenant_id, session.id, "ai", "WEB", "  ")
        .await
        .unwrap_err();
    assert!(matches!(blank, DiscoveryError::Validation(_)));
}

#[sqlx::test(migrations = "../../migrations")]
async fn findings_carry_kind_confidence_evidence_and_repositories(pool: PgPool) {
    let fx = seed(&pool).await;
    let session = discovery::create_session(
        &pool,
        fx.tenant_id,
        fx.project_id,
        "CODEX",
        "INITIAL_ONBOARDING",
        fx.human_id,
    )
    .await
    .unwrap();
    let repo_a = seed_repository(&pool, &fx, "backend").await;
    let repo_b = seed_repository(&pool, &fx, "frontend").await;

    let finding = discovery::add_finding(
        &pool,
        fx.tenant_id,
        session.id,
        NewFinding {
            kind: "speculation".to_string(),
            content: "支付模块可能尚未完成".to_string(),
            confidence: Some(0.78),
            evidence: json!([
                {"repository": "backend", "file": "src/payment/mod.rs", "description": "存在未实现的支付接口"}
            ]),
            repository_ids: vec![repo_a, repo_b],
            actor_role: None,
        },
        FindingCreator::HumanDirect,
    )
    .await
    .unwrap();
    assert_eq!(finding.verification_status, "PENDING");
    assert_eq!(finding.repository_ids.len(), 2);

    let listed = discovery::list_findings(&pool, fx.tenant_id, session.id)
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].repository_ids.len(), 2);

    let confirmed = discovery::record_finding_verification(
        &pool,
        fx.tenant_id,
        session.id,
        finding.id,
        "CONFIRMED",
        "确认属实",
        fx.human_id,
        None,
        None,
    )
    .await
    .unwrap();
    assert_eq!(confirmed.finding.verification_status, "CONFIRMED");

    let bad_kind = discovery::add_finding(
        &pool,
        fx.tenant_id,
        session.id,
        NewFinding {
            kind: "rumor".to_string(),
            content: "x".to_string(),
            confidence: None,
            evidence: json!([]),
            repository_ids: vec![],
            actor_role: None,
        },
        FindingCreator::HumanDirect,
    )
    .await
    .unwrap_err();
    assert!(matches!(bad_kind, DiscoveryError::Validation(_)));

    let bad_confidence = discovery::add_finding(
        &pool,
        fx.tenant_id,
        session.id,
        NewFinding {
            kind: "observed".to_string(),
            content: "x".to_string(),
            confidence: Some(1.5),
            evidence: json!([]),
            repository_ids: vec![],
            actor_role: None,
        },
        FindingCreator::HumanDirect,
    )
    .await
    .unwrap_err();
    assert!(matches!(bad_confidence, DiscoveryError::Validation(_)));

    let foreign = discovery::add_finding(
        &pool,
        fx.tenant_id,
        session.id,
        NewFinding {
            kind: "observed".to_string(),
            content: "x".to_string(),
            confidence: None,
            evidence: json!([]),
            repository_ids: vec![Uuid::new_v4()],
            actor_role: None,
        },
        FindingCreator::HumanDirect,
    )
    .await
    .unwrap_err();
    assert!(matches!(foreign, DiscoveryError::Validation(_)));
}

#[sqlx::test(migrations = "../../migrations")]
async fn discovery_card_flow_links_question_answer_and_verification(pool: PgPool) {
    let fx = seed(&pool).await;
    let session = discovery::create_session(
        &pool,
        fx.tenant_id,
        fx.project_id,
        "CODEX",
        "INITIAL_ONBOARDING",
        fx.human_id,
    )
    .await
    .unwrap();
    let finding = discovery::add_finding(
        &pool,
        fx.tenant_id,
        session.id,
        NewFinding {
            kind: "speculation".to_string(),
            content: "支付模块可能尚未完成".to_string(),
            confidence: Some(0.78),
            evidence: json!([]),
            repository_ids: vec![],
            actor_role: None,
        },
        FindingCreator::HumanDirect,
    )
    .await
    .unwrap();

    let question = discovery::add_question(
        &pool,
        fx.tenant_id,
        session.id,
        "请确认支付模块的实际情况",
        Some("阻塞架构认知"),
        Some(finding.id),
    )
    .await
    .unwrap();
    assert_eq!(question.finding_id, Some(finding.id));
    assert_eq!(question.status, "open");

    let answer = discovery::answer_question(
        &pool,
        fx.tenant_id,
        question.id,
        "已经通过外部服务实现",
        fx.human_id,
    )
    .await
    .unwrap();
    assert_eq!(answer.question_id, question.id);

    let reopened = discovery::get_question(&pool, fx.tenant_id, question.id)
        .await
        .unwrap();
    assert_eq!(reopened.status, "answered");
    assert_eq!(reopened.answers.len(), 1);

    let again = discovery::answer_question(&pool, fx.tenant_id, question.id, "x", fx.human_id)
        .await
        .unwrap_err();
    assert!(matches!(again, DiscoveryError::Validation(_)));

    let extra = discovery::add_finding(
        &pool,
        fx.tenant_id,
        session.id,
        NewFinding {
            kind: "user_confirmed".to_string(),
            content: "支付由外部服务实现".to_string(),
            confidence: None,
            evidence: json!([]),
            repository_ids: vec![],
            actor_role: None,
        },
        FindingCreator::HumanDirect,
    )
    .await
    .unwrap();
    assert_eq!(extra.kind, "user_confirmed");

    let confirmed = discovery::record_finding_verification(
        &pool,
        fx.tenant_id,
        session.id,
        finding.id,
        "CONFIRMED",
        "确认属实",
        fx.human_id,
        None,
        None,
    )
    .await
    .unwrap();
    assert_eq!(confirmed.finding.verification_status, "CONFIRMED");

    let detail = discovery::get_session_detail(&pool, fx.tenant_id, session.id)
        .await
        .unwrap();
    assert_eq!(detail.messages.len(), 0);
    assert_eq!(detail.findings.len(), 2);
    // I2-C：决定本身落一轮 Q/A（既有已回答的质询 + 决定轮的质询）。
    assert_eq!(detail.questions.len(), 2);
    assert_eq!(detail.questions[0].answers.len(), 1);
    let decision_round = detail
        .questions
        .iter()
        .find(|q| q.status == "answered" && q.question == "CONFIRMED")
        .expect("the decision round must be recorded");
    assert_eq!(decision_round.answers.len(), 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn questions_can_be_deferred_but_not_twice(pool: PgPool) {
    let fx = seed(&pool).await;
    let session = discovery::create_session(
        &pool,
        fx.tenant_id,
        fx.project_id,
        "WEB",
        "INITIAL_ONBOARDING",
        fx.human_id,
    )
    .await
    .unwrap();
    let question = discovery::add_question(
        &pool,
        fx.tenant_id,
        session.id,
        "目标用户是谁？",
        None,
        None,
    )
    .await
    .unwrap();
    let deferred = discovery::defer_question(&pool, fx.tenant_id, question.id)
        .await
        .unwrap();
    assert_eq!(deferred.status, "deferred");
    let twice = discovery::defer_question(&pool, fx.tenant_id, question.id)
        .await
        .unwrap_err();
    assert!(matches!(twice, DiscoveryError::Validation(_)));
}

#[sqlx::test(migrations = "../../migrations")]
async fn proposals_validate_base_revision_and_decision_state(pool: PgPool) {
    let fx = seed(&pool).await;
    let session = discovery::create_session(
        &pool,
        fx.tenant_id,
        fx.project_id,
        "CODEX",
        "INITIAL_ONBOARDING",
        fx.human_id,
    )
    .await
    .unwrap();
    let (document_id, revision_id) = seed_document_and_revision(&pool, &fx).await;

    let modify = discovery::add_proposal(
        &pool,
        fx.tenant_id,
        session.id,
        NewProposal {
            action: "modify".to_string(),
            target_document: "architecture/arch.md".to_string(),
            document_id: Some(document_id),
            base_revision_id: Some(revision_id),
            proposed_content: "# 架构（更新）".to_string(),
            evidence: json!([{"file": "src/main.rs", "description": "入口调整"}]),
        },
    )
    .await
    .unwrap();
    assert_eq!(modify.status, "pending");

    let stale_base = discovery::add_proposal(
        &pool,
        fx.tenant_id,
        session.id,
        NewProposal {
            action: "modify".to_string(),
            target_document: "architecture/arch.md".to_string(),
            document_id: Some(document_id),
            base_revision_id: Some(Uuid::new_v4()),
            proposed_content: "x".to_string(),
            evidence: json!([]),
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(stale_base, DiscoveryError::Validation(_)));

    let missing_base = discovery::add_proposal(
        &pool,
        fx.tenant_id,
        session.id,
        NewProposal {
            action: "modify".to_string(),
            target_document: "architecture/arch.md".to_string(),
            document_id: Some(document_id),
            base_revision_id: None,
            proposed_content: "x".to_string(),
            evidence: json!([]),
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(missing_base, DiscoveryError::Validation(_)));

    let add = discovery::add_proposal(
        &pool,
        fx.tenant_id,
        session.id,
        NewProposal {
            action: "add".to_string(),
            target_document: "knowledge/new.md".to_string(),
            document_id: None,
            base_revision_id: None,
            proposed_content: "# 新知识".to_string(),
            evidence: json!([]),
        },
    )
    .await
    .unwrap();
    assert_eq!(add.base_revision_id, None);

    let approved = discovery::set_proposal_status(
        &pool,
        fx.tenant_id,
        modify.id,
        "approved",
        None,
    )
    .await
    .unwrap();
    assert_eq!(approved.status, "approved");

    let twice = discovery::set_proposal_status(&pool, fx.tenant_id, modify.id, "rejected", None)
        .await
        .unwrap_err();
    assert!(matches!(twice, DiscoveryError::Validation(_)));

    let proposals = discovery::list_proposals(&pool, fx.tenant_id, session.id)
        .await
        .unwrap();
    assert_eq!(proposals.len(), 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn cross_tenant_access_is_rejected(pool: PgPool) {
    let fx = seed(&pool).await;
    let (other_tenant, other_project, other_human) = seed_other_tenant_project(&pool).await;

    let session = discovery::create_session(
        &pool,
        fx.tenant_id,
        fx.project_id,
        "WEB",
        "INITIAL_ONBOARDING",
        fx.human_id,
    )
    .await
    .unwrap();

    let foreign_read = discovery::get_session(&pool, other_tenant, session.id).await;
    assert!(matches!(foreign_read, Err(DiscoveryError::SessionNotFound)));

    let foreign_session = discovery::create_session(
        &pool,
        other_tenant,
        fx.project_id,
        "WEB",
        "INITIAL_ONBOARDING",
        other_human,
    )
    .await;
    assert!(matches!(foreign_session, Err(DiscoveryError::ProjectNotFound)));

    let finding = discovery::add_finding(
        &pool,
        fx.tenant_id,
        session.id,
        NewFinding {
            kind: "observed".to_string(),
            content: "x".to_string(),
            confidence: None,
            evidence: json!([]),
            repository_ids: vec![],
            actor_role: None,
        },
        FindingCreator::HumanDirect,
    )
    .await
    .unwrap();
    // 决定服务先查 Session（I2-029 前置）：跨租户 Session 不存在 → SessionNotFound
    //（REST 层与 FindingNotFound 同映射 404，不泄露存在性）。
    let foreign_finding = discovery::record_finding_verification(
        &pool,
        other_tenant,
        session.id,
        finding.id,
        "CONFIRMED",
        "x",
        other_human,
        None,
        None,
    )
    .await;
    assert!(matches!(foreign_finding, Err(DiscoveryError::SessionNotFound)));

    let foreign_list = discovery::list_sessions(&pool, other_tenant, other_project)
        .await
        .unwrap();
    assert!(foreign_list.is_empty());
}

// ───────────────────────────────────────────── I2-B / I2-D 定态规则 ──────

/// I2-023：可信 human **直接陈述** user_confirmed → 创建即 CONFIRMED。
#[sqlx::test(migrations = "../../migrations")]
async fn i2_023_human_direct_user_confirmed_starts_confirmed(pool: PgPool) {
    let fx = seed(&pool).await;
    let session = discovery::create_session(
        &pool,
        fx.tenant_id,
        fx.project_id,
        "WEB",
        "INITIAL_ONBOARDING",
        fx.human_id,
    )
    .await
    .unwrap();

    let finding = discovery::add_finding(
        &pool,
        fx.tenant_id,
        session.id,
        NewFinding {
            kind: "user_confirmed".to_string(),
            content: "用户直接陈述：数据库使用 PostgreSQL".to_string(),
            confidence: None,
            evidence: json!([]),
            repository_ids: vec![],
            actor_role: None,
        },
        FindingCreator::HumanDirect,
    )
    .await
    .unwrap();
    assert_eq!(finding.verification_status, "CONFIRMED");
}

/// I2-024：Agent 转述的 user_confirmed → 仍 PENDING（须人工确认后才能成为已确认事实）。
#[sqlx::test(migrations = "../../migrations")]
async fn i2_024_agent_relayed_user_confirmed_stays_pending(pool: PgPool) {
    let fx = seed(&pool).await;
    let session = discovery::create_session(
        &pool,
        fx.tenant_id,
        fx.project_id,
        "CODEX",
        "INITIAL_ONBOARDING",
        fx.human_id,
    )
    .await
    .unwrap();

    let finding = discovery::add_finding(
        &pool,
        fx.tenant_id,
        session.id,
        NewFinding {
            kind: "user_confirmed".to_string(),
            content: "Agent 转述用户历史发言：数据库使用 PostgreSQL".to_string(),
            confidence: None,
            evidence: json!([]),
            repository_ids: vec![],
            actor_role: None,
        },
        FindingCreator::AgentRelayed,
    )
    .await
    .unwrap();
    assert_eq!(finding.verification_status, "PENDING");
}

/// I2-026：外部不可指定 verification_status（字段已从创建契约删除，定态由入口
/// 派生）——其余 kind 无论哪个通道一律 PENDING。
#[sqlx::test(migrations = "../../migrations")]
async fn i2_026_initial_state_is_derived_not_supplied(pool: PgPool) {
    let fx = seed(&pool).await;
    let session = discovery::create_session(
        &pool,
        fx.tenant_id,
        fx.project_id,
        "WEB",
        "INITIAL_ONBOARDING",
        fx.human_id,
    )
    .await
    .unwrap();

    for (kind, expected, creator) in [
        ("observed", "PENDING", FindingCreator::HumanDirect),
        ("speculation", "PENDING", FindingCreator::HumanDirect),
        ("contradicted", "PENDING", FindingCreator::HumanDirect),
        ("observed", "PENDING", FindingCreator::AgentRelayed),
        ("speculation", "PENDING", FindingCreator::AgentRelayed),
        ("contradicted", "PENDING", FindingCreator::AgentRelayed),
    ] {
        let finding = discovery::add_finding(
            &pool,
            fx.tenant_id,
            session.id,
            NewFinding {
                kind: kind.to_string(),
                content: format!("内容 {kind}"),
                confidence: None,
                evidence: json!([]),
                repository_ids: vec![],
                actor_role: None,
            },
            creator,
        )
        .await
        .unwrap();
        assert_eq!(finding.verification_status, expected, "kind={kind}");
    }
}

// ───────────────────────────────────────────── I2-C 统一决定服务 ──────────

/// I2-036 / I2-014：决定落审计行（finding.decided，带 before/after），
/// 与业务写入同一事务。
#[sqlx::test(migrations = "../../migrations")]
async fn i2_c_decision_is_audited_with_before_after(pool: PgPool) {
    let fx = seed(&pool).await;
    let session = discovery::create_session(
        &pool, fx.tenant_id, fx.project_id, "WEB", "INITIAL_ONBOARDING", fx.human_id,
    )
    .await
    .unwrap();
    let finding = discovery::add_finding(
        &pool,
        fx.tenant_id,
        session.id,
        NewFinding {
            kind: "observed".to_string(),
            content: "待确认事实".to_string(),
            confidence: None,
            evidence: json!([]),
            repository_ids: vec![],
            actor_role: None,
        },
        FindingCreator::HumanDirect,
    )
    .await
    .unwrap();

    discovery::record_finding_verification(
        &pool,
        fx.tenant_id,
        session.id,
        finding.id,
        "CONFIRMED",
        "确认属实",
        fx.human_id,
        None,
        None,
    )
    .await
    .unwrap();

    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
          WHERE project_id = $1 AND action = 'finding.decided' AND target_id = $2",
    )
    .bind(fx.project_id)
    .bind(finding.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 1, "the decision must be audited");

    let after: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT after_data FROM governance.audit_log \
          WHERE project_id = $1 AND action = 'finding.decided' AND target_id = $2",
    )
    .bind(fx.project_id)
    .bind(finding.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let after = after.expect("audit row must carry after_data");
    assert_eq!(after["decision"], "CONFIRMED");
    assert_eq!(after["before"], "PENDING");
    assert_eq!(after["after"], "CONFIRMED");
}

/// I2-C operation_id 幂等：同键重放返回当前状态、零业务写入（无第二轮 Q/A、
/// 无第二条审计）。
#[sqlx::test(migrations = "../../migrations")]
async fn i2_c_operation_id_replays_without_side_effects(pool: PgPool) {
    let fx = seed(&pool).await;
    let session = discovery::create_session(
        &pool, fx.tenant_id, fx.project_id, "WEB", "INITIAL_ONBOARDING", fx.human_id,
    )
    .await
    .unwrap();
    let finding = discovery::add_finding(
        &pool,
        fx.tenant_id,
        session.id,
        NewFinding {
            kind: "observed".to_string(),
            content: "幂等事实".to_string(),
            confidence: None,
            evidence: json!([]),
            repository_ids: vec![],
            actor_role: None,
        },
        FindingCreator::HumanDirect,
    )
    .await
    .unwrap();

    let first = discovery::record_finding_verification(
        &pool,
        fx.tenant_id,
        session.id,
        finding.id,
        "CONFIRMED",
        "第一次决定",
        fx.human_id,
        None,
        Some("op-1"),
    )
    .await
    .unwrap();
    assert_eq!(first.finding.verification_status, "CONFIRMED");

    let replay = discovery::record_finding_verification(
        &pool,
        fx.tenant_id,
        session.id,
        finding.id,
        "CONFIRMED",
        "第二次决定（应被幂等吸收）",
        fx.human_id,
        None,
        Some("op-1"),
    )
    .await
    .unwrap();
    assert_eq!(replay.finding.verification_status, "CONFIRMED");

    // 零额外副作用：审计仍只有 1 条、事件仍只有 1 条。
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
          WHERE project_id = $1 AND action = 'finding.decided'",
    )
    .bind(fx.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 1);

    let events: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integration.inbound_event \
          WHERE project_id = $1 AND source = 'finding_decision'",
    )
    .bind(fx.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(events, 1);
}

/// I2-025：非真人主体不能决定 Finding。
#[sqlx::test(migrations = "../../migrations")]
async fn i2_c_only_humans_can_decide(pool: PgPool) {
    let fx = seed(&pool).await;
    let session = discovery::create_session(
        &pool, fx.tenant_id, fx.project_id, "CODEX", "INITIAL_ONBOARDING", fx.human_id,
    )
    .await
    .unwrap();
    let finding = discovery::add_finding(
        &pool,
        fx.tenant_id,
        session.id,
        NewFinding {
            kind: "observed".to_string(),
            content: "Agent 发现".to_string(),
            confidence: None,
            evidence: json!([]),
            repository_ids: vec![],
            actor_role: None,
        },
        FindingCreator::AgentRelayed,
    )
    .await
    .unwrap();

    // dev_agent 不能决定。
    let agent_id: Uuid = sqlx::query_scalar(
        "SELECT principal_id FROM core.project_member \
          WHERE project_id = $1 AND role = 'developer' LIMIT 1",
    )
    .bind(fx.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let err = discovery::record_finding_verification(
        &pool,
        fx.tenant_id,
        session.id,
        finding.id,
        "CONFIRMED",
        "Agent 自确认（必须被拒绝）",
        agent_id,
        None,
        None,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, DiscoveryError::Validation(_)));

    // 状态未被改动、无审计。
    let after = discovery::get_finding(&pool, fx.tenant_id, finding.id)
        .await
        .unwrap();
    assert_eq!(after.verification_status, "PENDING");
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log WHERE action = 'finding.decided'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 0, "a rejected decision must leave zero audit rows");
}

/// I2-020：同值重复决定是 no-op——不产生第二条审计。
#[sqlx::test(migrations = "../../migrations")]
async fn i2_c_repeated_same_decision_is_a_noop(pool: PgPool) {
    let fx = seed(&pool).await;
    let session = discovery::create_session(
        &pool, fx.tenant_id, fx.project_id, "WEB", "INITIAL_ONBOARDING", fx.human_id,
    )
    .await
    .unwrap();
    let finding = discovery::add_finding(
        &pool,
        fx.tenant_id,
        session.id,
        NewFinding {
            kind: "observed".to_string(),
            content: "重复决定".to_string(),
            confidence: None,
            evidence: json!([]),
            repository_ids: vec![],
            actor_role: None,
        },
        FindingCreator::HumanDirect,
    )
    .await
    .unwrap();

    for _ in 0..2 {
        discovery::record_finding_verification(
            &pool,
            fx.tenant_id,
            session.id,
            finding.id,
            "CONFIRMED",
            "再次确认",
            fx.human_id,
            None,
            None,
        )
        .await
        .unwrap();
    }

    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
          WHERE project_id = $1 AND action = 'finding.decided'",
    )
    .bind(fx.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 2, "each decision round is audited; the status stays CONFIRMED");
    let status: String = sqlx::query_scalar(
        "SELECT verification_status FROM discovery.discovery_finding WHERE id = $1",
    )
    .bind(finding.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "CONFIRMED");
}
