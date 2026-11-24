//! Artifact 生命周期与审批流程集成测试（V0-2 服务层）。
//!
//! 覆盖（AC-3 / AC-16 / 13 号 §4）：
//! - 全路径：draft → pending → published → stale；draft → rejected；
//! - 解释图不可请求发布（DV-13）；
//! - 哈希漂移 → 409（乐观并发）；
//! - 同槽位第二条 pending → 409；
//! - 终态重审 → 409；
//! - **接受图形不修改任何业务对象**（AC-16：只写 viz.* 与审计）。

use sqlx::PgPool;
use uuid::Uuid;

use pm_application::actor::AuditActor;
use pm_application::visualization::artifact_service::{
    create_draft, decide_approval, get_artifact, mark_stale, request_approval, CreateDraftInput,
    VizError,
};

/// actor 的 principal_id 必须真实存在——审计表对 principal 有 FK。
fn actor(principal_id: Uuid) -> AuditActor {
    AuditActor {
        principal_id,
        identity_id: None,
        client: None,
        auth_mode: Some("password".to_string()),
        claimed_device_id: None,
    }
}

async fn ensure_project(pool: &PgPool) -> (Uuid, Uuid) {
    let tenant = Uuid::new_v4();
    let project = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let principal = Uuid::new_v4();
    sqlx::query("INSERT INTO core.tenant (id, provider, external_key, display_name) VALUES ($1, 'lmms', $2, '测试租户')")
        .bind(tenant)
        .bind(tenant.to_string())
        .execute(pool)
        .await
        .expect("insert tenant");
    // core.principal 无 tenant_id 列（租户映射在 principal_identity）
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name) VALUES ($1, 'human', '测试人')",
    )
    .bind(principal)
    .execute(pool)
    .await
    .expect("insert principal");
    sqlx::query(
        "INSERT INTO core.tenant_member (tenant_id, principal_id, role) VALUES ($1, $2, 'owner')",
    )
    .bind(tenant)
    .bind(principal)
    .execute(pool)
    .await
    .expect("insert tenant member");
    sqlx::query("INSERT INTO core.customer (id, tenant_id, name) VALUES ($1, $2, '测试客户')")
        .bind(customer)
        .bind(tenant)
        .execute(pool)
        .await
        .expect("insert customer");
    sqlx::query("INSERT INTO core.project (id, tenant_id, customer_id, code, name, status) VALUES ($1, $2, $3, 'P1', '测试项目', 'active')")
        .bind(project)
        .bind(tenant)
        .bind(customer)
        .execute(pool)
        .await
        .expect("insert project");
    sqlx::query("INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) VALUES ($1, $2, $3, 'owner')")
        .bind(tenant)
        .bind(project)
        .bind(principal)
        .execute(pool)
        .await
        .expect("insert member");
    (project, principal)
}

fn sample_spec() -> serde_json::Value {
    serde_json::json!({
        "spec_version": "1.0",
        "slot_key": "req-trace",
        "view": { "kind": "trace-graph" },
        "nodes": [],
        "edges": [],
        "meta": { "budget": "balanced" }
    })
}

#[sqlx::test(migrations = "../../migrations")]
async fn full_lifecycle_draft_to_published_to_stale(pool: PgPool) {
    let (project, principal) = ensure_project(&pool).await;
    let a = actor(principal);
    let artifact = create_draft(
        &pool,
        project,
        &a,
        CreateDraftInput {
            slot_key: "req-trace",
            nature: "business",
            spec: &sample_spec(),
            spec_hash: "spec-hash-1",
            source_hash: "source-hash-1",
            provenance: &serde_json::json!({ "source_domain": "delivery" }),
        },
    )
    .await
    .expect("创建 draft");

    assert_eq!(artifact.state, "draft");
    assert_eq!(artifact.version_no, 1);

    // draft → pending
    let approval = request_approval(&pool, artifact.id, &a)
        .await
        .expect("请求审批");
    // pending → published（approve 同事务）
    decide_approval(&pool, approval, &a, "approve", None)
        .await
        .expect("批准");

    let published = get_artifact(&pool, artifact.id).await.expect("读取");
    assert_eq!(published.state, "published");

    // published → stale
    mark_stale(&pool, artifact.id, &a)
        .await
        .expect("标记 stale");
    let stale = get_artifact(&pool, artifact.id).await.expect("读取");
    assert_eq!(stale.state, "stale");
}

