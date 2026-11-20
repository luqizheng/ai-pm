//! D-5 · 仓库分析报告（一次分析运行 = 一份报告，聚合本次全部产物）。
//!
//! 生命周期（docs/scenarios/s01/docs/s01-reanalysis-and-product-cognition-closure-v1.md，
//! 2026-11-15 用户裁决）：`analyzing`（受理即建行）→ `analyzed`（PC 任务完成，
//! 报告 open 待审）→ `closed`（人工显式关闭：自动处置未决项 + 固化 D-1 锚点）；
//! `failed` / `cancelled` 为终态，不占门。
//!
//! 门：analyzing/analyzed 期间该仓库禁止再分析（`repository::analyze_repository`
//! 查 open 报告 → 409 `ReportOpen`）。产物关联链（P3-11 起双源，G-1）：
//! - 报告.intake_id → integration.intake_task（kb_analyze / pc_analyze 的 result_ref）；
//! - 报告.kb_revision_id → kb.document_revision（本次报告正文修订）；
//! - 报告.pc_source_id → pc.source（identity 综合通道）→ pc.proposal（产品身份提案）；
//! - 报告.projc_source_id → projc.source（ProjC 快照通道）→ projc.proposal（技术认知
//!   提案）；**双源登记写入点唯一 = mark_analyzed**（同一事务，projc 源缺失 = 硬失败，
//!   报告不得进入 analyzed）；NULL 仅出现在 0026 之前的历史行 = 显式 Legacy；
//! - PC 本轮对象修订 = pc.cognition_revision WHERE document_revision_id = kb_revision_id。
//!
//! 全部状态推进对「报告行不存在」（存量运行/桩语义）静默跳过——钩子永不反向
//! 阻塞分析链。

use sqlx::PgPool;
use uuid::Uuid;

use crate::actor::AuditActor;

/// 报告行（列表投影；pending 计数由查询列给出）。
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct ReportListRow {
    pub id: Uuid,
    pub repository_id: Uuid,
    pub repository_name: String,
    pub commit_sha: String,
    pub status: String,
    pub stage_detail: Option<String>,
    pub error_code: Option<String>,
    pub error_detail: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub closed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 本次报告的 KB 修订与双域来源（关联键；analyzed 后写入）。
    pub kb_revision_id: Option<Uuid>,
    pub pc_source_id: Option<Uuid>,
    /// ProjC 快照通道来源（G-1；NULL = 0026 前历史行 Legacy）。
    pub projc_source_id: Option<Uuid>,
    /// D-2 运行口径：incremental（默认）/ full。
    pub run_mode: String,
    /// 增量起点（= 上一份关闭报告固化的锚点；全量/首次为 NULL）。
    pub since_commit: Option<String>,
    pub pending_proposals: i64,
    /// ProjC 未决提案数（draft/submitted；§8.2 加法计数）。
    pub projc_pending_proposals: i64,
    /// 双域未决合计（展示用；既有 pending_proposals 口径零改动）。
    pub total_pending_proposals: i64,
    pub has_pending_kb: bool,
}

/// 详情：KB 修订（全文）。
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct ReportKbRevision {
    pub revision_id: Uuid,
    pub document_id: Uuid,
    pub doc_key: String,
    pub revision_no: i32,
    pub status: String,
    pub analysis_mode: Option<String>,
    pub content: Option<String>,
    /// 报告正文的 commit 锚（分析时点 HEAD；追溯用，加法字段）。
    pub source_commit_sha: Option<String>,
}

/// 详情：本报告起草的提案（PC 与 ProjC 共用形状；审阅所需头字段为加法投影）。
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct ReportProposalRow {
    pub proposal_id: Uuid,
    pub title: String,
    pub status: String,
    pub expected_version_no: i64,
    pub member_count: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// AI 分析理由（提案起草时给出；审阅判断依据）。
    pub rationale: Option<String>,
    pub risk_level: Option<String>,
    pub confidence: Option<String>,
    /// 提案来源（pc: ai_agent/kb_memory/manual_input/…；projc 另有 repo_analysis）。
    pub origin_kind: String,
}

/// 详情：本报告的证据行（evidence 钉扎投影；支撑对象名经 revision→object 反查）。
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct ReportEvidenceRow {
    pub evidence_type: String,
    pub object_name: String,
    pub object_type: String,
    pub section_anchor: Option<String>,
    /// code 类证据定位（document_section 证据按 schema 约束恒 NULL）。
    pub file_path: Option<String>,
    pub commit_sha: Option<String>,
    pub start_line: Option<i32>,
    pub end_line: Option<i32>,
    pub excerpt: Option<String>,
    /// TEXT 自由描述（快照通道存 JSON：head_commit_sha/llm_evidence_refs/统计）。
    pub description: Option<String>,
    pub confidence: Option<String>,
    pub verification_status: Option<String>,
}

/// 详情：本次分析产出的 PC 对象修订（全文）。
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct ReportPcRevisionRow {
    pub object_id: Uuid,
    pub object_type: String,
    pub display_name: String,
    pub revision_no: i32,
    pub change_summary: Option<String>,
    pub content: Option<String>,
}

/// 详情聚合（报告头 + 产物集；projc_* 为 P3-11 加法数组，恒返回数组）。
#[derive(Debug, serde::Serialize)]
pub struct ReportDetail {
    #[serde(flatten)]
    pub header: ReportListRow,
    pub kb_revision: Option<ReportKbRevision>,
    pub proposals: Vec<ReportProposalRow>,
    pub pc_revisions: Vec<ReportPcRevisionRow>,
    /// ProjC 提案集（by projc_source_id；§8.3 加法，空集返回 []）。
    pub projc_proposals: Vec<ReportProposalRow>,
    /// ProjC 对象修订集（by projc_source_id；空集返回 []）。
    pub projc_revisions: Vec<ReportPcRevisionRow>,
    /// 双域证据投影（加法，空集 []；审阅判断依据展示）。
    pub pc_evidence: Vec<ReportEvidenceRow>,
    pub projc_evidence: Vec<ReportEvidenceRow>,
}

