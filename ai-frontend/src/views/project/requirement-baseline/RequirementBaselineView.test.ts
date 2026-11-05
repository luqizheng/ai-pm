// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { flushPromises } from "@vue/test-utils";

import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";

import RequirementBaselineView from "./RequirementBaselineView.vue";

const t = (key: string, params?: Record<string, unknown>) =>
  i18n.global.t(key, params ?? {}) as string;

describe("RequirementBaselineView", () => {
  it("渲染基线管理页：概览、历史列表与基线详情", async () => {
    const mounted = mountShell(RequirementBaselineView);
    await mounted.router.push("/project/requirements/baselines");
    await flushPromises();

    const text = mounted.wrapper.text();

    // 页头（S02-UI-06 §2）
    expect(text).toContain(t("pm.requirementBaseline.pageTitle"));
    expect(text).toContain(t("pm.requirementBaseline.pageSubtitle"));
    expect(text).toContain(t("pm.requirementBaseline.actionViewCurrent"));
    expect(text).toContain(t("pm.requirementBaseline.actionPrepare"));
    // 概览（§3）：当前基线 BL-002 v2 生效中
    expect(text).toContain(t("pm.requirementBaseline.overviewTitle"));
    expect(text).toContain("BL-002");
    expect(text).toContain(
      t("pm.requirementBaseline.baselineVersion", { version: 2 })
    );
    expect(text).toContain(t("pm.requirementBaseline.statusActive"));
    // 历史（§4）：两条记录
    expect(text).toContain(t("pm.requirementBaseline.listTitle"));
    expect(text).toContain("BL-001");
    expect(text).toContain(t("pm.requirementBaseline.statusSuperseded"));
    // 详情（§5）：元数据 + 含入需求（含入修订 v2）+ 摘要分布
    expect(text).toContain(t("pm.requirementBaseline.detailMetaTitle"));
    expect(text).toContain(t("pm.requirementBaseline.detailIncludedTitle"));
    expect(text).toContain(
      t("pm.requirements.revisionVersion", { version: 2 })
    );
    expect(text).toContain(t("pm.requirementBaseline.detailSummaryTitle"));
    expect(text).toContain(
      t("pm.requirementBaseline.summaryDistribution", {
        functional: 0,
        nonFunctional: 1
      })
    );
  });

  it("选中切换：点击 BL-001 显示其两条含入需求（历史保留原引用）", async () => {
    const mounted = mountShell(RequirementBaselineView);
    await mounted.router.push("/project/requirements/baselines");
    await flushPromises();

    const rows = mounted.wrapper.findAll(".rbl__row");
    await rows[1].trigger("click");
    await flushPromises();

    const text = mounted.wrapper.text();
    expect(text).toContain("REQ-001");
    expect(text).toContain(
      t("pm.requirements.revisionVersion", { version: 1 })
    );
  });

  it("查看当前基线：选中回到生效中的 BL-002", async () => {
    const mounted = mountShell(RequirementBaselineView);
    await mounted.router.push("/project/requirements/baselines");
    await flushPromises();

    await mounted.wrapper.findAll(".rbl__row")[1].trigger("click");
    await flushPromises();

    const viewCurrent = mounted.wrapper
      .findAll("button")
      .find(
        node =>
          node.text().trim() === t("pm.requirementBaseline.actionViewCurrent")
      );
    await viewCurrent?.trigger("click");
    await flushPromises();

    // 详情回到 BL-002（含入修订为 v2）
    expect(mounted.wrapper.text()).toContain(
      t("pm.requirements.revisionVersion", { version: 2 })
    );
  });

  it("准备向导：问题需求四类锁定明示，不静默含入", async () => {
    const mounted = mountShell(RequirementBaselineView);
    await mounted.router.push("/project/requirements/baselines");
    await flushPromises();

    const prepareButton = mounted.wrapper
      .findAll("button")
      .find(
        node => node.text().trim() === t("pm.requirementBaseline.actionPrepare")
      );
    await prepareButton?.trigger("click");
    await flushPromises();

    const text = mounted.wrapper.text();
    // Step 1 指示与可勾选需求（confirmed 的 REQ-002 默认勾选）
    expect(text).toContain(
      t("pm.requirementBaseline.wizardStep", { current: 1, total: 5 })
    );
    expect(text).toContain("REQ-002");
    // 四类问题需求锁定区（§6）
    expect(text).toContain(t("pm.requirementBaseline.lockedTitle"));
    expect(text).toContain(t("pm.requirementBaseline.issueDraft"));
    expect(text).toContain(t("pm.requirementBaseline.issuePending"));
    expect(text).toContain(t("pm.requirementBaseline.issueConflict"));
    expect(text).toContain(t("pm.requirementBaseline.issueIncomplete"));
    expect(text).toContain("REQ-003");
    // 确认前不得出现新基线（不自动建立）
    expect(text).not.toContain("BL-003");
  });

  it("五步推进后经显式确认弹窗建立基线，旧基线转已被替代", async () => {
    const mounted = mountShell(RequirementBaselineView);
    await mounted.router.push("/project/requirements/baselines");
    await flushPromises();

    const prepareButton = mounted.wrapper
      .findAll("button")
      .find(
        node => node.text().trim() === t("pm.requirementBaseline.actionPrepare")
      );
    await prepareButton?.trigger("click");
    await flushPromises();

    // 逐步推进到 Step 5
    for (let current = 1; current <= 4; current += 1) {
      const nextButton = mounted.wrapper
        .findAll("button")
        .find(
          node => node.text().trim() === t("pm.requirementBaseline.wizardNext")
        );
      await nextButton?.trigger("click");
      await flushPromises();
    }

    // Step 5 摘要复述

    expect(mounted.wrapper.text()).toContain(
      t("pm.requirementBaseline.wizardConfirmText", { version: 3, count: 1 })
    );

    const establishButton = mounted.wrapper
      .findAll("button")
      .find(
        node =>
          node.text().trim() === t("pm.requirementBaseline.wizardEstablish")
      );
    await establishButton?.trigger("click");
    await flushPromises();

    // 显式确认弹窗（Teleport 到 body）
    expect(document.body.textContent).toContain(
      t("pm.requirementBaseline.confirmModalTitle")
    );
    expect(document.body.textContent).toContain(
      t("pm.requirementBaseline.confirmCheckbox")
    );

    mounted.wrapper.findComponent(ConfirmationDialog).vm.$emit("confirm");
    await flushPromises();

    const text = mounted.wrapper.text();
    // 新基线生效 + 旧基线转已被替代（Deliverable 4）
    expect(text).toContain("BL-003");
    expect(text).toContain(
      t("pm.requirementBaseline.baselineVersion", { version: 3 })
    );
    expect(text).toContain(t("pm.requirementBaseline.statusActive"));
    expect(text).toContain(t("pm.requirementBaseline.statusSuperseded"));
  });

  it("基线比较：BL-001 → BL-002 呈现移除与修改（含验收标准差异）", async () => {
    const mounted = mountShell(RequirementBaselineView);
    await mounted.router.push("/project/requirements/baselines");
    await flushPromises();

    const compareButton = mounted.wrapper
      .findAll("button")
      .find(
        node => node.text().trim() === t("pm.requirementBaseline.actionCompare")
      );
    await compareButton?.trigger("click");
    await flushPromises();

    const text = mounted.wrapper.text();
    // 默认比较：旧 = BL-001 v1，新 = BL-002 v2
    expect(text).toContain(t("pm.requirementBaseline.compareTitle"));
    expect(text).toContain(
      t("pm.requirementBaseline.compareRemoved", { count: 1 })
    );
    expect(text).toContain(
      t("pm.requirementBaseline.compareModified", { count: 1 })
    );
    // REQ-002 v1→v2：验收标准差异（Deliverable 5）
    expect(text).toContain(
      "列表类接口在 500 并发下 P95 响应时间不超过 300ms。"
    );
    expect(text).toContain("列表类接口在 500 并发持续 10 分钟下 P95 ≤ 300ms。");
    expect(mounted.wrapper.find(".rbc__criterion--removed").exists()).toBe(
      true
    );
    expect(mounted.wrapper.find(".rbc__criterion--added").exists()).toBe(true);
  });
});
