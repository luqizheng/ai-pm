//! P3-D04-02 Step5 Batch1 · GAP-04 / GAP-37-A/B 库层保护测试（T-1/T-2/T-3；
//! 真实 DB，sqlx::test 全新临时库 0001~0037 全序应用；0037 GAP-38-B 起 projc
//! 具备完整外键面，夹具播种合法引用链——见 ProjcSeed）。
//!
//! 断言面（对应《Batch1 实施任务书 V1.2》§3.3）：
//! 1. T-1 GAP-37 探针：projc.cognition_revision / projc.version_member 的
//!    UPDATE（普通字段）/ DELETE 均被数据库拒绝（A/B 各自独立断言，异常消息
//!    指向 projc.forbid_row_update 的 RAISE 文本）；INSERT 不受影响（T-5 追加
//!    维度）；全部探针在显式事务内 ROLLBACK，零残留。
//! 2. T-2 镜像保护清单测试：pc/projc 两域全部非内部触发器六维断言
//!    （pg_get_triggerdef 规范串逐字比对，含触发器名/目标表/事件/时机+约束
//!    属性/行级/执行函数）＋完备性反向枚举（任一侧出现清单外触发器=失败）。
//!    基准表=任务书 §4.5 V1.3 定向修订 + Batch6 前置批（GAP-37-C：version_delete
//!    由 D 组例外行升级为 A 组两域一致行，0035）+ GAP-06 两域转换守卫行（0034）。
//!    **V1.2 R-5 边界**：清单核对=结构一致性；不同函数≠语义等价，本批语义
//!    验证重点=新增保护面（T-1 探针＋projc_formal_chain 回归承载）；其余既有
//!    保护面不因清单测试扩大为业务语义重新审计；B 组函数有意分歧按注记断言
//!    差异本身，不作语义裁断。
//! 3. T-3 GAP-04 探针：同 (object_id, revision_no) 二次 INSERT 被拒（两域各
//!    一）＋新约束目录断言（pg_get_constraintdef 逐字）。projc 侧另做跨
//!    tenant/project 重复——0037 后 (tenant_id, project_id, object_id) 复合外键
//!    （cognition_revision_object_composite_fkey）对该组合另有覆盖，但外键拒绝
//!    走 AFTER ROW 触发器、唯一索引内联检查先于此触发，surfaced 拒绝者=新约束；
//!    新约束作用域=精确 (object_id, revision_no)（无 tenant/project 列）由目录
//!    断言逐字承载；pc 侧 (tenant_id, project_id, object_id) 有复合外键（0001），
//!    跨域构造不可行，同域重复探针＋目录断言承载。

use sqlx::{Executor, PgPool, Postgres, Row};
use uuid::Uuid;

async fn seed_tenant_project(pool: &PgPool, tag: &str) -> (Uuid, Uuid) {
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('system', $1, $1) RETURNING id",
    )
    .bind(format!("b1-{tag}-{}", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .unwrap();
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(format!("b1-c-{tag}"))
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    let project: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, $2, $3) RETURNING id",
    )
    .bind(customer)
    .bind(format!("b1-{tag}"))
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    (tenant, project)
}

/// projc 探针全链播种（0037 GAP-38-B 起 projc 有 79 条 FK：cognition_revision
/// 的 document/document_revision/object 侧引用、document_revision.created_by、
/// cognition_version 的 confirmed_by/source_proposal_id、version_member 的
/// version/object 端点都必须指向真实行）。一次播种全链，探针复用其 id。
/// 播种行不含 cognition_revision / version_member（零残留断言面），二者由
/// 探针在各自事务内插入后 ROLLBACK。
struct ProjcSeed {
    tenant: Uuid,
    project: Uuid,
    principal: Uuid,
    document_id: Uuid,
    document_revision_id: Uuid,
    object_id: Uuid,
    proposal_id: Uuid,
    version_id: Uuid,
}

