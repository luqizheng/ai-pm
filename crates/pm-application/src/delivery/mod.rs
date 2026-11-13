//! Slice D4：交付事实与提交管线（`docs/slice-d/ai-pm-slice-d4-delivery-v1.md` §3）。
//!
//! 三层边界不可越：D3 证明 AI 执行过什么；**D4 证明它实际交付了什么**；D5 才能
//! 判断这些交付是否满足任务要求。D4 不裁决任务最终状态，也不改被审对象。
//!
//! 管线（⓪~⑦，单事务）：
//!
//! ```text
//! ⓪ 幂等识别（先于 fencing）→ ① fencing（lock_lease）→ ② 契约仓全集展开
//!   → ③ 上报校验 → ④ 只读仓事实校验 → ⑤ base_commit 比对 → ⑥ 逐仓 git 校验
//!   → ⑦ 落库（SubmitOutcome：Accepted / Rejected 都是正常 COMMIT）
//! ```
//!
//! 关键裁定：
//!
//! - **幂等识别必须在管线最前部**（步骤⓪，先于 fencing）：成功提交后 Run 已非
//!   活跃，步骤① 会直接拒绝重试请求——幂等若放在 UNIQUE 冲突分支就永远不可达。
//! - **校验范围 = 契约仓全集**，不是执行端上报集合（否则漏报只读仓即绕过）。
//! - **业务驳回不是事务失败**：Rejected 是成功提交的业务结果，数据库异常才回滚。
//! - **状态迁移统一经 D1 通道**（`update_task_status` 命令管线 + `validate_transition`），
//!   不直接 SQL 改任务状态。

pub mod git_verify;
pub mod manifest;

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

use pm_domain::TaskStatus;

use crate::command_executor::{apply_task_transition, system_context, TransitionFacts};

pub use git_verify::{
    is_remote_carrier, verify_repository_head, verify_repository_head_remote,
    workspace_root_for_run, GitVerification, GitVerificationError,
};
pub use manifest::{
    list_manifest, list_project_delivery, read_manifest, DeliveryManifest, ManifestEntry,
    ManifestRow, TaskDeliveryRollup,
};

/// 提交管线的事务类型（`'static`——`PgPool::begin` 返回的就是它）。
type Tx = sqlx::Transaction<'static, sqlx::Postgres>;

/// 提交管线的错误码（设计稿 §4.1 九项）。
#[derive(Debug, thiserror::Error)]
pub enum DeliveryError {
    /// token 错 / 世代不符 / Run 非活跃 / 跨项目。
    #[error("lease_invalid: {0}")]
    LeaseInvalid(String),
    /// 租约已过期或绝对执行截止已过。
    #[error("lease_expired: {0}")]
    LeaseExpired(String),
    /// `write_access=false` 的仓出现 commit（整单驳回）。
    #[error("readonly_repo_commit: {0}")]
    ReadonlyRepoCommit(String),
    /// 声明 base_commit ≠ 领取快照。
    #[error("baseline_mismatch: {0}")]
    BaselineMismatch(String),
    /// commit 不存在 / 非 Commit 类型 / 非 base_commit 后代 / ≠ 工作区 HEAD。
    #[error("commit_not_descendant: {0}")]
    CommitNotDescendant(String),
    /// 上报仓不在任务契约集合。
    #[error("repo_not_in_contract: {0}")]
    RepoNotInContract(String),
    /// 相同 Run 重复提交但指纹不同。
    #[error("submit_conflict: {0}")]
    SubmitConflict(String),
    /// `integration_status` 取值非法。
    #[error("integration_status_invalid: {0}")]
    IntegrationStatusInvalid(String),
    /// 输入完整性校验失败（§4.2 九项）。
    #[error("invalid_request: {0}")]
    InvalidRequest(String),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    /// 任务状态迁移被状态机拒绝。
    #[error("transition refused: {0}")]
    Transition(String),
}

impl DeliveryError {
    /// 错误码（机器可读），用于 MCP `isError` 文本与 HTTP 映射。
    pub fn code(&self) -> &'static str {
        match self {
            Self::LeaseInvalid(_) => "lease_invalid",
            Self::LeaseExpired(_) => "lease_expired",
            Self::ReadonlyRepoCommit(_) => "readonly_repo_commit",
            Self::BaselineMismatch(_) => "baseline_mismatch",
            Self::CommitNotDescendant(_) => "commit_not_descendant",
            Self::RepoNotInContract(_) => "repo_not_in_contract",
            Self::SubmitConflict(_) => "submit_conflict",
            Self::IntegrationStatusInvalid(_) => "integration_status_invalid",
            Self::InvalidRequest(_) => "invalid_request",
            Self::Db(_) => "db_error",
            Self::Transition(_) => "transition_refused",
        }
    }
}

