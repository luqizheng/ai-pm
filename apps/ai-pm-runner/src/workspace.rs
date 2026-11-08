//! 代码仓库域工作区准备（V-05-2 / Q-RC-2，检查点 2 §5.2）：
//! 用户自备凭据——Runner 只准备目录（fetch + detach 于 base_commit），clone/
//! push 的凭据全程走用户会话的 git 体系（ssh-agent / credential helper），
//! Runner 不读取、不转存、不注入任何 git 凭据。知识库域与平台只读验证面
//! 与此严格分域（设计稿 §5.1）。

use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, thiserror::Error)]
#[error("{0}")]
pub struct WorkspaceError(pub String);

/// 仓库映射条目（`AI_PM_RUNNER_GIT_MAP` JSON：`{ "<repository_id 或 name>":
/// {"local_path": "...", "url": "..."} }`——local_path 缺失时按 url clone）。
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RepoMapping {
    #[serde(default)]
    pub local_path: Option<PathBuf>,
    #[serde(default)]
    pub url: Option<String>,
}

pub fn load_git_map(path: Option<&Path>) -> Result<HashMap<String, RepoMapping>, WorkspaceError> {
    let Some(path) = path else {
        return Ok(HashMap::new());
    };
    let raw = std::fs::read_to_string(path)
        .map_err(|e| WorkspaceError(format!("git map {} unreadable: {e}", path.display())))?;
    serde_json::from_str(&raw)
        .map_err(|e| WorkspaceError(format!("git map {} is not valid JSON: {e}", path.display())))
}

/// Run 契约中的一仓（`pm_get_run_context` → repositories[] 子集）。
#[derive(Debug, Clone)]
pub struct ContractRepo {
    pub repository_id: String,
    pub name: String,
    pub base_commit: String,
    pub workspace_path: String,
}

/// 每仓准备：映射命中 local_path → fetch + detach 于 base_commit；未命中但配
/// url → clone 到 fallback_dir；都没有 → 失败（fail-fast，用户配置凭据/映射）。
/// 返回该仓的执行根（供工作区地图注入 Agent）。
pub async fn prepare_repo(
    mapping: Option<&RepoMapping>,
    fallback_dir: &Path,
    base_commit: &str,
    timeout: Duration,
) -> Result<PathBuf, WorkspaceError> {
    let local = mapping
        .and_then(|m| m.local_path.clone())
        .unwrap_or_else(|| fallback_dir.to_path_buf());
    if !local.join(".git").exists() {
        let url = mapping
            .and_then(|m| m.url.clone())
            .ok_or_else(|| {
                WorkspaceError(format!(
                    "repository has no local clone at `{}` and no url mapping (prepare it or set git map)",
                    local.display()
                ))
            })?;
        if let Some(parent) = local.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| WorkspaceError(format!("create clone parent failed: {e}")))?;
        }
        run_git(
            local.parent().unwrap_or(Path::new(".")),
            &["clone", &url, &local.to_string_lossy()],
            timeout,
        )
        .await?;
    }
    run_git(&local, &["fetch", "--all", "--prune"], timeout).await?;
    run_git(&local, &["checkout", "--detach", base_commit], timeout).await?;
    Ok(local)
}

/// Run 工作区地图落盘（`.aipm/workspace.json` + env 串）：Agent 据此找到
/// 各仓执行根与基线（V1：Agent 在其 cwd 自理 git，Runner 不代写仓库内容）。
pub fn workspace_note(repos: &[(ContractRepo, PathBuf)]) -> String {
    let lines: Vec<String> = repos
        .iter()
        .map(|(repo, path)| {
            format!(
                "- {} ({}) -> {} @ {}",
                repo.name,
                repo.repository_id,
                path.display(),
                repo.base_commit
            )
        })
        .collect();
    lines.join("\n")
}

pub fn workspace_env(repos: &[(ContractRepo, PathBuf)]) -> Option<(String, String)> {
    if repos.is_empty() {
        return None;
    }
    let map: Value = json!(repos
        .iter()
        .map(|(repo, path)| {
            json!({
                "repository_id": repo.repository_id,
                "name": repo.name,
                "base_commit": repo.base_commit,
                "local_path": path.to_string_lossy(),
            })
        })
        .collect::<Vec<_>>());
    Some((
        "AI_PM_WORKSPACE_MAP".to_string(),
        serde_json::to_string(&map).unwrap_or_default(),
    ))
}

async fn run_git(dir: &Path, args: &[&str], timeout: Duration) -> Result<(), WorkspaceError> {
    let mut cmd = tokio::process::Command::new("git");
    cmd.args(args)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0") // 禁交互——凭据必须来自用户会话体系
        .stdin(std::process::Stdio::null());
    let output = tokio::time::timeout(timeout, cmd.output())
        .await
        .map_err(|_| WorkspaceError(format!("git {} timed out", args.join(" "))))?
        .map_err(|e| WorkspaceError(format!("git {} failed: {e}", args.join(" "))))?;
    if !output.status.success() {
        return Err(WorkspaceError(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

/// 从 Run 契约 JSON 抽取仓库子集（容错：缺 repositories 视为无仓 Run）。
pub fn contract_repos(contract: &Value) -> Vec<ContractRepo> {
    contract
        .get("repositories")
        .and_then(|v| v.as_array())
        .map(|repos| {
            repos
                .iter()
                .filter_map(|repo| {
                    Some(ContractRepo {
                        repository_id: repo.get("repository_id")?.as_str()?.to_string(),
                        name: repo
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        base_commit: repo.get("base_commit")?.as_str()?.to_string(),
                        workspace_path: repo
                            .get("workspace_path")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn git_map_parses_entries() {
        let dir = std::env::temp_dir().join(format!("aipm-ws-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let map_path = dir.join("git-map.json");
        std::fs::write(
            &map_path,
            r#"{"ai-pm": {"local_path": "D:\\work\\ai-pm"},
                 "11111111-1111-1111-1111-111111111111": {"url": "git@host:x/y.git"}}"#,
        )
        .unwrap();
        let map = load_git_map(Some(&map_path)).unwrap();
        assert!(map.contains_key("ai-pm"));
        assert_eq!(
            map.get("ai-pm").unwrap().local_path.as_deref(),
            Some(Path::new("D:\\work\\ai-pm"))
        );
        assert!(map.contains_key("11111111-1111-1111-1111-111111111111"));

        let broken = dir.join("broken.json");
        std::fs::write(&broken, "{nope").unwrap();
        assert!(load_git_map(Some(&broken)).is_err());
        assert!(load_git_map(None).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn contract_repos_extracts_subset() {
        let contract = json!({
            "repositories": [
                {"repository_id": "re-1", "name": "ai-pm", "base_commit": "abc",
                 "workspace_path": "ai-pm", "write_access": true}
            ]
        });
        let repos = contract_repos(&contract);
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].base_commit, "abc");
        assert!(contract_repos(&json!({})).is_empty());
    }

    #[test]
    fn workspace_env_serializes_map() {
        let repo = ContractRepo {
            repository_id: "re-1".into(),
            name: "ai-pm".into(),
            base_commit: "abc".into(),
            workspace_path: "ai-pm".into(),
        };
        let repos = vec![(repo, PathBuf::from("D:\\work\\ai-pm"))];
        let (_, value) = workspace_env(&repos).unwrap();
        assert!(value.contains("re-1"));
        assert!(workspace_note(&repos).contains("ai-pm (re-1) ->"));
        assert!(workspace_env(&[]).is_none());
    }
}
