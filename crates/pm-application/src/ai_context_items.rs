//! 会话级对象上下文条目（AI Collaboration V1.0 · AI-02 / 后端契约 CX1·CX3 · B2）。
//!
//! **职责**：会话可用的对象级工作上下文（Requirement / Design / ADR / Knowledge /
//! Document / Task）的附加、启停、移除与查询。
//!
//! ## 与线程锚点的关系（CX1 裁定）
//!
//! `ai.conversation.context_anchor`（0003）负责 **Project / Repository 线程作用域
//! 定位**；本表（0019）负责**对象级**上下文——独立生命周期、启停、版本引用与
//! 审计。两者不混同一存储。
//!
//! ## 硬规则
//!
//! 1. **只存定位与版本引用，不存正文**——正文与状态一律服务端重读（SQL-07 在
//!    注入层复核版本与 content_hash）；
//! 2. **双轴分列**：`attachment`（attached/detached）与 `lifecycle`
//!    （active/inactive/pending/unavailable）独立流转；`read_only` 是可写性
//!    属性，不随状态流转切换；**Available 是候选查询结果，不入库**；
//! 3. **PATCH 走状态机白名单**（评审终裁），不接受客户端任意 lifecycle 字符串；
//! 4. **软移除**：DELETE = 移出当前会话（detached），不删业务对象、不丢版本引用；
//! 5. **属主隔离**：条目随会话归属（conversation.principal_id）；跨属主/跨项目
//!    访问一律 404 同形（SQL-06，服务层 join 校验）；
//! 6. **Pending → Active 必须真人**（DR-C2）：AI 主体不得把待确认条目转为可用；
//! 7. **审计**（SQL-08）：附加/启停/移除/状态异常逐笔落 `governance.audit_log`
//!    （四身份列全写，C174/C180 口径）。

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::actor::AuditActor;
use crate::pm_write::append_audit_with_after;

/// 可附加的对象类型（SQL-01 分类型版本引用矩阵的键）。
pub const OBJECT_TYPES: [&str; 6] = [
    "requirement",
    "design",
    "adr",
    "knowledge",
    "document",
    "task",
];

/// 归属轴（存储态只有两态；Available 是候选查询结果，不入库）。
pub const ATTACHMENT_ATTACHED: &str = "attached";
pub const ATTACHMENT_DETACHED: &str = "detached";

/// 生命周期轴。
pub const LIFECYCLE_ACTIVE: &str = "active";
pub const LIFECYCLE_INACTIVE: &str = "inactive";
pub const LIFECYCLE_PENDING: &str = "pending";
pub const LIFECYCLE_UNAVAILABLE: &str = "unavailable";

/// 附加来源（0019 source CHECK 词表）。
pub const SOURCES: [&str; 4] = ["page", "manual", "discovery", "ai_suggestion"];

/// 字段长度上限（与锚点 255 同量级）。
pub const MAX_OBJECT_CODE_CHARS: usize = 255;
pub const MAX_UNAVAILABLE_REASON_CHARS: usize = 500;

/// 单次批量附加上限（Add Context 多选的合理上界）。
pub const MAX_BATCH_ITEMS: usize = 20;

/// 条目视图（REST 直接序列化）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ContextItemView {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub object_type: String,
    pub object_id: String,
    pub object_code: Option<String>,
    /// 版本引用（按类型解释；task 只有 `row_version`，见 SQL-01）。
    pub revision_id: Option<Uuid>,
    pub version: Option<i32>,
    pub content_hash: Option<String>,
    pub row_version: Option<i64>,
    /// 归属轴：attached / detached。
    pub attachment: String,
    /// 生命周期轴：active / inactive / pending / unavailable。
    pub lifecycle: String,
    /// 可写性属性（独立于 lifecycle；不随状态流转切换）。
    pub read_only: bool,
    pub source: String,
    pub unavailable_reason: Option<String>,
    pub added_by: Uuid,
    pub added_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 附加请求项（批量 POST 的单条）。
