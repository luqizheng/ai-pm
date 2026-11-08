//! T-089 技能包同源分发：`/install/packages/*` + `/install/{tool}` JSON 的
//! `bootstrap` 段。
//!
//! 覆盖（公开路径——agent 在拿到 key 之前就要拉包）：
//! - manifest 可读：版本、SHA-256（64 hex）、integrity_only 标记、内容清单；
//! - zip 下载：PK 魔数、Content-Disposition、当前版本可下、未知版本 404；
//! - `/install/{tool}?format=json` 带 `bootstrap` 段（package_url 指向当前
//!   版本、endpoints 齐、key_handling 红线在场）；Markdown 形态不带该段。
//! - zip 内容逐一核对：六个白名单条目齐全、SKILL.md 含 frontmatter、脚本含
//!   用法行——构建期打包正确性的端到端证据。

use std::io::Read;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use pm_api::routes::router;
use tower::ServiceExt;

fn app(pool: sqlx::PgPool) -> axum::Router {
    router(pool)
}

async fn get(pool: sqlx::PgPool, uri: &str, accept: Option<&str>) -> (StatusCode, Vec<u8>) {
    let mut builder = Request::builder().method("GET").uri(uri);
    if let Some(a) = accept {
        builder = builder.header("accept", a);
    }
    let response = app(pool)
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    (status, bytes)
}

fn manifest_value(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).expect("manifest json")
}

#[sqlx::test(migrations = "../../migrations")]
async fn release_manifest_is_public_and_complete(pool: sqlx::PgPool) {
    let (status, bytes) = get(pool, "/install/packages/release-manifest.json", None).await;
    assert_eq!(status, StatusCode::OK);
    let v = manifest_value(&bytes);
    assert_eq!(v["package"], serde_json::json!("pm-ai-skills"));
    let sha = v["sha256"].as_str().expect("sha256");
    assert_eq!(sha.len(), 64);
    assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(
        v["integrity_only"],
        serde_json::json!(true),
        "评审 P1-4：只作完整性验证"
    );
    assert!(v["version"].as_str().is_some());
    let contents = v["contents"].as_array().expect("contents");
    assert_eq!(contents.len(), 6, "四 SKILL + 两脚本");
}

