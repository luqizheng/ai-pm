// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import CommandCenterView from "./index.vue";
import { mountShell } from "../../../test-utils";

const getCommandCenter = vi.fn();

vi.mock("@/api/command-center", () => ({
  getCommandCenter: (...args: unknown[]) => getCommandCenter(...args)
}));

vi.mock("vue-router", () => ({
  useRoute: () => ({ params: { projectId: "project-1" } }),
  useRouter: () => ({ push: vi.fn() })
}));

function okBlock<T>(data: T) {
  return { status: "ok", as_of: "2026-11-04T09:00:00Z", data };
}

function fullCenter() {
  return {
    project_id: "project-1",
    as_of: "2026-11-04T09:00:00Z",
    blocks: {
      project_summary: okBlock({
        project_id: "project-1",
        code: "DEMO",
        name: "演示项目",
        lifecycle: "planning",
        archived: false,
        goal: { present: true, revision_no: 2, pending_proposal: false },
        repository_count: 2,
        verified_repository_count: 1,
        member_count: 5,
        last_activity_at: "2026-11-04T08:00:00Z"
      }),
      initialization_progress: okBlock({
        items: [
          {
            key: "project_basics",
            title: "项目基本信息",
            status: "completed",
            required: true
          },
          {
            key: "repository_access",
            title: "仓库接入",
            status: "not_started",
            required: false
          }
        ],
        required_items: ["project_basics"],
        completion: "not_determinable",
        expanded: true
      }),
      work_summary: okBlock({
        task_total: 3,
        task_by_status: { running: 1 },
        active_run_count: 1,
        active_runs: [
          {
            run_id: "run-1",
            task_id: "task-1",
            task_title: "正在执行的任务",
            run_kind: "delivery",
            attempt_no: 1,
            status: "running",
            agent: {
              principal_id: "p-1",
              display_name: "dev-agent",
              kind: "dev_agent"
            },
            repositories: []
          }
        ],
        recent_runs: [],
        delivery: { delivered: 0, partial: 0, missing: 0, no_contract: 1 }
      }),
      human_action_summary: okBlock({
        total: 1,
        items: [
          {
            kind: "finding_pending",
            domain: "discovery",
            id: "finding-1",
            project_id: "project-1",
            title: "发现待处理（observed）",
            source_state: "PENDING",
            action_hint: "确认或拒绝该发现",
            link: { kind: "discovery_finding", params: { finding_id: "finding-1" } }
          }
        ]
      }),
      risk_summary: okBlock({ total: 0, items: [] }),
      recent_activity: okBlock({
        total: 1,
        items: [
          {
            id: "audit-1",
            activity_type: "finding_decided",
            source_type: "discovery_finding",
            source_id: "finding-1",
            occurred_at: "2026-11-04T08:00:00Z",
            actor: { kind: "human", name: "张三" },
            summary: "发现已由人工裁决",
            dedup_key: "finding_decided:discovery_finding:finding-1"
          }
        ]
      })
    }
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  getCommandCenter.mockResolvedValue(fullCenter());
});

describe("项目总控台", () => {
  it("渲染六个区块并按首屏优先级排序（待人工处理先于当前执行）", async () => {
    const wrapper = mountShell(CommandCenterView);
    await flushPromises();

    const text = wrapper.text();
    expect(text).toContain("待人工处理");
    expect(text).toContain("当前执行");
    expect(text).toContain("风险与阻塞");
    expect(text).toContain("初始化进度");
    expect(text).toContain("最近活动");
    expect(text).toContain("项目概况");
    // 待人工处理条目在当前执行之前。
    expect(text.indexOf("发现待处理")).toBeLessThan(
      text.indexOf("正在执行的任务")
    );
  });

  it("区块不可用时显示「暂不可用」而不是 0", async () => {
    const center = fullCenter();
    center.blocks.work_summary = {
      status: "unavailable",
      as_of: "2026-11-04T09:00:00Z",
      error: { code: "work_summary", message: "该数据源暂时不可用，请稍后重试" }
    } as never;
    getCommandCenter.mockResolvedValue(center);

    const wrapper = mountShell(CommandCenterView);
    await flushPromises();

    const text = wrapper.text();
    expect(text).toContain("该数据源暂时不可用");
    // 不可用不得显示成 0。
    expect(text).not.toContain("执行中: 0");
  });

  it("归档项目显示只读提示", async () => {
    const center = fullCenter();
    center.blocks.project_summary = okBlock({
      ...(center.blocks.project_summary.data as never),
      archived: true,
      lifecycle: "archived"
    });
    getCommandCenter.mockResolvedValue(center);

    const wrapper = mountShell(CommandCenterView);
    await flushPromises();

    expect(wrapper.text()).toContain("项目已归档");
  });

  it("初始化进度不展示百分比，且完成度不可判定时如实说明", async () => {
    const wrapper = mountShell(CommandCenterView);
    await flushPromises();

    const text = wrapper.text();
    expect(text).toContain("尚无初始化要求配置");
    expect(text).not.toContain("%");
  });

  it("空集合显示空状态文案（与不可用区分）", async () => {
    const center = fullCenter();
    center.blocks.human_action_summary = okBlock({ total: 0, items: [] });
    center.blocks.recent_activity = okBlock({ total: 0, items: [] });
    getCommandCenter.mockResolvedValue(center);

    const wrapper = mountShell(CommandCenterView);
    await flushPromises();

    const text = wrapper.text();
    expect(text).toContain("当前没有风险与阻塞");
    expect(text).toContain("当前没有待人工处理的事项");
    expect(text).toContain("暂无活动记录");
    // 空集合不得被显示成「暂不可用」。
    expect(text).not.toContain("该数据源暂时不可用");
  });
});
