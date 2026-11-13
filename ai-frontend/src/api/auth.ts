import { HttpCore, IamServicesError } from "iam-services";

import { iam } from "./base/iam";
import {
  clearSession,
  getAccessToken,
  saveSession,
  type StoredUser
} from "./base/token-storage";
import { sessionFetch } from "./base/session";

/**
 * 认证网关 —— 认证页与服务端之间的**唯一接口**。
 *
 * 实际实现对 lmms iam-service 经 api-gateway 的 `/v1/iam/auth/*` 与
 * `/v1/iam/users/me`。登录/登出使用上游 `iam-services` SDK；
 * 忘记密码 / 重置密码 / 邀请接受是本轮新增端点，SDK 尚未发布对应方法，
 * 因此复用 SDK 导出的 `HttpCore`（同一传输层与错误归一），
 * SDK 升级后直接替换为 `iam.auth.*` 方法即可。
 */

/** 登录成功后的会话摘要 */
export interface AuthSession {
  /** 当前账号（用户名） */
  account: string;
  /** 所属工作区名称（产品名，非租户实体字段） */
  organization: string;
  /** 登录用户资料 */
  user: StoredUser;
}

/** 认证失败码 */
export type AuthErrorCode =
  | "invalid_credentials"
  | "link_expired"
  | "network"
  | "unknown";

/** 认证错误（携带失败码，供页面映射文案） */
export class AuthError extends Error {
  readonly code: AuthErrorCode;

  constructor(code: AuthErrorCode, message?: string) {
    super(message ?? code);
    this.name = "AuthError";
    this.code = code;
  }
}

/** 登录入参 */
export interface SignInInput {
  /** 用户名 */
  account: string;
  /** 口令 */
  password: string;
  /** 记住我：优先写入持久账号 */
  remember: boolean;
}

/** 认证网关接口 */
export interface AuthGateway {
  /** 凭据登录 */
  signIn(input: SignInInput): Promise<AuthSession>;
  /** 退出登录（撤销服务端会话） */
  signOut(): Promise<void>;
  /** 请求口令重置邮件 */
  requestPasswordReset(input: { email: string }): Promise<void>;
  /** 以重置票据提交新口令 */
  resetPassword(input: { newPassword: string; token?: string }): Promise<void>;
  /** 接受工作区邀请并设置口令 */
  acceptInvitation(input: {
    token?: string;
    password?: string;
    nickname?: string;
  }): Promise<AuthSession>;
}

/** 工作区产品名（顶栏组织芯片展示；iam 登录响应不含租户名称） */
const WORKSPACE_NAME = "AI R&D Workspace";

const selfService = new HttpCore({
  baseUrl: import.meta.env.VITE_IAM_BASE_URL ?? "/gw",
  getToken: () => getAccessToken(),
  fetch: sessionFetch
});

function toAuthError(error: unknown): AuthError {
  if (error instanceof AuthError) return error;
  if (error instanceof IamServicesError) {
    if (error.status === 401) {
      return new AuthError("invalid_credentials", error.message);
    }
    if (error.code === "IAM_AUTH_1009") {
      return new AuthError("link_expired", error.message);
    }
    if (error.status === 0 || error.status >= 500) {
      return new AuthError("network", error.message);
    }
    return new AuthError("unknown", error.message);
  }
  return new AuthError("network");
}

function toSession(user: StoredUser): AuthSession {
  return {
    account: user.username,
    organization: WORKSPACE_NAME,
    user
  };
}

export const authGateway: AuthGateway = {
  async signIn(input: SignInInput): Promise<AuthSession> {
    try {
      const result = await iam.auth.login({
        username: input.account,
        password: input.password
      });
      saveSession({
        accessToken: result.access_token,
        refreshToken: result.refresh_token,
        expiresIn: result.expires_in,
        user: result.user
      });
      return toSession(result.user);
    } catch (error) {
      throw toAuthError(error);
    }
  },

  async signOut(): Promise<void> {
    try {
      await iam.auth.logout();
    } catch {
      // 会话可能已失效：本地清理优先，不阻断退出
    } finally {
      clearSession();
    }
  },

  async requestPasswordReset(input: { email: string }): Promise<void> {
    try {
      await selfService.post<void>("/v1/iam/auth/forgot-password", {
        email: input.email
      });
    } catch (error) {
      throw toAuthError(error);
    }
  },

  async resetPassword(input: { newPassword: string; token?: string }) {
    if (!input.token) {
      throw new AuthError("link_expired");
    }
    try {
      await selfService.post<void>("/v1/iam/auth/reset-password", {
        token: input.token,
        new_password: input.newPassword
      });
    } catch (error) {
      throw toAuthError(error);
    }
  },

  async acceptInvitation(input: {
    token?: string;
    password?: string;
    nickname?: string;
  }): Promise<AuthSession> {
    if (!input.token || !input.password) {
      throw new AuthError("unknown");
    }
    try {
      const result = await selfService.post<{
        access_token: string;
        refresh_token: string;
        expires_in: number;
        user: StoredUser;
      }>("/v1/iam/auth/invitations/accept", {
        token: input.token,
        new_password: input.password,
        nickname: input.nickname
      });
      saveSession({
        accessToken: result.access_token,
        refreshToken: result.refresh_token,
        expiresIn: result.expires_in,
        user: result.user
      });
      return toSession(result.user);
    } catch (error) {
      throw toAuthError(error);
    }
  }
};
