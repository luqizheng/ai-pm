// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { mountShell, settle, waitForRoute } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";
import { buildRoutes } from "@/router";

import ProductMemberAddView from "./ProductMemberAddView.vue";

/**
 * 添加产品成员页契约（P4-ORG UX 修订 / Figma M2）：
 *   - 候选 = 本团队成员且未加入本产品；身份固定 Product Member（disabled）；
 *   - Result preview 三列；提交后回成员页。
 */
const TEAM_ID = "00000000-0000-0000-0000-000000000010";
const PRODUCT_ID = "00000000-0000-0000-0000-000000000020";

const mountedWrappers: VueWrapper[] = [];

const fetchProduct = vi.fn();
const listProductMembers = vi.fn();
const addProductMember = vi.fn();
const listTeamMembers = vi.fn();
const fetchTeamContext = vi.fn();

vi.mock("@/api/products", () => ({
  fetchProduct: (...args: unknown[]) => fetchProduct(...args),
  listProductMembers: (...args: unknown[]) => listProductMembers(...args),
  addProductMember: (...args: unknown[]) => addProductMember(...args),
  listProducts: vi.fn(),
  listProductLinks: vi.fn(),
  removeProductMember: vi.fn(),
  appointProductOwner: vi.fn(),
  demoteProductOwner: vi.fn(),
  setProductLifecycle: vi.fn(),
  createProduct: vi.fn(),
  createProductLink: vi.fn(),
  endProductLink: vi.fn()
}));

vi.mock("@/api/team", () => ({
  listTeamMembers: (...args: unknown[]) => listTeamMembers(...args),
  fetchTeamContext: (...args: unknown[]) => fetchTeamContext(...args)
}));

vi.mock("@/components/ui/toast", () => ({
  useToast: () => ({ toast: vi.fn() })
}));

function pressKey(key: string): void {
  document.activeElement?.dispatchEvent(
    new KeyboardEvent("keydown", { key, bubbles: true })
  );
}

/** Select 键盘流：focus 触发器 → ArrowDown 开面板 → 对目标 option 派发 Enter。 */
async function chooseOption(
  wrapper: VueWrapper,
  ariaLabel: string,
  index: number
): Promise<void> {
  const trigger = wrapper.find<HTMLElement>(
    `[role="combobox"][aria-label="${ariaLabel}"]`
  );
  trigger.element.focus();
  pressKey("ArrowDown");
  await settle();
  const options = Array.from(
    document.querySelectorAll<HTMLElement>('[role="option"]')
  );
  const target = options[index];
  if (!target) throw new Error(`option ${index} not found`);
  target.dispatchEvent(
    new KeyboardEvent("keydown", {
      key: "Enter",
      bubbles: true,
      cancelable: true
    })
  );
  await settle();
}

async function mountAt() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push(`/settings/team/products/${PRODUCT_ID}/members/add`);
  await router.isReady();
  const wrapper = mountShell(ProductMemberAddView, {
    global: { plugins: [router] },
    attachTo: document.body
  });
  mountedWrappers.push(wrapper);
  await flushPromises();
  return { wrapper, router };
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
  fetchProduct.mockResolvedValue({
    id: PRODUCT_ID,
    team_id: TEAM_ID,
    key: "AUTH",
    name: "Authentication service",
    description: null,
    status: "normal",
    updated_at: "2026-11-21T00:00:00Z"
  });
  listProductMembers.mockResolvedValue([
    {
      principal_id: "p-maya",
      display_name: "Maya Chen",
      role: "owner",
      is_active: true,
      created_at: "x"
    }
  ]);
  listTeamMembers.mockResolvedValue([
    {
      principal_id: "p-maya",
      display_name: "Maya Chen",
      role: "owner",
      is_active: true,
      created_at: "x"
    },
    {
      principal_id: "p-casey",
      display_name: "Casey Wu",
      role: "member",
      is_active: true,
      created_at: "x"
    }
  ]);
});

describe("ProductMemberAddView", () => {
  it("filters already-added members, fixes the identity field and previews the result", async () => {
    const { wrapper } = await mountAt();
    // 已在产品内的成员（Maya）不进候选（首个候选=Casey）。
    await chooseOption(wrapper, "Human Principal · required", 0);
    expect(wrapper.find("[data-test='add-identity-static']").text()).toContain(
      "Product Member"
    );
    const preview = wrapper.find("[data-testid='add-result-preview']");
    expect(preview.text()).toContain("Casey Wu");
    expect(preview.text()).toContain("Product Member · AUTH");
    expect(preview.text()).toContain("Project access / Team affiliation");
  });

  it("submits the add and returns to the members page", async () => {
    addProductMember.mockResolvedValue(undefined);
    const { wrapper, router } = await mountAt();
    await chooseOption(wrapper, "Human Principal · required", 0);
    await wrapper.find("[data-test='add-submit']").trigger("click");
    await settle();
    expect(addProductMember).toHaveBeenCalledWith(PRODUCT_ID, "p-casey");
    await waitForRoute(router, `/settings/team/products/${PRODUCT_ID}/members`);
    expect(router.currentRoute.value.path).toBe(
      `/settings/team/products/${PRODUCT_ID}/members`
    );
  });
});
