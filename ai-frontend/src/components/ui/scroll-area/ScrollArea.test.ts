// @vitest-environment happy-dom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { ScrollArea } from "./index";

const source = readFileSync(resolve(__dirname, "ScrollArea.vue"), "utf8");

describe("ScrollArea", () => {
  it("渲染滚动容器与可聚焦的视口", () => {
    const w = mount(ScrollArea, { slots: { default: "<p>内容</p>" } });

    const root = w.element as HTMLElement;
    expect(root.classList.contains("ui-scroll-area")).toBe(true);

    const viewport = w.find("[data-reka-scroll-area-viewport]");
    expect(viewport.exists()).toBe(true);
    // 视口可聚焦，键盘可滚动
    expect(viewport.attributes("tabindex")).toBe("0");
    expect(viewport.text()).toContain("内容");
  });

  it("视口是普通 overflow:auto 容器（指示交给全局虚拟滚动条）", () => {
    // 2026-11-04 全局虚拟滚动条裁决：组件不再渲染自带 reka 滚动条，
    // 视口会被 src/lib/virtual-scrollbar.ts 自动跟踪并在 hover/滚动时浮出 thumb
    expect(source).not.toContain("ScrollAreaScrollbar");
    expect(source).not.toContain("ScrollAreaThumb");
    expect(source).toContain("overflow: auto");
  });

  it("不再有取向 prop 与 force-mount（全局体系双轴自动）", () => {
    expect(source).not.toMatch(/orientation\?\s*:/);
    expect(source).not.toContain("force-mount");
  });
});
