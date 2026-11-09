//! 统一检索面（WP-1）应用层集成测试：降级链、通道、隔离、恶意计划拒绝。
//!
//! 覆盖（裁决验收映射）：
//! - RT-016 Planner OFF：Planner 未注入 → 不调 LLM、降级 `planner_disabled`、
//!   双域直查仍可用；
//! - RT-017 Planner 超时 → 降级 `planner_timeout` 并返回结果；
//! - RT-018 Planner 坏 JSON → 降级 `invalid_planner_output` 并返回结果；
//! - RT-019 项目无模型策略 → 降级 `project_model_policy_missing`；
//! - RT-003 PC 独立检索命中 PC 实体；RT-004 KB 独立检索（V1 索引未落地 → 通道
//!   不可用原因码进 degradations）；
//! - RT-010 跨项目/非成员 → 拒绝（不越权）；
//! - RT-026 恶意 Planner 输出（SQL / 内部表名）→ 校验拒绝。

use std::sync::Arc;

use kb_pc_contracts::retrieval::{PlannerModel, PlannerModelError, RetrievalCaller};
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::retrieval::service::RetrievalServiceLive;
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

/// 假规划模型（按预设脚本返回）。
struct FakePlanner {
    result: Result<String, PlannerModelError>,
}

#[async_trait::async_trait]
impl PlannerModel for FakePlanner {
    async fn plan(&self, _query: &str) -> Result<String, PlannerModelError> {
        self.result.clone()
    }
}

async fn principal(pool: &PgPool, kind: &str, name: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO core.principal (kind, display_name) VALUES ($1, $2) RETURNING id",
    )
    .bind(kind)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 项目 + 成员（owner 角色）+ 一个 PC 认知对象（含正文）。
