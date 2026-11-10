//! KB 文档修订确认（WP-0 内容管线接缝）。
//!
//! **为什么有这一扇门**：`kb.document_revision` 此前只有 draft 落库（0053 digest
//! 半层），全仓无 draft→published 转换代码——文档进不了「现行」态，检索索引管线
//! （WP-3）无事件可消费，整条检索链是空的。本模块开这一扇门。
//!
//! **确认权归属（WP-0 裁决 Q-A=A\*）**：项目内任何真人成员可确认，但必须同时满足
//! 三重门：
//!
//! 1. **可信人类交互上下文**——`auth_mode='interactive'`（caller.rs 三分类：
//!    interactive / api_key / unknown）。`api_key`（Agent / Service Principal）与
//!    `unknown` 一律拒绝：**携带真人 JWT 不等于真人操作**，无法区分真人与 Agent
//!    代持凭据时拒绝确认。这是 auth_mode 三分类的**首个准入消费者**。
//! 2. **真人主体**——`core.principal.kind='human'`（经 Platform 鉴权函数）。
//! 3. **项目成员**——项目活跃 + 可见性（非成员一律 404，不泄漏项目存在性）。
//!
//! 不要求 Project Lead 或额外 Approver Grant；**Tenant Owner 不绕项目边界**。
//! MCP 不提供正式确认能力——确认入口只开 REST/UI。
//!
//! **状态机纪律**（裁决 §四核对结论）：本表库枚举是 `draft / published / rejected`，
//! 本批只交付 **draft → published** 单向转换。`approved` / `pending_confirmation` /
//! `superseded` 等词表属于 KB 记忆域审核面与旧 ai 域修订表，**不是本表**——本模块
//! 不引入新状态词、不做 approved↔published 映射。`rejected` 枚举原样保留（reject
//! 操作能力后置，生命周期语义不删）。
//!
//! **事务性事件**：状态变更与 outbox 事件**同事务提交**——不得先改库态再异步补事件。

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::outbox::{OutboxAppend, OutboxEvent, PgOutboxAppend};

use super::{audit, ensure_project_writable, KbServiceError, TYPE_DOCUMENT};

/// 审计动作名。
pub const AUDIT_ACTION_PUBLISHED: &str = "kb.document_revision.published";

/// 确认结果。
#[derive(Debug, Clone, Serialize)]
pub struct ConfirmOutcome {
    pub document_id: Uuid,
    pub revision_id: Uuid,
    pub revision_no: i32,
    pub status: String,
    pub confirmed_by: Uuid,
    pub published_at: DateTime<Utc>,
    /// 幂等命中：本次调用未改库态（原确认人 / 确认时间保持不变）。
    pub already_published: bool,
}

