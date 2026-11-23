// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { mountShell } from "@/lib/test-utils";
import { buildRoutes } from "@/router";

import SettingsTeamGeneralView from "./SettingsTeamGeneralView.vue";

/**
 * 团队设置 — 基本页契约（P4-ORG Phase 2）：
 *   - Team Owner 可编辑档案并保存（调 PATCH /v1/pm/teams/{team_id}）；
 *   - Member 视角输入禁用、无保存按钮（细则 #3 角色即入口）；
 *   - owner-transfer 已取消（多 Owner 任命制，任命/降级在成员管理页）；
 *   - 解散团队已随 P4-ORG DR-ORG-01 移除（Team 不可解散），页面无危险区。
 */
const TEAM_ID = "00000000-0000-0000-0000-000000000010";

const mountedWrappers: VueWrapper[] = [];

const fetchTeamProfile = vi.fn();
const listTeamMembers = vi.fn();
const updateTeamProfile = vi.fn();
const fetchTeamContext = vi.fn();

vi.mock("@/api/team", () => ({
  fetchTeamProfile: (...args: unknown[]) => fetchTeamProfile(...args),
  listTeamMembers: (...args: unknown[]) => listTeamMembers(...args),
  updateTeamProfile: (...args: unknown[]) => updateTeamProfile(...args),
  fetchTeamContext: (...args: unknown[]) => fetchTeamContext(...args)
}));

vi.mock("@/components/ui/toast", () => ({
  useToast: () => ({ toast: () => {} })
}));

function context(role: "owner" | "member") {
  return {
    principal_id: role === "owner" ? "p-owner" : "p-carol",
    display_name: role,
    is_active: true,
    is_admin: role === "owner",
    tenant_id: "00000000-0000-0000-0000-000000000099",
    team_role: role,
    team: {
      id: TEAM_ID,
      name: "Acme R&D",
      slug: "acme-rd",
      description: null
    },
    onboarding_required: false,
    memberships: [
      { team_id: TEAM_ID, team_name: "Acme R&D", role, status: "active" }
    ]
  };
}

async function mountAt(path = "/settings/team/general") {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push(path);
  await router.isReady();
  const wrapper = mountShell(SettingsTeamGeneralView, {
    global: { plugins: [router] }
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
  fetchTeamContext.mockResolvedValue(context("owner"));
  fetchTeamProfile.mockResolvedValue({
    id: TEAM_ID,
    name: "Acme R&D",
    slug: "acme-rd",
    description: null
  });
  listTeamMembers.mockResolvedValue([
    {
      principal_id: "p-owner",
      display_name: "owner",
      role: "owner",
      is_active: true,
      created_at: "x"
    }
  ]);
});

describe("SettingsTeamGeneralView", () => {
  it("as OWNER fetches the profile by team_id and renders the save action", async () => {
    const { wrapper } = await mountAt();
    expect(fetchTeamProfile).toHaveBeenCalledWith(TEAM_ID);
    expect(listTeamMembers).toHaveBeenCalledWith(TEAM_ID);
    expect(wrapper.find("[data-test='team-general-save']").exists()).toBe(true);
    const nameInput = wrapper.find("input#team-name")
      .element as HTMLInputElement;
    expect(nameInput.disabled).toBe(false);
  });

  it("as OWNER saving calls PATCH with team_id", async () => {
    const { wrapper } = await mountAt();
    updateTeamProfile.mockResolvedValue({
      id: TEAM_ID,
      name: "Acme Renamed",
      slug: "acme-rd",
      description: null
    });
    await wrapper.find("input#team-name").setValue("Acme Renamed");
    await wrapper.find("[data-test='team-general-save']").trigger("click");
    await flushPromises();
    expect(updateTeamProfile).toHaveBeenCalledWith(
      TEAM_ID,
      expect.objectContaining({ name: "Acme Renamed" })
    );
  });

  it("as MEMBER inputs are disabled and no save action", async () => {
    fetchTeamContext.mockResolvedValue(context("member"));
    const { wrapper } = await mountAt();
    expect(wrapper.find("[data-test='team-general-save']").exists()).toBe(
      false
    );
    const nameInput = wrapper.find("input#team-name")
      .element as HTMLInputElement;
    expect(nameInput.disabled).toBe(true);
    // owner-transfer 已取消；Team 不可解散（DR-ORG-01）。
    expect(wrapper.find("[data-test='team-transfer-open']").exists()).toBe(
      false
    );
    expect(wrapper.find("[data-test='team-dissolve-open']").exists()).toBe(
      false
    );
  });
});
