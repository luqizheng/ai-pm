//! 0063_pc_formal_model 迁移验证（KP-1 / PC-01B 正式模型 DDL 断言，照
//! migration_0048/0061 样式；真实数据库，不 Mock）。
//!
//! 两个用例：
//! 1. **fresh 路径**（sqlx::test 全量迁移）：01B §20 负向场景的库层子集——
//!    种子清单（18 对象类型 = 14 正典 + 4 snapshot 偏差 b / 12 关系类型）、
//!    单例部分唯一、生命周期终态、append-only、T-03/04 互斥 CHECK、
//!    T-26 闭合触发器、T-13/14/15 审批锚定、T-28 提案锚定、T-27 RR 端点
//!    一致性、写账本状态机、T-06 跨项目拒绝。
//! 2. **收敛路径**（手工建库：0001..0062 → 灌 0053 形态存量 → 应用 0063）：
//!    用户裁决「原地收敛保留草稿」的行为断言——正文迁 pc.document_revision、
//!    修订回填 document 引用与 content_hash、证据按时间钉修订并转
//!    document_section 四元组、来源 source_kind/指纹回填、对象 display_name /
//!    current_revision 指针 / created_by 归属。

use std::path::Path;

use sqlx::migrate::Migrator;
use sqlx::PgPool;
use uuid::Uuid;

const DISPATCH: Uuid = Uuid::from_u128(1);

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
    let rel_types: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pc.relationship_type")
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
        ["business_process", "business_rule", "product_decision", "product_vision"],
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
    assert!(revived.is_err(), "deprecated must be terminal (trg_pc_object_lifecycle)");

    let m3 = insert_object(&pool, tenant, project, "module", human).await;
    let sup = sqlx::query(
        "UPDATE pc.cognition_object SET lifecycle_status = 'superseded' WHERE id = $1",
    )
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
    assert!(doc_rev_touched.is_err(), "document_revision must be append-only");
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
    let rr_touched = sqlx::query(
        "UPDATE pc.relationship_revision SET change_summary = 'x' WHERE id = $1",
    )
    .bind(rel_rev)
    .execute(&pool)
    .await;
    assert!(rr_touched.is_err(), "relationship_revision must be append-only");

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
    assert!(broken.is_err(), "trg_pc_vm_closure must reject missing endpoint member");
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
    assert!(a2.is_err(), "uq_pc_approval_pending must forbid a second pending");

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
    assert!(retitled.is_err(), "trg_pc_proposal_anchors must freeze submitted content");

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
    assert!(cross.is_err(), "cross-project member must violate composite FK");

    // ── 派生视图可用 ──
    let view_rows: i64 = sqlx::query_scalar("SELECT count(*) FROM pc.v_revision_versioned")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(view_rows >= 1, "v_revision_versioned must be queryable");
}

// ══════════════════════════════════════════════════════════════════════════
// 用例 2：收敛路径——0001..0062 + 0053 形态存量 → 应用 0063
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn convergence_from_0053_legacy_rows_preserves_drafts() {
    // 测试库管理：DATABASE_URL 指向专用容器（cargo-test.sh 注入）。
    let base = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set by cargo-test.sh");
    let (prefix, _) = base
        .rsplit_once('/')
        .expect("DATABASE_URL must carry a db path");
    let admin_url = format!("{prefix}/postgres");
    let dbname = format!("kp1_conv_{}", Uuid::new_v4().simple());
    let db_url = format!("{prefix}/{dbname}");

    let admin = sqlx::PgPool::connect(&admin_url).await.unwrap();
    // TEMPLATE template0：绕开 template1 预建机制（其已含全部迁移，会破坏
    // 「先 0001..0062 再 0063」的两段式迁移序列）。
    sqlx::query(&format!("CREATE DATABASE \"{dbname}\" TEMPLATE template0"))
        .execute(&admin)
        .await
        .unwrap();

    let outcome = run_convergence(&db_url).await;

    // 收尾删库（尽力：先断开残余连接）
    sqlx::query(&format!(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
          WHERE datname = '{dbname}' AND pid <> pg_backend_pid()"
    ))
    .execute(&admin)
    .await
    .ok();
    sqlx::query(&format!("DROP DATABASE IF EXISTS \"{dbname}\""))
        .execute(&admin)
        .await
        .ok();
    admin.close().await;
    outcome.unwrap_or_else(|e| panic!("convergence failed: {e}"));
}

