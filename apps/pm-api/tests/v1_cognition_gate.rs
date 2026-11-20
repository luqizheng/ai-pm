//! 阶段 C · V-1 人工确认主体门负向测试（V1-T01~T12）。
//!
//! 口径（交付物 05 V1.1 §3.1 + 任务书 §5.3）：
//! - `regression_*` = Security Regression，计入 G2 通过统计；
//! - `char_*` = Characterization（现状基线），**不计入通过**。
//!
//! SEC-P1-1 已修复（两个逐项决策入口统一 `require_human_decision_context`）：
//! V1-T02/T05 与 V1-EXT-01 已由 Characterization 转为 Regression 拒绝断言；
//! 剩余 Characterization = T07-B（Repository 调用边界）与 T08（工具面无确认路径）。

use axum::http::StatusCode;
use pm_application::repo::CognitionRepo;
use pm_infrastructure::repo_cognition::PgCognitionRepo;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

mod l2_security;

use l2_security::*;

fn decision_uri(project_id: Uuid, item_id: Uuid) -> String {
    format!("/v1/pm/projects/{project_id}/cognition/items/{item_id}/decision")
}

fn confirm_body() -> Value {
    json!({ "action": "confirm" })
}

// ───────────────────────────────────────────────────────────── V1-T01

/// V1-T01（Regression）：Human interactive 主体正常确认 → 200；
/// `decided_by` = 认证真人；`ai.project_knowledge` 落行且 `confirmed_by` = 真人。
#[sqlx::test(migrations = "../../migrations")]
async fn regression_t01_human_confirm_succeeds_and_audits(pool: PgPool) {
    let (a, _b) = dual_tenants(&pool).await;
    let items = start_cognition_items(&a.app, &a.token, a.project_id).await;
    let item = items[0];

    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "POST",
            &decision_uri(a.project_id, item),
            Some(&a.token),
            Some(confirm_body()),
        ))
        .await
        .unwrap();
    let (status, body) = status_body(response).await;
    assert_eq!(status, StatusCode::OK, "human confirm must pass: {body}");
    assert_eq!(body["state"], json!("confirmed"));
    let knowledge_id = Uuid::parse_str(body["knowledge_id"].as_str().unwrap()).unwrap();

    let (state, decided_by, linked): (String, Option<Uuid>, Option<Uuid>) = sqlx::query_as(
        "SELECT state, decided_by, knowledge_id FROM ai.cognition_item WHERE id = $1",
    )
    .bind(item)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(state, "confirmed");
    assert_eq!(decided_by, Some(a.human_id), "decided_by must be the human");
    assert_eq!(linked, Some(knowledge_id));

    let (confirmed_by, knowledge_status): (Uuid, String) =
        sqlx::query_as("SELECT confirmed_by, status FROM ai.project_knowledge WHERE id = $1")
            .bind(knowledge_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(confirmed_by, a.human_id);
    assert_eq!(knowledge_status, "active");
}

// ───────────────────────────────────────────────────────────── V1-T02

/// V1-T02（Regression，SEC-P1-1 修复后）：Agent（dev_agent）主体直接调用正式确认
/// → 403（非交互式真人/非 human kind），条目仍 proposed，DB 快照无变化。
#[sqlx::test(migrations = "../../migrations")]
async fn regression_t02_agent_principal_cannot_confirm(pool: PgPool) {
    let (a, _b) = dual_tenants(&pool).await;
    let agent = agent_token(&pool, &a).await;
    let items = start_cognition_items(&a.app, &a.token, a.project_id).await;
    let item = items[0];
    let before = ProjectSnapshot::capture(&pool, a.project_id).await;

    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "POST",
            &decision_uri(a.project_id, item),
            Some(&agent),
            Some(confirm_body()),
        ))
        .await
        .unwrap();
    let body = expect_cross_tenant_denied(
        &pool,
        a.project_id,
        &before,
        response,
        // P4-ORG Phase 4：Agent 主体无 project_member 行 → 可见性闸 404（存在性
        // 隐藏）；若有行（历史遗留）则仍在 H8 闸 403。两种拒绝形态都合规。
        &[StatusCode::FORBIDDEN, StatusCode::NOT_FOUND],
    )
    .await;
    assert!(
        body["message"].as_str().unwrap_or_default().is_empty()
            || body["message"]
                .as_str()
                .unwrap_or_default()
                .contains("human")
            || body["message"] == json!("project not found"),
        "denial shape must be 404 visibility or 403 human-gate: {body}"
    );
}

