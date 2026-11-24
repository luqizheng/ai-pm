//! DK-D 验收 · 写回通道 hook（设计稿 §8.3 B 组 / E 组；§10 DK-D 行）。
//!
//! 夹具 = 真库（`#[sqlx::test]` 迁移全量）+ `FakeWriteBackModel`（不打外部
//! provider）。断言直接读库，不信任 hook 回包——回包只是"它以为做了什么"，
//! 库才是"实际做了什么"。
//!
//! 覆盖：
//! - B1 AI 主体 confirm → Forbidden 且 `requirement_confirmation` 零新增；
//! - B2 起草终止于 `pending_confirmation`，approved 只能由 Approver 真人产生；
//! - B3 澄清（finding CONFIRMED）不产生需求、也不推导需求状态；
//! - B4 AI 主体 link_finding / materialize_basis → Forbidden；
//! - B5 未定稿需求下调 decompose → 拒绝且文案指向需求页；
//! - E1 `task_requirement.revision_id` 指向 approved 修订（DB 断言）；
//! - E2 AI 生成的任务全 `draft`；AI 既不能批量置 ready 也不能直写 ready；
//! - E3 前置→主依赖落在 `task_dependency`（不是标题字符串）；
//! - E4 无验收口径 → 整批拒绝（零任务落库）；
//! - §7.3 夹具：超上限截断、角色越界丢弃、开关 off 零写入。

use std::sync::Arc;

use async_trait::async_trait;
use pm_ai::discovery_writeback::{
    BatchTaskKind, DecomposeOutcome, DiscoveryWriteBack, DraftOutcomeKind,
    EmptyCapabilityProjection, ModelCallFailure, ModelReply, ProviderStamp, RawTask, RawTaskBatch,
    RequirementDraft, RequirementDraftContext, TaskDecomposeContext, WriteBackError, WriteBackHook,
    WriteBackMode, WriteBackModel, AI_FORBIDDEN_COMMANDS, AI_WRITABLE_COMMANDS, MAX_MAIN_TASKS,
    MAX_PREREQUISITE_TASKS,
};
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::discovery;
use pm_application::knowledge::KnowledgeRoot;
use pm_application::pm_write::PmWriteError;
use pm_application::requirement::{self, ConfirmRevisionRequest};
use pm_application::requirement_traceability::{self, MaterializeBasisRequest};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

// ─────────────────────────────────────────────────────────────── 夹具

struct Fx {
    project_id: Uuid,
    ai_principal: Uuid,
    human_id: Uuid,
    session_id: Uuid,
    provider_id: Uuid,
    root: KnowledgeRoot,
}

/// 真库夹具：项目（bootstrap 自带 project_ai_config/角色）、访谈会话、
/// 记账用的 provider 行、临时知识根（submit 绑定点写 Git）。
async fn fixture(pool: &PgPool, code: &str) -> Fx {
    let boot = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: format!("dkd-{code}"),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let session = discovery::create_session(
        pool,
        SYSTEM_TENANT,
        boot.project_id,
        "WEB",
        "TASK_DISCOVERY",
        boot.human_id,
    )
    .await
    .unwrap();
    let provider_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ai.team_llm_provider \
         (tenant_id, name, provider_type, endpoint, created_by) \
         VALUES ($1, $2, 'openai_compatible', 'http://127.0.0.1:1', $3) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .bind(format!("dkd-provider-{code}"))
    .bind(boot.human_id)
    .fetch_one(pool)
    .await
    .unwrap();
    Fx {
        project_id: boot.project_id,
        ai_principal: boot.ai_principal_id,
        human_id: boot.human_id,
        session_id: session.id,
        provider_id,
        root: KnowledgeRoot::new(
            std::env::temp_dir().join(format!("ai-pm-dkd-{}", Uuid::new_v4())),
        ),
    }
}

async fn session_review_required(pool: &PgPool, fx: &Fx) {
    discovery::transition_session(pool, SYSTEM_TENANT, fx.session_id, "REVIEW_REQUIRED")
        .await
        .unwrap();
}

