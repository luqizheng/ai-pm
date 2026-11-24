//! P3-D04-02 Step5 Batch4a · GAP-30 起草段幂等用例 + GAP-27 发布段严格指纹用例
//! （真实 DB；sqlx::test 全新临时库 0001~0032 全序应用）。
//!
//! 断言面（D6.6 §6 场景矩阵 / §五 T4 / Q-01 方案 A）：
//! 1. 场景 1/9：同 op_id（`{domain}-analyze:{task_id}`）+ 同意图 → 起草段账本
//!    completed 短路，幂等返回原提案（不重复起草）。
//! 2. 场景 2：同 op_id + 异意图（title 变化）→ 指纹失配 409，不得误判重放，
//!    且不产生第二提案。
//! 3. 场景 8：账本行承载（command/status/request_fingerprint）断言。
//! 4. 发布段严格指纹（GAP-27）：completed 行指纹与当前提案内容锚比对——
//!    同 op 同 fp → replayed；fp 失配（合成）→ 409 mismatch。

use sqlx::{PgPool, Row};
use uuid::Uuid;

use pm_application::actor::AuditActor;
use pm_application::pc::{
    propose_from_analysis, publish_version, register_evidence, register_source,
    AnalysisProposalInput, AnalysisProposalOutcome, EvidenceInput, EvidenceSubject, SourceInput,
};

const LEAD: Uuid = Uuid::from_u128(0x7072_0000_0000_0000_0000_0000_0000_0001);

async fn seed_pc_fixture(pool: &PgPool, tag: &str) -> (Uuid, Uuid, Uuid) {
    // 租户/项目 + 对象草稿 + 源 + 证据（候选=对象当前修订）
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('system', $1, $1) RETURNING id",
    )
    .bind(format!("b4a-{tag}-{}", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .unwrap();
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(format!("b4a-c-{tag}"))
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    let project: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, $2, $3) RETURNING id",
    )
    .bind(customer)
    .bind(format!("b4a-{tag}"))
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES ($1, 'human', 'b4a-lead', true) ON CONFLICT (id) DO NOTHING",
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
    let lead = AuditActor::principal(LEAD);
    let obj = pm_application::pc::upsert_object_draft(
        pool,
        project,
        &lead,
        pm_application::pc::UpsertObjectDraft {
            object_type: "feature".into(),
            object_key: Some(format!("repo:{tag}:tech_stack")),
            display_name: "技术栈".into(),
            content_markdown: "rust".into(),
            change_summary: Some("b4a draft".into()),
            source_context: None,
            analysis_mode: None,
        },
    )
    .await
    .unwrap();
    let kb_doc: Uuid = sqlx::query_scalar(
        "INSERT INTO kb.document (tenant_id, project_id, doc_key, doc_type, canonical_path) \
         VALUES ($1, $2, $3, 'knowledge', $3) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(format!("kb-b4a-{}", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .unwrap();
    let kb_rev: Uuid = sqlx::query_scalar(
        "INSERT INTO kb.document_revision \
         (tenant_id, document_id, revision_no, content_hash, status, content, created_by, \
          confirmed_by, published_at) \
         VALUES ($1, $2, 1, 'hash-b4a', 'published', '# b4a', $3, $3, now()) RETURNING id",
    )
    .bind(tenant)
    .bind(kb_doc)
    .bind(LEAD)
    .fetch_one(pool)
    .await
    .unwrap();
    let source = register_source(
        pool,
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
    register_evidence(
        pool,
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
            evidence_hash: "h-b4a".into(),
            confidence: None,
        },
    )
    .await
    .unwrap();
    (tenant, project, source)
}

/// 场景 1/9 + 3 + 8：同 op 同意图幂等重放；账本行承载断言。
#[sqlx::test(migrations = "../../migrations")]
async fn analyze_replay_is_idempotent_via_ledger(pool: PgPool) {
    let (_tenant, project, source) = seed_pc_fixture(&pool, "replay").await;
    let task = Uuid::new_v4();
    let input = AnalysisProposalInput {
        source_id: source,
        task_id: task,
        title: "仓库认知更新".into(),
        rationale: "b4a-replay".into(),
    };

    let first = propose_from_analysis(&pool, project, input.clone())
        .await
        .unwrap();
    let AnalysisProposalOutcome::Proposed(r1) = first else {
        panic!("expected Proposed");
    };
    assert_eq!(r1.status, "submitted");

    // 账本行承载（场景 8）：command/status/request_fingerprint
    let (command, status, fp): (String, String, Option<String>) = sqlx::query_as(
        "SELECT command, status, request_fingerprint FROM pc.write_operation \
          WHERE project_id = $1 AND operation_id = $2",
    )
    .bind(project)
    .bind(format!("pc-analyze:{task}"))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(command, "pc_propose_from_analysis");
    assert_eq!(status, "completed");
    assert!(
        fp.as_deref().is_some_and(|v| !v.is_empty()),
        "请求指纹已锚定"
    );

    // 场景 1/9：同 op 同意图 → completed 短路，幂等返回原提案
    let second = propose_from_analysis(&pool, project, input).await.unwrap();
    let AnalysisProposalOutcome::Proposed(r2) = second else {
        panic!("expected Proposed (replay)");
    };
    assert_eq!(r2.proposal_id, r1.proposal_id, "幂等返回原提案");
    assert_eq!(r2.proposal_content_hash, r1.proposal_content_hash);

    // 不重复起草：同源提案仅 1 条
    let proposals: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pc.proposal WHERE source_id = $1")
            .bind(source)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(proposals, 1, "同 op 重放不得重复起草");
}

