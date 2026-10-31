// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import {
  createUploadHandler,
  formatBytes,
  matchesAccept,
  toMarkdown
} from "./upload-handler";
import type { AttachmentMeta, AttachmentUploader } from "./types";

const messages = {
  tooLarge: (name: string, limit: string) => `too-large:${name}:${limit}`,
  rejected: (name: string) => `rejected:${name}`,
  failed: (name: string) => `failed:${name}`
};

function imageFile(name = "shot.png"): File {
  return new File(["x"], name, { type: "image/png" });
}

function pdfFile(name = "report.pdf"): File {
  return new File(["x"], name, { type: "application/pdf" });
}

function uploaderOf(meta: (file: File) => AttachmentMeta): AttachmentUploader {
  return vi.fn(async (file: File) => meta(file));
}

describe("formatBytes", () => {
  it("按量级给出可读体积", () => {
    expect(formatBytes(0)).toBe("0B");
    expect(formatBytes(1023)).toBe("1023B");
    expect(formatBytes(1024)).toBe("1.0KB");
    expect(formatBytes(100 * 1024 * 1024)).toBe("100.0MB");
  });

  it("非法输入返回空串", () => {
    expect(formatBytes(Number.NaN)).toBe("");
    expect(formatBytes(-1)).toBe("");
  });
});

describe("matchesAccept", () => {
  it("未配置 accept 时全部放行", () => {
    expect(matchesAccept(pdfFile(), undefined)).toBe(true);
    expect(matchesAccept(pdfFile(), "")).toBe(true);
  });

  it("支持通配 MIME、精确 MIME 与扩展名", () => {
    expect(matchesAccept(imageFile(), "image/*")).toBe(true);
    expect(matchesAccept(pdfFile(), "image/*")).toBe(false);
    expect(matchesAccept(pdfFile(), "application/pdf")).toBe(true);
    expect(matchesAccept(pdfFile(), ".pdf,.docx")).toBe(true);
    expect(matchesAccept(imageFile(), ".pdf")).toBe(false);
  });
});

describe("toMarkdown", () => {
  it("图片走图片语法，其余走链接语法", () => {
    expect(
      toMarkdown({ id: "i1", name: "截图.png" }, imageFile("截图.png"))
    ).toBe("![截图.png](/v1/file/i1)");
    expect(toMarkdown({ id: "f1", name: "报告.pdf" }, pdfFile())).toBe(
      "[报告.pdf](/v1/file/f1)"
    );
  });
});

describe("createUploadHandler", () => {
  it("上传成功后由自身插入片段并返回 null", async () => {
    const insert = vi.fn();
    const handler = createUploadHandler({
      uploader: uploaderOf(() => ({ id: "i1", name: "shot.png" })),
      insert,
      messages
    });

    const result = await handler([imageFile()]);

    expect(result).toBeNull();
    expect(insert).toHaveBeenCalledWith("![shot.png](/v1/file/i1)");
  });

  it("多文件按选择顺序合并为多行", async () => {
    const insert = vi.fn();
    const handler = createUploadHandler({
      uploader: uploaderOf(file => ({
        id: file.name.replace(/\W/g, ""),
        name: file.name
      })),
      insert,
      messages
    });

    await handler([imageFile("a.png"), pdfFile()]);

    expect(insert).toHaveBeenCalledWith(
      "![a.png](/v1/file/apng)\n[report.pdf](/v1/file/reportpdf)"
    );
  });

  it("超过体积上限时不上传、不插入，返回提示文案", async () => {
    const insert = vi.fn();
    const uploader = uploaderOf(() => ({ id: "i1", name: "shot.png" }));
    const handler = createUploadHandler({
      uploader,
      insert,
      messages,
      maxSize: 10
    });

    const oversize = new File(["x".repeat(100)], "shot.png", {
      type: "image/png"
    });
    const result = await handler([oversize]);

    expect(result).toBe("too-large:shot.png:10B");
    expect(uploader).not.toHaveBeenCalled();
    expect(insert).not.toHaveBeenCalled();
  });

  it("类型不在 accept 内时拒绝", async () => {
    const insert = vi.fn();
    const handler = createUploadHandler({
      uploader: uploaderOf(() => ({ id: "f1", name: "report.pdf" })),
      insert,
      messages,
      accept: "image/*"
    });

    await expect(handler([pdfFile()])).resolves.toBe("rejected:report.pdf");
    expect(insert).not.toHaveBeenCalled();
  });

  it("上传端口抛错时返回失败文案且不插入", async () => {
    const insert = vi.fn();
    const handler = createUploadHandler({
      uploader: async () => {
        throw new Error("network");
      },
      insert,
      messages
    });

    await expect(handler([imageFile()])).resolves.toBe("failed:shot.png");
    expect(insert).not.toHaveBeenCalled();
  });

  it("上传端口返回空 id 视为失败", async () => {
    const insert = vi.fn();
    const handler = createUploadHandler({
      uploader: async () => ({ id: "", name: "x.png" }),
      insert,
      messages
    });

    await expect(handler([imageFile()])).resolves.toBe("failed:shot.png");
    expect(insert).not.toHaveBeenCalled();
  });

  it("空列表直接返回 null 不触发插入", async () => {
    const insert = vi.fn();
    const handler = createUploadHandler({
      uploader: uploaderOf(() => ({ id: "i1", name: "a.png" })),
      insert,
      messages
    });

    await expect(handler([])).resolves.toBeNull();
    expect(insert).not.toHaveBeenCalled();
  });
});
