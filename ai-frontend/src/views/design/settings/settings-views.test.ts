// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import type { Component } from "vue";
import { createMemoryHistory, createRouter } from "vue-router";
import { DOMWrapper } from "@vue/test-utils";

import { mountShell } from "@/lib/test-utils";
import { buildRoutes } from "@/router";
import { DESIGN_PATHS } from "@/router/design";

import SettingsArchitectureView from "./SettingsArchitectureView.vue";
import SettingsComponentsView from "./SettingsComponentsView.vue";
import SettingsPersonalAiView from "./SettingsPersonalAiView.vue";
import SettingsPersonalNotificationsView from "./SettingsPersonalNotificationsView.vue";
import SettingsPersonalPreferencesView from "./SettingsPersonalPreferencesView.vue";
import SettingsPersonalProfileView from "./SettingsPersonalProfileView.vue";
import SettingsPersonalSecurityView from "./SettingsPersonalSecurityView.vue";
import SettingsPersonalView from "./SettingsPersonalView.vue";
import SettingsProjectAiView from "./SettingsProjectAiView.vue";
import SettingsProjectDevelopmentView from "./SettingsProjectDevelopmentView.vue";
import SettingsProjectGeneralView from "./SettingsProjectGeneralView.vue";
import SettingsProjectMembersView from "./SettingsProjectMembersView.vue";
import SettingsProjectRepositoriesView from "./SettingsProjectRepositoriesView.vue";
import SettingsProjectView from "./SettingsProjectView.vue";
import SettingsStatesView from "./SettingsStatesView.vue";
import SettingsTenantGeneralView from "./SettingsTenantGeneralView.vue";
import SettingsTenantIntegrationsView from "./SettingsTenantIntegrationsView.vue";
import SettingsTenantMembersView from "./SettingsTenantMembersView.vue";
import SettingsTenantView from "./SettingsTenantView.vue";
import { useSettingsDemo } from "@/components/settings";

/**
 * Settings Framework 19 页目录视图契约（Settings Framework V1.0 §8）：
 *   - 每页可挂载且渲染关键内容（≥1 条行为用例/页）；
 *   - 权限演示：租户成员 denied、项目成员只读、负责人可编辑（§9）；
 *   - 交互演示页 12 样板齐备（§6）。
 *
 * 注：弹窗（ui/Dialog、ui/ConfirmationDialog）Teleport 到 body，
 * 断言与点击走 document.body；radio 为隐藏原生 input，经 setValue 触发。
 */
const mountedWrappers: VueWrapper[] = [];

async function mountAt(view: Component, path: string) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push(path);
  await router.isReady();
  const wrapper = mountShell(view, { global: { plugins: [router] } });
  mountedWrappers.push(wrapper);
  return wrapper;
}

function bodyButtons(): HTMLButtonElement[] {
  return [...document.body.querySelectorAll("button")];
}

async function clickBodyButton(text: string) {
  const button = bodyButtons().find(b => b.textContent?.includes(text));
  expect(button, `body button: ${text}`).toBeTruthy();
  await new DOMWrapper(button!).trigger("click");
}

function bodyText(): string {
  return document.body.textContent ?? "";
}

afterEach(() => {
  // 先卸载组件（含 Teleport/reka portal），再清理 body 残留；
  // 顺序颠倒会导致 Vue 卸载时引用已被移除的节点
  for (const wrapper of mountedWrappers.splice(0)) {
    wrapper.unmount();
  }
  document.body.innerHTML = "";
});

describe("帧 01 — Settings 架构总览", () => {
  it("三域卡片 + 18 帧索引可达链接", async () => {
    const wrapper = await mountAt(
      SettingsArchitectureView,
      DESIGN_PATHS.settings.architecture
    );
    expect(wrapper.findAll(".settings-arch__scope")).toHaveLength(3);
    expect(wrapper.findAll(".settings-arch__frame")).toHaveLength(18);
  });
});

