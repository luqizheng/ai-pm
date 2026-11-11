//! Batch G: the assistant-reply producer loop for the D-4 conversation store
//! (the "seventh loop" body, kept here so it is testable outside the worker
//! binary — the worker only owns the spawn + poll cadence, same split as
//! `review::run_review_loop`).
//!
//! Scan semantics: a conversation is *pending* iff its latest message
//! (by `created_at, id` DESC) has role=`user`. After a successful reply the
//! latest role flips to `assistant`, so the conversation naturally leaves
//! the scan set. Crash/restart safety needs no bookkeeping: at worst the
//! LLM is called again for a still-pending conversation (duplicate model
//! calls are accepted by the batch ruling; the append itself is idempotent
//! per reply, and duplicates would only add one more assistant turn).
//!
//! Provider resolution reuses the Task 17 three-level selection verbatim
//! (`analysis_model::select_analysis_model_config` → strict project
//! designation, no fallback → first enabled tenant provider → none).
//! `None` / resolution failure → **structured WARN, skip, no reply, no fake**
//! — a chat must never receive fabricated content.
//!
//! Starvation guard: a conversation whose reply cannot be produced (no
//! provider, resolution failure, model error, append failure) never leaves
//! the scan set, so it would sit at the FIFO head forever and starve newer
//! conversations. The worker keeps a small in-memory cooldown (30s) per
//! failed conversation; nothing is persisted, so a restart simply forgets
//! the cooldowns (immediate retry — acceptable per the crash-safety ruling).
//!
//! Switch `AI_PM_CONVERSATION_REPLY=off|on`, **default off** (aligned with
//! the `AI_PM_RUN_DISPATCH` off/probe/on tradition: a new producer loop must
//! not start billing real model tokens on upgrade; operators turn it on
//! explicitly per environment).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::analysis_model::select_model_for_role;
use crate::call_record::{self, elapsed_ms, PURPOSE_CONVERSATION_REPLY};
use crate::conversation_model::{self, ConversationTurn, CONTEXT_WINDOW};
use crate::model::ModelError;

/// Pending conversations processed per round (FIFO head, batch ruling N=5).
pub const MAX_CONVERSATIONS_PER_ROUND: i64 = 5;
/// Scan headroom above [`MAX_CONVERSATIONS_PER_ROUND`]: the SQL fetches a
/// few extra candidates so cooldown-skipped head entries cannot hide newer
/// pending conversations behind them.
pub const SCAN_CAP: i64 = 25;
/// Cooldown for a conversation whose reply attempt failed (see module doc).
pub const RETRY_COOLDOWN: Duration = Duration::from_secs(30);

/// Reply switch (`AI_PM_CONVERSATION_REPLY`): off by default — a new
/// billable producer must not silently start on upgrade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationReplyMode {
    Off,
    On,
}

impl ConversationReplyMode {
    pub fn parse(value: Option<&str>) -> Self {
        match value {
            Some("on") => Self::On,
            Some("off") | None => Self::Off,
            // A typo must not silently enable a billable loop: warn + off.
            Some(other) => {
                tracing::warn!(
                    value = other,
                    "unknown AI_PM_CONVERSATION_REPLY value (expected off|on); staying off"
                );
                Self::Off
            }
        }
    }

    pub fn from_env() -> Self {
        Self::parse(std::env::var("AI_PM_CONVERSATION_REPLY").ok().as_deref())
    }

    pub fn enabled(self) -> bool {
        matches!(self, Self::On)
    }
}

/// One round's outcome (observability surface for the worker log line and
/// the loop tests).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReplyReport {
    pub replied: usize,
    pub failed: usize,
    pub skipped_no_provider: usize,
}

impl ReplyReport {
    pub fn has_activity(&self) -> bool {
        self.replied + self.failed + self.skipped_no_provider > 0
    }
}

/// Stateful round runner: owns the in-memory retry cooldowns.
pub struct ConversationReplyWorker {
    pool: sqlx::PgPool,
    cooldowns: HashMap<Uuid, Instant>,
}

struct PendingConversation {
    id: Uuid,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
    /// Batch 2（0003）：线程级上下文锚点；NULL = 无锚点（存量线程）。
    ///
    /// 锚点只用于**定位**注入源（哪个仓库），不直接进模型——注入内容一律
    /// 由服务端按锚点重新投影实时事实（Batch 3A）。
    context_anchor: Option<serde_json::Value>,
}

