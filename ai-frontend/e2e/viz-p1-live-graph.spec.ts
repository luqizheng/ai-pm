import { expect, test, type APIRequestContext, type Page } from "@playwright/test";

/**
 * Visualization P1 · Live 交互图真栈 E2E（AC-P1-2 / AC-P1-6）。
 *
 * 前提 = checkpoint 栈在线（同 s02-ui-live.spec.ts 头注）；pm-api 的
 * Live 端点已就绪（P0）。数据自洽：不预置图形数据也可运行——
 * 空数据断言空态/表格降级，有数据断言图渲染（服务端裁剪后的内容）。
 *
 * 覆盖：
 * - 三张图（trace-graph / task-dag / architecture-graph）页签切换与呈现；
 * - 图/表切换（交互图 ↔ VizSpecTable）；
 * - mixed-revision / truncated 提示条（如出现）不阻断页面；
 * - AC-P1-6：未 active 关系不出边——断言 Live 响应与 UI 无 draft 边
 *   （服务端已裁剪，前端只渲染返回内容）。
 *
 * 运行：npx playwright test e2e/viz-p1-live-graph.spec.ts
 * 环境变量：VIZ_GATEWAY / VIZ_PROJECT_ID / VIZ_USERNAME / VIZ_PASSWORD
 */

const GATEWAY = process.env.VIZ_GATEWAY ?? "http://127.0.0.1:3000";
const USERNAME = process.env.VIZ_USERNAME ?? "leo";
const PASSWORD = process.env.VIZ_PASSWORD ?? "Checkpoint@123456";
const PROJECT_ID = process.env.VIZ_PROJECT_ID ?? "764906ca-6e6f-47b7-98a2-abc6d6233d10";

interface LoginResult {
  access_token: string;
  refresh_token: string;
  user?: { id: string; username: string; nickname: string | null };
}

test.beforeAll(async () => {
  const res = await fetch(`${GATEWAY}/health`).catch(() => null);
  test.skip(
    !res || !res.ok,
    `checkpoint 栈不在线（${GATEWAY}/health 不可达）——真栈 E2E 需要本机栈`
  );
});

async function apiLogin(page: Page): Promise<LoginResult> {
  const res = await page.request.post("/api/v1/iam/auth/login", {
    data: { username: USERNAME, password: PASSWORD }
  });
  expect(res.ok(), `登录失败：${res.status()}`).toBeTruthy();
  return (await res.json()) as LoginResult;
}

/** 注入真实 token + 锁定 zh-CN，绕过登录页直达业务页。 */
async function injectSession(page: Page, session: LoginResult) {
  await page.addInitScript(
    ([access, refresh, userId, username, nickname]) => {
      window.localStorage.setItem("prism_access_token", access!);
      window.localStorage.setItem("prism_refresh_token", refresh!);
      window.localStorage.setItem("prism_expires_at", String(Date.now() + 3_600_000));
      window.localStorage.setItem(
        "prism_user",
        JSON.stringify({
          id: userId,
          username,
          nickname,
          email: null,
          phone: null,
          avatar: null,
          roles: []
        })
      );
      window.localStorage.setItem("prism_locale", "zh-CN");
    },
    [
      session.access_token,
      session.refresh_token,
      session.user?.id ?? "",
      session.user?.username ?? USERNAME,
      session.user?.nickname ?? null
    ] as const
  );
}

/** 直达可视化页并等待 Live 区就绪。 */
async function openVisualizations(page: Page) {
  await page.goto(`/project/visualizations?project_id=${PROJECT_ID}`);
  await expect(page.getByTestId("viz-live-tabs")).toBeVisible();
}

/** Live 响应里不允许出现 draft 关系（AC-P1-6 前端面）。 */
async function assertNoDraftEdges(request: APIRequestContext, session: LoginResult) {
  const res = await request.get(
    `/api/v1/pm/visualizations/projects/${PROJECT_ID}/live/trace-graph`,
    { headers: { Authorization: `Bearer ${session.access_token}` } }
  );
  expect(res.ok()).toBeTruthy();
  const body = (await res.json()) as {
    data: { edges: Array<{ relation: string }> };
  };
  const relations = (body.data?.edges ?? []).map(e => e.relation);
  for (const relation of relations) {
    expect(
      ["design.covers_requirement", "task.implements_requirement", "derived_trace"].includes(relation),
      `未 active 的关系不应出现在 Live 响应：${relation}`
    ).toBe(true);
  }
}

test("三张 Live 图页签切换与图/表切换", async ({ page }) => {
  const session = await apiLogin(page);
  await injectSession(page, session);
  await openVisualizations(page);

  const tabs = page.getByTestId("viz-live-tab");
  await expect(tabs).toHaveCount(3);

  // trace-graph（默认选中）：图容器或空态出现，页面不崩
  await expect(page.getByTestId("live-graph")).toBeVisible();
  const graphMode = page.getByTestId("live-graph-mode-graph");
  await expect(graphMode).toBeVisible();

  // 图/表切换：表格模式复用 VizSpecTable（空数据时仍渲染表容器）
  await page.getByTestId("live-graph-mode-table").click();
  await expect(page.getByTestId("viz-spec-table")).toBeVisible();
  await page.getByTestId("live-graph-mode-graph").click();

  // task-dag 与 architecture-graph 切换后 Live 组件持续可用
  await tabs.nth(1).click();
  await expect(page.getByTestId("live-graph")).toBeVisible();
  await tabs.nth(2).click();
  await expect(page.getByTestId("live-graph")).toBeVisible();
});

test("Live 响应不含未 active 关系（AC-P1-6）", async ({ page }) => {
  const session = await apiLogin(page);
  await assertNoDraftEdges(page.request, session);
});

test("渲染降级路径：渲染器异常时表格兜底（AC-P1-5 e2e 面）", async ({ page }) => {
  const session = await apiLogin(page);
  await injectSession(page, session);
  // 拦截 G6 adapter 模块请求模拟加载失败 → LiveGraphView 应降级表格而非崩页
  await page.route("**/src/lib/viz/renderers/g6Adapter.ts*", route => route.abort());
  await openVisualizations(page);
  await expect(page.getByTestId("live-graph-degraded")).toBeVisible();
  await expect(page.getByTestId("viz-spec-table")).toBeVisible();
});