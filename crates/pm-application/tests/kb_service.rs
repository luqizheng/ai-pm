//! KP-3 KB 三层最小面：端口集成测试（K4 / K5 经 `KbServicePort`）+ L1 审核门负向用例。
//!
//! 覆盖：
//! - K5：团队池登记去重、一源多项目、项目隔离、`evidence_of`、越作用域拒绝；
//! - K4：approve 产生版本并 SUPERSEDED 旧版本、`kb.memory.approved` 同事务入 outbox
//!   （目录三字段 + 幂等键）、reject / revise 不产版本不产事件、WITHDRAW 终态；
//! - **L1 审核门负向**：AI 主体审核 403、过期 CAS 409、无待审 409、在途提案时再提案
//!   409、未审核提案不是版本（库层直写 CURRENT 被拒）、版本内容不可改、不可物理删除、
//!   项目归档后写入 409、审计四身份列全写；
//! - 检索：只返回 CURRENT 版本（待审 / 被替代 / 撤回均不进入）。

use kb_pc_contracts::ports::{KbMemoryReviewRequest, KbServicePort, PortError};
use kb_pc_contracts::{DomainRef, TargetDomain};
use pm_application::actor::AuditActor;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::kb_service::memory::{
    self, ChangeAction, MemoryContent, RefRole, ResolvedRef, ReviewAction,
};
use pm_application::kb_service::retrieval::{current_memories_for_subjects, SubjectKey};
use pm_application::kb_service::source::{self, RegisterSourceRequest};
use pm_application::kb_service::{KbCaller, KbServiceError, KbServiceLive};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    project_id: Uuid,
    human: AuditActor,
    ai: AuditActor,
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

async fn project(pool: &PgPool, code: &str) -> Uuid {
    bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap()
    .project_id
}

async fn seed(pool: &PgPool, code: &str) -> Fx {
    let project_id = project(pool, code).await;
    let human_id = principal(pool, "human", "reviewer").await;
    let ai_id = principal(pool, "ai_pm", "ai-pm").await;
    // 真人身份行（0018/0019）：四身份列全写断言与 audit_log FK 的前置。
    let identity_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal_identity (provider, tenant_id, subject, principal_id) \
         VALUES ('lmms', $1, $2, $3) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .bind(Uuid::new_v4().to_string())
    .bind(human_id)
    .fetch_one(pool)
    .await
    .unwrap();
    Fx {
        project_id,
        // 真人：带认证身份三列（四身份列全写断言用）。
        human: AuditActor::identified(
            human_id,
            identity_id,
            Some("web".into()),
            Some("interactive".into()),
        ),
        ai: AuditActor::principal(ai_id),
    }
}

fn caller(fx: &Fx, actor: &AuditActor) -> KbCaller {
    KbCaller {
        tenant_id: SYSTEM_TENANT,
        project_id: fx.project_id,
        actor: actor.clone(),
    }
}

fn inline_source(text: &str) -> RegisterSourceRequest {
    RegisterSourceRequest {
        source_kind: "meeting_notes".into(),
        title: "架构评审会纪要".into(),
        uri: None,
        inline_content: Some(text.into()),
        mime: Some("text/markdown".into()),
        size_bytes: None,
        content_hash: None,
    }
}

fn source_evidence(source_id: Uuid) -> ResolvedRef {
    ResolvedRef {
        role: RefRole::Evidence,
        target_domain: "knowledge_base".into(),
        target_type: "source".into(),
        target_id: source_id.to_string(),
        target_revision_id: None,
        revision_missing_reason: Some("no_revision_mechanism".into()),
        locator: Some(serde_json::json!({ "section": "决议" })),
    }
}

fn task_subject(task_id: Uuid) -> ResolvedRef {
    ResolvedRef {
        role: RefRole::Subject,
        target_domain: "requirement".into(),
        target_type: "task".into(),
        target_id: task_id.to_string(),
        target_revision_id: None,
        revision_missing_reason: Some("no_revision_mechanism".into()),
        locator: None,
    }
}

