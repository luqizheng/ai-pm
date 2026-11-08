import { IamServicesError } from "iam-services";

import { iam } from "./base/iam";

/**
 * 系统初始化（空库强制注册首个管理员）—— 判定与注册的唯一接口。
 *
 * 判定源 = lmms iam `GET /v1/iam/auth/setup-status`（公开，网关已放行）；
 * 注册 = `POST /v1/iam/auth/register`（公开；仅当 iam 用户表为空时允许，
 * 首个用户由上游自动授予 tenant_admin + 全部权限码——登录后经团队
 * onboarding 建团成为唯一租户 owner，即全系统最大管理员）。
 * 两者都走 `iam-services` SDK（`iam.auth.setupStatus/register`）。
 */

/** 初始化状态（setup_required=true 表示空系统，须注册首个账号） */
export interface SetupStatus {
  setup_required: boolean;
  user_count: number;
}

/** 注册失败码（页面据此映射文案） */
export type SetupErrorCode = "closed" | "duplicate" | "network" | "unknown";

/** 注册失败（携带失败码） */
export class SetupError extends Error {
  readonly code: SetupErrorCode;

  constructor(code: SetupErrorCode, message?: string) {
    super(message ?? code);
    this.name = "SetupError";
    this.code = code;
  }
}

/** 首个管理员注册入参（email 可选，服务端 CreateUserRequest 契约） */
export interface RegisterFirstAdminInput {
  username: string;
  password: string;
  nickname: string;
  email?: string;
}

/**
 * 查询系统初始化状态。失败返回 null（fail-open）：网关不可达时登录也
 * 注定失败，不把网络抖动放大成「强制注册」的误判——只有确证
 * setup_required=true 才引导到注册页。
 */
export async function fetchSetupStatus(): Promise<SetupStatus | null> {
  try {
    const status = await iam.auth.setupStatus();
    return {
      setup_required: status.setup_required,
      user_count: status.user_count
    };
  } catch {
    return null;
  }
}

/** 注册首个管理员账号（系统已有用户时上游 403，用户名重复 409） */
export async function registerFirstAdmin(
  input: RegisterFirstAdminInput
): Promise<void> {
  try {
    await iam.auth.register({
      username: input.username,
      password: input.password,
      nickname: input.nickname,
      ...(input.email ? { email: input.email } : {})
    });
  } catch (error) {
    if (error instanceof IamServicesError) {
      if (error.status === 403) {
        throw new SetupError("closed", error.message);
      }
      if (error.status === 409) {
        throw new SetupError("duplicate", error.message);
      }
      if (error.status === 0 || error.status >= 500) {
        throw new SetupError("network", error.message);
      }
      throw new SetupError("unknown", error.message);
    }
    throw new SetupError("network");
  }
}
