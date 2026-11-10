import type {
  ExecutionReadiness,
  ReadinessCheck,
  TaskReadiness
} from "@/api/task-plans";
import { shortId } from "../tasks-contract";

/**
 * S04-UI-07 Execution Readiness · 视图适配（真实读模型 → 页面模型）。
 *
 * 数据源 = `GET /v1/pm/projects/{project_id}/execution-readiness`：后端已
 * 按任务跑完 A–F 六段检查并给出三态汇总，前端只做展示聚合。
 *
 * 三态口径与后端一致：
 * - `ready` = 六段全过；
 * - `blocked` = C（依赖校验）/ D（工程准备）任一段不过（硬阻断段）；
 * - `needs_attention` = 其余段不过。
 *
 * 纪律：后端只给「事实 + 判定」（issues 为英文技术短语），没有严重级 /
 * 建议处置 / 豁免留痕字段——这些在 UI 上不编造：严重级按后端的硬阻断段
 * 规则推导（C/D = high，其余 = medium），建议处置留空。
 */

/** 检查段（A–F）。 */
export const READINESS_SECTIONS = ["A", "B", "C", "D", "E", "F"] as const;

/** 硬阻断段（后端口径：不过即 blocked）。 */
export const BLOCKING_SECTIONS = new Set(["C", "D"]);

/** 单条检查项。 */
export interface ReadinessCheckView {
  section: string;
  key: string;
  passed: boolean;
  issues: string[];
}

/** 单任务准备度。 */
export interface ReadinessTaskView {
  taskId: string;
  code: string;
  title: string;
  status: string;
  state: "ready" | "needs_attention" | "blocked";
  passedChecks: number;
  totalChecks: number;
  checks: ReadinessCheckView[];
  /** 未通过检查项的缺口文案（按检查顺序）。 */
  missing: string[];
}

/** 段聚合（跨任务）。 */
export interface ReadinessSectionView {
  section: string;
  passed: number;
  total: number;
  state: "pass" | "warn" | "block";
  failures: Array<{
    taskId: string;
    code: string;
    title: string;
    issues: string[];
  }>;
}

/** 待处置问题（由未通过检查项派生）。 */
export interface ReadinessIssueView {
  id: string;
  taskId: string;
  code: string;
  title: string;
  section: string;
  key: string;
  severity: "high" | "medium";
  reason: string;
}

/** 准备度视图模型。 */
export interface ReadinessViewModel {
  status: string;
  readyCount: number;
  attentionCount: number;
  blockedCount: number;
  totalCount: number;
  tasks: ReadinessTaskView[];
  sections: ReadinessSectionView[];
  issues: ReadinessIssueView[];
}

function toState(status: string): ReadinessTaskView["state"] {
  if (status === "ready") return "ready";
  if (status === "blocked") return "blocked";
  return "needs_attention";
}

function toChecks(checks: ReadinessCheck[]): ReadinessCheckView[] {
  return checks.map(check => ({
    section: check.section,
    key: check.key,
    passed: check.passed,
    issues: check.issues ?? []
  }));
}

/** 单任务准备度 → 页面模型。 */
export function toReadinessTask(task: TaskReadiness): ReadinessTaskView {
  const checks = toChecks(task.checks ?? []);
  const missing = checks
    .filter(check => !check.passed)
    .flatMap(check => check.issues);
  return {
    taskId: task.task_id,
    code: shortId(task.task_id),
    title: task.title,
    status: task.status,
    state: toState(task.summary.status),
    passedChecks: task.summary.ready_count ?? 0,
    totalChecks: task.summary.total_count ?? checks.length,
    checks,
    missing
  };
}

/** 项目级准备度 → 页面模型（任务表 + 六段聚合 + 问题清单）。 */
export function toReadinessView(
  payload: ExecutionReadiness
): ReadinessViewModel {
  const tasks = (payload.tasks ?? []).map(toReadinessTask);

  const sections: ReadinessSectionView[] = READINESS_SECTIONS.map(section => {
    const failures = tasks
      .map(task => {
        const check = task.checks.find(item => item.section === section);
        if (!check || check.passed) return null;
        return {
          taskId: task.taskId,
          code: task.code,
          title: task.title,
          issues: check.issues
        };
      })
      .filter((item): item is NonNullable<typeof item> => item !== null);
    const total = tasks.filter(task =>
      task.checks.some(check => check.section === section)
    ).length;
    const passed = total - failures.length;
    const state: ReadinessSectionView["state"] =
      failures.length === 0
        ? "pass"
        : BLOCKING_SECTIONS.has(section)
          ? "block"
          : "warn";
    return { section, passed, total, state, failures };
  });

  const issues: ReadinessIssueView[] = [];
  for (const task of tasks) {
    for (const check of task.checks) {
      if (check.passed) continue;
      issues.push({
        id: `${task.taskId}:${check.key}`,
        taskId: task.taskId,
        code: task.code,
        title: task.title,
        section: check.section,
        key: check.key,
        severity: BLOCKING_SECTIONS.has(check.section) ? "high" : "medium",
        reason: check.issues.join("；")
      });
    }
  }

  const totalCount = payload.summary?.total_count ?? tasks.length;
  const readyCount = payload.summary?.ready_count ?? 0;
  const blockedCount = payload.summary?.blocked_count ?? 0;

  return {
    status: payload.summary?.status ?? "",
    readyCount,
    blockedCount,
    attentionCount: Math.max(totalCount - readyCount - blockedCount, 0),
    totalCount,
    tasks,
    sections,
    issues
  };
}
