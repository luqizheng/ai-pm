// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import AnalysisReportDetailView from "./AnalysisReportDetailView.vue";
import MarkdownViewer from "@/components/ui/markdown-editor/MarkdownViewer.vue";
import { mountShell } from "@/components/shell/test-utils";
import { useShellStore } from "@/stores/shell";
import { HttpError } from "@digua/http";
import type { AnalysisReportDetail } from "@/api/analysis-reports";

/**
 * 详情页测试：
 * - P2-03：六节渲染（裁决 D-2）/ 404 显式错误态（GAP-6 关闭）/ D-4 综合门 /
 *   关闭按钮显隐与回执。
 * - P2-04：审阅交互（KB 确认 / PC 决策 / 关闭）与关闭语义七条状态驱动显隐、
 *   409/403 错误路径。端点 = 三组既有端点，契约零改动。
 */

const getAnalysisReport = vi.fn();
const closeAnalysisReport = vi.fn();
const synthesizeIdentity = vi.fn();
const confirmKbRevision = vi.fn();
const decidePcProposal = vi.fn();

vi.mock("@/api/analysis-reports", () => ({
  getAnalysisReport: (...args: unknown[]) => getAnalysisReport(...args),
  closeAnalysisReport: (...args: unknown[]) => closeAnalysisReport(...args),
  synthesizeIdentity: (...args: unknown[]) => synthesizeIdentity(...args)
}));

vi.mock("@/api/knowledge", () => ({
  confirmKbRevision: (...args: unknown[]) => confirmKbRevision(...args),
  decidePcProposal: (...args: unknown[]) => decidePcProposal(...args)
}));

function detailOf(
  overrides: Partial<AnalysisReportDetail> = {}
): AnalysisReportDetail {
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
    run_mode: "incremental",
    since_commit: "dead0000beef0000cafe",
    pending_proposals: 2,
    has_pending_kb: true,
    kb_revision: {
      revision_id: "kbr-1",
      document_id: "doc-1",
      doc_key: "analysis-report",
      revision_no: 3,
      status: "draft",
      analysis_mode: null,
      content: "# 分析报告\n\n正文内容。"
    },
    proposals: [
      {
        proposal_id: "prop-1",
        title: "订单状态机重定义",
        status: "submitted",
        expected_version_no: 4,
        member_count: 3,
        created_at: "2026-11-15T08:30:00Z"
      },
      {
        proposal_id: "prop-2",
        title: "会员等级口径",
        status: "approved",
        expected_version_no: 2,
        member_count: 2,
        created_at: "2026-11-15T08:40:00Z"
      }
    ],
    pc_revisions: [
      {
        object_id: "obj-1",
        object_type: "domain_object",
        display_name: "订单状态",
        revision_no: 3,
        change_summary: "补充状态机",
        content: "# 订单状态\n\n待付款 → 已付款。"
      }
    ],
    ...overrides
  };
}

async function mountDetail(reportId = "rep-1") {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/project/analysis", component: { template: "<div />" } },
      {
        path: "/project/analysis/reports/:reportId",
        component: AnalysisReportDetailView
      }
    ]
  });
  await router.push(`/project/analysis/reports/${reportId}`);
  await router.isReady();
  const { wrapper } = mountShell(
    AnalysisReportDetailView,
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
  getAnalysisReport.mockReset();
  closeAnalysisReport.mockReset();
  synthesizeIdentity.mockReset();
  confirmKbRevision.mockReset();
  decidePcProposal.mockReset();
  getAnalysisReport.mockResolvedValue(detailOf());
});

afterEach(() => {
  vi.useRealTimers();
});

