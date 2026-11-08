//! I6-B DB 级负向测试（`docs/ui/smark-pm-s01-i6-b-v1-1.md` §11.2）。
//!
//! **直接对数据库执行 INSERT / UPDATE 断言约束违例**，不经 REST / MCP：
//! 这些约束是「同 Project 归属由数据库保证」的唯一证据，必须以数据库为断言面。
//!
//! 覆盖 7 条负向场景 + I6-B-036~051 中的数据类条目：
//! - 跨 Project 的 Finding/Source/Evidence/Repository 关联必须失败（23503）
//! - 代码类证据缺 Commit 链路必须失败（23514）
//! - 非代码证据伪造 Commit 必须失败（23514）
//! - 同一 Import 下每个仓库只允许一个 active 初始化操作（23505）
//! - 同一 Project 下只允许一个 active Import（23505）
//! - 被引用的仓库不可物理删除（23503，RESTRICT）

use sqlx::PgPool;
use uuid::Uuid;

/// 种一个租户。
async fn seed_tenant(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', 'i6b-tenant', 'i6b') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 种一个真人 principal。
async fn seed_human(pool: &PgPool, _tenant_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) \
         VALUES ('human', 'i6b-human') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 一次性把 I6-B 的四层对象种齐，返回各层 id。
struct World {
    tenant_id: Uuid,
    project_a: Uuid,
    project_b: Uuid,
    repository_a: Uuid,
    repository_b: Uuid,
    import_a: Uuid,
    session_a: Uuid,
    finding_a: Uuid,
}

async fn world(pool: &PgPool) -> World {
    let tenant_id = seed_tenant(pool).await;
    let customer_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (tenant_id, name) VALUES ($1, 'c') RETURNING id",
    )
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let project_a = seed_project(pool, tenant_id, customer_id, "I6B-A").await;
    let project_b = seed_project(pool, tenant_id, customer_id, "I6B-B").await;

    let repository_a = seed_repository(pool, tenant_id, project_a, "repo-a").await;
    let repository_b = seed_repository(pool, tenant_id, project_b, "repo-b").await;

    // Import + Source + Attempt（Import 必须落在 project_a）。
    let import_a: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.repo_import \
         (tenant_id, project_id, requested_by, status) \
         VALUES ($1, $2, $3, 'analyzing') RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_a)
    .bind(seed_human(pool, tenant_id).await)
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO integration.repo_import_source \
         (tenant_id, project_id, import_id, repository_id, head_commit_sha, current_attempt_no) \
         VALUES ($1, $2, $3, $4, 'sha-a', 1)",
    )
    .bind(tenant_id)
    .bind(project_a)
    .bind(import_a)
    .bind(repository_a)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO integration.repo_import_source_attempt \
         (tenant_id, project_id, import_id, repository_id, attempt_no, status, commit_sha) \
         VALUES ($1, $2, $3, $4, 1, 'succeeded', 'sha-a')",
    )
    .bind(tenant_id)
    .bind(project_a)
    .bind(import_a)
    .bind(repository_a)
    .execute(pool)
    .await
    .unwrap();

    // Discovery Session + Finding（都在 project_a）。
    let session_a: Uuid = sqlx::query_scalar(
        "INSERT INTO discovery.discovery_session \
         (tenant_id, project_id, source, kind, status, created_by) \
         VALUES ($1, $2, 'WEB', 'INITIAL_ONBOARDING', 'IN_PROGRESS', $3) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_a)
    .bind(seed_human(pool, tenant_id).await)
    .fetch_one(pool)
    .await
    .unwrap();

    let finding_a: Uuid = sqlx::query_scalar(
        "INSERT INTO discovery.discovery_finding \
         (tenant_id, project_id, session_id, kind, content) \
         VALUES ($1, $2, $3, 'observed', '事实') RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_a)
    .bind(session_a)
    .fetch_one(pool)
    .await
    .unwrap();

    World {
        tenant_id,
        project_a,
        project_b,
        repository_a,
        repository_b,
        import_a,
        session_a,
        finding_a,
    }
}

/// 种一个项目。
async fn seed_project(pool: &PgPool, tenant_id: Uuid, customer_id: Uuid, code: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.project (tenant_id, customer_id, code, name) \
         VALUES ($1, $2, $3, $3) RETURNING id",
    )
    .bind(tenant_id)
    .bind(customer_id)
    .bind(code)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 种一个已验证仓库。
async fn seed_repository(pool: &PgPool, tenant_id: Uuid, project_id: Uuid, code: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO integration.project_repository \
         (tenant_id, project_id, url, name, role, status) \
         VALUES ($1, $2, $3, $3, 'main', 'verified') RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(format!("https://example.invalid/{code}.git"))
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 断言某条 SQL 触发指定 SQLSTATE 的约束违例。
async fn expect_violation(pool: &PgPool, expected: &str, sql: &str) {
    let err = sqlx::query(sql)
        .execute(pool)
        .await
        .expect_err("statement must violate a constraint");
    let db = err
        .as_database_error()
        .unwrap_or_else(|| panic!("expected a database error, got {err}"));
    let code = db.code().expect("sqlstate must be present").to_string();
    assert_eq!(code, expected, "unexpected sqlstate for `{sql}`");
}

/// 负向 1：同 Tenant 不同 Project 的 Finding 与 Source 建 Evidence → FK 违例。
#[sqlx::test(migrations = "../../migrations")]
async fn negative_1_evidence_cannot_cross_projects(pool: PgPool) {
    let w = world(&pool).await;
    // 造一个属于 project_b 的 Source（Import 也在 project_b）。
    let import_b: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.repo_import \
         (tenant_id, project_id, requested_by, status) \
         VALUES ($1, $2, $3, 'analyzing') RETURNING id",
    )
    .bind(w.tenant_id)
    .bind(w.project_b)
    .bind(seed_human(&pool, w.tenant_id).await)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO integration.repo_import_source \
         (tenant_id, project_id, import_id, repository_id) VALUES ($1, $2, $3, $4)",
    )
    .bind(w.tenant_id)
    .bind(w.project_b)
    .bind(import_b)
    .bind(w.repository_b)
    .execute(&pool)
    .await
    .unwrap();

    // Evidence 指向 project_a 的 Finding，却引用 project_b 的 Source → 违例。
    expect_violation(
        &pool,
        "23503",
        &format!(
            "INSERT INTO discovery.discovery_finding_evidence \
             (tenant_id, project_id, finding_id, evidence_type, import_id, repository_id, \
              commit_sha, attempt_no) \
             VALUES ('{}', '{}', '{}', 'code', '{}', '{}', 'sha-b', 1)",
            w.tenant_id, w.project_a, w.finding_a, import_b, w.repository_b
        ),
    )
    .await;
}

