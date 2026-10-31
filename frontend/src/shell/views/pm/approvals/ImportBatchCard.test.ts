// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import ImportBatchCard from "./ImportBatchCard.vue";
import { mountShell } from "../../../test-utils";

const getImportProposals = vi.fn();
const decideApproval = vi.fn();

vi.mock("@/api/imports", () => ({
  getImportProposals: (...args: unknown[]) => getImportProposals(...args)
}));

vi.mock("@/api/pm", () => ({
  decideApproval: (...args: unknown[]) => decideApproval(...args)
}));

vi.mock("@/components/ui/toast", () => ({
  useToast: () => ({ toast: vi.fn() })
}));

const proposals = [
  { kind: "create_milestones", proposal_id: "p1", approval_id: "a1", item_count: 1 },
  { kind: "create_requirements", proposal_id: "p2", approval_id: "a2", item_count: 2 },
  { kind: "create_tasks", proposal_id: "p3", approval_id: "a3", item_count: 3 },
  { kind: "write_documents", proposal_id: "p4", approval_id: "a4", item_count: 3 }
];

beforeEach(() => {
  vi.clearAllMocks();
  getImportProposals.mockResolvedValue({
    outcome: "proposed",
    proposals,
    analysis_run_id: "run-1"
  });
  decideApproval.mockResolvedValue({ id: "a", status: "approved" });
});

describe("导入批次审批卡", () => {
  it("按批次渲染 4 项提案与总件数", async () => {
    const wrapper = mountShell(ImportBatchCard, {
      props: { projectId: "project-1", importId: "import-1" }
    });
    await flushPromises();

    expect(getImportProposals).toHaveBeenCalledWith("project-1", "import-1");
    const text = wrapper.text();
    expect(text).toContain("create_milestones");
    expect(text).toContain("create_requirements");
    expect(text).toContain("create_tasks");
    expect(text).toContain("write_documents");
  });

  it("整批批准顺序调用 4 次 decide", async () => {
    const wrapper = mountShell(ImportBatchCard, {
      props: { projectId: "project-1", importId: "import-1" }
    });
    await flushPromises();

    const approve = wrapper
      .findAll("button")
      .find(button => button.text().includes("整批批准"));
    await approve!.trigger("click");
    await flushPromises();

    expect(decideApproval).toHaveBeenCalledTimes(4);
    expect(decideApproval).toHaveBeenNthCalledWith(
      1,
      "a1",
      expect.objectContaining({ decision: "approved" })
    );
  });

  it("部分失败展示失败项且其余继续", async () => {
    decideApproval
      .mockResolvedValueOnce({ id: "a1", status: "approved" })
      .mockRejectedValueOnce(new Error("conflict"))
      .mockResolvedValue({ id: "a", status: "approved" });
    const wrapper = mountShell(ImportBatchCard, {
      props: { projectId: "project-1", importId: "import-1" }
    });
    await flushPromises();

    await wrapper
      .findAll("button")
      .find(button => button.text().includes("整批批准"))!
      .trigger("click");
    await flushPromises();

    expect(decideApproval).toHaveBeenCalledTimes(4);
    expect(wrapper.text()).toContain("有 1 项失败");
  });
});