// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { HttpError } from "@digua/http";

import SettingsProjectAiExecutionPoliciesView from "./SettingsProjectAiExecutionPoliciesView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import type {
  AiTeamExecutionPolicies,
  AiTeamTaskPolicyOverride
} from "@/api/aiTeam";

const getProjectAiExecutionPolicies = vi.fn();
const setProjectExecutionPolicy = vi.fn();
const clearTaskPolicyOverride = vi.fn();

vi.mock("@/api/aiTeam", () => ({
  getProjectAiExecutionPolicies: (...args: unknown[]) =>
    getProjectAiExecutionPolicies(...args),
  setProjectExecutionPolicy: (...args: unknown[]) =>
    setProjectExecutionPolicy(...args),
  clearTaskPolicyOverride: (...args: unknown[]) =>
    clearTaskPolicyOverride(...args)
}));

/** 后端真实行形状（ExecutionPoliciesView 契约）的最小夹具 */
function policiesFixture(
  overrides: Partial<AiTeamExecutionPolicies> = {}
): AiTeamExecutionPolicies {
  return {
    organization: null,
    project: null,
    taskTypes: [],
    taskOverrides: [],
    composition: {
      layer_order: ["organization", "project", "task_type", "task"],
      defaults: {
        auto_accept: true,
        auto_merge: true,
        auto_publish_knowledge: true,
        require_human_approval: false,
        additional_checks: []
      },
      rules: [
        {
          key: "constraints",
          text: "约束类字段 require_human_approval 只能收紧：任一层为 true 即要求人工审批。"
        }
      ]
    },
    ...overrides
  };
}

function overrideRow(
  overrides: Partial<AiTeamTaskPolicyOverride>
): AiTeamTaskPolicyOverride {
  return {
    task_id: "task-1",
    task_title: "Implement login",
    auto_accept: null,
    auto_merge: null,
    auto_publish_knowledge: null,
    require_human_approval: true,
    updated_at: "2026-11-08T00:00:00Z",
    ...overrides
  };
}

async function mountView(
  path = "/project/settings/ai/execution-policies?project_id=prj-1"
) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  router.push(path);
  await router.isReady();
  const mounted = mountShell(SettingsProjectAiExecutionPoliciesView, {}, router);
  await flushPromises();
  return mounted;
}

function projectSwitch(wrapper: ReturnType<typeof mountShell>["wrapper"]) {
  return wrapper.find('[data-testid="ai-exec-pol-project-switch"] [role="switch"]');
}

