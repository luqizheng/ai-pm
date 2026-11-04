// @vitest-environment happy-dom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { mount, type VueWrapper } from "@vue/test-utils";
import { nextTick } from "vue";
import { Switch } from "./index";

const wrappers: VueWrapper[] = [];

function mountSwitch(props: Record<string, unknown> = {}) {
  const wrapper = mount(Switch, {
    props: { ariaLabel: "开关", ...props },
    attachTo: document.body
  });
  wrappers.push(wrapper);
  return wrapper;
}

/** 取得 switch 根按钮 */
function rootOf(w: ReturnType<typeof mountSwitch>) {
  return w.find<HTMLElement>('[role="switch"]');
}

afterEach(() => {
  while (wrappers.length) wrappers.pop()?.unmount();
  document.body.innerHTML = "";
});

describe("Switch", () => {
  it("渲染为 role=switch，初始为关闭态", () => {
    const w = mountSwitch();
    const root = rootOf(w);
    expect(root.exists()).toBe(true);
    expect(root.attributes("aria-checked")).toBe("false");
    expect(root.attributes("data-state")).toBe("unchecked");
  });

  it("点击切换状态并抛出 update:modelValue", async () => {
    const w = mountSwitch();
    await rootOf(w).trigger("click");
    expect(w.emitted("update:modelValue")?.[0]).toEqual([true]);
  });

  it("受控模式下 modelValue 为真时 aria-checked=true", () => {
    const w = mountSwitch({ modelValue: true });
    expect(rootOf(w).attributes("aria-checked")).toBe("true");
    expect(rootOf(w).attributes("data-state")).toBe("checked");
  });

  it("键盘 Enter 可以切换", async () => {
    const w = mountSwitch();
    const root = rootOf(w);
    root.element.focus();
    await root.trigger("keydown", { key: "Enter" });
    expect(w.emitted("update:modelValue")?.[0]).toEqual([true]);
  });

  it("禁用态不可切换", async () => {
    const w = mountSwitch({ disabled: true });
    const root = rootOf(w);
    expect(root.attributes("data-disabled")).toBeDefined();
    expect(root.attributes("disabled")).toBeDefined();
    await root.trigger("click");
    expect(w.emitted("update:modelValue")).toBeUndefined();
  });

  it("滑块随开关状态变化 data-state", async () => {
    const w = mountSwitch();
    const thumb = w.find(".ui-switch__thumb");
    expect(thumb.attributes("data-state")).toBe("unchecked");
    await w.setProps({ modelValue: true });
    await nextTick();
    expect(w.find(".ui-switch__thumb").attributes("data-state")).toBe(
      "checked"
    );
  });

  it("提供读屏专用文本（sr-only）说明当前状态", async () => {
    const w = mountSwitch({ stateLabel: { on: "已开启", off: "已关闭" } });
    expect(w.find(".ui-sr-only").text()).toBe("已关闭");
    await w.setProps({ modelValue: true });
    await nextTick();
    expect(w.find(".ui-sr-only").text()).toBe("已开启");
  });

  it("必填时输出 aria-required", () => {
    const w = mountSwitch({ required: true });
    expect(rootOf(w).attributes("aria-required")).toBe("true");
  });

  it("可见标签通过 for/id 与开关关联", () => {
    const w = mountSwitch({ label: "开启通知" });
    const root = rootOf(w);
    const label = w.find("label");
    expect(label.text()).toBe("开启通知");
    expect(label.attributes("for")).toBe(root.attributes("id"));
    expect(root.attributes("aria-labelledby")).toBe(label.attributes("id"));
  });
});

/**
 * 几何契约 —— Figma DS/Forms/Switch（13:33107）。
 *
 * 缺陷史：轨道高度曾误用 --size-switch-thumb（14px），
 * 导致轨道被压成 32×14，滑块溢出轨道、1px 描边被滑块盖住，
 * 开启态渲染成一块紫色糊状（用户反馈「与设计图不一致、少了边框」）。
 *
 * happy-dom 不做布局计算，故此处锁定样式源码中的几何取值。
 */
const sfc = readFileSync(
  resolve(process.cwd(), "src/components/ui/switch/Switch.vue"),
  "utf8"
);
const tokens = readFileSync(
  resolve(process.cwd(), "src/styles/tokens.css"),
  "utf8"
);

