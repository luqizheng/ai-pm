// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { HttpError } from "@digua/http";
import { mountShell, settle } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";
import { buildRoutes } from "@/router";

import ProductMembersView from "./ProductMembersView.vue";

/**
 * 产品成员页契约（P4-ORG UX 修订 / Figma M1 + D-remove + M5 恢复）：
 *   - Owners 卡只读（无任命/降级入口）；零 Owner + Team Owner → 恢复任命；
 *   - Members 卡普通成员行含 Remove，确认弹窗后调 removeProductMember；
 *   - 非 Product 成员 404 → 受限态。
 */
const TEAM_ID = "00000000-0000-0000-0000-000000000010";
const PRODUCT_ID = "00000000-0000-0000-0000-000000000020";

const mountedWrappers: VueWrapper[] = [];

const fetchProduct = vi.fn();
const listProductMembers = vi.fn();
const removeProductMember = vi.fn();
const appointProductOwner = vi.fn();
const listTeamMembers = vi.fn();
const fetchTeamContext = vi.fn();

vi.mock("@/api/products", () => ({
  fetchProduct: (...args: unknown[]) => fetchProduct(...args),
  listProductMembers: (...args: unknown[]) => listProductMembers(...args),
  removeProductMember: (...args: unknown[]) => removeProductMember(...args),
  appointProductOwner: (...args: unknown[]) => appointProductOwner(...args),
  listProducts: vi.fn(),
  listProductLinks: vi.fn(),
  addProductMember: vi.fn(),
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

function pressKey(key: string): void {
  document.activeElement?.dispatchEvent(
    new KeyboardEvent("keydown", { key, bubbles: true })
  );
}

/** Select 键盘流（SelectRoot 不透传 DOM，走触发器 + 浮层 option）。 */
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
  await router.push(`/settings/team/products/${PRODUCT_ID}/members`);
  await router.isReady();
  const wrapper = mountShell(ProductMembersView, {
    global: { plugins: [router] },
    attachTo: document.body
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
  listTeamMembers.mockResolvedValue([]);
  listProductMembers.mockResolvedValue([
    member(),
    member({
      principal_id: "p-sam",
      display_name: "Sam Rivera",
      role: "member"
    })
  ]);
});

describe("ProductMembersView", () => {
  it("splits owners (read-only card) from ordinary members with Remove", async () => {
    const wrapper = await mountAt();
    expect(
      wrapper.find("[data-testid='product-owners-card']").text()
    ).toContain("Maya Chen");
    expect(
      wrapper.find("[data-testid='product-owners-card']").text()
    ).toContain("Product Owner");
    // Owners 卡无任命/降级入口（严格按图）。
    expect(
      wrapper
        .find("[data-testid='product-owners-card']")
        .find("button[data-test^='owner-appoint']")
        .exists()
    ).toBe(false);
    expect(
      wrapper.find("[data-testid='product-members-card']").text()
    ).toContain("Sam Rivera");
    expect(wrapper.find("[data-test='member-remove-p-sam']").exists()).toBe(
      true
    );
  });

  it("removes an ordinary member through the D-remove confirmation dialog", async () => {
    removeProductMember.mockResolvedValue(undefined);
    const wrapper = await mountAt();
    await wrapper.find("[data-test='member-remove-p-sam']").trigger("click");
    await flushPromises();
    // 弹层 Teleport 到 body：断言与交互都走 document。
    expect(document.body.textContent).toContain(
      'Remove Product Member "Sam Rivera"?'
    );
    const confirmButton = dialogButton("Remove");
    expect(confirmButton).toBeDefined();
    confirmButton!.click();
    await settle();
    expect(removeProductMember).toHaveBeenCalledWith(PRODUCT_ID, "p-sam");
  });

  it("offers restorative owner appointment to the Team Owner when no owner remains (M5)", async () => {
    listProductMembers.mockResolvedValue([
      member({
        principal_id: "p-sam",
        display_name: "Sam Rivera",
        role: "member"
      })
    ]);
    appointProductOwner.mockResolvedValue(undefined);
    const wrapper = await mountAt();
    expect(wrapper.text()).toContain(
      "This product currently has no Product Owner"
    );
    await chooseOption(wrapper, "Restore Product Owner", 0);
    await wrapper.find("[data-test='restore-owner-confirm']").trigger("click");
    await settle();
    expect(appointProductOwner).toHaveBeenCalledWith(PRODUCT_ID, "p-sam");
  });

  it("renders the restricted state for non-product-members", async () => {
    fetchProduct.mockRejectedValue(new HttpError("not_found", { status: 404 }));
    const wrapper = await mountAt();
    expect(wrapper.find("[data-testid='team-restricted-state']").exists()).toBe(
      true
    );
  });
});