fn content(title: &str, summary: &str) -> MemoryContent {
    MemoryContent {
        memory_type: "rejected_option".into(),
        title: title.into(),
        summary: summary.into(),
        body: Some("详细讨论记录见纪要。".into()),
    }
}

/// 来源 + 一条 CREATE 提案（待审）。返回 (source_id, memory_id, revision_id, task_key)。
async fn propose_first(pool: &PgPool, fx: &Fx, tag: &str) -> (Uuid, Uuid, Uuid, Uuid) {
    let src = source::register_source(
        pool,
        SYSTEM_TENANT,
        fx.project_id,
        &fx.human,
        inline_source(&format!("纪要正文 {tag}")),
    )
    .await
    .unwrap();
    let task_key = Uuid::new_v4();
    let receipt = memory::propose_create(
        pool,
        SYSTEM_TENANT,
        fx.project_id,
        &fx.ai, // AI 可以提议
        content("放弃方案 X", "因延迟不可控否决方案 X"),
        "沉淀架构评审会结论",
        vec![source_evidence(src.source_id), task_subject(task_key)],
    )
    .await
    .unwrap();
    (
        src.source_id,
        receipt.memory_id,
        receipt.revision_id,
        task_key,
    )
}

fn review_request(action: &str, base: Uuid) -> KbMemoryReviewRequest {
    KbMemoryReviewRequest {
        action: action.into(),
        note: Some("结论与纪要一致".into()),
        base_memory_revision_id: base.to_string(),
    }
}

async fn outbox_rows(pool: &PgPool, project_id: Uuid) -> Vec<(String, serde_json::Value, String)> {
    sqlx::query_as(
        "SELECT event_type, payload, idempotency_key FROM integration.outbox_event \
          WHERE project_id = $1 AND event_type = 'kb.memory.approved' ORDER BY created_at, id",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .unwrap()
}

// ── K5 ──────────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn k5_registers_dedups_links_and_isolates(pool: PgPool) {
    let fx = seed(&pool, "KP3-K5").await;
    let other_project = project(&pool, "KP3-K5-B").await;
    let third_project = project(&pool, "KP3-K5-C").await;

    let first = source::register_source(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        &fx.human,
        inline_source("同一份纪要"),
    )
    .await
    .unwrap();
    assert!(!first.deduplicated && first.newly_linked);
    assert_eq!(
        first.content_hash,
        pm_application::knowledge::content_hash("同一份纪要")
    );

    // 同项目重放：零副作用（不新建、不新关联、不写审计）。
    let replay = source::register_source(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        &fx.human,
        inline_source("同一份纪要"),
    )
    .await
    .unwrap();
    assert_eq!(replay.source_id, first.source_id);
    assert!(replay.deduplicated && !replay.newly_linked);

    // 另一项目登记同内容：统一身份，只追加关联（一源多项目，定义稿 §6.2）。
    let shared = source::register_source(
        &pool,
        SYSTEM_TENANT,
        other_project,
        &fx.human,
        inline_source("同一份纪要"),
    )
    .await
    .unwrap();
    assert_eq!(shared.source_id, first.source_id);
    assert!(shared.deduplicated && shared.newly_linked);
    let source_rows: i64 = sqlx::query_scalar("SELECT count(*) FROM kb.source")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(source_rows, 1);

    let audits: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM governance.audit_log WHERE source_kind = 'kb_service' \
          AND target_id = $1 ORDER BY created_at, id",
    )
    .bind(first.source_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(audits, vec!["kb.source.registered", "kb.source.linked"]);

    // K5 经端口：项目 A 可见；未关联项目 C 看不到（不泄露团队池）。
    let port_a = KbServiceLive::for_caller(pool.clone(), caller(&fx, &fx.human));
    let items = port_a.list_sources(fx.project_id, None).await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].source_id, first.source_id.to_string());
    assert_eq!(items[0].content_hash, first.content_hash);
    let port_c = KbServiceLive::for_caller(
        pool.clone(),
        KbCaller {
            tenant_id: SYSTEM_TENANT,
            project_id: third_project,
            actor: fx.human.clone(),
        },
    );
    assert!(port_c
        .list_sources(third_project, None)
        .await
        .unwrap()
        .is_empty());
    assert!(matches!(
        source::get_source(&pool, third_project, first.source_id).await,
        Err(KbServiceError::NotFound(_))
    ));
    // 端口作用域外的项目：拒绝（调用方身份与项目作用域绑定在端口实例，D-KP3-3）。
    assert!(matches!(
        port_a.list_sources(other_project, None).await,
        Err(PortError::Forbidden(_))
    ));
}

