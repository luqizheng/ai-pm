//! Task 17: main-chain model selection behind the `Arc<dyn AnalysisModel>`
//! port. The worker keeps one [`RoutedAnalysisModel`]; the real
//! OpenAI-compatible adapter is resolved **per call, per project** (the
//! three-level ruling: project designation strictly without fallback →
//! first enabled tenant provider → none), so a mid-flight provider
//! enable/disable is picked up by the next event without a restart.
//!
//! Mode knob `AI_PM_ANALYSIS_MODEL` (Task 17 ruling):
//! - `auto` (default): resolve as above; a `None` or failed resolution is a
//!   **显式 Configuration Error**（DEC-LLM-004/§10.3：不静默降级、不回退
//!   FakeModel——fake 输出是 fixture 数据，冒充分析结论违反 Fail-Fast 裁决）。
//! - `fake`: pin the fixtures (development/tests only).

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use pm_application::team_llm::ResolvedProvider;
use pm_application::team_llm_catalog::{resolve_model_for_role, ModelResolutionError};
use pm_domain::ProjectId;

use crate::call_record::{self, elapsed_ms, PURPOSE_AI_ANALYSIS};
use crate::fake_model::FakeModel;
use crate::model::{AnalysisContext, AnalysisModel, ModelError};
use crate::openai_adapter::OpenAiCompatibleModel;
use crate::skill_router::SkillId;

/// 主分析链的调用用途 Role（任务书 §6.3 九链映射表）。
pub const ANALYSIS_ROLE_AI_PM_LEAD: &str = "ai_pm_lead";

/// How the worker picks the analysis model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisModelMode {
    /// Per-project provider resolution; unresolved → explicit
    /// [`ModelError::Configuration`]（无 fake 回退）。
    Auto,
    /// Always the deterministic fake (development/tests only).
    Fake,
}

impl AnalysisModelMode {
    /// `fake` pins the fixtures; unset and `auto` mean auto. Any other
    /// value is a typo — warn loudly and use the default (auto), never
    /// silently pin fake: a typo must not hide real analysis behind
    /// fixtures.
    pub fn parse(value: Option<&str>) -> Self {
        match value {
            Some("fake") => Self::Fake,
            Some("auto") | None => Self::Auto,
            Some(other) => {
                tracing::warn!(
                    value = other,
                    "unknown AI_PM_ANALYSIS_MODEL value (expected auto|fake); using default auto"
                );
                Self::Auto
            }
        }
    }

    pub fn from_env() -> Self {
        Self::parse(std::env::var("AI_PM_ANALYSIS_MODEL").ok().as_deref())
    }
}

/// The role-chain resolution outcome for one project (the selection logic
/// is exposed as a plain enum so tests can pin the branches without
/// reaching into the constructed model).
#[derive(Debug)]
pub enum ModelSelection {
    /// A usable provider: endpoint/model/key resolved (key decrypted),
    /// model carries its catalog `model_ref` for记账归因.
    Provider(ResolvedProvider),
    /// 四级链走完仍无可用模型，或命中行严格校验失败（§6.1 reason 词表）。
    /// 上层必须产出显式 Configuration Error（DEC-LLM-004：禁止降级/回退）。
    ConfigurationFailed {
        reason: &'static str,
        detail: String,
    },
    /// 解析基础设施失败（DB / 项目行缺失）。
    ResolutionFailed(String),
}

/// 四级 Role 解析链（Phase 3B，任务书 §6）：项目 Role 覆盖 → Team Role
/// 策略 → Team general → 显式 Configuration Error；命中即严格校验。
pub async fn select_model_for_role(
    pool: &sqlx::PgPool,
    project_id: ProjectId,
    role_key: &str,
) -> ModelSelection {
    match resolve_model_for_role(pool, project_id, role_key).await {
        Ok(cfg) => ModelSelection::Provider(cfg),
        Err(ModelResolutionError::Configuration { reason, detail }) => {
            ModelSelection::ConfigurationFailed { reason, detail }
        }
        Err(ModelResolutionError::Infrastructure(e)) => {
            ModelSelection::ResolutionFailed(e.to_string())
        }
    }
}

