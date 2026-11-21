//! 活动流：治理审计 → 归一化活动时间线（§4.6/§4.7）。

use super::*;

// ---------------------------------------------------------------------------
// §4.6 实现
// ---------------------------------------------------------------------------

pub(super) async fn load_recent_activity(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<RecentActivity, sqlx::Error> {
    // 去重键 = `{activity_type}:{target_type}:{target_id}`——**必须在 LIMIT 之前**完成，
    // 否则一页里可能出现重复项（少行），且同一条目会跨页重复出现（I5-030/I5-031）。
    // 因此去重下推到 SQL（`DISTINCT ON`），再排序分页；排序键 `created_at DESC, id DESC`
    // 保证并列时间下的稳定次序。
    let case_sql = ACTIVITY_TYPE_BY_ACTION
        .iter()
        .map(|(action, activity_type)| format!("WHEN '{action}' THEN '{activity_type}'"))
        .collect::<Vec<_>>()
        .join(" ");
    let deduped = format!(
        "SELECT DISTINCT ON (activity_type, target_type, target_id) \
                id, activity_type, target_type, target_id, actor_id, created_at \
           FROM ( \
                SELECT a.id, a.action, a.target_type, a.target_id, a.actor_id, a.created_at, \
                       CASE a.action {case_sql} END AS activity_type \
                  FROM governance.audit_log a \
                 WHERE a.tenant_id = $1 AND a.project_id = $2 AND a.action = ANY($3) \
           ) s \
          ORDER BY activity_type, target_type, target_id, created_at DESC, id DESC"
    );

    let total: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM ({deduped}) d"))
        .bind(tenant_id)
        .bind(project_id)
        .bind(activity_audit_actions())
        .fetch_one(pool)
        .await?;

    type Row = (
        Uuid,
        String,
        Option<String>,
        Option<Uuid>,
        Option<Uuid>,
        DateTime<Utc>,
    );
    let rows: Vec<Row> = sqlx::query_as(&format!(
        "SELECT d.id, d.activity_type, d.target_type, d.target_id, d.actor_id, d.created_at \
           FROM ({deduped}) d \
          ORDER BY d.created_at DESC, d.id DESC \
          LIMIT $4 OFFSET $5"
    ))
    .bind(tenant_id)
    .bind(project_id)
    .bind(activity_audit_actions())
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    // 执行主体名称（actor）单独批量取，避免与去重子查询耦合。
    let actor_ids: Vec<Uuid> = rows.iter().filter_map(|row| row.4).collect();
    let mut actors: std::collections::HashMap<Uuid, (Option<String>, Option<String>)> =
        std::collections::HashMap::new();
    if !actor_ids.is_empty() {
        let actor_rows: Vec<(Uuid, Option<String>, Option<String>)> =
            sqlx::query_as("SELECT id, display_name, kind FROM core.principal WHERE id = ANY($1)")
                .bind(&actor_ids)
                .fetch_all(pool)
                .await?;
        for (id, name, kind) in actor_rows {
            actors.insert(id, (name, kind));
        }
    }

    let items = rows
        .into_iter()
        .map(
            |(id, activity_type, target_type, target_id, actor_id, created_at)| {
                let (actor_name, actor_kind) = actor_id
                    .and_then(|actor| actors.get(&actor).cloned())
                    .unwrap_or((None, None));
                let dedup_key = format!(
                    "{activity_type}:{}:{}",
                    target_type.clone().unwrap_or_default(),
                    target_id.map(|v| v.to_string()).unwrap_or_default()
                );
                ActivityItem {
                    id,
                    summary: activity_summary(&activity_type),
                    activity_type,
                    source_type: target_type,
                    source_id: target_id,
                    occurred_at: created_at,
                    actor: ActivityActor {
                        kind: match actor_kind.as_deref() {
                            Some("human") => "human".to_string(),
                            Some("dev_agent") | Some("ai_pm") | Some("service") => {
                                "agent".to_string()
                            }
                            _ => "system".to_string(),
                        },
                        id: actor_id,
                        name: actor_name,
                    },
                    dedup_key,
                }
            },
        )
        .collect();

    Ok(RecentActivity { total, items })
}

/// 审计 action → 归一化活动类型（白名单映射，契约 §4.7；与 SQL `CASE` 同源）。
pub fn activity_type_for(action: &str) -> Option<&'static str> {
    ACTIVITY_TYPE_BY_ACTION
        .iter()
        .find(|(candidate, _)| *candidate == action)
        .map(|(_, activity_type)| *activity_type)
}

fn activity_summary(activity_type: &str) -> String {
    match activity_type {
        "project_closed" => "项目已关闭".to_string(),
        "goal_committed" => "项目目标已发布新版本".to_string(),
        "finding_decided" => "发现已由人工裁决".to_string(),
        "finding_revision_requested" => "发现被要求补充信息".to_string(),
        "repository_created" => "关联了新的仓库".to_string(),
        "repository_verified" => "仓库连通性已校验".to_string(),
        "analysis_succeeded" => "代码分析完成".to_string(),
        "analysis_failed" => "代码分析失败".to_string(),
        "run_started" => "执行已启动".to_string(),
        "run_finished" => "执行已结束".to_string(),
        "run_failed" => "执行启动失败".to_string(),
        "run_terminated" => "执行已终止".to_string(),
        "delivery_submitted" => "交付证据已提交".to_string(),
        "delivery_rejected" => "交付被驳回".to_string(),
        "review_completed" => "评审已完成".to_string(),
        "review_changes_requested" => "评审要求修改".to_string(),
        "member_changed" => "项目成员变更".to_string(),
        "device_bound" => "设备已绑定项目".to_string(),
        _ => "项目活动".to_string(),
    }
}
