// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import SettingsProjectAiMonitorView from "./SettingsProjectAiMonitorView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import type { AiTeamMonitor } from "@/api/aiTeam";

const getProjectAiTeamMonitor = vi.fn();

vi.mock("@/api/aiTeam", () => ({
  getProjectAiTeamOverview: vi.fn(),
  getProjectAiTeamMonitor: (...args: unknown[]) =>
    getProjectAiTeamMonitor(...args)
}));

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

function event(overrides: Partial<AiTeamMonitor["events"][number]> = {}) {
  return {
    action: "skip_task_run",
    target_id: "run-9",
    target_title: "Implement login",
    actor: "alice",
    reason: "scheduling_shield",
    created_at: "2026-11-08T08:50:00Z",
    ...overrides
  };
}

async function mountView(
  path = "/project/settings/ai/monitor?project_id=prj-1"
) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  router.push(path);
  await router.isReady();
  const mounted = mountShell(SettingsProjectAiMonitorView, {}, router);
  await flushPromises();
  return mounted;
}

describe("SettingsProjectAiMonitorView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getProjectAiTeamMonitor.mockResolvedValue({
      runs: [run()],
      events: [event()]
    });
  });

  it("无项目上下文时不发请求（scope 哨兵不得当作项目 id）", async () => {
    const { wrapper } = await mountView("/project/settings/ai/monitor");
    expect(getProjectAiTeamMonitor).not.toHaveBeenCalled();
    expect(wrapper.find('[data-testid="ai-monitor-no-project"]').exists()).toBe(
      true
    );
  });

  it("Runs 表渲染 task/status/stage/attempt/executor/shield/时间", async () => {
    const { wrapper } = await mountView();
    expect(getProjectAiTeamMonitor).toHaveBeenCalledWith("prj-1");
    const section = wrapper.find('[data-testid="ai-monitor-runs"]');
    expect(section.text()).toContain("Implement login");
    expect(section.text()).toContain("running");
    expect(section.text()).toContain("started");
    expect(section.text()).toContain("#2");
    expect(section.text()).toContain("codex");
    expect(section.text()).toContain("none");
    expect(wrapper.find('[data-testid="ai-monitor-run-run-1"]').exists()).toBe(
      true
    );
  });

  it("release_reason 与 shield 警示值原样展示（后端原词不翻译）", async () => {
    getProjectAiTeamMonitor.mockResolvedValue({
      runs: [
        run({
          run_id: "run-2",
          status: "queued",
          release_reason: "dependency_unmet",
          scheduling_shield: "human_wait"
        })
      ],
      events: []
    });
    const { wrapper } = await mountView();
    const row = wrapper.find('[data-testid="ai-monitor-run-run-2"]');
    expect(row.text()).toContain("dependency_unmet");
    expect(row.text()).toContain("human_wait");
  });

  it("Events 时间线渲染 action/actor/reason；actor 缺失回退「系统」", async () => {
    getProjectAiTeamMonitor.mockResolvedValue({
      runs: [],
      events: [
        event(),
        event({
          action: "reclaim_task_run",
          actor: null,
          reason: "lease_expired_unstarted",
          target_title: null
        })
      ]
    });
    const { wrapper } = await mountView();
    const timeline = wrapper.find('[data-testid="ai-monitor-events"]');
    expect(timeline.text()).toContain("skip_task_run");
    expect(timeline.text()).toContain("alice");
    expect(timeline.text()).toContain("scheduling_shield");
    expect(wrapper.find('[data-testid="ai-monitor-event-1"]').text()).toContain(
      "reclaim_task_run"
    );
    expect(wrapper.find('[data-testid="ai-monitor-event-1"]').text()).toContain(
      "lease_expired_unstarted"
    );
    expect(wrapper.find('[data-testid="ai-monitor-event-1"]').text()).toContain(
      "系统"
    );
  });

  it("空项目：Runs 与 Events 各自空态（零值结构可见）", async () => {
    getProjectAiTeamMonitor.mockResolvedValue({ runs: [], events: [] });
    const { wrapper } = await mountView();
    expect(wrapper.find('[data-testid="ai-monitor-runs-empty"]').exists()).toBe(
      true
    );
    expect(
      wrapper.find('[data-testid="ai-monitor-events-empty"]').exists()
    ).toBe(true);
  });
});
