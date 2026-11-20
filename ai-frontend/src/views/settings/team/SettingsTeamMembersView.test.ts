// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { mountShell } from "@/lib/test-utils";
import { buildRoutes } from "@/router";

import SettingsTeamMembersView from "./SettingsTeamMembersView.vue";

/**
 * 团队成员页契约（UI 细则 #3 角色即权限；P4-ORG DR-ORG-02）：
 *   - Team 角色仅 owner|member（admin 已取消）；管理操作仅 Team Owner；
 *   - OWNER 视角：行内角色下拉（任命 owner / 降级 member）+ 移除 + Make Owner；
 *   - MEMBER 视角：整页只读说明 + 操作列无权限态，而非隐形禁用；
 *   - owner-transfer 已取消（多 Owner 任命制）。
 */
const TEAM_ID = "00000000-0000-0000-0000-000000000010";

const mountedWrappers: VueWrapper[] = [];

const listTeamMembers = vi.fn();
const removeTeamMember = vi.fn();
const setTeamMemberRole = vi.fn();
const fetchTeamContext = vi.fn();

vi.mock("@/api/team", () => ({
  listTeamMembers: (...args: unknown[]) => listTeamMembers(...args),
  removeTeamMember: (...args: unknown[]) => removeTeamMember(...args),
  setTeamMemberRole: (...args: unknown[]) => setTeamMemberRole(...args),
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

async function mountAt(path = "/admin/members") {
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

function contextFor(role: "owner" | "member", principalId: string) {
  return {
    principal_id: principalId,
    display_name: role,
    is_active: true,
    is_admin: role === "owner",
    tenant_id: "00000000-0000-0000-0000-000000000099",
    team_role: role,
    team: { id: TEAM_ID, name: "Acme R&D", slug: null, description: null },
    onboarding_required: false,
    memberships: [
      { team_id: TEAM_ID, team_name: "Acme R&D", role, status: "active" }
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
  listTeamMembers.mockResolvedValue([
    member({ principal_id: "p-owner", display_name: "owner", role: "owner" }),
    member(),
    member({ principal_id: "p-carol", display_name: "carol", role: "member" })
  ]);
});

describe("SettingsTeamMembersView", () => {
  it("as OWNER shows role controls and per-row actions for others", async () => {
    fetchTeamContext.mockResolvedValue(contextFor("owner", "p-owner"));
    const wrapper = await mountAt();
    expect(listTeamMembers).toHaveBeenCalledWith(TEAM_ID);
    // 角色下拉：SelectRoot 不产 DOM（类名不透传），以触发器为准——
    // OWNER 视角可编辑除自己外的所有行（任命制：owner 行也可降级）。
    expect(wrapper.findAll(".ui-select__trigger")).toHaveLength(2);
    expect(
      wrapper.find("[data-test='team-member-remove-p-bob']").exists()
    ).toBe(true);
    expect(
      wrapper.find("[data-test='team-member-make-owner-p-bob']").exists()
    ).toBe(true);
    // 自己（owner）行：无下拉、无移除。
    expect(
      wrapper.find("[data-test='team-member-remove-p-owner']").exists()
    ).toBe(false);
  });

  it("as MEMBER renders the explicit read-only state instead of hidden actions", async () => {
    fetchTeamContext.mockResolvedValue(contextFor("member", "p-carol"));
    const wrapper = await mountAt();
    expect(
      wrapper.find("[data-testid='team-members-readonly-note']").exists()
    ).toBe(true);
    expect(wrapper.find(".tm-role-select").exists()).toBe(false);
    expect(wrapper.text()).toContain("Owner");
  });

  it("Make Owner appoints via PATCH role=owner (multi-owner system)", async () => {
    fetchTeamContext.mockResolvedValue(contextFor("owner", "p-owner"));
    setTeamMemberRole.mockResolvedValue(undefined);
    const wrapper = await mountAt();
    const button = wrapper.find("[data-test='team-member-make-owner-p-bob']");
    expect(button.exists()).toBe(true);
    await button.trigger("click");
    await flushPromises();
    expect(setTeamMemberRole).toHaveBeenCalledWith(
      TEAM_ID,
      "p-bob",
      "owner"
    );
  });

});
