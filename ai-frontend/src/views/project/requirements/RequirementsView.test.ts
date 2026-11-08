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

import RequirementCreateDialog from "./components/RequirementCreateDialog.vue";
import RequirementsView from "./RequirementsView.vue";

// 接真批（2026-11-08）：页面数据一律来自 @/api/requirements（S02-I1/I3 REST），
// 这里只 mock IO 函数，适配器（toEntryFromListItem/toRequirementEntry/toSourceItems）
// 用真实实现——映射口径本身就是被测对象。
const listProjectRequirements = vi.fn();
const getRequirement = vi.fn();
const listRequirementSources = vi.fn();
const listRequirementCriteria = vi.fn();
const createRequirement = vi.fn();
const createDraftRevision = vi.fn();
const updateDraftRevision = vi.fn();
const submitRevision = vi.fn();
const confirmRevision = vi.fn();
const rejectRevision = vi.fn();

vi.mock("@/api/requirements", async importOriginal => {
  const actual = await importOriginal<typeof import("@/api/requirements")>();
  return {
    ...actual,
    listProjectRequirements: (...args: unknown[]) =>
      listProjectRequirements(...args),
    getRequirement: (...args: unknown[]) => getRequirement(...args),
    listRequirementSources: (...args: unknown[]) =>
      listRequirementSources(...args),
    listRequirementCriteria: (...args: unknown[]) =>
      listRequirementCriteria(...args),
    createRequirement: (...args: unknown[]) => createRequirement(...args),
    createDraftRevision: (...args: unknown[]) => createDraftRevision(...args),
    updateDraftRevision: (...args: unknown[]) => updateDraftRevision(...args),
    submitRevision: (...args: unknown[]) => submitRevision(...args),
    confirmRevision: (...args: unknown[]) => confirmRevision(...args),
    rejectRevision: (...args: unknown[]) => rejectRevision(...args)
  };
});

const t = (key: string, params?: Record<string, unknown>) =>
  i18n.global.t(key, params ?? {}) as string;

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
    current_revision_id: "rev-1",
    current_version: 1,
    current_content_hash: "hash-v1",
    latest_revision_id: "rev-2",
    latest_version: 2,
    latest_state: "pending_confirmation",
    latest_submitted_at: "2026-11-08T08:00:00Z",
    ...over
  };
}

function revision(
  id: string,
  version: number,
  state: RequirementCoreDetail["revisions"][number]["state"],
  over: Partial<RequirementCoreDetail["revisions"][number]> = {}
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
    created_at: "2026-11-07T10:00:00Z",
    ...over
  };
}

const detailPending: RequirementCoreDetail = {
  requirement: row({}),
  revisions: [
    revision("rev-1", 1, "approved"),
    revision("rev-2", 2, "pending_confirmation")
  ],
  confirmations: []
};

