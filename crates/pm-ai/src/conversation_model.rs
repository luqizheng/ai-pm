//! Batch G: the lightweight conversation-reply model port (D-4 producer side).
//!
//! Unlike the analysis chain ([`crate::openai_adapter`]) this port returns
//! **free text** — no JSON contract, no `response_format`, no schema
//! validation. The reply lands verbatim in `ai.conversation_message`
//! (role=`assistant`), so the only quality gate is the non-empty check.
//!
//! Error mapping mirrors the analysis adapter (same [`ModelError`] taxonomy)
//! with one deliberate difference: HTTP 5xx and client timeouts are retried
//! **once** right here, because the conversation loop has no job-queue
//! backoff layer to lean on — a transient blip must not leave the user
//! without a reply for a whole poll cycle. 4xx and unusable output fail
//! immediately (retrying cannot help).

use crate::model::ModelError;
use serde::Deserialize;

/// System prompt for the project assistant reply chain. Content lives in a
/// `pub const` so it is reviewable and tunable without touching logic (same
/// convention as `openai_adapter::SYSTEM_PROMPT`).
pub const CONVERSATION_SYSTEM_PROMPT: &str = r#"你是 AI-PM 平台的项目 AI 助手，与用户就其项目展开对话。

要求：
- 只依据会话上下文中给出的事实回答；上下文没有的信息如实说明，不要编造。
- 回答简洁直接，避免冗长铺垫；必要时可用简单的 markdown（列表、代码块）组织内容。
- 跟随用户消息所用的语言：用户用中文就用中文回答，用户用英文就用英文回答。
- 输出普通文本，不要输出 JSON 或其他结构化包装。"#;

/// How many recent user/assistant turns feed the model (Batch G ruling:
/// last-20 window, time-ascending, system rows excluded).
pub const CONTEXT_WINDOW: usize = 20;

/// Fixed sampling temperature. Chosen deliberately conservative (0.3):
/// project-assistant answers should stay close to the conversation facts
/// (low hallucination surface) while keeping a little natural phrasing;
/// 0 would make repeated questions read verbatim-identical.
pub const TEMPERATURE: f64 = 0.3;

/// One chat turn handed to [`complete`]. `role` is the chat-completions
/// wire role (`system` / `user` / `assistant`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationTurn {
    pub role: String,
    pub content: String,
}

impl ConversationTurn {
    pub fn new(role: &str, content: impl Into<String>) -> Self {
        Self {
            role: role.to_string(),
            content: content.into(),
        }
    }
}

/// Token accounting reported by the provider in the chat-completions
/// `usage` object (Batch H: the conversation chain persists it to
/// `ai.llm_call_record`; the analysis/repo chains keep their columns NULL).
/// Per-field serde defaults tolerate partial usage objects (a provider
/// reporting only some counters must not fail the reply).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub prompt_tokens: i64,
    #[serde(default)]
    pub completion_tokens: i64,
    #[serde(default)]
    pub total_tokens: i64,
}

/// One successful completion: the free-text reply plus the provider's
/// optional token `usage` (missing → `None`, never an error — providers
/// without usage accounting must keep working).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationCompletion {
    pub content: String,
    pub usage: Option<Usage>,
}

/// The subset of the chat-completions response this port consumes.
#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
    /// Absent or `null` in some providers → `None` (with `#[serde(default)]`
    /// a missing key is fine; per-field defaults tolerate partial objects).
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Deserialize)]
struct Message {
    content: String,
}

/// One completion call: `messages` mapped verbatim onto the wire format.
/// Retries 5xx/timeout once; never logs or returns the API key.
pub async fn complete(
    endpoint: &str,
    api_key: &str,
    model: &str,
    timeout: chrono::Duration,
    messages: &[ConversationTurn],
) -> Result<ConversationCompletion, ModelError> {
    // Attempt 1 + one retry for 5xx / timeout only (see module doc).
    let mut last_error = None;
    for _ in 0..2 {
        match complete_raw(endpoint, api_key, model, timeout, messages, TEMPERATURE).await {
            Ok(completion) => return Ok(completion),
            // 5xx 与超时可重试；其他传输失败通常是确定性的（DNS、拒连）
            // ——快速失败。
            Err(err @ (ModelError::Timeout | ModelError::Unavailable(_)))
                if matches!(&err, ModelError::Unavailable(r) if r.contains("upstream returned HTTP"))
                    || matches!(err, ModelError::Timeout) =>
            {
                last_error = Some(err);
                continue;
            }
            Err(err) => return Err(err),
        }
    }
    Err(last_error.unwrap_or(ModelError::Timeout))
}