/// 确认 KB 文档修订（draft → published）。
///
/// 处理顺序（WP-0 裁决定版，不可换序）：
/// 请求进入 → 身份真实性校验 → Project Membership 校验 → Revision 所属 Document
/// 校验 → Revision 状态校验 → 事务（draft→published + 写确认人/时间 + outbox）
/// → 提交 → 返回。
pub async fn confirm_revision(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    document_id: Uuid,
    revision_id: Uuid,
    actor: &AuditActor,
) -> Result<ConfirmOutcome, KbServiceError> {
    // ① 身份真实性校验——纯内存 fail-fast，不开事务。
    //    fail-closed：非 interactive 一律拒绝（api_key = Agent/Service；unknown = 不可信）。
    match actor.auth_mode.as_deref() {
        Some("interactive") => {}
        _ => {
            return Err(KbServiceError::Forbidden(
                "kb revision confirmation requires an interactive human session",
            ))
        }
    }

    let mut tx = pool.begin().await?;

    // ② Project Membership 校验（项目行锁下确认项目仍活跃——同 K4 锁序）。
    ensure_project_writable(&mut tx, project_id).await?;

    // ③ 真人主体校验（连接版：调用方已在事务内，避免单连接池自锁）。
    crate::requirement::require_human_conn(tx.as_mut(), actor)
        .await
        .map_err(|e| match e {
            crate::pm_write::PmWriteError::Forbidden(_)
            | crate::pm_write::PmWriteError::NotFound(_) => {
                KbServiceError::Forbidden("kb revision confirmation requires a human principal")
            }
            crate::pm_write::PmWriteError::Db(err) => KbServiceError::Db(err),
            crate::pm_write::PmWriteError::ValidationMsg(m) => KbServiceError::Invalid(m),
            crate::pm_write::PmWriteError::Conflict(m)
            | crate::pm_write::PmWriteError::Validation(m) => KbServiceError::Invalid(m.into()),
        })?;

    // ④ Revision 所属 Document 校验 + 租户/项目归属校验（一次 JOIN 取齐）。
    //    任一不匹配一律 404——防跨项目枚举。
    let row = sqlx::query(
        "SELECT r.revision_no, r.status, r.confirmed_by, r.published_at \
           FROM kb.document_revision r \
           JOIN kb.document d ON d.id = r.document_id \
          WHERE r.id = $1 AND r.document_id = $2 \
            AND r.tenant_id = $3 AND d.tenant_id = $3 AND d.project_id = $4",
    )
    .bind(revision_id)
    .bind(document_id)
    .bind(tenant_id)
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(row) = row else {
        return Err(KbServiceError::NotFound(
            "kb document revision not found in this project",
        ));
    };

    let revision_no: i32 = row.get("revision_no");
    let status: String = row.get("status");
    let confirmed_by: Option<Uuid> = row.get("confirmed_by");
    let published_at: Option<DateTime<Utc>> = row.get("published_at");

    // ⑤ Revision 状态校验。
    match status.as_str() {
        // 幂等：已发布再次确认 → no-op，**不改原确认人与确认时间**。
        "published" => {
            tx.rollback().await?;
            return Ok(ConfirmOutcome {
                document_id,
                revision_id,
                revision_no,
                status,
                confirmed_by: confirmed_by.unwrap_or_default(),
                published_at: published_at.unwrap_or_default(),
                already_published: true,
            });
        }
        "draft" => {}
        "rejected" => {
            return Err(KbServiceError::Conflict(
                "kb document revision is rejected and cannot be published".into(),
            ))
        }
        other => {
            return Err(KbServiceError::Conflict(format!(
                "kb document revision has an unsupported status `{other}`"
            )))
        }
    }

    // ⑥ 事务内：状态变更 + 确认人/时间 + 审计 + outbox 事件。
    //    时间戳以**库时钟**为准（`now()` + RETURNING 回读）——PG timestamptz 是微秒
    //    精度，Rust `Utc::now()` 是纳秒，用应用层时间会让返回值与库内不一致
    //    （幂等断言即依赖两者相等）。
    let published_at: DateTime<Utc> = sqlx::query_scalar(
        "UPDATE kb.document_revision \
            SET status = 'published', confirmed_by = $2, published_at = now() \
          WHERE id = $1 AND status = 'draft' \
          RETURNING published_at",
    )
    .bind(revision_id)
    .bind(actor.principal_id)
    .fetch_one(&mut *tx)
    .await?;

    audit(
        &mut tx,
        project_id,
        actor,
        AUDIT_ACTION_PUBLISHED,
        "kb_document_revision",
        revision_id,
        serde_json::json!({
            "document_id": document_id.to_string(),
            "revision_id": revision_id.to_string(),
            "revision_no": revision_no,
            "status": "published",
            "confirmed_by": actor.principal_id.to_string(),
        }),
    )
    .await?;

    emit_revision_published(
        &mut tx,
        project_id,
        document_id,
        revision_id,
        revision_no,
        actor.principal_id,
        published_at,
    )
    .await?;

    tx.commit().await?;

    Ok(ConfirmOutcome {
        document_id,
        revision_id,
        revision_no,
        status: "published".to_string(),
        confirmed_by: actor.principal_id,
        published_at,
        already_published: false,
    })
}

