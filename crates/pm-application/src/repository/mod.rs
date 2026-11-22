//! 项目 ↔ 代码仓库绑定（P0：纯关系 CRUD）+ 项目级 SSH 凭据。
//!
//! 本模块只负责「建立与管理关系」：仓库地址、默认分支、角色、备注，以及一把
//! 项目级 ed25519 密钥的生成 / 读取 / 删除。不做 clone、不做可达性校验、不做
//! AI 分析——那些属于后续切片。
//!
//! 密钥策略与 `llm_config.rs` 一致：`AI_PM_SECRET_KEY` → `sha256` → AES-256-GCM，
//! 库里只存 `nonce || ciphertext || tag`，AAD 绑定 `project_id`；私钥明文永不回显。
//! 操作者由调用方显式传入（T-021a：HTTP handler 取 `AuthedPrincipal`），
//! 并写 `governance.audit_log`。

#![allow(unused_imports)]

pub(crate) use chrono::{DateTime, Utc};
pub(crate) use serde::{Deserialize, Serialize};
pub(crate) use sqlx::PgPool;
pub(crate) use uuid::Uuid;

pub(crate) use crate::actor::AuditActor;
pub(crate) use crate::git_access::{self, GitError};
pub(crate) use crate::pm_write::PmWriteError;
pub(crate) use crate::secret::{self, SecretError};

pub const REPOSITORY_ROLES: [&str; 4] = ["main", "docs", "service", "infra"];
const MAX_URL_LEN: usize = 512;
const MAX_NAME_LEN: usize = 120;
const MAX_BRANCH_LEN: usize = 200;
const MAX_COMMIT_SHA_LEN: usize = 64;
const MAX_DESCRIPTION_LEN: usize = 1000;
/// SSH key 名称（label 即身份：同名导入 = 替换该把）。
const MAX_KEY_LABEL_LEN: usize = 64;

mod repo;
mod ssh_key;
pub(crate) mod support;
mod url;

pub use repo::*;
pub use ssh_key::*;
pub use support::*;
pub use url::*;

#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("project not found")]
    ProjectNotFound,
    #[error("repository not found")]
    NotFound,
    #[error("repository ssh key not found")]
    SshKeyNotFound,
    #[error("invalid repository url")]
    InvalidUrl,
    #[error("repository url already bound to this project")]
    Duplicate,
    #[error("revision conflict")]
    RevisionConflict,
    #[error("invalid repository role")]
    InvalidRole,
    #[error("ssh key is still referenced by a verified repository")]
    KeyInUse,
    /// 重命名目标 label 与项目内既有 key 冲突（UNIQUE (project_id, label)）。
    #[error("ssh key label already in use")]
    LabelTaken,
    /// 仓库被历史执行记录引用，不可物理删除（C-4 / 基线 §5.1）。
    #[error("repository has history and cannot be deleted")]
    HasHistory,
    /// 已归档（软解绑）的仓库不得再做版本确认（S01-03 §5：确认基准是活仓库的治理动作）。
    #[error("repository is archived")]
    Archived,
    /// S1 手动增量分析：同仓库已有活跃快照分析（PENDING/DISPATCHED/PROCESSING）。
    /// 与 `intake::retry_task` 的「非 FAILED / NEEDS_HUMAN_REVIEW 不可重试」同族，
    /// 映射 409；终态（含 FAILED / NEEDS_HUMAN_REVIEW）释放名额允许重触发。
    #[error("repository analysis is already in progress")]
    AnalyzeInProgress,
    /// D-5 门：同仓库存在未关闭的分析报告（analyzing/analyzed）——报告关闭后才
    /// 允许再次分析（409，body 带 report_id 供前端深链）。
    #[error("repository has an open analysis report")]
    ReportOpen { report_id: Uuid },
    /// S1 手动增量分析：仓库未核验或没有可用分析锚点（Confirmed Commit / Observed HEAD）。
    #[error("repository has no verified analysis anchor")]
    AnalyzeNoAnchor,
    /// S1 手动增量分析：快照 intake 受理被校验拒绝（映射 422）。
    #[error("repository analysis intake rejected: {0}")]
    AnalyzeIntake(String),
    #[error("git access failed: {0}")]
    Git(String),
    #[error("{0}")]
    Validation(&'static str),
    #[error("AI_PM_SECRET_KEY is not set")]
    SecretKeyMissing,
    #[error("secret could not be encrypted or decrypted")]
    Crypto,
    #[error("ssh-keygen is not available: {0}")]
    SshKeygen(String),
    #[error("database error: {0}")]
    Db(sqlx::Error),
}

impl From<sqlx::Error> for RepositoryError {
    fn from(e: sqlx::Error) -> Self {
        RepositoryError::Db(e)
    }
}

impl From<SecretError> for RepositoryError {
    fn from(e: SecretError) -> Self {
        match e {
            SecretError::KeyMissing => RepositoryError::SecretKeyMissing,
            SecretError::Crypto => RepositoryError::Crypto,
            SecretError::SshKeygen(detail) => RepositoryError::SshKeygen(detail),
            // 导入的私钥无效是用户输入错误 → 400，与「ssh-keygen 不可用」(500) 区分。
            SecretError::InvalidKey(_) => {
                RepositoryError::Validation("private key is invalid or passphrase-protected")
            }
            SecretError::Fingerprint => {
                RepositoryError::SshKeygen("ssh-keygen produced no fingerprint".to_string())
            }
        }
    }
}

