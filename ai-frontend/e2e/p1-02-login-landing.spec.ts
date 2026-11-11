import { expect, test, type BrowserContext, type Page } from "@playwright/test";

import { api, claimTeam, signToken } from "./team/stack";
import { buildOwner, createProjectFor, signInVia, type TestOwner } from "./p0/fixtures";

/**
 * P1-02 · UX-052 Personal Home 登录默认落地（docs/e2e-spec/19 号 P1-02）
 *
 * 锚点（R74-R78）：登录路由 = /auth/login（标签「邮箱或用户名/口令/登录」）；
 * 落地逻辑 = /auth/signing-in 中转页 settleDestination，优先级
 * onboarding 门 > ?redirect= > 默认 /personal；守卫 beforeEach：
 * 无 token → /auth/login?redirect=<fullPath>；已登录访问登录页 → /dashboard
 * （R77 实际行为，D47 记录分歧）；R76：returnTo 链路断裂（LoginView 中转
 * 不带 query）→ 登录恒落 /personal（TC-03 为证据记录用例，D49）。
 * 自动化口径：注入 = 真实自签 RS256 JWT + pm-api jwks 真验（非伪造）。
 */

test.describe.configure({ mode: "serial" });

const TID_A = 9512;
const TID_B = 9513;
const TID_NO_TEAM = 9599; // 从未 claimTeam 的租户

let ownerA: TestOwner;
let ownerB: TestOwner;

async function inject(context: BrowserContext, sub: string, tid: number): Promise<void> {
  await signInVia(context, sub, tid);
}

async function gotoSigningIn(page: Page): Promise<void> {
  // 真实落地逻辑入口：中转页会调用 settleDestination（内部真拉 GET /v1/pm/me）
  await page.goto("/auth/signing-in");
}

test.beforeAll(async () => {
  ownerA = await buildOwner("p1-ux052-a", TID_A, "team-p1-ux052-a", "p1u1");
  await createProjectFor(ownerA, "E2EP1U1", "e2e-ux052-a");
  ownerB = await buildOwner("p1-ux052-b", TID_B, "team-p1-ux052-b", "p1u2");
  await createProjectFor(ownerB, "E2EP1U2", "e2e-ux052-b");
});

/** TC-P1-02-01 · 正常登录默认进入 /personal（多项目与零项目用户一致） */
test("TC-01 登录后默认落地 /personal（不进项目页）", async ({ browser }) => {
  const context = await browser.newContext();
  await inject(context, ownerA.sub, ownerA.tid);
  const page = await context.newPage();

  await gotoSigningIn(page);
  await expect(page).toHaveURL(/\/personal$/);
  await expect(page.locator("h2").first()).toBeVisible();

  // 零项目用户同样落地 /personal（不被跳过）
  const emptyCtx = await browser.newContext();
  await inject(emptyCtx, ownerB.sub, ownerB.tid);
  const emptyPage = await emptyCtx.newPage();
  await gotoSigningIn(emptyPage);
  await expect(emptyPage).toHaveURL(/\/personal$/);
  await context.close();
  await emptyCtx.close();
});

/** TC-P1-02-02 · 未登录访问受保护页面 → /auth/login?redirect=…（守卫写入） */
test("TC-02 未登录访问 /personal 被守卫拦到登录页", async ({ browser }) => {
  const context = await browser.newContext(); // 无任何 token
  const page = await context.newPage();
  await page.goto("/personal");
  await expect(page).toHaveURL(/\/auth\/login\?redirect=%2Fpersonal$/);
  await expect(page.getByLabel("邮箱或用户名")).toBeVisible();
  await context.close();
});

/**
 * TC-P1-02-03 · 登录后返回原目标页（R76 改判：证据记录用例）
 * 守卫写入 redirect ✓ → LoginView 中转丢弃 ✗ → 实际恒落 /personal。
 * D49 修复（透传 query）后本用例改为断言落回 /projects。
 */
test("TC-03 returnTo 链路证据：守卫写入 redirect，落地恒 /personal", async ({ browser }) => {
  const context = await browser.newContext();
  const page = await context.newPage();

  await page.goto("/projects"); // 未登录深链
  await expect(page).toHaveURL(/\/auth\/login\?redirect=%2Fprojects$/); // 守卫写入 ✓

  await inject(context, ownerA.sub, ownerA.tid);
  await gotoSigningIn(page);
  await expect(page).toHaveURL(/\/personal$/); // 中转丢失 redirect（R76，D49）
  await context.close();
});

