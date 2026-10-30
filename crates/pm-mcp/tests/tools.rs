//! MCP tool layer tests (Task 16, Step 1).
//!
//! Contract under test: the three tool functions in `pm_mcp::tools` are thin,
//! DB-testable wrappers — `pm_submit_technical_design` reuses the idempotent
//! `submit_event` (`event_type='technical_design_submitted'`, `source='mcp'`,
//! payload `{task_ref, fixture, summary}`) and answers
//! `{event_id, processing_status}`; `pm_get_submission_status` aggregates the
//! event status plus its proposals (`id`/`status`/`command_type` ordered by
//! `proposal_seq`) and the latest analysis run; `pm_get_task_context` reuses
//! `read::task_context` and turns cross-project / unknown task / unknown
//! project into tool errors, never panics.
//!
//! The transport smoke test at the bottom drives the real rmcp streamable
//! HTTP service in-process (axum on an ephemeral port) with raw JSON-RPC.

use pm_application::bootstrap::{BootstrapRequest, bootstrap_project};
use pm_mcp::server::PmMcpServer;
use pm_mcp::tools::{
    SubmissionStatusArgs, SubmitTechnicalDesignArgs, TaskContextArgs, ToolError,
    get_submission_status, get_task_context, submit_technical_design,
};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use uuid::Uuid;

fn seed(code: &str) -> BootstrapRequest {
    BootstrapRequest {
        project_code: code.to_string(),
        project_name: format!("project {code}"),
    }
}

