//! 身份供给仓储 port（任务书批 B 第四域，自 pm-api `auth.rs` 收口）。
//!
//! 域语义（Q1/Q2/D5，见 auth.rs）：上游身份三元组 `(provider, tenant_key,
//! subject)` 1:1 镜像为 core.principal_identity + core.principal +
//! core.tenant_member；角色唯一事实源是 `tenant_member.role`。并发首见语义
//! （identity 行抢占失败 → 删刚建 principal → 复用赢家行）由实现侧保证。
//!
//! 这是 IAM 底座（横切面），不是业务域：port 只暴露 resolve/provision 所需
//! 的最小读写面；dev-off 物化（`inject_dev_principal`）与 cpk 租户角色回退
//! 同样入此（同组表）。

use async_trait::async_trait;
use uuid::Uuid;

/// 已解析身份行（principal_identity ⋈ principal）。
#[derive(Debug, Clone)]
pub struct ResolvedIdentity {
    pub identity_id: Uuid,
    pub principal_id: Uuid,
    pub is_active: bool,
    pub display_name: String,
}

/// 首次供给输入。
#[derive(Debug, Clone)]
pub struct ProvisionIdentity<'a> {
    pub provider: &'a str,
    pub tenant_id: Uuid,
    pub subject: &'a str,
    pub display_name: &'a str,
    pub is_active: bool,
    pub role: &'a str,
}

/// 域级语义错误。
#[derive(Debug)]
pub enum IdentityRepoError {
    Db(sqlx::Error),
}

impl std::fmt::Display for IdentityRepoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Db(e) => write!(f, "identity repo db error: {e}"),
        }
    }
}

impl std::error::Error for IdentityRepoError {}

impl From<sqlx::Error> for IdentityRepoError {
    fn from(e: sqlx::Error) -> Self {
        Self::Db(e)
    }
}

#[async_trait]
pub trait IdentityRepo: Send + Sync {
    /// 按 `(provider, external_key)` 找租户 id。
    async fn find_tenant(
        &self,
        provider: &str,
        external_key: &str,
    ) -> Result<Option<Uuid>, IdentityRepoError>;

    /// 建租户；并发输家（唯一键冲突未插入）返回 None。
    async fn insert_tenant(
        &self,
        provider: &str,
        external_key: &str,
        display_name: &str,
    ) -> Result<Option<Uuid>, IdentityRepoError>;

    /// 按 `(provider, tenant, subject)` 解析已有身份（节流写 last_seen_at，
    /// 5 分钟内不重复写库——Q1 节流语义在实现侧）。
    async fn find_identity(
        &self,
        provider: &str,
        tenant_id: Uuid,
        subject: &str,
    ) -> Result<Option<ResolvedIdentity>, IdentityRepoError>;

    /// 首次供给：principal（默认未激活）+ identity（唯一键抢占）+ tenant_member。
    /// 抢占失败 → 删刚建 principal 并复用赢家行（并发输家语义）。
    async fn provision_principal(
        &self,
        input: ProvisionIdentity<'_>,
    ) -> Result<ResolvedIdentity, IdentityRepoError>;

    /// 租户内角色（角色唯一事实源 D5；无行 = None）。
    async fn tenant_role(
        &self,
        tenant_id: Uuid,
        principal_id: Uuid,
    ) -> Result<Option<String>, IdentityRepoError>;

    /// cpk 通道：跨租户读 owner 角色（无行 = None；无租户边界，按 principal）。
    async fn any_tenant_role(
        &self,
        principal_id: Uuid,
    ) -> Result<Option<String>, IdentityRepoError>;

    /// dev-off 物化（`AI_PM_AUTH_MODE=off`）：principal + tenant_member 落
    /// 系统租户（幂等 ON CONFLICT DO NOTHING；只随写请求调用）。
    async fn ensure_dev_off(&self, principal_id: Uuid, tenant_id: Uuid) -> Result<(), IdentityRepoError>;
}
