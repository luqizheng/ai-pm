// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { mountShell } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";
import { buildRoutes } from "@/router";

import FeedsView from "./FeedsView.vue";

/**
 * 统一信息流页契约（PA-05 UI 01/02/03）：
 *   - 三视角 tabs（My/Product/Project）；My 视角 = 未读徽标 + Mark all +
 *     All/Unread/Needs attention 视图条 + 筛选栏；
 *   - 活动卡：来源/时间/类型/未读徽标/标题/摘要/关联引用/行内展开；
 *   - Product/Project 视角未选 scope → 空态引导。
 */
const mountedWrappers: ReturnType<typeof mountShell>[] = [];

const fetchMyFeed = vi.fn();
const fetchProductFeed = vi.fn();
const fetchProjectFeed = vi.fn();
const markAllFeedRead = vi.fn();
const listProducts = vi.fn();
const listProjects = vi.fn();
const fetchTeamContext = vi.fn();

vi.mock("@/api/feeds", () => ({
  fetchMyFeed: (...args: unknown[]) => fetchMyFeed(...args),
  fetchProductFeed: (...args: unknown[]) => fetchProductFeed(...args),
  fetchProjectFeed: (...args: unknown[]) => fetchProjectFeed(...args),
  markAllFeedRead: (...args: unknown[]) => markAllFeedRead(...args),
  markFeedRead: vi.fn()
}));

vi.mock("@/api/products", () => ({
  listProducts: (...args: unknown[]) => listProducts(...args)
}));

vi.mock("@/api/pm", () => ({
  listProjects: (...args: unknown[]) => listProjects(...args)
}));

vi.mock("@/api/team", () => ({
  fetchTeamContext: (...args: unknown[]) => fetchTeamContext(...args)
}));

vi.mock("@/components/ui/toast", () => ({
  useToast: () => ({ toast: vi.fn() })
}));

function feedRow(overrides: Record<string, unknown> = {}) {
  return {
    id: "act-1",
    item_id: "w-1",
    target_type: "task",
    target_id: "t-1",
    item_title: "Validate retry behavior",
    activity_type: "status_changed",
    actor_type: "agent",
    actor_id: null,
    summary: "24 checks passed; no workflow state changed.",
    detail: { from: "in_progress", to: "done" },
    project_id: "p-1",
    product_id: null,
    created_at: "2026-11-26T10:28:00Z",
    read_at: null,
    ...overrides
  };
}

async function mountAt() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/feeds");
  await router.isReady();
  const wrapper = mountShell(FeedsView, {
    global: { plugins: [router] }
  });
  (mountedWrappers as unknown as VueWrapper[]).push(wrapper as VueWrapper);
  await flushPromises();
  return wrapper;
}

afterEach(() => {
  for (const wrapper of mountedWrappers.splice(0)) {
    wrapper.unmount();
  }
  document.body.innerHTML = "";
});

beforeEach(() => {
  vi.clearAllMocks();
  i18n.global.locale.value = "en";
  fetchTeamContext.mockResolvedValue({
    principal_id: "p-maya",
    display_name: "Maya Chen",
    is_active: true,
    is_admin: false,
    tenant_id: "t-9",
    team_role: "owner",
    team: { id: "tm-1", name: "PE", slug: null, description: null },
    onboarding_required: false,
    memberships: []
  });
  listProducts.mockResolvedValue([]);
  listProjects.mockResolvedValue({ items: [] });
  fetchMyFeed.mockResolvedValue({
    items: [
      feedRow(),
      feedRow({
        id: "act-2",
        item_id: "w-2",
        activity_type: "created",
        actor_type: "human",
        read_at: "2026-11-26T11:00:00Z",
        detail: null
      })
    ],
    counts: { total: 48, unread: 12, attention: 3 }
  });
});

describe("FeedsView", () => {
  it("renders my feed with view tabs, unread badge and activity cards", async () => {
    const wrapper = await mountAt();
    // 页头徽标 + Mark all as read（07 并入）。
    expect(wrapper.text()).toContain("12 unread · only you");
    expect(wrapper.find("[data-test='feeds-mark-all']").exists()).toBe(true);
    // 视图条三视图。
    expect(wrapper.text()).toContain("All activity");
    expect(wrapper.text()).toContain("Unread · 12");
    expect(wrapper.text()).toContain("Needs attention · 3");
    // 活动卡：未读徽标 + AI 来源 + 标题 + 关联引用。
    expect(wrapper.text()).toContain("Validate retry behavior");
    expect(wrapper.text()).toContain("Unread");
    expect(wrapper.text()).toContain("task · Validate retry behavior");
    // 请求带默认视图参数。
    expect(fetchMyFeed).toHaveBeenCalledWith(
      expect.objectContaining({ limit: 20, offset: 0, view: "all" })
    );
  });

  it("expands activity detail inline (canvas Expand ↓)", async () => {
    const wrapper = await mountAt();
    expect(wrapper.text()).not.toContain('"from"');
    await wrapper
      .find("[data-testid='feed-item-act-1'] .fd-item__expand")
      .trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain('"from"');
  });

  it("switches view to unread and refetches (needs-attention view included)", async () => {
    const wrapper = await mountAt();
    const unreadTab = wrapper
      .findAll(".fd-views__item")
      .find(button => button.text().startsWith("Unread"));
    await unreadTab!.trigger("click");
    await flushPromises();
    expect(fetchMyFeed).toHaveBeenLastCalledWith(
      expect.objectContaining({ view: "unread" })
    );
  });

  it("marks all as read and reloads", async () => {
    markAllFeedRead.mockResolvedValue(12);
    const wrapper = await mountAt();
    await wrapper.find("[data-test='feeds-mark-all']").trigger("click");
    await flushPromises();
    expect(markAllFeedRead).toHaveBeenCalled();
    expect(fetchMyFeed).toHaveBeenCalledTimes(2);
  });

  it("guides scope selection on product/project views", async () => {
    const wrapper = await mountAt();
    const productTab = wrapper
      .findAll(".fd-tabs__item")
      .find(button => button.text() === "Product feed");
    await productTab!.trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("No activity yet");
    expect(fetchProductFeed).not.toHaveBeenCalled();
  });
});
