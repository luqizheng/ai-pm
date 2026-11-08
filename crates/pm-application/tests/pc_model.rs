//! KP-1 · PC-01B 正式模型服务层集成测试（pm_application::pc；真实 DB）。
//!
//! 断言面：
//! 1. 对象草稿 upsert：具名幂等 / 内容寻址去重 / 推号 CAS / identity 占位回填 /
//!    匿名新建 / 未知类型拒绝 / 终态拒绝。
//! 2. 关系草稿：端点类型合法性 / 跨域通道（requirement 必填 Revision、
//!    design 关闭、product_plan 永久排除）/ 存在性探测 / 自引用 / 重复身份。
//! 3. 来源/证据：kb_document 收窄、通道校验、判别器（库层兜底）、主体归属。
//! 4. 发布事务：approved 提案（SQL 夹具）→ 版本 + 成员（端点自动并入）+
//!    snapshot_hash + proposal applied + 生命周期推进 + 审计 + outbox +
//!    账本 completed；同 operation_id 幂等重放。
//! 5. 发布门禁：AI 主体 403 / 非授权人 403 / L3 冻结 403 / CAS 失配 409 +
//!    提案转 failed。

use sqlx::PgPool;
use uuid::Uuid;

use pm_application::actor::AuditActor;
use pm_application::pc::{
    canonical_members_hash, canonical_proposal_hash, create_object_identity,
    create_relationship_draft, publish_version, register_evidence, register_source,
    upsert_object_draft, EvidenceInput, EvidenceSubject, MemberKind, PcEndpoint, PcError,
    PublishVersionInput, SourceInput, UpsertObjectDraft, VersionMemberPin,
};

const LEAD: Uuid = Uuid::from_u128(0x6161_0000_0000_0000_0000_0000_0000_0001);
const AI_BOT: Uuid = Uuid::from_u128(0x6161_0000_0000_0000_0000_0000_0000_0002);
const OUTSIDER: Uuid = Uuid::from_u128(0x6161_0000_0000_0000_0000_0000_0000_0003);

async fn seed_principals(pool: &PgPool) {
    for (id, kind, name) in [
        (LEAD, "human", "pc-lead"),
        (AI_BOT, "dev_agent", "pc-ai"),
        (OUTSIDER, "human", "pc-outsider"),
    ] {
        sqlx::query("INSERT INTO core.principal (id, kind, display_name, is_active) VALUES ($1, $2, $3, true) ON CONFLICT (id) DO NOTHING")
            .bind(id)
            .bind(kind)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
    }
}