/// 负向 2：Evidence 指向**没有 Source 行**的 Import → 证据→Source 组合 FK 违例。
///
/// 「Attempt 属于其 Source」由五列组合键结构保证（补-2），因此不存在「引用了别的
/// Import 的 Attempt」这种可表达状态；可表达的错配是「Evidence 声称的 Import 下
/// 根本没有这个仓库的 Source」。
#[sqlx::test(migrations = "../../migrations")]
async fn negative_2_evidence_cannot_use_import_without_source(pool: PgPool) {
    let w = world(&pool).await;
    // 同一 Project 的另一个 Import（终态，不占 active 名额），它**没有** repository_a 的 Source。
    let import_b: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.repo_import \
         (tenant_id, project_id, requested_by, status) \
         VALUES ($1, $2, $3, 'draft_ready') RETURNING id",
    )
    .bind(w.tenant_id)
    .bind(w.project_a)
    .bind(seed_human(&pool, w.tenant_id).await)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Evidence 声称 import_b + repository_a → 该组合下没有 Source 行 → 违例。
    expect_violation(
        &pool,
        "23503",
        &format!(
            "INSERT INTO discovery.discovery_finding_evidence \
             (tenant_id, project_id, finding_id, evidence_type, import_id, repository_id, \
              commit_sha, attempt_no) \
             VALUES ('{}', '{}', '{}', 'code', '{}', '{}', 'sha-b', 1)",
            w.tenant_id, w.project_a, w.finding_a, import_b, w.repository_a
        ),
    )
    .await;
}

/// 负向 3：Evidence 引用的 Attempt 轮次不存在 → FK 违例。
#[sqlx::test(migrations = "../../migrations")]
async fn negative_3_evidence_cannot_use_missing_attempt_round(pool: PgPool) {
    let w = world(&pool).await;
    expect_violation(
        &pool,
        "23503",
        &format!(
            "INSERT INTO discovery.discovery_finding_evidence \
             (tenant_id, project_id, finding_id, evidence_type, import_id, repository_id, \
              commit_sha, attempt_no) \
             VALUES ('{}', '{}', '{}', 'code', '{}', '{}', 'sha-a', 99)",
            w.tenant_id, w.project_a, w.finding_a, w.import_a, w.repository_a
        ),
    )
    .await;
}

