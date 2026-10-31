//! Wiremock-based wire tests for `OpenAiCompatibleModel` (Task 17 Step 1).
//!
//! No database. Six scenarios per the brief: success parse, 5xx ->
//! Unavailable, non-contract JSON -> BadOutput, delay vs client timeout ->
//! Timeout, 401 -> BadOutput, and request-shape verification.

use pm_ai::{AnalysisContext, AnalysisModel, ModelError, OpenAiCompatibleModel, SYSTEM_PROMPT};
use pm_domain::{Confidence, ProjectCommand, TaskStatus};
use serde_json::json;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const API_KEY: &str = "test-api-key-17";
const MODEL_NAME: &str = "test-model-17";

fn context() -> AnalysisContext {
    AnalysisContext {
        project_id: "11111111-1111-4111-8111-111111111111".parse().unwrap(),
        event_id: "44444444-4444-4444-8444-444444444444".parse().unwrap(),
        event_type: "technical_design_submitted".to_string(),
        payload: json!({ "design": "local in-process cache for map tiles" }),
        task: Some(json!({ "id": "TASK-102", "revision": 7 })),
        requirement_revision: None,
        milestone: Some(json!({ "id": "MILESTONE-1", "revision": 4 })),
        config: json!({ "automation_mode": "ai_lead" }),
        policy_version: 1,
    }
}

fn model(base_url: &str, timeout_secs: i64) -> OpenAiCompatibleModel {
    OpenAiCompatibleModel::new(
        base_url,
        MODEL_NAME,
        API_KEY,
        chrono::Duration::seconds(timeout_secs),
    )
}

/// Wraps a model-produced JSON document as the `choices[0].message.content`
/// string of a chat-completions response body.
fn completion_body(content: serde_json::Value) -> serde_json::Value {
    json!({
        "id": "chatcmpl-test",
        "object": "chat.completion",
        "choices": [{
            "index": 0,
            "finish_reason": "stop",
            "message": { "role": "assistant", "content": content.to_string() }
        }]
    })
}

/// A contract-conforming `AnalysisResult` with three proposals whose
/// confidences exercise the snake_case enum parsing: high / medium / low.
fn valid_analysis() -> serde_json::Value {
    json!({
        "summary": "本地缓存方案与 ADR-008 的集中式缓存策略冲突，建议先补齐信息再调研替代方案。",
        "findings": [{
            "skill": "architecture_consistency",
            "severity": "warning",
            "description": "方案使用进程内缓存，与 ADR-008 约定的集中式缓存不一致。",
            "evidence_refs": ["ADR-008", "TASK-102"]
        }],
        "proposals": [
            {
                "command": {
                    "kind": "update_task_status",
                    "data": {
                        "task_id": "22222222-2222-4222-8222-222222222222",
                        "expected_revision": 7,
                        "new_status": "awaiting_decision"
                    }
                },
                "rationale": "TASK-102 依赖尚未裁决的缓存选型，先置为等待决策（needs_info 语义并入 awaiting_decision）等待架构决议。",
                "risk_level": "low",
                "confidence": "high",
                "evidence": ["ADR-008"]
            },
            {
                "command": {
                    "kind": "adjust_milestone_plan",
                    "data": {
                        "project_id": "11111111-1111-4111-8111-111111111111",
                        "milestone_id": "33333333-3333-4333-8333-333333333333",
                        "new_planned_at": "2026-11-06T09:00:00Z",
                        "expected_revision": 4
                    }
                },
                "rationale": "缓存选型未定将阻塞里程碑验收，计划顺延 3 天容纳调研。",
                "risk_level": "medium",
                "confidence": "medium",
                "evidence": ["TASK-102"]
            },
            {
                "command": {
                    "kind": "create_task",
                    "data": {
                        "project_id": "11111111-1111-4111-8111-111111111111",
                        "title": "缓存调研",
                        "description": "对比本地缓存与集中式缓存的失效与一致性策略。",
                        "assignee_id": null
                    }
                },
                "rationale": "ADR-008 决议前需要独立调研给出可裁决的选型依据。",
                "risk_level": "low",
                "confidence": "low",
                "evidence": []
            }
        ]
    })
}

