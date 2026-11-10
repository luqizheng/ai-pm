// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import TechnicalDesignsView from "./TechnicalDesignsView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import { useShellStore } from "@/stores/shell";
import type {
  DesignAnalysisRow,
  DesignApprovalInboxRow,
  DesignEntry
} from "@/api/designs";

/**
 * S03 技术设计列表页行为测试：
 * 「评审状态」列来自项目级审批收件箱——**待裁决 ≠ 已批准**：
 * pending 审批只表示等待人类裁决，已批准看的是修订 state=approved。
 */

const listDesigns = vi.fn();
const listDesignApprovals = vi.fn();
const listDesignAnalysis = vi.fn();
const getDesignWorkspace = vi.fn();

vi.mock("@/api/designs", () => ({
  listDesigns: (...args: unknown[]) => listDesigns(...args),
  listDesignApprovals: (...args: unknown[]) => listDesignApprovals(...args),
  listDesignAnalysis: (...args: unknown[]) => listDesignAnalysis(...args),
  getDesignWorkspace: (...args: unknown[]) => getDesignWorkspace(...args)
}));

function design(overrides: Partial<DesignEntry> = {}): DesignEntry {
  return {
    id: "design-1",
    project_id: "proj-1",
    code: "TD-001",
    title: "登录设计",
    lifecycle_status: "active",
    row_revision: 1,
    current_revision_id: "rev-2",
    current_version: 2,
    current_state: "approved",
    latest_version: 3,
    latest_state: "pending_confirmation",
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
    kind: "design_revision",
    design_id: "design-1",
    adr_id: null,
    baseline_id: null,
    target_id: "rev-3",
    target_hash: "hash-1",
    status: "pending",
    requested_by: "p-1",
    requested_by_name: "张三",
    decided_by: null,
    decided_by_name: null,
    decision_reason: null,
    decided_at: null,
    created_at: "2026-11-10T09:00:00Z",
    target_code: "TD-001",
    target_title: "登录设计",
    ...overrides
  };
}

function analysis(
  overrides: Partial<DesignAnalysisRow> = {}
): DesignAnalysisRow {
  return {
    design_id: "design-1",
    code: "TD-001",
    title: "登录设计",
    lifecycle_status: "active",
    current_revision_id: "rev-2",
    current_version: 2,
    current_state: "approved",
    requirement_count: 0,
    stale_requirement_count: 0,
    open_finding_count: 0,
    requirements: [],
    adrs: [],
    ...overrides
  };
}

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/design/technical-designs");
  await router.isReady();
  const { wrapper } = mountShell(
    TechnicalDesignsView,
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
  listDesigns.mockReset();
  listDesignApprovals.mockReset();
  listDesignAnalysis.mockReset();
  getDesignWorkspace.mockReset();
  getDesignWorkspace.mockResolvedValue({
    design_count: 1,
    adr_count: 0,
    baseline_count: 0,
    pending_approval_count: 0,
    open_finding_count: 0,
    linked_requirement_count: 0,
    stale_requirement_count: 0,
    design_states: {},
    adr_states: {}
  });
  listDesigns.mockResolvedValue([design()]);
  listDesignApprovals.mockResolvedValue([approval()]);
  listDesignAnalysis.mockResolvedValue([analysis({ open_finding_count: 2 })]);
});

describe("TechnicalDesignsView", () => {
  it("分列当前正式版本与最新版本，并把待裁决审批与未闭问题显式呈现", async () => {
    const { wrapper } = await mountView();

    expect(listDesigns).toHaveBeenCalledWith("proj-1");
    expect(listDesignApprovals).toHaveBeenCalledWith("proj-1");
    const row = wrapper.find('[data-testid="design-row-TD-001"]');
    expect(row.exists()).toBe(true);
    const text = row.text();
    // 当前正式版本 v2 已批准；最新版本 v3 待确认——两者不可混为一谈
    expect(text).toContain("v2");
    expect(text).toContain("v3");
    expect(text).toContain("待裁决 1");
    expect(text).toContain("未闭 2");
  });

  it("按修订状态筛选设计列表", async () => {
    const { wrapper } = await mountView();

    await wrapper.find('[data-testid="designs-filter-draft"]').trigger("click");
    await flushPromises();

    expect(wrapper.find('[data-testid="design-row-TD-001"]').exists()).toBe(
      false
    );
  });
});
