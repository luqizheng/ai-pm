// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { flushPromises } from "@vue/test-utils";

import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";

import RequirementImpactView from "./RequirementImpactView.vue";

const t = (key: string, params?: Record<string, unknown>) =>
  i18n.global.t(key, params ?? {}) as string;

describe("RequirementImpactView", () => {
  it("渲染影响分析页：页头五要素、变更摘要、依赖图、影响面板与决策面板", async () => {
    const mounted = mountShell(RequirementImpactView);
    await mounted.router.push("/project/requirements/req-001/impact");
    await flushPromises();

    const text = mounted.wrapper.text();

    // 页头（S02-UI-07 §2）：五要素
    expect(text).toContain(t("pm.requirementImpact.pageTitle"));
    expect(text).toContain("REQ-001");
    expect(text).toContain(t("pm.requirementImpact.baselineNotIncluded"));
    expect(text).toContain(
      t("pm.requirements.revisionVersion", { version: 2 })
    );
    expect(text).toContain(t("pm.requirementImpact.statusAnalyzed"));
    // 变更摘要（§3）
    expect(text).toContain(t("pm.requirementImpact.summaryTitle"));
    expect(text).toContain(t("pm.requirementImpact.summaryProposedBy"));
    // 依赖图（§4）
    expect(text).toContain(t("pm.requirementImpact.graphTitle"));
    for (const code of [
      "AC-001",
      "TASK-210",
      "KB-AUTH-012",
      "BL-002",
      "M-01"
    ]) {
      expect(text).toContain(code);
    }
    // 四级影响（§4）
    expect(text).toContain(t("pm.requirementImpact.levelDirect"));
    expect(text).toContain(t("pm.requirementImpact.levelIndirect"));
    expect(text).toContain(t("pm.requirementImpact.levelPotential"));
    expect(text).toContain(t("pm.requirementImpact.levelUncertain"));
    // 影响面板（§5）：A-D 四类
    expect(text).toContain(t("pm.requirementImpact.panelRequirement"));
    expect(text).toContain(t("pm.requirementImpact.panelImplementation"));
    expect(text).toContain(t("pm.requirementImpact.panelDocumentation"));
    expect(text).toContain(t("pm.requirementImpact.panelBaseline"));
    // AI 分析（§6）与决策面板（§7）
    expect(text).toContain(t("pm.requirementImpact.aiTitle"));
    expect(text).toContain(t("pm.requirementImpact.decisionTitle"));
    expect(text).toContain(t("pm.requirementImpact.decisionAccept"));
  });

  it("依赖不虚构：REQ-001 的图中无 REQ-004（两者无真实关系）", async () => {
    const mounted = mountShell(RequirementImpactView);
    await mounted.router.push("/project/requirements/req-001/impact");
    await flushPromises();

    expect(mounted.wrapper.findAll(".rdg__target").length).toBe(5);
    expect(mounted.wrapper.text()).not.toContain("REQ-004");
  });

  it("AI 分析明确标注为分析而非确认事实", async () => {
    const mounted = mountShell(RequirementImpactView);
    await mounted.router.push("/project/requirements/req-001/impact");
    await flushPromises();

    const text = mounted.wrapper.text();
    expect(text).toContain(t("pm.requirementImpact.aiBadge"));
    expect(text).toContain(t("pm.requirementImpact.aiDisclaimer"));
    expect(text).toContain(t("pm.requirementImpact.aiRisk"));
    expect(text).toContain(t("pm.requirementImpact.aiConfidence"));
  });

  it("决策前复述变更摘要与影响发现计数", async () => {
    const mounted = mountShell(RequirementImpactView);
    await mounted.router.push("/project/requirements/req-001/impact");
    await flushPromises();

    const text = mounted.wrapper.text();
    expect(text).toContain(t("pm.requirementImpact.decisionRecapFindings"));
    expect(text).toContain(
      t("pm.requirementImpact.decisionRecapText", { findings: 7, direct: 2 })
    );
  });

  it("接受变更经显式确认弹窗，确认后留决策记录", async () => {
    const mounted = mountShell(RequirementImpactView);
    await mounted.router.push("/project/requirements/req-001/impact");
    await flushPromises();

    const acceptButton = mounted.wrapper
      .findAll("button")
      .find(
        node => node.text().trim() === t("pm.requirementImpact.decisionAccept")
      );
    await acceptButton?.trigger("click");
    await flushPromises();

    // 弹窗（Teleport 到 body）与人工审阅勾选
    expect(document.body.textContent).toContain(
      t("pm.requirementImpact.acceptModalTitle")
    );
    expect(document.body.textContent).toContain(
      t("pm.requirementImpact.acceptCheckbox")
    );

    mounted.wrapper.findComponent(ConfirmationDialog).vm.$emit("confirm");
    await flushPromises();

    expect(mounted.wrapper.text()).toContain(
      t("pm.requirementImpact.decisionAccepted")
    );
  });

  it("拒绝变更经危险确认弹窗，确认后留决策记录", async () => {
    const mounted = mountShell(RequirementImpactView);
    await mounted.router.push("/project/requirements/req-001/impact");
    await flushPromises();

    const rejectButton = mounted.wrapper
      .findAll("button")
      .find(
        node => node.text().trim() === t("pm.requirementImpact.decisionReject")
      );
    await rejectButton?.trigger("click");
    await flushPromises();

    expect(document.body.textContent).toContain(
      t("pm.requirementImpact.rejectModalTitle")
    );

    mounted.wrapper.findComponent(ConfirmationDialog).vm.$emit("confirm");
    await flushPromises();

    expect(mounted.wrapper.text()).toContain(
      t("pm.requirementImpact.decisionRejected")
    );
  });

  it("要求更多分析：分析状态回到等待分析并留决策记录", async () => {
    const mounted = mountShell(RequirementImpactView);
    await mounted.router.push("/project/requirements/req-001/impact");
    await flushPromises();

    const moreButton = mounted.wrapper
      .findAll("button")
      .find(
        node =>
          node.text().trim() === t("pm.requirementImpact.decisionMoreAnalysis")
      );
    await moreButton?.trigger("click");
    await flushPromises();

    const text = mounted.wrapper.text();
    expect(text).toContain(t("pm.requirementImpact.statusPending"));
    expect(text).toContain(t("pm.requirementImpact.decisionMoreAnalysis"));
  });
});
