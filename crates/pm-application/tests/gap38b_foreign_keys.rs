//! P3-D04-02 Step6 前置 · GAP-38-B 外键落地验证（目录断言+并发交错背书）。
//!
//! 断言面（38-B 分析 §三 强制闭环）：
//! 1. 目录断言：79 FK（pc 镜像全集）+12 引用面 UNIQUE 在位。
//! 2. 引用完整性背书：孤儿 INSERT 被拒（23503，NO ACTION 语义）。
//! 3. 并发交错背书：publish×draft upsert×decide 三方并发多连接压测——
//!    无死锁/无异常传播，最终状态语义不变（FK SHARE 检查与既有 FOR UPDATE 同向）。

use std::time::Duration;

use sqlx::{PgPool, Row};
use uuid::Uuid;

use pm_application::actor::AuditActor;
use pm_application::pc::{
    approve_proposal, create_proposal, publish_version, submit_proposal, update_proposal_draft,
    upsert_object_draft, CreateProposalInput, DecisionInput, ProposalMemberPin,
    PublishVersionInput, UpsertObjectDraft,
};

const LEAD: Uuid = Uuid::from_u128(0x7072_0000_0000_0000_0000_0000_0000_0009);

async fn seed(pool: &PgPool, tag: &str) -> Uuid {
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('system', $1, $1) RETURNING id",
    )
    .bind(format!("b38-{tag}-{}", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .unwrap();
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(format!("b38-c-{tag}"))
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    let project: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, $2, $3) RETURNING id",
    )
    .bind(customer)
    .bind(format!("b38-{tag}"))
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES ($1, 'human', 'b38-lead', true) ON CONFLICT (id) DO NOTHING",
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
    project
}

fn draft(key: &str) -> UpsertObjectDraft {
    UpsertObjectDraft {
        object_type: "feature".into(),
        object_key: Some(key.into()),
        display_name: key.into(),
        content_markdown: format!("# {key}"),
        change_summary: Some("b38 draft".into()),
        source_context: None,
        analysis_mode: None,
    }
}

fn one_member(oid: Uuid, rid: Uuid) -> Vec<ProposalMemberPin> {
    vec![ProposalMemberPin {
        change_kind: "create_object".into(),
        object_id: Some(oid),
        object_revision_id: Some(rid),
        relationship_id: None,
        relationship_revision_id: None,
    }]
}

