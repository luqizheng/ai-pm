//! D-5 · 仓库分析报告 REST 面（受理建报告 / 409 报告门 / 关闭事务 / 列表详情）。
//!
//! | 用例 | 断言焦点 |
//! |---|---|
//! | 1 | analyze 受理 → 报告 analyzing；重复 analyze → 409 `report_open`（带 report_id）；锚点列状态 |
//! | 2 | mark_analyzed 派生（kb_revision_id/pc_source_id）与 mark_failed 终态 |
//! | 3 | close（interactive）：API Key 403 / approved 批量发布 + 未决 withdrawn / KB 草稿 rejected / 锚点固化 / 门释放再 analyze 202 / 重复 close 409 / closed 后决定冻结 409 |
//! | 4 | 列表/详情形状 + 项目边界（非成员 404 / 他项目 404 / 缺参 400） |

mod common;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use digua_common_jwt::{sign_token, JWKSCache};
use pm_api::auth::AuthConfig;
use pm_api::routes::{router, router_with_auth_config};
use pm_application::actor::AuditActor;
use pm_application::actor::AuditActor as _AuditActor;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::pc::{
    create_proposal, decide_proposal, submit_proposal, upsert_object_draft, CreateProposalInput,
    DecisionAction, PcError, ProposalDecisionInput, ProposalMemberPin, UpsertObjectDraft,
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
const COMMIT: &str = "e105e9b97ca8de118a847aec97de7abdc676662a";

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

async fn seed_verified_repo(pool: &PgPool, project: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO integration.project_repository \
         (project_id, name, url, role, default_branch, status, head_commit_sha, tenant_id) \
         VALUES ($1, 'ai-pm主仓库', 'git@example.com:tooling/ai-pm.git', 'main', 'master', \
                 'verified', $2, (SELECT tenant_id FROM core.project WHERE id = $1)) \
         RETURNING id",
    )
    .bind(project)
    .bind(COMMIT)
    .fetch_one(pool)
    .await
    .expect("seed repository")
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

fn dev() -> AuditActor {
    AuditActor::principal(pm_api::auth::DEV_OFF_PRINCIPAL)
}

/// kb.document + draft 修订（报告关闭处置的 KB 侧夹具）。
async fn seed_kb_draft(pool: &PgPool, project: Uuid) -> (Uuid, Uuid) {
    let tenant: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project)
        .fetch_one(pool)
        .await
        .unwrap();
    let doc: Uuid = sqlx::query_scalar(
        "INSERT INTO kb.document (tenant_id, project_id, doc_key, doc_type, canonical_path) \
         VALUES ($1, $2, 'repo-fixture', 'knowledge', 'repos/fixture/snapshot.md') RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .fetch_one(pool)
    .await
    .unwrap();
    let rev: Uuid = sqlx::query_scalar(
        "INSERT INTO kb.document_revision \
         (tenant_id, document_id, revision_no, source_commit_sha, content_hash, status, \
          content, change_summary, analysis_mode, created_by) \
         VALUES ($1, $2, 1, $3, 'hash', 'draft', '# 报告正文', 'fixture', 'derived', $4) \
         RETURNING id",
    )
    .bind(tenant)
    .bind(doc)
    .bind(COMMIT)
    .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
    .fetch_one(pool)
    .await
    .unwrap();
    (doc, rev)
}

/// pc.source（kb_document 来源，指向 KB 修订）。
async fn seed_pc_source(pool: &PgPool, project: Uuid, kb_revision: Uuid) -> Uuid {
    let tenant: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project)
        .fetch_one(pool)
        .await
        .unwrap();
    sqlx::query_scalar(
        "INSERT INTO pc.source \
         (tenant_id, project_id, source_kind, target_domain, target_type, target_id, \
          target_revision_id, source_content_hash, collected_by) \
         VALUES ($1, $2, 'kb_document', 'knowledge_base', 'document', $3, $4, 'hash', $5) \
         RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(kb_revision.to_string())
    .bind(kb_revision.to_string())
    .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
    .fetch_one(pool)
    .await
    .expect("seed pc.source")
}

/// 提案落到指定来源（create 时携带 source_id——submitted 后锚列不可变）。
async fn submitted_proposal_on(pool: &PgPool, project: Uuid, source_id: Uuid, key: &str) -> Uuid {
    let actor = dev();
    let receipt = upsert_object_draft(
        pool,
        project,
        &actor,
        UpsertObjectDraft {
            object_type: "feature".into(),
            object_key: Some(key.into()),
            display_name: key.into(),
            content_markdown: format!("# {key}"),
            change_summary: Some("fixture".into()),
            source_context: None,
            analysis_mode: None,
        },
    )
    .await
    .unwrap();
    let drafted = create_proposal(
        pool,
        project,
        &actor,
        CreateProposalInput {
            title: format!("fixture proposal {key}"),
            rationale: "fixture".into(),
            origin_kind: "manual_input".into(),
            source_id: Some(source_id),
            governance_level: None,
            risk_level: None,
            confidence: None,
            members: vec![ProposalMemberPin {
                change_kind: "create_object".into(),
                object_id: Some(receipt.object_id),
                object_revision_id: Some(receipt.revision_id),
                relationship_id: None,
                relationship_revision_id: None,
            }],
        },
    )
    .await
    .unwrap();
    submit_proposal(pool, project, &actor, drafted.proposal_id, None)
        .await
        .unwrap();
    drafted.proposal_id
}

/// analyzed 报告夹具（直插行模拟"分析完成待审"）。
async fn seed_analyzed_report(
    pool: &PgPool,
    project: Uuid,
    repo: Uuid,
    kb_revision: Uuid,
    pc_source: Uuid,
) -> Uuid {
    let tenant: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project)
        .fetch_one(pool)
        .await
        .unwrap();
    sqlx::query_scalar(
        "INSERT INTO integration.repo_analysis_report \
         (tenant_id, project_id, repository_id, commit_sha, intake_id, status, \
          kb_revision_id, pc_source_id) \
         VALUES ($1, $2, $3, $4, $5, 'analyzed', $6, $7) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(repo)
    .bind(COMMIT)
    .bind(Uuid::new_v4())
    .bind(kb_revision)
    .bind(pc_source)
    .fetch_one(pool)
    .await
    .expect("seed report")
}

// ── 用例 1：受理建报告 + 报告门 ──────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn analyze_acceptance_creates_report_and_gate_blocks(pool: PgPool) {
    let project = seed_project(&pool, SYSTEM_TENANT, "D5-A").await;
    common::join_dev(&pool, project).await;
    let repo = seed_verified_repo(&pool, project).await;

    let (status, body) = send(
        &pool,
        post_json(
            &format!("/v1/pm/repositories/{repo}/analyze?project_id={project}"),
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    let report_id = body["report_id"].as_str().unwrap().to_string();
    assert!(!report_id.is_empty());

    // 报告行：analyzing + intake 关联
    // 报告行：analyzing + intake 关联 + D-2 口径（首次无锚点 → 自动退全量）
    let (db_status, intake, run_mode, since): (String, Uuid, String, Option<String>) =
        sqlx::query_as(
            "SELECT status, intake_id, run_mode, since_commit \
             FROM integration.repo_analysis_report WHERE id = $1",
        )
        .bind(Uuid::parse_str(&report_id).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(db_status, "analyzing");
    assert_eq!(
        intake,
        Uuid::parse_str(body["intake_id"].as_str().unwrap()).unwrap()
    );
    assert_eq!(run_mode, "full");
    assert!(since.is_none(), "首次分析无锚点 → 自动退全量");

    // 门：报告 open 期间重复 analyze → 409 report_open（带同一 report_id）
    let (status, body) = send(
        &pool,
        post_json(
            &format!("/v1/pm/repositories/{repo}/analyze?project_id={project}"),
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"], "report_open");
    assert_eq!(body["report_id"], report_id);
}

// ── 用例 2：mark_analyzed 派生 + mark_failed 终态（服务层直调）──────────────

#[sqlx::test(migrations = "../../migrations")]
async fn mark_analyzed_derives_links_and_failed_is_terminal(pool: PgPool) {
    use pm_application::analysis_report::{mark_analyzed, mark_failed};

    let project = seed_project(&pool, SYSTEM_TENANT, "D5-B").await;
    common::join_dev(&pool, project).await;
    let repo = seed_verified_repo(&pool, project).await;
    let (doc, kb_rev) = seed_kb_draft(&pool, project).await;
    let source = seed_pc_source(&pool, project, kb_rev).await;
    // G-1：ProjC 快照通道来源（derive 链双源登记的另一半）
    let projc_source = seed_projc_source(&pool, project, kb_rev).await;

    // intake + kb_analyze 任务（result_ref=扁平 ref 值，mark_analyzed 派生输入）
    let tenant: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project)
        .fetch_one(&pool)
        .await
        .unwrap();
    let intake: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.intake_record \
         (tenant_id, project_id, intake_type, channel, submitted_by, source_ref, title, \
          intake_status, idempotency_key) \
         VALUES ($1, $2, 'engineering_artifact', 'system', $3, '{}'::jsonb, 'fixture', \
                 'ROUTED', $4) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
    .bind(format!("fixture:{}", Uuid::new_v4()))
    .fetch_one(&pool)
    .await
    .unwrap();
    let kb_task: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.intake_task \
         (tenant_id, project_id, intake_id, target_service, task_type, task_status, \
          idempotency_key, result_ref) \
         VALUES ($1, $2, $3, 'kb', 'kb_analyze', 'COMPLETED', $4, $5) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(intake)
    .bind("fixture-kb")
    .bind(json!({
        "target_domain": "knowledge_base",
        "target_type": "document",
        "target_id": doc.to_string(),
        "target_revision_id": kb_rev.to_string()
    }))
    .fetch_one(&pool)
    .await
    .unwrap();
    let report_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.repo_analysis_report \
         (tenant_id, project_id, repository_id, commit_sha, intake_id, status) \
         VALUES ($1, $2, $3, $4, $5, 'analyzing') RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(repo)
    .bind(COMMIT)
    .bind(intake)
    .fetch_one(&pool)
    .await
    .unwrap();

    mark_analyzed(&pool, kb_task).await.unwrap();
    let (status, kb_link, src, projc_src): (String, Option<Uuid>, Option<Uuid>, Option<Uuid>) =
        sqlx::query_as(
            "SELECT status, kb_revision_id, pc_source_id, projc_source_id \
         FROM integration.repo_analysis_report WHERE id = $1",
        )
        .bind(report_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "analyzed");
    assert_eq!(kb_link, Some(kb_rev));
    assert_eq!(src, Some(source));
    assert_eq!(projc_src, Some(projc_source), "G-1 dual source registration");

    // DEF-CHAIN-02（branch-b4 场景）：file:// 路径不存在 → `git_not_found`，
    // 不得再落到历史证据里的 "git authentication failed"。
    use pm_application::git_access::{clone_bare, GitError};
    let missing = std::env::temp_dir().join(format!("ai-pm-b4-missing-{}", Uuid::new_v4()));
    let file_url = format!("file://{}", missing.to_string_lossy().replace('\\', "/"));
    let dest = std::env::temp_dir().join(format!("ai-pm-b4-dest-{}", Uuid::new_v4()));
    let git_err = clone_bare(&file_url, None, &dest, Duration::from_secs(30))
        .await
        .unwrap_err();
    assert!(matches!(git_err, GitError::NotFound), "{git_err:?}");
    // 与 kb_pc_analysis 的 AnalysisError::Git 展示口径一致（"git: <code>: <msg>"）。
    let detail = format!("git: {}: {}", git_err.stable_code(), git_err);
    assert!(!detail.to_ascii_lowercase().contains("authentication"));

    // failed：终态不占门 + DEF-CHAIN-01（stage_detail 必须随失败一并写入）
    mark_failed(&pool, kb_task, "kb_analysis_failed", &detail)
        .await
        .unwrap();
    let (status, code, stage, err_detail): (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT status, error_code, stage_detail, error_detail \
         FROM integration.repo_analysis_report WHERE id = $1",
    )
    .bind(report_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "failed");
    assert_eq!(code.as_deref(), Some("kb_analysis_failed"));
    assert_eq!(
        stage.as_deref(),
        Some("KB 分析失败"),
        "DEF-CHAIN-01: failed report must carry stage_detail"
    );
    assert_eq!(err_detail.as_deref(), Some(detail.as_str()));
    assert!(
        !err_detail
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains("authentication"),
        "DEF-CHAIN-02: missing file:// path must not be reported as auth failure"
    );
    // source 夹具未被后续用例误用
    let _ = source;
}

// ── 用例 3：关闭事务（interactive 门 / 自动处置 / 锚点固化 / 门释放）─────────

fn sign(claims: Value) -> String {
    sign_token(PRIVATE_PEM, &claims, Duration::from_secs(300)).expect("test token must sign")
}

#[sqlx::test(migrations = "../../migrations")]
async fn close_disposes_pending_items_and_releases_gate(pool: PgPool) {
    let tenant_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('lmms', '1', 'lmms:1') \
         ON CONFLICT (provider, external_key) DO UPDATE SET display_name = core.tenant.display_name \
         RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let project = seed_project(&pool, tenant_id, "D5-C").await;
    // dev-off principal 物化（off 上下文 403 断言 + dev() 夹具的外键依赖）。
    common::join_dev(&pool, project).await;
    let app = router_with_auth_config(
        pool.clone(),
        Arc::new(JWKSCache::from_pem(PUBLIC_PEM).unwrap()),
        AuthConfig {
            first_admin_tenant: Some("1".to_string()),
            first_admin_subject: Some("usr-lead".to_string()),
        },
    );
    let claims = |aud: &str| {
        sign(json!({
            "sub": "usr-lead", "tid": 1, "username": "usr-lead",
            "roles": ["tenant_admin"], "permissions": [], "aud": aud,
        }))
    };
    let browser = claims("coder-micro-services");
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

    // 首管理员激活 → 项目成员
    let me = app
        .clone()
        .oneshot(call(&browser, "GET", "/v1/pm/me", None))
        .await
        .unwrap();
    assert_eq!(me.status(), StatusCode::OK);
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

    let repo = seed_verified_repo(&pool, project).await;
    let (_doc, kb_rev) = seed_kb_draft(&pool, project).await;
    let source = seed_pc_source(&pool, project, kb_rev).await;
    let proposal = submitted_proposal_on(&pool, project, source, "d5-close").await;
    // 第二条提案：批准（暂记）→ close 时批量发布；第一条保持未决（close 将撤回）
    let proposal2 = submitted_proposal_on(&pool, project, source, "d5-close-2").await;
    let report = seed_analyzed_report(&pool, project, repo, kb_rev, source).await;

    // API Key 上下文（同一主体 Lead）→ 403：关闭须 interactive 真人（三重门之一）
    let api_key = sign(json!({
        "sub": "usr-lead", "tid": 1, "username": "usr-lead",
        "roles": ["tenant_admin"], "permissions": [], "client_id": "key-abc",
    }));
    let response = app
        .clone()
        .oneshot(call(
            &api_key,
            "POST",
            &format!("/v1/pm/analysis-reports/{report}/close?project_id={project}"),
            Some(json!({})),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    // 审阅流 v2：批准第二条提案（暂记，decide 不发布）
    let lead_actor = _AuditActor::principal(lead);
    let outcome = decide_proposal(
        &pool,
        project,
        &lead_actor,
        proposal2,
        ProposalDecisionInput {
            action: DecisionAction::Approve,
            reason: Some("决定理由".into()),
            expected_version_no: 0,
        },
    )
    .await
    .unwrap();
    assert_eq!(outcome.status, "approved");
    assert!(outcome.published_version.is_none());

    // interactive 关闭 → 自动处置（approved 批量发布 + 未决撤回）+ 锚点固化
    let response = app
        .clone()
        .oneshot(call(
            &browser,
            "POST",
            &format!("/v1/pm/analysis-reports/{report}/close?project_id={project}"),
            Some(json!({})),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let out: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(out["withdrawn_proposals"], 1);
    assert_eq!(out["applied_proposals"], 1);
    assert_eq!(out["publish_failed_proposals"], 0);
    assert_eq!(out["rejected_kb"], true);
    assert_eq!(out["last_analyzed_commit"], COMMIT);

    // DB 终态
    let (p_status,): (String,) = sqlx::query_as("SELECT status FROM pc.proposal WHERE id = $1")
        .bind(proposal)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(p_status, "withdrawn");
    let (p2_status,): (String,) = sqlx::query_as("SELECT status FROM pc.proposal WHERE id = $1")
        .bind(proposal2)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(p2_status, "applied", "approved proposal published at close");
    let (k_status,): (String,) =
        sqlx::query_as("SELECT status FROM kb.document_revision WHERE id = $1")
            .bind(kb_rev)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(k_status, "rejected");
    let (r_status, anchor): (String, Option<String>) = sqlx::query_as(
        "SELECT r.status, pr.last_analyzed_commit_sha \
         FROM integration.repo_analysis_report r \
         JOIN integration.project_repository pr ON pr.id = r.repository_id WHERE r.id = $1",
    )
    .bind(report)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(r_status, "closed");
    assert_eq!(anchor.as_deref(), Some(COMMIT));

    // 门释放：再 analyze → 202；重复 close → 409
    let response = app
        .clone()
        .oneshot(call(
            &browser,
            "POST",
            &format!("/v1/pm/repositories/{repo}/analyze?project_id={project}"),
            Some(json!({})),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let response = app
        .oneshot(call(
            &browser,
            "POST",
            &format!("/v1/pm/analysis-reports/{report}/close?project_id={project}"),
            Some(json!({})),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);

    // 审阅流 v2：报告 closed 后决定冻结 → 409
    let frozen = decide_proposal(
        &pool,
        project,
        &lead_actor,
        proposal2,
        ProposalDecisionInput {
            action: DecisionAction::Reject,
            reason: Some("决定理由".into()),
            expected_version_no: 0,
        },
    )
    .await;
    assert!(matches!(frozen, Err(PcError::Conflict(_))), "{frozen:?}");
}

// ── 用例 4：列表 / 详情形状 + 项目边界 ──────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn list_and_detail_are_project_scoped(pool: PgPool) {
    let project = seed_project(&pool, SYSTEM_TENANT, "D5-D").await;
    let stranger = seed_project(&pool, SYSTEM_TENANT, "D5-D2").await;
    common::join_dev(&pool, project).await;
    let repo = seed_verified_repo(&pool, project).await;
    let (doc, kb_rev) = seed_kb_draft(&pool, project).await;
    let source = seed_pc_source(&pool, project, kb_rev).await;
    submitted_proposal_on(&pool, project, source, "d5-detail").await;
    let report = seed_analyzed_report(&pool, project, repo, kb_rev, source).await;

    // 列表：pending 计数与 KB 草稿标记
    let (status, body) = send(
        &pool,
        get(&format!("/v1/pm/analysis-reports?project_id={project}")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], report.to_string());
    assert_eq!(items[0]["pending_proposals"], 1);
    assert_eq!(items[0]["has_pending_kb"], true);
    assert_eq!(items[0]["repository_name"], "ai-pm主仓库");
    assert_eq!(items[0]["status"], "analyzed");

    // status 过滤
    let (status, body) = send(
        &pool,
        get(&format!(
            "/v1/pm/analysis-reports?project_id={project}&status=closed"
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["items"].as_array().unwrap().len(), 0);

    // 详情：KB 修订全文 + 提案集
    let (status, body) = send(
        &pool,
        get(&format!(
            "/v1/pm/analysis-reports/{report}?project_id={project}"
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["kb_revision"]["revision_id"], kb_rev.to_string());
    assert!(body["kb_revision"]["content"]
        .as_str()
        .unwrap()
        .contains("报告正文"));
    // 提案行加法投影：审阅头字段（rationale/risk/confidence/origin）
    let proposals = body["proposals"].as_array().unwrap();
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0]["rationale"], "fixture");
    assert_eq!(proposals[0]["origin_kind"], "manual_input");
    // 证据投影（加法，恒数组）
    assert!(body["pc_evidence"].as_array().unwrap().is_empty());
    assert!(body["projc_evidence"].as_array().unwrap().is_empty());
    assert_eq!(body["pc_revisions"].as_array().unwrap().len(), 0);

    // 项目边界：非成员项目 / 他项目下查 / 缺参
    for uri in [
        format!("/v1/pm/analysis-reports/{report}?project_id={stranger}"),
        format!("/v1/pm/analysis-reports?project_id={stranger}"),
        format!("/v1/pm/analysis-reports/{report}"),
    ] {
        let (status, _) = send(&pool, get(&uri)).await;
        assert!(
            status == StatusCode::NOT_FOUND || status == StatusCode::BAD_REQUEST,
            "{uri} -> {status}"
        );
    }
    // 非成员项目严格 404（不泄露存在性）
    let (status, _) = send(
        &pool,
        get(&format!(
            "/v1/pm/analysis-reports/{report}?project_id={stranger}"
        )),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ── 用例 7：D-4 身份综合门禁（interactive / api_key / 非成员）────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn identity_synthesis_gates(pool: PgPool) {
    let tenant_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name)          VALUES ('lmms', '1', 'lmms:1')          ON CONFLICT (provider, external_key) DO UPDATE SET display_name = core.tenant.display_name          RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let project = seed_project(&pool, tenant_id, "D5-G").await;
    let stranger = seed_project(&pool, tenant_id, "D5-G2").await;
    let app = router_with_auth_config(
        pool.clone(),
        Arc::new(JWKSCache::from_pem(PUBLIC_PEM).unwrap()),
        AuthConfig {
            first_admin_tenant: Some("1".to_string()),
            first_admin_subject: Some("usr-lead".to_string()),
        },
    );
    let claims = |aud: &str| {
        sign(json!({
            "sub": "usr-lead", "tid": 1, "username": "usr-lead",
            "roles": ["tenant_admin"], "permissions": [], "aud": aud,
        }))
    };
    let browser = claims("coder-micro-services");
    let api_key = sign(json!({
        "sub": "usr-lead", "tid": 1, "username": "usr-lead",
        "roles": ["tenant_admin"], "permissions": [], "client_id": "key-abc",
    }));
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

    // 首管理员激活 → Lead 成员
    let me = app
        .clone()
        .oneshot(call(&browser, "GET", "/v1/pm/me", None))
        .await
        .unwrap();
    assert_eq!(me.status(), StatusCode::OK);
    let me: Value =
        serde_json::from_slice(&to_bytes(me.into_body(), usize::MAX).await.unwrap()).unwrap();
    let lead = Uuid::parse_str(me["principal_id"].as_str().unwrap()).unwrap();
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role)          VALUES ($1, $2, $3, 'owner') ON CONFLICT DO NOTHING",
    )
    .bind(tenant_id)
    .bind(project)
    .bind(lead)
    .execute(&pool)
    .await
    .unwrap();

    let uri = format!("/v1/pm/projects/{project}/cognition/identity-synthesis");

    // API Key（同一主体）→ 403：综合动作须 interactive 真人
    let response = app
        .clone()
        .oneshot(call(&api_key, "POST", &uri, Some(json!({}))))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    // interactive + 测试库无 LLM provider → 501（NotImplemented：综合不可用）
    let response = app
        .clone()
        .oneshot(call(&browser, "POST", &uri, Some(json!({}))))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED, "{}", {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        String::from_utf8_lossy(&bytes).to_string()
    });

    // 非成员项目 → 404（不泄露存在性）
    let response = app
        .oneshot(call(
            &browser,
            "POST",
            &format!("/v1/pm/projects/{stranger}/cognition/identity-synthesis"),
            Some(json!({})),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ══ P3-11 阶段二 · G-1 双源登记 / G-2 close 双域分流（测试断言先行）══════════

use pm_application::analysis_report::{self, close_report, AnalysisReportError};
use pm_application::projc::{
    approve_proposal as projc_approve, create_proposal as projc_create, publish_version,
    submit_proposal as projc_submit, upsert_object_draft as projc_upsert_draft,
    CreateProposalInput as ProjcCreateProposalInput, PublishVersionInput as ProjcPublishInput,
    UpsertObjectDraft as ProjcUpsertDraft,
};

/// projc 侧真人 owner 夹具 = dev-off（join_dev 已置 owner；human + A3 通过）。
fn projc_object_draft(key: &str, content: &str) -> ProjcUpsertDraft {
    ProjcUpsertDraft {
        object_type: "repository".into(),
        object_key: Some(key.into()),
        display_name: key.into(),
        content_markdown: content.into(),
        change_summary: Some("fixture".into()),
        source_context: None,
        analysis_mode: None,
    }
}

/// projc.source（kb_document 钉扎——与 pc.source 同修订，G-1 双源夹具）。
async fn seed_projc_source(pool: &PgPool, project: Uuid, kb_revision: Uuid) -> Uuid {
    let tenant: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project)
        .fetch_one(pool)
        .await
        .unwrap();
    sqlx::query_scalar(
        "INSERT INTO projc.source \
         (tenant_id, project_id, source_kind, target_domain, target_type, target_id, \
          target_revision_id, source_content_hash, collected_by) \
         VALUES ($1, $2, 'kb_document', 'knowledge_base', 'document', $3, $4, 'hash', $5) \
         RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(kb_revision.to_string())
    .bind(kb_revision.to_string())
    .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
    .fetch_one(pool)
    .await
    .expect("seed projc.source")
}

/// projc 提案（submitted；approve=true 时批准暂记）。
async fn projc_submitted_proposal_on(
    pool: &PgPool,
    project: Uuid,
    source_id: Uuid,
    key: &str,
    approve: bool,
) -> Uuid {
    let actor = dev();
    let receipt = projc_upsert_draft(pool, project, &actor, projc_object_draft(key, &format!("# {key}")))
        .await
        .unwrap();
    let drafted = projc_create(
        pool,
        project,
        &actor,
        ProjcCreateProposalInput {
            title: format!("projc fixture {key}"),
            rationale: "fixture".into(),
            origin_kind: "repo_analysis".into(),
            source_id: Some(source_id),
            governance_level: None,
            risk_level: None,
            confidence: None,
            members: vec![pm_application::projc::ProposalMemberPin {
                change_kind: "create_object".into(),
                object_id: Some(receipt.object_id),
                object_revision_id: Some(receipt.revision_id),
                relationship_id: None,
                relationship_revision_id: None,
            }],
        },
    )
    .await
    .unwrap();
    projc_submit(pool, project, &actor, drafted.proposal_id, None)
        .await
        .unwrap();
    if approve {
        projc_approve(
            pool,
            project,
            &actor,
            drafted.proposal_id,
            pm_application::projc::DecisionInput {
                reason: Some("决定理由".into()),
                expected_version_no: drafted.expected_version_no,
            },
        )
        .await
        .unwrap();
    }
    drafted.proposal_id
}

/// interactive 真人 actor（close 三重门之一；服务层直调 close_report 用）。
/// identity 列置 None（审计 FK 无夹具身份行；C174 四列显式写，NULL 合法）。
fn interactive_dev() -> AuditActor {
    AuditActor {
        principal_id: pm_api::auth::DEV_OFF_PRINCIPAL,
        identity_id: None,
        client: None,
        auth_mode: Some("interactive".into()),
        claimed_device_id: None,
    }
}

// ── G-1：mark_analyzed 同事务双源登记 ────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn mark_analyzed_registers_dual_sources(pool: PgPool) {
    use pm_application::analysis_report::mark_analyzed;

    let project = seed_project(&pool, SYSTEM_TENANT, "D5-P1").await;
    common::join_dev(&pool, project).await;
    let repo = seed_verified_repo(&pool, project).await;
    let (doc, kb_rev) = seed_kb_draft(&pool, project).await;
    let pc_source = seed_pc_source(&pool, project, kb_rev).await;
    let projc_source = seed_projc_source(&pool, project, kb_rev).await;

    let tenant: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project)
        .fetch_one(&pool)
        .await
        .unwrap();
    let intake: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.intake_record \
         (tenant_id, project_id, intake_type, channel, submitted_by, source_ref, title, \
          intake_status, idempotency_key) \
         VALUES ($1, $2, 'engineering_artifact', 'system', $3, '{}'::jsonb, 'fixture', \
                 'ROUTED', $4) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
    .bind(format!("fixture:{}", Uuid::new_v4()))
    .fetch_one(&pool)
    .await
    .unwrap();
    let kb_task: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.intake_task \
         (tenant_id, project_id, intake_id, target_service, task_type, task_status, \
          idempotency_key, result_ref) \
         VALUES ($1, $2, $3, 'kb', 'kb_analyze', 'COMPLETED', $4, $5) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(intake)
    .bind("fixture-kb-p1")
    .bind(json!({
        "target_domain": "knowledge_base",
        "target_type": "document",
        "target_id": doc.to_string(),
        "target_revision_id": kb_rev.to_string()
    }))
    .fetch_one(&pool)
    .await
    .unwrap();
    let report_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.repo_analysis_report \
         (tenant_id, project_id, repository_id, commit_sha, intake_id, status) \
         VALUES ($1, $2, $3, $4, $5, 'analyzing') RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(repo)
    .bind(COMMIT)
    .bind(intake)
    .fetch_one(&pool)
    .await
    .unwrap();

    mark_analyzed(&pool, kb_task).await.unwrap();
    let (status, pc_src, projc_src): (String, Option<Uuid>, Option<Uuid>) = sqlx::query_as(
        "SELECT status, pc_source_id, projc_source_id \
         FROM integration.repo_analysis_report WHERE id = $1",
    )
    .bind(report_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "analyzed");
    assert_eq!(pc_src, Some(pc_source), "G-1: pc_source_id（identity 通道）");
    assert_eq!(
        projc_src,
        Some(projc_source),
        "G-1: projc_source_id（ProjC 快照通道）同事务登记"
    );
}

// ── G-1：projc 源缺失 = 报告不得进入 analyzed（硬失败，不静默）───────────────

#[sqlx::test(migrations = "../../migrations")]
async fn mark_analyzed_hard_fails_without_projc_source(pool: PgPool) {
    use pm_application::analysis_report::mark_analyzed;

    let project = seed_project(&pool, SYSTEM_TENANT, "D5-P2").await;
    common::join_dev(&pool, project).await;
    let repo = seed_verified_repo(&pool, project).await;
    let (doc, kb_rev) = seed_kb_draft(&pool, project).await;
    let _pc_source = seed_pc_source(&pool, project, kb_rev).await;
    // 无 projc.source —— G-1 硬失败场景

    let tenant: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project)
        .fetch_one(&pool)
        .await
        .unwrap();
    let intake: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.intake_record \
         (tenant_id, project_id, intake_type, channel, submitted_by, source_ref, title, \
          intake_status, idempotency_key) \
         VALUES ($1, $2, 'engineering_artifact', 'system', $3, '{}'::jsonb, 'fixture', \
                 'ROUTED', $4) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
    .bind(format!("fixture:{}", Uuid::new_v4()))
    .fetch_one(&pool)
    .await
    .unwrap();
    let kb_task: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.intake_task \
         (tenant_id, project_id, intake_id, target_service, task_type, task_status, \
          idempotency_key, result_ref) \
         VALUES ($1, $2, $3, 'kb', 'kb_analyze', 'COMPLETED', $4, $5) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(intake)
    .bind("fixture-kb-p2")
    .bind(json!({
        "target_domain": "knowledge_base",
        "target_type": "document",
        "target_id": doc.to_string(),
        "target_revision_id": kb_rev.to_string()
    }))
    .fetch_one(&pool)
    .await
    .unwrap();
    let report_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.repo_analysis_report \
         (tenant_id, project_id, repository_id, commit_sha, intake_id, status) \
         VALUES ($1, $2, $3, $4, $5, 'analyzing') RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(repo)
    .bind(COMMIT)
    .bind(intake)
    .fetch_one(&pool)
    .await
    .unwrap();

    let outcome = mark_analyzed(&pool, kb_task).await;
    assert!(outcome.is_err(), "G-1: projc source missing must hard-fail");
    let (status, projc_src, stage): (String, Option<Uuid>, Option<String>) = sqlx::query_as(
        "SELECT status, projc_source_id, stage_detail \
         FROM integration.repo_analysis_report WHERE id = $1",
    )
    .bind(report_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "analyzing", "report must NOT enter analyzed");
    assert!(projc_src.is_none());
    // 不静默：失败留痕（审计事件可见）
    let (audit_rows,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM governance.audit_log \
          WHERE action = 'analysis_report.analyze_gate_failed' AND target_id = $1",
    )
    .bind(report_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(audit_rows >= 1, "hard failure must be observable (audit)");
}

// ── G-2：close 双域分流（独立计数 + 双域正式版本 + 审计键）───────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn close_disposes_both_domains_with_independent_counts(pool: PgPool) {
    let project = seed_project(&pool, SYSTEM_TENANT, "D5-P3").await;
    common::join_dev(&pool, project).await;
    let repo = seed_verified_repo(&pool, project).await;
    let (_doc, kb_rev) = seed_kb_draft(&pool, project).await;
    let pc_source = seed_pc_source(&pool, project, kb_rev).await;
    let projc_source = seed_projc_source(&pool, project, kb_rev).await;

    // PC 通道：一条 approved（close 批量发布）、一条未决（close 撤回）
    let pc_pending = submitted_proposal_on(&pool, project, pc_source, "d5-pc-pending").await;
    let pc_approved = submitted_proposal_on(&pool, project, pc_source, "d5-pc-approved").await;
    decide_proposal(
        &pool,
        project,
        &dev(),
        pc_approved,
        ProposalDecisionInput {
            action: DecisionAction::Approve,
            reason: Some("决定理由".into()),
            expected_version_no: 0,
        },
    )
    .await
    .unwrap();
    // ProjC 通道：一条 approved、一条未决（同构处置）
    let projc_pending =
        projc_submitted_proposal_on(&pool, project, projc_source, "d5-projc-pending", false).await;
    let projc_approved =
        projc_submitted_proposal_on(&pool, project, projc_source, "d5-projc-approved", true).await;
    let report = seed_analyzed_report(&pool, project, repo, kb_rev, pc_source).await;
    sqlx::query("UPDATE integration.repo_analysis_report SET projc_source_id = $2 WHERE id = $1")
        .bind(report)
        .bind(projc_source)
        .execute(&pool)
        .await
        .unwrap();

    let closer = interactive_dev();
    let outcome = close_report(&pool, project, report, &closer).await.unwrap();
    assert_eq!(outcome.applied_proposals, 1, "pc applied");
    assert_eq!(outcome.withdrawn_proposals, 1, "pc withdrawn");
    assert_eq!(outcome.projc_applied, 1, "G-2: projc 独立计数 applied");
    assert_eq!(outcome.projc_withdrawn, 1, "G-2: projc 独立计数 withdrawn");
    assert_eq!(outcome.projc_publish_failed_proposals, 0);
    assert_eq!(outcome.rejected_kb, true);
    // 发布清单：applied 与 replayed 均产生正式版本——双域各一条
    assert_eq!(outcome.published_versions.len(), 2, "pc+projc 各一条");
    assert!(outcome
        .published_versions
        .iter()
        .any(|v| v.domain == "pc" && v.proposal_id == pc_approved));
    assert!(outcome
        .published_versions
        .iter()
        .any(|v| v.domain == "projc" && v.proposal_id == projc_approved));

    // 双域正式版本各一（无重复正式版本）
    let (pc_versions, projc_versions): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM pc.cognition_version WHERE project_id = $1), \
                (SELECT count(*) FROM projc.cognition_version WHERE project_id = $1)",
    )
    .bind(project)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pc_versions, 1);
    assert_eq!(projc_versions, 1, "G-2: projc-decision → projc 正式版本");
    let (pc_status,): (String,) =
        sqlx::query_as("SELECT status FROM pc.proposal WHERE id = $1")
            .bind(pc_approved)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(pc_status, "applied");
    let (projc_status,): (String,) =
        sqlx::query_as("SELECT status FROM projc.proposal WHERE id = $1")
            .bind(projc_approved)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(projc_status, "applied");

    // 对象 → 来源分析报告追溯（lineage）：双域各取一个已入版本的对象，反查
    // version→proposal→报告。来源报告即本测试的 report。
    use pm_application::repo::kb_pc_query::KbPcQueryRepo;
    let query_repo =
        pm_infrastructure::repo_kb_pc_query::PgKbPcQueryRepo::new(pool.clone());
    for (schema_domain, repo_col) in [("pc", "pc_source_id"), ("projc", "projc_source_id")] {
        let (obj_id,): (Uuid,) = sqlx::query_as(&format!(
            "SELECT vm.object_id FROM {schema_domain}.version_member vm               JOIN {schema_domain}.cognition_version cv ON cv.id = vm.version_id              WHERE cv.project_id = $1 AND vm.object_id IS NOT NULL LIMIT 1"
        ))
        .bind(project)
        .fetch_one(&pool)
        .await
        .unwrap();
        let lineage = match schema_domain {
            "pc" => query_repo.pc_object_lineage(project, obj_id).await.unwrap(),
            _ => query_repo.projc_object_lineage(project, obj_id).await.unwrap(),
        };
        assert_eq!(lineage.len(), 1, "{schema_domain} lineage");
        assert_eq!(lineage[0].report_id, Some(report));
        assert_eq!(lineage[0].commit_sha.is_some(), true);
    }
    for (pending_id, table) in [(pc_pending, "pc"), (projc_pending, "projc")] {
        let (status,): (String,) = sqlx::query_as(&format!(
            "SELECT status FROM {table}.proposal WHERE id = $1"
        ))
        .bind(pending_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(status, "withdrawn", "{table} pending withdrawn at close");
    }

    // 审计：旧 6 键 + projc_* 新键
    let (payload,): (serde_json::Value,) = sqlx::query_as(
        "SELECT after_data FROM governance.audit_log \
          WHERE action = 'analysis_report.closed' AND target_id = $1",
    )
    .bind(report)
    .fetch_one(&pool)
    .await
    .unwrap();
    for key in [
        "status",
        "commit_sha",
        "applied_proposals",
        "publish_failed_proposals",
        "withdrawn_proposals",
        "rejected_kb",
    ] {
        assert!(payload.get(key).is_some(), "audit old key `{key}`");
    }
    assert_eq!(payload["projc_applied"], 1);
    assert_eq!(payload["projc_withdrawn"], 1);

    // 幂等重放语义：重复 close 409（状态门共用，双域一致）
    let again = close_report(&pool, project, report, &closer).await;
    assert!(matches!(again, Err(AnalysisReportError::Conflict(_))), "{again:?}");
}

// ── G-2：ProjC 发布失败独立计数、不阻断关闭 ─────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn close_projc_publish_failure_is_independent_and_non_blocking(pool: PgPool) {
    let project = seed_project(&pool, SYSTEM_TENANT, "D5-P4").await;
    common::join_dev(&pool, project).await;
    let repo = seed_verified_repo(&pool, project).await;
    let (_doc, kb_rev) = seed_kb_draft(&pool, project).await;
    let pc_source = seed_pc_source(&pool, project, kb_rev).await;
    let projc_source = seed_projc_source(&pool, project, kb_rev).await;

    // ProjC 两条提案同期起草（基线均 0）：先手工发布第一条推到 v1，
    // 第二条 approved 但基线过期 → close 时 CAS 失配内置 failed。
    let projc_stale =
        projc_submitted_proposal_on(&pool, project, projc_source, "d5-projc-stale", false).await;
    let projc_ok =
        projc_submitted_proposal_on(&pool, project, projc_source, "d5-projc-ok", false).await;
    // 批准 projc_ok 后手工发布（v1），projc_stale 批准留待 close（基线已过期）
    projc_approve(
        &pool,
        project,
        &dev(),
        projc_ok,
        pm_application::projc::DecisionInput {
            reason: Some("决定理由".into()),
            expected_version_no: 0,
        },
    )
    .await
    .unwrap();
    publish_version(
        &pool,
        project,
        &dev(),
        ProjcPublishInput { proposal_id: projc_ok },
    )
    .await
    .unwrap();
    projc_approve(
        &pool,
        project,
        &dev(),
        projc_stale,
        pm_application::projc::DecisionInput {
            reason: Some("决定理由".into()),
            expected_version_no: 0,
        },
    )
    .await
    .unwrap();
    // PC 一条 approved（close 正常发布）——双域互不阻断
    let pc_approved = submitted_proposal_on(&pool, project, pc_source, "d5-pc-ok").await;
    decide_proposal(
        &pool,
        project,
        &dev(),
        pc_approved,
        ProposalDecisionInput {
            action: DecisionAction::Approve,
            reason: Some("决定理由".into()),
            expected_version_no: 0,
        },
    )
    .await
    .unwrap();
    let report = seed_analyzed_report(&pool, project, repo, kb_rev, pc_source).await;
    sqlx::query("UPDATE integration.repo_analysis_report SET projc_source_id = $2 WHERE id = $1")
        .bind(report)
        .bind(projc_source)
        .execute(&pool)
        .await
        .unwrap();

    let outcome = close_report(&pool, project, report, &interactive_dev())
        .await
        .unwrap();
    assert_eq!(outcome.applied_proposals, 1, "pc 分支不受 ProjC 失败影响");
    assert_eq!(outcome.projc_applied, 0);
    assert_eq!(
        outcome.projc_publish_failed_proposals, 1,
        "G-2: ProjC 发布失败独立计数"
    );
    assert_eq!(outcome.status, "closed", "失败不阻断关闭");
    let (stale_status,): (String,) =
        sqlx::query_as("SELECT status FROM projc.proposal WHERE id = $1")
            .bind(projc_stale)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stale_status, "failed", "CAS 失配提案内置 failed");
}

// ── G-1：NULL projc_source_id = 显式 Legacy（跳过 ProjC 分支必留痕）──────────

#[sqlx::test(migrations = "../../migrations")]
async fn close_legacy_report_without_projc_source_writes_audit_marker(pool: PgPool) {
    let project = seed_project(&pool, SYSTEM_TENANT, "D5-P5").await;
    common::join_dev(&pool, project).await;
    let repo = seed_verified_repo(&pool, project).await;
    let (_doc, kb_rev) = seed_kb_draft(&pool, project).await;
    let pc_source = seed_pc_source(&pool, project, kb_rev).await;
    // seed_analyzed_report 不写 projc_source_id → NULL = Legacy 行
    let report = seed_analyzed_report(&pool, project, repo, kb_rev, pc_source).await;

    let outcome = close_report(&pool, project, report, &interactive_dev())
        .await
        .unwrap();
    assert_eq!(outcome.status, "closed");
    assert_eq!(outcome.projc_applied, 0);
    assert_eq!(outcome.projc_withdrawn, 0);
    let (payload,): (serde_json::Value,) = sqlx::query_as(
        "SELECT after_data FROM governance.audit_log \
          WHERE action = 'analysis_report.closed' AND target_id = $1",
    )
    .bind(report)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        payload["projc_branch"], "legacy_no_source",
        "G-1: Legacy 跳过必须显式留痕，绝不静默"
    );
}

// ── W8：close_compat_old_consumers（§8.3 兼容矩阵）──────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn close_compat_old_consumers(pool: PgPool) {
    let project = seed_project(&pool, SYSTEM_TENANT, "D5-P6").await;
    common::join_dev(&pool, project).await;
    let repo = seed_verified_repo(&pool, project).await;
    let (doc, kb_rev) = seed_kb_draft(&pool, project).await;
    let pc_source = seed_pc_source(&pool, project, kb_rev).await;
    let projc_source = seed_projc_source(&pool, project, kb_rev).await;
    let report = seed_analyzed_report(&pool, project, repo, kb_rev, pc_source).await;
    sqlx::query("UPDATE integration.repo_analysis_report SET projc_source_id = $2 WHERE id = $1")
        .bind(report)
        .bind(projc_source)
        .execute(&pool)
        .await
        .unwrap();

    // C-10：CloseOutcome 旧 7 字段逐一存在（serde JSON 形态）
    let outcome = close_report(&pool, project, report, &interactive_dev())
        .await
        .unwrap();
    let out_json = serde_json::to_value(&outcome).unwrap();
    for key in [
        "report_id",
        "status",
        "applied_proposals",
        "publish_failed_proposals",
        "withdrawn_proposals",
        "rejected_kb",
        "last_analyzed_commit",
    ] {
        assert!(out_json.get(key).is_some(), "CloseOutcome old key `{key}`");
    }

    // C-9：审计 payload 旧 6 键逐一存在
    let (payload,): (serde_json::Value,) = sqlx::query_as(
        "SELECT after_data FROM governance.audit_log \
          WHERE action = 'analysis_report.closed' AND target_id = $1",
    )
    .bind(report)
    .fetch_one(&pool)
    .await
    .unwrap();
    for key in [
        "status",
        "commit_sha",
        "applied_proposals",
        "publish_failed_proposals",
        "withdrawn_proposals",
        "rejected_kb",
    ] {
        assert!(payload.get(key).is_some(), "audit old key `{key}`");
    }

    // C-11：详情旧 4 字段存在；proposals/pc_revisions 在无 pc 提案数据时为空数组；
    // 新增数组字段恒数组（空集 [] 而非 null）
    let detail = analysis_report::report_detail(&pool, project, report)
        .await
        .unwrap();
    let detail_json = serde_json::to_value(&detail).unwrap();
    assert!(detail_json.get("kb_revision").is_some());
    assert!(detail_json.get("proposals").is_some());
    assert!(detail_json.get("pc_revisions").is_some());
    assert_eq!(detail_json["proposals"], json!([]));
    assert_eq!(detail_json["pc_revisions"], json!([]));
    assert_eq!(detail_json["projc_proposals"], json!([]));
    assert_eq!(detail_json["projc_revisions"], json!([]));
    // 列表行旧字段逐一存在 + 新计数键
    let rows = analysis_report::list_reports(&pool, project, None, None)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    let row_json = serde_json::to_value(&rows[0]).unwrap();
    for key in [
        "id",
        "repository_id",
        "repository_name",
        "commit_sha",
        "status",
        "created_at",
        "updated_at",
        "kb_revision_id",
        "pc_source_id",
        "run_mode",
        "pending_proposals",
        "has_pending_kb",
    ] {
        assert!(row_json.get(key).is_some(), "list row old key `{key}`");
    }
    assert_eq!(row_json["projc_pending_proposals"], 0);
    assert_eq!(row_json["total_pending_proposals"], 0);
}
