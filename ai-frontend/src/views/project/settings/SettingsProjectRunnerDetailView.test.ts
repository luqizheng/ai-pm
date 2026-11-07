// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import SettingsProjectRunnerDetailView from "./SettingsProjectRunnerDetailView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import type { DeviceBinding, DeviceDetail } from "@/api/devices";

const getDevice = vi.fn();
const bindDevice = vi.fn();
const unbindDevice = vi.fn();
const revokeDevice = vi.fn();
const listProjects = vi.fn();

vi.mock("@/api/devices", () => ({
  listDevices: vi.fn(),
  getDevice: (...args: unknown[]) => getDevice(...args),
  registerDevice: vi.fn(),
  bindDevice: (...args: unknown[]) => bindDevice(...args),
  unbindDevice: (...args: unknown[]) => unbindDevice(...args),
  reportDeviceChecks: vi.fn(),
  revokeDevice: (...args: unknown[]) => revokeDevice(...args)
}));

// scope store 的项目目录（绑定项目名映射用）
vi.mock("@/api/pm", () => ({
  listProjects: (...args: unknown[]) => listProjects(...args)
}));

function binding(overrides: Partial<DeviceBinding>): DeviceBinding {
  return {
    id: "bind-1",
    device_id: "dev-1",
    project_id: "prj-1",
    declared_repository_ids: ["repo-a", "repo-b"],
    verified_repositories: ["repo-a"],
    bound_by: "p-1",
    bound_at: "2026-11-02T00:00:00Z",
    unbound_at: null,
    ...overrides
  };
}

function detail(overrides: Partial<DeviceDetail> = {}): DeviceDetail {
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
    last_check_at: "2026-11-07T00:00:00Z",
    last_check: {
      installed: true,
      connected: true,
      tool: true,
      verified_repository_count: 1,
      declared_repository_count: 2
    },
    revoked_at: null,
    created_at: "2026-11-01T00:00:00Z",
    updated_at: "2026-11-05T00:00:00Z",
    bindings: [binding({})],
    ...overrides
  };
}

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  router.push("/project/settings/runners/dev-1");
  await router.isReady();
  const mounted = mountShell(SettingsProjectRunnerDetailView, {}, router);
  await flushPromises();
  return mounted;
}

function findButtonByText(
  wrapper: { findAll: (sel: string) => { text: () => string }[] },
  text: string
) {
  const target = wrapper.findAll("button").find(b => b.text().trim() === text);
  expect(target, `button "${text}" should exist`).toBeTruthy();
  return target!;
}

describe("SettingsProjectRunnerDetailView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getDevice.mockResolvedValue(detail());
    listProjects.mockResolvedValue({
      items: [{ id: "prj-1", name: "PM-AI", code: "PM" }],
      total: 1
    });
  });

  it("就绪横幅 + 健康检查五行（证据只读，仓库覆盖带计数）", async () => {
    const { wrapper } = await mountView();
    const banner = wrapper.find('[data-testid="env-banner"]');
    expect(banner.exists()).toBe(true);
    expect(banner.text()).toContain("Execution Ready");

    const health = wrapper.find('[data-testid="env-health"]');
    expect(health.text()).toContain("Runner Installed");
    expect(health.text()).toContain("Repository Permission");
    expect(health.text()).toContain("仓库验证覆盖 1/2");
    expect(health.text()).toContain("Authentication");
  });

  it("项目绑定渲染映射后的项目名；解绑走真实解绑契约", async () => {
    const { wrapper } = await mountView();
    const bindings = wrapper.find('[data-testid="env-bindings"]');
    expect(bindings.text()).toContain("PM-AI");
    await findButtonByText(wrapper, "解绑").trigger("click");
    await flushPromises();
    expect(unbindDevice).toHaveBeenCalledWith("dev-1", "bind-1");
  });

  it("吊销需显式确认；确认后走真实吊销契约", async () => {
    revokeDevice.mockResolvedValue(detail({ status: "revoked" }));
    const { wrapper } = await mountView();
    await wrapper.find('[data-testid="env-revoke-open"]').trigger("click");
    await flushPromises();

    // ConfirmationDialog 走 Teleport 且为多根组件（attrs 不继承）：
    // 断言落在 document.body 的 .ui-confirm 面板
    const dialog = document.querySelector(".ui-confirm");
    expect(dialog?.textContent).toContain("撤销设备 dev-1 的授权？");

    const confirmButton = Array.from(document.querySelectorAll("button")).find(
      b => b.textContent?.trim() === "撤销授权"
    );
    expect(confirmButton).toBeTruthy();
    confirmButton!.click();
    await flushPromises();
    expect(revokeDevice).toHaveBeenCalledWith("dev-1");
  });

  it("已吊销设备：横幅=danger 语义，解绑与绑定入口收起", async () => {
    getDevice.mockResolvedValue(
      detail({
        status: "revoked",
        readiness: null,
        last_check: null,
        revoked_at: "2026-11-06T00:00:00Z",
        bindings: []
      })
    );
    const { wrapper } = await mountView();
    expect(wrapper.find('[data-testid="env-banner"]').text()).toContain(
      "已吊销"
    );
    expect(wrapper.find('[data-testid="env-bind-current"]').exists()).toBe(
      false
    );
    expect(wrapper.find('[data-testid="env-danger"]').text()).toContain(
      "设备已吊销"
    );
  });

  it("设备不可见/不存在 → 错误提示", async () => {
    getDevice.mockRejectedValue(new Error("device not found"));
    const { wrapper } = await mountView();
    expect(wrapper.text()).toContain("device not found");
    expect(wrapper.find('[data-testid="env-health"]').exists()).toBe(false);
  });
});
