// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import AnalysisHomeView from "./AnalysisHomeView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { useShellStore } from "@/stores/shell";
import { HttpError } from "@digua/http";
import type { AnalysisReportRow } from "@/api/analysis-reports";

/**
 * P2-02 专属测试（关闭核验缺口 K-06）：状态分区 / 历史开关 / 轮询启停 /
 * 行点击导航 / 深链承接 / 四态（加载·空·错误·404 无权限语义）。
 * 列表行为与旧 AnalysisReportsPanel 等价（行展开升级为跳详情页）。
 */

const listAnalysisReports = vi.fn();

vi.mock("@/api/analysis-reports", () => ({
  listAnalysisReports: (...args: unknown[]) => listAnalysisReports(...args)
}));

function reportRow(overrides: Partial<AnalysisReportRow>): AnalysisReportRow {
  return {
    id: "rep-1",
    repository_id: "repo-1",
    repository_name: "payment-service",
    commit_sha: "abc123def4567890abcdef",
    status: "analyzed",
    stage_detail: null,
    error_code: null,
    error_detail: null,
    created_at: "2026-11-15T08:00:00Z",
    updated_at: "2026-11-15T09:00:00Z",
    closed_at: null,
    kb_revision_id: "kbr-1",
    pc_source_id: null,
    run_mode: "full",
    since_commit: null,
    pending_proposals: 2,
    has_pending_kb: true,
    ...overrides
  };
}

async function mountView(query?: Record<string, string>) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/project/analysis", component: AnalysisHomeView },
      {
        path: "/project/analysis/reports/:reportId",
        component: { template: "<div />" }
      }
    ]
  });
  await router.push({ path: "/project/analysis", query });
  await router.isReady();
  const { wrapper } = mountShell(
    AnalysisHomeView,
    { attachTo: document.body },
    router
  );
  const shell = useShellStore();
  shell.setProjects([{ id: "proj-1", name: "评测项目" }]);
  shell.setCurrentProject("proj-1");
  await flushPromises();
  return { wrapper, router, shell };
}

beforeEach(() => {
  listAnalysisReports.mockReset();
  listAnalysisReports.mockResolvedValue({ items: [] });
});

afterEach(() => {
  vi.useRealTimers();
});

