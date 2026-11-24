//! P3-D04-02 Step6 前置 · Q10-R1-FIX T1-T8e 分类矩阵测试（实施载体）。
//! 依据：V1.2 §五 + V1.2.1 R3/R4 + V1.2.2 §三 + 审批附件补充 B。
//!
//! 三层证明对象（附件 B.1，不混淆覆盖范围）：
//! - L1（T8-b/c/d/e）：协议同款事务 + 同线程语句边界 + pg_terminate_backend——
//!   目标 backend 在指定语句边界被终止后的**数据库侧行为**（语句报错 / 会话回滚 /
//!   advisory 释放）；不证明产品分支逻辑与 sqlx 池行为。
//! - L2（T8-a-1 / L2-F2 / L2-F3 / 语义映射）：直调 classify_*_after_publish_err
//!   （受控池：占位 / statement_timeout 注入）——F1-F5 → FailedTech{kind,error,
//!   cleanup_error} 正确转换 + Replay 唯一来源不变式（I-1）；不证明完整 close 行为。
//! - L3（T1-T7 / T8-a-2）：端到端 close_report——II-A 计数契约、报告 closed、
//!   逐提案容错；T8-a-2 不宣称 F1 精确归因（行为级）。
//!
//! 同步机制 = 可观测 barrier：pg_stat_activity 的 advisory 等待者计数（按
//! current_database 限界，多测试并行互不误伤）+ PostgreSQL advisory 队列 FIFO。
//! 全部并发断言依赖 advisory 互斥的因果序，禁 sleep 时序猜测。
//!
//! 【T7 实施期发现（CODE FACT，CAS 耦合墙）】close 列出的提案必须创建于 close
//! 之前，expected_version_no = 创建时项目 current 版本号 C。单 close 内
//! 「首发（applied）」与「竞争者完成 → 短路 replay」要求**两个真实发布**（各自
//! CAS 过闸：current == expected），而首个真实发布使 current=C+1，另一提案的
//! CAS（expected=C）必败——同代两真实发布互斥。故单 close 的 applied≥1 ∧
//! replayed≥1 仅可经「同一提案短路」（=T2 形态，applied=0）达成，字面 (1,1,1)
//! 不可满足。T7 以同测试两项目两 close 承载混合计数（(1,0,1) 首发+CAS /
//! (0,1,1) replay+CAS），三计数路径全覆盖且各 close 计数 II-A 正确；此发现随
//! 实施报告登记。

use std::time::Duration;

use sqlx::pool::PoolOptions;
use sqlx::{PgPool, Postgres};
use uuid::Uuid;

use pm_application::actor::AuditActor;
use pm_application::analysis_report::{
    classify_pc_after_publish_err, classify_projc_after_publish_err, close_report, ClassifyOutcome,
};
use pm_application::pc::{
    approve_proposal, create_proposal, publish_version, register_source, submit_proposal,
    upsert_object_draft, CreateProposalInput, DecisionInput, ProposalMemberPin,
    PublishVersionInput, SourceInput, UpsertObjectDraft,
};
use pm_application::projc::{
    approve_proposal as projc_approve, create_proposal as projc_create,
    publish_version as projc_publish, register_source as projc_source,
    submit_proposal as projc_submit, upsert_object_draft as projc_upsert,
    CreateProposalInput as ProjcCreate, DecisionInput as ProjcDecision,
    ProposalMemberPin as ProjcPin, PublishVersionInput as ProjcPub, SourceInput as ProjcSrc,
    UpsertObjectDraft as ProjcDraft,
};

const LEAD: Uuid = Uuid::from_u128(0x7172_0000_0000_0000_0000_0000_0000_0031);
/// T5 注入用：真人主体但无治理授权（ensure_formal_authorized → Forbidden，
/// 发生在 publish advisory 之前——Err 与竞争者发布的先后因此可确定性编排）。
const STRANGER: Uuid = Uuid::from_u128(0x7172_0000_0000_0000_0000_0000_0000_0032);

// ── 夹具 ────────────────────────────────────────────────────────────────────

async fn seed(pool: &PgPool, tag: &str) -> (Uuid, Uuid) {
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO core.tenant (provider, external_key, display_name) \
         VALUES ('system', $1, $1) RETURNING id",
    )
    .bind(format!("q10c-{tag}-{}", Uuid::new_v4().simple()))
    .fetch_one(pool)
    .await
    .unwrap();
    let customer: Uuid = sqlx::query_scalar(
        "INSERT INTO core.customer (name, tenant_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(format!("q10c-{tag}"))
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    let project: Uuid = sqlx::query_scalar(
        "INSERT INTO core.project (customer_id, code, name, tenant_id) \
         VALUES ($1, $2, $2, $3) RETURNING id",
    )
    .bind(customer)
    .bind(format!("q10c-{tag}"))
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    for pid in [LEAD] {
        sqlx::query(
            "INSERT INTO core.principal (id, kind, display_name, is_active) \
             VALUES ($1, 'human', 'q10c-lead', true) ON CONFLICT (id) DO NOTHING",
        )
        .bind(pid)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
             VALUES ($1, $2, $3, 'owner') ON CONFLICT DO NOTHING",
        )
        .bind(tenant)
        .bind(project)
        .bind(pid)
        .execute(pool)
        .await
        .unwrap();
    }
    (tenant, project)
}

