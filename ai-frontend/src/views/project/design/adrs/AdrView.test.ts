// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import AdrView from "./AdrView.vue";
import { mountShell, waitForRoute } from "@/components/shell/test-utils";
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
const createAdr = vi.fn();

vi.mock("@/api/designs", () => ({
  listAdrs: (...args: unknown[]) => listAdrs(...args),
  listDesignApprovals: (...args: unknown[]) => listDesignApprovals(...args),
  getDesignWorkspace: (...args: unknown[]) => getDesignWorkspace(...args),
  createAdr: (...args: unknown[]) => createAdr(...args)
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
  createAdr.mockReset();
  createAdr.mockResolvedValue({
    adr: adr({ id: "adr-9", code: "ADR-002", title: "缓存决策" }),
    revisions: [],
    design_links: [],
    requirement_links: [],
    approvals: []
  });
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

  // ── R4/B04-02：新建 ADR 写入口 ─────────────────────────────────────

  it("新建 ADR 调真实命令并跳转详情", async () => {
    const { wrapper, router } = await mountView();

    await wrapper.find('[data-testid="adrs-create"]').trigger("click");
    await wrapper.find('[data-testid="adr-create-title"] input').setValue("缓存决策");
    await wrapper
      .find('[data-testid="adr-create-question"] input')
      .setValue("是否引入分布式缓存？");
    await wrapper.find('[data-testid="adr-create-submit"]').trigger("click");
    await flushPromises();
    await waitForRoute(router, "/project/design/adrs/adr-9");

    expect(createAdr).toHaveBeenCalledWith("proj-1", {
      title: "缓存决策",
      decision_question: "是否引入分布式缓存？",
      context: undefined,
      rationale: undefined,
      consequences: undefined,
      blockers_implementation: false
    });
    expect(router.currentRoute.value.path).toBe("/project/design/adrs/adr-9");
  });

  it("决策问题缺失时禁止提交", async () => {
    const { wrapper } = await mountView();

    await wrapper.find('[data-testid="adrs-create"]').trigger("click");
    await wrapper.find('[data-testid="adr-create-title"] input').setValue("只有标题");
    await wrapper.find('[data-testid="adr-create-submit"]').trigger("click");
    await flushPromises();

    expect(createAdr).not.toHaveBeenCalled();
  });
});
