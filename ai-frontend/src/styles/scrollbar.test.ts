import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * 虚拟滚动条契约（2026-11-04 用户裁决：全系统 Figma 式 overlay 滚动条）。
 *
 * 三层分工：
 *   1. index.css 全局隐藏原生滚动条 + `.vsb-*` overlay thumb 样式；
 *   2. src/lib/virtual-scrollbar.ts 渲染与交互；
 *   3. tokens.css 提供 thumb 配色（深浅两主题）与厚度 token。
 * 任何一环缺失都会退回原生 Windows 滚动条（占位、突兀）或双指示。
 */
const indexCss = readFileSync(
  resolve(process.cwd(), "src/styles/index.css"),
  "utf8"
)
  .replace(/\/\*[\s\S]*?\*\//g, "")
  .replace(/\s+/g, " ");

const tokensCss = readFileSync(
  resolve(process.cwd(), "src/styles/tokens.css"),
  "utf8"
).replace(/\/\*[\s\S]*?\*\//g, "");

function ruleBody(css: string, selector: string): string {
  const re = new RegExp(
    `${selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*\\{([^}]*)\\}`
  );
  const m = css.match(re);
  expect(m, `缺少 ${selector} 规则`).toBeTruthy();
  return m![1];
}

/** 某选择器块内是否定义了某变量（按块切分，避免跨主题误匹配） */
function blockDefines(
  css: string,
  blockSelector: string,
  varName: string
): boolean {
  const re = new RegExp(
    `${blockSelector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*\\{([\\s\\S]*?)\\n\\}`
  );
  const m = css.match(re);
  return m ? m[1].includes(varName) : false;
}

describe("虚拟滚动条契约", () => {
  it("原生滚动条全量隐藏（scrollbar-width: none + webkit 伪元素清零）", () => {
    expect(ruleBody(indexCss, "*")).toMatch(/scrollbar-width:\s*none/);
    expect(ruleBody(indexCss, "*::-webkit-scrollbar")).toMatch(/width:\s*0/);
    expect(ruleBody(indexCss, "*::-webkit-scrollbar")).toMatch(/height:\s*0/);
  });

  it("thumb 固定层 z-index 高于全部浮层且不吃指针事件", () => {
    // 现役最高浮层是 ConfirmationDialog 的 z-index: 100
    const body = ruleBody(indexCss, ".vsb-layer");
    expect(body).toMatch(/position:\s*fixed/);
    expect(body).toMatch(/z-index:\s*120/);
    expect(body).toMatch(/pointer-events:\s*none/);
  });

  it("thumb 默认隐藏且连指针事件一起关（透明 thumb 不得挡住下层点击）", () => {
    const body = ruleBody(indexCss, ".vsb-thumb");
    expect(body).toMatch(/opacity:\s*0/);
    expect(body).toMatch(/pointer-events:\s*none/);
    // 配色只许来自 token（全应用禁裸色值）
    expect(body).toMatch(/background:\s*var\(--ws-scrollbar-thumb\)/);
  });

  it("thumb 显示态恢复指针事件（可拖拽），hover/拖拽加深", () => {
    const visible = ruleBody(indexCss, ".vsb-thumb.is-visible");
    expect(visible).toMatch(/opacity:\s*1/);
    expect(visible).toMatch(/pointer-events:\s*auto/);

    const active = ruleBody(
      indexCss,
      ".vsb-thumb:hover, .vsb-thumb.is-dragging"
    );
    expect(active).toMatch(/var\(--ws-scrollbar-thumb-active\)/);
  });

  it("thumb 厚度来自 token，禁裸 8px（主题无关尺寸收口在 tokens.css）", () => {
    expect(ruleBody(indexCss, ".vsb-thumb--vertical")).toMatch(
      /width:\s*var\(--ws-scrollbar-thickness\)/
    );
    expect(ruleBody(indexCss, ".vsb-thumb--horizontal")).toMatch(
      /height:\s*var\(--ws-scrollbar-thickness\)/
    );
    expect(indexCss).not.toMatch(/8px/);
  });

  it("tokens.css 深浅两主题都定义 thumb 配色，厚度为主题无关尺寸", () => {
    for (const varName of [
      "--ws-scrollbar-thumb:",
      "--ws-scrollbar-thumb-active:"
    ]) {
      expect(blockDefines(tokensCss, ":root", varName)).toBe(true);
      expect(blockDefines(tokensCss, '[data-theme="light"]', varName)).toBe(
        true
      );
    }
    expect(blockDefines(tokensCss, ":root", "--ws-scrollbar-thickness:")).toBe(
      true
    );
    expect(blockDefines(tokensCss, ":root", "--ws-scrollbar-min-thumb:")).toBe(
      true
    );
    // 浅色主题不改厚度（主题无关）
    expect(
      blockDefines(
        tokensCss,
        '[data-theme="light"]',
        "--ws-scrollbar-thickness:"
      )
    ).toBe(false);
  });
});
