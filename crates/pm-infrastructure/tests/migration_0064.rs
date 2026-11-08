//! PC 正式模型 DDL 断言（KP-1 / PC-01B，01B §20 负向场景的库层子集；真实
//! 数据库，不 Mock）。文件名沿用引入本域的历史迁移号 0064；该迁移已随
//! 2026-11-09 迁移基线合并（0001_baseline.sql 吸收 0001~0065）不再是独立
//! 文件，head 钉点断言与「收敛路径」用例（手工建链 0001..0063 → 灌 0053
//! 形态存量 → 应用 0064）一并退役——无迁移链即无升级收敛场景，见
//! docs/ai-pm-migration-ledger.md 基线合并记录。

use sqlx::PgPool;
use uuid::Uuid;

async fn seed_tenant_project(pool: &PgPool, tag: &str) -> (Uuid, Uuid) {
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('system', $1, $1) RETURNING id",
    )
    .bind(format!("pc63-{tag}-{}", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .unwrap();
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(format!("pc63-c-{tag}"))
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    let project: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, $2, $3) RETURNING id",
    )
    .bind(customer)
    .bind(format!("pc63-{tag}"))
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    (tenant, project)
}

async fn seed_principal(pool: &PgPool, tag: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) \
         VALUES ('human', $1, true) RETURNING id",
    )
    .bind(format!("pc63-p-{tag}-{}", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_object(
    pool: &PgPool,
    tenant: Uuid,
    project: Uuid,
    object_type: &str,
    created_by: Uuid,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO pc.cognition_object \
         (tenant_id, project_id, object_type, object_key, display_name, created_by) \
         VALUES ($1, $2, $3, $4, $4, $5) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(object_type)
    .bind(format!("{object_type}-{}", Uuid::new_v4().simple()))
    .bind(created_by)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 对象首修订（1 文档 + 1 文档修订 + 1 认知修订）。
async fn insert_first_revision(
    pool: &PgPool,
    tenant: Uuid,
    project: Uuid,
    object_id: Uuid,
    content: &str,
    created_by: Uuid,
) -> (Uuid, Uuid, Uuid) {
    let document_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.document (tenant_id, project_id, doc_key, canonical_path) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(format!("pc-object-{object_id}"))
    .bind(format!("pc/feature/{object_id}.md"))
    .fetch_one(pool)
    .await
    .unwrap();
    let document_revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.document_revision \
         (tenant_id, project_id, document_id, revision_no, content_hash, content, created_by) \
         VALUES ($1, $2, $3, 1, $4, $5, $6) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(document_id)
    .bind(format!("sha256:{content}"))
    .bind(content)
    .bind(created_by)
    .fetch_one(pool)
    .await
    .unwrap();
    let revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.cognition_revision \
         (tenant_id, project_id, object_id, revision_no, document_id, document_revision_id, \
          content_hash, change_summary, created_by) \
         VALUES ($1, $2, $3, 1, $4, $5, $6, 'r', $7) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(object_id)
    .bind(document_id)
    .bind(document_revision_id)
    .bind(format!("sha256:{content}"))
    .bind(created_by)
    .fetch_one(pool)
    .await
    .unwrap();
    (document_id, document_revision_id, revision_id)
}

async fn insert_proposal(
    pool: &PgPool,
    tenant: Uuid,
    project: Uuid,
    created_by: Uuid,
    status: &str,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO pc.proposal \
         (tenant_id, project_id, title, rationale, origin_kind, governance_level, \
          expected_version_no, proposal_content_hash, status, created_by) \
         VALUES ($1, $2, 't', 'r', 'manual_input', 'L1', 0, 'h0', $3, $4) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(status)
    .bind(created_by)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_version(
    pool: &PgPool,
    tenant: Uuid,
    project: Uuid,
    proposal_id: Uuid,
    confirmed_by: Uuid,
    version_no: i64,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO pc.cognition_version \
         (tenant_id, project_id, version_no, confirmed_by, source_proposal_id, snapshot_hash) \
         VALUES ($1, $2, $3, $4, $5, 'snap') RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(version_no)
    .bind(confirmed_by)
    .bind(proposal_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_object_member(
    pool: &PgPool,
    tenant: Uuid,
    project: Uuid,
    version_id: Uuid,
    object_id: Uuid,
    revision_id: Uuid,
) {
    sqlx::query(
        "INSERT INTO pc.version_member \
         (tenant_id, project_id, version_id, object_id, object_revision_id) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(tenant)
    .bind(project)
    .bind(version_id)
    .bind(object_id)
    .bind(revision_id)
    .execute(pool)
    .await
    .unwrap();
}

// ══════════════════════════════════════════════════════════════════════════
// 用例 1：fresh 路径——库层负向断言（01B §20 子集）
// ══════════════════════════════════════════════════════════════════════════

#[sqlx::test(migrations = "../../migrations")]
async fn formal_model_ddl_negatives_and_seeds(pool: PgPool) {
    // ── 种子清单：18 对象类型（14 正典 + 4 snapshot）+ 12 关系类型 ──
    let canonical: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pc.object_type WHERE layer <> 'snapshot'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(canonical, 14, "14 canonical object types");
    let snapshot: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pc.object_type WHERE layer = 'snapshot'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(snapshot, 4, "checkpoint snapshot types kept (deviation b)");
    let rel_types: i64 = sqlx::query_scalar("SELECT count(*) FROM pc.relationship_type")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rel_types, 12, "17 matrix rows folded into 12 type keys");
    let l2: Vec<(String,)> = sqlx::query_as(
        "SELECT type_key FROM pc.object_type WHERE governance_level = 'L2' ORDER BY type_key",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        l2.into_iter().map(|r| r.0).collect::<Vec<_>>(),
        [
            "business_process",
            "business_rule",
            "product_decision",
            "product_vision"
        ],
        "fixed-L2 types (§3.3)"
    );

    let (tenant, project) = seed_tenant_project(&pool, "a").await;
    let human = seed_principal(&pool, "a").await;
    let (tenant_b, project_b) = seed_tenant_project(&pool, "b").await;

    // ── 单例约束：product_profile 第二个活跃对象拒绝；module 可多个 ──
    let _profile = insert_object(&pool, tenant, project, "product_profile", human).await;
    let dup_singleton = sqlx::query(
        "INSERT INTO pc.cognition_object \
         (tenant_id, project_id, object_type, object_key, display_name, created_by) \
         VALUES ($1, $2, 'product_profile', 'p2', 'p2', $3)",
    )
    .bind(tenant)
    .bind(project)
    .bind(human)
    .execute(&pool)
    .await;
    assert!(
        dup_singleton.is_err(),
        "second singleton must violate uq_pc_object_singleton"
    );
    // 终态单例不占名额：废弃后可再建
    sqlx::query(
        "UPDATE pc.cognition_object SET lifecycle_status = 'deprecated', deprecated_at = now() \
          WHERE project_id = $1 AND object_type = 'product_profile'",
    )
    .bind(project)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO pc.cognition_object \
         (tenant_id, project_id, object_type, object_key, display_name, created_by) \
         VALUES ($1, $2, 'product_profile', 'p3', 'p3', $3)",
    )
    .bind(tenant)
    .bind(project)
    .bind(human)
    .execute(&pool)
    .await
    .expect("deprecated singleton frees the slot");
    let _m1 = insert_object(&pool, tenant, project, "module", human).await;
    let _m2 = insert_object(&pool, tenant, project, "module", human).await;

    // ── 生命周期：终态不可复活（P0001）；superseded 必填承接对象 ──
    let revived = sqlx::query(
        "UPDATE pc.cognition_object SET lifecycle_status = 'active' \
          WHERE project_id = $1 AND object_type = 'product_profile' \
            AND lifecycle_status = 'deprecated'",
    )
    .bind(project)
    .execute(&pool)
    .await;
    assert!(
        revived.is_err(),
        "deprecated must be terminal (trg_pc_object_lifecycle)"
    );

    let m3 = insert_object(&pool, tenant, project, "module", human).await;
    let sup =
        sqlx::query("UPDATE pc.cognition_object SET lifecycle_status = 'superseded' WHERE id = $1")
            .bind(m3)
            .execute(&pool)
            .await;
    assert!(sup.is_err(), "superseded requires superseded_by_id");

    // ── append-only：cognition_revision / document_revision ──
    let obj_a = insert_object(&pool, tenant, project, "feature", human).await;
    let (doc_a, _doc_rev_a, rev_a) =
        insert_first_revision(&pool, tenant, project, obj_a, "# body a", human).await;
    let touched =
        sqlx::query("UPDATE pc.cognition_revision SET change_summary = 'x' WHERE id = $1")
            .bind(rev_a)
            .execute(&pool)
            .await;
    assert!(touched.is_err(), "cognition_revision must be append-only");
    let doc_rev_touched =
        sqlx::query("UPDATE pc.document_revision SET content = 'x' WHERE id = $1")
            .bind(_doc_rev_a)
            .execute(&pool)
            .await;
    assert!(
        doc_rev_touched.is_err(),
        "document_revision must be append-only"
    );
    let _ = doc_a;

    // ── T-27：RR 端点身份与关系行不一致拒绝 ──
    let obj_b = insert_object(&pool, tenant, project, "feature", human).await;
    let (_db, _drb, rev_b) =
        insert_first_revision(&pool, tenant, project, obj_b, "# body b", human).await;
    let obj_c = insert_object(&pool, tenant, project, "feature", human).await;
    let (_dc, _drc, rev_c) =
        insert_first_revision(&pool, tenant, project, obj_c, "# body c", human).await;

    let rel: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.relationship \
         (tenant_id, project_id, relationship_type_key, lifecycle_status, \
          source_object_id, target_object_id, created_by) \
         VALUES ($1, $2, 'contains', 'draft', $3, $4, $5) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(obj_a)
    .bind(obj_b)
    .bind(human)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 端点错配（source 指向 obj_c）→ 触发器拒绝
    let mismatched = sqlx::query(
        "INSERT INTO pc.relationship_revision \
         (tenant_id, project_id, relationship_id, revision_no, relationship_type_key, \
          directionality, source_object_id, source_object_revision_id, \
          target_object_id, target_object_revision_id, content_hash, created_by) \
         VALUES ($1, $2, $3, 1, 'contains', 'directed', $4, $5, $6, $7, 'h', $8)",
    )
    .bind(tenant)
    .bind(project)
    .bind(rel)
    .bind(obj_c)
    .bind(rev_c)
    .bind(obj_b)
    .bind(rev_b)
    .bind(human)
    .execute(&pool)
    .await;
    assert!(
        mismatched.is_err(),
        "trg_pc_rel_revision_endpoint_match must reject mismatched endpoints"
    );

    // 端点一致的 RR 落库成功；append-only 生效
    let rel_rev: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.relationship_revision \
         (tenant_id, project_id, relationship_id, revision_no, relationship_type_key, \
          directionality, source_object_id, source_object_revision_id, \
          target_object_id, target_object_revision_id, content_hash, created_by) \
         VALUES ($1, $2, $3, 1, 'contains', 'directed', $4, $5, $6, $7, 'h', $8) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(rel)
    .bind(obj_a)
    .bind(rev_a)
    .bind(obj_b)
    .bind(rev_b)
    .bind(human)
    .fetch_one(&pool)
    .await
    .unwrap();
    let rr_touched =
        sqlx::query("UPDATE pc.relationship_revision SET change_summary = 'x' WHERE id = $1")
            .bind(rel_rev)
            .execute(&pool)
            .await;
    assert!(
        rr_touched.is_err(),
        "relationship_revision must be append-only"
    );

    // ── T-03/T-04：版本成员互斥 CHECK；T-26：闭合触发器 ──
    let proposal = insert_proposal(&pool, tenant, project, human, "approved").await;
    let version = insert_version(&pool, tenant, project, proposal, human, 1).await;

    let both = sqlx::query(
        "INSERT INTO pc.version_member \
         (tenant_id, project_id, version_id, object_id, object_revision_id, \
          relationship_id, relationship_revision_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(tenant)
    .bind(project)
    .bind(version)
    .bind(obj_a)
    .bind(rev_a)
    .bind(rel)
    .bind(rel_rev)
    .execute(&pool)
    .await;
    assert!(both.is_err(), "member must be single-dimension (CHECK)");

    let neither = sqlx::query(
        "INSERT INTO pc.version_member (tenant_id, project_id, version_id) VALUES ($1, $2, $3)",
    )
    .bind(tenant)
    .bind(project)
    .bind(version)
    .execute(&pool)
    .await;
    assert!(neither.is_err(), "empty member must violate CHECK");

    // T-26/T-09：对象成员 A 在、端点对象 B 缺 → 闭合触发器拒绝
    insert_object_member(&pool, tenant, project, version, obj_a, rev_a).await;
    let broken = sqlx::query(
        "INSERT INTO pc.version_member \
         (tenant_id, project_id, version_id, relationship_id, relationship_revision_id) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(tenant)
    .bind(project)
    .bind(version)
    .bind(rel)
    .bind(rel_rev)
    .execute(&pool)
    .await;
    assert!(
        broken.is_err(),
        "trg_pc_vm_closure must reject missing endpoint member"
    );
    // 补齐端点成员后同一关系成员可入库（闭合满足）
    insert_object_member(&pool, tenant, project, version, obj_b, rev_b).await;
    sqlx::query(
        "INSERT INTO pc.version_member \
         (tenant_id, project_id, version_id, relationship_id, relationship_revision_id) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(tenant)
    .bind(project)
    .bind(version)
    .bind(rel)
    .bind(rel_rev)
    .execute(&pool)
    .await
    .expect("closed member set must be accepted");
    // 成员 append-only
    let member_touched = sqlx::query(
        "UPDATE pc.version_member SET object_revision_id = object_revision_id WHERE version_id = $1",
    )
    .bind(version)
    .execute(&pool)
    .await;
    assert!(member_touched.is_err(), "version_member must be immutable");

    // ── T-13/14/15：审批锚定 + pending 唯一 + 单向状态 ──
    let p2 = insert_proposal(&pool, tenant, project, human, "submitted").await;
    let hash: String =
        sqlx::query_scalar("SELECT proposal_content_hash FROM pc.proposal WHERE id = $1")
            .bind(p2)
            .fetch_one(&pool)
            .await
            .unwrap();
    let a1: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.approval \
         (tenant_id, project_id, proposal_id, proposal_content_hash, expected_version_no) \
         VALUES ($1, $2, $3, $4, 0) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(p2)
    .bind(&hash)
    .fetch_one(&pool)
    .await
    .unwrap();
    let a2 = sqlx::query(
        "INSERT INTO pc.approval \
         (tenant_id, project_id, proposal_id, proposal_content_hash, expected_version_no) \
         VALUES ($1, $2, $3, $4, 0)",
    )
    .bind(tenant)
    .bind(project)
    .bind(p2)
    .bind(&hash)
    .execute(&pool)
    .await;
    assert!(
        a2.is_err(),
        "uq_pc_approval_pending must forbid a second pending"
    );

    let reanchor = sqlx::query("UPDATE pc.approval SET proposal_content_hash = 'x' WHERE id = $1")
        .bind(a1)
        .execute(&pool)
        .await;
    assert!(reanchor.is_err(), "approval anchor columns are immutable");

    sqlx::query(
        "UPDATE pc.approval SET status = 'approved', decided_by = $2, decided_at = now() \
          WHERE id = $1",
    )
    .bind(a1)
    .bind(human)
    .execute(&pool)
    .await
    .unwrap();
    let flip = sqlx::query("UPDATE pc.approval SET status = 'rejected' WHERE id = $1")
        .bind(a1)
        .execute(&pool)
        .await;
    assert!(flip.is_err(), "decided approval is one-way");

    // ── T-28：提案 submitted 后内容锚定 ──
    let retitled = sqlx::query("UPDATE pc.proposal SET title = 'x' WHERE id = $1")
        .bind(p2)
        .execute(&pool)
        .await;
    assert!(
        retitled.is_err(),
        "trg_pc_proposal_anchors must freeze submitted content"
    );

    // ── 写账本状态机：终态不可变 ──
    let op: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.write_operation \
         (tenant_id, project_id, operation_id, command, expected_content_hash, created_by) \
         VALUES ($1, $2, 'op-1', 'pc_publish_version', 'h', $3) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(human)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE pc.write_operation SET status = 'completed', version_id = $2 WHERE id = $1",
    )
    .bind(op)
    .bind(version)
    .execute(&pool)
    .await
    .unwrap();
    let rewound = sqlx::query("UPDATE pc.write_operation SET status = 'pending' WHERE id = $1")
        .bind(op)
        .execute(&pool)
        .await;
    assert!(rewound.is_err(), "completed write_operation is terminal");

    // ── T-06：成员跨项目引用拒绝（复合 FK project 维度）──
    let proposal_b = insert_proposal(&pool, tenant_b, project_b, human, "approved").await;
    let version_b = insert_version(&pool, tenant_b, project_b, proposal_b, human, 1).await;
    let cross = sqlx::query(
        "INSERT INTO pc.version_member \
         (tenant_id, project_id, version_id, object_id, object_revision_id) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(tenant_b)
    .bind(project_b)
    .bind(version_b)
    .bind(obj_a) // 项目 A 的对象
    .bind(rev_a)
    .execute(&pool)
    .await;
    assert!(
        cross.is_err(),
        "cross-project member must violate composite FK"
    );

    // ── 派生视图可用 ──
    let view_rows: i64 = sqlx::query_scalar("SELECT count(*) FROM pc.v_revision_versioned")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(view_rows >= 1, "v_revision_versioned must be queryable");
}