async fn mount_ok(server: &MockServer, content: serde_json::Value) {
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(completion_body(content)))
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test]
async fn ok_response_parses_analysis_result_with_snake_case_enums() {
    let server = MockServer::start().await;
    mount_ok(&server, valid_analysis()).await;

    let result = model(&server.uri(), 10)
        .analyze(&context(), &pm_ai::route("technical_design_submitted"))
        .await
        .unwrap();

    // 1. summary text
    assert_eq!(
        result.summary,
        "本地缓存方案与 ADR-008 的集中式缓存策略冲突，建议先补齐信息再调研替代方案。"
    );
    // 2. proposal count
    assert_eq!(result.proposals.len(), 3);
    // 3. confidence enums parsed from high/medium/low
    assert_eq!(result.proposals[0].confidence, Confidence::High);
    assert_eq!(result.proposals[1].confidence, Confidence::Medium);
    assert_eq!(result.proposals[2].confidence, Confidence::Low);
    // Bonus: adjacent tag + task status snake_case survived the round trip.
    match &result.proposals[0].command {
        ProjectCommand::UpdateTaskStatus { new_status, .. } => {
            assert_eq!(*new_status, TaskStatus::AwaitingDecision);
        }
        other => panic!("expected update_task_status, got {}", other.kind()),
    }
}

#[tokio::test]
async fn server_error_500_maps_to_unavailable() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(500).set_body_string("upstream exploded"))
        .expect(1)
        .mount(&server)
        .await;

    let err = model(&server.uri(), 10)
        .analyze(&context(), &[])
        .await
        .unwrap_err();

    assert!(matches!(err, ModelError::Unavailable(_)), "got {err:?}");
}

#[tokio::test]
async fn non_contract_json_maps_to_bad_output() {
    let server = MockServer::start().await;
    // Valid JSON, wrong shape: findings/proposals missing, wrong types.
    mount_ok(&server, json!({ "summary": "ok" })).await;

    let err = model(&server.uri(), 10)
        .analyze(&context(), &[])
        .await
        .unwrap_err();

    assert!(matches!(err, ModelError::BadOutput(_)), "got {err:?}");
}

#[tokio::test]
async fn slow_response_exceeds_client_timeout_maps_to_timeout() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(completion_body(valid_analysis()))
                .set_delay(std::time::Duration::from_millis(2000)),
        )
        .expect(1)
        .mount(&server)
        .await;

    let err = model(&server.uri(), 1).analyze(&context(), &[]).await.unwrap_err();

    assert_eq!(err, ModelError::Timeout);
}

#[tokio::test]
async fn unauthorized_401_maps_to_bad_output() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(401).set_body_string("{\"error\": \"bad key\"}"))
        .expect(1)
        .mount(&server)
        .await;

    let err = model(&server.uri(), 10)
        .analyze(&context(), &[])
        .await
        .unwrap_err();

    // 4xx is caller misconfiguration: retrying cannot help, so it must not
    // be classified as Unavailable.
    assert!(matches!(err, ModelError::BadOutput(_)), "got {err:?}");
}

#[tokio::test]
async fn request_shape_matches_openai_chat_completions_contract() {
    let server = MockServer::start().await;
    // The matchers double as assertions: if the request deviates (wrong auth
    // header, missing response_format, missing system message), the mock does
    // not match, `analyze` gets the unmatched-response error path, and the
    // final `verify` fails with the received request in its message.
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("authorization", &format!("Bearer {API_KEY}")))
        .and(body_partial_json(json!({
            "model": MODEL_NAME,
            "response_format": { "type": "json_object" },
            "messages": [
                { "role": "system", "content": SYSTEM_PROMPT },
                { "role": "user" }
            ]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(completion_body(valid_analysis())))
        .expect(1)
        .mount(&server)
        .await;

    model(&server.uri(), 10)
        .analyze(&context(), &pm_ai::route("technical_design_submitted"))
        .await
        .unwrap();

    server.verify().await;
}