const detailDraft: RequirementCoreDetail = {
  requirement: row({
    id: "req-2",
    code: "REQ-002",
    title: "Session timeout",
    priority: "normal",
    row_revision: 3,
    current_revision_id: null,
    current_version: null,
    current_content_hash: null,
    latest_revision_id: "rev-d1",
    latest_version: 1,
    latest_state: "draft",
    latest_submitted_at: null
  }),
  revisions: [
    revision("rev-d1", 1, "draft", { requirement_id: "req-2", summary: "" })
  ],
  confirmations: []
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

async function mountPage() {
  const mounted = mountShell(RequirementsView);
  await mounted.router.push("/project/requirements?project_id=prj-shield");
  await flushPromises();
  return mounted;
}

// 弹窗类组件经 Teleport 渲染进 document.body，用例间清场防 DOM 残留互染。
afterEach(() => {
  document.body.innerHTML = "";
});

describe("RequirementsView（接真）", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listProjectRequirements.mockResolvedValue({
      items: [detailPending.requirement, detailDraft.requirement]
    });
    getRequirement.mockImplementation(async (id: string) =>
      id === "req-1" ? detailPending : detailDraft
    );
    listRequirementSources.mockResolvedValue({
      items: [sourceItem({})]
    });
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
          verdict: "passed",
          verdict_note: "压测报告 PR-9",
          verdict_by: "prc-2",
          verdict_at: "2026-11-08T10:00:00Z",
          created_at: "2026-11-08T09:00:00Z",
          updated_at: "2026-11-08T10:00:00Z"
        }
      ]
    });
    createRequirement.mockResolvedValue({
      requirement: row({ id: "req-3", code: "REQ-003", latest_state: "draft" })
    });
    createDraftRevision.mockResolvedValue({
      revision: revision("rev-new", 3, "draft")
    });
    updateDraftRevision.mockResolvedValue({
      revision: revision("rev-d1", 1, "draft")
    });
    submitRevision.mockResolvedValue({
      revision: revision("rev-d1", 1, "pending_confirmation")
    });
    confirmRevision.mockResolvedValue({
      requirement: detailPending.requirement,
      criteria_drift: true
    });
    rejectRevision.mockResolvedValue({
      revision: revision("rev-2", 2, "rejected")
    });
  });

  it("渲染真实列表、详情正文与来源（human_input 映射为用户请求）", async () => {
    const { wrapper } = await mountPage();

    const text = wrapper.text();
    // 列表两行（真实 REST 读模型）
    expect(text).toContain("REQ-001");
    expect(text).toContain("REQ-002");
    // 详情默认选中 REQ-001：正文块 = 当前正式版本 Markdown
    expect(text).toContain("正文 v1");
    // 来源面板：human_input → 用户请求；ref 透传
    expect(text).toContain(t("pm.requirements.tabSources"));
    expect(text).toContain("用户访谈纪要");
    expect(text).toContain(t("pm.requirements.sourceUserRequest"));
    // 待确认态动作显隐（§8）
    expect(text).toContain(t("pm.requirements.actionConfirm"));
    expect(text).toContain(t("pm.requirements.actionReject"));
    // 验收标准（S02-I5 真数据）：陈述 + 承载词表徽标 + 裁决徽标
    expect(text).toContain("P95 接口响应 ≤ 300ms");
    expect(text).toContain(t("pm.requirements.critKindPerformance"));
    expect(text).toContain(t("pm.requirements.critVerdictPassed"));
    // mock 专供件不再渲染（AI 协作栏已随接真移除）
    expect(text).not.toContain(t("pm.requirements.aiBarNote"));
  });

  it("人类确认是显式操作：弹窗确认后携带核准哈希 + CAS 令牌调 REST", async () => {
    const { wrapper } = await mountPage();

    const confirmButton = wrapper
      .findAll("button")
      .find(node => node.text().includes(t("pm.requirements.actionConfirm")));
    await confirmButton?.trigger("click");
    await flushPromises();

    expect(document.body.textContent).toContain(
      t("pm.requirements.confirmDialogTitle")
    );
    // 未确认前状态不迁移
    expect(wrapper.text()).toContain(t("pm.requirements.statusPending"));

    const dialog = wrapper.findComponent(ConfirmationDialog);
    dialog.vm.$emit("confirm");
    await flushPromises();

    expect(confirmRevision).toHaveBeenCalledTimes(1);
    expect(confirmRevision).toHaveBeenCalledWith("req-1", "rev-2", {
      expected_content_hash: "hash-rev-2",
      expected_row_revision: 7
    });
    // Q-12 一致性提示：验收项在提交后被改 → 确认后漂移提示在场
    expect(wrapper.text()).toContain(t("pm.requirements.driftHintTitle"));
  });

  it("拒绝提案：弹窗确认后调 REST，待确认修订标记拒绝", async () => {
    const { wrapper } = await mountPage();

    const rejectButton = wrapper
      .findAll("button")
      .find(node => node.text().includes(t("pm.requirements.actionReject")));
    await rejectButton?.trigger("click");
    await flushPromises();

    expect(document.body.textContent).toContain(
      t("pm.requirements.rejectDialogTitle")
    );
    const dialog = wrapper.findComponent(ConfirmationDialog);
    dialog.vm.$emit("confirm");
    await flushPromises();

    expect(rejectRevision).toHaveBeenCalledTimes(1);
    expect(rejectRevision).toHaveBeenCalledWith("req-1", "rev-2");
  });

  it("编辑保存：最新修订为 draft 时更新该草稿（不是新建）", async () => {
    const { wrapper } = await mountPage();

    // 切到 REQ-002（draft，最新修订 rev-d1 为 draft）
    const rows = wrapper.findAll(".req-row");
    await rows[1].trigger("click");
    await flushPromises();

    const editButton = wrapper
      .findAll("button")
      .find(node => node.text().includes(t("pm.requirements.actionEdit")));
    await editButton?.trigger("click");
    await flushPromises();

    // 编辑表单：title 只读（创建后不可改），正文预填当前修订 Markdown
    const disabledInput = wrapper
      .findAll("input")
      .find(input => input.attributes("disabled") !== undefined);
    expect(disabledInput).toBeDefined();
    const textarea = wrapper.find("textarea");
    expect((textarea.element as HTMLTextAreaElement).value).toContain(
      "正文 v1"
    );

    await textarea.setValue("# 登录\n改后的正文");
    const saveButton = wrapper
      .findAll("button")
      .find(node => node.text().includes(t("pm.requirements.editSave")));
    await saveButton?.trigger("click");
    await flushPromises();

    expect(updateDraftRevision).toHaveBeenCalledTimes(1);
    expect(updateDraftRevision).toHaveBeenCalledWith(
      "req-2",
      "rev-d1",
      expect.objectContaining({ markdown: "# 登录\n改后的正文" })
    );
    expect(createDraftRevision).not.toHaveBeenCalled();
  });

  it("提交确认：最新修订为 draft 时直接 submit 该修订", async () => {
    const { wrapper } = await mountPage();

    const rows = wrapper.findAll(".req-row");
    await rows[1].trigger("click");
    await flushPromises();

    const proposeButton = wrapper
      .findAll("button")
      .find(node => node.text().includes(t("pm.requirements.actionPropose")));
    await proposeButton?.trigger("click");
    await flushPromises();

    expect(submitRevision).toHaveBeenCalledTimes(1);
    expect(submitRevision).toHaveBeenCalledWith("req-2", "rev-d1");
  });

  it("新建需求：对话框 emit 后调 createRequirement（编码自动递推）", async () => {
    const { wrapper } = await mountPage();

    const addButton = wrapper
      .findAll("button")
      .find(node => node.text().includes(t("pm.requirements.actionAdd")));
    await addButton?.trigger("click");
    await flushPromises();

    const createDialog = wrapper.findComponent(RequirementCreateDialog);
    expect(createDialog.exists()).toBe(true);
    expect(createDialog.props("nextCode")).toBe("REQ-003");

    createDialog.vm.$emit("create", {
      code: "REQ-003",
      title: "支持组织级 SSO",
      requirement_type: "functional",
      priority: "normal",
      markdown: undefined
    });
    await flushPromises();

    expect(createRequirement).toHaveBeenCalledTimes(1);
    expect(createRequirement).toHaveBeenCalledWith("prj-shield", {
      code: "REQ-003",
      title: "支持组织级 SSO",
      requirement_type: "functional",
      priority: "normal",
      markdown: undefined
    });
  });

  it("加载失败显示错误态与重试入口，而不是空列表", async () => {
    listProjectRequirements.mockRejectedValue(new Error("network down"));
    const { wrapper } = await mountPage();

    expect(wrapper.text()).toContain(t("pm.requirements.loadError"));
    expect(wrapper.text()).toContain("network down");
    expect(wrapper.text()).toContain(t("pm.requirements.loadRetry"));
  });
});