#[sqlx::test(migrations = "../../migrations")]
async fn k5_evidence_of_lists_sources_cited_by_memory(pool: PgPool) {
    let fx = seed(&pool, "KP3-EVI").await;
    let (source_id, memory_id, revision_id, _) = propose_first(&pool, &fx, "evi").await;
    // 未登记的第二个来源不应出现在 evidence_of 结果中。
    source::register_source(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        &fx.human,
        inline_source("无关资料"),
    )
    .await
    .unwrap();
    let port = KbServiceLive::for_caller(pool.clone(), caller(&fx, &fx.human));

    let by_revision = DomainRef::for_domain(
        TargetDomain::KnowledgeBase,
        "memory_revision",
        revision_id.to_string(),
        None,
    );
    let cited = port
        .list_sources(fx.project_id, Some(&by_revision))
        .await
        .unwrap();
    assert_eq!(cited.len(), 1);
    assert_eq!(cited[0].source_id, source_id.to_string());
    assert_eq!(cited[0].evidence_of.as_ref(), Some(&by_revision));

    // `memory` 维度 = CURRENT 版本的 evidence：批准前没有 CURRENT → 空。
    let by_memory = DomainRef::for_domain(
        TargetDomain::KnowledgeBase,
        "memory",
        memory_id.to_string(),
        None,
    );
    assert!(port
        .list_sources(fx.project_id, Some(&by_memory))
        .await
        .unwrap()
        .is_empty());
    port.review_memory(
        &memory_id.to_string(),
        review_request("approve", revision_id),
    )
    .await
    .unwrap();
    assert_eq!(
        port.list_sources(fx.project_id, Some(&by_memory))
            .await
            .unwrap()
            .len(),
        1
    );

    // 非 knowledge_base / 非记忆类型的 evidence_of：400 语义。
    let bad = DomainRef::for_domain(TargetDomain::KnowledgeBase, "document", "x", None);
    assert!(matches!(
        port.list_sources(fx.project_id, Some(&bad)).await,
        Err(PortError::Invalid(_))
    ));
}

