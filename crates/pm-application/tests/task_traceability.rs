//! PA07-IMP-007：审计与历史追溯闭环（实施规范 §10 P0-007）。
//!
//! 覆盖：traceability 聚合六段（Revision 链/依赖边/Run 门禁快照/Product 归属
//! 含撤回历史/验收历史/挂任务咨询）；Product↔Project Link 解除改软删（历史
//! 保留 + 撤回后可重建）；已撤回 Link 不再出现在有效列表。

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::human_consultation::create_consultation;
use pm_application::product::{create_product_link, end_product_link};
use pm_application::actor::AuditActor;
use pm_application::task_products::{
    add_task_product, record_product_acceptance, ProductAcceptanceInput,
};
use pm_application::task_traceability::task_traceability;
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

struct Fx {
    project_id: Uuid,
    actor_id: Uuid,
}

async fn fx(pool: &PgPool, code: &str) -> Fx {
    let o = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    Fx {
        project_id: o.project_id,
        actor_id: o.human_id,
    }
}

fn actor(fx: &Fx) -> AuditActor {
    AuditActor {
        principal_id: fx.actor_id,
        identity_id: None,
        client: None,
        auth_mode: None,
        claimed_device_id: None,
    }
}

async fn seed_task(pool: &PgPool, f: &Fx) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO delivery.task (project_id, title, status, revision, tenant_id) \
         VALUES ($1, 't', 'ready', 1, (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(f.project_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Product（归属新 Team）+ task-product 归属（复用 run_start_gates 的 CTE 模式）。
async fn seed_task_product(pool: &PgPool, f: &Fx, task_id: Uuid, name: &str) -> Uuid {
    let product_id: Uuid = sqlx::query_scalar(
        "WITH t AS ( \
             INSERT INTO core.team (tenant_id, name) \
             SELECT tenant_id, $1 || '-team' FROM delivery.task WHERE id = $2 RETURNING tenant_id, id \
         ), p AS ( \
             INSERT INTO core.product (team_id, tenant_id, key, name, status) \
             SELECT id, tenant_id, 'P-' || upper(substring(gen_random_uuid()::text from 1 for 8)), 'normal' FROM t RETURNING id \
         ) SELECT id FROM p",
    )
    .bind(name)
    .bind(task_id)
    .fetch_one(pool)
    .await
    .unwrap();
    add_task_product(pool, SYSTEM_TENANT, task_id, product_id, None, &actor(f))
        .await
        .unwrap();
    product_id
}

/// 全链场景 → traceability 六段全部可见且内容正确。
#[sqlx::test(migrations = "../../migrations")]
async fn traceability_aggregates_all_six_segments(pool: PgPool) {
    let f = fx(&pool, "TR-1").await;
    let task = seed_task(&pool, &f).await;

    // ① Revision 链（seed 未走 create_task，直种快照）。
    let rev1: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_revision (tenant_id, project_id, task_id, revision_no, change_reason) \
         SELECT tenant_id, project_id, $1, 1, 'initial' FROM delivery.task WHERE id = $1 RETURNING id",
    )
    .bind(task)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE delivery.task SET current_task_revision_id = $2 WHERE id = $1")
        .bind(task)
        .bind(rev1)
        .execute(&pool)
        .await
        .unwrap();

    // ② 依赖边 + ③ 领取快照。
    let upstream = seed_task(&pool, &f).await;
    sqlx::query(
        "INSERT INTO delivery.task_dependency \
         (tenant_id, project_id, task_id, depends_on_task_id, created_by) \
         SELECT tenant_id, project_id, $1, $2, $3 FROM delivery.task WHERE id = $1",
    )
    .bind(task)
    .bind(upstream)
    .bind(f.actor_id)
    .execute(&pool)
    .await
    .unwrap();
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 1, 'queued', (SELECT tenant_id FROM core.project WHERE id = $1)) RETURNING id",
    )
    .bind(f.project_id)
    .bind(task)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_run_dependency_snapshot \
         (tenant_id, project_id, run_id, task_id, predecessor_task_id, \
          predecessor_revision_id, required_condition, satisfied) \
         SELECT tenant_id, project_id, $2, $1, $3, NULL, 'completed', TRUE \
           FROM delivery.task WHERE id = $1",
    )
    .bind(task)
    .bind(run_id)
    .bind(upstream)
    .execute(&pool)
    .await
    .unwrap();

    // ④ Product 归属 + ⑤ 验收 + ⑥ 咨询。
    let product = seed_task_product(&pool, &f, task, "trace-prod").await;
    record_product_acceptance(
        &pool,
        SYSTEM_TENANT,
        task,
        ProductAcceptanceInput {
            product_id: product,
            task_revision_id: None,
            result: "approved".to_string(),
            condition_scope: "product_specific".to_string(),
            evidence_ref: None,
            source: "human".to_string(),
            note: None,
        },
        Some(f.actor_id),
        &actor(&f),
    )
    .await
    .unwrap();
    let consultation = create_consultation(
        &pool,
        SYSTEM_TENANT,
        f.project_id,
        Some(task),
        "范围决策",
        "是否纳入关联模块？",
        serde_json::json!([]),
        None,
        f.actor_id,
        &actor(&f),
    )
    .await
    .unwrap();

    let trace = task_traceability(&pool, SYSTEM_TENANT, task).await.unwrap();
    assert_eq!(trace.revisions.len(), 1);
    assert_eq!(trace.revisions[0].id, rev1);
    assert_eq!(trace.revisions[0].change_reason.as_deref(), Some("initial"));
    assert_eq!(trace.dependencies.len(), 1);
    assert_eq!(trace.dependencies[0].depends_on_task_id, upstream);
    assert_eq!(trace.dependencies[0].required_condition, "completed");
    assert_eq!(trace.run_gate_snapshots.len(), 1);
    assert_eq!(trace.run_gate_snapshots[0].run_id, run_id);
    assert!(trace.run_gate_snapshots[0].satisfied);
    assert_eq!(trace.product_links.len(), 1);
    assert!(!trace.product_links[0].withdrawn);
    assert_eq!(trace.acceptances.len(), 1);
    assert_eq!(trace.acceptances[0].result, "approved");
    assert_eq!(trace.consultations.len(), 1);
    assert_eq!(trace.consultations[0].id, consultation);
    let _ = product;
}

