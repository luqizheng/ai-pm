// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { mountShell } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";
import { buildRoutes } from "@/router";

import AuthorizationView from "./AuthorizationView.vue";

/**
 * PA-06 屏 H（Authorization & autonomy）契约：
 *   - 已确认目标卡（goal vN）+ AI 可自主 / 须人类确认 两列基线清单；
 *   - pending 审批 → 待决授权请求卡 + 审查入口（跳决策中心）；
 *   - 无 pending → 待决卡不渲染（不伪装请求）；
 *   - 右栏审批历史=已决 approval_request 真实数据；无版本化授权实体→
 *     徽标用「红线策略生效」诚实文案。
 */
const mountedWrappers: ReturnType<typeof mountShell>[] = [];

const listApprovals = vi.fn();
const getProjectGoal = vi.fn();
const fetchConsultations = vi.fn();
const fetchPendingDecisions = vi.fn();

vi.mock("@/api/pm", () => ({
  listApprovals: (...args: unknown[]) => listApprovals(...args),
  decideApproval: vi.fn(),
  listTasks: vi.fn()
}));

vi.mock("@/api/goal", () => ({
  getProjectGoal: (...args: unknown[]) => getProjectGoal(...args)
}));

vi.mock("@/api/collaboration", () => ({
  fetchConsultations: (...args: unknown[]) => fetchConsultations(...args),
  fetchPendingDecisions: (...args: unknown[]) => fetchPendingDecisions(...args),
  fetchDecisionCenter: vi.fn(),
  fetchProjectRisks: vi.fn(),
  fetchWaitingDecisionCount: vi.fn().mockResolvedValue(0),
  decideConsultation: vi.fn(),
  cancelConsultation: vi.fn()
}));

async function mountAt() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/collaboration/authorization?project_id=p-1");
  await router.isReady();
  const wrapper = mountShell(AuthorizationView, {
    global: { plugins: [router] }
  });
  (mountedWrappers as unknown as VueWrapper[]).push(wrapper as VueWrapper);
  await flushPromises();
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
  getProjectGoal.mockResolvedValue({
    state: "ready",
    metadata: { revision_no: 3 },
    markdown: "Ship reliable incremental sync for Atlas 2.4 by 27 Nov.\n",
    bodyError: ""
  });
  listApprovals.mockResolvedValue({
    items: [
      {
        id: "a-pending",
        project_id: "p-1",
        proposal_id: "prop-1",
        status: "pending",
        required_role: "project_owner",
        decided_by: null,
        decision_reason: null,
        decided_at: null,
        created_at: "2026-11-21T10:28:00Z"
      },
      {
        id: "a-done",
        project_id: "p-1",
        proposal_id: "prop-0",
        status: "approved",
        required_role: "project_owner",
        decided_by: "p-maya",
        decision_reason: "Staging test access added.",
        decided_at: "2026-11-20T09:00:00Z",
        created_at: "2026-11-20T08:00:00Z"
      }
    ],
    total: 2
  });
  fetchConsultations.mockResolvedValue([]);
  fetchPendingDecisions.mockResolvedValue([]);
});

describe("AuthorizationView", () => {
  it("renders the confirmed objective with revision and both boundary lists", async () => {
    const wrapper = await mountAt();
    expect(wrapper.find('[data-testid="au-objective"]').text()).toContain(
      "Confirmed objective · v3"
    );
    expect(wrapper.find('[data-testid="au-objective"]').text()).toContain(
      "Ship reliable incremental sync for Atlas 2.4 by 27 Nov."
    );
    expect(wrapper.find('[data-testid="au-autonomous"]').text()).toContain(
      "AI may act autonomously"
    );
    expect(wrapper.find('[data-testid="au-confirm"]').text()).toContain(
      "Human confirmation required"
    );
    // 各列 6 项
    expect(wrapper.findAll('[data-testid="au-autonomous"] li')).toHaveLength(6);
    expect(wrapper.findAll('[data-testid="au-confirm"] li')).toHaveLength(6);
  });

  it("renders the pending authorization request card when approvals are pending", async () => {
    const wrapper = await mountAt();
    const pending = wrapper.find('[data-testid="au-pending"]');
    expect(pending.exists()).toBe(true);
    expect(pending.text()).toContain("1 pending authorization requests");
    expect(pending.text()).toContain("Review bounded request");
  });

  it("hides the pending card when nothing is pending", async () => {
    listApprovals.mockResolvedValue({
      items: [
        {
          id: "a-done",
          project_id: "p-1",
          proposal_id: "prop-0",
          status: "approved",
          required_role: "project_owner",
          decided_by: "p-maya",
          decision_reason: "ok",
          decided_at: "2026-11-20T09:00:00Z",
          created_at: "2026-11-20T08:00:00Z"
        }
      ],
      total: 1
    });
    const wrapper = await mountAt();
    expect(wrapper.find('[data-testid="au-pending"]').exists()).toBe(false);
  });

  it("renders real approval history in the context panel (no fabricated version timeline)", async () => {
    const wrapper = await mountAt();
    const root = wrapper.element;
    expect(root.textContent).toContain("Human-owned authority");
    expect(root.textContent).toContain("Red-line policy active");
    expect(root.textContent).toContain("Staging test access added.");
  });

  it("shows operational constraints including the acceptance-baseline rule", async () => {
    const wrapper = await mountAt();
    expect(wrapper.find('[data-testid="au-constraints"]').text()).toContain(
      "cannot be lowered by AI-PM or Agents"
    );
  });
});
