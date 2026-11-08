//! Checkpoint-1 · KB/PC 分析本体——**仓库快照路径**（verify 触发链的执行端）。
//!
//! 范围切割（checkpoint-1 显式登记，迁移 0053 头注同源；0062 起 LLM 增强）：
//! * `source_ref.kind == "repository_snapshot"` 的 kb_analyze → 真实分析：
//!   bare clone（缓存目录复用 `repo_facts` 约定）→ 结构化事实采集
//!   （`pm_application::repo_facts`）→ kb.document/revision 草稿
//!   → 回写 kb.analysis.completed + **生产认知信号**（Phase 3 缺口闭合：
//!   `kb.cognition_signal.emitted` 首个真实生产者）。
//! * 其余输入（inline_text 等）与无 KB 文档引用的信号 → 保持 Phase 3 桩语义
//!   （完成回写、占位 ref、NO_CHANGE、不产信号）——真实文档分析本体归 Phase 4。
//! * PC 侧：信号携带 knowledge_base 文档引用 → 结构化派生骨架
//!   （确定性，借鉴 S01-04 pm_cognition 派生骨架）→ pc.\* draft（0053）
//!   + source/evidence → pc.analysis.completed（conclusion=PROPOSED）。
//!
//! LLM 增强（P2-13 裁决 D-1=A，2026-11-08）：两分析步可注入
//! `SnapshotAnalysisModel`（端口在 pm-application，实现在 pm-ai）——LLM 只做
//! **受约束增强**（对象骨架归派生；digest 叙述摘要与对象 summary/content 归
//! LLM，PC content 强制三区小节）。降级语义 D-3=A：无 provider（`Ok(None)`）
//! → derived；解析/调用失败 → derived_fallback——产物永远是真实派生事实，
//! 绝不回退 fixtures 假数据。模式持久化见迁移 0062（analysis_mode 列）。
//!
//! M3 门不破：全部产出 status='draft' / verification_status='unverified'，
//! AI 禁自动确认；confidence 只表达证据强度，不产生确认语义（PC-01B-04 纪律）。
//!
//! 审计：本文件两处生产审计 INSERT（KB/PC 各一），已登记
//! `pm-application/tests/audit_actor_coverage.rs` PRODUCTION_SITES——
//! 系统面 dispatch principal，四身份列显式 NULL（C174 口径，A5/intake 同款）。

use std::collections::HashMap;

use sqlx::PgPool;
use uuid::Uuid;

use kb_pc_contracts::events as ev;
use pm_application::git_access::{self, GitError};
use pm_application::intake::SignalEnvelope;
use pm_application::outbox::{OutboxAppend, OutboxEvent, PgOutboxAppend};
use pm_application::repo_facts::{self, RepoFacts};
use pm_application::repository::open_ssh_private_key;
use pm_application::run_dispatch::DISPATCH_SERVICE_PRINCIPAL;
use pm_application::snapshot_analysis::{
    SnapshotAnalysisModel, SnapshotObjectEnrichment, SnapshotObjectSkeleton,
};

const CLONE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);

/// 分析失败（→ processing.failed → 任务 FAILED + 人工重试入口）。
#[derive(Debug, thiserror::Error)]
pub enum AnalysisError {
    #[error("repository {0} not found")]
    RepositoryNotFound(Uuid),
    #[error("secret key: {0}")]
    Secret(String),
    #[error("git: {0}")]
    Git(String),
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
}

fn ref_json(domain: &str, ty: &str, id: &str, revision: Option<String>) -> serde_json::Value {
    serde_json::json!({
        "target_domain": domain,
        "target_type": ty,
        "target_id": id,
        "target_revision_id": revision,
    })
}

