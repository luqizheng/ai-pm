import { defineStore } from "pinia";
import { computed, ref } from "vue";

import {
  AuthError,
  authGateway,
  type AuthErrorCode,
  type AuthSession,
  type SignInInput
} from "@/api/auth";

/**
 * 认证状态 —— 认证域的唯一状态源。
 *
 * 覆盖认证页之间需要共享的四件事：
 *   - 当前会话（账号 + 组织）：会话过期页与 403 页据此回显上下文；
 *   - 登录态与错误码：页面据此渲染 Figma「Unable to sign in」反馈；
 *   - 「记住我」的账号：下次进登录页预填。
 *
 * 真正的凭据校验走网关（src/api/auth.ts），本 store 不自行判定口令。
 */

/** 记住的账号的持久化键 */
export const REMEMBERED_ACCOUNT_STORAGE_KEY = "aipm.auth.remembered-account";

function readRememberedAccount(): string | null {
  try {
    return window.localStorage.getItem(REMEMBERED_ACCOUNT_STORAGE_KEY);
  } catch {
    return null;
  }
}

function persistRememberedAccount(account: string | null) {
  try {
    if (account === null) {
      window.localStorage.removeItem(REMEMBERED_ACCOUNT_STORAGE_KEY);
    } else {
      window.localStorage.setItem(REMEMBERED_ACCOUNT_STORAGE_KEY, account);
    }
  } catch {
    // 隐私模式下不可用：内存态仍然有效
  }
}

export const useAuthStore = defineStore("auth", () => {
  /** 登录态 */
  const status = ref<"signed_out" | "signing_in" | "signed_in">("signed_out");
  /** 当前账号 */
  const account = ref<string | null>(null);
  /** 当前组织 / 工作区 */
  const organization = ref<string | null>(null);
  /** 最近一次失败码 */
  const errorCode = ref<AuthErrorCode | null>(null);
  /** 「记住我」保存的账号 */
  const rememberedAccount = ref<string | null>(readRememberedAccount());

  /** 是否已登录 */
  const signedIn = computed(() => status.value === "signed_in");
  /** 登录进行中（按钮 loading / 禁止重复提交） */
  const signingIn = computed(() => status.value === "signing_in");

  function applySession(session: AuthSession) {
    account.value = session.account;
    organization.value = session.organization;
    status.value = "signed_in";
  }

  function clearError() {
    errorCode.value = null;
  }

  /** 凭据登录；失败时抛 AuthError（页面据此映射文案） */
  async function signIn(input: SignInInput): Promise<AuthSession> {
    status.value = "signing_in";
    errorCode.value = null;
    try {
      const session = await authGateway.signIn(input);
      applySession(session);
      if (input.remember) {
        rememberedAccount.value = session.account;
        persistRememberedAccount(session.account);
      } else {
        rememberedAccount.value = null;
        persistRememberedAccount(null);
      }
      return session;
    } catch (error) {
      status.value = "signed_out";
      errorCode.value =
        error instanceof AuthError ? error.code : ("unknown" as AuthErrorCode);
      throw error;
    }
  }

  /** 请求口令重置邮件 */
  async function requestPasswordReset(email: string): Promise<void> {
    errorCode.value = null;
    try {
      await authGateway.requestPasswordReset({ email });
    } catch (error) {
      errorCode.value =
        error instanceof AuthError ? error.code : ("unknown" as AuthErrorCode);
      throw error;
    }
  }

  /** 提交新口令 */
  async function resetPassword(newPassword: string, token?: string) {
    errorCode.value = null;
    try {
      await authGateway.resetPassword({ newPassword, token });
    } catch (error) {
      errorCode.value =
        error instanceof AuthError ? error.code : ("unknown" as AuthErrorCode);
      throw error;
    }
  }

  /** 接受工作区邀请 */
  async function acceptInvitation(token?: string): Promise<AuthSession> {
    status.value = "signing_in";
    errorCode.value = null;
    try {
      const session = await authGateway.acceptInvitation({ token });
      applySession(session);
      return session;
    } catch (error) {
      status.value = "signed_out";
      errorCode.value =
        error instanceof AuthError ? error.code : ("unknown" as AuthErrorCode);
      throw error;
    }
  }

  /** 结束会话（会话过期页 / 切换账号） */
  function signOut() {
    status.value = "signed_out";
    account.value = null;
    organization.value = null;
    errorCode.value = null;
  }

  return {
    status,
    account,
    organization,
    errorCode,
    rememberedAccount,
    signedIn,
    signingIn,
    signIn,
    signOut,
    requestPasswordReset,
    resetPassword,
    acceptInvitation,
    clearError
  };
});