async fn seed_project_with_lead(pool: &PgPool, code: &str) -> Uuid {
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('system', $1, $1) RETURNING id",
    )
    .bind(format!("pcm-{code}-{}", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .unwrap();
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(format!("pcm-c-{code}"))
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
    project
}

fn draft(
    object_type: &str,
    object_key: Option<&str>,
    name: &str,
    content: &str,
    analysis_mode: Option<&str>,
) -> UpsertObjectDraft {
    UpsertObjectDraft {
        object_type: object_type.to_string(),
        object_key: object_key.map(str::to_string),
        display_name: name.to_string(),
        content_markdown: content.to_string(),
        change_summary: Some("test draft".into()),
        source_context: Some("service test".into()),
        analysis_mode: analysis_mode.map(str::to_string),
    }
}

async fn seed_requirement_with_revision(pool: &PgPool, project_id: Uuid) -> (Uuid, Uuid) {
    let tenant: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await
        .unwrap();
    let requirement_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement (tenant_id, project_id, code, title, lifecycle_status) \
         VALUES ($1, $2, $3, 'req', 'active') RETURNING id",
    )
    .bind(tenant)
    .bind(project_id)
    .bind(format!("req-{}", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .unwrap();
    let revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.requirement_revision \
         (tenant_id, requirement_id, project_id, version, state, content, content_hash, created_by) \
         VALUES ($1, $2, $3, 1, 'draft', '{\"markdown\":\"m\"}', 'h', $4) RETURNING id",
    )
    .bind(tenant)
    .bind(requirement_id)
    .bind(project_id)
    .bind(LEAD)
    .fetch_one(pool)
    .await
    .unwrap();
    (requirement_id, revision_id)
}

/// 提案成员夹具行（change_kind + 双维度目标）。
type MemberRow = (
    String,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
);

/// SQL 夹具：approved 提案 + 锚定审批（KP-1 不建提案命令——KP-2 范围）。
async fn seed_approved_proposal(
    pool: &PgPool,
    project_id: Uuid,
    expected_version_no: i64,
    member_rows: &[MemberRow],
    title: &str,
) -> Uuid {
    let tenant: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await
        .unwrap();
    let pins: Vec<VersionMemberPin> = member_rows
        .iter()
        .filter_map(|(_, o, or, r, rr)| match (o, or) {
            (Some(oid), Some(rid)) => Some(VersionMemberPin {
                kind: MemberKind::Object,
                identity_id: *oid,
                revision_id: *rid,
            }),
            _ => match (r, rr) {
                (Some(rid), Some(rrid)) => Some(VersionMemberPin {
                    kind: MemberKind::Relationship,
                    identity_id: *rid,
                    revision_id: *rrid,
                }),
                _ => None,
            },
        })
        .collect();
    let hash = canonical_proposal_hash(
        title,
        "publish fixture",
        "manual_input",
        "L1",
        expected_version_no,
        &pins,
    );
    // 先 draft 建提案 + 成员（锚定触发器放行 draft），再置 approved + 审批事实。
    let proposal_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.proposal \
         (tenant_id, project_id, title, rationale, origin_kind, governance_level, \
          expected_version_no, proposal_content_hash, status, created_by) \
         VALUES ($1, $2, $3, 'publish fixture', 'manual_input', 'L1', $4, $5, 'draft', $6) \
         RETURNING id",
    )
    .bind(tenant)
    .bind(project_id)
    .bind(title)
    .bind(expected_version_no)
    .bind(&hash)
    .bind(LEAD)
    .fetch_one(pool)
    .await
    .unwrap();
    for (change_kind, o, or, r, rr) in member_rows {
        sqlx::query(
            "INSERT INTO pc.proposal_member \
             (tenant_id, project_id, proposal_id, change_kind, object_id, object_revision_id, \
              relationship_id, relationship_revision_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(tenant)
        .bind(project_id)
        .bind(proposal_id)
        .bind(change_kind)
        .bind(o)
        .bind(or)
        .bind(r)
        .bind(rr)
        .execute(pool)
        .await
        .unwrap();
    }
    sqlx::query("UPDATE pc.proposal SET status = 'approved', submitted_at = now() WHERE id = $1")
        .bind(proposal_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO pc.approval \
         (tenant_id, project_id, proposal_id, proposal_content_hash, expected_version_no, \
          status, decided_by, decided_at, decision_reason) \
         VALUES ($1, $2, $3, $4, $5, 'approved', $6, now(), 'fixture')",
    )
    .bind(tenant)
    .bind(project_id)
    .bind(proposal_id)
    .bind(&hash)
    .bind(expected_version_no)
    .bind(LEAD)
    .execute(pool)
    .await
    .unwrap();
    proposal_id
}

// ══════════════════════════════════════════════════════════════════════════
// 1. 对象草稿
// ══════════════════════════════════════════════════════════════════════════

#[sqlx::test(migrations = "../../migrations")]
async fn object_draft_upsert_idempotent_and_content_addressed(pool: PgPool) {
    seed_principals(&pool).await;
    let project = seed_project_with_lead(&pool, "pc-o1").await;
    let lead = AuditActor::principal(LEAD);

    // 未知类型拒绝
    let bad = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("no_such_type", Some("k"), "n", "c", None),
    )
    .await;
    assert!(
        matches!(bad, Err(PcError::Invalid(_))),
        "unknown type rejected"
    );

    // 具名草稿：首建 rev1
    let r1 = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("feature", Some("feat-checkout"), "结账", "# 结账 v1", None),
    )
    .await
    .unwrap();
    assert!(r1.revision_created && r1.revision_no == 1);
    let lifecycle: String =
        sqlx::query_scalar("SELECT lifecycle_status FROM pc.cognition_object WHERE id = $1")
            .bind(r1.object_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(lifecycle, "draft", "draft objects never self-activate");

    // analysis_mode：人工草稿 NULL（分析产物才带归因）
    let mode: Option<String> =
        sqlx::query_scalar("SELECT analysis_mode FROM pc.cognition_revision WHERE id = $1")
            .bind(r1.revision_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(mode, None, "manual draft leaves analysis_mode NULL");

    // 内容寻址去重：同内容再 upsert → 复用修订
    let r2 = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft(
            "feature",
            Some("feat-checkout"),
            "结账（改名）",
            "# 结账 v1",
            None,
        ),
    )
    .await
    .unwrap();
    assert!(!r2.revision_created, "same content must reuse the revision");
    assert_eq!(r2.revision_id, r1.revision_id);
    let display: String =
        sqlx::query_scalar("SELECT display_name FROM pc.cognition_object WHERE id = $1")
            .bind(r1.object_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        display, "结账（改名）",
        "display_name follows latest upsert"
    );

    // 内容变化 → 推号 rev2 + 指针前移
    let r3 = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft(
            "feature",
            Some("feat-checkout"),
            "结账",
            "# 结账 v2",
            Some("llm"),
        ),
    )
    .await
    .unwrap();
    assert!(r3.revision_created && r3.revision_no == 2);
    let (pointer, mode): (Uuid, Option<String>) = sqlx::query_as(
        "SELECT current_revision_id, \
                (SELECT analysis_mode FROM pc.cognition_revision WHERE id = current_revision_id) \
           FROM pc.cognition_object WHERE id = $1",
    )
    .bind(r1.object_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pointer, r3.revision_id);
    assert_eq!(
        mode.as_deref(),
        Some("llm"),
        "analysis attribution on analysis revisions"
    );

    // 匿名草稿恒新建
    let a1 = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("module", None, "m", "# m", None),
    )
    .await
    .unwrap();
    let a2 = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("module", None, "m", "# m", None),
    )
    .await
    .unwrap();
    assert_ne!(a1.object_id, a2.object_id);

    // identity 占位：服务建骨架 → upsert 同 key 回填首修订并转 draft
    let skeleton = create_object_identity(
        &pool,
        project,
        &lead,
        "product_vision",
        Some("vision"),
        "产品愿景",
    )
    .await
    .unwrap();
    let fill = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft(
            "product_vision",
            Some("vision"),
            "产品愿景",
            "# Vision",
            None,
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        fill.object_id, skeleton,
        "identity placeholder is reused by key"
    );
    assert!(fill.revision_created);
    let lifecycle: String =
        sqlx::query_scalar("SELECT lifecycle_status FROM pc.cognition_object WHERE id = $1")
            .bind(skeleton)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(lifecycle, "draft");

    // 终态拒绝：SQL 置 deprecated（发布事务语义，库层触发器同样拦截复活）
    sqlx::query(
        "UPDATE pc.cognition_object SET lifecycle_status = 'deprecated', deprecated_at = now() \
          WHERE id = $1",
    )
    .bind(r1.object_id)
    .execute(&pool)
    .await
    .unwrap();
    let dead = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("feature", Some("feat-checkout"), "m", "# m2", None),
    )
    .await;
    assert!(
        matches!(dead, Err(PcError::Conflict(_))),
        "terminal object refuses new revisions"
    );
}

