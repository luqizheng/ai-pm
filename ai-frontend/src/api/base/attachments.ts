import type {
  AttachmentUploader,
  FileUrlResolver
} from "@/components/ui/markdown-editor";
import { file as fileServices } from "./file";

/**
 * Markdown 附件端口的上游实现。
 *
 * 传输链：浏览器 → pm-api 文件代理（`/v1/file/*`）→ file-service → 对象存储
 * （RustFS，S3 兼容）。上传走 multipart，不直连 S3：RustFS 的兼容矩阵里
 * POST Object 表单上传尚未支持，且签名密钥不应下发到浏览器。
 *
 * 下载走 file-service 签发的预签名 URL（浏览器直取对象存储字节，免 JWT）。
 * 正文只存 `/v1/file/{id}`，渲染时现签现用（见 markdown-editor/file-ref.ts）。
 */

/** 预签名有效期（秒），与网关侧保持一致的小值，够一次页面渲染即可 */
const PRESIGN_TTL_SECONDS = 900;
/** 剩余有效期低于该值时提前重签，避免翻页时图片刚好过期 */
const REFRESH_AHEAD_MS = 60 * 1000;

interface CachedUrl {
  url: string;
  expiresAt: number;
}

const urlCache = new Map<string, CachedUrl>();

/** 传给编辑器上传端口的实现。 */
export const uploadAttachment: AttachmentUploader = async source => {
  const info = await fileServices.files.upload(
    {
      filename: source.name,
      content_type: source.type || null
    },
    source,
    source.name
  );

  return {
    id: info.id,
    name: info.filename,
    size: info.size,
    contentType: info.content_type ?? undefined
  };
};

/** 传给编辑器解析端口的实现：按 file id 换临时下载地址，带进程内缓存。 */
export const resolveAttachmentUrl: FileUrlResolver = async fileId => {
  const cached = urlCache.get(fileId);
  if (cached && cached.expiresAt - Date.now() > REFRESH_AHEAD_MS) {
    return cached.url;
  }

  const signed = await fileServices.presign.create({
    file_id: fileId,
    operation: "download",
    expires_in_seconds: PRESIGN_TTL_SECONDS
  });

  const parsed = Date.parse(signed.expires_at);
  urlCache.set(fileId, {
    url: signed.url,
    expiresAt: Number.isNaN(parsed)
      ? Date.now() + PRESIGN_TTL_SECONDS * 1000
      : parsed
  });

  return signed.url;
};

/** 切换账号或登出时清空，避免把上一个人的可读地址留在内存里。 */
export function clearAttachmentUrlCache(): void {
  urlCache.clear();
}
