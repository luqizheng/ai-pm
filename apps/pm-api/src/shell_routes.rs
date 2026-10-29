//! 前端壳与业务读端点的路由聚合（frontend 契约，见
//! frontend/src/api/mock/fixtures.ts）。
//!
//! 独立成模块的原因：routes.rs 由并行开发流高频演进（ingestion / 命令通道），
//! 壳端点在此聚合、只由 main.rs merge 一次，降低两边覆盖彼此的概率。

// 实现文件在 src/routes/ 下（与 routes.rs 的 ingestion 模块互不干扰）
#[path = "routes/approvals.rs"]
pub mod approvals;
#[path = "routes/auth.rs"]
pub mod auth;
#[path = "routes/error.rs"]
pub mod error;
#[path = "routes/menu.rs"]
pub mod menu;
#[path = "routes/pagination.rs"]
pub mod pagination;
#[path = "routes/pm_lists.rs"]
pub mod pm_lists;
#[path = "routes/users.rs"]
pub mod users;

use axum::Router;
use sqlx::PgPool;

pub fn router() -> Router<PgPool> {
    auth::router()
        .merge(menu::router())
        .merge(users::router())
        .merge(pm_lists::router())
        .merge(approvals::router())
}
