// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import InitializationCard from "./InitializationCard.vue";
import { i18n } from "@/lib/i18n";
import { buildRoutes } from "@/router";
import type {
  CommandCenterInitData,
  InitItemView
} from "../command-center-data";

/**
 * 项目初始化卡（S01-01）行为测试：
 * `completion = not_determinable` 时**不得显示已完成**——无项目类型/初始化
 * 要求配置就无法判定，证书 §4.5 明确禁止宣称完成。
 * S3：代码分析事项的跳转链接与发起/重发动作。
 */

const listImports = vi.fn();
const startImport = vi.fn();
const listRepositoriesV2 = vi.fn();

vi.mock("@/api/imports", () => ({
  listImports: (...args: unknown[]) => listImports(...args),
  startImport: (...args: unknown[]) => startImport(...args)
}));

vi.mock("@/api/initialization", () => ({
  listRepositoriesV2: (...args: unknown[]) => listRepositoriesV2(...args)
}));

const t = (key: string, params?: Record<string, unknown>): string =>
  i18n.global.t(key, params ?? {}) as string;

const labels = {
  complete: "已完成",
  incomplete: "未完成",
  notDeterminable: "无法判定"
};

function analysisItem(): InitItemView {
  return {
    key: "code_analysis",
    title: "代码分析",
    statusLabel: "待开始",
    tone: "neutral",
    required: true,
    detail: "",
    // 深链由 real-data 映射层注入（P1 冷启动：每步可单独进入已有页面）。
    link: { path: "/project/settings/repositories", query: { project_id: "prj-1" } }
  };
}

function data(
  overrides: Partial<CommandCenterInitData> = {}
): CommandCenterInitData {
  return {
    items: [
      {
        key: "repository",
        title: "代码仓库接入",
        statusLabel: "已完成",
        tone: "success",
        required: true,
        detail: ""
      },
      {
        key: "requirements",
        title: "需求录入",
        statusLabel: "待人工处理",
        tone: "warning",
        required: true,
        detail: ""
      }
    ],
    completion: "incomplete",
    requiredDone: 1,
    requiredTotal: 2,
    expanded: true,
    ...overrides
  };
}

function mountCard(payload: CommandCenterInitData | null, projectId = "") {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  void router.push("/project/command-center");
  return mount(InitializationCard, {
    props: {
      title: "项目初始化",
      progressLabel: `必需 ${payload?.requiredDone ?? 0} / ${payload?.requiredTotal ?? 0}`,
      completionLabels: labels,
      requiredLabel: "必需",
      data: payload,
      projectId
    },
    global: { plugins: [i18n, router] }
  });
}

