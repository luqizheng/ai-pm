import { describe, expect, it } from "vitest";
import {
  aggregateStatusLabel,
  aggregateStatusTone,
  executionDisplay,
  formatDue,
  priorityGrade,
  schedulingCondition,
  TASK_STATUS_AGGREGATES
} from "./tasks-contract";

/**
 * PA-07 展示层词表契约（Figma 179:3603）：
 * 聚合/派生均为后端事实的投影——五态聚合、P0-P3 映射、调度条件四态
 * （product_blocked > running > waiting > ready > closed）。
 */
describe("tasks-contract PA-07 aggregates", () => {
  it("aggregates 13 lifecycle states into the five PA-07 buckets without leftovers", () => {
    const all = TASK_STATUS_AGGREGATES.flatMap(agg => agg.statuses as readonly string[]);
    expect(new Set(all).size).toBe(all.length);
    expect(all).toContain("draft");
    expect(all).toContain("running");
    expect(all).toContain("superseded");
    expect(aggregateStatusLabel("blocked")).toBe("Not started");
    expect(aggregateStatusLabel("awaiting_decision")).toBe("In progress");
    expect(aggregateStatusLabel("completed")).toBe("Completed");
    expect(aggregateStatusLabel("failed")).toBe("Failed");
    expect(aggregateStatusLabel("superseded")).toBe("Cancelled");
    expect(aggregateStatusTone("running")).toBe("info");
    expect(aggregateStatusTone("failed")).toBe("danger");
  });

  it("maps the four priorities onto the P0–P3 display grades", () => {
    expect(priorityGrade("urgent")).toBe("P0");
    expect(priorityGrade("high")).toBe("P1");
    expect(priorityGrade("normal")).toBe("P2");
    expect(priorityGrade("low")).toBe("P3");
  });

  it("renders execution column from the latest run only (no harness, no fabrication)", () => {
    expect(executionDisplay(null)).toEqual({ label: "No run", detail: null });
    expect(
      executionDisplay({
        id: "run-1",
        attempt_no: 3,
        status: "running",
        review_verdict: null,
        created_at: "2026-11-21T00:00:00Z"
      })
    ).toEqual({ label: "Run #3", detail: "running · Active" });
    expect(
      executionDisplay({
        id: "run-2",
        attempt_no: 1,
        status: "succeeded",
        review_verdict: "approved",
        created_at: "2026-11-21T00:00:00Z"
      }).detail
    ).toBe("succeeded");
  });

  it("derives scheduling conditions with the design precedence", () => {
    const base = {
      status: "draft",
      product_blocked: false,
      deps_total: 0,
      deps_met: 0,
      latest_run: null
    };
    // 终态优先：调度条件随生命周期关闭。
    expect(schedulingCondition({ ...base, status: "completed" }).key).toBe("closed");
    // 活跃 run 次之。
    expect(
      schedulingCondition({
        ...base,
        latest_run: { id: "r", attempt_no: 1, status: "running", review_verdict: null, created_at: "" }
      })
    ).toMatchObject({ key: "running", tone: "info" });
    // 产品门禁阻塞（danger）先于依赖等待。
    const blocked = schedulingCondition({
      ...base,
      product_blocked: true,
      deps_total: 2,
      deps_met: 0,
      products: [{ id: "p1", name: "Billing API", status: "suspended" }]
    });
    expect(blocked.key).toBe("blocked_product");
    expect(blocked.tone).toBe("danger");
    expect(blocked.label).toBe("Blocked by Product Suspension");
    expect(blocked.detail).toContain("Billing API");
    // 依赖未满足：x/y met。
    expect(
      schedulingCondition({ ...base, deps_total: 2, deps_met: 1 })
    ).toMatchObject({ key: "waiting", detail: "1/2 met" });
    // 全满足：Ready to Start（scheduled 紫）。
    const ready = schedulingCondition({ ...base, deps_total: 2, deps_met: 2 });
    expect(ready).toMatchObject({ key: "ready", tone: "scheduled" });
    expect(ready.detail).toBe("2/2 met");
    expect(schedulingCondition(base)).toMatchObject({ key: "ready", detail: "No predecessors" });
  });

  it("formats due dates in the design voice and never invents dates", () => {
    expect(formatDue(null)).toBe("");
    const base = new Date("2026-11-21T12:00:00");
    // 日期词形随 locale 变化，只锁设计稿口径的固定前缀。
    expect(formatDue("2026-11-23T00:00:00", base)).toMatch(/^Due /);
    expect(formatDue("2026-11-21T00:00:00", base)).toBe("Due today");
    expect(formatDue("2026-11-20T00:00:00", base)).toContain("Overdue");
  });
});
