//! Adapter 模板库（V-05-5 优先级 ZCode → Codex → DSH；Q-RC-6 裁决：模板库
//! 起步 + trait 接口位不强制多态）。三家以「注入通道」区分（V1.2 §7.1 / R-6）：
//!
//! | Adapter | 注入方式 | INPUT_INJECTED 证据 |
//! |---|---|---|
//! | zcode / codex | 启动参数/提示文件（启动即读 ⇒ 必然接收） | spawn 摘要 + 进程存活 |
//! | dsh | stdin 流式（观测首个输出轮次才算已接收；无法满足 → N-6 稳态不伪造） | stdin 注入 + 进程存活 |
//!
//! ⚠️ 启动参数是**起步模板**：按本机 CLI 版本核对后可用 `AI_PM_RUNNER_AGENT_CMD`
//! 覆盖（raw 模板路径），不匹配不影响 Runner 其余链路。

use crate::adapter::ResolvedLaunch;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub const ADAPTER_ZCODE: &str = "zcode";
pub const ADAPTER_CODEX: &str = "codex";
pub const ADAPTER_DSH: &str = "dsh";
pub const ADAPTER_CUSTOM: &str = "custom";

#[derive(Debug, Clone, thiserror::Error)]
#[error("{0}")]
pub struct ProfileError(pub String);

/// 解析 Adapter 选择 → 启动配置。`prompt` 为任务提示正文（含人工回复注入时
/// 的 Continuation 上下文）；`prompt_path` 为提示文件落点（提示文件注入形态）。
pub fn resolve(
    adapter: &str,
    custom_cmd: Option<&str>,
    prompt: &str,
    prompt_path: &Path,
) -> Result<ResolvedLaunch, ProfileError> {
    match adapter {
        ADAPTER_CUSTOM => {
            let Some(cmd) = custom_cmd else {
                return Err(ProfileError(
                    "adapter=custom requires AI_PM_RUNNER_AGENT_CMD".into(),
                ));
            };
            let mut parts = cmd.split_whitespace();
            let Some(program) = parts.next() else {
                return Err(ProfileError("agent command template is empty".into()));
            };
            Ok(ResolvedLaunch {
                program: program.to_string(),
                args: parts.map(str::to_string).collect(),
                stdin_prompt: None,
            })
        }
        ADAPTER_ZCODE | ADAPTER_CODEX => {
            // 提示文件注入：启动即读 ⇒ 进程启动成功 = 必然接收（R-6 第一类）。
            Ok(ResolvedLaunch {
                program: adapter.to_string(),
                args: vec!["exec".into(), prompt_path.to_string_lossy().to_string()],
                stdin_prompt: None,
            })
        }
        ADAPTER_DSH => {
            // stdin 流式注入（R-6 第三类：注入后观测首个输出轮次才算已接收）。
            Ok(ResolvedLaunch {
                program: "dsh".into(),
                args: vec![],
                stdin_prompt: Some(prompt.to_string()),
            })
        }
        other => Err(ProfileError(format!(
            "unknown adapter `{other}` (expected zcode|codex|dsh|custom)"
        ))),
    }
}

/// 任务提示 + instruction 合成正文（I8：标记文件协议 + A* 红线 + 工作区地图）。
pub fn build_prompt(
    project_id: &str,
    task_id: &str,
    run_id: &str,
    lease_generation: i32,
    human_reply: Option<&Value>,
    workspace_note: Option<&str>,
) -> String {
    let mut body = format!(
        "# Task Execution Brief\n\n\
         - project_id: {project_id}\n\
         - task_id: {task_id}\n\
         - run_id: {run_id}\n\
         - lease_generation: {lease_generation}\n\n\
         1. 调用 MCP 工具 `pm_get_run_context` 获取完整任务契约（目标 / 约束 / \
         逐条验收 / 产物路径 / 每仓基线 / 知识引用）。\n\
         2. 执行期间按心跳间隔调用 `pm_report_progress`（stage 变化即进展）。\n\
         3. 完成后调用 `pm_submit_result`；阻塞时 report blocked。\n\
         4. 需要人工介入时：在 `.aipm/` 下写标记文件 \
         `interaction.{{event_id}}.json`（schema `aipm.interaction.v1`，字段 \
         event_id/reason/title/question/required_response），**只创建新事件，\
         不修改已提交事件**。\n\
         5. 不得通过任何 MCP 工具自称获得人工批准（人类确认仅经 Platform REST/UI）。\n"
    );
    if let Some(reply) = human_reply {
        body.push_str(&format!(
            "\n## Continuation：人工回复（上一 Session 因介入终止；本 Session 携带回复继续）\n\n\
             ```json\n{}\n```\n",
            serde_json::to_string_pretty(reply).unwrap_or_default()
        ));
    }
    if let Some(note) = workspace_note {
        body.push_str(&format!("\n## Workspace\n\n{note}\n"));
    }
    body
}

/// 提示落盘（提示文件注入形态的文件 + 诊断留档；原子写）。
pub fn write_prompt_file(run_dir: &Path, prompt: &str) -> std::io::Result<PathBuf> {
    let dir = run_dir.join(".aipm");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("task-prompt.md");
    let tmp = dir.join("task-prompt.md.tmp");
    std::fs::write(&tmp, prompt)?;
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_known_adapters() {
        let prompt_path = Path::new("C:\\tmp\\run\\.aipm\\task-prompt.md");
        let zcode = resolve("zcode", None, "p", prompt_path).unwrap();
        assert_eq!(zcode.program, "zcode");
        assert!(zcode.stdin_prompt.is_none());

        let dsh = resolve("dsh", None, "stdin-prompt", prompt_path).unwrap();
        assert_eq!(dsh.stdin_prompt.as_deref(), Some("stdin-prompt"));

        let custom = resolve("custom", Some("my-cli --flag run"), "p", prompt_path).unwrap();
        assert_eq!(custom.program, "my-cli");
        assert_eq!(custom.args, vec!["--flag".to_string(), "run".to_string()]);

        assert!(resolve("unknown", None, "p", prompt_path).is_err());
        assert!(resolve("custom", None, "p", prompt_path).is_err());
    }

    #[test]
    fn prompt_carries_reply_and_workspace_note() {
        let prompt = build_prompt(
            "p",
            "t",
            "r",
            2,
            Some(&serde_json::json!({ "decision": "approve" })),
            Some("repo-1 -> D:\\work\\repo-1 @ base"),
        );
        assert!(prompt.contains("lease_generation: 2"));
        assert!(prompt.contains("Continuation"));
        assert!(prompt.contains("workspace_note") || prompt.contains("D:\\work\\repo-1"));
        assert!(prompt.contains("interaction.{event_id}.json"));
    }
}
