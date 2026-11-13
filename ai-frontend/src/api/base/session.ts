import { clearSession, getRefreshToken, saveTokens } from "./token-storage";

/**
 * 会话失效与 token 刷新的单源实现。
 *
 * 本应用有两条请求通道，401 必须走同一套处理：
 *   1. `@digua/http`（pm 业务面）—— 通过 refreshToken / onUnauthorized 回调接入；
 *   2. iam / prue / config / notification / audit / file SDK —— 它们的 fetch 在
 *      `sessionFetch` 里统一包装（SDK 本身不做刷新，见 ./iam.ts 注释）。
 *
 * 语义边界：只有「已经带上 Authorization 的 401」才视为会话失效；
 * 登录、刷新、改密、邀请接受等端点的 401 是业务结果，不能触发全局登出。
 */

const BASE_URL = import.meta.env.VITE_IAM_BASE_URL ?? "/gw";

/** 这些端点的 401 属于业务语义（凭据错误 / 票据失效），不参与刷新与登出 */
const AUTH_WHITELIST = [
  "/v1/iam/auth/login",
  "/v1/iam/auth/refresh",
  "/v1/iam/auth/logout",
  "/v1/iam/auth/setup-status",
  "/v1/iam/auth/register",
  "/v1/iam/auth/forgot-password",
  "/v1/iam/auth/reset-password",
  "/v1/iam/auth/invitations/accept",
  "/v1/iam/auth/password",
  "/v1/tokens/access",
  "/v1/tokens/refresh"
];

const handlers = new Set<() => void>();

/** 注册会话失效回调（store 重置、路由跳登录页等） */
export function onUnauthorized(handler: () => void): void {
  handlers.add(handler);
}

/** 会话彻底失效：清凭据并通知所有消费方 */
export function notifyUnauthorized(): void {
  clearSession();
  for (const handler of handlers) {
    try {
      handler();
    } catch {
      // 单个消费方失败不得阻断其他消费方
    }
  }
}

export function isAuthWhitelisted(url: string): boolean {
  // 去掉 query string 再匹配，避免带参时白名单失效
  const path = url.split("?")[0];
  return AUTH_WHITELIST.some(item => path.endsWith(item));
}

interface RefreshResult {
  access_token?: string;
  refresh_token?: string;
  expires_in?: number;
}

async function performRefresh(): Promise<string | undefined> {
  const refreshToken = getRefreshToken();
  if (!refreshToken) return undefined;
  try {
    const response = await fetch(`${BASE_URL}/v1/iam/auth/refresh`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ refresh_token: refreshToken })
    });
    if (!response.ok) return undefined;
    const result = (await response.json()) as RefreshResult;
    if (!result?.access_token) return undefined;
    saveTokens({
      accessToken: result.access_token,
      // iam 登录链路不轮换 refresh token：响应缺省时保留原值
      refreshToken: result.refresh_token ?? refreshToken,
      expiresIn: result.expires_in ?? 0
    });
    return result.access_token;
  } catch {
    return undefined;
  }
}

let refreshing: Promise<string | undefined> | null = null;

/**
 * 单飞刷新：并发 401 只发一次刷新请求。
 * 返回 undefined 表示无 refresh token 或刷新失败（调用方据此判定会话失效）。
 */
export function refreshSessionToken(): Promise<string | undefined> {
  if (!refreshing) {
    refreshing = performRefresh().finally(() => {
      refreshing = null;
    });
  }
  return refreshing;
}

const RETRIED = Symbol("session-fetch-retried");

type RetriableInit = RequestInit & { [RETRIED]?: boolean };

function readUrl(input: RequestInfo | URL): string {
  if (typeof input === "string") return input;
  if (input instanceof URL) return input.href;
  return input.url;
}

function readHeaders(input: RequestInfo | URL, init?: RequestInit): Headers {
  if (init?.headers) return new Headers(init.headers);
  if (input instanceof Request) return new Headers(input.headers);
  return new Headers();
}

/**
 * 给 lmms SDK 用的 fetch 包装：401 → 单飞刷新 → 带新 token 重放一次。
 *
 * 刷新不可用或重放仍 401 时通知会话失效（认证页与设计目录由路由侧忽略）。
 */
export const sessionFetch: typeof fetch = async (input, init) => {
  const response = await fetch(input, init);
  if (response.status !== 401) return response;

  if (isAuthWhitelisted(readUrl(input))) return response;

  const headers = readHeaders(input, init);
  // 匿名请求的 401 不是「会话过期」，不刷新也不登出
  if (!headers.has("Authorization")) return response;
  // 一个请求最多刷新重放一次，避免持续 401 时无限刷新
  if ((init as RetriableInit | undefined)?.[RETRIED]) return response;

  const token = await refreshSessionToken();
  if (!token) {
    notifyUnauthorized();
    return response;
  }

  const retryInit: RetriableInit = { ...init, headers: new Headers(headers) };
  (retryInit.headers as Headers).set("Authorization", `Bearer ${token}`);
  retryInit[RETRIED] = true;

  const retried = await fetch(input, retryInit);
  if (retried.status === 401) notifyUnauthorized();
  return retried;
};