impl ConversationReplyWorker {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self {
            pool,
            cooldowns: HashMap::new(),
        }
    }

    /// One scan-and-reply round: pending conversations in FIFO order
    /// (oldest pending first), at most [`MAX_CONVERSATIONS_PER_ROUND`]
    /// processed, cooldown-skipped entries excluded.
    pub async fn reply_round(&mut self) -> Result<ReplyReport, sqlx::Error> {
        let mut report = ReplyReport::default();
        let candidates = self.scan_pending().await?;
        let now = Instant::now();
        let mut processed = 0;
        for conversation in candidates {
            if processed >= MAX_CONVERSATIONS_PER_ROUND as usize {
                break;
            }
            if self
                .cooldowns
                .get(&conversation.id)
                .is_some_and(|t| now.duration_since(*t) < RETRY_COOLDOWN)
            {
                continue; // recently failed: cool down, do not count against N
            }
            processed += 1;
            self.reply_to_conversation(&conversation, &mut report).await;
        }
        // Cooldown GC: drop expired entries so the map cannot grow unbounded.
        self.cooldowns
            .retain(|_, t| now.duration_since(*t) < RETRY_COOLDOWN);
        Ok(report)
    }

    /// Pending = latest message (created_at, id DESC) has role='user';
    /// FIFO by that latest message's (created_at, id) ascending.
    async fn scan_pending(&self) -> Result<Vec<PendingConversation>, sqlx::Error> {
        let rows = sqlx::query_as::<_, (Uuid, Uuid, Uuid, Uuid, Option<serde_json::Value>)>(
            "SELECT c.id, c.tenant_id, c.project_id, c.principal_id, c.context_anchor \
             FROM ai.conversation c \
             JOIN LATERAL ( \
                 SELECT id, role, created_at FROM ai.conversation_message m \
                 WHERE m.conversation_id = c.id \
                 ORDER BY m.created_at DESC, m.id DESC LIMIT 1 \
             ) last ON last.role = 'user' \
             ORDER BY last.created_at ASC, last.id ASC \
             LIMIT $1",
        )
        .bind(SCAN_CAP)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(id, tenant_id, project_id, principal_id, context_anchor)| PendingConversation {
                    id,
                    tenant_id,
                    project_id,
                    principal_id,
                    context_anchor,
                },
            )
            .collect())
    }

    async fn reply_to_conversation(
        &mut self,
        conversation: &PendingConversation,
        report: &mut ReplyReport,
    ) {
        match self.try_reply(conversation).await {
            Ok(()) => report.replied += 1,
            Err(ReplyFailure::NoProvider { reason }) => {
                tracing::warn!(
                    conversation_id = %conversation.id,
                    %conversation.project_id,
                    reason,
                    "no LLM provider resolved for this conversation (strict three-level \
                     resolution found nothing usable); skipping the reply — no response is \
                     fabricated and none is recorded"
                );
                self.cooldowns.insert(conversation.id, Instant::now());
                report.skipped_no_provider += 1;
            }
            Err(ReplyFailure::Model(err)) => {
                tracing::warn!(
                    conversation_id = %conversation.id,
                    %conversation.project_id,
                    error = %err,
                    "conversation reply model call failed; will retry after cooldown"
                );
                self.cooldowns.insert(conversation.id, Instant::now());
                report.failed += 1;
            }
            Err(ReplyFailure::Db(error)) => {
                // Storage failure: not cooldown-worthy per se (the model call
                // already succeeded or never happened), but the round must
                // stay honest about it.
                tracing::warn!(
                    conversation_id = %conversation.id,
                    %error,
                    "conversation reply persistence failed; will retry after cooldown"
                );
                self.cooldowns.insert(conversation.id, Instant::now());
                report.failed += 1;
            }
        }
    }

    /// Resolve provider → assemble context → call model → append assistant
    /// message (with model/latency metadata) → land the call record.
    async fn try_reply(&self, conversation: &PendingConversation) -> Result<(), ReplyFailure> {
        let history = self.load_history(conversation.id).await?;

        // Batch 3A：chat 精简注入（S1 项目画像 + S2 仓库事实）。
        //
        // 注入源 = 线程锚点（Batch 2）→ 服务端按锚点**重新投影**实时事实；
        // 锚点缺失或查不到 → 零注入，模型上下文与注入前逐字节一致
        // （向后兼容：存量线程行为不变）。
        //
        // 注入失败（DB 错）**不阻断回复**——上下文是增强，不是回复的前提。
        let (injected, injection_meta) = match self.build_injection(conversation).await {
            Ok(pair) => pair,
            Err(error) => {
                tracing::warn!(
                    conversation_id = %conversation.id,
                    %error,
                    "chat context injection failed; replying without injected context"
                );
                (None, serde_json::Value::Null)
            }
        };

        // Phase 3B role chain（任务书 §6.3）：对话回复 = general 用途；
        // 四级解析（覆盖 → Role 策略 → general → 显式 Configuration Error）。
        match select_model_for_role(&self.pool, conversation.project_id, "general").await {
            crate::analysis_model::ModelSelection::Provider(cfg) => {
                let messages = Self::build_messages(&history, injected.as_deref());
                let timeout = chrono::Duration::seconds(i64::from(cfg.timeout_secs).max(1));
                let started = Instant::now();
                let reply = conversation_model::complete(
                    &cfg.endpoint,
                    &cfg.api_key,
                    &cfg.model,
                    timeout,
                    &messages,
                )
                .await;
                let latency = elapsed_ms(started);
                match reply {
                    Ok(completion) => {
                        // Record first, append second: a storage failure must
                        // not erase the (truthful) ok record of a model call
                        // that already happened — and every real call lands
                        // exactly one record either way. Batch H: the
                        // provider's usage (when present) fills the
                        // prompt/completion token columns (no total_tokens
                        // column in 0041 — the total rides in metadata).
                        call_record::record_call(
                            &self.pool,
                            conversation.project_id,
                            cfg.provider_id,
                            &cfg.model,
                            PURPOSE_CONVERSATION_REPLY,
                            "ok",
                            None,
                            None,
                            latency,
                            None,
                            completion.usage,
                        )
                        .await;
                        self.append_reply(
                            conversation,
                            completion.content,
                            &cfg.model,
                            latency,
                            completion.usage,
                            injection_meta.clone(),
                        )
                        .await?;
                        Ok(())
                    }
                    Err(err) => {
                        let message = err.to_string();
                        call_record::record_call(
                            &self.pool,
                            conversation.project_id,
                            cfg.provider_id,
                            &cfg.model,
                            PURPOSE_CONVERSATION_REPLY,
                            "failed",
                            Some(call_record::analysis_error_kind(&err)),
                            Some(message.as_str()),
                            latency,
                            None,
                            None,
                        )
                        .await;
                        Err(ReplyFailure::Model(err))
                    }
                }
            }
            crate::analysis_model::ModelSelection::ConfigurationFailed { reason, detail } => {
                tracing::warn!(
                    conversation_id = %conversation.id,
                    reason,
                    %detail,
                    "LLM model resolution returned an explicit configuration failure for this conversation"
                );
                Err(ReplyFailure::NoProvider {
                    reason: "no_model_for_role",
                })
            }
            crate::analysis_model::ModelSelection::ResolutionFailed(error) => {
                tracing::warn!(
                    conversation_id = %conversation.id,
                    error = %error,
                    "LLM model resolution failed for this conversation"
                );
                Err(ReplyFailure::NoProvider {
                    reason: "resolution_failed",
                })
            }
        }
    }

    /// Last [`CONTEXT_WINDOW`] user/assistant turns, time-ascending
    /// (system rows are storage-only and never enter the model context).
    async fn load_history(
        &self,
        conversation_id: Uuid,
    ) -> Result<Vec<(String, String)>, sqlx::Error> {
        let rows = sqlx::query_as::<_, (String, String)>(
            "SELECT role, content FROM ( \
                 SELECT role, content, created_at, id FROM ai.conversation_message \
                 WHERE conversation_id = $1 AND role IN ('user', 'assistant') \
                 ORDER BY created_at DESC, id DESC LIMIT $2 \
             ) recent \
             ORDER BY created_at ASC, id ASC",
        )
        .bind(conversation_id)
        .bind(i64::try_from(CONTEXT_WINDOW).unwrap_or(i64::MAX))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    fn build_messages(
        history: &[(String, String)],
        injected: Option<&str>,
    ) -> Vec<ConversationTurn> {
        let mut messages = Vec::with_capacity(history.len() + 2);
        messages.push(ConversationTurn::new(
            "system",
            conversation_model::CONVERSATION_SYSTEM_PROMPT,
        ));
        // 注入作为**第二个 system 轮**紧跟基础系统提示：`<context>` 内一律
        // 按数据对待（防注入），标题即来源标注——事实引用须能落到某节。
        if let Some(context) = injected {
            messages.push(ConversationTurn::new("system", context));
        }
        messages.extend(
            history
                .iter()
                .map(|(role, content)| ConversationTurn::new(role, content.clone())),
        );
        messages
    }

    /// 构造 chat 注入（Batch 3A）。
    ///
    /// 返回 `(注入正文, 记账载荷)`：无锚点 / 查不到项目 → `(None, Null)`——
    /// 调用方据此**完全跳过注入**（旧行为不变）。
    async fn build_injection(
        &self,
        conversation: &PendingConversation,
    ) -> Result<(Option<String>, serde_json::Value), sqlx::Error> {
        let repository =
            crate::chat_context::anchor_repository(conversation.context_anchor.as_ref());
        // B3 / CX2：会话上下文条目（attached + active）→ 对象节（S3+）。
        // 投影层重读对象实况（SQL-07）；无条目 = 空切片，行为与 B2 前一致。
        let objects = pm_application::ai_context_items::load_injection_sources(
            &self.pool,
            conversation.tenant_id,
            conversation.project_id,
            conversation.id,
        )
        .await
        .unwrap_or_default();
        let context = crate::chat_context::build_chat_context_with_objects(
            &self.pool,
            conversation.tenant_id,
            conversation.project_id,
            repository.as_deref(),
            &objects,
        )
        .await?;
        if context.is_empty() {
            return Ok((None, serde_json::Value::Null));
        }
        Ok((Some(context.render()), context.metadata()))
    }

    /// Append the assistant turn. Content is defensively clamped to the
    /// storage cap (`MAX_CONTENT_CHARS`): a pathological oversized reply
    /// must fail closed here rather than retry-loop a billable model call
    /// forever (the clamp only trims an absurd outlier, never normal text).
    /// `usage` (Batch H, when the provider reported it) adds `total_tokens`
    /// to the observation metadata (the 0041 record table has no
    /// total_tokens column, so the metadata copy is the only total).
    async fn append_reply(
        &self,
        conversation: &PendingConversation,
        content: String,
        model_name: &str,
        latency_ms: i64,
        usage: Option<conversation_model::Usage>,
        injection_meta: serde_json::Value,
    ) -> Result<(), pm_application::ai_conversations::ConversationError> {
        let clamped: String = content
            .chars()
            .take(pm_application::ai_conversations::MAX_CONTENT_CHARS)
            .collect();
        // 注入记账（D2 可观测性）挂在 assistant 消息的 metadata 上——
        // `ai.llm_call_record` 无 metadata 列（DK-C 已登记该缺口），
        // 而消息 metadata 列已在（0060），零迁移。
        let mut metadata = match usage {
            Some(u) => serde_json::json!({
                "model": model_name,
                "latency_ms": latency_ms,
                "total_tokens": u.total_tokens,
            }),
            None => serde_json::json!({
                "model": model_name,
                "latency_ms": latency_ms,
            }),
        };
        if !injection_meta.is_null() {
            metadata["injection"] = injection_meta;
        }
        pm_application::ai_conversations::append_message(
            &self.pool,
            conversation.tenant_id,
            conversation.project_id,
            conversation.principal_id,
            conversation.id,
            "assistant",
            &clamped,
            "markdown",
            metadata,
        )
        .await
        .map(|_| ())
    }
}

