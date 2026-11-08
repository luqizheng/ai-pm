// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";

import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";
import type {
  RequirementCoreDetail,
  RequirementCoreEntry,
  RequirementCoreSourceInput
} from "@/api/requirements";

import RequirementDetailView from "./RequirementDetailView.vue";
import DetailContextPanel from "./components/DetailContextPanel.vue";

// 接真批（2026-11-08）：只 mock IO，适配器/命令助手走真实实现。
const getRequirement = vi.fn();
const listRequirementSources = vi.fn();
const listRequirementCriteria = vi.fn();
const addRequirementCriterion = vi.fn();
const removeRequirementCriterion = vi.fn();
const recordCriterionVerdict = vi.fn();
const createDraftRevision = vi.fn();
const updateDraftRevision = vi.fn();
const submitRevision = vi.fn();

vi.mock("@/api/requirements", async importOriginal => {
  const actual = await importOriginal<typeof import("@/api/requirements")>();
  return {
    ...actual,
    getRequirement: (...args: unknown[]) => getRequirement(...args),
    listRequirementSources: (...args: unknown[]) =>
      listRequirementSources(...args),
    listRequirementCriteria: (...args: unknown[]) =>
      listRequirementCriteria(...args),
    addRequirementCriterion: (...args: unknown[]) =>
      addRequirementCriterion(...args),
    removeRequirementCriterion: (...args: unknown[]) =>
      removeRequirementCriterion(...args),
    recordCriterionVerdict: (...args: unknown[]) =>
      recordCriterionVerdict(...args),
    createDraftRevision: (...args: unknown[]) => createDraftRevision(...args),
    updateDraftRevision: (...args: unknown[]) => updateDraftRevision(...args),
    submitRevision: (...args: unknown[]) => submitRevision(...args)
  };
});

const t = (key: string, params?: Record<string, unknown>) =>
  i18n.global.t(key, params ?? {}) as string;

// 弹窗/抽屉经 Teleport 渲染进 document.body，用例间清场防 DOM 残留互染。
afterEach(() => {
  document.body.innerHTML = "";
});

function row(over: Partial<RequirementCoreEntry>): RequirementCoreEntry {
  return {
    id: "req-1",
    project_id: "prj-shield",
    code: "REQ-001",
    title: "User authentication",
    requirement_type: "functional",
    priority: "high",
    lifecycle_status: "active",
    created_at: "2026-11-07T10:00:00Z",
    updated_at: "2026-11-08T09:00:00Z",
    row_revision: 7,
    current_revision_id: "rev-2",
    current_version: 2,
    current_content_hash: "hash-v2",
    latest_revision_id: "rev-2",
    latest_version: 2,
    latest_state: "approved",
    latest_submitted_at: "2026-11-08T08:00:00Z",
    ...over
  };
}

function revision(
  id: string,
  version: number,
  state: RequirementCoreDetail["revisions"][number]["state"]
): RequirementCoreDetail["revisions"][number] {
  return {
    id,
    requirement_id: "req-1",
    version,
    state,
    markdown: `# 登录\n正文 v${version}`,
    summary: version === 1 ? "初始版本" : "补充锁定策略",
    content_hash: `hash-${id}`,
    markdown_version_ref: null,
    base_current_revision_id: null,
    base_row_revision: null,
    submitted_at: null,
    created_by: "prc-1",
    created_by_name: "林悦",
    approved_by: null,
    approved_by_name: null,
    created_at: "2026-11-07T10:00:00Z"
  };
}

const detailApproved: RequirementCoreDetail = {
  requirement: row({}),
  revisions: [
    revision("rev-1", 1, "superseded"),
    revision("rev-2", 2, "approved")
  ],
  confirmations: [
    {
      id: "conf-1",
      revision_id: "rev-2",
      action: "confirm",
      reason: null,
      human_principal_id: "prc-2",
      human_display_name: "审批人甲",
      source: "rest",
      content_hash: "hash-rev-2",
      markdown_version_ref: null,
      confirmed_at: "2026-11-08T09:00:00Z"
    }
  ]
};

function sourceItem(
  over: Partial<RequirementCoreSourceInput>
): RequirementCoreSourceInput {
  return {
    id: "src-1",
    project_id: "prj-shield",
    input_kind: "human_input",
    title: "用户访谈纪要",
    content: "用户希望支持邮箱登录",
    source_ref: "MIN-004",
    created_by: "prc-1",
    created_at: "2026-11-07T09:00:00Z",
    ...over
  };
}

async function mountPage(id = "req-1") {
  const mounted = mountShell(RequirementDetailView);
  await mounted.router.push(`/project/requirements/${id}`);
  await flushPromises();
  return mounted;
}