// ───────────────────────────────────────────────────────────── V1-T03

/// V1-T03（Regression）：Service JWT 调正式确认 → 403 IAM_PERM_3003；
/// 条目状态与全表快照无变化（拒绝发生在鉴权层，未触达业务写入）。
#[sqlx::test(migrations = "../../migrations")]
async fn regression_t03_service_jwt_cannot_confirm(pool: PgPool) {
    let (a, _b) = dual_tenants(&pool).await;
    let items = start_cognition_items(&a.app, &a.token, a.project_id).await;
    let before = ProjectSnapshot::capture(&pool, a.project_id).await;

    let service = sign_service("svc-l2", 1);
    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "POST",
            &decision_uri(a.project_id, items[0]),
            Some(&service),
            Some(confirm_body()),
        ))
        .await
        .unwrap();
    let body = expect_cross_tenant_denied(
        &pool,
        a.project_id,
        &before,
        response,
        &[StatusCode::FORBIDDEN],
    )
    .await;
    assert_eq!(body["code"], json!("IAM_PERM_3003"));
}

// ───────────────────────────────────────────────────────────── V1-T04

/// V1-T04（Regression）：未认证主体 → 401 IAM_AUTH_1005；无写入。
#[sqlx::test(migrations = "../../migrations")]
async fn regression_t04_unauthenticated_cannot_confirm(pool: PgPool) {
    let (a, _b) = dual_tenants(&pool).await;
    let items = start_cognition_items(&a.app, &a.token, a.project_id).await;
    let before = ProjectSnapshot::capture(&pool, a.project_id).await;

    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "POST",
            &decision_uri(a.project_id, items[0]),
            None,
            Some(confirm_body()),
        ))
        .await
        .unwrap();
    let body = expect_cross_tenant_denied(
        &pool,
        a.project_id,
        &before,
        response,
        &[StatusCode::UNAUTHORIZED],
    )
    .await;
    assert_eq!(body["code"], json!("IAM_AUTH_1005"));
}

// ───────────────────────────────────────────────────────────── V1-T05

/// V1-T05（Regression，SEC-P1-1 修复后）：Agent 主体 + 请求体/头伪造
/// `kind=human` / `decided_by=真人` → 仍 403；声明字段不参与鉴权。
#[sqlx::test(migrations = "../../migrations")]
async fn regression_t05_forged_claim_still_rejected(pool: PgPool) {
    let (a, _b) = dual_tenants(&pool).await;
    let agent = agent_token(&pool, &a).await;
    let items = start_cognition_items(&a.app, &a.token, a.project_id).await;
    let item = items[0];
    let before = ProjectSnapshot::capture(&pool, a.project_id).await;

    let forged_body = json!({
        "action": "confirm",
        "kind": "human",
        "principal_kind": "human",
        "is_human": true,
        "decided_by": a.human_id,
    });
    let response = a
        .app
        .clone()
        .oneshot(req_json_headers(
            "POST",
            &decision_uri(a.project_id, item),
            Some(&agent),
            Some(forged_body),
            &[
                ("x-ai-pm-principal-kind", "human"),
                ("x-ai-pm-actor-kind", "human"),
            ],
        ))
        .await
        .unwrap();
    let body = expect_cross_tenant_denied(
        &pool,
        a.project_id,
        &before,
        response,
        &[StatusCode::FORBIDDEN, StatusCode::NOT_FOUND],
    )
    .await;
    assert!(
        body["message"].as_str().unwrap_or_default().is_empty()
            || body["message"]
                .as_str()
                .unwrap_or_default()
                .contains("human")
            || body["message"] == json!("project not found"),
        "forged claims must not upgrade the actor: {body}"
    );
}