async fn seed(pool: &PgPool, code: &str) -> (Uuid, Uuid) {
    let project_id = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap()
    .project_id;

    let human_id = principal(pool, "human", "searcher").await;
    sqlx::query(
        "INSERT INTO core.project_member (project_id, principal_id, role, tenant_id) \
         VALUES ($1, $2, 'owner', $3)",
    )
    .bind(project_id)
    .bind(human_id)
    .bind(SYSTEM_TENANT)
    .execute(pool)
    .await
    .unwrap();

    // PC 文档 + 修订（正文）+ 认知对象 + 认知修订。
    let document_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.document (tenant_id, project_id, doc_key, canonical_path) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .bind(project_id)
    .bind("pc-order-status")
    .bind("pc/order-status.md")
    .fetch_one(pool)
    .await
    .unwrap();

    let document_revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.document_revision \
         (tenant_id, project_id, document_id, revision_no, content_hash, content, created_by) \
         VALUES ($1, $2, $3, 1, 'hash-1', $4, $5) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .bind(project_id)
    .bind(document_id)
    .bind("# 订单状态\n\n订单状态机描述：待付款 → 已付款 → 已发货。")
    .bind(human_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let object_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.cognition_object \
         (tenant_id, project_id, object_type, object_key, display_name, lifecycle_status, created_by) \
         VALUES ($1, $2, 'domain_object', 'order-status', '订单状态', 'active', $3) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .bind(project_id)
    .bind(human_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.cognition_revision \
         (tenant_id, project_id, object_id, revision_no, created_by, document_id, \
          document_revision_id, content_hash) \
         VALUES ($1, $2, $3, 1, $4, $5, $6, 'hash-1') RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .bind(project_id)
    .bind(object_id)
    .bind(human_id)
    .bind(document_id)
    .bind(document_revision_id)
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query("UPDATE pc.cognition_object SET current_revision_id = $2 WHERE id = $1")
        .bind(object_id)
        .bind(revision_id)
        .execute(pool)
        .await
        .unwrap();

    (project_id, human_id)
}

fn caller_for(project_id: Uuid, principal_id: Uuid) -> RetrievalCaller {
    RetrievalCaller {
        principal_id,
        tenant_id: SYSTEM_TENANT,
        project_id,
        auth_mode: "interactive".into(),
    }
}

/// 合法计划 JSON（PcOnly + pc_entity 通道）。
fn valid_plan_json() -> String {
    serde_json::json!({
        "intent": "factual_lookup",
        "mode": "pc_only",
        "domains": ["PC"],
        "retrieval_modes": ["pc_entity"],
        "entities": [],
        "rewritten_queries": ["订单状态"],
        "include_history": false,
        "budget": { "max_candidates": 60, "max_hops": 3, "timeout_ms": 8000 }
    })
    .to_string()
}

// ── 降级链 ────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn planner_off_falls_back_to_deterministic_dual_domain(pool: PgPool) {
    let (project_id, human_id) = seed(&pool, "RT-016").await;
    // Planner 未注入 = 关闭。
    let svc = RetrievalServiceLive::new(pool.clone(), None);
    let resp = svc
        .query("订单状态", &caller_for(project_id, human_id))
        .await
        .unwrap();

    assert_eq!(resp.plan_source, "deterministic_fallback");
    assert!(resp.degraded);
    assert_eq!(resp.degradation_reason.as_deref(), Some("planner_disabled"));
    assert!(resp.validation.all_passed());
    // 双域直查仍可用：PC 实体命中。
    assert!(
        resp.evidence
            .evidence
            .iter()
            .any(|e| e.title.as_deref() == Some("订单状态")),
        "deterministic fallback must still retrieve PC entities"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn planner_timeout_degrades_with_reason_and_still_returns(pool: PgPool) {
    let (project_id, human_id) = seed(&pool, "RT-017").await;
    let svc = RetrievalServiceLive::new(
        pool.clone(),
        Some(Arc::new(FakePlanner {
            result: Err(PlannerModelError::Timeout),
        })),
    );
    let resp = svc
        .query("订单状态", &caller_for(project_id, human_id))
        .await
        .unwrap();
    assert!(resp.degraded);
    assert_eq!(resp.degradation_reason.as_deref(), Some("planner_timeout"));
    assert!(!resp.evidence.evidence.is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn planner_invalid_output_degrades_with_reason(pool: PgPool) {
    let (project_id, human_id) = seed(&pool, "RT-018").await;
    let svc = RetrievalServiceLive::new(
        pool.clone(),
        Some(Arc::new(FakePlanner {
            result: Ok("{not json".into()),
        })),
    );
    let resp = svc
        .query("订单状态", &caller_for(project_id, human_id))
        .await
        .unwrap();
    assert!(resp.degraded);
    assert_eq!(
        resp.degradation_reason.as_deref(),
        Some("invalid_planner_output")
    );
    assert!(!resp.evidence.evidence.is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn missing_model_policy_degrades_with_reason(pool: PgPool) {
    let (project_id, human_id) = seed(&pool, "RT-019").await;
    let svc = RetrievalServiceLive::new(
        pool.clone(),
        Some(Arc::new(FakePlanner {
            result: Err(PlannerModelError::NoProvider),
        })),
    );
    let resp = svc
        .query("订单状态", &caller_for(project_id, human_id))
        .await
        .unwrap();
    assert!(resp.degraded);
    assert_eq!(
        resp.degradation_reason.as_deref(),
        Some("project_model_policy_missing")
    );
}

// ── 通道与隔离 ────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn pc_only_plan_hits_pc_entity(pool: PgPool) {
    let (project_id, human_id) = seed(&pool, "RT-003").await;
    let svc = RetrievalServiceLive::new(
        pool.clone(),
        Some(Arc::new(FakePlanner {
            result: Ok(valid_plan_json()),
        })),
    );
    let resp = svc
        .query("订单状态", &caller_for(project_id, human_id))
        .await
        .unwrap();

    assert_eq!(resp.plan_source, "llm_planner");
    assert!(!resp.degraded);
    assert!(resp.validation.all_passed());
    assert_eq!(resp.evidence.evidence.len(), 1);
    let hit = &resp.evidence.evidence[0];
    assert_eq!(hit.title.as_deref(), Some("订单状态"));
    assert_eq!(
        hit.authority_status,
        kb_pc_contracts::retrieval::AuthorityStatus::CurrentPublished
    );
    assert!(hit.content_excerpt.contains("订单状态机"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn kb_only_plan_reports_index_unavailable(pool: PgPool) {
    let (project_id, human_id) = seed(&pool, "RT-004").await;
    let plan_json = serde_json::json!({
        "intent": "factual_lookup",
        "mode": "kb_only",
        "domains": ["KB"],
        "retrieval_modes": ["kb_section"],
        "rewritten_queries": ["需求"],
        "budget": { "max_candidates": 60, "max_hops": 3, "timeout_ms": 8000 }
    })
    .to_string();
    let svc = RetrievalServiceLive::new(
        pool.clone(),
        Some(Arc::new(FakePlanner {
            result: Ok(plan_json),
        })),
    );
    let resp = svc
        .query("需求", &caller_for(project_id, human_id))
        .await
        .unwrap();
    assert!(resp.validation.all_passed());
    // V1 索引未落地 → 通道不可用原因码如实进 degradations（不假装有结果）。
    assert!(
        resp.evidence
            .degradations
            .contains(&"kb_section_index_unavailable".to_string()),
        "unavailable channel must be reported, got {:?}",
        resp.evidence.degradations
    );
    assert!(resp.evidence.evidence.is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn non_member_cannot_retrieve(pool: PgPool) {
    let (project_id, _) = seed(&pool, "RT-010").await;
    let outsider = principal(&pool, "human", "outsider").await;
    let svc = RetrievalServiceLive::new(pool.clone(), None);
    let err = svc
        .query("订单状态", &caller_for(project_id, outsider))
        .await
        .expect_err("non-member must be rejected");
    assert!(
        matches!(
            err,
            pm_application::retrieval::service::RetrievalError::Forbidden(_)
        ),
        "non-member must be forbidden, got {err:?}"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn malicious_plan_output_is_rejected_by_validator(pool: PgPool) {
    let (project_id, human_id) = seed(&pool, "RT-026").await;
    let malicious = serde_json::json!({
        "intent": "factual_lookup",
        "mode": "pc_only",
        "retrieval_modes": ["pc_entity"],
        "rewritten_queries": ["SELECT * FROM governance.audit_log"],
        "budget": { "max_candidates": 60, "max_hops": 3, "timeout_ms": 8000 }
    })
    .to_string();
    let svc = RetrievalServiceLive::new(
        pool.clone(),
        Some(Arc::new(FakePlanner {
            result: Ok(malicious),
        })),
    );
    let err = svc
        .query("订单状态", &caller_for(project_id, human_id))
        .await
        .expect_err("malicious plan must be rejected by the validator");
    assert!(
        matches!(
            err,
            pm_application::retrieval::service::RetrievalError::Forbidden(_)
        ),
        "malicious plan must be forbidden, got {err:?}"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn cross_domain_plan_passes_validation_but_traversal_is_not_wired(pool: PgPool) {
    let (project_id, human_id) = seed(&pool, "RT-CROSSDOMAIN").await;
    // 关系类型注册表已有 active 种子 → 白名单非空 → 校验通过；**遍历引擎 WP-2 才
    // 接真**（本批 expand 恒未接线），所以跨域计划可过校验但不产生关系路径。
    let plan_json = serde_json::json!({
        "intent": "impact_analysis",
        "mode": "cross_domain",
        "retrieval_modes": ["pc_entity", "relationship"],
        "rewritten_queries": ["订单状态"],
        "traversal": { "enabled": true, "max_hops": 3 },
        "budget": { "max_candidates": 60, "max_hops": 3, "timeout_ms": 8000 }
    })
    .to_string();
    let svc = RetrievalServiceLive::new(
        pool.clone(),
        Some(Arc::new(FakePlanner {
            result: Ok(plan_json),
        })),
    );
    let resp = svc
        .query("订单状态", &caller_for(project_id, human_id))
        .await
        .expect("cross-domain plan must pass validation (whitelist non-empty)");
    assert!(resp.validation.all_passed());
    // 本批未接遍历 → 不产生关系路径（不假装有影响分析结论）。
    assert!(resp.evidence.traversal.edges.is_empty());
    assert!(resp
        .evidence
        .evidence
        .iter()
        .all(|e| e.relationship_path.is_empty()));
}
