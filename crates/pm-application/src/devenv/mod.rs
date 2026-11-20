//! 开发环境目录域（T-088）：设备登记 / 绑定 / 自检 / 撤销。
//!
//! 边界（Codex Bootstrap V1.3 §5.4.1、评审 P0-1/P0-2）：
//!
//! - **Device 是目录实体**——V1 不持有执行凭据，不进认证链；设备声明只是
//!   「客户端声称的开发环境」，**不产生**项目或仓库权限。
//! - **Binding 是声明与事实记录**，不是权限授予机制。
//! - **Ready 只描述设备自身**（installed / connected / authenticated / ready /
//!   degraded / failed）；项目可用性是「绑定 × 当前调用人成员资格」的派生值，
//!   按调用人实时计算，**不落库**（设计稿 M7）。
//! - 审计：设备生命周期与「设备声明被忽略」事件写**租户级**
//!   `governance.tenant_audit_log`；绑定类事件写**项目级** `governance.audit_log`。

use sqlx::PgPool;
use sqlx::Row;
use uuid::Uuid;

use crate::caller::HumanActor;

mod availability;
mod binding;
mod checks;

pub use availability::{
    is_runner_available, refresh_runner_heartbeat, runner_availability_stale_secs,
};
pub use binding::{bind_project, unbind_binding};
pub use checks::{report_checks, DeviceChecks};

/// 设备生命周期状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStatus {
    Active,
    Revoked,
}

impl DeviceStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Revoked => "revoked",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "revoked" => Self::Revoked,
            _ => Self::Active,
        }
    }
}

/// 设备就绪快照（六层取值）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceReadiness {
    Installed,
    Connected,
    Authenticated,
    Ready,
    Degraded,
    Failed,
}

impl DeviceReadiness {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Installed => "installed",
            Self::Connected => "connected",
            Self::Authenticated => "authenticated",
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "installed" => Self::Installed,
            "connected" => Self::Connected,
            "authenticated" => Self::Authenticated,
            "ready" => Self::Ready,
            "failed" => Self::Failed,
            _ => Self::Degraded,
        }
    }
}

/// 设备目录行。
#[derive(Debug, Clone)]
pub struct DeviceRow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub owner_principal_id: Uuid,
    pub name: String,
    pub platform: String,
    pub os_version: Option<String>,
    pub executor_kind: String,
    pub executor_version: Option<String>,
    pub status: DeviceStatus,
    pub readiness: Option<DeviceReadiness>,
    pub last_check_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_check: Option<serde_json::Value>,
    pub revoked_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// D-2=c Q-D2C-1：领取型 Runner 载体（其绑定项目由 Runner 显式领取，
    /// worker 无人值守派发静默排除）。
    pub is_runner: bool,
    /// D-2=c Q-D2C-2：设备级心跳（可用性真源），已认证设备活动服务端归因刷新。
    pub last_heartbeat_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// 绑定行。
#[derive(Debug, Clone)]
pub struct BindingRow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub device_id: Uuid,
    pub project_id: Uuid,
    pub declared_repository_ids: Vec<Uuid>,
    pub verified_repositories: Vec<Uuid>,
    pub bound_by: Uuid,
    pub bound_at: chrono::DateTime<chrono::Utc>,
    pub unbound_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// 设备详情：设备 + 绑定 + **按调用人计算**的项目可用性。
#[derive(Debug, Clone)]
pub struct DeviceDetail {
    pub device: DeviceRow,
    pub bindings: Vec<BindingView>,
}

/// 单条绑定的对外视图（含派生字段）。
#[derive(Debug, Clone)]
pub struct BindingView {
    pub binding: BindingRow,
    /// 当前调用人是否为该 Project 的成员（派生值，不落库）。
    pub project_accessible: bool,
}

/// 设备域失败。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DeviceError {
    /// 设备不存在 / 不可见（**上层映射 404**，不泄露存在性）。
    #[error("device not found")]
    NotFound,
    /// 绑定不存在。
    #[error("binding not found")]
    BindingNotFound,
    /// 校验失败。
    #[error("{0}")]
    Validation(String),
    /// 数据库错误。
    #[error("database error: {0}")]
    Db(String),
}

