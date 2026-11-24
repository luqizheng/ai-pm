//! 渲染 job（V0-9 骨架）——产物生成链路。
//!
//! 链路：artifact(published/draft) → 渲染 Adapter → render_output 落库。
//!
//! ## 与 vendor SeeCode 的关系（ADR-VIZ-002 M-2 修订）
//!
//! 本模块**不 vendor 任何渲染器**：它定义 [`RenderAdapter`]（已在
//! `adapter_contract` 里声明）的调用编排 + 产物落库 + 失败降级。
//! 真实渲染器（SeeCode CLI 子进程）作为 Adapter 实现后注册进来——
//! 未注册时 job 明确失败并降级（前端已有表格降级通道），链不断。
//!
//! ## 失败策略（R-05）
//!
//! **失败的渲染不落 `viz.render_output`**——失败产物无消费价值且放大容量负担；
//! 失败信息进 job 记录与审计。已存在的旧产物保持在库（不因一次渲染失败而消失）。

use uuid::Uuid;

use crate::visualization::adapter_contract::{OutputFormat, RenderAdapter};
use crate::visualization::artifact_safety::inspect_svg;

/// 渲染 job 的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderOutcome {
    /// 渲染并落库成功（`byte_size` 为产物字节数）。
    Rendered {
        format: OutputFormat,
        byte_size: usize,
        content_hash: String,
    },
    /// 渲染器显式拒绝（超预算 / 不支持该图类型）——降级，不算故障。
    Rejected(String),
    /// 渲染失败（适配器异常 / 不可用）——进 job 失败路径。
    Failed(String),
}

