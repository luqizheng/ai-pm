//! `/v1/pm/customers` —— 客户主数据（0017）。
//!
//! 客户是项目的上层实体：一个客户可挂多个项目（`core.project.customer_id`
//! 非空外键）。建项不再隐式造客户，改由本模块维护后再在
//! `POST /v1/pm/projects` 里传 `customer_id` 挂接。
//!
//! 状态语义：`active`（正常）/ `archived`（归档，禁止新建项目，历史项目仍可读）/
//! `placeholder`（0017 之前由项目 code 隐式生成的未归类客户，待人工合并）。
//! 归档不级联删除项目——删除客户会让历史项目失去归属，代价高于收益。
//!
//! 分页与错误体约定同 `pm_lists`：裸 `{items,total,page,size}`、错误体只带
//! `{message}`（避免 @digua/http 覆盖成错误码）。

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::AuthedPrincipal;
use crate::routes::AppState;
use crate::support::{paging, repos::customer_repo, ApiResult};
use pm_application::repo::customer::{CustomerRepo, Page};
use pm_application::rows::CustomerRow;

/// 可写状态集合。`placeholder` 只能由迁移产生，不接受手工写入。
const WRITABLE_STATUS: [&str; 2] = ["active", "archived"];

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/pm/customers",
            get(list_customers).post(create_customer),
        )
        .route(
            "/v1/pm/customers/{customer_id}",
            get(get_customer).patch(update_customer),
        )
}

async fn fetch_customer(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    customer_id: Uuid,
    principal_id: Uuid,
) -> Result<Option<CustomerRow>, sqlx::Error> {
    customer_repo(pool).find_visible_row(tenant_id, customer_id, principal_id).await
}

pub(crate) async fn customer_visible_to(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    customer_id: Uuid,
    principal_id: Uuid,
) -> Result<bool, sqlx::Error> {
    customer_repo(pool).visible_to(tenant_id, customer_id, principal_id).await
}

async fn list_customers(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    let page = paging(&params);
    let query = params
        .get("q")
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(|v| format!("%{}%", v.replace('%', "\\%").replace('_', "\\_")));
    let status = params
        .get("status")
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string());

    let (items, total) = customer_repo(&state.pool)
        .list_visible(
            caller.tenant_id,
            caller.principal_id,
            query.as_deref(),
            status.as_deref(),
            Page { size: page.size, offset: page.offset },
        )
        .await?;

    Ok(Json(page.to_response(items, total)))
}

async fn get_customer(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(customer_id): Path<Uuid>,
) -> ApiResult<Response> {
    match fetch_customer(
        &state.pool,
        caller.tenant_id,
        customer_id,
        caller.principal_id,
    )
    .await?
    {
        Some(row) => Ok(Json(row).into_response()),
        None => Err(crate::support::ApiError::NotFound("customer not found")),
    }
}

#[derive(Debug, Deserialize)]
struct CreateCustomerBody {
    code: String,
    name: String,
    #[serde(default)]
    short_name: Option<String>,
    #[serde(default)]
    industry: Option<String>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    parent_id: Option<Uuid>,
}

fn bad_request(message: String) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "message": message })),
    )
        .into_response()
}

fn conflict(message: String) -> Response {
    (
        StatusCode::CONFLICT,
        Json(serde_json::json!({ "message": message })),
    )
        .into_response()
}

fn trimmed(value: &str) -> Option<String> {
    let v = value.trim();
    if v.is_empty() {
        None
    } else {
        Some(v.to_string())
    }
}

async fn create_customer(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Json(body): Json<CreateCustomerBody>,
) -> Response {
    let Some(code) = trimmed(&body.code) else {
        return bad_request("customer code is required".into());
    };
    let Some(name) = trimmed(&body.name) else {
        return bad_request("customer name is required".into());
    };

    if let Some(parent_id) = body.parent_id {
        match customer_visible_to(
            &state.pool,
            caller.tenant_id,
            parent_id,
            caller.principal_id,
        )
        .await
        {
            Ok(true) => {}
            Ok(false) => return bad_request("parent customer not found".into()),
            Err(err) => return internal(err),
        }
    }

    let inserted: Result<Uuid, sqlx::Error> = customer_repo(&state.pool)
        .insert_customer(
            caller.tenant_id,
            &code,
            &name,
            trimmed_opt(body.short_name.as_deref()).as_deref(),
            trimmed_opt(body.industry.as_deref()).as_deref(),
            trimmed_opt(body.notes.as_deref()).as_deref(),
            body.parent_id,
        )
        .await;

    let customer_id = match inserted {
        Ok(id) => id,
        Err(sqlx::Error::Database(db)) if db.is_unique_violation() => {
            return conflict(format!("customer code `{code}` already exists"));
        }
        Err(err) => return internal(err),
    };

    match fetch_customer(
        &state.pool,
        caller.tenant_id,
        customer_id,
        caller.principal_id,
    )
    .await
    {
        Ok(Some(row)) => (StatusCode::CREATED, Json(row)).into_response(),
        Ok(None) => internal(sqlx::Error::RowNotFound),
        Err(err) => internal(err),
    }
}