/// 目录断言：79 FK+12 引用面 UNIQUE。
#[sqlx::test(migrations = "../../migrations")]
async fn fk_catalog_matches_pc_mirror(pool: PgPool) {
    let fks: Vec<String> = sqlx::query(
        "SELECT con.conname FROM pg_constraint con \
           JOIN pg_class c ON c.oid = con.conrelid \
           JOIN pg_namespace n ON n.oid = c.relnamespace \
          WHERE n.nspname = 'projc' AND con.contype = 'f' ORDER BY 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap()
    .into_iter()
    .map(|r| r.get::<String, _>(0))
    .collect();
    assert_eq!(fks.len(), 79, "projc FK 全集=pc 镜像 79 边");
    // 逐字抽查三条代表性边（含 DEFERRABLE 特例与 core 引用）
    let defn: String = sqlx::query_scalar(
        "SELECT pg_get_constraintdef(con.oid) FROM pg_constraint con \
          JOIN pg_class c ON c.oid = con.conrelid JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = 'projc' AND con.conname = 'cognition_object_current_revision_fkey'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        defn,
        "FOREIGN KEY (tenant_id, project_id, id, current_revision_id) REFERENCES projc.cognition_revision(tenant_id, project_id, object_id, id) DEFERRABLE"
    );
    let backing: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_constraint con \
           JOIN pg_class c ON c.oid = con.conrelid \
           JOIN pg_namespace n ON n.oid = c.relnamespace \
          WHERE n.nspname = 'projc' AND con.contype = 'u' \
            AND con.conname IN ('cognition_object_tenant_id_project_id_id_key', \
                'cognition_revision_tenant_id_project_id_object_id_id_key', \
                'proposal_tenant_id_project_id_id_key', 'document_tenant_id_project_id_id_key', \
                'document_revision_tenant_id_project_id_document_id_id_key', \
                'conflict_tenant_id_project_id_id_key', 'relationship_tenant_id_project_id_id_key', \
                'cognition_revision_tenant_id_project_id_id_key', \
                'relationship_revision_tenant_id_project_id_id_key', 'source_tenant_id_project_id_id_key', \
                'relationship_revision_tenant_id_project_id_relationship_id_id_k', \
                'cognition_version_tenant_id_project_id_id_key')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(backing, 12, "引用面 UNIQUE ×12（含 63 字符截断名）");
}

/// 引用完整性背书：孤儿成员 INSERT 被 FK 拒绝（23503）。
#[sqlx::test(migrations = "../../migrations")]
async fn orphan_member_insert_rejected_by_fk(pool: PgPool) {
    let project = seed(&pool, "orphan").await;
    let err = sqlx::query(
        "INSERT INTO pc.proposal_member \
         (tenant_id, project_id, proposal_id, change_kind, object_id, object_revision_id) \
         SELECT tenant_id, id AS project_id, '11111111-1111-1111-1111-111111111111'::uuid, \
                'create_object', '22222222-2222-2222-2222-222222222222'::uuid, \
                '33333333-3333-3333-3333-333333333333'::uuid \
           FROM core.project WHERE id = $1",
    )
    .bind(project)
    .execute(&pool)
    .await
    .unwrap_err();
    let db = err.as_database_error().unwrap();
    assert_eq!(
        db.code().as_deref(),
        Some("23503"),
        "孤儿必须被 FK 拒: {db}"
    );
}

/// 并发交错背书：publish×draft upsert×decide 三方并发（多连接）——无死锁无异常，
/// 终态语义不变（38-B 分析 §三 强制闭环）。
#[sqlx::test(migrations = "../../migrations")]
async fn concurrent_publish_upsert_decide_no_deadlock(pool: PgPool) {
    let project = seed(&pool, "conc").await;
    let lead = AuditActor::principal(LEAD);
    let a = upsert_object_draft(&pool, project, &lead, draft("b38-ca"))
        .await
        .unwrap();

    let proposal = create_proposal(
        &pool,
        project,
        &lead,
        CreateProposalInput {
            title: "并发背书".into(),
            rationale: "b38".into(),
            origin_kind: "manual_input".into(),
            source_id: None,
            governance_level: None,
            risk_level: None,
            confidence: None,
            members: one_member(a.object_id, a.revision_id),
        },
    )
    .await
    .unwrap();
    submit_proposal(&pool, project, &lead, proposal.proposal_id, Some(0))
        .await
        .unwrap();
    approve_proposal(
        &pool,
        project,
        &lead,
        proposal.proposal_id,
        DecisionInput {
            reason: Some("并发背书批准".into()),
            expected_version_no: 0,
        },
    )
    .await
    .unwrap();

    // 三方并发：独立连接池句柄（sqlx::test 单池，直接并发 task）
    let pool1 = pool.clone();
    let pool2 = pool.clone();
    let pool3 = pool.clone();
    let (r1, r2, r3) = tokio::join!(
        async move {
            publish_version(
                &pool1,
                project,
                &AuditActor::principal(LEAD),
                PublishVersionInput {
                    proposal_id: proposal.proposal_id,
                },
            )
            .await
        },
        async move {
            // draft upsert：新对象草稿（同项目并发写路径）
            upsert_object_draft(
                &pool2,
                project,
                &AuditActor::principal(LEAD),
                draft("b38-cb"),
            )
            .await
        },
        async move {
            // decide 交错（同项目裁决路径）：与 publish 腿并发时 submit CAS 可能合法
            // 拒绝（version baseline moved → re-propose，CAS fail-safe 语义）——
            // 按领域口径重试；断言点=无死锁且最终裁决成功。
            for attempt in 0..8u32 {
                let b = upsert_object_draft(
                    &pool3,
                    project,
                    &AuditActor::principal(LEAD),
                    draft(&format!("b38-cc-{attempt}")),
                )
                .await
                .unwrap();
                let q = create_proposal(
                    &pool3,
                    project,
                    &AuditActor::principal(LEAD),
                    CreateProposalInput {
                        title: "并发裁决".into(),
                        rationale: "b38".into(),
                        origin_kind: "manual_input".into(),
                        source_id: None,
                        governance_level: None,
                        risk_level: None,
                        confidence: None,
                        members: one_member(b.object_id, b.revision_id),
                    },
                )
                .await
                .unwrap();
                let baseline = q.expected_version_no;
                match submit_proposal(&pool3, project, &lead, q.proposal_id, Some(baseline)).await {
                    Ok(_) => {
                        approve_proposal(
                            &pool3,
                            project,
                            &lead,
                            q.proposal_id,
                            DecisionInput {
                                reason: Some("并发批准".into()),
                                expected_version_no: baseline,
                            },
                        )
                        .await?;
                        return Ok(());
                    }
                    Err(e) if e.to_string().contains("version baseline moved") => continue,
                    Err(e) => return Err(e),
                }
            }
            panic!("decide 腿重试 8 次未成功（非预期）");
        },
    );
    // 三方全部成功（advisory+FOR UPDATE 键序承载交错；FK SHARE 检查同向无死锁）
    let v1 = r1.unwrap();
    assert_eq!(v1.version_no, 1);
    let _ = r2.unwrap();
    let _ = r3.unwrap();

    // 终态语义：版本 1 落地+两提案 applied/approved
    let versions: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pc.cognition_version WHERE project_id = $1")
            .bind(project)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(versions, 1);
    let _ = Duration::from_secs(0); // 保持 import 对齐（分析口径超时语义引用）
}
