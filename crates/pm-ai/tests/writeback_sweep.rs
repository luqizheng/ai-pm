//! DK-F 验收 · 写回扫描面（真栈联调补的触发面）。
//!
//! **为什么有这份测试**：DK-F 在真栈（DeepSeek 真 provider + 真库）上跑
//! 场景 1 时发现一条断头路——编排轮次跃迁到 `REVIEW_REQUIRED` 的那一刻，
//! finding 必然还是 `PENDING`（确认状态只能由人工裁决产生），所以轮次内
//! 的写回调用**必然**跳过起草（`no_confirmed_finding`）；而人工裁决之后，
//! 会话已停在 `REVIEW_REQUIRED`，不再满足编排扫描谓词
//! （只扫 `IN_PROGRESS`/`NEEDS_USER_INPUT`）→ 永远没有第二轮 → 需求永远
//! 起草不出来。
//!
//! DK-D 的单测看不见它：那批测试**直接造 CONFIRMED finding** 再手动调钩子，
//! 绕过了「轮次内触发」这个前提。
//!
//! 本文件钉住补上之后的触发面：
//! - `sweep_write_back` 在 `REVIEW_REQUIRED` + 已有 CONFIRMED finding 时起草；
//! - 人工裁决**之前**不起草（不把还没人确认的事实变成需求）；
//! - 幂等：已起草的会话再扫一次不产生第二条需求。

use std::sync::Arc;

use async_trait::async_trait;
use pm_ai::discovery_orchestrator::DiscoveryOrchestratorWorker;
use pm_ai::discovery_writeback::{
    DecomposeOutcome, DiscoveryWriteBack, DraftOutcome, DraftOutcomeKind, EmptyCapabilityProjection,
    ModelCallFailure, ModelReply, ProviderStamp, RawTaskBatch, RequirementDraft,
    RequirementDraftContext, TaskDecomposeContext, WriteBackError, WriteBackMode, WriteBackModel,
};
use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_application::knowledge::KnowledgeRoot;
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

async fn fixture(pool: &PgPool, code: &str) -> Fx {
    let boot = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: format!("dkf-{code}"),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .unwrap();
    let session = pm_application::discovery::create_session(
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
    .bind(format!("dkf-provider-{code}"))
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
            std::env::temp_dir().join(format!("ai-pm-dkf-sweep-{}", Uuid::new_v4())),
        ),
    }
}

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

async fn review_required(pool: &PgPool, fx: &Fx) {
    pm_application::discovery::transition_session(
        pool,
        SYSTEM_TENANT,
        fx.session_id,
        "REVIEW_REQUIRED",
    )
    .await
    .unwrap();
}