/// The worker-facing model: delegates to the real adapter or the fake per
/// call. The orchestrator never changes — the whole Task 17 swap lives
/// behind this `AnalysisModel` impl.
pub struct RoutedAnalysisModel {
    pool: sqlx::PgPool,
    mode: AnalysisModelMode,
}

impl RoutedAnalysisModel {
    pub fn new(pool: sqlx::PgPool, mode: AnalysisModelMode) -> Self {
        Self { pool, mode }
    }

    /// Reads `AI_PM_ANALYSIS_MODEL` once at startup and announces the
    /// effective mode (a wrong knob value must be visible in the logs).
    pub fn from_env(pool: sqlx::PgPool) -> Self {
        let mode = AnalysisModelMode::from_env();
        tracing::info!(?mode, "analysis model mode selected (AI_PM_ANALYSIS_MODEL)");
        Self { pool, mode }
    }

    async fn model_for(
        &self,
        project_id: ProjectId,
    ) -> Result<Arc<dyn AnalysisModel>, ModelError> {
        match self.mode {
            AnalysisModelMode::Fake => Ok(Arc::new(FakeModel::from_fixtures())),
            AnalysisModelMode::Auto => {
                match select_model_for_role(&self.pool, project_id, ANALYSIS_ROLE_AI_PM_LEAD).await
                {
                    ModelSelection::Provider(cfg) => {
                        tracing::info!(
                            provider_id = %cfg.provider_id,
                            model = %cfg.model,
                            "analysis model resolved via role chain (ai_pm_lead)"
                        );
                        // The DB CHECK keeps timeout_secs >= 1; the clamp only
                        // guards raw-seeded rows against the adapter's panic.
                        let timeout = chrono::Duration::seconds(i64::from(cfg.timeout_secs).max(1));
                        let real = OpenAiCompatibleModel::new(
                            &cfg.endpoint,
                            &cfg.model,
                            &cfg.api_key,
                            timeout,
                        );
                        Ok(Arc::new(RecordingAnalysisModel {
                            pool: self.pool.clone(),
                            project_id,
                            provider_id: cfg.provider_id,
                            model_name: cfg.model.clone(),
                            inner: Arc::new(real),
                        }))
                    }
                    ModelSelection::ConfigurationFailed { reason, detail } => {
                        // §6.1：四级链无可用模型或严格校验失败——显式
                        // Configuration Error，不降级不回退（DEC-LLM-004）。
                        tracing::warn!(
                            %project_id,
                            reason,
                            "analysis model resolution returned an explicit configuration failure"
                        );
                        Err(ModelError::Configuration(format!(
                            "model resolution failed for project {project_id}: {reason}: {detail}"
                        )))
                    }
                    ModelSelection::ResolutionFailed(error) => {
                        tracing::warn!(
                            %project_id,
                            reason = "resolution_failed",
                            %error,
                            "analysis model resolution failed (infrastructure)"
                        );
                        Err(ModelError::Configuration(format!(
                            "model resolution failed for project {project_id}: {error}"
                        )))
                    }
                }
            }
        }
    }
}

#[async_trait]
impl AnalysisModel for RoutedAnalysisModel {
    async fn analyze(
        &self,
        context: &AnalysisContext,
        skills: &[SkillId],
    ) -> Result<pm_domain::AnalysisResult, ModelError> {
        self.model_for(context.project_id)
            .await?
            .analyze(context, skills)
            .await
    }
}

/// Wraps the real OpenAI-compatible model: every call (ok or failed) lands
/// one `ai.llm_call_record` row, best-effort, linked back to the project
/// and the event's current analysis run. Fake fallbacks carry no provider
/// row and are never recorded.
struct RecordingAnalysisModel {
    pool: sqlx::PgPool,
    project_id: ProjectId,
    provider_id: uuid::Uuid,
    model_name: String,
    inner: Arc<dyn AnalysisModel>,
}

