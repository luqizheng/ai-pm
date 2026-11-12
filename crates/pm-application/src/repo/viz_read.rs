//! Visualization 读侧 port（Live 图查询）。
//!
//! 依据 14 号 §3.1/§5.2：Live 图必须**单条 SQL/CTE** 取数——单语句快照天然
//! 给出一致时点（`consistency=current`）；多语句拼接只能标 `mixed-revision`。
//!
//! 端点前置：调用方已完成项目可见性判定（`require_project_visible`）；
//! **端点双端隔离**在此层再校验一次（跨项目脏引用不得渗入，先例
//! `migrations/0007_relationship_owner_domain_seeds.sql:5-8`）。

use async_trait::async_trait;
use uuid::Uuid;

/// 追踪边行（Req ↔ Design ↔ Task 的既有关系）。
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct TraceEdgeRow {
    /// 关系键（`design.covers_requirement` / `task.implements_requirement`）。
    pub relation: String,
    pub from_id: Uuid,
    pub to_id: Uuid,
    pub from_title: String,
    pub to_title: String,
    /// 绑定修订（图边读**绑定修订**而非当前态，14 号 §5.4）。
    pub from_revision_id: Uuid,
    pub to_revision_id: Uuid,
}

/// Task 依赖边行。
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct TaskDependencyRow {
    pub task_id: Uuid,
    pub depends_on_task_id: Uuid,
    pub task_title: String,
    pub depends_on_title: String,
    pub task_status: String,
    pub depends_on_status: String,
}

/// 架构认知边行（cognition_item → baseline_item 的 derived_trace 推导）。
///
/// pc schema（`pc.relationship` / `pc.related`）尚未实施，ai 域也无对象间
/// 关系表——架构图边由结构化 FK 链推导：
/// `baseline_item.proposal_id → baseline_proposal.source_run_id ← cognition_item.run_id`
/// （同一个 cognition_run 产出的认知条目支撑该提案确认出的基线条目）。
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct CognitionEdgeRow {
    pub from_id: Uuid,
    pub to_id: Uuid,
    /// cognition_item.content（全文，label 截断在组装层做）。
    pub from_content: String,
    /// baseline_item.content。
    pub to_content: String,
    /// cognition_item.domain（identity/repository/domain/architecture/…）。
    pub from_kind: String,
}

#[async_trait]
pub trait VizReadRepo: Send + Sync {
    /// 追踪边（需求 ← 设计 ← 任务），**单条 SQL**：`design.covers_requirement`
    /// 与 `task.implements_requirement` 的 UNION ALL，两端 project 校验内联。
    async fn trace_edges(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
        limit: i64,
    ) -> Result<Vec<TraceEdgeRow>, sqlx::Error>;

    /// Task 依赖边（`delivery.task_dependency`），两端 project 校验内联。
    ///
    /// **方向约定**（17 号 R-05 裁决 + 既有实现对齐）：
    /// 返回行保持库内语义 `task_id`（依赖方）→ `depends_on_task_id`（前置方）；
    /// 图查询/影响分析的遍历方向由调用方显式声明，不在此层反转。
    async fn task_dependencies(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
        limit: i64,
    ) -> Result<Vec<TaskDependencyRow>, sqlx::Error>;

    /// 架构认知边（`ai.cognition_item` → `ai.baseline_item`），**单条 SQL**。
    ///
    /// 只取 confirmed 态两端（proposed/rejected 不是业务事实，不得投影）；
    /// 三表 FK 链（item→run→proposal→item）保证推导边有结构化依据——
    /// `derived_trace` 是合成关系（Registry active），不要求持久化边表。
    async fn cognition_edges(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
        limit: i64,
    ) -> Result<Vec<CognitionEdgeRow>, sqlx::Error>;
}
