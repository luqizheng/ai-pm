/**
 * 无损往返（spec §1.4 / flow E）：进入 /focus 时携带来源地址与锚点，
 * 返回时优先回到来源页；来源缺失或跨项目时回落到总控台。
 */
export function createFocusHref(
  projectId: string,
  objectType: string,
  id: string,
  options: { anchor?: string; from?: string } = {}
): string {
  const params = new URLSearchParams();
  if (options.from) params.set("from", options.from);
  if (options.anchor) params.set("anchor", options.anchor);
  const query = params.toString();
  return `/workspace/${projectId}/focus/${objectType}/${id}${query ? `?${query}` : ""}`;
}

export function returnHref(from: unknown, projectId: string): string {
  if (
    typeof from === "string" &&
    from.startsWith(`/workspace/${projectId}`) &&
    !from.includes("/focus/")
  ) {
    return from;
  }
  return `/workspace/${projectId}/console`;
}