// ══════════════════════════════════════════════════════════════════════════
// 2. 关系草稿
// ══════════════════════════════════════════════════════════════════════════

#[sqlx::test(migrations = "../../migrations")]
async fn relationship_draft_endpoint_rules(pool: PgPool) {
    seed_principals(&pool).await;
    let project = seed_project_with_lead(&pool, "pc-r1").await;
    let lead = AuditActor::principal(LEAD);

    let feature = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("feature", Some("feat-a"), "A", "# A", None),
    )
    .await
    .unwrap();
    let scenario = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("scenario", Some("scn-a"), "S", "# S", None),
    )
    .await
    .unwrap();
    let (_, req_rev) = seed_requirement_with_revision(&pool, project).await;

    // 端点类型非法：contains 的 source 集不含 feature
    let illegal = create_relationship_draft(
        &pool,
        project,
        &lead,
        pm_application::pc::RelationshipDraftInput {
            relationship_type: "contains".into(),
            source: PcEndpoint::Object {
                object_id: feature.object_id,
                revision_id: None,
            },
            target: PcEndpoint::Object {
                object_id: scenario.object_id,
                revision_id: None,
            },
            properties: serde_json::json!({}),
            change_summary: None,
        },
    )
    .await;
    assert!(
        matches!(illegal, Err(PcError::Invalid(_))),
        "endpoint type must be legal"
    );

    // 合法：uses（scenario → feature）
    let (rel_id, rr_id) = create_relationship_draft(
        &pool,
        project,
        &lead,
        pm_application::pc::RelationshipDraftInput {
            relationship_type: "uses".into(),
            source: PcEndpoint::Object {
                object_id: scenario.object_id,
                revision_id: None,
            },
            target: PcEndpoint::Object {
                object_id: feature.object_id,
                revision_id: None,
            },
            properties: serde_json::json!({}),
            change_summary: Some("uses".into()),
        },
    )
    .await
    .unwrap();
    let (dir, src, tgt): (String, Uuid, Uuid) = sqlx::query_as(
        "SELECT directionality, source_object_revision_id, target_object_revision_id \
           FROM pc.relationship_revision WHERE id = $1",
    )
    .bind(rr_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(dir, "directed");
    assert_eq!(
        src, scenario.revision_id,
        "source endpoint pins scenario revision"
    );
    assert_eq!(
        tgt, feature.revision_id,
        "target endpoint pins feature revision"
    );

    // 重复身份 → Conflict（活跃唯一）
    let dup = create_relationship_draft(
        &pool,
        project,
        &lead,
        pm_application::pc::RelationshipDraftInput {
            relationship_type: "uses".into(),
            source: PcEndpoint::Object {
                object_id: scenario.object_id,
                revision_id: None,
            },
            target: PcEndpoint::Object {
                object_id: feature.object_id,
                revision_id: None,
            },
            properties: serde_json::json!({}),
            change_summary: None,
        },
    )
    .await;
    assert!(
        matches!(dup, Err(PcError::Conflict(_))),
        "duplicate active identity"
    );

    // 自引用（affects 允许 decision→decision 端点集，但 allow_self_reference=false）
    let decision = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("product_decision", Some("dec-1"), "D", "# D", None),
    )
    .await
    .unwrap();
    let self_ref = create_relationship_draft(
        &pool,
        project,
        &lead,
        pm_application::pc::RelationshipDraftInput {
            relationship_type: "affects".into(),
            source: PcEndpoint::Object {
                object_id: decision.object_id,
                revision_id: None,
            },
            target: PcEndpoint::Object {
                object_id: decision.object_id,
                revision_id: None,
            },
            properties: serde_json::json!({}),
            change_summary: None,
        },
    )
    .await;
    assert!(
        matches!(self_ref, Err(PcError::Invalid(_))),
        "self reference forbidden"
    );

    // 跨域 requirement：无 Revision 拒绝（Q-PC-03）
    let no_rev = create_relationship_draft(
        &pool,
        project,
        &lead,
        pm_application::pc::RelationshipDraftInput {
            relationship_type: "specified_by".into(),
            source: PcEndpoint::Object {
                object_id: feature.object_id,
                revision_id: None,
            },
            target: PcEndpoint::External {
                domain: "requirement".into(),
                target_type: "requirement_revision".into(),
                target_id: req_rev.to_string(),
                revision_id: None,
            },
            properties: serde_json::json!({}),
            change_summary: None,
        },
    )
    .await;
    assert!(
        matches!(no_rev, Err(PcError::Invalid(_))),
        "requirement endpoint requires revision"
    );

    // 合法（带 Revision）+ 存在性通过
    let with_rev = create_relationship_draft(
        &pool,
        project,
        &lead,
        pm_application::pc::RelationshipDraftInput {
            relationship_type: "specified_by".into(),
            source: PcEndpoint::Object {
                object_id: feature.object_id,
                revision_id: None,
            },
            target: PcEndpoint::External {
                domain: "requirement".into(),
                target_type: "requirement_revision".into(),
                target_id: req_rev.to_string(),
                revision_id: Some(req_rev.to_string()),
            },
            properties: serde_json::json!({}),
            change_summary: None,
        },
    )
    .await;
    assert!(
        with_rev.is_ok(),
        "legal cross-domain endpoint with revision"
    );

    // 通道关闭：design（NotYetOpen）
    let design = create_relationship_draft(
        &pool,
        project,
        &lead,
        pm_application::pc::RelationshipDraftInput {
            relationship_type: "specified_by".into(),
            source: PcEndpoint::Object {
                object_id: feature.object_id,
                revision_id: None,
            },
            target: PcEndpoint::External {
                domain: "design".into(),
                target_type: "design".into(),
                target_id: "x".into(),
                revision_id: None,
            },
            properties: serde_json::json!({}),
            change_summary: None,
        },
    )
    .await;
    assert!(
        matches!(design, Err(PcError::Invalid(_))),
        "design channel closed"
    );

    // product_plan（PermanentlyExcluded）
    let plan = create_relationship_draft(
        &pool,
        project,
        &lead,
        pm_application::pc::RelationshipDraftInput {
            relationship_type: "specified_by".into(),
            source: PcEndpoint::Object {
                object_id: feature.object_id,
                revision_id: None,
            },
            target: PcEndpoint::External {
                domain: "product_plan".into(),
                target_type: "plan".into(),
                target_id: "x".into(),
                revision_id: None,
            },
            properties: serde_json::json!({}),
            change_summary: None,
        },
    )
    .await;
    assert!(
        matches!(plan, Err(PcError::Invalid(_))),
        "product_plan excluded"
    );

    let _ = rel_id;
}

