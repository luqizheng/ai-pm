import {
  AuthError,
  type AuthGateway,
  type AuthSession,
  type SignInInput
} from "../auth";

/**
 * 认证 mock 网关 —— 页面状态与交互的可演示实现。
 *
 * 行为约定（演示用）：
 *   - 登录：任意非空凭据通过；账号 `locked@acme.dev` 恒返回凭据错误，
 *     用于演示 Figma 登录页的错误反馈态；
 *   - 忘记密码：恒成功（页面走「查收邮件」信息态）；
 *   - 重置口令：票据 `expired` 返回「链接已过期」，其余成功；
 *   - 接受邀请：恒成功。
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
      organization: DEFAULT_ORGANIZATION
    };
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

  async acceptInvitation(): Promise<AuthSession> {
    await delay(DEFAULT_LATENCY);
    return {
      account: "alex.morgan@acme.dev",
      organization: DEFAULT_ORGANIZATION
    };
  }
};
