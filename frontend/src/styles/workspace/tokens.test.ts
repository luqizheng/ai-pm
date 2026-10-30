import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * workspace token 层契约测试。
 *
 * tokens.css 是工作区视觉的唯一真源：本测试锁住
 *   1. 深浅两主题块的变量完整性（设计系统 V1.0 §1.1 + 分析稿补缺）；
 *   2. WCAG AA 对比度（正文 ≥4.5:1，语义色 UI 用途 ≥3:1，语义色文字变体 ≥4.5:1）；
 *   3. `.workspace-shell` 作用域内的 --prism-* 桥接块（工作区深色下复用 PRISM 组件）。
 */
const cssPath = resolve(process.cwd(), "src/styles/workspace/tokens.css");
// 去注释后再定位选择器：头部说明注释里也出现了 `[data-theme="light"]` 字样
const css = readFileSync(cssPath, "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

function block(selector: string): string {
  const start = css.indexOf(selector);
  expect(start, `找不到选择器 ${selector}`).toBeGreaterThanOrEqual(0);
  const open = css.indexOf("{", start);
  let depth = 1;
  let i = open + 1;
  while (depth > 0 && i < css.length) {
    if (css[i] === "{") depth++;
    if (css[i] === "}") depth--;
    i++;
  }
  return css.slice(open + 1, i - 1);
}

function varsOf(blockBody: string): Map<string, string> {
  const map = new Map<string, string>();
  for (const m of blockBody.matchAll(/--([a-z0-9-]+)\s*:\s*([^;]+);/g)) {
    // 统一剥掉 ws- 前缀存取；桥接变量（--surface-base 等）本就无前缀
    map.set(m[1].replace(/^ws-/, ""), m[2].trim());
  }
  return map;
}

const dark = varsOf(block(":root"));
const light = varsOf(block('[data-theme="light"]'));

/** 相对亮度（WCAG 2.x） */
function luminance(hex: string): number {
  const m = hex.trim().match(/^#([0-9a-fA-F]{6})$/);
  expect(m, `非 6 位 hex：${hex}`).toBeTruthy();
  const n = parseInt(m![1], 16);
  const chan = [(n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff].map(v => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
  });
  return 0.2126 * chan[0] + 0.7152 * chan[1] + 0.0722 * chan[2];
}

function contrast(fg: string, bg: string): number {
  const l1 = luminance(fg);
  const l2 = luminance(bg);
  return (Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05);
}

function hexOf(vars: Map<string, string>, name: string): string {
  const v = vars.get(name);
  expect(v, `变量 --ws-${name} 未定义`).toBeTruthy();
  const m = v!.match(/#[0-9a-fA-F]{6}/);
  expect(m, `--ws-${name} 不含 hex 值：${v}`).toBeTruthy();
  return m![0];
}

describe("workspace tokens", () => {
  it("tokens.css 文件存在", () => {
    expect(existsSync(cssPath)).toBe(true);
  });

  it("深浅两主题块都定义了全部表面/文本/AI 色变量", () => {
    const base = [
      "canvas",
      "sidebar",
      "surface",
      "elevated",
      "border",
      "text-primary",
      "text-secondary",
      "accent"
    ];
    for (const vars of [dark, light]) {
      for (const name of base) {
        expect(vars.has(name), `--ws-${name} 缺失`).toBe(true);
      }
    }
  });

  it("深浅两主题块都定义了四组语义色（含文字变体）", () => {
    const semantic = [
      "color-info",
      "color-success",
      "color-warning",
      "color-danger",
      "color-info-text",
      "color-success-text",
      "color-warning-text",
      "color-danger-text"
    ];
    for (const vars of [dark, light]) {
      for (const name of semantic) {
        expect(vars.has(name), `--ws-${name} 缺失`).toBe(true);
      }
    }
  });

  it("布局尺寸与密度 token 按 V1.0 §1.2/§二 定义（主题无关，:root 定义即继承）", () => {
    expect(dark.get("sidebar-width")).toBe("232px");
    expect(dark.get("sidebar-collapsed")).toBe("60px");
    expect(dark.get("context-width")).toBe("320px");
    expect(dark.get("topbar-height")).toBe("52px");
    expect(dark.get("radius-card")).toBe("8px");
    expect(dark.get("radius-dialog")).toBe("12px");
    expect(dark.get("font-size-body")).toBe("13px");
    expect(dark.get("font-size-caption")).toBe("12px");
    expect(dark.get("row-height")).toBe("36px");
    // 主题无关的 focus ring（键盘章：可见焦点样式）
    expect(dark.has("focus-ring")).toBe(true);
    expect(light.has("focus-ring")).toBe(true);
  });

  it("正文与次文字对画布/表面达 WCAG AA（≥4.5:1，深浅两套）", () => {
    for (const vars of [dark, light]) {
      const canvas = hexOf(vars, "canvas");
      const surface = hexOf(vars, "surface");
      const primary = hexOf(vars, "text-primary");
      const secondary = hexOf(vars, "text-secondary");
      expect(contrast(primary, canvas)).toBeGreaterThanOrEqual(4.5);
      expect(contrast(primary, surface)).toBeGreaterThanOrEqual(4.5);
      expect(contrast(secondary, canvas)).toBeGreaterThanOrEqual(4.5);
      expect(contrast(secondary, surface)).toBeGreaterThanOrEqual(4.5);
    }
  });

  it("语义色基准对表面 ≥3:1（UI 组件用途），文字变体 ≥4.5:1（AA）", () => {
    for (const vars of [dark, light]) {
      const surface = hexOf(vars, "surface");
      for (const name of ["info", "success", "warning", "danger"]) {
        expect(
          contrast(hexOf(vars, `color-${name}`), surface),
          `深浅主题 --ws-color-${name} 对表面`
        ).toBeGreaterThanOrEqual(3);
        expect(
          contrast(hexOf(vars, `color-${name}-text`), surface),
          `深浅主题 --ws-color-${name}-text 对表面`
        ).toBeGreaterThanOrEqual(4.5);
      }
    }
  });

  it("AI 强调色对表面 ≥4.5:1（选中态文字场景）", () => {
    for (const vars of [dark, light]) {
      expect(
        contrast(hexOf(vars, "accent"), hexOf(vars, "surface"))
      ).toBeGreaterThanOrEqual(4.5);
    }
  });

  it("深色为默认（:root 即深色），color-scheme 随主题声明", () => {
    expect(dark.get("canvas")!.toLowerCase()).toBe("#111318");
    expect(light.get("canvas")!.toLowerCase()).toBe("#f7f8fa");
    expect(block(":root")).toMatch(/color-scheme:\s*dark/);
    expect(block('[data-theme="light"]')).toMatch(/color-scheme:\s*light/);
  });

  it(".workspace-shell 作用域内提供 --prism-* 桥接（复用 PRISM 组件在深色下正确显色）", () => {
    const bridged = [
      "surface-base",
      "surface-raised",
      "surface-sunken",
      "surface-overlay",
      "text-primary",
      "text-secondary",
      "text-tertiary",
      "border-default",
      "border-subtle",
      "accent",
      "accent-hover",
      "accent-pressed",
      "accent-subtle",
      "accent-contrast-text",
      "info",
      "success",
      "warning",
      "danger"
    ];
    const shellIdx = css.indexOf(".workspace-shell");
    expect(shellIdx, "缺少 .workspace-shell 桥接块").toBeGreaterThanOrEqual(0);
    const scope = css.slice(
      shellIdx,
      css.indexOf("}", css.indexOf("{", shellIdx))
    );
    for (const name of bridged) {
      expect(
        scope.includes(`--${name}:`),
        `桥接块缺少 --${name}（应映射到 var(--ws-…)）`
      ).toBe(true);
    }
  });
});
