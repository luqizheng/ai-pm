//! P3-11 · projc.* 独立域数据面集成测试（pm_application::projc；真实 DB）。
//!
//! 断言面：
//! 1. 对象草稿 upsert：具名幂等 / 内容寻址去重 / 推号 / 匿名新建 / 未知类型拒绝。
//! 2. 生命周期与不可变守卫：终态拒绝追加（服务层 Conflict）、superseded 无后继
//!    （触发器）、document_revision 不可变（触发器）、identity 才可删。
//! 3. 来源/证据：DomainRef 契约（kb_document 钉修订、通道关闭拒绝、SEC-P2-2
//!    探测）、判别器（库层兜底）、主体归属、身份去重。
//!
//! 证据口径（P3-11 差异报告 §R-01/R-02/R-07）：本文件通过 = 迁移 0025 可应用 +
//! projc 数据面写入者可用 + projc 与 pc 触发器/约束独立生效。

use sqlx::PgPool;
use uuid::Uuid;

use pm_application::actor::AuditActor;
use pm_application::projc::{
    register_evidence, register_source, upsert_object_draft, EvidenceInput, EvidenceSubject,
    ProjcError, SourceInput, UpsertObjectDraft,
};

const LEAD: Uuid = Uuid::from_u128(0x7072_0000_0000_0000_0000_0000_0000_0001);

async fn seed_principals(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES ($1, 'human', 'projc-lead', true) ON CONFLICT (id) DO NOTHING",
    )
    .bind(LEAD)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_project_with_lead(pool: &PgPool, code: &str) -> Uuid {
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('system', $1, $1) RETURNING id",
    )
    .bind(format!("pjm-{code}-{}", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .unwrap();
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(format!("pjm-c-{code}"))
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

#[sqlx::test(migrations = "../../migrations")]
async fn object_draft_upsert_idempotent_and_content_addressed(pool: PgPool) {
    seed_principals(&pool).await;
    let project = seed_project_with_lead(&pool, "pj-o1").await;
    let lead = AuditActor::principal(LEAD);

    // 未知类型拒绝（独立字典；pc 类型不可用 = R-02 权威边界）
    let bad = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("product_profile", Some("k"), "n", "c", None),
    )
    .await;
    assert!(matches!(bad, Err(ProjcError::Invalid(_))), "got {bad:?}");

    // 具名新建：identity → draft，首修订
    let r1 = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft(
            "repository",
            Some("repo:abc:snapshot"),
            "仓库快照 @ abc",
            "- HEAD：`abc1234`",
            Some("derived"),
        ),
    )
    .await
    .unwrap();
    assert_eq!(r1.revision_no, 1);
    assert!(r1.revision_created);

    // 内容未变：去重复用，不推新修订
    let r1_again = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft(
            "repository",
            Some("repo:abc:snapshot"),
            "仓库快照 @ abc（改名）",
            "- HEAD：`abc1234`",
            Some("derived"),
        ),
    )
    .await
    .unwrap();
    assert!(!r1_again.revision_created);
    assert_eq!(r1_again.revision_id, r1.revision_id);

    // 内容变化：推号 revision 2
    let r2 = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft(
            "repository",
            Some("repo:abc:snapshot"),
            "仓库快照 @ abc",
            "- HEAD：`def5678`",
            Some("derived"),
        ),
    )
    .await
    .unwrap();
    assert!(r2.revision_created);
    assert_eq!(r2.revision_no, 2);
    assert_ne!(r2.revision_id, r1.revision_id);

    // 匿名草稿：每次新对象
    let a1 = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("tech_stack", None, "技术栈", "rust", None),
    )
    .await
    .unwrap();
    let a2 = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft("tech_stack", None, "技术栈", "rust", None),
    )
    .await
    .unwrap();
    assert_ne!(a1.object_id, a2.object_id);

    // 正文字段落库（document/document_revision 链；经 cognition_revision 关联）
    let row: (i32, String) = sqlx::query_as(
        "SELECT dr.revision_no, d.doc_key FROM projc.cognition_revision cr \
           JOIN projc.document_revision dr ON dr.id = cr.document_revision_id \
           JOIN projc.document d ON d.id = dr.document_id \
          WHERE cr.tenant_id = (SELECT tenant_id FROM core.project WHERE id = $1) \
            AND cr.project_id = $1 AND cr.id = $2",
    )
    .bind(project)
    .bind(r2.revision_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, 2);
    assert!(row.1.starts_with("projc-object-"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn lifecycle_and_immutability_guards(pool: PgPool) {
    seed_principals(&pool).await;
    let project = seed_project_with_lead(&pool, "pj-g1").await;
    let lead = AuditActor::principal(LEAD);

    let r1 = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft(
            "repository_module",
            Some("repo:abc:module:src"),
            "src",
            "src/",
            None,
        ),
    )
    .await
    .unwrap();

    // superseded 无后继 → 触发器拒绝
    let no_successor = sqlx::query(
        "UPDATE projc.cognition_object SET lifecycle_status = 'superseded' WHERE id = $1",
    )
    .bind(r1.object_id)
    .execute(&pool)
    .await;
    assert!(no_successor.is_err(), "superseded requires successor");

    // 带后继的 deprecated：终态拒绝追加修订（服务层 Conflict）
    let other = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft(
            "repository_module",
            Some("repo:abc:module:src-v2"),
            "src v2",
            "src v2/",
            None,
        ),
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE projc.cognition_object \
            SET lifecycle_status = 'deprecated', superseded_by_id = $2, deprecated_at = now() \
          WHERE id = $1",
    )
    .bind(r1.object_id)
    .bind(other.object_id)
    .execute(&pool)
    .await
    .unwrap();
    let revive = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft(
            "repository_module",
            Some("repo:abc:module:src"),
            "src",
            "changed",
            None,
        ),
    )
    .await;
    assert!(
        matches!(revive, Err(ProjcError::Conflict(_))),
        "got {revive:?}"
    );

    // document_revision 不可变（触发器）
    let mutation = sqlx::query(
        "UPDATE projc.document_revision SET content = 'tampered' WHERE document_id = \
         (SELECT document_id FROM projc.cognition_revision WHERE id = $1)",
    )
    .bind(r1.revision_id)
    .execute(&pool)
    .await;
    assert!(mutation.is_err(), "document_revision must be immutable");

    // 非 identity 对象禁止删除；identity 占位可删
    let del = sqlx::query("DELETE FROM projc.cognition_object WHERE id = $1")
        .bind(r1.object_id)
        .execute(&pool)
        .await;
    assert!(del.is_err(), "non-identity delete forbidden");
}

