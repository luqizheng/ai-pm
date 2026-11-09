/**
 * 附件端口的上游实现单测（2026-11-10 RustFS 对象存储链路）。
 *
 * 覆盖：上传参数映射（租户/用户归属由服务端覆写，前端不声明）、预签名解析
 * 与进程内缓存、切换账号时缓存清空。
 */

import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  upload: vi.fn(),
  createPresign: vi.fn()
}));

vi.mock("./file", () => ({
  file: {
    files: { upload: mocks.upload },
    presign: { create: mocks.createPresign }
  }
}));

import {
  clearAttachmentUrlCache,
  resolveAttachmentUrl,
  uploadAttachment
} from "./attachments";

beforeEach(() => {
  clearAttachmentUrlCache();
  mocks.upload.mockReset();
  mocks.createPresign.mockReset();
});

describe("uploadAttachment", () => {
  it("只上报文件名与类型，归属字段交服务端按鉴权身份覆写", async () => {
    mocks.upload.mockResolvedValue({
      id: "f-1",
      filename: "spec.pdf",
      size: 2048,
      content_type: "application/pdf"
    });

    const file = new File(["%PDF-1.7"], "spec.pdf", {
      type: "application/pdf"
    });
    const meta = await uploadAttachment(file);

    expect(mocks.upload).toHaveBeenCalledTimes(1);
    const [metaArg, blobArg, nameArg] = mocks.upload.mock.calls[0];
    expect(metaArg).toEqual({
      filename: "spec.pdf",
      content_type: "application/pdf"
    });
    expect(metaArg).not.toHaveProperty("tenant_id");
    expect(metaArg).not.toHaveProperty("user_id");
    expect(blobArg).toBe(file);
    expect(nameArg).toBe("spec.pdf");

    expect(meta).toEqual({
      id: "f-1",
      name: "spec.pdf",
      size: 2048,
      contentType: "application/pdf"
    });
  });

  it("服务端未给 content_type 时留空，由编辑器按扩展名兜底", async () => {
    mocks.upload.mockResolvedValue({
      id: "f-2",
      filename: "shot.png",
      size: 10,
      content_type: null
    });

    const meta = await uploadAttachment(
      new File(["x"], "shot.png", { type: "" })
    );

    expect(meta.contentType).toBeUndefined();
  });
});

describe("resolveAttachmentUrl", () => {
  it("按 id 换预签名下载地址，并带上下载操作与有效期", async () => {
    mocks.createPresign.mockResolvedValue({
      url: "http://127.0.0.1:9000/ai-pm/f-1?X-Amz-Signature=abc",
      expires_at: new Date(Date.now() + 900_000).toISOString(),
      operation: "download"
    });

    const url = await resolveAttachmentUrl("f-1");

    expect(mocks.createPresign).toHaveBeenCalledWith({
      file_id: "f-1",
      operation: "download",
      expires_in_seconds: 900
    });
    expect(url).toBe("http://127.0.0.1:9000/ai-pm/f-1?X-Amz-Signature=abc");
  });

  it("同一 id 在有效期内只签一次", async () => {
    mocks.createPresign.mockResolvedValue({
      url: "http://127.0.0.1:9000/ai-pm/f-1?sig=1",
      expires_at: new Date(Date.now() + 900_000).toISOString(),
      operation: "download"
    });

    await resolveAttachmentUrl("f-1");
    await resolveAttachmentUrl("f-1");

    expect(mocks.createPresign).toHaveBeenCalledTimes(1);
  });

  it("清缓存后重新签发（切换账号不沿用上一个身份的地址）", async () => {
    mocks.createPresign.mockResolvedValue({
      url: "http://127.0.0.1:9000/ai-pm/f-1?sig=1",
      expires_at: new Date(Date.now() + 900_000).toISOString(),
      operation: "download"
    });

    await resolveAttachmentUrl("f-1");
    clearAttachmentUrlCache();
    await resolveAttachmentUrl("f-1");

    expect(mocks.createPresign).toHaveBeenCalledTimes(2);
  });

  it("expires_at 解析失败时按约定有效期兜底，不缓存过期地址", async () => {
    mocks.createPresign.mockResolvedValue({
      url: "http://127.0.0.1:9000/ai-pm/f-1?sig=1",
      expires_at: "not-a-date",
      operation: "download"
    });

    const url = await resolveAttachmentUrl("f-1");

    expect(url).toContain("sig=1");
    expect(mocks.createPresign).toHaveBeenCalledTimes(1);
  });
});
