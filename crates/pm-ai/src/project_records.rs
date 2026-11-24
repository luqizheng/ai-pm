//! chat 注入的「项目正式记录」投影（R2 / B09-01）。
//!
//! **为什么有这个模块**：v1.0 业务验收 B09-01/B03-02——chat AI 的上下文只有
//! 项目画像 + 仓库事实 + 用户手工附加对象，库内的需求/设计/ADR/任务/风险
//! 完全进不了对话，「基于实际记录的项目总结」名存实亡。本模块把各域的
//! **正式记录投影**为一个预算受控的注入节，使 chat AI 能：
//!
//! - 引用真实记录（每行带 code / version / 状态，可定位）；
//! - 区分**正式**（approved / accepted）与**草案/待确认**（draft /
//!   pending_confirmation / proposed / under_review——未经人工确认，不得当
//!   正式决策）；
//! - 知道「没有」——空域显式标 `（无）`，不允许凭空生成项目状态；
//! - 拿到最新事实——每次回复前实时投影（无缓存），记录变更后下一轮即生效。
//!
//! ## 知识架构边界（任务书 §五）
//!
//! Markdown+Git 是正文权威、PostgreSQL 是状态/版本/审批权威——本投影只读
//! 数据库（ID/编号/状态/版本/计数），**不**复制正文正文（approved 正文仅在
//! 用户显式附加对象时由 `ai_context_items` 注入）。这不是「聊天摘要冒充
//! 项目记忆」：无持久化副本、每次实时重投影、行级来源可追溯。
//!
//! ## 选择与长度控制（不是无差别倾倒）
//!
//! 每域限行数（需求 20 / 设计 10 / ADR 10 / 任务计数+最近 5 / 风险 8 /
//! 阻塞 5 / 审计 5），行内只带定位元数据与状态；超节预算时**自下而上整行
//! 裁剪**（历史 → 最近任务 → 风险 → 需求尾部……），不产生截断半行。
//!
//! ## 隔离
//!
//! 全部 SQL 首过滤 `tenant_id AND project_id`（沿 DK-C §5.5 既有模式）；
//! 不触碰任何凭据列；投影内一律按数据对待（`<context>` 注入隔离由
//! `chat_context` 的外层承担）。

use uuid::Uuid;

/// 单节硬预算（字符）。由 `chat_context` 的总预算体系引用。
pub const PROJECT_RECORDS_CAP_CHARS: usize = 6_000;

/// 每域行数上限（选择策略：updated_at 优先，稳定次排序 id）。
const MAX_REQUIREMENT_LINES: i64 = 20;
const MAX_DESIGN_LINES: i64 = 10;
const MAX_ADR_LINES: i64 = 10;
const MAX_RECENT_TASKS: i64 = 5;
const MAX_RISK_LINES: i64 = 8;
const MAX_BLOCKER_LINES: i64 = 5;
const MAX_AUDIT_LINES: i64 = 5;

// ── 只读投影 SQL（单 SELECT；无凭据列）────────────────────────────────

/// 需求：当前修订的状态/版本 + 标题（正式与草案一起取，渲染时分组）。
const SQL_REQUIREMENTS: &str = "SELECT r.code, r.title, cur.version, cur.state \
     FROM delivery.requirement r \
     LEFT JOIN delivery.requirement_revision cur ON cur.id = r.current_revision_id \
     WHERE r.tenant_id = $1 AND r.project_id = $2 AND r.lifecycle_status = 'active' \
     ORDER BY r.updated_at DESC, r.code LIMIT $3";

const SQL_DESIGNS: &str = "SELECT d.code, d.title, cur.version, cur.state \
     FROM design.design d \
     LEFT JOIN design.design_revision cur ON cur.id = d.current_revision_id \
     WHERE d.tenant_id = $1 AND d.project_id = $2 AND d.lifecycle_status = 'active' \
     ORDER BY d.updated_at DESC, d.code LIMIT $3";

