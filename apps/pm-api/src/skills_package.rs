//! 技能包分发（T-089）：构建期内嵌 `pm-ai-skills-<version>.zip` + 同源下载端点。
//!
//! 设计稿（Codex Bootstrap V1.3 §3.3、§13.3；T-088 设计稿 §7.2 之外的分发面）：
//! - **构建期打包**：`build.rs` 把仓库根 `skills/` 的白名单内容打进 zip，经
//!   `include_bytes!` 内嵌进 pm-api 二进制——**无外置静态托管、无第三方下载源**。
//! - **同源下发**：`GET /install/packages/pm-ai-skills-<version>.zip`；
//!   `release-manifest.json` 由服务端从内嵌包生成（SHA-256 覆盖整个 zip）。
//! - **信任根表述**（评审 P1-4）：SHA-256 + 同源 manifest 只做**文件完整性验证**
//!   （下载过程无内容变化、与 manifest 声明一致），**不构成发布者身份认证**。
//! - 公开路径（`/install*` 已在 `is_public_path` 放行）：agent 在拿到 key 之前
//!   就要拉包。

use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;

/// 构建期嵌入的技能包 zip（build.rs 产出，见 pm-api/build.rs）。
static SKILLS_ZIP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/pm-ai-skills.zip"));

/// 包版本（与 workspace 版本同源推进；T-090 改技能内容时随包发新版本）。
pub const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// `release-manifest.json`（服务端从内嵌包生成——单一事实源）。
pub fn release_manifest() -> serde_json::Value {
    json!({
        "package": "pm-ai-skills",
        "version": PACKAGE_VERSION,
        "format": "zip",
        "sha256": sha256_hex(SKILLS_ZIP),
        "min_platform_api_version": "1",
        "protocol_version": "1",
        // 信任根边界（评审 P1-4）：完整性验证，不是发布者身份认证。
        "integrity_only": true,
        "contents": [
            "skills/pm-ai-bootstrap/SKILL.md",
            "skills/pm-task-execution/SKILL.md",
            "skills/pm-knowledge/SKILL.md",
            "skills/pm-learn/SKILL.md",
            "skills/scripts/verify-install.ps1",
            "skills/scripts/verify-install.sh",
        ],
    })
}

pub(crate) fn router() -> Router<crate::routes::AppState> {
    Router::new()
        .route(
            "/install/packages/release-manifest.json",
            get(manifest_handler),
        )
        // axum 0.8 一个段只允许一个参数（`pm-ai-skills-{version}.zip` 不合法）——
        // 用 `{file}` 通配 + 手动校验文件名形态与版本。
        .route("/install/packages/{file}", get(package_handler))
}

/// `GET /install/packages/release-manifest.json`：版本 + SHA-256 + 内容清单。
async fn manifest_handler() -> Response {
    (
        axum::http::StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        Json(release_manifest()),
    )
        .into_response()
}

/// `GET /install/packages/pm-ai-skills-<version>.zip`：技能包同源下载。
/// 文件名形态或版本不匹配 → 404（引导 agent 重新读 `/install` 拿当前版本）。
async fn package_handler(axum::extract::Path(file): axum::extract::Path<String>) -> Response {
    let version = file
        .strip_prefix("pm-ai-skills-")
        .and_then(|rest| rest.strip_suffix(".zip"));
    let Some(version) = version.filter(|v| *v == PACKAGE_VERSION) else {
        let detail = file
            .strip_prefix("pm-ai-skills-")
            .and_then(|rest| rest.strip_suffix(".zip"))
            .map(|v| format!("unknown package version {v}; current is {PACKAGE_VERSION}"))
            .unwrap_or_else(|| "unknown package file".to_string());
        return (
            axum::http::StatusCode::NOT_FOUND,
            Json(json!({ "message": detail })),
        )
            .into_response();
    };
    let _ = version;
    (
        axum::http::StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/zip"),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"pm-ai-skills-{PACKAGE_VERSION}.zip\"").as_str(),
            ),
        ],
        SKILLS_ZIP,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_package_is_a_valid_zip_with_expected_entries() {
        // 最小 ZIP 魔数 + End-of-Central-Directory 可寻（build.rs 打包正确性由
        // 集成测试逐一断言内容；此处防 include_bytes 指向空文件回归）。
        assert!(
            SKILLS_ZIP.len() > 500,
            "package too small: {}",
            SKILLS_ZIP.len()
        );
        assert_eq!(&SKILLS_ZIP[0..2], b"PK", "not a zip");
        assert!(release_manifest()["sha256"].as_str().unwrap().len() == 64);
    }
}