/// `integration_status` 的合法取值（Q-D4-6）。
pub const INTEGRATION_STATUSES: [&str; 3] = ["passed", "failed", "not_run"];

/// 提交仓数量上限（§3.4「限制提交仓库数量与校验规模」）。
pub const MAX_SUBMITTED_REPOSITORIES: usize = 64;

/// 单条 `evidence_ref` 的长度上限（§4.2）。
pub const MAX_EVIDENCE_REF_LEN: usize = 512;

/// 提交声明里允许的最大测试条数（§3.4 规模约束）。
pub const MAX_TESTS_PER_REPOSITORY: usize = 200;

/// git 校验的单命令超时（§3.4「Git 命令设置明确超时」）。
pub const GIT_VERIFY_TIMEOUT: Duration = Duration::from_secs(20);

/// 提交管线的可调项（生产用默认值；测试可覆盖工作区根与超时）。
#[derive(Debug, Clone)]
pub struct DeliveryOptions {
    /// 工作区基目录（`{base}/{run_id}/{workspace_path}`）。默认读
    /// `AI_PM_WORKSPACE_ROOT`，缺省 `./run-workspaces`。
    pub workspace_base: PathBuf,
    /// git 校验的单命令超时。
    pub git_timeout: Duration,
}

impl Default for DeliveryOptions {
    fn default() -> Self {
        Self {
            workspace_base: PathBuf::from(crate::run_dispatch::workspace_root_from_env()),
            git_timeout: GIT_VERIFY_TIMEOUT,
        }
    }
}

impl DeliveryOptions {
    /// 从环境装配（生产入口）。
    pub fn from_env() -> Self {
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// 契约入参（D4 定稿：在 D2 draft 形状上只加 `integration_status`）
// ---------------------------------------------------------------------------

/// 一个仓库的交付声明。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmittedRepository {
    pub repository_id: Uuid,
    pub base_commit: String,
    pub commit_sha: String,
    pub changed_files: i32,
    pub tests: Vec<SubmittedTest>,
}

/// 一条测试证据（结论 ≠ 证据：这里只记录事实，不做裁决）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmittedTest {
    pub name: String,
    pub passed: bool,
    pub evidence_ref: Option<String>,
}

/// 执行端提出的知识建议（**只记数不落提案**——知识回写归 D5）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeProposalInput {
    pub document_id: Uuid,
    pub change_summary: String,
}

/// `pm_submit_result` 的入参。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitResultRequest {
    pub project_id: Uuid,
    pub run_id: Uuid,
    pub lease_token: String,
    pub lease_generation: i32,
    /// Run 级集成事实；缺省 `not_run`（draft 兼容）。
    pub integration_status: Option<String>,
    pub repositories: Vec<SubmittedRepository>,
    pub summary: String,
    pub knowledge_proposals: Vec<KnowledgeProposalInput>,
    /// T-088：认证调用人的完整审计身份（MCP 路径由工具层填入；worker / 测试为 None
    /// → 审计回退为仅执行主体）。不进 wire 契约。
    #[serde(skip)]
    pub caller: Option<crate::actor::AuditActor>,
}

// ---------------------------------------------------------------------------
// 事务结果模型（§3.2）
// ---------------------------------------------------------------------------

/// 一次提交的事务结果：**Accepted 与 Rejected 都是正常 COMMIT**。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum SubmitOutcome {
    /// 全部校验通过：交付事实已落库，任务已推进到 `submitted`。
    Accepted { manifest: DeliveryManifest },
    /// 业务驳回：**0 行 commit**，驳回原因与上报摘要已入审计。
    Rejected { rejection: DeliveryRejection },
}

impl SubmitOutcome {
    pub fn manifest(&self) -> Option<&DeliveryManifest> {
        match self {
            Self::Accepted { manifest } => Some(manifest),
            Self::Rejected { .. } => None,
        }
    }
    pub fn rejection(&self) -> Option<&DeliveryRejection> {
        match self {
            Self::Accepted { .. } => None,
            Self::Rejected { rejection } => Some(rejection),
        }
    }
}

/// 驳回结果（`delivery.rejected` 审计摘要的事实载体）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryRejection {
    pub code: String,
    pub message: String,
    /// 上报摘要（不含完整 `knowledge_proposals` 内容——审计不膨胀）。
    pub submitted_repository_count: usize,
    pub knowledge_proposal_count: usize,
}

// ---------------------------------------------------------------------------
// 幂等指纹（§3）
// ---------------------------------------------------------------------------

