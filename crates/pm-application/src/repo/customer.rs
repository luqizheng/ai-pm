//! 客户主数据 port（任务书批 B 域 10，自 pm-api `customers.rs` 收口）。
//!
//! 域语义（0017，见 customers 模块头）：客户可挂多项目；T-022 可见性谓词
//! （任一项目成员 或 无项目待归类池）在 SQL 内完成；`placeholder` 只能由
//! 迁移产生。唯一键冲突由调用方经 sqlx::Error::Database 识别（ApiError 链）。
use async_trait::async_trait;
use uuid::Uuid;

use crate::rows::CustomerRow;

/// 分页参数（size/offset）。
#[derive(Debug, Clone, Copy)]
pub struct Page {
    pub size: i64,
    pub offset: i64,
}

#[async_trait]
pub trait CustomerRepo: Send + Sync {
    /// T-022 可见性判定（建项挂接/父子校验共用）。
    async fn visible_to(
        &self,
        tenant_id: Uuid,
        customer_id: Uuid,
        principal_id: Uuid,
    ) -> Result<bool, sqlx::Error>;

    async fn find_visible_row(
        &self,
        tenant_id: Uuid,
        customer_id: Uuid,
        principal_id: Uuid,
    ) -> Result<Option<CustomerRow>, sqlx::Error>;

    async fn list_visible(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
        query: Option<&str>,
        status: Option<&str>,
        page: Page,
    ) -> Result<(Vec<CustomerRow>, i64), sqlx::Error>;

    /// 建客户（status='active'）；唯一键冲突原样上抛。
    #[allow(clippy::too_many_arguments)] // 客户行字段齐写
    async fn insert_customer(
        &self,
        tenant_id: Uuid,
        code: &str,
        name: &str,
        short_name: Option<&str>,
        industry: Option<&str>,
        notes: Option<&str>,
        parent_id: Option<Uuid>,
    ) -> Result<Uuid, sqlx::Error>;

    /// 全量字段更新（PATCH 语义在调用方合并后传终值；code/name 沿用原
    /// handler 语义——合并结果可为 NULL）。
    #[allow(clippy::too_many_arguments)] // 客户行字段齐写
    async fn update_customer(
        &self,
        tenant_id: Uuid,
        customer_id: Uuid,
        code: Option<&str>,
        name: &str,
        short_name: Option<&str>,
        industry: Option<&str>,
        notes: Option<&str>,
        status: &str,
        parent_id: Option<Uuid>,
    ) -> Result<(), sqlx::Error>;
}