/// 登记请求。
#[derive(Debug, Clone)]
pub struct RegisterDeviceRequest {
    /// 客户端生成的设备 id；`None` = 首次登记（服务端生成）。
    pub device_id: Option<Uuid>,
    pub name: String,
    pub platform: String,
    pub os_version: Option<String>,
    pub executor_kind: Option<String>,
    pub executor_version: Option<String>,
    /// D-2=c：领取型 Runner 载体声明（缺省 false）。复用登记时可更新。
    pub is_runner: Option<bool>,
}

/// 登记结果。
#[derive(Debug, Clone)]
pub struct RegisterDeviceOutcome {
    pub device: DeviceRow,
    /// 是否复用了既有（active 且 owner 匹配）设备。
    pub reused: bool,
}

/// 登记 / 复用设备（D5：id 优先）。
///
/// 并发语义：同一 UUID 的并发请求**收敛为一行**——INSERT 走
/// `ON CONFLICT DO NOTHING`，随后回读并按复用规则判定；已存在但已撤销的
/// 设备**不得自动恢复**（新建新 id，旧行留历史）。
///
/// **仲裁索引不能只写 `id`**：本表同时有 `PRIMARY KEY (id)` 与
/// `UNIQUE (tenant_id, id)`；并发插入同一 id 时 PostgreSQL 可能先命中复合
/// 唯一约束，而 `ON CONFLICT (id)` 只吞仲裁索引上的冲突 → 错误逃逸（并发登记
/// 报唯一键冲突而非收敛为一行）。故 INSERT **不指定仲裁目标**，任何唯一冲突
/// 都归入「已存在」分支，由回读 + 复用规则收敛。
pub async fn register_device(
    pool: &PgPool,
    caller: &HumanActor,
    req: &RegisterDeviceRequest,
) -> Result<RegisterDeviceOutcome, DeviceError> {
    validate_register(req)?;
    // 竞态窗口内冲突行可能被删除 → 重试（最多 3 轮；实践中一轮即收敛）。
    for _ in 0..3 {
        match register_once(pool, caller, req).await? {
            RegisterAttempt::Done(outcome) => return Ok(outcome),
            RegisterAttempt::Retry => continue,
        }
    }
    Err(DeviceError::Db(
        "device registration did not converge after retries".into(),
    ))
}

/// 登记的一次尝试（逐 attempt 瞬时信封，264B 拷贝无谓 boxing）。
#[allow(clippy::large_enum_variant)]
enum RegisterAttempt {
    Done(RegisterDeviceOutcome),
    /// 冲突行在锁定前被删除——需要重试。
    Retry,
}