describe("SettingsProjectAiExecutionPoliciesView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getProjectAiExecutionPolicies.mockResolvedValue(policiesFixture());
  });

  it("无项目上下文时不发请求（scope 哨兵不得当作项目 id）", async () => {
    const { wrapper } = await mountView("/project/settings/ai/execution-policies");
    expect(getProjectAiExecutionPolicies).not.toHaveBeenCalled();
    expect(
      wrapper.find('[data-testid="ai-exec-pol-no-project"]').exists()
    ).toBe(true);
  });

  it("分层渲染：org 只读态 / project 继承 / task types 表 / overrides 继承徽标 / 合成规则块", async () => {
    getProjectAiExecutionPolicies.mockResolvedValue(
      policiesFixture({
        organization: {
          require_human_approval: true,
          updated_by: "p-0",
          updated_at: "2026-11-01T00:00:00Z"
        },
        taskTypes: [
          {
            task_type: "backend",
            require_human_approval: true,
            updated_by: "p-0",
            updated_at: "2026-11-02T00:00:00Z"
          }
        ],
        taskOverrides: [overrideRow()]
      })
    );
    const { wrapper } = await mountView();

    // Organization：有行 → 值展示（只读卡，无开关）
    const org = wrapper.find('[data-testid="ai-exec-pol-org"]');
    expect(org.find('[data-testid="ai-exec-pol-org-value"]').exists()).toBe(
      true
    );
    expect(org.text()).toContain("已强制人工审批");
    expect(org.find('[role="switch"]').exists()).toBe(false);

    // Project：无行 → 继承态（开关关 + 继承文案），无清除按钮
    expect(projectSwitch(wrapper).attributes("disabled")).toBeUndefined();
    expect(
      wrapper.find('[data-testid="ai-exec-pol-project-state"]').text()
    ).toContain("继承组织层");
    expect(
      wrapper.find('[data-testid="ai-exec-pol-project-clear"]').exists()
    ).toBe(false);

    // Task types：只读表一行（backend / 开）
    expect(
      wrapper.find('[data-testid="ai-exec-pol-task-type-backend"]').exists()
    ).toBe(true);

    // Overrides：NULL 字段显示「继承」徽标，行带清除按钮
    const row = wrapper.find('[data-testid="ai-exec-pol-override-task-1"]');
    expect(row.exists()).toBe(true);
    expect(row.text()).toContain("Implement login");
    expect(row.text()).toContain("继承");
    expect(
      row.find('[data-testid="ai-exec-pol-override-clear"]').exists()
    ).toBe(true);

    // 合成规则块：默认值 + 后端 rules 文本
    const composition = wrapper.find('[data-testid="ai-exec-pol-composition"]');
    expect(composition.text()).toContain("organization → project");
    expect(composition.text()).toContain("只能收紧");
  });

  it("保存 project 开关：PUT true 契约，响应行落地后出现清除按钮", async () => {
    setProjectExecutionPolicy.mockResolvedValue({
      require_human_approval: true,
      updated_by: "p-1",
      updated_at: "2026-11-08T00:00:00Z"
    });
    const { wrapper } = await mountView();

    await projectSwitch(wrapper).trigger("click");
    await flushPromises();

    expect(setProjectExecutionPolicy).toHaveBeenCalledTimes(1);
    expect(setProjectExecutionPolicy).toHaveBeenCalledWith("prj-1", true);
    // 开关置开 + 清除（恢复继承）按钮出现
    expect(
      wrapper.find('[data-testid="ai-exec-pol-project-state"]').text()
    ).toContain("开");
    expect(
      wrapper.find('[data-testid="ai-exec-pol-project-clear"]').exists()
    ).toBe(true);
  });

  it("写 403 → 整页只读态（开关禁用 + 无权限提示）", async () => {
    setProjectExecutionPolicy.mockRejectedValue(
      new HttpError("project manager required", { status: 403 })
    );
    const { wrapper } = await mountView();

    await projectSwitch(wrapper).trigger("click");
    await flushPromises();

    const alert = wrapper.find('[data-testid="ai-exec-pol-action-error"]');
    expect(alert.exists()).toBe(true);
    expect(alert.text()).toContain("无权限");
    // 403 后写控件全禁用
    expect(projectSwitch(wrapper).attributes("disabled")).toBeDefined();
  });

  it("清除任务覆盖：DELETE 契约 + 成功后整页回读", async () => {
    // 首次 GET 带覆盖行；清除后的回读（第二次 GET）返回空表
    getProjectAiExecutionPolicies
      .mockResolvedValueOnce(policiesFixture({ taskOverrides: [overrideRow()] }))
      .mockResolvedValueOnce(policiesFixture());
    clearTaskPolicyOverride.mockResolvedValue(undefined);
    const { wrapper } = await mountView();
    expect(getProjectAiExecutionPolicies).toHaveBeenCalledTimes(1);

    await wrapper
      .find('[data-testid="ai-exec-pol-override-clear"]')
      .trigger("click");
    await flushPromises();

    expect(clearTaskPolicyOverride).toHaveBeenCalledTimes(1);
    expect(clearTaskPolicyOverride).toHaveBeenCalledWith("task-1");
    // 操作后回读（第二次 GET），覆盖行消失
    expect(getProjectAiExecutionPolicies).toHaveBeenCalledTimes(2);
    expect(
      wrapper.find('[data-testid="ai-exec-pol-override-task-1"]').exists()
    ).toBe(false);
  });
});
