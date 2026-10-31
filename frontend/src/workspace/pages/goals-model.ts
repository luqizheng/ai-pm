import type { Principal, Task } from "@/api/types";
import type { Milestone as ApiMilestone } from "@/api/workspace-view";
import type { Goal, MilestoneStatus, TaskStatus } from "../fixtures";

/** 真模式：里程碑映射成的目标卡（卡即里程碑）。 */
export interface PageGoal extends Goal {
  selfStatus?: MilestoneStatus;
  selfDue?: string;
}

export interface GoalTexts {
  noPlannedDate: string;
  unassigned: string;
  basisByTasks: (done: number, total: number) => string;
  formatPlannedAt: (value: string) => string;
}

export const API_TASK_STATUS: Record<string, TaskStatus> = {
  backlog: "todo",
  in_progress: "doing",
  in_review: "awaiting-acceptance",
  done: "done",
  blocked: "blocked",
  cancelled: "cancelled"
};

const API_MILESTONE_STATUS: Record<string, MilestoneStatus> = {
  planned: "pending",
  active: "active",
  done: "done"
};

export const UNASSIGNED_KEY = "__unassigned__";

function ownerName(
  id: string | null,
  principalNames: Record<string, string>
): string {
  if (!id) return "—";
  return principalNames[id] ?? id.slice(0, 8);
}

function toGoalTask(
  task: Task,
  principalNames: Record<string, string>
): Goal["tasks"][number] {
  return {
    id: task.id,
    name: task.title,
    status: API_TASK_STATUS[task.status] ?? "todo",
    owner: ownerName(task.assignee_id, principalNames)
  };
}

function buildPageGoal(
  id: string,
  code: string,
  name: string,
  plannedAt: string | null,
  status: string,
  own: Task[],
  principalNames: Record<string, string>,
  texts: GoalTexts
): PageGoal {
  const done = own.filter(task => task.status === "done").length;
  const due = plannedAt ? texts.formatPlannedAt(plannedAt) : texts.noPlannedDate;
  return {
    id,
    code,
    name,
    description: due,
    progress: own.length === 0 ? 0 : Math.round((done / own.length) * 100),
    progressBasis: texts.basisByTasks(done, own.length),
    dependencies: [],
    milestones: [],
    tasks: own.map(task => toGoalTask(task, principalNames)),
    proposedTasks: [],
    selfStatus: API_MILESTONE_STATUS[status] ?? "pending",
    selfDue: due
  };
}

/**
 * 真模式目标卡：里程碑 → 卡，任务按 milestone_id 分组；
 * 未挂里程碑的任务收进一张独立卡。进度只按已完成任务数，不做 AI 估算。
 */
export function buildPageGoals(
  milestones: ApiMilestone[],
  tasks: Task[],
  principalNames: Record<string, string>,
  texts: GoalTexts
): PageGoal[] {
  const grouped = new Map<string, Task[]>();
  for (const task of tasks) {
    const key = task.milestone_id ?? UNASSIGNED_KEY;
    grouped.set(key, [...(grouped.get(key) ?? []), task]);
  }
  const result = milestones.map((milestone, index) =>
    buildPageGoal(
      milestone.id,
      `MS-${String(index + 1).padStart(2, "0")}`,
      milestone.name,
      milestone.planned_at,
      milestone.status,
      grouped.get(milestone.id) ?? [],
      principalNames,
      texts
    )
  );
  const loose = grouped.get(UNASSIGNED_KEY) ?? [];
  if (loose.length) {
    result.push(
      buildPageGoal(
        UNASSIGNED_KEY,
        "PLAN",
        texts.unassigned,
        null,
        "planned",
        loose,
        principalNames,
        texts
      )
    );
  }
  return result;
}

export function principalNameMap(
  principals: Principal[]
): Record<string, string> {
  return Object.fromEntries(
    principals.map(principal => [principal.id, principal.display_name] as const)
  );
}

export function milestoneStatusOf(goal: PageGoal): MilestoneStatus {
  return goal.selfStatus ?? "pending";
}