//! 首管理员初始化探测（空库强制注册的启动检查 + 记号文件）。
//!
//! 职责（2026-11-08 用户裁决）：
//! 1. **启动完成后第一时间**判定系统是否已有「最大管理员」。判定源头 =
//!    lmms iam 的公开端点 `GET /v1/iam/auth/setup-status`（经 api-gateway，
//!    `setup_required = iam_users 空表`）。首用户注册即最大管理员：register
//!    空表门 + 首用户自动 `tenant_admin` 全权限码（上游语义，本侧不重复实现）。
//! 2. 已有用户 → **写空记号文件**（[`Probe::check_once`]），下次重启直接跳过
//!    网络检查；系统为空 → `Pending`（前端登录页据此强制引导到注册页）。
//!
//! 失败语义：检查失败一律 `Unknown`，**不阻塞启动**（短超时 + 后台重试），
//! 且 `setup_required` 只在 `Pending`（确证为空）时才为 true——Unknown 时
//! fail-open 保持登录页现状，不把网络抖动放大成引导误判。
//!
//! 公开端点 `GET /v1/pm/setup/status`（[`is_public_path`] 放行）供运维探测，
//! 前端强制注册的判定走网关 setup-status（同源真相），不依赖本端点。

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use axum::Json;
use serde_json::{Value, json};

/// 进程内探测句柄：`main.rs` 启动时 [`run_startup_check`]，处理器经 [`shared`]
/// 读取。全局单例与 `/install`、MCP hosts 同款模式（部署级配置，测试各自构造
/// [`Probe`]，不污染全局）。
static PROBE: OnceLock<Probe> = OnceLock::new();

/// 三态：Unknown（检查未成功）/ Initialized（已有最大管理员，记号已落）/
/// Pending（确证空系统，待注册）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupKind {
    Unknown,
    Initialized,
    Pending,
}

impl SetupKind {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => SetupKind::Initialized,
            2 => SetupKind::Pending,
            _ => SetupKind::Unknown,
        }
    }
    fn to_u8(self) -> u8 {
        match self {
            SetupKind::Unknown => 0,
            SetupKind::Initialized => 1,
            SetupKind::Pending => 2,
        }
    }
}

/// `AI_PM_SETUP_STATUS_URL` 的默认值：本机网关（dev）；compose 显式指向
/// `http://api-gateway:3000/...`。
const DEFAULT_STATUS_URL: &str = "http://127.0.0.1:3000/v1/iam/auth/setup-status";
/// `AI_PM_SETUP_MARKER_PATH` 的默认值：仓库 `data/` 已 gitignore（本地运行时产物目录）。
const DEFAULT_MARKER_PATH: &str = "data/setup-admin.marker";

/// 启动检查与状态探测。HTTP 客户端 no_proxy——与 JWKS 取数同配方（Windows
/// 系统代理残留值不得劫持内网调用）。
pub struct Probe {
    state: AtomicU8,
    marker_path: PathBuf,
    status_url: String,
    http: reqwest::Client,
    /// 串行化并发检查（启动任务与状态端点同时触发时只打一次网关）。
    check_lock: tokio::sync::Mutex<()>,
}

impl Probe {
    pub fn from_env() -> Self {
        let marker_path = std::env::var("AI_PM_SETUP_MARKER_PATH")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_MARKER_PATH));
        let status_url = std::env::var("AI_PM_SETUP_STATUS_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_STATUS_URL.to_string());
        let http = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap_or_default();
        Self::with_parts(marker_path, status_url, http)
    }

    /// 测试装配点：记号路径 / 状态 URL / 客户端全部显式。
    pub fn with_parts(marker_path: PathBuf, status_url: String, http: reqwest::Client) -> Self {
        Self {
            state: AtomicU8::new(SetupKind::Unknown.to_u8()),
            marker_path,
            status_url,
            http,
            check_lock: tokio::sync::Mutex::new(()),
        }
    }

    pub fn kind(&self) -> SetupKind {
        SetupKind::from_u8(self.state.load(Ordering::Relaxed))
    }

    fn set_kind(&self, kind: SetupKind) {
        self.state.store(kind.to_u8(), Ordering::Relaxed);
    }

    /// 单次检查：记号优先 → 网关 setup-status → 三态落位。返回检查后的状态。
    pub async fn check_once(&self) -> SetupKind {
        let _guard = self.check_lock.lock().await;
        if marker_present(&self.marker_path) {
            self.set_kind(SetupKind::Initialized);
            return SetupKind::Initialized;
        }
        match self.fetch_user_count().await {
            Ok(count) => {
                let kind = classify(count);
                if kind == SetupKind::Initialized {
                    if let Err(err) = write_marker(&self.marker_path) {
                        // 状态仍然落 Initialized（真相已确证），记号缺失只影响
                        // 下次启动多打一次网关。
                        tracing::warn!(error = %err, path = %self.marker_path.display(),
                            "setup marker write failed; startup will re-check next boot");
                    }
                }
                tracing::info!(?kind, user_count = count, "setup status checked");
                self.set_kind(kind);
                kind
            }
            Err(err) => {
                tracing::warn!(error = %err, url = %self.status_url, "setup status check failed");
                // Unknown 不覆盖已确证的状态（端点早已落位时网络抖动不回退）。
                if self.kind() == SetupKind::Unknown {
                    self.set_kind(SetupKind::Unknown);
                }
                self.kind()
            }
        }
    }

    /// 拉取网关 setup-status 的 `user_count`。
    async fn fetch_user_count(&self) -> Result<i64, String> {
        let resp = self
            .http
            .get(&self.status_url)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("status {}", resp.status()));
        }
        let body: Value = resp
            .json()
            .await
            .map_err(|e| format!("invalid json: {e}"))?;
        body.get("user_count")
            .and_then(Value::as_i64)
            .ok_or_else(|| "missing user_count field".to_string())
    }
}

