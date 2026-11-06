// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { mountShell } from "@/lib/test-utils";
import { buildRoutes } from "@/router";

import OnboardingWelcomeView from "./OnboardingWelcomeView.vue";

/**
 * Screen 01 — Welcome 契约：双入口（建团 / 加入）、**无 Skip/Later**
 * （产品裁决 Q1）、加入动作携带 token 跳转接受页。
 */
const mountedWrappers: VueWrapper[] = [];

vi.mock("@/api/team", async importOriginal => ({
  ...(await importOriginal<Record<string, unknown>>()),
  fetchTeamContext: vi.fn().mockResolvedValue({
    principal_id: "p-1",
    display_name: "founder",
    is_active: false,
    is_admin: false,
    tenant_id: "t-1",
    tenant_role: "member",
    team: null,
    onboarding_required: true,
    memberships: []
  })
}));

async function mountAt(path = "/onboarding") {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push(path);
  await router.isReady();
  const wrapper = mountShell(OnboardingWelcomeView, {
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
});

describe("OnboardingWelcomeView", () => {
  it("renders both entry choices without any skip option", async () => {
    const { wrapper } = await mountAt();
    expect(
      wrapper.find("[data-testid='onboarding-choice-create']").exists()
    ).toBe(true);
    const text = wrapper.text().toLowerCase();
    expect(text).not.toContain("skip");
    expect(text).not.toContain("稍后");
  });

  it("navigates to the accept page carrying the pasted token", async () => {
    const { wrapper, router } = await mountAt();
    const input = wrapper.find("input#join-token");
    await input.setValue("tok-123");
    await wrapper
      .find("[data-testid='onboarding-join-submit']")
      .trigger("click");
    await flushPromises();
    expect(router.currentRoute.value.path).toBe("/invitation/accept");
    expect(router.currentRoute.value.query.token).toBe("tok-123");
  });

  it("extracts the token from a full invitation link", async () => {
    const { wrapper, router } = await mountAt();
    const input = wrapper.find("input#join-token");
    await input.setValue(
      "http://localhost/invitation/accept?token=from-link&type=invite"
    );
    await wrapper
      .find("[data-testid='onboarding-join-submit']")
      .trigger("click");
    await flushPromises();
    expect(router.currentRoute.value.query.token).toBe("from-link");
  });
});