describe("帧 02~07 — 个人域", () => {
  it("落地页五类目卡", async () => {
    const wrapper = await mountAt(
      SettingsPersonalView,
      DESIGN_PATHS.settings.personal
    );
    expect(wrapper.findAll(".personal-landing__card")).toHaveLength(5);
  });

  it("资料页：编辑后保存条浮现，取消回滚", async () => {
    const wrapper = await mountAt(
      SettingsPersonalProfileView,
      DESIGN_PATHS.settings.personalProfile
    );
    expect(wrapper.find(".settings-savebar").exists()).toBe(false);

    const input = wrapper.find("#profile-display-name");
    await input.setValue("韩林");
    expect(wrapper.find(".settings-savebar").exists()).toBe(true);

    const cancel = wrapper
      .findAll("button")
      .find(b => b.text().includes("取消"));
    await cancel!.trigger("click");
    expect(wrapper.find(".settings-savebar").exists()).toBe(false);
    expect(
      (wrapper.find("#profile-display-name").element as HTMLInputElement).value
    ).toBe("Han Lin");
  });

  it("安全页：吊销会话走 danger 确认后移除该行", async () => {
    const wrapper = await mountAt(
      SettingsPersonalSecurityView,
      DESIGN_PATHS.settings.personalSecurity
    );
    const rowsBefore = wrapper.findAll("tbody tr").length;
    expect(rowsBefore).toBe(3);

    const revoke = wrapper
      .findAll("button")
      .find(b => b.text().includes("吊销"));
    await revoke!.trigger("click");
    expect(bodyText()).toContain("吊销此会话？");

    const confirmButtons = bodyButtons().filter(b =>
      b.textContent?.includes("吊销")
    );
    await new DOMWrapper(confirmButtons.at(-1)!).trigger("click");
    expect(wrapper.findAll("tbody tr").length).toBe(rowsBefore - 1);
  });

  it("偏好页：主题 radio 切换本区块 data-theme", async () => {
    const wrapper = await mountAt(
      SettingsPersonalPreferencesView,
      DESIGN_PATHS.settings.personalPreferences
    );
    const lightInput = wrapper.find('input[type=radio][value="light"]');
    expect(lightInput.exists()).toBe(true);
    await lightInput.setValue();
    expect(wrapper.find(".preferences-theme[data-theme=light]").exists()).toBe(
      true
    );
    // 快捷键表只读呈现
    expect(wrapper.find(".preferences-shortcuts").exists()).toBe(true);
  });

  it("通知页：邮件通道禁用（Proposed），类别开关可切换", async () => {
    const wrapper = await mountAt(
      SettingsPersonalNotificationsView,
      DESIGN_PATHS.settings.personalNotifications
    );
    const switches = wrapper.findAll("[role=switch]");
    expect(switches.length).toBeGreaterThanOrEqual(6);
    const disabled = switches.filter(
      s => s.attributes("disabled") !== undefined
    );
    expect(disabled.length).toBe(2);

    const firstInactive = switches.find(
      s =>
        s.attributes("disabled") === undefined &&
        s.attributes("aria-checked") === "false"
    );
    if (firstInactive) {
      await firstInactive.trigger("click");
      expect(firstInactive.attributes("aria-checked")).toBe("true");
    }
  });

  it("AI 偏好页：三种交互方式 + 边界说明", async () => {
    const wrapper = await mountAt(
      SettingsPersonalAiView,
      DESIGN_PATHS.settings.personalAi
    );
    expect(wrapper.findAll(".ui-radio")).toHaveLength(3);
    expect(wrapper.text()).toContain("个人 AI 偏好只影响你本人");
  });
});

