//! pm-application 集成测试共用夹具。
//!
//! 各测试文件是独立二进制，`mod common;` 引入后仅用到子集——整体豁免
//! dead_code（与 apps/pm-api/tests/common 同一约定）。
#![allow(dead_code)]

use pm_application::bootstrap::{bootstrap_project, BootstrapRequest};
use pm_domain::SYSTEM_TENANT;
use sqlx::PgPool;
use uuid::Uuid;

/// 系统租户下播种一个项目（各测试文件 `seed_project` 的单源实现，批 A5）。
/// 返回 (project_id, human_principal_id)。
pub async fn bootstrap_system_project(pool: &PgPool, code: &str) -> (Uuid, Uuid) {
    let outcome = bootstrap_project(
        pool,
        BootstrapRequest {
            project_code: code.to_string(),
            project_name: code.to_string(),
            tenant_id: SYSTEM_TENANT,
        },
    )
    .await
    .expect("bootstrap system project");
    (outcome.project_id, outcome.human_id)
}

/// 把 principal 加进租户（devenv 场景：bootstrap 产物默认只建项目成员关系）。
pub async fn ensure_tenant_member(pool: &PgPool, tenant_id: Uuid, principal_id: Uuid, role: &str) {
    sqlx::query(
        "INSERT INTO core.tenant_member (tenant_id, principal_id, role) VALUES ($1, $2, $3) \
         ON CONFLICT (tenant_id, principal_id) DO NOTHING",
    )
    .bind(tenant_id)
    .bind(principal_id)
    .bind(role)
    .execute(pool)
    .await
    .unwrap();
}