fn sha256_hex(text: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(text.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// 从 intake source_ref 提取 repository_id（非仓库快照输入 → None → 桩语义）。
pub fn source_repository_id(source_ref: &serde_json::Value) -> Option<Uuid> {
    let kind = source_ref.get("kind").and_then(|v| v.as_str())?;
    if kind != "repository_snapshot" {
        return None;
    }
    source_ref
        .get("repository_id")
        .and_then(|v| v.as_str())
        .and_then(|raw| Uuid::parse_str(raw).ok())
}

/// 从信号引用列表定位 KB 文档引用（subject 优先 document（带 revision 指针），
/// 退回 document_revision）。无引用 → None → 桩语义。
pub fn kb_document_ref_of(signal: &serde_json::Value) -> Option<(Uuid, Option<Uuid>)> {
    let refs = signal
        .get("subject_refs")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut evidence = signal
        .get("evidence_refs")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    evidence.extend(refs);
    for r in &evidence {
        if r.get("target_domain").and_then(|v| v.as_str()) != Some("knowledge_base") {
            continue;
        }
        match r.get("target_type").and_then(|v| v.as_str()) {
            Some("document") => {
                let document_id = r
                    .get("target_id")
                    .and_then(|v| v.as_str())
                    .and_then(|raw| Uuid::parse_str(raw).ok());
                let revision_id = r
                    .get("target_revision_id")
                    .and_then(|v| v.as_str())
                    .and_then(|raw| Uuid::parse_str(raw).ok());
                if let Some(document_id) = document_id {
                    return Some((document_id, revision_id));
                }
            }
            Some("document_revision") => {
                let revision_id = r
                    .get("target_id")
                    .and_then(|v| v.as_str())
                    .and_then(|raw| Uuid::parse_str(raw).ok());
                if let Some(revision_id) = revision_id {
                    return Some((Uuid::nil(), Some(revision_id)));
                }
            }
            _ => {}
        }
    }
    None
}

// ── 仓库事实采集（KB/PC 共用）──────────────────────────────────────────────

struct RepoAccess {
    name: String,
    url: String,
    branch: String,
    /// 人类确认的采用基准（0038 G-05；分析锚裁决 D-A=2026-11-08）。
    selected_commit: Option<String>,
}

/// 分析锚解析（裁决 D-A/A-2）：`selected_commit ?? observed_head ?? 分支 tip`。
/// `selected_commit` 是人类确认的采用基准；`observed_head` 来自 verify 的
/// ls-remote（intake source_ref 携带），消除 verify 与分析两时点的 HEAD 漂移；
/// 都没有才落到分支 tip。锚是一个 commit ref——`repo_facts::collect` 的全部
/// git 命令（rev-parse/log/ls-tree/show）对 commit SHA 与分支名同构。
fn analysis_anchor(access: &RepoAccess, observed_head: Option<&str>) -> Option<String> {
    access
        .selected_commit
        .clone()
        .or_else(|| observed_head.map(str::to_string))
}

async fn load_repo_access(pool: &PgPool, repository_id: Uuid) -> Result<RepoAccess, AnalysisError> {
    type Row = Option<(
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    )>;
    let row: Row = sqlx::query_as(
        "SELECT name, url, default_branch, selected_branch, selected_commit \
             FROM integration.project_repository WHERE id = $1",
    )
    .bind(repository_id)
    .fetch_optional(pool)
    .await?;
    let (name, url, default_branch, selected_branch, selected_commit) =
        row.ok_or(AnalysisError::RepositoryNotFound(repository_id))?;
    Ok(RepoAccess {
        name,
        url,
        // 读取分支随锚走：确认基准自带分支；否则 default_branch。
        branch: selected_branch
            .or(default_branch)
            .unwrap_or_else(|| "main".to_string()),
        selected_commit,
    })
}

async fn project_ssh_key_path(
    pool: &PgPool,
    repository_id: Uuid,
) -> Result<Option<git_access::KeyMaterial>, AnalysisError> {
    let project_id: Uuid =
        sqlx::query_scalar("SELECT project_id FROM integration.project_repository WHERE id = $1")
            .bind(repository_id)
            .fetch_one(pool)
            .await?;
    let ciphertext: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT private_key_ciphertext FROM integration.project_ssh_key \
         WHERE project_id = $1 AND status = 'active'",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    let Some(bytes) = ciphertext else {
        return Ok(None);
    };
    let private = open_ssh_private_key(project_id, &bytes)
        .map_err(|e| AnalysisError::Secret(e.to_string()))?;
    git_access::write_temp_key(&private)
        .map(Some)
        .map_err(|e| AnalysisError::Secret(e.to_string()))
}

/// bare clone（每次Fresh——`clone_bare` 失败自清理，与冷启动导入同款）并采集事实。
/// `observed_head` = verify 的 Observed HEAD（intake source_ref），作锚链次选。
async fn collect_fresh(
    pool: &PgPool,
    repository_id: Uuid,
    observed_head: Option<&str>,
) -> Result<(RepoAccess, RepoFacts), AnalysisError> {
    let access = load_repo_access(pool, repository_id).await?;
    let key = project_ssh_key_path(pool, repository_id).await?;
    let key_path = key.as_ref().map(|m| m.key_path());
    let dest = repo_facts::repo_cache_dir(repository_id);
    let _ = std::fs::remove_dir_all(&dest);
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    git_access::clone_bare(&access.url, key_path.as_deref(), &dest, CLONE_TIMEOUT)
        .await
        .map_err(|e| match e {
            GitError::Timeout => AnalysisError::Git("clone timed out".into()),
            other => AnalysisError::Git(other.to_string()),
        })?;
    let anchor = analysis_anchor(&access, observed_head);
    let facts = match &anchor {
        Some(commit) => repo_facts::collect(&dest, commit, repo_facts::Limits::from_env())
            .await
            .map_err(|e| AnalysisError::Git(e.to_string()))?,
        None => repo_facts::collect(&dest, &access.branch, repo_facts::Limits::from_env())
            .await
            .map_err(|e| AnalysisError::Git(e.to_string()))?,
    };
    Ok((access, facts))
}

/// PC 派生入口：优先复用 KB 分析留下的缓存 clone（collect 便宜且确定性）；
/// 缓存缺失/损坏时退回 fresh clone。锚链同 KB 侧（`analysis_anchor`）。
/// 返回 (facts, repo_name)。
pub async fn collect_repository_facts(
    pool: &PgPool,
    repository_id: Uuid,
    observed_head: Option<&str>,
) -> Result<(RepoFacts, String), AnalysisError> {
    let access = load_repo_access(pool, repository_id).await?;
    let anchor = analysis_anchor(&access, observed_head);
    let dest = repo_facts::repo_cache_dir(repository_id);
    if dest.is_dir() {
        let anchor_ref = anchor.as_deref().unwrap_or(access.branch.as_str());
        if let Ok(facts) =
            repo_facts::collect(&dest, anchor_ref, repo_facts::Limits::from_env()).await
        {
            return Ok((facts, access.name));
        }
    }
    let (_, facts) = collect_fresh(pool, repository_id, observed_head).await?;
    Ok((facts, access.name))
}

// ── KB：仓库快照 digest 与落库 ─────────────────────────────────────────────

/// 仓库快照 Markdown digest（纯函数，可单测）——结构化事实摘要，正文有界
/// （commits 最多列 30、hot files 10、docs 只列清单），总量由 commit 截断保底。
pub fn build_repo_digest(name: &str, url: &str, facts: &RepoFacts) -> String {
    compose_repo_digest(name, url, facts, None, None)
}

/// digest 拼装：结构化统计小节（恒有）+ 可选 LLM 叙述摘要（D-1=A 增强，
/// 有界 48K 字符）+ 模式尾注。`llm_markdown=None` 即纯派生（含「无 LLM 参与」
/// 尾注）；派生回退（derived_fallback）产物与纯派生同文——回退产物就是
/// 派生产物，模式差异只落在 0062 列与审计。
pub fn compose_repo_digest(
    name: &str,
    url: &str,
    facts: &RepoFacts,
    llm_markdown: Option<&str>,
    model_name: Option<&str>,
) -> String {
    let mut out = String::with_capacity(16 * 1024);
    out.push_str(&format!("# 仓库快照：{name}\n\n"));
    out.push_str(&format!("- 仓库：`{url}`\n"));
    out.push_str(&format!(
        "- HEAD：`{}`（分支 `{}`）\n",
        facts.head_commit_sha, facts.default_branch
    ));
    out.push_str(&format!(
        "- 文件数：{}｜体积：{}KB｜采集触顶：{}\n\n",
        facts.file_count,
        facts.size_bytes / 1024,
        if facts.truncated { "是" } else { "否" }
    ));

    if let Some(markdown) = llm_markdown {
        let bounded: String = markdown.trim().chars().take(48_000).collect();
        out.push_str("## 摘要（LLM 分析）\n\n");
        out.push_str(&bounded);
        out.push_str("\n\n");
    }

    out.push_str("## 技术栈构成（按扩展名 Top 8）\n\n");
    for lang in &facts.languages {
        out.push_str(&format!("- `.{}`：{} 个文件\n", lang.extension, lang.files));
    }
    out.push('\n');

    let commits = facts.commits.iter().take(30);
    out.push_str(&format!(
        "## 近期交付（共采集 {} 条 commit，展示最近 {} 条）\n\n",
        facts.commits.len(),
        facts.commits.len().min(30)
    ));
    for c in commits {
        let short: String = c.sha.chars().take(8).collect();
        out.push_str(&format!(
            "- `{short}` {} {} {}\n",
            c.author, c.authored_at, c.subject
        ));
    }
    out.push('\n');

    out.push_str("## 热点文件（近期 commit 触达 Top 20）\n\n");
    for h in facts.hot_files.iter().take(10) {
        out.push_str(&format!("- `{}`（触达 {} 次）\n", h.path, h.touches));
    }
    out.push('\n');

    out.push_str(&format!(
        "## 文档清单（{} 篇，正文不入快照）\n\n",
        facts.docs.len()
    ));
    for d in &facts.docs {
        out.push_str(&format!("- `{}`（{}B）\n", d.path, d.bytes));
    }
    out.push('\n');

    if let Some(model) = model_name {
        out.push_str(&format!(
            "> 本快照由仓库 verify 触发，LLM 增强分析（model={model}）；统计清单来自\n\
             > 结构化事实采集，叙述摘要由 LLM 生成，均以 kb.document_revision 草稿落库，\n\
             > 等待人工确认；确认语义不在 AI 侧。\n"
        ));
    } else {
        out.push_str("> 本快照由仓库 verify 触发的结构化事实派生生成（无 LLM 参与），\n");
        out.push_str("> 以 kb.document_revision 草稿落库，等待人工确认；确认语义不在 AI 侧。\n");
    }
    out
}

/// KB 仓库快照分析：采集 → digest（可选 LLM 增强，D-1=A）→ kb.document/
/// revision（0053/0062）落库 → 同事务回写 kb.analysis.completed + 认知信号
/// （真实生产者）。返回 (document_id, revision_id, content_hash)。
#[allow(clippy::too_many_arguments)]
pub async fn analyze_repository_snapshot(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    repository_id: Uuid,
    source_ref: serde_json::Value,
    snapshot_model: Option<&dyn SnapshotAnalysisModel>,
) -> Result<(Uuid, Uuid, String), AnalysisError> {
    let tenant_id: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await?;
    // 锚链次选：verify 的 Observed HEAD（source_ref 携带）——verify 与分析
    // 两时点之间 HEAD 若前进，产物仍锚定受理时的版本（D-A/A-2 裁决）。
    let observed_head = source_ref.get("head_commit_sha").and_then(|v| v.as_str());
    let (access, facts) = collect_fresh(pool, repository_id, observed_head).await?;
    // D-3=A 降级：Ok(None)=无 provider → derived；Err=解析/调用失败 →
    // derived_fallback。产物永远是真实派生事实，绝不回退 fixtures 假数据。
    let (digest, analysis_mode) = match snapshot_model {
        Some(model) => match model
            .enrich_digest(project_id, &access.name, &access.url, &facts)
            .await
        {
            Ok(Some(enriched)) => (
                compose_repo_digest(
                    &access.name,
                    &access.url,
                    &facts,
                    Some(&enriched.markdown),
                    Some(&enriched.model_name),
                ),
                "llm",
            ),
            Ok(None) => (
                build_repo_digest(&access.name, &access.url, &facts),
                "derived",
            ),
            Err(err) => {
                tracing::warn!(
                    %task_id, %repository_id, error = %err,
                    "kb: snapshot digest LLM enrichment failed; falling back to derived facts"
                );
                (
                    build_repo_digest(&access.name, &access.url, &facts),
                    "derived_fallback",
                )
            }
        },
        None => (
            build_repo_digest(&access.name, &access.url, &facts),
            "derived",
        ),
    };
    let content_hash = sha256_hex(&digest);

    let mut tx = pool.begin().await?;
    // kb.document：doc_key = repo-{repository_id}（项目内幂等）。
    let document_id: Uuid = sqlx::query_scalar(
        "INSERT INTO kb.document (tenant_id, project_id, doc_key, doc_type, canonical_path) \
         VALUES ($1, $2, $3, 'knowledge', $4) \
         ON CONFLICT (tenant_id, project_id, doc_key) DO UPDATE SET updated_at = now() \
         RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(format!("repo-{repository_id}"))
    .bind(format!("repos/{repository_id}/repository-snapshot.md"))
    .fetch_one(&mut *tx)
    .await?;

    // 追加修订（append-only；同 hash 不重复建修订）。
    let latest: Option<(i32, String)> = sqlx::query_as(
        "SELECT revision_no, content_hash FROM kb.document_revision \
          WHERE document_id = $1 ORDER BY revision_no DESC LIMIT 1",
    )
    .bind(document_id)
    .fetch_optional(&mut *tx)
    .await?;
    let revision_id = match latest {
        Some((_, hash)) if hash == content_hash => {
            let existing: Uuid = sqlx::query_scalar(
                "SELECT id FROM kb.document_revision \
                  WHERE document_id = $1 ORDER BY revision_no DESC LIMIT 1",
            )
            .bind(document_id)
            .fetch_one(&mut *tx)
            .await?;
            tracing::info!(%task_id, %repository_id, revision = %existing, "kb: digest unchanged, reuse revision");
            existing
        }
        Some((last_no, _)) => {
            insert_kb_revision(
                &mut tx,
                tenant_id,
                document_id,
                last_no + 1,
                &facts,
                &digest,
                &content_hash,
                analysis_mode,
            )
            .await?
        }
        None => {
            insert_kb_revision(
                &mut tx,
                tenant_id,
                document_id,
                1,
                &facts,
                &digest,
                &content_hash,
                analysis_mode,
            )
            .await?
        }
    };
    let revision_no: i32 =
        sqlx::query_scalar("SELECT revision_no FROM kb.document_revision WHERE id = $1")
            .bind(revision_id)
            .fetch_one(&mut *tx)
            .await?;

    // 审计（系统面；PRODUCTION_SITES 登记 #1）。
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (project_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
          action, target_type, target_id, after_data, source_kind, tenant_id) \
         VALUES ($1, $2, NULL, NULL, NULL, NULL, \
                 'kb_analysis_completed', 'kb_document_revision', $3, $4, 'kb_pc_analysis', $5)",
    )
    .bind(project_id)
    .bind(DISPATCH_SERVICE_PRINCIPAL)
    .bind(revision_id)
    .bind(serde_json::json!({
        "document_id": document_id,
        "revision_no": revision_no,
        "head_commit_sha": facts.head_commit_sha,
        "content_hash": content_hash,
        "truncated": facts.truncated,
        "analysis_mode": analysis_mode,
    }))
    .bind(tenant_id)
    .execute(&mut *tx)
    .await?;

    let outbox = PgOutboxAppend;
    let document_ref = ref_json(
        "knowledge_base",
        "document",
        &document_id.to_string(),
        Some(revision_id.to_string()),
    );
    let revision_ref = ref_json(
        "knowledge_base",
        "document_revision",
        &revision_id.to_string(),
        None,
    );
    let short: String = facts.head_commit_sha.chars().take(8).collect();
    let outbox_event = OutboxEvent {
        project_id,
        event_type: ev::KB_ANALYSIS_COMPLETED,
        aggregate_type: "intake_task",
        aggregate_id: &task_id.to_string(),
        payload: serde_json::json!({
            "kb_analysis_ref": revision_ref,
            "source_ref": source_ref,
            "document_ref": document_ref,
            "memory_draft_refs": [],
            "conclusion": "COMPLETED",
            "task_ref": ref_json("requirement", "intake_task", &task_id.to_string(), None),
        }),
        idempotency_key: &format!("kb_analysis:{task_id}"),
    };
    outbox.append(&mut tx, outbox_event).await?;

    // 认知信号（Phase 3 缺口闭合：首个真实生产者）。confidence=high：来源是
    // 权威仓库内容的确定性派生；RT 阈值默认 medium → required pc_analyze。
    let signal = SignalEnvelope {
        relevance: "PRODUCT_CANDIDATE".to_string(),
        confidence: Some("high".to_string()),
        rationale: Some(
            format!(
                "repository snapshot {short}: {} commits / {} files analyzed",
                facts.commits.len(),
                facts.file_count
            )
            .chars()
            .take(256)
            .collect(),
        ),
        subject_refs: vec![document_ref.clone()],
        evidence_refs: vec![ref_json(
            "knowledge_base",
            "document_revision",
            &revision_id.to_string(),
            None,
        )],
    };
    outbox
        .append(
            &mut tx,
            OutboxEvent {
                project_id,
                event_type: ev::KB_COGNITION_SIGNAL_EMITTED,
                aggregate_type: "intake_task",
                aggregate_id: &task_id.to_string(),
                payload: serde_json::json!({
                    "relevance": signal.relevance,
                    "confidence": signal.confidence,
                    "rationale": signal.rationale,
                    "subject_refs": signal.subject_refs,
                    "evidence_refs": signal.evidence_refs,
                }),
                idempotency_key: &format!("kb_signal:{task_id}:{revision_id}"),
            },
        )
        .await?;

    tx.commit().await?;
    tracing::info!(
        %task_id, %repository_id, %document_id, %revision_id,
        head = %facts.head_commit_sha,
        "kb: repository snapshot analyzed (revision {revision_no})"
    );
    Ok((document_id, revision_id, content_hash))
}

#[allow(clippy::too_many_arguments)] // 修订行写入面（0062 加 analysis_mode），列清单与表结构对应
async fn insert_kb_revision(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    document_id: Uuid,
    revision_no: i32,
    facts: &RepoFacts,
    digest: &str,
    content_hash: &str,
    analysis_mode: &str,
) -> Result<Uuid, AnalysisError> {
    let mode_suffix = match analysis_mode {
        "llm" => "，LLM 增强",
        "derived_fallback" => "，LLM 失败回退派生",
        _ => "",
    };
    let revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO kb.document_revision \
         (tenant_id, document_id, revision_no, source_commit_sha, content_hash, \
          status, content, change_summary, analysis_mode, created_by) \
         VALUES ($1, $2, $3, $4, $5, 'draft', $6, $7, $8, $9) RETURNING id",
    )
    .bind(tenant_id)
    .bind(document_id)
    .bind(revision_no)
    .bind(&facts.head_commit_sha)
    .bind(content_hash)
    .bind(digest)
    .bind(format!(
        "仓库快照分析 @ {}（{} commits / {} files{mode_suffix}）",
        facts.head_commit_sha,
        facts.commits.len(),
        facts.file_count
    ))
    .bind(analysis_mode)
    .bind(DISPATCH_SERVICE_PRINCIPAL)
    .fetch_one(&mut **tx)
    .await?;
    Ok(revision_id)
}

