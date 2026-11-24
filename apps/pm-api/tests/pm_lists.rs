//! `/v1/pm/*` 列表端点契约测试（Slice A）。契约源：`frontend/src/api/pm.ts` +
//! `frontend/src/api/mock/fixtures.ts`（活文档）——裸 `{items,total,page,size}`
//! payload、`project_id?` 过滤、`created_at DESC`、page 默认 1 / size 默认 10
//! 夹在 [1,200]；revisions 列表 `{items,total}`（version DESC，未知 rid 空列表）。

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_domain::SYSTEM_TENANT;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;

mod common;

fn get(uri: String) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

async fn body_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// 以 created_at 偏移插入任务，稳定控制排序。
async fn seed_task(
    pool: &PgPool,
    project_id: uuid::Uuid,
    title: &str,
    age_secs: i32,
) -> uuid::Uuid {
    let id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO delivery.task (id, project_id, title, status, priority, created_at, updated_at, tenant_id) \
         VALUES ($1, $2, $3, 'draft', 'normal', now() - make_interval(secs => $4), now() - make_interval(secs => $4), \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(id)
    .bind(project_id)
    .bind(title)
    .bind(age_secs)
    .execute(pool)
    .await
    .unwrap();
    id
}

#[sqlx::test(migrations = "../../migrations")]
async fn empty_database_lists_answer_zero_totals(pool: PgPool) {
    let app = router(pool.clone());
    for path in [
        "/v1/pm/projects",
        "/v1/pm/principals",
        "/v1/pm/requirements",
        "/v1/pm/tasks",
        "/v1/pm/milestones",
        "/v1/pm/approvals",
        "/v1/pm/decisions",
        "/v1/pm/audit-logs",
    ] {
        let response = app.clone().oneshot(get(path.to_string())).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path} must be 200");
        let body = body_json(response).await;

        assert_eq!(body["items"], json!([]), "{path} items must be empty");
        assert_eq!(body["total"], json!(0), "{path} total must be 0");
        assert_eq!(body["page"], json!(1), "{path} default page");
        assert_eq!(body["size"], json!(10), "{path} default size");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn tasks_pagination_clamps_and_slices(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "PG-1".into(),
            project_name: "pg".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    common::join_dev(&pool, boot.project_id).await;
    for i in 0..3 {
        seed_task(&pool, boot.project_id, &format!("task-{i}"), i).await;
    }
    let app = router(pool.clone());

    // 第一页每页两条：两条新任务在前。
    let response = app
        .clone()
        .oneshot(get("/v1/pm/tasks?page=1&size=2".into()))
        .await
        .unwrap();
    let body = body_json(response).await;
    assert_eq!(body["total"], json!(3));
    assert_eq!(body["page"], json!(1));
    assert_eq!(body["size"], json!(2));
    assert_eq!(body["items"].as_array().unwrap().len(), 2);

    // 第二页剩一条。
    let response = app
        .clone()
        .oneshot(get("/v1/pm/tasks?page=2&size=2".into()))
        .await
        .unwrap();
    let body = body_json(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1);

    // size 超上限截到 200；page=0 回落第一页。
    let response = app
        .clone()
        .oneshot(get("/v1/pm/tasks?page=0&size=500".into()))
        .await
        .unwrap();
    let body = body_json(response).await;
    assert_eq!(body["page"], json!(1));
    assert_eq!(body["size"], json!(200));
    assert_eq!(body["items"].as_array().unwrap().len(), 3);
}

#[sqlx::test(migrations = "../../migrations")]
async fn project_scoped_lists_filter_by_project(pool: PgPool) {
    let a = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "FLT-A".into(),
            project_name: "a".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    common::join_dev(&pool, a.project_id).await;
    let b = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "FLT-B".into(),
            project_name: "b".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    common::join_dev(&pool, b.project_id).await;
    seed_task(&pool, a.project_id, "in-a", 10).await;
    seed_task(&pool, b.project_id, "in-b", 5).await;
    let app = router(pool.clone());

    let response = app
        .oneshot(get(format!("/v1/pm/tasks?project_id={}", a.project_id)))
        .await
        .unwrap();
    let body = body_json(response).await;
    assert_eq!(body["total"], json!(1), "filter must scope to project A");
    assert_eq!(body["items"][0]["title"], json!("in-a"));
    assert_eq!(
        body["items"][0]["project_id"],
        json!(a.project_id.to_string())
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn lists_order_created_at_desc(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "ORD-1".into(),
            project_name: "o".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    common::join_dev(&pool, boot.project_id).await;
    let first = seed_task(&pool, boot.project_id, "oldest", 100).await;
    let last = seed_task(&pool, boot.project_id, "newest", 0).await;
    let app = router(pool.clone());

    let response = app.oneshot(get("/v1/pm/tasks".into())).await.unwrap();
    let body = body_json(response).await;
    let ids: Vec<String> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids[0], last.to_string(), "newest task must come first");
    assert_eq!(
        *ids.last().unwrap(),
        first.to_string(),
        "oldest task must come last"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn task_row_shape_matches_frontend_contract(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "SHP-1".into(),
            project_name: "s".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    common::join_dev(&pool, boot.project_id).await;
    seed_task(&pool, boot.project_id, "shaped", 0).await;
    let app = router(pool.clone());

    let response = app.oneshot(get("/v1/pm/tasks".into())).await.unwrap();
    let body = body_json(response).await;
    let task = &body["items"][0];
    for field in [
        "id",
        "project_id",
        "milestone_id",
        "title",
        "description",
        "status",
        "priority",
        "assignee_id",
        "revision",
        "created_at",
        "updated_at",
    ] {
        assert!(task.get(field).is_some(), "task row must carry `{field}`");
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn milestones_list_scopes_orders_and_carries_task_link(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "MS-1".into(),
            project_name: "ms".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    common::join_dev(&pool, boot.project_id).await;
    let other = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "MS-2".into(),
            project_name: "ms2".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let milestone_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO delivery.milestone (id, project_id, name, planned_at, status, tenant_id) \
         VALUES ($1, $2, 'M-A', now() + interval '2 days', 'active', \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(milestone_id)
    .bind(boot.project_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.milestone (id, project_id, name, planned_at, status, tenant_id) \
         VALUES ($1, $2, 'M-OTHER', now(), 'planned', \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(other.project_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task (id, project_id, milestone_id, title, status, priority, tenant_id) \
         VALUES ($1, $2, $3, 'linked', 'running', 'normal', \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(boot.project_id)
    .bind(milestone_id)
    .execute(&pool)
    .await
    .unwrap();
    let app = router(pool.clone());

    let response = app
        .clone()
        .oneshot(get(format!(
            "/v1/pm/milestones?project_id={}",
            boot.project_id
        )))
        .await
        .unwrap();
    let body = body_json(response).await;
    assert_eq!(
        body["total"],
        json!(1),
        "filter must scope milestones to project"
    );
    let milestone = &body["items"][0];
    assert_eq!(milestone["name"], json!("M-A"));
    assert_eq!(milestone["status"], json!("active"));
    for field in [
        "id",
        "project_id",
        "name",
        "planned_at",
        "status",
        "revision",
    ] {
        assert!(
            milestone.get(field).is_some(),
            "milestone row must carry `{field}`"
        );
    }

    let response = app
        .oneshot(get(format!("/v1/pm/tasks?project_id={}", boot.project_id)))
        .await
        .unwrap();
    let body = body_json(response).await;
    assert_eq!(
        body["items"][0]["milestone_id"],
        json!(milestone_id.to_string()),
        "task must keep its milestone link"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn decisions_join_change_proposal_fields(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "DEC-1".into(),
            project_name: "dec".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    common::join_dev(&pool, boot.project_id).await;
    let event_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO integration.inbound_event \
         (id, project_id, actor_id, source, event_type, idempotency_key, payload, payload_hash, tenant_id) \
         VALUES ($1, $2, $3, 'pm', 'requirement_changed', 'dec-1', '{}', 'hash', \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(event_id)
    .bind(boot.project_id)
    .bind(boot.human_id)
    .execute(&pool)
    .await
    .unwrap();
    let run_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO ai.analysis_run \
         (id, project_id, event_id, model_name, policy_version, config_snapshot, tenant_id) \
         VALUES ($1, $2, $3, 'test-model', 1, '{}', \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(run_id)
    .bind(boot.project_id)
    .bind(event_id)
    .execute(&pool)
    .await
    .unwrap();
    let proposal_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO ai.change_proposal \
         (id, project_id, analysis_run_id, proposal_seq, target_type, command_type, \
          proposed_data, rationale, evidence, risk_level, tenant_id) \
         VALUES ($1, $2, $3, 1, 'task', 'update_task', '{}', \
                 '把任务拆成两步以降低风险', '[{\"kind\":\"doc\",\"title\":\"REQ-1\"}]', 'high', \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(proposal_id)
    .bind(boot.project_id)
    .bind(run_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO governance.approval_request \
         (id, project_id, proposal_id, required_role, tenant_id) \
         VALUES ($1, $2, $3, 'project_owner', \
                 (SELECT tenant_id FROM ai.change_proposal WHERE id = $3))",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(boot.project_id)
    .bind(proposal_id)
    .execute(&pool)
    .await
    .unwrap();
    let app = router(pool.clone());

    let response = app
        .oneshot(get(format!(
            "/v1/pm/decisions?project_id={}",
            boot.project_id
        )))
        .await
        .unwrap();
    let body = body_json(response).await;
    assert_eq!(body["total"], json!(1));
    let decision = &body["items"][0];
    assert_eq!(decision["status"], json!("pending"));
    assert_eq!(decision["rationale"], json!("把任务拆成两步以降低风险"));
    assert_eq!(decision["target_type"], json!("task"));
    assert_eq!(decision["command_type"], json!("update_task"));
    assert_eq!(decision["risk_level"], json!("high"));
    assert_eq!(decision["proposal_id"], json!(proposal_id.to_string()));
    assert!(
        decision["evidence"].is_array(),
        "evidence must stay an array"
    );
    for field in [
        "id",
        "project_id",
        "status",
        "required_role",
        "decided_by",
        "decision_reason",
        "decided_at",
        "created_at",
        "proposal_status",
        "proposed_data",
        "analysis_run_id",
    ] {
        assert!(
            decision.get(field).is_some(),
            "decision row must carry `{field}`"
        );
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn revisions_unknown_requirement_answers_empty_list(pool: PgPool) {
    let app = router(pool.clone());
    let unknown = uuid::Uuid::new_v4();
    let response = app
        .oneshot(get(format!("/v1/pm/requirements/{unknown}/revisions")))
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "unknown rid must not 404"
    );
    let body = body_json(response).await;
    assert_eq!(body["items"], json!([]));
    assert_eq!(body["total"], json!(0));
}

#[sqlx::test(migrations = "../../migrations")]
async fn revisions_list_orders_by_version_desc(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "REV-1".into(),
            project_name: "r".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    common::join_dev(&pool, boot.project_id).await;
    let requirement_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO delivery.requirement (id, project_id, code, title, lifecycle_status, tenant_id) \
         VALUES ($1, $2, 'REQ-1', 'rev requirement', 'active', \
                 (SELECT tenant_id FROM core.project WHERE id = $2))",
    )
    .bind(requirement_id)
    .bind(boot.project_id)
    .execute(&pool)
    .await
    .unwrap();
    for version in 1..=3 {
        sqlx::query(
            "INSERT INTO delivery.requirement_revision \
             (id, requirement_id, project_id, version, content, state, content_hash, created_by, tenant_id) \
             VALUES ($1, $2, $3, $4, '{}', 'draft', 'hash', $5, \
                     (SELECT tenant_id FROM core.project WHERE id = $3))",
        )
        .bind(uuid::Uuid::new_v4())
        .bind(requirement_id)
        .bind(boot.project_id)
        .bind(version)
        .bind(boot.human_id)
        .execute(&pool)
        .await
        .unwrap();
    }
    let app = router(pool.clone());

    let response = app
        .oneshot(get(format!(
            "/v1/pm/requirements/{requirement_id}/revisions"
        )))
        .await
        .unwrap();
    let body = body_json(response).await;
    let versions: Vec<i64> = body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["version"].as_i64().unwrap())
        .collect();
    assert_eq!(versions, vec![3, 2, 1], "revisions must be version DESC");
    let revision = &body["items"][0];
    for field in [
        "id",
        "requirement_id",
        "project_id",
        "version",
        "content",
        "state",
        "content_hash",
        "created_by",
        "approved_by",
        "created_at",
    ] {
        assert!(
            revision.get(field).is_some(),
            "revision row must carry `{field}`"
        );
    }
}

// ─────────────────────── PA-07 列表读面（Figma 179:3603 A，0050 读面扩展）

/// 建 Product（归属该任务租户的新 Team，status 参数化）+ 有效 Task-Product 归属。
async fn seed_task_product(
    pool: &PgPool,
    project_id: uuid::Uuid,
    task_id: uuid::Uuid,
    name: &str,
    status: &str,
) -> uuid::Uuid {
    let product_id: uuid::Uuid = sqlx::query_scalar(
        "WITH t AS ( \
             INSERT INTO core.team (tenant_id, name) \
             SELECT tenant_id, $1 || '-team' FROM delivery.task WHERE id = $2 RETURNING tenant_id, id \
         ), p AS ( \
             INSERT INTO core.product (team_id, tenant_id, key, name, status) \
             SELECT id, tenant_id, 'P-' || upper(substring(gen_random_uuid()::text from 1 for 8)), $1, $3 FROM t RETURNING id \
         ) SELECT id FROM p",
    )
    .bind(name)
    .bind(task_id)
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO delivery.task_product (tenant_id, project_id, task_id, product_id) \
         SELECT tenant_id, project_id, $1, $2 FROM delivery.task WHERE id = $1",
    )
    .bind(task_id)
    .bind(product_id)
    .execute(pool)
    .await
    .unwrap();
    product_id
}

/// 直插依赖边（required_condition 可参数化）。
async fn seed_dependency_edge(
    pool: &PgPool,
    project_id: uuid::Uuid,
    task_id: uuid::Uuid,
    depends_on: uuid::Uuid,
    created_by: uuid::Uuid,
    condition: &str,
) {
    sqlx::query(
        "INSERT INTO delivery.task_dependency \
         (tenant_id, project_id, task_id, depends_on_task_id, created_by, required_condition) \
         SELECT tenant_id, project_id, $1, $2, $3, $4 FROM delivery.task WHERE id = $1",
    )
    .bind(task_id)
    .bind(depends_on)
    .bind(created_by)
    .bind(condition)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "../../migrations")]
async fn task_list_exposes_pa07_scheduling_material(pool: PgPool) {
    let boot = bootstrap_project(
        &pool,
        BootstrapRequest {
            project_code: "PA07-L".into(),
            project_name: "pa07".into(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    common::join_dev(&pool, boot.project_id).await;

    // 主任务：关联 normal+suspended 产品、两条依赖边（1 满足 1 未满足）、一条 queued run。
    let main = seed_task(&pool, boot.project_id, "main", 0).await;
    seed_task_product(&pool, boot.project_id, main, "Auth service", "normal").await;
    seed_task_product(&pool, boot.project_id, main, "Billing API", "suspended").await;
    let met_pred = seed_task(&pool, boot.project_id, "pred-met", 30).await;
    sqlx::query("UPDATE delivery.task SET status = 'completed' WHERE id = $1")
        .bind(met_pred)
        .execute(&pool)
        .await
        .unwrap();
    let unmet_pred = seed_task(&pool, boot.project_id, "pred-unmet", 20).await;
    seed_dependency_edge(
        &pool,
        boot.project_id,
        main,
        met_pred,
        boot.human_id,
        "completed",
    )
    .await;
    seed_dependency_edge(
        &pool,
        boot.project_id,
        main,
        unmet_pred,
        boot.human_id,
        "completed",
    )
    .await;
    sqlx::query(
        "INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
         VALUES ($1, $2, 1, 'queued', (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(boot.project_id)
    .bind(main)
    .execute(&pool)
    .await
    .unwrap();

    // 干净任务：无产品/依赖/run。
    seed_task(&pool, boot.project_id, "plain", 10).await;

    let app = router(pool.clone());
    let response = app
        .oneshot(get(format!("/v1/pm/tasks?project_id={}", boot.project_id)))
        .await
        .unwrap();
    let body = body_json(response).await;
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 4);

    let main_row = items.iter().find(|t| t["title"] == json!("main")).unwrap();
    // code：0050 触发器统一发号 PM-NNN（项目内连续）。
    let code = main_row["code"].as_str().unwrap();
    assert!(
        code.starts_with("PM-") && code[3..].chars().all(|c| c.is_ascii_digit()),
        "task code must be PM-NNN, got {code}"
    );
    // products：活跃关联产品名+状态齐出。
    let products = main_row["products"].as_array().unwrap();
    assert_eq!(products.len(), 2, "active task-product links only");
    let names: Vec<&str> = products
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"Auth service") && names.contains(&"Billing API"));
    // 调度原料：deps 计数与产品门禁态。
    assert_eq!(main_row["deps_total"], json!(2));
    assert_eq!(
        main_row["deps_met"],
        json!(1),
        "completed predecessor satisfies"
    );
    assert_eq!(
        main_row["product_blocked"],
        json!(true),
        "suspended product blocks"
    );
    // 最新 run 原料。
    assert_eq!(main_row["latest_run"]["attempt_no"], json!(1));
    assert_eq!(main_row["latest_run"]["status"], json!("queued"));

    let plain = items.iter().find(|t| t["title"] == json!("plain")).unwrap();
    assert_eq!(plain["products"], json!([]));
    assert!(plain["latest_run"].is_null());
    assert_eq!(plain["deps_total"], json!(0));
    assert_eq!(plain["deps_met"], json!(0));
    assert_eq!(plain["product_blocked"], json!(false));

    // 排序：priority 相同按 created_at DESC；不同 priority urgent 在前。
    let priority_order: Vec<String> = items
        .iter()
        .map(|t| t["priority"].as_str().unwrap().to_string())
        .collect();
    assert!(
        priority_order.iter().all(|p| p == "normal"),
        "seeded all normal"
    );
    let titles: Vec<&str> = items.iter().map(|t| t["title"].as_str().unwrap()).collect();
    assert_eq!(
        titles[0], "main",
        "same priority → created_at DESC keeps newest first"
    );
}