describe("AnalysisReportDetailView 六节结构（P2-03）", () => {
  it("六节标题齐备", async () => {
    const { wrapper } = await mountDetail();
    const titles = wrapper.findAll(".an-sec-title").map(n => n.text());
    expect(titles).toEqual([
      "基本信息",
      "分析报告正文",
      "PC 提案（2）",
      "PC 对象修订（1）",
      "D-4 身份综合（AI）",
      "关闭与处置"
    ]);
  });

  it("第 1 节：身份头、状态徽标、run_mode/since_commit、时间组", async () => {
    const { wrapper } = await mountDetail();
    expect(wrapper.text()).toContain("payment-service @ abc123def456");
    expect(wrapper.text()).toContain("待审理");
    expect(wrapper.text()).toContain("增量");
    expect(wrapper.text()).toContain("dead0000beef");
    expect(wrapper.text()).toContain("2026-11-15 08:00");
  });

  it("第 1 节：failed 报告展示 error_code/error_detail", async () => {
    getAnalysisReport.mockResolvedValue(
      detailOf({
        status: "failed",
        error_code: "worker_crash",
        error_detail: "LLM 通道超时"
      })
    );
    const { wrapper } = await mountDetail();
    expect(wrapper.text()).toContain("失败");
    expect(wrapper.text()).toContain("worker_crash");
    expect(wrapper.text()).toContain("LLM 通道超时");
  });

  it("第 2 节：KB 修订正文经 MarkdownViewer 渲染 + revision/status 徽标", async () => {
    const { wrapper } = await mountDetail();
    const viewer = wrapper.findComponent(MarkdownViewer);
    expect(viewer.exists()).toBe(true);
    expect(viewer.props("modelValue")).toContain("正文内容");
    expect(wrapper.text()).toContain("草稿");
    expect(wrapper.text()).toContain("KB 修订 v3");
  });

  it("第 2 节：无 KB 修订时展示占位文案（不虚构）", async () => {
    getAnalysisReport.mockResolvedValue(detailOf({ kb_revision: null }));
    const { wrapper } = await mountDetail();
    expect(wrapper.text()).toContain("本次分析未产出 KB 报告修订");
  });

  it("第 3 节：提案过滤（默认待决 → 已决 → 全部）与 approved 暂记文案", async () => {
    const { wrapper } = await mountDetail();

    // 默认待决：只显示 submitted/draft
    let titles = wrapper.findAll(".an-proposal-title").map(n => n.text());
    expect(titles).toEqual(["订单状态机重定义"]);

    // 已决：只显示 approved
    await wrapper
      .findAll(".an-filter-chip")
      .find(c => c.text() === "已决")!
      .trigger("click");
    titles = wrapper.findAll(".an-proposal-title").map(n => n.text());
    expect(titles).toEqual(["会员等级口径"]);
    expect(wrapper.text()).toContain("已批准·待发布");

    // 全部
    await wrapper
      .findAll(".an-filter-chip")
      .find(c => c.text() === "全部")!
      .trigger("click");
    titles = wrapper.findAll(".an-proposal-title").map(n => n.text());
    expect(titles).toEqual(["订单状态机重定义", "会员等级口径"]);
  });

  it("第 4 节：PC 修订对象名/类型/版本/变更摘要/全文", async () => {
    const { wrapper } = await mountDetail();
    expect(wrapper.text()).toContain("订单状态 · domain_object · v3");
    expect(wrapper.text()).toContain("补充状态机");
    const viewers = wrapper.findAllComponents(MarkdownViewer);
    expect(viewers.length).toBe(2);
    expect(viewers[1].props("modelValue")).toContain("待付款");
  });

  it("第 5 节 D-4 门：analyzed 可综合；非 analyzed 禁用（端点语义不变）", async () => {
    const { wrapper } = await mountDetail();
    const button = wrapper
      .findAll("button")
      .find(b => b.text().includes("综合业务身份"))!;
    expect(button.attributes("disabled")).toBeUndefined();

    getAnalysisReport.mockResolvedValue(detailOf({ status: "closed" }));
    const closed = await mountDetail();
    const closedButton = closed.wrapper
      .findAll("button")
      .find(b => b.text().includes("综合业务身份"))!;
    expect(closedButton.attributes("disabled")).toBeDefined();
  });

  it("D-4 综合动作：调用既有端点 → 三提案产出状态展示 → 详情刷新", async () => {
    synthesizeIdentity.mockResolvedValue({
      profile_proposal_id: "pp-1",
      vision_proposal_id: "pv-1",
      goal_proposal_id: "pg-1",
      goal_proposal_status: "submitted",
      uncertainty_notes: []
    });
    const { wrapper } = await mountDetail();
    expect(getAnalysisReport).toHaveBeenCalledTimes(1);

    const button = wrapper
      .findAll("button")
      .find(b => b.text().includes("综合业务身份"))!;
    await button.trigger("click");
    await flushPromises();

    expect(synthesizeIdentity).toHaveBeenCalledWith("proj-1");
    // 三提案产出状态
    expect(wrapper.text()).toContain("产品画像");
    expect(wrapper.text()).toContain("产品愿景");
    expect(wrapper.text()).toContain("项目目标");
    expect(wrapper.text()).toContain("已起草");
    // 动作后详情重拉（refreshAfterAction 等价）
    expect(getAnalysisReport).toHaveBeenCalledTimes(2);
    expect(wrapper.text()).toContain("三份提案已起草");
  });

  it("第 6 节：仅 analyzed 展示关闭按钮；closed 展示不可关文案", async () => {
    const { wrapper } = await mountDetail();
    expect(
      wrapper.findAll("button").some(b => b.text().includes("关闭报告"))
    ).toBe(true);

    getAnalysisReport.mockResolvedValue(detailOf({ status: "closed" }));
    const closed = await mountDetail();
    expect(
      closed.wrapper.findAll("button").some(b => b.text().includes("关闭报告"))
    ).toBe(false);
    expect(closed.wrapper.text()).toContain("仅待审理报告可关闭");
  });

  it("关闭链路：确认框计数预览 → close 端点 → 回执（applied/withdrawn/rejected/锚点）→ closed 回填", async () => {
    closeAnalysisReport.mockResolvedValue({
      report_id: "rep-1",
      status: "closed",
      applied_proposals: 1,
      publish_failed_proposals: 0,
      withdrawn_proposals: 2,
      rejected_kb: true,
      last_analyzed_commit: "abc123def4567890abcdef"
    });
    const { wrapper } = await mountDetail();

    await wrapper
      .findAll("button")
      .find(b => b.text().includes("关闭报告"))!
      .trigger("click");
    await flushPromises();

    // 计数预览：发布=approved 数（1）、撤回=pending_proposals（2）、拒绝 KB=1
    const dialog = wrapper.findComponent({ name: "ConfirmationDialog" });
    expect(dialog.exists()).toBe(true);
    expect(dialog.props("description")).toContain("发布 1 个已批准提案");
    expect(dialog.props("description")).toContain("撤回 2 个未决提案");
    expect(dialog.props("description")).toContain("拒绝 1 份 KB 草稿");

    // 关闭成功后详情重拉返回 closed 态
    getAnalysisReport.mockResolvedValue(
      detailOf({
        status: "closed",
        closed_at: "2026-11-15T10:00:00Z",
        pending_proposals: 0,
        has_pending_kb: false
      })
    );
    await dialog.vm.$emit("confirm");
    await flushPromises();

    expect(closeAnalysisReport).toHaveBeenCalledWith("rep-1", "proj-1");
    // 回执：applied/withdrawn/rejected/锚点短 sha
    expect(wrapper.text()).toContain("发布 1 个提案");
    expect(wrapper.text()).toContain("撤回 2 个未决提案");
    expect(wrapper.text()).toContain("拒绝 1 份 KB 草稿");
    expect(wrapper.text()).toContain("abc123def456");
    // closed 回填：关闭按钮消失
    expect(
      wrapper.findAll("button").some(b => b.text().includes("关闭报告"))
    ).toBe(false);
  });

  it("404/跨项目 Report ID → 显式错误态（GAP-6 关闭）；其余错误走通用错误态", async () => {
    getAnalysisReport.mockRejectedValue(
      new HttpError("not found", { status: 404 })
    );
    const { wrapper } = await mountDetail();
    expect(wrapper.text()).toContain("报告不存在或无权访问");
    wrapper.unmount();

    getAnalysisReport.mockRejectedValue(new Error("boom"));
    const second = await mountDetail();
    expect(second.wrapper.text()).toContain("加载失败");
    second.wrapper.unmount();
  });

  it("无项目上下文：空态且不请求详情（项目隔离）", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        {
          path: "/project/analysis/reports/:reportId",
          component: AnalysisReportDetailView
        }
      ]
    });
    await router.push("/project/analysis/reports/rep-1");
    await router.isReady();
    const { wrapper } = mountShell(
      AnalysisReportDetailView,
      { attachTo: document.body },
      router
    );
    const shell = useShellStore();
    shell.setProjects([{ id: "proj-1", name: "评测项目" }]);
    await flushPromises();

    expect(getAnalysisReport).not.toHaveBeenCalled();
    expect(wrapper.text()).toContain("请先选择项目");
    wrapper.unmount();
  });

  it("每次进入详情重新拉取（缓存陈旧原则）：reportId 变化即重拉", async () => {
    const { router, wrapper } = await mountDetail();
    expect(getAnalysisReport).toHaveBeenCalledTimes(1);

    await router.push("/project/analysis/reports/rep-2");
    await flushPromises();
    expect(getAnalysisReport).toHaveBeenCalledTimes(2);
    expect(getAnalysisReport).toHaveBeenLastCalledWith("rep-2", "proj-1");
    wrapper.unmount();
  });
});

