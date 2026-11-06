// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { mountShell } from "@/lib/test-utils";
import { buildRoutes } from "@/router";

import SettingsTeamMembersView from "./SettingsTeamMembersView.vue";

/**
 * 团队成员页契约（UI 细则 #3 角色即权限）：
 *   - OWNER 视角：行内角色下拉 + 移除/设为 Owner 操作可见；
 *   - MEMBER 视角：整页只读说明 + 操作列无权限态，而非隐形禁用。
 */
const TENANT_ID = "00000000-0000-0000-0000-000000000010";

const mountedWrappers: VueWrapper[] = [];

const listTeamMembers = vi.fn();
const removeTeamMember = vi.fn();
const setTeamMemberRole = vi.fn();
const transferTeamOwnership = vi.fn();
const fetchTeamContext = vi.fn();

vi.mock("@/api/team", () => ({
  listTeamMembers: (...args: unknown[]) => listTeamMembers(...args),
  removeTeamMember: (...args: unknown[]) => removeTeamMember(...args),
  setTeamMemberRole: (...args: unknown[]) => setTeamMemberRole(...args),
  transferTeamOwnership: (...args: unknown[]) => transferTeamOwnership(...args),
  fetchTeamContext: (...args: unknown[]) => fetchTeamContext(...args)
}));

vi.mock("@/components/ui/toast", () => ({
  useToast: () => ({ toast: vi.fn() })
}));

function member(overrides: Record<string, unknown> = {}) {
  return {
    principal_id: "p-bob",
    display_name: "bob",
    role: "member",
    is_active: true,
    created_at: "2026-11-02T09:30:00Z",
    ...overrides
  };
}

async function mountAt(path = "/settings/team/members") {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push(path);
  await router.isReady();
  const wrapper = mountShell(SettingsTeamMembersView, {
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
  listTeamMembers.mockResolvedValue([
    member({ principal_id: "p-owner", display_name: "admin", role: "owner" }),
    member(),
    member({ principal_id: "p-carol", display_name: "carol", role: "admin" })
  ]);
});

describe("SettingsTeamMembersView", () => {
  it("as OWNER shows role controls and per-row actions", async () => {
    fetchTeamContext.mockResolvedValue({
      principal_id: "p-owner",
      display_name: "admin",
      is_active: true,
      is_admin: true,
      tenant_id: TENANT_ID,
      tenant_role: "owner",
      team: { id: TENANT_ID, name: "Acme R&D", slug: null, description: null },
      onboarding_required: false,
      memberships: [
        {
          team_id: TENANT_ID,
          team_name: "Acme R&D",
          role: "owner",
          status: "active"
        }
      ]
    });
    const wrapper = await mountAt();
    expect(listTeamMembers).toHaveBeenCalledWith(TENANT_ID);
    // 角色下拉：SelectRoot 不产 DOM（类名不透传），以触发器为准——
    // OWNER 视角 bob（member）与 carol（admin）两行各一个。
    expect(wrapper.findAll(".ui-select__trigger")).toHaveLength(2);
    expect(
      wrapper.find("[data-test='team-member-remove-p-bob']").exists()
    ).toBe(true);
  });

  it("as MEMBER renders the explicit read-only state instead of hidden actions", async () => {
    fetchTeamContext.mockResolvedValue({
      principal_id: "p-carol",
      display_name: "carol",
      is_active: true,
      is_admin: false,
      tenant_id: TENANT_ID,
      tenant_role: "member",
      team: { id: TENANT_ID, name: "Acme R&D", slug: null, description: null },
      onboarding_required: false,
      memberships: [
        {
          team_id: TENANT_ID,
          team_name: "Acme R&D",
          role: "member",
          status: "active"
        }
      ]
    });
    const wrapper = await mountAt();
    expect(
      wrapper.find("[data-testid='team-members-readonly-note']").exists()
    ).toBe(true);
    expect(wrapper.find(".tm-role-select").exists()).toBe(false);
    expect(wrapper.text()).toContain("Owner");
  });

  it("as ADMIN denies owner rows but allows member rows", async () => {
    fetchTeamContext.mockResolvedValue({
      principal_id: "p-carol",
      display_name: "carol",
      is_active: true,
      is_admin: true,
      tenant_id: TENANT_ID,
      tenant_role: "admin",
      team: { id: TENANT_ID, name: "Acme R&D", slug: null, description: null },
      onboarding_required: false,
      memberships: [
        {
          team_id: TENANT_ID,
          team_name: "Acme R&D",
          role: "admin",
          status: "active"
        }
      ]
    });
    const wrapper = await mountAt();
    // owner 行不可移除；member 行可移除。
    expect(
      wrapper.find("[data-test='team-member-remove-p-owner']").exists()
    ).toBe(false);
    expect(
      wrapper.find("[data-test='team-member-remove-p-bob']").exists()
    ).toBe(true);
    expect(
      wrapper.find("[data-test='team-member-transfer-p-bob']").exists()
    ).toBe(false);
  });
});
