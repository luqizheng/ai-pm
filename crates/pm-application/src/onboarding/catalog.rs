//! 执行端接入工具清单（T-080）：内置常量表，无外部文件、无解析器。
//!
//! 清单分两档：
//!
//! - **已核实**（`verified = true`）：配置片段的字段形状与写入位置来自该工具的
//!   公开文档，可以照抄。
//! - **通用兜底**（`verified = false`）：本仓无该工具的格式记录，**不猜格式**
//!   ——只给通用 `mcpServers` JSON 片段 + 一句「先确认你这个工具的 MCP 配置文件
//!   在哪」。任何支持 MCP 的工具都能从通用条目接入。
//!
//! 新增一个已核实工具 = 加一条 `ToolEntry`，协议与渲染层不用改。

/// 配置片段的书写格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    /// JSON（`mcpServers` 形状）。
    Json,
    /// TOML（Codex `config.toml` 的 `mcp_servers` 形状）。
    Toml,
}

impl ConfigFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            ConfigFormat::Json => "json",
            ConfigFormat::Toml => "toml",
        }
    }
}

/// 一个执行端的接入条目。
#[derive(Debug, Clone)]
pub struct ToolEntry {
    /// URL 里的路径段（小写字母、数字、连字符）。
    pub slug: &'static str,
    /// 人看的名字（未知 slug 落兜底时用通用名）。
    pub display_name: &'static str,
    /// 配置片段的形状是否已核实。
    pub verified: bool,
    /// 配置写到哪里（文件路径或 UI 入口的中文描述）。
    pub config_target: &'static str,
    /// 配置片段的书写格式。
    pub config_format: ConfigFormat,
    /// MCP server 的配置名（`mcpServers.<name>` / `mcp_servers.<name>`）。
    pub server_name: &'static str,
}

/// 清单里的全部条目（顺序即清单展示顺序）。
pub const TOOLS: &[ToolEntry] = &[
    ToolEntry {
        slug: "trae",
        display_name: "Trae",
        verified: true,
        config_target: "Trae 设置 → MCP Servers → 添加服务器 → HTTP 类型",
        config_format: ConfigFormat::Json,
        server_name: "ai-pm",
    },
    ToolEntry {
        slug: "codex",
        display_name: "Codex",
        verified: true,
        config_target: "~/.codex/config.toml（用户级）或 <项目>/.codex/config.toml（受信任项目）",
        config_format: ConfigFormat::Toml,
        server_name: "ai-pm",
    },
    ToolEntry {
        slug: "workbuddy",
        display_name: "WorkBuddy",
        verified: true,
        config_target: "mcp.json",
        config_format: ConfigFormat::Json,
        server_name: "ai-pm",
    },
    ToolEntry {
        slug: "z-code",
        display_name: "z-code",
        verified: false,
        config_target: "先确认你这个工具的 MCP 配置文件在哪，再按下面的 JSON 形状写入",
        config_format: ConfigFormat::Json,
        server_name: "ai-pm",
    },
    ToolEntry {
        slug: "dsh",
        display_name: "dsh",
        verified: false,
        config_target: "先确认你这个工具的 MCP 配置文件在哪，再按下面的 JSON 形状写入",
        config_format: ConfigFormat::Json,
        server_name: "ai-pm",
    },
    ToolEntry {
        slug: "deepseek-harness",
        display_name: "DeepSeek Harness",
        verified: false,
        config_target: "先确认你这个工具的 MCP 配置文件在哪，再按下面的 JSON 形状写入",
        config_format: ConfigFormat::Json,
        server_name: "ai-pm",
    },
    ToolEntry {
        slug: "opencode",
        display_name: "opencode",
        verified: false,
        config_target: "先确认你这个工具的 MCP 配置文件在哪，再按下面的 JSON 形状写入",
        config_format: ConfigFormat::Json,
        server_name: "ai-pm",
    },
    ToolEntry {
        slug: "pi",
        display_name: "pi",
        verified: false,
        config_target: "先确认你这个工具的 MCP 配置文件在哪，再按下面的 JSON 形状写入",
        config_format: ConfigFormat::Json,
        server_name: "ai-pm",
    },
];

/// 通用兜底条目：任何未知 slug 都落这里（用户裁定「支持 MCP 就能接入」）。
pub const GENERIC: ToolEntry = ToolEntry {
    slug: "generic",
    display_name: "通用 MCP 客户端",
    verified: false,
    config_target: "先确认你这个工具的 MCP 配置文件在哪，再按下面的 JSON 形状写入",
    config_format: ConfigFormat::Json,
    server_name: "ai-pm",
};

/// slug 字符集：仅小写字母、数字、连字符（防拼接式路径穿越）。
pub fn is_valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// 按 slug 找条目；找不到返回通用兜底条目（**不 404**）。
pub fn find(slug: &str) -> ToolEntry {
    TOOLS
        .iter()
        .find(|entry| entry.slug == slug)
        .cloned()
        .unwrap_or(GENERIC)
}
