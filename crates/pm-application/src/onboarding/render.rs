//! 指引渲染：Markdown（人看）与 JSON（agent 读）——**同一份 `Guide` 的两种形态**。
//!
//! 不存在两套内容源：Markdown 的每一句都能在 JSON 里找到对应字段。

use super::{Catalog, Guide, API_KEY_PLACEHOLDER};

/// 渲染成 Markdown（人打开就能照着做）。
pub fn to_markdown(guide: &Guide) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# AI-PM 执行端接入：{}\n\n",
        guide.tool.display_name
    ));
    out.push_str(&format!(
        "平台地址：`{}`　MCP 端点：`{}`　传输：`{}`\n\n",
        guide.platform_base_url, guide.mcp_endpoint, guide.transport
    ));
    if guide.base_url_source == super::BaseUrlSource::Default {
        out.push_str(
            "> ⚠️ 平台未配置 `AI_PM_PUBLIC_BASE_URL`，以上是缺省地址。若平台不在本机，请让管理员设置该环境变量后重新拉取本指引。\n\n",
        );
    }
    if !guide.tool.verified {
        out.push_str(
            "> ℹ️ 这个工具的 MCP 配置格式未经核实，下面给的是通用 `mcpServers` 形状。写入前请先确认该工具的 MCP 配置文件位置与字段名。\n\n",
        );
    }

    out.push_str("## 1. 前置条件\n\n");
    for item in &guide.requirements {
        out.push_str(&format!("- {item}\n"));
    }

    out.push_str("\n## 2. 配置 MCP\n\n");
    out.push_str(&format!("写入位置：{}\n\n", guide.tool.config_target));
    out.push_str(&format!(
        "配置片段（`{API_KEY_PLACEHOLDER}` 处填入你的 API Key）：\n\n"
    ));
    match guide.tool.config_format.as_str() {
        "toml" => {
            let content = guide
                .config_template
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            out.push_str("```toml\n");
            out.push_str(content);
            out.push_str("```\n");
        }
        _ => {
            let mut value = guide.config_template.clone();
            if let Some(servers) = value.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
                for server in servers.values_mut() {
                    if let Some(headers) = server.get_mut("headers").and_then(|v| v.as_object_mut())
                    {
                        for header in headers.values_mut() {
                            if let Some(text) = header.as_str() {
                                *header = serde_json::Value::String(
                                    text.replace(API_KEY_PLACEHOLDER, "在此填入你的 API Key"),
                                );
                            }
                        }
                    }
                }
            }
            out.push_str("```json\n");
            out.push_str(
                &serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string()),
            );
            out.push_str("\n```\n");
        }
    }

    out.push_str("\n## 3. 验证连通\n\n");
    for step in &guide.verify {
        out.push_str(&format!(
            "- **{}**：`{} {}`（期望 {}{}）\n",
            step.name,
            step.method,
            step.url,
            step.expect_status,
            step.expect_field
                .as_ref()
                .map(|f| format!("，并读到字段 `{f}`"))
                .unwrap_or_default()
        ));
    }

    out.push_str(&format!(
        "\n### 如果身份未激活（{}）\n\n{}\n\n接口：`{}`\n",
        guide.activation.when, guide.activation.next_step, guide.activation.endpoint
    ));

    out.push_str("\n## 4. 开始干活\n\n");
    out.push_str(&format!("{}\n", guide.next_steps.claim));

    out.push_str("\n## 5. 租约与心跳\n\n");
    out.push_str(&format!("{}\n", guide.lease_hint.recommendation));

    out.push_str("\n## 6. 已知约束\n\n");
    for item in &guide.known_constraints {
        out.push_str(&format!("- {item}\n"));
    }
    out
}

/// 渲染工具清单（`/install`）为 Markdown。
pub fn catalog_to_markdown(catalog: &Catalog) -> String {
    let mut out = String::new();
    out.push_str("# AI-PM 执行端接入工具清单\n\n");
    out.push_str(&format!(
        "平台地址：`{}`　MCP 端点：`{}/mcp`\n\n",
        catalog.platform_base_url, catalog.platform_base_url
    ));
    out.push_str("把 `平台地址/install/<slug>` 粘给执行端 agent，并说「帮我安装」。\n\n");
    out.push_str("| slug | 工具 | 配置格式 | 写入位置 | 格式已核实 |\n");
    out.push_str("| --- | --- | --- | --- | --- |\n");
    for tool in &catalog.tools {
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            tool.slug,
            tool.display_name,
            tool.config_format,
            tool.config_target,
            if tool.verified {
                "是"
            } else {
                "否（通用条目）"
            }
        ));
    }
    out.push_str(
        "\n任何支持 MCP（Streamable HTTP）的工具都能接入：清单里没有的 slug 会落到通用条目。\n",
    );
    out
}