/// 对规范化后的完整提交清单计算摘要。
///
/// 规范化：仓按 `repository_id` 排序、字段按固定顺序、`tests` 按原序；空与缺省
/// 字段补成定长值。**指纹必配** `UNIQUE(task_run_id, repository_id)`——后者只能
/// 证明「库里已有该仓提交记录」，不能证明「第二次请求与第一次是同一个提交」。
pub fn request_fingerprint(
    integration_status: &str,
    repositories: &[SubmittedRepository],
) -> String {
    let mut sorted: Vec<&SubmittedRepository> = repositories.iter().collect();
    sorted.sort_by_key(|repo| repo.repository_id);
    let mut hasher = Sha256::new();
    hasher.update(integration_status.as_bytes());
    hasher.update([0u8]);
    for repo in sorted {
        hasher.update(repo.repository_id.as_bytes());
        hasher.update([0u8]);
        hasher.update(repo.base_commit.as_bytes());
        hasher.update([0u8]);
        hasher.update(repo.commit_sha.as_bytes());
        hasher.update([0u8]);
        hasher.update(repo.changed_files.to_be_bytes());
        hasher.update([0u8]);
        hasher.update((repo.tests.len() as u64).to_be_bytes());
        for test in &repo.tests {
            hasher.update(test.name.as_bytes());
            hasher.update([0u8]);
            hasher.update([u8::from(test.passed)]);
            hasher.update([0u8]);
            hasher.update(test.evidence_ref.as_deref().unwrap_or("").as_bytes());
            hasher.update([0u8]);
        }
    }
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

// ---------------------------------------------------------------------------
// 输入完整性校验（§4.2 九项）
// ---------------------------------------------------------------------------

fn validate_request(request: &SubmitResultRequest) -> Result<String, DeliveryError> {
    let integration_status = request
        .integration_status
        .clone()
        .unwrap_or_else(|| "not_run".to_string());
    if !INTEGRATION_STATUSES.contains(&integration_status.as_str()) {
        return Err(DeliveryError::IntegrationStatusInvalid(integration_status));
    }
    if request.repositories.len() > MAX_SUBMITTED_REPOSITORIES {
        return Err(DeliveryError::InvalidRequest(format!(
            "too many repositories: {} > {}",
            request.repositories.len(),
            MAX_SUBMITTED_REPOSITORIES
        )));
    }
    let mut seen: HashSet<Uuid> = HashSet::new();
    for repo in &request.repositories {
        if !seen.insert(repo.repository_id) {
            return Err(DeliveryError::InvalidRequest(format!(
                "duplicate repository_id {}",
                repo.repository_id
            )));
        }
        if repo.commit_sha.trim().is_empty() || repo.commit_sha.len() > 128 {
            return Err(DeliveryError::InvalidRequest(format!(
                "invalid commit_sha for repository {}",
                repo.repository_id
            )));
        }
        if repo.base_commit.trim().is_empty() || repo.base_commit.len() > 128 {
            return Err(DeliveryError::InvalidRequest(format!(
                "invalid base_commit for repository {}",
                repo.repository_id
            )));
        }
        if repo.changed_files < 0 {
            return Err(DeliveryError::InvalidRequest(format!(
                "negative changed_files for repository {}",
                repo.repository_id
            )));
        }
        if repo.tests.len() > MAX_TESTS_PER_REPOSITORY {
            return Err(DeliveryError::InvalidRequest(format!(
                "too many tests for repository {}",
                repo.repository_id
            )));
        }
        for test in &repo.tests {
            if test.name.trim().is_empty() {
                return Err(DeliveryError::InvalidRequest(format!(
                    "test without a name for repository {}",
                    repo.repository_id
                )));
            }
            if let Some(reference) = &test.evidence_ref {
                if reference.len() > MAX_EVIDENCE_REF_LEN {
                    return Err(DeliveryError::InvalidRequest(format!(
                        "evidence_ref too long for repository {}",
                        repo.repository_id
                    )));
                }
            }
        }
    }
    Ok(integration_status)
}

// ---------------------------------------------------------------------------
// 库内事实
// ---------------------------------------------------------------------------

/// 契约仓全集（步骤②）：任务声明的全部仓库，含可写与只读。
#[derive(Debug, Clone)]
pub struct ContractRepository {
    pub repository_id: Uuid,
    pub name: String,
    pub write_access: bool,
}

/// 本 Run 的基线快照（D3 领取时写入 `delivery.task_run_repository`）。
#[derive(Debug, Clone)]
pub struct RunBaseline {
    pub repository_id: Uuid,
    pub base_commit: String,
    pub workspace_path: String,
    pub write_access: bool,
}

/// 已落库的提交（步骤⓪的幂等识别用）。
#[derive(Debug, Clone)]
pub struct ExistingSubmission {
    pub run_id: Uuid,
    pub request_fingerprint: Option<String>,
    pub committed_repositories: usize,
}

// ---------------------------------------------------------------------------
// 主入口
// ---------------------------------------------------------------------------

/// 提交交付：**单事务**完成 ⓪~⑦。
///
/// 业务驳回（Rejected）与数据库异常是两条路：前者正常提交（驳回审计可靠存在、
/// 0 行 commit），后者才回滚。
pub async fn submit_delivery(
    pool: &sqlx::PgPool,
    request: &SubmitResultRequest,
) -> Result<SubmitOutcome, DeliveryError> {
    submit_delivery_with(pool, request, &DeliveryOptions::from_env()).await
}

/// 同 [`submit_delivery`]，但可注入工作区根与 git 超时（测试用）。
pub async fn submit_delivery_with(
    pool: &sqlx::PgPool,
    request: &SubmitResultRequest,
    options: &DeliveryOptions,
) -> Result<SubmitOutcome, DeliveryError> {
    // 步骤③ 的输入完整性校验在事务外先做——它是纯函数校验，不产生副作用，
    // 也不需要行锁。
    let integration_status = validate_request(request)?;
    let fingerprint = request_fingerprint(&integration_status, &request.repositories);

    let mut tx = pool.begin().await?;

    // ⓪ 幂等识别（**先于 fencing**）。
    if let Some(existing) = load_existing_submission(&mut tx, request.run_id).await? {
        return match existing.request_fingerprint.as_deref() {
            Some(stored) if stored == fingerprint => {
                let manifest = read_manifest(&mut tx, request.run_id).await?;
                tx.commit().await?;
                Ok(SubmitOutcome::Accepted { manifest })
            }
            _ => {
                // 指纹不一致（或既有提交没有指纹）：同一 Run 重复提交但内容不同
                // → 409。**不产生第二份审计**（幂等分支不是新的业务动作）。
                tx.rollback().await?;
                Err(DeliveryError::SubmitConflict(format!(
                    "run {} already has a submission with a different fingerprint",
                    existing.run_id
                )))
            }
        };
    }

    // ① fencing（事务内行锁 + 重验 token / generation / 状态 / 过期）。
    let lease = crate::run_context::lock_lease(
        &mut tx,
        request.project_id,
        request.run_id,
        &request.lease_token,
        request.lease_generation,
    )
    .await
    .map_err(map_lease_error)?;

    // 锁 Task（顺序 run → task，B2）。
    let task_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM delivery.task WHERE id = $1 AND project_id = $2 FOR UPDATE",
    )
    .bind(lease.task_id)
    .bind(request.project_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| DeliveryError::LeaseInvalid("task not found in this project".into()))?;

    // 验证模式判定（V-05-2 / 检查点 2 Q-RC-1）：cpk 通道（`client:{device_id}`）
    // 的工作区在用户 PC 上，平台改走远端只读验证；Worker / Model A 维持工作区模式。
    let lease_owner: Option<String> =
        sqlx::query_scalar("SELECT lease_owner FROM delivery.task_run WHERE id = $1")
            .bind(request.run_id)
            .fetch_optional(&mut *tx)
            .await?
            .flatten();
    let remote_mode = is_remote_carrier(lease_owner.as_deref());

    // ② 契约仓全集展开（校验范围 = 契约全集，不是上报集合）。
    let contract = load_contract_repositories(&mut tx, task_id, request.project_id).await?;
    let baselines = load_run_baselines(&mut tx, request.run_id).await?;

    // 步骤③：上报仓 ∈ 契约集合。
    let contract_ids: HashSet<Uuid> = contract.iter().map(|repo| repo.repository_id).collect();
    for submitted in &request.repositories {
        if !contract_ids.contains(&submitted.repository_id) {
            return finish_rejected(
                tx,
                request,
                DeliveryError::RepoNotInContract(format!(
                    "repository {} is not in the task contract",
                    submitted.repository_id
                )),
            )
            .await;
        }
    }

    // 步骤④ 只读仓事实校验 + 步骤⑤ base_commit 比对 + 步骤⑥ 逐仓 git 校验。
    //
    // 只读仓也要检查**实际 HEAD 是否变化**——执行端漏报只读仓不等于它没动过
    // （§3.3）。检查范围是 `base_commit → 当前工作区 HEAD`。
    let workspace_root = workspace_root_for_run(&options.workspace_base, request.run_id);
    let mut verified: Vec<(ContractRepository, GitVerification)> = Vec::new();
    for repo in &contract {
        let baseline = baselines
            .iter()
            .find(|item| item.repository_id == repo.repository_id)
            .ok_or_else(|| {
                DeliveryError::BaselineMismatch(format!(
                    "repository {} has no baseline snapshot for this run",
                    repo.repository_id
                ))
            })?;

        // 可写仓：若上报了，就校验声明的 base_commit == 领取快照（库层防篡改，
        // 步骤⑤ 先于步骤⑥——基线不匹配不必再进 git 子进程）。
        //
        // **没上报的可写仓不驳回**：D4 §5 明确 `submitted` ≠ 交付完整——提交
        // 2/3 个仓仍是有效的 D4 提交事实，视图据此显示 `partial`，最终由 D5
        // 裁决。可写仓的「漏报」由 `actual_delivery_count` 如实计数，不在此处
        // 替 D5 下结论。
        let submitted = if repo.write_access {
            match request
                .repositories
                .iter()
                .find(|item| item.repository_id == repo.repository_id)
            {
                Some(submitted) => {
                    if submitted.base_commit != baseline.base_commit {
                        return finish_rejected(
                            tx,
                            request,
                            DeliveryError::BaselineMismatch(format!(
                                "repository {} declared base {} but the run baseline is {}",
                                repo.repository_id, submitted.base_commit, baseline.base_commit
                            )),
                        )
                        .await;
                    }
                    Some(submitted)
                }
                None => continue,
            }
        } else {
            None
        };

        let verification = if remote_mode {
            // ── 远端模式（V-05-2 / 检查点 2 Q-RC-1）──────────────────
            // cpk 通道的工作区在用户 PC，平台只读验证申报 SHA：
            // 只读仓的工作区事实远端不可见 → 诚实跳过移动性检查（第三方推进
            // 不可归因于申报链路，不由平台伪造检查）；可写仓走远端两步验证。
            if !repo.write_access {
                tracing::info!(
                    run = %request.run_id,
                    repository = %repo.repository_id,
                    "remote mode: read-only repository movement check skipped (workspace on executor)"
                );
                continue;
            }
            let submitted = submitted.expect("writable repository reported above");
            // 迁移 0022：绑定把优先，未绑定（或绑定已失效）回退项目默认/最新 active。
            let row: Option<(String, Option<Vec<u8>>)> = sqlx::query_as(
                "SELECT pr.url, COALESCE(bound.private_key_ciphertext, fb.private_key_ciphertext) \
                   FROM integration.project_repository pr \
                   LEFT JOIN integration.project_ssh_key bound \
                     ON bound.id = pr.ssh_key_id AND bound.status = 'active' \
                   LEFT JOIN LATERAL (\
                     SELECT k.private_key_ciphertext \
                       FROM integration.project_ssh_key k \
                      WHERE k.project_id = pr.project_id AND k.status = 'active' \
                      ORDER BY k.is_default DESC, k.created_at DESC LIMIT 1\
                   ) fb ON pr.ssh_key_id IS NULL \
                  WHERE pr.id = $1 AND pr.project_id = $2",
            )
            .bind(repo.repository_id)
            .bind(request.project_id)
            .fetch_optional(&mut *tx)
            .await?;
            let remote = verify_remote_submission(
                &row,
                request.project_id,
                &baseline.base_commit,
                submitted,
                options.git_timeout,
            )
            .await;
            match remote {
                Ok(verification) => verification,
                Err(message) => {
                    // git 异常/认证失败/申报不可达统一走 Rejected 路径（§3.4 同款）。
                    return finish_rejected(
                        tx,
                        request,
                        DeliveryError::CommitNotDescendant(message),
                    )
                    .await;
                }
            }
        } else {
            match verify_repository_head(
                &workspace_root,
                &baseline.workspace_path,
                &baseline.base_commit,
                submitted,
                options.git_timeout,
            )
            .await
            {
                Ok(verification) => verification,
                Err(error) => {
                    // git 异常与超时统一走 Rejected 路径（§3.4）。
                    return finish_rejected(
                        tx,
                        request,
                        DeliveryError::CommitNotDescendant(error.to_string()),
                    )
                    .await;
                }
            }
        };

        if !repo.write_access {
            // 只读仓：工作区 HEAD 必须仍等于基线——任何变化即整单驳回。
            if verification.head_sha != baseline.base_commit {
                return finish_rejected(
                    tx,
                    request,
                    DeliveryError::ReadonlyRepoCommit(format!(
                        "read-only repository {} moved from {} to {}",
                        repo.repository_id, baseline.base_commit, verification.head_sha
                    )),
                )
                .await;
            }
            continue;
        }

        verified.push((repo.clone(), verification));
    }

    // 步骤⑥ 的最后一道：`changed_files` 以 Git 现算为准，执行端上报只做一致性
    // 比对，不一致即拒绝（Q-D4-3）。
    for (repo, verification) in &verified {
        let submitted = request
            .repositories
            .iter()
            .find(|item| item.repository_id == repo.repository_id)
            .expect("writable repositories are reported");
        if submitted.changed_files != verification.changed_files {
            return finish_rejected(
                tx,
                request,
                DeliveryError::CommitNotDescendant(format!(
                    "repository {} reported {} changed files but git counted {}",
                    repo.repository_id, submitted.changed_files, verification.changed_files
                )),
            )
            .await;
        }
    }

    // 步骤⑦ 落库。
    let manifest = persist_accepted(
        &mut tx,
        request,
        &task_id,
        &integration_status,
        &fingerprint,
        &verified,
    )
    .await?;

    tx.commit().await?;
    Ok(SubmitOutcome::Accepted { manifest })
}

