// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { flushPromises } from "@vue/test-utils";

import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";

import RequirementDetailView from "./RequirementDetailView.vue";

const t = (key: string, params?: Record<string, unknown>) =>
  i18n.global.t(key, params ?? {}) as string;

describe("RequirementDetailView", () => {
  it("渲染需求详情：概览、规格、验收标准、追溯链与右栏四 Tab", async () => {
    const mounted = mountShell(RequirementDetailView);
    await mounted.router.push("/project/requirements/req-001");
    await flushPromises();

    const text = mounted.wrapper.text();

    // 面包屑与页头（S02-UI-02 §2）
    expect(text).toContain(t("pm.projects.pageTitle"));
    expect(text).toContain("REQ-001");
    expect(text).toContain("User authentication");
    // §3A 概览
    expect(text).toContain(t("pm.requirementDetail.overviewTitle"));
    expect(text).toContain(t("pm.requirementDetail.overviewCreated"));
    // §3B 规格
    expect(text).toContain(t("pm.requirementDetail.specRationale"));
    expect(text).toContain(t("pm.requirementDetail.specExclusions"));
    // §3C 验收标准
    expect(text).toContain(t("pm.requirementDetail.acceptTitle"));
    // §3D 追溯链三段
    expect(text).toContain(t("pm.requirementDetail.traceUpstream"));
    expect(text).toContain(t("pm.requirementDetail.traceRequirement"));
    expect(text).toContain(t("pm.requirementDetail.traceDownstream"));
    // §4 右栏四 Tab
    expect(text).toContain(t("pm.requirementDetail.tabRelated"));
    expect(text).toContain(t("pm.requirementDetail.tabHistory"));
  });

  it("追溯链默认收起（+N 计数），展开后显示全部来源", async () => {
    const mounted = mountShell(RequirementDetailView);
    await mounted.router.push("/project/requirements/req-001");
    await flushPromises();

    // 收起态：追溯链内 REQ-001 三条来源只显示前两条（右栏 Sources Tab 始终全量）
    const chainBefore = mounted.wrapper.find(".reqd-trace__chain").text();
    expect(chainBefore).not.toContain("MIN-1103-02");
    expect(chainBefore).toContain("+1");
    const expandButton = mounted.wrapper
      .findAll("button")
      .find(node =>
        node.text().includes(t("pm.requirementDetail.traceExpand"))
      );
    await expandButton?.trigger("click");
    await flushPromises();

    // 展开态（Deliverable 2）：追溯链内全部来源可见
    expect(mounted.wrapper.find(".reqd-trace__chain").text()).toContain(
      "MIN-1103-02"
    );
    expect(mounted.wrapper.text()).toContain(
      t("pm.requirementDetail.traceCollapse")
    );
  });

  it("点击来源打开来源检查抽屉，标注来源不是正式需求", async () => {
    const mounted = mountShell(RequirementDetailView);
    await mounted.router.push("/project/requirements/req-001");
    await flushPromises();

    const sourceItem = mounted.wrapper.find(".reqd-trace__item--source");
    await sourceItem.trigger("click");
    await flushPromises();

    // Sheet 走 Teleport，从 document 断言（Deliverable 3 来源检查态）
    expect(document.body.textContent).toContain(
      t("pm.requirementDetail.sourceSheetTitle")
    );
    expect(document.body.textContent).toContain(
      t("pm.requirementDetail.sourceSheetNote")
    );
  });

  it("关联需求可导航：REQ-004 追溯链点击 REQ-002 跳转其详情页", async () => {
    const mounted = mountShell(RequirementDetailView);
    await mounted.router.push("/project/requirements/req-004");
    await flushPromises();
    expect(mounted.wrapper.text()).toContain("API response consistency");

    const navigable = mounted.wrapper.find(".reqd-trace__item--navigable");
    await navigable.trigger("click");
    await flushPromises();

    expect(mounted.router.currentRoute.value.path).toBe(
      "/project/requirements/req-002"
    );
    expect(mounted.wrapper.text()).toContain("System response time");
  });

  it("提案经显式确认弹窗后进入待确认状态，修订新增", async () => {
    const mounted = mountShell(RequirementDetailView);
    await mounted.router.push("/project/requirements/req-003");
    await flushPromises();
    expect(mounted.wrapper.text()).toContain(t("pm.requirements.statusDraft"));

    const proposeButton = mounted.wrapper
      .findAll("button")
      .find(node => node.text().includes(t("pm.requirements.actionPropose")));
    await proposeButton?.trigger("click");
    await flushPromises();

    // 弹窗描述（Teleport 内容），未确认前状态不变
    expect(document.body.textContent).toContain(
      t("pm.requirements.proposeDialogDesc", { code: "REQ-003" })
    );
    expect(mounted.wrapper.text()).toContain(t("pm.requirements.statusDraft"));

    mounted.wrapper.findComponent(ConfirmationDialog).vm.$emit("confirm");
    await flushPromises();

    expect(mounted.wrapper.text()).toContain(
      t("pm.requirements.statusPending")
    );
  });
});