describe("审阅交互与关闭语义七条（P2-04）", () => {
  /** 复用：第 3 节中取指定提案的操作按钮集合。 */
  function buttonsOf(wrapper: Awaited<ReturnType<typeof mountDetail>>["wrapper"]) {
    return wrapper.findAll(".an-proposal").map(card => ({
      card,
      buttons: card.findAll("button").map(b => b.text())
    }));
  }

  it("七条#1：approved 显示「已批准·待发布」暂记语义（批准≠正式发布）", async () => {
    const { wrapper } = await mountDetail();
    // approved 属「已决」，默认待决视图不显示——切到全部
    await wrapper
      .findAll(".an-filter-chip")
      .find(c => c.text() === "全部")!
      .trigger("click");
    expect(wrapper.text()).toContain("已批准·待发布");
    wrapper.unmount();
  });

  it("七条#2：开放期批准/驳回可互转——按钮显隐按现条件原样保留", async () => {
    const { wrapper } = await mountDetail();
    // 切到全部，覆盖 submitted 与 approved 两种状态
    await wrapper
      .findAll(".an-filter-chip")
      .find(c => c.text() === "全部")!
      .trigger("click");
    const cards = buttonsOf(wrapper);
    // submitted：批准 + 驳回
    expect(cards[0].buttons).toContain("批准");
    expect(cards[0].buttons).toContain("驳回");
    // approved：只剩驳回（可再驳回 = 互转开放）
    expect(cards[1].buttons).not.toContain("批准");
    expect(cards[1].buttons).toContain("驳回");

    // rejected：只剩批准（可再批准 = 互转开放）
    getAnalysisReport.mockResolvedValue(
      detailOf({
        proposals: [
          {
            proposal_id: "prop-3",
            title: "已驳回提案",
            status: "rejected",
            expected_version_no: 5,
            member_count: 1,
            created_at: "2026-11-15T08:50:00Z"
          }
        ]
      })
    );
    const rejected = await mountDetail();
    await rejected.wrapper
      .findAll(".an-filter-chip")
      .find(c => c.text() === "全部")!
      .trigger("click");
    const rejectedCards = buttonsOf(rejected.wrapper);
    expect(rejectedCards[0].buttons).toContain("批准");
    expect(rejectedCards[0].buttons).not.toContain("驳回");
  });

  it("七条#3：报告 closed 后决策按钮冻结不渲染（后端 409 frozen 的前端面）", async () => {
    getAnalysisReport.mockResolvedValue(detailOf({ status: "closed" }));
    const { wrapper } = await mountDetail();
    for (const card of buttonsOf(wrapper)) {
      expect(card.buttons).not.toContain("批准");
      expect(card.buttons).not.toContain("驳回");
    }
    wrapper.unmount();
  });

  it("KB 确认：draft + analyzed 渲染按钮；published 或 closed 不渲染", async () => {
    const { wrapper } = await mountDetail();
    expect(
      wrapper.findAll("button").some(b => b.text().includes("确认发布"))
    ).toBe(true);
    wrapper.unmount();

    getAnalysisReport.mockResolvedValue(
      detailOf({
        kb_revision: detailOf().kb_revision && {
          ...detailOf().kb_revision!,
          status: "published"
        }
      })
    );
    const published = await mountDetail();
    expect(
      published.wrapper
        .findAll("button")
        .some(b => b.text().includes("确认发布"))
    ).toBe(false);
    published.wrapper.unmount();

    getAnalysisReport.mockResolvedValue(detailOf({ status: "closed" }));
    const closed = await mountDetail();
    expect(
      closed.wrapper.findAll("button").some(b => b.text().includes("确认发布"))
    ).toBe(false);
    closed.wrapper.unmount();
  });

  it("KB 确认动作：调用既有 confirm 端点 → 回执 → 详情重拉", async () => {
    confirmKbRevision.mockResolvedValue({});
    const { wrapper } = await mountDetail();

    await wrapper
      .findAll("button")
      .find(b => b.text().includes("确认发布"))!
      .trigger("click");
    await flushPromises();

    expect(confirmKbRevision).toHaveBeenCalledTimes(1);
    expect(confirmKbRevision).toHaveBeenCalledWith(
      "doc-1",
      "kbr-1",
      "proj-1"
    );
    expect(wrapper.text()).toContain("已确认发布");
    expect(getAnalysisReport).toHaveBeenCalledTimes(2);
    wrapper.unmount();
  });

  it("PC 批准动作：CAS expected_version_no 随请求 → 暂记回执 → 详情重拉", async () => {
    decidePcProposal.mockResolvedValue({});
    const { wrapper } = await mountDetail();

    const approveButton = wrapper
      .findAll(".an-proposal")[0]
      .findAll("button")
      .find(b => b.text() === "批准")!;
    await approveButton.trigger("click");
    await flushPromises();

    expect(decidePcProposal).toHaveBeenCalledTimes(1);
    expect(decidePcProposal).toHaveBeenCalledWith(
      "prop-1",
      "proj-1",
      { action: "approve", expected_version_no: 4 }
    );
    expect(wrapper.text()).toContain("已批准（暂记");
    expect(getAnalysisReport).toHaveBeenCalledTimes(2);
    wrapper.unmount();
  });

  it("权限负向（AC-11/E2E-11）：403 显式呈现、不重试不吞错（KB 确认与 PC 决策）", async () => {
    confirmKbRevision.mockRejectedValue(
      new HttpError("forbidden", { status: 403 })
    );
    const { wrapper } = await mountDetail();
    await wrapper
      .findAll("button")
      .find(b => b.text().includes("确认发布"))!
      .trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("无权限");
    expect(confirmKbRevision).toHaveBeenCalledTimes(1);
    wrapper.unmount();

    decidePcProposal.mockRejectedValue(
      new HttpError("forbidden", { status: 403 })
    );
    const second = await mountDetail();
    await second.wrapper
      .findAll(".an-proposal")[0]
      .findAll("button")
      .find(b => b.text() === "批准")!
      .trigger("click");
    await flushPromises();
    expect(second.wrapper.text()).toContain("无权限");
    expect(decidePcProposal).toHaveBeenCalledTimes(1);
    second.wrapper.unmount();
  });

  it("CAS 409（E2E-08 单测级）：过期决定被拒，错误文案可见", async () => {
    decidePcProposal.mockRejectedValue(
      new HttpError("version_conflict", { status: 409 })
    );
    const { wrapper } = await mountDetail();
    await wrapper
      .findAll(".an-proposal")[0]
      .findAll("button")
      .find(b => b.text() === "批准")!
      .trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("决定失败");
    expect(wrapper.text()).toContain("并发冲突");
    wrapper.unmount();
  });

  it("七条#7：close 409 错误路径 UI 可见（仅 analyzed 渲染关闭入口）", async () => {
    closeAnalysisReport.mockRejectedValue(
      new HttpError("conflict", { status: 409 })
    );
    const { wrapper } = await mountDetail();
    await wrapper
      .findAll("button")
      .find(b => b.text().includes("关闭报告"))!
      .trigger("click");
    await flushPromises();
    const dialog = wrapper.findComponent({ name: "ConfirmationDialog" });
    await dialog.vm.$emit("confirm");
    await flushPromises();

    expect(closeAnalysisReport).toHaveBeenCalledTimes(1);
    expect(wrapper.text()).toContain("关闭失败");
    wrapper.unmount();
  });
});
