/**
 * 编辑器对外的两个端口。
 *
 * 组件只认这两个函数签名，不直接依赖 file-services —— 传输层由调用方注入，
 * 便于替换存储后端（RustFS / 其他 S3 兼容服务）与在测试里打桩。
 */

/** 上传成功后的附件标识。正文只保存 `id`，不保存任何会过期的地址。 */
export interface AttachmentMeta {
  /** 文件服务返回的文件 id，正文以 `/v1/file/{id}` 引用 */
  id: string;
  /** 原始文件名，作为附件链接文案 */
  name: string;
  /** 字节数；缺省时附件卡片不展示体积 */
  size?: number;
  /** MIME 类型；缺省时按扩展名推断 */
  contentType?: string;
}

/** 上传端口：把浏览器里的 File 变成可引用的附件标识。 */
export type AttachmentUploader = (file: File) => Promise<AttachmentMeta>;

/** 解析端口：把文件 id 换成可直接放进 src/href 的临时地址。 */
export type FileUrlResolver = (fileId: string) => Promise<string | null>;
