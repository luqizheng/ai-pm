import {
  AuthError,
  type AuthGateway,
  type AuthSession,
  type SignInInput
} from "../auth";
import type { StoredUser } from "../base/token-storage";

/**
 * 认证 mock 网关 —— 页面状态与交互的可演示实现。
 *
 * 行为约定（演示用）：
 *   - 登录：任意非空凭据通过；账号 `locked@acme.dev` 恒返回凭据错误，
 *     用于演示 Figma 登录页的错误反馈态；
 *   - 忘记密码：恒成功（页面走「查收邮件」信息态）；
 *   - 重置口令：票据 `expired` 返回「链接已过期」，其余成功；
 *   - 接受邀请：携带 token + 口令时成功，其余失败。
 *
 * 接真实认证时整体替换本实现（见 src/api/auth.ts 的说明）。
 */

/** 演示用：登录延迟（毫秒） */
const SIGN_IN_LATENCY = 700;
/** 演示用：其余流程延迟（毫秒） */
const DEFAULT_LATENCY = 400;

/** 演示账号：恒定返回凭据错误 */
const LOCKED_ACCOUNT = "locked@acme.dev";
/** 演示用：默认组织名 */
const DEFAULT_ORGANIZATION = "Acme Research";

function delay(ms: number) {
  return new Promise<void>(resolve => {
    setTimeout(resolve, ms);
  });
}

function mockUser(username: string, email?: string): StoredUser {
  return {
    id: `mock-${username}`,
    username,
    nickname: username,
    email: email ?? null,
    phone: null,
    avatar: null,
    roles: ["member"],
    permissions: []
  };
}

export const mockAuthGateway: AuthGateway = {
  async signIn(input: SignInInput): Promise<AuthSession> {
    await delay(SIGN_IN_LATENCY);
    if (!input.account || !input.password) {
      throw new AuthError("invalid_credentials");
    }
    if (input.account === LOCKED_ACCOUNT) {
      throw new AuthError("invalid_credentials");
    }
    return {
      account: input.account,
      organization: DEFAULT_ORGANIZATION,
      user: mockUser(input.account)
    };
  },

  async signOut(): Promise<void> {
    await delay(DEFAULT_LATENCY);
  },

  async requestPasswordReset(): Promise<void> {
    await delay(DEFAULT_LATENCY);
  },

  async resetPassword(input: { newPassword: string; token?: string }) {
    await delay(DEFAULT_LATENCY);
    if (input.token === "expired") {
      throw new AuthError("link_expired");
    }
    if (!input.newPassword) {
      throw new AuthError("unknown");
    }
  },

  async acceptInvitation(input: {
    token?: string;
    password?: string;
  }): Promise<AuthSession> {
    await delay(DEFAULT_LATENCY);
    if (!input.token || !input.password) {
      throw new AuthError("unknown");
    }
    const account = "alex.morgan@acme.dev";
    return {
      account,
      organization: DEFAULT_ORGANIZATION,
      user: mockUser(account, account)
    };
  }
};