fn sha256_hex(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let d = Sha256::digest(input.as_bytes());
    d.iter().map(|b| format!("{b:02x}")).collect()
}

async fn run_convergence(db_url: &str) -> Result<(), String> {
    // ① 迁移 0001..0062（截断 Migrator：迁移清单按版本升序，取前 62 个）
    let mut pre = Migrator::new(Path::new("../../migrations"))
        .await
        .map_err(|e| e.to_string())?;
    {
        let list = pre.migrations.to_mut();
        let last = list.last().expect("migrations non-empty");
        assert_eq!(last.version, 63, "head migration must be 0063");
        list.truncate(list.len() - 1);
    }
    let pool = sqlx::PgPool::connect(db_url).await.map_err(|e| e.to_string())?;
    pre.run(&pool).await.map_err(|e| e.to_string())?;

    // ② 灌 0053 形态存量（收敛前旧列集）
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('system', 'pc63-conv', 'pc63-conv') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ('c', $1) RETURNING id",
    )
    .bind(tenant)
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;
    let project: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, 'conv', 'conv', $2) RETURNING id",
    )
    .bind(customer)
    .bind(tenant)
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;

    // 旧形态对象：name 列、无 created_by/current_revision_id
    let object_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.cognition_object \
         (tenant_id, project_id, object_type, object_key, name, lifecycle_status) \
         VALUES ($1, $2, 'repository', 'repo:conv:snapshot', '旧名', 'draft') RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;

    // 旧形态修订 ×2：content/status 列；created_by NULL（回填路径）
    let rev1_created = chrono::Utc::now() - chrono::Duration::hours(2);
    let rev1: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.cognition_revision \
         (tenant_id, object_id, revision_no, summary, content, status, created_by, created_at) \
         VALUES ($1, $2, 1, '快照 r1', '# 快照 v1', 'draft', NULL, $3) RETURNING id",
    )
    .bind(tenant)
    .bind(object_id)
    .bind(rev1_created)
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;
    let rev2_created = chrono::Utc::now() - chrono::Duration::hours(1);
    let rev2: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.cognition_revision \
         (tenant_id, object_id, revision_no, summary, content, status, created_by, created_at) \
         VALUES ($1, $2, 2, '快照 r2', '# 快照 v2', 'draft', NULL, $3) RETURNING id",
    )
    .bind(tenant)
    .bind(object_id)
    .bind(rev2_created)
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;

    // 旧形态来源：locator JSONB
    let kb_document_ref = Uuid::new_v4();
    let kb_revision_ref = Uuid::new_v4();
    let source_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.source \
         (tenant_id, project_id, target_domain, target_type, target_id, target_revision_id, locator) \
         VALUES ($1, $2, 'knowledge_base', 'document', $3, $4, $5) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(kb_document_ref.to_string())
    .bind(kb_revision_ref.to_string())
    .bind(serde_json::json!({
        "kind": "kb_document_revision",
        "revision_id": kb_revision_ref,
        "content_hash": "abc123hash",
    }))
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;

    // 旧形态证据 ×2：rev1 时点（→钉 rev1）与 rev2 时点（→钉 rev2）
    for (slot, head, at) in [
        ("snapshot", "aaaa1111", rev1_created + chrono::Duration::minutes(1)),
        ("tech_stack", "bbbb2222", rev2_created + chrono::Duration::minutes(1)),
    ] {
        sqlx::query(
            "INSERT INTO pc.evidence \
             (tenant_id, project_id, source_id, cognition_object_id, evidence_type, locator, \
              evidence_hash, verification_status, confidence, created_at) \
             VALUES ($1, $2, $3, $4, 'repo_facts', $5, 'abc123hash', 'unverified', 'medium', $6)",
        )
        .bind(tenant)
        .bind(project)
        .bind(source_id)
        .bind(object_id)
        .bind(serde_json::json!({ "slot": slot, "head_commit_sha": head, "intake_id": "i1" }))
        .bind(at)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;
    }

    // ③ 应用 0063（全新 Migrator；记账 1..62 已在，只差 0063）
    let post = Migrator::new(Path::new("../../migrations"))
        .await
        .map_err(|e| e.to_string())?;
    post.run(&pool).await.map_err(|e| e.to_string())?;

    // ④ 断言：正文迁移（object → document 1:1；revision → document_revision 1:1）
    let doc: (Uuid, String, String) = sqlx::query_as(
        "SELECT id, doc_key, canonical_path FROM pc.document WHERE project_id = $1",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;
    assert_eq!(doc.1, format!("pc-object-{object_id}"), "doc_key fallback (deviation e)");
    assert_eq!(doc.2, format!("pc/repository/{object_id}.md"));

    let doc_revs: Vec<(i32, String, String)> = sqlx::query_as(
        "SELECT revision_no, content_hash, content FROM pc.document_revision \
          WHERE document_id = $1 ORDER BY revision_no",
    )
    .bind(doc.0)
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;
    assert_eq!(doc_revs.len(), 2, "both legacy revisions migrated");
    assert_eq!(doc_revs[0].2, "# 快照 v1");
    assert_eq!(doc_revs[1].2, "# 快照 v2");
    assert_eq!(doc_revs[0].1, sha256_hex("# 快照 v1"), "content_hash = sha256(content)");
    assert_eq!(doc_revs[1].1, sha256_hex("# 快照 v2"));

    // 修订行回填：project/document 引用/hash/change_summary；analysis_mode 保留
    let rev_row: (Uuid, String, String, Option<String>) = sqlx::query_as(
        "SELECT document_id, change_summary, content_hash, analysis_mode \
           FROM pc.cognition_revision WHERE id = $1",
    )
    .bind(rev2)
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;
    assert_eq!(rev_row.0, doc.0, "document_id backfilled");
    assert_eq!(rev_row.1, "快照 r2", "summary renamed to change_summary");
    assert_eq!(rev_row.2, sha256_hex("# 快照 v2"), "content_hash backfilled");
    assert_eq!(rev_row.3.as_deref(), Some("derived"), "0062 analysis_mode preserved");

    // 修订 append-only 已生效
    let touched =
        sqlx::query("UPDATE pc.cognition_revision SET change_summary = 'x' WHERE id = $1")
            .bind(rev1)
            .execute(&pool)
            .await;
    assert!(touched.is_err(), "converged revision rows are append-only");

    // 对象收敛：display_name / current_revision 指针 / created_by 归属
    let obj_row: (String, Option<Uuid>, Uuid) = sqlx::query_as(
        "SELECT display_name, current_revision_id, created_by \
           FROM pc.cognition_object WHERE id = $1",
    )
    .bind(object_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;
    assert_eq!(obj_row.0, "旧名", "name renamed to display_name");
    assert_eq!(obj_row.1, Some(rev2), "current_revision_id = latest revision");
    assert_eq!(obj_row.2, DISPATCH, "created_by backfilled to dispatch principal");

    // 来源收敛：source_kind / 指纹 / collected_by
    let src_row: (String, Option<String>, Uuid) = sqlx::query_as(
        "SELECT source_kind, source_content_hash, collected_by FROM pc.source WHERE id = $1",
    )
    .bind(source_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;
    assert_eq!(src_row.0, "kb_document");
    assert_eq!(src_row.1.as_deref(), Some("abc123hash"), "locator content_hash extracted");
    assert_eq!(src_row.2, DISPATCH);

    // 证据收敛：按时间钉修订 + document_section 四元组 + 槽位/legacy JSON 归档
    let ev_rows: Vec<(Uuid, String, Option<String>, Option<String>, Option<String>)> =
        sqlx::query_as(
            "SELECT object_revision_id, evidence_type, section_anchor, target_id, description \
               FROM pc.evidence WHERE project_id = $1 ORDER BY section_anchor",
        )
        .bind(project)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;
    assert_eq!(ev_rows.len(), 2);
    assert_eq!(ev_rows[0].0, rev1, "earlier evidence pinned to revision 1");
    assert_eq!(ev_rows[1].0, rev2, "later evidence pinned to revision 2");
    assert_eq!(ev_rows[0].1, "document_section");
    assert_eq!(ev_rows[0].2.as_deref(), Some("snapshot"), "slot -> section_anchor");
    assert_eq!(ev_rows[1].2.as_deref(), Some("tech_stack"));
    assert_eq!(
        ev_rows[0].3.as_deref(),
        Some(kb_document_ref.to_string().as_str()),
        "quad target_id from source"
    );
    let legacy = ev_rows[0].4.as_deref().expect("legacy locator archived");
    assert!(legacy.contains("aaaa1111"), "legacy locator JSON preserved in description");
    assert!(legacy.contains("intake_id"), "attribution preserved");

    pool.close().await;
    Ok(())
}
