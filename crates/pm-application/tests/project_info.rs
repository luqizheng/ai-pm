//! P1 Project Settings · General 服务测试（project_info 读改 + 审计）。

use sqlx::PgPool;
use uuid::Uuid;

use pm_application::actor::AuditActor;
use pm_application::project_info::{
    get_project_info, update_project_info, ProjectInfoError, UpdateProjectInfoRequest,
};

async fn seed(pool: &PgPool, tag: &str) -> (Uuid, Uuid, Uuid) {
    let customer_id = Uuid::new_v4();
    sqlx::query("INSERT INTO core.customer (id, name, tenant_id) VALUES ($1, 'Acme', $2)")
        .bind(customer_id)
        .bind(pm_domain::SYSTEM_TENANT)
        .execute(pool)
        .await
        .unwrap();
    let project_id = Uuid::new_v4();
    let code = format!("PI-{}", project_id.simple().to_string()[..8].to_uppercase());
    sqlx::query(
        "INSERT INTO core.project (id, customer_id, code, name, description, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(project_id)
    .bind(customer_id)
    .bind(&code)
    .bind(format!("项目 {tag}"))
    .bind("原始描述")
    .bind(pm_domain::SYSTEM_TENANT)
    .execute(pool)
    .await
    .unwrap();
    let principal_id = Uuid::new_v4();
    sqlx::query("INSERT INTO core.principal (id, kind, display_name) VALUES ($1, 'human', 'PM')")
        .bind(principal_id)
        .execute(pool)
        .await
        .unwrap();
    (project_id, customer_id, principal_id)
}

#[sqlx::test(migrations = "../../migrations")]
async fn get_returns_customer_join_and_defaults(pool: PgPool) {
    let (project_id, _customer_id, _principal) = seed(&pool, "get").await;
    let info = get_project_info(&pool, pm_domain::SYSTEM_TENANT, project_id)
        .await
        .unwrap();
    assert_eq!(info.customer_name.as_deref(), Some("Acme"));
    assert_eq!(info.description.as_deref(), Some("原始描述"));
    assert_eq!(info.status, "active");

    let missing = get_project_info(&pool, pm_domain::SYSTEM_TENANT, Uuid::new_v4()).await;
    assert!(matches!(missing, Err(ProjectInfoError::NotFound)));
}

#[sqlx::test(migrations = "../../migrations")]
async fn update_changes_name_description_and_writes_audit(pool: PgPool) {
    let (project_id, _customer_id, principal_id) = seed(&pool, "update").await;
    let actor = AuditActor::principal(principal_id);

    let updated = update_project_info(
        &pool,
        &actor,
        pm_domain::SYSTEM_TENANT,
        project_id,
        UpdateProjectInfoRequest {
            name: "  新名字  ".to_string(),
            description: Some("  新描述  ".to_string()),
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.name, "新名字", "名称必须 trim");
    assert_eq!(updated.description.as_deref(), Some("新描述"));
    assert!(updated.revision > 1, "revision 必须推进");

    // 空描述 = 置空（None），空名称拒绝。
    let cleared = update_project_info(
        &pool,
        &actor,
        pm_domain::SYSTEM_TENANT,
        project_id,
        UpdateProjectInfoRequest {
            name: "名字".to_string(),
            description: Some("   ".to_string()),
        },
    )
    .await
    .unwrap();
    assert_eq!(cleared.description, None);

    let err = update_project_info(
        &pool,
        &actor,
        pm_domain::SYSTEM_TENANT,
        project_id,
        UpdateProjectInfoRequest {
            name: "  ".to_string(),
            description: None,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ProjectInfoError::Validation(_)), "{err:?}");

    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM governance.audit_log \
         WHERE project_id = $1 AND action = 'project.info_updated'",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 2, "两次成功更新各落一条审计");
}
