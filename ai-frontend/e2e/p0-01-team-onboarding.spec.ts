import { expect, test, type BrowserContext } from "@playwright/test";

import { api, signToken } from "./team/stack";
import { psql, signInVia } from "./p0/fixtures";

/**
 * P0-01 · 用户登录与 Team Onboarding（docs/e2e-spec/11-p0-01 规格，3 案例）
 *
 * 评审修正已内化：
 *  - R1: 真实表单登录不可执行（无 iam-service）→ 本套件只做 P0-001a 会话级；
 *        P0-001b（表单登录）不在本文件，按规格标 BLOCKED。
 *  - R4/R5: 深链不被拦（现状记录）；DB 断言用 ACTIVE 口径 + slug IS NULL。
 *  - R57（本轮实测新增，回写规格 11 §0）: `accept_invitation` 在
 *    `team_onboarding.rs:759-760` 硬性绑定 tid（「接受只可能发生在 token
 *    所在租户」）——规格原文「User B 独立 tid」不成立，改为同 tid 不同 sub
 *    （与 B09-06 的 UI 接受路径同一被证绿路径）。
 *
 * 判定纪律：UI 状态 + API/DB 终态双证据；HTTP 200 不作唯一证据。
 */

test.describe.configure({ mode: "serial" });

/** P0-001a · 会话建立与身份正确（会话级登录，R1 拆分后的可自动化主体） */
test("P0-001a 会话注入 → 中转页分流；身份与主体行落库", async ({
  context,
  page
}) => {
  const sub = "p0-001a-user";
  const tid = 9101;
  const token = signToken(sub, tid);
  await context.addInitScript(
    tokens => {
      for (const [key, value] of Object.entries(tokens)) {
        window.localStorage.setItem(key, value as string);
      }
    },
    {
      prism_access_token: token,
      prism_refresh_token: `p0-refresh-${sub}`,
      prism_expires_at: String(Date.now() + 3_600_000),
      prism_user: JSON.stringify({ id: sub, username: sub, nickname: sub })
    }
  );

  await page.goto("/auth/signing-in");
  // 未建团 → 分流到 /onboarding（AuthLoadingView.vue:51）
  await expect(page).toHaveURL(/\/onboarding$/, { timeout: 20_000 });

  // 会话四件套确实写入 localStorage
  const keys = await page.evaluate(() => Object.keys(window.localStorage));
  for (const key of [
    "prism_access_token",
    "prism_refresh_token",
    "prism_expires_at",
    "prism_user"
  ]) {
    expect(keys, `localStorage must contain ${key}`).toContain(key);
  }

  // API 双证据：/me 身份与 onboarding 状态一致
  const me = await api("GET", "/v1/pm/me", token);
  expect(me.status).toBe(200);
  expect(me.body.onboarding_required).toBe(true);
  expect(me.body.auth_mode).toBe("interactive");

  // DB 双证据：首次 API 调用已按需 provision 主体行（inactive，is_first_admin 未命中）
  const row = psql(
    `SELECT p.is_active FROM core.principal_identity pi
       JOIN core.principal p ON p.id = pi.principal_id
      WHERE pi.subject = '${sub}' LIMIT 1`
  );
  expect(row).toBe("f");
});

/** P0-002 · 首次登录无 Team（R4 深链现状 + R5 DB 口径） */
test("P0-002 首登无 Team → 分流 /onboarding 双入口；深链现状=不被拦；无虚假默认团队", async ({
  context,
  page
}) => {
  const sub = "p0-002-user";
  const tid = 9102;
  const token = signToken(sub, tid);
  await context.addInitScript(
    tokens => {
      for (const [key, value] of Object.entries(tokens)) {
        window.localStorage.setItem(key, value as string);
      }
    },
    {
      prism_access_token: token,
      prism_refresh_token: `p0-refresh-${sub}`,
      prism_expires_at: String(Date.now() + 3_600_000),
      prism_user: JSON.stringify({ id: sub, username: sub, nickname: sub })
    }
  );

  // ① 中转页分流
  await page.goto("/auth/signing-in");
  await expect(page).toHaveURL(/\/onboarding$/, { timeout: 20_000 });

  // ②③ 创建 / 加入双入口（data-testid，禁 i18n 文案）
  await expect(page.getByTestId("onboarding-choice-create")).toBeVisible();
  await expect(page.getByTestId("onboarding-join-token")).toBeVisible();

  // ④ 无 Skip / Later（产品裁决 Q1）
  await expect(
    page.getByRole("button", { name: /跳过|稍后|skip|later/i })
  ).toHaveCount(0);

  // ⑤ 深链现状（R4）：不被拦——onboarding 门只在登录中转页，无路由级守卫
  //    （B09-18 fixme 同源待裁决；若产品补守卫，此处改回 toHaveURL(/\/onboarding/)）
  await page.goto("/projects");
  await expect(page).toHaveURL(/\/projects$/, { timeout: 20_000 });

  // ⑥ API 断言（R5 口径）：team=null + onboarding_required=true + tenant_id 为镜像租户 uuid
  const me = await api("GET", "/v1/pm/me", token);
  expect(me.status).toBe(200);
  expect(me.body.onboarding_required).toBe(true);
  expect(me.body.team).toBeNull();
  expect(typeof me.body.tenant_id).toBe("string");
  const tenantId = me.body.tenant_id as string;

  // ⑦ DB 断言（R5 口径）：不产生虚假默认 Team = 无 ACTIVE 成员 + 未认领。
  //    行数 ≠ 0：provision 已自动落 inactive member 行（auth.rs:616-624）。
  const activeMembers = psql(
    `SELECT count(*) FROM core.tenant_member tm
       JOIN core.principal p ON p.id = tm.principal_id
      WHERE tm.tenant_id = '${tenantId}' AND p.is_active`
  );
  expect(Number(activeMembers)).toBe(0);
  const slug = psql(`SELECT slug FROM core.tenant WHERE id = '${tenantId}'`);
  expect(slug).toBe("");
});

