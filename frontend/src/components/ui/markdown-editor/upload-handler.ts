import type { AttachmentMeta, AttachmentUploader } from "./types";
import {
  buildAttachmentMarkdown,
  buildImageMarkdown,
  isImageFile
} from "./file-ref";

/**
 * Vditor 的 `upload.handler` 语义与类型声明不一致，这里按源码实测行为封装：
 *
 *   handler 返回 **string** → Vditor 当作提示条文案显示，且**不插入任何内容**；
 *   handler 返回 **null**   → 视为成功，内容需由 handler 自己调 insert 写入。
 *
 * 所以「插入什么」不交给 Vditor 决定，由本工厂显式调用 `insert` 完成。
 */

export interface UploadMessages {
  /** 单文件超限，参数为文件名与上限的可读体积 */
  tooLarge: (name: string, limit: string) => string;
  /** 类型不被接受 */
  rejected: (name: string) => string;
  /** 传输失败 */
  failed: (name: string) => string;
}

export interface UploadHandlerDeps {
  uploader: AttachmentUploader;
  /** 把生成的 Markdown 片段写入光标处 */
  insert: (markdown: string) => void;
  messages: UploadMessages;
  /** 单文件上限（字节）；不传则不限 */
  maxSize?: number;
  /** 与 input accept 同义，如 `image/*,.pdf`；不传则不限 */
  accept?: string;
}

/** 把字节数格式化成可读体积。 */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const rounded = unit === 0 ? String(value) : value.toFixed(1);
  return `${rounded}${units[unit]}`;
}

/** 按 accept 规则判断单个文件是否可上传。 */
export function matchesAccept(file: File, accept?: string): boolean {
  if (!accept) return true;
  return accept
    .split(",")
    .map(token => token.trim().toLowerCase())
    .filter(Boolean)
    .some(token => {
      const type = file.type.toLowerCase();
      const name = file.name.toLowerCase();
      if (token.startsWith(".")) return name.endsWith(token);
      if (token.endsWith("/*")) return type.startsWith(token.slice(0, -1));
      return type === token;
    });
}

/** 由附件标识生成正文片段。 */
export function toMarkdown(meta: AttachmentMeta, file: File): string {
  return isImageFile(file)
    ? buildImageMarkdown(meta.id, meta.name || file.name)
    : buildAttachmentMarkdown(meta.id, meta.name || file.name);
}

/**
 * 生成可直接交给 Vditor 的 `upload.handler`。
 * 依次串行上传（保持插入顺序与用户选择顺序一致），任何一个失败即整体提示。
 */
export function createUploadHandler(
  deps: UploadHandlerDeps
): (files: File[]) => Promise<string | null> {
  const { uploader, insert, messages, maxSize, accept } = deps;

  return async (files: File[]): Promise<string | null> => {
    if (files.length === 0) return null;

    const snippets: string[] = [];

    for (const file of files) {
      if (!matchesAccept(file, accept)) {
        return messages.rejected(file.name);
      }
      if (maxSize !== undefined && file.size > maxSize) {
        return messages.tooLarge(file.name, formatBytes(maxSize));
      }

      let meta: AttachmentMeta;
      try {
        meta = await uploader(file);
      } catch {
        return messages.failed(file.name);
      }

      if (!meta?.id) return messages.failed(file.name);
      snippets.push(toMarkdown(meta, file));
    }

    insert(snippets.join("\n"));
    return null;
  };
}