// ══════════════════════════════════════════════════════════════════════════
// 3. 来源/证据
// ══════════════════════════════════════════════════════════════════════════

#[sqlx::test(migrations = "../../migrations")]
async fn source_and_evidence_registration(pool: PgPool) {
    seed_principals(&pool).await;
    let project = seed_project_with_lead(&pool, "pc-s1").await;
    let lead = AuditActor::principal(LEAD);
    let obj = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("feature", Some("f"), "F", "# F", None),
    )
    .await
    .unwrap();

    // kb_document 无 Revision → 库层收窄 CHECK 拒绝
    let kb_rev = Uuid::new_v4();
    let _ = kb_rev;
    let narrowed = register_source(
        &pool,
        project,
        &lead,
        SourceInput {
            source_kind: "kb_document".into(),
            target_domain: "knowledge_base".into(),
            target_type: "document".into(),
            target_id: Uuid::new_v4().to_string(),
            target_revision_id: None,
            revision_missing_reason: Some("n/a".into()),
            section_anchor: None,
            start_line: None,
            locator: None,
            description: None,
            source_content_hash: None,
        },
    )
    .await;
    assert!(
        narrowed.is_err(),
        "kb_document must pin a kb revision (C-4)"
    );

    // kb_document 合法（存在的 kb 修订）
    let tenant: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project)
        .fetch_one(&pool)
        .await
        .unwrap();
    let kb_doc: Uuid = sqlx::query_scalar(
        "INSERT INTO kb.document (tenant_id, project_id, doc_key, doc_type, canonical_path) \
         VALUES ($1, $2, 'kdoc', 'knowledge', 'repos/k.md') RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    let kb_doc_rev: Uuid = sqlx::query_scalar(
        "INSERT INTO kb.document_revision \
         (tenant_id, document_id, revision_no, content_hash, status, content, created_by) \
         VALUES ($1, $2, 1, 'kbh', 'draft', 'kb body', $3) RETURNING id",
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
            target_revision_id: Some(kb_doc_rev.to_string()),
            revision_missing_reason: None,
            section_anchor: None,
            start_line: None,
            locator: None,
            description: None,
            source_content_hash: Some("kbh".into()),
        },
    )
    .await
    .unwrap();

    // design 域来源 → 通道关闭
    let closed = register_source(
        &pool,
        project,
        &lead,
        SourceInput {
            source_kind: "external_reference".into(),
            target_domain: "design".into(),
            target_type: "design".into(),
            target_id: "x".into(),
            target_revision_id: None,
            revision_missing_reason: Some("r".into()),
            section_anchor: None,
            start_line: None,
            locator: None,
            description: None,
            source_content_hash: None,
        },
    )
    .await;
    assert!(
        matches!(closed, Err(PcError::Invalid(_))),
        "design source channel closed"
    );

    // 证据：document_section（主体 = 对象修订）
    let ev = register_evidence(
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
            target_revision_id: Some(kb_doc_rev.to_string()),
            file_path: None,
            symbol: None,
            code_range: None,
            commit_sha: None,
            section_anchor: Some("snapshot".into()),
            start_line: None,
            end_line: None,
            excerpt: None,
            description: Some("digest".into()),
            evidence_hash: "kbh".into(),
            confidence: Some("high".into()),
        },
    )
    .await
    .unwrap();
    let pinned: Uuid =
        sqlx::query_scalar("SELECT object_revision_id FROM pc.evidence WHERE id = $1")
            .bind(ev)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(pinned, obj.revision_id);

    // 判别器：commit 类证据缺 commit_sha → 库层 CHECK
    let bad_code = register_evidence(
        &pool,
        project,
        &lead,
        EvidenceInput {
            source_id: source,
            subject: EvidenceSubject::ObjectRevision(obj.revision_id),
            evidence_type: "commit".into(),
            target_domain: None,
            target_type: None,
            target_id: None,
            target_revision_id: None,
            file_path: Some("a.rs".into()),
            symbol: None,
            code_range: None,
            commit_sha: None,
            section_anchor: None,
            start_line: None,
            end_line: None,
            excerpt: None,
            description: None,
            evidence_hash: "h".into(),
            confidence: None,
        },
    )
    .await;
    assert!(
        bad_code.is_err(),
        "commit evidence requires file_path + commit_sha"
    );

    // 主体不存在 → NotFound
    let ghost = register_evidence(
        &pool,
        project,
        &lead,
        EvidenceInput {
            source_id: source,
            subject: EvidenceSubject::ObjectRevision(Uuid::new_v4()),
            evidence_type: "document_section".into(),
            target_domain: Some("knowledge_base".into()),
            target_type: Some("document".into()),
            target_id: Some(kb_doc.to_string()),
            target_revision_id: Some(kb_doc_rev.to_string()),
            file_path: None,
            symbol: None,
            code_range: None,
            commit_sha: None,
            section_anchor: None,
            start_line: None,
            end_line: None,
            excerpt: None,
            description: None,
            evidence_hash: "h".into(),
            confidence: None,
        },
    )
    .await;
    assert!(matches!(ghost, Err(PcError::NotFound(_))));
}