// ── PC：结构化派生（确定性，无 LLM）────────────────────────────────────────

/// 单条认知草稿（派生计划产物；纯数据，落库与单测共用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CognitionDraft {
    pub object_key: String,
    pub object_type: &'static str,
    pub name: String,
    pub summary: String,
    pub content: String,
}

/// 结构化派生计划（纯函数，可单测）：仓库快照 → 4 类认知对象草稿。
/// 内容全部来自 RepoFacts 既有事实，不虚构（S01-04 派生骨架同款边界）。
pub fn plan_cognition_objects(
    facts: &RepoFacts,
    repository_id: Uuid,
    max_modules: usize,
) -> Vec<CognitionDraft> {
    let repo = repository_id.to_string();
    let short: String = facts.head_commit_sha.chars().take(8).collect();
    let mut drafts = Vec::new();

    drafts.push(CognitionDraft {
        object_key: format!("repo:{repo}:snapshot"),
        object_type: "repository",
        name: format!("仓库快照 @ {short}"),
        summary: format!(
            "分支 {}，{} 个文件，{}KB",
            facts.default_branch,
            facts.file_count,
            facts.size_bytes / 1024
        ),
        content: format!(
            "- HEAD：`{}`（分支 `{}`）\n- 文件数：{}\n- 体积：{}KB\n- 采集触顶：{}\n",
            facts.head_commit_sha,
            facts.default_branch,
            facts.file_count,
            facts.size_bytes / 1024,
            if facts.truncated { "是" } else { "否" }
        ),
    });

    if !facts.languages.is_empty() {
        let mut content = String::from("按扩展名统计的文件构成：\n");
        for lang in &facts.languages {
            content.push_str(&format!("- `.{}`：{} 个文件\n", lang.extension, lang.files));
        }
        let langs = facts
            .languages
            .iter()
            .map(|l| l.extension.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        drafts.push(CognitionDraft {
            object_key: format!("repo:{repo}:tech_stack"),
            object_type: "tech_stack",
            name: format!("技术栈（{}）", langs),
            summary: langs,
            content,
        });
    }

    if !facts.commits.is_empty() {
        let mut content = format!("共采集 {} 条 commit，最近交付：\n", facts.commits.len());
        for c in facts.commits.iter().take(10) {
            let cshort: String = c.sha.chars().take(8).collect();
            content.push_str(&format!("- `{cshort}` {} {}\n", c.author, c.subject));
        }
        drafts.push(CognitionDraft {
            object_key: format!("repo:{repo}:activity"),
            object_type: "delivery_activity",
            name: format!("交付动态（最近 {} 条）", facts.commits.len().min(10)),
            summary: facts
                .commits
                .first()
                .map(|c| c.subject.clone())
                .unwrap_or_default(),
            content,
        });
    }

    for h in facts.hot_files.iter().take(max_modules) {
        drafts.push(CognitionDraft {
            object_key: format!("repo:{repo}:module:{}", h.path),
            object_type: "module",
            name: h.path.clone(),
            summary: format!("近期 commit 触达 {} 次", h.touches),
            content: format!(
                "`{}` 在近 {} 条 commit 窗口内被触达 {} 次。\n",
                h.path,
                facts.commits.len(),
                h.touches
            ),
        });
    }

    drafts
}

/// PC 认知派生落库：对象 upsert（object_key 幂等锚）与修订（内容同则跳过），
/// 加 source 四元组与 evidence（unverified/medium），同事务审计。
/// 可选注入 `SnapshotAnalysisModel`（D-1=A）：骨架形态归结构化派生，LLM 只
/// 回填 summary/content（三区小节）与白名单证据（D-2=A 受约束增强）；
/// 失败/无 provider 回退派生并标 derived_fallback/derived（D-3=A）。
/// 返回主对象（snapshot slot）的 (object_id, revision_id)。
#[allow(clippy::too_many_arguments)]
pub async fn derive_pc_cognition(
    pool: &PgPool,
    project_id: Uuid,
    intake_id: Uuid,
    task_id: Uuid,
    repository_id: Uuid,
    kb_document_id: Uuid,
    kb_revision_id: Uuid,
    kb_content_hash: &str,
    facts: &RepoFacts,
    snapshot_model: Option<&dyn SnapshotAnalysisModel>,
) -> Result<(Uuid, Uuid), AnalysisError> {
    let tenant_id: Uuid = sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await?;
    let drafts = plan_cognition_objects(facts, repository_id, 5);
    let mut enrichments: HashMap<String, SnapshotObjectEnrichment> = HashMap::new();
    let analysis_mode = match snapshot_model {
        Some(model) => {
            let skeletons: Vec<SnapshotObjectSkeleton> = drafts
                .iter()
                .map(|d| SnapshotObjectSkeleton {
                    object_key: d.object_key.clone(),
                    object_type: d.object_type.to_string(),
                    name: d.name.clone(),
                    summary: d.summary.clone(),
                    content: d.content.clone(),
                })
                .collect();
            match model.enrich_objects(project_id, facts, &skeletons).await {
                Ok(Some(enriched)) => {
                    for object in enriched.objects {
                        enrichments.insert(object.object_key.clone(), object);
                    }
                    "llm"
                }
                Ok(None) => "derived",
                Err(err) => {
                    tracing::warn!(
                        %task_id, %intake_id, error = %err,
                        "pc: LLM enrichment failed; falling back to derived facts"
                    );
                    "derived_fallback"
                }
            }
        }
        None => "derived",
    };
    let mut tx = pool.begin().await?;

    // 来源：本次分析的 KB 文档修订（四元组结构化列）。
    let source_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.source \
         (tenant_id, project_id, target_domain, target_type, target_id, target_revision_id, locator) \
         VALUES ($1, $2, 'knowledge_base', 'document', $3, $4, $5) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(kb_document_id.to_string())
    .bind(kb_revision_id.to_string())
    .bind(serde_json::json!({
        "kind": "kb_document_revision",
        "revision_id": kb_revision_id,
        "content_hash": kb_content_hash,
    }))
    .fetch_one(&mut *tx)
    .await?;

    let mut primary: Option<(Uuid, Uuid)> = None;
    for draft in &drafts {
        let (summary, content, llm_evidence_refs) = match enrichments.get(&draft.object_key) {
            Some(enriched) => (
                enriched.summary.as_str(),
                enriched.content.as_str(),
                Some(enriched.evidence_refs.as_slice()),
            ),
            None => (draft.summary.as_str(), draft.content.as_str(), None),
        };
        let object_id: Uuid = sqlx::query_scalar(
            "INSERT INTO pc.cognition_object \
             (tenant_id, project_id, object_type, object_key, name, lifecycle_status) \
             VALUES ($1, $2, $3, $4, $5, 'draft') \
             ON CONFLICT (tenant_id, project_id, object_key) \
             DO UPDATE SET name = EXCLUDED.name, updated_at = now() \
             RETURNING id",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(draft.object_type)
        .bind(&draft.object_key)
        .bind(&draft.name)
        .fetch_one(&mut *tx)
        .await?;

        // 内容寻址去重：最新修订内容相同则不追加修订行。
        let latest: Option<(i32, String)> = sqlx::query_as(
            "SELECT revision_no, content FROM pc.cognition_revision \
              WHERE object_id = $1 ORDER BY revision_no DESC LIMIT 1",
        )
        .bind(object_id)
        .fetch_optional(&mut *tx)
        .await?;
        let revision_id = match latest {
            Some((_, existing)) if existing == content => {
                sqlx::query_scalar(
                    "SELECT id FROM pc.cognition_revision \
                      WHERE object_id = $1 ORDER BY revision_no DESC LIMIT 1",
                )
                .bind(object_id)
                .fetch_one(&mut *tx)
                .await?
            }
            Some((last_no, _)) => {
                insert_pc_revision(
                    &mut tx,
                    tenant_id,
                    object_id,
                    last_no + 1,
                    summary,
                    content,
                    analysis_mode,
                )
                .await?
            }
            None => {
                insert_pc_revision(
                    &mut tx,
                    tenant_id,
                    object_id,
                    1,
                    summary,
                    content,
                    analysis_mode,
                )
                .await?
            }
        };

        // 证据：一次分析一条（evidence_hash = KB 修订内容指纹，可对账）；
        // LLM 增强时附加白名单过滤后的 llm_evidence_refs（实现侧已对照
        // RepoFacts 过滤，此处原样登记）。
        sqlx::query(
            "INSERT INTO pc.evidence \
             (tenant_id, project_id, source_id, cognition_object_id, evidence_type, \
              locator, evidence_hash, verification_status, confidence) \
             VALUES ($1, $2, $3, $4, 'repo_facts', $5, $6, 'unverified', 'medium')",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(source_id)
        .bind(object_id)
        .bind(serde_json::json!({
            "intake_id": intake_id,
            "task_id": task_id,
            "repository_id": repository_id,
            "head_commit_sha": facts.head_commit_sha,
            "slot": draft.object_key.rsplit(':').next().unwrap_or("snapshot"),
            "commit_count": facts.commits.len(),
            "file_count": facts.file_count,
            "truncated": facts.truncated,
            "analysis_mode": analysis_mode,
            "llm_evidence_refs": llm_evidence_refs.unwrap_or_default(),
        }))
        .bind(kb_content_hash)
        .execute(&mut *tx)
        .await?;

        if primary.is_none() {
            primary = Some((object_id, revision_id));
        }
    }
    let (primary_object, primary_revision) =
        primary.ok_or_else(|| AnalysisError::Git("no cognition drafts derived".into()))?;

    // 审计（系统面；PRODUCTION_SITES 登记 #2）。
    sqlx::query(
        "INSERT INTO governance.audit_log \
         (project_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
          action, target_type, target_id, after_data, source_kind, tenant_id) \
         VALUES ($1, $2, NULL, NULL, NULL, NULL, \
                 'pc_cognition_drafted', 'pc_cognition_object', $3, $4, 'kb_pc_analysis', $5)",
    )
    .bind(project_id)
    .bind(DISPATCH_SERVICE_PRINCIPAL)
    .bind(primary_object)
    .bind(serde_json::json!({
        "intake_id": intake_id,
        "kb_document_id": kb_document_id,
        "kb_revision_id": kb_revision_id,
        "objects": drafts.len(),
        "head_commit_sha": facts.head_commit_sha,
        "analysis_mode": analysis_mode,
        "llm_enriched_objects": enrichments.len(),
    }))
    .bind(tenant_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    tracing::info!(
        %task_id, %intake_id, %repository_id,
        objects = drafts.len(),
        "pc: cognition drafted from repository snapshot"
    );
    Ok((primary_object, primary_revision))
}

async fn insert_pc_revision(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    object_id: Uuid,
    revision_no: i32,
    summary: &str,
    content: &str,
    analysis_mode: &str,
) -> Result<Uuid, AnalysisError> {
    let revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pc.cognition_revision \
         (tenant_id, object_id, revision_no, summary, content, status, analysis_mode, created_by) \
         VALUES ($1, $2, $3, $4, $5, 'draft', $6, $7) RETURNING id",
    )
    .bind(tenant_id)
    .bind(object_id)
    .bind(revision_no)
    .bind(summary)
    .bind(content)
    .bind(analysis_mode)
    .bind(DISPATCH_SERVICE_PRINCIPAL)
    .fetch_one(&mut **tx)
    .await?;
    Ok(revision_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pm_application::repo_facts::{CommitFact, HotFile, LanguageStat};

    fn facts() -> RepoFacts {
        RepoFacts {
            head_commit_sha: "abcdef1234567890abcdef1234567890abcdef12".to_string(),
            default_branch: "main".to_string(),
            commits: vec![
                CommitFact {
                    sha: "abcdef1234567890abcdef1234567890abcdef12".to_string(),
                    author: "alice".to_string(),
                    authored_at: "2026-11-08T10:00:00Z".to_string(),
                    subject: "feat: checkpoint".to_string(),
                },
                CommitFact {
                    sha: "1111111111111111111111111111111111111111".to_string(),
                    author: "bob".to_string(),
                    authored_at: "2026-11-08T09:00:00Z".to_string(),
                    subject: "fix: bug".to_string(),
                },
            ],
            hot_files: vec![
                HotFile {
                    path: "crates/pm-api/src/routes.rs".to_string(),
                    touches: 12,
                },
                HotFile {
                    path: "docs/architecture/x.md".to_string(),
                    touches: 5,
                },
            ],
            languages: vec![
                LanguageStat {
                    extension: "rs".to_string(),
                    files: 120,
                },
                LanguageStat {
                    extension: "md".to_string(),
                    files: 40,
                },
            ],
            file_count: 160,
            docs: vec![],
            truncated: false,
            size_bytes: 2 * 1024 * 1024,
        }
    }

    #[test]
    fn digest_contains_structured_sections_and_is_stable() {
        let f = facts();
        let d1 = build_repo_digest("ai-pm", "git@example.com:tooling/ai-pm.git", &f);
        let d2 = build_repo_digest("ai-pm", "git@example.com:tooling/ai-pm.git", &f);
        assert_eq!(d1, d2, "digest must be deterministic");
        assert!(d1.contains("# 仓库快照：ai-pm"));
        assert!(d1.contains("abcdef12"));
        assert!(d1.contains("技术栈构成"));
        assert!(d1.contains("近期交付"));
        assert!(d1.contains("热点文件"));
        assert!(d1.contains("无 LLM 参与"));
        assert!(d1.len() < 64 * 1024, "digest must stay bounded");
    }

    #[test]
    fn composed_digest_marks_llm_mode_and_keeps_structured_sections() {
        let f = facts();
        let composed = compose_repo_digest(
            "ai-pm",
            "git@example.com:tooling/ai-pm.git",
            &f,
            Some("这是一个 Rust 仓库的叙述摘要。"),
            Some("test-model"),
        );
        assert!(composed.contains("# 仓库快照：ai-pm"));
        assert!(composed.contains("## 摘要（LLM 分析）"));
        assert!(composed.contains("这是一个 Rust 仓库的叙述摘要。"));
        assert!(composed.contains("model=test-model"));
        assert!(composed.contains("技术栈构成"), "structured sections kept");
        assert!(composed.contains("近期交付"));
        assert!(!composed.contains("无 LLM 参与"), "mode footer must flip");
        assert!(composed.len() < 64 * 1024);
    }

    #[test]
    fn derivation_is_deterministic_and_honest() {
        let f = facts();
        let rid = Uuid::nil();
        let a = plan_cognition_objects(&f, rid, 5);
        let b = plan_cognition_objects(&f, rid, 5);
        assert_eq!(a, b, "derivation must be deterministic");
        // snapshot + tech_stack + activity + 2 modules（hot_files 全量 ≤ max）。
        assert_eq!(a.len(), 5);
        assert_eq!(a[0].object_type, "repository");
        assert_eq!(a[1].object_type, "tech_stack");
        assert_eq!(a[2].object_type, "delivery_activity");
        assert_eq!(a[3].object_type, "module");
        assert!(a
            .iter()
            .all(|d| d.object_key.starts_with(&format!("repo:{rid}:"))));
        // 无语言/无 commit 时对应槽位不虚构。
        let mut empty = facts();
        empty.languages.clear();
        empty.commits.clear();
        empty.hot_files.clear();
        let minimal = plan_cognition_objects(&empty, rid, 5);
        assert_eq!(
            minimal.len(),
            1,
            "empty facts derive only the snapshot slot"
        );
    }

    #[test]
    fn source_repository_id_parses_snapshot_refs_only() {
        let ok = serde_json::json!({
            "kind": "repository_snapshot",
            "repository_id": "0193f4a0-0000-0000-0000-000000000000",
        });
        assert!(source_repository_id(&ok).is_some());
        let inline = serde_json::json!({ "kind": "inline_text" });
        assert!(source_repository_id(&inline).is_none());
        let bad_uuid = serde_json::json!({
            "kind": "repository_snapshot",
            "repository_id": "not-a-uuid",
        });
        assert!(source_repository_id(&bad_uuid).is_none());
    }

    #[test]
    fn kb_document_ref_resolves_subject_and_evidence() {
        let document = Uuid::nil();
        let revision = Uuid::from_u128(42);
        let signal = serde_json::json!({
            "subject_refs": [ref_json("knowledge_base", "document", &document.to_string(), Some(revision.to_string()))],
            "evidence_refs": [],
        });
        assert_eq!(
            kb_document_ref_of(&signal),
            Some((document, Some(revision)))
        );
        let by_revision = serde_json::json!({
            "subject_refs": [],
            "evidence_refs": [ref_json("knowledge_base", "document_revision", &revision.to_string(), None)],
        });
        assert_eq!(
            kb_document_ref_of(&by_revision),
            Some((Uuid::nil(), Some(revision)))
        );
        let none = serde_json::json!({ "subject_refs": [], "evidence_refs": [] });
        assert_eq!(kb_document_ref_of(&none), None);
    }
}
