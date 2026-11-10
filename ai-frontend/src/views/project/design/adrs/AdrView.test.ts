// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import AdrView from "./AdrView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import { useShellStore } from "@/stores/shell";
import type { AdrEntry, DesignApprovalInboxRow } from "@/api/designs";

/**
 * S03 架构决策列表页行为测试：
 * 决策状态六态与「实施阻塞」标记必须可区分——阻塞决策会挡住 Readiness
 * Gate（条件 3），因此不能只显示状态色。
 */

const listAdrs = vi.fn();
const listDesignApprovals = vi.fn();
const getDesignWorkspace = vi.fn();

vi.mock("@/api/designs", () => ({
  listAdrs: (...args: unknown[]) => listAdrs(...args),
  listDesignApprovals: (...args: unknown[]) => listDesignApprovals(...args),
  getDesignWorkspace: (...args: unknown[]) => getDesignWorkspace(...args)
}));

function adr(overrides: Partial<AdrEntry> = {}): AdrEntry {
  return {
    id: "adr-1",
    project_id: "proj-1",
    code: "ADR-001",
    title: "主键策略",
    status: "under_review",
    blockers_implementation: true,
    current_revision_id: "adr-rev-1",
    current_version: 1,
    superseded_by_adr_id: null,
    row_revision: 1,
    created_at: "2026-11-10T08:00:00Z",
    updated_at: "2026-11-10T09:00:00Z",
    ...overrides
  };
}

function approval(
  overrides: Partial<DesignApprovalInboxRow> = {}
): DesignApprovalInboxRow {
  return {
    id: "ap-1",
    kind: "adr_acceptance",
    design_id: null,
    adr_id: "adr-1",
    baseline_id: null,
    target_id: "adr-rev-1",
    target_hash: "hash-1",
    status: "pending",
    requested_by: "p-1",
    requested_by_name: "张三",
    decided_by: null,
    decided_by_name: null,
    decision_reason: null,
    decided_at: null,
    created_at: "2026-11-10T09:00:00Z",
    target_code: "ADR-001",
    target_title: "主键策略",
    ...overrides
  };
}

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/design/adrs");
  await router.isReady();
  const { wrapper } = mountShell(AdrView, { attachTo: document.body }, router);
  const shell = useShellStore();
  shell.setProjects([{ id: "proj-1", name: "评测项目" }]);
  shell.setCurrentProject("proj-1");
  await flushPromises();
  return { wrapper, router };
}

beforeEach(() => {
  listAdrs.mockReset();
  listDesignApprovals.mockReset();
  getDesignWorkspace.mockReset();
  getDesignWorkspace.mockResolvedValue({
    design_count: 0,
    adr_count: 1,
    baseline_count: 0,
    pending_approval_count: 1,
    open_finding_count: 0,
    linked_requirement_count: 0,
    stale_requirement_count: 0,
    design_states: {},
    adr_states: {}
  });
  listAdrs.mockResolvedValue([adr()]);
  listDesignApprovals.mockResolvedValue([approval()]);
});

describe("AdrView", () => {
  it("呈现决策状态、实施阻塞标记与待裁决审批", async () => {
    const { wrapper } = await mountView();

    expect(listAdrs).toHaveBeenCalledWith("proj-1");
    const row = wrapper.find('[data-testid="adr-row-ADR-001"]');
    expect(row.exists()).toBe(true);
    const text = row.text();
    expect(text).toContain("评审中");
    expect(text).toContain("阻塞实施");
    expect(text).toContain("待裁决 1");
  });

  it("按决策状态筛选", async () => {
    const { wrapper } = await mountView();

    await wrapper.find('[data-testid="adrs-filter-accepted"]').trigger("click");
    await flushPromises();

    expect(wrapper.find('[data-testid="adr-row-ADR-001"]').exists()).toBe(
      false
    );
  });
});
