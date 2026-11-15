//! 仓库事实采集：bare clone 后读 commit / 文件树 / 语言构成 / docs 正文。
//!
//! 上限（spec §九）是硬约束：commit 最近 300 条、文件树 20000、单篇 docs 64KB、
//! docs 总量 400KB。任一维度触顶即 `truncated=true`，不静默扩散成本。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::git_access::{self, GitError};

const GIT_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_commits: usize,
    pub max_files: usize,
    pub max_doc_bytes: usize,
    pub max_docs_total: usize,
    pub max_repo_mb: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_commits: 300,
            max_files: 20_000,
            max_doc_bytes: 64 * 1024,
            max_docs_total: 400 * 1024,
            max_repo_mb: 200,
        }
    }
}

impl Limits {
    pub fn from_env() -> Self {
        let mut limits = Self::default();
        if let Ok(raw) = std::env::var("AI_PM_REPO_MAX_MB") {
            if let Ok(value) = raw.trim().parse::<u64>() {
                if value > 0 {
                    limits.max_repo_mb = value;
                }
            }
        }
        limits
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitFact {
    pub sha: String,
    pub author: String,
    pub authored_at: String,
    pub subject: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HotFile {
    pub path: String,
    pub touches: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LanguageStat {
    pub extension: String,
    pub files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocContent {
    pub path: String,
    pub bytes: usize,
    pub markdown: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoFacts {
    pub head_commit_sha: String,
    pub default_branch: String,
    pub commits: Vec<CommitFact>,
    pub hot_files: Vec<HotFile>,
    pub languages: Vec<LanguageStat>,
    pub file_count: usize,
    pub docs: Vec<DocContent>,
    pub truncated: bool,
    pub size_bytes: u64,
}

/// 仓库缓存根：`AI_PM_REPO_CACHE_ROOT`；未设置时落在系统临时目录
/// （`ai-pm-repo-cache`），绝不写进仓库工作区——否则 `apps/*` 这类
/// workspace 成员 glob 会把 bare clone 当成 crate，直接打断构建。
pub fn cache_root() -> PathBuf {
    match std::env::var("AI_PM_REPO_CACHE_ROOT") {
        Ok(raw) if !raw.trim().is_empty() => PathBuf::from(raw.trim()),
        _ => std::env::temp_dir().join("ai-pm-repo-cache"),
    }
}

pub fn repo_cache_dir(repository_id: uuid::Uuid) -> PathBuf {
    cache_root().join(format!("repo-{repository_id}"))
}

fn branch_is_safe(branch: &str) -> bool {
    !branch.is_empty()
        && !branch.contains("..")
        && branch
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
}

fn is_doc_candidate(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    if ["node_modules/", "target/", "vendor/", "dist/", ".git/"]
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return false;
    }
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    // D-3：范围扩至 docx/pdf（docproc_parse 进程内解析为纯文本）。
    let doc_ext = name.ends_with(".md")
        || name.ends_with(".mdx")
        || name.ends_with(".rst")
        || name.ends_with(".txt")
        || name.ends_with(".docx")
        || name.ends_with(".pdf");
    if name.starts_with("readme") {
        return true;
    }
    if lower.starts_with("docs/") && doc_ext {
        return true;
    }
    // 根级文档：路径里没有 `/`。
    !lower.contains('/') && doc_ext
}

/// 文档字节 → 纯文本（D-3 分叉）：docx/pdf 走 docproc_parse（进程内解析，
/// 失败/不支持 → None=跳过计 truncated）；文本类直读 UTF-8。
fn decode_doc_text(path: &str, bytes: &[u8]) -> Option<String> {
    let lower = path.to_ascii_lowercase();
    let kind = if lower.ends_with(".docx") {
        "docx"
    } else if lower.ends_with(".pdf") {
        "pdf"
    } else {
        return String::from_utf8(bytes.to_vec()).ok();
    };
    let head: Vec<u8> = bytes.iter().take(16).copied().collect();
    let adapter = crate::docproc_parse::adapter_for(kind, &head)?;
    let outcome = adapter
        .parse(crate::docproc_parse::ParseCtx { bytes })
        .ok()?;
    Some(outcome.markdown)
}

fn extension_of(path: &str) -> Option<String> {
    let name = path.rsplit('/').next()?;
    let (_, ext) = name.rsplit_once('.')?;
    if ext.is_empty() || ext.len() > 12 {
        return None;
    }
    Some(ext.to_ascii_lowercase())
}

fn truncate_chars(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

/// 采集一个已 clone 的 bare 仓库。
pub async fn collect(root: &Path, branch: &str, limits: Limits) -> Result<RepoFacts, GitError> {
    if !branch_is_safe(branch) {
        return Err(GitError::Other("unsafe branch name".to_string()));
    }
    let head_commit_sha = String::from_utf8_lossy(
        &git_access::git_stdout(root, &["rev-parse", branch], GIT_TIMEOUT).await?,
    )
    .trim()
    .to_string();

    let mut truncated = false;

    // commits：多取一条用来判断是否触顶。
    let limit = (limits.max_commits + 1).to_string();
    let raw_log = git_access::git_stdout(
        root,
        &[
            "log",
            branch,
            "--format=%H%x1f%an%x1f%aI%x1f%s",
            "-n",
            limit.as_str(),
        ],
        GIT_TIMEOUT,
    )
    .await?;
    let mut commits = Vec::new();
    for line in String::from_utf8_lossy(&raw_log).lines() {
        let mut parts = line.split('\x1f');
        let (Some(sha), Some(author), Some(authored_at), Some(subject)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        commits.push(CommitFact {
            sha: sha.to_string(),
            author: author.to_string(),
            authored_at: authored_at.to_string(),
            subject: truncate_chars(subject, 200),
        });
    }
    if commits.len() > limits.max_commits {
        truncated = true;
        commits.truncate(limits.max_commits);
    }

    // 文件树。
    let raw_tree =
        git_access::git_stdout(root, &["ls-tree", "-r", "--name-only", branch], GIT_TIMEOUT)
            .await?;
    let mut files: Vec<String> = String::from_utf8_lossy(&raw_tree)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect();
    if files.len() > limits.max_files {
        truncated = true;
        files.truncate(limits.max_files);
    }
    let file_count = files.len();

    // 语言构成：扩展名计数前 8。
    let mut lang_counter: HashMap<String, usize> = HashMap::new();
    for file in &files {
        if let Some(ext) = extension_of(file) {
            *lang_counter.entry(ext).or_insert(0) += 1;
        }
    }
    let mut languages: Vec<LanguageStat> = lang_counter
        .into_iter()
        .map(|(extension, files)| LanguageStat { extension, files })
        .collect();
    languages.sort_by(|a, b| b.files.cmp(&a.files).then(a.extension.cmp(&b.extension)));
    languages.truncate(8);

    // 热点文件：最近 300 条 commit 的改动文件聚合。
    let raw_names = git_access::git_stdout(
        root,
        &["log", branch, "--name-only", "--format=", "-n", "300"],
        GIT_TIMEOUT,
    )
    .await?;
    let mut touch_counter: HashMap<String, usize> = HashMap::new();
    for line in String::from_utf8_lossy(&raw_names).lines() {
        let path = line.trim();
        if !path.is_empty() {
            *touch_counter.entry(path.to_string()).or_insert(0) += 1;
        }
    }
    let mut hot_files: Vec<HotFile> = touch_counter
        .into_iter()
        .map(|(path, touches)| HotFile { path, touches })
        .collect();
    hot_files.sort_by(|a, b| b.touches.cmp(&a.touches).then(a.path.cmp(&b.path)));
    hot_files.truncate(20);

    // docs 正文：单篇与总量双上限。
    let mut docs = Vec::new();
    let mut docs_total = 0usize;
    for path in files.iter().filter(|path| is_doc_candidate(path)) {
        let spec = format!("{branch}:{path}");
        let bytes = match git_access::git_stdout(root, &["show", &spec], GIT_TIMEOUT).await {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        if bytes.len() > limits.max_doc_bytes {
            truncated = true;
            continue;
        }
        if docs_total + bytes.len() > limits.max_docs_total {
            truncated = true;
            break;
        }
        let Some(markdown) = decode_doc_text(path, &bytes) else {
            // docx/pdf 解析失败或二进制不可读：跳过并计触顶（D-3 裁决）。
            truncated = true;
            continue;
        };
        docs_total += bytes.len();
        docs.push(DocContent {
            path: path.clone(),
            bytes: bytes.len(),
            markdown,
        });
    }

    let size_bytes = git_access::directory_size(root);
    if size_bytes > limits.max_repo_mb.saturating_mul(1024 * 1024) {
        return Err(GitError::TooLarge);
    }

    Ok(RepoFacts {
        head_commit_sha,
        default_branch: branch.to_string(),
        commits,
        hot_files,
        languages,
        file_count,
        docs,
        truncated,
        size_bytes,
    })
}

/// D-2 · 增量采集：自 `since_commit` 到 `anchor` 的区间（commits / 变更文档 /
/// 区间热点），文件树/语言/体积取当前态。`since_commit` 非锚祖先（force push
/// 等历史重写）→ 回退全量 `collect`（返回 false）。空区间合法（HEAD 未前进的
/// 重分析=重析既有认知，报告照出）。
pub async fn collect_incremental(
    root: &Path,
    anchor: &str,
    since_commit: &str,
    limits: Limits,
) -> Result<(RepoFacts, bool), GitError> {
    if !branch_is_safe(anchor) || !branch_is_safe(since_commit) {
        return Err(GitError::Other("unsafe ref name".to_string()));
    }
    // 祖先校验：since_commit 不在 anchor 历史里 → 区间无意义，回退全量。
    let ancestor = git_access::git_run(
        root,
        &["merge-base", "--is-ancestor", since_commit, anchor],
        GIT_TIMEOUT,
    )
    .await;
    let incremental = matches!(&ancestor, Ok(out) if out.code == 0);
    if !incremental {
        let facts = collect(root, anchor, limits).await?;
        return Ok((facts, false));
    }

    let head_commit_sha = String::from_utf8_lossy(
        &git_access::git_stdout(root, &["rev-parse", anchor], GIT_TIMEOUT).await?,
    )
    .trim()
    .to_string();

    let mut truncated = false;

    // 区间 commits（since_commit..anchor，不含 since_commit 自身）。
    let limit = (limits.max_commits + 1).to_string();
    let raw_log = git_access::git_stdout(
        root,
        &[
            "log",
            anchor,
            &format!("^{since_commit}"),
            "--format=%H%x1f%an%x1f%aI%x1f%s",
            "-n",
            limit.as_str(),
        ],
        GIT_TIMEOUT,
    )
    .await?;
    let mut commits = Vec::new();
    for line in String::from_utf8_lossy(&raw_log).lines() {
        let mut parts = line.split('\x1f');
        let (Some(sha), Some(author), Some(authored_at), Some(subject)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        commits.push(CommitFact {
            sha: sha.to_string(),
            author: author.to_string(),
            authored_at: authored_at.to_string(),
            subject: truncate_chars(subject, 200),
        });
    }
    if commits.len() > limits.max_commits {
        truncated = true;
        commits.truncate(limits.max_commits);
    }

    // 文件树/语言/体积：当前态（与全量同口径）。
    let raw_tree =
        git_access::git_stdout(root, &["ls-tree", "-r", "--name-only", anchor], GIT_TIMEOUT)
            .await?;
    let mut files: Vec<String> = String::from_utf8_lossy(&raw_tree)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect();
    if files.len() > limits.max_files {
        truncated = true;
        files.truncate(limits.max_files);
    }
    let file_count = files.len();
    let mut lang_counter: HashMap<String, usize> = HashMap::new();
    for file in &files {
        if let Some(ext) = extension_of(file) {
            *lang_counter.entry(ext).or_insert(0) += 1;
        }
    }
    let mut languages: Vec<LanguageStat> = lang_counter
        .into_iter()
        .map(|(extension, files)| LanguageStat { extension, files })
        .collect();
    languages.sort_by(|a, b| b.files.cmp(&a.files).then(a.extension.cmp(&b.extension)));
    languages.truncate(8);

    // 区间热点。
    let raw_names = git_access::git_stdout(
        root,
        &[
            "log",
            anchor,
            &format!("^{since_commit}"),
            "--name-only",
            "--format=",
            "-n",
            "300",
        ],
        GIT_TIMEOUT,
    )
    .await?;
    let mut touch_counter: HashMap<String, usize> = HashMap::new();
    for line in String::from_utf8_lossy(&raw_names).lines() {
        let path = line.trim();
        if !path.is_empty() {
            *touch_counter.entry(path.to_string()).or_insert(0) += 1;
        }
    }
    let mut hot_files: Vec<HotFile> = touch_counter
        .into_iter()
        .map(|(path, touches)| HotFile { path, touches })
        .collect();
    hot_files.sort_by(|a, b| b.touches.cmp(&a.touches).then(a.path.cmp(&b.path)));
    hot_files.truncate(20);

    // 区间内变更的文档（ACMR）+ 内容（D-3 范围与预算纪律同全量）。
    let raw_changed = git_access::git_stdout(
        root,
        &[
            "diff",
            "--name-only",
            "--diff-filter=ACMR",
            since_commit,
            anchor,
        ],
        GIT_TIMEOUT,
    )
    .await?;
    let mut docs = Vec::new();
    let mut docs_total = 0usize;
    for line in String::from_utf8_lossy(&raw_changed).lines() {
        let path = line.trim();
        if path.is_empty() || !is_doc_candidate(path) {
            continue;
        }
        let spec = format!("{anchor}:{path}");
        let bytes = match git_access::git_stdout(root, &["show", &spec], GIT_TIMEOUT).await {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        if bytes.len() > limits.max_doc_bytes {
            truncated = true;
            continue;
        }
        if docs_total + bytes.len() > limits.max_docs_total {
            truncated = true;
            break;
        }
        let Some(markdown) = decode_doc_text(path, &bytes) else {
            truncated = true;
            continue;
        };
        docs_total += bytes.len();
        docs.push(DocContent {
            path: path.to_string(),
            bytes: bytes.len(),
            markdown,
        });
    }

    let size_bytes = git_access::directory_size(root);
    if size_bytes > limits.max_repo_mb.saturating_mul(1024 * 1024) {
        return Err(GitError::TooLarge);
    }

    Ok((
        RepoFacts {
            head_commit_sha,
            default_branch: anchor.to_string(),
            commits,
            hot_files,
            languages,
            file_count,
            docs,
            truncated,
            size_bytes,
        },
        true,
    ))
}
/// 一个仓库及其事实，`repository_id` 用于证据回指。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoMaterial {
    pub repository_id: String,
    pub facts: RepoFacts,
}

/// 冷启动仓库分析端口。实现放在 `pm-ai`（Fake / OpenAI 兼容）；
/// 本层只依赖 trait，不反向依赖 `pm-ai`。
#[async_trait::async_trait]
pub trait RepoAnalysisModel: Send + Sync {
    async fn draft(
        &self,
        materials: &[RepoMaterial],
        org: &pm_domain::import::OrgChart,
    ) -> Result<pm_domain::import::ProjectDraft, pm_ai_port::ModelError>;
}

/// 最小的错误类型别名：pm-ai 的 `ModelError` 与本端口契约一致。
/// 这里用独立模块避免 pm-application 依赖 pm-ai。
pub mod pm_ai_port {
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ModelError {
        Timeout,
        Unavailable(String),
        BadOutput(String),
        /// 模型/Provider 配置不可用（§11 Configuration Error；Phase 3B repo
        /// 链切四级 Role 解析后携带显式失败原因）。
        Configuration(String),
    }

    impl std::fmt::Display for ModelError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                ModelError::Timeout => write!(f, "analysis model timed out"),
                ModelError::Unavailable(reason) => {
                    write!(f, "analysis model unavailable: {reason}")
                }
                ModelError::BadOutput(reason) => {
                    write!(f, "analysis model returned bad output: {reason}")
                }
                ModelError::Configuration(reason) => {
                    write!(f, "analysis model configuration error: {reason}")
                }
            }
        }
    }

    impl std::error::Error for ModelError {}
}
