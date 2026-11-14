// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";

const {
  reviewApi,
  discoveryApi,
  knowledgeApi,
  acceptanceApi,
  reviewDecisionApi
} = vi.hoisted(() => ({
  reviewApi: { loadHumanActionBlock: vi.fn() },
  discoveryApi: {
    confirmDiscoveryFinding: vi.fn(),
    getDiscoverySession: vi.fn()
  },
  knowledgeApi: { decidePcProposal: vi.fn(), getPcProposal: vi.fn() },
  acceptanceApi: { listTaskDelivery: vi.fn() },
  reviewDecisionApi: {
    decideReviewApproval: vi.fn(),
    listTaskReviewApprovals: vi.fn(),
    listCriterionVerdicts: vi.fn()
  }
}));

vi.mock("@/api/reviewCenter", async importOriginal => ({
  ...(await importOriginal<object>()),
  loadHumanActionBlock: reviewApi.loadHumanActionBlock
}));
vi.mock("@/api/discovery", () => discoveryApi);
vi.mock("@/api/knowledge", () => knowledgeApi);
vi.mock("@/api/acceptance", () => acceptanceApi);
vi.mock("@/api/review", () => reviewDecisionApi);

import ReviewCenterPage from "./ReviewCenterPage.vue";
import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";
import type { ActionItem } from "@/api/command-center";

const t = (key: string, params?: Record<string, unknown>) =>
  i18n.global.t(key, params ?? {}) as string;

function item(overrides: Partial<ActionItem> = {}): ActionItem {
  return {
    kind: "finding_pending",
    domain: "discovery",
    id: "f-1",
    project_id: "p-1",
    title: "发现待处理（architecture）",
    detail: "发现内容摘要",
    source_state: "PENDING",
    updated_at: "2026-11-15T10:00:00Z",
    action_hint: "确认或拒绝该发现",
    link: {
      kind: "discovery_finding",
      params: { finding_id: "f-1", session_id: "s-1" }
    },
    ...overrides
  };
}

const aggregate = (items: ActionItem[]) => ({
  status: "ok" as const,
  asOf: "2026-11-15T10:05:00Z",
  total: items.length,
  items
});

/** 人工审核中心页面：队列渲染 / Drawer 决策闭环 / URL 恢复。 */
describe("ReviewCenterPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    reviewApi.loadHumanActionBlock.mockResolvedValue(aggregate([item()]));
    discoveryApi.getDiscoverySession.mockResolvedValue({
      session: {},
      messages: [],
      findings: [
        {
          id: "f-1",
          session_id: "s-1",
          kind: "architecture",
          content: "缺少鉴权层",
          confidence: 0.8,
          verification_status: "PENDING",
          evidence: [],
          actor_role: null,
          repository_ids: [],
          created_at: "2026-11-15T10:00:00Z"
        }
      ]
    });
  });

  async function openPage(route = "/project/review-center?project_id=p-1") {
    // 先导航后挂载：模拟真实直达（onMounted 时 query 已就绪）；
    // Drawer 走 Teleport 到 body：stub 掉让内容留在 wrapper 内可查询
    const { createRouter, createMemoryHistory } = await import("vue-router");
    const { buildRoutes } = await import("@/router");
    const router = createRouter({
      history: createMemoryHistory(),
      routes: buildRoutes()
    });
    await router.push(route);
    await router.isReady();
    const mounted = mountShell(
      ReviewCenterPage,
      { global: { stubs: { teleport: true } } },
      router
    );
    await flushPromises();
    return mounted;
  }

  it("渲染混排队列（kind 标签 + 状态 + meta），列表无决策按钮（D4）", async () => {
    const { wrapper } = await openPage();
    const text = wrapper.text();
    expect(text).toContain(t("review.page.title"));
    expect(text).toContain("发现待处理（architecture）");
    expect(text).toContain(t("review.kind.finding_pending"));
    expect(text).toContain(t("review.sourceState.PENDING"));

    const queue = wrapper.find(".review-page__queue");
    expect(queue.exists()).toBe(true);
    // D4：队列卡片不出现决策区（决策区只在 Drawer 内渲染）
    expect(wrapper.find(".review-decision").exists()).toBe(false);
    expect(queue.text()).not.toContain(t("review.reason.findingLabel"));
  });

  it("选中事项打开 Drawer 并加载领域上下文；answer 必填（R05/R06）", async () => {
    const { wrapper } = await openPage();
    await wrapper.find(".review-item").trigger("click");
    await flushPromises();

    const drawer = wrapper.find(".review-drawer");
    expect(drawer.exists()).toBe(true);
    expect(drawer.text()).toContain("缺少鉴权层");
    expect(drawer.text()).toContain(t("review.reason.findingLabel"));
    expect(discoveryApi.getDiscoverySession).toHaveBeenCalledWith("p-1", "s-1");

    // answer 为空 → 确认按钮禁用（R05 必填）
    const buttons = drawer.findAll("button");
    const confirmButton = buttons.find(button =>
      button.text().includes(t("review.finding.confirm"))
    );
    expect(confirmButton?.attributes("disabled")).toBeDefined();

    await wrapper
      .find(".review-reason textarea")
      .setValue("依据：鉴权模块缺失");
    await flushPromises();
    // 从 wrapper 重新查找（重渲染后旧 wrapper 可能指向被替换的元素），
    // disabled 应已解除（R05 必填已满足）
    const confirmAfter = wrapper
      .find(".review-drawer")
      .findAll("button")
      .find(button => button.text().includes(t("review.finding.confirm")));
    expect(confirmAfter?.attributes("disabled")).toBeUndefined();

    await confirmAfter!.trigger("click");
    await flushPromises();
    expect(discoveryApi.confirmDiscoveryFinding).toHaveBeenCalledWith(
      "p-1",
      "s-1",
      "f-1",
      { status: "CONFIRMED", answer: "依据：鉴权模块缺失" }
    );
  });

  it("Drawer 关闭清 item 留 kind（URL 语义，V1.1 §1.1）", async () => {
    const { wrapper, router } = await openPage(
      "/project/review-center?project_id=p-1&kind=finding&item=finding_pending%3Af-1"
    );
    await flushPromises();
    expect(wrapper.find(".review-drawer").exists()).toBe(true);

    await wrapper.find(".review-drawer__close").trigger("click");
    await flushPromises();
    expect(wrapper.find(".review-drawer").exists()).toBe(false);
    expect(router.currentRoute.value.query.kind).toBe("finding");
    expect(router.currentRoute.value.query.item).toBeUndefined();
  });
});