async fn register_once(
    pool: &PgPool,
    caller: &HumanActor,
    req: &RegisterDeviceRequest,
) -> Result<RegisterAttempt, DeviceError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| DeviceError::Db(e.to_string()))?;

    let requested_id = req.device_id.unwrap_or_else(Uuid::new_v4);
    let inserted = sqlx::query(
        "INSERT INTO devenv.device \
         (id, tenant_id, owner_principal_id, name, platform, os_version, \
          executor_kind, executor_version, status, readiness, is_runner) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'active', NULL, $9) \
         ON CONFLICT DO NOTHING",
    )
    .bind(requested_id)
    .bind(caller.tenant_id)
    .bind(caller.principal_id)
    .bind(req.name.trim())
    .bind(req.platform.trim())
    .bind(req.os_version.as_deref().map(str::trim))
    .bind(req.executor_kind.as_deref().unwrap_or("codex"))
    .bind(req.executor_version.as_deref().map(str::trim))
    .bind(req.is_runner.unwrap_or(false))
    .execute(&mut *tx)
    .await
    .map_err(|e| DeviceError::Db(e.to_string()))?
    .rows_affected()
        == 1;

    if inserted {
        let row = fetch_device_in(&mut tx, requested_id)
            .await?
            .ok_or(DeviceError::NotFound)?;
        tx.commit()
            .await
            .map_err(|e| DeviceError::Db(e.to_string()))?;
        return Ok(RegisterAttempt::Done(RegisterDeviceOutcome {
            device: row,
            reused: false,
        }));
    }

    // 已存在：加锁后判定（并发请求串行化）。
    let existing = sqlx::query(
        "SELECT id, tenant_id, owner_principal_id, name, platform, os_version, \
                executor_kind, executor_version, status, readiness, last_check_at, \
                last_check, revoked_at, created_at, updated_at, is_runner, \
                last_heartbeat_at \
         FROM devenv.device WHERE id = $1 FOR UPDATE",
    )
    .bind(requested_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DeviceError::Db(e.to_string()))?;

    let existing = match existing {
        Some(row) => row,
        None => {
            // 竞态：冲突行已被删除。回滚后重试。
            tx.rollback().await.ok();
            return Ok(RegisterAttempt::Retry);
        }
    };

    // 他人的设备 id → 不可见（404），不新建。
    if existing.get::<Uuid, _>("owner_principal_id") != caller.principal_id
        || existing.get::<Uuid, _>("tenant_id") != caller.tenant_id
    {
        tx.rollback().await.ok();
        return Err(DeviceError::NotFound);
    }

    let status = DeviceStatus::parse(existing.get::<String, _>("status").as_str());
    if status == DeviceStatus::Revoked {
        // 已撤销设备不得自动恢复：新建新 id，旧行留历史。
        let fresh_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO devenv.device \
             (id, tenant_id, owner_principal_id, name, platform, os_version, \
              executor_kind, executor_version, status, readiness, is_runner) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'active', NULL, $9)",
        )
        .bind(fresh_id)
        .bind(caller.tenant_id)
        .bind(caller.principal_id)
        .bind(req.name.trim())
        .bind(req.platform.trim())
        .bind(req.os_version.as_deref().map(str::trim))
        .bind(req.executor_kind.as_deref().unwrap_or("codex"))
        .bind(req.executor_version.as_deref().map(str::trim))
        .bind(req.is_runner.unwrap_or(false))
        .execute(&mut *tx)
        .await
        .map_err(|e| DeviceError::Db(e.to_string()))?;
        let row = fetch_device_in(&mut tx, fresh_id)
            .await?
            .ok_or(DeviceError::NotFound)?;
        tx.commit()
            .await
            .map_err(|e| DeviceError::Db(e.to_string()))?;
        return Ok(RegisterAttempt::Done(RegisterDeviceOutcome {
            device: row,
            reused: false,
        }));
    }

    // 复用：更新可变字段。
    sqlx::query(
        "UPDATE devenv.device \
         SET name = $2, platform = $3, os_version = $4, executor_kind = $5, \
             executor_version = $6, is_runner = $7, updated_at = now() \
         WHERE id = $1",
    )
    .bind(requested_id)
    .bind(req.name.trim())
    .bind(req.platform.trim())
    .bind(req.os_version.as_deref().map(str::trim))
    .bind(req.executor_kind.as_deref().unwrap_or("codex"))
    .bind(req.executor_version.as_deref().map(str::trim))
    .bind(req.is_runner.unwrap_or(false))
    .execute(&mut *tx)
    .await
    .map_err(|e| DeviceError::Db(e.to_string()))?;
    let row = fetch_device_in(&mut tx, requested_id)
        .await?
        .ok_or(DeviceError::NotFound)?;
    tx.commit()
        .await
        .map_err(|e| DeviceError::Db(e.to_string()))?;
    Ok(RegisterAttempt::Done(RegisterDeviceOutcome {
        device: row,
        reused: true,
    }))
}

