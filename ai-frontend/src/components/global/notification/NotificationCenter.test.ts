// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { createPinia, setActivePinia } from "pinia";

import NotificationCenter from "./NotificationCenter.vue";
import Select from "@/components/ui/select/Select.vue";
import { demoNotificationRecords } from "./demo";
import { useNotificationsStore } from "@/stores/notifications";
import { i18n } from "@/lib/i18n";

/**
 * 完整通知中心（Global/Notification/Center）：双过滤 + 记录数行 +
 * 选中态 + 标记已读联动。
 */
describe("NotificationCenter", () => {
  function mountCenter() {
    const pinia = createPinia();
    setActivePinia(pinia);
    const store = useNotificationsStore();
    store.records = demoNotificationRecords;
    store.status = "ready";

    return {
      wrapper: mount(NotificationCenter, {
        global: { plugins: [pinia, i18n] }
      }),
      store
    };
  }

  it("渲染过滤区 + 记录数行（6 条记录 · 3 条未读）", () => {
    const { wrapper } = mountCenter();

    expect(wrapper.find(".notification-center__count").text()).toBe(
      "全部分类 · 6 条记录 · 3 条未读"
    );
    expect(wrapper.findAll(".notification-record").length).toBe(6);
  });

  it("仅看未读：勾选后只显示 3 条未读", async () => {
    const { wrapper } = mountCenter();

    await wrapper.find("[role='checkbox']").trigger("click");
    await nextTick();

    expect(wrapper.findAll(".notification-record").length).toBe(3);
    expect(wrapper.find(".notification-center__count").text()).toContain(
      "3 条记录"
    );
  });

  it("分类过滤：切到系统警报只剩 1 条", async () => {
    const { wrapper } = mountCenter();

    await wrapper
      .findComponent(Select)
      .vm.$emit("update:modelValue", "systemAlerts");
    await nextTick();

    expect(wrapper.findAll(".notification-record").length).toBe(1);
    expect(wrapper.find(".notification-center__count").text()).toBe(
      "系统警报 · 1 条记录 · 0 条未读"
    );
  });

  it("点击记录置为选中态（accent 面 + accent 描边）", async () => {
    const { wrapper } = mountCenter();

    await wrapper
      .findAllComponents({ name: "NotificationItem" })
      .at(0)!
      .trigger("click");
    await nextTick();

    expect(wrapper.find(".notification-record").classes()).toContain(
      "is-selected"
    );
    expect(wrapper.vm.selectedId).toBe("ntf-req-142");
  });

  it("过滤组合后为空：展示「已全部处理」空态", async () => {
    const { wrapper } = mountCenter();

    await wrapper
      .findComponent(Select)
      .vm.$emit("update:modelValue", "systemAlerts");
    await wrapper.find("[role='checkbox']").trigger("click");
    await nextTick();

    expect(wrapper.find(".notification-center__empty").exists()).toBe(true);
    expect(wrapper.find(".notification-center__empty-title").text()).toBe(
      "已全部处理"
    );
  });
});
