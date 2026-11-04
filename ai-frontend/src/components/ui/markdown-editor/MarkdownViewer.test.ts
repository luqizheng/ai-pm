// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { mountShell } from "@/lib/test-utils";

const md2html = vi.hoisted(() => vi.fn());

vi.mock("vditor", () => ({
  default: { md2html, preview: vi.fn(), version: "test" }
}));

import MarkdownViewer from "./MarkdownViewer.vue";

const wrappers: Array<{ unmount: () => void }> = [];

afterEach(() => {
  while (wrappers.length) wrappers.pop()?.unmount();
  md2html.mockReset();
  document.body.innerHTML = "";
});

function mountViewer(props: Record<string, unknown> = {}) {
  const wrapper = mountShell(MarkdownViewer, { props });
  wrappers.push(wrapper);
  return wrapper;
}

async function flush(): Promise<void> {
  for (let i = 0; i < 3; i += 1) await nextTick();
  await new Promise(resolve => setTimeout(resolve, 0));
  for (let i = 0; i < 3; i += 1) await nextTick();
}

describe("MarkdownViewer", () => {
  it("把 Markdown 渲染成 HTML", async () => {
    md2html.mockResolvedValue("<h1>标题</h1>");

    const wrapper = mountViewer({ modelValue: "# 标题" });
    await flush();

    expect(wrapper.find(".md-viewer__body").html()).toContain("<h1>标题</h1>");
  });

  it("空内容不调用渲染", async () => {
    mountViewer({ modelValue: "   " });
    await flush();

    expect(md2html).not.toHaveBeenCalled();
  });

  it("渲染后把文件引用换成临时地址", async () => {
    md2html.mockResolvedValue(
      '<p><img src="/v1/file/i1"><a href="/v1/file/f1">报告.pdf</a></p>'
    );
    const resolveUrl = vi.fn(async (id: string) => `https://signed/${id}`);
    const wrapper = mountViewer({ modelValue: "x", resolveUrl });
    await flush();

    const img = wrapper.find("img");
    const link = wrapper.find("a");
    expect(img.attributes("src")).toBe("https://signed/i1");
    expect(link.attributes("href")).toBe("https://signed/f1");
  });

  it("未注入解析端口时保留原始引用", async () => {
    md2html.mockResolvedValue('<p><img src="/v1/file/i1"></p>');

    const wrapper = mountViewer({ modelValue: "x" });
    await flush();

    expect(wrapper.find("img").attributes("src")).toBe("/v1/file/i1");
  });

  it("渲染失败时降级为空视图而不抛错", async () => {
    md2html.mockRejectedValue(new Error("boom"));

    const wrapper = mountViewer({ modelValue: "# 标题" });
    await flush();

    expect(wrapper.find(".md-viewer__body").exists()).toBe(false);
  });

  it("正文变化后重新渲染", async () => {
    md2html.mockResolvedValue("<p>第一版</p>");
    const wrapper = mountViewer({ modelValue: "a" });
    await flush();

    md2html.mockResolvedValue("<p>第二版</p>");
    await wrapper.setProps({ modelValue: "b" });
    await flush();

    expect(wrapper.find(".md-viewer__body").html()).toContain("第二版");
  });
});