/// T5 专用：STRANGER 真人主体（无 project_member 行 → ensure_governance_authorized
/// 403；close_report 本体不做成员门，仅 publish_version 的正式写门拒之）。
async fn seed_stranger(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES ($1, 'human', 'q10c-stranger', true) ON CONFLICT (id) DO NOTHING",
    )
    .bind(STRANGER)
    .execute(pool)
    .await
    .unwrap();
}

async fn register_pc_source(pool: &PgPool, project: Uuid) -> Uuid {
    register_source(
        pool,
        project,
        &AuditActor::principal(LEAD),
        SourceInput {
            source_kind: "manual_input".into(),
            target_domain: "pc".into(),
            target_type: "analysis".into(),
            target_id: Uuid::new_v4().to_string(),
            target_revision_id: None,
            revision_missing_reason: Some("q10c fixture".into()),
            section_anchor: None,
            start_line: None,
            locator: None,
            description: None,
            source_content_hash: None,
        },
    )
    .await
    .unwrap()
}

/// analyzed 报告（仅 pc 源；projc_source_id NULL = Legacy 跳过 ProjC 分支）。
async fn seed_analyzed_report(pool: &PgPool, project: Uuid, tenant: Uuid) -> (Uuid, Uuid) {
    let source_id = register_pc_source(pool, project).await;
    let repo_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.project_repository \
         (project_id, name, url, status, tenant_id) \
         VALUES ($1, 'q10c-repo', 'git@example.com:q10c.git', 'verified', $2) RETURNING id",
    )
    .bind(project)
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    let report_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.repo_analysis_report \
         (tenant_id, project_id, repository_id, commit_sha, intake_id, status, pc_source_id) \
         VALUES ($1, $2, $3, 'q10cabc1234', $4, 'analyzed', $5) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(repo_id)
    .bind(Uuid::new_v4())
    .bind(source_id)
    .fetch_one(pool)
    .await
    .unwrap();
    (report_id, source_id)
}

/// analyzed 报告（仅 projc 源；pc_source_id NULL = 跳过 pc 块）。
async fn seed_analyzed_report_projc(pool: &PgPool, project: Uuid, tenant: Uuid) -> (Uuid, Uuid) {
    let projc_source_id = projc_source(
        pool,
        project,
        &AuditActor::principal(LEAD),
        ProjcSrc {
            source_kind: "manual_input".into(),
            target_domain: "knowledge_base".into(),
            target_type: "analysis".into(),
            target_id: Uuid::new_v4().to_string(),
            target_revision_id: None,
            revision_missing_reason: Some("q10c fixture".into()),
            section_anchor: None,
            start_line: None,
            locator: None,
            description: None,
            source_content_hash: None,
        },
    )
    .await
    .unwrap();
    let repo_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.project_repository \
         (project_id, name, url, status, tenant_id) \
         VALUES ($1, 'q10c-repo-pj', 'git@example.com:q10cpj.git', 'verified', $2) RETURNING id",
    )
    .bind(project)
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    let report_id: Uuid = sqlx::query_scalar(
        "INSERT INTO integration.repo_analysis_report \
         (tenant_id, project_id, repository_id, commit_sha, intake_id, status, projc_source_id) \
         VALUES ($1, $2, $3, 'q10cabc5678', $4, 'analyzed', $5) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(repo_id)
    .bind(Uuid::new_v4())
    .bind(projc_source_id)
    .fetch_one(pool)
    .await
    .unwrap();
    (report_id, projc_source_id)
}

