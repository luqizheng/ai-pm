//! 自检上报与就绪判定（T-088）。
//!
//! 关键边界（Codex Bootstrap V1.3 §7.3~§7.5、评审 P1-1/P1-2）：
//!
//! - **Ready 只描述设备自身**——项目权限**不**纳入 Device 的 Ready；项目可用性
//!   是「绑定 × 当前调用人成员资格」的派生值，按调用人实时计算、不落库。
//! - **服务端不采信客户端的 "ready" 声明**——就绪结论由服务端按可验证事实算出。
//! - `verified_repositories` 只表示**服务端确认的仓库身份与项目归属**，**不代表**
//!   服务端验证过远程机器上的本地工作目录或 Git 工作区可用（后者由客户端自检，
//!   作为客户端项记录在 `last_check` 里）。

use sqlx::PgPool;
use uuid::Uuid;

use super::binding::project_repository_ids;
use super::{DeviceError, DeviceReadiness, DeviceRow, device_owner, fetch_device, list_bindings};
use crate::authz::require_project_member;
use crate::caller::HumanActor;

/// 客户端上报的自检结果。
#[derive(Debug, Clone, Default)]
pub struct DeviceChecks {
    /// 技能包已安装（客户端项）。
    pub installed: bool,
    /// 服务端连接检查通过（客户端项）。
    pub connected: bool,
    /// 核心 MCP 工具可正常调用（客户端项）。
    pub tool: bool,
    /// 客户端自报本地可达的仓库 id 集合（服务端会与项目归属求交集）。
    pub accessible_repositories: Vec<Uuid>,
    /// 客户端证据（本地工作目录 / Git 状态等，仅记录不判定）。
    pub evidence: Option<serde_json::Value>,
}

/// 上报自检 → 写 `readiness` 与 `last_check`。
pub async fn report_checks(
    pool: &PgPool,
    caller: &HumanActor,
    device_id: Uuid,
    checks: &DeviceChecks,
) -> Result<DeviceRow, DeviceError> {
    let (tenant_id, owner) = device_owner(pool, device_id)
        .await?
        .ok_or(DeviceError::NotFound)?;
    if tenant_id != caller.tenant_id || owner != caller.principal_id {
        return Err(DeviceError::NotFound);
    }
    let device = fetch_device(pool, device_id)
        .await?
        .ok_or(DeviceError::NotFound)?;
    if device.status == super::DeviceStatus::Revoked {
        return Err(DeviceError::Validation(
            "device is revoked; re-register before reporting checks".into(),
        ));
    }

    let bindings = list_bindings(pool, device_id).await?;

    // 逐绑定计算服务端确认的仓库集合：客户端自报 ∩ 该项目的仓库 id。
    let mut verified_by_binding: Vec<(Uuid, Vec<Uuid>)> = Vec::with_capacity(bindings.len());
    for binding in &bindings {
        let member = require_project_member(
            pool,
            caller.principal_id,
            caller.tenant_id,
            binding.project_id,
        )
        .await
        .is_ok();
        if !member {
            // 调用人不是该项目的成员 → 该绑定的仓库不进入验证结果。
            verified_by_binding.push((binding.id, Vec::new()));
            continue;
        }
        let project_repos = project_repository_ids(pool, binding.project_id).await?;
        let verified: Vec<Uuid> = checks
            .accessible_repositories
            .iter()
            .filter(|r| project_repos.contains(*r))
            .copied()
            .collect();
        verified_by_binding.push((binding.id, verified));
    }

    let total_verified: usize = verified_by_binding.iter().map(|(_, v)| v.len()).sum();
    let total_declared: usize = bindings
        .iter()
        .map(|b| b.declared_repository_ids.len())
        .sum();

    let readiness = compute_readiness(checks, total_verified, total_declared);

    let last_check = serde_json::json!({
        "installed": checks.installed,
        "connected": checks.connected,
        "tool": checks.tool,
        "verified_repository_count": total_verified,
        "declared_repository_count": total_declared,
        "evidence": checks.evidence.clone(),
    });

    let mut tx = pool.begin().await.map_err(|e| DeviceError::Db(e.to_string()))?;

    for (binding_id, verified) in &verified_by_binding {
        let json = serde_json::to_value(verified).map_err(|e| DeviceError::Db(e.to_string()))?;
        sqlx::query(
            "UPDATE devenv.device_project_binding SET verified_repositories = $2 WHERE id = $1",
        )
        .bind(binding_id)
        .bind(json)
        .execute(&mut *tx)
        .await
        .map_err(|e| DeviceError::Db(e.to_string()))?;
    }

    sqlx::query(
        "UPDATE devenv.device \
         SET readiness = $2, last_check = $3, last_check_at = now(), updated_at = now() \
         WHERE id = $1",
    )
    .bind(device_id)
    .bind(readiness.as_str())
    .bind(&last_check)
    .execute(&mut *tx)
    .await
    .map_err(|e| DeviceError::Db(e.to_string()))?;

    tx.commit().await.map_err(|e| DeviceError::Db(e.to_string()))?;

    fetch_device(pool, device_id)
        .await?
        .ok_or(DeviceError::NotFound)
}

/// 就绪判定（服务端算，**不采信客户端的 "ready" 声明**）。
///
/// 层级：installed < connected < authenticated < ready；
/// `failed` = 安装未完成；`degraded` = 能力齐备但仓库验证不全。
fn compute_readiness(
    checks: &DeviceChecks,
    verified_count: usize,
    declared_count: usize,
) -> DeviceReadiness {
    if !checks.installed {
        return DeviceReadiness::Failed;
    }
    if !checks.connected {
        return DeviceReadiness::Installed;
    }
    if !checks.tool {
        return DeviceReadiness::Connected;
    }
    if declared_count == 0 {
        // 无声明仓库：连接与工具都通，但尚未形成可执行的项目绑定面。
        return DeviceReadiness::Authenticated;
    }
    if verified_count == 0 {
        return DeviceReadiness::Authenticated;
    }
    if verified_count < declared_count {
        return DeviceReadiness::Degraded;
    }
    DeviceReadiness::Ready
}
