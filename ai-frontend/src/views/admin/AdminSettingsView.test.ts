// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import AdminSettingsView from "./AdminSettingsView.vue";
import { mountShell } from "@/components/shell/test-utils";

const getEmbeddingConfig = vi.fn();
const saveEmbeddingConfig = vi.fn();
const testEmbeddingConfig = vi.fn();

vi.mock("@/api/adminConfig", () => ({
  getEmbeddingConfig: (...args: unknown[]) => getEmbeddingConfig(...args),
  saveEmbeddingConfig: (...args: unknown[]) => saveEmbeddingConfig(...args),
  testEmbeddingConfig: (...args: unknown[]) => testEmbeddingConfig(...args)
}));

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: "/admin/settings", component: AdminSettingsView }]
  });
  await router.push("/admin/settings");
  await router.isReady();
  const { wrapper } = mountShell(
    AdminSettingsView,
    { attachTo: document.body },
    router
  );
  await flushPromises();
  return wrapper;
}

beforeEach(() => {
  vi.clearAllMocks();
  getEmbeddingConfig.mockResolvedValue({
    endpoint: "http://192.168.2.10:11434/v1",
    model: "bge-m3",
    enabled: false,
    updated_at: "2026-11-10T08:00:00Z"
  });
  saveEmbeddingConfig.mockImplementation((_body: unknown) =>
    Promise.resolve({
      endpoint: "http://192.168.2.10:11434/v1",
      model: "bge-m3",
      enabled: true,
      updated_at: "2026-11-10T09:00:00Z"
    })
  );
  testEmbeddingConfig.mockResolvedValue({
    ok: true,
    dims: 1024,
    latency_ms: 1234
  });
});

describe("AdminSettingsView", () => {
  it("loads and shows the current embedding config", async () => {
    const wrapper = await mountView();
    expect(getEmbeddingConfig).toHaveBeenCalledTimes(1);
    const inputs = wrapper.findAll(".settings-input input");
    expect((inputs[0].element as HTMLInputElement).value).toContain(
      "192.168.2.10:11434"
    );
    expect((inputs[1].element as HTMLInputElement).value).toBe("bge-m3");
  });

  it("saves edited endpoint and model", async () => {
    const wrapper = await mountView();
    const inputs = wrapper.findAll(".settings-input input");
    await inputs[0].setValue("http://127.0.0.1:11434/v1");
    await inputs[1].setValue("bge-m3");
    await wrapper.find(".admin-actions .admin-save").trigger("click");
    await flushPromises();
    expect(saveEmbeddingConfig).toHaveBeenCalledWith({
      endpoint: "http://127.0.0.1:11434/v1",
      model: "bge-m3",
      enabled: false
    });
    expect(wrapper.text()).toContain("已保存");
  });

  it("test connection reports dims and latency", async () => {
    const wrapper = await mountView();
    await wrapper.find(".admin-actions .admin-test").trigger("click");
    await flushPromises();
    expect(testEmbeddingConfig).toHaveBeenCalledTimes(1);
    expect(wrapper.text()).toContain("1024");
    expect(wrapper.text()).toContain("1234");
  });

  it("test connection failure shows the provider error", async () => {
    testEmbeddingConfig.mockResolvedValue({
      ok: false,
      dims: 0,
      latency_ms: 40,
      error: "provider returned HTTP 402"
    });
    const wrapper = await mountView();
    await wrapper.find(".admin-actions .admin-test").trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("provider returned HTTP 402");
  });
});
