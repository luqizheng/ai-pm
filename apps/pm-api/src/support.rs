//! `/v1/pm/*` 端点共用件：统一错误响应与分页解析（Slice A）。
//!
//! 错误体刻意只带 `{message}` 不带 code：@digua/http 对带 code 的错误体会先走
//! contracts 的错误码翻译，未知码会把 message 覆盖成码本身；只给 message 才能
//! 原样透传（沿用 prism-shell-wip 分支的结论）。

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

pub enum ApiError {
    BadRequest(&'static str),
    Conflict(&'static str),
    NotFound(&'static str),
    /// 有身份但无权限（S02-I1：AI 主体确认 / 非 Lead 治理命令）。与 404（不泄露
    /// 存在性）和 400（参数错）区分——S02D-02 #2 的拒绝必须可观测。
    Forbidden(&'static str),
    /// 动态文本错误（ACP-03 交互面）：InteractionError 携带运行时消息，
    /// 静态串变体装不下；状态码由映射方给出。
    Text(StatusCode, String),
    /// 契约路由已挂接、业务未接线（KB/PC 服务化 V1 占位端口；501 ≠ 404 误导）。
    NotImplemented(&'static str),
    Internal(sqlx::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // 动态文本先走自有路径（借用不能混入 &'static 序列）。
        if let ApiError::Text(status, m) = &self {
            return (*status, Json(json!({ "message": m }))).into_response();
        }
        let (status, message) = match self {
            ApiError::Text(_, _) => unreachable!("handled above"),
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            ApiError::Conflict(m) => (StatusCode::CONFLICT, m),
            ApiError::NotFound(m) => (StatusCode::NOT_FOUND, m),
            ApiError::Forbidden(m) => (StatusCode::FORBIDDEN, m),
            ApiError::NotImplemented(m) => (StatusCode::NOT_IMPLEMENTED, m),
            ApiError::Internal(e) => {
                tracing::error!(error = %e, "pm-api internal error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
            }
        };
        (status, Json(json!({ "message": message }))).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        ApiError::Internal(e)
    }
}

impl From<pm_application::pm_write::PmWriteError> for ApiError {
    fn from(e: pm_application::pm_write::PmWriteError) -> Self {
        match e {
            pm_application::pm_write::PmWriteError::NotFound(m) => ApiError::NotFound(m),
            pm_application::pm_write::PmWriteError::Conflict(m) => ApiError::Conflict(m),
            pm_application::pm_write::PmWriteError::Validation(m) => ApiError::BadRequest(m),
            pm_application::pm_write::PmWriteError::ValidationMsg(m) => {
                ApiError::Text(axum::http::StatusCode::BAD_REQUEST, m)
            }
            pm_application::pm_write::PmWriteError::Forbidden(m) => ApiError::Forbidden(m),
            pm_application::pm_write::PmWriteError::Db(err) => ApiError::Internal(err),
        }
    }
}

/// ACP-03 交互面（设计稿 smark-pm-s03-acp-slice-design-acp03-v1.md §7）：
/// NotFound 保留静态串（不泄露存在性），动态消息按语义给状态码。
impl From<pm_application::interaction::InteractionError> for ApiError {
    fn from(e: pm_application::interaction::InteractionError) -> Self {
        use pm_application::interaction::InteractionError as E;
        match e {
            E::NotFound => ApiError::NotFound("interaction not found in this project"),
            E::Validation(m) => ApiError::Text(StatusCode::BAD_REQUEST, m),
            E::Conflict(m) => ApiError::Text(StatusCode::CONFLICT, m),
            E::Forbidden(m) => ApiError::Text(StatusCode::FORBIDDEN, m),
            E::Transition(m) => ApiError::Text(StatusCode::CONFLICT, m),
            E::Db(err) => ApiError::Internal(err),
        }
    }
}

/// 确认类命令的可信交互上下文（§7.2 A*）：API Key 等机器凭据一律 403；
/// MCP 无确认工具（保持零工具，H-15）。登录令牌复用不可区分属 A* 已档边界。
/// （自 pm_requirements 上移共用——ACP-03 回复入口同一门。）
pub(crate) fn ensure_trusted_context(principal: &crate::auth::AuthedPrincipal) -> ApiResult<()> {
    if principal.auth_mode == "api_key" {
        return Err(ApiError::Forbidden(
            "confirmation requires a trusted interactive context",
        ));
    }
    Ok(())
}

impl From<pm_application::outbox_ops::ReplayError> for ApiError {
    fn from(e: pm_application::outbox_ops::ReplayError) -> Self {
        match e {
            // 跨项目事件与不存在事件统一 404（防事件 ID 探测）。
            pm_application::outbox_ops::ReplayError::NotFound => {
                ApiError::NotFound("outbox event not found")
            }
            pm_application::outbox_ops::ReplayError::NotDeadLetter => {
                ApiError::Conflict("outbox event is not dead-lettered")
            }
            pm_application::outbox_ops::ReplayError::Db(detail) => {
                tracing::error!(error = %detail, "outbox replay failed");
                ApiError::Internal(sqlx::Error::Protocol(detail))
            }
        }
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

pub const DEFAULT_PAGE: i64 = 1;
pub const DEFAULT_SIZE: i64 = 10;
pub const MAX_SIZE: i64 = 200;

pub struct Paging {
    pub page: i64,
    pub size: i64,
    pub offset: i64,
}

impl Paging {
    pub fn to_response<T: serde::Serialize>(self, items: Vec<T>, total: i64) -> serde_json::Value {
        json!({
            "items": items,
            "total": total,
            "page": self.page,
            "size": self.size,
        })
    }
}

/// 解析 page/size（非法或缺省回落默认值，size 截到 MAX_SIZE）。
pub fn paging(params: &HashMap<String, String>) -> Paging {
    let page = params
        .get("page")
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v >= 1)
        .unwrap_or(DEFAULT_PAGE);
    let size = params
        .get("size")
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v >= 1)
        .map(|v| v.min(MAX_SIZE))
        .unwrap_or(DEFAULT_SIZE);
    Paging {
        page,
        size,
        offset: (page - 1) * size,
    }
}

/// 可选的 project_id 过滤参数（非法值视为未传）。
pub fn project_filter(params: &HashMap<String, String>) -> Option<Uuid> {
    params
        .get("project_id")
        .and_then(|v| Uuid::parse_str(v).ok())
}

/// 可选的 customer_id 过滤参数（非法值视为未传）。项目列表按客户聚合时使用。
pub fn customer_filter(params: &HashMap<String, String>) -> Option<Uuid> {
    params
        .get("customer_id")
        .and_then(|v| Uuid::parse_str(v).ok())
}

/// 项目可见性（T-022）：同一个租户内，只有 `core.project_member` 里有调用者的
/// 项目才可见；跨租户与「同租户非成员」一律按不存在处理。终态软删
/// （team dissolve 置 `status='deleted'`）的项目双重保险排除。
pub async fn project_visible(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    tenant_id: Uuid,
    principal_id: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS ( \
             SELECT 1 FROM core.project p \
             JOIN core.project_member m ON m.project_id = p.id \
              WHERE p.id = $1 AND p.tenant_id = $2 AND m.principal_id = $3 \
                AND p.status <> 'deleted')",
    )
    .bind(project_id)
    .bind(tenant_id)
    .bind(principal_id)
    .fetch_one(pool)
    .await
}

/// 显式 `project_id` 过滤时的不存在语义：不可见即 404（不泄露存在性）。
pub async fn require_project_visible(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    tenant_id: Uuid,
    principal_id: Uuid,
) -> ApiResult<()> {
    if project_visible(pool, project_id, tenant_id, principal_id).await? {
        Ok(())
    } else {
        Err(ApiError::NotFound("project not found"))
    }
}

/// 任务级端点的项目边界：任务不存在 / 跨租户 / 调用者非项目成员一律 404，
/// 返回任务所属 `project_id`（D4 交付清单与 T-070 验收证据端点共用同一口径）。
pub async fn require_task_project(
    pool: &sqlx::PgPool,
    task_id: Uuid,
    principal: &crate::auth::AuthedPrincipal,
) -> ApiResult<Uuid> {
    let project_id: Option<Uuid> =
        sqlx::query_scalar("SELECT project_id FROM delivery.task WHERE id = $1 AND tenant_id = $2")
            .bind(task_id)
            .bind(principal.tenant_id)
            .fetch_optional(pool)
            .await?;
    let project_id = project_id.ok_or(ApiError::NotFound("task_not_found"))?;
    require_project_visible(
        pool,
        project_id,
        principal.tenant_id,
        principal.principal_id,
    )
    .await?;
    Ok(project_id)
}

// ───────────────────────────────────────────── 仓储构造器（批 B 收口辅助）

/// 各 handler/middleware 统一从这儿拿仓储实现（PgPool clone 廉价；
/// 换实现只动此处，调用方只依赖 pm_application 的 trait）。
pub mod repos {
    /// pm-api 各模块的仓储集合：按域惰性构造，无全局状态。
    pub fn identity_repo(
        pool: &sqlx::PgPool,
    ) -> pm_infrastructure::repo_identity::PgIdentityRepo {
        pm_infrastructure::repo_identity::PgIdentityRepo::new(pool.clone())
    }

    pub fn list_repo(pool: &sqlx::PgPool) -> pm_infrastructure::repo_lists::PgPmListRepo {
        pm_infrastructure::repo_lists::PgPmListRepo::new(pool.clone())
    }

    pub fn baseline_repo(pool: &sqlx::PgPool) -> pm_infrastructure::repo_baseline::PgBaselineRepo {
        pm_infrastructure::repo_baseline::PgBaselineRepo::new(pool.clone())
    }

    pub fn cognition_repo(pool: &sqlx::PgPool) -> pm_infrastructure::repo_cognition::PgCognitionRepo {
        pm_infrastructure::repo_cognition::PgCognitionRepo::new(pool.clone())
    }

    pub fn member_repo(
        pool: &sqlx::PgPool,
    ) -> pm_infrastructure::repo_tenant_members::PgMemberRepo {
        pm_infrastructure::repo_tenant_members::PgMemberRepo::new(pool.clone())
    }

    pub fn run_repo(pool: &sqlx::PgPool) -> pm_infrastructure::repo_runs::PgRunRepo {
        pm_infrastructure::repo_runs::PgRunRepo::new(pool.clone())
    }
}
