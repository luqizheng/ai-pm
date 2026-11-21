// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";

import TasksListView from "./TasksListView.vue";
import { mountShell } from "@/components/shell/test-utils";
import type { Task } from "@/api/types";

/**
 * PA-07 任务列表页（Figma 179:3603 A）行为测试：
 * 列 = code/产品/聚合状态/执行/调度条件；调度条件是派生展示
 * （tasks-contract），Harness 不显示（无数据源不伪装）；右栏 =
 * Execution context（RELATED PRODUCTS + CONTEXT, NOT A BYPASS）。
 */

const listTasks = vi.fn();
const listProjectProducts = vi.fn();

vi.mock("@/api/tasks", () => ({
  listTasks: (...args: unknown[]) => listTasks(...args)
}));

vi.mock("@/api/products", () => ({
  listProjectProducts: (...args: unknown[]) => listProjectProducts(...args)
}));

function task(overrides: Partial<Task>): Task {
  return {
    id: overrides.id ?? "11111111-1111-1111-1111-111111111111",
    code: "PM-001",
    project_id: "proj-1",
    milestone_id: null,
    title: "Implement authentication API",
    description: null,
    status: "draft",
    priority: "urgent",
    assignee_id: null,
    revision: 1,
    planned_end_at: null,
    products: [],
    latest_run: null,
    deps_total: 0,
    deps_met: 0,
    product_blocked: false,
    created_at: "2026-11-20T10:00:00Z",
    updated_at: "2026-11-20T10:00:00Z",
    ...overrides
  };
}

beforeEach(() => {
  listTasks.mockReset();
  listProjectProducts.mockReset();
  listTasks.mockResolvedValue({ items: [], total: 0, page: 1, size: 200 });
  listProjectProducts.mockResolvedValue([]);
});

describe("TasksListView（PA-07 列表）", () => {
  it("renders code / products / aggregated status / execution / scheduling condition", async () => {
    listTasks.mockResolvedValue({
      items: [
        task({
          code: "PM-208",
          title: "Implement authentication API",
          status: "running",
          priority: "urgent",
          planned_end_at: "2026-11-23T00:00:00Z",
          products: [
            { id: "p1", name: "Auth service", status: "normal" },
            { id: "p2", name: "Developer portal", status: "normal" }
          ],
          deps_total: 2,
          deps_met: 2,
          latest_run: {
            id: "run-1",
            attempt_no: 1,
            status: "running",
            review_verdict: null,
            created_at: "2026-11-20T11:00:00Z"
          }
        }),
        task({
          id: "22222222-2222-2222-2222-222222222222",
          code: "PM-209",
          title: "Implement invoice API",
          status: "draft",
          priority: "urgent",
          products: [{ id: "p3", name: "Billing API", status: "suspended" }],
          product_blocked: true,
          deps_total: 0,
          deps_met: 0
        }),
        task({
          id: "33333333-3333-3333-3333-333333333333",
          code: "PM-210",
          title: "Build token audit trail",
          status: "ready",
          priority: "high",
          deps_total: 2,
          deps_met: 2
        })
      ],
      total: 3,
      page: 1,
      size: 200
    });

    const { router, wrapper } = mountShell(TasksListView);
    await router.push("/project/tasks/list");
    await router.isReady();
    await flushPromises();

    const body = wrapper.find("[data-testid='list-count']").text();
    expect(body).toContain("3");

    const rows = wrapper.findAll("[data-test='task-list-row']");
    expect(rows).toHaveLength(3);

    // 调度条件派生：running / blocked（红）/ ready。
    const conditions = wrapper.findAll("[data-condition]").map(c => c.classes());
    expect(conditions.some(c => c.includes("sc-chip--info"))).toBe(true);
    expect(conditions.some(c => c.includes("sc-chip--danger"))).toBe(true);
    expect(conditions.some(c => c.includes("sc-chip--scheduled"))).toBe(true);

    // Harness 不存在也不显示（无数据源不伪装裁决）。
    expect(wrapper.text()).not.toContain("Harness");

    // 五态聚合：running → In progress。
    expect(wrapper.text()).toContain("In progress");
  });

  it("mounts the execution context panel with related products and bypass note", async () => {
    listProjectProducts.mockResolvedValue([
      {
        link_id: "link-1",
        linked_at: "2026-11-20T09:00:00Z",
        product: {
          id: "p1",
          team_id: "t1",
          key: "AUTH",
          name: "Auth service",
          description: null,
          status: "normal"
        },
        task_count: 2
      },
      {
        link_id: "link-2",
        linked_at: "2026-11-20T09:00:00Z",
        product: {
          id: "p2",
          team_id: "t1",
          key: "BILL",
          name: "Billing API",
          description: null,
          status: "suspended"
        },
        task_count: 1
      }
    ]);

    const { router, wrapper } = mountShell(TasksListView);
    await router.push("/project/tasks/list?project_id=proj-1");
    await router.isReady();
    await flushPromises();

    const panel = wrapper.find("[data-testid='execution-context-panel']");
    expect(panel.exists()).toBe(true);
    const products = wrapper.find("[data-testid='panel-products']");
    expect(products.text()).toContain("Auth service");
    expect(products.text()).toContain("Billing API");
    // 暂停产品标受限。
    expect(products.find(".is-restricted").exists()).toBe(true);
  });

  it("client-side filters by scheduling condition (Dependency filter)", async () => {
    listTasks.mockResolvedValue({
      items: [
        task({ status: "ready", deps_total: 2, deps_met: 2 }),
        task({
          id: "22222222-2222-2222-2222-222222222222",
          status: "draft",
          product_blocked: true,
          products: [{ id: "p3", name: "Billing API", status: "suspended" }]
        })
      ],
      total: 2,
      page: 1,
      size: 200
    });

    const { router, wrapper } = mountShell(TasksListView);
    await router.push("/project/tasks/list");
    await router.isReady();
    await flushPromises();
    expect(wrapper.findAll("[data-test='task-list-row']")).toHaveLength(2);

    const select = wrapper.find("[data-testid='filter-dependency'] select");
    if (select.exists()) {
      await select.setValue("blocked_product");
      await flushPromises();
      const rows = wrapper.findAll("[data-test='task-list-row']");
      expect(rows).toHaveLength(1);
      expect(rows[0].text()).toContain("Blocked by Product Suspension");
    }
  });
});
