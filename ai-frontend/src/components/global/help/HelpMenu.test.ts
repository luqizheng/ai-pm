// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";

import HelpMenu from "./HelpMenu.vue";
import { i18n } from "@/lib/i18n";

/** 帮助菜单（Global/Help/Menu 360 面板）：三项入口事件。 */
describe("HelpMenu", () => {
  function mountMenu() {
    return mount(HelpMenu, {
      attachTo: document.body,
      global: { plugins: [i18n] }
    });
  }

  function panelButtons() {
    return Array.from(
      document.querySelectorAll<HTMLButtonElement>(".help-menu button")
    );
  }

  beforeEach(() => {
    document.body.innerHTML = "";
  });

  it("打开：标题 + 三项（文档/快捷键/反馈）", async () => {
    const wrapper = mountMenu();
    await nextTick();

    await wrapper.find(".help-menu__trigger").trigger("click");
    await nextTick();

    const labels = panelButtons()
      .filter(b => b.className.includes("user-menu-item"))
      .map(b => b.textContent?.trim());
    expect(labels).toEqual(["帮助与文档", "键盘快捷键", "反馈"]);
  });

  it("三个入口分别发出 documentation / shortcuts / feedback", async () => {
    const wrapper = mountMenu();
    await nextTick();
    await wrapper.find(".help-menu__trigger").trigger("click");
    await nextTick();

    const items = panelButtons().filter(b =>
      b.className.includes("user-menu-item")
    );
    items[0].click();
    await nextTick();
    expect(wrapper.emitted("documentation")).toBeTruthy();

    await wrapper.find(".help-menu__trigger").trigger("click");
    await nextTick();
    panelButtons()
      .filter(b => b.className.includes("user-menu-item"))[1]
      .click();
    await nextTick();
    expect(wrapper.emitted("shortcuts")).toBeTruthy();

    await wrapper.find(".help-menu__trigger").trigger("click");
    await nextTick();
    panelButtons()
      .filter(b => b.className.includes("user-menu-item"))[2]
      .click();
    await nextTick();
    expect(wrapper.emitted("feedback")).toBeTruthy();
  });
});