describe("InitializationCard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listImports.mockResolvedValue({ items: [], total: 0 });
    startImport.mockResolvedValue({ import_id: "imp-1", status: "queued" });
    listRepositoriesV2.mockResolvedValue({
      items: [{ id: "repo-1", status: "verified" }],
      total: 1
    });
  });

  it("渲染必需事项完成数与逐项状态", async () => {
    const wrapper = mountCard(data());
    await flushPromises();

    expect(wrapper.find('[data-testid="cc-initialization"]').exists()).toBe(
      true
    );
    expect(wrapper.text()).toContain("必需 1 / 2");
    expect(wrapper.find('[data-testid="cc-init-repository"]').text()).toContain(
      "代码仓库接入"
    );
    expect(
      wrapper.find('[data-testid="cc-init-requirements"]').text()
    ).toContain("待人工处理");
  });

  it("not_determinable 时完成度显示「无法判定」，不误报已完成", async () => {
    const wrapper = mountCard(
      data({
        completion: "not_determinable",
        requiredDone: 2,
        requiredTotal: 2
      })
    );
    await flushPromises();

    // 完成度徽标（头部）必须是「无法判定」——逐项状态里的「已完成」不在此列。
    const head = wrapper.find(".init-card__head").text();
    expect(head).toContain("无法判定");
    expect(head).not.toContain("已完成");
  });

  it("无该块时整卡不渲染（不补空对象）", async () => {
    const wrapper = mountCard(null);
    await flushPromises();

    expect(wrapper.find('[data-testid="cc-initialization"]').exists()).toBe(
      false
    );
  });

  it("S3 代码分析事项带仓库设置跳转链接，动作按钮点击走 startImport（带幂等键）", async () => {
    const wrapper = mountCard(data({ items: [analysisItem()] }), "prj-1");
    await flushPromises();

    const row = wrapper.find('[data-testid="cc-init-code_analysis"]');
    expect(row.find("a").attributes("href")).toContain(
      "/project/settings/repositories?project_id=prj-1"
    );
    // 七态语义不变：状态文案仍按原样渲染。
    expect(row.text()).toContain("待开始");

    const action = wrapper.find('[data-testid="cc-init-analysis-action"]');
    expect(action.exists()).toBe(true);
    await action.trigger("click");
    await flushPromises();

    expect(startImport).toHaveBeenCalledTimes(1);
    const call = startImport.mock.calls[0]!;
    expect(call[0]).toBe("prj-1");
    expect(call[1]).toEqual(["repo-1"]);
    expect(typeof call[2]).toBe("string");
    expect(call[2]).toBeTruthy();
  });

  it("S3 有进行中导入时动作按钮禁用且不可重复发起", async () => {
    listImports.mockResolvedValue({
      items: [
        {
          id: "imp-1",
          status: "analyzing",
          progress: 42,
          model_name: "",
          draft_revision: 0,
          analysis_run_id: null,
          created_at: "2026-11-13T00:00:00Z",
          proposed_at: null,
          applied_at: null
        }
      ],
      total: 1
    });
    const wrapper = mountCard(data({ items: [analysisItem()] }), "prj-1");
    await flushPromises();

    const action = wrapper.find('[data-testid="cc-init-analysis-action"]');
    expect(action.attributes("disabled")).toBeDefined();
    expect(action.text()).toContain(
      t("s01.command-center.init.analysis.running")
    );
    await action.trigger("click");
    await flushPromises();
    expect(startImport).not.toHaveBeenCalled();
  });

  it("S3 failed/cancelled 批次显示「再次分析」，无项目上下文时整段隐藏动作", async () => {
    listImports.mockResolvedValue({
      items: [
        {
          id: "imp-1",
          status: "failed",
          progress: 0,
          model_name: "",
          draft_revision: 0,
          analysis_run_id: null,
          created_at: "2026-11-13T00:00:00Z",
          proposed_at: null,
          applied_at: null
        }
      ],
      total: 1
    });
    // listImports 在 onMounted 调用：先设好状态再挂载。
    const retry = mountCard(data({ items: [analysisItem()] }), "prj-1");
    await flushPromises();
    expect(
      retry.find('[data-testid="cc-init-analysis-action"]').text()
    ).toContain(t("s01.command-center.init.analysis.retry"));

    const noProject = mountCard(data({ items: [analysisItem()] }));
    await flushPromises();
    expect(
      noProject.find('[data-testid="cc-init-analysis-action"]').exists()
    ).toBe(false);
  });


  it("P1 冷启动：无待办且无进行中时自动收起，仅显示摘要与展开入口", async () => {
    const wrapper = mountCard(
      data({ completion: "incomplete", requiredDone: 2, requiredTotal: 2, expanded: false }),
      "prj-1"
    );
    await flushPromises();
    expect(wrapper.find("[data-testid='cc-init-collapsed']").exists()).toBe(
      true
    );
    expect(
      wrapper.find("[data-testid='cc-init-expand']").exists()
    ).toBe(true);
    // 收起不得隐藏人工待办（本夹具含 warning 行 → 后端会判 expanded，此处
    // 直接验证收起态徽标计数呈现）。
    expect(wrapper.text()).toContain(
      i18n.global.t("s01.command-center.init.waitingBadge", { count: 1 })
    );
    await wrapper.find("[data-testid='cc-init-expand']").trigger("click");
    await flushPromises();
    expect(wrapper.find("[data-testid='cc-init-collapsed']").exists()).toBe(
      false
    );
    expect(wrapper.find("[data-testid='cc-init-collapse']").exists()).toBe(
      true
    );
  });
});