/// 执行一次渲染：调 Adapter → **静态化检查**（SVG 必经）→ 落库。
///
/// `pool` 为 `None` 时只做渲染与检查（测试/预演用，不落库）。
pub async fn render_artifact(
    pool: Option<&sqlx::PgPool>,
    adapter: &dyn RenderAdapter,
    artifact_id: Uuid,
    tenant_id: Uuid,
    project_id: Uuid,
    spec: &pm_domain::visualization::VizSpec,
    format: OutputFormat,
) -> RenderOutcome {
    if !adapter.supported_formats().contains(&format) {
        return RenderOutcome::Rejected(format!(
            "渲染器 {} 不支持 {} 格式",
            adapter.id(),
            format.as_str()
        ));
    }

    let output = match adapter.render(spec, format) {
        Ok(output) => output,
        Err(crate::visualization::adapter_contract::RenderError::Rejected(reason)) => {
            return RenderOutcome::Rejected(reason);
        }
        Err(error) => return RenderOutcome::Failed(error.to_string()),
    };

    // SVG 必须过静态化检查（AC-7′：零脚本、零外链）——
    // 这是产物入库前的最后一道闸，渲染器缺陷不能穿透到用户浏览器。
    if format == OutputFormat::Svg {
        let report = inspect_svg(&output.content);
        if !report.is_clean() {
            return RenderOutcome::Failed(format!("SVG 未通过静态化检查：{}", report.summary()));
        }
    }

    let bytes = output.content.as_bytes();
    let content_hash = hex_sha256(bytes);
    let byte_size = bytes.len();

    if let Some(pool) = pool {
        let inserted = sqlx::query(
            "INSERT INTO viz.render_output \
                 (tenant_id, project_id, artifact_id, format, content, content_hash, \
                  byte_size, renderer_id, renderer_version) \
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
              ON CONFLICT (artifact_id, format) DO UPDATE \
                 SET content = EXCLUDED.content, content_hash = EXCLUDED.content_hash, \
                     byte_size = EXCLUDED.byte_size, renderer_id = EXCLUDED.renderer_id, \
                     renderer_version = EXCLUDED.renderer_version",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(artifact_id)
        .bind(format.as_str())
        .bind(&output.content)
        .bind(&content_hash)
        .bind(byte_size as i32)
        .bind(adapter.id())
        .bind(adapter.version())
        .execute(pool)
        .await;

        if let Err(error) = inserted {
            return RenderOutcome::Failed(format!("产物落库失败：{error}"));
        }
    }

    RenderOutcome::Rendered {
        format,
        byte_size,
        content_hash,
    }
}

/// 小写 hex sha256（对齐 `pm_write::support::hex_sha256`，无 hex crate）。
fn hex_sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::visualization::adapter_contract::{RenderError, RenderOutput};
    use pm_domain::visualization::{
        Budget, SourceRef, VizMeta, VizNode, VizSpec, VizView, SPEC_VERSION,
    };

    fn spec() -> VizSpec {
        VizSpec {
            spec_version: SPEC_VERSION.to_string(),
            slot_key: "s".to_string(),
            view: VizView {
                kind: "dag".to_string(),
                focus: vec![],
                lenses: vec![],
                layout_hint: None,
            },
            nodes: vec![VizNode {
                id: "n1".to_string(),
                label: "节点".to_string(),
                kind: "task".to_string(),
                status: None,
                nature: None,
                source_ref: Some(SourceRef::new("delivery", "task", "T-1")),
                derived: None,
            }],
            edges: vec![],
            meta: VizMeta {
                title: None,
                lang: None,
                budget: Budget::Balanced,
                nature: None,
                registry_version: None,
                provenance: None,
            },
        }
    }

    struct CleanSvg;
    impl RenderAdapter for CleanSvg {
        fn id(&self) -> &'static str {
            "clean-svg"
        }
        fn version(&self) -> &'static str {
            "1.0"
        }
        fn supported_formats(&self) -> &'static [OutputFormat] {
            &[OutputFormat::Svg]
        }
        fn render(
            &self,
            spec: &VizSpec,
            format: OutputFormat,
        ) -> Result<RenderOutput, RenderError> {
            Ok(RenderOutput {
                format,
                content: format!("<svg><text>{}</text></svg>", spec.nodes[0].label),
            })
        }
    }

    /// 产物含外链的坏渲染器——必须被静态化闸拦下。
    struct LeakySvg;
    impl RenderAdapter for LeakySvg {
        fn id(&self) -> &'static str {
            "leaky"
        }
        fn version(&self) -> &'static str {
            "1.0"
        }
        fn supported_formats(&self) -> &'static [OutputFormat] {
            &[OutputFormat::Svg]
        }
        fn render(
            &self,
            _spec: &VizSpec,
            format: OutputFormat,
        ) -> Result<RenderOutput, RenderError> {
            Ok(RenderOutput {
                format,
                content: r#"<svg><a href="https://evil.example/x">x</a></svg>"#.to_string(),
            })
        }
    }

    struct Rejecting;
    impl RenderAdapter for Rejecting {
        fn id(&self) -> &'static str {
            "rejecting"
        }
        fn version(&self) -> &'static str {
            "1.0"
        }
        fn supported_formats(&self) -> &'static [OutputFormat] {
            &[OutputFormat::Svg]
        }
        fn render(
            &self,
            _spec: &VizSpec,
            _format: OutputFormat,
        ) -> Result<RenderOutput, RenderError> {
            Err(RenderError::Rejected("节点超预算".to_string()))
        }
    }

    #[tokio::test]
    async fn clean_render_succeeds_without_db() {
        let outcome = render_artifact(
            None,
            &CleanSvg,
            Uuid::nil(),
            Uuid::nil(),
            Uuid::nil(),
            &spec(),
            OutputFormat::Svg,
        )
        .await;
        match outcome {
            RenderOutcome::Rendered {
                format,
                byte_size,
                content_hash,
            } => {
                assert_eq!(format, OutputFormat::Svg);
                assert!(byte_size > 0);
                assert_eq!(content_hash.len(), 64);
            }
            other => panic!("应渲染成功：{other:?}"),
        }
    }

    #[tokio::test]
    async fn leaky_render_is_blocked_before_persist() {
        // AC-7′：产物入库前的静态化闸——渲染器缺陷不得穿透
        let outcome = render_artifact(
            None,
            &LeakySvg,
            Uuid::nil(),
            Uuid::nil(),
            Uuid::nil(),
            &spec(),
            OutputFormat::Svg,
        )
        .await;
        assert!(
            matches!(outcome, RenderOutcome::Failed(ref m) if m.contains("静态化")),
            "含外链的 SVG 必须被拦下：{outcome:?}"
        );
    }

    #[tokio::test]
    async fn adapter_rejection_is_not_a_failure() {
        let outcome = render_artifact(
            None,
            &Rejecting,
            Uuid::nil(),
            Uuid::nil(),
            Uuid::nil(),
            &spec(),
            OutputFormat::Svg,
        )
        .await;
        assert!(
            matches!(outcome, RenderOutcome::Rejected(_)),
            "显式拒绝不算故障：{outcome:?}"
        );
    }

    #[tokio::test]
    async fn unsupported_format_is_rejected() {
        let outcome = render_artifact(
            None,
            &CleanSvg,
            Uuid::nil(),
            Uuid::nil(),
            Uuid::nil(),
            &spec(),
            OutputFormat::Html,
        )
        .await;
        assert!(matches!(outcome, RenderOutcome::Rejected(_)));
    }
}
