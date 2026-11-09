// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import KnowledgeView from "./KnowledgeView.vue";
import MarkdownViewer from "@/components/ui/markdown-editor/MarkdownViewer.vue";
import { mountShell } from "@/components/shell/test-utils";
import { useShellStore } from "@/stores/shell";
import type {
  KbDocumentDetail,
  KbDocumentRow,
  PcObjectRow
} from "@/api/knowledge";

const listKbDocuments = vi.fn();
const getKbDocument = vi.fn();
const confirmKbRevision = vi.fn();
const listPcObjects = vi.fn();

vi.mock("@/api/knowledge", () => ({
  listKbDocuments: (...args: unknown[]) => listKbDocuments(...args),
  getKbDocument: (...args: unknown[]) => getKbDocument(...args),
  confirmKbRevision: (...args: unknown[]) => confirmKbRevision(...args),
  listPcObjects: (...args: unknown[]) => listPcObjects(...args)
}));

function kbRow(overrides: Partial<KbDocumentRow>): KbDocumentRow {
  return {
    document_id: "doc-1",
    doc_key: "auth-design",
    doc_type: "architecture",
    revision_no: 2,
    status: "published",
    content_hash: "hash",
    published_at: "2026-11-10T08:00:00Z",
    confirmed_by: "p-1",
    updated_at: "2026-11-10T08:00:00Z",
    ...overrides
  };
}

function pcRow(overrides: Partial<PcObjectRow>): PcObjectRow {
  return {
    object_id: "obj-1",
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

function detailOf(row: KbDocumentRow): KbDocumentDetail {
  return {
    document: row,
    content: "# 权限设计\n\n正文内容。",
    revisions: [
      {
        revision_id: "rev-2",
        revision_no: 2,
        status: row.status,
        content_hash: "hash",
        confirmed_by: "p-1",
        published_at: "2026-11-10T08:00:00Z",
        created_at: "2026-11-10T08:00:00Z"
      },
      {
        revision_id: "rev-3",
        revision_no: 3,
        status: "draft",
        content_hash: "hash3",
        confirmed_by: null,
        published_at: null,
        created_at: "2026-11-10T09:00:00Z"
      }
    ]
  };
}

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: "/project/knowledge", component: KnowledgeView }]
  });
  await router.push("/project/knowledge");
  await router.isReady();
  const { wrapper } = mountShell(
    KnowledgeView,
    { attachTo: document.body },
    router
  );
  // 项目上下文（视图经 shell store 取 currentProjectId）。
  const shell = useShellStore();
  shell.setProjects([{ id: "proj-1", name: "评测项目" }]);
  shell.setCurrentProject("proj-1");
  await flushPromises();
  return wrapper;
}

beforeEach(() => {
  vi.clearAllMocks();
  listKbDocuments.mockResolvedValue({
    items: [
      kbRow({}),
      kbRow({
        document_id: "doc-2",
        doc_key: "billing-rule",
        doc_type: "knowledge",
        revision_no: 1,
        status: "draft",
        published_at: null
      })
    ]
  });
  listPcObjects.mockResolvedValue({ items: [pcRow({})] });
  getKbDocument.mockImplementation((_id: string, _pid: string) =>
    Promise.resolve(detailOf(kbRow({})))
  );
  confirmKbRevision.mockResolvedValue({});
});

describe("KnowledgeView", () => {
  it("loads both KB documents and PC objects", async () => {
    const wrapper = await mountView();
    expect(listKbDocuments).toHaveBeenCalledWith("proj-1");
    expect(listPcObjects).toHaveBeenCalledWith("proj-1");
    expect(wrapper.text()).toContain("auth-design");
    // PC 数据已加载（页签计数），内容在切页签后进 DOM。
    expect(wrapper.text()).toContain("PC 对象 (1)");
  });

  it("shows status badges for draft and published", async () => {
    const wrapper = await mountView();
    const text = wrapper.text();
    expect(text).toContain("已发布");
    expect(text).toContain("草稿");
  });

  it("opens detail with revisions and content, then confirms the draft", async () => {
    const wrapper = await mountView();
    const head = wrapper.find(".kb-row-head");
    await head.trigger("click");
    await flushPromises();

    expect(getKbDocument).toHaveBeenCalledWith("doc-1", "proj-1");
    expect(wrapper.text()).toContain("修订历史");
    // 正文经 MarkdownViewer 渲染（组件挂载 + 源内容传递）。
    const viewer = wrapper.findComponent(MarkdownViewer);
    expect(viewer.exists()).toBe(true);
    expect(viewer.props("modelValue")).toContain("正文内容");

    // 草稿确认按钮存在并可触发（后端门负责权限，前端只是入口）。
    const confirmBtn = wrapper.find(".kb-confirm");
    expect(confirmBtn.exists()).toBe(true);
    await confirmBtn.trigger("click");
    await flushPromises();
    expect(confirmKbRevision).toHaveBeenCalledWith("doc-1", "rev-3", "proj-1");
    // 确认后刷新列表。
    expect(listKbDocuments).toHaveBeenCalledTimes(2);
    expect(wrapper.text()).toContain("已确认发布");
  });

  it("renders empty hints when both domains have no rows", async () => {
    listKbDocuments.mockResolvedValue({ items: [] });
    listPcObjects.mockResolvedValue({ items: [] });
    const wrapper = await mountView();
    expect(wrapper.text()).toContain("暂无 KB 文档");
    // 切 PC 页签。
    const tabs = wrapper.findAll(".knowledge-tab");
    await tabs[1].trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("暂无 PC 认知对象");
  });

  it("switches to PC tab and shows lifecycle badge", async () => {
    const wrapper = await mountView();
    const tabs = wrapper.findAll(".knowledge-tab");
    await tabs[1].trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("现行");
    expect(wrapper.text()).toContain("补充状态机");
  });
});
