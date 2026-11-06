import {
  expect,
  test,
  type BrowserContext,
  type Locator,
  type Page
} from "@playwright/test";

import {
  api,
  browserSession,
  claimTeam,
  joinAs,
  signToken
} from "./team/stack";

/**
 * Team QA · UI E2E（TEAM-QA-B09-*，真实后端）
 *
 * 后端 = global-setup 起的独立 pm-api（jwks 模式 + 自签令牌 + 一次性空库），
 * 不使用 mock fixtures。每个用例占用独立 tid（= 独立租户），互不污染。
 * 判定口径：UI 状态 + 直连 pm-api 的终态双证据（HTTP 200 不作唯一证据）。
 *
 * 选择器：团队域组件自带 data-testid / data-test，优先使用；其次 role/label。
 */

async function signIn(
  context: BrowserContext,
  sub: string,
  tid: number,
  email: string | null = null
) {
  await context.addInitScript(
    tokens => {
      for (const [key, value] of Object.entries(tokens)) {
        window.localStorage.setItem(key, value as string);
      }
    },
    browserSession(sub, tid, email)
  );
}

const dataTest = (page: Page, id: string) =>
  page.locator(`[data-test="${id}"]`);

/** ui/Input、ui/Textarea 的 testid 落在包裹 div 上：取其内（或自身）的原生控件。 */
const field = (loc: Locator) =>
  loc
    .locator("xpath=descendant-or-self::*[self::input or self::textarea]")
    .first();

async function me(sub: string, tid: number) {
  const res = await api("GET", "/v1/pm/me", signToken(sub, tid));
  expect(res.status).toBe(200);
  return res.body;
}

// ─────────────────────────────────────────────── 首登与建团（Screen 01-05）

