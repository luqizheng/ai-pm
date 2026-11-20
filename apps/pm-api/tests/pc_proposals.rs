//! KP-2 · PC 确认闭环 REST 面（P4 提案详情 / P5 审批转达 / P3 结论展开）。
//!
//! | 用例 | 断言焦点 |
//! |---|---|
//! | 1 | P4：提案头 + 成员展示投影 + 审批历史 + approval_ref；项目作用域 404（非成员项目 / 他项目提案 / 非法 ID）；缺 project_id 400 |
//! | 2 | P5：approve = 审批 + 发布（published_version_ref）、幂等重放；reject 终态；400（动作/CAS 缺失）/ 409（陈旧 CAS） |
//! | 3 | P5 门禁：普通成员 403（P4 仍可读）、AI 主体 403（服务层真人闸） |
//! | 4 | P5 A*：API Key 上下文 403，同主体交互式令牌通过（可信交互上下文闸） |
//! | 5 | P3：提案结论展开（PROPOSED + proposal_ref + 对象集）；NO_CHANGE 来源展开（KP-1 别名缺陷回归）+ versioned 发布前后翻转 |

mod common;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use digua_common_jwt::{sign_token, JWKSCache};
use pm_api::auth::AuthConfig;
use pm_api::routes::{router, router_with_auth_config};
use pm_application::actor::AuditActor;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::pc::{
    create_proposal, propose_from_analysis, publish_version, register_evidence, register_source,
    submit_proposal, upsert_object_draft, AnalysisProposalInput, AnalysisProposalOutcome,
    CreateProposalInput, EvidenceInput, EvidenceSubject, ProposalMemberPin, PublishVersionInput,
    SourceInput, UpsertObjectDraft,
};
use pm_domain::SYSTEM_TENANT;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;
use uuid::Uuid;

const PRIVATE_PEM: &str = include_str!("keys/private.pem");
const PUBLIC_PEM: &str = include_str!("keys/public.pem");

async fn seed_project(pool: &PgPool, tenant_id: Uuid, code: &str) -> Uuid {
    bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id,
        },
    )
    .await
    .expect("bootstrap project")
    .project_id
}

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

