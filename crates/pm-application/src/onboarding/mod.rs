//! 执行端接入引导（T-080）：把「粘贴 URL → 帮我安装」变成一次可执行的步骤。
//!
//! 本模块是**纯数据 + 纯渲染**：无状态、不碰库、不读环境变量。平台地址由调用方
//! （`pm-api`）从 `AI_PM_PUBLIC_BASE_URL` 读出后传入。
//!
//! 两处硬规则：
//!
//! 1. **URL 与指引内容不承载凭据**——key 只以 `<API_KEY>` 占位符出现。
//! 2. **Markdown 与 JSON 是同一份事实的两种渲染**——不存在两套内容源。

pub mod catalog;
pub mod render;

use serde::{Deserialize, Serialize};

pub use catalog::{ConfigFormat, ToolEntry, find, is_valid_slug};

/// API Key 在指引里的占位符（出现即为占位，不是真实凭据）。
pub const API_KEY_PLACEHOLDER: &str = "<API_KEY>";

/// 平台地址的缺省值（本机单机部署）。
pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8090";

/// 平台地址的来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaseUrlSource {
    /// 来自 `AI_PM_PUBLIC_BASE_URL`。
    Env,
    /// 用了缺省值，部署方可能还没配。
    Default,
}

/// 一个执行端的接入指引（agent 可读的事实源）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Guide {
    pub platform_base_url: String,
    pub base_url_source: BaseUrlSource,
    pub mcp_endpoint: String,
    pub transport: String,
    pub tool: ToolInfo,
    /// 该工具的 MCP 配置片段；key 位置用 `<API_KEY>` 占位。
    pub config_template: serde_json::Value,
    pub requirements: Vec<String>,
    pub steps: Vec<Step>,
    pub verify: Vec<VerifyStep>,
    pub activation: Activation,
    pub next_steps: NextSteps,
    pub lease_hint: LeaseHint,
    pub known_constraints: Vec<String>,
}

/// 工具身份与配置落点。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub slug: String,
    pub display_name: String,
    /// 配置片段形状是否已核实；`false` = 通用兜底条目。
    pub verified: bool,
    pub config_target: String,
    pub config_format: String,
}

/// 一个安装步骤（机器可读的枚举，不留自由文本）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub id: String,
    pub title: String,
}

/// 一个自检项。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyStep {
    pub name: String,
    pub method: String,
    pub url: String,
    pub headers: serde_json::Value,
    pub expect_status: u16,
    /// 期望读到的响应字段（可空）。
    pub expect_field: Option<String>,
}

/// 未激活路径（`is_active == false`）的下一步指引。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activation {
    pub when: String,
    pub next_step: String,
    pub endpoint: String,
}

/// 安装完成之后怎么开始干活。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextSteps {
    pub claim: String,
}

/// 租约/心跳建议值（**不改服务端默认值**）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseHint {
    /// 服务端当前默认值（秒）。
    pub lease_secs: u64,
    /// 给人工档的建议（秒）。
    pub recommended_lease_secs: u64,
    pub recommendation: String,
}

/// 工具清单条目（`/install` 的返回）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogItem {
    pub slug: String,
    pub display_name: String,
    pub verified: bool,
    pub config_target: String,
    pub config_format: String,
}

/// 工具清单。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    pub platform_base_url: String,
    pub base_url_source: BaseUrlSource,
    pub tools: Vec<CatalogItem>,
}

/// 工具清单（`/install`）。
pub fn tool_catalog(base_url: &str, source: BaseUrlSource) -> Catalog {
    Catalog {
        platform_base_url: base_url.to_string(),
        base_url_source: source,
        tools: catalog::TOOLS
            .iter()
            .map(|entry| CatalogItem {
                slug: entry.slug.to_string(),
                display_name: entry.display_name.to_string(),
                verified: entry.verified,
                config_target: entry.config_target.to_string(),
                config_format: entry.config_format.as_str().to_string(),
            })
            .collect(),
    }
}