#[derive(Debug, Deserialize)]
struct UpdateCustomerBody {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    short_name: Option<String>,
    #[serde(default)]
    industry: Option<String>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    parent_id: Option<Uuid>,
}

/// `PATCH /v1/pm/customers/{id}`：字段缺省即不改；可空文本传空串即清空。
async fn update_customer(
    State(state): State<AppState>,
    Extension(caller): Extension<AuthedPrincipal>,
    Path(customer_id): Path<Uuid>,
    Json(body): Json<UpdateCustomerBody>,
) -> Response {
    let current = match fetch_customer(
        &state.pool,
        caller.tenant_id,
        customer_id,
        caller.principal_id,
    )
    .await
    {
        Ok(Some(row)) => row,
        Ok(None) => {
            return crate::support::ApiError::NotFound("customer not found").into_response()
        }
        Err(err) => return internal(err),
    };

    let code = match body.code {
        None => current.code,
        Some(raw) => match trimmed(&raw) {
            Some(v) => Some(v),
            None => return bad_request("customer code cannot be empty".into()),
        },
    };
    let name = match body.name {
        None => current.name,
        Some(raw) => match trimmed(&raw) {
            Some(v) => v,
            None => return bad_request("customer name cannot be empty".into()),
        },
    };
    let status = match body.status {
        None => current.status,
        Some(raw) => {
            let v = raw.trim().to_string();
            if !WRITABLE_STATUS.contains(&v.as_str()) {
                return bad_request(format!(
                    "status must be one of {}",
                    WRITABLE_STATUS.join(" / ")
                ));
            }
            v
        }
    };
    // parent_id 的「不改」与「清空」在 JSON 上无法区分（都落在 None），故约定：
    // 缺省/null 不改，指向自身视为非法。
    let parent_id = match body.parent_id {
        None => current.parent_id,
        Some(pid) if pid == customer_id => {
            return bad_request("customer cannot be its own parent".into());
        }
        Some(pid) => {
            match customer_visible_to(&state.pool, caller.tenant_id, pid, caller.principal_id).await
            {
                Ok(true) => Some(pid),
                Ok(false) => return bad_request("parent customer not found".into()),
                Err(err) => return internal(err),
            }
        }
    };

    let updated = customer_repo(&state.pool)
        .update_customer(
            caller.tenant_id,
            customer_id,
            code.as_deref(),
            &name,
            trimmed_opt_or(body.short_name.as_deref(), current.short_name).as_deref(),
            trimmed_opt_or(body.industry.as_deref(), current.industry).as_deref(),
            trimmed_opt_or(body.notes.as_deref(), current.notes).as_deref(),
            &status,
            parent_id,
        )
        .await
        .map(|_| ());

    if let Err(err) = updated {
        return match err {
            sqlx::Error::Database(db) if db.is_unique_violation() => {
                conflict("customer code already exists".into())
            }
            other => internal(other),
        };
    }

    match fetch_customer(
        &state.pool,
        caller.tenant_id,
        customer_id,
        caller.principal_id,
    )
    .await
    {
        Ok(Some(row)) => Json(row).into_response(),
        Ok(None) => crate::support::ApiError::NotFound("customer not found").into_response(),
        Err(err) => internal(err),
    }
}

fn trimmed_opt(value: Option<&str>) -> Option<String> {
    value.and_then(trimmed)
}

/// PATCH 语义：字段缺省保留原值；显式传空串清空。
fn trimmed_opt_or(next: Option<&str>, current: Option<String>) -> Option<String> {
    match next {
        None => current,
        Some(raw) => trimmed(raw),
    }
}

fn internal(err: sqlx::Error) -> Response {
    crate::support::ApiError::Internal(err).into_response()
}
