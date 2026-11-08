// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";

import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";
import type {
  RequirementCoreDetail,
  RequirementCoreEntry
} from "@/api/requirements";

import RequirementHistoryView from "./RequirementHistoryView.vue";

// 接真批（2026-11-08）：只 mock IO（GET 详情 + create/submit 命令），
// 适配器与命令助手走真实实现。
const getRequirement = vi.fn();
const createDraftRevision = vi.fn();
const submitRevision = vi.fn();

vi.mock("@/api/requirements", async importOriginal => {
  const actual = await importOriginal<typeof import("@/api/requirements")>();
  return {
    ...actual,
    getRequirement: (...args: unknown[]) => getRequirement(...args),
    createDraftRevision: (...args: unknown[]) => createDraftRevision(...args),
    submitRevision: (...args: unknown[]) => submitRevision(...args)
  };
});

const t = (key: string, params?: Record<string, unknown>) =>
  i18n.global.t(key, params ?? {}) as string;

// 弹窗经 Teleport 渲染进 document.body，用例间清场防 DOM 残留互染。
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
  state: RequirementCoreDetail["revisions"][number]["state"],
  markdown: string
): RequirementCoreDetail["revisions"][number] {
  return {
    id,
    requirement_id: "req-1",
    version,
    state,
    markdown,
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

const detailTwo: RequirementCoreDetail = {
  requirement: row({}),
  revisions: [
    revision("rev-1", 1, "superseded", "# 登录\n- 邮箱登录"),
    revision("rev-2", 2, "approved", "# 登录\n- 邮箱登录\n- 失败锁定")
  ],
  confirmations: [
    {
      id: "conf-2",
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

async function mountPage() {
  const mounted = mountShell(RequirementHistoryView);
  await mounted.router.push("/project/requirements/req-1/history");
  await flushPromises();
  return mounted;
}

describe("RequirementHistoryView（接真）", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getRequirement.mockResolvedValue(detailTwo);
    createDraftRevision.mockResolvedValue({
      revision: revision("rev-3", 3, "draft", "# 登录\n改后正文")
    });
    submitRevision.mockResolvedValue({
      revision: revision("rev-3", 3, "pending_confirmation", "# 登录\n改后正文")
    });
  });

  it("渲染修订历史页：页头、时间线两版、详情与审计卡（确认决策来自真实记录）", async () => {
    const { wrapper } = await mountPage();

    const text = wrapper.text();
    expect(text).toContain(t("pm.requirementHistory.pageTitle"));
    expect(text).toContain("REQ-001");
    expect(text).toContain(t("pm.requirementHistory.currentVersion"));
    // 三动作
    expect(text).toContain(t("pm.requirementHistory.actionBack"));
    expect(text).toContain(t("pm.requirementHistory.actionCompare"));
    expect(text).toContain(t("pm.requirementHistory.actionPropose"));
    // 时间线（§3）
    expect(text).toContain(t("pm.requirementHistory.timelineTitle"));
    // 详情（§4）与审计（§6）：确认决策映射自 Confirmation 记录
    expect(text).toContain(t("pm.requirementHistory.detailTitle"));
    expect(text).toContain(t("pm.requirementHistory.auditTitle"));
    expect(text).toContain("审批人甲");
  });

  it("选中切换：点击 v1 后详情显示该版（已被替代）", async () => {
    const { wrapper } = await mountPage();

    const items = wrapper.findAll(".rh-timeline__item");
    await items[1].trigger("click");
    await flushPromises();

    expect(wrapper.text()).toContain(t("pm.requirements.revStateSuperseded"));
  });

  it("比较模式：默认相邻两版，行内 diff 呈现新增行", async () => {
    const { wrapper } = await mountPage();

    const compareButton = wrapper
      .findAll("button")
      .find(
        node => node.text().trim() === t("pm.requirementHistory.actionCompare")
      );
    await compareButton?.trigger("click");
    await flushPromises();

    const text = wrapper.text();
    expect(text).toContain(
      t("pm.requirementHistory.compareOld", { version: 1 })
    );
    expect(text).toContain(
      t("pm.requirementHistory.compareNew", { version: 2 })
    );
    // 行内 diff：v1→v2 新增一行（失败锁定）
    expect(wrapper.findAll(".rhc__row--added").length).toBe(1);
  });

  it("提议新修订经显式确认弹窗：以当前正文 create+submit，确认后回刷时间线", async () => {
    const { wrapper } = await mountPage();

    const proposeButton = wrapper
      .findAll("button")
      .find(
        node => node.text().trim() === t("pm.requirementHistory.actionPropose")
      );
    await proposeButton?.trigger("click");
    await flushPromises();

    // 弹窗（Teleport 到 body），未确认前无命令发出
    expect(document.body.textContent).toContain(
      t("pm.requirementHistory.proposeModalTitle")
    );
    expect(createDraftRevision).not.toHaveBeenCalled();

    const dialog = wrapper.findComponent(ConfirmationDialog);
    expect(dialog.exists()).toBe(true);
    dialog.vm.$emit("confirm");
    await flushPromises();

    expect(createDraftRevision).toHaveBeenCalledTimes(1);
    expect(createDraftRevision).toHaveBeenCalledWith(
      "req-1",
      expect.objectContaining({ markdown: expect.stringContaining("# 登录") })
    );
    expect(submitRevision).toHaveBeenCalledWith("req-1", "rev-3");
  });

  it("历史不可覆盖：页面明示无恢复/覆盖操作", async () => {
    const { wrapper } = await mountPage();

    // §7 红线的沟通呈现；页面不存在恢复/覆盖类动作按钮
    expect(wrapper.text()).toContain(
      t("pm.requirementHistory.noOverwriteNote")
    );
    const buttonTexts = wrapper.findAll("button").map(node => node.text());
    expect(
      buttonTexts.some(text => text.includes("恢复") || text.includes("覆盖"))
    ).toBe(false);
  });

  it("返回需求：导航到该需求的详情页", async () => {
    const { wrapper, router } = await mountPage();

    const backButton = wrapper
      .findAll("button")
      .find(
        node => node.text().trim() === t("pm.requirementHistory.actionBack")
      );
    await backButton?.trigger("click");
    await flushPromises();

    expect(router.currentRoute.value.path).toBe("/project/requirements/req-1");
  });

  it("加载失败显示错误态与重试入口", async () => {
    getRequirement.mockRejectedValue(new Error("network down"));
    const { wrapper } = await mountPage();

    expect(wrapper.text()).toContain(t("pm.requirements.loadError"));
    expect(wrapper.text()).toContain("network down");
  });
});