async fn requirement_count(pool: &PgPool, project_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM delivery.requirement WHERE project_id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// 记录调用次数的假模型（不碰外部 provider）。
struct CountingModel {
    draft: RequirementDraft,
    stamp: ProviderStamp,
    drafts: Arc<std::sync::atomic::AtomicUsize>,
}

#[async_trait]
impl WriteBackModel for CountingModel {
    async fn draft_requirement(
        &self,
        _ctx: &RequirementDraftContext,
    ) -> Result<ModelReply<RequirementDraft>, ModelCallFailure> {
        self.drafts
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(ModelReply {
            value: self.draft.clone(),
            provider: self.stamp.clone(),
            latency_ms: 5,
        })
    }

    async fn decompose_tasks(
        &self,
        _ctx: &TaskDecomposeContext,
    ) -> Result<ModelReply<RawTaskBatch>, ModelCallFailure> {
        Ok(ModelReply {
            value: RawTaskBatch::default(),
            provider: self.stamp.clone(),
            latency_ms: 5,
        })
    }
}

fn draft_reply() -> RequirementDraft {
    RequirementDraft {
        code: Some("PHONE-LOGIN".to_string()),
        title: "手机号验证码登录".to_string(),
        summary: Some("普通买家可用手机号+短信验证码登录".to_string()),
        markdown: "# 手机号验证码登录\n\n## 目标\n普通买家可用手机号登录。\n"
            .to_string(),
    }
}

fn worker_with(
    pool: &PgPool,
    fx: &Fx,
    drafts: Arc<std::sync::atomic::AtomicUsize>,
) -> DiscoveryOrchestratorWorker {
    let hook = DiscoveryWriteBack {
        pool: pool.clone(),
        model: Arc::new(CountingModel {
            draft: draft_reply(),
            stamp: ProviderStamp {
                provider_id: fx.provider_id,
                model: "fake-dkf".to_string(),
            },
            drafts,
        }),
        capability: Arc::new(EmptyCapabilityProjection),
        knowledge: fx.root.clone(),
        mode: WriteBackMode::On,
    };
    DiscoveryOrchestratorWorker::new(pool.clone()).with_write_back(Arc::new(hook))
}

// ─────────────────────────────────────────────────────────────── 用例

/// 真栈断头路的回归用例：人工裁决 CONFIRMED finding 之后，扫描面必须起草。
#[sqlx::test(migrations = "../../migrations")]
async fn sweep_drafts_after_human_confirms_findings(pool: PgPool) {
    let fx = fixture(&pool, "DKF-SWEEP-1").await;
    let drafts = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    // ① 会话进入 REVIEW_REQUIRED，finding 仍是 PENDING（编排器写的默认态）。
    seed_finding(&pool, &fx, "user_confirmed", "需要手机号登录", "PENDING").await;
    review_required(&pool, &fx).await;

    // ② 人工裁决之前不起草（不把还没人确认的事实变成需求）。
    let mut worker = worker_with(&pool, &fx, drafts.clone());
    let report = worker.sweep_write_back().await.unwrap();
    assert_eq!(
        report.drafts_submitted, 0,
        "no CONFIRMED finding yet — no requirement may be drafted"
    );
    assert_eq!(requirement_count(&pool, fx.project_id).await, 0);

    // ③ 人工裁决（CONFIRMED）→ 扫描面起草（止步 pending_confirmation）。
    sqlx::query(
        "UPDATE discovery.discovery_finding SET verification_status = 'CONFIRMED' \
         WHERE tenant_id = $1 AND session_id = $2",
    )
    .bind(SYSTEM_TENANT)
    .bind(fx.session_id)
    .execute(&pool)
    .await
    .unwrap();

    let mut worker = worker_with(&pool, &fx, drafts.clone());
    let report = worker.sweep_write_back().await.unwrap();
    assert_eq!(report.drafts_submitted, 1, "CONFIRMED finding must be drafted");
    assert_eq!(requirement_count(&pool, fx.project_id).await, 1);

    // ④ AI 止步 pending_confirmation（approved 只能由真人产生）。
    let state: String = sqlx::query_scalar(
        "SELECT state FROM delivery.requirement_revision \
         WHERE requirement_id = (SELECT id FROM delivery.requirement WHERE project_id = $1)",
    )
    .bind(fx.project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(state, "pending_confirmation");

    // ⑤ 幂等：再扫一次不产生第二条需求（写回侧的 AlreadyDrafted）。
    let mut worker = worker_with(&pool, &fx, drafts.clone());
    worker.sweep_write_back().await.unwrap();
    assert_eq!(requirement_count(&pool, fx.project_id).await, 1);
}

/// 编排扫描谓词不含 `REVIEW_REQUIRED`——这正是本批要补触发面的原因，
/// 用例把这个前提钉住（若哪天谓词变了，扫描面可退化为轮次内触发）。
#[sqlx::test(migrations = "../../migrations")]
async fn orchestration_scan_does_not_cover_review_required_sessions(pool: PgPool) {
    let fx = fixture(&pool, "DKF-SWEEP-2").await;
    seed_finding(&pool, &fx, "user_confirmed", "需要手机号登录", "CONFIRMED").await;
    review_required(&pool, &fx).await;

    let mut worker = DiscoveryOrchestratorWorker::new(pool.clone());
    let report = worker.orchestrate_round().await.unwrap();
    assert_eq!(
        report.processed, 0,
        "REVIEW_REQUIRED sessions are not orchestration candidates"
    );
}

/// 写回钩子在 `WriteBackMode::Off` 时零写入（开关纪律）。
#[sqlx::test(migrations = "../../migrations")]
async fn write_back_off_means_zero_writes(pool: PgPool) {
    let fx = fixture(&pool, "DKF-SWEEP-3").await;
    seed_finding(&pool, &fx, "user_confirmed", "需要手机号登录", "CONFIRMED").await;
    review_required(&pool, &fx).await;

    let hook = DiscoveryWriteBack {
        pool: pool.clone(),
        model: Arc::new(CountingModel {
            draft: draft_reply(),
            stamp: ProviderStamp {
                provider_id: fx.provider_id,
                model: "fake-dkf".to_string(),
            },
            drafts: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }),
        capability: Arc::new(EmptyCapabilityProjection),
        knowledge: fx.root.clone(),
        mode: WriteBackMode::Off,
    };
    let mut worker =
        DiscoveryOrchestratorWorker::new(pool.clone()).with_write_back(Arc::new(hook));
    let report = worker.sweep_write_back().await.unwrap();
    assert_eq!(report.drafts_submitted, 0);
    assert_eq!(requirement_count(&pool, fx.project_id).await, 0);
}

// 编译器要求这两个类型出现在断言里（`DraftOutcome`/`WriteBackError` 的
// 语义在本文件只用到其枚举分支，显式引用以免未来重构时误删 import）。
#[allow(dead_code)]
fn _type_anchors(_: DraftOutcome, _: WriteBackError, _: DecomposeOutcome, _: DraftOutcomeKind) {}