/// 记号判存（独立函数便于测试）。
fn marker_present(path: &Path) -> bool {
    path.is_file()
}

/// 写空记号文件（幂等；父目录不存在则创建）。
fn write_marker(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::File::create(path).map(|_| ())
}

/// `user_count → 三态` 的纯分类（>0 = 已有最大管理员）。
fn classify(user_count: i64) -> SetupKind {
    if user_count > 0 {
        SetupKind::Initialized
    } else {
        SetupKind::Pending
    }
}

/// 进程级探测句柄（handler 用）。
pub fn shared() -> &'static Probe {
    PROBE.get_or_init(Probe::from_env)
}

/// 启动检查（`main.rs`，serve 前）：短超时首轮 + 失败转后台每 15s 重试，
/// 拿到非 Unknown 结论即停。`off` 模式（本地开发）只跑首轮，不挂后台重试
/// ——本机没有网关时 15s 一次的 warn 是纯噪音。
pub async fn run_startup_check(auth_mode_off: bool) {
    let probe = shared();
    let first = tokio::time::timeout(Duration::from_secs(3), probe.check_once()).await;
    if first.is_err() {
        tracing::warn!("startup setup check timed out (3s)");
    }
    if auth_mode_off || probe.kind() != SetupKind::Unknown {
        return;
    }
    tokio::spawn(async move {
        let probe = shared();
        loop {
            tokio::time::sleep(Duration::from_secs(15)).await;
            if probe.check_once().await != SetupKind::Unknown {
                break;
            }
        }
    });
}

/// `GET /v1/pm/setup/status`（公开，[`crate::auth::is_public_path`] 放行）：
/// 未落定（Unknown/Pending）时带一次实时复查。响应：
/// `{ setup_required, initialized }` —— `setup_required` 仅在确证空系统
/// （Pending）时为 true。
pub async fn status_handler() -> Json<Value> {
    let probe = shared();
    if probe.kind() != SetupKind::Initialized {
        probe.check_once().await;
    }
    let kind = probe.kind();
    Json(json!({
        "setup_required": kind == SetupKind::Pending,
        "initialized": kind == SetupKind::Initialized,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe_with_marker(marker: Option<&Path>) -> Probe {
        let dir = std::env::temp_dir().join(format!("pm-api-setup-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = match marker {
            Some(p) => p.to_path_buf(),
            None => dir.join("absent.marker"),
        };
        Probe::with_parts(
            path,
            "http://127.0.0.1:1/v1/iam/auth/setup-status".to_string(),
            reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_millis(200))
                .build()
                .expect("client"),
        )
    }

    #[test]
    fn classify_maps_user_count() {
        assert_eq!(classify(0), SetupKind::Pending);
        assert_eq!(classify(1), SetupKind::Initialized);
        assert_eq!(classify(42), SetupKind::Initialized);
        assert_eq!(classify(-1), SetupKind::Pending);
    }

    #[test]
    fn marker_roundtrip_is_idempotent() {
        let dir = std::env::temp_dir().join(format!("pm-api-setup-marker-{}", std::process::id()));
        let path = dir.join("nested").join("setup-admin.marker");
        assert!(!marker_present(&path));
        write_marker(&path).expect("first write");
        write_marker(&path).expect("second write (idempotent)");
        assert!(marker_present(&path));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn check_once_short_circuits_on_marker_without_http() {
        // 记号存在 ⇒ 直接 Initialized，即使 URL 不可达也不打网络。
        let dir = std::env::temp_dir().join(format!("pm-api-setup-short-{}", std::process::id()));
        let path = dir.join("setup-admin.marker");
        write_marker(&path).expect("marker");
        let probe = probe_with_marker(Some(&path));
        assert_eq!(probe.check_once().await, SetupKind::Initialized);
        assert_eq!(probe.kind(), SetupKind::Initialized);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn unreachable_endpoint_keeps_unknown() {
        // 无记号 + URL 不可达（127.0.0.1:1 连接拒绝）⇒ Unknown，不误判。
        let probe = probe_with_marker(None);
        assert_eq!(probe.check_once().await, SetupKind::Unknown);
        assert_eq!(probe.kind(), SetupKind::Unknown);
    }
}
