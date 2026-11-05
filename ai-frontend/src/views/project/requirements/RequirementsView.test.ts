// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { flushPromises } from "@vue/test-utils";

import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";

import RequirementsView from "./RequirementsView.vue";

const t = (key: string, params?: Record<string, unknown>) =>
  i18n.global.t(key, params ?? {}) as string;

describe("RequirementsView", () => {
  it("渲染需求工作区：页头、摘要聚合、列表样本、详情与上下文面板", async () => {
    const mounted = mountShell(RequirementsView);
    await mounted.router.push("/project/requirements");
    await flushPromises();

    const text = mounted.wrapper.text();

    // 页头（S02-UI-01 §4）
    expect(text).toContain(t("pm.requirements.pageTitle"));
    expect(text).toContain(t("pm.requirements.actionAdd"));
    expect(text).toContain(t("pm.requirements.baselineEntry"));
    // 摘要五聚合（§5）
    expect(text).toContain(t("pm.requirements.summaryTotal"));
    expect(text).toContain(t("pm.requirements.summaryPending"));
    expect(text).toContain(t("pm.requirements.summaryConflicts"));
    // 列表四条样本记录（§6.1）
    for (const code of ["REQ-001", "REQ-002", "REQ-003", "REQ-004"]) {
      expect(text).toContain(code);
    }
    // 详情面板五节（默认选中 REQ-001）
    expect(text).toContain(t("pm.requirements.detailAcceptance"));
    expect(text).toContain(t("pm.requirements.detailSources"));
    expect(text).toContain(t("pm.requirements.detailRevisions"));
    expect(text).toContain(t("pm.requirements.detailRelated"));
    // 右栏四 Tab（§7）
    expect(text).toContain(t("pm.requirements.tabSources"));
    expect(text).toContain(t("pm.requirements.tabAi"));
    // AI 协作入口（§10）：提示文案与建议动作可见（placeholder 是 attribute 不进 text()）
    expect(text).toContain(t("pm.requirements.aiBarNote"));
    expect(text).toContain(t("pm.requirements.aiActionCompleteness"));
  });

  it("操作按状态显隐：待确认显示确认/拒绝，草稿显示编辑/提交确认", async () => {
    const mounted = mountShell(RequirementsView);
    await mounted.router.push("/project/requirements");
    await flushPromises();

    // 默认选中 REQ-001（pending_confirmation）
    const pendingText = mounted.wrapper.text();
    expect(pendingText).toContain(t("pm.requirements.actionConfirm"));
    expect(pendingText).toContain(t("pm.requirements.actionReject"));
    expect(pendingText).not.toContain(t("pm.requirements.actionEdit"));

    // 切到 REQ-003（draft）
    const rows = mounted.wrapper.findAll(".req-row");
    await rows[2].trigger("click");
    await flushPromises();

    const draftText = mounted.wrapper.text();
    expect(draftText).toContain(t("pm.requirements.actionEdit"));
    expect(draftText).toContain(t("pm.requirements.actionPropose"));
    expect(draftText).not.toContain(t("pm.requirements.actionConfirm"));
  });

  it("人类确认是显式操作：必须经确认弹窗，确认后需求定稿", async () => {
    const mounted = mountShell(RequirementsView);
    await mounted.router.push("/project/requirements");
    await flushPromises();

    // 点击「确认需求」→ 弹窗出现（含人工审阅勾选文案），状态未迁移
    const confirmButton = mounted.wrapper
      .findAll("button")
      .find(node => node.text().includes(t("pm.requirements.actionConfirm")));
    await confirmButton?.trigger("click");
    await flushPromises();

    // ConfirmationDialog 经 Teleport 渲染到 body，从 document 断言
    expect(document.body.textContent).toContain(
      t("pm.requirements.confirmDialogTitle")
    );
    expect(document.body.textContent).toContain(
      t("pm.requirements.confirmCheckbox")
    );
    expect(mounted.wrapper.text()).toContain(
      t("pm.requirements.statusPending")
    );

    // 显式确认 → REQ-001 转为已确认，修订定稿
    const dialog = mounted.wrapper.findComponent(ConfirmationDialog);
    expect(dialog.exists()).toBe(true);
    dialog.vm.$emit("confirm");
    await flushPromises();

    const text = mounted.wrapper.text();
    expect(text).toContain(t("pm.requirements.statusConfirmed"));
    expect(text).toContain(t("pm.requirements.revStateApproved"));
  });

  it("AI 动作只追加提案到 AI Assistance，绝不改写需求状态", async () => {
    const mounted = mountShell(RequirementsView);
    await mounted.router.push("/project/requirements");
    await flushPromises();

    const actionButton = mounted.wrapper
      .findAll("button")
      .find(node =>
        node.text().includes(t("pm.requirements.aiActionCompleteness"))
      );
    await actionButton?.trigger("click");
    await flushPromises();

    const text = mounted.wrapper.text();
    // 提案徽标出现在 AI Assistance 面板
    expect(text).toContain(t("pm.requirements.aiProposalBadge"));
    // 需求状态未被 AI 改写（红线：AI 不得自动确认）
    expect(text).toContain(t("pm.requirements.statusPending"));
  });
});