// ───────────────────────────────────────────────────────────── V1-T06

/// V1-T06（Regression）：Service 主体 + 伪造 Human 声明 → 仍 403；
/// 声明字段不参与鉴权。
#[sqlx::test(migrations = "../../migrations")]
async fn regression_t06_forged_human_claim_cannot_upgrade_service(pool: PgPool) {
    let (a, _b) = dual_tenants(&pool).await;
    let items = start_cognition_items(&a.app, &a.token, a.project_id).await;
    let before = ProjectSnapshot::capture(&pool, a.project_id).await;

    let service = sign_service("svc-l2-forged", 1);
    let response = a
        .app
        .clone()
        .oneshot(req_json_headers(
            "POST",
            &decision_uri(a.project_id, items[0]),
            Some(&service),
            Some(json!({
                "action": "confirm",
                "kind": "human",
                "principal_kind": "human",
                "decided_by": a.human_id,
            })),
            &[
                ("x-ai-pm-principal-kind", "human"),
                ("x-ai-pm-actor-kind", "human"),
            ],
        ))
        .await
        .unwrap();
    let body = expect_cross_tenant_denied(
        &pool,
        a.project_id,
        &before,
        response,
        &[StatusCode::FORBIDDEN],
    )
    .await;
    assert_eq!(body["code"], json!("IAM_PERM_3003"));
}

// ───────────────────────────────────────────────────────────── SEC-P1-1 补充

/// SEC-P1-1 补充（Regression）：真人 principal 的 **API Key 形态**凭据
/// （`purpose=api_key`）调决策 → 403；契约要求「交互式真人会话」而非仅 kind=human。
#[sqlx::test(migrations = "../../migrations")]
async fn regression_decision_rejects_api_key_human(pool: PgPool) {
    let (a, _b) = dual_tenants(&pool).await;
    let items = start_cognition_items(&a.app, &a.token, a.project_id).await;
    let before = ProjectSnapshot::capture(&pool, a.project_id).await;

    let api_key = sign_api_key(&a.human_id.to_string(), a.tid, &[]);
    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "POST",
            &decision_uri(a.project_id, items[0]),
            Some(&api_key),
            Some(confirm_body()),
        ))
        .await
        .unwrap();
    let body = expect_cross_tenant_denied(
        &pool,
        a.project_id,
        &before,
        response,
        &[StatusCode::FORBIDDEN],
    )
    .await;
    assert!(
        body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("interactive"),
        "api_key must be rejected as non-interactive: {body}"
    );
}

/// V1-T07-B（**Characterization**，不计入通过）：Repository 层内部调用边界登记。
///
/// 事实：`PgCognitionRepo::decide_item` 只接收 `decided_by: Uuid`，不做
/// JWT/Human 识别（身份授权职责在 Application/Service 层——设计口径正确）。
/// SEC-P1-1 修复把 Human 门放在 HTTP handler（两个入口），Repository 保持
/// 身份无关；本用例直接以 dev_agent principal 调 Repository → 成功，固化该边界。
///
/// V1-T07-A（服务层授权边界）登记为 UNKNOWN：代码中不存在独立于 HTTP handler
/// 的决策 Application Service；前置 SEC-P1-1 未落地。
#[sqlx::test(migrations = "../../migrations")]
async fn char_t07b_repository_decide_item_has_no_principal_gate(pool: PgPool) {
    let (a, _b) = dual_tenants(&pool).await;
    let items = start_cognition_items(&a.app, &a.token, a.project_id).await;
    let item = items[0];

    let repo = PgCognitionRepo::new(pool.clone());
    let decision = repo
        .decide_item(
            a.tenant_id,
            item,
            "confirmed",
            a.agent_id,
            Some("l2-t07b characterization"),
            None,
        )
        .await
        .expect("Characterization baseline：Repository 不识别主体类型，直接调用成功");
    assert!(
        decision.knowledge_id.is_some(),
        "confirm 必须产出 knowledge 行"
    );

    let decided_by: Option<Uuid> =
        sqlx::query_scalar("SELECT decided_by FROM ai.cognition_item WHERE id = $1")
            .bind(item)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(decided_by, Some(a.agent_id));
}