test.describe("Team QA · Onboarding 流程", () => {
  test("B09-01 首登无团队 → 登录中转页分流到 /onboarding", async ({
    context,
    page
  }) => {
    await signIn(context, "ui-first", 101);
    await page.goto("/auth/signing-in");
    await expect(page).toHaveURL(/\/onboarding$/, { timeout: 20_000 });
    await expect(page.getByTestId("onboarding-choice-create")).toBeVisible();
    expect((await me("ui-first", 101)).onboarding_required).toBe(true);
  });

  test("B09-02 建团 → 邀请页横幅；后端创建者=owner、团队档案落库", async ({
    context,
    page
  }) => {
    await signIn(context, "ui-founder", 102);
    await page.goto("/onboarding");
    await page.getByTestId("onboarding-choice-create").click();
    await expect(page).toHaveURL(/\/onboarding\/create$/);

    await field(page.getByTestId("onboarding-team-name")).fill("QA 团队 102");
    await field(page.getByTestId("onboarding-team-slug")).fill("qa-team-102");
    await field(page.getByTestId("onboarding-team-description")).fill(
      "created by e2e"
    );
    await page.getByTestId("onboarding-create-submit").click();

    await expect(page).toHaveURL(/\/onboarding\/invite/);
    await expect(page.getByTestId("onboarding-created-banner")).toContainText(
      "QA 团队 102"
    );

    const body = await me("ui-founder", 102);
    expect(body.onboarding_required).toBe(false);
    expect(body.tenant_role).toBe("owner");
    expect(body.team).toMatchObject({
      name: "QA 团队 102",
      slug: "qa-team-102"
    });
  });

  test("B09-03 slug 被占用 → 页面显示错误且不跳转", async ({
    context,
    page
  }) => {
    await claimTeam("ui-squatter", 103, "Squatter", "taken-slug");
    await signIn(context, "ui-late", 104);
    await page.goto("/onboarding/create");
    await field(page.getByTestId("onboarding-team-name")).fill("Late Team");
    await field(page.getByTestId("onboarding-team-slug")).fill("taken-slug");
    await page.getByTestId("onboarding-create-submit").click();

    await expect(page.getByTestId("onboarding-create-error")).toBeVisible();
    await expect(page).toHaveURL(/\/onboarding\/create$/);
    expect((await me("ui-late", 104)).onboarding_required).toBe(true);
  });

  test("B09-04 建团后连续流程：邀请成员 → 生成一次性链接；后端 PENDING 邀请落库", async ({
    context,
    page
  }) => {
    await signIn(context, "ui-inviter", 105);
    await page.goto("/onboarding/create");
    await field(page.getByTestId("onboarding-team-name")).fill("Inviter Team");
    await page.getByTestId("onboarding-create-submit").click();
    await expect(page).toHaveURL(/\/onboarding\/invite/);

    await page
      .getByTestId("onboarding-invite-row-0")
      .locator("input")
      .first()
      .fill("newbie@team-qa.test");
    await page.getByTestId("onboarding-invite-submit").click();
    const issued = page.getByTestId("onboarding-issued");
    await expect(issued).toContainText("newbie@team-qa.test");
    await expect(issued).toContainText("/invitation/accept?token=");

    const token = signToken("ui-inviter", 105);
    const tenantId = (await me("ui-inviter", 105)).tenant_id;
    const list = await api(
      "GET",
      `/v1/pm/tenants/${tenantId}/invitations`,
      token
    );
    expect(list.body).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          email: "newbie@team-qa.test",
          role: "member",
          status: "PENDING"
        })
      ])
    );
  });

  test("B09-05 建团后连续流程：跳过邀请 → 链接页生成链接 → 完成进入团队首页", async ({
    context,
    page
  }) => {
    await signIn(context, "ui-linker", 106);
    await page.goto("/onboarding/create");
    await field(page.getByTestId("onboarding-team-name")).fill("Linker Team");
    await page.getByTestId("onboarding-create-submit").click();
    await page.getByTestId("onboarding-invite-skip").click();
    await expect(page).toHaveURL(/\/onboarding\/link$/);

    await page.getByTestId("onboarding-link-create").click();
    await expect(page.getByTestId("onboarding-link-result")).toContainText(
      "type=link"
    );

    const token = signToken("ui-linker", 106);
    const tenantId = (await me("ui-linker", 106)).tenant_id;
    const links = await api(
      "GET",
      `/v1/pm/tenants/${tenantId}/invitation-links`,
      token
    );
    expect(links.body).toHaveLength(1);
    expect(links.body[0]).toMatchObject({
      status: "ACTIVE",
      default_role: "member"
    });

    await page.getByTestId("onboarding-link-finish").click();
    await expect(page).toHaveURL(/\/dashboard$/);
  });

  test("B09-19 链接页「默认角色」字段显示 MEMBER", async ({
    context,
    page
  }) => {
    await claimTeam("ui-roleview", 107, "Role View Team");
    await signIn(context, "ui-roleview", 107);
    await page.goto("/onboarding/link");
    await expect(field(page.getByTestId("onboarding-link-role"))).toHaveValue(
      "MEMBER"
    );
  });

  test("B09-20 刷新/深链进入邀请页 → 仍能发出邀请（团队上下文须自行加载）", async ({
    context,
    page
  }) => {
    const { tenantId, token } = await claimTeam(
      "ui-reload",
      108,
      "Reload Team"
    );
    await signIn(context, "ui-reload", 108);
    await page.goto("/onboarding/invite");
    await expect(page.getByTestId("onboarding-created-banner")).toContainText(
      "Reload Team"
    );
    await page
      .getByTestId("onboarding-invite-row-0")
      .locator("input")
      .first()
      .fill("deep@team-qa.test");
    await page.getByTestId("onboarding-invite-submit").click();
    await expect(page.getByTestId("onboarding-issued")).toContainText(
      "deep@team-qa.test"
    );
    const list = await api(
      "GET",
      `/v1/pm/tenants/${tenantId}/invitations`,
      token
    );
    expect(list.body).toHaveLength(1);
  });
});

// ─────────────────────────────────────────────── 接受邀请（Screen 06/07）