describe("RequirementDetailView（接真）", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getRequirement.mockResolvedValue(detailApproved);
    listRequirementSources.mockResolvedValue({ items: [sourceItem({})] });
    listRequirementCriteria.mockResolvedValue({
      items: [
        {
          id: "ac-1",
          requirement_id: "req-1",
          position: 1,
          statement: "P95 接口响应 ≤ 300ms",
          verification_kind: "performance",
          params: { threshold: 300 },
          established_in_revision_id: null,
          verdict: "failed",
          verdict_note: "回归复测未过",
          verdict_by: "prc-2",
          verdict_at: "2026-11-08T10:00:00Z",
          created_at: "2026-11-08T09:00:00Z",
          updated_at: "2026-11-08T10:00:00Z"
        }
      ]
    });
    addRequirementCriterion.mockResolvedValue({ criterion: {} });
    removeRequirementCriterion.mockResolvedValue(undefined);
    recordCriterionVerdict.mockResolvedValue({ criterion: {} });
    createDraftRevision.mockResolvedValue({
      revision: revision("rev-3", 3, "draft")
    });
    updateDraftRevision.mockResolvedValue({
      revision: revision("rev-2", 2, "draft")
    });
    submitRevision.mockResolvedValue({
      revision: revision("rev-3", 3, "pending_confirmation")
    });
  });

  it("渲染真实详情：四卡 + 右栏 Tab + 来源（human_input→用户请求）+ 确认决策", async () => {
    const { wrapper } = await mountPage();

    const text = wrapper.text();
    expect(text).toContain("REQ-001");
    expect(text).toContain("User authentication");
    // 正文块 = 当前正式版本 Markdown
    expect(text).toContain("正文 v2");
    // 追溯链来源（真实 REST，S02-I3）
    expect(text).toContain("用户访谈纪要");
    // 右栏四 Tab 在场
    expect(text).toContain(t("pm.requirementDetail.tabRelated"));
    expect(text).toContain(t("pm.requirementDetail.tabHistory"));

    // 切到 History Tab（reka Tabs 在 happy-dom 不响应原生 click，走组件事件）
    // 修订链确认决策已映射 → 关联决策「审批人甲」
    const panel = wrapper.findComponent(DetailContextPanel);
    expect(panel.exists()).toBe(true);
    panel.vm.$emit("update:activeTab", "history");
    await flushPromises();
    expect(wrapper.text()).toContain("审批人甲");
  });

  it("点击来源打开来源检查抽屉，标注来源不是正式需求", async () => {
    const { wrapper } = await mountPage();

    // 追溯链（或右栏面板）的来源条目点击 → 来源检查抽屉
    const chainSource = wrapper.find(".reqd-trace__item--source");
    const target = chainSource.exists()
      ? chainSource
      : wrapper.findAll(".reqd-trace__item").at(0);
    if (target && target.exists()) await target.trigger("click");
    await flushPromises();

    // Sheet 走 Teleport，从 document 断言（来源检查态）
    expect(document.body.textContent).toContain(
      t("pm.requirementDetail.sourceSheetTitle")
    );
    expect(document.body.textContent).toContain(
      t("pm.requirementDetail.sourceSheetNote")
    );
  });

  it("编辑保存：已定稿（无 draft）时起草新修订而非覆盖", async () => {
    const { wrapper } = await mountPage();

    const editButton = wrapper
      .findAll("button")
      .find(node => node.text().includes(t("pm.requirements.actionEdit")));
    await editButton?.trigger("click");
    await flushPromises();

    // EditSheet 抽屉渲染进 body：标题只读 + 正文预填当前正式版
    expect(document.body.textContent).toContain(
      t("pm.requirementDetail.editSheetTitle")
    );
    const textarea = [...document.body.querySelectorAll("textarea")].find(
      node => (node as HTMLTextAreaElement).value.includes("正文 v2")
    );
    expect(textarea).toBeDefined();
    textarea.value = "# 登录\n改后的正文";
    textarea.dispatchEvent(new Event("input", { bubbles: true }));
    await flushPromises();

    const saveButton = [...document.body.querySelectorAll("button")].find(
      node => node.textContent?.includes(t("pm.requirements.editSave"))
    );
    saveButton?.click();
    await flushPromises();

    expect(createDraftRevision).toHaveBeenCalledTimes(1);
    expect(createDraftRevision).toHaveBeenCalledWith(
      "req-1",
      expect.objectContaining({ markdown: "# 登录\n改后的正文" })
    );
    expect(updateDraftRevision).not.toHaveBeenCalled();
  });

  it("提交确认：存在 draft 修订时经弹窗直接 submit 该修订", async () => {
    getRequirement.mockResolvedValue({
      requirement: row({
        current_revision_id: null,
        current_version: null,
        current_content_hash: null,
        latest_revision_id: "rev-1",
        latest_version: 1,
        latest_state: "draft"
      }),
      revisions: [revision("rev-1", 1, "draft")],
      confirmations: []
    });
    const { wrapper } = await mountPage();

    const proposeButton = wrapper
      .findAll("button")
      .find(node => node.text().includes(t("pm.requirements.actionPropose")));
    await proposeButton?.trigger("click");
    await flushPromises();

    expect(document.body.textContent).toContain(
      t("pm.requirements.proposeDialogTitle")
    );
    // 未确认前不提交
    expect(submitRevision).not.toHaveBeenCalled();

    const dialog = wrapper.findComponent(ConfirmationDialog);
    expect(dialog.exists()).toBe(true);
    dialog.vm.$emit("confirm");
    await flushPromises();

    expect(submitRevision).toHaveBeenCalledTimes(1);
    expect(submitRevision).toHaveBeenCalledWith("req-1", "rev-1");
  });

  it("404 渲染未找到态并可返回列表", async () => {
    getRequirement.mockRejectedValue(new Error("404: requirement not found"));
    const { wrapper } = await mountPage("req-missing");

    expect(wrapper.text()).toContain(t("pm.requirementDetail.notFoundTitle"));
    expect(wrapper.text()).toContain(t("pm.requirementDetail.backToList"));
  });

  it("加载失败显示错误态与重试入口", async () => {
    getRequirement.mockRejectedValue(new Error("network down"));
    const { wrapper } = await mountPage();

    expect(wrapper.text()).toContain(t("pm.requirements.loadError"));
    expect(wrapper.text()).toContain("network down");
    expect(wrapper.text()).toContain(t("pm.requirements.loadRetry"));
  });
});
