//! Original Sources（KB 第一层）：团队级来源池登记、项目关联与只读查询（K5）。
//!
//! 规则（定义稿 §5.1 / §6；迁移 0064）：
//! - 原始资料**不可改写**：`kb.source` 行由触发器拒绝 UPDATE/DELETE；
//! - **统一身份**：同团队同 `content_hash` 只有一个来源（二次登记 = 追加项目关联，
//!   `deduplicated=true`）；
//! - **一源多项目**：`kb.source_project` 行存在 = 经人工确认的项目归属（V0.5 由项目
//!   成员登记即确认）；读取一律按项目关联过滤——未关联项目看不到该来源；
//! - 内联文本来源的 `content_hash` 由服务端计算（提交方声明不一致即拒）；uri 来源由
//!   提交方声明（拉取重算比对归 Phase 4 解析链）。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::{audit, check_len, ensure_project_writable, KbServiceError, TYPE_SOURCE};
use crate::actor::AuditActor;

/// 来源类别（与 0064 `kb.source.source_kind` CHECK 同集）。
pub const SOURCE_KINDS: &[&str] = &[
    "markdown",
    "docx",
    "pdf",
    "meeting_notes",
    "email",
    "im_log",
    "design",
    "recording",
    "other",
];

/// 内联文本来源上限（防超大行；文件类走 uri）。
pub const MAX_INLINE_BYTES: usize = 1024 * 1024;

/// 来源登记请求（REST `POST /v1/kb/sources` 的 body）。
#[derive(Debug, Clone, Deserialize)]
pub struct RegisterSourceRequest {
    pub source_kind: String,
    pub title: String,
    #[serde(default)]
    pub uri: Option<String>,
    #[serde(default)]
    pub inline_content: Option<String>,
    #[serde(default)]
    pub mime: Option<String>,
    #[serde(default)]
    pub size_bytes: Option<i64>,
    /// uri 来源必填；内联来源可省（服务端计算，提供则必须一致）。
    #[serde(default)]
    pub content_hash: Option<String>,
}

/// 登记回执。
#[derive(Debug, Clone, Serialize)]
pub struct RegisteredSource {
    pub source_id: Uuid,
    pub project_id: Uuid,
    pub content_hash: String,
    /// 同内容已在团队池：复用既有身份，只追加 / 确认项目关联。
    pub deduplicated: bool,
    /// 本次新建了项目关联（false = 该项目早已关联，幂等重放零副作用）。
    pub newly_linked: bool,
}

/// 项目视角的来源条目（列表与详情共用；`inline_content` 只在详情返回）。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct SourceView {
    pub source_id: Uuid,
    pub source_kind: String,
    pub title: String,
    pub uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_content: Option<String>,
    pub has_inline_content: bool,
    pub mime: Option<String>,
    pub size_bytes: Option<i64>,
    pub content_hash: String,
    pub registered_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub linked_by: Uuid,
    pub linked_at: DateTime<Utc>,
}

fn is_hex_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// 规范化并校验登记请求，返回 (uri, inline, size_bytes, content_hash)。
fn normalize(
    req: &RegisterSourceRequest,
) -> Result<(Option<String>, Option<String>, Option<i64>, String), KbServiceError> {
    if !SOURCE_KINDS.contains(&req.source_kind.as_str()) {
        return Err(KbServiceError::Invalid(format!(
            "source_kind must be one of {SOURCE_KINDS:?}"
        )));
    }
    check_len("title", &req.title, 1, 200)?;
    if let Some(size) = req.size_bytes {
        if size < 0 {
            return Err(KbServiceError::Invalid("size_bytes must be >= 0".into()));
        }
    }
    let uri = req
        .uri
        .as_deref()
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .map(str::to_string);
    let inline = req
        .inline_content
        .as_deref()
        .filter(|c| !c.is_empty())
        .map(str::to_string);
    match (uri, inline) {
        (Some(_), Some(_)) | (None, None) => Err(KbServiceError::Invalid(
            "exactly one of uri / inline_content is required".into(),
        )),
        (None, Some(content)) => {
            if content.len() > MAX_INLINE_BYTES {
                return Err(KbServiceError::Invalid(format!(
                    "inline_content exceeds {MAX_INLINE_BYTES} bytes; register large files by uri"
                )));
            }
            let computed = crate::knowledge::content_hash(&content);
            if let Some(declared) = req.content_hash.as_deref() {
                if declared != computed {
                    return Err(KbServiceError::Invalid(
                        "content_hash does not match inline_content (sha256, lowercase hex)".into(),
                    ));
                }
            }
            let size = content.len() as i64;
            Ok((None, Some(content), Some(size), computed))
        }
        (Some(uri), None) => {
            let declared = req.content_hash.as_deref().unwrap_or_default();
            if !is_hex_sha256(declared) {
                return Err(KbServiceError::Invalid(
                    "content_hash (sha256, 64 lowercase hex) is required for uri sources".into(),
                ));
            }
            Ok((Some(uri), None, req.size_bytes, declared.to_string()))
        }
    }
}

