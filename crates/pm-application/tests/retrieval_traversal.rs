//! WP-2 · 受控跨域多跳遍历集成测试。
//!
//! 覆盖：正向/反向方向语义（RT-030/031）、跨域关联（RT-005）、环检测（RT-014）、
//! draft 边隔离（RT-015）、跨项目与端点污染隔离（RT-010/036）、候选预算耗尽
//! （RT-035）、关系缺失的缺口报告（RT-011）、多路径保留（RT-032）。

use kb_pc_contracts::retrieval::{
    EntityAnchor, RetrievalCaller, RetrievalChannel, RetrievalMode, RetrievalPlan,
    TraversalDirection,
};
use kb_pc_contracts::RetrievalPort;
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::retrieval::service::RetrievalServiceLive;
use pm_application::retrieval::traversal::{self, TraversalRequest};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

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

/// 端点（PC 对象 / 外部域对象）。
#[derive(Clone, Copy)]
enum End {
    Object(Uuid),
    External { domain: &'static str, id: Uuid },
}

async fn seed_object(
    pool: &PgPool,
    project_id: Uuid,
    object_type: &str,
    key: &str,
    name: &str,
) -> Uuid {
    let created_by = principal(pool, "human", "seed").await;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.cognition_object \
         (tenant_id, project_id, object_type, object_key, display_name, lifecycle_status, created_by) \
         VALUES ($1, $2, $3, $4, $5, 'active', $6) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .bind(project_id)
    .bind(object_type)
    .bind(key)
    .bind(name)
    .bind(created_by)
    .fetch_one(pool)
    .await
    .unwrap();
    id
}

async fn seed_kb_document(pool: &PgPool, project_id: Uuid, doc_key: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO kb.document (tenant_id, project_id, doc_key, doc_type, canonical_path) \
         VALUES ($1, $2, $3, 'architecture', $4) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .bind(project_id)
    .bind(doc_key)
    .bind(format!("kb/{doc_key}.md"))
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 关系行（直接插表造 fixture；不经过确认链——RT-033 的生命周期断言另测）。
async fn seed_relation(
    pool: &PgPool,
    project_id: Uuid,
    type_key: &str,
    src: End,
    tgt: End,
    status: &str,
) -> Uuid {
    let created_by = principal(pool, "human", "rel-seed").await;
    let (s_obj, s_dom, s_typ, s_id) = match src {
        End::Object(id) => (Some(id), None, None, None),
        End::External { domain, id } => (None, Some(domain), Some("doc"), Some(id.to_string())),
    };
    let (t_obj, t_dom, t_typ, t_id) = match tgt {
        End::Object(id) => (Some(id), None, None, None),
        End::External { domain, id } => (None, Some(domain), Some("doc"), Some(id.to_string())),
    };
    sqlx::query_scalar(
        "INSERT INTO pc.relationship \
         (tenant_id, project_id, relationship_type_key, lifecycle_status, \
          source_object_id, target_object_id, \
          source_ext_domain, source_ext_type, source_ext_id, \
          target_ext_domain, target_ext_type, target_ext_id, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .bind(project_id)
    .bind(type_key)
    .bind(status)
    .bind(s_obj)
    .bind(t_obj)
    .bind(s_dom)
    .bind(s_typ)
    .bind(s_id)
    .bind(t_dom)
    .bind(t_typ)
    .bind(t_id)
    .bind(created_by)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 便捷：项目 + 成员。
async fn seed_project(pool: &PgPool, code: &str) -> (Uuid, Uuid) {
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
    (project_id, human_id)
}

fn caller(project_id: Uuid, principal_id: Uuid) -> RetrievalCaller {
    RetrievalCaller {
        principal_id,
        tenant_id: SYSTEM_TENANT,
        project_id,
        auth_mode: "interactive".into(),
    }
}

// ── 方向语义 ──────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn forward_traversal_records_forward_direction(pool: PgPool) {
    let (project_id, human_id) = seed_project(&pool, "TR-FWD").await;
    let module = seed_object(
        &pool,
        project_id,
        "feature_module",
        "order-module",
        "订单模块",
    )
    .await;
    let design = seed_kb_document(&pool, project_id, "order-design").await;
    seed_relation(
        &pool,
        project_id,
        "module-covered-by-design",
        End::Object(module),
        End::External {
            domain: "knowledge_base",
            id: design,
        },
        "active",
    )
    .await;

    let svc = RetrievalServiceLive::new(pool.clone(), None);
    let plan = RetrievalPlan {
        intent: kb_pc_contracts::retrieval::RetrievalIntent::ImpactAnalysis,
        mode: RetrievalMode::CrossDomain,
        domains: vec!["PC".into(), "KB".into()],
        retrieval_modes: vec![RetrievalChannel::PcEntity, RetrievalChannel::Relationship],
        entities: vec![EntityAnchor {
            kind: "pc_object".into(),
            type_hint: Some("feature_module".into()),
            key_or_name: "order-module".into(),
        }],
        rewritten_queries: vec!["订单模块".into()],
        traversal: Some(kb_pc_contracts::retrieval::TraversalSpec {
            enabled: true,
            max_hops: 3,
        }),
        analysis: None,
        include_history: false,
        budget: None,
    };
    let validated =
        pm_application::retrieval::validator::validate(&pool, plan, &caller(project_id, human_id))
            .await
            .expect("plan must validate");
    let result = svc.expand(&validated).await.unwrap();

    assert_eq!(result.edges.len(), 1);
    assert_eq!(
        result.edges[0].traversal_direction,
        TraversalDirection::Forward
    );
    assert_eq!(
        result.edges[0].relationship_type,
        "module-covered-by-design"
    );
    assert!(result.nodes.contains(&format!("pc:{module}")));
    assert!(result.nodes.contains(&format!("knowledge_base:{design}")));
}

#[sqlx::test(migrations = "../../migrations")]
async fn reverse_traversal_records_reverse_direction(pool: PgPool) {
    let (project_id, human_id) = seed_project(&pool, "TR-REV").await;
    let module = seed_object(
        &pool,
        project_id,
        "feature_module",
        "order-module",
        "订单模块",
    )
    .await;
    let design = seed_kb_document(&pool, project_id, "order-design").await;
    seed_relation(
        &pool,
        project_id,
        "module-covered-by-design",
        End::Object(module),
        End::External {
            domain: "knowledge_base",
            id: design,
        },
        "active",
    )
    .await;

    let svc = RetrievalServiceLive::new(pool.clone(), None);
    let plan = RetrievalPlan {
        intent: kb_pc_contracts::retrieval::RetrievalIntent::ImpactAnalysis,
        mode: RetrievalMode::CrossDomain,
        domains: vec!["KB".into()],
        retrieval_modes: vec![RetrievalChannel::Relationship],
        entities: vec![EntityAnchor {
            kind: "kb_document".into(),
            type_hint: None,
            key_or_name: "order-design".into(),
        }],
        rewritten_queries: vec!["订单设计".into()],
        traversal: Some(kb_pc_contracts::retrieval::TraversalSpec {
            enabled: true,
            max_hops: 3,
        }),
        analysis: None,
        include_history: false,
        budget: None,
    };
    let validated =
        pm_application::retrieval::validator::validate(&pool, plan, &caller(project_id, human_id))
            .await
            .expect("plan must validate");
    let result = svc.expand(&validated).await.unwrap();

    assert_eq!(result.edges.len(), 1);
    assert_eq!(
        result.edges[0].traversal_direction,
        TraversalDirection::Reverse,
        "traversing from the KB side must record Reverse, not a fabricated relation type"
    );
    // 反向不得虚构关系类型——类型名保持注册表原值。
    assert_eq!(
        result.edges[0].relationship_type,
        "module-covered-by-design"
    );
}

// ── 环 / 隔离 / 预算 ──────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn cyclic_relations_do_not_loop_forever(pool: PgPool) {
    let (project_id, human_id) = seed_project(&pool, "TR-CYCLE").await;
    let a = seed_object(&pool, project_id, "domain_object", "obj-a", "对象甲").await;
    let b = seed_object(&pool, project_id, "domain_object", "obj-b", "对象乙").await;
    seed_relation(
        &pool,
        project_id,
        "object-referenced-by-requirement",
        End::Object(a),
        End::Object(b),
        "active",
    )
    .await;
    seed_relation(
        &pool,
        project_id,
        "object-referenced-by-requirement",
        End::Object(b),
        End::Object(a),
        "active",
    )
    .await;

    let svc = RetrievalServiceLive::new(pool.clone(), None);
    let plan = RetrievalPlan {
        intent: kb_pc_contracts::retrieval::RetrievalIntent::ImpactAnalysis,
        mode: RetrievalMode::CrossDomain,
        domains: vec!["PC".into()],
        retrieval_modes: vec![RetrievalChannel::Relationship],
        entities: vec![EntityAnchor {
            kind: "pc_object".into(),
            type_hint: None,
            key_or_name: "obj-a".into(),
        }],
        rewritten_queries: vec!["对象甲".into()],
        traversal: Some(kb_pc_contracts::retrieval::TraversalSpec {
            enabled: true,
            max_hops: 3,
        }),
        analysis: None,
        include_history: false,
        budget: None,
    };
    let validated =
        pm_application::retrieval::validator::validate(&pool, plan, &caller(project_id, human_id))
            .await
            .expect("plan must validate");
    let result = svc.expand(&validated).await.unwrap();

    // 环不无限展开：边数有界（两条关系各至多走两次方向）。
    assert!(
        result.edges.len() <= 4,
        "cycle must not explode: {}",
        result.edges.len()
    );
    assert!(result.nodes.len() <= 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn draft_relations_never_appear_in_traversal(pool: PgPool) {
    let (project_id, human_id) = seed_project(&pool, "TR-DRAFT").await;
    let module = seed_object(
        &pool,
        project_id,
        "feature_module",
        "order-module",
        "订单模块",
    )
    .await;
    let design = seed_kb_document(&pool, project_id, "order-design").await;
    seed_relation(
        &pool,
        project_id,
        "module-covered-by-design",
        End::Object(module),
        End::External {
            domain: "knowledge_base",
            id: design,
        },
        "draft",
    )
    .await;

    let svc = RetrievalServiceLive::new(pool.clone(), None);
    let plan = RetrievalPlan {
        intent: kb_pc_contracts::retrieval::RetrievalIntent::ImpactAnalysis,
        mode: RetrievalMode::CrossDomain,
        domains: vec!["PC".into()],
        retrieval_modes: vec![RetrievalChannel::Relationship],
        entities: vec![EntityAnchor {
            kind: "pc_object".into(),
            type_hint: None,
            key_or_name: "order-module".into(),
        }],
        rewritten_queries: vec!["订单模块".into()],
        traversal: Some(kb_pc_contracts::retrieval::TraversalSpec {
            enabled: true,
            max_hops: 3,
        }),
        analysis: None,
        include_history: false,
        budget: None,
    };
    let validated =
        pm_application::retrieval::validator::validate(&pool, plan, &caller(project_id, human_id))
            .await
            .expect("plan must validate");
    let result = svc.expand(&validated).await.unwrap();

    assert!(
        result.edges.is_empty(),
        "draft edges must not be traversable"
    );
    assert!(result.nodes.is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn cross_project_edges_and_endpoint_pollution_are_excluded(pool: PgPool) {
    let (project_id, human_id) = seed_project(&pool, "TR-ISO").await;
    let (other_project, _) = seed_project(&pool, "TR-ISO-OTHER").await;

    let module = seed_object(
        &pool,
        project_id,
        "feature_module",
        "order-module",
        "订单模块",
    )
    .await;
    // 另一项目的对象（同租户）——端点污染 fixture。
    let foreign = seed_object(
        &pool,
        other_project,
        "domain_object",
        "foreign",
        "他项目对象",
    )
    .await;

    // PC 对象端点的跨项目污染：**库层 FK 直接拒绝**
    // （relationship_..._target_object_id_fkey）——这是第一道防线。
    let rejected = sqlx::query(
        "INSERT INTO pc.relationship          (tenant_id, project_id, relationship_type_key, lifecycle_status,           source_object_id, target_object_id, created_by)          VALUES ($1, $2, 'module-covered-by-design', 'active', $3, $4, $5)",
    )
    .bind(SYSTEM_TENANT)
    .bind(project_id)
    .bind(module)
    .bind(foreign)
    .bind(principal(&pool, "human", "pollution").await)
    .execute(&pool)
    .await;
    assert!(
        rejected.is_err(),
        "FK must reject a cross-project object endpoint"
    );

    // ext 端点**无物理 FK**（受控 ID 引用）→ 用它验证双端隔离的第二道防线
    // （pc.endpoint_in_scope）。
    let foreign_doc = seed_kb_document(&pool, other_project, "foreign-design").await;
    seed_relation(
        &pool,
        project_id,
        "module-covered-by-design",
        End::Object(module),
        End::External {
            domain: "knowledge_base",
            id: foreign_doc,
        },
        "active",
    )
    .await;
    // 注：**跨项目的关系行在库层不可能存在**——FK
    // relationship_(tenant_id, project_id, *_object_id)_fkey 强制对象端点与关系行
    // 同项目；本项目遍历因此天然看不到他项目关系（无需另测）。

    let svc = RetrievalServiceLive::new(pool.clone(), None);
    let plan = RetrievalPlan {
        intent: kb_pc_contracts::retrieval::RetrievalIntent::ImpactAnalysis,
        mode: RetrievalMode::CrossDomain,
        domains: vec!["PC".into()],
        retrieval_modes: vec![RetrievalChannel::Relationship],
        entities: vec![EntityAnchor {
            kind: "pc_object".into(),
            type_hint: None,
            key_or_name: "order-module".into(),
        }],
        rewritten_queries: vec!["订单模块".into()],
        traversal: Some(kb_pc_contracts::retrieval::TraversalSpec {
            enabled: true,
            max_hops: 3,
        }),
        analysis: None,
        include_history: false,
        budget: None,
    };
    let validated =
        pm_application::retrieval::validator::validate(&pool, plan, &caller(project_id, human_id))
            .await
            .expect("plan must validate");
    let result = svc.expand(&validated).await.unwrap();

    assert!(
        result.edges.is_empty(),
        "cross-project edges and polluted endpoints must be excluded, got {:?}",
        result.edges
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn candidate_budget_exhaustion_is_reported(pool: PgPool) {
    let (project_id, human_id) = seed_project(&pool, "TR-BUDGET").await;
    let a = seed_object(&pool, project_id, "domain_object", "obj-a", "对象甲").await;
    let b = seed_object(&pool, project_id, "domain_object", "obj-b", "对象乙").await;
    let c = seed_object(&pool, project_id, "domain_object", "obj-c", "对象丙").await;
    seed_relation(
        &pool,
        project_id,
        "object-referenced-by-requirement",
        End::Object(a),
        End::Object(b),
        "active",
    )
    .await;
    seed_relation(
        &pool,
        project_id,
        "object-referenced-by-requirement",
        End::Object(a),
        End::Object(c),
        "active",
    )
    .await;

    let svc = RetrievalServiceLive::new(pool.clone(), None);
    let plan = RetrievalPlan {
        intent: kb_pc_contracts::retrieval::RetrievalIntent::ImpactAnalysis,
        mode: RetrievalMode::CrossDomain,
        domains: vec!["PC".into()],
        retrieval_modes: vec![RetrievalChannel::Relationship],
        entities: vec![EntityAnchor {
            kind: "pc_object".into(),
            type_hint: None,
            key_or_name: "obj-a".into(),
        }],
        rewritten_queries: vec!["对象甲".into()],
        traversal: Some(kb_pc_contracts::retrieval::TraversalSpec {
            enabled: true,
            max_hops: 3,
        }),
        analysis: None,
        include_history: false,
        budget: Some(kb_pc_contracts::retrieval::RetrievalBudget {
            max_candidates: 1,
            max_hops: 3,
            timeout_ms: 8_000,
        }),
    };
    let validated =
        pm_application::retrieval::validator::validate(&pool, plan, &caller(project_id, human_id))
            .await
            .expect("plan must validate");
    let result = svc.expand(&validated).await.unwrap();

    assert_eq!(result.edges.len(), 1, "budget must cap the returned edges");
    assert!(
        result
            .gaps
            .iter()
            .any(|g| g.kind == "candidate_budget_exceeded"),
        "budget exhaustion must be reported as a gap, got {:?}",
        result.gaps
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn missing_relation_yields_empty_result_not_fabricated_edges(pool: PgPool) {
    let (project_id, human_id) = seed_project(&pool, "TR-EMPTY").await;
    let module = seed_object(
        &pool,
        project_id,
        "feature_module",
        "lonely-module",
        "孤立模块",
    )
    .await;

    let svc = RetrievalServiceLive::new(pool.clone(), None);
    let plan = RetrievalPlan {
        intent: kb_pc_contracts::retrieval::RetrievalIntent::ImpactAnalysis,
        mode: RetrievalMode::CrossDomain,
        domains: vec!["PC".into()],
        retrieval_modes: vec![RetrievalChannel::Relationship],
        entities: vec![EntityAnchor {
            kind: "pc_object".into(),
            type_hint: None,
            key_or_name: "lonely-module".into(),
        }],
        rewritten_queries: vec!["孤立模块".into()],
        traversal: Some(kb_pc_contracts::retrieval::TraversalSpec {
            enabled: true,
            max_hops: 3,
        }),
        analysis: None,
        include_history: false,
        budget: None,
    };
    let validated =
        pm_application::retrieval::validator::validate(&pool, plan, &caller(project_id, human_id))
            .await
            .expect("plan must validate");
    let result = svc.expand(&validated).await.unwrap();

    assert!(result.edges.is_empty(), "no fabricated relations");
    assert!(!result.nodes.contains(&format!("pc:{module}")));
}

#[sqlx::test(migrations = "../../migrations")]
async fn owner_domain_seeds_are_registered_and_scoped(pool: PgPool) {
    // 存量 12 条全归 PC；新增 KB 种子明确归 kb。
    let pc_owned: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pc.relationship_type WHERE owner_domain = 'pc'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        pc_owned >= 12,
        "existing PC seeds must stay pc-owned: {pc_owned}"
    );

    let kb_owned: Vec<String> = sqlx::query_scalar(
        "SELECT type_key FROM pc.relationship_type WHERE owner_domain = 'kb' ORDER BY type_key",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        kb_owned,
        vec![
            "adr-documented-in".to_string(),
            "design-constrained-by-adr".to_string(),
            "requirement-implemented-by-design".to_string(),
        ],
        "KB-owned relation types must be exactly the three KB-side seeds"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn direct_traversal_api_keeps_scope_isolation(pool: PgPool) {
    // 底层遍历 API 的直接调用（不经 planner）：跨项目种子不得越权。
    let (project_id, _) = seed_project(&pool, "TR-DIRECT").await;
    let (other_project, _) = seed_project(&pool, "TR-DIRECT-OTHER").await;
    let foreign = seed_object(
        &pool,
        other_project,
        "domain_object",
        "foreign",
        "他项目对象",
    )
    .await;

    let request = TraversalRequest {
        seed_nodes: vec![format!("pc:{foreign}")],
        max_hops: 3,
        max_candidates: 60,
        timeout_ms: 8_000,
    };
    let result = traversal::traverse(&pool, SYSTEM_TENANT, project_id, &request).await;
    assert!(
        result.edges.is_empty(),
        "a seed node outside the project must yield no edges"
    );
}
