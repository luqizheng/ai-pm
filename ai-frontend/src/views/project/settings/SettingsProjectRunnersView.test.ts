// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DOMWrapper, flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import SettingsProjectRunnersView from "./SettingsProjectRunnersView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import type { Device } from "@/api/devices";

const listDevices = vi.fn();
const registerDevice = vi.fn();

vi.mock("@/api/devices", () => ({
  listDevices: (...args: unknown[]) => listDevices(...args),
  getDevice: vi.fn(),
  registerDevice: (...args: unknown[]) => registerDevice(...args),
  bindDevice: vi.fn(),
  unbindDevice: vi.fn(),
  reportDeviceChecks: vi.fn(),
  revokeDevice: vi.fn()
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
    last_check_at: "2026-11-07T00:00:00Z",
    last_check: { installed: true, connected: true, tool: true },
    revoked_at: null,
    created_at: "2026-11-01T00:00:00Z",
    updated_at: "2026-11-05T00:00:00Z",
    ...overrides
  };
}

async function mountView(path = "/project/settings/runners") {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  router.push(path);
  await router.isReady();
  const mounted = mountShell(SettingsProjectRunnersView, {}, router);
  await flushPromises();
  return mounted;
}

function findButtonByText(
  wrapper: { findAll: (sel: string) => { text: () => string }[] },
  text: string
) {
  const target = wrapper
    .findAll("button")
    .find(b => b.text().trim().includes(text));
  expect(target, `button "${text}" should exist`).toBeTruthy();
  return target!;
}

/** Dialog 内容经 Teleport 渲染在 document.body，按钮从这里找 */
function findBodyButton(text: string): DOMWrapper<Element> {
  const el = Array.from(document.querySelectorAll("button")).find(b =>
    b.textContent?.trim().includes(text)
  );
  expect(el, `body button "${text}" should exist`).toBeTruthy();
  return new DOMWrapper(el!);
}

describe("SettingsProjectRunnersView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listDevices.mockResolvedValue({ items: [], total: 0 });
  });

  it("指标三件套来自列表派生（总数 / Execution Ready / 异常）", async () => {
    listDevices.mockResolvedValue({
      items: [
        device({ id: "a", readiness: "ready" }),
        device({ id: "b", readiness: "degraded" }),
        device({ id: "c", readiness: null, last_check: null })
      ],
      total: 3
    });
    const { wrapper } = await mountView();
    const metrics = wrapper.find('[data-testid="env-metrics"]');
    expect(metrics.exists()).toBe(true);
    expect(metrics.text()).toContain("设备总数");
    expect(metrics.text()).toContain("3");
    expect(metrics.text()).toContain("1");
    // 三张设备卡 + 服务端就绪快照的阶段链（线稿：Installed → … → 终态）
    expect(wrapper.findAll('[data-testid="env-card"]')).toHaveLength(3);
    expect(wrapper.text()).toContain("Execution Ready");
    expect(wrapper.text()).toContain("Degraded");
    expect(wrapper.text()).toContain("Unchecked");
  });

  it("设备卡点击进入 Runner 详情路由", async () => {
    listDevices.mockResolvedValue({
      items: [device({ id: "dev-9", readiness: "ready" })],
      total: 1
    });
    const { wrapper, router } = await mountView();
    await wrapper.find('[data-testid="env-card-open-dev-9"]').trigger("click");
    await flushPromises();
    expect(router.currentRoute.value.path).toBe(
      "/project/settings/runners/dev-9"
    );
  });

  it("带项目上下文时进入详情保留 project_id（设置各节继承当前项目）", async () => {
    listDevices.mockResolvedValue({
      items: [device({ id: "dev-9", readiness: "ready" })],
      total: 1
    });
    const { wrapper, router } = await mountView(
      "/project/settings/runners?project_id=prj-1"
    );
    await wrapper.find('[data-testid="env-card-open-dev-9"]').trigger("click");
    await flushPromises();
    expect(router.currentRoute.value.path).toBe(
      "/project/settings/runners/dev-9"
    );
    expect(router.currentRoute.value.query.project_id).toBe("prj-1");
  });

  it("搜索关键字过滤设备卡（SearchInput 原生输入）", async () => {
    listDevices.mockResolvedValue({
      items: [
        device({ id: "dev-win", name: "Windows Dev PC" }),
        device({ id: "dev-mac", name: "MacBook Pro", platform: "macos" })
      ],
      total: 2
    });
    const { wrapper } = await mountView();
    expect(wrapper.findAll('[data-testid="env-card"]')).toHaveLength(2);
    await wrapper.find("input[type=search]").setValue("MacBook");
    await flushPromises();
    const cards = wrapper.findAll('[data-testid="env-card"]');
    expect(cards).toHaveLength(1);
    expect(cards[0].text()).toContain("dev-mac");
  });

  it("零设备空态给出登记引导，入口动作打开添加向导", async () => {
    const { wrapper } = await mountView();
    const empty = wrapper.find('[data-testid="env-empty"]');
    expect(empty.exists()).toBe(true);
    expect(empty.text()).toContain("尚未登记执行设备");
    await findButtonByText(wrapper, "添加设备").trigger("click");
    await flushPromises();
    // Dialog 走 Teleport：向导步骤渲染在 document.body
    expect(
      document.querySelector('[data-testid="env-wizard-step-1"]')
    ).toBeTruthy();
  });

  it("向导三步登记走真实登记契约（平台 → 安装 → 注册）", async () => {
    registerDevice.mockResolvedValue(device({ id: "dev-new" }));
    const { wrapper } = await mountView();
    await findButtonByText(wrapper, "添加设备").trigger("click");
    await flushPromises();

    // ① 平台卡选择
    await new DOMWrapper(
      document.querySelector('[data-testid="env-wizard-platform-linux"]')!
    ).trigger("click");
    await findBodyButton("下一步").trigger("click");

    // ② 安装引导（命令块展示引导地址）
    expect(
      document.querySelector('[data-testid="env-wizard-command"]')?.textContent
    ).toContain("/api/install/codex");
    expect(document.body.textContent).toContain("安装成功 ≠ 注册完成");
    await findBodyButton("下一步").trigger("click");

    // ③ 注册表单 → 真实契约
    await new DOMWrapper(
      document.querySelector('[data-testid="env-wizard-name"]')!
    ).setValue("Linux Box");
    await new DOMWrapper(
      document.querySelector('[data-testid="env-wizard-os"]')!
    ).setValue("22.04");
    await findBodyButton("登记设备").trigger("click");
    await flushPromises();

    expect(registerDevice).toHaveBeenCalledWith({
      name: "Linux Box",
      platform: "linux",
      os_version: "22.04",
      executor_kind: "codex",
      executor_version: undefined
    });
    expect(
      document.querySelector('[data-testid="env-wizard-success"]')
    ).toBeTruthy();
  });
});
