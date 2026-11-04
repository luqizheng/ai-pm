// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import {
  FILE_REF_ERROR_ATTR,
  HYDRATED_ATTR,
  buildAttachmentMarkdown,
  buildFileRef,
  buildImageMarkdown,
  hydrateFileRefs,
  isImageFile,
  parseFileRef
} from "./file-ref";

describe("文件引用约定", () => {
  it("按 id 生成正文引用路径", () => {
    expect(buildFileRef("f_01")).toBe("/v1/file/f_01");
  });

  it("从相对路径、绝对地址、带查询串的地址里取出 id", () => {
    expect(parseFileRef("/v1/file/abc")).toBe("abc");
    expect(parseFileRef("https://host/gw/v1/file/abc")).toBe("abc");
    expect(parseFileRef("/v1/file/abc?expires=1")).toBe("abc");
  });

  it("非文件引用一律返回 null", () => {
    expect(parseFileRef("/api/other")).toBeNull();
    expect(parseFileRef("")).toBeNull();
    expect(parseFileRef(null)).toBeNull();
  });

  it("生成图片片段并转义方括号", () => {
    expect(buildImageMarkdown("i1", "架构 [草图]")).toBe(
      "![架构 \\[草图\\]](/v1/file/i1)"
    );
  });

  it("生成附件片段并转义方括号", () => {
    expect(buildAttachmentMarkdown("f1", "季度[终]版.pdf")).toBe(
      "[季度\\[终\\]版.pdf](/v1/file/f1)"
    );
  });

  it("按 MIME 或扩展名判定图片", () => {
    expect(isImageFile(new File(["x"], "a.png", { type: "image/png" }))).toBe(
      true
    );
    expect(isImageFile(new File(["x"], "a.PNG"))).toBe(true);
    expect(
      isImageFile(new File(["x"], "a.pdf", { type: "application/pdf" }))
    ).toBe(false);
  });
});

describe("hydrateFileRefs", () => {
  function makeRoot(html: string): HTMLElement {
    const root = document.createElement("div");
    root.innerHTML = html;
    return root;
  }

  it("把图片与附件的引用换成临时地址", async () => {
    const root = makeRoot(
      '<img src="/v1/file/i1"><a href="/v1/file/f1">附件</a>'
    );
    const resolve = vi.fn(async (id: string) => `https://signed/${id}`);

    const count = await hydrateFileRefs(root, resolve);

    expect(count).toBe(2);
    expect(root.querySelector("img")?.getAttribute("src")).toBe(
      "https://signed/i1"
    );
    expect(root.querySelector("a")?.getAttribute("href")).toBe(
      "https://signed/f1"
    );
    expect(root.querySelector("img")?.getAttribute(HYDRATED_ATTR)).toBe("1");
  });

  it("同一 id 在一次渲染里只解析一次，重复调用不重复请求", async () => {
    const root = makeRoot('<img src="/v1/file/same"><img src="/v1/file/same">');
    const resolve = vi.fn(async (id: string) => `https://signed/${id}`);

    await hydrateFileRefs(root, resolve);
    expect(resolve).toHaveBeenCalledTimes(1);

    await hydrateFileRefs(root, resolve);
    expect(resolve).toHaveBeenCalledTimes(1);
  });

  it("解析失败时打错误标记并保留原引用", async () => {
    const root = makeRoot('<img src="/v1/file/missing">');
    const resolve = vi.fn(async () => null);

    const count = await hydrateFileRefs(root, resolve);

    expect(count).toBe(0);
    expect(root.querySelector("img")?.getAttribute(FILE_REF_ERROR_ATTR)).toBe(
      "1"
    );
    expect(root.querySelector("img")?.getAttribute("src")).toBe(
      "/v1/file/missing"
    );
  });

  it("解析端口抛错时不打断整批替换", async () => {
    const root = makeRoot('<img src="/v1/file/bad"><img src="/v1/file/good">');
    const resolve = vi.fn(async (id: string) => {
      if (id === "bad") throw new Error("boom");
      return `https://signed/${id}`;
    });

    const count = await hydrateFileRefs(root, resolve);

    expect(count).toBe(1);
    expect(root.querySelectorAll("img").item(1).getAttribute("src")).toBe(
      "https://signed/good"
    );
  });

  it("无文件引用时不做任何请求", async () => {
    const root = makeRoot("<p>纯文本</p>");
    const resolve = vi.fn(async () => "x");

    await expect(hydrateFileRefs(root, resolve)).resolves.toBe(0);
    expect(resolve).not.toHaveBeenCalled();
  });
});
