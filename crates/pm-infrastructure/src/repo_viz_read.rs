//! `VizReadRepo` 的 PostgreSQL 实现（Live 图读侧）。
//!
//! 一致性纪律（14 号 §5.2）：两个查询都是**单条 SQL**（UNION ALL / 单表），
//! 因此投影层可标 `consistency=current`。
//!
//! 端点双端隔离：每条 UNION 分支都带 `project_id = $2`，跨项目脏引用不会渗入。

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use pm_application::repo::viz_read::{TaskDependencyRow, TraceEdgeRow, VizReadRepo};

/// PostgreSQL 实现。
pub struct PgVizReadRepo {
    pool: PgPool,
}

impl PgVizReadRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl VizReadRepo for PgVizReadRepo {
    async fn trace_edges(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
        limit: i64,
    ) -> Result<Vec<TraceEdgeRow>, sqlx::Error> {
        // 单条 SQL：设计覆盖需求 + 任务实现需求（两条既有关系合一的追踪视图）
        sqlx::query_as(
            "SELECT 'design.covers_requirement'::text AS relation, \
                    d.design_id AS from_id, d.requirement_id AS to_id, \
                    COALESCE(dd.title, '') AS from_title, \
                    COALESCE(r.title, '') AS to_title, \
                    d.design_revision_id AS from_revision_id, \
                    d.requirement_revision_id AS to_revision_id \
               FROM design.design_requirement d \
               LEFT JOIN design.design dd ON dd.id = d.design_id \
               LEFT JOIN delivery.requirement r ON r.id = d.requirement_id \
              WHERE d.tenant_id = $1 AND d.project_id = $2 \
             UNION ALL \
             SELECT 'task.implements_requirement'::text, \
                    t.task_id, t.requirement_id, \
                    COALESCE(dt.title, ''), COALESCE(r2.title, ''), \
                    t.task_id, t.revision_id \
               FROM delivery.task_requirement t \
               LEFT JOIN delivery.task dt ON dt.id = t.task_id \
               LEFT JOIN delivery.requirement r2 ON r2.id = t.requirement_id \
              WHERE t.tenant_id = $1 AND t.project_id = $2 \
              LIMIT $3",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    async fn task_dependencies(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
        limit: i64,
    ) -> Result<Vec<TaskDependencyRow>, sqlx::Error> {
        // 单条 SQL：依赖边 + 两端标题/状态（同 project 校验内联）
        sqlx::query_as(
            "SELECT d.task_id, d.depends_on_task_id, \
                    COALESCE(t.title, '') AS task_title, \
                    COALESCE(p.title, '') AS depends_on_title, \
                    COALESCE(t.status, '') AS task_status, \
                    COALESCE(p.status, '') AS depends_on_status \
               FROM delivery.task_dependency d \
               JOIN delivery.task t ON t.id = d.task_id \
               JOIN delivery.task p ON p.id = d.depends_on_task_id \
              WHERE d.tenant_id = $1 AND d.project_id = $2 \
                AND t.project_id = $2 AND p.project_id = $2 \
              ORDER BY d.created_at DESC \
              LIMIT $3",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }
}