test.describe("Team QA · 接受邀请", () => {
  test("B09-06 被邀者预览并接受 → success；后端激活为 member", async ({
    context,
    page
  }) => {
    const { tenantId, token } = await claimTeam("ui-host", 110, "Host Team");
    const inv = await api(
      "POST",
      `/v1/pm/tenants/${tenantId}/invitations`,
      token,
      {
        email: "guest@team-qa.test",
        role: "member"
      }
    );
    await signIn(context, "ui-guest", 110, "guest@team-qa.test");
    await page.goto(`/invitation/accept?token=${inv.body.token}&type=invite`);
    await expect(page.getByText("Host Team").first()).toBeVisible();
    await page.getByTestId("accept-confirm").click();
    await expect(page.getByTestId("accept-result")).toHaveAttribute(
      "data-state",
      "success"
    );

    const body = await me("ui-guest", 110);
    expect(body.is_active).toBe(true);
    expect(body.memberships[0]).toMatchObject({
      role: "member",
      team_name: "Host Team"
    });

    await page.getByTestId("accept-continue").click();
    await expect(page).toHaveURL(/\/dashboard$/);
  });

  test("B09-07 已撤销邀请 → 预览即阻断（revoked）", async ({
    context,
    page
  }) => {
    const { tenantId, token } = await claimTeam(
      "ui-host2",
      111,
      "Revoker Team"
    );
    const inv = await api(
      "POST",
      `/v1/pm/tenants/${tenantId}/invitations`,
      token,
      {
        email: "late@team-qa.test",
        role: "member"
      }
    );
    await api(
      "POST",
      `/v1/pm/tenants/${tenantId}/invitations/${inv.body.id}/revoke`,
      token
    );
    await signIn(context, "ui-late2", 111);
    await page.goto(`/invitation/accept?token=${inv.body.token}&type=invite`);
    await expect(page.getByTestId("accept-preview-blocked")).toBeVisible();
    await expect(page.getByTestId("accept-confirm")).toHaveCount(0);
  });

  test("B09-08 登录邮箱与邀请不符 → emailMismatch，后端未激活", async ({
    context,
    page
  }) => {
    const { tenantId, token } = await claimTeam("ui-host3", 112, "Mail Team");
    const inv = await api(
      "POST",
      `/v1/pm/tenants/${tenantId}/invitations`,
      token,
      {
        email: "right@team-qa.test",
        role: "member"
      }
    );
    await signIn(context, "ui-wrong", 112, "wrong@team-qa.test");
    await page.goto(`/invitation/accept?token=${inv.body.token}&type=invite`);
    await page.getByTestId("accept-confirm").click();
    await expect(page.getByTestId("accept-result")).toHaveAttribute(
      "data-state",
      "emailMismatch"
    );
    expect((await me("ui-wrong", 112)).is_active).toBe(false);
  });

  test("B09-09 名额耗尽的链接 → 预览即阻断（exhausted）", async ({
    context,
    page
  }) => {
    const { tenantId, token } = await claimTeam("ui-host4", 113, "Quota Team");
    const link = await api(
      "POST",
      `/v1/pm/tenants/${tenantId}/invitation-links`,
      token,
      { max_uses: 1 }
    );
    await api(
      "POST",
      "/v1/pm/invitation-links/accept",
      signToken("ui-first-taker", 113),
      {
        token: link.body.token
      }
    );
    await signIn(context, "ui-second", 113);
    await page.goto(`/invitation/accept?token=${link.body.token}&type=link`);
    await expect(page.getByTestId("accept-preview-blocked")).toBeVisible();
  });

  test("B09-10 跨租户接受 → tenantMismatch", async ({ context, page }) => {
    const { tenantId, token } = await claimTeam(
      "ui-host5",
      114,
      "Foreign Team"
    );
    const link = await api(
      "POST",
      `/v1/pm/tenants/${tenantId}/invitation-links`,
      token,
      {}
    );
    await signIn(context, "ui-outsider", 115);
    await page.goto(`/invitation/accept?token=${link.body.token}&type=link`);
    await page.getByTestId("accept-confirm").click();
    await expect(page.getByTestId("accept-result")).toHaveAttribute(
      "data-state",
      "tenantMismatch"
    );
  });

  test("B09-11 无效 token → invalid", async ({ context, page }) => {
    await signIn(context, "ui-random", 116);
    await page.goto("/invitation/accept?token=not-a-real-token&type=invite");
    await expect(page.getByTestId("accept-result")).toHaveAttribute(
      "data-state",
      "invalid"
    );
  });
});

// ─────────────────────────────────────────────── 团队设置与权限（细则 #1/#2）

