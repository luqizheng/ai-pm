// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import TaskDetailView from "./TaskDetailView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import type { TaskDetailView as TaskDetailModel } from "@/api/task-plans";

/**
 * 任务详情（S04-UI-03，接真）行为测试：
 * 依据追溯 / 依赖方向 / 验收标准 / 审计历史全部来自
 * GET /v1/pm/tasks/{task_id} 的真实读模型——**不得用演示文案填充**。
 */

const getTaskDetail = vi.fn();

vi.mock("@/api/task-plans", () => ({
  getTaskDetail: (...args: unknown[]) => getTaskDetail(...args)
}));

function detail(): TaskDetailModel {
  return {
    task: {
      id: "11111111-1111-1111-1111-111111111111",
      project_id: "proj-1",
      title: "实现登录端点",
      description: "登录端点、凭据校验、令牌签发。",
      status: "draft",
      priority: "high",
      assignee_id: null,
      revision: 1,
      goal: "在 core-api 提供 POST /v1/auth/login 端点。",
      constraints: { security: "不得记录明文凭据" },
      deliverables: ["代码变更（core-api）", "单元测试"],
      git_branch: null,
      planned_start_at: null,
      planned_end_at: null,
      estimate_hours: null,
      implementation_baseline_id: "ib-1",
      plan_revision_id: null,
      created_at: "2026-11-10T08:00:00Z",
      updated_at: "2026-11-10T09:00:00Z"
    },
    traces: [
      {
        kind: "requirement",
        target_id: "req-1",
        code: "REQ-001",
        title: "用户登录",
        version: "3",
        state: "approved"
      },
      {
        kind: "adr",
        target_id: "adr-1",
        code: "ADR-001",
        title: "会话安全",
        version: null,
        state: "accepted"
      }
    ],
    dependencies: [
      {
        id: "22222222-2222-2222-2222-222222222222",
        title: "定义认证请求模型",
        status: "completed",
        priority: "normal",
        direction: "upstream"
      },
      {
        id: "33333333-3333-3333-3333-333333333333",
        title: "补充认证测试",
        status: "draft",
        priority: "normal",
        direction: "downstream"
      }
    ],
    repositories: [
      {
        repository_id: "repo-1",
        repository_name: "core-api",
        base_ref: "main",
        base_commit: "abc123",
        write_access: true,
        path_scope: ["services/auth"],
        role: "primary",
        repository_verified: true
      }
    ],
    criteria: [
      {
        id: "44444444-4444-4444-4444-444444444444",
        ordinal: 1,
        text: "给定有效凭据，登录返回 200 与访问令牌",
        verification_kind: "automated_test"
      }
    ],
    approvals: [],
    history: [
      {
        id: "h1",
        actor_id: "p-1",
        actor_name: "张三",
        action: "task.created",
        created_at: "2026-11-10T08:00:00Z"
      }
    ],
    baseline: {
      id: "ib-1",
      code: "IB-001",
      status: "frozen",
      content: {
        scope: { included: ["auth"], excluded: ["OAuth"], modules: ["auth"] }
      }
    }
  };
}

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/tasks/11111111-1111-1111-1111-111111111111");
  await router.isReady();
  const wrapper = mountShell(
    TaskDetailView,
    { attachTo: document.body },
    router
  ).wrapper;
  await flushPromises();
  return { wrapper, router };
}

beforeEach(() => {
  getTaskDetail.mockReset();
  getTaskDetail.mockResolvedValue(detail());
});

describe("TaskDetailView", () => {
  it("读真实单任务端点并渲染七区块", async () => {
    const { wrapper } = await mountView();

    expect(getTaskDetail).toHaveBeenCalledWith(
      "11111111-1111-1111-1111-111111111111"
    );
    for (const section of [
      "dt-section-objective",
      "dt-section-trace",
      "dt-section-deps",
      "dt-section-spec",
      "dt-section-criteria",
      "dt-section-delivery",
      "dt-section-history"
    ]) {
      expect(wrapper.find(`[data-testid="${section}"]`).exists()).toBe(true);
    }
    expect(wrapper.text()).toContain("实现登录端点");
    expect(wrapper.text()).toContain("REQ-001");
    expect(wrapper.text()).toContain("ADR-001");
  });

  it("依赖方向区分前置与后继；验收标准与交付来自真实字段", async () => {
    const { wrapper } = await mountView();

    const deps = wrapper.find('[data-testid="dt-section-deps"]').text();
    expect(deps).toContain("定义认证请求模型");
    expect(deps).toContain("补充认证测试");
    expect(
      wrapper.find('[data-testid="dt-section-criteria"]').text()
    ).toContain("登录返回 200");
    expect(
      wrapper.find('[data-testid="dt-section-delivery"]').text()
    ).toContain("代码变更（core-api）");
    expect(wrapper.find('[data-testid="dt-section-spec"]').text()).toContain(
      "core-api"
    );
  });

  it("读端点失败时给错误态而非空壳", async () => {
    getTaskDetail.mockRejectedValue(new Error("boom"));
    const { wrapper } = await mountView();

    expect(wrapper.text()).toContain("加载失败");
  });
});
