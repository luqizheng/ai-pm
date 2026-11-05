import { HttpCore, IamServicesError } from "iam-services";

import { file } from "./file";
import { getAccessToken } from "./token-storage";

/**
 * 自助资料与会话（iam-service 本轮新增端点）。
 *
 * 路径与 DTO 以上游 iam-service 为准；SDK 发布新版本后把这里的方法
 * 迁回 `iam.users` / `iam.auth`（调用方接口保持不变）。
 */
export interface SelfProfile {
  id: string;
  username: string;
  email: string | null;
  phone: string | null;
  nickname: string;
  avatar: string | null;
  status: string;
  email_verified: boolean;
  phone_verified: boolean;
  last_login_at?: string | null;
  created_at: string;
  updated_at: string;
}

export interface SelfSession {
  id: string;
  user_id: string;
  client_ip: string;
  user_agent: string | null;
  created_at: string;
  expires_at: string;
  is_current: boolean;
}

export interface SelfProfileUpdate {
  nickname?: string;
  avatar?: string;
}

export { IamServicesError };

const http = new HttpCore({
  baseUrl: import.meta.env.VITE_IAM_BASE_URL ?? "/gw",
  getToken: () => getAccessToken(),
  fetch: fetch.bind(globalThis)
});

export function getMe(): Promise<SelfProfile> {
  return http.get<SelfProfile>("/v1/iam/users/me", { auth: true });
}

export function updateMe(input: SelfProfileUpdate): Promise<SelfProfile> {
  return http.put<SelfProfile>("/v1/iam/users/me", input, { auth: true });
}

export function changePassword(
  oldPassword: string,
  newPassword: string
): Promise<void> {
  return http.post<void>(
    "/v1/iam/auth/password",
    { old_password: oldPassword, new_password: newPassword },
    { auth: true }
  );
}

export function listSessions(): Promise<SelfSession[]> {
  return http.get<SelfSession[]>("/v1/iam/auth/sessions", { auth: true });
}

export function revokeSession(sessionId: string): Promise<void> {
  return http.delete<void>(
    `/v1/iam/auth/sessions/${encodeURIComponent(sessionId)}`,
    { auth: true }
  );
}

/** 上传头像并返回可持久化的文件地址（`/v1/file/{id}`）。 */
export async function uploadAvatar(picked: File): Promise<string> {
  const info = await file.files.upload(
    { filename: picked.name, content_type: picked.type },
    picked
  );
  return info.url;
}

/**
 * 头像地址需要 Bearer 才能下载，`<img src>` 无法带 token：
 * 这里用 SDK 下载字节并转成 object URL。
 */
export async function resolveAvatarSrc(
  avatar: string | null
): Promise<string | undefined> {
  if (!avatar) return undefined;
  const match = /\/v1\/file\/([^/?#]+)/.exec(avatar);
  if (!match) return avatar;
  try {
    const download = await file.files.download(match[1]);
    return URL.createObjectURL(
      new Blob([download.bytes], {
        type: download.contentType || "application/octet-stream"
      })
    );
  } catch {
    return undefined;
  }
}
