// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";

import type { Device } from "@/api/devices";
import {
  anomalyKey,
  bannerKeyPrefix,
  bannerVariant,
  checkRelativeTime,
  envMetrics,
  healthRows,
  readinessChainSteps,
  readinessVariant
} from "./domain";

function device(overrides: Partial<Device> = {}): Device {
  return {
    id: "dev-1",
    tenant_id: "tenant-1",
    owner_principal_id: "p-1",
    name: "Windows Dev PC",
    platform: "windows",
    os_version: "11",
    executor_kind: "codex",
    executor_version: "0.163",
    status: "active",
    readiness: "ready",
    last_check_at: "2026-11-05T00:00:00Z",
    last_check: { installed: true, connected: true, tool: true },
    revoked_at: null,
    created_at: "2026-11-01T00:00:00Z",
    updated_at: "2026-11-05T00:00:00Z",
    ...overrides
  };
}

describe("readinessChainSteps", () => {
  it("ready 设备全链通过，终态 = Ready", () => {
    const steps = readinessChainSteps(device({ readiness: "ready" }));
    expect(steps.map(step => step.state)).toEqual([
      "pass",
      "pass",
      "pass",
      "pass"
    ]);
    expect(steps[3].labelKey).toBe("environments.chain.ready");
  });

  it("readiness=connected 时前两步通过、认证等待", () => {
    const steps = readinessChainSteps(
      device({ readiness: "connected", last_check: null })
    );
    expect(steps.map(step => step.state)).toEqual([
      "pass",
      "pass",
      "pending",
      "pending"
    ]);
  });

  it("connected=false 的证据把第二步骤渲染为断连（红线稿）", () => {
    const steps = readinessChainSteps(
      device({
        readiness: "degraded",
        last_check: { installed: true, connected: false }
      })
    );
    expect(steps[0]).toMatchObject({ state: "pass" });
    expect(steps[1]).toMatchObject({
      state: "fail",
      labelKey: "environments.chain.disconnected"
    });
    expect(steps[2].state).toBe("unknown");
    expect(steps[3]).toMatchObject({
      state: "warning",
      labelKey: "environments.chain.degraded"
    });
  });

  it("未自检设备全链 unknown", () => {
    const steps = readinessChainSteps(
      device({ readiness: null, last_check: null })
    );
    expect(steps.every(step => step.state === "unknown")).toBe(true);
  });

  it("已吊销设备终态 = Revoked（fail）", () => {
    const steps = readinessChainSteps(device({ status: "revoked" }));
    expect(steps[3]).toMatchObject({
      state: "fail",
      labelKey: "environments.chain.revoked"
    });
  });
});

describe("envMetrics / anomaly / variant", () => {
  const devices = [
    device({ id: "a", readiness: "ready" }),
    device({ id: "b", readiness: "degraded" }),
    device({ id: "c", readiness: "failed" }),
    device({ id: "d", status: "revoked", readiness: null, last_check: null }),
    device({ id: "e", readiness: null, last_check: null })
  ];

  it("指标 = 总数 / 就绪 / 异常（降级+失败+吊销）", () => {
    expect(envMetrics(devices)).toEqual({ total: 5, ready: 1, trouble: 3 });
  });

  it("异常行：degraded/failed/revoked 各有文案键，正常设备无异常行", () => {
    expect(anomalyKey(devices[0])).toBeNull();
    expect(anomalyKey(devices[1])).toBe("environments.anomaly.degraded");
    expect(anomalyKey(devices[2])).toBe("environments.anomaly.failed");
    expect(anomalyKey(devices[3])).toBe("environments.anomaly.revoked");
  });

  it("readiness → 语义变体", () => {
    expect(readinessVariant(devices[0])).toBe("success");
    expect(readinessVariant(devices[1])).toBe("danger");
    expect(readinessVariant(devices[3])).toBe("danger");
    expect(readinessVariant(devices[4])).toBe("neutral");
  });
});

describe("checkRelativeTime", () => {
  const now = Date.parse("2026-11-07T12:00:00Z");

  it("缺自检 → never", () => {
    expect(checkRelativeTime(null, now).key).toBe("environments.time.never");
  });

  it("1 分钟内 → justNow，12 分钟 → minutesAgo(12)，超 24h → daysAgo", () => {
    expect(checkRelativeTime("2026-11-07T11:59:30Z", now).key).toBe(
      "environments.time.justNow"
    );
    const minutes = checkRelativeTime("2026-11-07T11:48:00Z", now);
    expect(minutes).toEqual({
      key: "environments.time.minutesAgo",
      n: 12
    });
    expect(checkRelativeTime("2026-11-05T00:30:00Z", now)).toEqual({
      key: "environments.time.daysAgo",
      n: 2
    });
  });
});

describe("healthRows", () => {
  it("证据齐全 → 五行按证据渲染，仓库部分验证 = warning", () => {
    const rows = healthRows(
      device({
        readiness: "ready",
        last_check: {
          installed: true,
          connected: true,
          tool: true,
          verified_repository_count: 1,
          declared_repository_count: 2
        }
      })
    );
    expect(rows.map(row => row.state)).toEqual([
      "pass",
      "pass",
      "pass",
      "warning",
      "pass"
    ]);
    expect(rows[3].coverage).toEqual({ verified: 1, declared: 2 });
  });

  it("证据缺失不越级推断：readiness=installed 不代表 tool 通过", () => {
    const rows = healthRows(
      device({ readiness: "installed", last_check: null })
    );
    expect(rows[0].state).toBe("pass");
    expect(rows[1].state).toBe("unknown");
    expect(rows[2].state).toBe("unknown");
    expect(rows[4].state).toBe("unknown");
  });
});

describe("banner", () => {
  it("终态横幅：ready=success / degraded=warning 语义 / failed=danger", () => {
    expect(bannerVariant(device({ readiness: "ready" }))).toBe("success");
    expect(bannerKeyPrefix(device({ readiness: "degraded" }))).toBe(
      "environments.banner.degraded"
    );
    expect(bannerVariant(device({ readiness: "failed" }))).toBe("danger");
    expect(bannerKeyPrefix(device({ status: "revoked" }))).toBe(
      "environments.banner.revoked"
    );
  });
});