#[derive(Debug, Clone)]
pub struct AttachItemRequest {
    pub object_type: String,
    pub object_id: Uuid,
    pub object_code: Option<String>,
    /// 版本引用（按类型校验：requirement/design/adr → 修订 id；document →
    /// document_version.id；knowledge/task → 必须为 None）。
    pub revision_id: Option<Uuid>,
    /// 版本号（写入时与修订实际版本复核；不一致 → Validation）。
    pub version: Option<i32>,
    /// 内容哈希（写入时与修订实际哈希复核；不一致 → Validation）。
    pub content_hash: Option<String>,
    /// 乐观锁行版本快照（仅 task 语义）。
    pub row_version: Option<i64>,
    /// 初始生命周期：active / pending（pending = 等待真人确认）。
    pub initial_lifecycle: Option<String>,
    pub read_only: bool,
    pub source: String,
}

/// 生命周期流转请求（PATCH）。
#[derive(Debug, Clone)]
pub struct PatchLifecycleRequest {
    /// 目标生命周期（必须能从当前状态按白名单到达）。
    pub lifecycle: String,
    /// 进入 unavailable 必填；离开 unavailable 时服务端清空。
    pub unavailable_reason: Option<String>,
}

/// Context 条目域失败。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ContextItemError {
    /// 会话/条目不存在或跨属主/跨项目（**上层映射 404**，不泄露存在性）。
    #[error("context item or conversation not found")]
    NotFound,
    /// 校验失败（400）。
    #[error("{0}")]
    Validation(String),
    /// 有身份但无权限（403；如 Pending → Active 非真人）。
    #[error("{0}")]
    Forbidden(String),
    /// 状态机不允许的转换 / 重复附加（409）。
    #[error("{0}")]
    Conflict(String),
    /// 数据库错误。
    #[error("database error: {0}")]
    Db(String),
}

impl ContextItemError {
    pub(crate) fn db(e: sqlx::Error) -> Self {
        Self::Db(e.to_string())
    }
}

// ───────────────────────────────────────────────── 归属校验（SQL-06）

/// 校验会话存在且属于该 (tenant, project, principal)；返回会话行。
/// 跨属主/跨项目 → NotFound（404 同形，不泄露存在性）。
async fn ensure_conversation_owned(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
    conversation_id: Uuid,
) -> Result<(), ContextItemError> {
    let exists: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM ai.conversation \
         WHERE id = $1 AND tenant_id = $2 AND project_id = $3 AND principal_id = $4",
    )
    .bind(conversation_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(principal_id)
    .fetch_optional(pool)
    .await
    .map_err(ContextItemError::db)?;
    exists.ok_or(ContextItemError::NotFound).map(|_| ())
}

