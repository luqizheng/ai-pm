// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { HttpError } from "@digua/http";
import { mountShell } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";
import { buildRoutes } from "@/router";

import ProductLifecycleView from "./ProductLifecycleView.vue";

/**
 * 产品生命周期页契约（P4-ORG UX 修订 / Figma L1/L2 + D-abandon/D-restart）：
 *   - Active → Danger「Abandon Product」（确认弹窗后 setProductLifecycle）；
 *   - archived →「Restart Product」；
 *   - 无级联警告 + 关联执行上下文卡（Project/State）；非成员 → 受限态。
 */
const TEAM_ID = "00000000-0000-0000-0000-000000000010";
const PRODUCT_ID = "00000000-0000-0000-0000-000000000020";
const PROJECT_ID = "00000000-0000-0000-0000-000000000030";

const mountedWrappers: VueWrapper[] = [];

const fetchProduct = vi.fn();
const listProductMembers = vi.fn();
const listProductLinks = vi.fn();
const setProductLifecycle = vi.fn();
const listProjects = vi.fn();
const fetchTeamContext = vi.fn();

vi.mock("@/api/products", () => ({
  fetchProduct: (...args: unknown[]) => fetchProduct(...args),
  listProductMembers: (...args: unknown[]) => listProductMembers(...args),
  listProductLinks: (...args: unknown[]) => listProductLinks(...args),
  setProductLifecycle: (...args: unknown[]) => setProductLifecycle(...args),
  listProducts: vi.fn(),
  addProductMember: vi.fn(),
  removeProductMember: vi.fn(),
  appointProductOwner: vi.fn(),
  demoteProductOwner: vi.fn(),
  createProduct: vi.fn(),
  createProductLink: vi.fn(),
  endProductLink: vi.fn()
}));

vi.mock("@/api/pm", () => ({
  listProjects: (...args: unknown[]) => listProjects(...args)
}));

vi.mock("@/api/team", () => ({
  fetchTeamContext: (...args: unknown[]) => fetchTeamContext(...args)
}));

vi.mock("@/components/ui/toast", () => ({
  useToast: () => ({ toast: vi.fn() })
}));

function profile(status = "normal") {
  return {
    id: PRODUCT_ID,
    team_id: TEAM_ID,
    key: "AUTH",
    name: "Authentication service",
    description: null,
    status,
    updated_at: "2026-11-21T00:00:00Z"
  };
}

/** 确认弹窗为 Teleport 弹层：在 document 上按文案找弹窗内按钮。 */
function dialogButton(label: string): HTMLButtonElement | undefined {
  return Array.from(
    document.querySelectorAll<HTMLButtonElement>(".ui-confirm button")
  ).find(button => button.textContent?.trim() === label);
}

async function mountAt() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push(`/settings/team/products/${PRODUCT_ID}/lifecycle`);
  await router.isReady();
  const wrapper = mountShell(ProductLifecycleView, {
    global: { plugins: [router] }
  });
  mountedWrappers.push(wrapper);
  await flushPromises();
  return wrapper;
}

afterEach(() => {
  for (const wrapper of mountedWrappers.splice(0)) {
    wrapper.unmount();
  }
  document.body.innerHTML = "";
});

beforeEach(() => {
  vi.clearAllMocks();
  i18n.global.locale.value = "en";
  fetchTeamContext.mockResolvedValue({
    principal_id: "p-maya",
    display_name: "Maya Chen",
    is_active: true,
    is_admin: false,
    tenant_id: "00000000-0000-0000-0000-000000000099",
    team_role: "owner",
    team: {
      id: TEAM_ID,
      name: "Platform Engineering",
      slug: "PE",
      description: null
    },
    onboarding_required: false,
    memberships: [
      {
        team_id: TEAM_ID,
        team_name: "Platform Engineering",
        role: "owner",
        status: "active"
      }
    ]
  });
  fetchProduct.mockResolvedValue(profile());
  listProductMembers.mockResolvedValue([
    {
      principal_id: "p-maya",
      display_name: "Maya Chen",
      role: "owner",
      is_active: true,
      created_at: "x"
    }
  ]);
  listProductLinks.mockResolvedValue([
    {
      id: "l-1",
      product_id: PRODUCT_ID,
      project_id: PROJECT_ID,
      created_at: "x"
    }
  ]);
  listProjects.mockResolvedValue({
    items: [
      {
        id: PROJECT_ID,
        customer_id: "c-1",
        customer_name: "Acme",
        customer_status: "active",
        code: "ATLAS",
        name: "Atlas Platform",
        description: null,
        status: "active",
        revision: 1,
        created_at: "x",
        updated_at: "x"
      }
    ]
  });
});

describe("ProductLifecycleView", () => {
  it("abandons an active product through the D-abandon dialog", async () => {
    setProductLifecycle.mockResolvedValue(profile("archived"));
    const wrapper = await mountAt();
    expect(wrapper.find("[data-testid='lifecycle-decision']").text()).toContain(
      "Authentication service · AUTH"
    );
    expect(wrapper.text()).toContain("No cascading Project or Agent changes");
    expect(wrapper.find("[data-testid='lifecycle-context']").text()).toContain(
      "Atlas Platform"
    );
    await wrapper.find("[data-test='lifecycle-abandon']").trigger("click");
    await flushPromises();
    // 弹层 Teleport 到 body：断言与交互都走 document。
    expect(document.body.textContent).toContain("Abandon Product?");
    const confirmButton = dialogButton("Abandon Product");
    expect(confirmButton).toBeDefined();
    confirmButton!.click();
    await flushPromises();
    expect(setProductLifecycle).toHaveBeenCalledWith(PRODUCT_ID, "abandon");
  });

  it("offers Restart for an archived product (L2)", async () => {
    fetchProduct.mockResolvedValue(profile("archived"));
    setProductLifecycle.mockResolvedValue(profile());
    const wrapper = await mountAt();
    expect(wrapper.text()).toContain("Restart Product");
    expect(wrapper.find("[data-test='lifecycle-abandon']").exists()).toBe(
      false
    );
    await wrapper.find("[data-test='lifecycle-restart']").trigger("click");
    await flushPromises();
    const confirmButton = dialogButton("Restart Product");
    expect(confirmButton).toBeDefined();
    confirmButton!.click();
    await flushPromises();
    expect(setProductLifecycle).toHaveBeenCalledWith(PRODUCT_ID, "restart");
  });

  it("renders the restricted state for non-product-members (L0)", async () => {
    fetchProduct.mockRejectedValue(new HttpError("not_found", { status: 404 }));
    const wrapper = await mountAt();
    expect(wrapper.find("[data-testid='team-restricted-state']").exists()).toBe(
      true
    );
  });
});
