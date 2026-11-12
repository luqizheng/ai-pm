//! AI Team 总览/监控聚合 port（任务书批 B 域 9，自 pm-api `ai_team_monitor.rs`
//! 收口）。
//!
//! 域语义（UI-2 裁决包 §〇，见 ai_team_monitor 模块头）：只读聚合——overview
//! 全 COUNT + Provider 绑定态（不读凭证/不解密）；monitor 恰 2 条明细 SQL。
//! 视图 struct 与解析语义裁决（项目指定严格不回退）留 HTTP 层。
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// 项目行租户归属（不存在 → None）。
#[async_trait]
pub trait AiTeamMonitorRepo: Send + Sync {
    async fn project_tenant(&self, project_id: Uuid) -> Result<Option<Uuid>, sqlx::Error>;

    /// ① roles 计数：(启用数, 停用数, 模板总数)。
    async fn roles_summary(&self, project_id: Uuid)
        -> Result<(i64, i64, i64), sqlx::Error>;

    /// ② runs 计数：(活跃, 近24h成功, 近24h失败)。
    async fn runs_summary(&self, project_id: Uuid)
        -> Result<(i64, i64, i64), sqlx::Error>;

    /// ③ tasks 计数：(awaiting_decision, blocked)。
    async fn tasks_summary(&self, project_id: Uuid)
        -> Result<(i64, i64), sqlx::Error>;

    /// 项目指定的 Provider 绑定行：(显示名, 状态, model 配置)；未指定 → None。
    async fn designated_provider(
        &self,
        tenant_id: Uuid,
        project_id: Uuid,
    ) -> Result<Option<(String, String, String)>, sqlx::Error>;

    /// Team 首个 enabled+有模型 Provider 名（创建序）；无 → None。
    async fn team_default_provider(
        &self,
        tenant_id: Uuid,
    ) -> Result<Option<String>, sqlx::Error>;

    async fn recent_runs(
        &self,
        project_id: Uuid,
        limit: i64,
    ) -> Result<Vec<crate::repo::ai_team_monitor::AiTeamRunRow>, sqlx::Error>;

    async fn recent_events(
        &self,
        project_id: Uuid,
        limit: i64,
    ) -> Result<Vec<crate::repo::ai_team_monitor::AiTeamEventRow>, sqlx::Error>;
}


/// monitor 明细行：run（最近 50 条投影）。
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct AiTeamRunRow {
    pub run_id: Uuid,
    pub task_id: Uuid,
    pub task_title: String,
    pub status: String,
    /// 执行阶段（dispatch/started/…；queued Run 可空）
    pub stage: Option<String>,
    pub run_kind: Option<String>,
    pub attempt_no: i32,
    pub executor_kind: Option<String>,
    pub execution_mode: Option<String>,
    pub release_reason: Option<String>,
    /// N-1 调度保护（none/human_wait/start_wait/timed_out，0046）
    pub scheduling_shield: Option<String>,
    pub claimed_at: Option<DateTime<Utc>>,
    /// 终态结果（succeeded/failed/timed_out/cancelled；未终态为 null）
    pub outcome: Option<String>,
    /// 被替代终止时间（B3 隔离/围栏证据；一般终止为 null）
    pub terminated_at: Option<DateTime<Utc>>,
    /// 展示用完成时间锚：COALESCE(terminated_at, finished_at)
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// monitor 明细行：run_dispatch 审计事件（最近 100 条投影）。
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct AiTeamEventRow {
    pub action: String,
    /// 动作目标：claim/skip/reclaim = run_id；task_ready_* = task_id
    pub target_id: Uuid,
    /// JOIN 出的目标任务标题（run 目标经 task_run.task_id 二跳；解析不到为 null）
    pub target_title: Option<String>,
    /// actor 摘要 = principal.display_name（含 dispatch 服务主体；行缺失为 null）
    pub actor: Option<String>,
    /// 释放/跳过原因：COALESCE(after_data->>'release_reason', after_data->>'reason')
    pub reason: Option<String>,
    pub created_at: DateTime<Utc>,
}