/// 远端模式的单仓验证（Q-RC-1）：解析仓库 URL 与平台只读部署密钥（SSH 形态
/// 才注入；HTTP(S) 远端以匿名可读性如实尝试），临时密钥 RAII 用完即删，
/// 核心校验委托 [`verify_repository_head_remote`]。任何失败都归并为字符串
/// 由调用方走 Rejected 路径——错误文案保持机器可读错误码前缀。
async fn verify_remote_submission(
    row: &Option<(String, Option<Vec<u8>>)>,
    project_id: Uuid,
    base_commit: &str,
    submitted: &SubmittedRepository,
    timeout: Duration,
) -> Result<GitVerification, String> {
    let (url, key_ciphertext) = row.as_ref().ok_or_else(|| {
        format!(
            "workspace_unavailable: repository {} not found for project {project_id}",
            submitted.repository_id
        )
    })?;
    let is_ssh = url.starts_with("git@") || url.starts_with("ssh://");
    let key_material =
        match (is_ssh, key_ciphertext) {
            (true, Some(cipher)) => {
                let plain = crate::repository::open_ssh_private_key(project_id, cipher)
                    .map_err(|e| format!("git_verification_failed: deploy key unavailable: {e}"))?;
                Some(crate::git_access::write_temp_key(&plain).map_err(|e| {
                    format!("git_verification_failed: temp key material failed: {e}")
                })?)
            }
            _ => None,
        };
    let key_path = key_material.as_ref().map(|material| material.key_path());
    git_verify::verify_repository_head_remote(
        url,
        key_path.as_deref(),
        base_commit,
        &submitted.commit_sha,
        submitted.changed_files,
        timeout,
    )
    .await
    .map_err(|e| e.to_string())
}