/// 校验条目属于该会话（条目 → 会话 → 属主一条链），返回条目视图。
async fn fetch_item_owned(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
    conversation_id: Uuid,
    item_id: Uuid,
) -> Result<ContextItemView, ContextItemError> {
    let row = sqlx::query(
        "SELECT i.id, i.conversation_id, i.object_type, i.object_id, i.object_code, \
                i.revision_id, i.version, i.content_hash, i.row_version, \
                i.attachment, i.lifecycle, i.read_only, i.source, \
                i.unavailable_reason, i.added_by, i.added_at, i.updated_at \
           FROM ai.conversation_context_item i \
           JOIN ai.conversation c ON c.id = i.conversation_id \
          WHERE i.id = $1 AND i.conversation_id = $2 \
            AND i.tenant_id = $3 AND i.project_id = $4 \
            AND c.tenant_id = $3 AND c.project_id = $4 AND c.principal_id = $5",
    )
    .bind(item_id)
    .bind(conversation_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(principal_id)
    .fetch_optional(pool)
    .await
    .map_err(ContextItemError::db)?
    .ok_or(ContextItemError::NotFound)?;
    Ok(item_from_row(&row))
}

fn item_from_row(row: &sqlx::postgres::PgRow) -> ContextItemView {
    use sqlx::Row;
    ContextItemView {
        id: row.get("id"),
        conversation_id: row.get("conversation_id"),
        object_type: row.get("object_type"),
        object_id: row.get("object_id"),
        object_code: row.get("object_code"),
        revision_id: row.get("revision_id"),
        version: row.get("version"),
        content_hash: row.get("content_hash"),
        row_version: row.get("row_version"),
        attachment: row.get("attachment"),
        lifecycle: row.get("lifecycle"),
        read_only: row.get("read_only"),
        source: row.get("source"),
        unavailable_reason: row.get("unavailable_reason"),
        added_by: row.get("added_by"),
        added_at: row.get("added_at"),
        updated_at: row.get("updated_at"),
    }
}

// ───────────────────────────────────── 对象存在性与版本校验（SQL-01 / SQL-05）

/// 校验对象存在且归属 (tenant, project)，并按**分类型矩阵**校验版本引用。
///
/// 返回 (object_code, 任务行版本快照)。**不信任客户端传入的权限/版本快照**：
/// 版本与哈希一律与库中修订实值复核（写入即复核；注入层再复核一次，SQL-07）。
///
/// 分类型矩阵（契约 §SQL-01）：
/// | 类型 | revision_id | version | content_hash | row_version |
/// |---|---|---|---|---|
/// | requirement | requirement_revision.id | 复核 | 复核 | ✗ |
/// | design | design_revision.id | 复核 | 复核 | ✗ |
/// | adr | adr_revision.id | 复核 | 复核 | ✗ |
/// | knowledge | ✗（对象行即版本） | 复核（对象行 version） | ✗ | ✗ |
/// | document | document_version.id | 复核 version_number | 复核 checksum | ✗ |
/// | task | ✗（无内容版本） | ✗ | ✗ | 快照 task.revision |
#[allow(clippy::type_complexity)]
async fn validate_object_reference(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    req: &AttachItemRequest,
) -> Result<(Option<String>, Option<i64>), ContextItemError> {
    match req.object_type.as_str() {
        "requirement" => {
            let code: Option<String> = sqlx::query_scalar(
                "SELECT code FROM delivery.requirement \
                 WHERE tenant_id = $1 AND project_id = $2 AND id = $3",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(req.object_id)
            .fetch_optional(pool)
            .await
            .map_err(ContextItemError::db)?;
            let Some(code) = code else {
                return Err(ContextItemError::Validation(
                    "object does not exist in the target project".to_string(),
                ));
            };
            if let Some(revision_id) = req.revision_id {
                verify_revision(
                    pool,
                    tenant_id,
                    project_id,
                    "delivery.requirement_revision",
                    "requirement_id",
                    req.object_id,
                    revision_id,
                    req.version,
                    req.content_hash.as_deref(),
                )
                .await?;
            } else if req.version.is_some() || req.content_hash.is_some() {
                return Err(ContextItemError::Validation(
                    "version/content_hash require revision_id".to_string(),
                ));
            }
            Ok((Some(code), None))
        }
        "design" => {
            let code: Option<String> = sqlx::query_scalar(
                "SELECT code FROM design.design \
                 WHERE tenant_id = $1 AND project_id = $2 AND id = $3",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(req.object_id)
            .fetch_optional(pool)
            .await
            .map_err(ContextItemError::db)?;
            let Some(code) = code else {
                return Err(ContextItemError::Validation(
                    "object does not exist in the target project".to_string(),
                ));
            };
            if let Some(revision_id) = req.revision_id {
                verify_revision(
                    pool,
                    tenant_id,
                    project_id,
                    "design.design_revision",
                    "design_id",
                    req.object_id,
                    revision_id,
                    req.version,
                    req.content_hash.as_deref(),
                )
                .await?;
            } else if req.version.is_some() || req.content_hash.is_some() {
                return Err(ContextItemError::Validation(
                    "version/content_hash require revision_id".to_string(),
                ));
            }
            Ok((Some(code), None))
        }
        "adr" => {
            let code: Option<String> = sqlx::query_scalar(
                "SELECT code FROM design.adr \
                 WHERE tenant_id = $1 AND project_id = $2 AND id = $3",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(req.object_id)
            .fetch_optional(pool)
            .await
            .map_err(ContextItemError::db)?;
            let Some(code) = code else {
                return Err(ContextItemError::Validation(
                    "object does not exist in the target project".to_string(),
                ));
            };
            if let Some(revision_id) = req.revision_id {
                verify_revision(
                    pool,
                    tenant_id,
                    project_id,
                    "design.adr_revision",
                    "adr_id",
                    req.object_id,
                    revision_id,
                    req.version,
                    req.content_hash.as_deref(),
                )
                .await?;
            } else if req.version.is_some() || req.content_hash.is_some() {
                return Err(ContextItemError::Validation(
                    "version/content_hash require revision_id".to_string(),
                ));
            }
            Ok((Some(code), None))
        }
        "knowledge" => {
            // 对象行即版本（supersedes 链）；revision_id 必须为空。
            if req.revision_id.is_some() {
                return Err(ContextItemError::Validation(
                    "knowledge items carry their own version; revision_id must be null"
                        .to_string(),
                ));
            }
            if req.content_hash.is_some() {
                return Err(ContextItemError::Validation(
                    "knowledge has no content_hash column; content_hash must be null"
                        .to_string(),
                ));
            }
            let row: Option<(String, i32)> = sqlx::query_as(
                "SELECT code_version.domain, code_version.version FROM (\
                    SELECT domain, version FROM ai.project_knowledge \
                     WHERE tenant_id = $1 AND project_id = $2 AND id = $3\
                 ) AS code_version",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(req.object_id)
            .fetch_optional(pool)
            .await
            .map_err(ContextItemError::db)?;
            let Some((domain, version)) = row else {
                return Err(ContextItemError::Validation(
                    "object does not exist in the target project".to_string(),
                ));
            };
            if let Some(given) = req.version {
                if given != version {
                    return Err(ContextItemError::Validation(format!(
                        "knowledge version mismatch: item {given} != actual {version}"
                    )));
                }
            }
            Ok((Some(domain), None))
        }
        "document" => {
            if req.content_hash.is_some() {
                return Err(ContextItemError::Validation(
                    "document hash column is named checksum; pass it via revision verification"
                        .to_string(),
                ));
            }
            // 对象必须归属本项目（document.project_id 可空 → NULL 不算匹配）。
            let exists: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM docproc.document \
                 WHERE tenant_id = $1 AND project_id = $2 AND id = $3",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(req.object_id)
            .fetch_optional(pool)
            .await
            .map_err(ContextItemError::db)?;
            if exists.is_none() {
                return Err(ContextItemError::Validation(
                    "object does not exist in the target project".to_string(),
                ));
            }
            if let Some(revision_id) = req.revision_id {
                // document_version：version_number + checksum。
                let row: Option<(i32, String)> = sqlx::query_as(
                    "SELECT version_number, checksum FROM docproc.document_version \
                     WHERE tenant_id = $1 AND id = $2 AND document_id = $3",
                )
                .bind(tenant_id)
                .bind(revision_id)
                .bind(req.object_id)
                .fetch_optional(pool)
                .await
                .map_err(ContextItemError::db)?;
                let Some((version_number, checksum)) = row else {
                    return Err(ContextItemError::Validation(
                        "revision does not belong to the referenced document".to_string(),
                    ));
                };
                if let Some(given) = req.version {
                    if given != version_number {
                        return Err(ContextItemError::Validation(format!(
                            "document version mismatch: item {given} != actual {version_number}"
                        )));
                    }
                }
                // content_hash 在本表承载 checksum 语义：写回时落实值。
                if let Some(given) = req.content_hash.as_deref() {
                    if given != checksum {
                        return Err(ContextItemError::Validation(
                            "content_hash mismatch with the referenced document version"
                                .to_string(),
                        ));
                    }
                }
            }
            Ok((None, None))
        }
        "task" => {
            // 任务只有乐观锁行版本（revision bigint）——**不得伪造内容版本**。
            if req.revision_id.is_some() || req.version.is_some() || req.content_hash.is_some() {
                return Err(ContextItemError::Validation(
                    "tasks have no content revision; revision_id/version/content_hash \
                     must be null (row_version only)"
                        .to_string(),
                ));
            }
            let row: Option<(String, i64)> = sqlx::query_as(
                "SELECT title, revision FROM delivery.task \
                 WHERE tenant_id = $1 AND project_id = $2 AND id = $3",
            )
            .bind(tenant_id)
            .bind(project_id)
            .bind(req.object_id)
            .fetch_optional(pool)
            .await
            .map_err(ContextItemError::db)?;
            let Some((title, revision)) = row else {
                return Err(ContextItemError::Validation(
                    "object does not exist in the target project".to_string(),
                ));
            };
            if let Some(given) = req.row_version {
                if given != revision {
                    return Err(ContextItemError::Validation(format!(
                        "task row_version mismatch: item {given} != actual {revision}"
                    )));
                }
            }
            Ok((Some(title), Some(revision)))
        }
        other => Err(ContextItemError::Validation(format!(
            "unknown object_type {other:?}"
        ))),
    }
}

/// 修订引用复核（requirement / design / adr 共用：同构的 `_revision` 表）。
#[allow(clippy::too_many_arguments)]
async fn verify_revision(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    table: &str,
    object_column: &str,
    object_id: Uuid,
    revision_id: Uuid,
    version: Option<i32>,
    content_hash: Option<&str>,
) -> Result<(), ContextItemError> {
    // 表名 / 列名来自编译期常量（非用户输入），无注入面。
    let sql = format!(
        "SELECT version, content_hash FROM {table} \
         WHERE tenant_id = $1 AND project_id = $2 AND {object_column} = $3 AND id = $4"
    );
    let row: Option<(i32, String)> = sqlx::query_as(&sql)
        .bind(tenant_id)
        .bind(project_id)
        .bind(object_id)
        .bind(revision_id)
        .fetch_optional(pool)
        .await
        .map_err(ContextItemError::db)?;
    let Some((actual_version, actual_hash)) = row else {
        return Err(ContextItemError::Validation(
            "revision does not belong to the referenced object".to_string(),
        ));
    };
    if let Some(given) = version {
        if given != actual_version {
            return Err(ContextItemError::Validation(format!(
                "revision version mismatch: item {given} != actual {actual_version}"
            )));
        }
    }
    if let Some(given) = content_hash {
        if given != actual_hash {
            return Err(ContextItemError::Validation(
                "content_hash mismatch with the referenced revision".to_string(),
            ));
        }
    }
    Ok(())
}

// ───────────────────────────────────────────────────────── 审计（SQL-08）

/// 条目域审计（governance.audit_log；四身份列全写——C174/C180 口径）。
async fn audit(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    project_id: Uuid,
    actor: &AuditActor,
    action: &str,
    item_id: Uuid,
    after: serde_json::Value,
) -> Result<(), ContextItemError> {
    append_audit_with_after(
        tx,
        project_id,
        actor,
        action,
        "conversation_context_item",
        Some(item_id),
        after,
    )
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db_err) => {
            ContextItemError::Db(db_err.message().to_string())
        }
        err => ContextItemError::Db(err.to_string()),
    })
}

// ─────────────────────────────────────────────────────────── 附加（POST）

/// 批量附加对象（Add Context 多选；逐项校验）。
///
/// - `initial_lifecycle`：缺省 `active`；`pending` = 等待真人确认
///   （**Pending 不得被自动当作 Active**）；
/// - 不改变其它已有条目的状态（评审：Add 不得连带改状态）；
/// - 同会话同对象（含 revision）重复附加 → Conflict。
pub async fn attach_items(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
    conversation_id: Uuid,
    actor: &AuditActor,
    items: &[AttachItemRequest],
) -> Result<Vec<ContextItemView>, ContextItemError> {
    if items.is_empty() {
        return Err(ContextItemError::Validation(
            "at least one context item is required".to_string(),
        ));
    }
    if items.len() > MAX_BATCH_ITEMS {
        return Err(ContextItemError::Validation(format!(
            "at most {MAX_BATCH_ITEMS} items per batch"
        )));
    }
    ensure_conversation_owned(
        pool,
        tenant_id,
        project_id,
        principal_id,
        conversation_id,
    )
    .await?;

    let mut tx = pool.begin().await.map_err(ContextItemError::db)?;
    let mut attached = Vec::with_capacity(items.len());
    for req in items {
        validate_item_request(req)?;
        let (object_code, task_row_version) =
            validate_object_reference(&pool, tenant_id, project_id, req).await?;

        let lifecycle = req.initial_lifecycle.as_deref().unwrap_or(LIFECYCLE_ACTIVE);
        if lifecycle != LIFECYCLE_ACTIVE && lifecycle != LIFECYCLE_PENDING {
            return Err(ContextItemError::Validation(
                "initial_lifecycle must be active or pending".to_string(),
            ));
        }

        let inserted = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO ai.conversation_context_item \
             (tenant_id, project_id, conversation_id, object_type, object_id, object_code, \
              revision_id, version, content_hash, row_version, \
              attachment, lifecycle, read_only, source, added_by) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, \
                     'attached', $11, $12, $13, $14) \
             RETURNING id",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(conversation_id)
        .bind(&req.object_type)
        .bind(req.object_id)
        .bind(&object_code)
        .bind(req.revision_id)
        .bind(req.version)
        .bind(req.content_hash.clone())
        .bind(task_row_version)
        .bind(lifecycle)
        .bind(req.read_only)
        .bind(&req.source)
        .bind(actor.principal_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(ref db_err)
                if db_err.code().as_deref() == Some("23505") =>
            {
                ContextItemError::Conflict(
                    "the object is already attached to this conversation".to_string(),
                )
            }
            _ => ContextItemError::db(e),
        })?;

        audit(
            &mut tx,
            project_id,
            actor,
            "context_item.attach",
            inserted,
            serde_json::json!({
                "conversation_id": conversation_id,
                "object_type": req.object_type,
                "object_id": req.object_id,
                "revision_id": req.revision_id,
                "lifecycle": lifecycle,
                "source": req.source,
            }),
        )
        .await?;

        attached.push((inserted, req));
    }
    tx.commit().await.map_err(ContextItemError::db)?;

    // 逐条回读（同事务已提交；视图与库一致）。
    let mut views = Vec::with_capacity(attached.len());
    for (id, _) in attached {
        let item = fetch_item_owned(
            pool,
            tenant_id,
            project_id,
            principal_id,
            conversation_id,
            id,
        )
        .await?;
        views.push(item);
    }
    Ok(views)
}