/// Round-step failures. `NoProvider` carries the structured warn reason.
enum ReplyFailure {
    NoProvider { reason: &'static str },
    Model(ModelError),
    Db(pm_application::ai_conversations::ConversationError),
}

impl From<sqlx::Error> for ReplyFailure {
    fn from(e: sqlx::Error) -> Self {
        ReplyFailure::Db(pm_application::ai_conversations::ConversationError::Db(
            e.to_string(),
        ))
    }
}

impl From<pm_application::ai_conversations::ConversationError> for ReplyFailure {
    fn from(e: pm_application::ai_conversations::ConversationError) -> Self {
        ReplyFailure::Db(e)
    }
}

impl std::fmt::Display for ReplyFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplyFailure::NoProvider { reason } => write!(f, "no provider ({reason})"),
            ReplyFailure::Model(err) => write!(f, "model call failed: {err}"),
            ReplyFailure::Db(err) => write!(f, "persistence failed: {err}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pm_domain::SYSTEM_TENANT;
    use serde_json::Value;
    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Bootstrap-equivalent rows: system-tenant customer + project
    /// (same shape as `analysis_model::tests`).
    async fn seed_project(pool: &sqlx::PgPool) -> Uuid {
        let customer_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();
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

    /// An enabled tenant provider (`openai_compatible` needs no credential
    /// row — anonymous endpoints are allowed).
    async fn seed_enabled_provider(pool: &sqlx::PgPool, endpoint: &str, model: &str) -> Uuid {
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
             VALUES ($1, $2, 'seed-provider', 'openai_compatible', $3, $4, 'enabled', $5)",
        )
        .bind(provider_id)
        .bind(SYSTEM_TENANT)
        .bind(endpoint)
        .bind(serde_json::json!({ "model": model }))
        .bind(principal_id)
        .execute(pool)
        .await
        .unwrap();
        // Phase 3B：解析链只认目录面——补目录模型 + 授权 + general 策略
        crate::test_seed::seed_model_catalog(pool, SYSTEM_TENANT, provider_id, model, principal_id)
            .await;
        provider_id
    }

    /// Conversation owner principal (the loop appends with the owner's
    /// identity stored on the conversation row).
    async fn seed_owner(pool: &sqlx::PgPool) -> Uuid {
        let owner = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO core.principal (id, kind, display_name) VALUES ($1, 'ai_pm', 'Owner')",
        )
        .bind(owner)
        .execute(pool)
        .await
        .unwrap();
        owner
    }