// ───────────────────────────────────────────────────────────── V1-T08

/// V1-EXT-01（Regression，SEC-P1-1 修复后）：Project Baseline 逐项决策是第二个
/// 正式状态写入入口（`pm_baseline.rs::decide_item`）——Agent 主体 → 403，
/// item 仍 proposed，DB 快照不变。
#[sqlx::test(migrations = "../../migrations")]
async fn regression_v1_ext01_baseline_item_requires_human(pool: PgPool) {
    let (a, _b) = dual_tenants(&pool).await;
    let agent = agent_token(&pool, &a).await;

    let proposal_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.baseline_proposal (tenant_id, project_id, version, status, created_by) \
         VALUES ($1, $2, 1, 'pending', $3) RETURNING id",
    )
    .bind(a.tenant_id)
    .bind(a.project_id)
    .bind(a.human_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let item_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.baseline_item (tenant_id, project_id, proposal_id, category, content, state) \
         VALUES ($1, $2, $3, 'identity', 'l2 baseline regression', 'proposed') RETURNING id",
    )
    .bind(a.tenant_id)
    .bind(a.project_id)
    .bind(proposal_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let before = ProjectSnapshot::capture(&pool, a.project_id).await;

    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "POST",
            &format!(
                "/v1/pm/projects/{}/baseline/items/{}/decision",
                a.project_id, item_id
            ),
            Some(&agent),
            Some(json!({ "action": "confirm" })),
        ))
        .await
        .unwrap();
    let body = expect_cross_tenant_denied(
        &pool,
        a.project_id,
        &before,
        response,
        &[StatusCode::FORBIDDEN, StatusCode::NOT_FOUND],
    )
    .await;
    assert!(
        body["message"].as_str().unwrap_or_default().is_empty()
            || body["message"]
                .as_str()
                .unwrap_or_default()
                .contains("human")
            || body["message"] == json!("project not found"),
        "baseline item decision must require a human principal: {body}"
    );
}

