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
const createDesignRevision = vi.fn();
const submitDesignRevision = vi.fn();
const requestDesignApproval = vi.fn();
const decideDesignApproval = vi.fn();
const linkDesignRequirement = vi.fn();
const listProjectRequirements = vi.fn();

vi.mock("@/api/designs", () => ({
  getDesign: (...args: unknown[]) => getDesign(...args),
  listDesignRequirements: (...args: unknown[]) =>
    listDesignRequirements(...args),
  listDesignFindings: (...args: unknown[]) => listDesignFindings(...args),
  getDesignWorkspace: (...args: unknown[]) => getDesignWorkspace(...args),
  createDesignRevision: (...args: unknown[]) => createDesignRevision(...args),
  submitDesignRevision: (...args: unknown[]) => submitDesignRevision(...args),
  requestDesignApproval: (...args: unknown[]) =>
    requestDesignApproval(...args),
  decideDesignApproval: (...args: unknown[]) => decideDesignApproval(...args),
  linkDesignRequirement: (...args: unknown[]) => linkDesignRequirement(...args)
}));

vi.mock("@/api/requirements", () => ({
  listProjectRequirements: (...args: unknown[]) =>
    listProjectRequirements(...args)
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
  createDesignRevision.mockResolvedValue({
    revision: { id: "rev-new", version: 4, state: "draft" }
  });
  submitDesignRevision.mockResolvedValue({ revision: { state: "pending_confirmation" } });
  requestDesignApproval.mockResolvedValue({ approval: { id: "ap-1" } });
  decideDesignApproval.mockResolvedValue({ approval: { id: "ap-1" } });
  linkDesignRequirement.mockResolvedValue({});
  listProjectRequirements.mockResolvedValue({
    items: [
      {
        id: "req-1",
        project_id: "proj-1",
        code: "REQ-001",
        title: "手机号登录",
        requirement_type: "functional",
        priority: "normal",
        lifecycle_status: "active",
        row_revision: 1,
        current_revision_id: "rrev-1",
        current_version: 1,
        current_content_hash: "h",
        latest_revision_id: "rrev-1",
        latest_version: 1,
        latest_state: "approved",
        latest_submitted_at: null,
        created_at: "2026-11-10T08:00:00Z",
        updated_at: "2026-11-10T08:00:00Z"
      }
    ]
  });
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

  // ── R4/B04-02：写路径 ──────────────────────────────────────────────

  it("draft 修订：新修订提交真实命令（每次编辑产生新版本）", async () => {
    const { wrapper } = await mountView();

    // fixture 默认选中 rev-2（approved）；先选中 draft rev-3。
    await wrapper.find('[data-testid="design-revision-3"]').trigger("click");
    await flushPromises();

    await wrapper.find('[data-testid="design-new-revision"]').trigger("click");
    await wrapper
      .find('[data-testid="design-revision-markdown-input"]')
      .setValue("# v4 正文");
    await wrapper.find('[data-testid="design-revision-save"]').trigger("click");
    await flushPromises();

    expect(createDesignRevision).toHaveBeenCalledWith("design-1", {
      markdown: "# v4 正文",
      summary: undefined
    });
  });

  it("draft 修订可提交（draft → pending_confirmation）", async () => {
    const { wrapper } = await mountView();

    await wrapper.find('[data-testid="design-revision-3"]').trigger("click");
    await flushPromises();
    await wrapper
      .find('[data-testid="design-submit-revision"]')
      .trigger("click");
    await flushPromises();

    expect(submitDesignRevision).toHaveBeenCalledWith("design-1", "rev-3");
  });

  it("已批准修订不给提交/请求审批入口（状态闸门，UI 不推断）", async () => {
    const { wrapper } = await mountView();

    // 默认选中 rev-2（approved）。
    expect(
      wrapper.find('[data-testid="design-submit-revision"]').exists()
    ).toBe(false);
    expect(
      wrapper.find('[data-testid="design-request-approval"]').exists()
    ).toBe(false);
  });

  it("pending 审批行出现人工批准/拒绝面板，拒绝必填理由，带 SC-06 双令牌", async () => {
    getDesign.mockResolvedValue({
      ...detail(),
      approvals: [
        {
          id: "ap-1",
          kind: "design_revision",
          design_id: "design-1",
          adr_id: null,
          baseline_id: null,
          target_id: "rev-3",
          target_hash: "hash",
          status: "pending",
          requested_by: "u1",
          decided_by: null,
          decision_reason: null,
          decided_at: null,
          created_at: "2026-11-10T09:00:00Z"
        }
      ]
    });
    const { wrapper } = await mountView();

    // 默认选中 approved rev-2 → 面板不出现（pending 审批挂在 rev-3）。
    expect(
      wrapper.find('[data-testid="design-approval-panel"]').exists()
    ).toBe(false);
    await wrapper.find('[data-testid="design-revision-3"]').trigger("click");
    await flushPromises();

    expect(
      wrapper.find('[data-testid="design-approval-panel"]').exists()
    ).toBe(true);
    await wrapper
      .find('[data-testid="design-approval-reason"] input')
      .setValue("通过");
    await wrapper.find('[data-testid="design-approve"]').trigger("click");
    await flushPromises();

    expect(decideDesignApproval).toHaveBeenCalledWith("ap-1", {
      decision: "approve",
      reason: "通过",
      expected_current_revision_id: "rev-2",
      expected_row_revision: 1
    });
  });

  it("draft 修订可关联需求（真实命令）", async () => {
    const { wrapper } = await mountView();

    await wrapper.find('[data-testid="design-revision-3"]').trigger("click");
    await flushPromises();
    await wrapper
      .find('[data-testid="design-open-link-form"]')
      .trigger("click");
    await flushPromises();
    await wrapper
      .find('[data-testid="design-link-requirement-select"]')
      .setValue("req-1");
    await wrapper.find('[data-testid="design-link-submit"]').trigger("click");
    await flushPromises();

    expect(linkDesignRequirement).toHaveBeenCalledWith(
      "design-1",
      "rev-3",
      { requirement_id: "req-1" }
    );
  });
});
