#[sqlx::test(migrations = "../../migrations")]
async fn core_schema_ready(pool: sqlx::PgPool) {
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*)::int8 FROM information_schema.tables \
         WHERE table_schema = 'core'"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(n, 4);
}
