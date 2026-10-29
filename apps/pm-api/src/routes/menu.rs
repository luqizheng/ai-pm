//! `GET /v1/pure/get-async-routes` —— 壳的 DB 菜单。
//!
//! pm-api 尚无菜单配置表（上游 prue_backend 的菜单配置版本机制），先下发
//! 与前端 mock 相同的静态树；菜单表落地后改为查库。信封 `{code:0,...}`
//! 是壳 getAsyncRoutes 的既定契约（code 非 0 会被壳丢弃）。

use axum::{Json, Router, routing::get};
use serde_json::json;
use sqlx::PgPool;

use super::auth::Authed;

pub fn router() -> Router<PgPool> {
    Router::new().route("/v1/pure/get-async-routes", get(async_routes))
}

async fn async_routes(_auth: Authed) -> Json<serde_json::Value> {
    Json(json!({
        "code": 0,
        "message": "ok",
        "data": [
            {
                "path": "/overview",
                "name": "PmOverview",
                "component": "pm/overview",
                "meta": { "title": "概览", "icon": "layout-dashboard" }
            },
            {
                "path": "/delivery",
                "name": "DeliveryGroup",
                "meta": { "title": "交付" },
                "children": [
                    { "path": "/projects", "name": "PmProjects", "component": "pm/projects", "meta": { "title": "项目", "icon": "folder-tree" } },
                    { "path": "/requirements", "name": "PmRequirements", "component": "pm/requirements", "meta": { "title": "需求", "icon": "file-text" } },
                    { "path": "/tasks", "name": "PmTasks", "component": "pm/tasks", "meta": { "title": "任务", "icon": "list-tree" } }
                ]
            },
            {
                "path": "/governance",
                "name": "GovernanceGroup",
                "meta": { "title": "治理" },
                "children": [
                    { "path": "/approvals", "name": "PmApprovals", "component": "pm/approvals", "meta": { "title": "审批确认", "icon": "shield-check" } },
                    { "path": "/audit", "name": "PmAudit", "component": "pm/audit", "meta": { "title": "审计日志", "icon": "scroll-text" } }
                ]
            },
            {
                "path": "/pm-settings",
                "name": "PmSettings",
                "component": "pm/settings",
                "meta": { "title": "设置", "icon": "sliders-horizontal" }
            }
        ]
    }))
}
