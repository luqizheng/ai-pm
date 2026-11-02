/**
 * 开发环境管理页（T-091）演示数据 —— 与 acceptance/fixtures.ts 同一家族
 * （devices/ 子目录，fixtures.ts 末尾 re-export）。
 *
 * 数据形状与真后端完全同构：/v1/pm/devices（GET 列表）、/v1/pm/devices/{id}
 * （详情 + bindings[].project_accessible 派生值），snake_case。
 * 状态语义全部消费「后端字段」：readiness 六层、status 两态、last_check 快照
 * ——页面零推导（V1.3 §九：无在线徽标、无 Worker 徽标；就绪是快照）。
 */
import type { Device, DeviceDetail } from "@/api/devices";

function device(overrides: Partial<Device> & { id: string; name: string }): Device {
  return {
    tenant_id: "00000000-0000-0000-0000-000000000001",
    owner_principal_id: "00000000-0000-0000-0000-0000000000aa",
    platform: "windows",
    os_version: "Windows 11 Pro",
    executor_kind: "codex",
    executor_version: "0.163.0",
    status: "active",
    readiness: null,
    last_check_at: null,
    last_check: null,
    revoked_at: null,
    created_at: "2026-11-02T09:00:00Z",
    updated_at: "2026-11-02T09:00:00Z",
    ...overrides
  };
}

export const demoDeviceReady: Device = device({
  id: "d1000000-0000-0000-0000-000000000001",
  name: "Windows Development PC",
  readiness: "ready",
  last_check_at: "2026-11-03T08:30:00Z",
  last_check: {
    installed: true,
    connected: true,
    tool: true,
    verified_repository_count: 2,
    declared_repository_count: 2
  }
});

export const demoDeviceDegraded: Device = device({
  id: "d1000000-0000-0000-0000-000000000002",
  name: "MacBook Pro",
  platform: "macos",
  os_version: "macOS 15.1",
  executor_version: "0.161.0",
  readiness: "degraded",
  last_check_at: "2026-11-03T07:10:00Z",
  last_check: {
    installed: true,
    connected: true,
    tool: true,
    verified_repository_count: 1,
    declared_repository_count: 3
  }
});

export const demoDeviceInstalling: Device = device({
  id: "d1000000-0000-0000-0000-000000000003",
  name: "Linux CI Sandbox",
  platform: "linux",
  os_version: "Ubuntu 24.04",
  readiness: null,
  last_check_at: null,
  last_check: null
});

export const demoDeviceRevoked: Device = device({
  id: "d1000000-0000-0000-0000-000000000004",
  name: "Retired Laptop",
  platform: "macos",
  status: "revoked",
  readiness: null,
  revoked_at: "2026-11-02T18:00:00Z",
  last_check_at: "2026-11-01T12:00:00Z",
  last_check: {
    installed: true,
    connected: true,
    tool: true,
    verified_repository_count: 1,
    declared_repository_count: 1
  }
});

/** 详情视图：绑定 + 派生的 project_accessible（后端按当前调用人计算）。 */
export interface DeviceDetailFixture {
  device: Device;
  bindings: DeviceDetail["bindings"];
}

function binding(
  id: string,
  deviceId: string,
  projectId: string,
  accessible: boolean,
  verified: number
): DeviceDetail["bindings"][number] {
  return {
    id,
    device_id: deviceId,
    project_id: projectId,
    declared_repository_ids: [
      `r1000000-0000-0000-0000-00000000000${verified}`,
      `r2000000-0000-0000-0000-00000000000${verified}`,
      `r3000000-0000-0000-0000-00000000000${verified}`
    ],
    verified_repositories:
      verified > 0
        ? [
            `r1000000-0000-0000-0000-00000000000${verified}`,
            `r2000000-0000-0000-0000-00000000000${verified}`
          ].slice(0, verified)
        : [],
    bound_by: "00000000-0000-0000-0000-0000000000aa",
    bound_at: "2026-11-02T10:00:00Z",
    unbound_at: null,
    project_accessible: accessible
  };
}

export const demoDeviceDetails: Record<string, DeviceDetailFixture> = {
  [demoDeviceReady.id]: {
    device: demoDeviceReady,
    bindings: [
      binding("b1", demoDeviceReady.id, "p1000000-0000-0000-0000-000000000001", true, 2)
    ]
  },
  [demoDeviceDegraded.id]: {
    device: demoDeviceDegraded,
    bindings: [
      // 多 Project Ready 语义（C182）：Ready 不随调用人变化；可达性按调用人派生。
      binding("b2", demoDeviceDegraded.id, "p1000000-0000-0000-0000-000000000001", true, 1),
      binding("b3", demoDeviceDegraded.id, "p2000000-0000-0000-0000-000000000002", false, 0)
    ]
  },
  [demoDeviceInstalling.id]: {
    device: demoDeviceInstalling,
    bindings: []
  },
  [demoDeviceRevoked.id]: {
    device: demoDeviceRevoked,
    bindings: [
      // 撤销设备的 Binding 行保留（历史与审计），但有效性受设备状态约束。
      binding("b4", demoDeviceRevoked.id, "p1000000-0000-0000-0000-000000000001", true, 0)
    ]
  }
};

export const demoDevices: Device[] = [
  demoDeviceReady,
  demoDeviceDegraded,
  demoDeviceInstalling,
  demoDeviceRevoked
];
