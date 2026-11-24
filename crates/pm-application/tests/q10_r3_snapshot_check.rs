//! P3-D04-02 Step6 前置 · Q10-R1-FIX-01-R3 快照一致性核查测试（专家补证要求 §二.4）。
//!
//! 证明对象：R-1 分类查询（修复将采用的单条 JOIN 文本）在「读取与发布提交交错」
//! 下**永不产生混合观察对**——applied 与 completed 同事务原子提交（R2），单语句
//! 单快照（READ COMMITTED 语句级）只能看到提交前或提交后的一致二元组：
//!   合法对（V1.1 精确矩阵）：(approved, 无行) 或 (applied, completed)——仅此两元
//!   非法混合对（必须为 0）：(approved, completed) / (pending|failed, applied)
//! 两域各一用例；读取循环与发布事务真并发（tokio::join!）。

use sqlx::PgPool;
use uuid::Uuid;

use pm_application::actor::AuditActor;
use pm_application::pc::{
    create_proposal, publish_version, register_source, submit_proposal, upsert_object_draft,
    CreateProposalInput, ProposalMemberPin, PublishVersionInput, SourceInput, UpsertObjectDraft,
};
use pm_application::projc::{
    create_proposal as projc_create, publish_version as projc_publish,
    register_source as projc_source, submit_proposal as projc_submit,
    upsert_object_draft as projc_upsert, CreateProposalInput as ProjcCreate,
    ProposalMemberPin as ProjcPin, PublishVersionInput as ProjcPub, SourceInput as ProjcSrc,
    UpsertObjectDraft as ProjcDraft,
};

const LEAD: Uuid = Uuid::from_u128(0x7072_0000_0000_0000_0000_0000_0000_0021);

async fn seed(pool: &PgPool, tag: &str) -> (Uuid, Uuid) {
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('system', $1, $1) RETURNING id",
    )
    .bind(format!("r3-{tag}-{}", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .unwrap();
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(format!("r3-c-{tag}"))
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    let project: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, $2, $3) RETURNING id",
    )
    .bind(customer)
    .bind(format!("r3-{tag}"))
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES ($1, 'human', 'r3-lead', true) ON CONFLICT (id) DO NOTHING",
    )
    .bind(LEAD)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ($1, $2, $3, 'owner') ON CONFLICT DO NOTHING",
    )
    .bind(tenant)
    .bind(project)
    .bind(LEAD)
    .execute(pool)
    .await
    .unwrap();
    (tenant, project)
}

async fn pc_approved_proposal(pool: &PgPool, project: Uuid) -> Uuid {
    let source = register_source(
        pool,
        project,
        &AuditActor::principal(LEAD),
        SourceInput {
            source_kind: "manual_input".into(),
            target_domain: "pc".into(),
            target_type: "analysis".into(),
            target_id: Uuid::new_v4().to_string(),
            target_revision_id: None,
            revision_missing_reason: Some("r3 fixture".into()),
            section_anchor: None,
            start_line: None,
            locator: None,
            description: None,
            source_content_hash: None,
        },
    )
    .await
    .unwrap();
    let obj = upsert_object_draft(
        pool,
        project,
        &AuditActor::principal(LEAD),
        UpsertObjectDraft {
            object_type: "feature".into(),
            object_key: Some("r3-pc".into()),
            display_name: "r3".into(),
            content_markdown: "# r3".into(),
            change_summary: Some("r3".into()),
            source_context: None,
            analysis_mode: None,
        },
    )
    .await
    .unwrap();
    let drafted = create_proposal(
        pool,
        project,
        &AuditActor::principal(LEAD),
        CreateProposalInput {
            title: "r3 pc".into(),
            rationale: "r3".into(),
            origin_kind: "manual_input".into(),
            source_id: Some(source),
            governance_level: None,
            risk_level: None,
            confidence: None,
            members: vec![ProposalMemberPin {
                change_kind: "create_object".into(),
                object_id: Some(obj.object_id),
                object_revision_id: Some(obj.revision_id),
                relationship_id: None,
                relationship_revision_id: None,
            }],
        },
    )
    .await
    .unwrap();
    submit_proposal(
        pool,
        project,
        &AuditActor::principal(LEAD),
        drafted.proposal_id,
        Some(0),
    )
    .await
    .unwrap();
    pm_application::pc::approve_proposal(
        pool,
        project,
        &AuditActor::principal(LEAD),
        drafted.proposal_id,
        pm_application::pc::DecisionInput {
            reason: Some("r3".into()),
            expected_version_no: 0,
        },
    )
    .await
    .unwrap();
    drafted.proposal_id
}

/// R-1 分类查询（修复将采用的**单条 JOIN**文本——单语句单快照）。
const R1_CLASSIFY_SQL_PC: &str = "SELECT p.status AS proposal_status, wo.status AS ledger_status \
     FROM pc.proposal p \
     LEFT JOIN pc.write_operation wo \
       ON wo.tenant_id = p.tenant_id AND wo.project_id = p.project_id \
      AND wo.operation_id = 'pc-decision:' || p.id::text \
    WHERE p.id = $1";