/// 列出调用人可见的设备：member 见自己的，tenant owner/admin 见全租户。
pub async fn list_devices(
    pool: &PgPool,
    caller: &HumanActor,
) -> Result<Vec<DeviceRow>, DeviceError> {
    let is_admin = caller_is_team_owner(pool, caller).await?;
    let rows = sqlx::query(
        "SELECT id, tenant_id, owner_principal_id, name, platform, os_version, \
                executor_kind, executor_version, status, readiness, last_check_at, \
                last_check, revoked_at, created_at, updated_at, is_runner, \
                last_heartbeat_at \
         FROM devenv.device \
         WHERE tenant_id = $1 AND ($2 OR owner_principal_id = $3) \
         ORDER BY created_at DESC",
    )
    .bind(caller.tenant_id)
    .bind(is_admin)
    .bind(caller.principal_id)
    .fetch_all(pool)
    .await
    .map_err(|e| DeviceError::Db(e.to_string()))?;
    Ok(rows.into_iter().map(device_from_row).collect())
}

/// 取设备详情（含绑定与按调用人计算的项目可用性）。
pub async fn get_device(
    pool: &PgPool,
    caller: &HumanActor,
    device_id: Uuid,
) -> Result<DeviceDetail, DeviceError> {
    let is_admin = caller_is_team_owner(pool, caller).await?;
    let device = fetch_device(pool, device_id)
        .await?
        .ok_or(DeviceError::NotFound)?;
    if device.tenant_id != caller.tenant_id
        || (!is_admin && device.owner_principal_id != caller.principal_id)
    {
        return Err(DeviceError::NotFound);
    }
    let bindings = list_bindings(pool, device_id).await?;
    let mut views = Vec::with_capacity(bindings.len());
    for binding in bindings {
        let accessible = crate::authz::require_project_member(
            pool,
            caller.principal_id,
            caller.tenant_id,
            binding.project_id,
        )
        .await
        .is_ok();
        views.push(BindingView {
            binding,
            project_accessible: accessible,
        });
    }
    Ok(DeviceDetail {
        device,
        bindings: views,
    })
}

/// 撤销设备（生命周期 → revoked）。幂等。
pub async fn revoke_device(
    pool: &PgPool,
    caller: &HumanActor,
    device_id: Uuid,
) -> Result<DeviceRow, DeviceError> {
    let is_admin = caller_is_team_owner(pool, caller).await?;
    let device = fetch_device(pool, device_id)
        .await?
        .ok_or(DeviceError::NotFound)?;
    if device.tenant_id != caller.tenant_id
        || (!is_admin && device.owner_principal_id != caller.principal_id)
    {
        return Err(DeviceError::NotFound);
    }
    sqlx::query(
        "UPDATE devenv.device \
         SET status = 'revoked', revoked_at = now(), readiness = NULL, updated_at = now() \
         WHERE id = $1",
    )
    .bind(device_id)
    .execute(pool)
    .await
    .map_err(|e| DeviceError::Db(e.to_string()))?;
    fetch_device(pool, device_id)
        .await?
        .ok_or(DeviceError::NotFound)
}

/// 取一行设备（不判可见性——调用方自行判）。
pub(crate) async fn fetch_device(
    pool: &PgPool,
    device_id: Uuid,
) -> Result<Option<DeviceRow>, DeviceError> {
    let row = sqlx::query(
        "SELECT id, tenant_id, owner_principal_id, name, platform, os_version, \
                executor_kind, executor_version, status, readiness, last_check_at, \
                last_check, revoked_at, created_at, updated_at, is_runner, \
                last_heartbeat_at \
         FROM devenv.device WHERE id = $1",
    )
    .bind(device_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| DeviceError::Db(e.to_string()))?;
    Ok(row.map(device_from_row))
}

/// 取一行设备（事务内）。
pub(crate) async fn fetch_device_in(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    device_id: Uuid,
) -> Result<Option<DeviceRow>, DeviceError> {
    let row = sqlx::query(
        "SELECT id, tenant_id, owner_principal_id, name, platform, os_version, \
                executor_kind, executor_version, status, readiness, last_check_at, \
                last_check, revoked_at, created_at, updated_at, is_runner, \
                last_heartbeat_at \
         FROM devenv.device WHERE id = $1",
    )
    .bind(device_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| DeviceError::Db(e.to_string()))?;
    Ok(row.map(device_from_row))
}