async fn seed_projc_chain(pool: &PgPool, tag: &str) -> ProjcSeed {
    let (tenant, project) = seed_tenant_project(pool, tag).await;
    let principal: Uuid = sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name, is_active) \
         VALUES ('human', $1, true) RETURNING id",
    )
    .bind(format!("b1-p-{tag}"))
    .fetch_one(pool)
    .await
    .unwrap();
    let document_id: Uuid = sqlx::query_scalar(
        "INSERT INTO projc.document (tenant_id, project_id, doc_key, canonical_path) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(format!("{tag}-doc-{}", Uuid::new_v4().simple()))
    .bind(format!("{tag}/doc/{}.md", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .unwrap();
    let document_revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO projc.document_revision \
         (tenant_id, project_id, document_id, revision_no, content_hash, content, created_by) \
         VALUES ($1, $2, $3, 1, 'sha256:x', '# x', $4) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(document_id)
    .bind(principal)
    .fetch_one(pool)
    .await
    .unwrap();
    let object_id: Uuid = sqlx::query_scalar(
        "INSERT INTO projc.cognition_object \
         (tenant_id, project_id, object_type, object_key, display_name, created_by) \
         VALUES ($1, $2, 'tech_stack', $3, $3, $4) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(format!("{tag}-obj-{}", Uuid::new_v4().simple()))
    .bind(principal)
    .fetch_one(pool)
    .await
    .unwrap();
    let proposal_id: Uuid = sqlx::query_scalar(
        "INSERT INTO projc.proposal \
         (tenant_id, project_id, title, rationale, origin_kind, governance_level, \
          expected_version_no, proposal_content_hash, created_by) \
         VALUES ($1, $2, $3, 'seed', 'manual_input', 'L1', 1, 'sha256:p', $4) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(format!("b1-proposal-{tag}"))
    .bind(principal)
    .fetch_one(pool)
    .await
    .unwrap();
    let version_id: Uuid = sqlx::query_scalar(
        "INSERT INTO projc.cognition_version \
         (tenant_id, project_id, version_no, confirmed_by, source_proposal_id, snapshot_hash) \
         VALUES ($1, $2, 1, $3, $4, 'sha256:v') RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(principal)
    .bind(proposal_id)
    .fetch_one(pool)
    .await
    .unwrap();
    ProjcSeed {
        tenant,
        project,
        principal,
        document_id,
        document_revision_id,
        object_id,
        proposal_id,
        version_id,
    }
}

/// projc.cognition_revision 最小合法行（0037 后 FK 引用面必须真实：
/// document/document_revision/object 侧与 created_by 全部复用全链播种 id；
/// CHECK revision_no>=1 由值满足）。接受连接/池/事务执行器，便于探针在
/// 事务内 seed。
async fn insert_projc_revision<'e, E>(executor: E, seed: &ProjcSeed, revision_no: i32) -> Uuid
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_scalar(
        "INSERT INTO projc.cognition_revision \
         (tenant_id, project_id, object_id, revision_no, document_id, document_revision_id, \
          content_hash, change_summary, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, 'sha256:x', 'r', $7) RETURNING id",
    )
    .bind(seed.tenant)
    .bind(seed.project)
    .bind(seed.object_id)
    .bind(revision_no)
    .bind(seed.document_id)
    .bind(seed.document_revision_id)
    .bind(seed.principal)
    .fetch_one(executor)
    .await
    .unwrap()
}

/// projc.version_member 最小合法行（对象对满足 version_member_check；端点
/// 闭合 CONSTRAINT 触发器 DEFERRABLE INITIALLY DEFERRED——探针事务
/// ROLLBACK，永不触发。0037 后 version 端点=全链播种行、object 端点=同
/// 事务内已插入的 cognition_revision 行）。
async fn insert_projc_member<'e, E>(
    executor: E,
    seed: &ProjcSeed,
    object_revision_id: Uuid,
) where E: Executor<'e, Database = Postgres> {
    sqlx::query(
        "INSERT INTO projc.version_member \
         (tenant_id, project_id, version_id, object_id, object_revision_id) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(seed.tenant)
    .bind(seed.project)
    .bind(seed.version_id)
    .bind(seed.object_id)
    .bind(object_revision_id)
    .execute(executor)
    .await
    .unwrap();
}

// ══════════════════════════════════════════════════════════════════════════
// T-1：GAP-37 探针（A=cognition_revision / B=version_member 各自独立断言）
// ══════════════════════════════════════════════════════════════════════════

#[sqlx::test(migrations = "../../migrations")]
async fn t1_gap37_update_delete_probes(pool: PgPool) {
    let seed = seed_projc_chain(&pool, "probe").await;

    // ── A1：UPDATE 普通字段（含 INSERT 成功断言=T-5）──
    {
        let mut tx = pool.begin().await.unwrap();
        let rev = insert_projc_revision(&mut *tx, &seed, 1).await;
        let err = sqlx::query(
            "UPDATE projc.cognition_revision SET change_summary = 'x' WHERE id = $1",
        )
        .bind(rev)
        .execute(&mut *tx)
        .await
        .unwrap_err();
        let db = err.as_database_error().unwrap();
        assert!(
            db.message().contains("in projc.cognition_revision is immutable"),
            "UPDATE 普通字段必须被 trg_projc_revision_immutable 拒绝，got: {db}"
        );
        tx.rollback().await.unwrap();
    }
    // ── A2：DELETE（独立事务）──
    {
        let mut tx = pool.begin().await.unwrap();
        let rev = insert_projc_revision(&mut *tx, &seed, 2).await;
        let err = sqlx::query("DELETE FROM projc.cognition_revision WHERE id = $1")
            .bind(rev)
            .execute(&mut *tx)
            .await
            .unwrap_err();
        let db = err.as_database_error().unwrap();
        assert!(
            db.message().contains("in projc.cognition_revision is immutable"),
            "DELETE 必须被 trg_projc_revision_immutable 拒绝，got: {db}"
        );
        tx.rollback().await.unwrap();
    }

    // ── B1：version_member UPDATE（含 INSERT 成功断言=T-5）──
    {
        let mut tx = pool.begin().await.unwrap();
        let rev = insert_projc_revision(&mut *tx, &seed, 1).await;
        insert_projc_member(&mut *tx, &seed, rev).await;
        let err = sqlx::query(
            "UPDATE projc.version_member SET object_revision_id = object_revision_id",
        )
        .execute(&mut *tx)
        .await
        .unwrap_err();
        let db = err.as_database_error().unwrap();
        assert!(
            db.message().contains("in projc.version_member is immutable"),
            "UPDATE 必须被 trg_projc_vm_immutable 拒绝，got: {db}"
        );
        tx.rollback().await.unwrap();
    }
    // ── B2：version_member DELETE（独立事务）──
    {
        let mut tx = pool.begin().await.unwrap();
        let rev = insert_projc_revision(&mut *tx, &seed, 2).await;
        insert_projc_member(&mut *tx, &seed, rev).await;
        let err = sqlx::query("DELETE FROM projc.version_member WHERE project_id = $1")
            .bind(seed.project)
            .execute(&mut *tx)
            .await
            .unwrap_err();
        let db = err.as_database_error().unwrap();
        assert!(
            db.message().contains("in projc.version_member is immutable"),
            "DELETE 必须被 trg_projc_vm_immutable 拒绝，got: {db}"
        );
        tx.rollback().await.unwrap();
    }

    // ── 零残留：全部探针事务已 ROLLBACK ──
    let revs: i64 = sqlx::query_scalar("SELECT count(*) FROM projc.cognition_revision")
        .fetch_one(&pool)
        .await
        .unwrap();
    let members: i64 = sqlx::query_scalar("SELECT count(*) FROM projc.version_member")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((revs, members), (0, 0), "探针事务 ROLLBACK 零残留");
}

// ══════════════════════════════════════════════════════════════════════════
// T-2：镜像保护清单测试（六维 + 完备性反向枚举；基准=§4.5 V1.3）
// ══════════════════════════════════════════════════════════════════════════

/// 基准表：任务书 §4.5（V1.3 定向修订后）。字符串=pg_get_triggerdef 规范
/// 输出（2026-11-17 活库导出逐字；事件序为 PG 规范序 DELETE OR UPDATE）。
/// 分组注记：[A]=两域一致 immutable/identity 保护面；[B]=函数有意分歧注记；
/// [C]=closure/anchored 校验面（CONSTRAINT，登记不测拒绝行为）；
/// [D]=pc 独有例外登记（断言 pc 有/projc 无）。
/// [V1.3]=本批定向修订补入的 cognition_version 行（两侧既有，非本批新增）。
const PC_TRIGGERS: &[(&str, &str)] = &[
    // ── [A] immutable/identity 保护面 ──
    (
        "trg_pc_approval_immutable",
        "CREATE TRIGGER trg_pc_approval_immutable BEFORE UPDATE ON pc.approval FOR EACH ROW EXECUTE FUNCTION pc.forbid_approval_mutation()",
    ),
    (
        "trg_pc_object_delete",
        "CREATE TRIGGER trg_pc_object_delete BEFORE DELETE ON pc.cognition_object FOR EACH ROW EXECUTE FUNCTION pc.forbid_identity_delete()",
    ),
    (
        "trg_pc_object_lifecycle",
        "CREATE TRIGGER trg_pc_object_lifecycle BEFORE UPDATE ON pc.cognition_object FOR EACH ROW EXECUTE FUNCTION pc.enforce_object_lifecycle()",
    ),
    (
        "trg_pc_revision_immutable",
        "CREATE TRIGGER trg_pc_revision_immutable BEFORE DELETE OR UPDATE ON pc.cognition_revision FOR EACH ROW EXECUTE FUNCTION pc.forbid_row_update()",
    ),
    (
        "trg_pc_vm_immutable",
        "CREATE TRIGGER trg_pc_vm_immutable BEFORE DELETE OR UPDATE ON pc.version_member FOR EACH ROW EXECUTE FUNCTION pc.forbid_row_update()",
    ),
    (
        "trg_pc_proposal_anchors",
        "CREATE TRIGGER trg_pc_proposal_anchors BEFORE UPDATE ON pc.proposal FOR EACH ROW EXECUTE FUNCTION pc.forbid_proposal_content_mutation()",
    ),
    (
        "trg_pc_write_op_state",
        "CREATE TRIGGER trg_pc_write_op_state BEFORE UPDATE ON pc.write_operation FOR EACH ROW EXECUTE FUNCTION pc.enforce_write_op_state()",
    ),
    // ── [B] 两域同位但函数有意分歧（0009 FTS 演进：pc 允许 FTS 列更新，
    //        projc 无 FTS 全拒——设计内分歧，非缺位；注记差异本身）──
    (
        "trg_pc_document_revision_guard",
        "CREATE TRIGGER trg_pc_document_revision_guard BEFORE DELETE OR UPDATE ON pc.document_revision FOR EACH ROW EXECUTE FUNCTION pc.guard_document_revision()",
    ),
    // ── [C] closure/anchored 校验面（CONSTRAINT 触发器，非保护面）──
    (
        "trg_pc_vm_closure",
        "CREATE CONSTRAINT TRIGGER trg_pc_vm_closure AFTER INSERT ON pc.version_member DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION pc.enforce_member_endpoint_closure()",
    ),
    (
        "trg_pc_pm_anchored",
        "CREATE CONSTRAINT TRIGGER trg_pc_pm_anchored AFTER DELETE OR UPDATE ON pc.proposal_member DEFERRABLE INITIALLY IMMEDIATE FOR EACH ROW EXECUTE FUNCTION pc.enforce_proposal_member_anchored()",
    ),
    (
        "trg_pc_pm_anchored_ins",
        "CREATE CONSTRAINT TRIGGER trg_pc_pm_anchored_ins AFTER INSERT ON pc.proposal_member DEFERRABLE INITIALLY IMMEDIATE FOR EACH ROW EXECUTE FUNCTION pc.enforce_proposal_member_anchored()",
    ),
    // ── [A] version_delete（GAP-37-C：Batch6 前置批升级为 A 组两域一致）──
    (
        "trg_pc_version_delete",
        "CREATE TRIGGER trg_pc_version_delete BEFORE DELETE ON pc.cognition_version FOR EACH ROW EXECUTE FUNCTION pc.forbid_row_update()",
    ),
    // ── [A][GAP-06] proposal 状态转换 DB 守卫（0034，两域一致）──
    (
        "trg_pc_proposal_transition",
        "CREATE TRIGGER trg_pc_proposal_transition BEFORE UPDATE ON pc.proposal FOR EACH ROW EXECUTE FUNCTION pc.guard_proposal_transition()",
    ),
    // ── [D] pc 独有例外登记行（projc 断言不存在，由完备性反向枚举承载）──
    // D-3 已裁：暂不补齐；未来开放关系修订/修改/删除路径前强制前置。
    (
        "trg_pc_rel_revision_endpoint_match",
        "CREATE TRIGGER trg_pc_rel_revision_endpoint_match BEFORE INSERT ON pc.relationship_revision FOR EACH ROW EXECUTE FUNCTION pc.enforce_rel_revision_endpoint_match()",
    ),
    (
        "trg_pc_rel_revision_immutable",
        "CREATE TRIGGER trg_pc_rel_revision_immutable BEFORE DELETE OR UPDATE ON pc.relationship_revision FOR EACH ROW EXECUTE FUNCTION pc.forbid_row_update()",
    ),
    (
        "trg_pc_relationship_anchors",
        "CREATE TRIGGER trg_pc_relationship_anchors BEFORE UPDATE ON pc.relationship FOR EACH ROW EXECUTE FUNCTION pc.enforce_relationship_anchors()",
    ),
    (
        "trg_pc_relationship_delete",
        "CREATE TRIGGER trg_pc_relationship_delete BEFORE DELETE ON pc.relationship FOR EACH ROW EXECUTE FUNCTION pc.forbid_identity_delete()",
    ),
    // ── [A][V1.3] cognition_version UPDATE 守卫镜像对（两侧既有；V1.2 漏列，
    //        2026-11-17 负责人批准定向修订补入）──
    (
        "trg_pc_version_immutable",
        "CREATE TRIGGER trg_pc_version_immutable BEFORE UPDATE ON pc.cognition_version FOR EACH ROW EXECUTE FUNCTION pc.forbid_version_mutation()",
    ),
];

const PROJC_TRIGGERS: &[(&str, &str)] = &[
    // ── [A] immutable/identity 保护面 ──
    (
        "trg_projc_approval_immutable",
        "CREATE TRIGGER trg_projc_approval_immutable BEFORE UPDATE ON projc.approval FOR EACH ROW EXECUTE FUNCTION projc.forbid_approval_mutation()",
    ),
    (
        "trg_projc_object_delete",
        "CREATE TRIGGER trg_projc_object_delete BEFORE DELETE ON projc.cognition_object FOR EACH ROW EXECUTE FUNCTION projc.forbid_identity_delete()",
    ),
    (
        "trg_projc_object_lifecycle",
        "CREATE TRIGGER trg_projc_object_lifecycle BEFORE UPDATE ON projc.cognition_object FOR EACH ROW EXECUTE FUNCTION projc.enforce_object_lifecycle()",
    ),
    // 本批新增（GAP-37-A，0030）。
    (
        "trg_projc_revision_immutable",
        "CREATE TRIGGER trg_projc_revision_immutable BEFORE DELETE OR UPDATE ON projc.cognition_revision FOR EACH ROW EXECUTE FUNCTION projc.forbid_row_update()",
    ),
    // 本批新增（GAP-37-B，0030）。
    (
        "trg_projc_vm_immutable",
        "CREATE TRIGGER trg_projc_vm_immutable BEFORE DELETE OR UPDATE ON projc.version_member FOR EACH ROW EXECUTE FUNCTION projc.forbid_row_update()",
    ),
    (
        "trg_projc_proposal_anchors",
        "CREATE TRIGGER trg_projc_proposal_anchors BEFORE UPDATE ON projc.proposal FOR EACH ROW EXECUTE FUNCTION projc.forbid_proposal_content_mutation()",
    ),
    (
        "trg_projc_write_op_state",
        "CREATE TRIGGER trg_projc_write_op_state BEFORE UPDATE ON projc.write_operation FOR EACH ROW EXECUTE FUNCTION projc.enforce_write_op_state()",
    ),
    // ── [B] 函数有意分歧注记（projc 无 FTS：forbid_row_update 全拒）──
    (
        "trg_projc_document_revision_immutable",
        "CREATE TRIGGER trg_projc_document_revision_immutable BEFORE DELETE OR UPDATE ON projc.document_revision FOR EACH ROW EXECUTE FUNCTION projc.forbid_row_update()",
    ),
    // ── [C] closure/anchored 校验面（0026 重绑/新增）──
    (
        "trg_projc_vm_closure",
        "CREATE CONSTRAINT TRIGGER trg_projc_vm_closure AFTER INSERT ON projc.version_member DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION projc.enforce_member_endpoint_closure()",
    ),
    (
        "trg_projc_proposal_member_anchored",
        "CREATE CONSTRAINT TRIGGER trg_projc_proposal_member_anchored AFTER DELETE OR UPDATE ON projc.proposal_member DEFERRABLE INITIALLY IMMEDIATE FOR EACH ROW EXECUTE FUNCTION projc.enforce_proposal_member_anchored()",
    ),
    (
        "trg_projc_proposal_member_anchored_ins",
        "CREATE CONSTRAINT TRIGGER trg_projc_proposal_member_anchored_ins AFTER INSERT ON projc.proposal_member DEFERRABLE INITIALLY IMMEDIATE FOR EACH ROW EXECUTE FUNCTION projc.enforce_proposal_member_anchored()",
    ),
    // ── [A][V1.3] cognition_version UPDATE 守卫镜像对（既有）──
    (
        "trg_projc_cognition_version_immutable",
        "CREATE TRIGGER trg_projc_cognition_version_immutable BEFORE UPDATE ON projc.cognition_version FOR EACH ROW EXECUTE FUNCTION projc.forbid_version_mutation()",
    ),
    // ── [A] version_delete（GAP-37-C：Batch6 前置批 0035 新增）──
    (
        "trg_projc_version_delete",
        "CREATE TRIGGER trg_projc_version_delete BEFORE DELETE ON projc.cognition_version FOR EACH ROW EXECUTE FUNCTION projc.forbid_row_update()",
    ),
    // ── [A][GAP-06] proposal 状态转换 DB 守卫（0034，两域一致）──
    (
        "trg_projc_proposal_transition",
        "CREATE TRIGGER trg_projc_proposal_transition BEFORE UPDATE ON projc.proposal FOR EACH ROW EXECUTE FUNCTION projc.guard_proposal_transition()",
    ),
];

async fn trigger_defs(pool: &PgPool, schema: &str) -> Vec<(String, String)> {
    sqlx::query_as(
        "SELECT t.tgname, pg_get_triggerdef(t.oid) \
         FROM pg_trigger t \
         JOIN pg_class c ON c.oid = t.tgrelid \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = $1 AND NOT t.tgisinternal \
         ORDER BY t.tgname",
    )
    .bind(schema)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn t2_mirror_guard_inventory(pool: PgPool) {
    for (schema, expected) in [("pc", PC_TRIGGERS), ("projc", PROJC_TRIGGERS)] {
        let mut actual = trigger_defs(&pool, schema).await;
        actual.sort_by(|a, b| a.0.cmp(&b.0));

        let mut expected_sorted: Vec<(String, String)> = expected
            .iter()
            .map(|(n, d)| (n.to_string(), d.to_string()))
            .collect();
        expected_sorted.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(
            actual.len(),
            expected_sorted.len(),
            "{schema} 触发器总数漂移（完备性反向枚举：清单外新增/删减未同步清单即失败）"
        );
        for (idx, (name, def)) in expected_sorted.iter().enumerate() {
            assert_eq!(
                actual[idx].0, *name,
                "{schema} 第 {idx} 个触发器名漂移：期望 {name}，实际 {}",
                actual[idx].0
            );
            assert_eq!(
                actual[idx].1, *def,
                "{schema}.{name} 六维定义漂移（触发器名/目标表/事件/时机+约束属性/行级/执行函数）:\n  期望: {def}\n  实际: {}",
                actual[idx].1
            );
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════
// T-3：GAP-04 探针（同 (object_id, revision_no) 二次 INSERT 被拒；两域各一）
// ══════════════════════════════════════════════════════════════════════════

#[sqlx::test(migrations = "../../migrations")]
async fn t3_gap04_revision_no_unique_probe(pool: PgPool) {
    // ── 目录断言：两域新约束在位且定义逐字 ──
    for schema in ["pc", "projc"] {
        let (name, def): (String, String) = sqlx::query_as(
            "SELECT con.conname, pg_get_constraintdef(con.oid) \
             FROM pg_constraint con \
             JOIN pg_class c ON c.oid = con.conrelid \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE n.nspname = $1 AND c.relname = 'cognition_revision' \
               AND con.conname = 'cognition_revision_object_id_revision_no_key'",
        )
        .bind(schema)
        .fetch_one(&pool)
        .await
        .unwrap_or_else(|_| panic!("{schema} 缺 cognition_revision_object_id_revision_no_key"));
        assert_eq!(name, "cognition_revision_object_id_revision_no_key");
        assert_eq!(def, "UNIQUE (object_id, revision_no)");
    }

    // ── pc 探针：同域重复（(tenant,project,object) 复合外键使跨域构造不可行；
    //    拒绝由唯一约束族承载，新约束在位由上方目录断言证明）──
    {
        let (tenant, project) = seed_tenant_project(&pool, "pc").await;
        let author: Uuid = sqlx::query_scalar(
            "INSERT INTO core.principal (kind, display_name, is_active) \
             VALUES ('human', 'b1-pc-author', true) RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let object_id: Uuid = sqlx::query_scalar(
            "INSERT INTO pc.cognition_object \
             (tenant_id, project_id, object_type, object_key, display_name, created_by) \
             VALUES ($1, $2, 'feature', $3, 'b1', $4) RETURNING id",
        )
        .bind(tenant)
        .bind(project)
        .bind(format!("b1-feat-{}", Uuid::new_v4().simple()))
        .bind(author)
        .fetch_one(&pool)
        .await
        .unwrap();
        let document_id: Uuid = sqlx::query_scalar(
            "INSERT INTO pc.document (tenant_id, project_id, doc_key, canonical_path) \
             VALUES ($1, $2, $3, $4) RETURNING id",
        )
        .bind(tenant)
        .bind(project)
        .bind(format!("pc-object-{object_id}"))
        .bind(format!("pc/feature/{object_id}.md"))
        .fetch_one(&pool)
        .await
        .unwrap();
        let document_revision_id: Uuid = sqlx::query_scalar(
            "INSERT INTO pc.document_revision \
             (tenant_id, project_id, document_id, revision_no, content_hash, content, created_by) \
             VALUES ($1, $2, $3, 1, 'sha256:x', '# x', $4) RETURNING id",
        )
        .bind(tenant)
        .bind(project)
        .bind(document_id)
        .bind(author)
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO pc.cognition_revision \
             (tenant_id, project_id, object_id, revision_no, document_id, document_revision_id, \
              content_hash, change_summary, created_by) \
             VALUES ($1, $2, $3, 1, $4, $5, 'sha256:x', 'r', $6) RETURNING id",
        )
        .bind(tenant)
        .bind(project)
        .bind(object_id)
        .bind(document_id)
        .bind(document_revision_id)
        .bind(author)
        .fetch_one(&pool)
        .await
        .expect("首 INSERT 必须成功（GAP-04 不影响正常写入）");
        let dup = sqlx::query(
            "INSERT INTO pc.cognition_revision \
             (tenant_id, project_id, object_id, revision_no, document_id, document_revision_id, \
              content_hash, change_summary, created_by) \
             VALUES ($1, $2, $3, 1, $4, $5, 'sha256:y', 'dup', $6)",
        )
        .bind(tenant)
        .bind(project)
        .bind(object_id)
        .bind(document_id)
        .bind(document_revision_id)
        .bind(author)
        .execute(&pool)
        .await;
        assert!(dup.is_err(), "pc 同 (object_id, revision_no) 二次 INSERT 必须被拒");
    }

    // ── projc 探针一：同域同 (object_id, revision_no) 重复 → 仅新约束可拒 ──
    //    dup 复用全链播种 id（0037 后外键面必须真实），唯一违规者=新约束本身 ──
    {
        let seed = seed_projc_chain(&pool, "pj").await;
        insert_projc_revision(&pool, &seed, 1).await;
        let dup = sqlx::query(
            "INSERT INTO projc.cognition_revision \
             (tenant_id, project_id, object_id, revision_no, document_id, document_revision_id, \
              content_hash, change_summary, created_by) \
             VALUES ($1, $2, $3, 1, $4, $5, 'sha256:y', 'dup', $6)",
        )
        .bind(seed.tenant)
        .bind(seed.project)
        .bind(seed.object_id)
        .bind(seed.document_id)
        .bind(seed.document_revision_id)
        .bind(seed.principal)
        .execute(&pool)
        .await
        .unwrap_err();
        let db = dup.as_database_error().unwrap();
        assert_eq!(
            db.constraint(),
            Some("cognition_revision_object_id_revision_no_key"),
            "dup 引用行全合法——拒绝者必为 (object_id, revision_no) 新唯一约束，got: {db}"
        );
    }

    // ── projc 探针二：跨 tenant/project 重复——新约束作用域不含 tenant/project
    //    列，跨域组合同样被拒。0037 后 (tenant_id, project_id, object_id) 复合
    //    外键对该组合另有覆盖（cognition_object PK=id 全局唯一，跨域同 id 行
    //    不可构造），但外键拒绝走 AFTER ROW 触发器、唯一索引内联检查先于此
    //    触发——surfaced 拒绝者=新约束。b 域播种合法 document/document_revision/
    //    principal 复用，排除其余外键噪声 ──
    {
        let seed_a = seed_projc_chain(&pool, "pja").await;
        let seed_b = seed_projc_chain(&pool, "pjb").await;
        insert_projc_revision(&pool, &seed_a, 1).await;
        let dup = sqlx::query(
            "INSERT INTO projc.cognition_revision \
             (tenant_id, project_id, object_id, revision_no, document_id, document_revision_id, \
              content_hash, change_summary, created_by) \
             VALUES ($1, $2, $3, 1, $4, $5, 'sha256:z', 'cross', $6)",
        )
        .bind(seed_b.tenant)
        .bind(seed_b.project)
        .bind(seed_a.object_id)
        .bind(seed_b.document_id)
        .bind(seed_b.document_revision_id)
        .bind(seed_b.principal)
        .execute(&pool)
        .await
        .unwrap_err();
        let db = dup.as_database_error().unwrap();
        assert_eq!(
            db.constraint(),
            Some("cognition_revision_object_id_revision_no_key"),
            "跨域重复 surfaced 拒绝者=新唯一约束（唯一索引内联检查先于 AFTER ROW 外键触发），got: {db}"
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════
// Batch6 前置批：GAP-37-C DELETE 探针 + GAP-38-A 主键/业务身份目录断言
// ══════════════════════════════════════════════════════════════════════════

#[sqlx::test(migrations = "../../migrations")]
async fn t4_gap37c_delete_probe_and_gap38a_catalog(pool: PgPool) {
    // ── GAP-37-C：DELETE projc.cognition_version 必须被拒（INSERT 不受影响）──
    //    0037 后 confirmed_by→principal、source_proposal_id→proposal 必须真实，
    //    复用全链播种 id；version_no=2 避开播种行 (tenant,project,version_no)
    //    唯一约束 ──
    let seed = seed_projc_chain(&pool, "g37c").await;
    {
        let mut tx = pool.begin().await.unwrap();
        sqlx::query(
            "INSERT INTO projc.cognition_version \
             (tenant_id, project_id, version_no, confirmed_by, source_proposal_id, snapshot_hash) \
             VALUES ($1, $2, 2, $3, $4, 'sha256:probe')",
        )
        .bind(seed.tenant)
        .bind(seed.project)
        .bind(seed.principal)
        .bind(seed.proposal_id)
        .execute(&mut *tx)
        .await
        .expect("INSERT 不受 0035 影响");
        let err = sqlx::query("DELETE FROM projc.cognition_version WHERE project_id = $1")
            .bind(seed.project)
            .execute(&mut *tx)
            .await
            .unwrap_err();
        let db = err.as_database_error().unwrap();
        assert!(
            db.message().contains("in projc.cognition_version is immutable"),
            "DELETE 必须被 trg_projc_version_delete 拒绝，got: {db}"
        );
        tx.rollback().await.unwrap();
    }

    // ── GAP-38-A：18 PK + 7 业务身份 UNIQUE 目录断言（名逐字）──
    let pks: Vec<String> = sqlx::query(
        "SELECT c.relname || '.' || con.conname FROM pg_constraint con \
           JOIN pg_class c ON c.oid = con.conrelid \
           JOIN pg_namespace n ON n.oid = c.relnamespace \
          WHERE n.nspname = 'projc' AND con.contype = 'p' ORDER BY 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap()
    .into_iter()
    .map(|r| r.get::<String, _>(0))
    .collect();
    assert_eq!(pks.len(), 18, "projc 18 表全 PK");
    for expected in [
        "approval.approval_pkey",
        "cognition_object.cognition_object_pkey",
        "cognition_revision.cognition_revision_pkey",
        "cognition_version.cognition_version_pkey",
        "conflict.conflict_pkey",
        "conflict_object.conflict_object_pkey",
        "document.document_pkey",
        "document_revision.document_revision_pkey",
        "evidence.evidence_pkey",
        "object_type.object_type_pkey",
        "proposal.proposal_pkey",
        "proposal_member.proposal_member_pkey",
        "relationship.relationship_pkey",
        "relationship_revision.relationship_revision_pkey",
        "relationship_type.relationship_type_pkey",
        "source.source_pkey",
        "version_member.version_member_pkey",
        "write_operation.write_operation_pkey",
    ] {
        assert!(pks.contains(&expected.to_string()), "缺主键 {expected}");
    }

    let uqs: Vec<String> = sqlx::query(
        "SELECT con.conname FROM pg_constraint con \
           JOIN pg_class c ON c.oid = con.conrelid \
           JOIN pg_namespace n ON n.oid = c.relnamespace \
          WHERE n.nspname = 'projc' AND con.contype = 'u' ORDER BY 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap()
    .into_iter()
    .map(|r| r.get::<String, _>(0))
    .collect();
    for expected in [
        "cognition_version_tenant_id_project_id_version_no_key",
        "document_tenant_id_project_id_doc_key_key",
        "document_revision_tenant_id_project_id_document_id_revision_key",
        "relationship_revision_tenant_id_project_id_relationship_id__key",
        "cognition_object_tenant_id_project_id_object_key_key",
        "cognition_revision_object_id_revision_no_key",
        "write_operation_tenant_id_project_id_operation_id_key",
    ] {
        assert!(uqs.contains(&expected.to_string()), "缺业务身份唯一约束 {expected}");
    }
    assert_eq!(
        uqs.len(),
        19,
        "projc UNIQUE 总数=7 业务身份（0001/0025/0036）+12 FK 引用面（0037 GAP-38-B）"
    );
}
