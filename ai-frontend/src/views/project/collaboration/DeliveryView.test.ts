// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { mountShell } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";
import { buildRoutes } from "@/router";

import DeliveryView from "./DeliveryView.vue";

/**
 * PA-06 屏 F（Verification & delivery）契约：
 *   - 验收口径×裁决×证据表=criterion-verdicts 真实裁决（evaluated=false
 *     呈现「未被覆盖」不造假）；有未过口径 → 交付阻塞红卡（passed/total）；
 *   - 「只看未通过」就地过滤；产物段=delivery 真实清单（未齐=prepared only）；
 *   - 无验证中任务 → 诚实空态。
 */
const mountedWrappers: ReturnType<typeof mountShell>[] = [];

const listTasks = vi.fn();
const listCriterionVerdicts = vi.fn();
const listReviewFindings = vi.fn();
const listTaskDelivery = vi.fn();
const listTaskReviewApprovals = vi.fn();
const fetchConsultations = vi.fn();
const fetchPendingDecisions = vi.fn();

vi.mock("@/api/pm", () => ({
  listTasks: (...args: unknown[]) => listTasks(...args),
  decideApproval: vi.fn()
}));

vi.mock("@/api/review", () => ({
  listCriterionVerdicts: (...args: unknown[]) => listCriterionVerdicts(...args),
  listReviewFindings: (...args: unknown[]) => listReviewFindings(...args),
  listTaskReviewApprovals: (...args: unknown[]) =>
    listTaskReviewApprovals(...args)
}));

vi.mock("@/api/acceptance", () => ({
  listTaskDelivery: (...args: unknown[]) => listTaskDelivery(...args)
}));

vi.mock("@/api/collaboration", () => ({
  fetchConsultations: (...args: unknown[]) => fetchConsultations(...args),
  fetchPendingDecisions: (...args: unknown[]) => fetchPendingDecisions(...args),
  fetchDecisionCenter: vi.fn(),
  fetchProjectRisks: vi.fn(),
  fetchWaitingDecisionCount: vi.fn().mockResolvedValue(0),
  fetchWaitingQueue: vi.fn().mockResolvedValue([]),
  decideConsultation: vi.fn(),
  cancelConsultation: vi.fn()
}));

function verdict(overrides: Record<string, unknown> = {}) {
  return {
    criterion_id: "ac-1",
    ordinal: 1,
    text: "Replay produces no duplicates",
    verification_kind: "manual",
    evaluated: true,
    review_run_id: "run-1",
    review_attempt_no: 1,
    verdict: "passed",
    evidence: { ref: "replay-checks.html" },
    decided_by: null,
    decided_at: "2026-11-21T11:16:00Z",
    verdict_created_at: null,
    ...overrides
  };
}

async function mountAt() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/collaboration/delivery?project_id=p-1");
  await router.isReady();
  const wrapper = mountShell(DeliveryView, {
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
  listTasks.mockResolvedValue({
    items: [
      {
        id: "task-1",
        code: "PM-101",
        title: "Sync engine",
        status: "verifying",
        assignee_id: null,
        deps_total: 0,
        deps_met: 0
      }
    ],
    total: 1
  });
  listCriterionVerdicts.mockResolvedValue({
    items: [
      verdict(),
      verdict({
        criterion_id: "ac-2",
        ordinal: 2,
        text: "Timeout recovery preserves cursor",
        verdict: "failed",
        evidence: { ref: "recovery-test.html" }
      }),
      verdict({
        criterion_id: "ac-3",
        ordinal: 3,
        text: "Manual sign-off",
        evaluated: false,
        verdict: null,
        evidence: null
      })
    ]
  });
  listReviewFindings.mockResolvedValue({
    items: [
      {
        severity: "blocker",
        message: "ISSUE-31: timeout retry can advance the cursor",
        evidence_ref: null,
        code_location: null,
        created_at: "2026-11-21T10:00:00Z"
      }
    ]
  });
  listTaskDelivery.mockResolvedValue({
    task_id: "task-1",
    project_id: "p-1",
    items: [
      {
        branch: "feat/sync",
        delivery_state: "partial",
        commit_author: null,
        commit_authored_at: null,
        expected_delivery_count: 1,
        actual_delivery_count: 1,
        repository_id: "repo-1"
      }
    ],
    total: 1
  });
  listTaskReviewApprovals.mockResolvedValue({ items: [] });
  fetchConsultations.mockResolvedValue([]);
  fetchPendingDecisions.mockResolvedValue([]);
});

describe("DeliveryView", () => {
  it("P1 Delivery Review 整合：提供审核台与 S05 执行审查的交叉导航", async () => {
    const wrapper = await mountAt();
    const link = wrapper.find('[data-testid="dl-review-center-link"]');
    expect(link.exists()).toBe(true);
    expect(link.attributes("href")).toContain("/project/review-center");
    expect(link.attributes("href")).toContain("kind=delivery");
    const execLink = wrapper.find('[data-testid="dl-execution-review-link"]');
    expect(execLink.exists()).toBe(true);
    expect(execLink.attributes("href")).toContain("/project/execution/review");
  });

  it("renders the criteria verdict table with honest not-covered rows", async () => {
    const wrapper = await mountAt();
    expect(wrapper.find('[data-testid="dl-verdict-1"]').text()).toContain(
      "Replay produces no duplicates"
    );
    const failed = wrapper.find('[data-testid="dl-verdict-2"]');
    expect(failed.text()).toContain("Failed");
    const uncovered = wrapper.find('[data-testid="dl-verdict-3"]');
    expect(uncovered.text()).toContain("Not covered by any review run");
  });

  it("shows the delivery-blocked card with pass ratio and blocker message", async () => {
    const wrapper = await mountAt();
    const blocked = wrapper.find('[data-testid="dl-blocked"]');
    expect(blocked.exists()).toBe(true);
    expect(blocked.text()).toContain("Delivery is blocked · 1/3 passing");
    expect(blocked.text()).toContain("ISSUE-31");
    expect(wrapper.text()).toContain(
      "Prepared only · no delivery recorded while acceptance is incomplete"
    );
  });

  it("filters to failed criteria with the inspect toggle", async () => {
    const wrapper = await mountAt();
    await wrapper.find('[data-testid="dl-failed-filter"]').trigger("click");
    await flushPromises();
    expect(wrapper.find('[data-testid="dl-verdict-1"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="dl-verdict-2"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="dl-verdict-3"]').exists()).toBe(false);
  });

  it("shows the honest empty state when nothing is under verification", async () => {
    listTasks.mockResolvedValue({ items: [], total: 0 });
    const wrapper = await mountAt();
    expect(wrapper.find('[data-testid="screen-feedback-empty"]').exists()).toBe(
      true
    );
  });
});
