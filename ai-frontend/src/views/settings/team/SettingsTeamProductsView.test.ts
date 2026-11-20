// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { mountShell } from "@/lib/test-utils";
import { buildRoutes } from "@/router";

import SettingsTeamProductsView from "./SettingsTeamProductsView.vue";

/**
 * 产品管理页契约（P4-ORG Phase 3 / DR-PA-01~03）：
 *   - 列表：Team Owner 全量；普通 Team Member 仅参与的（信息路由）；
 *   - 创建（Team Owner）：显式指定首位 Product Owner；
 *   - 生命周期按钮（abandon/restart）；
 *   - 任命 Owner=继任指定（多 Owner 并存）。
 */
const TEAM_ID = "00000000-0000-0000-0000-000000000010";
const PRODUCT_ID = "00000000-0000-0000-0000-000000000020";

const mountedWrappers: VueWrapper[] = [];

const listProducts = vi.fn();
const createProduct = vi.fn();
const fetchProduct = vi.fn();
const listProductMembers = vi.fn();
const listProductLinks = vi.fn();
const setProductLifecycle = vi.fn();
const appointProductOwner = vi.fn();
const listTeamMembers = vi.fn();
const fetchTeamContext = vi.fn();

vi.mock("@/api/products", () => ({
  listProducts: (...args: unknown[]) => listProducts(...args),
  createProduct: (...args: unknown[]) => createProduct(...args),
  fetchProduct: (...args: unknown[]) => fetchProduct(...args),
  listProductMembers: (...args: unknown[]) => listProductMembers(...args),
  listProductLinks: (...args: unknown[]) => listProductLinks(...args),
  setProductLifecycle: (...args: unknown[]) => setProductLifecycle(...args),
  appointProductOwner: (...args: unknown[]) => appointProductOwner(...args),
  addProductMember: vi.fn(),
  removeProductMember: vi.fn(),
  demoteProductOwner: vi.fn(),
  createProductLink: vi.fn(),
  endProductLink: vi.fn()
}));

vi.mock("@/api/team", () => ({
  listTeamMembers: (...args: unknown[]) => listTeamMembers(...args),
  fetchTeamContext: (...args: unknown[]) => fetchTeamContext(...args)
}));

vi.mock("@/components/ui/toast", () => ({
  useToast: () => ({ toast: () => {} })
}));

function context(role: "owner" | "member") {
  return {
    principal_id: "p-owner",
    display_name: role,
    is_active: true,
    is_admin: role === "owner",
    tenant_id: "00000000-0000-0000-0000-000000000099",
    team_role: role,
    team: { id: TEAM_ID, name: "Acme R&D", slug: "acme-rd", description: null },
    onboarding_required: false,
    memberships: [
      { team_id: TEAM_ID, team_name: "Acme R&D", role, status: "active" }
    ]
  };
}

function product(status: "active" | "abandoned" = "active") {
  return {
    id: PRODUCT_ID,
    team_id: TEAM_ID,
    name: "Alpha",
    description: "first",
    status
  };
}

async function mountAt(path = "/settings/team/products") {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push(path);
  await router.isReady();
  const wrapper = mountShell(SettingsTeamProductsView, {
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
  fetchTeamContext.mockResolvedValue(context("owner"));
  listTeamMembers.mockResolvedValue([
    { principal_id: "p-owner", display_name: "owner", role: "owner", is_active: true, created_at: "x" },
    { principal_id: "p-bob", display_name: "bob", role: "member", is_active: true, created_at: "x" }
  ]);
  listProducts.mockResolvedValue([product()]);
  fetchProduct.mockResolvedValue(product());
  listProductMembers.mockResolvedValue([
    { principal_id: "p-owner", display_name: "owner", role: "owner", is_active: true, created_at: "x" },
    { principal_id: "p-bob", display_name: "bob", role: "member", is_active: true, created_at: "x" }
  ]);
  listProductLinks.mockResolvedValue([]);
});

describe("SettingsTeamProductsView", () => {
  it("as OWNER lists products, shows create action, and detail loads", async () => {
    const wrapper = await mountAt();
    expect(listProducts).toHaveBeenCalledWith(TEAM_ID);
    expect(wrapper.find("[data-test='team-products-create']").exists()).toBe(
      true
    );
    expect(wrapper.text()).toContain("Alpha");
    expect(wrapper.find("[data-test='product-abandon']").exists()).toBe(true);
  });

  it("as OWNER creating sends explicit first owner", async () => {
    const wrapper = await mountAt();
    createProduct.mockResolvedValue(product());
    await wrapper.find("[data-test='team-products-create']").trigger("click");
    await flushPromises();
    // Dialog 经 reka Portal 挂 document.body，wrapper.find 不可达。
    const nameEl = document.body.querySelector(
      "[data-test='product-create-name'] input"
    ) as HTMLInputElement | null;
    expect(nameEl).not.toBeNull();
    nameEl!.value = "Beta";
    nameEl!.dispatchEvent(new Event("input"));
    const ownerEl = document.body.querySelector(
      "[data-test='product-create-owner']"
    ) as HTMLSelectElement | null;
    expect(ownerEl).not.toBeNull();
    ownerEl!.value = "p-bob";
    ownerEl!.dispatchEvent(new Event("change"));
    await flushPromises();
    const confirmEl = document.body.querySelector(
      "[data-test='product-create-confirm']"
    ) as HTMLButtonElement | null;
    expect(confirmEl).not.toBeNull();
    await new (await import("@vue/test-utils")).DOMWrapper(confirmEl!).trigger("click");
    await flushPromises();
    expect(createProduct).toHaveBeenCalledWith(
      TEAM_ID,
      expect.objectContaining({
        name: "Beta",
        first_owner_principal_id: "p-bob"
      })
    );
  });

  it("abandon calls lifecycle and swaps to restart", async () => {
    const wrapper = await mountAt();
    setProductLifecycle.mockResolvedValue(product("abandoned"));
    await wrapper.find("[data-test='product-abandon']").trigger("click");
    await flushPromises();
    expect(setProductLifecycle).toHaveBeenCalledWith(PRODUCT_ID, "abandon");
    expect(wrapper.find("[data-test='product-restart']").exists()).toBe(true);
  });

  it("as MEMBER sees only participating products and no management actions", async () => {
    fetchTeamContext.mockResolvedValue(context("member"));
    listProducts.mockResolvedValue([]); // Member 视角：未参与 → 空列表（信息路由）。
    const wrapper = await mountAt();
    expect(listProducts).toHaveBeenCalledWith(TEAM_ID);
    expect(wrapper.find("[data-test='team-products-create']").exists()).toBe(
      false
    );
    expect(wrapper.find("[data-test='products-empty']").exists()).toBe(true);
    expect(wrapper.find("[data-test='product-abandon']").exists()).toBe(false);
  });

  it("appoint owner button per member row", async () => {
    const wrapper = await mountAt();
    appointProductOwner.mockResolvedValue(undefined);
    await wrapper.find("[data-test='product-appoint-p-bob']").trigger("click");
    await flushPromises();
    expect(appointProductOwner).toHaveBeenCalledWith(PRODUCT_ID, "p-bob");
  });
});
