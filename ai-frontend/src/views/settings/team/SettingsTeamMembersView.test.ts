// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { HttpError } from "@digua/http";
import { mountShell, waitForRoute } from "@/lib/test-utils";
import { i18n } from "@/lib/i18n";
import { buildRoutes } from "@/router";

import SettingsTeamMembersView from "./SettingsTeamMembersView.vue";

/**
 * 团队成员页契约（P4-ORG UX 修订 V1.0 / Figma T2 + T5）：
 *   - 成员表只读：Human Principal / Team identity / Joined / Status，
 *     无行内角色与移除操作（成员变更走邀请，Owner 调整走继任流程页）；
 *   - Team Owner 行以徽标呈现，Team Member 行为普通文本；
 *   - 「变更 Team Owner」仅 Team Owner 可见，指向 owner-succession；
 *   - T5 保存态：继任完成带 query 回跳时显示一次性保存说明；
 *   - 资格不足（403）整页呈现 T4 受限态。
 */
const TEAM_ID = "00000000-0000-0000-0000-000000000010";

const mountedWrappers: VueWrapper[] = [];

const listTeamMembers = vi.fn();
const fetchTeamContext = vi.fn();

vi.mock("@/api/team", () => ({
  listTeamMembers: (...args: unknown[]) => listTeamMembers(...args),
  fetchTeamContext: (...args: unknown[]) => fetchTeamContext(...args)
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
  return { wrapper, router };
}

function contextFor(role: "owner" | "member", principalId: string) {
  return {
    principal_id: principalId,
    display_name: role === "owner" ? "Alex Morgan" : "Jordan Lee",
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
  // 断言按设计稿英文文案（i18n 默认随 localStorage 落 zh-CN）。
  i18n.global.locale.value = "en";
  listTeamMembers.mockResolvedValue([
    member({
      principal_id: "p-alex",
      display_name: "Alex Morgan",
      role: "owner"
    }),
    member({
      principal_id: "p-maya",
      display_name: "Maya Chen",
      role: "owner"
    }),
    member(),
    member({
      principal_id: "p-casey",
      display_name: "Casey Wu",
      is_active: false
    })
  ]);
  fetchTeamContext.mockResolvedValue(contextFor("owner", "p-alex"));
});

describe("SettingsTeamMembersView", () => {
  it("renders the read-only membership table with owner badges", async () => {
    const { wrapper } = await mountAt();
    expect(listTeamMembers).toHaveBeenCalledWith(TEAM_ID);
    // 只读：无行内角色下拉、无移除/任命按钮（旧全操作页退役）。
    expect(wrapper.findAll(".ui-select__trigger")).toHaveLength(0);
    expect(
      wrapper.find("[data-test='team-member-remove-p-bob']").exists()
    ).toBe(false);
    // Team Owner 行 = 徽标；Team Member 行 = 普通文本。
    const rows = wrapper.findAll("[data-testid^='team-member-row-']");
    expect(rows).toHaveLength(4);
    expect(rows[0].find(".ui-badge").text()).toBe("Team Owner");
    expect(rows[2].text()).toContain("Team Member");
    // Owner 横幅与成员计数卡。
    expect(wrapper.text()).toContain("Multiple Team Owners are supported");
    expect(wrapper.text()).toContain("Team membership · 4 people");
  });

  it("shows Change Team Owner only for the Team Owner and navigates to succession", async () => {
    const { wrapper, router } = await mountAt();
    const button = wrapper.find("[data-test='team-members-change-owner']");
    expect(button.exists()).toBe(true);
    expect(wrapper.text()).toContain("Current identity: Team Owner");
    await button.trigger("click");
    await waitForRoute(router, "/settings/team/members/owner-succession");
    expect(router.currentRoute.value.path).toBe(
      "/settings/team/members/owner-succession"
    );
  });

  it("hides Change Team Owner for plain members but keeps the identity note", async () => {
    fetchTeamContext.mockResolvedValue(contextFor("member", "p-bob"));
    const { wrapper } = await mountAt();
    expect(
      wrapper.find("[data-test='team-members-change-owner']").exists()
    ).toBe(false);
    expect(wrapper.text()).toContain("Current identity: Team Member");
  });

  it("shows the saved note after an owner change (T5)", async () => {
    const { wrapper } = await mountAt(
      "/settings/team/members?owner_changed=1&owner_changed_from=Alex%20Morgan&owner_changed_to=Maya%20Chen"
    );
    await flushPromises();
    const note = wrapper.find("[data-testid='team-members-saved-note']");
    expect(note.exists()).toBe(true);
    expect(note.text()).toContain(
      "Saved · Alex Morgan is Team Member; Maya Chen is the remaining Team Owner."
    );
  });

  it("renders the restricted state when membership access is forbidden (T4)", async () => {
    listTeamMembers.mockRejectedValue(
      new HttpError("forbidden", { status: 403 })
    );
    const { wrapper } = await mountAt();
    expect(wrapper.find("[data-testid='team-restricted-state']").exists()).toBe(
      true
    );
    expect(wrapper.text()).toContain("Team details are restricted");
  });
});
