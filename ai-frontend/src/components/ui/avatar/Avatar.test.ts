// @vitest-environment happy-dom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { Avatar } from "./index";

const source = readFileSync(resolve(__dirname, "Avatar.vue"), "utf8");

/**
 * reka-ui 的 AvatarImage 用 `new window.Image()` 预加载并监听 load / error，
 * happy-dom 不会真的发起请求，这里用可控替身模拟两种结局。
 */
class FakeImage {
  static status: "load" | "error" = "load";
  complete = false;
  naturalWidth = 0;
  referrerPolicy = "";
  crossOrigin: string | null = null;
  private currentSrc = "";
  private listeners = new Map<string, Array<(event: Event) => void>>();

  get src(): string {
    return this.currentSrc;
  }

  set src(value: string) {
    this.currentSrc = value;
    setTimeout(() => {
      if (FakeImage.status === "load") {
        this.complete = true;
        this.naturalWidth = 64;
        this.emit("load");
      } else {
        this.complete = true;
        this.naturalWidth = 0;
        this.emit("error");
      }
    }, 0);
  }

  addEventListener(type: string, handler: (event: Event) => void): void {
    const list = this.listeners.get(type) ?? [];
    list.push(handler);
    this.listeners.set(type, list);
  }

  removeEventListener(type: string, handler: (event: Event) => void): void {
    const list = this.listeners.get(type) ?? [];
    this.listeners.set(
      type,
      list.filter(item => item !== handler)
    );
  }

  private emit(type: string): void {
    for (const handler of this.listeners.get(type) ?? []) {
      handler(new Event(type));
    }
  }
}

const originalImage = window.Image;

afterEach(() => {
  window.Image = originalImage;
  FakeImage.status = "load";
});

async function flush() {
  await nextTick();
  await new Promise(r => setTimeout(r, 5));
  await nextTick();
}

describe("Avatar", () => {
  it("渲染 28px 圆形头像容器", () => {
    const w = mount(Avatar, { props: { name: "Ada Lovelace" } });
    const root = w.element as HTMLElement;
    expect(root.tagName).toBe("SPAN");
    expect(root.classList.contains("ui-avatar")).toBe(true);
  });

  it("图片加载成功时显示图片、隐藏回退", async () => {
    window.Image = FakeImage as unknown as typeof Image;
    const w = mount(Avatar, { props: { src: "/a.png", name: "Ada Lovelace" } });
    await flush();

    expect(w.find("img").exists()).toBe(true);
    expect(w.find("img").attributes("style") ?? "").not.toContain(
      "display: none"
    );
    expect(w.find(".ui-avatar__fallback").exists()).toBe(false);
  });

  it("图片加载失败时回退到首字母", async () => {
    window.Image = FakeImage as unknown as typeof Image;
    FakeImage.status = "error";
    const w = mount(Avatar, {
      props: { src: "/broken.png", name: "Ada Lovelace" }
    });
    await flush();

    expect(w.find("img").attributes("style") ?? "").toContain("display: none");
    const fallback = w.find(".ui-avatar__fallback");
    expect(fallback.exists()).toBe(true);
    // Figma Initials 态：首词 + 末词首字母，最多两位
    expect(fallback.text()).toBe("AL");
  });

  it("没有图片时直接渲染首字母回退", () => {
    const w = mount(Avatar, { props: { name: "grace hopper" } });
    expect(w.find(".ui-avatar__fallback").text()).toBe("GH");
    expect(w.find("img").exists()).toBe(false);
  });

  it("支持自定义回退内容（slot）", () => {
    const w = mount(Avatar, {
      props: { name: "Ada Lovelace" },
      slots: { fallback: "AL" }
    });
    expect(w.find(".ui-avatar__fallback").text()).toBe("AL");
  });

  it("提供 label 时以其作为头像的可访问名称，且回退首字母退化为装饰", () => {
    const w = mount(Avatar, {
      props: { name: "Ada Lovelace", label: "Ada Lovelace" }
    });
    const root = w.element as HTMLElement;
    expect(root.getAttribute("aria-label")).toBe("Ada Lovelace");
    expect(w.find(".ui-avatar__fallback").attributes("aria-hidden")).toBe(
      "true"
    );
  });

  it("三档尺寸、圆形与配色全部来自 token（Figma 28/36/44 = 控件三档）", () => {
    expect(source).toContain("var(--radius-full)");
    expect(source).toContain("var(--surface-raised)");
    expect(source).toContain("var(--border-default)");
    expect(source).toContain("var(--size-control-sm)");
    expect(source).toContain("var(--size-control-md)");
    expect(source).toContain("var(--size-control-lg)");
    expect(source).not.toMatch(/28px|36px|44px/);
  });
});