/// V1-T08（Characterization，不计入通过）：后台任务/Worker 无可达的正式确认入口。
///
/// 运行时证据：MCP 工具面无 cognition 决策工具（tools/list 枚举断言）。
/// 代码证据（D04 登记）：`ai-pm-worker` 不调用 `decide_item`；`ai.cognition_item`
/// 状态变更唯一入口是 HTTP decision handler（repo_cognition.rs FOR UPDATE）。
#[sqlx::test(migrations = "../../migrations")]
async fn char_t08_mcp_tool_surface_has_no_cognition_decision_tool(pool: PgPool) {
    let (a, _b) = dual_tenants(&pool).await;
    let token = sign_claims(json!({
        "sub": a.human_id.to_string(),
        "tid": a.tid,
        "username": "alice",
        "roles": [],
        "permissions": ["PM_MCP_READ"],
        "aud": "coder-micro-services",
    }));

    let response = a
        .app
        .clone()
        .oneshot(mcp_request(
            &token,
            json!({"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}),
        ))
        .await
        .unwrap();
    let (status, body) = status_body(response).await;
    assert_eq!(status, StatusCode::OK, "tools/list failed: {body}");
    let tools = body["result"]["tools"]
        .as_array()
        .expect("tools array in tools/list result");
    let names: Vec<&str> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap_or_default())
        .collect();
    assert!(
        !names.is_empty(),
        "tools/list must enumerate the tool surface"
    );
    assert!(
        !names
            .iter()
            .any(|name| name.contains("cognition") || name.contains("decide")),
        "no worker/agent tool may reach cognition confirmation; tools={names:?}"
    );
}

// ───────────────────────────────────────────────────────────── V1-T09

/// V1-T09（Regression）：owner 的 cpk 凭据打业务决策端点 → 401 IAM_AUTH_1007
/// （路径门：cpk 仅 /mcp）；无写入。
#[sqlx::test(migrations = "../../migrations")]
async fn regression_t09_cpk_owner_blocked_on_business_decision(pool: PgPool) {
    let (a, _b) = dual_tenants(&pool).await;
    let items = start_cognition_items(&a.app, &a.token, a.project_id).await;
    let cpk = issue_cpk(&pool, a.tenant_id, a.human_id).await;
    let before = ProjectSnapshot::capture(&pool, a.project_id).await;

    let response = a
        .app
        .clone()
        .oneshot(req_json(
            "POST",
            &decision_uri(a.project_id, items[0]),
            Some(&cpk.plaintext),
            Some(confirm_body()),
        ))
        .await
        .unwrap();
    let body = expect_cross_tenant_denied(
        &pool,
        a.project_id,
        &before,
        response,
        &[StatusCode::UNAUTHORIZED],
    )
    .await;
    assert_eq!(body["code"], json!("IAM_AUTH_1007"));
}

// ───────────────────────────────────────────────────────────── V1-T10 / T11

/// V1-T10（Regression）：Agent 进程持 owner cpk 打业务决策端点 → 同一路径门 401；
/// 附加伪造「我是 Agent」头不改变通道判定。
#[sqlx::test(migrations = "../../migrations")]
async fn regression_t10_agent_process_with_cpk_blocked(pool: PgPool) {
    let (a, _b) = dual_tenants(&pool).await;
    let items = start_cognition_items(&a.app, &a.token, a.project_id).await;
    let cpk = issue_cpk(&pool, a.tenant_id, a.human_id).await;
    let before = ProjectSnapshot::capture(&pool, a.project_id).await;

    let response = a
        .app
        .clone()
        .oneshot(req_json_headers(
            "POST",
            &decision_uri(a.project_id, items[0]),
            Some(&cpk.plaintext),
            Some(confirm_body()),
            &[
                ("x-ai-pm-principal-kind", "agent"),
                ("x-ai-pm-device-id", &cpk.device_id.to_string()),
            ],
        ))
        .await
        .unwrap();
    let body = expect_cross_tenant_denied(
        &pool,
        a.project_id,
        &before,
        response,
        &[StatusCode::UNAUTHORIZED],
    )
    .await;
    assert_eq!(body["code"], json!("IAM_AUTH_1007"));
}

/// V1-T11（Regression）：Service 进程持 owner cpk 打业务决策端点 → 同一 401。
#[sqlx::test(migrations = "../../migrations")]
async fn regression_t11_service_process_with_cpk_blocked(pool: PgPool) {
    let (a, _b) = dual_tenants(&pool).await;
    let items = start_cognition_items(&a.app, &a.token, a.project_id).await;
    let cpk = issue_cpk(&pool, a.tenant_id, a.human_id).await;
    let before = ProjectSnapshot::capture(&pool, a.project_id).await;

    let response = a
        .app
        .clone()
        .oneshot(req_json_headers(
            "POST",
            &decision_uri(a.project_id, items[0]),
            Some(&cpk.plaintext),
            Some(confirm_body()),
            &[("x-ai-pm-principal-kind", "service")],
        ))
        .await
        .unwrap();
    let body = expect_cross_tenant_denied(
        &pool,
        a.project_id,
        &before,
        response,
        &[StatusCode::UNAUTHORIZED],
    )
    .await;
    assert_eq!(body["code"], json!("IAM_AUTH_1007"));
}

// ───────────────────────────────────────────────────────────── V1-T12 + R3

/// V1-T12（Regression）：撤销 owner 的**项目成员资格**后，旧 cpk 调 `/mcp` 工具
/// → 工具层 `ensure_member` 拒绝（isError + not found）；命令/项目数据零写入。
#[sqlx::test(migrations = "../../migrations")]
async fn regression_t12_cpk_after_project_membership_revoked(pool: PgPool) {
    let (a, _b) = dual_tenants(&pool).await;
    let task = seed_task(&pool, a.project_id, "L2-MCP-TASK", "ready").await;
    let cpk = issue_cpk(&pool, a.tenant_id, a.human_id).await;
    let before = ProjectSnapshot::capture(&pool, a.project_id).await;

    // 撤销前：合法成员可读。
    let response = a
        .app
        .clone()
        .oneshot(mcp_tool_call(
            &cpk.plaintext,
            "pm_list_tasks",
            json!({ "project_id": a.project_id }),
        ))
        .await
        .unwrap();
    let (status, body) = status_body(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        !mcp_is_error(&body),
        "member must read tasks before revocation: {body}"
    );
    assert!(
        mcp_text(&body).contains(&task.to_string()),
        "task must be visible before revocation: {body}"
    );

    sqlx::query("DELETE FROM core.project_member WHERE project_id = $1 AND principal_id = $2")
        .bind(a.project_id)
        .bind(a.human_id)
        .execute(&pool)
        .await
        .unwrap();

    // 撤销后：旧 cpk 仍在有效期内，但成员门拒绝（四项链 owner_active 之外的项目授权）。
    let response = a
        .app
        .clone()
        .oneshot(mcp_tool_call(
            &cpk.plaintext,
            "pm_list_tasks",
            json!({ "project_id": a.project_id }),
        ))
        .await
        .unwrap();
    let (status, body) = status_body(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        mcp_is_error(&body),
        "revoked project membership must fail the tool call: {body}"
    );
    assert!(
        mcp_text(&body).contains("not found"),
        "non-member must read as not found: {body}"
    );

    let after = ProjectSnapshot::capture(&pool, a.project_id).await;
    assert_eq!(after, before, "no project data may change");
}

/// R3 补充（Regression）：凭据撤销（`credential_revoked_at` 置位）后任意 /mcp
/// 调用 → 401 IAM_AUTH_1008（约束 C：下一请求即时失效，无缓存窗口）。
#[sqlx::test(migrations = "../../migrations")]
async fn regression_r3_revoked_credential_is_immediately_rejected(pool: PgPool) {
    let (a, _b) = dual_tenants(&pool).await;
    let task = seed_task(&pool, a.project_id, "L2-R3-TASK", "ready").await;
    let cpk = issue_cpk(&pool, a.tenant_id, a.human_id).await;
    let before = ProjectSnapshot::capture(&pool, a.project_id).await;

    let ok = a
        .app
        .clone()
        .oneshot(mcp_tool_call(
            &cpk.plaintext,
            "pm_list_tasks",
            json!({ "project_id": a.project_id }),
        ))
        .await
        .unwrap();
    let (status, body) = status_body(ok).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!mcp_is_error(&body));
    assert!(mcp_text(&body).contains(&task.to_string()));

    revoke_cpk(&pool, a.tenant_id, cpk.device_id, a.human_id).await;

    let rejected = a
        .app
        .clone()
        .oneshot(mcp_tool_call(
            &cpk.plaintext,
            "pm_list_tasks",
            json!({ "project_id": a.project_id }),
        ))
        .await
        .unwrap();
    let (status, body) = status_body(rejected).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["code"], json!("IAM_AUTH_1008"));

    let after = ProjectSnapshot::capture(&pool, a.project_id).await;
    assert_eq!(after, before, "revoked credential must not write anything");
}