/// Minimal `delivery.task` row; the tools only need its id, project and title.
async fn insert_task(pool: &PgPool, project_id: Uuid, title: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO delivery.task (project_id, title) VALUES ($1, $2) RETURNING id")
        .bind(project_id)
        .bind(title)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// One inbound event row (status `received`) plus its `ai_analysis` job, the
/// way `submit_event` would have committed it — used when the status test
/// wants full control over the event id.
async fn insert_event(pool: &PgPool, project_id: Uuid, actor_id: Uuid) -> Uuid {
    let event_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO integration.inbound_event \
         (id, project_id, actor_id, source, event_type, idempotency_key, payload, payload_hash) \
         VALUES ($1, $2, $3, 'mcp', 'technical_design_submitted', 'seed-key', '{}', 'seed-hash')",
    )
    .bind(event_id)
    .bind(project_id)
    .bind(actor_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO integration.job (project_id, event_id, job_type) VALUES ($1, $2, 'ai_analysis')")
        .bind(project_id)
        .bind(event_id)
        .execute(pool)
        .await
        .unwrap();
    event_id
}

async fn insert_analysis_run(
    pool: &PgPool,
    project_id: Uuid,
    event_id: Uuid,
    status: &str,
    created_at_expr: &str,
) -> Uuid {
    // `created_at_expr` is a raw SQL timestamptz expression (e.g. "now()",
    // "now() + interval '1 hour'") — test-controlled constants only, never
    // external input, hence interpolation instead of a bind parameter.
    let sql = format!(
        "INSERT INTO ai.analysis_run (project_id, event_id, status, model_name, policy_version, \
         config_snapshot, created_at) \
         VALUES ($1, $2, $3, 'fake', 1, '{{}}'::jsonb, {created_at_expr}) RETURNING id"
    );
    sqlx::query_scalar(&sql)
        .bind(project_id)
        .bind(event_id)
        .bind(status)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn insert_proposal(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
    seq: i32,
    command_type: &str,
    status: &str,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO ai.change_proposal \
         (project_id, analysis_run_id, proposal_seq, target_type, command_type, proposed_data, \
          rationale, risk_level, status) \
         VALUES ($1, $2, $3, 'task', $4, '{}'::jsonb, 'because', 'low', $5) RETURNING id",
    )
    .bind(project_id)
    .bind(run_id)
    .bind(seq)
    .bind(command_type)
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn submit_args(project_id: Uuid, task_id: Uuid, idempotency_key: &str) -> SubmitTechnicalDesignArgs {
    SubmitTechnicalDesignArgs {
        project_id,
        task_ref: task_id.to_string(),
        idempotency_key: idempotency_key.to_string(),
        fixture: "cache_overlap".to_string(),
        summary: "switch the login cache to a write-through layer".to_string(),
    }
}

/// Step 1 test 1: the tool accepts a submission (persists the event with the
/// pinned `source`/`event_type`/payload plus its `ai_analysis` job) and a
/// second submission with the same `idempotency_key` replays the same event.
#[sqlx::test(migrations = "../../migrations")]
async fn submit_technical_design_persists_and_replays_idempotently(pool: PgPool) {
    let boot = bootstrap_project(&pool, seed("MCP-1")).await.unwrap();
    let task_id = insert_task(&pool, boot.project_id, "login latency").await;

    let first = submit_technical_design(&pool, submit_args(boot.project_id, task_id, "mcp-1"))
        .await
        .unwrap();
    assert_eq!(first["processing_status"], "queued");
    let event_id = first["event_id"].as_str().unwrap();

    let event = sqlx::query(
        "SELECT source, event_type, idempotency_key, payload, status \
         FROM integration.inbound_event WHERE id = $1",
    )
    .bind(Uuid::parse_str(event_id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event.get::<String, _>("source"), "mcp");
    assert_eq!(event.get::<String, _>("event_type"), "technical_design_submitted");
    assert_eq!(event.get::<String, _>("idempotency_key"), "mcp-1");
    assert_eq!(event.get::<String, _>("status"), "received");
    assert_eq!(
        event.get::<Value, _>("payload"),
        json!({
            "task_ref": task_id.to_string(),
            "fixture": "cache_overlap",
            "summary": "switch the login cache to a write-through layer",
        })
    );

    let job_status: String = sqlx::query_scalar(
        "SELECT status FROM integration.job WHERE event_id = $1 AND job_type = 'ai_analysis'",
    )
    .bind(Uuid::parse_str(event_id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(job_status, "queued");

    // Same key + same payload: idempotent replay of the same event.
    let second = submit_technical_design(&pool, submit_args(boot.project_id, task_id, "mcp-1"))
        .await
        .unwrap();
    assert_eq!(second["event_id"], first["event_id"]);
    assert_eq!(second["processing_status"], "duplicate");
}

/// Step 1 test 2: the status tool aggregates the event status, an empty
/// proposal list (and no analysis run) before the pipeline ran, and the
/// proposal summaries ordered by `proposal_seq` plus the latest run once the
/// pipeline produced them.
#[sqlx::test(migrations = "../../migrations")]
async fn submission_status_aggregates_event_proposals_and_latest_run(pool: PgPool) {
    let boot = bootstrap_project(&pool, seed("MCP-2")).await.unwrap();
    let event_id = insert_event(&pool, boot.project_id, boot.human_id).await;

    let empty = get_submission_status(
        &pool,
        SubmissionStatusArgs { event_id },
    )
    .await
    .unwrap();
    assert_eq!(empty["event_id"], json!(event_id));
    assert_eq!(empty["status"], "received");
    assert_eq!(empty["proposals"], json!([]));
    assert!(empty["analysis_run"].is_null());

    // The pipeline's output: one run with two proposals out of order, then a
    // newer run (status only) — the tool must show the proposals of all runs
    // of the event ordered by seq and the newest run's status.
    let run_old = insert_analysis_run(&pool, boot.project_id, event_id, "succeeded", "now()")
        .await;
    let run_new = insert_analysis_run(
        &pool,
        boot.project_id,
        event_id,
        "succeeded",
        "now() + interval '1 hour'",
    )
    .await;
    let p1 = insert_proposal(&pool, boot.project_id, run_old, 1, "update_task_status", "executed")
        .await;
    let p0 = insert_proposal(&pool, boot.project_id, run_old, 0, "create_task", "proposed").await;
    let _ = insert_proposal(&pool, boot.project_id, run_new, 0, "update_task_status", "proposed")
        .await;

    let full = get_submission_status(&pool, SubmissionStatusArgs { event_id })
        .await
        .unwrap();
    assert_eq!(full["status"], "received");
    assert_eq!(
        full["analysis_run"],
        json!({"id": run_new, "status": "succeeded"})
    );
    let proposals = full["proposals"].as_array().unwrap();
    assert_eq!(proposals.len(), 3);
    // Ordered by proposal_seq across runs: seq 0 rows first (order between
    // equal seqs is unspecified), then seq 1.
    assert_eq!(proposals[2]["proposal_seq"], 1);
    assert_eq!(proposals[2]["id"], json!(p1));
    assert_eq!(proposals[2]["status"], "executed");
    assert_eq!(proposals[2]["command_type"], "update_task_status");
    let seq0: Vec<&Value> = proposals
        .iter()
        .filter(|p| p["proposal_seq"] == 0)
        .collect();
    assert_eq!(seq0.len(), 2);
    assert!(
        seq0.iter().any(|p| p["id"] == json!(p0))
            && seq0.iter().all(|p| p["command_type"] == "update_task_status"
                || p["command_type"] == "create_task")
    );
    assert!(
        proposals
            .windows(2)
            .all(|w| w[0]["proposal_seq"].as_i64() <= w[1]["proposal_seq"].as_i64())
    );
}

/// Step 1 test 3a: `pm_get_task_context` returns the task snapshot for a task
/// of the caller's project.
#[sqlx::test(migrations = "../../migrations")]
async fn task_context_returns_task_of_own_project(pool: PgPool) {
    let boot = bootstrap_project(&pool, seed("MCP-3")).await.unwrap();
    let task_id = insert_task(&pool, boot.project_id, "rewrite ingest path").await;

    let context = get_task_context(
        &pool,
        TaskContextArgs {
            project_id: boot.project_id,
            task_ref: task_id.to_string(),
        },
    )
    .await
    .unwrap();
    assert_eq!(context["task_id"], json!(task_id));
    assert_eq!(context["title"], "rewrite ingest path");
    assert_eq!(context["status"], "backlog");
    assert_eq!(context["revision"], 1);
    assert!(context["requirement"].is_null());
    assert!(context["milestone"].is_null());
}

/// Step 1 test 3b: the same task read through another project's lens is a
/// tool error (cross-project invisibility), not a panic.
#[sqlx::test(migrations = "../../migrations")]
async fn task_context_of_another_project_is_a_tool_error(pool: PgPool) {
    let boot_a = bootstrap_project(&pool, seed("MCP-4A")).await.unwrap();
    let boot_b = bootstrap_project(&pool, seed("MCP-4B")).await.unwrap();
    let task_id = insert_task(&pool, boot_a.project_id, "a-owned task").await;

    let err = get_task_context(
        &pool,
        TaskContextArgs {
            project_id: boot_b.project_id,
            task_ref: task_id.to_string(),
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::NotFound(_)), "got {err:?}");
    assert!(err.to_string().contains("not found"));
}

/// Step 1 test 4: submissions and status reads against unknown ids are tool
/// errors — the unknown project never reaches a foreign-key panic and the
/// unknown event reads as `NotFound`.
#[sqlx::test(migrations = "../../migrations")]
async fn unknown_project_and_event_are_tool_errors(pool: PgPool) {
    let boot = bootstrap_project(&pool, seed("MCP-5")).await.unwrap();
    let task_id = insert_task(&pool, boot.project_id, "owned").await;

    let err = submit_technical_design(
        &pool,
        SubmitTechnicalDesignArgs {
            project_id: Uuid::new_v4(),
            task_ref: task_id.to_string(),
            idempotency_key: "ghost-1".to_string(),
            fixture: "cache_overlap".to_string(),
            summary: "s".to_string(),
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::NotFound(_)), "got {err:?}");

    let err = get_submission_status(
        &pool,
        SubmissionStatusArgs { event_id: Uuid::new_v4() },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::NotFound(_)), "got {err:?}");
}

/// Step 1 test 4b (transport smoke): the rmcp server, driven over the real
/// streamable HTTP transport in-process, lists exactly the three pinned tools
/// and answers a tool call with an MCP error result for an unknown task.
#[sqlx::test(migrations = "../../migrations")]
async fn streamable_http_smoke_lists_tools_and_serves_calls(pool: PgPool) {
    let service = PmMcpServer::streamable_http_service(pool.clone());
    let app = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    // .no_proxy(): host proxy env residue (cargo config's 127.0.0.1:10808)
    // must never route loopback MCP traffic — same fix as the Task 17 adapter.
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let url = format!("http://{addr}/mcp");

    // tools/list: the three tool names are registered. The streamable HTTP
    // server rejects requests whose Accept header lacks both JSON and SSE,
    // so every request below carries it.
    let body: Value = client
        .post(&url)
        .header("Accept", "application/json, text/event-stream")
        .json(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(body.get("error").is_none(), "tools/list failed: {body}");
    let names: Vec<&str> = body["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|t| t["name"].as_str().expect("tool name"))
        .collect();
    assert_eq!(
        names,
        vec![
            "pm_get_submission_status",
            "pm_get_task_context",
            "pm_submit_technical_design",
        ]
    );

    // tools/call, error branch: a task that does not exist in the project
    // answers as an MCP tool error (isError), not a JSON-RPC failure.
    let ghost_project = Uuid::new_v4();
    let body: Value = client
        .post(&url)
        .header("Accept", "application/json, text/event-stream")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "pm_get_task_context",
                "arguments": {
                    "project_id": ghost_project,
                    "task_ref": Uuid::new_v4().to_string(),
                },
            },
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        body["result"]["isError"].as_bool().unwrap_or(false),
        "expected isError result, got {body}"
    );

    // tools/call, success branch: the seeded task comes back as structured
    // content.
    let boot = bootstrap_project(&pool, seed("MCP-SMOKE")).await.unwrap();
    let task_id = insert_task(&pool, boot.project_id, "smoke task").await;
    let body: Value = client
        .post(&url)
        .header("Accept", "application/json, text/event-stream")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "pm_get_task_context",
                "arguments": {
                    "project_id": boot.project_id,
                    "task_ref": task_id.to_string(),
                },
            },
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(body.get("error").is_none(), "tools/call failed: {body}");
    assert_eq!(
        body["result"]["structuredContent"]["title"],
        json!("smoke task")
    );
}
