import { describe, expect, it } from "vitest";
import type { Task } from "@/api/types";
import type { Milestone } from "@/api/workspace-view";
import { buildPageGoals, principalNameMap, milestoneStatusOf } from "./goals-model";

const texts = {
  noPlannedDate: "no-date",
  unassigned: "unassigned",
  basisByTasks: (done: number, total: number) => `${done}/${total}`,
  formatPlannedAt: (value: string) => `date:${value.slice(0, 10)}`
};

function milestone(id: string, name: string, plannedAt: string | null = null): Milestone {
  return {
    id,
    project_id: "p1",
    name,
    planned_at: plannedAt,
    status: "active",
    revision: 1
  };
}

function task(
  id: string,
  title: string,
  status: string,
  milestoneId: string | null,
  assigneeId: string | null = null
): Task {
  return {
    id,
    project_id: "p1",
    milestone_id: milestoneId,
    title,
    description: null,
    status,
    priority: "normal",
    assignee_id: assigneeId,
    revision: 1,
    created_at: "2026-10-31T00:00:00Z",
    updated_at: "2026-10-31T00:00:00Z"
  };
}

describe("buildPageGoals（真模式目标卡）", () => {
  it("里程碑映射成目标卡，进度只按已完成任务数", () => {
    const result = buildPageGoals(
      [milestone("m1", "M1", "2026-11-02T00:00:00Z")],
      [
        task("t1", "done task", "done", "m1", "u1"),
        task("t2", "wip task", "in_progress", "m1", "u2"),
        task("t3", "third", "backlog", "m1")
      ],
      { u1: "Alice", u2: "Bob" },
      texts
    );
    expect(result).toHaveLength(1);
    expect(result[0].code).toBe("MS-01");
    expect(result[0].progress).toBe(33);
    expect(result[0].progressBasis).toBe("1/3");
    expect(result[0].description).toBe("date:2026-11-02");
    expect(result[0].tasks.map(t => t.status)).toEqual([
      "done",
      "doing",
      "todo"
    ]);
    expect(result[0].tasks.map(t => t.owner)).toEqual(["Alice", "Bob", "—"]);
    expect(result[0].dependencies).toEqual([]);
    expect(result[0].milestones).toEqual([]);
    expect(milestoneStatusOf(result[0])).toBe("active");
  });

  it("未挂里程碑的任务并入独立卡，无任务的里程碑进度为 0", () => {
    const result = buildPageGoals(
      [milestone("m1", "empty")],
      [task("t1", "loose", "blocked", null)],
      {},
      texts
    );
    expect(result.map(goal => goal.code)).toEqual(["MS-01", "PLAN"]);
    expect(result[0].progress).toBe(0);
    expect(result[0].progressBasis).toBe("0/0");
    expect(result[1].name).toBe("unassigned");
    expect(result[1].tasks[0].status).toBe("blocked");
    expect(result[1].selfDue).toBe("no-date");
  });

  it("未知状态回落默认值，principal id 缺失时显示 id 前缀", () => {
    const result = buildPageGoals(
      [{ ...milestone("m1", "M1"), status: "frozen" }],
      [task("t1", "odd", "mystery", "m1", "0a1b2c3d-eeee")],
      {},
      texts
    );
    expect(milestoneStatusOf(result[0])).toBe("pending");
    expect(result[0].tasks[0].status).toBe("todo");
    expect(result[0].tasks[0].owner).toBe("0a1b2c3d");
  });
});

describe("principalNameMap", () => {
  it("把 principal 列表折成 id → display_name", () => {
    expect(
      principalNameMap([
        {
          id: "u1",
          kind: "human",
          display_name: "Alice",
          is_active: true,
          created_at: "2026-10-31T00:00:00Z"
        }
      ])
    ).toEqual({ u1: "Alice" });
  });
});