#[sqlx::test(migrations = "../../migrations")]
async fn pc_single_statement_join_never_observes_mixed_pair(pool: PgPool) {
    let (_t, project) = seed(&pool, "pc").await;
    let proposal = pc_approved_proposal(&pool, project).await;

    // 读取循环 ∥ 发布事务（一次性把 approved+无行 → applied+completed）
    let p1 = pool.clone();
    let (pub_r, observations) = tokio::join!(
        async move {
            publish_version(
                &p1,
                project,
                &AuditActor::principal(LEAD),
                PublishVersionInput {
                    proposal_id: proposal,
                },
            )
            .await
        },
        async {
            let mut seen: Vec<(String, Option<String>)> = Vec::new();
            for _ in 0..2000 {
                let row: Option<(String, Option<String>)> = sqlx::query_as(R1_CLASSIFY_SQL_PC)
                    .bind(proposal)
                    .fetch_optional(&pool)
                    .await
                    .unwrap();
                if let Some(pair) = row {
                    seen.push(pair);
                }
                // 交错窗口最大化：读侧不锁不等待，与发布事务真并发
            }
            seen
        },
    );
    pub_r.unwrap();

    assert!(!observations.is_empty(), "必须采集到观测");
    // V1.1 精确矩阵（废弃 V1.0 的 (approved, _) 通配——该通配会放过 (approved, completed)
    // 混合快照，属测试缺陷）。合法域按账本行全生命周期路径逐项定义（见核查 V1.1）：
    //   提交前：(approved, 无行)——账本行仅在发布事务内诞生，中途态不可见
    //   提交后：(applied, completed)——同事务原子提交的可观察面
    // 其余一切组合均非法（含显式反例：(approved, completed)/(applied, pending)/
    // (applied, failed)/(applied, 无行)）。
    let mut illegal: Vec<(String, Option<String>)> = Vec::new();
    for pair in &observations {
        let legal = matches!(
            (pair.0.as_str(), pair.1.as_deref()),
            ("approved", None) | ("applied", Some("completed"))
        );
        if !legal {
            illegal.push(pair.clone());
        }
    }
    assert!(
        illegal.is_empty(),
        "单条 JOIN 出现非法观察对（快照一致性破坏）: {illegal:?}——合法域仅          (approved, 无行) 与 (applied, completed) 两元"
    );
    // 终态断言：最后一次观测必为提交后一致对
    let (last_p, last_w) = observations.last().unwrap();
    assert_eq!(
        (last_p.as_str(), last_w.as_deref()),
        ("applied", Some("completed")),
        "发布完成后观测收敛于一致对"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn projc_single_statement_join_never_observes_mixed_pair(pool: PgPool) {
    let (_t, project) = seed(&pool, "pj").await;
    let source = projc_source(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        ProjcSrc {
            source_kind: "manual_input".into(),
            target_domain: "knowledge_base".into(),
            target_type: "analysis".into(),
            target_id: Uuid::new_v4().to_string(),
            target_revision_id: None,
            revision_missing_reason: Some("r3 fixture".into()),
            section_anchor: None,
            start_line: None,
            locator: None,
            description: None,
            source_content_hash: None,
        },
    )
    .await
    .unwrap();
    let obj = projc_upsert(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        ProjcDraft {
            object_type: "tech_stack".into(),
            object_key: Some("r3-pj".into()),
            display_name: "r3".into(),
            content_markdown: "# r3".into(),
            change_summary: Some("r3".into()),
            source_context: None,
            analysis_mode: None,
        },
    )
    .await
    .unwrap();
    let drafted = projc_create(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        ProjcCreate {
            title: "r3 pj".into(),
            rationale: "r3".into(),
            origin_kind: "manual_input".into(),
            source_id: Some(source),
            governance_level: None,
            risk_level: None,
            confidence: None,
            members: vec![ProjcPin {
                change_kind: "create_object".into(),
                object_id: Some(obj.object_id),
                object_revision_id: Some(obj.revision_id),
                relationship_id: None,
                relationship_revision_id: None,
            }],
        },
    )
    .await
    .unwrap();
    projc_submit(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        drafted.proposal_id,
        Some(0),
    )
    .await
    .unwrap();
    pm_application::projc::approve_proposal(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        drafted.proposal_id,
        pm_application::projc::DecisionInput {
            reason: Some("r3".into()),
            expected_version_no: 0,
        },
    )
    .await
    .unwrap();
    let proposal = drafted.proposal_id;

    let sql = "SELECT p.status AS proposal_status, wo.status AS ledger_status \
         FROM projc.proposal p \
         LEFT JOIN projc.write_operation wo \
           ON wo.tenant_id = p.tenant_id AND wo.project_id = p.project_id \
          AND wo.operation_id = 'projc-decision:' || p.id::text \
        WHERE p.id = $1";
    let p1 = pool.clone();
    let (pub_r, observations) = tokio::join!(
        async move {
            projc_publish(
                &p1,
                project,
                &AuditActor::principal(LEAD),
                ProjcPub {
                    proposal_id: proposal,
                },
            )
            .await
        },
        async {
            let mut seen: Vec<(String, Option<String>)> = Vec::new();
            for _ in 0..2000 {
                let row: Option<(String, Option<String>)> = sqlx::query_as(sql)
                    .bind(proposal)
                    .fetch_optional(&pool)
                    .await
                    .unwrap();
                if let Some(pair) = row {
                    seen.push(pair);
                }
            }
            seen
        },
    );
    pub_r.unwrap();

    let illegal: Vec<&(String, Option<String>)> = observations
        .iter()
        .filter(|(p, w)| {
            !matches!(
                (p.as_str(), w.as_deref()),
                ("approved", None) | ("applied", Some("completed"))
            )
        })
        .collect();
    assert!(
        illegal.is_empty(),
        "ProjC 单条 JOIN 出现非法观察对: {illegal:?}（合法域同 pc：精确两元）"
    );
    let (last_p, last_w) = observations.last().unwrap();
    assert_eq!(
        (last_p.as_str(), last_w.as_deref()),
        ("applied", Some("completed"))
    );
}
