/**
 * 错误信息归一（S04 backlog 内联函数提升为共享，PA-07 批 2026-11-21）。
 * HttpError 已由 @digua/http translateError 转为带 message 的 Error；
 * 这里只兜住非 Error 抛出物。
 */
export function describeError(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}
