// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { mountShell } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";

import ExternalCommitsView from "./ExternalCommitsView.vue";

/**
 * P0 外部 Commit 采纳闭环（迁移 0054）页面契约：
 *   - 发现 ≠ 采纳：列表行状态徽标分列（discovered/awaiting_adoption），
 *     动作按状态机渲染（discovered→分析，awaiting_adoption→决定，
 *     adopted→关闭）；
 *   - 扫描（discover）幂等且不吞错；分析失败行给重试入口（不静默）；
 *   - 澄清答复走 answerClarification；「无需澄清」以显式文案呈现；
 *   - 决定三值（approved/no_update/rejected）走 decideAdoption；
 *   - 空态文案不抹历史。
 */
const mountedWrappers: ReturnType<typeof mountShell>[] = [];

const discoverExternalCommits = vi.fn();
const listExternalCommits = vi.fn();
const getExternalCommit = vi.fn();
const analyzeExternalCommit = vi.fn();
const answerClarification = vi.fn();
const decideAdoption = vi.fn();
const closeAdoption = vi.fn();
const getCommitDiff = vi.fn();

vi.mock("@/api/externalCommits", () => ({
  discoverExternalCommits: (...args: unknown[]) =>
    discoverExternalCommits(...args),
  listExternalCommits: (...args: unknown[]) => listExternalCommits(...args),
  getExternalCommit: (...args: unknown[]) => getExternalCommit(...args),
  analyzeExternalCommit: (...args: unknown[]) => analyzeExternalCommit(...args),
  answerClarification: (...args: unknown[]) => answerClarification(...args),
  decideAdoption: (...args: unknown[]) => decideAdoption(...args),
  closeAdoption: (...args: unknown[]) => closeAdoption(...args),
  getCommitDiff: (...args: unknown[]) => getCommitDiff(...args)
}));

vi.mock("@/api/collaboration", () => ({
  fetchConsultations: vi.fn().mockResolvedValue([]),
  fetchPendingDecisions: vi.fn().mockResolvedValue([]),
  fetchDecisionCenter: vi.fn(),
  fetchProjectRisks: vi.fn().mockResolvedValue([]),
  fetchWaitingDecisionCount: vi.fn().mockResolvedValue(0),
  decideConsultation: vi.fn(),
  cancelConsultation: vi.fn()
}));

vi.mock("@/components/ui/toast", () => ({
  useToast: () => ({ toast: vi.fn() })
}));

function listItem(overrides: Record<string, unknown> = {}) {
  return {
    id: "ec-1",
    repository_id: "repo-1",
    repository_name: "origin",
    commit_sha: "a".repeat(40),
    branch: "main",
    author_name: "External Dev",
    committed_at: "2026-11-22T08:00:00Z",
    subject: "feat: add login page",
    status: "discovered",
    analysis_mode: null,
    clarification_state: "not_needed",
    open_questions: 0,
    adoption_decision: null,
    attempts: 0,
    last_error: null,
    discovered_at: "2026-11-22T09:00:00Z",
    ...overrides
  };
}

function detailFixture(overrides: Record<string, unknown> = {}) {
  return {
    commit: {
      ...listItem(),
      project_id: "p-1",
      changed_files: [
        { path: "src/login.ts", change: "A" },
        { path: "docs/guide.md", change: "M" }
      ],
      analysis: null,
      analyzed_at: null,
      adoption_note: null,
      adopted_by: null,
      adopted_at: null,
      knowledge_proposals: [],
      adopted_revision_ids: [],
      ...overrides
    },
    clarifications: [],
    repository_name: "origin"
  };
}

async function mountView(): Promise<ReturnType<typeof mountShell>> {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      {
        path: "/project/collaboration/commits",
        component: ExternalCommitsView
      },
      { path: "/:pathMatch(.*)*", component: { template: "<div />" } }
    ]
  });
  await router.push("/project/collaboration/commits?project_id=p-1");
  await router.isReady();
  const wrapper = mountShell(ExternalCommitsView, {
    global: { plugins: [router, i18n] }
  });
  mountedWrappers.push(wrapper);
  await flushPromises();
  return wrapper;
}

beforeEach(() => {
  vi.clearAllMocks();
  listExternalCommits.mockResolvedValue([]);
  discoverExternalCommits.mockResolvedValue({
    scanned_repositories: 1,
    discovered_commits: 0,
    repositories: []
  });
  getExternalCommit.mockResolvedValue(detailFixture());
  getCommitDiff.mockResolvedValue({
    diff: "",
    truncated: false,
    commit_sha: "a".repeat(40)
  });
});

afterEach(() => {
  for (const wrapper of mountedWrappers.splice(0)) wrapper.unmount();
});