/** 取样式块中某选择器的规则体 */
function ruleBody(selector: string): string {
  const styleStart = sfc.indexOf("<style");
  const css = sfc.slice(styleStart).replace(/\/\*[\s\S]*?\*\//g, "");
  const re = new RegExp(
    `${selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*\\{([^}]*)\\}`
  );
  const m = css.match(re);
  expect(m, `Switch.vue 缺少 ${selector} 规则`).toBeTruthy();
  return m![1];
}

describe("Switch 几何契约（Figma 13:33107）", () => {
  it("尺寸 token：md 轨道 32×20、lg 轨道 40×24、thumb 14/18、内缩 2px", () => {
    expect(tokens).toMatch(/--size-switch-track:\s*32px/);
    expect(tokens).toMatch(/--size-switch-track-h:\s*20px/);
    expect(tokens).toMatch(/--size-switch-track-lg:\s*40px/);
    expect(tokens).toMatch(/--size-switch-track-h-lg:\s*24px/);
    expect(tokens).toMatch(/--size-switch-thumb:\s*14px/);
    expect(tokens).toMatch(/--size-switch-thumb-lg:\s*18px/);
    expect(tokens).toMatch(/--size-switch-inset:\s*2px/);
  });

  it("轨道高度取轨道尺寸 token，不得复用 thumb 尺寸", () => {
    expect(ruleBody(".ui-switch__root")).toMatch(
      /height:\s*var\(--size-switch-track-h\)/
    );
    expect(ruleBody(".ui-switch__root--lg")).toMatch(
      /height:\s*var\(--size-switch-track-h-lg\)/
    );
    // 回归护栏：轨道高度曾误写为 --size-switch-thumb
    expect(ruleBody(".ui-switch__root")).not.toMatch(
      /height:\s*var\(--size-switch-thumb\b/
    );
    expect(ruleBody(".ui-switch__root--lg")).not.toMatch(
      /height:\s*var\(--size-switch-thumb-lg\)/
    );
  });

  it("滑块距轨道外沿 3px（Figma padding 3 = 1px 描边 + 2px 内缩）", () => {
    expect(ruleBody(".ui-switch__thumb")).toMatch(
      /left:\s*var\(--size-switch-inset\)/
    );
  });

  it("开启态行程按轨道/滑块/内缩/描边计算，不硬编码位移", () => {
    const travel = ruleBody(
      '.ui-switch__root[data-state="checked"] .ui-switch__thumb'
    );
    expect(travel).toMatch(
      /var\(--size-switch-track\) - var\(--size-switch-thumb\) -/
    );
    expect(travel).toMatch(/var\(--size-switch-inset\) \* 2/);
    expect(travel).toMatch(/var\(--border-1\) \* 2/);
    // 旧公式多减了 2px 描边，导致开启态滑块右侧留白 4px 而非 Figma 的 3px
    expect(travel).not.toMatch(/6px/);
  });

  it("开启态保留 1px 描边（Figma On 变体），焦点环 2px accent", () => {
    expect(ruleBody('.ui-switch__root[data-state="checked"]')).not.toMatch(
      /border-color/
    );
    expect(ruleBody(".ui-switch__root:focus-visible")).toMatch(
      /border-color:\s*var\(--accent\)/
    );
    expect(ruleBody(".ui-switch__root:focus-visible")).toMatch(
      /inset 0 0 0 var\(--border-1\) var\(--accent\)/
    );
  });

  it("开启 + 焦点：描边不转 accent（否则与 accent 底同色不可见），焦点环移到轨道外侧", () => {
    const rule = ruleBody(
      '.ui-switch__root[data-state="checked"]:focus-visible'
    );
    expect(rule).toMatch(/border-color:\s*var\(--border-strong\)/);
    expect(rule).toMatch(
      /box-shadow:\s*0 0 0 var\(--border-2\) var\(--accent\)/
    );
  });

  it("禁用态整体 0.4 不透明度（Figma Disabled 变体）", () => {
    expect(ruleBody(".ui-switch:has(.ui-switch__root[data-disabled])")).toMatch(
      /opacity:\s*var\(--opacity-disabled-control\)/
    );
  });
});