#[async_trait]
impl AnalysisModel for RecordingAnalysisModel {
    async fn analyze(
        &self,
        context: &AnalysisContext,
        skills: &[SkillId],
    ) -> Result<pm_domain::AnalysisResult, ModelError> {
        let started = Instant::now();
        let result = self.inner.analyze(context, skills).await;
        let (status, error_kind, error_message) = match &result {
            Ok(_) => ("ok", None, None),
            Err(err) => (
                "failed",
                Some(call_record::analysis_error_kind(err)),
                Some(err.to_string()),
            ),
        };
        call_record::record_call(
            &self.pool,
            self.project_id,
            self.provider_id,
            &self.model_name,
            PURPOSE_AI_ANALYSIS,
            status,
            error_kind,
            error_message.as_deref(),
            elapsed_ms(started),
            Some(context.event_id),
            // Batch H: only the conversation-reply chain reads the provider
            // `usage` object; the analysis chain keeps its token columns
            // NULL (its usage accounting remains挂账).
            None,
        )
        .await;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pm_domain::SYSTEM_TENANT;
    use uuid::Uuid;

    #[test]
    fn mode_parse_pins_fake_and_defaults_to_auto() {
        assert_eq!(AnalysisModelMode::parse(None), AnalysisModelMode::Auto);
        assert_eq!(
            AnalysisModelMode::parse(Some("auto")),
            AnalysisModelMode::Auto
        );
        assert_eq!(
            AnalysisModelMode::parse(Some("fake")),
            AnalysisModelMode::Fake
        );
        // A typo must not silently pin fake: warn + default auto.
        assert_eq!(
            AnalysisModelMode::parse(Some("fkae")),
            AnalysisModelMode::Auto
        );
    }

    /// Bootstrap-equivalent rows (same shape as tests/orchestrator.rs):
    /// customer + project under the system tenant.
    async fn seed_project(pool: &sqlx::PgPool) -> ProjectId {
        let customer_id = Uuid::new_v4();
        let project_id = ProjectId::new_v4();
        sqlx::query("INSERT INTO core.customer (id, name, tenant_id) VALUES ($1, 'Acme', $2)")
            .bind(customer_id)
            .bind(SYSTEM_TENANT)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO core.project (id, customer_id, code, name, tenant_id) \
             VALUES ($1, $2, 'SEL', 'Selection Project', $3)",
        )
        .bind(project_id)
        .bind(customer_id)
        .bind(SYSTEM_TENANT)
        .execute(pool)
        .await
        .unwrap();
        project_id
    }

