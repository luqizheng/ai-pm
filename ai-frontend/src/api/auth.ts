import { mockAuthGateway } from "./mock/auth";

/**
 * 认证网关 —— 认证页与服务端之间的**唯一接口**。
 *
 * 当前实现为 mock（src/api/mock/auth.ts），原因与接入路径：
 *   - 登录端点 `POST /v1/iam/auth/login` 已在 iam-services SDK 就绪
 *     （见 src/api/base/iam.ts），接真实认证时把 signIn 换成
 *     `iam.auth.login({ username, password })` + `saveSession(...)` 即可；
 *   - 「忘记密码 / 重置口令 / 接受邀请」三个流程在 lmms 侧尚无端点，
 *     上游补齐前用 mock 承载页面状态与交互。
 *
 * 页面只依赖本文件的接口，不直接依赖 mock 实现。
 */

/** 登录成功后的会话摘要 */
export interface AuthSession {
  /** 当前账号（邮箱或用户名） */
  account: string;
  /** 所属组织 / 工作区名 */
  organization: string;
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
  /** 邮箱或用户名 */
  account: string;
  /** 口令 */
  password: string;
  /** 记住我：会话保持更久 */
  remember: boolean;
}

/** 认证网关接口 */
export interface AuthGateway {
  /** 凭据登录 */
  signIn(input: SignInInput): Promise<AuthSession>;
  /** 请求口令重置邮件 */
  requestPasswordReset(input: { email: string }): Promise<void>;
  /** 以重置票据提交新口令 */
  resetPassword(input: { newPassword: string; token?: string }): Promise<void>;
  /** 接受工作区邀请 */
  acceptInvitation(input: { token?: string }): Promise<AuthSession>;
}

/** 生效中的认证网关实现 */
export const authGateway: AuthGateway = mockAuthGateway;
