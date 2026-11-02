// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import DeliveryPage from "./DeliveryPage.vue";
import { mountShell } from "../../shell/test-utils";
import {
  acceptanceDetails,
  acceptanceOverview,
  type AcceptanceDetailFixture
} from "../fixtures";

/**
 * 交付验收工作台（T-070 §6.5）：走真后端分支 + api 替身，验证
 *   - 三指标与徽标互斥渲染、「待补充」任务不被过滤（Q1 回归）
 *   - 详情区按证据分段渲染且全部来自后端字段（前端无资格判定）
 *   - 退回无理由被拦截；通过后原位刷新
 *   - 动作区可用性只由 review_status / pending_approval_id 驱动
 */

const PROJECT = "11111111-2222-4333-8444-555555555555";

vi.mock("vue-router", () => ({
  useRoute: () => ({
    path: `/workspace/${PROJECT}/delivery`,
    params: { projectId: PROJECT },
    query: {}
  }),
  useRouter: () => ({ push: vi.fn() })
}));

vi.mock("@/api/pm", () => ({
  listProjects: async () => ({
    items: [{ id: PROJECT, name: "World Map", code: "WM", status: "active" }],
    total: 1,
    page: 1,
    size: 200
  })
}));

const getAcceptanceOverview = vi.fn();
const listTaskDelivery = vi.fn();
vi.mock("@/api/acceptance", () => ({
  getAcceptanceOverview: (...args: unknown[]) => getAcceptanceOverview(...args),
  listTaskDelivery: (...args: unknown[]) => listTaskDelivery(...args)
}));

const listReviewFindings = vi.fn();
const listCriterionVerdicts = vi.fn();
const listTaskReviewApprovals = vi.fn();
const decideReviewApproval = vi.fn();
vi.mock("@/api/review", () => ({
  listReviewFindings: (...args: unknown[]) => listReviewFindings(...args),
  listCriterionVerdicts: (...args: unknown[]) => listCriterionVerdicts(...args),
  listTaskReviewApprovals: (...args: unknown[]) =>
    listTaskReviewApprovals(...args),
  decideReviewApproval: (...args: unknown[]) => decideReviewApproval(...args)
}));

const listTaskRuns = vi.fn();
vi.mock("@/api/execution", () => ({
  listTaskRuns: (...args: unknown[]) => listTaskRuns(...args),
  getTaskRun: vi.fn()
}));

const emptyDetail: AcceptanceDetailFixture = {
  delivery: [],
  findings: [],
  verdicts: [],
  approvals: [],
  runs: []
};

function detailFor(taskId: string): AcceptanceDetailFixture {
  return acceptanceDetails[taskId] ?? emptyDetail;
}

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  getAcceptanceOverview.mockResolvedValue(structuredClone(acceptanceOverview));
  listTaskDelivery.mockImplementation(async (taskId: string) => ({
    task_id: taskId,
    project_id: PROJECT,
    items: detailFor(taskId).delivery,
    total: detailFor(taskId).delivery.length
  }));
  listReviewFindings.mockImplementation(async (taskId: string) => ({
    items: detailFor(taskId).findings,
    total: detailFor(taskId).findings.length
  }));
  listCriterionVerdicts.mockImplementation(async (taskId: string) => ({
    items: detailFor(taskId).verdicts,
    total: detailFor(taskId).verdicts.length
  }));
  listTaskReviewApprovals.mockImplementation(async (taskId: string) => ({
    items: detailFor(taskId).approvals,
    total: detailFor(taskId).approvals.length
  }));
  listTaskRuns.mockImplementation(async (params: { task_id: string }) => ({
    items: detailFor(params.task_id).runs,
    total: detailFor(params.task_id).runs.length,
    page: 1,
    size: 50
  }));
  decideReviewApproval.mockResolvedValue({
    outcome: "changes_requested",
    task_id: "task-forest-v02"
  });
});

async function mountPage() {
  const w = mountShell(DeliveryPage);
  await flushPromises();
  await flushPromises();
  return w;
}

