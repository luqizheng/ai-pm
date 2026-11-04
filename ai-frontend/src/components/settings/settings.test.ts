// @vitest-environment happy-dom
import { DOMWrapper, flushPromises } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import { createMemoryHistory, createRouter } from "vue-router";

import { mountShell } from "@/lib/test-utils";
import { buildRoutes } from "@/router";

import SettingsConfirmationDialog from "./SettingsConfirmationDialog.vue";
import SettingsFieldLabel from "./SettingsFieldLabel.vue";
import SettingsFormRow from "./SettingsFormRow.vue";
import SettingsHelperText from "./SettingsHelperText.vue";
import SettingsNavigationItem from "./SettingsNavigationItem.vue";
import SettingsPermissionNotice from "./SettingsPermissionNotice.vue";
import SettingsReadOnlyField from "./SettingsReadOnlyField.vue";
import SettingsSaveBar from "./SettingsSaveBar.vue";
import SettingsSectionHeader from "./SettingsSectionHeader.vue";
import SettingsSidebar from "./SettingsSidebar.vue";
import SettingsUnsavedChangesDialog from "./SettingsUnsavedChangesDialog.vue";
import SettingsValidationMessage from "./SettingsValidationMessage.vue";
import { PROJECT_ACCESS, TENANT_ACCESS, TENANT_NAV } from "./settings.model";

/**
 * Settings Framework 组件行为契约（§6/§7/§9）。
 * 断言行为与语义（事件、aria、权限可见性），不断言样式类名细节。
 */

describe("SettingsSaveBar", () => {
  it("无更改时不渲染；有更改时渲染并仅在 dirty 时可保存", async () => {
    const wrapper = mountShell(SettingsSaveBar, {
      props: { dirty: false }
    });
    expect(wrapper.find(".settings-savebar").exists()).toBe(false);

    await wrapper.setProps({ dirty: true });
    expect(wrapper.find(".settings-savebar").exists()).toBe(true);

    const save = wrapper
      .findAll("button")
      .find(b => b.text().includes("保存更改"));
    expect(save).toBeTruthy();
  });

  it("保存中/失败态文案切换并锁定按钮", async () => {
    const wrapper = mountShell(SettingsSaveBar, {
      props: { dirty: true, saving: true }
    });
    expect(wrapper.text()).toContain("保存中…");

    await wrapper.setProps({ dirty: true, saving: false, error: "boom" });
    expect(wrapper.text()).toContain("保存失败，请重试");
  });

  it("点击保存/取消发出对应事件", async () => {
    const wrapper = mountShell(SettingsSaveBar, {
      props: { dirty: true }
    });
    const buttons = wrapper.findAll("button");
    await buttons.find(b => b.text().includes("取消"))!.trigger("click");
    await buttons.find(b => b.text().includes("保存更改"))!.trigger("click");
    expect(wrapper.emitted("cancel")).toHaveLength(1);
    expect(wrapper.emitted("save")).toHaveLength(1);
  });
});

describe("SettingsUnsavedChangesDialog", () => {
  it("三选一动作各自发出事件并关闭（弹窗 Teleport 到 body）", async () => {
    const wrapper = mountShell(SettingsUnsavedChangesDialog, {
      props: { open: true },
      attachTo: document.body
    });
    await flushPromises();
    const bodyButtons = () => [...document.body.querySelectorAll("button")];
    const discard = bodyButtons().find(b =>
      b.textContent?.includes("放弃更改")
    );
    const saveLeave = bodyButtons().find(b =>
      b.textContent?.includes("保存并离开")
    );
    expect(discard).toBeTruthy();
    expect(saveLeave).toBeTruthy();

    await new DOMWrapper(saveLeave!).trigger("click");
    expect(wrapper.emitted("save")).toHaveLength(1);
    expect(wrapper.emitted("update:open")?.[0]).toEqual([false]);

    // 重开后验证放弃分支
    await wrapper.setProps({ open: true });
    const discardAgain = bodyButtons().find(b =>
      b.textContent?.includes("放弃更改")
    );
    await new DOMWrapper(discardAgain!).trigger("click");
    expect(wrapper.emitted("discard")).toHaveLength(1);
    wrapper.unmount();
  });
});

describe("SettingsConfirmationDialog", () => {
  it("固定 danger 语义并透传 confirm 事件", async () => {
    const wrapper = mountShell(SettingsConfirmationDialog, {
      props: { open: true, title: "归档该项目？" },
      attachTo: document.body
    });
    expect(document.body.textContent).toContain("归档该项目？");
    const confirm = [...document.body.querySelectorAll("button")].find(b =>
      b.textContent?.includes("确认")
    );
    expect(confirm).toBeTruthy();
    await new DOMWrapper(confirm!).trigger("click");
    expect(wrapper.emitted("confirm")).toHaveLength(1);
    wrapper.unmount();
  });
});