describe("帧 08~11 — 租户域", () => {
  it("落地页：成员角色切换到无授权 → denied 空态", async () => {
    const demo = useSettingsDemo();
    demo.setTenantPersona("tenant-admin");
    const wrapper = await mountAt(
      SettingsTenantView,
      DESIGN_PATHS.settings.tenant
    );
    expect(wrapper.findAll(".tenant-landing__card")).toHaveLength(4);

    const memberInput = wrapper.find(
      'input[type=radio][value="tenant-member"]'
    );
    await memberInput.setValue();
    expect(wrapper.text()).toContain("无权访问租户管理");
    expect(wrapper.find(".tenant-landing__grid").exists()).toBe(false);

    demo.setTenantPersona("tenant-admin");
  });

  it("基本信息页：管理员可编辑；成员角色 → denied", async () => {
    const demo = useSettingsDemo();
    demo.setTenantPersona("tenant-admin");
    const wrapper = await mountAt(
      SettingsTenantGeneralView,
      DESIGN_PATHS.settings.tenantGeneral
    );
    const input = wrapper.find("#tenant-name");
    expect(input.attributes("disabled")).toBeUndefined();
    await input.setValue("新租户名");
    expect(wrapper.find(".settings-savebar").exists()).toBe(true);

    demo.setTenantPersona("tenant-member");
    const denied = await mountAt(
      SettingsTenantGeneralView,
      DESIGN_PATHS.settings.tenantGeneral
    );
    expect(denied.text()).toContain("无权访问租户管理设置");
    expect(denied.find("#tenant-name").exists()).toBe(false);
    demo.setTenantPersona("tenant-admin");
  });

  it("成员页：移除成员走确认弹窗；邀请弹窗可开", async () => {
    const demo = useSettingsDemo();
    demo.setTenantPersona("tenant-admin");
    const wrapper = await mountAt(
      SettingsTenantMembersView,
      DESIGN_PATHS.settings.tenantMembers
    );
    const rowsBefore = wrapper.findAll("tbody tr").length;

    const remove = wrapper
      .findAll("button")
      .find(b => b.text().includes("移除"));
    await remove!.trigger("click");
    expect(bodyText()).toContain("移除该成员？");
    await clickBodyButton("移除");
    expect(wrapper.findAll("tbody tr").length).toBe(rowsBefore - 1);

    const invite = wrapper
      .findAll("button")
      .find(b => b.text().includes("邀请成员"));
    await invite!.trigger("click");
    expect(bodyText()).toContain("向新成员发送加入邀请");
    demo.setTenantPersona("tenant-admin");
  });

  it("集成页：连接动作翻转状态徽标；SSO 恒禁用（Proposed）", async () => {
    const demo = useSettingsDemo();
    demo.setTenantPersona("tenant-admin");
    const wrapper = await mountAt(
      SettingsTenantIntegrationsView,
      DESIGN_PATHS.settings.tenantIntegrations
    );
    expect(wrapper.text()).toContain("已连接");

    const connect = wrapper
      .findAll("button")
      .filter(b => b.text().includes("连接") && !b.text().includes("断开"))
      .find(b => b.attributes("disabled") === undefined);
    await connect!.trigger("click");
    expect(wrapper.text().split("已连接").length - 1).toBeGreaterThanOrEqual(2);

    const disabledConnect = wrapper
      .findAll("button")
      .filter(b => b.text().includes("连接"))
      .find(b => b.attributes("disabled") !== undefined);
    expect(disabledConnect).toBeTruthy();
    demo.setTenantPersona("tenant-admin");
  });
});

