//! 检查点 2 R1 新行为测试：凭据 fail-closed 分类（I1/I2）、event_id 双键
//! 一致性（I6/P2-1）、回执信封（I7）、孤儿扫描（Q-RC-5）。不依赖真实 Platform。

use ai_pm_runner::adapter::{
    find_unreported_markers, quarantine_marker, scan_orphans, ExitOutcome,
};
use ai_pm_runner::mcp::McpClient;
use ai_pm_runner::runner_loop::{preflight_continue, receipt_v1};
use serde_json::json;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("aipm-c2-{name}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// I1：401 → `Unauthorized` 分类（fail-closed；调用方据此停止自动恢复退出）。
#[tokio::test]
async fn unauthorized_http_status_classifies_as_credential_rejection() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "error": { "code": "IAM_AUTH_INVALID", "message": "credential revoked" }
        })))
        .mount(&server)
        .await;
    let client = McpClient::new(format!("{}/mcp", server.uri()), "cpk_revoked");
    let err = client.list_ready_tasks(Uuid::nil()).await.unwrap_err();
    assert!(err.is_unauthorized(), "{err}");
    let other = ai_pm_runner::mcp::RunnerError::HttpStatus {
        status: 503,
        detail: "down".into(),
    };
    assert!(!other.is_unauthorized());
}

/// I2：租约类业务拒绝（isError 文本）→ 续领触发判定。
#[tokio::test]
async fn lease_class_tool_error_is_detected() {
    use ai_pm_runner::mcp::RunnerError;
    let lease = RunnerError::Tool("lease_expired: token no longer valid".into());
    assert!(lease.is_lease_error());
    let generation = RunnerError::Tool("generation mismatch (have 1, run at 2)".into());
    assert!(generation.is_lease_error());
    let business = RunnerError::Tool("an open intervention chain exists".into());
    assert!(!business.is_lease_error());
}

/// I6/P2-1：文件名 event_id 与正文不一致 = 非法（双键一致），交由调用方隔离。
#[tokio::test]
async fn marker_filename_content_mismatch_is_invalid() {
    let dir = temp_dir("event-id");
    let aipm = dir.join(".aipm");
    std::fs::create_dir_all(&aipm).unwrap();
    std::fs::write(
        aipm.join("interaction.wrong-id.json"),
        json!({
            "schema_version": "aipm.interaction.v1",
            "event_id": "real-id",
            "reason": "needs_decision",
            "question": "q"
        })
        .to_string(),
    )
    .unwrap();
    let hits = find_unreported_markers(&dir);
    assert_eq!(hits.len(), 1);
    assert!(
        hits[0].1.is_err(),
        "filename/content mismatch must surface as invalid"
    );
    quarantine_marker(&hits[0].0).unwrap();
    assert!(aipm.join("interaction.wrong-id.json.invalid").is_file());
    assert!(find_unreported_markers(&dir).is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

/// Q-RC-5：孤儿扫描只报告（死 pid 也要报告——记录本身是诊断证据）。
#[tokio::test]
async fn orphan_scan_reports_without_killing() {
    let dir = temp_dir("orphan");
    let run_dir = dir.join("run-1");
    std::fs::create_dir_all(run_dir.join(".aipm")).unwrap();
    std::fs::write(
        run_dir.join(".aipm").join("process.json"),
        json!({
            "pid": 4_000_000u64, // 不可能存活的 pid（Windows pid 上限内空档）
            "session_id": "runner:1",
            "identity_hint": "win-ft-0-0"
        })
        .to_string(),
    )
    .unwrap();
    let reports = scan_orphans(&dir);
    assert_eq!(reports.len(), 1);
    assert!(!reports[0].alive, "dead pid reported as not alive");
    assert_eq!(reports[0].record.session_id, "runner:1");
    let _ = std::fs::remove_dir_all(dir);
}

/// I7：回执信封 = schema v1 + 当前代次 + 原字段透传。
#[test]
fn terminate_receipt_carries_requested_confirmed_pair() {
    let receipt = receipt_v1(
        3,
        json!({
            "exit_outcome": ExitOutcome::Lost.as_str(),
            "pid": serde_json::Value::Null,
            "terminate_requested_at": "2026-11-08T00:00:00Z",
            "terminate_confirmed_at": "2026-11-08T00:00:01Z"
        }),
    );
    assert_eq!(receipt["schema_version"], "aipm.receipt.v1");
    assert_eq!(receipt["run_generation"], 3);
    assert_eq!(receipt["exit_outcome"], "Lost");
    assert!(receipt["terminate_requested_at"].is_string());
}

/// ExitOutcome 五值对齐平台权威枚举（I10 序列化面）。
#[test]
fn exit_outcome_serializes_five_values() {
    assert_eq!(ExitOutcome::Success.as_str(), "Success");
    assert_eq!(ExitOutcome::NonZeroExit(2).as_str(), "NonZeroExit");
    assert_eq!(ExitOutcome::Signaled(9).as_str(), "Signaled");
    assert_eq!(ExitOutcome::Killed.as_str(), "Killed");
    assert_eq!(ExitOutcome::Lost.as_str(), "Lost");
}

/// F-1 回归钉：preflight 语义不随本批漂移。
#[test]
fn preflight_semantics_unchanged() {
    assert!(preflight_continue(&json!({ "continuation_start_frozen": true })).is_err());
    assert!(preflight_continue(&json!({})).is_ok());
    assert!(preflight_continue(&serde_json::Value::Null).is_ok());
}