describe("AnalysisHomeView 列表（P2-02）", () => {
  it("状态分区：默认展示 analyzing/analyzed，历史（closed/failed）隐藏", async () => {
    listAnalysisReports.mockResolvedValue({
      items: [
        reportRow({ id: "r-a", status: "analyzing" }),
        reportRow({ id: "r-b", status: "analyzed" }),
        reportRow({ id: "r-c", status: "closed", closed_at: "2026-11-15T10:00:00Z" }),
        reportRow({ id: "r-d", status: "failed", error_detail: "worker crashed" })
      ]
    });
    const { wrapper } = await mountView();

    const groupText = wrapper
      .findAll(".an-group-title")
      .map(n => n.text())
      .join("|");
    expect(groupText).toContain("分析中");
    expect(groupText).toContain("待审理");
    expect(groupText).not.toContain("已关闭");
    expect(groupText).not.toContain("失败");
    expect(wrapper.findAll(".an-row").length).toBe(2);
    // failed 明细不随历史泄漏
    expect(wrapper.text()).not.toContain("worker crashed");
  });

  it("「显示历史」开关展开 closed/failed 分组，再点隐藏", async () => {
    listAnalysisReports.mockResolvedValue({
      items: [
        reportRow({ id: "r-b", status: "analyzed" }),
        reportRow({ id: "r-c", status: "closed", closed_at: "2026-11-15T10:00:00Z" }),
        reportRow({ id: "r-d", status: "failed", error_detail: "worker crashed" })
      ]
    });
    const { wrapper } = await mountView();

    const toggle = wrapper
      .findAll("button")
      .find(b => b.text().includes("显示历史"))!;
    await toggle.trigger("click");
    await flushPromises();

    expect(wrapper.findAll(".an-row").length).toBe(3);
    expect(wrapper.text()).toContain("已关闭");
    expect(wrapper.text()).toContain("worker crashed");

    const hideToggle = wrapper
      .findAll("button")
      .find(b => b.text().includes("隐藏历史"))!;
    await hideToggle.trigger("click");
    await flushPromises();
    expect(wrapper.findAll(".an-row").length).toBe(1);
  });

  it("行字段：增量徽标 + pending 计数（pending_proposals + has_pending_kb）", async () => {
    listAnalysisReports.mockResolvedValue({
      items: [reportRow({ id: "r-b", run_mode: "incremental" })]
    });
    const { wrapper } = await mountView();

    expect(wrapper.text()).toContain("增量");
    expect(wrapper.text()).toContain("3 项待处置");
    expect(wrapper.text()).toContain("payment-service @ abc123def456");
  });

  it("空态：默认视图无可见行时展示空态文案", async () => {
    listAnalysisReports.mockResolvedValue({
      items: [reportRow({ id: "r-c", status: "closed", closed_at: "2026-11-15T10:00:00Z" })]
    });
    const { wrapper } = await mountView();
    expect(wrapper.text()).toContain("暂无分析报告");
  });

  it("无项目上下文：空态且不请求列表（不展示其他项目数据）", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/project/analysis", component: AnalysisHomeView }]
    });
    await router.push("/project/analysis");
    await router.isReady();
    const { wrapper } = mountShell(
      AnalysisHomeView,
      { attachTo: document.body },
      router
    );
    const shell = useShellStore();
    shell.setProjects([{ id: "proj-1", name: "评测项目" }]);
    await flushPromises();

    expect(listAnalysisReports).not.toHaveBeenCalled();
    expect(wrapper.text()).toContain("请先选择项目");
    wrapper.unmount();
  });

  it("analyzing 行存在时 3s 轮询；转为 analyzed 后轮询停止；卸载清理", async () => {
    vi.useFakeTimers();
    listAnalysisReports.mockResolvedValue({
      items: [reportRow({ id: "r-a", status: "analyzing" })]
    });
    const { wrapper } = await mountView();
    expect(listAnalysisReports).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(3000);
    expect(listAnalysisReports).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(3000);
    expect(listAnalysisReports).toHaveBeenCalledTimes(3);

    // analyzing → analyzed：下一拍拉到新状态后轮询停止
    listAnalysisReports.mockResolvedValue({
      items: [reportRow({ id: "r-a", status: "analyzed" })]
    });
    await vi.advanceTimersByTimeAsync(3000);
    expect(listAnalysisReports).toHaveBeenCalledTimes(4);
    await vi.advanceTimersByTimeAsync(9000);
    expect(listAnalysisReports).toHaveBeenCalledTimes(4);

    wrapper.unmount();
  });

  it("无 analyzing 行时不启动轮询", async () => {
    vi.useFakeTimers();
    listAnalysisReports.mockResolvedValue({
      items: [reportRow({ id: "r-b", status: "analyzed" })]
    });
    const { wrapper } = await mountView();
    await vi.advanceTimersByTimeAsync(9000);
    expect(listAnalysisReports).toHaveBeenCalledTimes(1);
    wrapper.unmount();
  });

  it("行点击：analyzed 行跳详情页；analyzing 行不可进", async () => {
    listAnalysisReports.mockResolvedValue({
      items: [
        reportRow({ id: "r-a", status: "analyzing" }),
        reportRow({ id: "r-b", status: "analyzed" })
      ]
    });
    const { wrapper, router } = await mountView();

    const heads = wrapper.findAll(".an-row .an-head");
    expect(heads[0].attributes("disabled")).toBeDefined(); // analyzing 行
    await heads[0].trigger("click");
    expect(router.currentRoute.value.path).toBe("/project/analysis");

    await heads[1].trigger("click");
    await flushPromises();
    expect(router.currentRoute.value.path).toBe(
      "/project/analysis/reports/r-b"
    );
  });

  it("?report= 深链承接：定位列表内报告并跳详情页（保留其余 query）", async () => {
    listAnalysisReports.mockResolvedValue({
      items: [reportRow({ id: "rep-9", status: "analyzed" })]
    });
    const { router, wrapper } = await mountView({
      project_id: "proj-1",
      report: "rep-9"
    });
    await flushPromises();

    expect(router.currentRoute.value.path).toBe(
      "/project/analysis/reports/rep-9"
    );
    expect(router.currentRoute.value.query.project_id).toBe("proj-1");
    expect(router.currentRoute.value.query.report).toBeUndefined();
    wrapper.unmount();
  });

  it("?report= 非法/跨项目 ID：404 语义错误态，不静默不跳转（GAP-6 列表侧）", async () => {
    listAnalysisReports.mockResolvedValue({
      items: [reportRow({ id: "rep-1", status: "analyzed" })]
    });
    const { router, wrapper } = await mountView({ report: "rep-404" });
    await flushPromises();

    expect(router.currentRoute.value.path).toBe("/project/analysis");
    expect(wrapper.text()).toContain("报告不存在或无权访问");
    wrapper.unmount();
  });

  it("列表 404（项目不可见）→ 无权限语义态；其他错误 → 通用错误态", async () => {
    listAnalysisReports.mockRejectedValue(
      new HttpError("not found", { status: 404 })
    );
    const { wrapper } = await mountView();
    expect(wrapper.text()).toContain("报告不存在或无权访问");
    wrapper.unmount();

    listAnalysisReports.mockRejectedValue(new Error("boom"));
    const second = await mountView();
    expect(second.wrapper.text()).toContain("加载失败");
    second.wrapper.unmount();
  });

  it("项目切换：重置状态并以新项目重载列表", async () => {
    listAnalysisReports.mockResolvedValue({
      items: [reportRow({ id: "r-b", status: "analyzed" })]
    });
    const { wrapper, shell } = await mountView();
    expect(listAnalysisReports).toHaveBeenNthCalledWith(1, "proj-1");

    shell.setProjects([
      { id: "proj-1", name: "评测项目" },
      { id: "proj-2", name: "另一项目" }
    ]);
    shell.setCurrentProject("proj-2");
    await flushPromises();

    expect(listAnalysisReports).toHaveBeenNthCalledWith(2, "proj-2");
    wrapper.unmount();
  });
});
