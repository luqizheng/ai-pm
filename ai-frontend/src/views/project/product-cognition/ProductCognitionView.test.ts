// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";

import ProductCognitionView from "./ProductCognitionView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { useShellStore } from "@/stores/shell";
import type {
  PcObjectDetail,
  PcObjectRow
} from "@/api/knowledge";

const listPcObjects = vi.fn();
const getPcObject = vi.fn();

vi.mock("@/api/knowledge", () => ({
  listPcObjects: (...args: unknown[]) => listPcObjects(...args),
  getPcObject: (...args: unknown[]) => getPcObject(...args)
}));

function pcRow(overrides: Partial<PcObjectRow> = {}): PcObjectRow {
  return {
    object_id: "pc-1",
    object_type: "domain_object",
    object_key: "order-status",
    display_name: "订单状态",
    lifecycle_status: "active",
    revision_no: 3,
    change_summary: "补充状态机",
    excerpt: "订单状态机包括待付款与已付款。",
    updated_at: "2026-11-10T07:00:00Z",
    ...overrides
  };
}

function pcDetail(row: PcObjectRow): PcObjectDetail {
  return {
    object_id: row.object_id,
    object_type: row.object_type,
    object_key: row.object_key,
    name: row.display_name,
    lifecycle_status: row.lifecycle_status,
    latest_revision: {
      revision_no: 3,
      summary: "补充状态机",
      content: "# 订单状态\n\n待付款 → 已付款 → 已发货。",
      analysis_mode: "derived",
      versioned: true
    },
    analysis_sources: [
      {
        report_id: "report-1",
        repository_name: "order-service",
        commit_sha: "0123456789abcdef",
        version_no: 3,
        confirmed_at: "2026-11-10T07:00:00Z"
      }
    ]
  };
}

async function mountView(projectId: string | null = "proj-1") {
  const mounted = mountShell(ProductCognitionView);
  const shell = useShellStore();
  shell.setProjects(
    projectId ? [{ id: projectId, name: "评测项目" }] : []
  );
  shell.setCurrentProject(projectId);
  await flushPromises();
  return mounted;
}

describe("ProductCognitionView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listPcObjects.mockResolvedValue({ items: [pcRow()] });
    getPcObject.mockImplementation((id: string) =>
      Promise.resolve(pcDetail(pcRow({ object_id: id })))
    );
  });

  it("读取当前项目的产品认知对象并展示摘要", async () => {
    const { wrapper } = await mountView();

    expect(listPcObjects).toHaveBeenCalledWith("proj-1");
    expect(wrapper.text()).toContain("订单状态");
    expect(wrapper.text()).toContain("补充状态机");
  });

  it("展开对象时读取真实详情、版本正文与来源报告", async () => {
    const { wrapper, router } = await mountView();

    await wrapper.find(".product-cognition__head").trigger("click");
    await flushPromises();

    expect(getPcObject).toHaveBeenCalledWith("pc-1", "proj-1");
    expect(wrapper.text()).toContain("待付款 → 已付款 → 已发货。");
    expect(wrapper.text()).toContain("order-service @0123456789ab");

    await wrapper.find(".product-cognition__link").trigger("click");
    await flushPromises();
    expect(router.currentRoute.value.path).toBe(
      "/project/analysis/reports/report-1"
    );
  });

  it("提案按钮跳转统一人工审核中心，不在本页执行审批", async () => {
    const { wrapper, router } = await mountView();

    const button = wrapper.findAll("button").find(item =>
      item.text().includes("提案")
    );
    expect(button).toBeTruthy();
    await button!.trigger("click");
    await flushPromises();

    expect(router.currentRoute.value.path).toBe("/project/review-center");
    expect(router.currentRoute.value.query).toMatchObject({
      project_id: "proj-1",
      kind: "proposal"
    });
  });

  it("没有项目上下文时显示空态且不请求跨项目数据", async () => {
    const { wrapper } = await mountView(null);

    expect(listPcObjects).not.toHaveBeenCalled();
    expect(wrapper.find(".product-cognition__list").exists()).toBe(false);
  });

  it("加载失败显示错误，不伪装成空数据", async () => {
    listPcObjects.mockRejectedValue(new Error("permission denied"));
    const { wrapper } = await mountView();

    expect(wrapper.text()).toContain("permission denied");
    expect(wrapper.find(".product-cognition__list").exists()).toBe(false);
  });
});
