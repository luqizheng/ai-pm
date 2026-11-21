// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import KnowledgeObjectTraceabilityView from "./KnowledgeObjectTraceabilityView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { useShellStore } from "@/stores/shell";
import { HttpError } from "@digua/http";
import type { KbDocumentDetail, PcObjectDetail } from "@/api/knowledge";

/**
 * 知识对象与溯源页测试（Figma 21/39/40 落地）：
 * - PC 对象：对象卡（名称/版本/生命周期）+ 溯源链（报告跳链接/仓库/commit）。
 * - KB 文档：对象卡 + 修订历史 + 非报告通道标注（不虚构报告跳）。
 * - 404 / 非法 domain / 无项目上下文 → 显式错误态。
 */

const getKbDocument = vi.fn();
const getPcObject = vi.fn();
const getProjcObject = vi.fn();

vi.mock("@/api/knowledge", () => ({
  getKbDocument: (...args: unknown[]) => getKbDocument(...args),
  getPcObject: (...args: unknown[]) => getPcObject(...args),
  getProjcObject: (...args: unknown[]) => getProjcObject(...args)
}));

function pcDetailOf(overrides: Partial<PcObjectDetail> = {}): PcObjectDetail {
  return {
    object_id: "obj-1",
    object_type: "domain_object",
    object_key: "order-status",
    name: "订单状态",
    lifecycle_status: "active",
    latest_revision: {
      revision_no: 3,
      summary: "补充状态机",
      content: "# 订单状态",
      analysis_mode: "derived",
      versioned: true
    },
    analysis_sources: [
      {
        report_id: "rep-1",
        repository_name: "payment-service",
        commit_sha: "abc123def4567890",
        version_no: 3,
        confirmed_at: "2026-11-15T10:00:00Z"
      }
    ],
    ...overrides
  };
}

function kbDetailOf(
  overrides: Partial<KbDocumentDetail> = {}
): KbDocumentDetail {
  return {
    document: {
      document_id: "doc-1",
      doc_key: "auth-design",
      doc_type: "architecture",
      revision_no: 2,
      status: "published",
      content_hash: "hash",
      published_at: "2026-11-10T08:00:00Z",
      confirmed_by: "p-1",
      updated_at: "2026-11-10T08:00:00Z"
    },
    content: "# 权限设计",
    revisions: [
      {
        revision_id: "kbr-2",
        revision_no: 2,
        status: "published",
        content_hash: "hash",
        confirmed_by: "p-1",
        published_at: "2026-11-10T08:00:00Z",
        created_at: "2026-11-10T07:00:00Z"
      }
    ],
    ...overrides
  };
}

async function mountTrace(
  domain: string,
  objectId = "obj-1"
): Promise<ReturnType<typeof mountShell>["wrapper"]> {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/project/knowledge", component: { template: "<div />" } },
      {
        path: "/project/analysis/reports/:reportId",
        component: { template: "<div />" }
      },
      {
        path: "/project/knowledge/objects/:domain/:objectId",
        component: KnowledgeObjectTraceabilityView
      }
    ]
  });
  await router.push(`/project/knowledge/objects/${domain}/${objectId}`);
  await router.isReady();
  const { wrapper } = mountShell(
    KnowledgeObjectTraceabilityView,
    { attachTo: document.body },
    router
  );
  const shell = useShellStore();
  shell.setProjects([{ id: "proj-1", name: "评测项目" }]);
  shell.setCurrentProject("proj-1");
  await flushPromises();
  return wrapper;
}

beforeEach(() => {
  getKbDocument.mockReset();
  getPcObject.mockReset();
  getProjcObject.mockReset();
});

describe("KnowledgeObjectTraceabilityView（溯源页）", () => {
  it("PC 对象：对象卡（名称/版本/生命周期）+ 溯源链报告跳/仓库/commit", async () => {
    getPcObject.mockResolvedValue(pcDetailOf());
    const wrapper = await mountTrace("pc");
    expect(wrapper.text()).toContain("订单状态");
    expect(wrapper.text()).toContain("v3");
    expect(wrapper.text()).toContain("domain_object");
    expect(wrapper.text()).toContain("order-status");
    // 溯源链：报告跳（可点击链接）+ 仓库 + 不可变 commit 快照
    expect(wrapper.text()).toContain("payment-service");
    expect(wrapper.text()).toContain("abc123def456");
    const link = wrapper.find(".trace-link");
    expect(link.exists()).toBe(true);
    expect(link.attributes("href")).toBe("/project/analysis/reports/rep-1");
    expect(wrapper.text()).not.toContain("非报告通道发布");
    expect(getPcObject).toHaveBeenCalledWith("obj-1", "proj-1");
    wrapper.unmount();
  });

  it("PC 对象无来源报告跳：显示非报告通道标注（不虚构）", async () => {
    getPcObject.mockResolvedValue(pcDetailOf({ analysis_sources: [] }));
    const wrapper = await mountTrace("pc");
    expect(wrapper.text()).toContain("非报告通道发布");
    expect(wrapper.find(".trace-link").exists()).toBe(false);
    wrapper.unmount();
  });

  it("KB 文档：对象卡 + 修订历史 + 非报告通道标注", async () => {
    getKbDocument.mockResolvedValue(kbDetailOf());
    const wrapper = await mountTrace("kb", "doc-1");
    expect(wrapper.text()).toContain("auth-design");
    expect(wrapper.text()).toContain("v2");
    expect(wrapper.text()).toContain("已发布");
    expect(wrapper.text()).toContain("非报告通道发布");
    expect(wrapper.text()).toContain("2026-11-10 08:00");
    expect(getKbDocument).toHaveBeenCalledWith("doc-1", "proj-1");
    expect(getPcObject).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it("404（跨项目/不存在）→ 显式错误态", async () => {
    getPcObject.mockRejectedValue(new HttpError("no", { status: 404 }));
    const wrapper = await mountTrace("pc");
    expect(wrapper.text()).toContain("对象不存在或无权访问");
    wrapper.unmount();
  });

  it("非法 domain → 404 语义态且不发请求", async () => {
    const wrapper = await mountTrace("bogus");
    expect(wrapper.text()).toContain("对象不存在或无权访问");
    expect(getPcObject).not.toHaveBeenCalled();
    expect(getKbDocument).not.toHaveBeenCalled();
    wrapper.unmount();
  });
});
