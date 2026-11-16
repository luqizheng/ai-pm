//! 对象草稿与修订推号（projc.*；pc::object 同款机制，独立业务表）。

use sqlx::PgPool;
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::pc::sha256_hex;

use super::{load_object_type, project_tenant, ProjcError};

pub struct UpsertObjectDraft {
    pub object_type: String,
    /// 受控键（项目内幂等锚）；None = 匿名草稿（每次新对象）。
    pub object_key: Option<String>,
    pub display_name: String,
    pub content_markdown: String,
    pub change_summary: Option<String>,
    /// 生成语境摘要（AI 链路填分析语境，人工草稿可空）。
    pub source_context: Option<String>,
    /// 分析归因（derived/derived_fallback/llm）；None = 非分析产物。
    pub analysis_mode: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ObjectUpsertReceipt {
    pub object_id: Uuid,
    pub revision_id: Uuid,
    pub revision_no: i32,
    pub revision_created: bool,
}

fn doc_slug(object_key: Option<&str>, object_id: Uuid) -> (String, String) {
    // 正典 doc_key = projc-object-{object_key} 仅当 object_key 为 slug 字符集
    // （[A-Za-z0-9._-]）；否则兜底 projc-object-{object_id}（pc 偏差 e 同款）。
    let valid = object_key
        .map(|k| {
            !k.is_empty()
                && k.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
        })
        .unwrap_or(false);
    match (valid, object_key) {
        (true, Some(key)) => {
            let doc_key = format!("projc-object-{key}");
            let path = format!("projc/object/{key}.md");
            (doc_key, path)
        }
        _ => (
            format!("projc-object-{object_id}"),
            format!("projc/object/{object_id}.md"),
        ),
    }
}

/// 对象草稿 upsert（分析链路与人工草稿共用写入口；pc::upsert_object_draft 同款语义）：
/// * 匿名草稿（object_key=None）恒新建；
/// * 具名草稿按 object_key 幂等：最新修订内容同 hash → 复用（内容寻址去重）；
///   否则行锁推号新修订 + CAS 回填指针；
/// * display_name 跟随最新一次 upsert；
/// * 终态（deprecated/superseded）对象拒绝追加修订（复活走新提案）。
pub async fn upsert_object_draft(
    pool: &PgPool,
    project_id: Uuid,
    actor: &AuditActor,
    input: UpsertObjectDraft,
) -> Result<ObjectUpsertReceipt, ProjcError> {
    let spec = load_object_type(pool, &input.object_type).await?;
    let _ = spec;
    let tenant_id = project_tenant(pool, project_id).await?;
    let content_hash = sha256_hex(&input.content_markdown);

    let mut tx = pool.begin().await?;

    let existing: Option<(Uuid, String)> = match &input.object_key {
        Some(key) => {
            sqlx::query_as(
                "SELECT id, lifecycle_status FROM projc.cognition_object \
                  WHERE tenant_id = $1 AND project_id = $2 AND object_key = $3",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(key)
            .fetch_optional(&mut *tx)
            .await?
        }
        None => None,
    };

    let receipt = match existing {
        Some((object_id, lifecycle)) => {
            if lifecycle == "deprecated" || lifecycle == "superseded" {
                return Err(ProjcError::Conflict(format!(
                    "object {object_id} is {lifecycle}; revival requires a new proposal"
                )));
            }
            // 最新修订内容寻址去重
            let latest: Option<(i32, Uuid, String)> = sqlx::query_as(
                "SELECT cr.revision_no, cr.id, cr.content_hash \
                   FROM projc.cognition_revision cr \
                  WHERE cr.tenant_id = $1 AND cr.project_id = $2 AND cr.object_id = $3 \
                  ORDER BY cr.revision_no DESC LIMIT 1",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(object_id)
            .fetch_optional(&mut *tx)
            .await?;
            if let Some((no, rev_id, hash)) = latest {
                if hash == content_hash {
                    // 内容未变：仍刷新 display_name（受控投影）
                    sqlx::query(
                        "UPDATE projc.cognition_object SET display_name = $2, updated_by = $3, \
                         updated_at = now() WHERE id = $1",
                    )
                    .bind(object_id)
                    .bind(&input.display_name)
                    .bind(actor.principal_id)
                    .execute(&mut *tx)
                    .await?;
                    tx.commit().await?;
                    return Ok(ObjectUpsertReceipt {
                        object_id,
                        revision_id: rev_id,
                        revision_no: no,
                        revision_created: false,
                    });
                }
                // 追加修订：对象行锁推号
                let locked: (Option<Uuid>, String) = sqlx::query_as(
                    "SELECT current_revision_id, updated_at::text FROM projc.cognition_object \
                      WHERE id = $1 FOR UPDATE",
                )
                .bind(object_id)
                .fetch_one(&mut *tx)
                .await?;
                let revision_id = append_revision(
                    &mut tx,
                    tenant_id,
                    project_id,
                    object_id,
                    no + 1,
                    &content_hash,
                    input.change_summary.as_deref(),
                    input.source_context.as_deref(),
                    input.analysis_mode.as_deref(),
                    actor.principal_id,
                    &input.content_markdown,
                )
                .await?;
                sqlx::query(
                    "UPDATE projc.cognition_object \
                        SET current_revision_id = $2, updated_by = $3, updated_at = now() \
                      WHERE id = $1 AND current_revision_id IS NOT DISTINCT FROM $4",
                )
                .bind(object_id)
                .bind(revision_id)
                .bind(actor.principal_id)
                .bind(locked.0)
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
                return Ok(ObjectUpsertReceipt {
                    object_id,
                    revision_id,
                    revision_no: no + 1,
                    revision_created: true,
                });
            }
            // 身份行存在但尚无修订（identity 占位）：补首修订
            let revision_id = first_revision_for(
                &mut tx,
                tenant_id,
                project_id,
                object_id,
                &content_hash,
                input.change_summary.as_deref(),
                input.source_context.as_deref(),
                input.analysis_mode.as_deref(),
                actor.principal_id,
                &input.content_markdown,
            )
            .await?;
            sqlx::query(
                "UPDATE projc.cognition_object \
                    SET current_revision_id = $2, lifecycle_status = 'draft', \
                        updated_by = $3, updated_at = now() \
                  WHERE id = $1",
            )
            .bind(object_id)
            .bind(revision_id)
            .bind(actor.principal_id)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            Ok(ObjectUpsertReceipt {
                object_id,
                revision_id,
                revision_no: 1,
                revision_created: true,
            })
        }
        None => {
            // 新对象：identity → doc → revision(1) → draft + 指针
            let object_id: Uuid = sqlx::query_scalar(
                "INSERT INTO projc.cognition_object \
                 (tenant_id, project_id, object_type, object_key, display_name, lifecycle_status, \
                  created_by, updated_by) \
                 VALUES ($1, $2, $3, $4, $5, 'identity', $6, $6) RETURNING id",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(&input.object_type)
            .bind(&input.object_key)
            .bind(&input.display_name)
            .bind(actor.principal_id)
            .fetch_one(&mut *tx)
            .await?;
            let (doc_key, canonical_path) = doc_slug(input.object_key.as_deref(), object_id);
            let document_id: Uuid = sqlx::query_scalar(
                "INSERT INTO projc.document (tenant_id, project_id, doc_key, canonical_path) \
                 VALUES ($1, $2, $3, $4) RETURNING id",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(&doc_key)
            .bind(&canonical_path)
            .fetch_one(&mut *tx)
            .await?;
            let document_revision_id: Uuid = sqlx::query_scalar(
                "INSERT INTO projc.document_revision \
                 (tenant_id, project_id, document_id, revision_no, content_hash, content, created_by) \
                 VALUES ($1, $2, $3, 1, $4, $5, $6) RETURNING id",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(document_id)
            .bind(&content_hash)
            .bind(&input.content_markdown)
            .bind(actor.principal_id)
            .fetch_one(&mut *tx)
            .await?;
            let revision_id: Uuid = sqlx::query_scalar(
                "INSERT INTO projc.cognition_revision \
                 (tenant_id, project_id, object_id, revision_no, document_id, document_revision_id, \
                  content_hash, change_summary, source_context, analysis_mode, created_by) \
                 VALUES ($1, $2, $3, 1, $4, $5, $6, $7, $8, $9, $10) RETURNING id",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(object_id)
            .bind(document_id)
            .bind(document_revision_id)
            .bind(&content_hash)
            .bind(&input.change_summary)
            .bind(&input.source_context)
            .bind(&input.analysis_mode)
            .bind(actor.principal_id)
            .fetch_one(&mut *tx)
            .await?;
            sqlx::query(
                "UPDATE projc.cognition_object \
                    SET current_revision_id = $2, lifecycle_status = 'draft', updated_at = now() \
                  WHERE id = $1",
            )
            .bind(object_id)
            .bind(revision_id)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            Ok(ObjectUpsertReceipt {
                object_id,
                revision_id,
                revision_no: 1,
                revision_created: true,
            })
        }
    };
    receipt
}

#[allow(clippy::too_many_arguments)]
async fn append_revision(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    project_id: Uuid,
    object_id: Uuid,
    revision_no: i32,
    content_hash: &str,
    change_summary: Option<&str>,
    source_context: Option<&str>,
    analysis_mode: Option<&str>,
    created_by: Uuid,
    content_markdown: &str,
) -> Result<Uuid, ProjcError> {
    let document_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM projc.document WHERE tenant_id = $1 AND project_id = $2 \
          AND doc_key LIKE 'projc-object-%' \
          AND id = (SELECT d.id FROM projc.cognition_revision r \
                     JOIN projc.document d ON d.tenant_id = r.tenant_id \
                                  AND d.project_id = r.project_id \
                                  AND d.id = r.document_id \
                    WHERE r.tenant_id = $1 AND r.project_id = $2 AND r.object_id = $3 \
                    ORDER BY r.revision_no DESC LIMIT 1)",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(object_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| ProjcError::NotFound("object document not found".into()))?;
    let document_revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO projc.document_revision \
         (tenant_id, project_id, document_id, revision_no, content_hash, content, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(document_id)
    .bind(revision_no)
    .bind(content_hash)
    .bind(content_markdown)
    .bind(created_by)
    .fetch_one(&mut **tx)
    .await?;
    let revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO projc.cognition_revision \
         (tenant_id, project_id, object_id, revision_no, document_id, document_revision_id, \
          content_hash, change_summary, source_context, analysis_mode, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(object_id)
    .bind(revision_no)
    .bind(document_id)
    .bind(document_revision_id)
    .bind(content_hash)
    .bind(change_summary)
    .bind(source_context)
    .bind(analysis_mode)
    .bind(created_by)
    .fetch_one(&mut **tx)
    .await?;
    Ok(revision_id)
}

#[allow(clippy::too_many_arguments)]
async fn first_revision_for(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: Uuid,
    project_id: Uuid,
    object_id: Uuid,
    content_hash: &str,
    change_summary: Option<&str>,
    source_context: Option<&str>,
    analysis_mode: Option<&str>,
    created_by: Uuid,
    content_markdown: &str,
) -> Result<Uuid, ProjcError> {
    // identity 占位行尚无文档：建文档 + 首修订
    let (object_key, _object_type): (Option<String>, String) =
        sqlx::query_as("SELECT object_key, object_type FROM projc.cognition_object WHERE id = $1")
            .bind(object_id)
            .fetch_one(&mut **tx)
            .await?;
    let (doc_key, canonical_path) = doc_slug(object_key.as_deref(), object_id);
    let document_id: Uuid = sqlx::query_scalar(
        "INSERT INTO projc.document (tenant_id, project_id, doc_key, canonical_path) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(&doc_key)
    .bind(&canonical_path)
    .fetch_one(&mut **tx)
    .await?;
    let document_revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO projc.document_revision \
         (tenant_id, project_id, document_id, revision_no, content_hash, content, created_by) \
         VALUES ($1, $2, $3, 1, $4, $5, $6) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(document_id)
    .bind(content_hash)
    .bind(content_markdown)
    .bind(created_by)
    .fetch_one(&mut **tx)
    .await?;
    let revision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO projc.cognition_revision \
         (tenant_id, project_id, object_id, revision_no, document_id, document_revision_id, \
          content_hash, change_summary, source_context, analysis_mode, created_by) \
         VALUES ($1, $2, $3, 1, $4, $5, $6, $7, $8, $9, $10) RETURNING id",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(object_id)
    .bind(document_id)
    .bind(document_revision_id)
    .bind(content_hash)
    .bind(change_summary)
    .bind(source_context)
    .bind(analysis_mode)
    .bind(created_by)
    .fetch_one(&mut **tx)
    .await?;
    Ok(revision_id)
}
