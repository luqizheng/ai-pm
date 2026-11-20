//! P4-ORG Phase 1 组织关系数据模型约束测试（基线 V0.1 §2；DR-ORG-01~05）。
//!
//! 纯 DB 层：role CHECK、唯一约束、FK、级联——不含业务流程。
//! 权限计算（Team→Product→Project 传播、三角色推导）归 Phase 3/4。

use sqlx::PgPool;
use uuid::Uuid;

async fn insert_tenant(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', gen_random_uuid()::text, 'Tenant A') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_principal(pool: &PgPool, kind: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ($1, 'P') RETURNING id",
    )
    .bind(kind)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_team(pool: &PgPool, tenant_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.team (tenant_id, name) VALUES ($1, 'Team A') RETURNING id",
    )
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_team_member(
    pool: &PgPool,
    team_id: Uuid,
    principal_id: Uuid,
    tenant_id: Uuid,
    role: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO core.team_member (team_id, principal_id, tenant_id, role, granted_by) \
         VALUES ($1, $2, $3, $4, $2)",
    )
    .bind(team_id)
    .bind(principal_id)
    .bind(tenant_id)
    .bind(role)
    .execute(pool)
    .await
    .map(|_| ())
}

async fn insert_customer(pool: &PgPool, tenant_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ('Customer', $1) RETURNING id",
    )
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_project(pool: &PgPool, tenant_id: Uuid, customer_id: Uuid, code: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, 'Project', $3) RETURNING id",
    )
    .bind(customer_id)
    .bind(code)
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_product(pool: &PgPool, team_id: Uuid, tenant_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.product (team_id, tenant_id, name) VALUES ($1, $2, 'Product A') \
         RETURNING id",
    )
    .bind(team_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 断言错误为数据库约束违规并返回错误文本（用于匹配约束名）。
fn violation(err: sqlx::Error) -> String {
    match err {
        sqlx::Error::Database(db) => db.message().to_string(),
        other => panic!("expected db error, got {other}"),
    }
}

/// ck_team_member_role：Team 角色只有 owner|member（基线 §3.2；DR-ORG-02①），
/// 不存在第三角色，人类 admin 不再是 Team 角色。
#[sqlx::test(migrations = "../../migrations")]
async fn team_member_role_is_owner_or_member_only(pool: PgPool) {
    let tenant = insert_tenant(&pool).await;
    let team = insert_team(&pool, tenant).await;
    let p = insert_principal(&pool, "human").await;

    let err = violation(
        insert_team_member(&pool, team, p, tenant, "admin")
            .await
            .expect_err("admin must violate ck_team_member_role"),
    );
    assert!(err.contains("ck_team_member_role"), "{err}");
}

/// 多 Owner 并存合法（基线 §3.2；DR-ORG-02①②）。
#[sqlx::test(migrations = "../../migrations")]
async fn multiple_team_owners_allowed(pool: PgPool) {
    let tenant = insert_tenant(&pool).await;
    let team = insert_team(&pool, tenant).await;
    let p1 = insert_principal(&pool, "human").await;
    let p2 = insert_principal(&pool, "human").await;

    insert_team_member(&pool, team, p1, tenant, "owner")
        .await
        .unwrap();
    insert_team_member(&pool, team, p2, tenant, "owner")
        .await
        .unwrap();
    let owners: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.team_member WHERE team_id = $1 AND role = 'owner'",
    )
    .bind(team)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(owners, 2);
}

/// (team_id, principal_id) 主键：同一 (team, principal) 只能有一行成员关系。
#[sqlx::test(migrations = "../../migrations")]
async fn team_member_pair_is_unique(pool: PgPool) {
    let tenant = insert_tenant(&pool).await;
    let team = insert_team(&pool, tenant).await;
    let p = insert_principal(&pool, "human").await;

    insert_team_member(&pool, team, p, tenant, "owner")
        .await
        .unwrap();
    let err = violation(
        insert_team_member(&pool, team, p, tenant, "member")
            .await
            .expect_err("duplicate pair must violate team_member_pkey"),
    );
    assert!(err.contains("team_member_pkey"), "{err}");
}

/// FK：team 归属必须存在的 tenant；成员行随 team 删除级联清理（卫生级联）。
#[sqlx::test(migrations = "../../migrations")]
async fn team_fk_and_member_cascade(pool: PgPool) {
    let tenant = insert_tenant(&pool).await;
    let orphan = Uuid::new_v4();
    let err = violation(
        sqlx::query("INSERT INTO core.team (tenant_id, name) VALUES ($1, 'Bad') RETURNING id")
            .bind(orphan)
            .fetch_one(&pool)
            .await
            .expect_err("bogus tenant must violate fk_team_tenant"),
    );
    assert!(err.contains("fk_team_tenant"), "{err}");

    let team = insert_team(&pool, tenant).await;
    let p = insert_principal(&pool, "human").await;
    insert_team_member(&pool, team, p, tenant, "member")
        .await
        .unwrap();
    sqlx::query("DELETE FROM core.team WHERE id = $1")
        .bind(team)
        .execute(&pool)
        .await
        .unwrap();
    let left: i64 =
        sqlx::query_scalar("SELECT count(*) FROM core.team_member WHERE team_id = $1")
            .bind(team)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(left, 0, "team_member must cascade with team");
}

