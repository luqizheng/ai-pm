//! Project Cognition（projc.*）数据面服务层——P3-11 R-01 独立 schema 落地。
//!
//! 裁决依据：《Phase3-裁决稿-P3-11仓库分析认知归属与双域治理-v1.1.md》。
//! 复用 `pc.*` 机制模式（内容寻址 + CAS 推号 + 生命周期触发器），不复用业务表；
//! 两域权威边界五条见裁决稿 §1（不共享身份/状态机/确认权威/正文权威/字典）。
//!
//! Stage 1 范围（数据面写入者，仓库分析链重挂 T-2 的目标落点）：
//! * `object.rs`：对象草稿 upsert（具名幂等/内容寻址去重/行锁推号/CAS 指针）。
//! * `evidence.rs`：来源与证据登记（DomainRef target 侧契约：target_domain/
//!   target_type/target_id/target_revision_id + revision_missing_reason，与
//!   kb.memory_ref、pc.source 同款；与 relationship_type.owner_domain 的
//!   「关系确认权威域」语义严格分离——裁决稿 §3）。
//! * 提案/审批/发布链（T-2）与关系/冲突（P3-D04）不在本批：表结构已随
//!   0025 建齐（结构先建、写入者后启）。
//!
//! 门禁不破（fail-closed，与 pc 同口径）：草稿面不判主体身份——AI 只能提案、
//! 产物恒 draft，`lifecycle_status` 永不入 active（正式推进只发生在 ProjC
//! 自己的发布事务，T-2 落地时补真人主体 + A3 权限闸，pc::ensure_formal_authorized
//! 同款模式）。
//!
//! 哈希纪律：`content_hash` = sha256(markdown)，复用 `pc::sha256_hex`（域间
//! 共享的是通用基础设施模式，非业务权威——架构红线一允许共享项）。

mod evidence;
mod object;

pub use evidence::{
    register_evidence, register_source, EvidenceInput, EvidenceSubject, SourceInput,
};
pub use object::{upsert_object_draft, ObjectUpsertReceipt, UpsertObjectDraft};

use sqlx::PgPool;
use uuid::Uuid;

// ── 错误 ────────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum ProjcError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("invalid: {0}")]
    Invalid(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
}

// ── 类型注册校验（独立字典；与 pc.object_type 互不可见——R-02 权威边界）──────

pub struct ObjectTypeSpec {
    pub type_key: String,
    pub governance_level: String,
    pub singleton: bool,
}

pub(crate) async fn load_object_type(
    exec: impl sqlx::PgExecutor<'_>,
    type_key: &str,
) -> Result<ObjectTypeSpec, ProjcError> {
    let row: Option<(String, bool)> = sqlx::query_as(
        "SELECT governance_level, singleton FROM projc.object_type \
          WHERE type_key = $1 AND status = 'active'",
    )
    .bind(type_key)
    .fetch_optional(exec)
    .await?;
    row.map(|(governance_level, singleton)| ObjectTypeSpec {
        type_key: type_key.to_string(),
        governance_level,
        singleton,
    })
    .ok_or_else(|| ProjcError::Invalid(format!("unknown object type `{type_key}`")))
}

/// 租户归属解析（各写入者共用；pc 同款口径）。
pub(crate) async fn project_tenant(pool: &PgPool, project_id: Uuid) -> Result<Uuid, ProjcError> {
    sqlx::query_scalar("SELECT tenant_id FROM core.project WHERE id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await
        .map_err(|_| ProjcError::NotFound("project not found".into()))
}
