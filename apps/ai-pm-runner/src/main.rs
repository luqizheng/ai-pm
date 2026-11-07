//! ai-pm-runner：用户 PC 客户端（V0.5 目标 1 三方通信的执行端载体，WP-3）。
//! 循环主体在 [`ai_pm_runner::runner_loop`]（F-12：lib 化供测试驱动）；
//! 本 bin 只做环境配置装载。
//!
//! 非目标（V0.5 基线 §八）：多 Runner 高可用、自动升级、LLM 凭据透传。

use ai_pm_runner::mcp;
use ai_pm_runner::runner_loop::{self, Config};
use uuid::Uuid;

fn main() -> Result<(), String> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    let cfg = load_config()?;
    let bearer_token = std::env::var("AI_PM_RUNNER_TOKEN").unwrap_or_default();
    let client = mcp::McpClient::new(&cfg.platform_mcp_url, &bearer_token);
    tracing::info!(project = %cfg.project_id, client = %cfg.client_name, "ai-pm-runner starting");
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime builds")
        .block_on(runner_loop::run_loop(&cfg, &client));
    Ok(())
}

fn load_config() -> Result<Config, String> {
    let url = std::env::var("AI_PM_PLATFORM_URL").map_err(|_| {
        "AI_PM_PLATFORM_URL is required (e.g. http://192.168.2.10:8090/mcp)".to_string()
    })?;
    if std::env::var("AI_PM_RUNNER_TOKEN").is_err() {
        return Err("AI_PM_RUNNER_TOKEN is required (Bearer credential for /mcp)".to_string());
    }
    let project_id: Uuid = std::env::var("AI_PM_RUNNER_PROJECT_ID")
        .map_err(|_| "AI_PM_RUNNER_PROJECT_ID is required (V1: single-project runner)".to_string())?
        .parse()
        .map_err(|_| "AI_PM_RUNNER_PROJECT_ID is not a UUID".to_string())?;
    let host = std::env::var("AI_PM_RUNNER_CLIENT_NAME").unwrap_or_else(|_| runner_loop::hostname());
    let client_name = format!("runner:{host}");
    let workspace_root = std::env::var("AI_PM_RUNNER_WORKSPACE_ROOT")
        .map_err(|_| "AI_PM_RUNNER_WORKSPACE_ROOT is required".to_string())?
        .into();
    let agent_cmd = std::env::var("AI_PM_RUNNER_AGENT_CMD")
        .map_err(|_| "AI_PM_RUNNER_AGENT_CMD is required (agent launch template)".to_string())?;
    Ok(Config {
        platform_mcp_url: url,
        project_id,
        client_name,
        workspace_root,
        agent_cmd,
        claim_interval_secs: runner_loop::env_num("AI_PM_RUNNER_CLAIM_INTERVAL_SECS", 10),
        heartbeat_secs: runner_loop::env_num("AI_PM_RUNNER_HEARTBEAT_SECS", 60),
        detect_interval_secs: runner_loop::env_num("AI_PM_INTERACTION_DETECT_INTERVAL_SECS", 5),
        command_poll_secs: runner_loop::env_num("AI_PM_COMMAND_POLL_INTERVAL_SECS", 3),
        terminate_attempts: Config::terminate_attempts_default(),
        terminate_timeout_secs: Config::terminate_timeout_secs_default(),
    })
}
