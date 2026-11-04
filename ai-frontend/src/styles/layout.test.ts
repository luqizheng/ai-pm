import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * 布局契约（用户验收反馈：栏位错位）。
 *
 * 根因：CSS Grid 自动放置中，「锁行不锁列」的项目先于全自动项目放置，
 * 右栏（grid-row:2）会抢在主区（全自动）前面拿第 2 列。
 * 契约：三个栏位子项必须显式声明 grid-column，不依赖自动放置。
 */
const css = readFileSync(
  resolve(process.cwd(), "src/styles/index.css"),
  "utf8"
).replace(/\/\*[\s\S]*?\*\//g, "");

function ruleBody(selector: string): string {
  const re = new RegExp(
    `${selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*\\{([^}]*)\\}`
  );
  const m = css.match(re);
  expect(m, `index.css 缺少 ${selector} 规则`).toBeTruthy();
  return m![1];
}

describe("workspace 布局契约", () => {
  it("shell 三列模板：左导航 | 自适应主区 | 右上下文", () => {
    const body = ruleBody(".workspace-shell");
    expect(body).toMatch(/display:\s*grid/);
    expect(body).toMatch(/grid-template-columns/);
    expect(body).toMatch(/var\(--ws-sidebar-current\)/);
    expect(body).toMatch(/minmax\(0,\s*1fr\)/);
    expect(body).toMatch(/var\(--ws-context-width\)/);
  });

  it("顶栏横跨全部列（首行）", () => {
    const body = ruleBody(".workspace-topbar");
    expect(body).toMatch(/grid-column:\s*1\s*\/\s*-1/);
    expect(body).toMatch(/grid-row:\s*1/);
  });

  it("左导航显式定位第 1 列", () => {
    const body = ruleBody(".workspace-leftnav");
    expect(body).toMatch(/grid-column:\s*1\s*;/);
    expect(body).toMatch(/grid-row:\s*2/);
  });

  it("主区显式定位第 2 列（中间列，不依赖自动放置）", () => {
    const body = ruleBody(".workspace-main");
    expect(body).toMatch(/grid-column:\s*2\s*;/);
    expect(body).toMatch(/grid-row:\s*2/);
  });

  it("右上下文显式定位第 3 列", () => {
    const body = ruleBody(".workspace-context");
    expect(body).toMatch(/grid-column:\s*3\s*;/);
    expect(body).toMatch(/grid-row:\s*2/);
  });

  it("滚动收敛在 main-scroll（内容不得渗出盖住指令栏，用户反馈）", () => {
    const main = ruleBody(".workspace-main");
    expect(main).toMatch(/overflow:\s*hidden/);
    const scroll = ruleBody(".workspace-main-scroll");
    expect(scroll).toMatch(/overflow-y:\s*auto/);
  });

  it("指令栏为停靠式通栏：主列底部 + 上边框，不遮挡内容也不压右栏", () => {
    const body = ruleBody(".workspace-commandbar");
    // 不使用 sticky/覆盖式定位——停靠在滚动区下方，天然不遮内容
    expect(body).not.toMatch(/position:\s*sticky/);
    expect(body).not.toMatch(/position:\s*absolute/);
    // Figma 两帧口径：停靠通栏（上边框分隔 + 与主列同背景），非浮动留边面板
    expect(body).toMatch(
      /border-top:\s*var\(--border-1\)\s+solid\s+var\(--ws-border\)/
    );
    expect(body).toMatch(/background:\s*var\(--ws-sidebar\)/);
    expect(body).toMatch(/flex:\s*none/);
    // 停靠式不再使用四周留边（那会与通栏通宽语义冲突）
    expect(body).not.toMatch(/margin:\s*0\s+24px\s+16px/);
  });

  it("全局盒模型为 border-box（指令栏 token 高度含 padding，否则整页溢出视口）", () => {
    // content-box 下：min-height 120px + padding 32px = 152px，
    // 叠加顶栏后超出 100dvh，页面出现外层滚动条、底栏被切。
    expect(css).toMatch(
      /\*,\s*\*::before,\s*\*::after\s*\{[^}]*box-sizing:\s*border-box/
    );
  });

  it("页面级重置：body margin 归零（默认 8px 会把 100dvh 的壳顶出视口）", () => {
    expect(css).toMatch(/html,\s*body\s*\{[^}]*margin:\s*0/);
  });
});