test.describe("Team QA · 团队设置与角色", () => {
  test("B09-12 TeamSwitcher 显示当前团队名", async ({ context, page }) => {
    await claimTeam("ui-switch", 120, "Switcher Team");
    await signIn(context, "ui-switch", 120);
    await page.goto("/dashboard");
    await expect(page.getByTestId("team-switcher")).toContainText(
      "Switcher Team"
    );
  });

  test("B09-13 admin 改团队资料成功；看不到 Danger Zone", async ({
    context,
    page
  }) => {
    const { tenantId, token } = await claimTeam(
      "ui-own",
      121,
      "Admin Edit Team"
    );
    await joinAs(token, tenantId, "ui-admin", 121, "admin");
    await signIn(context, "ui-admin", 121);
    await page.goto("/settings/team/general");
    await field(dataTest(page, "team-general-description")).fill(
      "edited by admin"
    );
    await dataTest(page, "team-general-save").click();
    await expect
      .poll(
        async () =>
          (await api("GET", `/v1/pm/tenants/${tenantId}`, token)).body
            .description
      )
      .toBe("edited by admin");
    await expect(dataTest(page, "team-dissolve-open")).toHaveCount(0);
  });

  test("B09-14 member 只读：通用设置无保存、名册只读、无管理按钮", async ({
    context,
    page
  }) => {
    const { tenantId, token } = await claimTeam(
      "ui-own2",
      122,
      "Readonly Team"
    );
    await joinAs(token, tenantId, "ui-member", 122, "member");
    await signIn(context, "ui-member", 122);

    await page.goto("/settings/team/general");
    await expect(page.locator("#team-name")).toBeVisible();
    await expect(dataTest(page, "team-general-save")).toHaveCount(0);

    await page.goto("/settings/team/members");
    await expect(page.getByTestId("team-members-readonly-note")).toBeVisible();
    await expect(page.getByText("ui-own2").first()).toBeVisible();
    await expect(
      page.locator('[data-test^="team-member-remove-"]')
    ).toHaveCount(0);
  });

  test("B09-15 owner 在邀请管理页创建并撤销邀请", async ({ context, page }) => {
    const { tenantId, token } = await claimTeam("ui-own3", 123, "Invites Team");
    await signIn(context, "ui-own3", 123);
    await page.goto("/settings/team/invitations");
    await field(dataTest(page, "invitations-email")).fill(
      "ui-invited@team-qa.test"
    );
    await dataTest(page, "invitations-submit").click();

    const row = page.locator('[data-test^="invitation-"]', {
      hasText: "ui-invited@team-qa.test"
    });
    await expect(row).toBeVisible();
    await row.getByRole("button").click();
    await expect(row).toHaveCount(0);

    const list = await api(
      "GET",
      `/v1/pm/tenants/${tenantId}/invitations`,
      token
    );
    expect(list.body).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          email: "ui-invited@team-qa.test",
          status: "REVOKED"
        })
      ])
    );
  });

  test("B09-16 owner 在名册中移除成员 → 后端成员行消失", async ({
    context,
    page
  }) => {
    const { tenantId, token } = await claimTeam("ui-own4", 124, "Remove Team");
    await joinAs(token, tenantId, "ui-leaver", 124, "member");
    const leaverId = (await me("ui-leaver", 124)).principal_id;
    await signIn(context, "ui-own4", 124);
    await page.goto("/settings/team/members");
    await dataTest(page, `team-member-remove-${leaverId}`).click();
    await dataTest(page, "team-member-remove-confirm").click();
    await expect(dataTest(page, `team-member-remove-${leaverId}`)).toHaveCount(
      0
    );

    const roster = await api(
      "GET",
      `/v1/pm/tenants/${tenantId}/members`,
      token
    );
    const ids = (roster.body.items ?? roster.body).map(
      (m: any) => m.principal_id
    );
    expect(ids).not.toContain(leaverId);
  });

  test("B09-17 owner 解散：名称不符不解锁；确认后登出并回到 onboarding 门", async ({
    context,
    page
  }) => {
    const { tenantId } = await claimTeam("ui-own5", 125, "Doomed Team");
    await signIn(context, "ui-own5", 125);
    await page.goto("/settings/team/general");

    const open = dataTest(page, "team-dissolve-open");
    await field(dataTest(page, "team-dissolve-confirm")).fill("Wrong Name");
    await expect(open).toBeDisabled();
    await field(dataTest(page, "team-dissolve-confirm")).fill("Doomed Team");
    await expect(open).toBeEnabled();
    await open.click();
    await dataTest(page, "team-dissolve-confirm-button").click();

    await expect(page).toHaveURL(/\/auth\/login/, { timeout: 20_000 });
    const body = await me("ui-own5", 125);
    expect(body.team).toBeNull();
    expect(body.onboarding_required).toBe(true);
    const profile = await api(
      "GET",
      `/v1/pm/tenants/${tenantId}`,
      signToken("ui-own5", 125)
    );
    expect(profile.status).not.toBe(500);
  });
});

// ─────────────────────────────────────────────── 待裁决项（设计口径断言，现状不符）

test.describe("Team QA · 待裁决", () => {
  test.fixme(
    true,
    "待裁决：onboarding 门只在登录中转页，无路由级守卫——未入团用户深链 /dashboard 不会被拦回 /onboarding"
  );
  test("B09-18 未入团用户深链 /dashboard → 应被引导到 /onboarding", async ({
    context,
    page
  }) => {
    await signIn(context, "ui-sneaky", 130);
    await page.goto("/dashboard");
    await expect(page).toHaveURL(/\/onboarding/);
  });
});