/** TC-P1-02-04 · 刷新 /personal 恢复（无循环、不要求重登） */
test("TC-04 刷新 /personal 后仍保持", async ({ browser }) => {
  const context = await browser.newContext();
  await inject(context, ownerA.sub, ownerA.tid);
  const page = await context.newPage();
  await gotoSigningIn(page);
  await expect(page).toHaveURL(/\/personal$/);

  await page.reload();
  await expect(page).toHaveURL(/\/personal$/);
  await expect(page.locator("h2").first()).toBeVisible();
  await context.close();
});

/** TC-P1-02-05 · 已登录用户访问登录页 → /dashboard（R77 实际行为，D47 分歧） */
test("TC-05 已登录访问 /auth/login 被送出登录流程（实际落 /dashboard）", async ({ browser }) => {
  const context = await browser.newContext();
  await inject(context, ownerA.sub, ownerA.tid);
  const page = await context.newPage();
  await page.goto("/auth/login");
  await expect(page).not.toHaveURL(/\/auth\/login/); // 核心验收：不再停留登录页
  await expect(page).toHaveURL(/\/dashboard$/); // 实际落点（D47：产品若要 /personal 属需求变更）
  await context.close();
});

/** TC-P1-02-06 · Personal Home 导航进出项目（基本导航，不替代 SEC-054） */
test("TC-06 从 /personal 进出项目往返正常", async ({ browser }) => {
  const context = await browser.newContext();
  await inject(context, ownerA.sub, ownerA.tid);
  const page = await context.newPage();
  await gotoSigningIn(page);
  await expect(page).toHaveURL(/\/personal$/);

  const listA = await api("GET", "/v1/pm/projects", ownerA.token);
  const pid = listA.body.items[0].id as string;
  await page.goto(`/project/command-center?project_id=${pid}`);
  await expect(page.getByText("项目初始化")).toBeVisible();

  await page.goBack();
  await expect(page).toHaveURL(/\/personal$/);
  await context.close();
});

/** TC-P1-02-07 · 无 Team 用户 → onboarding 门优先；建团后落地 /personal */
test("TC-07 无 Team 用户被引导 Onboarding，建团后进入 /personal", async ({ browser }) => {
  const sub = "p1-ux052-noteam";
  const token = signToken(sub, TID_NO_TEAM);

  const context = await browser.newContext();
  await inject(context, sub, TID_NO_TEAM);
  const page = await context.newPage();
  await gotoSigningIn(page);
  await expect(page).toHaveURL(/\/onboarding/); // 门优先于 redirect 与默认落地（R75）
  await expect(page.getByTestId("onboarding-choice-create")).toBeVisible();
  await context.close();

  // 建团（真实链路）后重走登录落地 → /personal
  await claimTeam(sub, TID_NO_TEAM, "team-p1-noteam", "p1u9");
  const ctx2 = await browser.newContext();
  await inject(ctx2, sub, TID_NO_TEAM);
  const page2 = await ctx2.newPage();
  await gotoSigningIn(page2);
  await expect(page2).toHaveURL(/\/personal$/);
  await ctx2.close();
});

/**
 * TC-P1-02-08 · 多 Team（多 tid）用户：基线 = 各会话独立落地、列表互斥。
 * TeamSwitcher 单会话内切换的令牌语义待 D48 —— 落地后补切换断言。
 */
test("TC-08 多 tid 会话各自落地 /personal 且项目列表互斥", async ({ browser }) => {
  for (const owner of [ownerA, ownerB]) {
    const context = await browser.newContext();
    await inject(context, owner.sub, owner.tid);
    const page = await context.newPage();
    await gotoSigningIn(page);
    await expect(page).toHaveURL(/\/personal$/);
    await context.close();
  }
  const listA = await api("GET", "/v1/pm/projects", ownerA.token);
  const listB = await api("GET", "/v1/pm/projects", ownerB.token);
  const idsA = listA.body.items.map((p: { id: string }) => p.id);
  const idsB = listB.body.items.map((p: { id: string }) => p.id);
  expect(idsA).toHaveLength(listA.body.items.length);
  for (const id of idsB) expect(idsA).not.toContain(id);
});