/// 关闭回执：自动处置计数（projc_* 为 P3-11 加法字段，G-2 独立计数）。
///
/// II-A 分类时点一致性契约（Q10-R1-FIX V1.2；负责任确认 2026-11-18；V1.2.1 R4.3
/// 归类规则 + V1.2.2 §四 failed 口径定稿；契约源=该三文档）：
/// - applied：本次 close 自身完成首发；
/// - replayed：已确认先行发布完成——有且仅有两个来源：
///   ①`Ok(replayed=true)`（publish 短路确认）②Err 后锁内分类 JOIN 确认
///   `applied ∧ completed`；
/// - failed：本次未计为成功发布——包括①业务失败（分类时点确认无先行完成且
///   本次尝试失败）与②分类事务技术故障（无法确认是否存在先行完成；≠已观察
///   到未发布）。技术故障经结构化 warn 日志（kind/error/cleanup_error）区分，
///   不改变计数口径。
///
/// 显式声明：不保证报告最终 COMMIT 时数据库终态与计数永久一致
/// （II-B 不可实现性证明=FIX-02 Q2；授权外变体不采用）。
#[derive(Debug, serde::Serialize)]
pub struct CloseOutcome {
    pub report_id: Uuid,
    pub status: &'static str,
    /// 本次 close 自身完成首发发布的数量（II-A；不含先行 replay）。
    pub applied_proposals: u64,
    /// 分类时点已确认先行发布完成的数量（II-A 双来源：短路确认 / Err 后锁内 JOIN）。
    pub replayed_proposals: u64,
    /// 批量发布失败（CAS 失配已内置 failed 等；II-A failed①）与分类事务技术故障
    /// （II-A failed②，warn 日志可辨）的数量。
    pub publish_failed_proposals: u64,
    pub withdrawn_proposals: u64,
    pub rejected_kb: bool,
    /// 固化的分析锚点（D-1）。
    pub last_analyzed_commit: String,
    /// ProjC 批准提案在关闭时批量发布的数量（projc-decision:{id}）。
    pub projc_applied: u64,
    /// ProjC 分类时点已确认先行发布完成的数量（同 replayed 双来源口径）。
    pub projc_replayed: u64,
    /// ProjC 批量发布失败数量（口径同 publish_failed_proposals；独立计数）。
    pub projc_publish_failed_proposals: u64,
    /// ProjC 未决提案在关闭时撤回的数量。
    pub projc_withdrawn: u64,
    /// 本次关闭实际发布成功的版本清单（applied 与 replayed 均计入——两者都产生
    /// 了正式版本；前端用于「确认后看到正式对象 ID/版本」反馈）。发布失败不记入。
    pub published_versions: Vec<PublishedVersionEntry>,
}

/// 关闭回执 · 单条发布记录（前端映射提案标题展示）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct PublishedVersionEntry {
    /// `"pc" | "projc"`。
    pub domain: &'static str,
    pub proposal_id: Uuid,
    pub version_no: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum AnalysisReportError {
    #[error("analysis report not found")]
    NotFound,
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Forbidden(String),
    /// G-1 硬失败：projc 源未登记（derive 链未注册 projc.source）→ 报告不得进入
    /// analyzed；审计事件 `analysis_report.analyze_gate_failed` 同时落库留痕。
    #[error("projc source not registered for report {0}; analyze gate refuses analyzed (G-1)")]
    ProjcSourceMissing(Uuid),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// 受理即建报告（`analyze_repository` 受理成功后调用；幂等重放经
/// `ON CONFLICT (intake_id) DO NOTHING` 落到既有行——返回值恒为报告 id）。
pub async fn ensure_for_intake(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    repository_id: Uuid,
    commit_sha: &str,
    intake_id: Uuid,
    run_mode: &str,
    since_commit: Option<&str>,
) -> Result<Uuid, sqlx::Error> {
    let created: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO integration.repo_analysis_report \
         (tenant_id, project_id, repository_id, commit_sha, intake_id, status, \
          run_mode, since_commit) \
         VALUES ($1, $2, $3, $4, $5, 'analyzing', $6, $7) \
         ON CONFLICT (intake_id) DO NOTHING RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(repository_id)
    .bind(commit_sha)
    .bind(intake_id)
    .bind(run_mode)
    .bind(since_commit)
    .fetch_optional(pool)
    .await?;
    if let Some(id) = created {
        return Ok(id);
    }
    sqlx::query_scalar("SELECT id FROM integration.repo_analysis_report WHERE intake_id = $1")
        .bind(intake_id)
        .fetch_one(pool)
        .await
}

/// D-2 · 既有认知摘要（merge 重析输入；P3-11 起读 ProjC 快照通道）：取该仓库
/// 最近一份有产物的报告（analyzed/closed），经 projc 证据钉扎反查对象修订，
/// 拼装为有界 Markdown 文本（最多 12 个对象、单对象正文截 4000 字）。
/// 无报告/无产物 → None。
pub async fn prior_cognition_digest(
    pool: &PgPool,
    project_id: Uuid,
    repository_id: Uuid,
) -> Result<Option<String>, sqlx::Error> {
    let source_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT projc_source_id FROM integration.repo_analysis_report \
          WHERE project_id = $1 AND repository_id = $2 \
            AND status IN ('analyzed', 'closed') AND projc_source_id IS NOT NULL \
          ORDER BY created_at DESC LIMIT 1",
    )
    .bind(project_id)
    .bind(repository_id)
    .fetch_optional(pool)
    .await?;
    let Some(source_id) = source_id else {
        return Ok(None);
    };
    let rows: Vec<(String, String, i32, Option<String>)> = sqlx::query_as(
        "SELECT o.display_name, o.object_type, r.revision_no, \
                left(dr.content, 4000) \
           FROM projc.evidence e \
           JOIN projc.cognition_revision r ON r.id = e.object_revision_id \
           JOIN projc.cognition_object o ON o.id = r.object_id \
           JOIN projc.document_revision dr ON dr.id = r.document_revision_id \
          WHERE e.source_id = $1 \
          ORDER BY o.display_name ASC LIMIT 12",
    )
    .bind(source_id)
    .fetch_all(pool)
    .await?;
    if rows.is_empty() {
        return Ok(None);
    }
    let mut out = String::from("# 既有项目认知（上一份报告的对象修订）\n");
    for (name, kind, no, content) in rows {
        out.push_str(&format!("\n## {name}（{kind} · v{no}）\n"));
        out.push_str(content.as_deref().unwrap_or(""));
        out.push('\n');
    }
    Ok(Some(out))
}

