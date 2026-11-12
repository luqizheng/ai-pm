//! SeeCode Adapter（服务端 SVG 出图器，Adapter B）。
//!
//! 依据 ADR-VIZ-002 M-2 修订：SeeCode 是**服务端出图器**，非核心运行依赖。
//! - 只以子进程方式调用 `node tools/seecode/scripts/seecode.mjs`
//! - 只产出 **SVG**（HTML 中间产物不向浏览器投递）
//! - 全程经 `viz_render_sandbox`（零凭据 / 输入不进命令行 / 超时 / 输出上限）
//! - 产物由 `artifact_safety::inspect_svg` 静态化检查把闸（在 `render_job` 中）
//!
//! ## 两步渲染
//!
//! SeeCode CLI 的形态是 `render`（spec → HTML）再 `export`（HTML → SVG）。
//! 两步必须在**同一个沙箱工作目录**内完成（中间 HTML 要传递），
//! 因此本适配器自管临时目录并复用给两次沙箱调用。

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;

use pm_application::visualization::adapter_contract::{
    OutputFormat, RenderAdapter, RenderError, RenderOutput,
};
use pm_application::visualization::seecode_compiler;
use pm_domain::visualization::VizSpec;

use crate::viz_render_sandbox::{run_sandboxed, SandboxConfig, SandboxError};

/// SeeCode Adapter。
pub struct SeeCodeAdapter {
    /// `tools/seecode/scripts/seecode.mjs` 的路径。
    cli: PathBuf,
    /// 单次子进程执行超时。
    timeout: Duration,
    /// stdout 上限。
    max_output_bytes: usize,
}

impl SeeCodeAdapter {
    /// 以默认路径（`tools/seecode/scripts/seecode.mjs`）构造。
    pub fn new(cli: impl Into<PathBuf>) -> Self {
        Self {
            cli: cli.into(),
            timeout: Duration::from_secs(60),
            max_output_bytes: 8 * 1024 * 1024,
        }
    }

    fn base_config(&self, args: Vec<String>, workdir: PathBuf) -> SandboxConfig {
        SandboxConfig {
            program: node_program(),
            args,
            timeout: self.timeout,
            max_output_bytes: self.max_output_bytes,
            extra_env: vec![],
            workdir: Some(workdir),
            payload_file: None,
        }
    }

    /// 跑一步 CLI，返回 stdout（JSON 单行）。
    async fn run_cli(
        &self,
        args: Vec<String>,
        workdir: &std::path::Path,
        payload_file: Option<(&str, &[u8])>,
    ) -> Result<Vec<u8>, SandboxError> {
        let mut config = self.base_config(args, workdir.to_path_buf());
        let payload: Vec<u8> = match payload_file {
            Some((filename, bytes)) => {
                config.payload_file = Some(filename.to_string());
                bytes.to_vec()
            }
            None => Vec::new(),
        };
        run_sandboxed(&config, &payload).await
    }
}

/// Node 可执行名（Windows 是 `node.exe`，由 PATH 解析）。
fn node_program() -> String {
    "node".to_string()
}

/// 从 CLI 的 JSON 单行输出里取 `out` 字段（相对/绝对路径都接受）。
fn parse_out_path(stdout: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(stdout);
    let line = text.lines().find(|l| l.trim_start().starts_with('{'))?;
    let value: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    if value.get("ok").and_then(|v| v.as_bool()) == Some(false) {
        return None;
    }
    value.get("out").and_then(|v| v.as_str()).map(str::to_string)
}

#[async_trait]
impl RenderAdapter for SeeCodeAdapter {
    fn id(&self) -> &'static str {
        "seecode"
    }

    fn version(&self) -> &'static str {
        // 与 tools/seecode/VERSION 保持一致；升级时同步
        "0.6.3"
    }

    fn supported_formats(&self) -> &'static [OutputFormat] {
        // 只承诺 SVG（HTML 中间产物不对外）
        &[OutputFormat::Svg]
    }

    fn render(&self, spec: &VizSpec, format: OutputFormat) -> Result<RenderOutput, RenderError> {
        if format != OutputFormat::Svg {
            return Err(RenderError::Rejected(format!(
                "SeeCode Adapter 只产出 SVG（请求 {}）",
                format.as_str()
            )));
        }
        // 契约 trait 是同步的（八项断言在单测里同步跑），而子进程必须异步——
        // 用独立线程跑当前 runtime 的 block_on，避免与调用方 runtime 冲突。
        let compiled = seecode_compiler::compile(spec)
            .map_err(|e| RenderError::Rejected(e.to_string()))?;
        let adapter = SeeCodeAdapter {
            cli: self.cli.clone(),
            timeout: self.timeout,
            max_output_bytes: self.max_output_bytes,
        };
        let spec_json = serde_json::to_vec(&compiled.spec)
            .map_err(|e| RenderError::Failed(format!("serialize spec failed: {e}")))?;

        let handle = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| RenderError::Failed(format!("build runtime failed: {e}")))?;
            runtime.block_on(async move { adapter.render_async(spec_json).await })
        });
        handle
            .join()
            .map_err(|_| RenderError::Failed("render thread panicked".to_string()))?
    }
}

