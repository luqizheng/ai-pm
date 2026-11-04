// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import {
  createMemoryHistory,
  createRouter,
  createRouter as _cr
} from "vue-router";
import { createPinia, setActivePinia } from "pinia";

import NotificationsView from "./NotificationsView.vue";
import { buildRoutes } from "@/router";
import { useNotificationsStore } from "@/stores/notifications";
import { i18n } from "@/lib/i18n";

/** 通知中心页：中心 + 选中记录的详情面板联动。 */
describe("NotificationsView", () => {
  it("渲染通知中心；点击记录后出现详情面板", async () => {
    document.body.innerHTML = "";

    const pinia = createPinia();
    setActivePinia(pinia);
    const store = useNotificationsStore();
    store.records = (
      await import("@/components/global/notification/demo")
    ).demoNotificationRecords;
    store.status = "ready";

    const router = createRouter({
      history: createMemoryHistory(),
      routes: buildRoutes()
    });

    const wrapper = mount(NotificationsView, {
      attachTo: document.body,
      global: { plugins: [pinia, i18n, router] }
    });
    await nextTick();

    expect(wrapper.find(".notification-center").exists()).toBe(true);
    expect(wrapper.find(".notification-detail").exists()).toBe(false);

    await wrapper
      .findAllComponents({ name: "NotificationItem" })
      .at(0)!
      .trigger("click");
    await nextTick();

    expect(wrapper.find(".notification-detail").exists()).toBe(true);
    expect(wrapper.find(".notification-detail__subject").text()).toBe(
      "Decision requested on REQ-142"
    );
  });
});