#[sqlx::test(migrations = "../../migrations")]
async fn rejection_requires_reason_and_goes_to_rejected(pool: PgPool) {
    let (project, principal) = ensure_project(&pool).await;
    let a = actor(principal);
    let artifact = create_draft(
        &pool,
        project,
        &a,
        CreateDraftInput {
            slot_key: "req-trace",
            nature: "business",
            spec: &sample_spec(),
            spec_hash: "spec-hash-1",
            source_hash: "source-hash-1",
            provenance: &serde_json::json!({ "source_domain": "delivery" }),
        },
    )
    .await
    .expect("创建 draft");
    let approval = request_approval(&pool, artifact.id, &a)
        .await
        .expect("请求审批");

    // 无理由拒绝 → Validation
    let result = decide_approval(&pool, approval, &a, "reject", None).await;
    assert!(
        matches!(result, Err(VizError::Validation(_))),
        "拒绝必须带理由"
    );

    // 带理由拒绝 → 成功，artifact 置 rejected
    decide_approval(&pool, approval, &a, "reject", Some("不符合追踪语义"))
        .await
        .expect("带理由拒绝");
    let rejected = get_artifact(&pool, artifact.id).await.expect("读取");
    assert_eq!(rejected.state, "rejected");
}

#[sqlx::test(migrations = "../../migrations")]
async fn explanatory_artifact_cannot_request_approval(pool: PgPool) {
    let (project, principal) = ensure_project(&pool).await;
    let a = actor(principal);
    let artifact = create_draft(
        &pool,
        project,
        &a,
        CreateDraftInput {
            slot_key: "explanatory-demo",
            nature: "explanatory",
            spec: &sample_spec(),
            spec_hash: "h",
            source_hash: "s",
            provenance: &serde_json::json!({ "source_domain": "ai" }),
        },
    )
    .await
    .expect("创建解释图草稿");
    let result = request_approval(&pool, artifact.id, &a).await;
    assert!(
        matches!(result, Err(VizError::Validation(_))),
        "解释图不得进入发布流程（DV-13）"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn hash_drift_causes_conflict(pool: PgPool) {
    let (project, principal) = ensure_project(&pool).await;
    let a = actor(principal);
    let artifact = create_draft(
        &pool,
        project,
        &a,
        CreateDraftInput {
            slot_key: "req-trace",
            nature: "business",
            spec: &sample_spec(),
            spec_hash: "spec-hash-1",
            source_hash: "source-hash-1",
            provenance: &serde_json::json!({ "source_domain": "delivery" }),
        },
    )
    .await
    .expect("创建 draft");
    let approval = request_approval(&pool, artifact.id, &a)
        .await
        .expect("请求审批");

    // 审批请求后篡改 spec_hash（模拟 artifact 被改）
    sqlx::query("UPDATE viz.artifact SET spec_hash = 'drifted-hash' WHERE id = $1")
        .bind(artifact.id)
        .execute(&pool)
        .await
        .expect("篡改哈希");

    let result = decide_approval(&pool, approval, &a, "approve", None).await;
    assert!(
        matches!(result, Err(VizError::Conflict(_))),
        "哈希漂移必须 409"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn second_pending_on_same_slot_conflicts(pool: PgPool) {
    let (project, principal) = ensure_project(&pool).await;
    let a = actor(principal);
    let first = create_draft(
        &pool,
        project,
        &a,
        CreateDraftInput {
            slot_key: "req-trace",
            nature: "business",
            spec: &sample_spec(),
            spec_hash: "h1",
            source_hash: "s1",
            provenance: &serde_json::json!({ "source_domain": "delivery" }),
        },
    )
    .await
    .expect("第一个 draft");
    let second = create_draft(
        &pool,
        project,
        &a,
        CreateDraftInput {
            slot_key: "req-trace",
            nature: "business",
            spec: &sample_spec(),
            spec_hash: "h2",
            source_hash: "s2",
            provenance: &serde_json::json!({ "source_domain": "delivery" }),
        },
    )
    .await
    .expect("第二个 draft（version 2）");

    request_approval(&pool, first.id, &a)
        .await
        .expect("第一条 pending");
    let result = request_approval(&pool, second.id, &a).await;
    assert!(
        matches!(result, Err(VizError::Conflict(_))),
        "同槽位第二条 pending 必须 409"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn decided_approval_cannot_be_redecided(pool: PgPool) {
    let (project, principal) = ensure_project(&pool).await;
    let a = actor(principal);
    let artifact = create_draft(
        &pool,
        project,
        &a,
        CreateDraftInput {
            slot_key: "req-trace",
            nature: "business",
            spec: &sample_spec(),
            spec_hash: "h",
            source_hash: "s",
            provenance: &serde_json::json!({ "source_domain": "delivery" }),
        },
    )
    .await
    .expect("创建 draft");
    let approval = request_approval(&pool, artifact.id, &a)
        .await
        .expect("请求审批");
    decide_approval(&pool, approval, &a, "approve", None)
        .await
        .expect("批准");

    let result = decide_approval(&pool, approval, &a, "reject", Some("再来一次")).await;
    assert!(matches!(result, Err(VizError::Conflict(_))), "终态不得重审");
}

#[sqlx::test(migrations = "../../migrations")]
async fn accept_does_not_touch_business_tables(pool: PgPool) {
    // AC-16：接受图形后，业务域表（delivery/design/kb/pc/ai）零变更。
    let (project, principal) = ensure_project(&pool).await;
    let a = actor(principal);
    let artifact = create_draft(
        &pool,
        project,
        &a,
        CreateDraftInput {
            slot_key: "req-trace",
            nature: "business",
            spec: &sample_spec(),
            spec_hash: "h",
            source_hash: "s",
            provenance: &serde_json::json!({ "source_domain": "delivery" }),
        },
    )
    .await
    .expect("创建 draft");
    let approval = request_approval(&pool, artifact.id, &a)
        .await
        .expect("请求审批");
    decide_approval(&pool, approval, &a, "approve", None)
        .await
        .expect("批准");

    // 业务域对象计数（在测试库中应全为 0——本测试只写过 viz.* 与审计）
    for table in [
        "delivery.requirement",
        "delivery.task",
        "design.design",
        "design.adr",
        "kb.document",
        "kb.memory",
        "pc.cognition_object",
        "ai.cognition_item",
    ] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(
            count, 0,
            "{table} 不应有任何行——可视化零业务写路径（AC-16）"
        );
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn regenerate_creates_new_version_not_overwrite(pool: PgPool) {
    let (project, principal) = ensure_project(&pool).await;
    let a = actor(principal);
    let first = create_draft(
        &pool,
        project,
        &a,
        CreateDraftInput {
            slot_key: "req-trace",
            nature: "business",
            spec: &sample_spec(),
            spec_hash: "h1",
            source_hash: "s1",
            provenance: &serde_json::json!({ "source_domain": "delivery" }),
        },
    )
    .await
    .expect("v1");
    let second = create_draft(
        &pool,
        project,
        &a,
        CreateDraftInput {
            slot_key: "req-trace",
            nature: "business",
            spec: &sample_spec(),
            spec_hash: "h2",
            source_hash: "s2",
            provenance: &serde_json::json!({ "source_domain": "delivery" }),
        },
    )
    .await
    .expect("v2");
    assert_eq!(first.version_no, 1);
    assert_eq!(second.version_no, 2);
    // v1 仍在（不覆盖）
    assert!(get_artifact(&pool, first.id).await.is_ok());
}