    /// An enabled tenant provider; `openai_compatible` needs no credential
    /// row (anonymous endpoints are allowed), so the decrypt path stays out.
    async fn seed_enabled_provider(pool: &sqlx::PgPool, endpoint: &str) -> uuid::Uuid {
        let principal_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO core.principal (id, kind, display_name) VALUES ($1, 'ai_pm', 'Seed')",
        )
        .bind(principal_id)
        .execute(pool)
        .await
        .unwrap();
        let provider_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO ai.team_llm_provider \
             (id, tenant_id, name, provider_type, endpoint, configuration, status, created_by) \
             VALUES ($1, $2, 'seed-provider', 'openai_compatible', $3, \
                     '{\"model\": \"glm-4-flash\"}', 'enabled', $4)",
        )
        .bind(provider_id)
        .bind(SYSTEM_TENANT)
        .bind(endpoint)
        .bind(principal_id)
        .execute(pool)
        .await
        .unwrap();
        // Phase 3B：解析链只认目录面——补目录模型 + 授权 + general 策略
        crate::test_seed::seed_model_catalog(
            pool,
            SYSTEM_TENANT,
            provider_id,
            "glm-4-flash",
            principal_id,
        )
        .await;
        provider_id
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn auto_resolves_the_tenant_enabled_provider(pool: sqlx::PgPool) {
        let project_id = seed_project(&pool).await;
        let provider_id = seed_enabled_provider(&pool, "http://127.0.0.1:9").await;
        match select_model_for_role(&pool, project_id, ANALYSIS_ROLE_AI_PM_LEAD).await {
            ModelSelection::Provider(cfg) => {
                assert_eq!(cfg.provider_id, provider_id);
                assert_eq!(cfg.model, "glm-4-flash");
            }
            other => panic!("expected a provider, got {other:?}"),
        }
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn auto_without_any_policy_yields_explicit_configuration_failure(pool: sqlx::PgPool) {
        let project_id = seed_project(&pool).await;
        // 四级链走完无任何配置 → general_policy_missing（§6.1）；消费层
        // （model_for）产出显式 ModelError::Configuration（无 fake 回退）。
        assert!(matches!(
            select_model_for_role(&pool, project_id, ANALYSIS_ROLE_AI_PM_LEAD).await,
            ModelSelection::ConfigurationFailed {
                reason: "general_policy_missing",
                ..
            }
        ));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn unknown_project_is_an_explicit_configuration_failure(pool: sqlx::PgPool) {
        assert!(matches!(
            select_model_for_role(&pool, Uuid::new_v4(), ANALYSIS_ROLE_AI_PM_LEAD).await,
            ModelSelection::ConfigurationFailed {
                reason: "no_model_for_role",
                ..
            }
        ));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn fake_mode_never_records_or_reaches_out(pool: sqlx::PgPool) {
        // No project/provider seeding on purpose: the fake arm must not
        // touch the database at all.
        let routed = RoutedAnalysisModel::new(pool.clone(), AnalysisModelMode::Fake);
        let result = routed
            .analyze(
                &AnalysisContext {
                    project_id: Uuid::new_v4(),
                    event_id: Uuid::new_v4(),
                    event_type: "customer_message_added".to_string(),
                    payload: serde_json::json!({}),
                    task: None,
                    requirement_revision: None,
                    milestone: None,
                    config: serde_json::json!({}),
                    policy_version: 1,
                },
                &[],
            )
            .await
            .unwrap();
        assert!(result.proposals.is_empty(), "fake default is no-change");
        let recorded: i64 = sqlx::query_scalar("SELECT count(*) FROM ai.llm_call_record")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(recorded, 0, "fake calls must not land call records");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn auto_real_calls_land_ok_and_failed_call_records(pool: sqlx::PgPool) {
        let project_id = seed_project(&pool).await;
        let server = wiremock::MockServer::start().await;
        seed_enabled_provider(&pool, &server.uri()).await;

        // Mount order matters: mocks are matched in mounting order, so
        // call 1 hits the 200 and call 2 falls through to the 500 once the
        // 200's expectation is exhausted.
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/chat/completions"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "choices": [{"message": {"content":
                        "{\"summary\":\"s\",\"findings\":[],\"proposals\":[]}"}}
                    ]
                })),
            )
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/chat/completions"))
            .respond_with(wiremock::ResponseTemplate::new(500))
            .expect(1)
            .mount(&server)
            .await;

        let routed = RoutedAnalysisModel::new(pool.clone(), AnalysisModelMode::Auto);
        let ctx = AnalysisContext {
            project_id,
            event_id: Uuid::new_v4(),
            event_type: "customer_message_added".to_string(),
            payload: serde_json::json!({}),
            task: None,
            requirement_revision: None,
            milestone: None,
            config: serde_json::json!({}),
            policy_version: 1,
        };
        let ok = routed.analyze(&ctx, &[]).await.unwrap();
        assert_eq!(ok.summary, "s");
        assert!(matches!(
            routed.analyze(&ctx, &[]).await,
            Err(ModelError::Unavailable(_))
        ));

        let rows: Vec<(String, String, Option<String>, Option<i64>)> = sqlx::query_as(
            "SELECT purpose, status, error_kind, latency_ms FROM ai.llm_call_record \
             WHERE project_id = $1 ORDER BY status DESC",
        )
        .bind(project_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(rows.len(), 2, "both calls must be recorded");
        let failed = rows.iter().find(|r| r.1 == "failed").expect("failed row");
        let ok_row = rows.iter().find(|r| r.1 == "ok").expect("ok row");
        assert_eq!(failed.0, "ai_analysis");
        assert_eq!(failed.2.as_deref(), Some("model_unavailable"));
        assert_eq!(ok_row.0, "ai_analysis");
        assert_eq!(ok_row.2, None);
        assert!(ok_row.3.is_some(), "latency must be recorded");

        let meta: (String, String, uuid::Uuid) = sqlx::query_as(
            "SELECT model, purpose, provider_id FROM ai.llm_call_record WHERE project_id = $1 LIMIT 1",
        )
        .bind(project_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(meta.0, "glm-4-flash");
        assert_eq!(meta.1, "ai_analysis");
    }
}