const SQL_ADRS: &str = "SELECT a.code, a.title, a.status, a.blockers_implementation \
     FROM design.adr a \
     WHERE a.tenant_id = $1 AND a.project_id = $2 \
     ORDER BY a.updated_at DESC, a.code LIMIT $3";

const SQL_TASK_COUNTS: &str = "SELECT t.status, count(*) FROM delivery.task t \
     WHERE t.tenant_id = $1 AND t.project_id = $2 GROUP BY t.status ORDER BY t.status";

const SQL_RECENT_TASKS: &str = "SELECT t.title, t.status FROM delivery.task t \
     WHERE t.tenant_id = $1 AND t.project_id = $2 \
     ORDER BY t.updated_at DESC, t.id LIMIT $3";

const SQL_RISKS: &str = "SELECT r.title, r.status FROM delivery.risk r \
     WHERE r.tenant_id = $1 AND r.project_id = $2 \
     ORDER BY r.updated_at DESC, r.id LIMIT $3";

const SQL_BLOCKERS: &str = "SELECT b.title, b.status FROM delivery.blocker b \
     WHERE b.tenant_id = $1 AND b.project_id = $2 \
     ORDER BY b.updated_at DESC, b.id LIMIT $3";

/// 待人工处理计数：需求/设计待确认 + ADR 待审 + 访谈待人工（NEEDS_USER_INPUT
/// / REVIEW_REQUIRED）。四条标量子查询（一次往返）。
const SQL_PENDING_HUMAN: &str = "SELECT \
       (SELECT count(*) FROM delivery.requirement_revision rr \
         JOIN delivery.requirement r ON r.id = rr.requirement_id \
         WHERE rr.tenant_id = $1 AND rr.project_id = $2 AND rr.state = 'pending_confirmation'), \
       (SELECT count(*) FROM design.design_revision dr \
         JOIN design.design d ON d.id = dr.design_id \
         WHERE dr.tenant_id = $1 AND dr.project_id = $2 AND dr.state = 'pending_confirmation'), \
       (SELECT count(*) FROM design.adr a \
         WHERE a.tenant_id = $1 AND a.project_id = $2 \
           AND a.status IN ('proposed', 'under_review')), \
       (SELECT count(*) FROM discovery.discovery_session s \
         WHERE s.tenant_id = $1 AND s.project_id = $2 \
           AND s.status IN ('NEEDS_USER_INPUT', 'REVIEW_REQUIRED'))";

/// 最近记录变更（审计投影；「重要变更历史」的紧凑形态）。
const SQL_RECENT_AUDIT: &str = "SELECT a.action, a.target_type, a.created_at \
     FROM governance.audit_log a \
     WHERE a.tenant_id = $1 AND a.project_id = $2 \
     ORDER BY a.created_at DESC, a.id DESC LIMIT $3";

// ── 投影装配 ─────────────────────────────────────────────────────────

/// 一段投影片（小标题 + 行集）。`priority` 越大越先被裁。
struct Part {
    heading: &'static str,
    lines: Vec<String>,
    priority: u8,
}

impl Part {
    fn chars(&self) -> usize {
        self.heading.chars().count()
            + 2
            + self
                .lines
                .iter()
                .map(|l| l.chars().count() + 1)
                .sum::<usize>()
    }
}