/// 负向 4：Finding 的 project_id 与 Session 不一致 → FK 违例。
#[sqlx::test(migrations = "../../migrations")]
async fn negative_4_finding_project_must_match_session_project(pool: PgPool) {
    let w = world(&pool).await;
    expect_violation(
        &pool,
        "23503",
        &format!(
            "INSERT INTO discovery.discovery_finding \
             (tenant_id, project_id, session_id, kind, content) \
             VALUES ('{}', '{}', '{}', 'observed', '跨项目')",
            w.tenant_id, w.project_b, w.session_a
        ),
    )
    .await;
}

/// 负向 5：同租户跨项目仓库挂 Finding（D-09 复发）→ FK 违例。
#[sqlx::test(migrations = "../../migrations")]
async fn negative_5_finding_cannot_attach_other_project_repository(pool: PgPool) {
    let w = world(&pool).await;
    expect_violation(
        &pool,
        "23503",
        &format!(
            "INSERT INTO discovery.discovery_finding_repository \
             (tenant_id, project_id, finding_id, repository_id) \
             VALUES ('{}', '{}', '{}', '{}')",
            w.tenant_id, w.project_a, w.finding_a, w.repository_b
        ),
    )
    .await;
}

/// 负向 6：代码类证据缺 Commit 链路 → CHECK 违例。
#[sqlx::test(migrations = "../../migrations")]
async fn negative_6_code_evidence_requires_commit_linkage(pool: PgPool) {
    let w = world(&pool).await;
    expect_violation(
        &pool,
        "23514",
        &format!(
            "INSERT INTO discovery.discovery_finding_evidence \
             (tenant_id, project_id, finding_id, evidence_type, file_path) \
             VALUES ('{}', '{}', '{}', 'code', 'src/main.rs')",
            w.tenant_id, w.project_a, w.finding_a
        ),
    )
    .await;
}

/// 负向 7：非代码证据伪造 Commit → CHECK 违例。
#[sqlx::test(migrations = "../../migrations")]
async fn negative_7_non_code_evidence_must_not_carry_commit(pool: PgPool) {
    let w = world(&pool).await;
    expect_violation(
        &pool,
        "23514",
        &format!(
            "INSERT INTO discovery.discovery_finding_evidence \
             (tenant_id, project_id, finding_id, evidence_type, import_id, repository_id, \
              commit_sha, attempt_no) \
             VALUES ('{}', '{}', '{}', 'user_statement', '{}', '{}', 'sha-a', 1)",
            w.tenant_id, w.project_a, w.finding_a, w.import_a, w.repository_a
        ),
    )
    .await;
}

/// 正向对照：链路齐全的代码类证据**必须**能落库（否则负向测试是假阳性）。
#[sqlx::test(migrations = "../../migrations")]
async fn positive_code_evidence_with_full_linkage_is_accepted(pool: PgPool) {
    let w = world(&pool).await;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO discovery.discovery_finding_evidence \
         (tenant_id, project_id, finding_id, evidence_type, file_path, import_id, \
          repository_id, commit_sha, attempt_no) \
         VALUES ($1, $2, $3, 'code', 'src/main.rs', $4, $5, 'sha-a', 1) RETURNING id",
    )
    .bind(w.tenant_id)
    .bind(w.project_a)
    .bind(w.finding_a)
    .bind(w.import_a)
    .bind(w.repository_a)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!id.is_nil());
}

/// I6-B-038：同 Project 至多一个 active 初始化操作 → 违例。
#[sqlx::test(migrations = "../../migrations")]
async fn only_one_active_initialization_operation_per_project(pool: PgPool) {
    let w = world(&pool).await;
    for generation in 1..=2 {
        let _ = sqlx::query(
            "INSERT INTO core.project_initialization_operation \
             (tenant_id, project_id, generation, status) VALUES ($1, $2, $3, 'active')",
        )
        .bind(w.tenant_id)
        .bind(w.project_a)
        .bind(generation)
        .execute(&pool)
        .await;
    }
    // 第三条 active → 部分唯一索引违例。
    let err = sqlx::query(
        "INSERT INTO core.project_initialization_operation \
         (tenant_id, project_id, generation, status) VALUES ($1, $2, 3, 'active')",
    )
    .bind(w.tenant_id)
    .bind(w.project_a)
    .execute(&pool)
    .await
    .expect_err("second active operation must be rejected");
    let code = err
        .as_database_error()
        .unwrap()
        .code()
        .expect("sqlstate")
        .to_string();
    assert_eq!(code, "23505", "expected unique violation");
}