/// 步骤⓪的幂等识别：查既有提交。
async fn load_existing_submission(
    tx: &mut Tx,
    run_id: Uuid,
) -> Result<Option<ExistingSubmission>, DeliveryError> {
    let row = sqlx::query(
        "SELECT r.request_fingerprint, \
                (SELECT count(*) FROM delivery.task_repository_commit c \
                  WHERE c.task_run_id = r.id)::int AS commit_count \
           FROM delivery.task_run r WHERE r.id = $1",
    )
    .bind(run_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    if row.get::<i32, _>("commit_count") == 0 {
        return Ok(None);
    }
    Ok(Some(ExistingSubmission {
        run_id,
        request_fingerprint: row.get("request_fingerprint"),
        committed_repositories: row.get::<i32, _>("commit_count") as usize,
    }))
}

/// 契约仓全集（步骤②）。
async fn load_contract_repositories(
    tx: &mut Tx,
    task_id: Uuid,
    project_id: Uuid,
) -> Result<Vec<ContractRepository>, DeliveryError> {
    let rows = sqlx::query(
        "SELECT tr.repository_id, pr.name, tr.write_access \
           FROM delivery.task_repository tr \
           JOIN integration.project_repository pr \
             ON pr.id = tr.repository_id AND pr.project_id = tr.project_id \
          WHERE tr.task_id = $1 AND tr.project_id = $2 \
          ORDER BY tr.repository_id",
    )
    .bind(task_id)
    .bind(project_id)
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| ContractRepository {
            repository_id: row.get("repository_id"),
            name: row.get("name"),
            write_access: row.get("write_access"),
        })
        .collect())
}

