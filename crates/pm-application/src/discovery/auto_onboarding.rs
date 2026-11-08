//! DK-E：快照分析完成 → 自动开 `INITIAL_ONBOARDING` 访谈（设计稿 §8.2 场景 2 首段）。
//!
//! 触发面 = `kb.analysis.completed` 的 outbox 消费（与 0048 intake 事件同款
//! 「事务内登记事件 + 消费者推进」纪律；**不新增定时扫描面**）。门控 =
//! `AI_PM_DISCOVERY_ORCHESTRATOR` 同一个开关：`enabled=false` 时本函数**零写入**
//! 直接返回（开关默认 off，F22 传统）。
//!
//! 产物只有两样，全部走 discovery 既有单点服务：
//! 1. 一条 `source='SYSTEM'` 的 `INITIAL_ONBOARDING` 会话（项目已有该 kind 的
//!    会话则复用；最近一条已 `COMPLETED` 则不复活）；
//! 2. 由**分析产物自身的不确定性**派生的 `discovery_question`（`why_blocking`
//!    必填）：分析模式非 `llm`（低置信）与采集触顶（覆盖未知）两类。
//!
//! 本模块不写 KB/PC、不写 proposal、不写 finding——那些是编排器后续轮次的
//! 职责（D-DK-10：本批 PC/KB 零写入）。

use super::*;

/// 开关关闭：零动作（不是错误）。
pub const ONBOARDING_SKIP_DISABLED: &str = "disabled";
/// 最近的 `INITIAL_ONBOARDING` 会话已 `COMPLETED`：不复活、不加新问题。
pub const ONBOARDING_SKIP_COMPLETED: &str = "completed_session";
/// `document_ref` 指向的 KB 修订不存在（非仓库快照/桩语义事件）或不属于本项目。
pub const ONBOARDING_SKIP_NO_ANALYSIS: &str = "analysis_not_found";

/// 分析产物里「我们不知道/不敢断言」的一条（转成一个必答问题）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisUnknownQuestion {
    pub question: String,
    pub why_blocking: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoOnboardingOutcome {
    pub session_id: Option<Uuid>,
    pub created_session: bool,
    pub questions_added: usize,
    /// `Some(reason)` = 本轮零动作的原因（见 `ONBOARDING_SKIP_*`）。
    pub skipped: Option<&'static str>,
}

impl AutoOnboardingOutcome {
    fn skipped(reason: &'static str) -> Self {
        Self {
            session_id: None,
            created_session: false,
            questions_added: 0,
            skipped: Some(reason),
        }
    }
}

/// 从分析元数据派生「未知/低置信点」（纯函数，可单测）。
///
/// - `analysis_mode != "llm"`：摘要只有确定性事实（纯派生或 LLM 回退），
///   产品/架构判断低置信 → 问项目背景；
/// - `truncated`：采集触顶，部分模块不在快照里 → 问未覆盖的关键模块。
///
/// 干净的 LLM 分析（`llm` 且未触顶）返回空表：会话照开，问题交给编排器按
/// 槽位缺口生成（§4.2 系统定骨架）。
pub fn analysis_unknown_questions(
    analysis_mode: &str,
    truncated: bool,
) -> Vec<AnalysisUnknownQuestion> {
    let mut questions = Vec::new();
    if analysis_mode != "llm" {
        questions.push(AnalysisUnknownQuestion {
            question: "仓库快照分析未经 LLM 增强（纯派生或回退），项目背景与架构判断可能不完整：\
                       请补充项目的核心业务、目标用户与当前要解决的问题。"
                .to_string(),
            why_blocking:
                "低置信分析缺失的部分会被后续需求当成事实，需求与任务会建立在错误前提上。"
                    .to_string(),
        });
    }
    if truncated {
        questions.push(AnalysisUnknownQuestion {
            question: "仓库采集触及文件/提交上限，部分模块未进入快照：\
                       哪些未覆盖的模块或目录对当前阶段是关键？"
                .to_string(),
            why_blocking: "未采集的关键模块会让技术栈与影响面判断缺项，访谈结论无法覆盖全项目。"
                .to_string(),
        });
    }
    questions
}

/// 分析完成事件的自动开访谈入口（幂等；`enabled=false` 时零写入）。
///
/// 幂等锚点（零迁移）：项目最近一条 `INITIAL_ONBOARDING` 会话 + 问题正文去重
/// ——同一分析事件重投（outbox 至少一次）不会重复建会话/问题。
pub async fn ensure_onboarding_from_analysis(
    pool: &PgPool,
    project_id: Uuid,
    revision_id: Uuid,
    enabled: bool,
) -> Result<AutoOnboardingOutcome, DiscoveryError> {
    if !enabled {
        return Ok(AutoOnboardingOutcome::skipped(ONBOARDING_SKIP_DISABLED));
    }
    let analysis: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT r.tenant_id, r.analysis_mode FROM kb.document_revision r \
         JOIN kb.document d ON d.tenant_id = r.tenant_id AND d.id = r.document_id \
         WHERE r.id = $1 AND d.project_id = $2",
    )
    .bind(revision_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    let Some((tenant_id, analysis_mode)) = analysis else {
        return Ok(AutoOnboardingOutcome::skipped(ONBOARDING_SKIP_NO_ANALYSIS));
    };
    // 采集触顶只在分析的审计 after_data 里（0062 前的列面没有它）。
    let truncated: Option<bool> = sqlx::query_scalar(
        "SELECT (after_data->>'truncated')::boolean FROM governance.audit_log \
         WHERE project_id = $1 AND target_type = 'kb_document_revision' \
           AND target_id = $2 AND action = 'kb_analysis_completed' \
         ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .bind(project_id)
    .bind(revision_id)
    .fetch_optional(pool)
    .await?;
    let truncated = truncated.unwrap_or(false);

    let latest: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT id, status FROM discovery.discovery_session \
         WHERE tenant_id = $1 AND project_id = $2 AND kind = 'INITIAL_ONBOARDING' \
         ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    if latest
        .as_ref()
        .is_some_and(|(_, status)| status == SESSION_STATUS_COMPLETED)
    {
        return Ok(AutoOnboardingOutcome::skipped(ONBOARDING_SKIP_COMPLETED));
    }
    let (session_id, created_session) = match latest {
        Some((session_id, _)) => (session_id, false),
        None => (
            create_session(
                pool,
                tenant_id,
                project_id,
                "SYSTEM",
                "INITIAL_ONBOARDING",
                crate::run_dispatch::DISPATCH_SERVICE_PRINCIPAL,
            )
            .await?
            .id,
            true,
        ),
    };

    let mut questions_added = 0usize;
    for unknown in analysis_unknown_questions(&analysis_mode, truncated) {
        let exists: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM discovery.discovery_question \
             WHERE tenant_id = $1 AND session_id = $2 \
               AND lower(btrim(question)) = lower(btrim($3)) LIMIT 1",
        )
        .bind(tenant_id)
        .bind(session_id)
        .bind(&unknown.question)
        .fetch_optional(pool)
        .await?;
        if exists.is_some() {
            continue;
        }
        add_question(
            pool,
            tenant_id,
            session_id,
            &unknown.question,
            Some(unknown.why_blocking.as_str()),
            None,
        )
        .await?;
        questions_added += 1;
    }

    Ok(AutoOnboardingOutcome {
        session_id: Some(session_id),
        created_session,
        questions_added,
        skipped: None,
    })
}
