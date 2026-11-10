// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import AdrDetailView from "./AdrDetailView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import { useShellStore } from "@/stores/shell";
import type { AdrDetail } from "@/api/designs";

/**
 * S03 架构决策详情行为测试：
 * 决策内容按 ADR 修订读取（决策记录不可变，改决策 = 新版本 + 显式替代），
 * 并呈现影响关系（关联设计/需求）。
 */

const getAdr = vi.fn();
const getDesignWorkspace = vi.fn();

vi.mock("@/api/designs", () => ({
  getAdr: (...args: unknown[]) => getAdr(...args),
  getDesignWorkspace: (...args: unknown[]) => getDesignWorkspace(...args)
}));

function detail(): AdrDetail {
  return {
    adr: {
      id: "adr-1",
      project_id: "proj-1",
      code: "ADR-001",
      title: "主键策略",
      status: "accepted",
      blockers_implementation: false,
      current_revision_id: "adr-rev-1",
      current_version: 1,
      superseded_by_adr_id: null,
      row_revision: 1,
      created_at: "2026-11-10T08:00:00Z",
      updated_at: "2026-11-10T09:00:00Z"
    },
    revisions: [
      {
        id: "adr-rev-1",
        adr_id: "adr-1",
        version: 1,
        content: {
          decision_question: "主键用什么？",
          context: "需要跨表唯一",
          options: ["UUID", "自增整数"],
          chosen_option: "UUID",
          rationale: "分布式友好",
          consequences: "索引变大"
        },
        state: "submitted",
        content_hash: "hash-1",
        created_by: "p-1",
        created_at: "2026-11-10T08:30:00Z"
      }
    ],
    design_links: [
      {
        adr_revision_id: "adr-rev-1",
        design_id: "design-1",
        design_revision_id: "rev-2",
        design_code: "TD-001",
        design_revision_version: 2
      }
    ],
    requirement_links: [
      {
        adr_revision_id: "adr-rev-1",
        requirement_id: "req-1",
        requirement_revision_id: "req-rev-1",
        requirement_code: "REQ-001",
        requirement_revision_version: 3
      }
    ],
    approvals: []
  };
}

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/design/adrs/adr-1");
  await router.isReady();
  const { wrapper } = mountShell(
    AdrDetailView,
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
  getAdr.mockReset();
  getDesignWorkspace.mockReset();
  getDesignWorkspace.mockResolvedValue({
    design_count: 1,
    adr_count: 1,
    baseline_count: 0,
    pending_approval_count: 0,
    open_finding_count: 0,
    linked_requirement_count: 1,
    stale_requirement_count: 0,
    design_states: {},
    adr_states: {}
  });
  getAdr.mockResolvedValue(detail());
});

describe("AdrDetailView", () => {
  it("呈现决策内容与影响关系（关联设计、需求）", async () => {
    const { wrapper } = await mountView();

    expect(getAdr).toHaveBeenCalledWith("adr-1");
    const text = wrapper.text();
    expect(text).toContain("主键用什么？");
    expect(text).toContain("UUID");
    expect(text).toContain("分布式友好");
    expect(text).toContain("TD-001");
    expect(text).toContain("REQ-001");
  });
});
