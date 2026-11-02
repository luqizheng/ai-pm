import { describe, expect, it } from "vitest";
import {
  demoDeviceDegraded,
  demoDeviceDetails,
  demoDeviceInstalling,
  demoDeviceReady,
  demoDeviceRevoked,
  demoDevices
} from "./fixtures";

/**
 * T-091 演示数据一致性（与 acceptance/fixtures.test.ts 同一家族）：
 * 状态语义消费后端字段、四台设备覆盖全部关键状态组合、
 * 多 Project Binding 下 Ready 不随调用人变化（C182 的前端投影）。
 */
describe("开发环境 fixtures（T-091）", () => {
  it("四台设备覆盖关键状态组合：ready / degraded / 未自检 / revoked", () => {
    expect(demoDevices).toHaveLength(4);
    expect(demoDeviceReady.readiness).toBe("ready");
    expect(demoDeviceReady.status).toBe("active");
    expect(demoDeviceDegraded.readiness).toBe("degraded");
    expect(demoDeviceInstalling.readiness).toBeNull();
    expect(demoDeviceInstalling.last_check_at).toBeNull();
    expect(demoDeviceRevoked.status).toBe("revoked");
    expect(demoDeviceRevoked.revoked_at).not.toBeNull();
    // 撤销设备 readiness 清空（后端 revoke 语义）
    expect(demoDeviceRevoked.readiness).toBeNull();
  });

  it("readiness 六层取值合法（后端 CHECK 约束的镜像）", () => {
    const legal = [
      "installed",
      "connected",
      "authenticated",
      "ready",
      "degraded",
      "failed"
    ];
    for (const d of demoDevices) {
      if (d.readiness !== null) {
        expect(legal).toContain(d.readiness);
      }
    }
  });

  it("详情：多 Project Binding 下 Ready 不随调用人变化（C182）", () => {
    const detail = demoDeviceDetails[demoDeviceDegraded.id];
    expect(detail.bindings).toHaveLength(2);
    const [a, b] = detail.bindings;
    // 设备 readiness 是设备自身属性——页面从 device 读，不从 binding 推导
    expect(detail.device.readiness).toBe("degraded");
    // project_accessible 是后端按调用人派生的值（两行一真一假）
    expect(a.project_accessible).toBe(true);
    expect(b.project_accessible).toBe(false);
    // verified ⊆ declared（P1-2：服务端确认的仓库身份与项目归属）
    for (const binding of detail.bindings) {
      for (const repo of binding.verified_repositories) {
        expect(binding.declared_repository_ids).toContain(repo);
      }
    }
  });

  it("ready 设备的自检快照带逐项结果（就绪是快照）", () => {
    expect(demoDeviceReady.last_check).not.toBeNull();
    expect(demoDeviceReady.last_check?.installed).toBe(true);
    expect(demoDeviceReady.last_check?.connected).toBe(true);
    expect(demoDeviceReady.last_check?.tool).toBe(true);
    expect(demoDeviceReady.last_check?.verified_repository_count).toBe(
      demoDeviceReady.last_check?.declared_repository_count
    );
  });
});