/// 请求级校验（词表 / 长度 / 十六进制哈希形状）。
fn validate_item_request(req: &AttachItemRequest) -> Result<(), ContextItemError> {
    if !OBJECT_TYPES.contains(&req.object_type.as_str()) {
        return Err(ContextItemError::Validation(format!(
            "unknown object_type {:?}",
            req.object_type
        )));
    }
    if !SOURCES.contains(&req.source.as_str()) {
        return Err(ContextItemError::Validation(format!(
            "unknown source {:?}",
            req.source
        )));
    }
    if let Some(code) = req.object_code.as_deref() {
        if code.len() > MAX_OBJECT_CODE_CHARS {
            return Err(ContextItemError::Validation(
                "object_code too long".to_string(),
            ));
        }
    }
    Ok(())
}

// ────────────────────────────────────────────────────── 生命周期（PATCH）

/// 生命周期流转（**状态机白名单**，评审终裁）：
///
/// | 转换 | 条件 |
/// |---|---|
/// | active → inactive | 授权用户 Disable |
/// | inactive → active | **重新校验**对象存在性与版本有效性 |
/// | pending → active | **必须真人主体**（DR-C2；AI 不得把待确认转为可用） |
/// | 任意 → unavailable | 必须给原因（服务端落库；离开时清空） |
/// | unavailable → active | **重新校验**对象可用性与版本 |
/// | detached → 任意 | **不允许**（须重新附加并重新校验，Conflict） |
///
/// `read_only` 不在本 PATCH 的职责内（可写性属性，按来源对象权限另行计算）。
pub async fn patch_lifecycle(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
    conversation_id: Uuid,
    item_id: Uuid,
    actor: &AuditActor,
    req: &PatchLifecycleRequest,
) -> Result<ContextItemView, ContextItemError> {
    ensure_conversation_owned(
        pool,
        tenant_id,
        project_id,
        principal_id,
        conversation_id,
    )
    .await?;
    let item = fetch_item_owned(
        pool,
        tenant_id,
        project_id,
        principal_id,
        conversation_id,
        item_id,
    )
    .await?;

    if item.attachment != ATTACHMENT_ATTACHED {
        return Err(ContextItemError::Conflict(
            "detached items cannot change lifecycle; re-attach instead".to_string(),
        ));
    }

    let from = item.lifecycle.as_str();
    let to = req.lifecycle.as_str();

    // Pending → Active 必须真人（fail-closed：kind 查不到也拒绝）。
    if from == LIFECYCLE_PENDING && to == LIFECYCLE_ACTIVE {
        crate::requirement::require_human(pool, actor)
            .await
            .map_err(|e| match e {
                crate::pm_write::PmWriteError::Forbidden(_)
                | crate::pm_write::PmWriteError::NotFound(_) => ContextItemError::Forbidden(
                    "confirming a pending context item requires a human principal".to_string(),
                ),
                crate::pm_write::PmWriteError::Db(err) => ContextItemError::Db(err.to_string()),
                crate::pm_write::PmWriteError::Conflict(msg)
                | crate::pm_write::PmWriteError::Validation(msg) => {
                    ContextItemError::Validation(msg.to_string())
                }
                crate::pm_write::PmWriteError::ValidationMsg(msg) => {
                    ContextItemError::Validation(msg)
                }
            })?;
    }

    let allowed = matches!(
        (from, to),
        (LIFECYCLE_ACTIVE, LIFECYCLE_INACTIVE)
            | (LIFECYCLE_INACTIVE, LIFECYCLE_ACTIVE)
            | (LIFECYCLE_PENDING, LIFECYCLE_ACTIVE)
            | (LIFECYCLE_PENDING, LIFECYCLE_UNAVAILABLE)
            | (LIFECYCLE_ACTIVE, LIFECYCLE_UNAVAILABLE)
            | (LIFECYCLE_INACTIVE, LIFECYCLE_UNAVAILABLE)
            | (LIFECYCLE_UNAVAILABLE, LIFECYCLE_ACTIVE)
    );
    if !allowed {
        return Err(ContextItemError::Conflict(format!(
            "lifecycle transition {from} -> {to} is not allowed"
        )));
    }

    // 进入/离开 unavailable 的原因约束（SQL-03：原因与状态同生同灭）。
    let reason = match to {
        LIFECYCLE_UNAVAILABLE => {
            let reason = req
                .unavailable_reason
                .as_deref()
                .map(str::trim)
                .filter(|r| !r.is_empty())
                .ok_or_else(|| {
                    ContextItemError::Validation(
                        "unavailable_reason is required when entering unavailable".to_string(),
                    )
                })?;
            if reason.len() > MAX_UNAVAILABLE_REASON_CHARS {
                return Err(ContextItemError::Validation(
                    "unavailable_reason too long".to_string(),
                ));
            }
            Some(reason.to_string())
        }
        _ => None, // 离开 unavailable → 清空原因（不得留误导性当前原因）
    };

    // inactive/unavailable → active：**重新校验**对象存在性与版本有效性
    //（对象可能在条目存活期间被删除 / 修订被取代）。
    if to == LIFECYCLE_ACTIVE {
        let recheck = AttachItemRequest {
            object_type: item.object_type.clone(),
            object_id: item
                .object_id
                .parse()
                .map_err(|_| ContextItemError::NotFound)?,
            object_code: item.object_code.clone(),
            revision_id: item.revision_id,
            version: item.version,
            content_hash: item.content_hash.clone(),
            row_version: item.row_version,
            initial_lifecycle: None,
            read_only: item.read_only,
            source: item.source.clone(),
        };
        validate_object_reference(pool, tenant_id, project_id, &recheck).await?;
    }

    let mut tx = pool.begin().await.map_err(ContextItemError::db)?;
    sqlx::query(
        "UPDATE ai.conversation_context_item \
         SET lifecycle = $1, unavailable_reason = $2, updated_at = now() \
         WHERE id = $3",
    )
    .bind(&req.lifecycle)
    .bind(&reason)
    .bind(item_id)
    .execute(&mut *tx)
    .await
    .map_err(ContextItemError::db)?;

    audit(
        &mut tx,
        project_id,
        actor,
        "context_item.lifecycle",
        item_id,
        serde_json::json!({
            "conversation_id": conversation_id,
            "object_type": item.object_type,
            "from": from,
            "to": to,
            "reason": reason,
        }),
    )
    .await?;
    tx.commit().await.map_err(ContextItemError::db)?;

    fetch_item_owned(
        pool,
        tenant_id,
        project_id,
        principal_id,
        conversation_id,
        item_id,
    )
    .await
}

