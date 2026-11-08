//! D-2=c：领取型 Runner 的派发可用性真源（设备级心跳）。
//!
//! 语义定版（实施基线 §三）：
//! - `is_runner` 是**静态事实**：worker 对领取型项目恒不领（claim 候选发现排除，
//!   在 `run_dispatch/claim.rs`），与心跳新鲜度无关——避免「Runner 离线 → worker
//!   抢活」与 Runner A3 离线恢复的竞态。
//! - `last_heartbeat_at` 是**可用性真源**：已认证设备活动（领取/列表/上报/续领/
//!   发现/命令轮询）由 MCP 层服务端归因刷新。本期只落库与定义新鲜度语义，
//!   **不门控 worker 领取**——「离线兜底接管」是 V2 调度入口闭合的裁决范围。

use sqlx::PgPool;
use uuid::Uuid;

/// 刷新设备级心跳：仅对 `is_runner` 设备生效（非 Runner 设备 no-op）。
///
/// best-effort：失败调用方只日志，不得阻断业务主流程（可观测性信号）。
pub async fn refresh_runner_heartbeat(pool: &PgPool, device_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE devenv.device SET last_heartbeat_at = now(), updated_at = now() \
          WHERE id = $1 AND is_runner",
    )
    .bind(device_id)
    .execute(pool)
    .await
    .map(|_| ())
}

/// 心跳新鲜度阈值（秒）：`AI_PM_RUNNER_AVAILABILITY_STALE_SECS`，缺省 300。
/// Runner 空闲期每 ~10s 轮询 `pm_list_tasks`（心跳归因点），300s ≈ 30 次漏拍。
pub fn runner_availability_stale_secs() -> i64 {
    std::env::var("AI_PM_RUNNER_AVAILABILITY_STALE_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|v| *v > 0)
        .unwrap_or(300)
}

/// 可用性判定（纯函数，测试注入）：心跳距今 ≤ 阈值 = 可用；**从未心跳 = 不可用**
/// （fail-closed：不得把从未证明在线过的 Runner 当作可用）。
pub fn is_runner_available(
    last_heartbeat_at: Option<chrono::DateTime<chrono::Utc>>,
    stale_secs: i64,
) -> bool {
    match last_heartbeat_at {
        Some(at) => chrono::Utc::now() - at <= chrono::Duration::seconds(stale_secs),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn availability_fresh_and_stale() {
        let now = chrono::Utc::now();
        assert!(is_runner_available(Some(now), 300));
        assert!(is_runner_available(
            Some(now - chrono::Duration::seconds(299)),
            300
        ));
        assert!(!is_runner_available(
            Some(now - chrono::Duration::seconds(301)),
            300
        ));
    }

    #[test]
    fn availability_never_heartbeated_is_unavailable() {
        assert!(!is_runner_available(None, 300));
    }
}