impl From<GitError> for RepositoryError {
    fn from(e: GitError) -> Self {
        RepositoryError::Git(e.to_string())
    }
}

impl From<PmWriteError> for RepositoryError {
    fn from(e: PmWriteError) -> Self {
        match e {
            PmWriteError::NotFound(m) => RepositoryError::Validation(m),
            PmWriteError::Conflict(m) => RepositoryError::Validation(m),
            PmWriteError::Validation(m) => RepositoryError::Validation(m),
            PmWriteError::ValidationMsg(m) => {
                RepositoryError::Validation(Box::leak(m.into_boxed_str()))
            }
            // S02-I1：仓库链路不会触发 Forbidden；兜底转校验错误。
            PmWriteError::Forbidden(m) => RepositoryError::Validation(m),
            PmWriteError::Db(err) => RepositoryError::Db(err),
        }
    }
}

// ------------------------------------------------------------------ rows / dto

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct RepositoryRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub url: String,
    pub role: String,
    pub default_branch: Option<String>,
    pub description: Option<String>,
    pub ssh_key_id: Option<Uuid>,
    pub status: String,
    pub head_commit_sha: Option<String>,
    /// 项目采用分支（Confirmed 基准的一半；仅经 update-version 人工确认写入）。
    pub selected_branch: Option<String>,
    /// 项目确认采用的 Commit（S01-03 §5.1 Confirmed Commit；PATCH 不得触碰）。
    pub selected_commit: Option<String>,
    pub selected_confirmed_at: Option<DateTime<Utc>>,
    pub selected_confirmed_by: Option<Uuid>,
    pub last_verified_at: Option<DateTime<Utc>>,
    pub last_error_code: Option<String>,
    pub last_error_detail: Option<String>,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 对外只暴露公钥与指纹；私钥密文不出库。多把 key（迁移 0022）：
/// `id` 供仓库绑定引用，`is_default` = 未绑定仓库的回退目标。
#[derive(Debug, Clone, Serialize)]
pub struct SshKeyView {
    pub id: Uuid,
    pub project_id: Uuid,
    pub label: String,
    pub public_key: String,
    pub fingerprint: String,
    pub status: String,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
}

/// 校验后的 git 地址。`normalized` 是写库的规范形式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitUrl {
    pub user: String,
    pub host: String,
    pub path: String,
    pub normalized: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateRepositoryRequest {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub default_branch: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// 接入时的初始采用分支/Commit（用户在接入流程中显式给出 = 人工确认）。
    /// 提供即写入 Confirmed 基准并落首条确认记录；不提供则留空待确认。
    #[serde(default)]
    pub selected_branch: Option<String>,
    #[serde(default)]
    pub selected_commit: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateRepositoryRequest {
    pub revision: i64,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub default_branch: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    // 注意：本请求**不含** selected_branch/selected_commit —— 确认基准只能走
    // confirm_repository_version（POST /update-version），防止绕过人工确认（S01-03 §5.1）。
}

/// POST /v1/pm/repositories/{id}/update-version：人类确认更新采用基准（G-05）。
/// 不触碰 head_commit_sha（Observed HEAD 只属 verify），也不触碰任何正式基线。
#[derive(Debug, Clone, Deserialize)]
pub struct ConfirmVersionRequest {
    pub selected_branch: String,
    pub selected_commit: String,
    #[serde(default)]
    pub reason: Option<String>,
}

/// POST /v1/pm/repositories/{id}/analyze：手动增量分析（S1，2026-11-13）。
///
/// `idempotency_key` 由调用方持有以防双击双发；服务端把它作为幂等键的
/// 「触发时刻」段，重复提交同一键命中同一次 intake（零副作用重放）。
/// 缺省时服务端按触发时刻生成新段 —— **HEAD 未前进也允许重触发**。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AnalyzeRepositoryRequest {
    #[serde(default)]
    pub idempotency_key: Option<String>,
    /// D-2 运行口径：incremental（缺省，自上一份关闭报告锚点起区间采集）| full
    /// （显式整库重析）。非法值 → 400。
    #[serde(default)]
    pub mode: Option<String>,
}

/// 手动增量分析的受理回执（202 纯受理语义，与 `start_import` 一致）。
#[derive(Debug, Clone, Serialize)]
pub struct AnalyzeRepositoryOutcome {
    pub repository_id: Uuid,
    pub project_id: Uuid,
    /// 本次分析锚定的 commit（Confirmed Commit 优先，回退 Observed HEAD）。
    pub commit: String,
    pub intake_id: Uuid,
    /// D-5：本次分析对应的报告（受理即建，analyzing）。
    pub report_id: Uuid,
    pub status: &'static str,
    /// true = 命中既有 intake 的幂等重放，未新增任何行。
    pub replayed: bool,
}

// -------------------------------------------------------------------- ssh key