// ══════════════════════════════════════════════════════════════════════════
// 4. 发布事务（happy path + 幂等）
// ══════════════════════════════════════════════════════════════════════════

#[sqlx::test(migrations = "../../migrations")]
async fn publish_happy_path_idempotent_and_lifecycle(pool: PgPool) {
    seed_principals(&pool).await;
    let project = seed_project_with_lead(&pool, "pc-p1").await;
    let lead = AuditActor::principal(LEAD);

    // 草稿面：A（feature，2 修订）、B（scenario，1 修订）
    let _a1 = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("feature", Some("fa"), "A", "# A1", None),
    )
    .await
    .unwrap();
    let a2 = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("feature", Some("fa"), "A", "# A2", None),
    )
    .await
    .unwrap();
    let b = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("scenario", Some("sb"), "B", "# B1", None),
    )
    .await
    .unwrap();
    assert_eq!(a2.revision_no, 2);

    // 关系草稿：uses（B@1 → A@2）
    let (rel_id, rr_id) = create_relationship_draft(
        &pool,
        project,
        &lead,
        pm_application::pc::RelationshipDraftInput {
            relationship_type: "uses".into(),
            source: PcEndpoint::Object {
                object_id: b.object_id,
                revision_id: None,
            },
            target: PcEndpoint::Object {
                object_id: a2.object_id,
                revision_id: Some(a2.revision_id),
            },
            properties: serde_json::json!({}),
            change_summary: None,
        },
    )
    .await
    .unwrap();

    // 提案夹具：update_object(A@2) + create_relationship(R@RR) + 端点并入(B@1)
    let member_rows = vec![
        (
            "update_object".to_string(),
            Some(a2.object_id),
            Some(a2.revision_id),
            None,
            None,
        ),
        (
            "create_relationship".to_string(),
            None,
            None,
            Some(rel_id),
            Some(rr_id),
        ),
        (
            "update_object".to_string(),
            Some(b.object_id),
            Some(b.revision_id),
            None,
            None,
        ),
    ];
    let proposal = seed_approved_proposal(&pool, project, 0, &member_rows, "首版本").await;

    let receipt = publish_version(
        &pool,
        project,
        &lead,
        PublishVersionInput {
            proposal_id: proposal,
            operation_id: "op-publish-1".into(),
        },
    )
    .await
    .unwrap();
    assert!(!receipt.replayed);
    assert_eq!(receipt.version_no, 1);
    let _ = receipt.member_count;

    // snapshot_hash = 成员集规范哈希（A@2, B@1, R@RR）
    let expected_hash = canonical_members_hash(&[
        VersionMemberPin {
            kind: MemberKind::Object,
            identity_id: a2.object_id,
            revision_id: a2.revision_id,
        },
        VersionMemberPin {
            kind: MemberKind::Object,
            identity_id: b.object_id,
            revision_id: b.revision_id,
        },
        VersionMemberPin {
            kind: MemberKind::Relationship,
            identity_id: rel_id,
            revision_id: rr_id,
        },
    ]);
    assert_eq!(receipt.snapshot_hash, expected_hash);

    // 版本成员三行
    let member_n: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pc.version_member WHERE version_id = $1")
            .bind(receipt.version_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(member_n, 3);

    // 提案 applied / 审批事实保留 / 账本 completed
    let pstatus: String = sqlx::query_scalar("SELECT status FROM pc.proposal WHERE id = $1")
        .bind(proposal)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(pstatus, "applied");
    let opstatus: String = sqlx::query_scalar(
        "SELECT status FROM pc.write_operation \
          WHERE project_id = $1 AND operation_id = 'op-publish-1'",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(opstatus, "completed");

    // 生命周期推进：A/B/R 全 active
    let active_n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pc.cognition_object \
          WHERE project_id = $1 AND lifecycle_status = 'active'",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(active_n, 2, "formalized objects are active");
    let rel_status: String =
        sqlx::query_scalar("SELECT lifecycle_status FROM pc.relationship WHERE id = $1")
            .bind(rel_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rel_status, "active");

    // 审计 + outbox 事件
    let audit_n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
          WHERE project_id = $1 AND action = 'pc_version.publish'",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit_n, 1);
    let event: (String, serde_json::Value) = sqlx::query_as(
        "SELECT event_type, payload FROM integration.outbox_event \
          WHERE project_id = $1 AND event_type = 'pc.cognition.version_published'",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event.0, "pc.cognition.version_published");
    assert_eq!(event.1["version_no"], 1);
    assert_eq!(event.1["snapshot_hash"], expected_hash);
    assert!(event.1["version_ref"]["target_id"].is_string());
    assert!(event.1["proposal_ref"]["target_id"].is_string());

    // 幂等重放：同 operation_id 再发布 → replayed，不产生第二版本
    let replay = publish_version(
        &pool,
        project,
        &lead,
        PublishVersionInput {
            proposal_id: proposal,
            operation_id: "op-publish-1".into(),
        },
    )
    .await
    .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.version_id, receipt.version_id);
    let version_n: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pc.cognition_version WHERE project_id = $1")
            .bind(project)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(version_n, 1, "no duplicate version on replay");
}

