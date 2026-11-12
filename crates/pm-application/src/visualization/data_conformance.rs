//! 关系数据符合性核查（G-2）。
//!
//! 专家裁决 G-2：**Active 不代表所有历史数据自动合法**。三层必须分开：
//! ① 定义已批准（Registry active）② 数据满足关系约束（本模块）③ 数据可进图查询（投影层）。
//!
//! 本模块检查已激活关系的**源数据**是否真的满足关系定义——四条脏数据形态：
//!
//! | # | 形态 | 后果 |
//! |---|---|---|
//! | C-1 | 关系行与其端点对象**跨项目** | 图上画出跨项目假关系；双端隔离会把它裁掉但数据本身脏 |
//! | C-2 | 绑定修订**属于另一个对象** | 图边指向错误版本；stale 判定失真 |
//! | C-3 | 端点或修订**悬空**（指向不存在的行） | 图边无法渲染；溯源断链 |
//! | C-4 | 依赖两端**跨项目** | Task DAG 混入他项目任务 |
//!
//! 新系统按 AGENTS.md 规则 1 无存量数据，故**基线应为 0 违规**；本模块的价值是
//! 让"为 0"成为可复跑的核查结论，而非口头断言。

use sqlx::PgPool;

/// 单条核查结果。
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct ConformanceViolation {
    /// 检查编号（C-1 ~ C-4）。
    pub check_id: String,
    /// 涉及的表。
    pub table_name: String,
    /// 违规行数。
    pub violations: i64,
}

/// 跑全部四项核查（限租户；`tenant_id` 为空则全库）。
///
/// 返回每项检查的违规计数；全零即数据符合关系定义。
pub async fn check_relation_data(
    pool: &PgPool,
    tenant_id: Option<uuid::Uuid>,
) -> Result<Vec<ConformanceViolation>, sqlx::Error> {
    sqlx::query_as(
        r#"
        -- C-1a：design_requirement 与 requirement 跨项目
        SELECT 'C-1' AS check_id, 'design.design_requirement' AS table_name,
               count(*)::bigint AS violations
          FROM design.design_requirement d
          JOIN delivery.requirement r ON r.id = d.requirement_id
         WHERE r.project_id <> d.project_id
           AND ($1::uuid IS NULL OR d.tenant_id = $1)
        UNION ALL
        -- C-1b：design_requirement 与 design 跨项目
        SELECT 'C-1', 'design.design_requirement', count(*)::bigint
          FROM design.design_requirement d
          JOIN design.design dd ON dd.id = d.design_id
         WHERE dd.project_id <> d.project_id
           AND ($1::uuid IS NULL OR d.tenant_id = $1)
        UNION ALL
        -- C-1c：task_requirement 与 requirement 跨项目
        SELECT 'C-1', 'delivery.task_requirement', count(*)::bigint
          FROM delivery.task_requirement t
          JOIN delivery.requirement r ON r.id = t.requirement_id
         WHERE r.project_id <> t.project_id
           AND ($1::uuid IS NULL OR t.tenant_id = $1)
        UNION ALL
        -- C-2a：design_requirement 的绑定修订属于另一个 requirement
        SELECT 'C-2', 'design.design_requirement', count(*)::bigint
          FROM design.design_requirement d
          JOIN delivery.requirement_revision rr ON rr.id = d.requirement_revision_id
         WHERE rr.requirement_id <> d.requirement_id
           AND ($1::uuid IS NULL OR d.tenant_id = $1)
        UNION ALL
        -- C-2b：task_requirement 的绑定修订属于另一个 requirement
        SELECT 'C-2', 'delivery.task_requirement', count(*)::bigint
          FROM delivery.task_requirement t
          JOIN delivery.requirement_revision rr ON rr.id = t.revision_id
         WHERE rr.requirement_id <> t.requirement_id
           AND ($1::uuid IS NULL OR t.tenant_id = $1)
        UNION ALL
        -- C-3a：design_requirement 指向不存在的修订
        SELECT 'C-3', 'design.design_requirement', count(*)::bigint
          FROM design.design_requirement d
          LEFT JOIN delivery.requirement_revision rr ON rr.id = d.requirement_revision_id
         WHERE rr.id IS NULL
           AND ($1::uuid IS NULL OR d.tenant_id = $1)
        UNION ALL
        -- C-3b：task_requirement 指向不存在的修订
        SELECT 'C-3', 'delivery.task_requirement', count(*)::bigint
          FROM delivery.task_requirement t
          LEFT JOIN delivery.requirement_revision rr ON rr.id = t.revision_id
         WHERE rr.id IS NULL
           AND ($1::uuid IS NULL OR t.tenant_id = $1)
        UNION ALL
        -- C-4：task_dependency 两端任务跨项目（或与关系行本身跨项目）
        SELECT 'C-4', 'delivery.task_dependency', count(*)::bigint
          FROM delivery.task_dependency dep
          JOIN delivery.task t ON t.id = dep.task_id
          JOIN delivery.task p ON p.id = dep.depends_on_task_id
         WHERE (t.project_id <> dep.project_id OR p.project_id <> dep.project_id)
           AND ($1::uuid IS NULL OR dep.tenant_id = $1)
        "#,
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
}

/// 是否全部为 0（数据符合关系定义）。
pub fn is_conformant(rows: &[ConformanceViolation]) -> bool {
    rows.iter().all(|row| row.violations == 0)
}

/// 违规摘要（用于日志与验收记录）。
pub fn summary(rows: &[ConformanceViolation]) -> String {
    let offending: Vec<String> = rows
        .iter()
        .filter(|row| row.violations > 0)
        .map(|row| format!("{}/{}={}", row.check_id, row.table_name, row.violations))
        .collect();
    if offending.is_empty() {
        format!("关系数据符合性核查通过（{} 项检查，全部 0 违规）", rows.len())
    } else {
        format!("关系数据符合性核查失败：{}", offending.join(", "))
    }
}
