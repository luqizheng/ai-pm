// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { HttpError } from "@digua/http";
import { mountShell } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";
import { buildRoutes } from "@/router";

import ProductsListView from "./ProductsListView.vue";

/**
 * 产品列表页契约（P4-ORG UX 修订 / Figma P1 + P0 + PX）：
 *   - 列 = Product/key · Lifecycle · Owner · Members · Projects · Updated；
 *   - Owner/Members/Projects 不可读（非产品成员 404）→ 如实「—」；
 *   - 创建入口仅 Team Owner；P0 空态；PX 403 → 受限态。
 */
const TEAM_ID = "00000000-0000-0000-0000-000000000010";
const PRODUCT_ID = "00000000-0000-0000-0000-000000000020";

const mountedWrappers: VueWrapper[] = [];

const listProducts = vi.fn();
const listProductMembers = vi.fn();
const listProductLinks = vi.fn();
const fetchTeamContext = vi.fn();

vi.mock("@/api/products", () => ({
  listProducts: (...args: unknown[]) => listProducts(...args),
  listProductMembers: (...args: unknown[]) => listProductMembers(...args),
  listProductLinks: (...args: unknown[]) => listProductLinks(...args),
  fetchProduct: vi.fn(),
  addProductMember: vi.fn(),
  removeProductMember: vi.fn(),
  appointProductOwner: vi.fn(),
  demoteProductOwner: vi.fn(),
  setProductLifecycle: vi.fn(),
  createProduct: vi.fn(),
  createProductLink: vi.fn(),
  endProductLink: vi.fn()
}));

vi.mock("@/api/team", () => ({
  listTeamMembers: vi.fn(),
  fetchTeamContext: (...args: unknown[]) => fetchTeamContext(...args)
}));

function product(overrides: Record<string, unknown> = {}) {
  return {
    id: PRODUCT_ID,
    team_id: TEAM_ID,
    key: "AUTH",
    name: "Authentication service",
    description: null,
    status: "normal",
    updated_at: "2026-11-21T10:48:00Z",
    ...overrides
  };
}

async function mountAt(path = "/settings/team/products") {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push(path);
  await router.isReady();
  const wrapper = mountShell(ProductsListView, {
    global: { plugins: [router] }
  });
  mountedWrappers.push(wrapper);
  await flushPromises();
  return wrapper;
}

function contextFor(role: "owner" | "member") {
  return {
    principal_id: "p-owner",
    display_name: "Maya Chen",
    is_active: true,
    is_admin: false,
    tenant_id: "00000000-0000-0000-0000-000000000099",
    team_role: role,
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
        role,
        status: "active"
      }
    ]
  };
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
  fetchTeamContext.mockResolvedValue(contextFor("owner"));
  listProducts.mockResolvedValue([product()]);
});

describe("ProductsListView", () => {
  it("renders the product table with readable member/link facts", async () => {
    listProductMembers.mockResolvedValue([
      {
        principal_id: "p-1",
        display_name: "Maya Chen",
        role: "owner",
        is_active: true,
        created_at: "x"
      },
      {
        principal_id: "p-2",
        display_name: "Alex",
        role: "member",
        is_active: true,
        created_at: "x"
      }
    ]);
    listProductLinks.mockResolvedValue([
      {
        id: "l-1",
        product_id: PRODUCT_ID,
        project_id: "proj-1",
        created_at: "x"
      },
      {
        id: "l-2",
        product_id: PRODUCT_ID,
        project_id: "proj-2",
        created_at: "x"
      }
    ]);
    const wrapper = await mountAt();
    expect(listProducts).toHaveBeenCalledWith(TEAM_ID);
    const row = wrapper.find(`[data-testid='products-row-${PRODUCT_ID}']`);
    expect(row.text()).toContain("Authentication service / AUTH");
    expect(row.text()).toContain("Active");
    expect(row.text()).toContain("Maya Chen");
    expect(row.text()).toContain("2"); // members 与 projects 计数
    // P1 顶部摘要 + 创建入口（Owner）。
    expect(wrapper.find("[data-testid='products-summary']").text()).toBe(
      "Platform Engineering · 1 Products"
    );
    expect(wrapper.find("[data-test='products-create']").exists()).toBe(true);
    expect(wrapper.text()).toContain("Discovery is not access");
  });

  it("shows em dash for products whose member table is not readable", async () => {
    listProductMembers.mockRejectedValue(
      new HttpError("not_found", { status: 404 })
    );
    listProductLinks.mockRejectedValue(
      new HttpError("not_found", { status: 404 })
    );
    const wrapper = await mountAt();
    const row = wrapper.find(`[data-testid='products-row-${PRODUCT_ID}']`);
    expect(row.text()).toContain("—");
  });

  it("hides Create Product for plain team members", async () => {
    fetchTeamContext.mockResolvedValue(contextFor("member"));
    const wrapper = await mountAt();
    expect(wrapper.find("[data-test='products-create']").exists()).toBe(false);
  });

  it("renders the empty state for a team without products (P0)", async () => {
    listProducts.mockResolvedValue([]);
    const wrapper = await mountAt();
    expect(wrapper.text()).toContain("No products yet");
  });

  it("renders the restricted state when product access is forbidden (PX)", async () => {
    listProducts.mockRejectedValue(new HttpError("forbidden", { status: 403 }));
    const wrapper = await mountAt();
    expect(wrapper.find("[data-testid='team-restricted-state']").exists()).toBe(
      true
    );
  });
});
