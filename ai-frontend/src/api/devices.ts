import { http } from "@/api/base";

/**
 * 开发环境目录域读模型（T-091 / `docs/ai-pm-codex-bootstrap-v1.md` V1.3 §九）。
 *
 * 后端契约：pm-api `/v1/pm/devices*`（T-088 `33fc743`）——
 * - Device 是目录实体（V1 无凭据、无心跳）：`status`（active/revoked）与
 *   `readiness`（installed/connected/authenticated/ready/degraded/failed）两列正交。
 * - `bindings[].project_accessible` 由后端按**当前调用人**派生（不落库）。
 * - Ready 是快照：`last_check_at` / `last_check` 是最近一次自检的证据。
 * 错误体 `{message}`（@digua/http 原样透传）；设备不可见/不存在统一 404。
 */

export type DeviceStatus = "active" | "revoked";

export type DeviceReadiness =
  | "installed"
  | "connected"
  | "authenticated"
  | "ready"
  | "degraded"
  | "failed";

export interface DeviceBinding {
  id: string;
  device_id: string;
  project_id: string;
  declared_repository_ids: string[];
  verified_repositories: string[];
  bound_by: string;
  bound_at: string;
  unbound_at: string | null;
  /** 派生值（仅 GET /{id} 返回）：当前调用人是否为该项目的成员。 */
  project_accessible?: boolean;
}

export interface Device {
  id: string;
  tenant_id: string;
  owner_principal_id: string;
  name: string;
  platform: "windows" | "macos" | "linux";
  os_version: string | null;
  executor_kind: string;
  executor_version: string | null;
  status: DeviceStatus;
  readiness: DeviceReadiness | null;
  last_check_at: string | null;
  last_check: {
    installed?: boolean;
    connected?: boolean;
    tool?: boolean;
    verified_repository_count?: number;
    declared_repository_count?: number;
    evidence?: unknown;
  } | null;
  revoked_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface DeviceDetail extends Device {
  bindings: DeviceBinding[];
}

export interface ListResponse<T> {
  items: T[];
  total: number;
}

export function listDevices() {
  return http.get<ListResponse<Device>>("/v1/pm/devices");
}

export function getDevice(id: string) {
  return http.get<DeviceDetail>(`/v1/pm/devices/${id}`);
}

export interface RegisterDevicePayload {
  /** 客户端持久化的设备 id（device.json）；缺省 = 服务端生成（首次登记）。 */
  device_id?: string;
  name: string;
  platform: "windows" | "macos" | "linux";
  os_version?: string | null;
  executor_kind?: string;
  executor_version?: string | null;
}

export function registerDevice(body: RegisterDevicePayload) {
  return http.post<Device & { reused: boolean }>("/v1/pm/devices", body);
}

export function bindDevice(
  deviceId: string,
  body: { project_id: string; declared_repository_ids?: string[] }
) {
  return http.post<DeviceBinding>(`/v1/pm/devices/${deviceId}/bindings`, body);
}

export function unbindDevice(deviceId: string, bindingId: string) {
  return http.delete(`/v1/pm/devices/${deviceId}/bindings/${bindingId}`);
}

export interface DeviceChecksPayload {
  installed: boolean;
  connected: boolean;
  tool: boolean;
  accessible_repositories: string[];
  evidence?: unknown;
}

export function reportDeviceChecks(
  deviceId: string,
  body: DeviceChecksPayload
) {
  return http.post<Device>(`/v1/pm/devices/${deviceId}/checks`, body);
}

export function revokeDevice(deviceId: string) {
  return http.post<Device>(`/v1/pm/devices/${deviceId}/revoke`, {});
}

/** `/install/{tool}?format=json` 的 bootstrap 段（T-089 b2f0a0c）。 */
export interface BootstrapInfo {
  package_url: string;
  release_manifest: {
    package: string;
    version: string;
    sha256: string;
    integrity_only: boolean;
    contents: string[];
  };
  endpoints: Record<string, string>;
  key_handling: string;
}