impl SeeCodeAdapter {
    /// 实际的异步渲染（两步：render → export svg）。
    async fn render_async(&self, spec_json: Vec<u8>) -> Result<RenderOutput, RenderError> {
        let workdir = std::env::temp_dir()
            .join("ai-pm-viz-seecode")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&workdir).map_err(|e| {
            RenderError::Failed(format!("create workdir {} failed: {e}", workdir.display()))
        })?;

        // 结果在 return 前读走；workdir 由本函数负责清理
        let result = self.render_two_steps(&workdir, &spec_json).await;
        let _ = std::fs::remove_dir_all(&workdir);
        result
    }

    async fn render_two_steps(
        &self,
        workdir: &std::path::Path,
        spec_json: &[u8],
    ) -> Result<RenderOutput, RenderError> {
        let cli = self.cli.to_string_lossy().to_string();

        // ① render：spec.json → diagram.html（spec 经文件传入，不进命令行）
        let render_stdout = self
            .run_cli(
                vec![cli.clone(), "render".to_string(), "spec.json".to_string()],
                workdir,
                Some(("spec.json", spec_json)),
            )
            .await
            .map_err(map_sandbox_error)?;

        let html_path = parse_out_path(&render_stdout)
            .ok_or_else(|| {
                RenderError::Failed(format!(
                    "render 未返回 out 路径：{}",
                    String::from_utf8_lossy(&render_stdout)
                ))
            })?;
        let html_name = std::path::Path::new(&html_path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .ok_or_else(|| RenderError::Failed("render 返回的路径无文件名".to_string()))?;

        // ② export：diagram.html → diagram.svg（纯 Node，无需浏览器）
        let _ = self
            .run_cli(
                vec![
                    cli,
                    "export".to_string(),
                    html_name,
                    "--formats".to_string(),
                    "svg".to_string(),
                ],
                workdir,
                None,
            )
            .await
            .map_err(map_sandbox_error)?;

        // ③ 读回 SVG（export 产物与 HTML 同名同目录）
        let svg_path = workdir.join(
            std::path::Path::new(&html_path)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "diagram".to_string())
                + ".svg",
        );
        let content = std::fs::read_to_string(&svg_path).map_err(|e| {
            RenderError::Failed(format!("read {} failed: {e}", svg_path.display()))
        })?;

        Ok(RenderOutput {
            format: OutputFormat::Svg,
            content,
        })
    }
}

fn map_sandbox_error(error: SandboxError) -> RenderError {
    match error {
        SandboxError::Timeout(duration) => {
            RenderError::Failed(format!("渲染超时（{}s）", duration.as_secs()))
        }
        SandboxError::OutputTooLarge { limit } => {
            RenderError::Failed(format!("渲染输出超过上限 {limit} 字节"))
        }
        SandboxError::NonZeroExit { code, stderr } => {
            RenderError::Failed(format!("渲染器退出码 {code}：{stderr}"))
        }
        SandboxError::Internal(message) => RenderError::Unavailable(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn out_path_is_parsed_from_cli_json_line() {
        let stdout = br#"{"ok":true,"out":"C:\\tmp\\x\\attack.html","type":"architecture"}"#;
        assert_eq!(
            parse_out_path(stdout).as_deref(),
            Some("C:\\tmp\\x\\attack.html")
        );
    }

    #[test]
    fn failed_cli_output_yields_none() {
        let stdout = br#"{"ok":false,"problems":[{"code":"E_SPEC","at":"nodes[0]"}]}"#;
        assert_eq!(parse_out_path(stdout), None);
    }

    #[test]
    fn html_formats_are_rejected() {
        let adapter = SeeCodeAdapter::new("tools/seecode/scripts/seecode.mjs");
        let spec = pm_domain::visualization::VizSpec {
            spec_version: "1.0".to_string(),
            slot_key: "s".to_string(),
            view: pm_domain::visualization::VizView {
                kind: "trace-graph".to_string(),
                focus: vec![],
                lenses: vec![],
                layout_hint: None,
            },
            nodes: vec![],
            edges: vec![],
            meta: pm_domain::visualization::VizMeta {
                title: None,
                lang: None,
                budget: pm_domain::visualization::Budget::Balanced,
                nature: None,
                registry_version: None,
                provenance: None,
            },
        };
        let result = adapter.render(&spec, OutputFormat::Html);
        assert!(matches!(result, Err(RenderError::Rejected(_))));
        // 空 spec 也应被显式拒绝（不是崩溃）
        let result = adapter.render(&spec, OutputFormat::Svg);
        assert!(matches!(result, Err(RenderError::Rejected(_))));
    }

    #[test]
    fn adapter_identity_is_stable() {
        let adapter = SeeCodeAdapter::new("x");
        assert_eq!(adapter.id(), "seecode");
        assert_eq!(adapter.version(), "0.6.3");
        assert_eq!(adapter.supported_formats(), &[OutputFormat::Svg]);
    }
}