describe("DeliveryPage（交付验收工作台）", () => {
  it("三指标 + 总表：徽标互斥、按后端顺序渲染、「待补充」零交付任务不被过滤（Q1）", async () => {
    const w = await mountPage();

    expect(getAcceptanceOverview).toHaveBeenCalledWith(PROJECT);
    const metrics = w.find("[data-test='delivery-metrics']").text();
    expect(metrics).toContain("5");
    expect(metrics).toContain("待验收");
    expect(metrics).toContain("可验收");
    expect(metrics).toContain("存在阻塞");

    const tasks = w.findAll("[data-test='delivery-task']");
    expect(tasks).toHaveLength(acceptanceOverview.tasks.length);
    // 每个任务恰好一个徽标（互斥）；标签来自后端 badge 字段，顺序即后端排序。
    const badges = tasks.map(task => {
      const found = task.findAll(".ws-task-head .ws-badge");
      expect(found).toHaveLength(1);
      return found[0].text();
    });
    expect(badges).toEqual([
      "存在阻塞",
      "存在阻塞",
      "待补充",
      "待补充",
      "可验收"
    ]);
    // Q1：无仓无 run 的 Submitted 任务仍在总表。
    expect(w.text()).toContain("生物群系规格起草");
    const bare = tasks.find(task => task.text().includes("生物群系规格起草"));
    expect(bare?.text()).toContain("交付 0/0 仓");
  });

  it("点指标数字过滤列表，再点待验收恢复全量", async () => {
    const w = await mountPage();
    await w.find("[data-test='metric-blocked']").trigger("click");
    expect(w.findAll("[data-test='delivery-task']")).toHaveLength(2);
    await w.find("[data-test='metric-eligible']").trigger("click");
    expect(w.findAll("[data-test='delivery-task']")).toHaveLength(1);
    expect(w.find("[data-test='delivery-task']").text()).toContain(
      "地形渲染架构评审"
    );
    await w.find("[data-test='metric-all']").trigger("click");
    expect(w.findAll("[data-test='delivery-task']")).toHaveLength(5);
  });

  it("详情区按证据分段渲染，徽标/资格横幅/动作区全部消费后端字段", async () => {
    const w = await mountPage();
    // 默认选中第一行（forest：blocked + awaiting_decision）。
    const banner = w.find("[data-test='delivery-banner']");
    expect(banner.classes()).toContain("is-blocked");
    expect(banner.text()).toContain("暂不可验收");
    expect(banner.text()).toContain("等待验收决定");
    expect(banner.text()).toContain("集成通过");

    expect(w.findAll("[data-test='delivery-repo']")).toHaveLength(2);
    const findings = w.findAll("[data-test='delivery-finding']");
    expect(findings).toHaveLength(2);
    expect(findings[0].text()).toContain("阻塞");
    expect(findings[0].text()).toContain("worldmap-gen/src/forest.rs:88-102");
    const verdicts = w.findAll("[data-test='delivery-verdict']");
    expect(verdicts).toHaveLength(2);
    expect(verdicts[0].text()).toContain("失败");
    expect(verdicts[1].text()).toContain("未被审查覆盖");
    expect(w.findAll("[data-test='delivery-run']")).toHaveLength(2);
    expect(w.findAll("[data-test='delivery-approval']")).toHaveLength(1);
    expect(w.find("[data-test='delivery-approval']").text()).toContain(
      "待决定"
    );
    // awaiting_decision + pending_approval_id → 按钮可点。
    expect(w.find("[data-test='delivery-approve']").exists()).toBe(true);
    expect(w.find("[data-test='delivery-reject']").exists()).toBe(true);

    // 切到部分交付任务：缺仓红标「未交付」来自 commit_sha 为空的清单行。
    const cloud = w
      .findAll("[data-test='delivery-task']")
      .find(task => task.text().includes("云层文档同步"));
    await cloud!.trigger("click");
    await flushPromises();
    const repos = w.findAll("[data-test='delivery-repo']");
    expect(repos).toHaveLength(2);
    expect(repos.map(r => r.text()).join(" ")).toContain("未交付");
    expect(w.find("[data-test='delivery-banner']").classes()).toContain(
      "is-missing"
    );
    expect(w.find("[data-test='delivery-banner']").text()).toContain(
      "缺：shared-contracts"
    );
    // review_status=running → 按钮不出现，提示审查进行中。
    expect(w.find("[data-test='delivery-approve']").exists()).toBe(false);
    expect(w.find("[data-test='delivery-action-hint']").text()).toContain(
      "审查进行中"
    );
  });

  it("退回必填理由：无理由被拦截且不调用 decision 端点", async () => {
    const w = await mountPage();
    await w.find("[data-test='delivery-reject']").trigger("click");
    await flushPromises();
    expect(decideReviewApproval).not.toHaveBeenCalled();
    expect(w.find("[data-test='delivery-action-error']").text()).toContain(
      "退回必须填写理由"
    );
  });

  it("通过验收：调用既有 decision 端点，回执展示并原位刷新总表与详情", async () => {
    const w = await mountPage();
    expect(getAcceptanceOverview).toHaveBeenCalledTimes(1);
    await w.find("[data-test='delivery-approve']").trigger("click");
    await flushPromises();
    await flushPromises();
    expect(decideReviewApproval).toHaveBeenCalledWith("appr-forest-1", {
      project_id: PROJECT,
      decision: "approved",
      reason: undefined
    });
    expect(w.find("[data-test='delivery-decision-outcome']").text()).toContain(
      "已退回修改"
    );
    // 原位刷新：总表再拉一次，不整页跳转。
    expect(getAcceptanceOverview).toHaveBeenCalledTimes(2);
  });

  it("退回：带理由提交，理由透传 decision 端点", async () => {
    const w = await mountPage();
    await w.find("[data-test='delivery-reason']").setValue("海拔权重未接入");
    await w.find("[data-test='delivery-reject']").trigger("click");
    await flushPromises();
    expect(decideReviewApproval).toHaveBeenCalledWith("appr-forest-1", {
      project_id: PROJECT,
      decision: "rejected",
      reason: "海拔权重未接入"
    });
  });

  it("无审查记录的任务：动作区显示「尚未发起审查」且无按钮", async () => {
    const w = await mountPage();
    const river = w
      .findAll("[data-test='delivery-task']")
      .find(task => task.text().includes("河流生成性能回归"));
    await river!.trigger("click");
    await flushPromises();
    expect(w.find("[data-test='delivery-approve']").exists()).toBe(false);
    expect(w.find("[data-test='delivery-action-hint']").text()).toContain(
      "尚未发起审查"
    );
    expect(w.find("[data-test='delivery-banner']").text()).toContain(
      "集成失败"
    );
  });

  it("加载失败显示可重试提示，不伪装为空态", async () => {
    getAcceptanceOverview.mockRejectedValueOnce(new Error("boom"));
    const w = await mountPage();
    expect(w.find("[data-test='delivery-failed']").exists()).toBe(true);
    expect(w.find("[data-test='delivery-empty']").exists()).toBe(false);
  });
});
