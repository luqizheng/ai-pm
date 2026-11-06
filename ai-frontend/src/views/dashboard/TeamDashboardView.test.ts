// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { mountShell } from "@/lib/test-utils";
import { buildRoutes } from "@/router";

import TeamDashboardView from "./TeamDashboardView.vue";

/**
 * Team Dashboard 契约（UI 细则 #1 组织入口）：团队档案 + 成员概览 +
 * 团队项目入口 + 邀请摘要（admin）；不放项目执行指标。
 */
const TENANT_ID = "00000000-0000-0000-0000-000000000010";

const mountedWrappers: VueWrapper[] = [];

const fetchTeamContext = vi.fn();
const listTeamMembers = vi.fn();
const listTeamInvitations = vi.fn();
const listProjects = vi.fn();

vi.mock("@/api/team", () => ({
  fetchTeamContext: (...args: unknown[]) => fetchTeamContext(...args),
  listTeamMembers: (...args: unknown[]) => listTeamMembers(...args),
  listTeamInvitations: (...args: unknown[]) => listTeamInvitations(...args),
  listInvitationLinks: vi.fn().mockResolvedValue([])
}));

vi.mock("@/api/pm", () => ({
  listProjects: (...args: unknown[]) => listProjects(...args)
}));

const ownerContext = {
  principal_id: "p-owner",
  display_name: "admin",
  is_active: true,
  is_admin: true,
  tenant_id: TENANT_ID,
  tenant_role: "owner",
  team: { id: TENANT_ID, name: "Acme R&D", slug: "acme-rd", description: null },
  onboarding_required: false,
  memberships: [
    {
      team_id: TENANT_ID,
      team_name: "Acme R&D",
      role: "owner",
      status: "active"
    }
  ]
};

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

afterEach(() => {
  for (const wrapper of mountedWrappers.splice(0)) {
    wrapper.unmount();
  }
  document.body.innerHTML = "";
});

beforeEach(() => {
  vi.clearAllMocks();
  fetchTeamContext.mockResolvedValue(ownerContext);
  listTeamMembers.mockResolvedValue([
    {
      principal_id: "p-owner",
      display_name: "admin",
      role: "owner",
      is_active: true,
      created_at: "2026-11-01T08:00:00Z"
    },
    {
      principal_id: "p-bob",
      display_name: "bob",
      role: "member",
      is_active: true,
      created_at: "2026-11-02T09:30:00Z"
    }
  ]);
  listTeamInvitations.mockResolvedValue([
    {
      id: "i-1",
      email: "dave@example.com",
      role: "member",
      status: "PENDING",
      invited_by: "p-owner",
      invited_by_name: "admin",
      expires_at: "2026-11-13T08:00:00Z",
      created_at: "2026-11-06T08:00:00Z",
      accepted_at: null
    }
  ]);
  listProjects.mockResolvedValue({
    items: [
      {
        id: "pj-1",
        code: "AIPM",
        name: "AI-PM Platform",
        status: "planning",
        customer_id: "c",
        customer_name: "",
        customer_status: "",
        description: null,
        revision: 1,
        created_at: "2026-11-01T00:00:00Z",
        updated_at: "2026-11-01T00:00:00Z"
      }
    ],
    total: 1
  });
});

describe("TeamDashboardView", () => {
  it("renders the team profile, member stats and project entries", async () => {
    const wrapper = await mountAt();
    expect(wrapper.text()).toContain("Acme R&D");
    expect(
      wrapper.find("[data-testid='team-dashboard-profile']").exists()
    ).toBe(true);
    expect(
      wrapper.find("[data-testid='team-dashboard-projects']").exists()
    ).toBe(true);
    expect(wrapper.text()).toContain("AI-PM Platform");
  });

  it("shows the invitation summary only for admins", async () => {
    const wrapper = await mountAt();
    expect(
      wrapper.find("[data-testid='team-dashboard-invitations']").exists()
    ).toBe(true);

    fetchTeamContext.mockResolvedValue({
      ...ownerContext,
      is_admin: false,
      tenant_role: "member",
      memberships: [
        {
          team_id: TENANT_ID,
          team_name: "Acme R&D",
          role: "member",
          status: "active"
        }
      ]
    });
    const memberWrapper = await mountAt();
    expect(
      memberWrapper.find("[data-testid='team-dashboard-invitations']").exists()
    ).toBe(false);
  });
});
