// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import SettingsProjectAiOverviewView from "./SettingsProjectAiOverviewView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import type { AiTeamMonitor, AiTeamOverview } from "@/api/aiTeam";

const getProjectAiTeamOverview = vi.fn();
const getProjectAiTeamMonitor = vi.fn();

vi.mock("@/api/aiTeam", () => ({
  getProjectAiTeamOverview: (...args: unknown[]) =>
    getProjectAiTeamOverview(...args),
  getProjectAiTeamMonitor: (...args: unknown[]) =>
    getProjectAiTeamMonitor(...args)
}));

/** 后端真实形状（AiTeamOverview 契约）的最小夹具 */
function overviewFixture(
  overrides: Partial<AiTeamOverview> = {}
): AiTeamOverview {
  const base: AiTeamOverview = {
    roles: { enabled: 2, disabled: 1, templates: 5 },
    runs: { active: 1, succeeded_24h: 3, failed_24h: 1 },
    tasks: { awaiting_decision: 2, blocked: 1 },
    provider: { bound: true, provider_name: "DeepSeek Main" }
  };
  return {
    ...base,
    ...overrides,
    roles: { ...base.roles, ...(overrides.roles ?? {}) },
    runs: { ...base.runs, ...(overrides.runs ?? {}) },
    tasks: { ...base.tasks, ...(overrides.tasks ?? {}) },
    provider: { ...base.provider, ...(overrides.provider ?? {}) }
  };
}

function monitorFixture(runs: AiTeamMonitor["runs"]): AiTeamMonitor {
  return { runs, events: [] };
}

function run(overrides: Partial<AiTeamMonitor["runs"][number]> = {}) {
  return {
    run_id: "run-1",
    task_id: "task-1",
    task_title: "Implement login",
    status: "running",
    stage: "started",
    run_kind: "delivery",
    attempt_no: 2,
    executor_kind: "codex",
    execution_mode: "spawn",
    release_reason: null,
    scheduling_shield: "none",
    claimed_at: "2026-11-08T09:00:00Z",
    outcome: null,
    terminated_at: null,
    finished_at: null,
    created_at: "2026-11-08T08:55:00Z",
    ...overrides
  };
}

async function mountView(
  path = "/project/settings/ai/overview?project_id=prj-1"
) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  router.push(path);
  await router.isReady();
  const mounted = mountShell(SettingsProjectAiOverviewView, {}, router);
  await flushPromises();
  return mounted;
}

describe("SettingsProjectAiOverviewView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getProjectAiTeamOverview.mockResolvedValue(overviewFixture());
    getProjectAiTeamMonitor.mockResolvedValue(monitorFixture([run()]));
  });

  it("无项目上下文时不发请求（scope 哨兵不得当作项目 id）", async () => {
    const { wrapper } = await mountView("/project/settings/ai/overview");
    expect(getProjectAiTeamOverview).not.toHaveBeenCalled();
    expect(getProjectAiTeamMonitor).not.toHaveBeenCalled();
    expect(
      wrapper.find('[data-testid="ai-overview-no-project"]').exists()
    ).toBe(true);
  });

  it("统计卡渲染四组计数（roles / 活跃 Run / 等待或阻塞 / Provider）", async () => {
    const { wrapper } = await mountView();
    const stats = wrapper.find('[data-testid="ai-overview-stats"]');
    expect(stats.exists()).toBe(true);
    const text = stats.text();
    expect(
      wrapper.find('[data-testid="ai-overview-stat-roles"]').text()
    ).toContain("2");
    expect(text).toContain("2 / 5");
    expect(
      wrapper.find('[data-testid="ai-overview-stat-runs"]').text()
    ).toContain("3");
    expect(text).toContain("近 24h 成功 3 / 失败 1");
    expect(
      wrapper.find('[data-testid="ai-overview-stat-tasks"]').text()
    ).toContain("3");
    expect(text).toContain("等待决策 2 / 阻塞 1");
    const provider = wrapper.find('[data-testid="ai-overview-stat-provider"]');
    expect(provider.text()).toContain("已绑定");
    expect(provider.text()).toContain("DeepSeek Main");
  });

  it("最近执行列表取 monitor.runs 前 N 条并渲染状态徽标", async () => {
    getProjectAiTeamMonitor.mockResolvedValue(
      monitorFixture([
        run({ run_id: "run-a", status: "failed", outcome: "failed" }),
        run({ run_id: "run-b", status: "queued" })
      ])
    );
    const { wrapper } = await mountView();
    expect(getProjectAiTeamMonitor).toHaveBeenCalledWith("prj-1");
    const list = wrapper.find('[data-testid="ai-overview-recent-runs"]');
    expect(list.text()).toContain("Implement login");
    expect(list.text()).toContain("failed");
    expect(list.text()).toContain("queued");
    expect(wrapper.find('[data-testid="ai-overview-run-run-a"]').exists()).toBe(
      true
    );
  });

  it("空项目：全部计数为零 + 无 Run 空态（零值结构可见）", async () => {
    getProjectAiTeamOverview.mockResolvedValue(
      overviewFixture({
        roles: { enabled: 0, disabled: 0, templates: 5 },
        runs: { active: 0, succeeded_24h: 0, failed_24h: 0 },
        tasks: { awaiting_decision: 0, blocked: 0 },
        provider: { bound: false, provider_name: null }
      })
    );
    getProjectAiTeamMonitor.mockResolvedValue(monitorFixture([]));
    const { wrapper } = await mountView();
    expect(
      wrapper.find('[data-testid="ai-overview-stat-provider"]').text()
    ).toContain("未绑定");
    expect(
      wrapper.find('[data-testid="ai-overview-runs-empty"]').exists()
    ).toBe(true);
  });

  it("「查看执行监控」链接指向 Monitor 子路由并携带项目上下文", async () => {
    const { wrapper } = await mountView();
    const link = wrapper.find('[data-testid="ai-overview-goto-monitor"]');
    expect(link.attributes("href")).toBe(
      "/project/settings/ai/monitor?project_id=prj-1"
    );
  });
});
