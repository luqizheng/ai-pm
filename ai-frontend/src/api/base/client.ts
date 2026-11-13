import { createHttp, type HttpClient } from "@digua/http";
import { ErrorMessages, ErrorMessagesZhCN } from "@digua/contracts";
import { getAccessToken, type StoredUser } from "./token-storage";
import { notifyUnauthorized, refreshSessionToken } from "./session";

export interface LoginResult {
  access_token: string;
  refresh_token: string;
  token_type: string;
  expires_in: number;
  user: StoredUser;
}

/** 业务错误码 → 中文文案（找不到时退回英文，最后退回 code 本身） */
export function translateError(code: string): string {
  const zh = (ErrorMessagesZhCN as Record<string, string>)[code];
  if (zh) return zh;
  const en = (ErrorMessages as Record<string, string>)[code];
  return en ?? code;
}

/** 会话失效回调的注册入口（与 sessionFetch 共用同一份订阅） */
export { onUnauthorized } from "./session";

/**
 * 壳的 HTTP 客户端。
 *
 * @digua/http 是框架无关的：token 存取、刷新、文案翻译都在这里注入。
 * 刷新与失效通知统一走 ./session.ts，与各 lmms SDK 的 sessionFetch 共用同一个
 * 单飞刷新，避免同一时刻向 iam 发多次 refresh。
 */
export const http: HttpClient = createHttp({
  baseURL: import.meta.env.VITE_API_BASE ?? "/api",
  getToken: () => getAccessToken(),
  refreshToken: () => refreshSessionToken(),
  onUnauthorized: () => notifyUnauthorized(),
  translateError,
  authWhiteList: [
    "/v1/iam/auth/login",
    "/v1/iam/auth/refresh",
    "/v1/iam/auth/setup-status"
  ]
});
