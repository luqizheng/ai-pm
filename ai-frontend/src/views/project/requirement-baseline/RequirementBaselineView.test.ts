// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import RequirementBaselineView from "./RequirementBaselineView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import { useShellStore } from "@/stores/shell";
import type { RequirementBaseline } from "@/api/requirement-read";

/**
 * 需求基线（S02-UI-04，接真）行为测试：
 * 基线 = 已确认需求的当前正式修订集合（派生视图，无冻结动作）；
 * 未确认需求**在基线表里但状态可辨**，不得当成已入基线。
 */

const getRequirementBaseline = vi.fn();

vi.mock("@/api/requirement-read", () => ({
  getRequirementBaseline: (...args: unknown[]) =>
    getRequirementBaseline(...args)
}));

function baseline(): RequirementBaseline {
  return {
    items: [
      {
        requirement_id: "req-1",
        code: "REQ-001",
        title: "用户登录",
        requirement_type: "functional",
        priority: "high",
        lifecycle_status: "active",
        revision_id: "rev-1",
        version: 3,
        state: "approved",
        content_hash: "h1",
        confirmed_at: "2026-11-10T09:00:00Z",
        confirmed_by: "张三",
        acceptance_criteria_count: 2,
        source_count: 1
      },
      {
        requirement_id: "req-2",
        code: "REQ-002",
        title: "口令策略",
        requirement_type: "functional",
        priority: "normal",
        lifecycle_status: "active",
        revision_id: "rev-2",
        version: 1,
        state: "pending_confirmation",
        content_hash: "h2",
        confirmed_at: null,
        confirmed_by: null,
        acceptance_criteria_count: 0,
        source_count: 0
      }
    ],
    confirmed_count: 1,
    total_count: 2,
    unconfirmed_count: 1
  };
}

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/requirements/baselines");
  await router.isReady();
  const { wrapper } = mountShell(
    RequirementBaselineView,
    { attachTo: document.body },
    router
  );
  const shell = useShellStore();
  shell.setProjects([{ id: "proj-1", name: "评测项目" }]);
  shell.setCurrentProject("proj-1");
  await flushPromises();
  return { wrapper, router };
}

beforeEach(() => {
  getRequirementBaseline.mockReset();
  getRequirementBaseline.mockResolvedValue(baseline());
});

describe("RequirementBaselineView", () => {
  it("读真实基线端点，已确认与未确认分列且计数一致", async () => {
    const { wrapper } = await mountView();

    expect(getRequirementBaseline).toHaveBeenCalledWith("proj-1");
    const stats = wrapper.find('[data-testid="rb-stats"]').text();
    expect(stats).toContain("2"); // 总数
    expect(stats).toContain("1"); // 已确认
    expect(wrapper.find('[data-testid="rb-row-REQ-001"]').text()).toContain(
      "已确认"
    );
    expect(wrapper.find('[data-testid="rb-row-REQ-002"]').text()).toContain(
      "待确认"
    );
  });

  it("存在未确认需求时给出缺口提示", async () => {
    const { wrapper } = await mountView();

    expect(wrapper.text()).toContain("存在未确认需求");
  });

  it("无需求时给空态", async () => {
    getRequirementBaseline.mockResolvedValue({
      items: [],
      confirmed_count: 0,
      total_count: 0,
      unconfirmed_count: 0
    });
    const { wrapper } = await mountView();

    expect(wrapper.text()).toContain("暂无需求");
  });
});
