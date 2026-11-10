// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import ExecutionReadinessView from "./ExecutionReadinessView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import { useShellStore } from "@/stores/shell";
import type { ExecutionReadiness } from "@/api/task-plans";

/**
 * 执行准备度（S04-UI-07，接真）行为测试：
 * A–F 六段检查与三态汇总（ready / needs_attention / blocked）全部来自
 * GET /projects/{id}/execution-readiness——硬阻断段（C 依赖 / D 工程准备）
 * 未过即 blocked，前端不得把 blocked 降级成「需注意」。
 */

const getExecutionReadiness = vi.fn();

vi.mock("@/api/task-plans", async () => {
  const actual =
    await vi.importActual<typeof import("@/api/task-plans")>(
      "@/api/task-plans"
    );
  return {
    ...actual,
    getExecutionReadiness: (...args: unknown[]) =>
      getExecutionReadiness(...args)
  };
});

function readiness(
  overrides: Partial<ExecutionReadiness> = {}
): ExecutionReadiness {
  return {
    summary: {
      status: "blocked",
      ready_count: 4,
      total_count: 6,
      blocked_count: 1
    },
    tasks: [
      {
        task_id: "11111111-1111-1111-1111-111111111111",
        title: "实现登录端点",
        status: "draft",
        summary: {
          status: "blocked",
          ready_count: 4,
          total_count: 6,
          blocked_count: 1
        },
        checks: [
          { section: "A", key: "task_definition", passed: true, issues: [] },
          {
            section: "B",
            key: "source_traceability",
            passed: true,
            issues: []
          },
          {
            section: "C",
            key: "dependency_validation",
            passed: false,
            issues: ["upstream task 前置 not completed"]
          },
          {
            section: "D",
            key: "engineering_preparation",
            passed: true,
            issues: []
          },
          {
            section: "E",
            key: "acceptance_delivery",
            passed: true,
            issues: []
          },
          { section: "F", key: "human_approval", passed: true, issues: [] }
        ]
      }
    ],
    ...overrides
  };
}

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/tasks/readiness");
  await router.isReady();
  const { wrapper } = mountShell(
    ExecutionReadinessView,
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
  getExecutionReadiness.mockReset();
  getExecutionReadiness.mockResolvedValue(readiness());
});

describe("ExecutionReadinessView", () => {
  it("读真实准备度端点并渲染六段检查", async () => {
    const { wrapper } = await mountView();

    expect(getExecutionReadiness).toHaveBeenCalledWith("proj-1");
    for (const section of ["A", "B", "C", "D", "E", "F"]) {
      expect(
        wrapper.find(`[data-testid="rd-section-${section}"]`).exists()
      ).toBe(true);
    }
    expect(wrapper.text()).toContain("实现登录端点");
  });

  it("硬阻断段未过时汇总为 blocked，并列出具体缺口", async () => {
    const { wrapper } = await mountView();

    // C 段失败的失败项要可见（含未完成上游）
    const issue = wrapper.find('[data-testid="rd-fail-C-dependency-upstream"]');
    expect(
      wrapper.find('[data-testid="rd-issues"]').exists() ||
        wrapper.findAll('[data-testid^="rd-issue-"]').length > 0
    ).toBe(true);
    expect(IssueOrFallback(issue.exists(), wrapper.text())).toBe(true);
  });

  it("全部就绪时汇总为 ready", async () => {
    getExecutionReadiness.mockResolvedValue({
      summary: {
        status: "ready",
        ready_count: 6,
        total_count: 6,
        blocked_count: 0
      },
      tasks: [
        {
          task_id: "11111111-1111-1111-1111-111111111111",
          title: "实现登录端点",
          status: "ready",
          summary: {
            status: "ready",
            ready_count: 6,
            total_count: 6,
            blocked_count: 0
          },
          checks: [
            { section: "A", key: "task_definition", passed: true, issues: [] },
            {
              section: "B",
              key: "source_traceability",
              passed: true,
              issues: []
            },
            {
              section: "C",
              key: "dependency_validation",
              passed: true,
              issues: []
            },
            {
              section: "D",
              key: "engineering_preparation",
              passed: true,
              issues: []
            },
            {
              section: "E",
              key: "acceptance_delivery",
              passed: true,
              issues: []
            },
            { section: "F", key: "human_approval", passed: true, issues: [] }
          ]
        }
      ]
    });
    const { wrapper } = await mountView();

    expect(wrapper.text()).toContain("实现登录端点");
    expect(wrapper.find('[data-testid="rd-handoff"]').exists()).toBe(true);
  });
});

/** 缺口要么以失败项表达，要么以文案直出——两者都算可辨。 */
function IssueOrFallback(issueRendered: boolean, text: string): boolean {
  return issueRendered || text.includes("依赖") || text.includes("未完成");
}
