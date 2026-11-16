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
  PcObjectDetail,
  PcObjectRow,
  PcProposalDetail,
  PcProposalRow
} from "@/api/knowledge";

const listKbDocuments = vi.fn();
const getKbDocument = vi.fn();
const confirmKbRevision = vi.fn();
const listPcObjects = vi.fn();
const getPcObject = vi.fn();
const listPcProposals = vi.fn();
const getPcProposal = vi.fn();
const decidePcProposal = vi.fn();
const retrievalQuery = vi.fn();
const ingestKbDocument = vi.fn();

vi.mock("@/api/knowledge", () => ({
  listKbDocuments: (...args: unknown[]) => listKbDocuments(...args),
  getKbDocument: (...args: unknown[]) => getKbDocument(...args),
  confirmKbRevision: (...args: unknown[]) => confirmKbRevision(...args),
  listPcObjects: (...args: unknown[]) => listPcObjects(...args),
  getPcObject: (...args: unknown[]) => getPcObject(...args),
  listPcProposals: (...args: unknown[]) => listPcProposals(...args),
  getPcProposal: (...args: unknown[]) => getPcProposal(...args),
  decidePcProposal: (...args: unknown[]) => decidePcProposal(...args),
  retrievalQuery: (...args: unknown[]) => retrievalQuery(...args),
  ingestKbDocument: (...args: unknown[]) => ingestKbDocument(...args)
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
  // P2-06（裁决 D-1）起默认页签即 KB，存量断言直接以 KB 视野成立。
  return wrapper;
}

function proposalRow(overrides: Partial<PcProposalRow>): PcProposalRow {
  return {
    proposal_id: "prop-1",
    title: "订单状态机重定义",
    status: "submitted",
    risk_level: "medium",
    confidence: "high",
    origin_kind: "ai_agent",
    expected_version_no: 4,
    member_count: 3,
    submitted_at: "2026-11-10T06:00:00Z",
    created_at: "2026-11-10T05:00:00Z",
    ...overrides
  };
}

function pcDetailOf(row: PcObjectRow): PcObjectDetail {
  return {
    object_id: row.object_id,
    object_type: row.object_type,
    object_key: row.object_key,
    name: row.display_name,
    lifecycle_status: row.lifecycle_status,
    latest_revision: {
      revision_no: 3,
      summary: "补充状态机",
      content: "# 订单状态\n\n完整状态机正文：待付款 → 已付款 → 已发货。",
      analysis_mode: "derived",
      versioned: false
    }
  };
}

function proposalDetailOf(row: PcProposalRow): PcProposalDetail {
  return {
    proposal_id: row.proposal_id,
    title: row.title,
    rationale: "状态机口径需与支付域对齐。",
    status: row.status,
    origin_kind: row.origin_kind,
    governance_level: "L1",
    risk_level: row.risk_level,
    confidence: row.confidence,
    expected_version_no: row.expected_version_no,
    submitted_at: row.submitted_at,
    created_at: row.created_at,
    updated_at: row.created_at,
    members: [
      {
        change_kind: "update_object",
        object_id: "obj-1",
        object_revision_id: "rev-3",
        object_type: "domain_object",
        object_key: "order-status",
        display_name: "订单状态",
        object_revision_no: 3,
        change_summary: "补充状态机",
        relationship_id: null,
        relationship_revision_id: null,
        relationship_type: null,
        relationship_revision_no: null
      }
    ],
    approvals: [
      {
        approval_id: "apr-1",
        status: "pending",
        proposal_content_hash: "hash",
        expected_version_no: row.expected_version_no,
        decided_by: null,
        decided_by_name: null,
        decision_reason: null,
        decided_at: null,
        created_at: row.created_at
      }
    ],
    content_hash: "hash",
    published_version_no: null
  };
}

const listKbBase: KbDocumentRow[] = [
  kbRow({}),
  kbRow({
    document_id: "doc-2",
    doc_key: "billing-rule",
    doc_type: "knowledge",
    revision_no: 1,
    status: "draft",
    published_at: null
  })
];

beforeEach(() => {
  vi.clearAllMocks();
  listKbDocuments.mockResolvedValue({ items: listKbBase });
  listPcObjects.mockResolvedValue({ items: [pcRow({})] });
  listPcProposals.mockResolvedValue({
    items: [
      proposalRow({}),
      proposalRow({
        proposal_id: "prop-2",
        title: "账期规则修订",
        status: "approved",
        expected_version_no: 2
      })
    ]
  });
  getKbDocument.mockImplementation((_id: string, _pid: string) =>
    Promise.resolve(detailOf(kbRow({})))
  );
  getPcObject.mockImplementation((id: string) =>
    Promise.resolve(pcDetailOf(pcRow({ object_id: id })))
  );
  getPcProposal.mockImplementation((id: string) =>
    Promise.resolve(proposalDetailOf(proposalRow({ proposal_id: id })))
  );
  confirmKbRevision.mockResolvedValue({});
  decidePcProposal.mockResolvedValue({});
  ingestKbDocument.mockResolvedValue({
    document_id: "doc-ingested",
    revision_id: "rev-ing",
    revision_no: 1,
    status: "draft"
  });
  // 摄入后刷新：第二次调用返回含新文档的列表（runIngest 里按 id 找行打开详情）。
  listKbDocuments.mockImplementation(() =>
    Promise.resolve({
      items: [
        ...listKbBase,
        kbRow({
          document_id: "doc-ingested",
          doc_key: "fresh-doc",
          status: "draft",
          revision_no: 1,
          published_at: null
        })
      ]
    })
  );
  retrievalQuery.mockResolvedValue({
    plan_source: "deterministic_fallback",
    degraded: true,
    degradation_reason: "planner_disabled",
    validation: { checks: [] },
    evidence: {
      evidence: [],
      traversal: {
        edges: [
          {
            relationship_id: "r1",
            relationship_type: "module-covered-by-design",
            source_id: "pc:obj-1",
            target_id: "knowledge_base:doc-9",
            traversal_direction: "forward"
          }
        ],
        gaps: []
      },
      gaps: [],
      degradations: [],
      budget_report: {}
    }
  });
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
    // 选择器 scoped 到详情动作区——影响面板的「分析」按钮也是 .kb-confirm。
    const confirmBtn = wrapper.find(".kb-detail-actions .kb-confirm");
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
    const tabs = wrapper.findAll('[role="tab"]');
    // reka TabsTrigger 激活事件是 mousedown（非 click）。
    await tabs[1].trigger("mousedown");
    await flushPromises();
    expect(wrapper.text()).toContain("暂无 PC 认知对象");
  });

  it("switches to PC tab and shows lifecycle badge", async () => {
    const wrapper = await mountView();
    const tabs = wrapper.findAll('[role="tab"]');
    await tabs[1].trigger("mousedown");
    await flushPromises();
    expect(wrapper.text()).toContain("现行");
    expect(wrapper.text()).toContain("补充状态机");
  });

  it("opens detail via ?doc= deep link", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/project/knowledge", component: KnowledgeView }]
    });
    await router.push("/project/knowledge?doc=doc-1");
    await router.isReady();
    const { wrapper } = mountShell(
      KnowledgeView,
      { attachTo: document.body },
      router
    );
    const shell = useShellStore();
    shell.setProjects([{ id: "proj-1", name: "评测项目" }]);
    shell.setCurrentProject("proj-1");
    await flushPromises();

    expect(getKbDocument).toHaveBeenCalledWith("doc-1", "proj-1");
    expect(wrapper.find(".kb-detail").exists()).toBe(true);
  });

  it("ingest panel: paste markdown creates a draft and opens its detail", async () => {
    const wrapper = await mountView();
    await wrapper.find(".impact-panel summary").trigger("click");
    // 摄入面板是 KB 页签下第二个 details。
    const panels = wrapper.findAll("details.impact-panel");
    await panels[1].find("summary").trigger("click");
    await panels[1].find(".impact-input input").setValue("fresh-doc");
    await panels[1].find("textarea").setValue("# 新文档\n\n粘贴正文。");
    await panels[1].find(".ingest-run").trigger("click");
    await flushPromises();

    expect(ingestKbDocument).toHaveBeenCalledTimes(1);
    const [pid, body] = ingestKbDocument.mock.calls[0];
    expect(pid).toBe("proj-1");
    expect(body.doc_key).toBe("fresh-doc");
    expect(body.kind).toBe("markdown");
    // base64 解码回 UTF-8 原文（atob 产出 latin1 化字节串，须按字节还原）。
    const decoded = new TextDecoder().decode(
      Uint8Array.from(atob(body.content_b64), c => c.charCodeAt(0))
    );
    expect(decoded).toContain("粘贴正文");
    // 成功后刷新列表 + 打开详情。
    expect(getKbDocument).toHaveBeenCalled();
    expect(wrapper.text()).toContain("已摄入为草稿");
  });

  it("ingest panel validates empty input", async () => {
    const wrapper = await mountView();
    const panels = wrapper.findAll("details.impact-panel");
    await panels[1].find("summary").trigger("click");
    await panels[1].find(".ingest-run").trigger("click");
    await flushPromises();
    expect(ingestKbDocument).not.toHaveBeenCalled();
    expect(wrapper.text()).toContain("请填写文档标识");
  });

  it("impact panel renders traversal path and node labels", async () => {
    const wrapper = await mountView();
    // 展开影响分析面板并提交查询。
    await wrapper.find(".impact-panel summary").trigger("click");
    await wrapper
      .find(".impact-input input")
      .setValue("修改订单状态会影响什么");
    await wrapper.find(".impact-form").trigger("submit");
    await flushPromises();

    expect(retrievalQuery).toHaveBeenCalledWith(
      "修改订单状态会影响什么",
      "proj-1"
    );
    // 路径链：pc 节点映射 display_name（来自已加载 PC 对象），反向箭头语义。
    const path = wrapper.find(".impact-path");
    expect(path.exists()).toBe(true);
    expect(path.text()).toContain("订单状态");
    expect(path.text()).toContain("module-covered-by-design");
    // 降级提示（planner off）。
    expect(wrapper.text()).toContain("规划器未介入");
  });

  it("expands PC object detail with full content and version badge", async () => {
    const wrapper = await mountView();
    const tabs = wrapper.findAll('[role="tab"]');
    await tabs[1].trigger("mousedown");
    await flushPromises();
    // 收起态仍显示摘要与 excerpt（240 字截断）。
    expect(wrapper.text()).toContain("补充状态机");

    await wrapper.find(".pc-row-head").trigger("click");
    await flushPromises();

    expect(getPcObject).toHaveBeenCalledWith("obj-1", "proj-1");
    // 详情形状：修订号 + versioned 徽章 + 分析模式 + 全文（非 excerpt 截断）。
    expect(wrapper.text()).toContain("未入正式版本");
    expect(wrapper.text()).toContain("分析模式: derived");
    const viewer = wrapper.findComponent(MarkdownViewer);
    expect(viewer.exists()).toBe(true);
    expect(viewer.props("modelValue")).toContain("完整状态机正文");

    // 收起保留缓存：再展开不重拉。
    await wrapper.find(".pc-row-head").trigger("click");
    await flushPromises();
    await wrapper.find(".pc-row-head").trigger("click");
    await flushPromises();
    expect(getPcObject).toHaveBeenCalledTimes(1);
  });

  it("shows no-revision hint for identity PC objects", async () => {
    listPcObjects.mockResolvedValue({
      items: [
        pcRow({
          object_id: "obj-id",
          object_type: "product_profile",
          object_key: null,
          display_name: "ai-pm",
          lifecycle_status: "identity",
          revision_no: null,
          change_summary: null,
          excerpt: null
        })
      ]
    });
    getPcObject.mockImplementation((id: string) =>
      Promise.resolve({
        ...pcDetailOf(pcRow({ object_id: id })),
        latest_revision: null
      })
    );
    const wrapper = await mountView();
    const tabs = wrapper.findAll('[role="tab"]');
    await tabs[1].trigger("mousedown");
    await flushPromises();
    await wrapper.find(".pc-row-head").trigger("click");
    await flushPromises();

    expect(getPcObject).toHaveBeenCalledWith("obj-id", "proj-1");
    expect(wrapper.text()).toContain("暂无修订");
  });

  it("默认页签 = KB，reports 页签不存在（裁决 D-1，P2-06）", async () => {
    const wrapper = await mountView();
    const tabTexts = wrapper.findAll('[role="tab"]').map(tab => tab.text());
    // 页签构成 = kb | pc；reports 迁出（E2E-05 单测级：默认页签=kb）。
    expect(tabTexts.length).toBe(2);
    expect(tabTexts[0]).toContain("KB 文档");
    expect(tabTexts[1]).toContain("PC 对象");
    const joined = tabTexts.join("|");
    expect(joined).not.toContain("分析报告");
    // 默认渲染 KB 内容。
    expect(wrapper.text()).toContain("auth-design");
  });

  it("?report= 兼容跳转 Analysis 详情页（保留 project_id，不丢参）", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/project/knowledge", component: KnowledgeView }]
    });
    await router.push("/project/knowledge?project_id=proj-1&report=rpt-9");
    await router.isReady();
    const { wrapper } = mountShell(
      KnowledgeView,
      { attachTo: document.body },
      router
    );
    const shell = useShellStore();
    shell.setProjects([{ id: "proj-1", name: "评测项目" }]);
    shell.setCurrentProject("proj-1");
    const replaceSpy = vi
      .spyOn(router, "replace")
      .mockResolvedValue(undefined as never);
    await flushPromises();

    // §6 定版表：report 升级为路径参数，其余 query（project_id）保留——
    // 不是丢参 redirect（E2E-04 单测级）。非法/跨项目 ID 的显式错误态
    // 由详情页 GAP-6 呈现，本页不校验不静默拦截。
    expect(replaceSpy).toHaveBeenCalledTimes(1);
    expect(replaceSpy).toHaveBeenCalledWith({
      path: "/project/analysis/reports/rpt-9",
      query: { project_id: "proj-1" }
    });
    wrapper.unmount();
  });

  it("?report= 与 ?doc= 同现：report 分支先行跳离，KB 详情不被应用", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/project/knowledge", component: KnowledgeView }]
    });
    await router.push(
      "/project/knowledge?project_id=proj-1&report=rpt-9&doc=doc-1"
    );
    await router.isReady();
    const { wrapper } = mountShell(
      KnowledgeView,
      { attachTo: document.body },
      router
    );
    const shell = useShellStore();
    shell.setProjects([{ id: "proj-1", name: "评测项目" }]);
    shell.setCurrentProject("proj-1");
    const replaceSpy = vi
      .spyOn(router, "replace")
      .mockResolvedValue(undefined as never);
    await flushPromises();

    // report 分支先行跳离（组合自然消解，方案 §6；不自行猜测优先级排序语义）
    expect(replaceSpy).toHaveBeenCalledWith({
      path: "/project/analysis/reports/rpt-9",
      query: { project_id: "proj-1", doc: "doc-1" }
    });
    // KB 文档详情分支未执行（report 分支 return）
    expect(getKbDocument).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it("opens PC object detail via ?object= deep link", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/project/knowledge", component: KnowledgeView }]
    });
    await router.push("/project/knowledge?object=obj-1");
    await router.isReady();
    const { wrapper } = mountShell(
      KnowledgeView,
      { attachTo: document.body },
      router
    );
    const shell = useShellStore();
    shell.setProjects([{ id: "proj-1", name: "评测项目" }]);
    shell.setCurrentProject("proj-1");
    await flushPromises();

    expect(getPcObject).toHaveBeenCalledWith("obj-1", "proj-1");
    expect(wrapper.find(".kb-detail").exists()).toBe(true);
  });
});
