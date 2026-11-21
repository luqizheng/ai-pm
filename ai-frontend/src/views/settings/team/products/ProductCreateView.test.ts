// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { mountShell, settle, waitForRoute } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";
import { buildRoutes } from "@/router";

import ProductCreateView from "./ProductCreateView.vue";

/**
 * 创建流程页契约（P4-ORG UX 修订 / Figma C1→C2→C3）：
 *   - 三步向导：基本信息 → 首位 Owner（必选阻断）→ Review 提交；
 *   - Product key = 只读派生预览（PA-07 §4.8，服务端权威生成）；
 *   - 提交载荷 = { name, first_owner_principal_id }，成功后深链产品概览；
 *   - 非 Team Owner 直接回列表。
 */
const TEAM_ID = "00000000-0000-0000-0000-000000000010";
const PRODUCT_ID = "00000000-0000-0000-0000-000000000020";

const mountedWrappers: VueWrapper[] = [];

const listTeamMembers = vi.fn();
const createProduct = vi.fn();
const fetchTeamContext = vi.fn();

vi.mock("@/api/team", () => ({
  listTeamMembers: (...args: unknown[]) => listTeamMembers(...args),
  fetchTeamContext: (...args: unknown[]) => fetchTeamContext(...args)
}));

vi.mock("@/api/products", () => ({
  createProduct: (...args: unknown[]) => createProduct(...args),
  listProducts: vi.fn(),
  listProductMembers: vi.fn(),
  listProductLinks: vi.fn(),
  fetchProduct: vi.fn(),
  addProductMember: vi.fn(),
  removeProductMember: vi.fn(),
  appointProductOwner: vi.fn(),
  demoteProductOwner: vi.fn(),
  setProductLifecycle: vi.fn(),
  createProductLink: vi.fn(),
  endProductLink: vi.fn()
}));

vi.mock("@/components/ui/toast", () => ({
  useToast: () => ({ toast: vi.fn() })
}));

function member(overrides: Record<string, unknown> = {}) {
  return {
    principal_id: "p-maya",
    display_name: "Maya Chen",
    role: "owner",
    is_active: true,
    created_at: "2026-06-08T00:00:00Z",
    ...overrides
  };
}

async function mountAt() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/settings/team/products/create");
  await router.isReady();
  const wrapper = mountShell(ProductCreateView, {
    global: { plugins: [router] }
  });
  mountedWrappers.push(wrapper);
  await flushPromises();
  return { wrapper, router };
}

function contextFor(role: "owner" | "member") {
  return {
    principal_id: "p-owner",
    display_name: "Alex Morgan",
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
  listTeamMembers.mockResolvedValue([
    member(),
    member({
      principal_id: "p-jordan",
      display_name: "Jordan Lee",
      role: "member"
    })
  ]);
});

describe("ProductCreateView", () => {
  it("derives the key preview from the name and advances to owner selection", async () => {
    const { wrapper } = await mountAt();
    expect(wrapper.find("[data-testid='create-steps']").exists()).toBe(true);
    // C1：key 只读派生预览（PA-07 §4.8 架构决策覆盖设计稿输入框）。
    const nameInput = wrapper.find("[data-test='create-name'] input");
    await nameInput.setValue("World View");
    expect(
      (
        wrapper.find("[data-test='create-key-preview']")
          .element as HTMLInputElement
      ).querySelector("input")!.value
    ).toBe("WORLD-VIEW");
    expect(
      (
        wrapper.find("[data-test='create-key-preview']")
          .element as HTMLInputElement
      ).querySelector("input")!.disabled
    ).toBe(true);
    expect(wrapper.text()).toContain("Platform Engineering");
    await wrapper.find("[data-test='create-next']").trigger("click");
    await flushPromises();
    // C2：未选 Owner 时 Review 阻断（C0 校验错态语义）。
    expect(
      (wrapper.find("[data-test='create-review']").element as HTMLButtonElement)
        .disabled
    ).toBe(true);
    await wrapper
      .find("[data-test='create-candidate-p-jordan']")
      .trigger("click");
    await flushPromises();
    expect(
      (wrapper.find("[data-test='create-review']").element as HTMLButtonElement)
        .disabled
    ).toBe(false);
  });

  it("reviews the draft and submits { name, first_owner_principal_id }, then deep-links the overview", async () => {
    createProduct.mockResolvedValue({
      id: PRODUCT_ID,
      team_id: TEAM_ID,
      key: "NOTI",
      name: "Notification service",
      description: null,
      status: "normal",
      updated_at: "2026-11-21T00:00:00Z"
    });
    const { wrapper, router } = await mountAt();
    await wrapper
      .find("[data-test='create-name'] input")
      .setValue("Notification service");
    await wrapper.find("[data-test='create-next']").trigger("click");
    await flushPromises();
    await wrapper
      .find("[data-test='create-candidate-p-maya']")
      .trigger("click");
    await flushPromises();
    await wrapper.find("[data-test='create-review']").trigger("click");
    await flushPromises();
    // C3 复核明细。
    const review = wrapper.find("[data-testid='create-review-details']");
    expect(review.text()).toContain("NOTIFICATION-SERVICE");
    expect(review.text()).toContain("Maya Chen");
    expect(review.text()).toContain("Alex Morgan · Team Owner");
    await wrapper.find("[data-test='create-submit']").trigger("click");
    await settle();
    expect(createProduct).toHaveBeenCalledWith(TEAM_ID, {
      name: "Notification service",
      first_owner_principal_id: "p-maya"
    });
    await waitForRoute(router, `/settings/team/products/${PRODUCT_ID}`);
    expect(router.currentRoute.value.path).toBe(
      `/settings/team/products/${PRODUCT_ID}`
    );
  });

  it("redirects plain team members back to the list", async () => {
    fetchTeamContext.mockResolvedValue(contextFor("member"));
    const { router } = await mountAt();
    await settle();
    await waitForRoute(router, "/settings/team/products");
    expect(router.currentRoute.value.path).toBe("/settings/team/products");
  });
});