#[sqlx::test(migrations = "../../migrations")]
async fn source_and_evidence_registration(pool: PgPool) {
    seed_principals(&pool).await;
    let project = seed_project_with_lead(&pool, "pj-s1").await;
    let lead = AuditActor::principal(LEAD);

    let rev = upsert_object_draft(
        &pool,
        project,
        &lead,
        draft(
            "tech_stack",
            Some("repo:abc:tech_stack"),
            "技术栈（rust）",
            "rust",
            Some("derived"),
        ),
    )
    .await
    .unwrap();

    // 通道永久排除域拒绝（R-07 契约：target_domain 必须已登记且通道开放）
    let excluded = register_source(
        &pool,
        project,
        &lead,
        SourceInput {
            source_kind: "external_reference".into(),
            target_domain: "product_reality".into(),
            target_type: "anything".into(),
            target_id: "x".into(),
            target_revision_id: None,
            revision_missing_reason: Some("external".into()),
            section_anchor: None,
            start_line: None,
            locator: None,
            description: None,
            source_content_hash: None,
        },
    )
    .await;
    assert!(
        matches!(excluded, Err(ProjcError::Invalid(_))),
        "got {excluded:?}"
    );

    // SEC-P2-2：product_cognition 目标必须真实存在于本项目（探测 pc.cognition_object）
    let missing_pc = register_source(
        &pool,
        project,
        &lead,
        SourceInput {
            source_kind: "product_cognition".into(),
            target_domain: "pc".into(),
            target_type: "cognition_object".into(),
            target_id: Uuid::new_v4().to_string(),
            target_revision_id: None,
            revision_missing_reason: Some("draft".into()),
            section_anchor: None,
            start_line: None,
            locator: None,
            description: None,
            source_content_hash: None,
        },
    )
    .await;
    assert!(
        matches!(missing_pc, Err(ProjcError::NotFound(_))),
        "got {missing_pc:?}"
    );

    let pc_object: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.cognition_object \
         (tenant_id, project_id, object_type, object_key, display_name, lifecycle_status, \
          created_by, updated_by) \
         SELECT tenant_id, id, 'business_capability', 'cap:probe', '探测能力', 'draft', \
                $2, $2 FROM core.project WHERE id = $1 RETURNING id",
    )
    .bind(project)
    .bind(LEAD)
    .fetch_one(&pool)
    .await
    .unwrap();
    let ok_pc = register_source(
        &pool,
        project,
        &lead,
        SourceInput {
            source_kind: "product_cognition".into(),
            target_domain: "pc".into(),
            target_type: "cognition_object".into(),
            target_id: pc_object.to_string(),
            target_revision_id: None,
            revision_missing_reason: Some("draft".into()),
            section_anchor: None,
            start_line: None,
            locator: None,
            description: None,
            source_content_hash: None,
        },
    )
    .await
    .unwrap();

    // kb_document：目标探测在前（SEC-P2-2），pin 校验在后——先建真实 KB 文档，
    // 缺修订钉扎时才走 Invalid 分支（pc::register_source 同序）。
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
    let kb_no_pin = register_source(
        &pool,
        project,
        &lead,
        SourceInput {
            source_kind: "kb_document".into(),
            target_domain: "knowledge_base".into(),
            target_type: "document".into(),
            target_id: kb_doc.to_string(),
            target_revision_id: None,
            revision_missing_reason: None,
            section_anchor: None,
            start_line: None,
            locator: None,
            description: None,
            source_content_hash: None,
        },
    )
    .await;
    assert!(
        matches!(kb_no_pin, Err(ProjcError::Invalid(_))),
        "got {kb_no_pin:?}"
    );

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
    let kb_ok = register_source(
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

    // 证据登记：主体归属 + code 判别器（库层 CHECK 兜底）+ 身份去重
    let ev = register_evidence(
        &pool,
        project,
        &lead,
        EvidenceInput {
            source_id: kb_ok,
            subject: EvidenceSubject::ObjectRevision(rev.revision_id),
            evidence_type: "document_section".into(),
            target_domain: Some("knowledge_base".into()),
            target_type: Some("document".into()),
            target_id: Some(kb_doc.to_string()),
            target_revision_id: Some(kb_rev.to_string()),
            file_path: None,
            symbol: None,
            code_range: None,
            commit_sha: None,
            section_anchor: Some("§1".into()),
            start_line: None,
            end_line: None,
            excerpt: None,
            description: Some("KB 技术规范节".into()),
            evidence_hash: "hash-e1".into(),
            confidence: Some("high".into()),
        },
    )
    .await
    .unwrap();

    let dup = register_evidence(
        &pool,
        project,
        &lead,
        EvidenceInput {
            source_id: kb_ok,
            subject: EvidenceSubject::ObjectRevision(rev.revision_id),
            evidence_type: "document_section".into(),
            target_domain: Some("knowledge_base".into()),
            target_type: Some("document".into()),
            target_id: Some(kb_doc.to_string()),
            target_revision_id: Some(kb_rev.to_string()),
            file_path: None,
            symbol: None,
            code_range: None,
            commit_sha: None,
            section_anchor: Some("§1".into()),
            start_line: None,
            end_line: None,
            excerpt: None,
            description: Some("KB 技术规范节".into()),
            evidence_hash: "hash-e1".into(),
            confidence: None,
        },
    )
    .await;
    assert!(
        matches!(dup, Err(ProjcError::Db(_))),
        "identity dedup, got {dup:?}"
    );

    // code 证据缺 file_path/commit_sha → 库层判别器 CHECK 拒绝
    let bad_code = register_evidence(
        &pool,
        project,
        &lead,
        EvidenceInput {
            source_id: ok_pc,
            subject: EvidenceSubject::ObjectRevision(rev.revision_id),
            evidence_type: "code".into(),
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
            description: None,
            evidence_hash: "hash-e2".into(),
            confidence: None,
        },
    )
    .await;
    assert!(
        matches!(bad_code, Err(ProjcError::Db(_))),
        "locator CHECK, got {bad_code:?}"
    );

    // 主体修订不存在 → NotFound
    let ghost = register_evidence(
        &pool,
        project,
        &lead,
        EvidenceInput {
            source_id: ok_pc,
            subject: EvidenceSubject::ObjectRevision(Uuid::new_v4()),
            evidence_type: "document_section".into(),
            target_domain: Some("knowledge_base".into()),
            target_type: Some("document".into()),
            target_id: Some(kb_doc.to_string()),
            target_revision_id: Some(kb_rev.to_string()),
            file_path: None,
            symbol: None,
            code_range: None,
            commit_sha: None,
            section_anchor: None,
            start_line: None,
            end_line: None,
            excerpt: Some("x".into()),
            description: None,
            evidence_hash: "hash-e3".into(),
            confidence: None,
        },
    )
    .await;
    assert!(
        matches!(ghost, Err(ProjcError::NotFound(_))),
        "got {ghost:?}"
    );

    // 证据落库与主体归属终验
    let owner: Uuid = sqlx::query_scalar(
        "SELECT object_revision_id FROM projc.evidence WHERE id = $1 \
           AND project_id = $2",
    )
    .bind(ev)
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(owner, rev.revision_id);
}