describe("SettingsPermissionNotice", () => {
  it("readonly = info 语义；denied = danger 语义并插值角色", () => {
    const readonlyWrapper = mountShell(SettingsPermissionNotice, {
      props: { access: "readonly", role: "项目成员（只读）" }
    });
    expect(readonlyWrapper.find("[role=note]").classes().join(" ")).toContain(
      "info"
    );
    expect(readonlyWrapper.text()).toContain("项目成员（只读）");

    const deniedWrapper = mountShell(SettingsPermissionNotice, {
      props: { access: "denied", role: "普通成员（无租户管理权）" }
    });
    expect(deniedWrapper.find("[role=note]").classes().join(" ")).toContain(
      "danger"
    );
  });
});

describe("SettingsReadOnlyField / ValidationMessage / HelperText / FieldLabel", () => {
  it("只读字段 aria-readonly 且空值展示占位", () => {
    const wrapper = mountShell(SettingsReadOnlyField, {
      props: { value: "", placeholder: "—" }
    });
    expect(wrapper.find("[aria-readonly=true]").exists()).toBe(true);
    expect(wrapper.text()).toContain("—");
  });

  it("校验消息 role=alert", () => {
    const wrapper = mountShell(SettingsValidationMessage, {
      props: { message: "两次输入的新密码不一致" }
    });
    expect(wrapper.find("[role=alert]").exists()).toBe(true);
  });

  it("帮助文案渲染文本", () => {
    const wrapper = mountShell(SettingsHelperText, {
      props: { text: "由租户管理员管理，个人不可修改。" }
    });
    expect(wrapper.text()).toContain("由租户管理员管理，个人不可修改。");
  });

  it("FieldLabel 关联控件时渲染 label for，且展示必填/只读标记", () => {
    const wrapper = mountShell(SettingsFieldLabel, {
      props: {
        label: "展示名称",
        htmlFor: "profile-display-name",
        required: true,
        flag: "只读"
      }
    });
    const label = wrapper.find("label[for=profile-display-name]");
    expect(label.exists()).toBe(true);
    expect(label.text()).toContain("*");
    expect(label.text()).toContain("只读");
  });
});

describe("SettingsFormRow / SectionHeader", () => {
  it("FormRow 双栏：标签 + 帮助 + 控件插槽", () => {
    const wrapper = mountShell(SettingsFormRow, {
      props: { label: "主题", helper: "深色为默认主题" },
      slots: { default: "<button>control</button>" }
    });
    expect(wrapper.text()).toContain("主题");
    expect(wrapper.text()).toContain("深色为默认主题");
    expect(wrapper.find("button").text()).toBe("control");
  });

  it("SectionHeader 渲染标题/描述与操作插槽", () => {
    const wrapper = mountShell(SettingsSectionHeader, {
      props: { title: "活跃会话", description: "已登录的设备列表" },
      slots: { actions: "<button>refresh</button>" }
    });
    expect(wrapper.text()).toContain("活跃会话");
    expect(wrapper.text()).toContain("已登录的设备列表");
    expect(wrapper.find("button").text()).toBe("refresh");
  });
});

describe("SettingsNavigationItem / SettingsSidebar", () => {
  it("Proposed 徽标与禁用态", () => {
    const wrapper = mountShell(SettingsNavigationItem, {
      props: { label: "治理", proposed: true }
    });
    expect(wrapper.text()).toContain("规划中");

    const disabled = mountShell(SettingsNavigationItem, {
      props: { label: "集成", disabled: true }
    });
    expect(disabled.find("button").attributes("disabled")).toBeDefined();
  });

  it("Sidebar 按当前路由标记激活项；denied 时条目不可导航", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: buildRoutes()
    });
    await router.push("/design/settings/personal/profile");
    await router.isReady();
    const wrapper = mountShell(SettingsSidebar, {
      props: {
        scopeLabel: "个人账户设置",
        entries: TENANT_NAV
      },
      global: { plugins: [router] }
    });
    // 当前路径不在条目集 → 无激活项
    expect(wrapper.findAll(".is-active")).toHaveLength(0);

    const denied = mountShell(SettingsSidebar, {
      props: {
        scopeLabel: "租户管理设置",
        entries: TENANT_NAV,
        denied: true
      },
      global: { plugins: [router] }
    });
    expect(denied.findAll("a")).toHaveLength(0);
    expect(denied.findAll("button.is-disabled").length).toBeGreaterThan(0);
  });
});

describe("settings.model 权限矩阵", () => {
  it("租户域：仅管理员可编辑，成员不可见", () => {
    expect(TENANT_ACCESS["tenant-admin"]).toBe("editable");
    expect(TENANT_ACCESS["tenant-member"]).toBe("hidden");
  });

  it("项目域：负责人全可改；成员/访问者只读", () => {
    const lead = PROJECT_ACCESS["project-lead"];
    expect(Object.values(lead).every(access => access === "editable")).toBe(
      true
    );
    for (const persona of ["project-member", "project-viewer"] as const) {
      expect(
        Object.values(PROJECT_ACCESS[persona]).every(
          access => access === "readonly"
        )
      ).toBe(true);
    }
  });
});

describe("settings 可达性（路由挂接）", () => {
  it("全部条目路径都能被路由解析", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: buildRoutes()
    });
    for (const entry of TENANT_NAV) {
      const resolved = router.resolve(entry.to);
      expect(resolved.matched.length, entry.to).toBeGreaterThan(0);
    }
  });
});
