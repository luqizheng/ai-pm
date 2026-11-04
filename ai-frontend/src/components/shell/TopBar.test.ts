// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { nextTick } from "vue";
import { createMemoryHistory, createRouter } from "vue-router";
import { DOMWrapper, flushPromises } from "@vue/test-utils";

import TopBar from "./TopBar.vue";
import { demoProject, demoProjects } from "./demo";
import { buildRoutes } from "@/router";
import { THEME_STORAGE_KEY } from "@/lib/theme";
import { useShellStore } from "@/stores/shell";
import { mountShell } from "./test-utils";

/**
 * 顶栏行为（Application Shell §2A）。
 */
describe("TopBar", () => {
  it("渲染产品名与租户上下文指示（Global/TenantIndicator，只读）", async () => {
    const { wrapper } = mountShell(TopBar);
    await nextTick();

    expect(wrapper.find(".shell-topbar__product").text()).toBe(
      "AI R&D Workspace"
    );
    const tenant = wrapper.find(".tenant-indicator");
    expect(tenant.exists()).toBe(true);
    expect(tenant.text()).toContain("Acme Research");
  });

  it("项目芯片显示当前项目名（取自 store 的项目上下文）", async () => {
    const { wrapper } = mountShell(TopBar);
    const shell = useShellStore();
    shell.setProjects(demoProjects);
    shell.setCurrentProject(demoProject.id);
    await nextTick();

    // 06 板 ProjectSwitcher Trigger：选中态 accent 底 + 当前项目名
    const trigger = wrapper.find(".ps-trigger");
    expect(trigger.classes().join(" ")).toContain("is-selected");
    expect(trigger.find(".ps-trigger__label").text()).toBe("Atlas Platform");
  });

  it("未选项目时触发器显示「选择项目」，无选中态", async () => {
    const { wrapper } = mountShell(TopBar);
    const shell = useShellStore();
    shell.setProjects(demoProjects);
    await nextTick();

    const trigger = wrapper.find(".ps-trigger");
    expect(trigger.classes().join(" ")).not.toContain("is-selected");
    expect(trigger.find(".ps-trigger__label").text()).toBe("选择项目");
  });

  it("菜单经 Teleport 挂 body：搜索行 + 当前项徽标 + 分区头（06 板）", async () => {
    const { wrapper } = mountShell(TopBar, { attachTo: document.body });
    const shell = useShellStore();
    shell.setProjects(demoProjects);
    shell.setCurrentProject(demoProject.id);
    await nextTick();

    await wrapper.find(".ps-trigger").trigger("click");
    await nextTick();

    expect(document.body.querySelector(".ps-menu")).toBeTruthy();
    // 面板标题（i18n）+ 搜索行占位（input 属性，非 textContent）+ 当前项徽标
    expect(document.body.textContent).toContain("项目切换");
    expect(
      document.body.querySelector('.ps-menu input[placeholder="搜索可用项目…"]')
    ).toBeTruthy();
    expect(document.body.textContent).toContain("当前");
    expect(document.body.textContent).toContain("Atlas Platform");

    wrapper.unmount();
    document.body.innerHTML = "";
  });

  it("搜索入口为按钮并带无障碍名称", async () => {
    const { wrapper } = mountShell(TopBar);
    await nextTick();

    const search = wrapper.find(".shell-topbar__search");
    expect(search.attributes("aria-label")).toBe("搜索或跳转到…");
  });

  it("点击搜索入口发出 search 事件", async () => {
    const { wrapper } = mountShell(TopBar);
    await nextTick();

    await wrapper.find(".shell-topbar__search").trigger("click");
    expect(wrapper.emitted("search")).toBeTruthy();
  });

  it("通知入口渲染铃铛按钮（Global/Notification 下拉触发器）", async () => {
    const { wrapper } = mountShell(TopBar);
    await nextTick();

    const bell = wrapper.find(".notification-bell__button");
    expect(bell.exists()).toBe(true);
    expect(bell.attributes("aria-label")).toBe("通知");
  });

  it("主题开关紧随铃铛：单击写 html data-theme 并持久化（深/浅往返）", async () => {
    localStorage.removeItem(THEME_STORAGE_KEY);
    document.documentElement.removeAttribute("data-theme");
    const { wrapper } = mountShell(TopBar);
    await nextTick();

    const toggle = wrapper.find(".theme-toggle");
    expect(toggle.exists()).toBe(true);
    // 位置承诺：铃铛（NotificationDropdown 渲染根）旁边
    expect(
      toggle.element.previousElementSibling?.classList.contains(
        "notification-bell"
      )
    ).toBe(true);

    // 默认深色 → 单击转浅色并持久化；再击回深色（往返）
    expect(toggle.attributes("aria-label")).toBe("切换到浅色模式");
    await toggle.trigger("click");
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("light");

    await toggle.trigger("click");
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("dark");
  });

  it("账户入口渲染头像触发按钮，帮助入口渲染触发按钮", async () => {
    const { wrapper } = mountShell(TopBar);
    await nextTick();

    expect(wrapper.find(".user-menu-button").attributes("aria-label")).toBe(
      "账户菜单"
    );
    expect(wrapper.find(".help-menu__trigger").attributes("aria-label")).toBe(
      "帮助"
    );
  });

  it("账户菜单「账户设置」跳转设置域落地页（UserMenu → /settings/*）", async () => {
    // onAccountSelect 将菜单 id 映射到 /settings/*（Settings Framework 个人域）；
    // /settings 经路由重定向落在个人域落地页。
    // mountShell 第三参注入同一 router（options.plugins 会装第二个 → $route 冲突）
    const router = createRouter({
      history: createMemoryHistory(),
      routes: buildRoutes()
    });
    await router.push("/dashboard");
    await router.isReady();

    const { wrapper } = mountShell(TopBar, { attachTo: document.body }, router);
    await nextTick();

    // 浮层经 Portal 挂 body：打开账户菜单后点「账户设置」
    await wrapper.find(".user-menu-button").trigger("click");
    await nextTick();
    const settings = Array.from(
      document.body.querySelectorAll<HTMLButtonElement>(".user-menu-item")
    ).find(b => b.textContent?.includes("账户设置"));
    expect(settings).toBeTruthy();
    await new DOMWrapper(settings!).trigger("click");
    await flushPromises();

    expect(router.currentRoute.value.path).toBe("/settings/personal");

    wrapper.unmount();
    document.body.innerHTML = "";
  });
});
