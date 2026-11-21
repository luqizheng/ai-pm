// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { mountShell } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";
import { buildRoutes } from "@/router";

import KnowledgeReviewView from "./KnowledgeReviewView.vue";

/**
 * PA-06 屏 D（Knowledge change review）契约：
 *   - 待决提案队列 + 已确认基线 vs AI 提案双列对照（提案列=change_summary，
 *     不伪造全文 diff）；
 *   - Confirm 基线走 confirmProposal（当前用户 principal + 幂等 operation_id）；
 *   - resulting_revision_id 存在才渲染 Veto（走 vetoRevision）；
 *   - 空态=「AI 自主维护草稿；仅改正式含义需确认」。
 */
const mountedWrappers: ReturnType<typeof mountShell>[] = [];

const listProposals = vi.fn();
const listDocuments = vi.fn();
const getDocument = vi.fn();
const confirmProposal = vi.fn();
const vetoRevision = vi.fn();
const fetchTeamContext = vi.fn();
const fetchConsultations = vi.fn();
const fetchPendingDecisions = vi.fn();

vi.mock("@/api/documents", () => ({
  listProposals: (...args: unknown[]) => listProposals(...args),
  listDocuments: (...args: unknown[]) => listDocuments(...args),
  getDocument: (...args: unknown[]) => getDocument(...args),
  confirmProposal: (...args: unknown[]) => confirmProposal(...args),
  vetoRevision: (...args: unknown[]) => vetoRevision(...args)
}));

vi.mock("@/api/team", () => ({
  fetchTeamContext: (...args: unknown[]) => fetchTeamContext(...args)
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

vi.mock("@/components/ui/toast", () => ({
  useToast: () => ({ toast: vi.fn() })
}));

function proposalRow(overrides: Record<string, unknown> = {}) {
  return {
    id: "prop-1",
    doc_key: "req-18-audit-retention",
    doc_type: "requirement",
    status: "redline_pending",
    quality_gate_result: { passed: true, failures: [] },
    redline_kind: "formal_meaning",
    change_summary: "Retain audit events for 90 days instead of 30.",
    resulting_revision_id: "rev-9ab1",
    created_at: "2026-11-21T10:00:00Z",
    ...overrides
  };
}

async function mountAt() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/collaboration/knowledge?project_id=p-1");
  await router.isReady();
  const wrapper = mountShell(KnowledgeReviewView, {
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
  listProposals.mockResolvedValue({
    items: [
      proposalRow({
        status: "pending",
        resulting_revision_id: null,
        change_summary: "Retain audit events for 90 days."
      })
    ]
  });
  listDocuments.mockResolvedValue({
    items: [
      {
        id: "doc-18",
        doc_key: "req-18-audit-retention",
        doc_type: "requirement",
        canonical_path: "docs/req-18.md",
        revision_no: 3,
        updated_at: "2026-11-20T09:00:00Z"
      }
    ]
  });
  getDocument.mockResolvedValue({
    document_id: "doc-18",
    doc_key: "req-18-audit-retention",
    markdown: "# REQ-18\n\nRetain audit events for 30 days.\n",
    revision_no: 3,
    git_commit_sha: "abc",
    content_hash: "h",
    canonical_path: "docs/req-18.md",
    doc_type: "requirement"
  });
  fetchTeamContext.mockResolvedValue({ principal_id: "p-maya" });
  fetchConsultations.mockResolvedValue([]);
  fetchPendingDecisions.mockResolvedValue([]);
});

describe("KnowledgeReviewView", () => {
  it("renders baseline vs proposal columns with honest excerpts", async () => {
    const wrapper = await mountAt();
    const baseline = wrapper.find('[data-testid="kb-baseline"]');
    const proposal = wrapper.find('[data-testid="kb-proposal"]');
    expect(baseline.exists()).toBe(true);
    expect(baseline.text()).toContain("Retain audit events for 30 days.");
    expect(baseline.text()).toContain("Confirmed baseline · v3");
    expect(proposal.text()).toContain("AI proposal · Not approved");
    expect(proposal.text()).toContain("Retain audit events for 90 days.");
    // 提案无 resulting_revision_id → 不渲染 Veto（无目标修订不可否决）
    expect(wrapper.find('[data-testid="kb-veto"]').exists()).toBe(false);
  });

  it("confirms the proposal through the redline confirm endpoint", async () => {
    listProposals.mockResolvedValue({
      items: [
        proposalRow({
          status: "pending",
          resulting_revision_id: "rev-9ab1",
          change_summary: "Retain audit events for 90 days."
        })
      ]
    });
    confirmProposal.mockResolvedValue({});
    const wrapper = await mountAt();
    await wrapper.find('[data-testid="kb-confirm"]').trigger("click");
    await flushPromises();
    await flushPromises();
    expect(confirmProposal).toHaveBeenCalledWith(
      "prop-1",
      "p-1",
      "p-maya",
      expect.any(String)
    );
  });

  it("shows the empty state when no proposals are pending", async () => {
    listProposals.mockResolvedValue({ items: [] });
    const wrapper = await mountAt();
    expect(wrapper.find('[data-testid="screen-feedback-empty"]').exists()).toBe(
      true
    );
    expect(wrapper.text()).toContain(
      "only proposals that change formal meaning need human confirmation"
    );
  });
});