/// 第二轮发布（KP-2 修复回归）：提案目标替换前版本携带的同身份旧 Revision——
/// 完整快照每身份恰一个 Revision（T-11 语义）；历史版本成员不受影响。
#[sqlx::test(migrations = "../../migrations")]
async fn publish_next_version_replaces_carried_revision(pool: PgPool) {
    seed_principals(&pool).await;
    let project = seed_project_with_lead(&pool, "pc-p2").await;
    let lead = AuditActor::principal(LEAD);

    let a1 = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("feature", Some("ra"), "A", "# A1", None),
    )
    .await
    .unwrap();
    let c1 = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("scenario", Some("rc"), "C", "# C1", None),
    )
    .await
    .unwrap();
    let p1 = seed_approved_proposal(
        &pool,
        project,
        0,
        &[
            (
                "update_object".to_string(),
                Some(a1.object_id),
                Some(a1.revision_id),
                None,
                None,
            ),
            (
                "update_object".to_string(),
                Some(c1.object_id),
                Some(c1.revision_id),
                None,
                None,
            ),
        ],
        "v1",
    )
    .await;
    let v1 = publish_version(
        &pool,
        project,
        &lead,
        PublishVersionInput {
            proposal_id: p1,
            operation_id: "op-round-1".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(v1.version_no, 1);

    let a2 = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("feature", Some("ra"), "A", "# A2", None),
    )
    .await
    .unwrap();
    assert_eq!(a2.object_id, a1.object_id);
    assert_eq!(a2.revision_no, 2);
    let p2 = seed_approved_proposal(
        &pool,
        project,
        1,
        &[(
            "update_object".to_string(),
            Some(a2.object_id),
            Some(a2.revision_id),
            None,
            None,
        )],
        "v2",
    )
    .await;
    let v2 = publish_version(
        &pool,
        project,
        &lead,
        PublishVersionInput {
            proposal_id: p2,
            operation_id: "op-round-2".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(v2.version_no, 2);

    let members_of = |version_id: Uuid| {
        let pool = pool.clone();
        async move {
            sqlx::query_as::<_, (Uuid, Uuid)>(
                "SELECT object_id, object_revision_id FROM pc.version_member \
                  WHERE version_id = $1 AND object_id IS NOT NULL",
            )
            .bind(version_id)
            .fetch_all(&pool)
            .await
            .unwrap()
        }
    };
    let v2_members = members_of(v2.version_id).await;
    assert_eq!(
        v2_members.len(),
        2,
        "one revision per identity: {v2_members:?}"
    );
    assert!(v2_members.contains(&(a2.object_id, a2.revision_id)));
    assert!(v2_members.contains(&(c1.object_id, c1.revision_id)));
    assert!(
        !v2_members.contains(&(a1.object_id, a1.revision_id)),
        "carried A@1 must be replaced by the proposal target A@2"
    );
    let expected_hash = canonical_members_hash(&[
        VersionMemberPin {
            kind: MemberKind::Object,
            identity_id: a2.object_id,
            revision_id: a2.revision_id,
        },
        VersionMemberPin {
            kind: MemberKind::Object,
            identity_id: c1.object_id,
            revision_id: c1.revision_id,
        },
    ]);
    assert_eq!(v2.snapshot_hash, expected_hash);

    // 历史版本不受影响：v1 仍为 {A@1, C@1}
    let v1_members = members_of(v1.version_id).await;
    assert_eq!(v1_members.len(), 2);
    assert!(v1_members.contains(&(a1.object_id, a1.revision_id)));
}

// ══════════════════════════════════════════════════════════════════════════
// 5. 发布门禁（fail-closed）
// ══════════════════════════════════════════════════════════════════════════

#[sqlx::test(migrations = "../../migrations")]
async fn publish_gates_fail_closed(pool: PgPool) {
    seed_principals(&pool).await;
    let project = seed_project_with_lead(&pool, "pc-g1").await;
    let lead = AuditActor::principal(LEAD);
    let ai = AuditActor::principal(AI_BOT);
    let outsider = AuditActor::principal(OUTSIDER);

    let a = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("feature", Some("g-a"), "A", "# A", None),
    )
    .await
    .unwrap();
    let proposal = seed_approved_proposal(
        &pool,
        project,
        0,
        &[(
            "update_object".to_string(),
            Some(a.object_id),
            Some(a.revision_id),
            None,
            None,
        )],
        "gates",
    )
    .await;

    // AI 主体 → 403
    let ai_attempt = publish_version(
        &pool,
        project,
        &ai,
        PublishVersionInput {
            proposal_id: proposal,
            operation_id: "op-ai".into(),
        },
    )
    .await;
    assert!(
        matches!(ai_attempt, Err(PcError::Forbidden(_))),
        "AI principal must be rejected"
    );

    // 非成员真人 → 403（A3 闸）
    let outsider_attempt = publish_version(
        &pool,
        project,
        &outsider,
        PublishVersionInput {
            proposal_id: proposal,
            operation_id: "op-out".into(),
        },
    )
    .await;
    assert!(matches!(outsider_attempt, Err(PcError::Forbidden(_))));

    // L3 冻结：未解决冲突圈定目标对象
    let tenant: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project)
        .fetch_one(&pool)
        .await
        .unwrap();
    let conflict: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.conflict \
         (tenant_id, project_id, conflict_type, severity, description, status, created_by) \
         VALUES ($1, $2, 'scope_conflict', 'major', 'frozen', 'detected', $3) RETURNING id",
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
    .bind(a.object_id)
    .execute(&pool)
    .await
    .unwrap();
    let frozen = publish_version(
        &pool,
        project,
        &lead,
        PublishVersionInput {
            proposal_id: proposal,
            operation_id: "op-frozen".into(),
        },
    )
    .await;
    assert!(
        matches!(frozen, Err(PcError::Forbidden(_))),
        "L3 freeze blocks publish"
    );

    // 解冻后：CAS 失配（提案基线 0 vs 现存版本 1）→ 409 + 提案转 failed
    sqlx::query("UPDATE pc.conflict SET status = 'resolved', resolved_by = $2, resolved_at = now(), resolution_note = 'ok' WHERE id = $1")
        .bind(conflict)
        .bind(LEAD)
        .execute(&pool)
        .await
        .unwrap();
    // 先发布一个别的提案造出版本 1
    let a2 = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("feature", Some("g-a"), "A", "# A2", None),
    )
    .await
    .unwrap();
    let proposal2 = seed_approved_proposal(
        &pool,
        project,
        0,
        &[(
            "update_object".to_string(),
            Some(a2.object_id),
            Some(a2.revision_id),
            None,
            None,
        )],
        "baseline",
    )
    .await;
    publish_version(
        &pool,
        project,
        &lead,
        PublishVersionInput {
            proposal_id: proposal2,
            operation_id: "op-base".into(),
        },
    )
    .await
    .unwrap();
    // 原 proposal（基线 0）现在 CAS 失配 → 失败路径
    let cas = publish_version(
        &pool,
        project,
        &lead,
        PublishVersionInput {
            proposal_id: proposal,
            operation_id: "op-cas".into(),
        },
    )
    .await;
    assert!(
        matches!(cas, Err(PcError::Conflict(_))),
        "stale baseline must fail CAS"
    );
    let pstatus: String = sqlx::query_scalar("SELECT status FROM pc.proposal WHERE id = $1")
        .bind(proposal)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(pstatus, "failed", "CAS mismatch moves proposal to failed");
    let opstatus: String = sqlx::query_scalar(
        "SELECT status FROM pc.write_operation \
          WHERE project_id = $1 AND operation_id = 'op-cas'",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(opstatus, "failed", "ledger row converges to failed");
}
