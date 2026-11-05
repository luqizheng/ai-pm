// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";

import BaselinePage from "./BaselinePage.vue";
import { mountShell } from "@/components/shell/test-utils";
import type { BaselineOverview } from "@/api/baseline";

const loadBaselineOverview = vi.fn();
const createBaselineProposal = vi.fn();
const decideBaselineItem = vi.fn();
const confirmBaseline = vi.fn();

vi.mock("@/api/baseline", () => ({
  loadBaselineOverview: (...args: unknown[]) => loadBaselineOverview(...args),
  createBaselineProposal: (...args: unknown[]) =>
    createBaselineProposal(...args),
  decideBaselineItem: (...args: unknown[]) => decideBaselineItem(...args),
  confirmBaseline: (...args: unknown[]) => confirmBaseline(...args)
}));

function fixture(): BaselineOverview {
  return {
    current_version: null,
    current_confirmed_at: null,
    history: [],
    proposal: {
      id: "bp-1",
      version: 1,
      status: "pending",
      created_at: "2026-11-05T09:00:00Z"
    },
    items: [
      {
        id: "bi-1",
        category: "identity",
        content: "Confirmed identity from cognition",
        evidence: { type: "knowledge" },
        confidence: "high",
        state: "proposed",
        decided_by: null,
        decided_at: null,
        decision_note: null
      }
    ]
  };
}

async function mountPage() {
  const mounted = mountShell(BaselinePage);
  await mounted.router.push("/project/baseline?project_id=prj-shield");
  await flushPromises();
  return mounted;
}

describe("BaselinePage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    loadBaselineOverview.mockResolvedValue(fixture());
  });

  it("呈现待确认提案条目；有未决条目时确认基线禁用", async () => {
    const { wrapper } = await mountPage();

    expect(wrapper.find('[data-testid="baseline-proposal"]').exists()).toBe(
      true
    );
    expect(wrapper.findAll('[data-testid="baseline-item"]')).toHaveLength(1);
    const confirm = wrapper.find('[data-testid="baseline-confirm"]');
    expect(confirm.exists()).toBe(true);
    expect((confirm.element as HTMLButtonElement).disabled).toBe(true);
  });

  it("确认单条后重载；全部决策完则确认基线可用", async () => {
    decideBaselineItem.mockResolvedValue({ id: "bi-1", state: "confirmed" });
    const { wrapper } = await mountPage();

    await wrapper
      .find('[data-testid="baseline-confirm-bi-1"]')
      .trigger("click");
    await flushPromises();

    expect(decideBaselineItem).toHaveBeenCalledWith("prj-shield", "bi-1", {
      action: "confirm"
    });
    expect(loadBaselineOverview).toHaveBeenCalledTimes(2);
  });

  it("全部决策完成后确认基线并重载版本状态", async () => {
    const fixture2 = fixture();
    fixture2.items[0].state = "confirmed";
    loadBaselineOverview.mockResolvedValue(fixture2);
    confirmBaseline.mockResolvedValue({ version: 1, status: "confirmed" });
    const { wrapper } = await mountPage();

    const confirm = wrapper.find('[data-testid="baseline-confirm"]');
    expect((confirm.element as HTMLButtonElement).disabled).toBe(false);
    await confirm.trigger("click");
    await flushPromises();

    expect(confirmBaseline).toHaveBeenCalledWith("prj-shield");
    expect(loadBaselineOverview).toHaveBeenCalledTimes(2);
  });

  it("无提案时提供生成提案入口", async () => {
    const empty = fixture();
    empty.proposal = null;
    empty.items = [];
    loadBaselineOverview.mockResolvedValue(empty);
    createBaselineProposal.mockResolvedValue({
      proposal_id: "bp-2",
      version: 1,
      item_count: 3
    });
    const { wrapper } = await mountPage();

    const propose = wrapper.find('[data-testid="baseline-propose"]');
    expect(propose.exists()).toBe(true);
    await propose.trigger("click");
    await flushPromises();

    expect(createBaselineProposal).toHaveBeenCalledWith("prj-shield");
  });
});
