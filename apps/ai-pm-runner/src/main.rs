//! ai-pm-runner：用户 PC 客户端（V0.5 目标 1 三方通信的执行端载体，WP-3）。
//! 循环主体在 [`ai_pm_runner::runner_loop`]（F-12：lib 化供测试驱动）；
//! 本 bin 做环境配置装载、日志面（§8.2：stdout + 滚动文件）与退出码语义。
//!
//! 退出码：0 = 正常退出；2 = 凭据被拒（401/403，fail-closed——人工处置后
//! supervisor 重启）；1 = 其余致命错误。
//!
//! 非目标（V0.5 基线 §八）：多 Runner 高可用、自动升级、LLM 凭据透传。

use ai_pm_runner::mcp;
use ai_pm_runner::runner_loop::{self, Config};
use std::path::PathBuf;
use uuid::Uuid;

const EXIT_OK: i32 = 0;
const EXIT_ERROR: i32 = 1;
const EXIT_CREDENTIAL: i32 = 2;

fn main() {
    let code = real_main();
    std::process::exit(code);
}

fn real_main() -> i32 {
    init_tracing();
    let cfg = match load_config() {
        Ok(cfg) => cfg,
        Err(message) => {
            eprintln!("[config] {message}");
            return EXIT_ERROR;
        }
    };
    if let Err(message) = cfg.validate() {
        eprintln!("[config] {message}");
        return EXIT_ERROR;
    }
    let bearer_token = std::env::var("AI_PM_RUNNER_TOKEN").unwrap_or_default();
    let client = mcp::McpClient::new(&cfg.platform_mcp_url, &bearer_token);
    tracing::info!(
        project = %cfg.project_id,
        client = %cfg.client_name,
        adapter = %cfg.adapter,
        "ai-pm-runner starting"
    );
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime builds");
    match runtime.block_on(runner_loop::run_loop(&cfg, &client)) {
        Ok(()) => EXIT_OK,
        Err(e) if e.is_unauthorized() => {
            // 已在循环内告警；此处保证进程退出码可被 supervisor 区分。
            eprintln!("[credential] {e}");
            EXIT_CREDENTIAL
        }
        Err(e) => {
            eprintln!("[fatal] {e}");
            EXIT_ERROR
        }
    }
}

fn init_tracing() {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    match std::env::var("AI_PM_RUNNER_LOG_DIR")
        .ok()
        .filter(|d| !d.is_empty())
    {
        Some(dir) => {
            std::fs::create_dir_all(&dir).expect("log dir creatable");
            let appender = tracing_appender::rolling::daily(dir, "runner.log");
            // non_blocking guard 常驻进程生命周期——刻意 forget 以保证 flush。
            let (file_writer, guard) = tracing_appender::non_blocking(appender);
            std::mem::forget(guard);
            let file_layer = tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(file_writer);
            let stdout_layer = tracing_subscriber::fmt::layer();
            tracing_subscriber::registry()
                .with(filter)
                .with(stdout_layer)
                .with(file_layer)
                .init();
        }
        None => {
            tracing_subscriber::fmt().with_env_filter(filter).init();
        }
    }
}

fn load_config() -> Result<Config, String> {
    let url = std::env::var("AI_PM_PLATFORM_URL").map_err(|_| {
        "AI_PM_PLATFORM_URL is required (e.g. http://192.168.2.10:8090/mcp)".to_string()
    })?;
    if std::env::var("AI_PM_RUNNER_TOKEN").is_err() {
        return Err("AI_PM_RUNNER_TOKEN is required (Bearer credential for /mcp; cpk_ on the client channel)".to_string());
    }
    let project_id: Uuid = std::env::var("AI_PM_RUNNER_PROJECT_ID")
        .map_err(|_| "AI_PM_RUNNER_PROJECT_ID is required (V1: single-project runner)".to_string())?
        .parse()
        .map_err(|_| "AI_PM_RUNNER_PROJECT_ID is not a UUID".to_string())?;
    // V-05-1（0049）：client_name = 设备登记名（登记制，须与 devenv.device.name
    // 一致）；不再拼 `runner:` 前缀（前缀已降级为兼容识别，不新签）。
    let client_name =
        std::env::var("AI_PM_RUNNER_CLIENT_NAME").unwrap_or_else(|_| runner_loop::hostname());
    let workspace_root = std::env::var("AI_PM_RUNNER_WORKSPACE_ROOT")
        .map_err(|_| "AI_PM_RUNNER_WORKSPACE_ROOT is required".to_string())?
        .into();
    let adapter = std::env::var("AI_PM_RUNNER_ADAPTER").unwrap_or_else(|_| "custom".into());
    let agent_cmd = std::env::var("AI_PM_RUNNER_AGENT_CMD").unwrap_or_default();
    if adapter == "custom" && agent_cmd.is_empty() {
        return Err(
            "AI_PM_RUNNER_AGENT_CMD is required when AI_PM_RUNNER_ADAPTER=custom \
             (agent launch template; zcode|codex|dsh use built-in starter templates)"
                .to_string(),
        );
    }
    let git_map = std::env::var("AI_PM_RUNNER_GIT_MAP")
        .ok()
        .filter(|p| !p.is_empty())
        .map(PathBuf::from);
    Ok(Config {
        platform_mcp_url: url,
        project_id,
        client_name,
        workspace_root,
        adapter,
        agent_cmd,
        git_map,
        // 平台 AI_PM_RUN_LEASE_SECS 的镜像值（默认 300，须与平台一致）。
        lease_secs: runner_loop::env_num("AI_PM_RUN_LEASE_SECS", 300),
        claim_interval_secs: runner_loop::env_num("AI_PM_RUNNER_CLAIM_INTERVAL_SECS", 10),
        heartbeat_secs: runner_loop::env_num("AI_PM_RUNNER_HEARTBEAT_SECS", 60),
        detect_interval_secs: runner_loop::env_num("AI_PM_INTERACTION_DETECT_INTERVAL_SECS", 5),
        command_poll_secs: runner_loop::env_num("AI_PM_COMMAND_POLL_INTERVAL_SECS", 3),
        terminate_attempts: Config::terminate_attempts_default(),
        terminate_timeout_secs: Config::terminate_timeout_secs_default(),
        backoff_base_ms: runner_loop::env_num("AI_PM_RUNNER_BACKOFF_BASE_MS", 2000),
        backoff_cap_ms: runner_loop::env_num("AI_PM_RUNNER_BACKOFF_CAP_MS", 60_000),
    })
}