// ───────────────────────────────────────────────────── 软移除（DELETE）

/// 软移除（SQL-04）：`attachment='detached'`——不删原始业务对象、不丢版本引用；
/// 移除后不再注入 AI、不再出现在 Attached 列表、History 可见移除事件。
pub async fn remove_item(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
    conversation_id: Uuid,
    item_id: Uuid,
    actor: &AuditActor,
) -> Result<ContextItemView, ContextItemError> {
    ensure_conversation_owned(
        pool,
        tenant_id,
        project_id,
        principal_id,
        conversation_id,
    )
    .await?;
    let item = fetch_item_owned(
        pool,
        tenant_id,
        project_id,
        principal_id,
        conversation_id,
        item_id,
    )
    .await?;

    if item.attachment == ATTACHMENT_DETACHED {
        return Err(ContextItemError::Conflict(
            "item is already detached".to_string(),
        ));
    }

    let mut tx = pool.begin().await.map_err(ContextItemError::db)?;
    sqlx::query(
        "UPDATE ai.conversation_context_item \
         SET attachment = 'detached', updated_at = now() WHERE id = $1",
    )
    .bind(item_id)
    .execute(&mut *tx)
    .await
    .map_err(ContextItemError::db)?;

    audit(
        &mut tx,
        project_id,
        actor,
        "context_item.remove",
        item_id,
        serde_json::json!({
            "conversation_id": conversation_id,
            "object_type": item.object_type,
            "object_id": item.object_id,
            "lifecycle_before": item.lifecycle,
        }),
    )
    .await?;
    tx.commit().await.map_err(ContextItemError::db)?;

    fetch_item_owned(
        pool,
        tenant_id,
        project_id,
        principal_id,
        conversation_id,
        item_id,
    )
    .await
}

