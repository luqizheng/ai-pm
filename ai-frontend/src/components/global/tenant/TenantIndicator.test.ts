// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";

import TenantIndicator from "./TenantIndicator.vue";
import TenantIdentityCard from "./TenantIdentityCard.vue";
import { i18n } from "@/lib/i18n";

/**
 * 租户上下文指示器：只读芯片 + Tooltip + 身份卡四态。
 * 红线：不含切换/管理入口（无任何可切换交互元素）。
 */
describe("TenantIndicator / TenantIdentityCard", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
  });

  it("芯片：渲染组织名 + tooltip 标签（组织 · 组织名 · 身份）", () => {
    const wrapper = mount(TenantIndicator, {
      props: { tenant: "Acme Research", identity: "Alex Morgan" },
      attachTo: document.body,
      global: { plugins: [i18n] }
    });

    expect(wrapper.find(".tenant-indicator__name").text()).toBe(
      "Acme Research"
    );
    expect(wrapper.find(".tenant-indicator").attributes("aria-label")).toBe(
      "组织 · Acme Research · Alex Morgan"
    );
  });

  it("加载态：芯片显示加载文案", () => {
    const wrapper = mount(TenantIndicator, {
      props: { tenant: "Acme Research", status: "loading" },
      attachTo: document.body,
      global: { plugins: [i18n] }
    });

    expect(wrapper.find(".tenant-indicator__loading").text()).toBe(
      "正在加载当前身份…"
    );
  });

  it("点击芯片打开身份卡浮层（Full 态：组织串 + 身份邮箱）", async () => {
    const wrapper = mount(TenantIndicator, {
      props: {
        tenant: "Acme Research",
        identity: "Alex Morgan",
        email: "alex.morgan@acme.dev"
      },
      attachTo: document.body,
      global: { plugins: [i18n] }
    });
    await nextTick();

    await wrapper.find(".tenant-indicator").trigger("click");
    await nextTick();

    const card = document.querySelector(".tenant-card");
    expect(card).not.toBeNull();
    expect(card?.textContent).toContain("组织 · Acme Research");
    expect(card?.textContent).toContain("alex.morgan@acme.dev");
  });

  it("身份卡 Full + 只读徽标", () => {
    const wrapper = mount(TenantIdentityCard, {
      props: { tenant: "Acme Research", email: "svc@acme.dev", readOnly: true },
      global: { plugins: [i18n] }
    });

    expect(wrapper.text()).toContain("svc@acme.dev");
    expect(wrapper.find(".ui-badge").text()).toBe("只读身份");
  });

  it("身份卡 Compact：无邮箱时不渲染身份行与徽标", () => {
    const wrapper = mount(TenantIdentityCard, {
      props: { tenant: "Acme Research" },
      global: { plugins: [i18n] }
    });

    expect(wrapper.text()).toContain("组织 · Acme Research");
    expect(wrapper.find(".ui-badge").exists()).toBe(false);
  });

  it("身份卡 Loading：提示 + 骨架行；Unavailable：说明文案", () => {
    const loading = mount(TenantIdentityCard, {
      props: { tenant: "Acme Research", status: "loading" },
      global: { plugins: [i18n] }
    });
    expect(loading.findAll(".tenant-card__skeleton-row").length).toBe(2);

    const unavailable = mount(TenantIdentityCard, {
      props: { tenant: "Acme Research", status: "unavailable" },
      global: { plugins: [i18n] }
    });
    expect(unavailable.text()).toContain("当前身份信息不可用。");
  });
});
