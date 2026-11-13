// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

type SessionModule = typeof import("./session");

const REFRESH_URL = "/gw/v1/iam/auth/refresh";

function response(status: number, body: unknown = {}): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" }
  });
}

function authorized(token = "old-token"): RequestInit {
  return { headers: { Authorization: `Bearer ${token}` } };
}

describe("sessionFetch 401 统一处理", () => {
  let fetchMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    vi.resetModules();
    window.localStorage.clear();
    window.localStorage.setItem("prism_access_token", "old-token");
    window.localStorage.setItem("prism_refresh_token", "refresh-1");
    fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  async function load(): Promise<SessionModule> {
    return import("./session");
  }

  it("非 401 直接放行，不触发刷新", async () => {
    const { sessionFetch } = await load();
    fetchMock.mockResolvedValueOnce(response(200, { ok: true }));

    const result = await sessionFetch("/gw/v1/iam/users/me", authorized());

    expect(result.status).toBe(200);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("白名单端点的 401 不刷新也不登出", async () => {
    const { sessionFetch, onUnauthorized } = await load();
    const expired = vi.fn();
    onUnauthorized(expired);
    fetchMock.mockResolvedValueOnce(response(401));

    const result = await sessionFetch("/gw/v1/iam/auth/login", authorized());

    expect(result.status).toBe(401);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(expired).not.toHaveBeenCalled();
  });

  it("匿名请求（无 Authorization）的 401 不触发登出", async () => {
    const { sessionFetch, onUnauthorized } = await load();
    const expired = vi.fn();
    onUnauthorized(expired);
    fetchMock.mockResolvedValueOnce(response(401));

    const result = await sessionFetch("/gw/v1/iam/users/me");

    expect(result.status).toBe(401);
    expect(expired).not.toHaveBeenCalled();
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("带 Authorization 的 401 刷新后带新 token 重放一次", async () => {
    const { sessionFetch } = await load();
    fetchMock
      .mockResolvedValueOnce(response(401))
      .mockResolvedValueOnce(
        response(200, {
          access_token: "new-token",
          refresh_token: "refresh-1",
          expires_in: 900
        })
      )
      .mockResolvedValueOnce(response(200, { ok: true }));

    const result = await sessionFetch("/gw/v1/iam/users/me", authorized());

    expect(result.status).toBe(200);
    expect(fetchMock).toHaveBeenCalledTimes(3);
    expect(fetchMock.mock.calls[1][0]).toBe(REFRESH_URL);
    const retryInit = fetchMock.mock.calls[2][1] as RequestInit;
    expect(new Headers(retryInit.headers).get("Authorization")).toBe(
      "Bearer new-token"
    );
    expect(window.localStorage.getItem("prism_access_token")).toBe("new-token");
  });

  it("刷新失败时清空凭据并通知会话失效", async () => {
    const { sessionFetch, onUnauthorized } = await load();
    const expired = vi.fn();
    onUnauthorized(expired);
    fetchMock
      .mockResolvedValueOnce(response(401))
      .mockResolvedValueOnce(response(401));

    const result = await sessionFetch("/gw/v1/iam/users/me", authorized());

    expect(result.status).toBe(401);
    expect(expired).toHaveBeenCalledTimes(1);
    expect(window.localStorage.getItem("prism_access_token")).toBeNull();
    expect(window.localStorage.getItem("prism_refresh_token")).toBeNull();
  });

  it("并发 401 只发一次刷新", async () => {
    const { sessionFetch } = await load();
    let refreshCalls = 0;
    fetchMock.mockImplementation(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        if (String(input) === REFRESH_URL) {
          refreshCalls += 1;
          await new Promise(resolve => setTimeout(resolve, 0));
          return response(200, {
            access_token: "new-token",
            refresh_token: "refresh-1",
            expires_in: 900
          });
        }
        const token = new Headers(init?.headers).get("Authorization");
        return token === "Bearer new-token"
          ? response(200, { ok: true })
          : response(401);
      }
    );

    const [first, second] = await Promise.all([
      sessionFetch("/gw/v1/iam/users/me", authorized()),
      sessionFetch("/gw/v1/iam/projects", authorized())
    ]);

    expect(first.status).toBe(200);
    expect(second.status).toBe(200);
    expect(refreshCalls).toBe(1);
  });
});
