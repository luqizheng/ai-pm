import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * 布局契约（用户验收反馈：栏位错位）。
 *
 * 根因：CSS Grid 自动放置中，「锁行不锁列」的项目先于全自动项目放置，
 * 右栏（grid-row:2）会抢在主区（全自动）前面拿第 2 列。
 * 契约：**四个**栏位子项必须显式声明 grid-column，不依赖自动放置。
 *
 * 四栏主体工作区（AI Collaboration V1.0 / DR-C5 + DR-C8，1440px 基准）：
 *   左导航 188px | 主区 518px（1fr，基准下正好 518）| AI Chat 446px | 右上下文 288px
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
  it("shell 四列模板：左导航 | 主区 | AI Chat | 右上下文", () => {
    const body = ruleBody(".workspace-shell");
    expect(body).toMatch(/display:\s*grid/);
    expect(body).toMatch(/grid-template-columns/);
    expect(body).toMatch(/var\(--ws-sidebar-current\)/);
    expect(body).toMatch(/minmax\(0,\s*1fr\)/);
    expect(body).toMatch(/var\(--ws-chat-width\)/);
    expect(body).toMatch(/var\(--ws-context-width\)/);
  });

  it("四列宽度合计 1440px（188 + 518 + 446 + 288）——token 层契约", () => {
    const tokens = readFileSync(
      resolve(process.cwd(), "src/styles/tokens.css"),
      "utf8"
    );
    const width = (name: string) => {
      const m = tokens.match(new RegExp(`--${name}:\\s*(\\d+)px`));
      expect(m, `tokens.css 缺少 --${name}`).toBeTruthy();
      return Number(m![1]);
    };
    expect(width("ws-sidebar-width")).toBe(188);
    expect(width("ws-main-width")).toBe(518);
    expect(width("ws-chat-width")).toBe(446);
    expect(width("ws-context-width")).toBe(288);
    // 1440px 桌面基准：188 + 446 + 288 = 922，主区 1fr 恰好补足 518
    expect(1440 - width("ws-sidebar-width") - width("ws-chat-width") - width("ws-context-width")).toBe(
      width("ws-main-width")
    );
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

  it("AI Chat 常驻列显式定位第 3 列", () => {
    const body = ruleBody(".workspace-chat");
    expect(body).toMatch(/grid-column:\s*3\s*;/);
    expect(body).toMatch(/grid-row:\s*2/);
    // 独立滚动：AI Chat 内部滚动，主区不被其内容撑开
    expect(body).toMatch(/overflow:\s*hidden/);
    expect(body).toMatch(/min-width:\s*0/);
  });

  it("右上下文显式定位第 4 列", () => {
    const body = ruleBody(".workspace-context");
    expect(body).toMatch(/grid-column:\s*4\s*;/);
    expect(body).toMatch(/grid-row:\s*2/);
  });

  it("专注模式（DR-C8）：收起 AI Chat 与右栏，主区独占", () => {
    const body = ruleBody('.workspace-shell[data-focus="on"]');
    expect(body).toMatch(/grid-template-columns/);
    expect(body).toMatch(/var\(--ws-sidebar-current\)/);
    expect(body).toMatch(/minmax\(0,\s*1fr\)/);
    // 专注模式不得保留 AI Chat / 右栏列位
    expect(body).not.toMatch(/var\(--ws-chat-width\)/);
    expect(body).not.toMatch(/var\(--ws-context-width\)/);
  });

  it("右栏关闭时保留 AI Chat 列（三列）", () => {
    const body = ruleBody('.workspace-shell[data-context="closed"]');
    expect(body).toMatch(/var\(--ws-chat-width\)/);
    expect(body).not.toMatch(/var\(--ws-context-width\)/);
  });

  it("滚动收敛在 main-scroll（内容不得渗出盖住底部元素，用户反馈）", () => {
    const main = ruleBody(".workspace-main");
    expect(main).toMatch(/overflow:\s*hidden/);
    const scroll = ruleBody(".workspace-main-scroll");
    expect(scroll).toMatch(/overflow-y:\s*auto/);
  });

  it("指令栏为 AI Chat 列内的输入组件：上边框分隔 + 非覆盖式定位", () => {
    const body = ruleBody(".workspace-commandbar");
    // 不使用 sticky/覆盖式定位——停靠在列底，天然不遮内容
    expect(body).not.toMatch(/position:\s*sticky/);
    expect(body).not.toMatch(/position:\s*absolute/);
    expect(body).toMatch(
      /border-top:\s*var\(--border-1\)\s+solid\s+var\(--ws-border\)/
    );
    expect(body).toMatch(/background:\s*var\(--ws-surface\)/);
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
