// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import TechnicalDesignDetailView from "./TechnicalDesignDetailView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import { useShellStore } from "@/stores/shell";
import type {
  DesignDetail,
  DesignRequirementLink,
  ReviewFinding
} from "@/api/designs";

/**
 * S03 技术设计详情行为测试：
 * 版本链默认选中**当前正式版本**（design.current_revision_id），并按该
 * 版本读需求覆盖与评审问题——版本不可变，不同版本的关联不可混读。
 */

const getDesign = vi.fn();
const listDesignRequirements = vi.fn();
const listDesignFindings = vi.fn();
const getDesignWorkspace = vi.fn();

vi.mock("@/api/designs", () => ({
  getDesign: (...args: unknown[]) => getDesign(...args),
  listDesignRequirements: (...args: unknown[]) =>
    listDesignRequirements(...args),
  listDesignFindings: (...args: unknown[]) => listDesignFindings(...args),
  getDesignWorkspace: (...args: unknown[]) => getDesignWorkspace(...args)
}));

function detail(): DesignDetail {
  return {
    design: {
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
      latest_state: "draft",
      created_at: "2026-11-10T08:00:00Z",
      updated_at: "2026-11-10T09:00:00Z"
    },
    revisions: [
      {
        id: "rev-3",
        design_id: "design-1",
        version: 3,
        state: "draft",
        content: { markdown: "# v3 正文" },
        content_hash: "hash-3",
        source: "human",
        analysis_run_id: null,
        base_current_revision_id: "rev-2",
        created_by: "p-1",
        created_by_name: "张三",
        submitted_at: null,
        approved_by: null,
        approved_by_name: null,
        approved_at: null,
        created_at: "2026-11-10T10:00:00Z"
      },
      {
        id: "rev-2",
        design_id: "design-1",
        version: 2,
        state: "approved",
        content: { markdown: "# v2 正文", summary: "当前正式版本" },
        content_hash: "hash-2",
        source: "ai",
        analysis_run_id: null,
        base_current_revision_id: null,
        created_by: "p-2",
        created_by_name: "李四",
        submitted_at: "2026-11-10T08:30:00Z",
        approved_by: "p-1",
        approved_by_name: "张三",
        approved_at: "2026-11-10T08:40:00Z",
        created_at: "2026-11-10T08:20:00Z"
      }
    ],
    approvals: []
  };
}

function requirementLink(
  overrides: Partial<DesignRequirementLink> = {}
): DesignRequirementLink {
  return {
    design_revision_id: "rev-2",
    requirement_id: "req-1",
    requirement_revision_id: "req-rev-1",
    requirement_code: "REQ-001",
    requirement_version: 3,
    requirement_revision_state: "approved",
    is_current: true,
    created_at: "2026-11-10T08:30:00Z",
    ...overrides
  };
}

function finding(overrides: Partial<ReviewFinding> = {}): ReviewFinding {
  return {
    id: "find-1",
    design_revision_id: "rev-2",
    finding_kind: "issue",
    severity: "blocker",
    body: "缺少回滚方案",
    status: "open",
    resolution_note: null,
    raised_by: "p-1",
    resolved_by: null,
    created_at: "2026-11-10T08:35:00Z",
    resolved_at: null,
    ...overrides
  };
}

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/design/technical-designs/design-1");
  await router.isReady();
  const { wrapper } = mountShell(
    TechnicalDesignDetailView,
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
  getDesign.mockReset();
  listDesignRequirements.mockReset();
  listDesignFindings.mockReset();
  getDesignWorkspace.mockReset();
  getDesignWorkspace.mockResolvedValue({
    design_count: 1,
    adr_count: 0,
    baseline_count: 0,
    pending_approval_count: 0,
    open_finding_count: 1,
    linked_requirement_count: 1,
    stale_requirement_count: 0,
    design_states: {},
    adr_states: {}
  });
  getDesign.mockResolvedValue(detail());
  listDesignRequirements.mockResolvedValue([requirementLink()]);
  listDesignFindings.mockResolvedValue([finding()]);
});

describe("TechnicalDesignDetailView", () => {
  it("默认选中当前正式版本，并按该版本读需求与评审问题", async () => {
    const { wrapper } = await mountView();

    expect(getDesign).toHaveBeenCalledWith("design-1");
    expect(listDesignRequirements).toHaveBeenCalledWith("design-1", "rev-2");
    expect(listDesignFindings).toHaveBeenCalledWith("design-1", "rev-2");
    expect(
      wrapper.find('[data-testid="design-revision-markdown"]').text()
    ).toContain("v2 正文");
    expect(
      wrapper.find('[data-testid="design-revision-summary"]').text()
    ).toContain("当前正式版本");
    expect(wrapper.text()).toContain("REQ-001");
    expect(wrapper.text()).toContain("缺少回滚方案");
  });

  it("切换版本后按新版本重读关联，不沿用上一版数据", async () => {
    const { wrapper } = await mountView();

    await wrapper.find('[data-testid="design-revision-3"]').trigger("click");
    await flushPromises();

    expect(listDesignRequirements).toHaveBeenCalledWith("design-1", "rev-3");
    expect(
      wrapper.find('[data-testid="design-revision-markdown"]').text()
    ).toContain("v3 正文");
  });
});
