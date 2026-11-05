// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import BootstrapView from "./BootstrapView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import type { Device } from "@/api/devices";

const listDevices = vi.fn();
const getDevice = vi.fn();
const registerDevice = vi.fn();
const bindDevice = vi.fn();
const unbindDevice = vi.fn();
const revokeDevice = vi.fn();

vi.mock("@/api/devices", () => ({
  listDevices: (...args: unknown[]) => listDevices(...args),
  getDevice: (...args: unknown[]) => getDevice(...args),
  registerDevice: (...args: unknown[]) => registerDevice(...args),
  bindDevice: (...args: unknown[]) => bindDevice(...args),
  unbindDevice: (...args: unknown[]) => unbindDevice(...args),
  revokeDevice: (...args: unknown[]) => revokeDevice(...args)
}));

function device(overrides: Partial<Device>): Device {
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

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  router.push("/project/devenv?project_id=prj-1");
  await router.isReady();
  const mounted = mountShell(BootstrapView, {}, router);
  await flushPromises();
  return mounted;
}

function findButtonByText(
  wrapper: { findAll: (sel: string) => { text: () => string }[] },
  text: string
) {
  const target = wrapper.findAll("button").find(b => b.text().includes(text));
  expect(target, `button "${text}" should exist`).toBeTruthy();
  return target!;
}

describe("BootstrapView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listDevices.mockResolvedValue({ items: [], total: 0 });
  });

  it("零设备空态明确传达「未接入 ≠ 不可用」（R-S01-01-01）", async () => {
    const { wrapper } = await mountView();
    const empty = wrapper.find('[data-testid="devenv-empty"]');
    expect(empty.exists()).toBe(true);
    expect(empty.text()).toContain("尚未登记执行设备");
    expect(empty.text()).toContain("项目已可以开展工作");
  });

  it("登记设备走真实登记契约（首次登记缺省 device_id 由服务端生成）", async () => {
    registerDevice.mockResolvedValue(device({}));
    const { wrapper } = await mountView();
    await findButtonByText(wrapper, "登记设备").trigger("click");
    await wrapper.find('[data-testid="register-name"]').setValue("Linux Box");
    await wrapper
      .find('[data-testid="register-platform"]')
      .setValue("linux");
    await wrapper.find('[data-testid="register-submit"]').trigger("click");
    await flushPromises();

    expect(registerDevice).toHaveBeenCalledWith({
      name: "Linux Box",
      platform: "linux",
      os_version: null,
      executor_kind: "codex",
      executor_version: undefined
    });
  });

  it("readiness 六层只读展示 + 绑定当前项目走真实绑定端点", async () => {
    listDevices.mockResolvedValue({
      items: [device({ id: "dev-ready", readiness: "ready" })],
      total: 1
    });
    getDevice.mockResolvedValue({
      ...device({ id: "dev-ready" }),
      bindings: []
    });
    bindDevice.mockResolvedValue({});
    const { wrapper } = await mountView();

    // readiness = ready 徽标（服务端计算快照，页面只读）
    expect(wrapper.text()).toContain("执行就绪");
    await findButtonByText(wrapper, "详情").trigger("click");
    await flushPromises();
    expect(wrapper.find('[data-testid="device-detail"]').exists()).toBe(true);
    expect(getDevice).toHaveBeenCalledWith("dev-ready");

    await findButtonByText(wrapper, "绑定当前项目").trigger("click");
    await flushPromises();
    expect(bindDevice).toHaveBeenCalledWith("dev-ready", {
      project_id: "prj-1"
    });
  });
});
