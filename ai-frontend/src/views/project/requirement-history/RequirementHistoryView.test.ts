// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { flushPromises } from "@vue/test-utils";

import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";

import RequirementHistoryView from "./RequirementHistoryView.vue";

const t = (key: string, params?: Record<string, unknown>) =>
  i18n.global.t(key, params ?? {}) as string;

describe("RequirementHistoryView", () => {
  it("渲染修订历史页：页头、时间线、详情与审计卡", async () => {
    const mounted = mountShell(RequirementHistoryView);
    await mounted.router.push("/project/requirements/req-001/history");
    await flushPromises();

    const text = mounted.wrapper.text();

    // 页头（S02-UI-05 §2）
    expect(text).toContain(t("pm.requirementHistory.pageTitle"));
    expect(text).toContain("REQ-001");
    expect(text).toContain(t("pm.requirementHistory.currentVersion"));
    expect(text).toContain(
      t("pm.requirementDetail.currentVersion", { version: 2 })
    );
    // 三动作
    expect(text).toContain(t("pm.requirementHistory.actionBack"));
    expect(text).toContain(t("pm.requirementHistory.actionCompare"));
    expect(text).toContain(t("pm.requirementHistory.actionPropose"));
    // 时间线（§3）：两版 + 来源与决策区分
    expect(text).toContain(t("pm.requirementHistory.timelineTitle"));
    expect(text).toContain(t("pm.requirementHistory.originAi"));
    expect(text).toContain(t("pm.requirementHistory.originHuman"));
    expect(text).toContain(
      t("pm.requirementHistory.decisionConfirmed", {
        actor: "林悦",
        time: "2026-10-30 09:00"
      })
    );
    // 详情（§4）与审计（§6）
    expect(text).toContain(t("pm.requirementHistory.detailTitle"));
    expect(text).toContain(t("pm.requirementHistory.auditTitle"));
    expect(text).toContain(t("pm.requirementHistory.auditInitiated"));
  });

  it("选中切换：点击 v1 后详情与审计显示该版快照", async () => {
    const mounted = mountShell(RequirementHistoryView);
    await mounted.router.push("/project/requirements/req-001/history");
    await flushPromises();

    const items = mounted.wrapper.findAll(".rh-timeline__item");
    await items[1].trigger("click");
    await flushPromises();

    const text = mounted.wrapper.text();
    // v1 快照：只有一条验收标准；v1 曾被定稿（关联决策出现）
    expect(text).toContain(t("pm.requirements.revStateSuperseded"));
    expect(text).toContain(t("pm.requirementHistory.auditConfirmed"));
  });

  it("比较模式：默认相邻两版，行内 diff 与元数据差异呈现", async () => {
    const mounted = mountShell(RequirementHistoryView);
    await mounted.router.push("/project/requirements/req-001/history");
    await flushPromises();

    const compareButton = mounted.wrapper
      .findAll("button")
      .find(
        node => node.text().trim() === t("pm.requirementHistory.actionCompare")
      );
    await compareButton?.trigger("click");
    await flushPromises();

    // 版本徽标与 diff 统计（v1→v2：+2 行）
    expect(mounted.wrapper.text()).toContain(
      t("pm.requirementHistory.compareOld", { version: 1 })
    );
    expect(mounted.wrapper.text()).toContain(
      t("pm.requirementHistory.compareNew", { version: 2 })
    );
    expect(mounted.wrapper.text()).toContain(
      t("pm.requirementHistory.compareStat", { added: 2, removed: 0 })
    );
    // 行内 diff：新增行渲染
    expect(mounted.wrapper.findAll(".rhc__row--added").length).toBe(2);
    // 元数据 diff：v1→v2 新增两条验收标准
    expect(mounted.wrapper.text()).toContain(
      t("pm.requirementHistory.compareCriteriaLabel")
    );
  });

  it("提议新修订经显式确认弹窗，确认后时间线新增待确认修订", async () => {
    const mounted = mountShell(RequirementHistoryView);
    await mounted.router.push("/project/requirements/req-001/history");
    await flushPromises();

    const proposeButton = mounted.wrapper
      .findAll("button")
      .find(
        node => node.text().trim() === t("pm.requirementHistory.actionPropose")
      );
    await proposeButton?.trigger("click");
    await flushPromises();

    // 弹窗（Teleport 到 body）
    expect(document.body.textContent).toContain(
      t("pm.requirementHistory.proposeModalTitle")
    );

    mounted.wrapper.findComponent(ConfirmationDialog).vm.$emit("confirm");
    await flushPromises();

    const text = mounted.wrapper.text();
    expect(text).toContain(
      t("pm.requirements.revisionVersion", { version: 3 })
    );
    expect(text).toContain(t("pm.requirements.revStatePending"));
  });

  it("历史不可覆盖：页面明示无恢复/覆盖操作", async () => {
    const mounted = mountShell(RequirementHistoryView);
    await mounted.router.push("/project/requirements/req-001/history");
    await flushPromises();

    // §7 红线的沟通呈现；页面不存在恢复/覆盖类动作按钮
    expect(mounted.wrapper.text()).toContain(
      t("pm.requirementHistory.noOverwriteNote")
    );
    const buttonTexts = mounted.wrapper
      .findAll("button")
      .map(node => node.text());
    expect(
      buttonTexts.some(text => text.includes("恢复") || text.includes("覆盖"))
    ).toBe(false);
  });

  it("返回需求：导航到该需求的详情页", async () => {
    const mounted = mountShell(RequirementHistoryView);
    await mounted.router.push("/project/requirements/req-001/history");
    await flushPromises();

    const backButton = mounted.wrapper
      .findAll("button")
      .find(
        node => node.text().trim() === t("pm.requirementHistory.actionBack")
      );
    await backButton?.trigger("click");
    await flushPromises();

    expect(mounted.router.currentRoute.value.path).toBe(
      "/project/requirements/req-001"
    );
  });
});
