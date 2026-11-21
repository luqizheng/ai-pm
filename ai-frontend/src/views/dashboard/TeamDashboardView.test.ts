// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { HttpError } from "@digua/http";
import { mountShell } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";
import { buildRoutes } from "@/router";

import TeamDashboardView from "./TeamDashboardView.vue";

/**
 * Team 概览契约（P4-ORG UX 修订 V1.0 / Figma T1）：
 *   - 团队档案卡：slug · 名称 + TEAM 徽标 + Owner 名单 + 人数/身份/产品计数；
 *   - Products in this Team：Lifecycle 徽标 / Product Owner（可读成员表时）/
 *     Access（读成员表资格判定 = Open / Restricted，abandoned 为受限文本）；
 *   - 组织隶属边界横幅常驻；资格不足（403）整页呈现 T4 受限态；
 *   - 不再承载成员操作 / 项目入口 / 邀请摘要（成员走 Members 页签）。
 */
const TEAM_ID = "00000000-0000-0000-0000-000000000010";
const PRODUCT_AUTH = "00000000-0000-0000-0000-000000000020";
const PRODUCT_PORT = "00000000-0000-0000-0000-000000000021";
const PRODUCT_BILL = "00000000-0000-0000-0000-000000000022";

const mountedWrappers: VueWrapper[] = [];

const fetchTeamContext = vi.fn();
const listTeamMembers = vi.fn();
const listProducts = vi.fn();
const listProductMembers = vi.fn();

vi.mock("@/api/team", () => ({
  listTeamMembers: (...args: unknown[]) => listTeamMembers(...args),
  fetchTeamContext: (...args: unknown[]) => fetchTeamContext(...args)
}));

vi.mock("@/api/products", () => ({
  listProducts: (...args: unknown[]) => listProducts(...args),
  listProductMembers: (...args: unknown[]) => listProductMembers(...args)
}));

function product(overrides: Record<string, unknown> = {}) {
  return {
    id: PRODUCT_AUTH,
    team_id: TEAM_ID,
    name: "Authentication service",
    description: null,
    status: "active",
    ...overrides
  };
}

function productMember(overrides: Record<string, unknown> = {}) {
  return {
    principal_id: "p-maya",
    display_name: "Maya Chen",
    role: "owner",
    is_active: true,
    created_at: "2026-06-08T00:00:00Z",
    ...overrides
  };
}

async function mountAt(path = "/dashboard") {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push(path);
  await router.isReady();
  const wrapper = mountShell(TeamDashboardView, {
    global: { plugins: [router] }
  });
  mountedWrappers.push(wrapper);
  await flushPromises();
  return wrapper;
}

function contextFor(role: "owner" | "member") {
  return {
    principal_id: "p-maya",
    display_name: "Maya Chen",
    is_active: true,
    is_admin: false,
    tenant_id: "00000000-0000-0000-0000-000000000099",
    team_role: role,
    team: {
      id: TEAM_ID,
      name: "Platform Engineering",
      slug: "PE",
      description: "Build and maintain the shared engineering platform."
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
  // 断言按设计稿英文文案（i18n 默认随 localStorage 落 zh-CN）。
  i18n.global.locale.value = "en";
  fetchTeamContext.mockResolvedValue(contextFor("owner"));
  listTeamMembers.mockResolvedValue([
    {
      principal_id: "p-maya",
      display_name: "Maya Chen",
      role: "owner",
      is_active: true,
      created_at: "2026-06-08T00:00:00Z"
    },
    {
      principal_id: "p-jordan",
      display_name: "Jordan Lee",
      role: "member",
      is_active: true,
      created_at: "2026-07-19T00:00:00Z"
    },
    {
      principal_id: "p-casey",
      display_name: "Casey Wu",
      role: "member",
      is_active: false,
      created_at: "2026-10-14T00:00:00Z"
    }
  ]);
  listProducts.mockResolvedValue([
    product(),
    product({ id: PRODUCT_PORT, name: "Developer portal" }),
    product({ id: PRODUCT_BILL, name: "Billing API", status: "abandoned" })
  ]);
});

describe("TeamDashboardView", () => {
  it("renders the team summary card with owners and counts", async () => {
    const wrapper = await mountAt();
    const summary = wrapper.find("[data-testid='team-overview-summary']");
    expect(summary.text()).toContain("PE · Platform Engineering");
    expect(summary.text()).toContain("TEAM");
    expect(wrapper.find("[data-testid='team-overview-owners']").text()).toBe(
      "Team Owners Maya Chen"
    );
    // 3 人（含 1 名未激活）· 1 Owner · 2 Member · 3 Products。
    expect(wrapper.find("[data-testid='team-overview-stats']").text()).toBe(
      "3 people · 1 Team Owners · 2 Team Members · 3 Products"
    );
    // 概览不承载成员操作 / 项目入口 / 邀请摘要。
    expect(
      wrapper.find("[data-testid='team-dashboard-projects']").exists()
    ).toBe(false);
    expect(wrapper.text()).toContain(
      "Organization affiliation does not grant Product Membership"
    );
  });

  it("marks products eligible when the caller can read the product member list", async () => {
    listProductMembers.mockImplementation((productId: string) => {
      if (productId === PRODUCT_AUTH) {
        return Promise.resolve([productMember()]);
      }
      return Promise.reject(new HttpError("not_found", { status: 404 }));
    });
    const wrapper = await mountAt();
    const rows = wrapper.findAll("[data-testid^='team-product-row-']");
    expect(rows).toHaveLength(3);
    // AUTH：可读成员表 → Owner 名 + Open Product 按钮。
    expect(rows[0].text()).toContain("Maya Chen");
    expect(
      wrapper.find(`[data-test='team-product-open-${PRODUCT_AUTH}']`).exists()
    ).toBe(true);
    // PORT：非成员 → Restricted（禁用按钮 + 提示），Owner 名如实为 —。
    expect(rows[1].text()).toContain("Restricted");
    expect(rows[1].find("button[disabled]").attributes("title")).toBe(
      "Requires Product membership"
    );
    // BILL：abandoned → 受限文本（非按钮）。
    expect(rows[2].text()).toContain("Abandoned");
    expect(rows[2].find("button").exists()).toBe(false);
  });

  it("renders the restricted state when team access is forbidden (T4)", async () => {
    listTeamMembers.mockRejectedValue(
      new HttpError("forbidden", { status: 403 })
    );
    const wrapper = await mountAt();
    expect(wrapper.find("[data-testid='team-restricted-state']").exists()).toBe(
      true
    );
    expect(wrapper.text()).toContain("Team details are restricted");
    expect(wrapper.text()).toContain("No implicit access");
  });
});