/** P0-003 · Team 创建与邀请全链（R57 修正：受邀者与 owner 同 tid） */
test("P0-003 UI 建团 → API 邀请 → 受邀者 UI 接受 → owner/member 落库", async ({
  context,
  page
}) => {
  const tid = 9103;
  const ownerSub = "p0-003-owner";
  const memberSub = "p0-003-member";
  const ownerToken = signToken(ownerSub, tid);
  const memberToken = signToken(memberSub, tid);

  // ① Owner UI 建团
  await signInVia(context, ownerSub, tid);
  await page.goto("/onboarding");
  await page.getByTestId("onboarding-choice-create").click();
  await expect(page).toHaveURL(/\/onboarding\/create$/);
  await page.getByTestId("onboarding-team-name").locator("input").fill("P0-003 团队");
  await page.getByTestId("onboarding-team-slug").locator("input").fill("p0-003-team");
  await page.getByTestId("onboarding-create-submit").click();
  await expect(page).toHaveURL(/\/onboarding\/invite/, { timeout: 20_000 });
  await expect(page.getByTestId("onboarding-created-banner")).toContainText("P0-003 团队");

  // ② Owner 身份（API 双证据；P4-ORG Phase 2：Team 锚 = team_id）
  const ownerMe = await api("GET", "/v1/pm/me", ownerToken);
  expect(ownerMe.status).toBe(200);
  expect(ownerMe.body.onboarding_required).toBe(false);
  expect(ownerMe.body.team_role).toBe("owner");
  const teamId = ownerMe.body.memberships[0].team_id as string;

  // ③ 发邀请（真实链路 → 201）
  const inv = await api(
    "POST",
    `/v1/pm/teams/${teamId}/invitations`,
    ownerToken,
    { email: `${memberSub}@p0.test`, role: "member" }
  );
  expect(inv.status).toBe(201);
  const invitationToken = inv.body.token as string;
  expect(invitationToken).not.toBe("");

  // ④ 受邀者 UI 接受（R57：同 tid 不同 sub——跨 tid 被服务端硬拒）
  await signInVia(context, memberSub, tid);
  await page.goto(`/invitation/accept?token=${invitationToken}&type=invite`);
  await expect(page.getByTestId("accept-confirm")).toBeVisible({ timeout: 20_000 });
  await page.getByTestId("accept-confirm").click();
  await expect(page.getByTestId("accept-result")).toBeVisible({ timeout: 20_000 });

  // ⑤ 终态 API 断言：受邀者已入团为 member
  const memberMe = await api("GET", "/v1/pm/me", memberToken);
  expect(memberMe.status).toBe(200);
  expect(memberMe.body.onboarding_required).toBe(false);
  expect(memberMe.body.team?.id).toBe(teamId);

  // ⑥ DB 双证据：恰好 2 个 ACTIVE 成员（owner + member），角色正确
  //    （P4-ORG Phase 2：成员关系事实源 = core.team_member）
  const roles = psql(
    `SELECT string_agg(tm.role, ',' ORDER BY tm.role) FROM core.team_member tm
       JOIN core.principal p ON p.id = tm.principal_id
      WHERE tm.team_id = '${teamId}' AND p.is_active`
  );
  expect(roles).toBe("member,owner");
});
