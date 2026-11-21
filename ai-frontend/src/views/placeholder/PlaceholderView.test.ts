// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { nextTick } from "vue";

import PlaceholderView from "./PlaceholderView.vue";
import { mountShell } from "@/components/shell/test-utils";

/**
 * 占位视图行为：标题取自路由 meta 的 i18n key。
 */
describe("PlaceholderView", () => {
  it("渲染路由标题与建设中提示", async () => {
    const { wrapper, router } = mountShell(PlaceholderView);
    // /project/execution 已是实页（S05），用仍是占位的导航项
    // （Product Cognition，auxiliary 源 PLANNED 占位路由）验证标题渲染。
    await router.push("/project/product-cognition");
    await nextTick();

    expect(wrapper.find(".shell-page-header__title").text()).toBe(
      "产品认知"
    );
    expect(wrapper.find(".shell-placeholder__empty-title").text()).toBe(
      "该页面正在建设中"
    );
  });

  it("未知路径渲染 404 标题", async () => {
    const { wrapper, router } = mountShell(PlaceholderView);
    await router.push("/no-such-page");
    await nextTick();

    expect(wrapper.find(".shell-page-header__title").text()).toBe("页面不存在");
  });
});