/// 发布事件（**与状态变更同事务**）。
async fn emit_revision_published(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
    document_id: Uuid,
    revision_id: Uuid,
    revision_no: i32,
    confirmed_by: Uuid,
    published_at: DateTime<Utc>,
) -> Result<(), KbServiceError> {
    let payload = serde_json::json!({
        "document_ref": {
            "target_domain": kb_pc_contracts::TargetDomain::KnowledgeBase.as_str(),
            "target_type": TYPE_DOCUMENT,
            "target_id": document_id.to_string(),
            "target_revision_id": revision_id.to_string(),
        },
        "confirmed_by": confirmed_by.to_string(),
        "published_at": published_at.to_rfc3339(),
        "revision_no": revision_no,
    });
    kb_pc_contracts::events::validate_payload_references(&payload).map_err(|v| {
        KbServiceError::Invalid(format!("kb.document_revision.published payload: {v}"))
    })?;
    PgOutboxAppend
        .append(
            tx,
            OutboxEvent {
                project_id,
                event_type: kb_pc_contracts::events::KB_DOCUMENT_REVISION_PUBLISHED,
                aggregate_type: "kb_document",
                aggregate_id: &document_id.to_string(),
                payload,
                idempotency_key: &format!("kb_document_revision_published:{revision_id}"),
            },
        )
        .await?;
    Ok(())
}

// ── 读面（KB 文档库管理面；前端 KnowledgeView 消费）────────────────────────

/// 文档列表行（最新修订投影）。
#[derive(Debug, Clone, Serialize)]
pub struct KbDocumentRow {
    pub document_id: Uuid,
    pub doc_key: String,
    pub doc_type: String,
    pub revision_no: i32,
    pub status: String,
    pub content_hash: String,
    pub published_at: Option<DateTime<Utc>>,
    pub confirmed_by: Option<Uuid>,
    pub updated_at: DateTime<Utc>,
}

/// 项目内 KB 文档列表（最新修订投影；项目成员可读）。
pub async fn list_documents(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<Vec<KbDocumentRow>, KbServiceError> {
    let rows = sqlx::query(
        "SELECT d.id, d.doc_key, d.doc_type, d.updated_at, \
                r.revision_no, r.status, r.content_hash, r.published_at, r.confirmed_by \
           FROM kb.document d \
           LEFT JOIN LATERAL ( \
                SELECT r2.revision_no, r2.status, r2.content_hash, r2.published_at, r2.confirmed_by \
                  FROM kb.document_revision r2 \
                 WHERE r2.document_id = d.id \
                 ORDER BY r2.revision_no DESC LIMIT 1 \
           ) r ON true \
          WHERE d.tenant_id = $1 AND d.project_id = $2 \
          ORDER BY d.updated_at DESC, d.doc_key ASC",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| KbDocumentRow {
            document_id: row.get("id"),
            doc_key: row.get("doc_key"),
            doc_type: row.get("doc_type"),
            revision_no: row.get::<Option<i32>, _>("revision_no").unwrap_or(0),
            status: row
                .get::<Option<String>, _>("status")
                .unwrap_or_else(|| "empty".to_string()),
            content_hash: row
                .get::<Option<String>, _>("content_hash")
                .unwrap_or_default(),
            published_at: row.get("published_at"),
            confirmed_by: row.get("confirmed_by"),
            updated_at: row.get("updated_at"),
        })
        .collect())
}