fn post_json(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn send(pool: &PgPool, request: Request<Body>) -> (StatusCode, Value) {
    let response = router(pool.clone()).oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn draft(object_type: &str, key: &str, content: &str) -> UpsertObjectDraft {
    UpsertObjectDraft {
        object_type: object_type.to_string(),
        object_key: Some(key.to_string()),
        display_name: key.to_string(),
        content_markdown: content.to_string(),
        change_summary: Some("rest fixture".into()),
        source_context: None,
        analysis_mode: None,
    }
}

/// 草稿对象 + 提交态提案（成员 create_object），返回 proposal_id。
async fn submitted_proposal(
    pool: &PgPool,
    project: Uuid,
    actor: &AuditActor,
    keys: &[&str],
    expected_version_no: i64,
) -> Uuid {
    let mut members = Vec::new();
    for key in keys {
        let receipt = upsert_object_draft(
            pool,
            project,
            actor,
            draft("feature", key, &format!("# {key}")),
        )
        .await
        .unwrap();
        members.push(ProposalMemberPin {
            change_kind: "create_object".into(),
            object_id: Some(receipt.object_id),
            object_revision_id: Some(receipt.revision_id),
            relationship_id: None,
            relationship_revision_id: None,
        });
    }
    let drafted = create_proposal(
        pool,
        project,
        actor,
        CreateProposalInput {
            title: format!("rest proposal {}", keys.join("+")),
            rationale: "rest fixture".into(),
            origin_kind: "manual_input".into(),
            source_id: None,
            governance_level: None,
            risk_level: None,
            confidence: None,
            members,
        },
    )
    .await
    .unwrap();
    assert_eq!(drafted.expected_version_no, expected_version_no);
    submit_proposal(pool, project, actor, drafted.proposal_id, None)
        .await
        .unwrap();
    drafted.proposal_id
}

fn dev() -> AuditActor {
    AuditActor::principal(pm_api::auth::DEV_OFF_PRINCIPAL)
}

// ── 用例 1：P4 详情 + 项目作用域 ─────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn p4_returns_proposal_detail_and_is_project_scoped(pool: PgPool) {
    let project = seed_project(&pool, SYSTEM_TENANT, "PCR-A").await;
    let other_member_project = seed_project(&pool, SYSTEM_TENANT, "PCR-A2").await;
    let stranger = seed_project(&pool, SYSTEM_TENANT, "PCR-A3").await;
    common::join_dev(&pool, project).await;
    common::join_dev(&pool, other_member_project).await;
    let proposal = submitted_proposal(&pool, project, &dev(), &["p4-a", "p4-b"], 0).await;

    let (status, body) = send(
        &pool,
        get(&format!("/v1/pc/proposals/{proposal}?project_id={project}")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["proposal_id"], proposal.to_string());
    assert_eq!(body["status"], "submitted");
    assert_eq!(body["governance_level"], "L1");
    assert_eq!(body["origin_kind"], "manual_input");
    assert_eq!(body["expected_version_no"], 0);
    assert!(body["content_hash"].as_str().unwrap().len() == 64);
    let members = body["members"].as_array().unwrap();
    assert_eq!(members.len(), 2);
    assert!(members.iter().all(|m| m["change_kind"] == "create_object"
        && m["object_type"] == "feature"
        && m["object_revision_no"] == 1));
    assert_eq!(body["approval_ref"]["target_domain"], "pc");
    assert_eq!(body["approval_ref"]["target_type"], "approval");
    assert_eq!(body["approvals"][0]["status"], "pending");
    assert!(body["published_version_ref"].is_null());

    // 项目作用域：非成员项目 404 / 他项目（可见）下查本提案 404 / 非法 ID 404 / 缺参 400
    for uri in [
        format!("/v1/pc/proposals/{proposal}?project_id={stranger}"),
        format!("/v1/pc/proposals/{proposal}?project_id={other_member_project}"),
        format!("/v1/pc/proposals/not-a-uuid?project_id={project}"),
    ] {
        let (status, _) = send(&pool, get(&uri)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
    let (status, _) = send(&pool, get(&format!("/v1/pc/proposals/{proposal}"))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── 用例 2：P5 approve 发布 / 幂等 / reject / 参数与 CAS ──────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn p5_approve_publishes_and_reject_closes(pool: PgPool) {
    let project = seed_project(&pool, SYSTEM_TENANT, "PCR-B").await;
    common::join_dev(&pool, project).await;
    let p1 = submitted_proposal(&pool, project, &dev(), &["p5-a"], 0).await;
    let decision_uri = |id: Uuid| format!("/v1/pc/proposals/{id}/decision?project_id={project}");

    // 参数校验：非法动作 / 缺 CAS → 400；陈旧 CAS → 409
    for body in [
        json!({ "action": "publish", "expected_version_no": 0 }),
        json!({ "action": "approve" }),
    ] {
        let (status, _) = send(&pool, post_json(&decision_uri(p1), body.clone())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
    let (status, body) = send(
        &pool,
        post_json(
            &decision_uri(p1),
            json!({ "action": "approve", "reason": "测试理由", "expected_version_no": 3 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    // approve = 决定暂记（审阅流 v2：approved/rejected 可互转，正式版本在报告
    // close 批量发布——91003f6 语义；本用例的正式版本走服务层显式发布）。
    let (status, body) = send(
        &pool,
        post_json(
            &decision_uri(p1),
            json!({ "action": "approve", "note": "认知确认", "expected_version_no": 0 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "approved");
    assert!(body["published_version_ref"].is_null());
    assert!(body["published_version_no"].is_null());
    assert!(body["snapshot_hash"].is_null());
    assert_eq!(body["approval_ref"]["target_type"], "approval");

    // 幂等重放：同态重放 → 同一审批事实，不产生新版本
    let (status, replay) = send(
        &pool,
        post_json(
            &decision_uri(p1),
            json!({ "action": "approve", "reason": "测试理由", "expected_version_no": 0 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{replay}");
    assert_eq!(replay["status"], "approved");
    let versions: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pc.cognition_version WHERE project_id = $1")
            .bind(project)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(versions, 0, "decide does not publish (P5 暂记)");

    // P4 回读：approved + 审批事实（决策人 = 调用方）；published_version_ref 恒 null
    let (_, view) = send(
        &pool,
        get(&format!("/v1/pc/proposals/{p1}?project_id={project}")),
    )
    .await;
    assert_eq!(view["status"], "approved");
    assert_eq!(view["approvals"][0]["status"], "approved");
    assert_eq!(view["approvals"][0]["decided_by_name"], "dev-off");
    assert_eq!(view["approvals"][0]["decision_reason"], "认知确认");
    assert!(view["published_version_ref"].is_null());

    // 正式发布（服务层显式补发布 = close 双域分流的同款入口；pc-decision 幂等键）
    let published = pm_application::pc::publish_version(
        &pool,
        project,
        &dev(),
        pm_application::pc::PublishVersionInput {
            proposal_id: p1,
        },
    )
    .await
    .unwrap();
    assert_eq!(published.version_no, 1);
    let version_ref = published.version_id.to_string();

    // P4 回读：applied + 已发布版本引用
    let (_, view) = send(
        &pool,
        get(&format!("/v1/pc/proposals/{p1}?project_id={project}")),
    )
    .await;
    assert_eq!(view["status"], "applied");
    assert_eq!(view["published_version_ref"]["target_type"], "cognition_version");
    assert_eq!(view["published_version_ref"]["target_id"], version_ref);

    // reject：基于版本 1 的新提案被驳回（reason 为 note 别名）
    let p2 = submitted_proposal(&pool, project, &dev(), &["p5-b"], 1).await;
    let (status, body) = send(
        &pool,
        post_json(
            &decision_uri(p2),
            json!({ "action": "reject", "reason": "证据不足", "expected_version_no": 1 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "rejected");
    assert!(body["published_version_ref"].is_null());
    // 决定终局（Q-02b/Q-11，GAP-35）：rejected 终态——跨态重决一律 409
    let (status, flipped) = send(
        &pool,
        post_json(
            &decision_uri(p2),
            json!({ "action": "approve", "reason": "测试理由", "expected_version_no": 1 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{flipped}");
    assert!(
        flipped["message"]
            .as_str()
            .unwrap()
            .contains("decisions are terminal"),
        "{flipped}"
    );

    let audits: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT action, actor_auth_mode FROM governance.audit_log \
          WHERE project_id = $1 AND action IN ('pc_proposal.approve','pc_proposal.reject') \
          ORDER BY created_at",
    )
    .bind(project)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        audits,
        vec![
            ("pc_proposal.approve".to_string(), Some("off".to_string())),
            ("pc_proposal.reject".to_string(), Some("off".to_string())),
        ]
    );
}

// ── 用例 3：P5 门禁（普通成员 / AI 主体）──────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn p5_forbids_plain_members_and_ai_principals(pool: PgPool) {
    let project = seed_project(&pool, SYSTEM_TENANT, "PCR-C").await;
    common::join_dev(&pool, project).await;
    let proposal = submitted_proposal(&pool, project, &dev(), &["gate-a"], 0).await;
    let uri = format!("/v1/pc/proposals/{proposal}/decision?project_id={project}");
    let approve = json!({ "action": "approve", "reason": "测试理由", "expected_version_no": 0 });

    // 普通成员：可读（P4 200）不可确认（P5 403，A3 闸）
    sqlx::query(
        "UPDATE core.project_member SET role = 'member' WHERE project_id = $1 AND principal_id = $2",
    )
    .bind(project)
    .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
    .execute(&pool)
    .await
    .unwrap();
    let (status, _) = send(
        &pool,
        get(&format!("/v1/pc/proposals/{proposal}?project_id={project}")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "members can read proposals");
    let (status, body) = send(&pool, post_json(&uri, approve.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    sqlx::query(
        "UPDATE core.project_member SET role = 'owner' WHERE project_id = $1 AND principal_id = $2",
    )
    .bind(project)
    .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
    .execute(&pool)
    .await
    .unwrap();

    // AI 主体（同一调用方改为 ai_pm）：Lead 身份亦不放行——服务层真人闸（Q2-C）
    sqlx::query("UPDATE core.principal SET kind = 'ai_pm' WHERE id = $1")
        .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = send(&pool, post_json(&uri, approve.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let (status, body) = send(
        &pool,
        post_json(
            &uri,
            json!({ "action": "reject", "reason": "测试理由", "expected_version_no": 0 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");

    let state: String = sqlx::query_scalar("SELECT status FROM pc.proposal WHERE id = $1")
        .bind(proposal)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, "submitted", "no gate leaked a decision");
}

// ── 用例 4：P5 A*（API Key 上下文 403）─────────────────────────────────────────

fn sign(claims: Value) -> String {
    sign_token(PRIVATE_PEM, &claims, Duration::from_secs(300)).expect("test token must sign")
}

#[sqlx::test(migrations = "../../migrations")]
async fn p5_rejects_api_key_context(pool: PgPool) {
    let tenant_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', '1', 'lmms:1') \
         ON CONFLICT (provider, external_key) DO UPDATE SET display_name = core.tenant.display_name \
         RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let project = seed_project(&pool, tenant_id, "PCR-D").await;
    let app = router_with_auth_config(
        pool.clone(),
        Arc::new(JWKSCache::from_pem(PUBLIC_PEM).unwrap()),
        AuthConfig {
            first_admin_tenant: Some("1".to_string()),
            first_admin_subject: Some("usr-lead".to_string()),
        },
    );
    let claims = |extra: Value| {
        let mut base = json!({
            "sub": "usr-lead", "tid": 1, "username": "usr-lead",
            "roles": ["tenant_admin"], "permissions": [],
        });
        base.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        sign(base)
    };
    let browser = claims(json!({ "aud": "coder-micro-services" }));
    let api_key = claims(json!({ "client_id": "key-abc" }));
    let call = |token: &str, method: &str, uri: &str, body: Option<Value>| {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {token}"));
        let body = match body {
            Some(value) => {
                builder = builder.header("content-type", "application/json");
                Body::from(value.to_string())
            }
            None => Body::empty(),
        };
        builder.body(body).unwrap()
    };

    // 首管理员激活 → 拿到主体 → 设为项目 Lead
    let me = app
        .clone()
        .oneshot(call(&browser, "GET", "/v1/pm/me", None))
        .await
        .unwrap();
    let me: Value =
        serde_json::from_slice(&to_bytes(me.into_body(), usize::MAX).await.unwrap()).unwrap();
    let lead = Uuid::parse_str(me["principal_id"].as_str().unwrap()).unwrap();
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ($1, $2, $3, 'owner') ON CONFLICT DO NOTHING",
    )
    .bind(tenant_id)
    .bind(project)
    .bind(lead)
    .execute(&pool)
    .await
    .unwrap();
    let proposal = submitted_proposal(
        &pool,
        project,
        &AuditActor::principal(lead),
        &["astar-a"],
        0,
    )
    .await;
    let uri = format!("/v1/pc/proposals/{proposal}/decision?project_id={project}");
    let approve = json!({ "action": "approve", "reason": "测试理由", "expected_version_no": 0 });

    // API Key：同一主体（Lead）也 403——确认须可信交互上下文
    let response = app
        .clone()
        .oneshot(call(&api_key, "POST", &uri, Some(approve.clone())))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    // P4 读面对 API Key 开放（不改变正式认知）
    let response = app
        .clone()
        .oneshot(call(
            &api_key,
            "GET",
            &format!("/v1/pc/proposals/{proposal}?project_id={project}"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // 交互式令牌：通过，审计记 interactive
    let response = app
        .clone()
        .oneshot(call(&browser, "POST", &uri, Some(approve)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let mode: Option<String> = sqlx::query_scalar(
        "SELECT actor_auth_mode FROM governance.audit_log \
          WHERE project_id = $1 AND action = 'pc_proposal.approve'",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(mode.as_deref(), Some("interactive"));
}

// ── 用例 5：P3 结论展开（提案 / NO_CHANGE 来源）──────────────────────────────

/// 一次分析运行：来源 + 对每个修订登记证据（与分析链同构）。
async fn analysis_source(pool: &PgPool, project: Uuid, revisions: &[Uuid]) -> Uuid {
    let dispatch = AuditActor::dispatch();
    let source_id = register_source(
        pool,
        project,
        &dispatch,
        SourceInput {
            source_kind: "manual_input".into(),
            target_domain: "pc".into(),
            target_type: "analysis".into(),
            target_id: Uuid::new_v4().to_string(),
            target_revision_id: None,
            revision_missing_reason: Some("rest fixture".into()),
            section_anchor: None,
            start_line: None,
            locator: None,
            description: None,
            source_content_hash: None,
        },
    )
    .await
    .unwrap();
    for revision_id in revisions {
        register_evidence(
            pool,
            project,
            &dispatch,
            EvidenceInput {
                source_id,
                subject: EvidenceSubject::ObjectRevision(*revision_id),
                evidence_type: "system_observation".into(),
                target_domain: None,
                target_type: None,
                target_id: None,
                target_revision_id: None,
                file_path: None,
                symbol: None,
                code_range: None,
                commit_sha: None,
                section_anchor: None,
                start_line: None,
                end_line: None,
                excerpt: None,
                description: Some("rest evidence".into()),
                evidence_hash: "rest".into(),
                confidence: None,
            },
        )
        .await
        .unwrap();
    }
    source_id
}

/// pc_analyze 任务夹具（intake 行 + 任务行，PROCESSING）。
async fn pc_analyze_task(pool: &PgPool, project: Uuid) -> Uuid {
    let intake: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.intake_record \
         (tenant_id, project_id, intake_type, channel, submitted_by, source_ref, title, \
          intake_status, idempotency_key) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, 'engineering_artifact', \
                 'system', $2, '{}'::jsonb, 'rest p3', 'ROUTED', $3) RETURNING id",
    )
    .bind(project)
    .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
    .bind(format!("rest-p3:{}", Uuid::new_v4()))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO integration.intake_task \
         (tenant_id, project_id, intake_id, target_service, task_type, task_status, idempotency_key) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, 'pc', 'pc_analyze', \
                 'PROCESSING', $3) RETURNING id",
    )
    .bind(project)
    .bind(intake)
    .bind(format!("pc_analyze:{intake}"))
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn p3_expands_proposal_and_no_change_conclusions(pool: PgPool) {
    let project = seed_project(&pool, SYSTEM_TENANT, "PCR-E").await;
    common::join_dev(&pool, project).await;
    let dispatch = AuditActor::dispatch();
    // P3-11 W7：pc snapshot 字典四类（repository/tech_stack/delivery_activity/
    // module）已废弃——PC 通道夹具改用活跃类型（业务能力/功能点）。
    let x = upsert_object_draft(
        &pool,
        project,
        &dispatch,
        draft("business_capability", "p3.x", "# X"),
    )
    .await
    .unwrap();
    let y = upsert_object_draft(
        &pool,
        project,
        &dispatch,
        draft("feature", "p3.y", "# Y"),
    )
    .await
    .unwrap();

    // 任务 1：分析起草提案 → Platform 回写 result_ref=proposal（handler 同款 complete_task）
    let task1 = pc_analyze_task(&pool, project).await;
    let source1 = analysis_source(&pool, project, &[x.revision_id, y.revision_id]).await;
    let AnalysisProposalOutcome::Proposed(receipt) = propose_from_analysis(
        &pool,
        project,
        AnalysisProposalInput {
            source_id: source1,
            task_id: task1,
            title: "p3 proposal".into(),
            rationale: "rest fixture".into(),
        },
    )
    .await
    .unwrap() else {
        panic!("uncovered revisions must be proposed");
    };
    pm_application::intake::complete_task(
        &pool,
        task1,
        json!({ "target_domain": "pc", "target_type": "proposal",
                "target_id": receipt.proposal_id.to_string(), "target_revision_id": null }),
    )
    .await
    .unwrap();
    let (status, body) = send(
        &pool,
        get(&format!("/v1/pc/analyses/{task1}?project_id={project}")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["conclusion"], "PROPOSED");
    assert_eq!(
        body["proposal_ref"]["target_id"],
        receipt.proposal_id.to_string()
    );
    assert_eq!(body["proposal"]["status"], "submitted");
    let objects = body["objects"].as_array().unwrap();
    assert_eq!(objects.len(), 2);
    assert!(objects
        .iter()
        .all(|o| o["latest_revision"]["versioned"] == false));

    // 任务 2：同修订集重分析 → NO_CHANGE，result_ref 锚来源（KP-1 别名缺陷回归面）
    let task2 = pc_analyze_task(&pool, project).await;
    let source2 = analysis_source(&pool, project, &[x.revision_id, y.revision_id]).await;
    assert!(matches!(
        propose_from_analysis(
            &pool,
            project,
            AnalysisProposalInput {
                source_id: source2,
                task_id: task2,
                title: "p3 again".into(),
                rationale: "rest fixture".into(),
            },
        )
        .await
        .unwrap(),
        AnalysisProposalOutcome::NoChange
    ));
    pm_application::intake::complete_task(
        &pool,
        task2,
        json!({ "target_domain": "pc", "target_type": "source",
                "target_id": source2.to_string(), "target_revision_id": null }),
    )
    .await
    .unwrap();
    let uri2 = format!("/v1/pc/analyses/{task2}?project_id={project}");
    let (status, body) = send(&pool, get(&uri2)).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "source expansion must not 500: {body}"
    );
    let objects = body["objects"].as_array().unwrap();
    assert_eq!(objects.len(), 2, "{body}");
    assert!(objects
        .iter()
        .all(|o| o["latest_revision"]["versioned"] == false));

    // 人工确认（P5 暂记）+ 服务层补发布：versioned 翻真（正式版本成员派生，
    // §5.3 不冗余状态列）。审阅流 v2：决定不发布，发布走 close 双域分流/
    // 显式 publish_version（本用例取后者）。
    let (status, _) = send(
        &pool,
        post_json(
            &format!(
                "/v1/pc/proposals/{}/decision?project_id={project}",
                receipt.proposal_id
            ),
            json!({ "action": "approve", "reason": "测试理由", "expected_version_no": 0 }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    pm_application::pc::publish_version(
        &pool,
        project,
        &dev(),
        pm_application::pc::PublishVersionInput {
            proposal_id: receipt.proposal_id,
        },
    )
    .await
    .unwrap();
    let (_, body) = send(&pool, get(&uri2)).await;
    let objects = body["objects"].as_array().unwrap();
    assert!(
        objects
            .iter()
            .all(|o| o["latest_revision"]["versioned"] == true
                && o["lifecycle_status"] == "active"),
        "{body}"
    );
}

// ── 用例 6：PC 对象详情（全文 + 项目作用域）──────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn pc_object_detail_returns_full_content_and_is_scoped(pool: PgPool) {
    let project = seed_project(&pool, SYSTEM_TENANT, "PCR-F").await;
    let other_member_project = seed_project(&pool, SYSTEM_TENANT, "PCR-F2").await;
    let stranger = seed_project(&pool, SYSTEM_TENANT, "PCR-F3").await;
    common::join_dev(&pool, project).await;
    common::join_dev(&pool, other_member_project).await;

    // 列表 excerpt 只有 240 字截断——详情必须给全文（超长正文回归面）。
    // P3-11 W7：pc snapshot 类型已废弃，夹具改用活跃类型（feature）。
    let long_content = "# 交付动态\n\n共采集 300 条 commit，最近交付：\n\
        - `e105e9b9` style: 替换裸值为设计系统语义 token\n\
        - `.rs`：553 个文件\n\
        - `.md`：543 个文件\n\
        - `.ts`：488 个文件\n\
        - `.vue`：359 个文件\n\
        - `.json`：219 个文件\n";
    let receipt = upsert_object_draft(
        &pool,
        project,
        &dev(),
        UpsertObjectDraft {
            object_type: "feature".into(),
            object_key: Some("delivery".into()),
            display_name: "交付动态（最近 10 条）".into(),
            content_markdown: long_content.into(),
            change_summary: Some("首次采集".into()),
            source_context: None,
            analysis_mode: Some("derived".into()),
        },
    )
    .await
    .unwrap();

    let (status, body) = send(
        &pool,
        get(&format!(
            "/v1/pc/objects/{}?project_id={project}",
            receipt.object_id
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["object_id"], receipt.object_id.to_string());
    assert_eq!(body["object_type"], "feature");
    assert_eq!(body["object_key"], "delivery");
    assert_eq!(body["name"], "交付动态（最近 10 条）");
    assert_eq!(body["lifecycle_status"], "draft");
    assert_eq!(body["latest_revision"]["revision_no"], 1);
    assert_eq!(body["latest_revision"]["summary"], "首次采集");
    assert_eq!(body["latest_revision"]["analysis_mode"], "derived");
    assert_eq!(body["latest_revision"]["versioned"], false);
    assert_eq!(body["latest_revision"]["content"], long_content);

    // 项目作用域：非成员项目 / 他项目（可见）下查 / 非法 ID / 不存在对象 → 404
    for uri in [
        format!("/v1/pc/objects/{}?project_id={stranger}", receipt.object_id),
        format!(
            "/v1/pc/objects/{}?project_id={other_member_project}",
            receipt.object_id
        ),
        format!("/v1/pc/objects/not-a-uuid?project_id={project}"),
        format!("/v1/pc/objects/{}?project_id={project}", Uuid::new_v4()),
    ] {
        let (status, _) = send(&pool, get(&uri)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
    let (status, _) = send(&pool, get(&format!("/v1/pc/objects/{}", receipt.object_id))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── 用例 8：ProjC 对象浏览面（/v1/projc/objects；与 pc 同构只读管理面）───────

#[sqlx::test(migrations = "../../migrations")]
async fn projc_objects_browse_list_detail_and_scoped(pool: PgPool) {
    use pm_application::projc::{
        upsert_object_draft as projc_upsert_object_draft,
        UpsertObjectDraft as ProjcUpsertObjectDraft,
    };

    let project = seed_project(&pool, SYSTEM_TENANT, "PCR-G").await;
    let stranger = seed_project(&pool, SYSTEM_TENANT, "PCR-G2").await;
    common::join_dev(&pool, project).await;

    let receipt = projc_upsert_object_draft(
        &pool,
        project,
        &dev(),
        ProjcUpsertObjectDraft {
            object_type: "tech_stack".into(),
            object_key: Some("stack".into()),
            display_name: "技术栈（rust）".into(),
            content_markdown: "# 技术栈\n\nrust 为主。".into(),
            change_summary: Some("快照派生".into()),
            source_context: None,
            analysis_mode: Some("derived".into()),
        },
    )
    .await
    .unwrap();

    // 列表
    let (status, body) = send(
        &pool,
        get(&format!("/v1/projc/objects?project_id={project}")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["object_id"], receipt.object_id.to_string());
    assert_eq!(items[0]["object_type"], "tech_stack");

    // 详情：全文 + analysis_sources（未发布版本 → 空数组）
    let (status, body) = send(
        &pool,
        get(&format!(
            "/v1/projc/objects/{}?project_id={project}",
            receipt.object_id
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["name"], "技术栈（rust）");
    assert_eq!(body["latest_revision"]["analysis_mode"], "derived");
    assert_eq!(body["latest_revision"]["versioned"], false);
    assert_eq!(body["analysis_sources"].as_array().unwrap().len(), 0);

    // PC 对象详情加法字段：analysis_sources 同形状（未发布 → 空数组）
    let pc_receipt = upsert_object_draft(
        &pool,
        project,
        &dev(),
        UpsertObjectDraft {
            object_type: "feature".into(),
            object_key: Some("pc-feature".into()),
            display_name: "特性A".into(),
            content_markdown: "# 特性A".into(),
            change_summary: None,
            source_context: None,
            analysis_mode: None,
        },
    )
    .await
    .unwrap();
    let (status, body) = send(
        &pool,
        get(&format!(
            "/v1/pc/objects/{}?project_id={project}",
            pc_receipt.object_id
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["analysis_sources"].as_array().unwrap().len(), 0);

    // 项目作用域：非成员 / 非法 ID / 不存在 → 404；缺 project_id → 400
    for uri in [
        format!("/v1/projc/objects/{}?project_id={stranger}", receipt.object_id),
        format!("/v1/projc/objects/not-a-uuid?project_id={project}"),
        format!("/v1/projc/objects/{}?project_id={project}", Uuid::new_v4()),
    ] {
        let (status, _) = send(&pool, get(&uri)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
    let (status, _) = send(&pool, get(&format!("/v1/projc/objects/{}", receipt.object_id))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