// ── K4 主路径 ───────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn k4_approve_versions_supersedes_and_emits_event(pool: PgPool) {
    let fx = seed(&pool, "KP3-K4").await;
    let (source_id, memory_id, rev1, _) = propose_first(&pool, &fx, "k4").await;
    let port = KbServiceLive::for_caller(pool.clone(), caller(&fx, &fx.human));

    let status = port
        .review_memory(&memory_id.to_string(), review_request("approve", rev1))
        .await
        .unwrap();
    assert_eq!(status.status, "APPROVED");
    assert_eq!(status.memory_draft_refs.len(), 1);
    assert_eq!(status.memory_draft_refs[0].target_type, "memory_revision");
    assert_eq!(status.memory_draft_refs[0].target_id, rev1.to_string());

    let detail = memory::get_memory(&pool, SYSTEM_TENANT, fx.project_id, memory_id)
        .await
        .unwrap();
    assert_eq!(detail.state, "active");
    assert_eq!(detail.current_revision_id, Some(rev1));
    assert_eq!(detail.revisions[0].version_no, Some(1));
    assert_eq!(detail.revisions[0].reviewed_by, Some(fx.human.principal_id));

    // 修改提案 → 批准 → v2 CURRENT，v1 SUPERSEDED（内容原样保留，可历史还原）。
    let rev2 = memory::propose_change(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        memory_id,
        &fx.human,
        ChangeAction::Update,
        Some(content(
            "放弃方案 X（补充）",
            "因延迟不可控且成本高否决方案 X",
        )),
        "补充否决理由",
        vec![source_evidence(source_id)],
    )
    .await
    .unwrap();
    assert_eq!(rev2.revision_no, 2);
    port.review_memory(
        &memory_id.to_string(),
        review_request("approve", rev2.revision_id),
    )
    .await
    .unwrap();
    let detail = memory::get_memory(&pool, SYSTEM_TENANT, fx.project_id, memory_id)
        .await
        .unwrap();
    let states: Vec<(i32, Option<i32>, Option<String>)> = detail
        .revisions
        .iter()
        .map(|r| (r.revision_no, r.version_no, r.version_status.clone()))
        .collect();
    assert_eq!(
        states,
        vec![
            (1, Some(1), Some("SUPERSEDED".into())),
            (2, Some(2), Some("CURRENT".into())),
        ]
    );
    assert_eq!(detail.revisions[0].summary, "因延迟不可控否决方案 X");

    // 每次批准一条 kb.memory.approved（目录三字段；幂等键 = memory_revision_id）。
    let events = outbox_rows(&pool, fx.project_id).await;
    assert_eq!(events.len(), 2);
    let (_, payload, key) = &events[1];
    assert_eq!(key, &format!("kb_memory_approved:{}", rev2.revision_id));
    assert_eq!(payload["memory_ref"]["target_domain"], "knowledge_base");
    assert_eq!(payload["memory_ref"]["target_type"], "memory");
    assert_eq!(payload["memory_ref"]["target_id"], memory_id.to_string());
    assert_eq!(
        payload["memory_ref"]["target_revision_id"],
        rev2.revision_id.to_string()
    );
    assert_eq!(payload["reviewed_by"], fx.human.principal_id.to_string());
    assert_eq!(payload["approval_state"], "CURRENT");
    assert!(kb_pc_contracts::events::validate_payload_references(payload).is_ok());

    // 审计：提案 / 批准全链，四身份列全写（C174/C180）。
    type AuditRow = (String, Uuid, Option<Uuid>, Option<String>, Option<String>);
    let rows: Vec<AuditRow> = sqlx::query_as(
        "SELECT action, actor_id, actor_identity_id, actor_client, actor_auth_mode \
           FROM governance.audit_log WHERE source_kind = 'kb_service' AND target_id = $1 \
          ORDER BY created_at, id",
    )
    .bind(memory_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    let actions: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
    assert_eq!(
        actions,
        vec![
            "kb.memory.change_proposed",
            "kb.memory.change_approved",
            "kb.memory.change_proposed",
            "kb.memory.change_approved",
        ]
    );
    let approved = &rows[1];
    assert_eq!(approved.1, fx.human.principal_id);
    assert_eq!(approved.2, fx.human.identity_id);
    assert_eq!(approved.3.as_deref(), Some("web"));
    assert_eq!(approved.4.as_deref(), Some("interactive"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn k4_reject_and_revise_close_proposal_without_version(pool: PgPool) {
    let fx = seed(&pool, "KP3-REJ").await;
    let (source_id, memory_id, rev1, _) = propose_first(&pool, &fx, "rej").await;
    let port = KbServiceLive::for_caller(pool.clone(), caller(&fx, &fx.human));

    let st = port
        .review_memory(&memory_id.to_string(), review_request("revise", rev1))
        .await
        .unwrap();
    assert_eq!(st.status, "REVISION_REQUESTED");
    let detail = memory::get_memory(&pool, SYSTEM_TENANT, fx.project_id, memory_id)
        .await
        .unwrap();
    assert_eq!(detail.state, "not_adopted");
    assert!(detail.current_revision_id.is_none());
    assert!(detail.revisions[0].version_no.is_none());

    // 被退回的首提案无 CURRENT：不能提修改（须另起新记忆）。
    let err = memory::propose_change(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        memory_id,
        &fx.human,
        ChangeAction::Update,
        Some(content("x", "y")),
        "retry",
        vec![source_evidence(source_id)],
    )
    .await
    .unwrap_err();
    assert!(matches!(err, KbServiceError::Conflict(_)));

    // 驳回同样不产版本、不产事件。
    let (_, memory_b, rev_b, _) = propose_first(&pool, &fx, "rej-b").await;
    let st = port
        .review_memory(&memory_b.to_string(), review_request("reject", rev_b))
        .await
        .unwrap();
    assert_eq!(st.status, "REJECTED");
    assert!(outbox_rows(&pool, fx.project_id).await.is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn withdraw_requires_review_and_ends_retrieval(pool: PgPool) {
    let fx = seed(&pool, "KP3-WD").await;
    let (_, memory_id, rev1, task_key) = propose_first(&pool, &fx, "wd").await;
    let port = KbServiceLive::for_caller(pool.clone(), caller(&fx, &fx.human));
    port.review_memory(&memory_id.to_string(), review_request("approve", rev1))
        .await
        .unwrap();
    let subjects = [SubjectKey::new("requirement", "task", task_key.to_string())];
    assert_eq!(
        current_memories_for_subjects(&pool, fx.project_id, &subjects)
            .await
            .unwrap()
            .len(),
        1
    );

    // 撤回不带新内容。
    let err = memory::propose_change(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        memory_id,
        &fx.human,
        ChangeAction::Withdraw,
        Some(content("x", "y")),
        "过时",
        vec![],
    )
    .await
    .unwrap_err();
    assert!(matches!(err, KbServiceError::Invalid(_)));

    let wd = memory::propose_change(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        memory_id,
        &fx.ai,
        ChangeAction::Withdraw,
        None,
        "方案 X 已被新架构取代，结论过时",
        vec![],
    )
    .await
    .unwrap();
    // 撤回提案待审期间：原 CURRENT 仍生效（未经审核不改变浅层记忆）。
    assert_eq!(
        current_memories_for_subjects(&pool, fx.project_id, &subjects)
            .await
            .unwrap()
            .len(),
        1
    );
    port.review_memory(
        &memory_id.to_string(),
        review_request("approve", wd.revision_id),
    )
    .await
    .unwrap();
    let detail = memory::get_memory(&pool, SYSTEM_TENANT, fx.project_id, memory_id)
        .await
        .unwrap();
    assert_eq!(detail.state, "withdrawn");
    assert_eq!(
        detail.revisions[1].version_status.as_deref(),
        Some("WITHDRAWN")
    );
    assert_eq!(
        detail.revisions[0].version_status.as_deref(),
        Some("SUPERSEDED")
    );
    // 撤回版本自描述：内容与引用承接自被撤回的版本。
    assert_eq!(detail.revisions[1].summary, detail.revisions[0].summary);
    assert_eq!(
        detail.revisions[1].refs.len(),
        detail.revisions[0].refs.len()
    );
    assert!(
        current_memories_for_subjects(&pool, fx.project_id, &subjects)
            .await
            .unwrap()
            .is_empty()
    );
    let events = outbox_rows(&pool, fx.project_id).await;
    assert_eq!(events.last().unwrap().1["approval_state"], "WITHDRAWN");
}

// ── L1 审核门负向 ───────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn l1_gate_ai_principal_cannot_review(pool: PgPool) {
    let fx = seed(&pool, "KP3-AI").await;
    let (_, memory_id, rev1, task_key) = propose_first(&pool, &fx, "ai").await;
    let ai_port = KbServiceLive::for_caller(pool.clone(), caller(&fx, &fx.ai));
    for action in ["approve", "reject", "revise"] {
        let err = ai_port
            .review_memory(&memory_id.to_string(), review_request(action, rev1))
            .await
            .unwrap_err();
        assert!(
            matches!(err, PortError::Forbidden(_)),
            "AI principal must not {action}: {err:?}"
        );
    }
    // 未审核：仍是待审提案，不进入检索，不产事件。
    let detail = memory::get_memory(&pool, SYSTEM_TENANT, fx.project_id, memory_id)
        .await
        .unwrap();
    assert_eq!(detail.state, "proposed");
    assert_eq!(detail.pending_revision_id, Some(rev1));
    assert!(current_memories_for_subjects(
        &pool,
        fx.project_id,
        &[SubjectKey::new("requirement", "task", task_key.to_string())]
    )
    .await
    .unwrap()
    .is_empty());
    assert!(outbox_rows(&pool, fx.project_id).await.is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn l1_gate_stale_cas_and_missing_pending_are_conflicts(pool: PgPool) {
    let fx = seed(&pool, "KP3-CAS").await;
    let (source_id, memory_id, rev1, _) = propose_first(&pool, &fx, "cas").await;
    let port = KbServiceLive::for_caller(pool.clone(), caller(&fx, &fx.human));

    // base 不是待审提案 → 409。
    let err = port
        .review_memory(
            &memory_id.to_string(),
            review_request("approve", Uuid::new_v4()),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, PortError::Conflict(_)), "{err:?}");
    port.review_memory(&memory_id.to_string(), review_request("approve", rev1))
        .await
        .unwrap();
    // 已裁决的提案再审 → 无待审 409。
    let err = port
        .review_memory(&memory_id.to_string(), review_request("approve", rev1))
        .await
        .unwrap_err();
    assert!(matches!(err, PortError::Conflict(_)), "{err:?}");

    // 在途提案期间再提案 → 409（单 pending）。
    memory::propose_change(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        memory_id,
        &fx.human,
        ChangeAction::Update,
        Some(content("v2", "第二版")),
        "更新",
        vec![source_evidence(source_id)],
    )
    .await
    .unwrap();
    let err = memory::propose_change(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        memory_id,
        &fx.human,
        ChangeAction::Update,
        Some(content("v3", "第三版")),
        "并发更新",
        vec![source_evidence(source_id)],
    )
    .await
    .unwrap_err();
    assert!(matches!(err, KbServiceError::Conflict(_)), "{err:?}");

    // 内容无变化的修改提案 → 400（不产生空版本）。
    let (_, memory_b, rev_b, task_b) = propose_first(&pool, &fx, "cas-b").await;
    port.review_memory(&memory_b.to_string(), review_request("approve", rev_b))
        .await
        .unwrap();
    let src_b: Uuid = sqlx::query_scalar(
        "SELECT target_id::uuid FROM kb.memory_ref WHERE revision_id = $1 AND ref_role = 'evidence'",
    )
    .bind(rev_b)
    .fetch_one(&pool)
    .await
    .unwrap();
    let err = memory::propose_change(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        memory_b,
        &fx.human,
        ChangeAction::Update,
        Some(content("放弃方案 X", "因延迟不可控否决方案 X")),
        "no-op",
        vec![source_evidence(src_b), task_subject(task_b)],
    )
    .await
    .unwrap_err();
    assert!(matches!(err, KbServiceError::Invalid(_)), "{err:?}");

    // 未知动作 / 非 UUID base → 400；跨项目记忆 → 404。
    assert!(matches!(
        port.review_memory(&memory_id.to_string(), review_request("merge", rev1))
            .await,
        Err(PortError::Invalid(_))
    ));
    let other = project(&pool, "KP3-CAS-B").await;
    let other_port = KbServiceLive::for_caller(
        pool.clone(),
        KbCaller {
            tenant_id: SYSTEM_TENANT,
            project_id: other,
            actor: fx.human.clone(),
        },
    );
    assert!(matches!(
        other_port
            .review_memory(&memory_id.to_string(), review_request("approve", rev1))
            .await,
        Err(PortError::NotFound(_))
    ));
    assert!(matches!(
        port.review_memory("not-a-uuid", review_request("approve", rev1))
            .await,
        Err(PortError::NotFound(_))
    ));
}

#[sqlx::test(migrations = "../../migrations")]
async fn l1_gate_db_rejects_unreviewed_version_and_mutation(pool: PgPool) {
    let fx = seed(&pool, "KP3-DB").await;
    let (_, memory_id, rev1, _) = propose_first(&pool, &fx, "db").await;

    // 绕过应用层直写 CURRENT（未审核）→ 库层拒绝（L1 审核门 fail-closed）。
    let err = sqlx::query(
        "UPDATE kb.memory_revision SET version_status = 'CURRENT', version_no = 1 WHERE id = $1",
    )
    .bind(rev1)
    .execute(&pool)
    .await
    .unwrap_err();
    assert!(
        err.to_string()
            .contains("illegal version_status transition"),
        "{err}"
    );
    // 直接插入「已批准」修订 → 拒绝（一切内容变化必须先进待审核区）。
    let err = sqlx::query(
        "INSERT INTO kb.memory_revision (tenant_id, memory_id, revision_no, change_action, \
            memory_type, title, summary, change_reason, content_hash, proposed_by, review_status, \
            reviewed_by, reviewed_at, version_no, version_status) \
         VALUES ($1, $2, 9, 'CREATE', 'discussion', 't', 's', 'r', repeat('a', 64), $3, \
                 'APPROVED', $3, now(), 9, 'CURRENT')",
    )
    .bind(SYSTEM_TENANT)
    .bind(memory_id)
    .bind(fx.human.principal_id)
    .execute(&pool)
    .await
    .unwrap_err();
    assert!(err.to_string().contains("L1 review gate"), "{err}");

    let port = KbServiceLive::for_caller(pool.clone(), caller(&fx, &fx.human));
    port.review_memory(&memory_id.to_string(), review_request("approve", rev1))
        .await
        .unwrap();
    // 版本内容不可变、不可物理删除、审核结论不可翻案。
    for (sql, needle) in [
        (
            "UPDATE kb.memory_revision SET summary = 'tampered' WHERE id = $1",
            "content is immutable",
        ),
        (
            "DELETE FROM kb.memory_revision WHERE id = $1",
            "append-only",
        ),
        (
            "UPDATE kb.memory_revision SET review_status = 'REJECTED', version_status = NULL, \
             version_no = NULL WHERE id = $1",
            "review decision is final",
        ),
        (
            "DELETE FROM kb.memory_ref WHERE revision_id = $1",
            "immutable",
        ),
    ] {
        let err = sqlx::query(sql)
            .bind(rev1)
            .execute(&pool)
            .await
            .unwrap_err();
        assert!(err.to_string().contains(needle), "{sql}: {err}");
    }
    // 原始资料不可改写。
    let err = sqlx::query("UPDATE kb.source SET title = 'x'")
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("immutable"), "{err}");
}

#[sqlx::test(migrations = "../../migrations")]
async fn writes_are_closed_on_archived_project(pool: PgPool) {
    let fx = seed(&pool, "KP3-ARC").await;
    let (source_id, memory_id, rev1, _) = propose_first(&pool, &fx, "arc").await;
    sqlx::query("UPDATE core.project SET status = 'closed' WHERE id = $1")
        .bind(fx.project_id)
        .execute(&pool)
        .await
        .unwrap();
    let port = KbServiceLive::for_caller(pool.clone(), caller(&fx, &fx.human));
    assert!(matches!(
        port.review_memory(&memory_id.to_string(), review_request("approve", rev1))
            .await,
        Err(PortError::Conflict(_))
    ));
    assert!(matches!(
        source::register_source(
            &pool,
            SYSTEM_TENANT,
            fx.project_id,
            &fx.human,
            inline_source("归档后资料")
        )
        .await,
        Err(KbServiceError::Conflict(_))
    ));
    assert!(matches!(
        memory::propose_create(
            &pool,
            SYSTEM_TENANT,
            fx.project_id,
            &fx.human,
            content("t", "s"),
            "r",
            vec![source_evidence(source_id)],
        )
        .await,
        Err(KbServiceError::Conflict(_))
    ));
    // 读面不受影响。
    assert_eq!(
        port.list_sources(fx.project_id, None).await.unwrap().len(),
        1
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn proposal_requires_evidence_and_linked_source(pool: PgPool) {
    let fx = seed(&pool, "KP3-REF").await;
    let other = project(&pool, "KP3-REF-B").await;
    // 只有 subject、没有来源依据 → 400（结构化记忆必须保留来源依据）。
    let err = memory::propose_create(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        &fx.human,
        content("t", "s"),
        "r",
        vec![task_subject(Uuid::new_v4())],
    )
    .await
    .unwrap_err();
    assert!(matches!(err, KbServiceError::Invalid(_)));
    // 引用的来源只关联到别的项目 → 400（跨项目来源不可作依据）。
    let foreign = source::register_source(
        &pool,
        SYSTEM_TENANT,
        other,
        &fx.human,
        inline_source("别的项目的资料"),
    )
    .await
    .unwrap();
    let err = memory::propose_create(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        &fx.human,
        content("t", "s"),
        "r",
        vec![source_evidence(foreign.source_id)],
    )
    .await
    .unwrap_err();
    assert!(matches!(err, KbServiceError::Invalid(_)), "{err:?}");
    // 未知记忆类型 → 400。
    let mut c = content("t", "s");
    c.memory_type = "gossip".into();
    let err = memory::propose_create(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        &fx.human,
        c,
        "r",
        vec![source_evidence(foreign.source_id)],
    )
    .await
    .unwrap_err();
    assert!(matches!(err, KbServiceError::Invalid(_)));
    let kb_rows: i64 = sqlx::query_scalar("SELECT count(*) FROM kb.memory")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(kb_rows, 0, "rejected proposals leave no rows");
}

#[sqlx::test(migrations = "../../migrations")]
async fn retrieval_returns_current_versions_only(pool: PgPool) {
    let fx = seed(&pool, "KP3-RET").await;
    let port = KbServiceLive::for_caller(pool.clone(), caller(&fx, &fx.human));
    // A：已批准（CURRENT）；B：待审；C：被驳回。全部挂同一 task 主题。
    let (source_id, memory_a, rev_a, task_key) = propose_first(&pool, &fx, "ret-a").await;
    port.review_memory(&memory_a.to_string(), review_request("approve", rev_a))
        .await
        .unwrap();
    for (tag, decide) in [("ret-b", None), ("ret-c", Some("reject"))] {
        let r = memory::propose_create(
            &pool,
            SYSTEM_TENANT,
            fx.project_id,
            &fx.human,
            content(tag, tag),
            "r",
            vec![source_evidence(source_id), task_subject(task_key)],
        )
        .await
        .unwrap();
        if let Some(action) = decide {
            port.review_memory(
                &r.memory_id.to_string(),
                review_request(action, r.revision_id),
            )
            .await
            .unwrap();
        }
    }
    // A 再提修改（待审）：检索仍返回 A 的 v1，而非待审内容。
    memory::propose_change(
        &pool,
        SYSTEM_TENANT,
        fx.project_id,
        memory_a,
        &fx.human,
        ChangeAction::Update,
        Some(content("A 待审改写", "未经审核的新结论")),
        "改写",
        vec![source_evidence(source_id), task_subject(task_key)],
    )
    .await
    .unwrap();
    let hits = current_memories_for_subjects(
        &pool,
        fx.project_id,
        &[SubjectKey::new("requirement", "task", task_key.to_string())],
    )
    .await
    .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].memory_id, memory_a);
    assert_eq!(hits[0].revision_id, rev_a);
    assert_eq!(hits[0].summary, "因延迟不可控否决方案 X");
    assert_eq!(hits[0].matched_subjects, vec![format!("task:{task_key}")]);
    // 其他项目同 id 主题：不串项目。
    let other = project(&pool, "KP3-RET-B").await;
    assert!(current_memories_for_subjects(
        &pool,
        other,
        &[SubjectKey::new("requirement", "task", task_key.to_string())]
    )
    .await
    .unwrap()
    .is_empty());
    // 列表过滤：pending_review 含「已生效记忆的修改提案」与首提案。
    let pending =
        memory::list_memories(&pool, SYSTEM_TENANT, fx.project_id, Some("pending_review"))
            .await
            .unwrap();
    assert_eq!(pending.len(), 2);
    let active = memory::list_memories(&pool, SYSTEM_TENANT, fx.project_id, Some("active"))
        .await
        .unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].headline.revision_id, rev_a);
    // 审核动作词表与 L1 修订计数口径。
    assert_eq!(ReviewAction::parse("approve"), Some(ReviewAction::Approve));
}