/// 构造某个工具的接入指引。`slug` 命中不了清单时落通用兜底条目（**不 404**）。
pub fn install_guide(slug: &str, base_url: &str, source: BaseUrlSource) -> Guide {
    let entry = catalog::find(slug);
    let base_url = base_url.trim_end_matches('/');
    let mcp_endpoint = format!("{base_url}/mcp");
    let host_header = host_header_for(base_url);

    Guide {
        platform_base_url: base_url.to_string(),
        base_url_source: source,
        mcp_endpoint: mcp_endpoint.clone(),
        transport: "streamable-http".to_string(),
        tool: ToolInfo {
            slug: entry.slug.to_string(),
            display_name: entry.display_name.to_string(),
            verified: entry.verified,
            config_target: entry.config_target.to_string(),
            config_format: entry.config_format.as_str().to_string(),
        },
        config_template: config_template(&entry, &mcp_endpoint, &host_header),
        requirements: vec![
            "一个支持 MCP（Streamable HTTP）的客户端".to_string(),
            "平台签发的 API Key（已绑定真人）——请向平台管理员索取".to_string(),
        ],
        steps: vec![
            Step {
                id: "read_guide".to_string(),
                title: "读取本指引".to_string(),
            },
            Step {
                id: "ask_api_key".to_string(),
                title: "向用户索取 API Key（不要臆造，也不要写入本 URL）".to_string(),
            },
            Step {
                id: "write_config".to_string(),
                title: "把 config_template 写入 config_target，并把 <API_KEY> 替换为用户给的 key"
                    .to_string(),
            },
            Step {
                id: "verify".to_string(),
                title: "按 verify 列表逐条自检".to_string(),
            },
        ],
        verify: vec![
            VerifyStep {
                name: "health".to_string(),
                method: "GET".to_string(),
                url: format!("{base_url}/health"),
                headers: serde_json::json!({}),
                expect_status: 200,
                expect_field: None,
            },
            VerifyStep {
                name: "identity".to_string(),
                method: "GET".to_string(),
                url: format!("{base_url}/v1/pm/me"),
                headers: serde_json::json!({ "Authorization": format!("Bearer {API_KEY_PLACEHOLDER}") }),
                expect_status: 200,
                expect_field: Some("is_active".to_string()),
            },
        ],
        activation: Activation {
            when: "identity 响应中 is_active == false".to_string(),
            next_step: "请平台管理员执行 PATCH /v1/pm/principals/{principal_id}，body {\"is_active\": true}，然后重试 identity 步骤。".to_string(),
            endpoint: "PATCH /v1/pm/principals/{principal_id}".to_string(),
        },
        next_steps: NextSteps {
            claim: "从 Web 控制台挑选 ready 任务拿到 task_id 与 project_id，再调用 pm_claim_task 领取（当前版本没有任务列表工具，执行端不能自行发现任务）。".to_string(),
        },
        lease_hint: LeaseHint {
            lease_secs: 300,
            recommended_lease_secs: 7200,
            recommendation: "人工档建议租约 ≥ 7200s、心跳 ≤ 1800s（需运维调整 AI_PM_RUN_LEASE_SECS；服务端默认 300s）。".to_string(),
        },
        known_constraints: vec![
            "/mcp 默认只接受回环 Host；跨机接入需先配置服务端允许的 Host（T-081）。".to_string(),
        ],
    }
}

/// 平台地址里的 host（带非默认端口），用作显式 `Host` 头。
///
/// 依据：rmcp 的 `parse_host_header` 优先读客户端自带的 `Host`，其源码注释明确
/// 写着 `axum::Router::nest` 可能丢掉 hyper 合成的 `Host`——pm-api 恰恰是
/// `nest("/mcp", …)`，所以显式带上更稳；rmcp 默认白名单含 `127.0.0.1` 且不限端口。
fn host_header_for(base_url: &str) -> String {
    let rest = base_url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(base_url);
    let authority = rest.split('/').next().unwrap_or(rest);
    if authority.is_empty() {
        "127.0.0.1".to_string()
    } else {
        authority.to_string()
    }
}

/// 该工具的配置片段。`Host` 头的值从部署地址推导，不写死回环地址。
fn config_template(entry: &ToolEntry, mcp_endpoint: &str, host_header: &str) -> serde_json::Value {
    let authorization = format!("Bearer {API_KEY_PLACEHOLDER}");
    match entry.config_format {
        ConfigFormat::Json => serde_json::json!({
            "mcpServers": {
                entry.server_name: {
                    "url": mcp_endpoint,
                    "headers": {
                        "Authorization": authorization,
                        "Host": host_header,
                    }
                }
            }
        }),
        ConfigFormat::Toml => {
            // TOML 片段不是 JSON——按 Codex 的 `mcp_servers` 形状给出一段文本。
            serde_json::json!({
                "format": "toml",
                "content": format!(
                    "[mcp_servers.{name}]\nurl = \"{url}\"\nhttp_headers = {{ Authorization = \"{auth}\", Host = \"{host}\" }}\n",
                    name = entry.server_name,
                    url = mcp_endpoint,
                    auth = authorization,
                    host = host_header,
                )
            })
        }
    }
}