/// 构建节正文（预算内整行裁剪，不产生半行；空域显式 `（无）`）。
pub async fn build_project_records_body(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
) -> Result<String, sqlx::Error> {
    // ① 需求（正式 / 草案分组）。
    let reqs = sqlx::query_as::<_, (String, String, Option<i32>, Option<String>)>(SQL_REQUIREMENTS)
        .bind(tenant_id)
        .bind(project_id)
        .bind(MAX_REQUIREMENT_LINES)
        .fetch_all(pool)
        .await?;
    let mut formal_reqs: Vec<String> = Vec::new();
    let mut draft_reqs: Vec<String> = Vec::new();
    for (code, title, version, state) in &reqs {
        let line = match (version, state) {
            (Some(v), Some(s)) => format!("- {code} · v{v} · {s} · {title}"),
            _ => format!("- {code} · （尚无修订版本）· {title}"),
        };
        if matches!(state.as_deref(), Some("approved")) {
            formal_reqs.push(line);
        } else {
            // draft / pending_confirmation / rejected / superseded / 无修订：
            // 全部按「未经人工确认」呈现，不得当正式内容。
            draft_reqs.push(format!("{line}（草案/待确认——未经人工确认）"));
        }
    }

    // ② 设计。
    let designs = sqlx::query_as::<_, (String, String, Option<i32>, Option<String>)>(SQL_DESIGNS)
        .bind(tenant_id)
        .bind(project_id)
        .bind(MAX_DESIGN_LINES)
        .fetch_all(pool)
        .await?;
    let design_lines: Vec<String> = designs
        .iter()
        .map(|(code, title, version, state)| match (version, state) {
            (Some(v), Some(s)) => format!("- {code} · v{v} · {s} · {title}"),
            _ => format!("- {code} · （尚无修订版本）· {title}"),
        })
        .collect();

    // ③ ADR。
    let adrs = sqlx::query_as::<_, (String, String, String, bool)>(SQL_ADRS)
        .bind(tenant_id)
        .bind(project_id)
        .bind(MAX_ADR_LINES)
        .fetch_all(pool)
        .await?;
    let adr_lines: Vec<String> = adrs
        .iter()
        .map(|(code, title, status, blocking)| {
            let mut line = format!("- {code} · {status} · {title}");
            if *blocking {
                line.push_str("（阻塞实施）");
            }
            line
        })
        .collect();

    // ④ 任务（状态计数 + 最近任务）。
    let counts = sqlx::query_as::<_, (String, i64)>(SQL_TASK_COUNTS)
        .bind(tenant_id)
        .bind(project_id)
        .fetch_all(pool)
        .await?;
    let task_count_line = if counts.is_empty() {
        "（无）".to_string()
    } else {
        let detail = counts
            .iter()
            .map(|(s, c)| format!("{s}:{c}"))
            .collect::<Vec<_>>()
            .join("，");
        format!("- 状态计数 {detail}")
    };
    let tasks = sqlx::query_as::<_, (String, String)>(SQL_RECENT_TASKS)
        .bind(tenant_id)
        .bind(project_id)
        .bind(MAX_RECENT_TASKS)
        .fetch_all(pool)
        .await?;
    let mut task_lines = vec![task_count_line];
    task_lines.extend(
        tasks
            .iter()
            .map(|(title, status)| format!("- [{status}] {title}")),
    );

    // ⑤ 风险 / 阻塞。
    let risks = sqlx::query_as::<_, (String, String)>(SQL_RISKS)
        .bind(tenant_id)
        .bind(project_id)
        .bind(MAX_RISK_LINES)
        .fetch_all(pool)
        .await?;
    let risk_lines: Vec<String> = risks
        .iter()
        .map(|(title, status)| format!("- [{status}] {title}"))
        .collect();
    let blockers = sqlx::query_as::<_, (String, String)>(SQL_BLOCKERS)
        .bind(tenant_id)
        .bind(project_id)
        .bind(MAX_BLOCKER_LINES)
        .fetch_all(pool)
        .await?;
    let blocker_lines: Vec<String> = blockers
        .iter()
        .map(|(title, status)| format!("- [{status}] {title}"))
        .collect();

    // ⑥ 待人工处理。
    let pending = sqlx::query_as::<_, (i64, i64, i64, i64)>(SQL_PENDING_HUMAN)
        .bind(tenant_id)
        .bind(project_id)
        .fetch_one(pool)
        .await?;
    let pending_lines = vec![format!(
        "- 需求待确认 {} · 设计待审批 {} · ADR 待审 {} · 访谈待人工 {}",
        pending.0, pending.1, pending.2, pending.3
    )];

    // ⑦ 最近记录变更（审计投影，紧凑行）。
    let audit =
        sqlx::query_as::<_, (String, String, chrono::DateTime<chrono::Utc>)>(SQL_RECENT_AUDIT)
            .bind(tenant_id)
            .bind(project_id)
            .bind(MAX_AUDIT_LINES)
            .fetch_all(pool)
            .await?;
    let audit_lines: Vec<String> = audit
        .iter()
        .map(|(action, target, at)| {
            format!(
                "- {} · {} · {}",
                action,
                target,
                at.format("%m-%d %H:%M UTC")
            )
        })
        .collect();

    // ⑧ 装配 + 预算内裁剪（头部不可裁；整段裁优先于段内裁；段内自尾向头）。
    // 渲染顺序 = parts 原序（正式需求最前，审计最后），裁剪顺序 = priority 大者先。
    let mut parts = vec![
        Part {
            heading: "需求（正式，已确认）",
            lines: formal_reqs,
            priority: 1,
        },
        Part {
            heading: "需求（草案/待确认——未经人工确认，不得当正式决策）",
            lines: draft_reqs,
            priority: 2,
        },
        Part {
            heading: "技术设计（当前修订状态）",
            lines: design_lines,
            priority: 3,
        },
        Part {
            heading: "架构决策（ADR）",
            lines: adr_lines,
            priority: 4,
        },
        Part {
            heading: "任务",
            lines: task_lines,
            priority: 6,
        },
        Part {
            heading: "风险（未关闭即列出）",
            lines: risk_lines,
            priority: 5,
        },
        Part {
            heading: "阻塞",
            lines: blocker_lines,
            priority: 5,
        },
        Part {
            heading: "待人工处理",
            lines: pending_lines,
            priority: 0,
        },
        Part {
            heading: "最近记录变更（审计投影）",
            lines: audit_lines,
            priority: 7,
        },
    ];
    for part in &mut parts {
        if part.lines.is_empty() {
            part.lines.push("（无）".to_string());
        }
    }

    const HEADER: &str = "以下为系统登记的项目记录投影（权威源=数据库，实时读取）；\
「草案/待确认」未经人工确认，不得当作正式决策；未列出的记录即不存在。\n";
    let header_chars = HEADER.chars().count();
    let budget = PROJECT_RECORDS_CAP_CHARS.saturating_sub(header_chars);

    // 整段裁剪：优先级大的段先整体删除（保留 priority=0 的「待人工处理」）。
    let mut dropped: Vec<&'static str> = Vec::new();
    for candidate in parts.iter().filter(|p| p.priority > 0) {
        let total: usize = parts.iter().map(|p| p.chars()).sum::<usize>()
            - dropped
                .iter()
                .filter_map(|h| parts.iter().find(|p| p.heading == *h))
                .map(|p| p.chars())
                .sum::<usize>();
        if total <= budget {
            break;
        }
        if !candidate.lines.is_empty() {
            dropped.push(candidate.heading);
        }
    }
    // 段内行裁剪（自尾向头；每段至少留一行——空态语义不因预算消失）。
    loop {
        let dropped_chars: usize = dropped
            .iter()
            .filter_map(|h| parts.iter().find(|p| p.heading == *h))
            .map(|p| p.chars())
            .sum();
        let total: usize = parts.iter().map(|p| p.chars()).sum::<usize>() - dropped_chars;
        if total <= budget {
            break;
        }
        let target = parts
            .iter_mut()
            .filter(|p| !dropped.contains(&p.heading) && p.lines.len() > 1)
            .max_by_key(|p| p.priority);
        match target {
            Some(part) => {
                part.lines.pop();
            }
            None => break,
        }
    }

    let mut body = String::from(HEADER);
    for part in parts.iter() {
        if dropped.contains(&part.heading) {
            continue;
        }
        body.push_str(&format!("### {}\n", part.heading));
        for line in &part.lines {
            body.push_str(line);
            body.push('\n');
        }
        body.push('\n');
    }
    Ok(body.trim_end().to_string())
}
