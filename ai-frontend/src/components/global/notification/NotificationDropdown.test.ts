// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { createPinia, setActivePinia } from "pinia";

import NotificationDropdown from "./NotificationDropdown.vue";
import { demoNotificationRecords } from "./demo";
import { useNotificationsStore } from "@/stores/notifications";
import { i18n } from "@/lib/i18n";

/**
 * 通知下拉（Global/Notification/Dropdown 五态）。
 * 浮层经 Portal 挂到 body，用 document 查询（仓库既有约定）；
 * 关闭断言要等 reka-ui Presence 卸载（下一帧）。
 */
describe("NotificationDropdown", () => {
  function seedStore(records = demoNotificationRecords) {
    const pinia = createPinia();
    setActivePinia(pinia);
    const store = useNotificationsStore();
    store.records = records;
    store.status = "ready";
    return { pinia, store };
  }

  function mountDropdown(pinia: ReturnType<typeof createPinia>) {
    return mount(NotificationDropdown, {
      attachTo: document.body,
      global: { plugins: [pinia, i18n] }
    });
  }

  function panel() {
    return document.querySelector<HTMLElement>(".notification-dropdown");
  }

  function panelButtons() {
    return Array.from(
      panel()?.querySelectorAll<HTMLButtonElement>("button") ?? []
    );
  }

  beforeEach(() => {
    document.body.innerHTML = "";
  });

  it("打开后按分类分组渲染记录，需人工处理的带徽标", async () => {
    const { pinia } = seedStore();
    const wrapper = mountDropdown(pinia);
    await nextTick();

    await wrapper.find(".notification-bell__button").trigger("click");
    await nextTick();

    const labels = Array.from(
      panel()?.querySelectorAll(".notification-dropdown__group-label") ?? []
    ).map(el => el.textContent?.trim());
    expect(labels).toContain("人工审批请求");
    expect(labels).toContain("人工任务指派");
    expect(labels).toContain("AI 执行动态");

    const badges = Array.from(
      panel()?.querySelectorAll(".ui-badge") ?? []
    ).map(el => el.textContent?.trim());
    expect(badges).toContain("需人工处理");
  });

  it("Unread 页签只显示未读记录", async () => {
    const { pinia } = seedStore();
    const wrapper = mountDropdown(pinia);
    await nextTick();

    await wrapper.find(".notification-bell__button").trigger("click");
    await nextTick();

    const unreadTab = panelButtons().find(b => b.textContent?.includes("未读"))!;
    expect(unreadTab.textContent?.trim()).toBe("未读 · 3");
    unreadTab.click();
    await nextTick();

    const records = panel()?.querySelectorAll(".notification-item");
    expect(records?.length).toBe(3);
  });

  it("全部标记已读：清空未读计数（铃铛徽标消失）", async () => {
    const { pinia, store } = seedStore();
    const wrapper = mountDropdown(pinia);
    await nextTick();

    expect(store.unreadCount).toBe(3);

    await wrapper.find(".notification-bell__button").trigger("click");
    await nextTick();

    panelButtons().find(b => b.textContent?.trim() === "全部标记已读")!.click();
    await nextTick();

    expect(store.unreadCount).toBe(0);
    expect(wrapper.find(".ui-badge").exists()).toBe(false);
  });

  it("打开对象发出 open（目标路由）并关闭浮层", async () => {
    const { pinia } = seedStore();
    const wrapper = mountDropdown(pinia);
    await nextTick();

    await wrapper.find(".notification-bell__button").trigger("click");
    await nextTick();

    panelButtons()
      .find(b => b.textContent?.trim() === "打开 REQ-142")!
      .click();
    await nextTick();

    expect(wrapper.emitted("open")?.[0]?.[0]).toBe("/project/requirements");

    await new Promise(resolve => setTimeout(resolve, 50));
    expect(panel()).toBeNull();
  });

  it("查看全部通知发出 view-all", async () => {
    const { pinia } = seedStore();
    const wrapper = mountDropdown(pinia);
    await nextTick();

    await wrapper.find(".notification-bell__button").trigger("click");
    await nextTick();

    panelButtons()
      .find(b => b.textContent?.trim() === "查看全部通知")!
      .click();
    await nextTick();

    expect(wrapper.emitted("view-all")).toBeTruthy();
  });

  it("空闲事件流：打开触发加载（Loading → Ready）", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const store = useNotificationsStore();
    expect(store.status).toBe("idle");

    const wrapper = mount(NotificationDropdown, {
      attachTo: document.body,
      global: {
        plugins: [
          pinia,
          i18n
        ]
      }
    });
    await nextTick();

    await wrapper.find(".notification-bell__button").trigger("click");
    await new Promise(resolve => setTimeout(resolve, 10));
    expect(store.status).toBe("loading");

    await new Promise(resolve => setTimeout(resolve, 300));
    expect(store.status).toBe("ready");
    expect(store.records.length).toBe(6);
  });

  it("空事件流：展示「已全部处理」空态", async () => {
    const { pinia } = seedStore([]);
    const wrapper = mountDropdown(pinia);
    await nextTick();

    await wrapper.find(".notification-bell__button").trigger("click");
    await nextTick();

    expect(panel()?.querySelector(".notification-dropdown__empty")).not.toBeNull();
    expect(panel()?.textContent).toContain("已全部处理");
  });

  it("加载失败：danger 反馈框 + 重试", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const store = useNotificationsStore();
    store.status = "error";

    const wrapper = mountDropdown(pinia);
    await nextTick();

    await wrapper.find(".notification-bell__button").trigger("click");
    await nextTick();

    expect(document.querySelector(".ui-alert")).not.toBeNull();
    expect(panel()?.textContent).toContain("通知不可用");

    // 重试走注入的成功 loader → ready
    store.load(async () => demoNotificationRecords);
    await new Promise(resolve => setTimeout(resolve, 10));
    expect(store.status).toBe("ready");
  });
});