/// pc 批准态提案（挂报告 source）：draft object → create → submit → approve。
/// `expected` = 调用方声明的提案基线（须与创建时项目 current 版本号一致）。
/// 返回 (proposal_id, object_id)。
async fn approved_proposal(
    pool: &PgPool,
    project: Uuid,
    source: Uuid,
    key: &str,
    expected: i64,
) -> (Uuid, Uuid) {
    let obj = upsert_object_draft(
        pool,
        project,
        &AuditActor::principal(LEAD),
        UpsertObjectDraft {
            object_type: "feature".into(),
            object_key: Some(key.into()),
            display_name: key.into(),
            content_markdown: format!("# {key}"),
            change_summary: Some("q10c".into()),
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
            title: format!("q10c {key}"),
            rationale: "q10c".into(),
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
        Some(expected),
    )
    .await
    .unwrap();
    approve_proposal(
        pool,
        project,
        &AuditActor::principal(LEAD),
        drafted.proposal_id,
        DecisionInput {
            reason: Some("q10c".into()),
            expected_version_no: expected,
        },
    )
    .await
    .unwrap();
    (drafted.proposal_id, obj.object_id)
}

/// projc 批准态提案（挂 projc source）。
async fn projc_approved_proposal(pool: &PgPool, project: Uuid, source: Uuid, key: &str) -> Uuid {
    let obj = projc_upsert(
        pool,
        project,
        &AuditActor::principal(LEAD),
        ProjcDraft {
            object_type: "tech_stack".into(),
            object_key: Some(key.into()),
            display_name: key.into(),
            content_markdown: format!("# {key}"),
            change_summary: Some("q10c".into()),
            source_context: None,
            analysis_mode: None,
        },
    )
    .await
    .unwrap();
    let drafted = projc_create(
        pool,
        project,
        &AuditActor::principal(LEAD),
        ProjcCreate {
            title: format!("q10c {key}"),
            rationale: "q10c".into(),
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
        pool,
        project,
        &AuditActor::principal(LEAD),
        drafted.proposal_id,
        Some(0),
    )
    .await
    .unwrap();
    projc_approve(
        pool,
        project,
        &AuditActor::principal(LEAD),
        drafted.proposal_id,
        ProjcDecision {
            reason: Some("q10c".into()),
            expected_version_no: 0,
        },
    )
    .await
    .unwrap();
    drafted.proposal_id
}

/// 未解决冲突（L3 冻结圈定 object_id）。
async fn insert_unresolved_conflict(
    pool: &PgPool,
    tenant: Uuid,
    project: Uuid,
    object_id: Uuid,
) -> Uuid {
    let conflict_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.conflict (tenant_id, project_id, conflict_type, severity, \
              description, status, created_by) \
         VALUES ($1, $2, 'content_contradiction', 'major', 'q10c matrix fixture', \
                 'detected', $3) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(LEAD)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO pc.conflict_object (conflict_id, tenant_id, project_id, object_id) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(conflict_id)
    .bind(tenant)
    .bind(project)
    .bind(object_id)
    .execute(pool)
    .await
    .unwrap();
    conflict_id
}

// ── 可观测 barrier 与受控池 ─────────────────────────────────────────────────

/// 本测试库内等待 advisory 锁的后端数（pg_stat_activity；按 current_database 限界）。
async fn advisory_waiters(pool: &PgPool) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM pg_stat_activity \
          WHERE datname = current_database() \
            AND wait_event_type = 'Lock' AND wait_event = 'advisory'",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

/// barrier：等待 ≥at_least 个 advisory 等待者出现（有界轮询可观测状态；
/// 每轮一次 DB 往返即让出，不依赖 sleep 时序）。
async fn wait_advisory_waiters(pool: &PgPool, at_least: i64, label: &str) {
    for _ in 0..20_000 {
        if advisory_waiters(pool).await >= at_least {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("barrier timeout: {label}（advisory 等待者 < {at_least}）");
}

/// 测试侧持有 pc 发布 advisory（显式事务——pg_advisory_xact_lock 必须在事务内
/// 才能跨语句持有）。drop = 释放。
async fn hold_publish_advisory(
    pool: &PgPool,
    project: Uuid,
) -> sqlx::Transaction<'static, Postgres> {
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('pc_publish:' || $1::text)::bigint)")
        .bind(project.to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    tx
}

/// 有界探针：发布 advisory 当前是否无人持有（lock_timeout 1s 内获锁=free）。
async fn probe_advisory_free(pool: &PgPool, project: Uuid) -> bool {
    let mut probe = pool.begin().await.unwrap();
    sqlx::query("SET LOCAL lock_timeout = '1s'")
        .execute(&mut *probe)
        .await
        .unwrap();
    let r =
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext('pc_publish:' || $1::text)::bigint)")
            .bind(project.to_string())
            .execute(&mut *probe)
            .await;
    probe.rollback().await.ok();
    r.is_ok()
}

/// 受控池（同库）：max/max_connections、短 acquire_timeout。
async fn small_pool(pool: &PgPool, max: u32, acquire_ms: u64) -> PgPool {
    PoolOptions::<Postgres>::new()
        .max_connections(max)
        .acquire_timeout(Duration::from_millis(acquire_ms))
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap()
}

/// 会话级 statement_timeout 注入（SET 为会话作用域，连接归还池后仍生效；
/// 只能字面拼 ms——调用方传 u64 常量）。
async fn poison_statement_timeout(pool: &PgPool, ms: u64) {
    let mut c = pool.acquire().await.unwrap();
    sqlx::query(&format!("SET statement_timeout = '{ms}ms'"))
        .execute(&mut *c)
        .await
        .unwrap();
    drop(c);
}

async fn terminate_backend(pool: &PgPool, pid: i32) -> bool {
    let mut admin = pool.acquire().await.unwrap();
    let ok: bool = sqlx::query_scalar("SELECT pg_terminate_backend($1)")
        .bind(pid)
        .fetch_one(&mut *admin)
        .await
        .unwrap();
    ok
}

// ── 断言帮手 ────────────────────────────────────────────────────────────────

async fn report_status(pool: &PgPool, report: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM integration.repo_analysis_report WHERE id = $1")
        .bind(report)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn proposal_status(pool: &PgPool, proposal: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM pc.proposal WHERE id = $1")
        .bind(proposal)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn version_count(pool: &PgPool, project: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM pc.cognition_version WHERE project_id = $1")
        .bind(project)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn op_status(pool: &PgPool, proposal: Uuid) -> Option<String> {
    sqlx::query_scalar(
        "SELECT wo.status FROM pc.write_operation wo \
          WHERE wo.operation_id = 'pc-decision:' || $1::text",
    )
    .bind(proposal.to_string())
    .fetch_optional(pool)
    .await
    .unwrap()
}

fn assert_counts(
    outcome: &pm_application::analysis_report::CloseOutcome,
    applied: u64,
    replayed: u64,
    failed: u64,
) {
    assert_eq!(outcome.applied_proposals, applied, "applied: {outcome:?}");
    assert_eq!(
        outcome.replayed_proposals, replayed,
        "replayed: {outcome:?}"
    );
    assert_eq!(
        outcome.publish_failed_proposals, failed,
        "failed: {outcome:?}"
    );
}

fn expect_kind(outcome: ClassifyOutcome, kind: &str, cleanup_is_none: bool) {
    match outcome {
        ClassifyOutcome::FailedTech {
            kind: k,
            cleanup_error,
            ..
        } => {
            assert_eq!(k, kind, "kind 不符");
            assert_eq!(
                cleanup_error.is_none(),
                cleanup_is_none,
                "cleanup_error 形态不符"
            );
        }
        other => panic!("预期 FailedTech{{{kind}}}，实得 {other:?}"),
    }
}

// ── T1：正常发布（L3）──────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn t1_normal_publish_applied(pool: PgPool) {
    let (tenant, project) = seed(&pool, "t1").await;
    let (report, source) = seed_analyzed_report(&pool, project, tenant).await;
    let (proposal, _) = approved_proposal(&pool, project, source, "t1-p", 0).await;

    let outcome = close_report(&pool, project, report, &AuditActor::principal(LEAD))
        .await
        .unwrap();

    assert_counts(&outcome, 1, 0, 0);
    assert_eq!(outcome.withdrawn_proposals, 0);
    assert_eq!(report_status(&pool, report).await, "closed");
    assert_eq!(proposal_status(&pool, proposal).await, "applied");
    assert_eq!(
        op_status(&pool, proposal).await.as_deref(),
        Some("completed")
    );
    assert_eq!(version_count(&pool, project).await, 1);
}

// ── T2：已发布 replay·时序 1（L3；barrier=advisory 队列 FIFO）──────────────

#[sqlx::test(migrations = "../../migrations")]
async fn t2_competitor_publish_close_short_circuit_replayed(pool: PgPool) {
    let (tenant, project) = seed(&pool, "t2").await;
    let (report, source) = seed_analyzed_report(&pool, project, tenant).await;
    let (proposal, _) = approved_proposal(&pool, project, source, "t2-p", 0).await;

    // 测试持发布 advisory；竞争腿先入队（可观测 barrier ≥1）
    let holder = hold_publish_advisory(&pool, project).await;
    let p2 = pool.clone();
    let competitor = tokio::spawn(async move {
        publish_version(
            &p2,
            project,
            &AuditActor::principal(LEAD),
            PublishVersionInput {
                proposal_id: proposal,
            },
        )
        .await
    });
    wait_advisory_waiters(&pool, 1, "竞争腿入队").await;

    // close 入队（FIFO 于竞争者之后；此时 approved_ids 读到 approved 态）
    let p1 = pool.clone();
    let closer = tokio::spawn(async move {
        close_report(&p1, project, report, &AuditActor::principal(LEAD)).await
    });
    wait_advisory_waiters(&pool, 2, "close 的 publish 入队").await;

    drop(holder); // FIFO：竞争者先完成 → close 的 publish 命中 completed 短路
    let (close_r, comp_r) = tokio::join!(closer, competitor);
    let outcome = close_r.unwrap().unwrap();
    comp_r.unwrap().unwrap();

    // R4 语义：短路确认 → replayed（来源①），applied=0
    assert_counts(&outcome, 0, 1, 0);
    assert_eq!(report_status(&pool, report).await, "closed");
    assert_eq!(proposal_status(&pool, proposal).await, "applied");
    assert_eq!(
        op_status(&pool, proposal).await.as_deref(),
        Some("completed")
    );
    assert_eq!(version_count(&pool, project).await, 1, "同提案至多一版本");
}

// ── T3：CAS 提交型 Err（E8）→ 分类 (failed,failed) → Failed（L3）──────────

#[sqlx::test(migrations = "../../migrations")]
async fn t3_cas_commit_err_classifies_failed(pool: PgPool) {
    let (tenant, project) = seed(&pool, "t3").await;
    let (report, source) = seed_analyzed_report(&pool, project, tenant).await;
    // 版本载体（发布后 applied，不进 close 清单）+ 基线 0 的目标提案
    let (vehicle, _) = approved_proposal(&pool, project, source, "t3-veh", 0).await;
    let (target, _) = approved_proposal(&pool, project, source, "t3-cas", 0).await;
    publish_version(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        PublishVersionInput {
            proposal_id: vehicle,
        },
    )
    .await
    .unwrap();

    let outcome = close_report(&pool, project, report, &AuditActor::principal(LEAD))
        .await
        .unwrap();

    // 提交型 Err：CAS 同事务内置 (failed,failed) → 分类 Failed（业务）
    assert_counts(&outcome, 0, 0, 1);
    assert_eq!(report_status(&pool, report).await, "closed");
    assert_eq!(proposal_status(&pool, target).await, "failed");
    assert_eq!(op_status(&pool, target).await.as_deref(), Some("failed"));
    assert_eq!(
        version_count(&pool, project).await,
        1,
        "目标提案不得产生新版本"
    );
}

// ── T4：回滚型 Err（E9 L3 Forbidden）→ 分类 (approved,None) → Failed（L3）─

#[sqlx::test(migrations = "../../migrations")]
async fn t4_l3_forbidden_rollback_err_classifies_failed(pool: PgPool) {
    let (tenant, project) = seed(&pool, "t4").await;
    let (report, source) = seed_analyzed_report(&pool, project, tenant).await;
    let (target, object_id) = approved_proposal(&pool, project, source, "t4-l3", 0).await;
    insert_unresolved_conflict(&pool, tenant, project, object_id).await;

    let outcome = close_report(&pool, project, report, &AuditActor::principal(LEAD))
        .await
        .unwrap();

    // 回滚型 Err：提案态保持 approved → 分类时点未确认先行完成 → Failed
    assert_counts(&outcome, 0, 0, 1);
    assert_eq!(report_status(&pool, report).await, "closed");
    assert_eq!(
        proposal_status(&pool, target).await,
        "approved",
        "回滚型 Err 不得改提案态"
    );
    assert_eq!(
        op_status(&pool, target).await,
        None,
        "回滚型 Err 不得落账本行"
    );
    assert_eq!(version_count(&pool, project).await, 0);
}

// ── T5：Err 后竞争者成功发布·时序 2 端到端 → 分类 Replay（L3）──────────────
//
// 注入 = pre-advisory Forbidden（close 主体无治理授权——publish 的正式写门在
// 获取 advisory 之前拒绝，Err 与竞争者发布的先后因此可确定性编排）。
// （L3-Forbidden 变体在端到端下无法确定性产出 classify-Replay：其 Err 发生于
// advisory 临界区内，竞争者要么先于 close 的 completed 检查提交（→短路，非
// 分类路径）、要么在 FIFO 上落后于 classify（→Failed）；见实施报告。Err/分类
// 失败路径的 L3-Forbidden 端到端由 T4 覆盖。）

#[sqlx::test(migrations = "../../migrations")]
async fn t5_err_then_competitor_publish_classifies_replay(pool: PgPool) {
    let (tenant, project) = seed(&pool, "t5").await;
    seed_stranger(&pool).await;
    let (report, source) = seed_analyzed_report(&pool, project, tenant).await;
    let (target, _) = approved_proposal(&pool, project, source, "t5-p", 0).await;

    // 测试持 advisory；竞争腿（LEAD，有授权）先入队
    let holder = hold_publish_advisory(&pool, project).await;
    let p2 = pool.clone();
    let competitor = tokio::spawn(async move {
        publish_version(
            &p2,
            project,
            &AuditActor::principal(LEAD),
            PublishVersionInput {
                proposal_id: target,
            },
        )
        .await
    });
    wait_advisory_waiters(&pool, 1, "竞争腿入队").await;

    // close（STRANGER）：publish 在 advisory 之前即 Forbidden → 分类事务入队其后
    let p1 = pool.clone();
    let closer = tokio::spawn(async move {
        close_report(&p1, project, report, &AuditActor::principal(STRANGER)).await
    });
    wait_advisory_waiters(&pool, 2, "分类事务入队（竞争者之后）").await;

    drop(holder); // FIFO：竞争者先完成（applied+completed）→ 分类 JOIN 确认 → Replay
    let (close_r, comp_r) = tokio::join!(closer, competitor);
    let outcome = close_r.unwrap().unwrap();
    comp_r.unwrap().unwrap();

    // II-A：分类时点已确认先行发布完成（来源②）
    assert_counts(&outcome, 0, 1, 0);
    assert_eq!(report_status(&pool, report).await, "closed");
    assert_eq!(proposal_status(&pool, target).await, "applied");
    assert_eq!(op_status(&pool, target).await.as_deref(), Some("completed"));
    assert_eq!(version_count(&pool, project).await, 1);
}

// ── T6：分类持锁期间竞争者无法完成（协议级互斥实证）────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn t6_classify_advisory_mutual_exclusion_protocol(pool: PgPool) {
    let (tenant, project) = seed(&pool, "t6").await;
    let (report, source) = seed_analyzed_report(&pool, project, tenant).await;
    let (target, _) = approved_proposal(&pool, project, source, "t6-p", 0).await;
    // 预发布（applied+completed）→ 后续 publish 为幂等重放
    publish_version(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        PublishVersionInput {
            proposal_id: target,
        },
    )
    .await
    .unwrap();

    // 测试自建协议同款事务：BEGIN + P + 同款 JOIN，保持打开
    let mut holder = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('pc_publish:' || $1::text)::bigint)")
        .bind(project.to_string())
        .execute(&mut *holder)
        .await
        .unwrap();
    let _: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT p.status, wo.status FROM pc.proposal p \
         LEFT JOIN pc.write_operation wo \
           ON wo.tenant_id = p.tenant_id AND wo.project_id = p.project_id \
          AND wo.operation_id = 'pc-decision:' || p.id::text \
         WHERE p.id = $1",
    )
    .bind(target)
    .fetch_optional(&mut *holder)
    .await
    .unwrap();

    let p2 = pool.clone();
    let competitor = tokio::spawn(async move {
        publish_version(
            &p2,
            project,
            &AuditActor::principal(LEAD),
            PublishVersionInput {
                proposal_id: target,
            },
        )
        .await
    });
    wait_advisory_waiters(&pool, 1, "发布者被协议事务阻塞").await;

    // 持锁期间：发布者未完成（P 互斥实证），库态冻结
    assert!(!competitor.is_finished(), "持 advisory 期间发布者不得完成");
    assert_eq!(version_count(&pool, project).await, 1);

    drop(holder); // 释放 → 幂等重放完成
    let receipt = competitor.await.unwrap().unwrap();
    assert!(receipt.replayed, "释放后 publish=Ok(replayed)");
    assert_eq!(version_count(&pool, project).await, 1, "重放不新增版本");
    let _ = report; // 本用例不 close（协议级）
}

// ── T7：混合计数（两项目两 close 组合；见文件头 CAS 耦合墙说明）────────────

#[sqlx::test(migrations = "../../migrations")]
async fn t7_mixed_counts_two_closes(pool: PgPool) {
    // ── 项目 A：首发 + CAS（close 先行，无竞争者）→ (1,0,1) ──
    // 顺序约束（CAS 耦合墙的镜像）：cas_a 以基线 0 创建（其发布时 current=1 必败）；
    // first_a 在载体发布（v1）之后创建（expected=1，其发布时 current=1 过闸）。
    let (tenant_a, project_a) = seed(&pool, "t7a").await;
    let (report_a, source_a) = seed_analyzed_report(&pool, project_a, tenant_a).await;
    let (veh_a, _) = approved_proposal(&pool, project_a, source_a, "t7a-veh", 0).await;
    let (cas_a, _) = approved_proposal(&pool, project_a, source_a, "t7a-cas", 0).await;
    publish_version(
        &pool,
        project_a,
        &AuditActor::principal(LEAD),
        PublishVersionInput { proposal_id: veh_a },
    )
    .await
    .unwrap();
    let (first_a, _) = approved_proposal(&pool, project_a, source_a, "t7a-first", 1).await;
    let outcome_a = close_report(&pool, project_a, report_a, &AuditActor::principal(LEAD))
        .await
        .unwrap();
    assert_counts(&outcome_a, 1, 0, 1);
    assert_eq!(proposal_status(&pool, first_a).await, "applied");
    assert_eq!(proposal_status(&pool, cas_a).await, "failed");
    assert_eq!(version_count(&pool, project_a).await, 2);

    // ── 项目 B：竞争者完成 + 短路 replay + CAS（T2 机制）→ (0,1,1) ──
    let (tenant_b, project_b) = seed(&pool, "t7b").await;
    let (report_b, source_b) = seed_analyzed_report(&pool, project_b, tenant_b).await;
    let (first_b, _) = approved_proposal(&pool, project_b, source_b, "t7b-first", 0).await;
    let (replay_b, _) = approved_proposal(&pool, project_b, source_b, "t7b-replay", 0).await;

    let holder = hold_publish_advisory(&pool, project_b).await;
    let p2 = pool.clone();
    let competitor = tokio::spawn(async move {
        publish_version(
            &p2,
            project_b,
            &AuditActor::principal(LEAD),
            PublishVersionInput {
                proposal_id: replay_b,
            },
        )
        .await
    });
    wait_advisory_waiters(&pool, 1, "T7b 竞争腿入队").await;
    let p1 = pool.clone();
    let closer = tokio::spawn(async move {
        close_report(&p1, project_b, report_b, &AuditActor::principal(LEAD)).await
    });
    wait_advisory_waiters(&pool, 2, "T7b close 入队").await;
    drop(holder);
    let (close_r, comp_r) = tokio::join!(closer, competitor);
    let outcome_b = close_r.unwrap().unwrap();
    comp_r.unwrap().unwrap();
    // 竞争者完成 replay_b（current 0→1）；close 对 first_b 的真实发布 CAS 败
    // （耦合墙）→ failed；对 replay_b 短路 → replayed
    assert_counts(&outcome_b, 0, 1, 1);
    assert_eq!(proposal_status(&pool, replay_b).await, "applied");
    assert_eq!(proposal_status(&pool, first_b).await, "failed");
    assert_eq!(
        version_count(&pool, project_b).await,
        1,
        "项目 B 无载体：replay_b 发布即 v1"
    );

    // 两 close 合计 = (1,1,2)：三计数路径全覆盖；字面 (1,1,1) 见文件头不可满足证明
}

// ── T8-a-1：L2 直调——F1 pool.begin 失败（受控池占位）──────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn t8a1_l2_begin_failure_direct_call(pool: PgPool) {
    let (tenant, project) = seed(&pool, "t8a1").await;
    let small = small_pool(&pool, 1, 300).await;
    let guard = small.acquire().await.unwrap(); // 占据唯一连接

    let outcome = classify_pc_after_publish_err(&small, project, Uuid::new_v4()).await;
    expect_kind(outcome, "begin", true); // F1：无事务、未获任何锁

    drop(guard);
    small.close().await;
    let _ = tenant;
}