/// 列出设备的有效绑定（未解绑）。
pub(crate) async fn list_bindings(
    pool: &PgPool,
    device_id: Uuid,
) -> Result<Vec<BindingRow>, DeviceError> {
    let rows = sqlx::query(
        "SELECT id, tenant_id, device_id, project_id, declared_repository_ids, \
                verified_repositories, bound_by, bound_at, unbound_at \
         FROM devenv.device_project_binding \
         WHERE device_id = $1 AND unbound_at IS NULL \
         ORDER BY bound_at DESC",
    )
    .bind(device_id)
    .fetch_all(pool)
    .await
    .map_err(|e| DeviceError::Db(e.to_string()))?;
    Ok(rows.into_iter().map(binding_from_row).collect())
}

/// 调用人是否是 Team Owner（`core.team_member.role` 是唯一事实源，
/// P4-ORG DR-ORG-02：admin 角色已取消，token 的 roles 不参与授权）。
pub(crate) async fn caller_is_team_owner(
    pool: &PgPool,
    caller: &HumanActor,
) -> Result<bool, DeviceError> {
    let role: Option<String> = sqlx::query_scalar(
        "SELECT tm.role FROM core.team_member tm            JOIN core.team t ON t.id = tm.team_id           WHERE t.tenant_id = $1 AND tm.principal_id = $2",
    )
    .bind(caller.tenant_id)
    .bind(caller.principal_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| DeviceError::Db(e.to_string()))?;
    Ok(role.as_deref() == Some("owner"))
}

/// 取设备的 owner（用于可见性判定）。
pub(crate) async fn device_owner(
    pool: &PgPool,
    device_id: Uuid,
) -> Result<Option<(Uuid, Uuid)>, DeviceError> {
    let row: Option<(Uuid, Uuid)> =
        sqlx::query_as("SELECT tenant_id, owner_principal_id FROM devenv.device WHERE id = $1")
            .bind(device_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| DeviceError::Db(e.to_string()))?;
    Ok(row)
}

fn validate_register(req: &RegisterDeviceRequest) -> Result<(), DeviceError> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(DeviceError::Validation("device name is required".into()));
    }
    if name.chars().count() > 120 {
        return Err(DeviceError::Validation("device name is too long".into()));
    }
    let platform = req.platform.trim();
    if !matches!(platform, "windows" | "macos" | "linux") {
        return Err(DeviceError::Validation(
            "platform must be one of windows/macos/linux".into(),
        ));
    }
    Ok(())
}

fn device_from_row(row: sqlx::postgres::PgRow) -> DeviceRow {
    DeviceRow {
        id: row.get("id"),
        tenant_id: row.get("tenant_id"),
        owner_principal_id: row.get("owner_principal_id"),
        name: row.get("name"),
        platform: row.get("platform"),
        os_version: row.get("os_version"),
        executor_kind: row.get("executor_kind"),
        executor_version: row.get("executor_version"),
        status: DeviceStatus::parse(row.get::<String, _>("status").as_str()),
        readiness: row
            .get::<Option<String>, _>("readiness")
            .map(|v| DeviceReadiness::parse(v.as_str())),
        last_check_at: row.get("last_check_at"),
        last_check: row.get("last_check"),
        revoked_at: row.get("revoked_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        is_runner: row.get("is_runner"),
        last_heartbeat_at: row.get("last_heartbeat_at"),
    }
}

fn binding_from_row(row: sqlx::postgres::PgRow) -> BindingRow {
    BindingRow {
        id: row.get("id"),
        tenant_id: row.get("tenant_id"),
        device_id: row.get("device_id"),
        project_id: row.get("project_id"),
        declared_repository_ids: uuid_list(row.get("declared_repository_ids")),
        verified_repositories: uuid_list(row.get("verified_repositories")),
        bound_by: row.get("bound_by"),
        bound_at: row.get("bound_at"),
        unbound_at: row.get("unbound_at"),
    }
}

/// JSONB 里的 uuid 数组 → `Vec<Uuid>`（非数组 / 元素非法时为空）。
pub(crate) fn uuid_list(value: serde_json::Value) -> Vec<Uuid> {
    serde_json::from_value(value).unwrap_or_default()
}