/// 本 Run 的基线快照（D3 领取快照——基线事实的唯一来源）。
async fn load_run_baselines(tx: &mut Tx, run_id: Uuid) -> Result<Vec<RunBaseline>, DeliveryError> {
    let rows = sqlx::query(
        "SELECT repository_id, base_commit, workspace_path, write_access \
           FROM delivery.task_run_repository WHERE run_id = $1 ORDER BY repository_id",
    )
    .bind(run_id)
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| RunBaseline {
            repository_id: row.get("repository_id"),
            base_commit: row.get("base_commit"),
            workspace_path: row.get("workspace_path"),
            write_access: row.get("write_access"),
        })
        .collect())
}

/// 步骤⑦：写 commit 行 + Run 集成事实 + 任务状态迁移 + 审计。
async fn persist_accepted(
    tx: &mut Tx,
    request: &SubmitResultRequest,
    task_id: &Uuid,
    integration_status: &str,
    fingerprint: &str,
    verified: &[(ContractRepository, GitVerification)],
) -> Result<DeliveryManifest, DeliveryError> {
    let submitted_at = chrono::Utc::now().to_rfc3339();
    let mut entries: Vec<ManifestEntry> = Vec::new();
    for (repo, verification) in verified {
        let submitted = request
            .repositories
            .iter()
            .find(|item| item.repository_id == repo.repository_id)
            .expect("writable repositories are reported");
        let test_evidence = serde_json::json!({
            "submitted_at": submitted_at,
            "tests": submitted.tests,
        });
        sqlx::query(
            "INSERT INTO delivery.task_repository_commit \
             (tenant_id, project_id, task_run_id, repository_id, commit_sha, branch, \
              changed_files, test_evidence, commit_author, commit_authored_at) \
             VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, $3, $4, $5, \
                     $6, $7, $8, $9)",
        )
        .bind(request.project_id)
        .bind(request.run_id)
        .bind(repo.repository_id)
        .bind(&verification.head_sha)
        .bind(&verification.branch)
        .bind(verification.changed_files)
        .bind(&test_evidence)
        .bind(&verification.author)
        .bind(verification.authored_at)
        .execute(&mut **tx)
        .await?;
        entries.push(ManifestEntry {
            repository_id: repo.repository_id,
            repository_name: repo.name.clone(),
            write_access: repo.write_access,
            base_commit: request
                .repositories
                .iter()
                .find(|item| item.repository_id == repo.repository_id)
                .map(|item| item.base_commit.clone())
                .unwrap_or_default(),
            commit_sha: verification.head_sha.clone(),
            changed_files: verification.changed_files,
            branch: verification.branch.clone(),
            commit_author: verification.author.clone(),
            commit_authored_at: verification.authored_at,
            test_evidence,
        });
    }

    // Run 级集成事实（单一事实来源）+ 提交指纹。
    sqlx::query(
        "UPDATE delivery.task_run SET integration_status = $2, request_fingerprint = $3 WHERE id = $1",
    )
    .bind(request.run_id)
    .bind(integration_status)
    .bind(fingerprint)
    .execute(&mut **tx)
    .await?;

    // 任务状态迁移：统一经 D1 受守卫通道（`validate_transition` + 行锁 + 审计）。
    // E1 守卫要求 `delivery_submitted` 事实——本次提交已全部校验通过，事实成立。
    let ctx = system_context(tx, request.project_id)
        .await
        .map_err(|e| DeliveryError::Transition(e.to_string()))?;
    apply_task_transition(
        tx,
        *task_id,
        None,
        TaskStatus::Submitted,
        &TransitionFacts::for_command_channel(),
        &ctx,
    )
    .await
    .map_err(|e| DeliveryError::Transition(e.to_string()))?;

    // 审计摘要（§3.5）：只记数不记内容。
    audit_delivery(
        tx,
        request.project_id,
        request.caller.as_ref(),
        "delivery.submitted",
        request.run_id,
        serde_json::json!({
            "repository_count": verified.len(),
            "commit_count": entries.len(),
            "knowledge_proposal_count": request.knowledge_proposals.len(),
            "integration_status": integration_status,
        }),
    )
    .await?;

    // D5 拍① 的触发源（§9）：交付事实与事件**同事务**。幂等键以 delivery run 钉死
    // ——同一 run 的重复提交坍缩为同一事件；消费侧另有锚定哈希判据兜底。
    if crate::outbox::event_is_enabled("DeliverySubmitted") {
        use crate::outbox::{OutboxAppend, OutboxEvent};
        let aggregate_id = request.run_id.to_string();
        let idempotency_key = format!("delivery_submitted:{}", request.run_id);
        crate::outbox::PgOutboxAppend
            .append(
                tx,
                OutboxEvent {
                    project_id: request.project_id,
                    event_type: "DeliverySubmitted",
                    aggregate_type: "TaskRun",
                    aggregate_id: &aggregate_id,
                    payload: serde_json::json!({
                        "task_id": task_id,
                        "run_id": request.run_id,
                        "integration_status": integration_status,
                        "commit_count": entries.len(),
                    }),
                    idempotency_key: &idempotency_key,
                },
            )
            .await?;
    }

    Ok(DeliveryManifest {
        run_id: request.run_id,
        task_id: *task_id,
        integration_status: integration_status.to_string(),
        commit_count: entries.len(),
        knowledge_proposal_count: request.knowledge_proposals.len(),
        repositories: entries,
    })
}