describe("帧 12~17 — 项目域", () => {
  it("落地页：三角色切换器 + 五类目卡", async () => {
    const wrapper = await mountAt(
      SettingsProjectView,
      DESIGN_PATHS.settings.project
    );
    expect(wrapper.findAll(".project-landing__card")).toHaveLength(5);
    expect(wrapper.findAll(".ui-radio")).toHaveLength(3);
  });

  it("基本信息页：成员只读（禁用输入+横幅）；负责人见危险区", async () => {
    const demo = useSettingsDemo();
    demo.setProjectPersona("project-member");
    const member = await mountAt(
      SettingsProjectGeneralView,
      DESIGN_PATHS.settings.projectGeneral
    );
    expect(member.find("#project-name").attributes("disabled")).toBeDefined();
    expect(member.text()).toContain("只有查看权限");
    expect(member.text()).not.toContain("危险区");

    demo.setProjectPersona("project-lead");
    const lead = await mountAt(
      SettingsProjectGeneralView,
      DESIGN_PATHS.settings.projectGeneral
    );
    const archive = lead
      .findAll("button")
      .find(b => b.text().includes("归档项目"));
    expect(archive).toBeTruthy();
    await archive!.trigger("click");
    expect(bodyText()).toContain("归档该项目？");
  });

  it("成员页：负责人可改角色；成员只见文本角色", async () => {
    const demo = useSettingsDemo();
    demo.setProjectPersona("project-lead");
    const lead = await mountAt(
      SettingsProjectMembersView,
      DESIGN_PATHS.settings.projectMembers
    );
    expect(lead.findAll(".ui-select__trigger").length).toBeGreaterThan(0);

    demo.setProjectPersona("project-member");
    const member = await mountAt(
      SettingsProjectMembersView,
      DESIGN_PATHS.settings.projectMembers
    );
    expect(member.text()).toContain("开发者");
    demo.setProjectPersona("project-lead");
  });

  it("仓库页：设主仓库确认弹窗；管理入口卡存在", async () => {
    const demo = useSettingsDemo();
    demo.setProjectPersona("project-lead");
    const wrapper = await mountAt(
      SettingsProjectRepositoriesView,
      DESIGN_PATHS.settings.projectRepositories
    );
    expect(wrapper.text()).toContain("主仓库");

    const setPrimary = wrapper
      .findAll("button")
      .find(b => b.text().includes("设为主仓库"));
    await setPrimary!.trigger("click");
    expect(bodyText()).toContain("设为主仓库？");
    await clickBodyButton("设为主仓库");
    // 管理入口卡存在（不复刻仓库管理界面）
    expect(wrapper.text()).toContain("前往仓库管理");
  });

  it("开发配置页：模板下拉 + Proposed 禁用开关", async () => {
    const demo = useSettingsDemo();
    demo.setProjectPersona("project-lead");
    const wrapper = await mountAt(
      SettingsProjectDevelopmentView,
      DESIGN_PATHS.settings.projectDevelopment
    );
    expect(wrapper.text()).toContain("基础（Node + Python）");
    const disabled = wrapper
      .findAll("[role=switch]")
      .filter(s => s.attributes("disabled") !== undefined);
    expect(disabled.length).toBe(1);
  });

  it("AI 配置页：两种执行方式；只读时无保存条", async () => {
    const demo = useSettingsDemo();
    demo.setProjectPersona("project-lead");
    const lead = await mountAt(
      SettingsProjectAiView,
      DESIGN_PATHS.settings.projectAi
    );
    expect(lead.findAll(".ui-radio")).toHaveLength(2);
    expect(lead.text()).not.toContain("跳过人工确认");

    demo.setProjectPersona("project-viewer");
    const viewer = await mountAt(
      SettingsProjectAiView,
      DESIGN_PATHS.settings.projectAi
    );
    expect(viewer.find(".settings-savebar").exists()).toBe(false);
    demo.setProjectPersona("project-lead");
  });
});

describe("帧 18~19 — 交互态与组件库", () => {
  it("交互态页：12 样板 + 未保存/破坏性弹窗可开", async () => {
    const wrapper = await mountAt(
      SettingsStatesView,
      DESIGN_PATHS.settings.states
    );
    expect(wrapper.findAll(".design-specimen")).toHaveLength(12);

    const openButtons = wrapper
      .findAll("button")
      .filter(b => b.text().includes("打开弹窗"));
    expect(openButtons.length).toBe(2);

    await openButtons[0].trigger("click");
    expect(bodyText()).toContain("有未保存的更改");
    const closeKeep = bodyButtons().find(b =>
      b.textContent?.includes("继续编辑")
    );
    await new DOMWrapper(closeKeep!).trigger("click");
    expect(bodyText()).not.toContain("有未保存的更改");

    await openButtons[1].trigger("click");
    expect(bodyText()).toContain("归档该项目？");
  });

  it("组件库页：14 组件索引齐备", async () => {
    const wrapper = await mountAt(
      SettingsComponentsView,
      DESIGN_PATHS.settings.components
    );
    expect(wrapper.findAll(".scomponents__index")).toHaveLength(14);
    expect(wrapper.text()).toContain("Settings/SaveBar");
  });
});