#[sqlx::test(migrations = "../../migrations")]
async fn package_zip_downloads_and_unknown_version_404s(pool: sqlx::PgPool) {
    // 读 manifest 拿当前版本（端到端：manifest 声明的版本必须可下载）
    let (_, mbytes) = get(
        pool.clone(),
        "/install/packages/release-manifest.json",
        None,
    )
    .await;
    let version = manifest_value(&mbytes)["version"]
        .as_str()
        .unwrap()
        .to_string();

    let (status, zip) = get(
        pool.clone(),
        &format!("/install/packages/pm-ai-skills-{version}.zip"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(zip.len() > 500, "zip too small: {}", zip.len());
    assert_eq!(&zip[0..2], b"PK", "not a zip");

    // 未知版本 → 404（引导重读 /install）
    let (status, _) = get(pool, "/install/packages/pm-ai-skills-99.0.0.zip", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn zip_contains_all_whitelisted_entries(pool: sqlx::PgPool) {
    let (_, mbytes) = get(
        pool.clone(),
        "/install/packages/release-manifest.json",
        None,
    )
    .await;
    let version = manifest_value(&mbytes)["version"]
        .as_str()
        .unwrap()
        .to_string();
    let (_, zip) = get(
        pool,
        &format!("/install/packages/pm-ai-skills-{version}.zip"),
        None,
    )
    .await;

    // 极简 STORE-zip 读取器：扫 local file headers（PK\x03\x04），文件名紧随
    // 固定 30 字节头；数据长度取自头内 little-endian 字段（store 方法 = 0）。
    let mut names: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i + 4 <= zip.len() {
        if &zip[i..i + 4] == b"PK\x03\x04" {
            let method = u16::from_le_bytes([zip[i + 8], zip[i + 9]]);
            let size =
                u32::from_le_bytes([zip[i + 18], zip[i + 19], zip[i + 20], zip[i + 21]]) as usize;
            let name_len = u16::from_le_bytes([zip[i + 26], zip[i + 27]]) as usize;
            let extra_len = u16::from_le_bytes([zip[i + 28], zip[i + 29]]) as usize;
            let name = String::from_utf8_lossy(&zip[i + 30..i + 30 + name_len]).into_owned();
            assert_eq!(method, 0, "store method expected for {name}");
            names.push(name);
            i += 30 + name_len + extra_len + size;
        } else {
            break;
        }
    }
    let expected: &[&str] = &[
        "skills/pm-ai-bootstrap/SKILL.md",
        "skills/pm-task-execution/SKILL.md",
        "skills/pm-knowledge/SKILL.md",
        "skills/pm-learn/SKILL.md",
        "skills/scripts/verify-install.ps1",
        "skills/scripts/verify-install.sh",
    ];
    assert_eq!(names.len(), expected.len(), "{names:?}");
    for (got, want) in names.iter().zip(expected) {
        assert_eq!(got, want);
    }

    // 抽验内容：bootstrap SKILL.md 有 frontmatter；verify-install.sh 有用法行
    let mut cursor = std::io::Cursor::new(zip);
    let mut archive = zip_bytes(&mut cursor);
    let bootstrap = archive
        .remove("skills/pm-ai-bootstrap/SKILL.md")
        .expect("entry");
    assert!(String::from_utf8_lossy(&bootstrap).starts_with("---"));
    // T-090 正文进了包（不是占位骨架）
    assert!(
        String::from_utf8_lossy(&bootstrap).contains("七步生命周期"),
        "bootstrap SKILL.md should carry the seven-step lifecycle body"
    );
    let script = archive
        .remove("skills/scripts/verify-install.sh")
        .expect("entry");
    let script = String::from_utf8_lossy(&script);
    assert!(script.contains("verify-install"), "usage line missing");
}

/// 把 store-method zip 的全部条目读进 map（测试用极简实现，见上）。
fn zip_bytes(cursor: &mut std::io::Cursor<Vec<u8>>) -> std::collections::HashMap<String, Vec<u8>> {
    let mut out = std::collections::HashMap::new();
    let mut buf = Vec::new();
    cursor.read_to_end(&mut buf).unwrap();
    let mut i = 0usize;
    while i + 4 <= buf.len() && &buf[i..i + 4] == b"PK\x03\x04" {
        let size =
            u32::from_le_bytes([buf[i + 18], buf[i + 19], buf[i + 20], buf[i + 21]]) as usize;
        let name_len = u16::from_le_bytes([buf[i + 26], buf[i + 27]]) as usize;
        let extra_len = u16::from_le_bytes([buf[i + 28], buf[i + 29]]) as usize;
        let name = String::from_utf8_lossy(&buf[i + 30..i + 30 + name_len]).into_owned();
        let start = i + 30 + name_len + extra_len;
        out.insert(name, buf[start..start + size].to_vec());
        i = start + size;
    }
    out
}

#[sqlx::test(migrations = "../../migrations")]
async fn install_tool_json_carries_bootstrap_section(pool: sqlx::PgPool) {
    let (status, bytes) = get(
        pool.clone(),
        "/install/codex?format=json",
        Some("application/json"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v = manifest_value(&bytes);

    let boot = &v["bootstrap"];
    assert!(boot.is_object(), "bootstrap section missing: {v}");
    // package_url 指向当前版本
    let version = {
        let (_, mbytes) = get(pool, "/install/packages/release-manifest.json", None).await;
        manifest_value(&mbytes)["version"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let url = boot["package_url"].as_str().unwrap();
    assert!(
        url.ends_with(&format!("/install/packages/pm-ai-skills-{version}.zip")),
        "{url}"
    );
    // 端点清单齐
    for key in ["devices", "bindings", "checks", "revoke"] {
        assert!(boot["endpoints"][key].is_string(), "endpoint {key} missing");
    }
    // key 红线在场
    assert!(boot["key_handling"].as_str().unwrap().contains("当面提供"));
    // release_manifest 内嵌
    assert_eq!(
        boot["release_manifest"]["package"],
        serde_json::json!("pm-ai-skills")
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn install_tool_markdown_has_no_bootstrap_section(pool: sqlx::PgPool) {
    let (status, bytes) = get(pool, "/install/codex", None).await;
    assert_eq!(status, StatusCode::OK);
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        !text.contains("bootstrap"),
        "markdown 是给人看的指引，不含机器段"
    );
    assert!(text.contains("/mcp"), "仍含 MCP 端点");
}