// ─────────────────────────────────────────────────────────────── 查询

/// 列会话的上下文条目（默认只列 attached；`include_detached` 供 History 用）。
pub async fn list_items(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
    conversation_id: Uuid,
    include_detached: bool,
) -> Result<Vec<ContextItemView>, ContextItemError> {
    ensure_conversation_owned(
        pool,
        tenant_id,
        project_id,
        principal_id,
        conversation_id,
    )
    .await?;
    let attachment_filter = if include_detached { "%" } else { "attached" };
    let rows = sqlx::query(
        "SELECT i.id, i.conversation_id, i.object_type, i.object_id, i.object_code, \
                i.revision_id, i.version, i.content_hash, i.row_version, \
                i.attachment, i.lifecycle, i.read_only, i.source, \
                i.unavailable_reason, i.added_by, i.added_at, i.updated_at \
           FROM ai.conversation_context_item i \
          WHERE i.conversation_id = $1 AND i.tenant_id = $2 AND i.project_id = $3 \
            AND (i.attachment = $4 OR $4 = '%') \
          ORDER BY i.added_at DESC, i.id",
    )
    .bind(conversation_id)
    .bind(tenant_id)
    .bind(project_id)
    .bind(attachment_filter)
    .fetch_all(pool)
    .await
    .map_err(ContextItemError::db)?;
    Ok(rows.iter().map(item_from_row).collect())
}
