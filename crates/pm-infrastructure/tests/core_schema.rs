#[sqlx::test(migrations = "../../migrations")]
async fn core_schema_ready(pool: sqlx::PgPool) {
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*)::int8 FROM information_schema.tables \
         WHERE table_schema = 'core'"
    ).fetch_one(&pool).await.unwrap();
    // customer / project / principal / project_member + principal_identity（0018）
    // + tenant / tenant_member（0019）+ tenant_agent（0020）
    // + project_initialization_operation（0031，S01 · I6-B）
    assert_eq!(n, 9);
}