/// Product↔Project Link：解除为软删（历史保留）+ 撤回后可重建新行 + 有效列表
/// 不含已撤回行。
#[sqlx::test(migrations = "../../migrations")]
async fn product_link_withdraw_is_soft_and_relinkable(pool: PgPool) {
    let f = fx(&pool, "TR-2").await;
    // 建 Product（归属 bootstrap team；P4-ORG 下 team_member owner 需要）。
    let product_id: Uuid = sqlx::query_scalar(
        "WITH t AS ( \
             INSERT INTO core.team (tenant_id, name) \
             SELECT tenant_id, 'link-team' FROM core.project WHERE id = $1 RETURNING tenant_id, id \
         ), p AS ( \
             INSERT INTO core.product (team_id, tenant_id, key, name, status) \
             SELECT id, tenant_id, 'link-prod', 'normal' FROM t RETURNING id, tenant_id \
         ), pm AS ( \
             INSERT INTO core.product_member (product_id, principal_id, tenant_id, role) \
             SELECT id, $2, tenant_id, 'owner' FROM p RETURNING product_id \
         ) SELECT pm.product_id FROM pm",
    )
    .bind(f.project_id)
    .bind(f.actor_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let link1 = create_product_link(&pool, &actor(&f), f.actor_id, product_id, f.project_id)
        .await
        .unwrap();
    // 重复建 → link_already_exists。
    let err = create_product_link(&pool, &actor(&f), f.actor_id, product_id, f.project_id)
        .await
        .unwrap_err();
    assert!(matches!(err, pm_application::product::ProductError::Conflict("link_already_exists")));

    // 解除 → 软删：行仍在且有 withdrawn_at。
    end_product_link(&pool, &actor(&f), f.actor_id, link1).await.unwrap();
    let (withdrawn_at, withdrawn_by): (Option<chrono::DateTime<chrono::Utc>>, Option<Uuid>) =
        sqlx::query_as(
            "SELECT withdrawn_at, withdrawn_by FROM core.product_project_link WHERE id = $1",
        )
        .bind(link1)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(withdrawn_at.is_some());
    assert_eq!(withdrawn_by, Some(f.actor_id));

    // 撤回后重建 → 新行成功（partial unique 只约束有效行）。
    let link2 = create_product_link(&pool, &actor(&f), f.actor_id, product_id, f.project_id)
        .await
        .unwrap();
    assert_ne!(link1, link2);

    // 有效列表只含新行。
    let links = pm_application::product::list_product_links(&pool, product_id, f.actor_id)
        .await
        .unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].id, link2);
}