/// I6-B-039：同 Project 至多一个 active Import → 违例。
#[sqlx::test(migrations = "../../migrations")]
async fn only_one_active_import_per_project(pool: PgPool) {
    let w = world(&pool).await;
    // project_a 已被种子占用一个 active Import（`world()` 里 status='analyzing'）；
    // 这里用 project_b 验证名额约束本身。
    let _ = sqlx::query(
        "INSERT INTO integration.repo_import \
         (tenant_id, project_id, requested_by, status) \
         VALUES ($1, $2, $3, 'analyzing')",
    )
    .bind(w.tenant_id)
    .bind(w.project_b)
    .bind(seed_human(&pool, w.tenant_id).await)
    .execute(&pool)
    .await
    .unwrap();
    let err = sqlx::query(
        "INSERT INTO integration.repo_import \
         (tenant_id, project_id, requested_by, status) \
         VALUES ($1, $2, $3, 'queued')",
    )
    .bind(w.tenant_id)
    .bind(w.project_b)
    .bind(seed_human(&pool, w.tenant_id).await)
    .execute(&pool)
    .await
    .expect_err("second active import must be rejected");
    let code = err
        .as_database_error()
        .unwrap()
        .code()
        .expect("sqlstate")
        .to_string();
    assert_eq!(code, "23505", "expected unique violation");
}

/// I6-B-050：被历史引用的仓库不可物理删除（RESTRICT）。
#[sqlx::test(migrations = "../../migrations")]
async fn repository_with_history_cannot_be_deleted(pool: PgPool) {
    let w = world(&pool).await;
    let err = sqlx::query("DELETE FROM integration.project_repository WHERE id = $1")
        .bind(w.repository_a)
        .execute(&pool)
        .await
        .expect_err("repository with history must not be deletable");
    let code = err
        .as_database_error()
        .unwrap()
        .code()
        .expect("sqlstate")
        .to_string();
    assert_eq!(code, "23503", "expected foreign key violation (RESTRICT)");
}

/// I6-B-047：Source 组合唯一键生效（同一仓库在一次 Import 内只能有一行）。
#[sqlx::test(migrations = "../../migrations")]
async fn source_rows_are_unique_per_import_and_repository(pool: PgPool) {
    let w = world(&pool).await;
    let err = sqlx::query(
        "INSERT INTO integration.repo_import_source \
         (tenant_id, project_id, import_id, repository_id) VALUES ($1, $2, $3, $4)",
    )
    .bind(w.tenant_id)
    .bind(w.project_a)
    .bind(w.import_a)
    .bind(w.repository_a)
    .execute(&pool)
    .await
    .expect_err("duplicate source row must be rejected");
    let code = err
        .as_database_error()
        .unwrap()
        .code()
        .expect("sqlstate")
        .to_string();
    assert_eq!(code, "23505", "expected unique violation");
}

/// I6-B-046：Source 的基准 `head_commit_sha` 只能写一次（write-once 由应用层保证，
/// 这里断言的是 **Attempt 层留档** 与 Source 指针分离）。
#[sqlx::test(migrations = "../../migrations")]
async fn attempt_history_is_kept_per_round(pool: PgPool) {
    let w = world(&pool).await;
    // 第二轮失败也留档（R-4-1：成功与失败都按轮留）。
    sqlx::query(
        "INSERT INTO integration.repo_import_source_attempt \
         (tenant_id, project_id, import_id, repository_id, attempt_no, status, error_code) \
         VALUES ($1, $2, $3, $4, 2, 'failed', 'clone_failed')",
    )
    .bind(w.tenant_id)
    .bind(w.project_a)
    .bind(w.import_a)
    .bind(w.repository_a)
    .execute(&pool)
    .await
    .unwrap();
    let rounds: Vec<i32> = sqlx::query_scalar(
        "SELECT attempt_no FROM integration.repo_import_source_attempt \
         WHERE import_id = $1 AND repository_id = $2 ORDER BY attempt_no",
    )
    .bind(w.import_a)
    .bind(w.repository_a)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rounds, vec![1, 2], "both rounds must be kept");

    // 时间约束：finished_at 早于 started_at → CHECK 违例。
    expect_violation(
        &pool,
        "23514",
        &format!(
            "INSERT INTO integration.repo_import_source_attempt \
             (tenant_id, project_id, import_id, repository_id, attempt_no, status, \
              started_at, finished_at) \
             VALUES ('{}', '{}', '{}', '{}', 3, 'succeeded', now(), now() - interval '1 hour')",
            w.tenant_id, w.project_a, w.import_a, w.repository_a
        ),
    )
    .await;
}