/// 场景 2：同 op_id + 异意图 → 指纹失配 409（不得误判重放、不产生第二提案）。
#[sqlx::test(migrations = "../../migrations")]
async fn analyze_same_op_different_intent_rejected(pool: PgPool) {
    let (_tenant, project, source) = seed_pc_fixture(&pool, "mismatch").await;
    let task = Uuid::new_v4();
    let input = AnalysisProposalInput {
        source_id: source,
        task_id: task,
        title: "原始意图".into(),
        rationale: "v1".into(),
    };
    let first = propose_from_analysis(&pool, project, input).await.unwrap();
    assert!(matches!(first, AnalysisProposalOutcome::Proposed(_)));

    let conflicting = AnalysisProposalInput {
        source_id: source,
        task_id: task,
        title: "被篡改的意图".into(),
        rationale: "v1".into(),
    };
    let r = propose_from_analysis(&pool, project, conflicting).await;
    assert!(
        matches!(r, Err(pm_application::pc::PcError::Conflict(ref e)) if e.contains("fingerprint mismatch")),
        "同 op 异意图必须 409 指纹失配: {r:?}"
    );
    let proposals: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pc.proposal WHERE source_id = $1")
            .bind(source)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(proposals, 1, "失配拒绝不得产生第二提案");
}

/// GAP-27 发布段严格指纹：completed 行 fp=内容锚；fp 失配（合成）→ 409；
/// 恢复后 replayed。
#[sqlx::test(migrations = "../../migrations")]
async fn publish_completed_short_circuit_compares_fingerprint(pool: PgPool) {
    let (_tenant, project, source) = seed_pc_fixture(&pool, "pubfp").await;
    let task = Uuid::new_v4();
    let first = propose_from_analysis(
        &pool,
        project,
        AnalysisProposalInput {
            source_id: source,
            task_id: task,
            title: "发布指纹验证".into(),
            rationale: "b4a".into(),
        },
    )
    .await
    .unwrap();
    let AnalysisProposalOutcome::Proposed(receipt) = first else {
        panic!("expected Proposed");
    };
    // AI 提案需人工批准（decide 为终局；GAP-35/GAP-05 语义在位）
    pm_application::pc::approve_proposal(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        receipt.proposal_id,
        pm_application::pc::DecisionInput {
            reason: Some("发布指纹验证批准".into()),
            expected_version_no: receipt.expected_version_no,
        },
    )
    .await
    .unwrap();

    // 首次发布（completed 行 fp=内容锚）
    let v1 = publish_version(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        pm_application::pc::PublishVersionInput {
            proposal_id: receipt.proposal_id,
        },
    )
    .await
    .unwrap();
    assert!(!v1.replayed);

    // 合成失配：篡改账本指纹 → 同 op 重发布 409（不得误判重放）
    sqlx::query(
        "UPDATE pc.write_operation SET request_fingerprint = 'tampered' \
          WHERE project_id = $1 AND operation_id = $2",
    )
    .bind(project)
    .bind(format!("pc-decision:{}", receipt.proposal_id))
    .execute(&pool)
    .await
    .unwrap();
    let mismatched = publish_version(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        pm_application::pc::PublishVersionInput {
            proposal_id: receipt.proposal_id,
        },
    )
    .await;
    assert!(
        matches!(mismatched, Err(pm_application::pc::PcError::Conflict(ref e)) if e.contains("fingerprint mismatch")),
        "发布段同 op 异 fp 必须 409: {mismatched:?}"
    );

    // 恢复指纹 → 同 op 同 fp → 幂等重放
    let content_hash: String =
        sqlx::query_scalar("SELECT proposal_content_hash FROM pc.proposal WHERE id = $1")
            .bind(receipt.proposal_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(
        "UPDATE pc.write_operation SET request_fingerprint = $2 \
          WHERE project_id = $1 AND operation_id = $3",
    )
    .bind(project)
    .bind(&content_hash)
    .bind(format!("pc-decision:{}", receipt.proposal_id))
    .execute(&pool)
    .await
    .unwrap();
    let replay = publish_version(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        pm_application::pc::PublishVersionInput {
            proposal_id: receipt.proposal_id,
        },
    )
    .await
    .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.version_id, v1.version_id);
}

