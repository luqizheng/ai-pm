import { getAccessToken } from "@/api/base/token-storage";
import type { AuditLog } from "./types";

/**
 * 订阅 BFF 的 SSE 审计事件流（GET /v1/pm/events/stream）。
 *
 * EventSource 无法携带 Authorization 头，token 走 query 参数（BFF 契约）。
 * 断线由 EventSource 自动重连；token 彻底失效时服务端返回 4xx，
 * readyState 变为 CLOSED 后停止（由调用方引导重新登录）。
 *
 * 返回关闭函数。useMock=1 时无 BFF 流，connect() 直接返回 no-op 并回调
 * unavailable，页面据此隐藏实时面板。
 */
export function connectAuditStream(
  onAudit: (log: AuditLog) => void,
  onUnavailable?: () => void
): () => void {
  const token = getAccessToken();
  if (!token) {
    onUnavailable?.();
    return () => undefined;
  }

  const source = new EventSource(
    `/api/v1/pm/events/stream?access_token=${encodeURIComponent(token)}`
  );

  source.addEventListener("audit", event => {
    try {
      onAudit(JSON.parse((event as MessageEvent).data) as AuditLog);
    } catch {
      // 单条坏数据只丢弃该条
    }
  });
  source.onerror = () => {
    if (source.readyState === EventSource.CLOSED) {
      onUnavailable?.();
      source.close();
    }
  };

  return () => source.close();
}
