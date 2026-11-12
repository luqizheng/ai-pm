// @vitest-environment happy-dom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * 组件样式契约测试。
 *
 * happy-dom 无法计算 scoped CSS 的最终样式，因此对"CSS 里必须出现的声明"
 * 采用源码契约断言 —— 它拦住的是"删掉一段样式，类名断言仍然全绿"这类回归。
 *
 * 契约基准（2026-11-04 起）= Figma 组件库
 * file=hNlGqLEMQpOVCZ0o5u3DkT page="13 — Component Library"。
 */
const UI_DIR = resolve(process.cwd(), "src/components/ui");

function source(...parts: string[]): string {
  return readFileSync(resolve(UI_DIR, ...parts), "utf8");
}

describe("组件样式契约（Figma 组件库数值必须来自 token）", () => {
  it("Button 的尺寸 / spinner / 时长 / 交互态色来自 token", () => {
    const s = source("button", "Button.vue");
    expect(s).toContain("var(--size-control-md)");
    expect(s).toContain("var(--size-control-sm)");
    expect(s).toContain("var(--size-control-lg)");
    expect(s).toContain("var(--size-icon-button)");
    expect(s).toContain("var(--duration-spinner)");
    expect(s).toContain("var(--size-spinner)");
    // Figma Primary hover #9B94FF / pressed #7971DE 走 token
    expect(s).toContain("var(--accent-hover)");
    expect(s).toContain("var(--accent-pressed)");
    // 禁用态不透明度走 token（0.38 档）
    expect(s).toContain("var(--opacity-disabled-button)");
    expect(s).not.toMatch(/\b\d+ms\b(?![^{]*\bvar\()/);
  });

  it("Button danger 变体使用 danger 反馈面三 token", () => {
    const s = source("button", "Button.vue");
    expect(s).toContain("var(--danger-surface)");
    expect(s).toContain("var(--danger-border)");
    expect(s).toContain("var(--danger-title)");
  });

  it("Button 在 reduced-motion 下停 spinner", () => {
    const s = source("button", "Button.vue");
    expect(s).toContain("prefers-reduced-motion");
    expect(s).toMatch(/animation:\s*none/);
  });

  it("Skeleton 的 shimmer 周期来自 token 且 reduced-motion 下静止", () => {
    const s = source("skeleton", "Skeleton.vue");
    expect(s).toContain("var(--duration-shimmer)");
    expect(s).toContain("prefers-reduced-motion");
    expect(s).toContain("animation: none");
  });

  it("Input / Textarea 的控件高度来自 token", () => {
    const input = source("input/Input.vue");
    expect(input).toContain("var(--size-control-md)");
    expect(input).toContain("var(--size-control-sm)");
    expect(input).not.toMatch(/height:\s*3[268]px/);

    // Figma Textarea 三档 72/88/112 走 --size-textarea* token
    const textarea = source("textarea/Textarea.vue");
    expect(textarea).toContain("var(--size-textarea)");
    expect(textarea).toContain("var(--size-textarea-sm)");
    expect(textarea).not.toMatch(/height:\s*\d+px/);
  });

  it("Input 错误态为 danger 高亮描边 + 内嵌警示图标（Figma Error 态）", () => {
    const s = source("input/Input.vue");
    expect(s).toContain("var(--danger-title)");
    expect(s).toContain("ui-input__icon--danger");
  });

  it("Input 聚焦为 accent 描边（Figma Focus：无光晕）", () => {
    const s = source("input/Input.vue");
    expect(s).toContain("var(--accent)");
    expect(s).not.toContain("var(--input-focus-glow)");
  });

  it("Input 密码类型切换 mono 字族（Figma PasswordInput）", () => {
    const s = source("input/Input.vue");
    expect(s).toContain("var(--font-mono)");
    expect(s).toContain("is-mono");
  });

  it("Table 行高 / 表头高 / Badge 高来自 token", () => {
    expect(source("table", "TableRow.vue")).toContain("var(--size-row)");
    // 表头单元格样式在 TableHead.vue（th 渲染器）——8ccdaacc 修正模板对调后
    // 的语义正确归属；TableHeader.vue 只做 thead 包装。
    expect(source("table", "TableHead.vue")).toContain(
      "var(--size-header-cell)"
    );
    expect(source("badge", "Badge.vue")).toContain("var(--size-badge)");
  });

  it("Table 表头字重为 450 档", () => {
    const s = source("table", "TableHead.vue");
    expect(s).toContain("var(--weight-caption)");
    expect(s).not.toContain("var(--weight-regular)");
  });

  it("Table 行 hover 时分隔线升到 border-default", () => {
    expect(source("table", "TableRow.vue")).toContain("var(--border-default)");
  });

  it("Table 的 caption 不依赖尚未引入的全局 .sr-only", () => {
    const s = source("table", "Table.vue");
    expect(s).not.toContain('class="sr-only"');
    expect(s).toContain("ui-sr-only");
  });
});
