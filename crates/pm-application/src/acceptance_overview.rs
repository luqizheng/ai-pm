//! T-070 项目级交付验收聚合读模型（`docs/ai-pm-delivery-acceptance-workbench-v1.md` §四 / §5.1）。
//!
//! 四概念分离：任务生命周期（`delivery.task.status`）、执行 Run（`delivery.task_run`）、
//! 仓库交付事实（`v_task_delivery_manifest`）、人工验收决定（`governance.review_approval`）。
//! 本模块只**读**这四处已落库事实，并在后端统一计算验收资格徽标与动作区状态——
//! 前端禁止自行拼表推导（Q2）；徽标互斥，优先级 `blocked > eligible > missing`。
//!
//! 它**不改状态机**、不新增迁移：所有列 / 表 / 视图均已就位（0010 / 0011 / 0022 / 0026）。

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

/// 待验收集合的唯一任务级入口口径（Q1）：不得经仓库交付状态筛选任务。
pub const PENDING_TASK_STATUSES: [&str; 2] = ["submitted", "reviewing"];

/// 验收资格徽标（§三「三指标口径」/ §四判定规则）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceBadge {
    /// 交付证据满足验收前置条件。
    Eligible,
    /// 存在未解决 blocker 级验收问题，或最新 delivery run 集成失败。
    Blocked,
    /// 其余：零交付 / 部分交付且集成未失败 / 无审查记录。
    Missing,
}

impl AcceptanceBadge {
    /// 总表排序位：阻塞优先、其次待补充、可验收靠后（§5.1「默认就按处理优先级排」）。
    fn rank(self) -> u8 {
        match self {
            AcceptanceBadge::Blocked => 0,
            AcceptanceBadge::Missing => 1,
            AcceptanceBadge::Eligible => 2,
        }
    }
}

/// 动作区状态（§5.1 裁定：避免前端从审批行自行推断）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    /// 尚无 review run。
    None,
    /// review run 处于 queued / claimed / running。
    Running,
    /// 存在 pending 审批行——唯一可操作的状态。
    AwaitingDecision,
    /// 最新审批行已 approved / rejected。
    Decided,
}

/// 交付摘要（任务级，来自交付清单视图按 task 聚合）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeliverySummary {
    pub expected_repos: i32,
    pub delivered_repos: i32,
    /// `delivery_required` 却在最新 delivery run 上没有 commit 的仓名（按名排序）。
    pub missing_repos: Vec<String>,
    /// 最新 delivery run 的集成事实（`passed` / `failed` / `not_run`）；无 run 时为空。
    pub integration_status: Option<String>,
}

