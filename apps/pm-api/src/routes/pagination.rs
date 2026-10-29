use serde::Serialize;
use std::collections::HashMap;
use uuid::Uuid;

pub const DEFAULT_PAGE: i64 = 1;
pub const DEFAULT_SIZE: i64 = 10;
pub const MAX_SIZE: i64 = 200;

pub struct Paging {
    pub page: i64,
    pub size: i64,
    pub offset: i64,
}

impl Paging {
    pub fn to_response<T: Serialize>(self, items: Vec<T>, total: i64) -> serde_json::Value {
        serde_json::json!({
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

/// 可选的 project_id 过滤参数。
pub fn project_filter(params: &HashMap<String, String>) -> Option<Uuid> {
    params
        .get("project_id")
        .and_then(|v| Uuid::parse_str(v).ok())
}
