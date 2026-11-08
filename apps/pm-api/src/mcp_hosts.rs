//! T-081：`/mcp` 跨机暴露的 Host 白名单配置。
//!
//! rmcp 的 DNS-rebinding 防护默认只接受回环 Host（`localhost` / `127.0.0.1` /
//! `::1`，不限端口）。跨机接入（服务器栈 + Windows 开发机）要放行部署地址的
//! Host，由部署方用 `AI_PM_MCP_ALLOWED_HOSTS`（逗号分隔）声明。
//!
//! 口径：
//! - **只增不减**——回环默认值恒在，本机探针与本地自检不受影响；
//! - 条目语义沿用 rmcp：只写 host 表示不限端口，`host:port` 则端口须精确匹配；
//! - **不提供「清空白名单」的开关**——rmcp 里空名单等于放行全部，本仓不给这条路。

/// 环境变量名（部署侧声明额外放行的 Host）。
pub const ENV_ALLOWED_HOSTS: &str = "AI_PM_MCP_ALLOWED_HOSTS";

/// `/mcp` 的 Host 白名单配置。`extra_allowed` 为部署方声明的额外 Host；
/// `None` 表示没有额外放行（只有回环默认）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct McpHosts {
    pub extra_allowed: Option<Vec<String>>,
}

impl McpHosts {
    /// 从环境变量读取（生产路径；测试走 [`McpHosts::default`] 保持确定性）。
    pub fn from_env() -> Self {
        Self {
            extra_allowed: std::env::var(ENV_ALLOWED_HOSTS)
                .ok()
                .and_then(|raw| parse_allowed_hosts(&raw)),
        }
    }
}

/// 解析 `AI_PM_MCP_ALLOWED_HOSTS` 的原始值：逗号分隔，空白项丢弃。
///
/// 解析后为空 → `None`（「没有额外放行」），**不是**「放行全部」——
/// 后者由 rmcp 的空名单语义承担，本仓不开放。
pub fn parse_allowed_hosts(raw: &str) -> Option<Vec<String>> {
    let hosts: Vec<String> = raw
        .split(',')
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect();
    if hosts.is_empty() {
        None
    } else {
        Some(hosts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_comma_separated_hosts_and_trims() {
        let hosts = parse_allowed_hosts(" 192.168.2.10 , ai-pm.internal ,192.168.2.10 ").unwrap();
        assert_eq!(
            hosts,
            vec!["192.168.2.10", "ai-pm.internal", "192.168.2.10"]
        );
    }

    #[test]
    fn blank_and_separator_only_values_mean_no_extras() {
        assert!(parse_allowed_hosts("").is_none());
        assert!(parse_allowed_hosts("   ").is_none());
        assert!(parse_allowed_hosts(", ,").is_none());
    }

    #[test]
    fn host_port_entries_are_kept_verbatim() {
        let hosts = parse_allowed_hosts("ai-pm.internal:8443").unwrap();
        assert_eq!(hosts, vec!["ai-pm.internal:8443"]);
    }
}