/// 总表的一行。
#[derive(Debug, Clone, Serialize)]
pub struct AcceptanceTask {
    pub task_id: Uuid,
    pub title: String,
    pub status: String,
    pub badge: AcceptanceBadge,
    pub delivery_summary: DeliverySummary,
    /// 最新一次成功 review run 的 blocker 级 finding 数（§四「未解决」简化口径）。
    pub blocker_count: i64,
    pub latest_delivery_run_id: Option<Uuid>,
    pub latest_attempt_no: Option<i32>,
    pub review_status: ReviewStatus,
    /// 可操作时为 pending 审批行 id，否则为空。
    pub pending_approval_id: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 顶部三指标：三者维度不同，不得混同（§三）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AcceptanceCounts {
    /// 任务生命周期：`status ∈ {submitted, reviewing}`。
    pub pending: i64,
    /// 交付证据完整性：待验收中满足前置条件者。
    pub eligible: i64,
    /// 验收问题 / 交付条件：待验收中存在阻塞者。
    pub blocked: i64,
}

/// `GET /v1/pm/acceptance-overview` 的响应体。
#[derive(Debug, Clone, Serialize)]
pub struct AcceptanceOverview {
    pub project_id: Uuid,
    pub counts: AcceptanceCounts,
    pub tasks: Vec<AcceptanceTask>,
    pub total: usize,
}

/// 聚合输入：一行 = 一个待验收任务的全部已落库事实（纯读，不含任何推导）。
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AcceptanceFacts {
    pub task_id: Uuid,
    pub title: String,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expected_repos: i32,
    pub delivered_repos: i32,
    pub missing_repos: Vec<String>,
    pub latest_delivery_run_id: Option<Uuid>,
    pub latest_attempt_no: Option<i32>,
    pub integration_status: Option<String>,
    pub blocker_count: i64,
    pub review_running: bool,
    pub pending_approval_id: Option<Uuid>,
    pub has_decided_approval: bool,
}

/// §四判定规则的唯一实现。
///
/// ```text
/// eligible = 所有 delivery_required 仓库已交付 AND 最新 delivery run integration_status='passed'
///            AND 不存在未解决的 blocker review_finding
/// blocked  = 存在未解决的 blocker review_finding OR 最新 delivery run integration_status='failed'
/// missing  = 其余
/// ```
///
/// `integration_status='failed'` 属阻塞（集成事实失败，验收人需要看到失败原因）；
/// 零交付 / 部分交付属「待补充」——证据还没齐，谈不上阻塞。
pub fn classify_badge(facts: &AcceptanceFacts) -> AcceptanceBadge {
    let integration_failed = facts.integration_status.as_deref() == Some("failed");
    if facts.blocker_count > 0 || integration_failed {
        return AcceptanceBadge::Blocked;
    }
    let all_required_delivered = facts.delivered_repos >= facts.expected_repos;
    let integration_passed = facts.integration_status.as_deref() == Some("passed");
    if all_required_delivered && integration_passed {
        AcceptanceBadge::Eligible
    } else {
        AcceptanceBadge::Missing
    }
}

/// 动作区状态：pending 审批行优先（唯一可点），其次进行中的审查，再次已有结论。
pub fn classify_review_status(facts: &AcceptanceFacts) -> ReviewStatus {
    if facts.pending_approval_id.is_some() {
        ReviewStatus::AwaitingDecision
    } else if facts.review_running {
        ReviewStatus::Running
    } else if facts.has_decided_approval {
        ReviewStatus::Decided
    } else {
        ReviewStatus::None
    }
}

/// 把事实行组装成总表：计算徽标 / 动作区状态、按处理优先级排序、汇总三指标。纯函数。
pub fn build_overview(project_id: Uuid, facts: Vec<AcceptanceFacts>) -> AcceptanceOverview {
    let mut tasks: Vec<AcceptanceTask> = facts
        .into_iter()
        .map(|f| {
            let badge = classify_badge(&f);
            let review_status = classify_review_status(&f);
            AcceptanceTask {
                task_id: f.task_id,
                title: f.title,
                status: f.status,
                badge,
                delivery_summary: DeliverySummary {
                    expected_repos: f.expected_repos,
                    delivered_repos: f.delivered_repos,
                    missing_repos: f.missing_repos,
                    integration_status: f.integration_status,
                },
                blocker_count: f.blocker_count,
                latest_delivery_run_id: f.latest_delivery_run_id,
                latest_attempt_no: f.latest_attempt_no,
                review_status,
                pending_approval_id: f.pending_approval_id,
                created_at: f.created_at,
            }
        })
        .collect();
    tasks.sort_by(|a, b| {
        a.badge
            .rank()
            .cmp(&b.badge.rank())
            .then_with(|| b.created_at.cmp(&a.created_at))
            .then_with(|| a.task_id.cmp(&b.task_id))
    });
    let counts = AcceptanceCounts {
        pending: tasks.len() as i64,
        eligible: tasks
            .iter()
            .filter(|t| t.badge == AcceptanceBadge::Eligible)
            .count() as i64,
        blocked: tasks
            .iter()
            .filter(|t| t.badge == AcceptanceBadge::Blocked)
            .count() as i64,
    };
    let total = tasks.len();
    AcceptanceOverview {
        project_id,
        counts,
        tasks,
        total,
    }
}

/// 读取项目内全部待验收任务的事实行（Q1 状态过滤 + 左连交付清单 + blocker 集 + 审批状态）。
///
/// - 没有任何 `task_repository` 的任务不在视图里，左连后 expected / delivered 为 0、
///   `missing_repos` 为空——仍出现在总表（Q1「待补充」）。
/// - 最新 delivery run 直接取自 `task_run`（与视图 `latest_run` 同序），覆盖无仓任务。
/// - blocker 集 = 每个 task 最新一次 `run_kind='review' AND status='succeeded'` 的 run
///   的 `severity='blocker'` findings（§四简化口径，不引入新表、不改写历史）。
pub async fn load_facts(
    pool: &PgPool,
    project_id: Uuid,
    tenant_id: Uuid,
) -> Result<Vec<AcceptanceFacts>, sqlx::Error> {
    sqlx::query_as::<_, AcceptanceFacts>(
        "WITH pending AS ( \
             SELECT t.id AS task_id, t.title, t.status, t.created_at \
               FROM delivery.task t \
              WHERE t.project_id = $1 AND t.tenant_id = $2 \
                AND t.status = ANY($3::text[]) \
         ), manifest AS ( \
             SELECT m.task_id, \
                    max(m.expected_delivery_count) AS expected_repos, \
                    max(m.actual_delivery_count)   AS delivered_repos, \
                    coalesce(array_agg(m.repository_name ORDER BY m.repository_name) \
                             FILTER (WHERE m.delivery_required AND m.commit_sha IS NULL), \
                             '{}'::text[]) AS missing_repos \
               FROM delivery.v_task_delivery_manifest m \
               JOIN pending p ON p.task_id = m.task_id \
              WHERE m.project_id = $1 \
              GROUP BY m.task_id \
         ), latest_delivery AS ( \
             SELECT DISTINCT ON (r.task_id) \
                    r.task_id, r.id AS run_id, r.attempt_no, r.integration_status \
               FROM delivery.task_run r \
               JOIN pending p ON p.task_id = r.task_id \
              WHERE r.project_id = $1 AND r.run_kind = 'delivery' \
              ORDER BY r.task_id, r.attempt_no DESC, r.created_at DESC \
         ), latest_review AS ( \
             SELECT DISTINCT ON (r.task_id) r.task_id, r.id AS run_id \
               FROM delivery.task_run r \
               JOIN pending p ON p.task_id = r.task_id \
              WHERE r.project_id = $1 AND r.run_kind = 'review' AND r.status = 'succeeded' \
              ORDER BY r.task_id, r.attempt_no DESC, r.created_at DESC \
         ), blockers AS ( \
             SELECT lr.task_id, count(*)::bigint AS blocker_count \
               FROM latest_review lr \
               JOIN delivery.review_finding f ON f.review_run_id = lr.run_id \
              WHERE f.severity = 'blocker' \
              GROUP BY lr.task_id \
         ) \
         SELECT p.task_id, p.title, p.status, p.created_at, \
                coalesce(m.expected_repos, 0)::int  AS expected_repos, \
                coalesce(m.delivered_repos, 0)::int AS delivered_repos, \
                coalesce(m.missing_repos, '{}'::text[]) AS missing_repos, \
                ld.run_id            AS latest_delivery_run_id, \
                ld.attempt_no        AS latest_attempt_no, \
                ld.integration_status, \
                coalesce(b.blocker_count, 0)::bigint AS blocker_count, \
                EXISTS (SELECT 1 FROM delivery.task_run r \
                         WHERE r.task_id = p.task_id AND r.run_kind = 'review' \
                           AND r.status IN ('queued', 'claimed', 'running')) AS review_running, \
                (SELECT a.id FROM governance.review_approval a \
                  WHERE a.task_id = p.task_id AND a.status = 'pending' \
                  ORDER BY a.created_at DESC LIMIT 1) AS pending_approval_id, \
                EXISTS (SELECT 1 FROM governance.review_approval a \
                         WHERE a.task_id = p.task_id \
                           AND a.status IN ('approved', 'rejected')) AS has_decided_approval \
           FROM pending p \
           LEFT JOIN manifest m        ON m.task_id = p.task_id \
           LEFT JOIN latest_delivery ld ON ld.task_id = p.task_id \
           LEFT JOIN blockers b         ON b.task_id = p.task_id \
          ORDER BY p.created_at DESC, p.task_id",
    )
    .bind(project_id)
    .bind(tenant_id)
    .bind(PENDING_TASK_STATUSES.map(str::to_owned).to_vec())
    .fetch_all(pool)
    .await
}

/// 项目级聚合读端点的应用层入口：读事实 → 组装总表。
pub async fn load_overview(
    pool: &PgPool,
    project_id: Uuid,
    tenant_id: Uuid,
) -> Result<AcceptanceOverview, sqlx::Error> {
    let facts = load_facts(pool, project_id, tenant_id).await?;
    Ok(build_overview(project_id, facts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    fn facts(title: &str) -> AcceptanceFacts {
        AcceptanceFacts {
            task_id: Uuid::new_v4(),
            title: title.to_string(),
            status: "submitted".to_string(),
            created_at: Utc::now(),
            expected_repos: 0,
            delivered_repos: 0,
            missing_repos: vec![],
            latest_delivery_run_id: None,
            latest_attempt_no: None,
            integration_status: None,
            blocker_count: 0,
            review_running: false,
            pending_approval_id: None,
            has_decided_approval: false,
        }
    }

    #[test]
    fn zero_delivery_is_missing_not_blocked() {
        // Q1：零交付的 Submitted 任务仍在总表，标「待补充」。
        assert_eq!(classify_badge(&facts("bare")), AcceptanceBadge::Missing);
    }

    #[test]
    fn partial_delivery_with_integration_not_failed_is_missing() {
        let mut f = facts("partial");
        f.expected_repos = 3;
        f.delivered_repos = 2;
        f.missing_repos = vec!["shared-contracts".into()];
        f.integration_status = Some("passed".into());
        assert_eq!(classify_badge(&f), AcceptanceBadge::Missing);
    }

    #[test]
    fn full_delivery_passed_no_blockers_is_eligible() {
        let mut f = facts("ok");
        f.expected_repos = 2;
        f.delivered_repos = 2;
        f.integration_status = Some("passed".into());
        assert_eq!(classify_badge(&f), AcceptanceBadge::Eligible);
    }

    #[test]
    fn full_delivery_but_integration_not_run_is_missing() {
        let mut f = facts("not-run");
        f.expected_repos = 1;
        f.delivered_repos = 1;
        f.integration_status = Some("not_run".into());
        assert_eq!(classify_badge(&f), AcceptanceBadge::Missing);
    }

    #[test]
    fn blocker_finding_wins_over_eligibility() {
        let mut f = facts("blocked");
        f.expected_repos = 1;
        f.delivered_repos = 1;
        f.integration_status = Some("passed".into());
        f.blocker_count = 1;
        assert_eq!(classify_badge(&f), AcceptanceBadge::Blocked);
    }

    #[test]
    fn integration_failed_is_blocked_even_with_partial_delivery() {
        let mut f = facts("int-failed");
        f.expected_repos = 2;
        f.delivered_repos = 1;
        f.integration_status = Some("failed".into());
        assert_eq!(classify_badge(&f), AcceptanceBadge::Blocked);
    }

    #[test]
    fn review_status_priority_pending_then_running_then_decided() {
        let mut f = facts("rs");
        assert_eq!(classify_review_status(&f), ReviewStatus::None);
        f.has_decided_approval = true;
        assert_eq!(classify_review_status(&f), ReviewStatus::Decided);
        f.review_running = true;
        assert_eq!(classify_review_status(&f), ReviewStatus::Running);
        f.pending_approval_id = Some(Uuid::new_v4());
        assert_eq!(classify_review_status(&f), ReviewStatus::AwaitingDecision);
    }

    #[test]
    fn overview_sorts_blocked_missing_eligible_and_counts_each_dimension() {
        let now = Utc::now();
        let mut eligible = facts("eligible");
        eligible.expected_repos = 1;
        eligible.delivered_repos = 1;
        eligible.integration_status = Some("passed".into());
        eligible.created_at = now;

        let mut blocked = facts("blocked");
        blocked.blocker_count = 2;
        blocked.created_at = now - Duration::hours(3);

        let mut missing_old = facts("missing-old");
        missing_old.created_at = now - Duration::hours(2);
        let mut missing_new = facts("missing-new");
        missing_new.created_at = now - Duration::hours(1);

        let overview = build_overview(
            Uuid::new_v4(),
            vec![eligible, missing_old, blocked, missing_new],
        );

        let titles: Vec<&str> = overview.tasks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(
            titles,
            vec!["blocked", "missing-new", "missing-old", "eligible"],
            "blocked first, then missing newest-first, eligible last"
        );
        assert_eq!(
            overview.counts,
            AcceptanceCounts {
                pending: 4,
                eligible: 1,
                blocked: 1
            }
        );
        assert_eq!(overview.total, 4);
        // 每个任务恰好一个徽标，三值互斥。
        for task in &overview.tasks {
            let one_of = [
                AcceptanceBadge::Eligible,
                AcceptanceBadge::Blocked,
                AcceptanceBadge::Missing,
            ];
            assert_eq!(one_of.iter().filter(|b| **b == task.badge).count(), 1);
        }
    }

    #[test]
    fn badge_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&AcceptanceBadge::Eligible).unwrap(),
            "\"eligible\""
        );
        assert_eq!(
            serde_json::to_string(&ReviewStatus::AwaitingDecision).unwrap(),
            "\"awaiting_decision\""
        );
    }
}
