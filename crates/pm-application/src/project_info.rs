//! 项目基本信息读改（P1 Project Settings · General）。
//!
//! GET 单项目详情（租户内可见即可读，可见性守卫在 API 层）；PATCH 名称/描述
//! （守卫 = 生命周期管理者，DR-PA-05② 同款：关联 Products 的有效 Product
//! Owner，与 close/reopen 同一执法点）。项目标识 `code` 不可经此面修改——
//! 它承担租户内唯一键与知识库 canonical path 前缀，改名属另立正式变更
//! （不入基本信息编辑）。危险操作（关闭/重开）走既有
//! `archive::{close_project, reopen_project}`，本模块不复制生命周期语义。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::actor::AuditActor;

const MAX_NAME_LEN: usize = 200;
const MAX_DESCRIPTION_LEN: usize = 2_000;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ProjectInfo {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub customer_name: Option<String>,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectInfoRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectInfoError {
    #[error("project not found")]
    NotFound,
    #[error("{0}")]
    Validation(String),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

/// 项目详情（租户内）。
pub async fn get_project_info(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<ProjectInfo, ProjectInfoError> {
    sqlx::query_as::<_, ProjectInfo>(
        "SELECT p.id, p.customer_id, c.name AS customer_name, p.code, p.name, p.description, \
                p.status, p.revision, p.created_at, p.updated_at \
         FROM core.project p \
         LEFT JOIN core.customer c ON c.id = p.customer_id \
         WHERE p.id = $1 AND p.tenant_id = $2",
    )
    .bind(project_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await?
    .ok_or(ProjectInfoError::NotFound)
}

/// 更新名称/描述（守卫在 API 层：生命周期管理者）。落审计、推进 revision。
pub async fn update_project_info(
    pool: &PgPool,
    actor: &AuditActor,
    tenant_id: Uuid,
    project_id: Uuid,
    req: UpdateProjectInfoRequest,
) -> Result<ProjectInfo, ProjectInfoError> {
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return Err(ProjectInfoError::Validation(
            "project name must not be empty".to_string(),
        ));
    }
    if name.chars().count() > MAX_NAME_LEN {
        return Err(ProjectInfoError::Validation(format!(
            "project name exceeds {MAX_NAME_LEN} characters"
        )));
    }
    let description = req
        .description
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty());
    if let Some(description) = &description {
        if description.chars().count() > MAX_DESCRIPTION_LEN {
            return Err(ProjectInfoError::Validation(format!(
                "project description exceeds {MAX_DESCRIPTION_LEN} characters"
            )));
        }
    }

    let mut tx = pool.begin().await?;
    let row = sqlx::query_as::<_, ProjectInfo>(
        "UPDATE core.project p SET name = $3, description = $4, \
             revision = p.revision + 1, updated_at = now() \
         FROM core.customer c \
         WHERE p.id = $1 AND p.tenant_id = $2 AND c.id = p.customer_id \
         RETURNING p.id, p.customer_id, c.name AS customer_name, p.code, p.name, \
                   p.description, p.status, p.revision, p.created_at, p.updated_at",
    )
    .bind(project_id)
    .bind(tenant_id)
    .bind(&name)
    .bind(&description)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ProjectInfoError::NotFound)?;

    sqlx::query(
        "INSERT INTO governance.audit_log \
         (id, project_id, actor_id, actor_identity_id, actor_client, actor_auth_mode, claimed_device_id, \
          action, target_type, target_id, tenant_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'project.info_updated', 'project', $2, $8)",
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(actor.principal_id)
    .bind(actor.identity_id)
    .bind(&actor.client)
    .bind(&actor.auth_mode)
    .bind(actor.claimed_device_id)
    .bind(tenant_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(row)
}
