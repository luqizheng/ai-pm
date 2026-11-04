// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { nextTick } from "vue";

import WorkspaceShell from "./WorkspaceShell.vue";
import { mountShell } from "./test-utils";

/**
 * 壳的无障碍不变量。
 *
 * 工具条与导航里的图标按钮/链接都带提示浮层；
 * 若浮层触发器再包一层 button，会形成嵌套交互元素（无效 HTML），
 * 且可访问名称重复朗读（「通知 通知」）。这里锁住该不变量。
 */
describe("shell 无障碍不变量", () => {
  it("不存在嵌套的交互元素（button 内不再出现 button）", async () => {
    const { wrapper } = mountShell(WorkspaceShell);
    await nextTick();

    const nested = wrapper.findAll("button button");
    expect(nested.length).toBe(0);

    const nestedLink = wrapper.findAll("button a");
    expect(nestedLink.length).toBe(0);
  });

  it("图标按钮的可访问名称不重复", async () => {
    const { wrapper } = mountShell(WorkspaceShell);
    await nextTick();

    const bell = wrapper
      .findAll("button")
      .find(el => el.attributes("aria-label") === "通知");
    expect(bell).toBeTruthy();
    // 触发器只有一个 button 元素（浮层不额外生成）
    expect(
      wrapper.findAll("button").filter(el => el.text().includes("通知")).length
    ).toBe(0);
  });

  it("展开态导航项以文案命名（可访问名称来自可见文本）", async () => {
    const { wrapper } = mountShell(WorkspaceShell);
    await nextTick();

    const link = wrapper
      .findAll("a")
      .find(el => el.attributes("href") === "/inbox");
    expect(link).toBeTruthy();
    expect(link!.text()).toContain("收件箱");
  });
});