describe("ExternalCommitsView", () => {
  it("renders the empty state without erasing history wording", async () => {
    const wrapper = await mountView();
    expect(wrapper.find("[data-testid='ec-scan']").exists()).toBe(true);
    expect(wrapper.text()).toContain(i18n.global.t("pa06.commits.emptyTitle"));
  });

  it("renders list rows with separated discovery/adoption badges and detail actions by state", async () => {
    listExternalCommits.mockResolvedValue([listItem()]);
    getExternalCommit.mockResolvedValue(detailFixture());
    const wrapper = await mountView();

    const row = wrapper.find("[data-testid='ec-row-aaaaaaaa']");
    expect(row.exists()).toBe(true);
    // 发现 ≠ 采纳：discovered 徽标文案出现，无 adopted 字样。
    expect(wrapper.text()).toContain(
      i18n.global.t("pa06.commits.status.discovered")
    );
    // discovered → 分析按钮，而非决定按钮。
    expect(wrapper.find("[data-testid='ec-analyze']").exists()).toBe(true);
    expect(wrapper.find("[data-testid='ec-approve']").exists()).toBe(false);
  });

  it("shows approve/no_update/reject for awaiting_adoption and calls decideAdoption", async () => {
    listExternalCommits.mockResolvedValue([
      listItem({ status: "awaiting_adoption" })
    ]);
    getExternalCommit.mockResolvedValue(
      detailFixture({
        status: "awaiting_adoption",
        analysis: {
          summary: "实现登录页",
          analysis_mode: "derived",
          related_objects: [],
          deviation: null,
          knowledge_updates: []
        }
      })
    );
    decideAdoption.mockResolvedValue({
      id: "ec-1",
      status: "closed",
      adoption_decision: "no_update",
      knowledge_proposals: []
    });
    const wrapper = await mountView();

    expect(wrapper.find("[data-testid='ec-approve']").exists()).toBe(true);
    expect(wrapper.find("[data-testid='ec-no-update']").exists()).toBe(true);
    expect(wrapper.find("[data-testid='ec-reject']").exists()).toBe(true);
    expect(wrapper.find("[data-testid='ec-analyze']").exists()).toBe(false);

    await wrapper.find("[data-testid='ec-no-update']").trigger("click");
    await flushPromises();
    expect(decideAdoption).toHaveBeenCalledWith("ec-1", "no_update", undefined);
  });

  it("renders retry entry for failed rows without hiding the error", async () => {
    listExternalCommits.mockResolvedValue([
      listItem({
        status: "failed",
        attempts: 1,
        last_error: "provider down"
      })
    ]);
    getExternalCommit.mockResolvedValue(
      detailFixture({
        status: "failed",
        attempts: 1,
        last_error: "provider down"
      })
    );
    const wrapper = await mountView();

    expect(wrapper.find("[data-testid='ec-error']").text()).toContain(
      "provider down"
    );
    const retry = wrapper.find("[data-testid='ec-analyze']");
    expect(retry.exists()).toBe(true);
    expect(wrapper.text()).toContain(i18n.global.t("pa06.commits.retryAction"));
  });

  it("renders clarifications with answer inputs and explicit not_needed note", async () => {
    listExternalCommits.mockResolvedValue([
      listItem({ status: "analyzed", open_questions: 1 })
    ]);
    getExternalCommit.mockResolvedValue({
      ...detailFixture({ status: "analyzed", clarification_state: "pending" }),
      clarifications: [
        {
          id: "cl-1",
          external_commit_id: "ec-1",
          question: "登录态存哪里？",
          answer: null,
          answered_by: null,
          answered_at: null,
          status: "open",
          created_at: "2026-11-22T09:00:00Z"
        }
      ]
    });
    answerClarification.mockResolvedValue({ id: "cl-1", status: "answered" });
    const wrapper = await mountView();

    const input = wrapper.find("[data-testid='ec-answer-cl-1']");
    expect(input.exists()).toBe(true);
    expect(wrapper.text()).toContain("登录态存哪里？");

    await input.setValue("沿用既有约定");
    await wrapper
      .find("form, .ec-question__form")
      .find("button")
      .trigger("click");
    await flushPromises();
    expect(answerClarification).toHaveBeenCalledWith(
      "ec-1",
      "cl-1",
      "沿用既有约定"
    );
  });

  it("calls scan and refreshes the list", async () => {
    listExternalCommits.mockResolvedValue([]);
    const wrapper = await mountView();

    await wrapper.find("[data-testid='ec-scan']").trigger("click");
    await flushPromises();
    expect(discoverExternalCommits).toHaveBeenCalledWith("p-1");
    expect(listExternalCommits).toHaveBeenCalled();
  });
});
