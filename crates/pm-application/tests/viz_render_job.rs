//! 渲染 job 落库集成测试（V0-9）。
//!
//! 覆盖：产物落库、同 artifact+format 重复渲染幂等（ON CONFLICT）、
//! **失败不落库**（R-05：失败产物无消费价值且放大容量负担）。

use pm_application::visualization::adapter_contract::{
    OutputFormat, RenderAdapter, RenderError, RenderOutput,
};
use pm_application::visualization::render_artifact;
use pm_application::visualization::RenderOutcome;
use pm_domain::visualization::{
    Budget, SourceRef, VizMeta, VizNode, VizSpec, VizView, SPEC_VERSION,
};
use sqlx::PgPool;
use uuid::Uuid;

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
        "test-clean"
    }
    fn version(&self) -> &'static str {
        "1.0"
    }
    fn supported_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::Svg]
    }
    fn render(&self, spec: &VizSpec, format: OutputFormat) -> Result<RenderOutput, RenderError> {
        Ok(RenderOutput {
            format,
            content: format!("<svg><text>{}</text></svg>", spec.nodes[0].label),
        })
    }
}

struct LeakySvg;
impl RenderAdapter for LeakySvg {
    fn id(&self) -> &'static str {
        "test-leaky"
    }
    fn version(&self) -> &'static str {
        "1.0"
    }
    fn supported_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::Svg]
    }
    fn render(&self, _spec: &VizSpec, format: OutputFormat) -> Result<RenderOutput, RenderError> {
        Ok(RenderOutput {
            format,
            content: r#"<svg><a href="https://evil.example/x">x</a></svg>"#.to_string(),
        })
    }
}

/// 造一份 viz.artifact（渲染产物的 FK 目标）。
async fn seed_artifact(pool: &PgPool, tenant: Uuid, project: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO viz.artifact \
             (tenant_id, project_id, slot_key, version_no, state, nature, \
              spec, spec_hash, source_hash, provenance, created_by) \
          VALUES ($1, $2, 'render-test', 1, 'draft', 'business', \
                  '{}'::jsonb, 'sh', 'souh', '{}'::jsonb, $3) RETURNING id",
    )
    .bind(tenant)
    .bind(project)
    .bind(Uuid::new_v4())
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn clean_render_persists_output(pool: PgPool) {
    let tenant = Uuid::new_v4();
    let project = Uuid::new_v4();
    let artifact = seed_artifact(&pool, tenant, project).await;

    let outcome = render_artifact(
        Some(&pool),
        &CleanSvg,
        artifact,
        tenant,
        project,
        &spec(),
        OutputFormat::Svg,
    )
    .await;
    assert!(
        matches!(outcome, RenderOutcome::Rendered { .. }),
        "{outcome:?}"
    );

    let (format, renderer_id, size): (String, String, i32) = sqlx::query_as(
        "SELECT format, renderer_id, byte_size FROM viz.render_output WHERE artifact_id = $1",
    )
    .bind(artifact)
    .fetch_one(&pool)
    .await
    .expect("产物应落库");
    assert_eq!(format, "svg");
    assert_eq!(renderer_id, "test-clean", "renderer_id 必须可追溯");
    assert!(size > 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn rerender_same_format_is_idempotent(pool: PgPool) {
    let tenant = Uuid::new_v4();
    let project = Uuid::new_v4();
    let artifact = seed_artifact(&pool, tenant, project).await;

    for _ in 0..3 {
        let outcome = render_artifact(
            Some(&pool),
            &CleanSvg,
            artifact,
            tenant,
            project,
            &spec(),
            OutputFormat::Svg,
        )
        .await;
        assert!(matches!(outcome, RenderOutcome::Rendered { .. }));
    }

    // 同 artifact+format 只有一行（ON CONFLICT DO UPDATE）
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM viz.render_output WHERE artifact_id = $1")
            .bind(artifact)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1, "重复渲染应幂等（不累积行）");
}

#[sqlx::test(migrations = "../../migrations")]
async fn failed_render_does_not_persist(pool: PgPool) {
    let tenant = Uuid::new_v4();
    let project = Uuid::new_v4();
    let artifact = seed_artifact(&pool, tenant, project).await;

    let outcome = render_artifact(
        Some(&pool),
        &LeakySvg,
        artifact,
        tenant,
        project,
        &spec(),
        OutputFormat::Svg,
    )
    .await;
    assert!(matches!(outcome, RenderOutcome::Failed(_)), "{outcome:?}");

    // R-05：失败产物不落表
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM viz.render_output WHERE artifact_id = $1")
            .bind(artifact)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0, "失败产物不得落库");
}