/// fk_product_team：Product 归属唯一 Team（基线 §2.2；DR-ORG-03①）。
#[sqlx::test(migrations = "../../migrations")]
async fn product_requires_existing_team(pool: PgPool) {
    let tenant = insert_tenant(&pool).await;
    let err = violation(
        sqlx::query(
            "INSERT INTO core.product (team_id, tenant_id, name) VALUES ($1, $2, 'Bad')",
        )
        .bind(Uuid::new_v4())
        .bind(tenant)
        .execute(&pool)
        .await
        .expect_err("bogus team must violate fk_product_team"),
    );
    assert!(err.contains("fk_product_team"), "{err}");

    let team = insert_team(&pool, tenant).await;
    insert_product(&pool, team, tenant).await;
    insert_product(&pool, team, tenant).await;
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM core.product WHERE team_id = $1")
            .bind(team)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 2, "one team manages many products");
}

/// ck_product_project_link_pair：Product↔Project 多对多，同一对唯一（基线 §4.3）。
#[sqlx::test(migrations = "../../migrations")]
async fn product_project_link_is_many_to_many_unique_pair(pool: PgPool) {
    let tenant = insert_tenant(&pool).await;
    let team_a = insert_team(&pool, tenant).await;
    let team_b = insert_team(&pool, tenant).await;
    let product_a = insert_product(&pool, team_a, tenant).await;
    let product_b = insert_product(&pool, team_b, tenant).await;
    let customer = insert_customer(&pool, tenant).await;
    let project_x = insert_project(&pool, tenant, customer, "PRJ-X").await;
    let project_y = insert_project(&pool, tenant, customer, "PRJ-Y").await;

    for (product, project) in [
        (product_a, project_x),
        (product_b, project_x),
        (product_a, project_y),
    ] {
        sqlx::query(
            "INSERT INTO core.product_project_link (product_id, project_id, tenant_id) \
             VALUES ($1, $2, $3)",
        )
        .bind(product)
        .bind(project)
        .bind(tenant)
        .execute(&pool)
        .await
        .unwrap();
    }

    let err = violation(
        sqlx::query(
            "INSERT INTO core.product_project_link (product_id, project_id, tenant_id) \
             VALUES ($1, $2, $3)",
        )
        .bind(product_a)
        .bind(project_x)
        .bind(tenant)
        .execute(&pool)
        .await
        .expect_err("duplicate active pair must violate uq_product_project_link_active"),
    );
    // PA07-IMP-007（0047）：唯一约束改为 partial（仅约束有效行，撤回后可重建）。
    assert!(err.contains("uq_product_project_link_active"), "{err}");
}

/// FK：link 必须指向存在的 product 与 project；产品删除级联清 link（卫生级联）。
#[sqlx::test(migrations = "../../migrations")]
async fn product_project_link_fk_and_cascade(pool: PgPool) {
    let tenant = insert_tenant(&pool).await;
    let team = insert_team(&pool, tenant).await;
    let product = insert_product(&pool, team, tenant).await;
    let customer = insert_customer(&pool, tenant).await;
    let project = insert_project(&pool, tenant, customer, "PRJ-Z").await;

    for (product_id, project_id, expect) in [
        (Uuid::new_v4(), project, "fk_product_project_link_product"),
        (product, Uuid::new_v4(), "fk_product_project_link_project"),
    ] {
        let err = violation(
            sqlx::query(
                "INSERT INTO core.product_project_link (product_id, project_id, tenant_id) \
                 VALUES ($1, $2, $3)",
            )
            .bind(product_id)
            .bind(project_id)
            .bind(tenant)
            .execute(&pool)
            .await
            .expect_err("bogus reference must violate FK"),
        );
        assert!(err.contains(expect), "{err}");
    }

    sqlx::query(
        "INSERT INTO core.product_project_link (product_id, project_id, tenant_id) \
         VALUES ($1, $2, $3)",
    )
    .bind(product)
    .bind(project)
    .bind(tenant)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("DELETE FROM core.product WHERE id = $1")
        .bind(product)
        .execute(&pool)
        .await
        .unwrap();
    let left: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.product_project_link WHERE product_id = $1",
    )
    .bind(product)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(left, 0, "link must cascade with product");
}