/// KB 分析完成（PC 派生进行中）——仅刷新阶段描述，不动状态。
pub async fn mark_kb_stage(pool: &PgPool, task_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE integration.repo_analysis_report \
          SET stage_detail = 'KB 分析完成，认知派生中', updated_at = now() \
         WHERE intake_id = (SELECT intake_id FROM integration.intake_task WHERE id = $1) \
           AND status = 'analyzing'",
    )
    .bind(task_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// PC 任务完成（提案起草 / NO_CHANGE / 冲突登记）→ 报告 analyzed（open 待审）。
///
/// G-1 双源登记（写入点唯一，同一事务）：kb_revision_id 取同 intake 的
/// kb_analyze result_ref（**扁平四元组**：`target_revision_id` 或 `target_id`）；
/// `pc_source_id`（identity 综合通道，口径不变）与 `projc_source_id`（ProjC 快照
/// 通道）均经 `*.source.target_revision_id = kb_revision` 反查（derive 链每次
/// 分析各登记一条）。**projc 源缺失 = 硬失败**：报告不得进入 analyzed，审计事件
/// `analysis_report.analyze_gate_failed` 落库留痕（不静默）。报告行不存在
/// （存量运行/桩语义）仍静默跳过——钩子永不反向阻塞分析链。
pub async fn mark_analyzed(pool: &PgPool, task_id: Uuid) -> Result<(), AnalysisReportError> {
    let mut tx = pool.begin().await?;
    // 报告行（行锁；不存在 = 存量/桩语义，静默跳过）
    let report: Option<(Uuid, Uuid, Uuid)> = sqlx::query_as(
        "SELECT id, tenant_id, project_id FROM integration.repo_analysis_report \
          WHERE intake_id = (SELECT intake_id FROM integration.intake_task WHERE id = $1) \
            AND status = 'analyzing' \
          FOR UPDATE",
    )
    .bind(task_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((report_id, tenant_id, project_id)) = report else {
        tx.commit().await?;
        return Ok(());
    };

    // kb_analyze 结果修订（原 CTE 语义平移）
    let kb_revision: Option<Uuid> = sqlx::query_scalar(
        "SELECT COALESCE( \
              result_ref ->> 'target_revision_id', \
              result_ref ->> 'target_id')::uuid \
           FROM integration.intake_task \
          WHERE intake_id = (SELECT intake_id FROM integration.intake_task WHERE id = $1) \
            AND task_type = 'kb_analyze' AND result_ref IS NOT NULL \
          LIMIT 1",
    )
    .bind(task_id)
    .fetch_optional(&mut *tx)
    .await?
    .flatten();

    // G-1：projc 源（快照通道）必达，缺失即硬失败
    let projc_source: Option<Uuid> = match kb_revision {
        Some(rev) => sqlx::query_scalar(
            "SELECT s.id FROM projc.source s \
              WHERE s.tenant_id = $1 AND s.project_id = $2 \
                AND s.target_domain = 'knowledge_base' \
                AND s.target_revision_id = $3 \
              ORDER BY s.created_at DESC LIMIT 1",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(rev.to_string())
        .fetch_optional(&mut *tx)
        .await?,
        None => None,
    };
    let Some(projc_source_id) = projc_source else {
        crate::pm_write::append_audit_with_after(
            &mut tx,
            project_id,
            &AuditActor::dispatch(),
            "analysis_report.analyze_gate_failed",
            "repo_analysis_report",
            Some(report_id),
            serde_json::json!({
                "reason": "projc source not registered (G-1 hard gate)",
                "task_id": task_id,
            }),
        )
        .await?;
        tx.commit().await?;
        return Err(AnalysisReportError::ProjcSourceMissing(report_id));
    };

    // pc 源（identity 综合通道；口径不变，反查可得）
    let pc_source: Option<Uuid> = match kb_revision {
        Some(rev) => sqlx::query_scalar(
            "SELECT s.id FROM pc.source s \
              WHERE s.tenant_id = $1 AND s.project_id = $2 \
                AND s.target_domain = 'knowledge_base' \
                AND s.target_revision_id = $3 \
              ORDER BY s.created_at DESC LIMIT 1",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(rev.to_string())
        .fetch_optional(&mut *tx)
        .await?,
        None => None,
    };

    sqlx::query(
        "UPDATE integration.repo_analysis_report \
            SET status = 'analyzed', \
                kb_revision_id = $2, \
                pc_source_id = $3, \
                projc_source_id = $4, \
                stage_detail = NULL, updated_at = now() \
          WHERE id = $1 AND status = 'analyzing'",
    )
    .bind(report_id)
    .bind(kb_revision)
    .bind(pc_source)
    .bind(projc_source_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// 任务失败 → 报告 failed（终态，不占门）。
///
/// DEF-CHAIN-01：失败时同时写 `stage_detail`（branch-b4 证据里该列为空，
/// 前端需兼容空值）；阶段描述按失败来源区分 KB 分析 / PC 派生 / 其他。
pub async fn mark_failed(
    pool: &PgPool,
    task_id: Uuid,
    error_code: &str,
    error_detail: &str,
) -> Result<(), sqlx::Error> {
    let stage_detail = if error_code.starts_with("kb_") {
        "KB 分析失败"
    } else if error_code.starts_with("pc_") {
        "PC 认知派生失败"
    } else {
        "分析失败"
    };
    sqlx::query(
        "UPDATE integration.repo_analysis_report \
          SET status = 'failed', error_code = $2, error_detail = $3, \
              stage_detail = $4, updated_at = now() \
         WHERE intake_id = (SELECT intake_id FROM integration.intake_task WHERE id = $1) \
           AND status IN ('analyzing', 'analyzed')",
    )
    .bind(task_id)
    .bind(error_code)
    .bind(error_detail)
    .bind(stage_detail)
    .execute(pool)
    .await?;
    Ok(())
}

/// 项目分析报告列表（最新在前；repository_id / status 可选过滤）。
pub async fn list_reports(
    pool: &PgPool,
    project_id: Uuid,
    repository_id: Option<Uuid>,
    status: Option<&str>,
) -> Result<Vec<ReportListRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT r.id, r.repository_id, p.name AS repository_name, r.commit_sha, r.status, \
                r.stage_detail, r.error_code, r.error_detail, r.created_at, r.updated_at, \
                r.closed_at, r.kb_revision_id, r.pc_source_id, r.projc_source_id, \
                r.run_mode, r.since_commit, \
                (SELECT count(*) FROM pc.proposal pp \
                  WHERE pp.source_id = r.pc_source_id \
                    AND pp.status IN ('draft','submitted')) AS pending_proposals, \
                (SELECT count(*) FROM projc.proposal jp \
                  WHERE jp.source_id = r.projc_source_id \
                    AND jp.status IN ('draft','submitted')) AS projc_pending_proposals, \
                ((SELECT count(*) FROM pc.proposal pp \
                   WHERE pp.source_id = r.pc_source_id \
                     AND pp.status IN ('draft','submitted')) \
                 + (SELECT count(*) FROM projc.proposal jp \
                     WHERE jp.source_id = r.projc_source_id \
                       AND jp.status IN ('draft','submitted'))) AS total_pending_proposals, \
                (r.kb_revision_id IS NOT NULL AND EXISTS ( \
                     SELECT 1 FROM kb.document_revision kv \
                      WHERE kv.id = r.kb_revision_id AND kv.status = 'draft')) AS has_pending_kb \
           FROM integration.repo_analysis_report r \
           JOIN integration.project_repository p ON p.id = r.repository_id \
          WHERE r.tenant_id = (SELECT tenant_id FROM core.project WHERE id = $1) \
            AND r.project_id = $1 \
            AND ($2::uuid IS NULL OR r.repository_id = $2) \
            AND ($3::text IS NULL OR r.status = $3) \
          ORDER BY r.created_at DESC",
    )
    .bind(project_id)
    .bind(repository_id)
    .bind(status)
    .fetch_all(pool)
    .await
}

/// 详情聚合（报告头 + KB 修订全文 + 提案集 + PC 修订集）。
pub async fn report_detail(
    pool: &PgPool,
    project_id: Uuid,
    report_id: Uuid,
) -> Result<ReportDetail, AnalysisReportError> {
    let headers = list_reports(pool, project_id, None, None).await?;
    let header = headers
        .into_iter()
        .find(|row| row.id == report_id)
        .ok_or(AnalysisReportError::NotFound)?;

    let kb_revision: Option<ReportKbRevision> = match header.kb_revision_id {
        Some(rev) => {
            sqlx::query_as(
                "SELECT kv.id AS revision_id, kv.document_id, d.doc_key, kv.revision_no, \
                    kv.status, kv.analysis_mode, kv.content, kv.source_commit_sha \
               FROM kb.document_revision kv \
               JOIN kb.document d ON d.id = kv.document_id \
              WHERE kv.id = $1",
            )
            .bind(rev)
            .fetch_optional(pool)
            .await?
        }
        None => None,
    };

    let proposals: Vec<ReportProposalRow> = match header.pc_source_id {
        Some(source_id) => {
            sqlx::query_as(
                "SELECT pp.id AS proposal_id, pp.title, pp.status, pp.expected_version_no, \
                    (SELECT count(*) FROM pc.proposal_member m \
                      WHERE m.tenant_id = pp.tenant_id AND m.project_id = pp.project_id \
                        AND m.proposal_id = pp.id) AS member_count, \
                    pp.created_at, pp.rationale, pp.risk_level, pp.confidence, pp.origin_kind \
               FROM pc.proposal pp \
              WHERE pp.source_id = $1 \
              ORDER BY pp.created_at DESC",
            )
            .bind(source_id)
            .fetch_all(pool)
            .await?
        }
        None => Vec::new(),
    };

    // 本次分析产出的 PC 对象修订：经证据钉扎反查（evidence.object_revision_id，
    // 认知修订锚各自对象的 pc.document_revision，与 KB 报告修订不同表）。
    let pc_revisions: Vec<ReportPcRevisionRow> = match header.pc_source_id {
        Some(source_id) => {
            sqlx::query_as(
                "SELECT o.id AS object_id, o.object_type, o.display_name, r.revision_no, \
                    r.change_summary, dr.content \
               FROM pc.evidence e \
               JOIN pc.cognition_revision r ON r.id = e.object_revision_id \
               JOIN pc.cognition_object o ON o.id = r.object_id \
               JOIN pc.document_revision dr ON dr.id = r.document_revision_id \
              WHERE e.source_id = $1 \
              ORDER BY o.display_name ASC",
            )
            .bind(source_id)
            .fetch_all(pool)
            .await?
        }
        None => Vec::new(),
    };

    // P3-11 加法（§8.3）：ProjC 提案集 / 对象修订集（by projc_source_id；空集 []）
    let projc_proposals: Vec<ReportProposalRow> = match header.projc_source_id {
        Some(source_id) => {
            sqlx::query_as(
                "SELECT jp.id AS proposal_id, jp.title, jp.status, jp.expected_version_no, \
                    (SELECT count(*) FROM projc.proposal_member m \
                      WHERE m.tenant_id = jp.tenant_id AND m.project_id = jp.project_id \
                        AND m.proposal_id = jp.id) AS member_count, \
                    jp.created_at, jp.rationale, jp.risk_level, jp.confidence, jp.origin_kind \
               FROM projc.proposal jp \
              WHERE jp.source_id = $1 \
              ORDER BY jp.created_at DESC",
            )
            .bind(source_id)
            .fetch_all(pool)
            .await?
        }
        None => Vec::new(),
    };
    let projc_revisions: Vec<ReportPcRevisionRow> = match header.projc_source_id {
        Some(source_id) => {
            sqlx::query_as(
                "SELECT o.id AS object_id, o.object_type, o.display_name, r.revision_no, \
                    r.change_summary, dr.content \
               FROM projc.evidence e \
               JOIN projc.cognition_revision r ON r.id = e.object_revision_id \
               JOIN projc.cognition_object o ON o.id = r.object_id \
               JOIN projc.document_revision dr ON dr.id = r.document_revision_id \
              WHERE e.source_id = $1 \
              ORDER BY o.display_name ASC",
            )
            .bind(source_id)
            .fetch_all(pool)
            .await?
        }
        None => Vec::new(),
    };

    // 证据投影（加法）：审阅判断依据——支撑对象名经 revision→object 反查。
    let pc_evidence: Vec<ReportEvidenceRow> = match header.pc_source_id {
        Some(source_id) => sqlx::query_as(
            "SELECT e.evidence_type, o.display_name AS object_name, o.object_type, \
                    e.section_anchor, e.file_path, e.commit_sha, e.start_line, e.end_line, \
                    e.excerpt, e.description, e.confidence, e.verification_status \
               FROM pc.evidence e \
               JOIN pc.cognition_revision r ON r.id = e.object_revision_id \
               JOIN pc.cognition_object o ON o.id = r.object_id \
              WHERE e.source_id = $1 \
              ORDER BY o.display_name ASC, e.created_at ASC",
        )
        .bind(source_id)
        .fetch_all(pool)
        .await?,
        None => Vec::new(),
    };
    let projc_evidence: Vec<ReportEvidenceRow> = match header.projc_source_id {
        Some(source_id) => sqlx::query_as(
            "SELECT e.evidence_type, o.display_name AS object_name, o.object_type, \
                    e.section_anchor, e.file_path, e.commit_sha, e.start_line, e.end_line, \
                    e.excerpt, e.description, e.confidence, e.verification_status \
               FROM projc.evidence e \
               JOIN projc.cognition_revision r ON r.id = e.object_revision_id \
               JOIN projc.cognition_object o ON o.id = r.object_id \
              WHERE e.source_id = $1 \
              ORDER BY o.display_name ASC, e.created_at ASC",
        )
        .bind(source_id)
        .fetch_all(pool)
        .await?,
        None => Vec::new(),
    };

    Ok(ReportDetail {
        header,
        kb_revision,
        proposals,
        pc_revisions,
        projc_proposals,
        projc_revisions,
        pc_evidence,
        projc_evidence,
    })
}

// ── R-1 分类事务（Q10-R1-FIX V1.2/V1.2.1 R1/V1.2.2 §一；II-A 契约）─────────

/// R-1 分类结果。不变式（V1.2.1 R1.2）：I-1 `Replay` 唯一来源=持锁 JOIN 确认
/// `applied ∧ completed`——技术故障绝不产生 Replay；I-2 技术故障必为
/// `FailedTech` + `tracing::warn` 结构化日志（不伪装 Replay、不静默）；
/// I-3 F3 必显式回滚（勿依赖 drop 时序）。
#[derive(Debug)]
pub enum ClassifyOutcome {
    /// 分类时点已确认先行发布完成（`applied ∧ completed`）。
    Replay,
    /// 业务失败：分类时点未确认先行发布完成（含未找到提案 / 条件不满足）。
    Failed,
    /// 分类事务自身技术故障（F1-F5；计数口径 II-A 仍 failed，warn 日志可辨）。
    FailedTech {
        kind: &'static str,
        error: sqlx::Error,
        /// 双故障并列保留（V1.2.2 §1.1）：回滚自身失败的错误（原始故障在
        /// `error` 不丢失）；此时 kind=`join_and_rollback`。
        cleanup_error: Option<sqlx::Error>,
    },
}

/// pc 发布 advisory 获取语句（**逐字同** pc/publish.rs:117——键文本与括号形态零差）。
const PC_PUBLISH_ADVISORY_SQL: &str =
    "SELECT pg_advisory_xact_lock(hashtext('pc_publish:' || $1::text)::bigint)";
/// projc 发布 advisory 获取语句（**逐字同** projc/publish.rs:118）。
const PROJC_PUBLISH_ADVISORY_SQL: &str =
    "SELECT pg_advisory_xact_lock(hashtext('projc_publish:' || $1::text)::bigint)";

/// R-1 分类读单条 JOIN（单语句单快照；V1.2 §2.1 形态，与
/// q10_r3_snapshot_check.rs 的契约文本同构）。
const PC_CLASSIFY_JOIN_SQL: &str = "SELECT p.status, wo.status \
     FROM pc.proposal p \
     LEFT JOIN pc.write_operation wo \
       ON wo.tenant_id = p.tenant_id AND wo.project_id = p.project_id \
      AND wo.operation_id = 'pc-decision:' || p.id::text \
    WHERE p.id = $1";
const PROJC_CLASSIFY_JOIN_SQL: &str = "SELECT p.status, wo.status \
     FROM projc.proposal p \
     LEFT JOIN projc.write_operation wo \
       ON wo.tenant_id = p.tenant_id AND wo.project_id = p.project_id \
      AND wo.operation_id = 'projc-decision:' || p.id::text \
    WHERE p.id = $1";

/// R-1 分类事务核心（V1.2.1 R1.2 五失败点 + V1.2.2 §1.1 双错误保留）：
/// BEGIN → 既有 publish advisory（逐字同 publish.rs:117/:118）→ 单条 JOIN →
/// COMMIT 的独立短命只读事务。READ COMMITTED 语句快照 + advisory 串行化下
/// JOIN 读到 P 序列化下的分类时点终局（V1.2.2 §2.2 四步证明）；事务内仅
/// SELECT（advisory 无数据效应），零状态机/Schema 触碰。
async fn classify_after_publish_err(
    pool: &PgPool,
    advisory_sql: &str,
    join_sql: &str,
    project_id: Uuid,
    proposal_id: Uuid,
) -> ClassifyOutcome {
    // F1：begin 失败——无事务、未获任何锁
    let mut cls = match pool.begin().await {
        Ok(t) => t,
        Err(e) => {
            return ClassifyOutcome::FailedTech { kind: "begin", error: e, cleanup_error: None };
        }
    };
    // F2：advisory 失败——开启未提交、未持有锁；drop 清理（回滚成败不可观测
    // =接受限制，审批附件补充 A；锁释放由服务端会话中断兜底）
    if let Err(e) = sqlx::query(advisory_sql)
        .bind(project_id.to_string())
        .execute(&mut *cls)
        .await
    {
        return ClassifyOutcome::FailedTech { kind: "advisory", error: e, cleanup_error: None };
    }
    // 单条 JOIN（表级 ACCESS SHARE、不取行锁——V1.2.2 §2.1；唯一阻塞点=上方 advisory）
    let row: Result<Option<(String, Option<String>)>, sqlx::Error> =
        sqlx::query_as(join_sql).bind(proposal_id).fetch_optional(&mut *cls).await;
    let row = match row {
        Ok(r) => r,
        Err(e) => {
            // F3：持 advisory 中——显式回滚释放（I-3）；双故障并列保留原始错误
            return match cls.rollback().await {
                Ok(()) => {
                    ClassifyOutcome::FailedTech { kind: "join", error: e, cleanup_error: None }
                }
                Err(rbe) => ClassifyOutcome::FailedTech {
                    kind: "join_and_rollback",
                    error: e,
                    cleanup_error: Some(rbe),
                },
            };
        }
    };
    // F4：COMMIT 失败——行未采信（中止态，服务端已回滚；drop 无操作）
    match cls.commit().await {
        Ok(()) => match row {
            Some((p, w)) if p == "applied" && w.as_deref() == Some("completed") => {
                ClassifyOutcome::Replay
            }
            _ => ClassifyOutcome::Failed,
        },
        Err(e) => ClassifyOutcome::FailedTech { kind: "commit", error: e, cleanup_error: None },
    }
}

/// pc 域 publish Err 后分类（`pub` 供 T8 L2 层测试直调封装边界）。
pub async fn classify_pc_after_publish_err(
    pool: &PgPool,
    project_id: Uuid,
    proposal_id: Uuid,
) -> ClassifyOutcome {
    classify_after_publish_err(
        pool,
        PC_PUBLISH_ADVISORY_SQL,
        PC_CLASSIFY_JOIN_SQL,
        project_id,
        proposal_id,
    )
    .await
}

/// projc 域 publish Err 后分类（两域同构；advisory 逐字同 projc/publish.rs:118）。
pub async fn classify_projc_after_publish_err(
    pool: &PgPool,
    project_id: Uuid,
    proposal_id: Uuid,
) -> ClassifyOutcome {
    classify_after_publish_err(
        pool,
        PROJC_PUBLISH_ADVISORY_SQL,
        PROJC_CLASSIFY_JOIN_SQL,
        project_id,
        proposal_id,
    )
    .await
}

/// 关闭报告（人工显式动作）：仅 `analyzed` 可关。收尾处置——双域分流（P3-11
/// G-2）：pc 块逐行不动（C-4~C-6 契约），新增 ProjC 追加块——approved projc 提案
/// 逐个发布（`projc-decision:{id}` 幂等键；CAS 失配内置 failed；失败独立计数不
/// 阻断关闭）、未决 → withdrawn（逐条留审计）；NULL projc_source_id = 显式
/// Legacy（跳过 ProjC 分支，审计 payload 带 `projc_branch:"legacy_no_source"`，
/// 绝不静默）。KB 草稿修订 → rejected；随后固化 D-1 锚点。关闭后门释放。
pub async fn close_report(
    pool: &PgPool,
    project_id: Uuid,
    report_id: Uuid,
    actor: &AuditActor,
) -> Result<CloseOutcome, AnalysisReportError> {
    let mut tx = pool.begin().await?;

    // 项目边界 + 行锁（tenant/project 双重校验；项目不匹配一律 NotFound）。
    let row: Option<(Uuid, String, Option<Uuid>, Option<Uuid>, Option<Uuid>, String)> =
        sqlx::query_as(
            "SELECT repository_id, commit_sha, pc_source_id, kb_revision_id, projc_source_id, status \
               FROM integration.repo_analysis_report \
              WHERE id = $1 AND project_id = $2 \
                AND tenant_id = (SELECT tenant_id FROM core.project WHERE id = $2) \
              FOR UPDATE",
        )
        .bind(report_id)
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await?;
    let Some((repository_id, commit_sha, pc_source_id, kb_revision_id, projc_source_id, status)) =
        row
    else {
        return Err(AnalysisReportError::NotFound);
    };
    if status != "analyzed" {
        return Err(AnalysisReportError::Conflict(format!(
            "report is `{status}`; only an analyzed report can be closed"
        )));
    }

    // 已批准提案 → 批量发布（审阅流 v2：决定暂记，close 才写 PC）。publish_version
    // 自带事务、advisory 锁与幂等键；CAS 失配由发布事务内置 failed（§18.1②），
    // 其余失败计数不阻断——报告照常关闭。
    let mut applied_proposals: u64 = 0;
    let mut replayed_proposals: u64 = 0;
    let mut publish_failed_proposals: u64 = 0;
    let mut published_versions: Vec<PublishedVersionEntry> = Vec::new();
    if let Some(source_id) = pc_source_id {
        let approved_ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM pc.proposal WHERE source_id = $1 AND status = 'approved'",
        )
        .bind(source_id)
        .fetch_all(&mut *tx)
        .await?;
        for proposal_id in approved_ids {
            match crate::pc::publish_version(
                pool,
                project_id,
                actor,
                crate::pc::PublishVersionInput { proposal_id },
            )
            .await
            {
                Ok(receipt) => {
                    if receipt.replayed {
                        // 短路确认先行完成（II-A replayed 来源①；V1.2.1 R4.2）
                        replayed_proposals += 1;
                    } else {
                        applied_proposals += 1;
                    }
                    // applied 与 replayed 都产生了正式版本——均记入发布清单。
                    published_versions.push(PublishedVersionEntry {
                        domain: "pc",
                        proposal_id,
                        version_no: receipt.version_no,
                    });
                }
                Err(publish_err) => {
                    // R-1 分类（II-A）：BEGIN → 既有 advisory（逐字同
                    // pc/publish.rs:117）→ 单条 JOIN → COMMIT 独立只读事务；
                    // 技术故障绝不产生 Replay（不变式 I-1）
                    match classify_pc_after_publish_err(pool, project_id, proposal_id).await {
                        ClassifyOutcome::Replay => replayed_proposals += 1,
                        ClassifyOutcome::Failed => publish_failed_proposals += 1,
                        ClassifyOutcome::FailedTech { kind, error, cleanup_error } => {
                            tracing::warn!(
                                kind,
                                proposal_id = %proposal_id,
                                source_publish_error = %publish_err,
                                error = %error,
                                cleanup_error = ?cleanup_error,
                                "R-1 classify tx technical failure"
                            );
                            publish_failed_proposals += 1;
                        }
                    }
                }
            }
        }
    }

    // 未决 PC 提案 → withdrawn（逐条审计；决策事实留痕同 withdraw_proposal 口径）。
    let mut withdrawn: u64 = 0;
    if let Some(source_id) = pc_source_id {
        let ids: Vec<(Uuid, String)> = sqlx::query_as(
            "SELECT id, status FROM pc.proposal \
              WHERE source_id = $1 AND status IN ('draft','submitted') FOR UPDATE",
        )
        .bind(source_id)
        .fetch_all(&mut *tx)
        .await?;
        for (proposal_id, prev) in ids {
            sqlx::query(
                "UPDATE pc.proposal SET status = 'withdrawn', updated_at = now() WHERE id = $1",
            )
            .bind(proposal_id)
            .execute(&mut *tx)
            .await?;
            crate::pm_write::append_audit_with_after(
                &mut tx,
                project_id,
                actor,
                "pc_proposal.withdraw",
                "pc_proposal",
                Some(proposal_id),
                serde_json::json!({ "status": prev, "reason": "analysis report closed" }),
            )
            .await?;
            withdrawn += 1;
        }
    }

    // ── P3-11 新增：ProjC 分流（纯追加块；G-2 与 pc 同构、独立计数）──────────
    let mut projc_applied: u64 = 0;
    let mut projc_replayed: u64 = 0;
    let mut projc_publish_failed: u64 = 0;
    let mut projc_withdrawn: u64 = 0;
    let mut projc_legacy_no_source = false;
    match projc_source_id {
        Some(projc_source) => {
            // 已批准 ProjC 提案 → 批量发布（projc-decision:{id}；CAS 失配内置
            // failed；失败独立计数不阻断——pc 块同口径）。
            let projc_approved_ids: Vec<Uuid> = sqlx::query_scalar(
                "SELECT id FROM projc.proposal WHERE source_id = $1 AND status = 'approved'",
            )
            .bind(projc_source)
            .fetch_all(&mut *tx)
            .await?;
            for proposal_id in projc_approved_ids {
                match crate::projc::publish_version(
                    pool,
                    project_id,
                    actor,
                    crate::projc::PublishVersionInput { proposal_id },
                )
                .await
                {
                    Ok(receipt) => {
                        if receipt.replayed {
                            // 短路确认先行完成（II-A replayed 来源①；V1.2.1 R4.2）
                            projc_replayed += 1;
                        } else {
                            projc_applied += 1;
                        }
                        published_versions.push(PublishedVersionEntry {
                            domain: "projc",
                            proposal_id,
                            version_no: receipt.version_no,
                        });
                    }
                    Err(publish_err) => {
                        // R-1 分类（II-A）：advisory 逐字同 projc/publish.rs:118
                        match classify_projc_after_publish_err(pool, project_id, proposal_id)
                            .await
                        {
                            ClassifyOutcome::Replay => projc_replayed += 1,
                            ClassifyOutcome::Failed => projc_publish_failed += 1,
                            ClassifyOutcome::FailedTech { kind, error, cleanup_error } => {
                                tracing::warn!(
                                    kind,
                                    proposal_id = %proposal_id,
                                    source_publish_error = %publish_err,
                                    error = %error,
                                    cleanup_error = ?cleanup_error,
                                    "R-1 classify tx technical failure"
                                );
                                projc_publish_failed += 1;
                            }
                        }
                    }
                }
            }
            // 未决 ProjC 提案 → withdrawn（逐条审计，projc_ 前缀口径）。
            let projc_pending: Vec<(Uuid, String)> = sqlx::query_as(
                "SELECT id, status FROM projc.proposal \
                  WHERE source_id = $1 AND status IN ('draft','submitted') FOR UPDATE",
            )
            .bind(projc_source)
            .fetch_all(&mut *tx)
            .await?;
            for (proposal_id, prev) in projc_pending {
                sqlx::query(
                    "UPDATE projc.proposal SET status = 'withdrawn', updated_at = now() \
                      WHERE id = $1",
                )
                .bind(proposal_id)
                .execute(&mut *tx)
                .await?;
                crate::pm_write::append_audit_with_after(
                    &mut tx,
                    project_id,
                    actor,
                    "projc_proposal.withdraw",
                    "projc_proposal",
                    Some(proposal_id),
                    serde_json::json!({ "status": prev, "reason": "analysis report closed" }),
                )
                .await?;
                projc_withdrawn += 1;
            }
        }
        None => {
            // NULL projc_source_id 仅可能出现在 0026 之前的历史报告行 = 显式
            // Legacy：跳过 ProjC 分支，审计留痕（绝不静默）。
            projc_legacy_no_source = true;
        }
    }

    // 未确认 KB 草稿修订 → rejected（0023 已放宽守卫白名单）。
    let mut rejected_kb = false;
    if let Some(rev) = kb_revision_id {
        let done = sqlx::query(
            "UPDATE kb.document_revision SET status = 'rejected' \
              WHERE id = $1 AND status = 'draft'",
        )
        .bind(rev)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        rejected_kb = done > 0;
        if rejected_kb {
            crate::pm_write::append_audit_with_after(
                &mut tx,
                project_id,
                actor,
                "kb.document_revision.rejected",
                "kb_document_revision",
                Some(rev),
                serde_json::json!({ "status": "rejected", "reason": "analysis report closed" }),
            )
            .await?;
        }
    }

    // 报告 closed + 固化 D-1 锚点（revision 计数与 verify 同款 +1，乐观锁惯例）。
    sqlx::query(
        "UPDATE integration.repo_analysis_report \
          SET status = 'closed', closed_at = now(), closed_by = $2, updated_at = now() \
         WHERE id = $1 AND status = 'analyzed'",
    )
    .bind(report_id)
    .bind(actor.principal_id)
    .execute(&mut *tx)
    .await?;
    let mut audit_payload = serde_json::json!({
        "status": "closed",
        "commit_sha": commit_sha,
        "applied_proposals": applied_proposals,
        "publish_failed_proposals": publish_failed_proposals,
        "withdrawn_proposals": withdrawn,
        "rejected_kb": rejected_kb,
        "projc_applied": projc_applied,
        "projc_publish_failed_proposals": projc_publish_failed,
        "projc_withdrawn": projc_withdrawn,
    });
    if projc_legacy_no_source {
        audit_payload["projc_branch"] = serde_json::json!("legacy_no_source");
    }
    crate::pm_write::append_audit_with_after(
        &mut tx,
        project_id,
        actor,
        "analysis_report.closed",
        "repo_analysis_report",
        Some(report_id),
        audit_payload,
    )
    .await?;
    sqlx::query(
        "UPDATE integration.project_repository \
          SET last_analyzed_commit_sha = $2, last_analyzed_at = now(), \
              revision = revision + 1, updated_at = now() \
         WHERE id = $1",
    )
    .bind(repository_id)
    .bind(&commit_sha)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(CloseOutcome {
        report_id,
        status: "closed",
        applied_proposals,
        replayed_proposals,
        publish_failed_proposals,
        withdrawn_proposals: withdrawn,
        rejected_kb,
        last_analyzed_commit: commit_sha,
        projc_applied,
        projc_replayed,
        projc_publish_failed_proposals: projc_publish_failed,
        projc_withdrawn,
        published_versions,
    })
}

/// 供审计/通知复用的报告标题（与提案标题同风格：仓库名 @ 短 sha）。
pub fn report_title(repository_name: &str, commit_sha: &str) -> String {
    let short: String = commit_sha.chars().take(12).collect();
    format!("{repository_name} @ {short}")
}

/// 确认 actor 可执行关闭（服务层门，镜像 kb confirm：interactive 真人会话）。
pub fn ensure_closable(actor: &AuditActor) -> Result<(), AnalysisReportError> {
    match actor.auth_mode.as_deref() {
        Some("interactive") => Ok(()),
        _ => Err(AnalysisReportError::Forbidden(
            "closing an analysis report requires an interactive human session".to_string(),
        )),
    }
}