/// Single-shot chat-completions call shared by every role adapter (批 D 单源：
/// repo/snapshot 两个手写残留迁移后，全 crate 只剩这一个 HTTP 端点)。
///
/// `temperature` 由调用方给定（确定性任务 0，会话任务 [`TEMPERATURE`]）；
/// 无重试——重试语义属于 [`complete`]。
pub async fn complete_raw(
    endpoint: &str,
    api_key: &str,
    model: &str,
    timeout: chrono::Duration,
    messages: &[ConversationTurn],
    temperature: f64,
) -> Result<ConversationCompletion, ModelError> {
    // Same client posture as the analysis adapter: no_proxy so a stale
    // OS proxy cannot hijack provider traffic; timeout bounds the whole
    // request (connect + headers + body).
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(timeout.to_std().expect("timeout must be positive"))
        .build()
        .expect("reqwest client must build");
    let base_url = endpoint.trim_end_matches('/');
    let body = serde_json::json!({
        "model": model,
        "temperature": temperature,
        "messages": messages
            .iter()
            .map(|turn| serde_json::json!({"role": turn.role, "content": turn.content}))
            .collect::<Vec<_>>(),
    });

    let response = client
        .post(format!("{base_url}/chat/completions"))
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|err| {
            if err.is_timeout() {
                ModelError::Timeout
            } else if err.is_connect() {
                ModelError::Unavailable(format!("cannot reach model endpoint: {err}"))
            } else {
                ModelError::Unavailable(format!("model request failed: {err}"))
            }
        })?;

    let status = response.status();
    if status.is_server_error() {
        return Err(ModelError::Unavailable(format!(
            "upstream returned HTTP {status}"
        )));
    }
    if !status.is_success() {
        // 4xx: caller misconfiguration (bad key, wrong model). Retrying
        // cannot help, so it must not read as Unavailable.
        let text = response.text().await.unwrap_or_default();
        return Err(ModelError::BadOutput(format!(
            "upstream rejected request with HTTP {status}: {}",
            text.chars().take(200).collect::<String>()
        )));
    }

    let completion: ChatCompletionResponse = response
        .json()
        .await
        .map_err(|e| ModelError::BadOutput(format!("unexpected response envelope: {e}")))?;
    let content = completion
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| ModelError::BadOutput("response contained no choices".to_string()))?
        .message
        .content;
    // Free-text contract: the only quality gate is non-emptiness. An
    // empty reply would violate the append validation downstream anyway,
    // so it is rejected here where it can be recorded as provider error.
    if content.trim().is_empty() {
        return Err(ModelError::BadOutput(
            "model returned an empty reply".to_string(),
        ));
    }
    Ok(ConversationCompletion {
        content,
        usage: completion.usage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn turns() -> Vec<ConversationTurn> {
        vec![
            ConversationTurn::new("system", "sys"),
            ConversationTurn::new("user", "hello"),
        ]
    }

    #[tokio::test]
    async fn returns_free_text_without_json_parsing() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": "plain **markdown** reply"}}]
            })))
            .mount(&server)
            .await;
        let completion = complete(
            &server.uri(),
            "key",
            "m",
            chrono::Duration::seconds(5),
            &turns(),
        )
        .await
        .unwrap();
        assert_eq!(completion.content, "plain **markdown** reply");
        // No usage object in the envelope → None (never an error).
        assert_eq!(completion.usage, None);
    }

    /// Batch H: the provider's `usage` object is surfaced on the completion
    /// (conversation chain persists prompt/completion tokens to
    /// `ai.llm_call_record`).
    #[tokio::test]
    async fn usage_object_is_surfaced_when_present() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": "reply"}}],
                "usage": {"prompt_tokens": 12, "completion_tokens": 34, "total_tokens": 46}
            })))
            .mount(&server)
            .await;
        let completion = complete(
            &server.uri(),
            "key",
            "m",
            chrono::Duration::seconds(5),
            &turns(),
        )
        .await
        .unwrap();
        assert_eq!(
            completion.usage,
            Some(Usage {
                prompt_tokens: 12,
                completion_tokens: 34,
                total_tokens: 46
            })
        );
    }

    /// A `null` usage (some providers omit accounting entirely) is None, not
    /// an envelope error; the non-usage reply shape keeps working.
    #[tokio::test]
    async fn null_usage_is_none_not_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": "reply"}}],
                "usage": null
            })))
            .mount(&server)
            .await;
        let completion = complete(
            &server.uri(),
            "key",
            "m",
            chrono::Duration::seconds(5),
            &turns(),
        )
        .await
        .unwrap();
        assert_eq!(completion.content, "reply");
        assert_eq!(completion.usage, None);
    }

    #[tokio::test]
    async fn empty_content_is_bad_output() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": "  "}}]
            })))
            .expect(1)
            .mount(&server)
            .await;
        let err = complete(
            &server.uri(),
            "key",
            "m",
            chrono::Duration::seconds(5),
            &turns(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ModelError::BadOutput(_)), "{err:?}");
    }

    #[tokio::test]
    async fn server_error_is_retried_once_then_unavailable() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(500))
            .expect(2)
            .mount(&server)
            .await;
        let err = complete(
            &server.uri(),
            "key",
            "m",
            chrono::Duration::seconds(5),
            &turns(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ModelError::Unavailable(_)), "{err:?}");
    }

    #[tokio::test]
    async fn client_error_is_not_retried() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": {"message": "bad key"}
            })))
            .expect(1)
            .mount(&server)
            .await;
        let err = complete(
            &server.uri(),
            "key",
            "m",
            chrono::Duration::seconds(5),
            &turns(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ModelError::BadOutput(_)), "{err:?}");
    }

    #[tokio::test]
    async fn timeout_is_retried_once_then_mapped() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200).set_delay(std::time::Duration::from_millis(200)),
            )
            .expect(2)
            .mount(&server)
            .await;
        let err = complete(
            &server.uri(),
            "key",
            "m",
            chrono::Duration::milliseconds(50),
            &turns(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ModelError::Timeout), "{err:?}");
    }
}