/// 来源登记（团队池去重 + 项目关联；同事务审计）。
///
/// 幂等：同内容再次登记到同项目 = 零副作用重放（不写审计）；登记到新项目 = 只追加
/// 关联（审计 `kb.source.linked`）；首次登记审计 `kb.source.registered`。
pub async fn register_source(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    actor: &AuditActor,
    req: RegisterSourceRequest,
) -> Result<RegisteredSource, KbServiceError> {
    let (uri, inline, size_bytes, content_hash) = normalize(&req)?;
    let mut tx = pool.begin().await?;
    ensure_project_writable(&mut tx, project_id).await?;

    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO kb.source \
         (tenant_id, source_kind, title, uri, inline_content, mime, size_bytes, content_hash, registered_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
         ON CONFLICT (tenant_id, content_hash) DO NOTHING \
         RETURNING id",
    )
    .bind(tenant_id)
    .bind(&req.source_kind)
    .bind(req.title.trim())
    .bind(&uri)
    .bind(&inline)
    .bind(&req.mime)
    .bind(size_bytes)
    .bind(&content_hash)
    .bind(actor.principal_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (source_id, deduplicated) = match inserted {
        Some(id) => (id, false),
        None => {
            let id: Uuid = sqlx::query_scalar(
                "SELECT id FROM kb.source WHERE tenant_id = $1 AND content_hash = $2",
            )
            .bind(tenant_id)
            .bind(&content_hash)
            .fetch_one(&mut *tx)
            .await?;
            (id, true)
        }
    };

    let linked: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO kb.source_project (tenant_id, source_id, project_id, linked_by) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (source_id, project_id) DO NOTHING \
         RETURNING source_id",
    )
    .bind(tenant_id)
    .bind(source_id)
    .bind(project_id)
    .bind(actor.principal_id)
    .fetch_optional(&mut *tx)
    .await?;
    let newly_linked = linked.is_some();

    let action = if !deduplicated {
        Some("kb.source.registered")
    } else if newly_linked {
        Some("kb.source.linked")
    } else {
        None
    };
    if let Some(action) = action {
        audit(
            &mut tx,
            tenant_id,
            project_id,
            actor,
            action,
            "kb_source",
            source_id,
            serde_json::json!({
                "source_kind": req.source_kind,
                "title": req.title.trim(),
                "content_hash": content_hash,
                "locator": if uri.is_some() { "uri" } else { "inline" },
                "deduplicated": deduplicated,
            }),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(RegisteredSource {
        source_id,
        project_id,
        content_hash,
        deduplicated,
        newly_linked,
    })
}

const VIEW_COLS: &str = "s.id AS source_id, s.source_kind, s.title, s.uri, \
     s.inline_content IS NOT NULL AS has_inline_content, s.mime, s.size_bytes, s.content_hash, \
     s.registered_by, s.created_at, sp.linked_by, sp.linked_at";

/// 项目关联的来源列表（新关联在前）。列表不带内联正文。
pub async fn list_project_sources(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Vec<SourceView>, KbServiceError> {
    let sql = format!(
        "SELECT {VIEW_COLS}, NULL::text AS inline_content \
           FROM kb.source_project sp JOIN kb.source s ON s.id = sp.source_id \
          WHERE sp.project_id = $1 \
          ORDER BY sp.linked_at DESC, s.id"
    );
    Ok(sqlx::query_as(&sql)
        .bind(project_id)
        .fetch_all(pool)
        .await?)
}

/// 被某条记忆引用为 evidence 的来源（K5 `evidence_of`，差异说明 D-KP3-4）。
///
/// `memory` → 该记忆 CURRENT 版本的 evidence；`memory_revision` → 指定修订的
/// evidence。记忆/修订不属于该项目时结果为空（不泄露存在性）。只返回同时关联到该
/// 项目的来源。
pub async fn sources_cited_by(
    pool: &PgPool,
    project_id: Uuid,
    target_type: &str,
    target_id: Uuid,
) -> Result<Vec<SourceView>, KbServiceError> {
    let revision_filter = match target_type {
        super::TYPE_MEMORY => "r.memory_id = $2 AND r.version_status = 'CURRENT'",
        super::TYPE_MEMORY_REVISION => "r.id = $2",
        other => {
            return Err(KbServiceError::Invalid(format!(
                "evidence_of type `{other}` is not supported (expected memory | memory_revision)"
            )))
        }
    };
    let sql = format!(
        "SELECT {VIEW_COLS}, NULL::text AS inline_content \
           FROM kb.memory_ref mr \
           JOIN kb.memory_revision r ON r.id = mr.revision_id \
           JOIN kb.memory m ON m.id = r.memory_id \
           JOIN kb.source s ON s.id::text = mr.target_id \
           JOIN kb.source_project sp ON sp.source_id = s.id AND sp.project_id = $1 \
          WHERE m.project_id = $1 AND {revision_filter} \
            AND mr.ref_role = 'evidence' AND mr.target_domain = 'knowledge_base' \
            AND mr.target_type = '{TYPE_SOURCE}' \
          ORDER BY sp.linked_at DESC, s.id"
    );
    Ok(sqlx::query_as(&sql)
        .bind(project_id)
        .bind(target_id)
        .fetch_all(pool)
        .await?)
}

/// 来源详情（含内联正文）；未关联到该项目 → NotFound（不泄露团队池存在性）。
pub async fn get_source(
    pool: &PgPool,
    project_id: Uuid,
    source_id: Uuid,
) -> Result<SourceView, KbServiceError> {
    let sql = format!(
        "SELECT {VIEW_COLS}, s.inline_content \
           FROM kb.source_project sp JOIN kb.source s ON s.id = sp.source_id \
          WHERE sp.project_id = $1 AND s.id = $2"
    );
    sqlx::query_as(&sql)
        .bind(project_id)
        .bind(source_id)
        .fetch_optional(pool)
        .await?
        .ok_or(KbServiceError::NotFound("source not found"))
}

/// 项目内来源存在性（记忆 evidence 引用校验用；KB 域自校验）。
pub(crate) async fn source_linked_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
    source_id: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM kb.source_project WHERE project_id = $1 AND source_id = $2)",
    )
    .bind(project_id)
    .bind(source_id)
    .fetch_one(&mut **tx)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(uri: Option<&str>, inline: Option<&str>, hash: Option<&str>) -> RegisterSourceRequest {
        RegisterSourceRequest {
            source_kind: "meeting_notes".into(),
            title: "周会纪要".into(),
            uri: uri.map(str::to_string),
            inline_content: inline.map(str::to_string),
            mime: None,
            size_bytes: None,
            content_hash: hash.map(str::to_string),
        }
    }

    #[test]
    fn inline_hash_is_server_computed() {
        let (_, inline, size, hash) = normalize(&req(None, Some("hello"), None)).unwrap();
        assert_eq!(inline.as_deref(), Some("hello"));
        assert_eq!(size, Some(5));
        assert_eq!(hash, crate::knowledge::content_hash("hello"));
    }

    #[test]
    fn inline_hash_mismatch_is_rejected() {
        let wrong = "0".repeat(64);
        assert!(normalize(&req(None, Some("hello"), Some(&wrong))).is_err());
    }

    #[test]
    fn exactly_one_locator() {
        assert!(normalize(&req(None, None, None)).is_err());
        let h = "a".repeat(64);
        assert!(normalize(&req(Some("s3://b/k"), Some("x"), Some(&h))).is_err());
    }

    #[test]
    fn uri_requires_declared_hash() {
        assert!(normalize(&req(Some("s3://b/k"), None, None)).is_err());
        assert!(normalize(&req(Some("s3://b/k"), None, Some("XYZ"))).is_err());
        let h = "b".repeat(64);
        let (uri, inline, _, hash) = normalize(&req(Some(" s3://b/k "), None, Some(&h))).unwrap();
        assert_eq!(uri.as_deref(), Some("s3://b/k"));
        assert!(inline.is_none());
        assert_eq!(hash, h);
    }

    #[test]
    fn unknown_kind_is_rejected() {
        let mut r = req(None, Some("x"), None);
        r.source_kind = "tweet".into();
        assert!(normalize(&r).is_err());
    }
}
