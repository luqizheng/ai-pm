// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  downloadBlob,
  exportArtifactHtml,
  exportArtifactSvg,
  exportFilename
} from "./export";

const api = vi.hoisted(() => ({
  fetchArtifactOutputBlob: vi.fn(
    async () => new Blob(["<svg/>"], { type: "image/svg+xml" })
  )
}));

vi.mock("@/api/visualizations", () => api);

afterEach(() => {
  vi.clearAllMocks();
  document.body.innerHTML = "";
});

describe("产物导出", () => {
  it("文件名含槽位、版本与格式（可追溯）", () => {
    expect(exportFilename("req-trace:R-042", 3, "svg")).toBe(
      "req-trace-R-042.v3.svg"
    );
  });

  it("文件名清洗掉路径分隔类字符", () => {
    const name = exportFilename("a/b\\c:d", 1, "html");
    expect(name).not.toContain("/");
    expect(name).not.toContain("\\");
  });

  it("downloadBlob 释放 object URL（防 Blob 泄漏）", () => {
    const revoke = vi.spyOn(URL, "revokeObjectURL");
    const click = vi
      .spyOn(HTMLAnchorElement.prototype, "click")
      .mockImplementation(() => {});
    downloadBlob(new Blob(["x"]), "a.svg");
    expect(click).toHaveBeenCalled();
    expect(revoke).toHaveBeenCalled();
    revoke.mockRestore();
    click.mockRestore();
  });

  it("导出 SVG 走产物字节接口（带鉴权，不用裸 fetch）", async () => {
    await exportArtifactSvg("artifact-1", "slot", 2);
    expect(api.fetchArtifactOutputBlob).toHaveBeenCalledWith(
      "artifact-1",
      "svg"
    );
  });

  it("导出 HTML 走产物字节接口", async () => {
    await exportArtifactHtml("artifact-1", "slot", 2);
    expect(api.fetchArtifactOutputBlob).toHaveBeenCalledWith(
      "artifact-1",
      "html"
    );
  });

  it("导出失败向上抛（调用方负责提示，不静默）", async () => {
    api.fetchArtifactOutputBlob.mockRejectedValueOnce(new Error("boom"));
    await expect(exportArtifactSvg("a", "s", 1)).rejects.toThrow("boom");
  });
});