// ── T8-a-2：L3 端到端——小池耗尽被逐提案容错吞没（行为级，不宣称精确 F1）────

#[sqlx::test(migrations = "../../migrations")]
async fn t8a2_l3_small_pool_exhaustion_fault_tolerated(pool: PgPool) {
    let (tenant, project) = seed(&pool, "t8a2").await;
    let (report, source) = seed_analyzed_report(&pool, project, tenant).await;
    let (target, _) = approved_proposal(&pool, project, source, "t8a2-p", 0).await;

    let small = small_pool(&pool, 1, 300).await;
    let outcome = close_report(&small, project, report, &AuditActor::principal(LEAD))
        .await
        .unwrap();

    // publish（池饿死）与分类（begin 饿死 → FailedTech）均容错；报告照常关闭
    assert_counts(&outcome, 0, 0, 1);
    assert_eq!(report_status(&pool, report).await, "closed");
    assert_eq!(
        proposal_status(&pool, target).await,
        "approved",
        "技术故障不改提案态"
    );
    small.close().await;
}

// ── T8-b/c/d：L1 数据库侧——backend 在语句边界被终止（协议同款事务）─────────

#[sqlx::test(migrations = "../../migrations")]
async fn t8bcd_l1_terminate_at_statement_boundaries(pool: PgPool) {
    let (tenant, project) = seed(&pool, "t8bcd").await;
    let advisory_sql = "SELECT pg_advisory_xact_lock(hashtext('pc_publish:' || $1::text)::bigint)";
    let join_sql = "SELECT p.status, wo.status FROM pc.proposal p \
         LEFT JOIN pc.write_operation wo \
           ON wo.tenant_id = p.tenant_id AND wo.project_id = p.project_id \
          AND wo.operation_id = 'pc-decision:' || p.id::text \
         WHERE p.id = $1";

    // F2 边界：BEGIN 成功后、advisory 语句前终止 → advisory 语句报错
    let mut tx = pool.begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert!(terminate_backend(&pool, pid).await);
    let adv = sqlx::query(advisory_sql)
        .bind(project.to_string())
        .execute(&mut *tx)
        .await;
    assert!(adv.is_err(), "F2 边界：终止后 advisory 语句必须报错");
    drop(tx);
    let gone: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE pid = $1")
        .bind(pid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        gone, 0,
        "被终止 backend 须自 pg_stat_activity 消失（会话回滚）"
    );

    // F3 边界：advisory 已获 → 终止 → JOIN 报错 + 显式 rollback 亦报错（F5 的
    // 数据库形态）——且 advisory 由服务端会话中断释放（正确性兜底事实）
    let mut tx = pool.begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    sqlx::query(advisory_sql)
        .bind(project.to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    assert!(terminate_backend(&pool, pid).await);
    let join = sqlx::query_as::<_, (String, Option<String>)>(join_sql)
        .bind(Uuid::new_v4())
        .fetch_optional(&mut *tx)
        .await;
    assert!(join.is_err(), "F3 边界：终止后 JOIN 必须报错");
    let rb = tx.rollback().await;
    assert!(rb.is_err(), "F5 数据库形态：死连接上显式回滚亦须报错");
    assert!(
        probe_advisory_free(&pool, project).await,
        "advisory 须已被服务端释放"
    );

    // F4 边界：JOIN 成功后、COMMIT 前终止 → COMMIT 报错（服务端已回滚）
    let mut tx = pool.begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    sqlx::query(advisory_sql)
        .bind(project.to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    let row: Option<(String, Option<String>)> = sqlx::query_as(join_sql)
        .bind(Uuid::new_v4())
        .fetch_optional(&mut *tx)
        .await
        .unwrap();
    assert_eq!(row, None);
    assert!(terminate_backend(&pool, pid).await);
    let commit = tx.commit().await;
    assert!(commit.is_err(), "F4 边界：终止后 COMMIT 必须报错");
    assert!(
        probe_advisory_free(&pool, project).await,
        "COMMIT 失败后 advisory 须释放"
    );
    let _ = tenant;
}

// ── T8-e：经实际 Pool 归还路径的坏连接剔除实测（附件 B.2 五步）─────────────

#[sqlx::test(migrations = "../../migrations")]
async fn t8e_l1_pool_recovery_five_steps(pool: PgPool) {
    // 步骤 1：从被测 Pool 获取连接 C，记录 PID
    let mut c = pool.acquire().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *c)
        .await
        .unwrap();
    // 步骤 2：经独立（管理）连接终止该 backend
    assert!(terminate_backend(&pool, pid).await);
    // 步骤 3：在 C 上走真实事务结束路径——语句失败证明连接已死，drop 归还坏句柄
    let dead: Result<i32, sqlx::Error> = sqlx::query_scalar("SELECT 1").fetch_one(&mut *c).await;
    assert!(dead.is_err(), "被终止连接上的语句必须报错");
    drop(c);
    // 步骤 4：再次从同一 Pool 获取（循环重取，不得命中已终止句柄）
    // 步骤 5：断言查询成功且 PID ≠ 已终止 PID——Pool 归还路径确实剔除坏连接
    for _ in 0..10 {
        let mut conn = pool.acquire().await.unwrap();
        let fresh: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *conn)
            .await
            .unwrap();
        assert_ne!(fresh, pid, "Pool 不得再交付已终止 backend 的连接");
        drop(conn);
    }
}