    async fn seed_conversation(pool: &sqlx::PgPool, project_id: Uuid, owner: Uuid) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO ai.conversation (id, tenant_id, project_id, principal_id, title) \
             VALUES ($1, $2, $3, $4, 'seed')",
        )
        .bind(id)
        .bind(SYSTEM_TENANT)
        .bind(project_id)
        .bind(owner)
        .execute(pool)
        .await
        .unwrap();
        id
    }

    async fn seed_message(
        pool: &sqlx::PgPool,
        conversation_id: Uuid,
        role: &str,
        content: &str,
    ) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO ai.conversation_message (id, conversation_id, role, content) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(id)
        .bind(conversation_id)
        .bind(role)
        .bind(content)
        .execute(pool)
        .await
        .unwrap();
        id
    }

    async fn message_count(pool: &sqlx::PgPool, conversation_id: Uuid, role: &str) -> i64 {
        sqlx::query_scalar(
            "SELECT count(*) FROM ai.conversation_message \
             WHERE conversation_id = $1 AND role = $2",
        )
        .bind(conversation_id)
        .bind(role)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    fn ok_reply(content: &str) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "choices": [{"message": {"content": content}}]
        }))
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn round_appends_assistant_reply_with_metadata_and_record(pool: sqlx::PgPool) {
        let project_id = seed_project(&pool).await;
        let owner = seed_owner(&pool).await;
        let conversation_id = seed_conversation(&pool, project_id, owner).await;
        seed_message(&pool, conversation_id, "user", "哪个任务风险最高？").await;

        let server = MockServer::start().await;
        let provider_id = seed_enabled_provider(&pool, &server.uri(), "test-model").await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ok_reply("**任务 A** 风险最高。"))
            .expect(1)
            .mount(&server)
            .await;

        let mut worker = ConversationReplyWorker::new(pool.clone());
        let report = worker.reply_round().await.unwrap();
        assert_eq!(
            report,
            ReplyReport {
                replied: 1,
                failed: 0,
                skipped_no_provider: 0
            }
        );

        // Assistant message landed with markdown content + observation metadata.
        let row: (String, String, Value) = sqlx::query_as(
            "SELECT role, content, metadata FROM ai.conversation_message \
             WHERE conversation_id = $1 AND role = 'assistant'",
        )
        .bind(conversation_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.0, "assistant");
        assert_eq!(row.1, "**任务 A** 风险最高。");
        assert_eq!(row.2["model"], "test-model");
        assert!(row.2["latency_ms"].is_i64(), "latency must be recorded");

        // Call record: purpose=conversation_reply, ok, provider linked,
        // analysis_run_id NULL (no analysis run backs a chat turn). The
        // mock reply carries no `usage` object → token columns stay NULL
        // (Batch H: absent usage must never fabricate zeros).
        let record: (String, String, Uuid, Option<Uuid>, Option<i64>, Option<i64>) =
            sqlx::query_as(
                "SELECT purpose, status, provider_id, analysis_run_id, \
                        prompt_tokens, completion_tokens \
                 FROM ai.llm_call_record WHERE project_id = $1",
            )
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(record.0, "conversation_reply");
        assert_eq!(record.1, "ok");
        assert_eq!(record.2, provider_id);
        assert!(record.3.is_none());
        assert!(record.4.is_none(), "no usage → prompt_tokens NULL");
        assert!(record.5.is_none(), "no usage → completion_tokens NULL");

        // The model context: system prompt first, then the injected context
        // (Batch 3A), then the user turn.
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 3, "system + injection + 1 user turn");
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[1]["role"], "system");
        assert_eq!(messages[2]["role"], "user");
        assert_eq!(messages[2]["content"], "哪个任务风险最高？");

        // Replied conversation left the scan set naturally.
        let report = worker.reply_round().await.unwrap();
        assert_eq!(report, ReplyReport::default());
    }

    /// Batch H: a provider reply carrying the chat-completions `usage`
    /// object → prompt/completion token columns land on
    /// `ai.llm_call_record`, and `total_tokens` rides in the assistant
    /// message metadata (0041 has no total_tokens column).
    #[sqlx::test(migrations = "../../migrations")]
    async fn usage_from_provider_is_persisted_to_record_and_metadata(pool: sqlx::PgPool) {
        let project_id = seed_project(&pool).await;
        let owner = seed_owner(&pool).await;
        let conversation_id = seed_conversation(&pool, project_id, owner).await;
        seed_message(&pool, conversation_id, "user", "hi").await;

        let server = MockServer::start().await;
        seed_enabled_provider(&pool, &server.uri(), "test-model").await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": "reply with usage"}}],
                "usage": {"prompt_tokens": 111, "completion_tokens": 22, "total_tokens": 133}
            })))
            .mount(&server)
            .await;

        let mut worker = ConversationReplyWorker::new(pool.clone());
        let report = worker.reply_round().await.unwrap();
        assert_eq!(report.replied, 1);

        let (prompt, completion): (Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT prompt_tokens, completion_tokens FROM ai.llm_call_record \
             WHERE project_id = $1 AND purpose = 'conversation_reply'",
        )
        .bind(project_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(prompt, Some(111));
        assert_eq!(completion, Some(22));

        let (metadata,): (Value,) = sqlx::query_as(
            "SELECT metadata FROM ai.conversation_message \
             WHERE conversation_id = $1 AND role = 'assistant'",
        )
        .bind(conversation_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(metadata["model"], "test-model");
        assert_eq!(metadata["total_tokens"], 133);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn no_provider_skips_without_reply_or_record(pool: sqlx::PgPool) {
        let project_id = seed_project(&pool).await;
        let owner = seed_owner(&pool).await;
        let conversation_id = seed_conversation(&pool, project_id, owner).await;
        seed_message(&pool, conversation_id, "user", "hello").await;

        let mut worker = ConversationReplyWorker::new(pool.clone());
        let report = worker.reply_round().await.unwrap();
        assert_eq!(
            report,
            ReplyReport {
                replied: 0,
                failed: 0,
                skipped_no_provider: 1
            }
        );
        assert_eq!(message_count(&pool, conversation_id, "assistant").await, 0);
        let records: i64 = sqlx::query_scalar("SELECT count(*) FROM ai.llm_call_record")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(records, 0, "no provider → no call record");

        // Cooldown: the second round must not reprocess the same head entry
        // (starvation guard), and the failure stays visible in the report
        // only on the round that actually attempted it.
        let report = worker.reply_round().await.unwrap();
        assert_eq!(report, ReplyReport::default());
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn upstream_failure_records_failed_call_without_reply(pool: sqlx::PgPool) {
        let project_id = seed_project(&pool).await;
        let owner = seed_owner(&pool).await;
        let conversation_id = seed_conversation(&pool, project_id, owner).await;
        seed_message(&pool, conversation_id, "user", "hi").await;

        let server = MockServer::start().await;
        seed_enabled_provider(&pool, &server.uri(), "test-model").await;
        // 500 ×2 = the adapter's single retry, then give up.
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(500))
            .expect(2)
            .mount(&server)
            .await;

        let mut worker = ConversationReplyWorker::new(pool.clone());
        let report = worker.reply_round().await.unwrap();
        assert_eq!(report.replied, 0);
        assert_eq!(report.failed, 1);

        assert_eq!(message_count(&pool, conversation_id, "assistant").await, 0);
        let record: (String, String, Option<String>) = sqlx::query_as(
            "SELECT purpose, status, error_kind FROM ai.llm_call_record \
             WHERE project_id = $1",
        )
        .bind(project_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(record.0, "conversation_reply");
        assert_eq!(record.1, "failed");
        assert_eq!(record.2.as_deref(), Some("model_unavailable"));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn scan_ignores_empty_and_assistant_latest_conversations(pool: sqlx::PgPool) {
        let project_id = seed_project(&pool).await;
        let owner = seed_owner(&pool).await;
        let server = MockServer::start().await;
        seed_enabled_provider(&pool, &server.uri(), "test-model").await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ok_reply("nope"))
            .expect(0)
            .mount(&server)
            .await;

        // Empty conversation (no messages at all).
        seed_conversation(&pool, project_id, owner).await;
        // Latest message is assistant (already answered).
        let answered = seed_conversation(&pool, project_id, owner).await;
        seed_message(&pool, answered, "user", "earlier").await;
        seed_message(&pool, answered, "assistant", "answered").await;
        // Latest is assistant even though an older user message exists.
        let mixed = seed_conversation(&pool, project_id, owner).await;
        seed_message(&pool, mixed, "user", "q1").await;
        seed_message(&pool, mixed, "assistant", "a1").await;
        seed_message(&pool, mixed, "user", "q2").await;
        seed_message(&pool, mixed, "assistant", "a2").await;

        let mut worker = ConversationReplyWorker::new(pool.clone());
        let report = worker.reply_round().await.unwrap();
        assert_eq!(report, ReplyReport::default(), "nothing pending");
        let requests = server.received_requests().await.unwrap();
        assert!(requests.is_empty(), "no model call may be made");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn multiple_conversations_are_processed_fifo_oldest_first(pool: sqlx::PgPool) {
        let project_id = seed_project(&pool).await;
        let owner = seed_owner(&pool).await;
        let server = MockServer::start().await;
        seed_enabled_provider(&pool, &server.uri(), "test-model").await;

        // Three pending conversations; seeding order makes the *newest*
        // conversation c_new carry the OLDEST pending message? No —
        // created_at is transaction time, so insert oldest first, but
        // force order via explicit created_at instead of relying on clock.
        let old = seed_conversation(&pool, project_id, owner).await;
        let mid = seed_conversation(&pool, project_id, owner).await;
        let new = seed_conversation(&pool, project_id, owner).await;
        for (conversation_id, age_secs) in [(old, 300i64), (mid, 200), (new, 100)] {
            sqlx::query(
                "INSERT INTO ai.conversation_message (conversation_id, role, content, created_at) \
                 VALUES ($1, 'user', 'q', now() - make_interval(secs => $2))",
            )
            .bind(conversation_id)
            .bind(age_secs)
            .execute(&pool)
            .await
            .unwrap();
        }

        // Mount order pins FIFO: request 1 → "reply-old", request 2 →
        // "reply-mid", request 3 → "reply-new".
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ok_reply("reply-old"))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ok_reply("reply-mid"))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ok_reply("reply-new"))
            .mount(&server)
            .await;

        let mut worker = ConversationReplyWorker::new(pool.clone());
        let report = worker.reply_round().await.unwrap();
        assert_eq!(report.replied, 3);

        let reply_for = |conversation_id: Uuid| {
            let pool = pool.clone();
            async move {
                let (content,): (String,) = sqlx::query_as(
                    "SELECT content FROM ai.conversation_message \
                     WHERE conversation_id = $1 AND role = 'assistant'",
                )
                .bind(conversation_id)
                .fetch_one(&pool)
                .await
                .unwrap();
                content
            }
        };
        assert_eq!(reply_for(old).await, "reply-old");
        assert_eq!(reply_for(mid).await, "reply-mid");
        assert_eq!(reply_for(new).await, "reply-new");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn context_window_keeps_only_the_last_20_turns(pool: sqlx::PgPool) {
        let project_id = seed_project(&pool).await;
        let owner = seed_owner(&pool).await;
        let conversation_id = seed_conversation(&pool, project_id, owner).await;
        // 25 user turns; the window must keep the LAST 20.
        for i in 0..25 {
            seed_message(&pool, conversation_id, "user", &format!("turn-{i:02}")).await;
        }

        let server = MockServer::start().await;
        seed_enabled_provider(&pool, &server.uri(), "test-model").await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ok_reply("ok"))
            .mount(&server)
            .await;

        let mut worker = ConversationReplyWorker::new(pool.clone());
        worker.reply_round().await.unwrap();

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
        let messages = body["messages"].as_array().unwrap();
        // Batch 3A：注入作为第二个 system 轮（紧跟基础系统提示），
        // 故轮数 = system + 注入 + 20-turn 窗口。
        assert_eq!(messages.len(), 22, "system + injection + 20-turn window");
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[1]["role"], "system");
        assert!(
            messages[1]["content"]
                .as_str()
                .unwrap_or_default()
                .contains("项目画像"),
            "second system turn carries the injected context"
        );
        // Dropped: turn-00..turn-04; kept: turn-05..turn-24, ascending.
        assert_eq!(messages[2]["content"], "turn-05");
        assert_eq!(messages[21]["content"], "turn-24");
    }

    /// The system prompt must instruct the model to mirror the user's
    /// language (Batch G prompt contract) — pinned so prompt edits that
    /// silently drop the language rule fail here.
    #[test]
    fn system_prompt_follows_user_language() {
        assert!(conversation_model::CONVERSATION_SYSTEM_PROMPT.contains("语言"));
    }

    /// `AI_PM_CONVERSATION_REPLY` parsing: default off, typo stays off
    /// (a billable producer must never be enabled by accident).
    #[test]
    fn reply_mode_defaults_off_and_typo_stays_off() {
        assert_eq!(
            ConversationReplyMode::parse(None),
            ConversationReplyMode::Off
        );
        assert_eq!(
            ConversationReplyMode::parse(Some("off")),
            ConversationReplyMode::Off
        );
        assert_eq!(
            ConversationReplyMode::parse(Some("on")),
            ConversationReplyMode::On
        );
        assert_eq!(
            ConversationReplyMode::parse(Some("ture")),
            ConversationReplyMode::Off
        );
    }

    /// The context builder maps history rows onto wire turns in order,
    /// system prompt first.
    #[test]
    fn build_messages_maps_history_in_order() {
        let history = vec![
            ("user".to_string(), "q".to_string()),
            ("assistant".to_string(), "a".to_string()),
        ];
        let messages = ConversationReplyWorker::build_messages(&history, None);
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1], ConversationTurn::new("user", "q"));
        assert_eq!(messages[2], ConversationTurn::new("assistant", "a"));
    }

    /// Batch 3A：注入作为第二个 system 轮紧跟基础系统提示；
    /// 无注入时轮次与位置与注入前逐字节一致（向后兼容）。
    #[test]
    fn build_messages_inserts_injection_as_second_system_turn() {
        let history = vec![("user".to_string(), "q".to_string())];
        let messages =
            ConversationReplyWorker::build_messages(&history, Some("<context>项目画像</context>"));
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].role, "system");
        assert_eq!(messages[1].content, "<context>项目画像</context>");
        assert_eq!(messages[2], ConversationTurn::new("user", "q"));
    }

    /// body_partial_json is exercised in one test to prove the wire payload
    /// carries the fixed temperature (prompt-tuning observability).
    #[sqlx::test(migrations = "../../migrations")]
    async fn wire_payload_carries_fixed_temperature_and_model(pool: sqlx::PgPool) {
        let project_id = seed_project(&pool).await;
        let owner = seed_owner(&pool).await;
        let conversation_id = seed_conversation(&pool, project_id, owner).await;
        seed_message(&pool, conversation_id, "user", "hi").await;

        let server = MockServer::start().await;
        seed_enabled_provider(&pool, &server.uri(), "test-model").await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(body_partial_json(serde_json::json!({
                "model": "test-model",
                "temperature": conversation_model::TEMPERATURE
            })))
            .respond_with(ok_reply("ok"))
            .mount(&server)
            .await;

        let mut worker = ConversationReplyWorker::new(pool.clone());
        let report = worker.reply_round().await.unwrap();
        assert_eq!(report.replied, 1, "payload matcher must have matched");
    }

    /// REAL DeepSeek end-to-end round (ignored by default — burns tokens and
    /// needs network; run manually with `cargo test -p pm-ai -- --ignored`).
    ///
    /// Gating discipline: the API key is read **only** from the
    /// `DEEPSEEK_API_KEY` environment variable, injected by the operator at
    /// test time; it must never appear in source, logs or reports. The
    /// credential row is created via the same AES-256-GCM encrypt path the
    /// REST layer uses (`AI_PM_SECRET_KEY` pinned to a test-only value inside
    /// this test), so the decrypt path in `resolve_repo_call_config` is
    /// exercised for real.
    #[sqlx::test(migrations = "../../migrations")]
    #[ignore = "real DeepSeek call: needs DEEPSEEK_API_KEY injected at run time (costs tokens)"]
    async fn real_deepseek_replies_in_conversation(pool: sqlx::PgPool) {
        let Some(api_key) = std::env::var("DEEPSEEK_API_KEY")
            .ok()
            .filter(|k| !k.trim().is_empty())
        else {
            panic!("DEEPSEEK_API_KEY must be injected by the operator to run this test");
        };
        // Cipher key for the credential row (test-only value). The guard
        // restores the operator's env even when an assertion panics.
        struct SecretGuard(Option<String>);
        impl Drop for SecretGuard {
            fn drop(&mut self) {
                match self.0.take() {
                    Some(v) => std::env::set_var("AI_PM_SECRET_KEY", v),
                    None => std::env::remove_var("AI_PM_SECRET_KEY"),
                }
            }
        }
        let _guard = SecretGuard(std::env::var("AI_PM_SECRET_KEY").ok());
        std::env::set_var("AI_PM_SECRET_KEY", "batch-g-real-test-secret");
        real_deepseek_round(&pool, &api_key)
            .await
            .expect("real DeepSeek round must succeed");
    }

    async fn real_deepseek_round(
        pool: &sqlx::PgPool,
        api_key: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use pm_application::team_llm::encrypt_api_key;

        let project_id = seed_project(pool).await;
        let owner = seed_owner(pool).await;
        let conversation_id = seed_conversation(pool, project_id, owner).await;
        seed_message(
            pool,
            conversation_id,
            "user",
            "用一句话介绍这个平台是什么。",
        )
        .await;

        // Real provider row, same schema the REST/team-LLM layer writes
        // (openai_compatible + encrypted credential + enabled).
        let provider_principal = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO core.principal (id, kind, display_name) VALUES ($1, 'ai_pm', 'Seed')",
        )
        .bind(provider_principal)
        .execute(pool)
        .await
        .unwrap();
        let provider_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO ai.team_llm_provider \
             (id, tenant_id, name, provider_type, endpoint, configuration, status, created_by) \
             VALUES ($1, $2, 'deepseek', 'openai_compatible', 'https://api.deepseek.com', \
                     '{\"model\": \"deepseek-chat\"}', 'enabled', $3) RETURNING id",
        )
        .bind(provider_id)
        .bind(SYSTEM_TENANT)
        .bind(provider_principal)
        .fetch_one(pool)
        .await?;
        let (ciphertext, hint) = encrypt_api_key(api_key)?;
        let credential_id: Uuid = sqlx::query_scalar(
            "INSERT INTO ai.team_llm_credential \
             (tenant_id, provider_id, api_key_ciphertext, api_key_hint, created_by) \
             VALUES ($1, $2, $3, $4, $5) RETURNING id",
        )
        .bind(SYSTEM_TENANT)
        .bind(provider_id)
        .bind(&ciphertext)
        .bind(&hint)
        .bind(provider_principal)
        .fetch_one(pool)
        .await?;
        // The resolver joins on provider.credential_id (attach_credential
        // contract) — the insert alone is not enough.
        sqlx::query("UPDATE ai.team_llm_provider SET credential_id = $2 WHERE id = $1")
            .bind(provider_id)
            .bind(credential_id)
            .execute(pool)
            .await?;
        // Phase 3B：解析链只认目录面——补目录模型 + 授权 + general 策略
        crate::test_seed::seed_model_catalog(
            pool,
            SYSTEM_TENANT,
            provider_id,
            "deepseek-chat",
            provider_principal,
        )
        .await;

        // One full round against the real API (generous timeout for a cold
        // first call; DeepSeek chat completions typically answer in seconds).
        let mut worker = ConversationReplyWorker::new(pool.clone());
        let report = worker.reply_round().await?;
        if report.replied != 1 {
            // Diagnostic (never carries the key — bearer headers are not logged).
            let record: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
                "SELECT status, error_kind, error_message FROM ai.llm_call_record \
                 WHERE project_id = $1",
            )
            .bind(project_id)
            .fetch_optional(pool)
            .await?;
            panic!("real round report {report:?}; call record: {record:?}");
        }

        let (content, metadata): (String, Value) = sqlx::query_as(
            "SELECT content, metadata FROM ai.conversation_message \
             WHERE conversation_id = $1 AND role = 'assistant'",
        )
        .bind(conversation_id)
        .fetch_one(pool)
        .await?;
        assert!(
            !content.trim().is_empty(),
            "assistant content must be non-empty"
        );
        assert_eq!(metadata["model"], "deepseek-chat");
        println!(
            "real DeepSeek reply ({} chars): {}",
            content.chars().count(),
            content
        );

        let (status, latency): (String, Option<i64>) = sqlx::query_as(
            "SELECT status, latency_ms FROM ai.llm_call_record \
             WHERE project_id = $1 AND purpose = 'conversation_reply'",
        )
        .bind(project_id)
        .fetch_one(pool)
        .await?;
        assert_eq!(status, "ok");
        assert!(latency.is_some());

        // Batch H: real DeepSeek chat completions always carry a `usage`
        // object → the token columns must be non-NULL on the record and the
        // total must ride in the assistant metadata.
        let (prompt_tokens, completion_tokens): (Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT prompt_tokens, completion_tokens FROM ai.llm_call_record \
             WHERE project_id = $1 AND purpose = 'conversation_reply'",
        )
        .bind(project_id)
        .fetch_one(pool)
        .await?;
        assert!(
            prompt_tokens.is_some() && completion_tokens.is_some(),
            "real DeepSeek usage must fill the token columns \
             (prompt={prompt_tokens:?}, completion={completion_tokens:?})"
        );
        println!("real DeepSeek usage: prompt={prompt_tokens:?}, completion={completion_tokens:?}");
        assert!(
            metadata["total_tokens"].is_i64(),
            "real DeepSeek usage must surface total_tokens in metadata"
        );
        Ok(())
    }
}