/// 直插 finding（verification_status 可控——CONFIRMED 只能由人工决定产生，
/// 测试直接造终态，不走 `record_finding_verification` 的审计长链）。
async fn seed_finding(
    pool: &PgPool,
    fx: &Fx,
    kind: &str,
    content: &str,
    verification_status: &str,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO discovery.discovery_finding \
         (tenant_id, session_id, project_id, kind, content, verification_status) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(SYSTEM_TENANT)
    .bind(fx.session_id)
    .bind(fx.project_id)
    .bind(kind)
    .bind(content)
    .bind(verification_status)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn stamp(fx: &Fx) -> ProviderStamp {
    ProviderStamp {
        provider_id: fx.provider_id,
        model: "fake-dkd".to_string(),
    }
}

fn draft_reply() -> RequirementDraft {
    RequirementDraft {
        code: Some("fr phone-login".to_string()),
        title: "手机号登录".to_string(),
        summary: Some("支持手机号+验证码登录".to_string()),
        markdown:
            "# 手机号登录\n\n用户可用手机号+短信验证码登录。\n\n## 验收\n\n- 正确验证码可登录\n"
                .to_string(),
    }
}

fn sample_batch() -> RawTaskBatch {
    RawTaskBatch {
        prerequisite: vec![RawTask {
            key: "sms".to_string(),
            title: "接入短信服务商并配置登录模板".to_string(),
            description: Some("选型并配置短信通道".to_string()),
            goal: Some("短信通道可用".to_string()),
            deliverables: vec!["短信通道配置".to_string()],
            acceptance_criteria: vec!["短信通道连通性自测通过".to_string()],
            depends_on: vec![],
            role_ref: Some("developer".to_string()),
        }],
        main: vec![
            RawTask {
                key: "login".to_string(),
                title: "实现手机号验证码登录".to_string(),
                description: Some("含发送/校验/过期".to_string()),
                goal: Some("验证码登录可用".to_string()),
                deliverables: vec!["登录模块".to_string()],
                acceptance_criteria: vec!["正确验证码可登录".to_string()],
                depends_on: vec!["sms".to_string()],
                role_ref: Some("nobody".to_string()),
            },
            RawTask {
                key: "guard".to_string(),
                title: "补登录失败风控".to_string(),
                description: None,
                goal: None,
                deliverables: vec![],
                acceptance_criteria: vec!["连续失败 5 次锁定 10 分钟".to_string()],
                depends_on: vec!["login".to_string()],
                role_ref: None,
            },
        ],
    }
}

struct FakeModel {
    draft: RequirementDraft,
    batch: RawTaskBatch,
    stamp: ProviderStamp,
}

#[async_trait]
impl WriteBackModel for FakeModel {
    async fn draft_requirement(
        &self,
        _ctx: &RequirementDraftContext,
    ) -> Result<ModelReply<RequirementDraft>, ModelCallFailure> {
        Ok(ModelReply {
            value: self.draft.clone(),
            provider: self.stamp.clone(),
            latency_ms: 7,
        })
    }

    async fn decompose_tasks(
        &self,
        _ctx: &TaskDecomposeContext,
    ) -> Result<ModelReply<RawTaskBatch>, ModelCallFailure> {
        Ok(ModelReply {
            value: self.batch.clone(),
            provider: self.stamp.clone(),
            latency_ms: 11,
        })
    }
}

fn hook(
    pool: &PgPool,
    fx: &Fx,
    draft: RequirementDraft,
    batch: RawTaskBatch,
) -> DiscoveryWriteBack {
    DiscoveryWriteBack {
        pool: pool.clone(),
        model: Arc::new(FakeModel {
            draft,
            batch,
            stamp: stamp(fx),
        }),
        capability: Arc::new(EmptyCapabilityProjection),
        knowledge: fx.root.clone(),
        mode: WriteBackMode::On,
    }
}

async fn requirement_count(pool: &PgPool, project_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM delivery.requirement WHERE project_id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn view_session(pool: &PgPool, fx: &Fx) -> pm_application::discovery::DiscoverySessionView {
    discovery::get_session(pool, SYSTEM_TENANT, fx.session_id)
        .await
        .unwrap()
}

// ─────────────────────────────────────────────────────────────── B2 / E1~E3

/// 主链：起草（停在 pending）→ 人类确认 → 分解出 draft 任务批。
///
/// 一条用例串起 B2 / E1 / E2 / E3 四个判据——它们描述的是同一条时序上的
/// 四个不同断言点（拆开会让夹具重复四遍而收益为零）。
#[sqlx::test(migrations = "../../migrations")]
async fn draft_stops_at_pending_then_decompose_pins_approved_revision(pool: PgPool) {
    let fx = fixture(&pool, "DKD-MAIN").await;
    let confirmed = seed_finding(&pool, &fx, "user_confirmed", "需要手机号登录", "CONFIRMED").await;
    session_review_required(&pool, &fx).await;
    let session = view_session(&pool, &fx).await;

    // ① 起草：AI 主体经九命令链，止步 pending_confirmation（B2）。
    let write_back = hook(&pool, &fx, draft_reply(), sample_batch());
    let draft = write_back
        .on_review_required(SYSTEM_TENANT, &session)
        .await
        .unwrap();
    assert_eq!(draft.kind, DraftOutcomeKind::Submitted);
    let requirement_id = draft.requirement_id.unwrap();
    let revision_id = draft.revision_id.unwrap();
    assert_eq!(
        draft.revision_state.as_deref(),
        Some("pending_confirmation")
    );
    assert_eq!(draft.basis_finding_ids, vec![confirmed]);

    // basis 落库 + 视图透出 + 修订正文来自模型（§3.3 提案级标记）。
    let detail = requirement::requirement_detail(&pool, SYSTEM_TENANT, requirement_id)
        .await
        .unwrap();
    let revision = detail
        .revisions
        .iter()
        .find(|r| r.id == revision_id)
        .expect("revision exists");
    assert_eq!(revision.state, "pending_confirmation");
    assert_eq!(
        revision.basis_finding_ids,
        Some(serde_json::json!([confirmed])),
        "basis_finding_ids must ride inside content"
    );
    assert!(revision.markdown.as_deref().unwrap().contains("手机号登录"));

    // ② 记账（D-DK-11）：purpose 独立，不复用 conversation_reply。
    let (purpose, status): (Option<String>, String) = sqlx::query_as(
        "SELECT purpose, status FROM ai.llm_call_record \
          WHERE project_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(fx.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(purpose.as_deref(), Some("discovery_orchestration"));
    assert_eq!(status, "ok");

    // ③ 幂等：同一 session 再跑一轮起草 → AlreadyDrafted，不建第二条需求。
    let again = write_back
        .on_review_required(SYSTEM_TENANT, &session)
        .await
        .unwrap();
    assert_eq!(again.kind, DraftOutcomeKind::AlreadyDrafted);
    assert_eq!(again.requirement_id, Some(requirement_id));
    assert_eq!(requirement_count(&pool, fx.project_id).await, 1);

    // ④ B5 前置门：未定稿就分解 → 拒绝，文案指向需求页。
    let blocked = write_back
        .decompose_if_approved(SYSTEM_TENANT, &session)
        .await
        .unwrap_err();
    match blocked {
        WriteBackError::RequirementNotApproved { requirement_id: id } => {
            assert_eq!(id, requirement_id)
        }
        other => panic!("expected RequirementNotApproved, got {other:?}"),
    }
    assert!(
        blocked.to_string().contains("请先在需求页确认"),
        "B5 message must point at the requirement page: {blocked}"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM delivery.task WHERE project_id = $1")
            .bind(fx.project_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0,
        "a rejected decomposition must not create tasks"
    );

    // ⑤ 人类 Approver 确认（唯一可达 approved 的路径）。
    let content_hash = revision.content_hash.clone();
    requirement::confirm_revision(
        &pool,
        SYSTEM_TENANT,
        requirement_id,
        revision_id,
        ConfirmRevisionRequest {
            expected_content_hash: content_hash,
            expected_current_revision_id: None,
            expected_row_revision: None,
        },
        &pm_application::actor::AuditActor::principal(fx.human_id),
    )
    .await
    .unwrap();

    // ⑥ 分解：三源 + 四道系统校验 → 全部 draft（E2）。
    let outcome = write_back
        .decompose_if_approved(SYSTEM_TENANT, &session)
        .await
        .unwrap();
    let DecomposeOutcome::Generated(batch) = outcome else {
        panic!("expected a generated batch, got {outcome:?}");
    };
    assert_eq!(batch.requirement_id, requirement_id);
    assert_eq!(batch.revision_id, revision_id);
    assert_eq!(batch.prerequisite_count, 1);
    assert_eq!(batch.main_count, 2);
    assert_eq!(batch.truncated, 0);
    assert_eq!(
        batch.dropped_role_suggestions, 1,
        "unknown role_ref dropped"
    );
    assert_eq!(batch.tasks[0].kind, BatchTaskKind::Prerequisite);
    assert!(
        batch.tasks[0].ai_role_version_id.is_some(),
        "developer role matched"
    );
    assert!(
        batch.tasks[1].ai_role_version_id.is_none(),
        "越界角色置空而非编造"
    );

    // B05-01：S04 就绪检查 A 层要求 goal/deliverables 非空——创建即落。
    // 任务①模型给了 goal/deliverables；任务③（guard）模型未给 → 回退
    // description（None→title）/验收口径，不允许落库后 A 层全 ⚠。
    let contract_rows: Vec<(Option<String>, serde_json::Value)> = sqlx::query_as(
        "SELECT goal, deliverables FROM delivery.task WHERE project_id = $1          ORDER BY created_at, id",
    )
    .bind(fx.project_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(contract_rows.len(), 3);
    for (goal, deliverables) in &contract_rows {
        let goal = goal.as_deref().map(str::trim).unwrap_or("");
        assert!(
            !goal.is_empty(),
            "B05-01: generated task must carry a non-empty goal"
        );
        assert!(
            deliverables.as_array().map(Vec::len).unwrap_or(0) > 0,
            "B05-01: generated task must carry non-empty deliverables"
        );
    }
    // 模型给了 goal/deliverables 的任务按原样落库（不覆盖为回退值）。
    assert_eq!(contract_rows[0].0.as_deref(), Some("短信通道可用"));

    // E1：task_requirement 指向 approved 修订（DB 断言，不看回包）。
    let pins: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT tr.revision_id, rv.state \
           FROM delivery.task_requirement tr \
           JOIN delivery.requirement_revision rv ON rv.id = tr.revision_id \
          WHERE tr.project_id = $1",
    )
    .bind(fx.project_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(pins.len(), 3);
    assert!(pins
        .iter()
        .all(|(id, state)| *id == revision_id && state == "approved"));

    // E2：AI 生成的任务全部 draft。
    let statuses: Vec<String> =
        sqlx::query_scalar("SELECT status FROM delivery.task WHERE project_id = $1")
            .bind(fx.project_id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(statuses.len(), 3);
    assert!(statuses.iter().all(|s| s == "draft"), "AI never sets ready");

    // E3：依赖落在边表，且方向是「主任务 → 前置任务」。
    let edges: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT task_id, depends_on_task_id FROM delivery.task_dependency WHERE project_id = $1",
    )
    .bind(fx.project_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(edges.len(), 2);
    let login = batch.tasks[1].task_id;
    let guard = batch.tasks[2].task_id;
    let sms = batch.tasks[0].task_id;
    assert!(edges.contains(&(login, sms)));
    assert!(edges.contains(&(guard, login)));

    // 重放：approved 修订已挂 task_requirement → 离集，不再重复生成。
    let replay = write_back
        .decompose_if_approved(SYSTEM_TENANT, &session)
        .await
        .unwrap();
    assert!(matches!(replay, DecomposeOutcome::NothingToDo { .. }));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM delivery.task WHERE project_id = $1")
            .bind(fx.project_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        3
    );
}

// ─────────────────────────────────────────────────────────────── B1 / B4

/// B1/B4：AI 主体在**服务层**被拒（这是四道闸里的第 1 道，白名单是第 4 道）。
#[sqlx::test(migrations = "../../migrations")]
async fn ai_principal_cannot_confirm_link_or_materialize(pool: PgPool) {
    let fx = fixture(&pool, "DKD-FORBID").await;
    let finding = seed_finding(&pool, &fx, "user_confirmed", "需要手机号登录", "CONFIRMED").await;
    session_review_required(&pool, &fx).await;
    let session = view_session(&pool, &fx).await;

    let write_back = hook(&pool, &fx, draft_reply(), sample_batch());
    let draft = write_back
        .on_review_required(SYSTEM_TENANT, &session)
        .await
        .unwrap();
    let requirement_id = draft.requirement_id.unwrap();
    let revision_id = draft.revision_id.unwrap();
    let ai_actor = pm_application::actor::AuditActor::principal(fx.ai_principal);

    // B1：AI 确认 → Forbidden，且 requirement_confirmation 零新增。
    let hash: String =
        sqlx::query_scalar("SELECT content_hash FROM delivery.requirement_revision WHERE id = $1")
            .bind(revision_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let error = requirement::confirm_revision(
        &pool,
        SYSTEM_TENANT,
        requirement_id,
        revision_id,
        ConfirmRevisionRequest {
            expected_content_hash: hash,
            expected_current_revision_id: None,
            expected_row_revision: None,
        },
        &ai_actor,
    )
    .await
    .unwrap_err();
    assert!(matches!(error, PmWriteError::Forbidden(_)), "got {error:?}");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM delivery.requirement_confirmation WHERE requirement_id = $1"
        )
        .bind(requirement_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );

    // B1 续：AI 的 lifecycle 治理同样 Forbidden。
    let error = requirement::transition_lifecycle(
        &pool,
        SYSTEM_TENANT,
        requirement_id,
        requirement::LifecycleAction::Withdraw,
        &ai_actor,
    )
    .await
    .unwrap_err();
    assert!(matches!(error, PmWriteError::Forbidden(_)), "got {error:?}");

    // B4：AI 关联 finding / 一键物化 → Forbidden（S02D-20 不动）。
    let error = requirement_traceability::link_finding(
        &pool,
        SYSTEM_TENANT,
        requirement_id,
        finding,
        &ai_actor,
    )
    .await
    .unwrap_err();
    assert!(matches!(error, PmWriteError::Forbidden(_)), "got {error:?}");

    let error = requirement_traceability::materialize_basis(
        &pool,
        SYSTEM_TENANT,
        requirement_id,
        MaterializeBasisRequest {
            finding_ids: None,
            register_source: None,
            source_title: None,
        },
        &ai_actor,
    )
    .await
    .unwrap_err();
    assert!(matches!(error, PmWriteError::Forbidden(_)), "got {error:?}");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM delivery.requirement_finding WHERE requirement_id = $1"
        )
        .bind(requirement_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0,
        "no formal traceability row may appear from an AI call"
    );
}

// ─────────────────────────────────────────────────────────────── B3

/// B3：澄清（finding CONFIRMED）不产生需求，也不产生需求状态变化。
#[sqlx::test(migrations = "../../migrations")]
async fn confirmed_finding_alone_creates_no_requirement(pool: PgPool) {
    let fx = fixture(&pool, "DKD-B3").await;
    seed_finding(&pool, &fx, "user_confirmed", "需要手机号登录", "CONFIRMED").await;
    // 也走一次真裁决入口，证明「CONFIRMED 只是访谈事实」这条轴独立。
    let finding = seed_finding(&pool, &fx, "observed", "验证码 60s 有效", "PENDING").await;
    discovery::record_finding_verification(
        &pool,
        SYSTEM_TENANT,
        fx.session_id,
        finding,
        "CONFIRMED",
        "对，60 秒有效",
        fx.human_id,
        None,
        Some("op-dkd-b3"),
    )
    .await
    .unwrap();

    assert_eq!(
        requirement_count(&pool, fx.project_id).await,
        0,
        "澄清不等于确认：finding 全 CONFIRMED 也不该凭空出现需求"
    );

    // 会话未到 REVIEW_REQUIRED 时 hook 也不写（状态前置）。
    let session = view_session(&pool, &fx).await;
    let outcome = hook(&pool, &fx, draft_reply(), sample_batch())
        .on_review_required(SYSTEM_TENANT, &session)
        .await
        .unwrap();
    assert_eq!(outcome.kind, DraftOutcomeKind::Skipped);
    assert_eq!(outcome.reason, Some("session_not_review_required"));
    assert_eq!(requirement_count(&pool, fx.project_id).await, 0);
}

// ─────────────────────────────────────────────────────────────── E4 / §7.3

/// E4：任一条任务缺验收口径 → 整批拒绝，零任务落库（不留半批）。
#[sqlx::test(migrations = "../../migrations")]
async fn task_without_acceptance_rejects_the_whole_batch(pool: PgPool) {
    let fx = fixture(&pool, "DKD-E4").await;
    seed_finding(&pool, &fx, "user_confirmed", "需要手机号登录", "CONFIRMED").await;
    session_review_required(&pool, &fx).await;

    let mut batch = sample_batch();
    batch.main[1].acceptance_criteria = vec!["   ".to_string()];
    let write_back = hook(&pool, &fx, draft_reply(), batch);
    let session = view_session(&pool, &fx).await;
    let approved = approve_draft(&pool, &fx, &write_back, &session).await;

    let error = write_back
        .decompose_if_approved(SYSTEM_TENANT, &session)
        .await
        .unwrap_err();
    assert!(
        matches!(error, WriteBackError::BatchRejected(_)),
        "got {error:?}"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM delivery.task WHERE project_id = $1")
            .bind(fx.project_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0,
        "batch rejected → no task rows"
    );
    assert_eq!(approved, 1);
}

/// §7.3 夹具：超上限截断（前置 >3、主 >8）+ 未知依赖键整批拒绝。
#[sqlx::test(migrations = "../../migrations")]
async fn over_limit_batch_is_truncated(pool: PgPool) {
    let fx = fixture(&pool, "DKD-TRUNC").await;
    seed_finding(&pool, &fx, "user_confirmed", "需要手机号登录", "CONFIRMED").await;
    session_review_required(&pool, &fx).await;

    let mut batch = RawTaskBatch::default();
    for index in 0..(MAX_PREREQUISITE_TASKS + 2) {
        batch.prerequisite.push(RawTask {
            key: format!("p{index}"),
            title: format!("前置任务 {index}"),
            description: None,
            goal: None,
            deliverables: vec![],
            acceptance_criteria: vec![format!("前置 {index} 自测通过")],
            depends_on: vec![],
            role_ref: None,
        });
    }
    for index in 0..(MAX_MAIN_TASKS + 3) {
        batch.main.push(RawTask {
            key: format!("m{index}"),
            title: format!("主任务 {index}"),
            description: None,
            goal: None,
            deliverables: vec![],
            acceptance_criteria: vec![format!("主任务 {index} 验收通过")],
            depends_on: vec!["p0".to_string()],
            role_ref: None,
        });
    }
    let write_back = hook(&pool, &fx, draft_reply(), batch);
    let session = view_session(&pool, &fx).await;
    approve_draft(&pool, &fx, &write_back, &session).await;

    let outcome = write_back
        .decompose_if_approved(SYSTEM_TENANT, &session)
        .await
        .unwrap();
    let DecomposeOutcome::Generated(batch) = outcome else {
        panic!("expected generated batch, got {outcome:?}");
    };
    assert_eq!(batch.prerequisite_count, MAX_PREREQUISITE_TASKS);
    assert_eq!(batch.main_count, MAX_MAIN_TASKS);
    assert_eq!(batch.truncated, 2 + 3);
    assert_eq!(batch.task_count(), MAX_PREREQUISITE_TASKS + MAX_MAIN_TASKS);
}

/// §7.3 夹具：依赖引用未知键 → 整批拒绝（禁"标题字符串语义"的结构化保证）。
#[sqlx::test(migrations = "../../migrations")]
async fn dependency_on_unknown_key_rejects_the_batch(pool: PgPool) {
    let fx = fixture(&pool, "DKD-DEP").await;
    seed_finding(&pool, &fx, "user_confirmed", "需要手机号登录", "CONFIRMED").await;
    session_review_required(&pool, &fx).await;

    let mut batch = sample_batch();
    batch.main[0].depends_on = vec!["ghost".to_string()];
    let write_back = hook(&pool, &fx, draft_reply(), batch);
    let session = view_session(&pool, &fx).await;
    approve_draft(&pool, &fx, &write_back, &session).await;

    let error = write_back
        .decompose_if_approved(SYSTEM_TENANT, &session)
        .await
        .unwrap_err();
    assert!(
        matches!(error, WriteBackError::BatchRejected(_)),
        "got {error:?}"
    );
}

// ─────────────────────────────────────────────────────────────── 开关 / 白名单

/// 开关 off：起草与分解都零写入（默认 off 的 fail-closed 行为）。
#[sqlx::test(migrations = "../../migrations")]
async fn switch_off_writes_nothing(pool: PgPool) {
    let fx = fixture(&pool, "DKD-OFF").await;
    seed_finding(&pool, &fx, "user_confirmed", "需要手机号登录", "CONFIRMED").await;
    session_review_required(&pool, &fx).await;
    let session = view_session(&pool, &fx).await;

    let mut write_back = hook(&pool, &fx, draft_reply(), sample_batch());
    write_back.mode = WriteBackMode::Off;
    let draft = write_back
        .on_review_required(SYSTEM_TENANT, &session)
        .await
        .unwrap();
    assert_eq!(draft.kind, DraftOutcomeKind::Skipped);
    assert_eq!(draft.reason, Some("orchestrator_off"));
    assert_eq!(requirement_count(&pool, fx.project_id).await, 0);

    let outcome = write_back
        .decompose_if_approved(SYSTEM_TENANT, &session)
        .await
        .unwrap();
    assert!(matches!(outcome, DecomposeOutcome::NothingToDo { .. }));
}

/// 白名单是常量：✅ 项与 ❌ 项互斥，且禁止项一个都不在可写集合里（§6.2 第 4 道闸）。
#[test]
fn command_whitelist_excludes_every_forbidden_command() {
    assert_eq!(
        AI_WRITABLE_COMMANDS,
        [
            "create_requirement",
            "create_draft",
            "update_draft",
            "submit_revision"
        ]
    );
    for forbidden in AI_FORBIDDEN_COMMANDS {
        assert!(
            !AI_WRITABLE_COMMANDS.contains(&forbidden),
            "{forbidden} must never be AI-writable"
        );
    }
    // 四道闸里最容易漏的两个："确认"与"置 ready"。
    assert!(AI_FORBIDDEN_COMMANDS.contains(&"confirm_revision"));
    assert!(AI_FORBIDDEN_COMMANDS.contains(&"mark_task_ready"));
}

/// 开关解析：拼错值必须落在 off（不静默开启计费循环）。
#[test]
fn write_back_mode_parses_conservatively() {
    assert_eq!(WriteBackMode::parse(Some("on")), WriteBackMode::On);
    assert_eq!(WriteBackMode::parse(Some("off")), WriteBackMode::Off);
    assert_eq!(WriteBackMode::parse(None), WriteBackMode::Off);
    assert_eq!(WriteBackMode::parse(Some("ON")), WriteBackMode::Off);
    assert_eq!(WriteBackMode::parse(Some("true")), WriteBackMode::Off);
}

// ─────────────────────────────────────────────────────────────── 助手

/// 起草 + 人类确认，返回需求条数（调用方用它做"确实建了一条"的断言）。
async fn approve_draft(
    pool: &PgPool,
    fx: &Fx,
    write_back: &DiscoveryWriteBack,
    session: &pm_application::discovery::DiscoverySessionView,
) -> i64 {
    let draft = write_back
        .on_review_required(SYSTEM_TENANT, session)
        .await
        .unwrap();
    let requirement_id = draft.requirement_id.expect("draft created");
    let revision_id = draft.revision_id.expect("revision created");
    let hash: String =
        sqlx::query_scalar("SELECT content_hash FROM delivery.requirement_revision WHERE id = $1")
            .bind(revision_id)
            .fetch_one(pool)
            .await
            .unwrap();
    requirement::confirm_revision(
        pool,
        SYSTEM_TENANT,
        requirement_id,
        revision_id,
        ConfirmRevisionRequest {
            expected_content_hash: hash,
            expected_current_revision_id: None,
            expected_row_revision: None,
        },
        &pm_application::actor::AuditActor::principal(fx.human_id),
    )
    .await
    .unwrap();
    requirement_count(pool, fx.project_id).await
}
