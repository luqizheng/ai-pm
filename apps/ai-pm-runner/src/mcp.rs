//! Runner ↔ Platform MCP 通道（V0.5 WP-3 · 设计稿 docs/ui/s03-acp/
//! smark-pm-s03-acp-slice-design-acp01-02-08-v1.md V1.1 §3.3 命令面 / §1 通道面）。
//!
//! 薄 JSON-RPC 客户端：Platform `/mcp`（Streamable HTTP，`json_response`
//! 模式、无会话状态——pm-mcp smoke 测试同路径）。执行面工具经 Bearer 凭据
//! （Model A onboarding 既有链路；V-05-1 后换 Client 凭据，本层不变）。

use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum RunnerError {
    #[error("http transport: {0}")]
    Http(#[from] reqwest::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("adapter: {0}")]
    Adapter(String),
    #[error("platform tool error: {0}")]
    Tool(String),
    #[error("malformed platform response: {0}")]
    Malformed(String),
}

pub struct McpClient {
    http: reqwest::Client,
    url: String,
    bearer_token: String,
    next_id: std::sync::atomic::AtomicU64,
}

impl McpClient {
    pub fn new(url: impl Into<String>, bearer_token: &str) -> Self {
        let http = reqwest::Client::builder()
            // 环境代理残留不得劫持内网 Platform 流量（对齐 smoke 测试 .no_proxy 修复）。
            .no_proxy()
            // Platform rmcp 每个 session 响应后即关闭连接：禁用空闲复用（陈旧
            // 连接会 TCP 重传黑洞）+ 全局超时（无超时的挂起会卡死监督循环）。
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .pool_idle_timeout(std::time::Duration::from_secs(0))
            .pool_max_idle_per_host(0)
            .build()
            .expect("reqwest client builds without IO");
        Self {
            http,
            url: url.into(),
            bearer_token: bearer_token.to_string(),
            next_id: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// `tools/call` 一次调用：成功取 `result.structuredContent`（rmcp
    /// structured 输出），无结构化体时回退解析 `content[0].text`；
    /// `isError: true` → [`RunnerError::Tool`]（content[0].text 文本）。
    pub async fn call_tool(&self, name: &str, args: Value) -> Result<Value, RunnerError> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let body = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": name, "arguments": args },
        });
        let started = std::time::Instant::now();
        tracing::debug!(tool = name, "mcp call start");
        let resp: Value = self
            .http
            .post(&self.url)
            .header("Accept", "application/json, text/event-stream")
            .bearer_auth(&self.bearer_token)
            .json(&body)
            .send()
            .await?
            .json()
            .await?;
        tracing::debug!(tool = name, elapsed_ms = started.elapsed().as_millis() as u64, "mcp call done");
        if let Some(err) = resp.get("error") {
            return Err(RunnerError::Tool(format!("json-rpc error: {err}")));
        }
        let result = resp
            .get("result")
            .ok_or_else(|| RunnerError::Malformed(format!("no result in response: {resp}")))?;
        if result.get("isError").and_then(|v| v.as_bool()) == Some(true) {
            let text = result
                .get("content")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("unknown tool error")
                .to_string();
            return Err(RunnerError::Tool(text));
        }
        if let Some(structured) = result.get("structuredContent") {
            return Ok(structured.clone());
        }
        let text = result
            .get("content")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| RunnerError::Malformed("no structuredContent or text content".into()))?;
        serde_json::from_str(text)
            .map_err(|e| RunnerError::Malformed(format!("text content is not JSON: {e}")))
    }

    pub async fn list_ready_tasks(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<(Uuid, Uuid)>, RunnerError> {
        let value = self
            .call_tool("pm_list_tasks", json!({ "project_id": project_id, "limit": 20 }))
            .await?;
        let mut out = Vec::new();
        if let Some(tasks) = value.get("tasks").and_then(|t| t.as_array()) {
            for task in tasks {
                let task_id = task.get("task_id").and_then(|v| v.as_str());
                let run_id = task.get("queued_run_id").and_then(|v| v.as_str());
                if let (Some(task_id), Some(run_id)) = (task_id, run_id) {
                    if let (Ok(task_id), Ok(run_id)) = (task_id.parse(), run_id.parse()) {
                        out.push((task_id, run_id));
                    }
                }
            }
        }
        Ok(out)
    }

    /// `pm_claim_task`（Model A 通道；`client_name` 携保留前缀 `runner:` →
    /// Platform 侧 stamp execution_mode='spawn'，见 claim.rs V1.1 过渡标记）。
    pub async fn claim_task(
        &self,
        project_id: Uuid,
        task_id: Uuid,
        client_name: &str,
    ) -> Result<Value, RunnerError> {
        self.call_tool(
            "pm_claim_task",
            json!({ "project_id": project_id, "task_id": task_id, "client_name": client_name }),
        )
        .await
    }

    pub async fn report_progress(
        &self,
        project_id: Uuid,
        run_id: Uuid,
        lease_token: &str,
        lease_generation: i32,
        stage: &str,
        note: &str,
    ) -> Result<Value, RunnerError> {
        self.call_tool(
            "pm_report_progress",
            json!({
                "project_id": project_id, "run_id": run_id,
                "lease_token": lease_token, "lease_generation": lease_generation,
                "stage": stage, "note": note,
            }),
        )
        .await
    }

    /// `pm_report_interaction`（N-2 ②段：执行端 → Platform；非人类确认入口）。
    #[allow(clippy::too_many_arguments)]
    pub async fn report_interaction(
        &self,
        project_id: Uuid,
        task_id: Uuid,
        run_id: Uuid,
        lease_token: &str,
        lease_generation: i32,
        event: &crate::adapter::MarkerEvent,
    ) -> Result<Value, RunnerError> {
        self.call_tool(
            "pm_report_interaction",
            json!({
                "project_id": project_id, "task_id": task_id, "run_id": run_id,
                "lease_token": lease_token, "lease_generation": lease_generation,
                "event_id": event.event_id,
                "reason": event.reason,
                "title": event.title,
                "question": event.question,
                "options": event.options,
                "required_response": event.required_response,
                "checkpoint_path": event.checkpoint_path,
            }),
        )
        .await
    }

    pub async fn poll_commands(
        &self,
        project_id: Uuid,
        run_id: Uuid,
        worker_id: &str,
    ) -> Result<Vec<Value>, RunnerError> {
        let value = self
            .call_tool(
                "pm_poll_commands",
                json!({ "project_id": project_id, "run_id": run_id, "worker_id": worker_id }),
            )
            .await?;
        Ok(value
            .get("commands")
            .and_then(|c| c.as_array())
            .cloned()
            .unwrap_or_default())
    }

    pub async fn ack_command(
        &self,
        project_id: Uuid,
        command_id: Uuid,
        worker_id: &str,
        outcome: &str,
        receipt: Value,
        error: Option<String>,
    ) -> Result<Value, RunnerError> {
        self.call_tool(
            "pm_ack_command",
            json!({
                "project_id": project_id, "command_id": command_id, "worker_id": worker_id,
                "outcome": outcome, "receipt": receipt, "error": error,
            }),
        )
        .await
    }

    /// `pm_discover_runs`（R-11/§3.6 发现面）：名下 start_wait 且有可投递
    /// continue 的 Run 摘要。只读、无副作用。
    pub async fn discover_runs(&self, project_id: Uuid) -> Result<Value, RunnerError> {
        self.call_tool(
            "pm_discover_runs",
            json!({ "project_id": project_id, "limit": 20, "offset": 0 }),
        )
        .await
    }

    /// `pm_reacquire_lease`（R-16 方案 a 续领）：同 Run 重签租约 + 命令改路由。
    pub async fn reacquire_lease(
        &self,
        project_id: Uuid,
        run_id: Uuid,
    ) -> Result<Value, RunnerError> {
        self.call_tool(
            "pm_reacquire_lease",
            json!({ "project_id": project_id, "run_id": run_id }),
        )
        .await
    }

    /// `pm_report_consumption`（N-2 ④段；证据不足 Platform 会拒绝，不伪造）。
    pub async fn report_consumption(
        &self,
        project_id: Uuid,
        run_id: Uuid,
        lease_token: &str,
        lease_generation: i32,
        interaction_id: Uuid,
    ) -> Result<Value, RunnerError> {
        self.call_tool(
            "pm_report_consumption",
            json!({
                "project_id": project_id, "run_id": run_id,
                "lease_token": lease_token, "lease_generation": lease_generation,
                "interaction_id": interaction_id,
                "evidence": { "kinds": ["initial_input_injected", "process_alive"] },
            }),
        )
        .await
    }
}