/// 业务驳回：**0 行 commit**，只写驳回审计，然后正常 COMMIT。
///
/// 事务由调用方移入——本函数负责收尾（审计 + COMMIT），调用方随即返回。
async fn finish_rejected(
    mut tx: Tx,
    request: &SubmitResultRequest,
    error: DeliveryError,
) -> Result<SubmitOutcome, DeliveryError> {
    let rejection = DeliveryRejection {
        code: error.code().to_string(),
        message: error.to_string(),
        submitted_repository_count: request.repositories.len(),
        knowledge_proposal_count: request.knowledge_proposals.len(),
    };
    audit_delivery(
        &mut tx,
        request.project_id,
        request.caller.as_ref(),
        "delivery.rejected",
        request.run_id,
        serde_json::json!({
            "code": rejection.code,
            "message": rejection.message,
            "repository_count": rejection.submitted_repository_count,
            "knowledge_proposal_count": rejection.knowledge_proposal_count,
        }),
    )
    .await?;
    tx.commit().await?;
    Ok(SubmitOutcome::Rejected { rejection })
}

/// 写一条 `governance.audit_log`（与业务变更同事务）。
///
/// T-088：`caller` 是认证调用人的完整审计身份（真人三列 + 设备归因）；`None`
/// 时回退为仅执行主体（项目 human 成员 / dispatch service，真人三列 NULL——不猜）。
async fn audit_delivery(
    tx: &mut Tx,
    project_id: Uuid,
    caller: Option<&crate::actor::AuditActor>,
    action: &str,
    run_id: Uuid,
    after: serde_json::Value,
) -> Result<(), DeliveryError> {
    let actor = match caller {
        Some(a) => a.clone(),
        None => {
            let guessed: Option<Uuid> = sqlx::query_scalar(
                "SELECT p.id FROM core.project_member m \
                   JOIN core.principal p ON p.id = m.principal_id \
                  WHERE m.project_id = $1 AND p.kind = 'human' AND p.is_active \
                  LIMIT 1",
            )
            .bind(project_id)
            .fetch_optional(&mut **tx)
            .await?;
            crate::actor::AuditActor::principal(
                guessed.unwrap_or(crate::run_dispatch::DISPATCH_SERVICE_PRINCIPAL),
            )
        }
    };
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (project_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
          action, target_type, target_id, after_data, source_kind, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'task_run', $8, $9, 'run_dispatch', \
                 (SELECT tenant_id FROM core.project WHERE id = $1))",
    )
    .bind(project_id)
    .bind(actor.principal_id)
    .bind(actor.identity_id)
    .bind(&actor.client)
    .bind(&actor.auth_mode)
    .bind(actor.claimed_device_id)
    .bind(action)
    .bind(run_id)
    .bind(after)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn map_lease_error(error: crate::run_context::LeaseError) -> DeliveryError {
    use crate::run_context::LeaseError as E;
    match error {
        E::NotFound => DeliveryError::LeaseInvalid("run not found in this project".into()),
        E::Forbidden => DeliveryError::LeaseInvalid("lease credential rejected".into()),
        E::Expired => DeliveryError::LeaseExpired("lease expired; claim the task again".into()),
        E::Conflict(message) => DeliveryError::LeaseInvalid(message),
        E::Db(e) => DeliveryError::Db(e),
        E::Random => DeliveryError::LeaseInvalid("lease token generation failed".into()),
    }
}

/// 按仓分组的上报摘要（审计用；不含完整 `knowledge_proposals` 内容）。
pub fn submitted_summary(
    repositories: &[SubmittedRepository],
) -> BTreeMap<Uuid, (String, String, i32)> {
    repositories
        .iter()
        .map(|repo| {
            (
                repo.repository_id,
                (
                    repo.base_commit.clone(),
                    repo.commit_sha.clone(),
                    repo.changed_files,
                ),
            )
        })
        .collect()
}
