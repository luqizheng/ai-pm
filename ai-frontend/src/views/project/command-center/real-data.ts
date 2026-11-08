import { getCommandCenter } from "@/api/command-center";
import type {
  CommandCenter,
  CommandCenterBlock,
  ProjectSummary,
  RunBrief,
  WorkSummary
} from "@/api/command-center";
import type { CommandCenterData, Translate } from "./command-center-data";
import type { TaskStat, TaskStatTone } from "./task-overview";

/** 诚实空态：区块 `status=unavailable` 不得当 0 / 空集合用 */
function blockData<T>(block: CommandCenterBlock<T>): T | undefined {
  return block.status === "ok" ? block.data : undefined;
}

function pad(value: number): string {
  return String(value).padStart(2, "0");
}

/** RFC3339 → HH:mm（总控台健康度新鲜度 / 动态时间） */
function formatClock(iso?: string): string {
  if (!iso) return "";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  return `${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

/** 运行耗时：不足 1 小时用 m，超过用 h{m} */
function formatDuration(fromIso?: string, toIso?: string): string {
  if (!fromIso) return "";
  const start = new Date(fromIso).getTime();
  const end = toIso ? new Date(toIso).getTime() : Date.now();
  if (!Number.isFinite(start) || !Number.isFinite(end) || end < start) return "";
  const minutes = Math.round((end - start) / 60000);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest === 0 ? `${hours}h` : `${hours}h${rest}m`;
}

const RUN_TONES: Record<string, TaskStatTone> = {
  queued: "info",
  claimed: "info",
  running: "success",
  succeeded: "success",
  failed: "danger",
  timed_out: "danger",
  terminated: "danger",
  cancelled: "neutral"
};

/** 运行状态文案：词表在 `s01.command-center.runStatus.*`，未知值原样透出 */
function runStatusLabel(status: string, t: Translate): string {
  const key = `s01.command-center.runStatus.${status}`;
  const translated = t(key);
  return translated === key ? status : translated;
}

/** 待关注事项的领域文案：词表在 `s01.command-center.attentionDomains.*`，未知原样透出 */
function domainLabel(domain: string, t: Translate): string {
  const key = `s01.command-center.attentionDomains.${domain}`;
  const translated = t(key);
  return translated === key ? domain : translated;
}

/** 历史动态的类别文案：复用 `pm.commandCenter.activity.*`（unknown 走 other） */
function activityLabel(type: string, t: Translate): string {
  const camel = type.replace(/_([a-z0-9])/g, (_match, char: string) =>
    char.toUpperCase()
  );
  const key = `pm.commandCenter.activity.${camel}`;
  const translated = t(key);
  return translated === key ? t("pm.commandCenter.activity.other") : translated;
}

function activityTone(type: string): TaskStatTone {
  if (
    type.includes("failed") ||
    type.includes("terminated") ||
    type.includes("rejected")
  ) {
    return "danger";
  }
  if (type.includes("changes_requested")) return "warning";
  if (type.includes("archived")) return "neutral";
  if (
    type.includes("succeeded") ||
    type.includes("completed") ||
    type.includes("verified") ||
    type.includes("finished")
  ) {
    return "success";
  }
  return "info";
}

const TASK_IN_PROGRESS_KEYS = [
  "claimed",
  "running",
  "in_progress",
  "in_review",
  "reviewing"
];
const TASK_WAITING_KEYS = [
  "awaiting_decision",
  "submitted",
  "changes_requested",
  "pending_confirmation"
];
const TASK_DONE_KEYS = ["completed", "done"];
const TASK_FAILED_KEYS = ["failed"];

function sumStatuses(
  byStatus: Record<string, number>,
  keys: string[]
): number {
  return keys.reduce((total, key) => total + (byStatus[key] ?? 0), 0);
}

function buildHealth(
  work: WorkSummary | undefined,
  risks: { total: number; items: Array<{ severity: string }> } | undefined,
  actions: { total: number } | undefined,
  t: Translate
) {
  const byStatus = work?.task_by_status ?? {};
  const total = work?.task_total ?? 0;
  const done = sumStatuses(byStatus, TASK_DONE_KEYS);
  const percent = total > 0 ? Math.round((done / total) * 100) : 0;
  const riskHigh =
    risks?.items.filter(item => item.severity === "high").length ?? 0;
  const actionTotal = actions?.total ?? 0;
  const unavailableHint = t("s01.command-center.health.unavailableHint");

  return [
    {
      label: t("s01.command-center.health.overall"),
      value: "—",
      hint: unavailableHint
    },
    {
      label: t("s01.command-center.health.requirements"),
      value: "—",
      hint: unavailableHint
    },
    {
      label: t("s01.command-center.health.tasks"),
      value: percent,
      suffix: "%",
      progress: percent,
      hint: t("s01.command-center.health.tasksHint", { total, done })
    },
    {
      label: t("s01.command-center.health.risks"),
      value: risks?.total ?? 0,
      suffix: t("s01.command-center.health.itemUnit"),
      chip:
        riskHigh > 0
          ? {
              label: t("s01.command-center.health.chipBlocked", {
                count: riskHigh
              }),
              tone: "danger" as const
            }
          : undefined,
      hint: t("s01.command-center.health.risksHint")
    },
    {
      label: t("s01.command-center.health.actions"),
      value: actionTotal,
      suffix: t("s01.command-center.health.itemUnit"),
      chip:
        actionTotal > 0
          ? {
              label: t("s01.command-center.health.chipNeedsHandle"),
              tone: "warning" as const
            }
          : undefined,
      hint: t("s01.command-center.health.actionsHint")
    }
  ];
}

function buildTaskStats(work: WorkSummary | undefined, t: Translate): TaskStat[] {
  const byStatus = work?.task_by_status ?? {};
  const unit = t("s01.command-center.tasks.unit");
  return [
    {
      label: t("s01.command-center.tasks.inProgress"),
      unit,
      count: sumStatuses(byStatus, TASK_IN_PROGRESS_KEYS),
      hint: t("s01.command-center.tasks.inProgressHint")
    },
    {
      label: t("s01.command-center.tasks.waitingHuman"),
      unit,
      count: sumStatuses(byStatus, TASK_WAITING_KEYS),
      hint: t("s01.command-center.tasks.waitingHint")
    },
    {
      label: t("s01.command-center.tasks.done"),
      unit,
      count: sumStatuses(byStatus, TASK_DONE_KEYS),
      hint: t("s01.command-center.tasks.doneHint")
    }
  ];
}

function buildAgentMetrics(work: WorkSummary | undefined, t: Translate) {
  const activeRuns = work?.active_runs ?? [];
  const runningAgents = new Set(
    activeRuns
      .map(run => run.agent?.principal_id)
      .filter((id): id is string => Boolean(id))
  ).size;
  const recentRuns = work?.recent_runs ?? [];
  const finished = recentRuns.filter(run => run.finished_at);
  const minutes = finished.length
    ? finished.reduce((total, run) => {
        const start = new Date(run.created_at).getTime();
        const end = new Date(run.finished_at as string).getTime();
        return Number.isFinite(start) && Number.isFinite(end)
          ? total + Math.max(0, (end - start) / 60000)
          : total;
      }, 0) / finished.length
    : 0;

  return [
    {
      label: t("s01.command-center.agents.running"),
      primary: String(runningAgents),
      secondary: t("s01.command-center.agents.online")
    },
    {
      label: t("s01.command-center.agents.recentRuns"),
      primary: String(recentRuns.length),
      secondary: t("s01.command-center.agents.times")
    },
    {
      label: t("s01.command-center.agents.avgDuration"),
      primary: minutes.toFixed(1),
      secondary: t("s01.command-center.agents.minutes")
    }
  ];
}

function buildAgentRows(work: WorkSummary | undefined, t: Translate) {
  return (work?.active_runs ?? []).map(run => ({
    name: run.agent?.display_name ?? t("pm.commandCenter.noAgent"),
    role: run.agent?.kind ?? run.run_kind,
    work: run.task_title,
    statusLabel: runStatusLabel(run.status, t),
    statusTone: RUN_TONES[run.status] ?? "info",
    progress: null,
    duration: formatDuration(run.started_at ?? run.claimed_at)
  }));
}

function buildRunEntries(
  runs: RunBrief[] | undefined,
  t: Translate
) {
  return (runs ?? []).map(run => ({
    id: run.run_id,
    runId: run.run_id,
    agent: t("pm.commandCenter.noAgent"),
    trigger: `${run.task_title} · ${run.run_kind}`,
    statusLabel: runStatusLabel(run.status, t),
    statusTone: RUN_TONES[run.status] ?? "info",
    duration: formatDuration(run.created_at, run.finished_at)
  }));
}

/**
 * `GET /v1/pm/projects/{id}/command-center` → 页面视图数据。
 *
 * 映射口径（不伪造数据）：
 * - 风险 / 待人工事项 / 任务 / 运行 / 动态 直接取六块；
 * - 需求进度、整体进度、交付物明细、知识快照、成员名单当前接口未提供 → 空态；
 * - 字段缺失时保留后端原值或「—」，不猜 0。
 */
export async function loadCommandCenterData(
  projectId: string,
  t: Translate
): Promise<CommandCenterData> {
  const center: CommandCenter = await getCommandCenter(projectId);
  const summary = blockData<ProjectSummary>(center.blocks.project_summary);
  const work = blockData<WorkSummary>(center.blocks.work_summary);
  const actions = blockData(center.blocks.human_action_summary);
  const risks = blockData(center.blocks.risk_summary);
  const recent = blockData(center.blocks.recent_activity);
  const byStatus = work?.task_by_status ?? {};
  const failedTasks = sumStatuses(byStatus, TASK_FAILED_KEYS);
  const highRisks =
    risks?.items.filter(item => item.severity === "high").length ?? 0;
  const actionTotal = actions?.total ?? 0;

  return {
    header: {
      name: summary?.name ?? "",
      description: summary?.goal?.change_summary ?? "",
      statusLabel: summary
        ? t(`pm.status.${summary.lifecycle}`)
        : t("pm.commandCenter.unavailable"),
      statusTone: summary?.archived ? "neutral" : "success",
      visibilityLabel: "",
      metaEntries: summary
        ? [
            {
              label: t("pm.commandCenter.field.repositories"),
              value: `${summary.verified_repository_count} / ${summary.repository_count}`
            },
            {
              label: t("pm.commandCenter.field.members"),
              value: String(summary.member_count)
            }
          ]
        : []
    },
    health: buildHealth(work, risks, actions, t),
    healthMetaTime: formatClock(center.as_of),
    attention: {
      pendingCount: actionTotal,
      urgentCount: highRisks + failedTasks,
      notifyCount: Math.max(actionTotal - highRisks - failedTasks, 0),
      items: (actions?.items ?? []).map(item => ({
        id: item.id,
        title: item.title,
        category: domainLabel(item.domain, t),
        object:
          Object.values(item.link?.params ?? {})[0] ??
          t("pm.commandCenter.none"),
        statusLabel: item.action_hint,
        statusTone: "warning" as const,
        owner: ""
      }))
    },
    taskTotal: work?.task_total ?? 0,
    taskStats: buildTaskStats(work, t),
    agentMetrics: buildAgentMetrics(work, t),
    agentRows: buildAgentRows(work, t),
    runs: buildRunEntries(work?.recent_runs, t),
    // 需求级交付/验收明细不在总控台读模型内（独立记录，不能由 AI 执行推导）
    deliverables: [],
    activity: (recent?.items ?? []).map(item => ({
      id: item.id,
      time: formatClock(item.occurred_at),
      categoryLabel: activityLabel(item.activity_type, t),
      tone: activityTone(item.activity_type),
      text: item.summary
    })),
    // 知识快照与成员名单当前接口未提供
    knowledge: [],
    members: [],
    memberHumanCount: summary?.member_count ?? 0,
    memberAgentCount: 0
  };
}
