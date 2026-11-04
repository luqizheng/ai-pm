import type { FileUrlResolver } from "./types";

/**
 * 正文引用约定。
 *
 * 图片与附件一律以 `/v1/file/{id}` 相对路径写进 Markdown：
 *   - 图片 `![alt](/v1/file/{id})`
 *   - 附件 `[文件名.pdf](/v1/file/{id})`
 *
 * 绝不写入预签名地址 —— 预签名有有效期，写进正文后文档会随时间自行失效，
 * 且存档内容无法再回填。渲染时按 id 现签现用。
 */
export const FILE_REF_PREFIX = "/v1/file/";

/** 一次最多并发多少个签名请求，避免打开长文档时把网关打满。 */
const RESOLVE_CONCURRENCY = 5;

/** 引号用十六进制转义写出：裸引号会让 i18n 门禁的注释剥离状态机误判字符串起点 */
const FILE_REF_RE = /\/v1\/file\/([^/?#)\s\x22\x27]+)/;

/** 拼出正文用的引用路径。 */
export function buildFileRef(fileId: string): string {
  return `${FILE_REF_PREFIX}${fileId}`;
}

/**
 * 从任意形式的地址里取出文件 id。
 * 兼容相对路径、绝对 URL，以及带 query/hash/标题后缀的写法。
 */
export function parseFileRef(raw: string | null | undefined): string | null {
  if (!raw) return null;
  const match = FILE_REF_RE.exec(raw);
  return match ? match[1] : null;
}

/** 是否为图片附件，决定插入图片语法还是链接语法。 */
export function isImageFile(file: File): boolean {
  if (file.type) return file.type.startsWith("image/");
  return /\.(png|jpe?g|gif|webp|avif|bmp|svg)$/i.test(file.name);
}

function escapeLinkText(text: string): string {
  return text.replace(/([\\[\]])/g, "\\$1");
}

/** 生成图片片段。 */
export function buildImageMarkdown(fileId: string, alt: string): string {
  return `![${escapeLinkText(alt)}](${buildFileRef(fileId)})`;
}

/** 生成附件片段。 */
export function buildAttachmentMarkdown(fileId: string, name: string): string {
  return `[${escapeLinkText(name)}](${buildFileRef(fileId)})`;
}

/** 把附件名或任意地址还原成展示用文件名。 */
export function displayNameOf(ref: string, fallback: string): string {
  const raw = ref.split("/").pop() ?? "";
  return raw ? decodeURIComponent(raw) : fallback;
}

async function mapWithLimit<T, R>(
  items: T[],
  limit: number,
  task: (item: T) => Promise<R>
): Promise<R[]> {
  const results = new Array<R>(items.length);
  let cursor = 0;

  async function worker(): Promise<void> {
    while (cursor < items.length) {
      const index = cursor++;
      results[index] = await task(items[index]);
    }
  }

  const workers = Array.from({ length: Math.min(limit, items.length) }, () =>
    worker()
  );
  await Promise.all(workers);
  return results;
}

/** 已替换过的节点打标记，避免重复签名。 */
export const HYDRATED_ATTR = "data-file-hydrated";
/** 签名失败标记，供样式层给出兜底外观。 */
export const FILE_REF_ERROR_ATTR = "data-file-error";

interface RefTarget {
  element: HTMLElement;
  attribute: "src" | "href";
  fileId: string;
}

/**
 * 就地替换容器内的文件引用为临时地址。
 *
 * 只改 DOM，不回写 Markdown —— 正文始终保留 `/v1/file/{id}`。
 * 返回实际替换成功的节点数。
 */
export async function hydrateFileRefs(
  root: ParentNode,
  resolve: FileUrlResolver
): Promise<number> {
  const targets: RefTarget[] = [];

  const collect = (element: HTMLElement, attribute: "src" | "href"): void => {
    if (element.hasAttribute(HYDRATED_ATTR)) return;
    const raw = element.getAttribute(attribute);
    const fileId = parseFileRef(raw);
    if (!fileId) return;
    targets.push({ element, attribute, fileId });
  };

  root
    .querySelectorAll<HTMLImageElement>("img[src]")
    .forEach(node => collect(node, "src"));
  root
    .querySelectorAll<HTMLAnchorElement>("a[href]")
    .forEach(node => collect(node, "href"));

  if (targets.length === 0) return 0;

  const pending = new Map<string, Promise<string | null>>();
  const resolveOnce = (fileId: string): Promise<string | null> => {
    const cached = pending.get(fileId);
    if (cached) return cached;
    const promise = resolve(fileId).catch(() => null);
    pending.set(fileId, promise);
    return promise;
  };

  await mapWithLimit(targets, RESOLVE_CONCURRENCY, async target => {
    const url = await resolveOnce(target.fileId);
    if (!url) {
      target.element.setAttribute(FILE_REF_ERROR_ATTR, "1");
      return false;
    }
    target.element.setAttribute(target.attribute, url);
    target.element.setAttribute(HYDRATED_ATTR, "1");
    return true;
  });

  return targets.filter(
    target => target.element.getAttribute(HYDRATED_ATTR) === "1"
  ).length;
}