// ── T8 L2：F2/F3 错误映射（statement_timeout 确定性注入）+ I-3 释放证明 ────

#[sqlx::test(migrations = "../../migrations")]
async fn t8_l2_advisory_and_join_timeout_mapping(pool: PgPool) {
    let (tenant, project) = seed(&pool, "t8l2").await;
    let (report, source) = seed_analyzed_report(&pool, project, tenant).await;
    let (target, _) = approved_proposal(&pool, project, source, "t8l2-p", 0).await;

    // F2：分类 begin 成功 → advisory 获取被测试侧持锁阻塞 → statement_timeout
    // 令 advisory 语句失败 → FailedTech{kind:"advisory"}（drop 清理路径）
    let small = small_pool(&pool, 1, 5_000).await;
    poison_statement_timeout(&small, 250).await;
    let holder = hold_publish_advisory(&pool, project).await;
    let outcome = classify_pc_after_publish_err(&small, project, target).await;
    expect_kind(outcome, "advisory", true);
    drop(holder);
    assert!(
        probe_advisory_free(&pool, project).await,
        "holder 释放后 advisory 须空闲"
    );
    small.close().await;

    // F3：advisory 获取成功 → JOIN 被 ACCESS EXCLUSIVE 表锁阻塞 →
    // statement_timeout 失败 → 显式 rollback 成功 → FailedTech{kind:"join",
    // cleanup_error:None}（单错误形态）；I-3：分类持有的 advisory 随显式回滚释放
    let small2 = small_pool(&pool, 1, 5_000).await;
    poison_statement_timeout(&small2, 300).await;
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE pc.proposal IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let outcome = classify_pc_after_publish_err(&small2, project, target).await;
    expect_kind(outcome, "join", true);
    drop(blocker);
    assert!(
        probe_advisory_free(&pool, project).await,
        "I-3：F3 显式回滚必须释放 advisory"
    );
    small2.close().await;
    let _ = report;
}