/// ProjC 镜像：起草段账本幂等重放（同 op 同意图）+ 异意图 409。
#[sqlx::test(migrations = "../../migrations")]
async fn projc_analyze_replay_is_idempotent_via_ledger(pool: PgPool) {
    use pm_application::projc::{
        propose_from_analysis as projc_propose, AnalysisProposalInput as ProjcInput,
        AnalysisProposalOutcome as ProjcOutcome,
    };

    // 复用 pc 夹具（upsert_object_draft 走 projc 域）需要 projc 域种子——
    // 直接以 projc 助手重建（与 pc 版同构）。
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('system', $1, $1) RETURNING id",
    )
    .bind(format!("b4a-pj-{}", Uuid::new_v4().simple()))
    .fetch_one(&pool)
    .await
    .unwrap();
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ($1, $2) RETURNING id",
    )
    .bind("b4a-pj-c")
    .bind(tenant)
    .fetch_one(&pool)
    .await
    .unwrap();
    let project: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, $2, $3) RETURNING id",
    )
    .bind(customer)
    .bind("b4a-pj")
    .bind(tenant)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES ($1, 'human', 'b4a-lead', true) ON CONFLICT (id) DO NOTHING",
    )
    .bind(LEAD)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ($1, $2, $3, 'owner') ON CONFLICT DO NOTHING",
    )
    .bind(tenant)
    .bind(project)
    .bind(LEAD)
    .execute(&pool)
    .await
    .unwrap();
    let lead = AuditActor::principal(LEAD);
    let obj = pm_application::projc::upsert_object_draft(
        &pool,
        project,
        &lead,
        pm_application::projc::UpsertObjectDraft {
            object_type: "tech_stack".into(),
            object_key: Some("repo:projc:tech_stack".into()),
            display_name: "技术栈".into(),
            content_markdown: "rust".into(),
            change_summary: Some("b4a draft".into()),
            source_context: None,
            analysis_mode: None,
        },
    )
    .await
    .unwrap();
    let kb_doc: Uuid = sqlx::query_scalar(
        "INSERT INTO kb.document (tenant_id, project_id, doc_key, doc_type, canonical_path) \
         VALUES ($1, $2, $3, 'knowledge', $3) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(format!("kb-b4apj-{}", Uuid::new_v4().simple()))
    .fetch_one(&pool)
    .await
    .unwrap();
    let kb_rev: Uuid = sqlx::query_scalar(
        "INSERT INTO kb.document_revision \
         (tenant_id, document_id, revision_no, content_hash, status, content, created_by, \
          confirmed_by, published_at) \
         VALUES ($1, $2, 1, 'hash-b4apj', 'published', '# b4apj', $3, $3, now()) RETURNING id",
    )
    .bind(tenant)
    .bind(kb_doc)
    .bind(LEAD)
    .fetch_one(&pool)
    .await
    .unwrap();
    let source = pm_application::projc::register_source(
        &pool,
        project,
        &lead,
        pm_application::projc::SourceInput {
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
    pm_application::projc::register_evidence(
        &pool,
        project,
        &lead,
        pm_application::projc::EvidenceInput {
            source_id: source,
            subject: pm_application::projc::EvidenceSubject::ObjectRevision(obj.revision_id),
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
            evidence_hash: "h-b4apj".into(),
            confidence: None,
        },
    )
    .await
    .unwrap();

    let task = Uuid::new_v4();
    let input = ProjcInput {
        source_id: source,
        task_id: task,
        title: "仓库认知更新".into(),
        rationale: "b4a-pj".into(),
    };
    let first = projc_propose(&pool, project, input.clone()).await.unwrap();
    let ProjcOutcome::Proposed(r1) = first else {
        panic!("expected Proposed");
    };
    let second = projc_propose(&pool, project, input).await.unwrap();
    let ProjcOutcome::Proposed(r2) = second else {
        panic!("expected Proposed (replay)");
    };
    assert_eq!(
        r2.proposal_id, r1.proposal_id,
        "ProjC 同 op 同意图幂等返回原提案"
    );

    let (command, status): (String, String) = sqlx::query_as(
        "SELECT command, status FROM projc.write_operation \
          WHERE project_id = $1 AND operation_id = $2",
    )
    .bind(project)
    .bind(format!("projc-analyze:{task}"))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(command, "projc_propose_from_analysis");
    assert_eq!(status, "completed");

    let conflicting = ProjcInput {
        source_id: source,
        task_id: task,
        title: "异意图".into(),
        rationale: "b4a-pj".into(),
    };
    let r = projc_propose(&pool, project, conflicting).await;
    assert!(
        matches!(r, Err(pm_application::projc::ProjcError::Conflict(ref e)) if e.contains("fingerprint mismatch")),
        "ProjC 同 op 异意图必须 409: {r:?}"
    );
    let _ = obj;
}
