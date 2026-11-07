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
    // 仓库管理已迁入 Project Setting（原 /project/repository/overview 现为重定向），
    // 改用仍是占位的导航项（AI 执行中心）验证标题渲染。
    await router.push("/project/execution");
    await nextTick();

    expect(wrapper.find(".shell-page-header__title").text()).toBe(
      "AI 执行中心"
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
