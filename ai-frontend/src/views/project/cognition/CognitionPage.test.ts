// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";

import CognitionPage from "./CognitionPage.vue";
import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";
import type { CognitionOverview } from "@/api/cognition";

const loadCognitionOverview = vi.fn();
const startCognitionRun = vi.fn();
const decideCognitionItem = vi.fn();
const listProjectKnowledge = vi.fn();

vi.mock("@/api/cognition", () => ({
  loadCognitionOverview: (...args: unknown[]) => loadCognitionOverview(...args),
  startCognitionRun: (...args: unknown[]) => startCognitionRun(...args),
  decideCognitionItem: (...args: unknown[]) => decideCognitionItem(...args),
  listProjectKnowledge: (...args: unknown[]) => listProjectKnowledge(...args)
}));

const t = (key: string) => i18n.global.t(key) as string;

function overviewFixture(): CognitionOverview {
  return {
    runs: [
      {
        id: "run-1",
        scope: {},
        item_count: 2,
        confirmed_count: 1,
        status: "completed",
        created_at: "2026-11-05T08:00:00Z"
      }
    ],
    latest_run_id: "run-1",
    items: [
      {
        id: "item-1",
        domain: "identity",
        content: "Project 'Atlas Platform' — R&D delivery workbench",
        evidence: { type: "project_record" },
        confidence: "high",
        has_conflict: false,
        state: "proposed",
        decided_by: null,
        decided_at: null,
        decision_note: null
      },
      {
        id: "item-2",
        domain: "repository",
        content: "Repository 'atlas-platform' (main): https://github.com/acme-labs/atlas-platform",
        evidence: { type: "repository_metadata" },
        confidence: "high",
        has_conflict: false,
        state: "proposed",
        decided_by: null,
        decided_at: null,
        decision_note: null
      }
    ],
    knowledge_active: 1,
    proposed: 2
  };
}

async function mountPage() {
  const mounted = mountShell(CognitionPage, {
    global: {
      mocks: { $route: { query: { project_id: "prj-shield" } } }
    }
  });
  // 路由 query 通过 useRoute 提供——mountShell 的 memory router 无 query，
  // 直接替换当前路由（push 携带 query）更贴近真实。
  await mounted.router.push("/project/cognition?project_id=prj-shield");
  await flushPromises();
  return mounted;
}

describe("CognitionPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    loadCognitionOverview.mockResolvedValue(overviewFixture());
    listProjectKnowledge.mockResolvedValue({
      items: [
        {
          id: "kn-1",
          domain: "identity",
          content: "Confirmed identity knowledge",
          version: 1,
          source_item_id: "item-1",
          confirmed_by: "prc-1",
          confirmed_at: "2026-11-05T08:30:00Z"
        }
      ]
    });
  });

  it("呈现概览统计、Run 历史与待确认条目", async () => {
    const { wrapper } = await mountPage();

    expect(wrapper.find('[data-testid="cognition-stats"]').exists()).toBe(true);
    expect(wrapper.findAll('[data-testid="cognition-run"]')).toHaveLength(1);
    expect(wrapper.findAll('[data-testid="cognition-item"]')).toHaveLength(2);
    expect(wrapper.text()).toContain("Atlas Platform");
    // 确认人视角的正式知识区块在场
    expect(wrapper.findAll('[data-testid="cognition-knowledge"]')).toHaveLength(1);
  });

  it("确认单条后重载概览（逐项确认，不整批）", async () => {
    decideCognitionItem.mockResolvedValue({
      id: "item-1",
      state: "confirmed",
      knowledge_id: "kn-9"
    });
    const { wrapper } = await mountPage();

    const button = wrapper.find('[data-testid="cognition-confirm-item-1"]');
    expect(button.exists()).toBe(true);
    await button.trigger("click");
    await flushPromises();

    expect(decideCognitionItem).toHaveBeenCalledWith("prj-shield", "item-1", {
      action: "confirm"
    });
    expect(loadCognitionOverview).toHaveBeenCalledTimes(2);
  });

  it("启动分析调用 startCognitionRun 并重载", async () => {
    startCognitionRun.mockResolvedValue({
      run_id: "run-2",
      status: "completed",
      item_count: 2
    });
    const { wrapper } = await mountPage();

    await wrapper.find('[data-testid="cognition-start"]').trigger("click");
    await flushPromises();

    expect(startCognitionRun).toHaveBeenCalledWith("prj-shield");
    expect(loadCognitionOverview).toHaveBeenCalledTimes(2);
  });

  it("加载失败显示错误态而非空态", async () => {
    loadCognitionOverview.mockRejectedValue(new Error("boom"));
    const { wrapper } = await mountPage();

    expect(wrapper.text()).toContain(t("s01.cognition.state.loadFailed.title"));
    expect(wrapper.text()).toContain("boom");
  });
});
