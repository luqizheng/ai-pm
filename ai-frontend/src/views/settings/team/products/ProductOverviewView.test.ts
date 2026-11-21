// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { HttpError } from "@digua/http";
import { mountShell } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";
import { buildRoutes } from "@/router";

import ProductOverviewView from "./ProductOverviewView.vue";

/**
 * 产品概览页契约（P4-ORG UX 修订 / Figma O1；非成员 = O2 只读受限态）：
 *   - 徽标行 [key][Lifecycle][You · Product 身份]；
 *   - 概览卡 meta（Owner / 成员数 / 关联数）；
 *   - Associated Projects：用户可见项目 → Open Project，否则 Restricted 文本；
 *   - 非成员 404 → 受限态。
 */
const TEAM_ID = "00000000-0000-0000-0000-000000000010";
const PRODUCT_ID = "00000000-0000-0000-0000-000000000020";
const PROJECT_ID = "00000000-0000-0000-0000-000000000030";

const mountedWrappers: VueWrapper[] = [];

const fetchProduct = vi.fn();
const listProductMembers = vi.fn();
const listProductLinks = vi.fn();
const listProjects = vi.fn();
const fetchTeamContext = vi.fn();

vi.mock("@/api/products", () => ({
  fetchProduct: (...args: unknown[]) => fetchProduct(...args),
  listProductMembers: (...args: unknown[]) => listProductMembers(...args),
  listProductLinks: (...args: unknown[]) => listProductLinks(...args),
  listProducts: vi.fn(),
  addProductMember: vi.fn(),
  removeProductMember: vi.fn(),
  appointProductOwner: vi.fn(),
  demoteProductOwner: vi.fn(),
  setProductLifecycle: vi.fn(),
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

function profile() {
  return {
    id: PRODUCT_ID,
    team_id: TEAM_ID,
    key: "AUTH",
    name: "Authentication service",
    description: "Secure identity services.",
    status: "normal",
    updated_at: "2026-11-21T00:00:00Z"
  };
}

async function mountAt() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push(`/settings/team/products/${PRODUCT_ID}`);
  await router.isReady();
  const wrapper = mountShell(ProductOverviewView, {
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
    },
    {
      principal_id: "p-2",
      display_name: "Alex Morgan",
      role: "member",
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

describe("ProductOverviewView", () => {
  it("renders badges, overview meta and per-project access (O1)", async () => {
    const wrapper = await mountAt();
    const badges = wrapper.find("[data-testid='product-badges']");
    expect(badges.text()).toContain("AUTH");
    expect(badges.text()).toContain("Active");
    expect(badges.text()).toContain("You · Product Owner");
    const meta = wrapper.find("[data-testid='product-overview-meta']");
    expect(meta.text()).toContain("Product Owner · Maya Chen");
    expect(meta.text()).toContain("Product Members · 2");
    expect(meta.text()).toContain("Associated Projects · 1");
    const association = wrapper.find("[data-testid='association-l-1']");
    expect(association.text()).toContain("Atlas Platform");
    expect(wrapper.find("[data-test='association-open-l-1']").exists()).toBe(
      true
    );
  });

  it("renders Restricted for projects outside the viewer's visible set", async () => {
    listProjects.mockResolvedValue({ items: [] });
    const wrapper = await mountAt();
    const association = wrapper.find("[data-testid='association-l-1']");
    expect(association.text()).toContain("Restricted");
    expect(wrapper.find("[data-test='association-open-l-1']").exists()).toBe(
      false
    );
  });

  it("renders the restricted state for non-product-members (O2)", async () => {
    fetchProduct.mockRejectedValue(new HttpError("not_found", { status: 404 }));
    const wrapper = await mountAt();
    expect(wrapper.find("[data-testid='team-restricted-state']").exists()).toBe(
      true
    );
  });
});