// ── L2：分类语义映射（pc/projc 两域同构）+ Replay 唯一来源不变式 ───────────

#[sqlx::test(migrations = "../../migrations")]
async fn l2_classify_semantic_mapping_pc_projc(pool: PgPool) {
    let (tenant, project) = seed(&pool, "l2map").await;
    let (_report, source) = seed_analyzed_report(&pool, project, tenant).await;
    let (target, _) = approved_proposal(&pool, project, source, "l2map-p", 0).await;

    // 未发布：(approved, 无行) → 业务失败
    let o1 = classify_pc_after_publish_err(&pool, project, target).await;
    assert!(matches!(o1, ClassifyOutcome::Failed), "{o1:?}");
    // 发布后：持锁 JOIN 确认 applied∧completed → Replay（I-1 唯一来源）
    publish_version(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        PublishVersionInput {
            proposal_id: target,
        },
    )
    .await
    .unwrap();
    let o2 = classify_pc_after_publish_err(&pool, project, target).await;
    assert!(matches!(o2, ClassifyOutcome::Replay), "{o2:?}");

    // projc 同构
    let (_report_pj, projc_src) = seed_analyzed_report_projc(&pool, project, tenant).await;
    let target_pj = projc_approved_proposal(&pool, project, projc_src, "l2map-pj").await;
    let o3 = classify_projc_after_publish_err(&pool, project, target_pj).await;
    assert!(matches!(o3, ClassifyOutcome::Failed), "{o3:?}");
    projc_publish(
        &pool,
        project,
        &AuditActor::principal(LEAD),
        ProjcPub {
            proposal_id: target_pj,
        },
    )
    .await
    .unwrap();
    let o4 = classify_projc_after_publish_err(&pool, project, target_pj).await;
    assert!(matches!(o4, ClassifyOutcome::Replay), "{o4:?}");
}

// ── T1-projc：projc 域 close 首发（Ok 分支 R4 拆分·projc 侧）───────────────

#[sqlx::test(migrations = "../../migrations")]
async fn t1_projc_close_applied(pool: PgPool) {
    let (tenant, project) = seed(&pool, "t1pj").await;
    let (report, projc_src) = seed_analyzed_report_projc(&pool, project, tenant).await;
    let target = projc_approved_proposal(&pool, project, projc_src, "t1pj-p").await;

    let outcome = close_report(&pool, project, report, &AuditActor::principal(LEAD))
        .await
        .unwrap();

    assert_eq!(outcome.applied_proposals, 0);
    assert_eq!(outcome.replayed_proposals, 0);
    assert_eq!(outcome.publish_failed_proposals, 0);
    assert_eq!(outcome.projc_applied, 1);
    assert_eq!(outcome.projc_replayed, 0);
    assert_eq!(outcome.projc_publish_failed_proposals, 0);
    assert_eq!(report_status(&pool, report).await, "closed");
    let status: String = sqlx::query_scalar("SELECT status FROM projc.proposal WHERE id = $1")
        .bind(target)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "applied");
}
