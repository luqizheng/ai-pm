// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { HttpError } from "@digua/http";
import { mountShell } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";
import { buildRoutes } from "@/router";

import ProductProjectsView from "./ProductProjectsView.vue";

/**
 * 关联项目页契约（P4-ORG UX 修订 / Figma A1；A0 空态同页）：
 *   - 列 = Project · State · Product links（同项目其他产品 key）· Access/action；
 *   - 可见项目 → View association（进 A2 详情）；不可见 → Restricted 文本；
 *   - **只读**：设计稿明确省略 Link/unlink 控件（授权未显式批准）；
 *   - 非成员 404 → 受限态。
 */
const TEAM_ID = "00000000-0000-0000-0000-000000000010";
const PRODUCT_ID = "00000000-0000-0000-0000-000000000020";
const LINK_ID = "00000000-0000-0000-0000-000000000040";
const PROJECT_ID = "00000000-0000-0000-0000-000000000030";

const mountedWrappers: VueWrapper[] = [];

const fetchProduct = vi.fn();
const listProductLinks = vi.fn();
const listProducts = vi.fn();
const listProjects = vi.fn();
const fetchTeamContext = vi.fn();

vi.mock("@/api/products", () => ({
  fetchProduct: (...args: unknown[]) => fetchProduct(...args),
  listProductLinks: (...args: unknown[]) => listProductLinks(...args),
  listProducts: (...args: unknown[]) => listProducts(...args),
  listProductMembers: vi.fn(),
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
    description: null,
    status: "normal",
    updated_at: "2026-11-21T00:00:00Z"
  };
}

async function mountAt() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push(`/settings/team/products/${PRODUCT_ID}/projects`);
  await router.isReady();
  const wrapper = mountShell(ProductProjectsView, {
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
  listProductLinks.mockImplementation((productId: string) => {
    if (productId === PRODUCT_ID) {
      return Promise.resolve([
        {
          id: LINK_ID,
          product_id: PRODUCT_ID,
          project_id: PROJECT_ID,
          created_at: "x"
        }
      ]);
    }
    // PORT（同团队另一产品）也关联同一项目 → Product links 列。
    return Promise.resolve([
      {
        id: "l-9",
        product_id: "00000000-0000-0000-0000-000000000021",
        project_id: PROJECT_ID,
        created_at: "x"
      }
    ]);
  });
  listProducts.mockResolvedValue([
    profile(),
    {
      id: "00000000-0000-0000-0000-000000000021",
      team_id: TEAM_ID,
      key: "PORT",
      name: "Developer portal",
      description: null,
      status: "normal",
      updated_at: "2026-11-20T00:00:00Z"
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

describe("ProductProjectsView", () => {
  it("lists associations with project state and sibling product keys (A1)", async () => {
    const wrapper = await mountAt();
    const head = wrapper.find("[data-testid='product-projects-head']");
    expect(head.text()).toContain("AUTH · Active");
    expect(head.text()).toContain("1 associations");
    const row = wrapper.find(`[data-testid='association-row-${LINK_ID}']`);
    expect(row.text()).toContain("Atlas Platform");
    expect(row.text()).toContain("PORT");
    expect(
      wrapper.find(`[data-test='association-view-${LINK_ID}']`).exists()
    ).toBe(true);
    // 只读声明（设计稿：Link/unlink 控件省略）。
    expect(wrapper.text()).toContain(
      "Link / unlink controls are omitted because their authorization has not been explicitly approved."
    );
    expect(wrapper.find("[data-test='link-create']").exists()).toBe(false);
  });

  it("renders Restricted for associations outside the viewer's visible projects", async () => {
    listProjects.mockResolvedValue({ items: [] });
    const wrapper = await mountAt();
    const row = wrapper.find(`[data-testid='association-row-${LINK_ID}']`);
    expect(row.text()).toContain("Restricted");
    expect(
      wrapper.find(`[data-test='association-view-${LINK_ID}']`).exists()
    ).toBe(false);
  });

  it("renders the empty state without associations (A0)", async () => {
    listProductLinks.mockResolvedValue([]);
    const wrapper = await mountAt();
    expect(
      wrapper.find("[data-testid='product-projects-card']").text()
    ).toContain("No associated projects");
  });

  it("renders the restricted state for non-product-members", async () => {
    fetchProduct.mockRejectedValue(new HttpError("not_found", { status: 404 }));
    const wrapper = await mountAt();
    expect(wrapper.find("[data-testid='team-restricted-state']").exists()).toBe(
      true
    );
  });
});
