// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { mountShell } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";
import { buildRoutes } from "@/router";

import WorkItemDetailView from "./WorkItemDetailView.vue";
import ActivityDetailView from "./ActivityDetailView.vue";

/**
 * 详情页契约（PA-05 UI 屏 04/05/06）：
 *   - Work item detail：type/状态徽标 + Relationships（双向+对端引用）+
 *     Timeline/Execution logs 页签（logs 仅 task）；
 *   - Activity detail：眉行+actor/时间+变更明细+受影响事项+个人状态说明
 *     （打开即标已读由后端完成）。
 */
const ITEM_ID = "00000000-0000-0000-0000-000000000001";
const ACTIVITY_ID = "00000000-0000-0000-0000-000000000002";

const fetchWorkItemDetail = vi.fn();
const fetchExecutionLogs = vi.fn();
const fetchActivityDetail = vi.fn();

vi.mock("@/api/feeds", () => ({
  fetchWorkItemDetail: (...args: unknown[]) => fetchWorkItemDetail(...args),
  fetchExecutionLogs: (...args: unknown[]) => fetchExecutionLogs(...args),
  fetchActivityDetail: (...args: unknown[]) => fetchActivityDetail(...args),
  fetchMyFeed: vi.fn(),
  fetchProductFeed: vi.fn(),
  fetchProjectFeed: vi.fn(),
  markAllFeedRead: vi.fn(),
  markFeedRead: vi.fn()
}));

function itemDetail() {
  return {
    item: {
      id: ITEM_ID,
      target_type: "task",
      target_id: "t-1",
      title: "Validate retry behavior",
      status: "in_progress",
      project_id: "p-1",
      created_at: "2026-11-20T00:00:00Z",
      updated_at: "2026-11-26T10:28:00Z"
    },
    relations: [
      {
        id: "r-1",
        direction: "outgoing",
        relation_type: "implements",
        other_item_id: ITEM_ID,
        other_type: "requirement",
        other_title: "Adaptive retry policy",
        other_status: "open",
        origin: "human",
        reason: "pinned approved requirement revision"
      }
    ],
    activities: [
      {
        id: ACTIVITY_ID,
        item_id: ITEM_ID,
        target_type: "task",
        target_id: "t-1",
        item_title: "Validate retry behavior",
        activity_type: "status_changed",
        actor_type: "human",
        actor_id: null,
        summary: "Detail task: open → in_progress",
        detail: null,
        project_id: "p-1",
        product_id: null,
        created_at: "2026-11-26T10:00:00Z",
        read_at: null
      }
    ]
  };
}

async function mountAt(
  component: typeof WorkItemDetailView | typeof ActivityDetailView,
  path: string
) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push(path);
  await router.isReady();
  const wrapper = mountShell(component, {
    global: { plugins: [router] }
  });
  await flushPromises();
  return wrapper;
}

afterEach(() => {
  document.body.innerHTML = "";
});

beforeEach(() => {
  vi.clearAllMocks();
  i18n.global.locale.value = "en";
  fetchWorkItemDetail.mockResolvedValue(itemDetail());
  fetchExecutionLogs.mockResolvedValue([
    {
      id: "log-1",
      run_id: "00000000-0000-0000-0000-0000000000aa",
      event_type: "run_started",
      occurred_at: "2026-11-26T10:28:11Z",
      detail: null
    }
  ]);
  fetchActivityDetail.mockResolvedValue({
    id: ACTIVITY_ID,
    item_id: ITEM_ID,
    target_type: "task",
    target_id: "t-1",
    item_title: "Validate retry behavior",
    activity_type: "status_changed",
    actor_type: "human",
    actor_id: null,
    summary: "Detail task: open → in_progress",
    detail: { from: "open", to: "in_progress" },
    project_id: "p-1",
    product_id: null,
    created_at: "2026-11-26T10:00:00Z",
    read_at: "2026-11-26T11:00:00Z"
  });
});

describe("WorkItemDetailView", () => {
  it("renders badges, relationships and timeline/logs tabs (04+06)", async () => {
    const wrapper = await mountAt(
      WorkItemDetailView,
      `/feeds/work-items/${ITEM_ID}`
    );
    const badges = wrapper.find("[data-testid='work-item-badges']");
    expect(badges.text()).toContain("task");
    expect(badges.text()).toContain("in_progress");
    const relations = wrapper.find("[data-testid='work-item-relations']");
    expect(relations.text()).toContain("implements");
    expect(relations.text()).toContain("requirement · Adaptive retry policy");
    // 06 页签默认 Timeline；Logs 页签仅 task 显示且可切换。
    expect(wrapper.find("[data-testid='work-item-tabs']").text()).toContain(
      "Execution logs"
    );
    expect(wrapper.text()).toContain("Detail task: open → in_progress");
    const logsTab = wrapper
      .findAll(".wd-tabs__item")
      .find(button => button.text() === "Execution logs");
    await logsTab!.trigger("click");
    await flushPromises();
    expect(wrapper.find("[data-testid^='exec-log-']").exists()).toBe(true);
    expect(wrapper.text()).toContain("run_started");
  });

  it("hides logs tab for non-task items", async () => {
    fetchWorkItemDetail.mockResolvedValue({
      ...itemDetail(),
      item: { ...itemDetail().item, target_type: "adr" }
    });
    const wrapper = await mountAt(
      WorkItemDetailView,
      `/feeds/work-items/${ITEM_ID}`
    );
    expect(wrapper.text()).not.toContain("Execution logs");
  });
});

describe("ActivityDetailView", () => {
  it("renders the historical record with change summary and affected item (05)", async () => {
    const wrapper = await mountAt(
      ActivityDetailView,
      `/feeds/activities/${ACTIVITY_ID}`
    );
    expect(fetchActivityDetail).toHaveBeenCalledWith(ACTIVITY_ID);
    expect(wrapper.text()).toContain("Historical activity / status_changed");
    expect(wrapper.text()).toContain("Detail task: open → in_progress");
    expect(wrapper.text()).toContain("Change summary");
    expect(wrapper.text()).toContain('"from": "open"');
    expect(wrapper.text()).toContain("task · Validate retry behavior");
    // PA-05-09 口径常驻：已读不等于处理/批准。
    expect(wrapper.text()).toContain(
      "it does not complete or approve the work"
    );
  });
});
