/**
 * 产物导出（下载）工具。
 *
 * 口径（ADR-VIZ-007 / 15 号安全审查）：
 * - SVG：服务端纯 Node 出图，**零外网**（字体内嵌）——P0 承诺；
 * - HTML：可选下载（不向浏览器投递；下载物经静态化检查，无脚本）；
 * - PNG/GIF/MP4：Deferred（需浏览器），**不在此实现**。
 *
 * 安全：下载走 SDK 取字节（`<img src>` 无法带 Bearer），再以 Blob 触发下载；
 * object URL 必须 revoke，否则 Blob 泄漏。
 */
import { fetchArtifactOutputBlob } from "@/api/visualizations";

/** 触发浏览器下载（object URL 用完即释放）。 */
export function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  try {
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = filename;
    document.body.appendChild(anchor);
    anchor.click();
    anchor.remove();
  } finally {
    URL.revokeObjectURL(url);
  }
}

/** 导出文件名：槽位 + 版本 + 时间（可追溯）。 */
export function exportFilename(
  slotKey: string,
  version: number,
  format: string
): string {
  const safeSlot = slotKey.replace(/[^\w.-]+/g, "-");
  return `${safeSlot}.v${version}.${format}`;
}

/** 导出 SVG 产物（服务端取字节 → 下载）。 */
export async function exportArtifactSvg(
  artifactId: string,
  slotKey: string,
  version: number
): Promise<void> {
  const blob = await fetchArtifactOutputBlob(artifactId, "svg");
  downloadBlob(blob, exportFilename(slotKey, version, "svg"));
}

/** 导出 HTML 产物（可选下载；不含任何脚本执行上下文）。 */
export async function exportArtifactHtml(
  artifactId: string,
  slotKey: string,
  version: number
): Promise<void> {
  const blob = await fetchArtifactOutputBlob(artifactId, "html");
  downloadBlob(blob, exportFilename(slotKey, version, "html"));
}