/// 修订历史行。
#[derive(Debug, Clone, Serialize)]
pub struct KbRevisionRow {
    pub revision_id: Uuid,
    pub revision_no: i32,
    pub status: String,
    pub content_hash: String,
    pub confirmed_by: Option<Uuid>,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// 文档详情（正文 = 最新修订；含全部修订历史）。
pub struct KbDocumentDetail {
    pub document: KbDocumentRow,
    pub content: Option<String>,
    pub revisions: Vec<KbRevisionRow>,
}

/// 文档详情（项目成员可读；404 防枚举同 confirm）。
pub async fn get_document(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    document_id: Uuid,
) -> Result<KbDocumentDetail, KbServiceError> {
    let owned: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM kb.document \
          WHERE id = $1 AND tenant_id = $2 AND project_id = $3",
    )
    .bind(document_id)
    .bind(tenant_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    let Some(_) = owned else {
        return Err(KbServiceError::NotFound("kb document not found"));
    };

    let documents = list_documents(pool, tenant_id, project_id).await?;
    let Some(document) = documents.into_iter().find(|d| d.document_id == document_id) else {
        return Err(KbServiceError::NotFound("kb document not found"));
    };

    let revisions = sqlx::query(
        "SELECT id, revision_no, status, content_hash, confirmed_by, published_at, created_at \
           FROM kb.document_revision \
          WHERE document_id = $1 \
          ORDER BY revision_no DESC",
    )
    .bind(document_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| KbRevisionRow {
        revision_id: row.get("id"),
        revision_no: row.get("revision_no"),
        status: row.get("status"),
        content_hash: row.get("content_hash"),
        confirmed_by: row.get("confirmed_by"),
        published_at: row.get("published_at"),
        created_at: row.get("created_at"),
    })
    .collect();

    // 正文 = 最新修订内容（最新修订可能未发布——详情如实展示并标状态）。
    let content = sqlx::query_scalar(
        "SELECT content FROM kb.document_revision \
          WHERE document_id = $1 ORDER BY revision_no DESC LIMIT 1",
    )
    .bind(document_id)
    .fetch_optional(pool)
    .await?
    .flatten();

    Ok(KbDocumentDetail {
        document,
        content,
        revisions,
    })
}

/* ── 摄入链（WP-0 解析层接线；红线③：解析只能产生 Draft）────────────────── */

/// 摄入输入。
pub struct IngestInput {
    pub doc_key: String,
    pub doc_type: String,
    pub parse_kind: crate::knowledge_parse::ParseKind,
    pub bytes: Vec<u8>,
}

/// 摄入：解析（纯函数）→ 家族追加 draft 修订（新 doc_key 建 document）。
///
/// **不产事件、不发 outbox**——draft 无检索语义；真人确认（[`confirm_revision`]
/// ）才产 `published` 事件。版本族语义：同 doc_key 追加 revision_no = max+1
/// （显式指定目标家族，非文件名自动归并——后者 gated D-6）。
pub async fn ingest_document(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    input: IngestInput,
    actor: &AuditActor,
) -> Result<(Uuid, Uuid, i32), KbServiceError> {
    let parsed = crate::knowledge_parse::parse(input.parse_kind, &input.bytes)
        .map_err(|e| KbServiceError::Invalid(format!("parse failed: {e}")))?;
    if parsed.markdown.trim().is_empty() {
        return Err(KbServiceError::Invalid("parsed content is empty".into()));
    }

    let mut tx = pool.begin().await?;
    ensure_project_writable(&mut tx, project_id).await?;

    let content_hash = crate::pc::sha256_hex(&parsed.markdown);
    let document_id: Uuid = match sqlx::query_scalar(
        "SELECT id FROM kb.document \
          WHERE tenant_id = $1 AND project_id = $2 AND doc_key = $3",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(&input.doc_key)
    .fetch_optional(&mut *tx)
    .await?
    {
        Some(id) => id,
        None => sqlx::query_scalar(
            "INSERT INTO kb.document (tenant_id, project_id, doc_key, doc_type, canonical_path) \
             VALUES ($1, $2, $3, $4, $5) RETURNING id",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(&input.doc_key)
        .bind(&input.doc_type)
        .bind(format!("kb/{}", input.doc_key))
        .fetch_one(&mut *tx)
        .await?,
    };
    let revision_no: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(revision_no), 0) + 1 FROM kb.document_revision WHERE document_id = $1",
    )
    .bind(document_id)
    .fetch_one(&mut *tx)
    .await?;

    let revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO kb.document_revision \
         (tenant_id, document_id, revision_no, content_hash, status, content, created_by) \
         VALUES ($1, $2, $3, $4, 'draft', $5, $6) RETURNING id",
    )
    .bind(tenant_id)
    .bind(document_id)
    .bind(revision_no)
    .bind(&content_hash)
    .bind(&parsed.markdown)
    .bind(actor.principal_id)
    .fetch_one(&mut *tx)
    .await?;

    super::audit(
        &mut tx,
        project_id,
        actor,
        "kb.document_revision.ingested",
        "kb_document_revision",
        revision_id,
        serde_json::json!({
            "document_id": document_id.to_string(),
            "doc_key": input.doc_key,
            "revision_no": revision_no,
            "warnings": parsed.warnings,
        }),
    )
    .await?;

    tx.commit().await?;
    Ok((document_id, revision_id, revision_no))
}
