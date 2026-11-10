// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import RequirementImpactView from "./RequirementImpactView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import type { RequirementImpact } from "@/api/requirement-read";

/**
 * 影响分析（S02-UI-07，接真）行为测试：
 * **只列事实不下结论**——谁绑了本需求、绑的是哪一版、是否已过期。
 * 过期绑定（is_current=false）必须显式标出，不得当成当前依据。
 */

const getRequirementImpact = vi.fn();

vi.mock("@/api/requirement-read", () => ({
  getRequirementImpact: (...args: unknown[]) => getRequirementImpact(...args)
}));

function impact(): RequirementImpact {
  return {
    requirement_id: "req-1",
    code: "REQ-001",
    title: "用户登录",
    current_version: 3,
    designs: [
      {
        kind: "design",
        target_id: "design-1",
        code: "TD-001",
        title: "登录设计",
        bound_version: 2,
        current_version: 3,
        is_current: false,
        target_status: "approved"
      }
    ],
    tasks: [
      {
        kind: "task",
        target_id: "aaaaaaaa-1111-1111-1111-111111111111",
        code: null,
        title: "实现登录端点",
        bound_version: 3,
        current_version: 3,
        is_current: true,
        target_status: "draft"
      }
    ],
    stale_binding_count: 1
  };
}

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/requirements/req-1/impact");
  await router.isReady();
  const { wrapper } = mountShell(
    RequirementImpactView,
    { attachTo: document.body },
    router
  );
  await flushPromises();
  return { wrapper, router };
}

beforeEach(() => {
  getRequirementImpact.mockReset();
  getRequirementImpact.mockResolvedValue(impact());
});

describe("RequirementImpactView", () => {
  it("读真实影响端点并分列设计/任务绑定", async () => {
    const { wrapper } = await mountView();

    expect(getRequirementImpact).toHaveBeenCalledWith("req-1");
    expect(wrapper.find('[data-testid="ri-design-TD-001"]').text()).toContain(
      "登录设计"
    );
    expect(wrapper.text()).toContain("实现登录端点");
  });

  it("过期绑定显式标出并给缺口提示", async () => {
    const { wrapper } = await mountView();

    expect(wrapper.text()).toContain("存在过期绑定");
    expect(wrapper.find('[data-testid="ri-design-TD-001"]').text()).toContain(
      "已过期"
    );
  });
});
