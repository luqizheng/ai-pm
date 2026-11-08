import {
  expect,
  test,
  type APIRequestContext,
  type Page
} from "@playwright/test";

/**
 * S02 UI · 真栈 E2E V1.0（2026-11-08 接真批的后继验收）
 *
 * 前提 = 本机 checkpoint 栈活着（网关 :3000 /health ok；pm-api 读 pm_ckpt；
 * iam 读 lmms_ckpt）。fixtures 演示态已退役（见 s02-ui.spec.ts 头注），本 spec
 * 直接驱动真实 REST：登录 → 需求工作区 → 详情页 → 修订历史页。
 *
 * 数据自洽：spec 自动确保项目内存在冒烟需求 REQ-901（缺失则现场走全链造数：
 * 创建 → 起草 → 提交 → 确认（hash+CAS）→ 登记并关联来源），可重复运行。
 *
 * 运行：npx playwright test e2e/s02-ui-live.spec.ts（webServer 自动起 vite :8848）
 * 环境变量：S02_GATEWAY（默认 http://127.0.0.1:3000）、S02_PROJECT_ID、
 *          S02_USERNAME（默认 leo）、S02_PASSWORD（默认 Checkpoint@123456）
 */

const GATEWAY = process.env.S02_GATEWAY ?? "http://127.0.0.1:3000";
const USERNAME = process.env.S02_USERNAME ?? "leo";
const PASSWORD = process.env.S02_PASSWORD ?? "Checkpoint@123456";
const PROJECT_ID =
  process.env.S02_PROJECT_ID ?? "764906ca-6e6f-47b7-98a2-abc6d6233d10";
const DEMO_CODE = "REQ-901";
const DEMO_TITLE = "冒烟需求：登录会话";
const DEMO_BODY = "连续失败5次锁定账户";
const DEMO_SOURCE = "冒烟访谈纪要";
const NO_OVERWRITE_NOTE =
  "历史不可覆盖：本页不提供恢复或覆盖旧版本的操作；变更一律通过新增修订完成。";

interface LoginResult {
  access_token: string;
  refresh_token: string;
  user?: {
    id: string;
    username: string;
    nickname: string | null;
  };
}

test.beforeAll(async () => {
  // 栈不在线则整体跳过（本 spec 是真栈验收，不是单测）
  const res = await fetch(`${GATEWAY}/health`).catch(() => null);
  test.skip(
    !res || !res.ok,
    `checkpoint 栈不在线（${GATEWAY}/health 不可达）——真栈 E2E 需要本机栈`
  );
});

/** 走 vite 代理（/api → 网关）API 登录，返回真实会话。 */
async function apiLogin(page: Page): Promise<LoginResult> {
  const res = await page.request.post("/api/v1/iam/auth/login", {
    data: { username: USERNAME, password: PASSWORD }
  });
  expect(
    res.ok(),
    `登录失败：${res.status()} ${await res.text()}`
  ).toBeTruthy();
  return (await res.json()) as LoginResult;
}

/** 带 Bearer 的请求助手（page.request 不自动带会话头）。 */
function authed(
  request: APIRequestContext,
  session: LoginResult
): {
  get: (path: string) => Promise<any>;
  post: (path: string, data: unknown) => Promise<any>;
  patch: (path: string, data: unknown) => Promise<any>;
} {
  const headers = { Authorization: `Bearer ${session.access_token}` };
  return {
    get: path => request.get(path, { headers }),
    post: (path, data) => request.post(path, { headers, data }),
    patch: (path, data) => request.patch(path, { headers, data })
  };
}

/** 注入真实 token + 锁定 zh-CN 文案，绕过登录页直达业务页。 */
async function injectSession(page: Page, session: LoginResult) {
  await page.addInitScript(
    ([access, refresh, userId, username, nickname]) => {
      window.localStorage.setItem("prism_access_token", access);
      window.localStorage.setItem("prism_refresh_token", refresh);
      window.localStorage.setItem(
        "prism_expires_at",
        String(Date.now() + 3_600_000)
      );
      window.localStorage.setItem(
        "prism_user",
        JSON.stringify({
          id: userId,
          username,
          nickname,
          email: null,
          phone: null,
          avatar: null,
          roles: [],
          permissions: []
        })
      );
      window.localStorage.setItem("aipm-locale", "zh-CN");
    },
    [
      session.access_token,
      session.refresh_token,
      session.user?.id ?? "e2e-user",
      session.user?.username ?? USERNAME,
      session.user?.nickname ?? null
    ] as const
  );
}

/**
 * 确保项目内存在冒烟需求 REQ-901（含确认与来源）；缺失则现场全链造数。
 * 链路 = 创建（v1 draft）→ 提交 → 确认（expected_content_hash + row_revision
 * CAS）→ 登记来源 → 关联来源。返回需求 id。
 */
async function ensureDemoRequirement(
  request: APIRequestContext,
  session: LoginResult
): Promise<string> {
  const api = authed(request, session);

  const list = await api.get(`/api/v1/pm/projects/${PROJECT_ID}/requirements`);
  const items = (await list.json()).items as Array<{
    id: string;
    code: string;
    row_revision: number;
    current_revision_id: string | null;
    latest_revision_id: string | null;
    latest_state: string | null;
  }>;
  const existing = items.find(item => item.code === DEMO_CODE);
  if (
    existing &&
    existing.current_revision_id &&
    existing.latest_state === "approved"
  ) {
    // 已有定稿版本（前次运行产物）——来源在创建链路中已关联过
    return existing.id;
  }
  if (existing) {
    // 残缺态（中断的造数）：走生命周期到定稿
    await promoteToApproved(api, existing.id, existing.row_revision);
    return existing.id;
  }

  const created = await api.post(
    `/api/v1/pm/projects/${PROJECT_ID}/requirements`,
    {
      code: DEMO_CODE,
      title: DEMO_TITLE,
      requirement_type: "functional",
      priority: "high",
      markdown: `# 登录会话\n${DEMO_BODY}。`,
      description: "支持邮箱登录"
    }
  );
  expect(created.ok(), `造数创建失败：${created.status()}`).toBeTruthy();
  const { requirement } = (await created.json()) as {
    requirement: { id: string; row_revision: number };
  };
  await promoteToApproved(api, requirement.id, requirement.row_revision);
  return requirement.id;
}

/** 提交+确认当前最新 draft，并补齐来源关联（幂等：已确认则跳过）。 */
async function promoteToApproved(
  api: ReturnType<typeof authed>,
  requirementId: string,
  rowRevision: number
): Promise<void> {
  const detail = (await (
    await api.get(`/api/v1/pm/requirements/${requirementId}`)
  ).json()) as {
    revisions: Array<{
      id: string;
      version: number;
      state: string;
      content_hash: string;
    }>;
  };
  const latest = detail.revisions.reduce((a, b) =>
    a.version > b.version ? a : b
  );
  if (latest.state === "draft") {
    await api.post(
      `/api/v1/pm/requirements/${requirementId}/revisions/${latest.id}/submit`,
      {}
    );
    await api.post(
      `/api/v1/pm/requirements/${requirementId}/revisions/${latest.id}/confirm`,
      {
        expected_content_hash: latest.content_hash,
        expected_row_revision: rowRevision
      }
    );
  }

  // 来源（幂等：link 重复会 409/200，先查后链）
  const sources = await api.get(
    `/api/v1/pm/requirements/${requirementId}/source-inputs`
  );
  const linked = (
    (await sources.json()) as {
      items: Array<{ title: string }>;
    }
  ).items.some(item => item.title === DEMO_SOURCE);
  if (!linked) {
    const registered = await api.post(
      `/api/v1/pm/projects/${PROJECT_ID}/source-inputs`,
      {
        input_kind: "human_input",
        title: DEMO_SOURCE,
        content: "用户希望支持邮箱登录",
        source_ref: "MIN-004"
      }
    );
    expect(
      registered.ok(),
      `来源登记失败：${registered.status()}`
    ).toBeTruthy();
    const inputId = (
      (await registered.json()) as {
        source_input: { id: string };
      }
    ).source_input.id;
    await api.post(`/api/v1/pm/requirements/${requirementId}/source-inputs`, {
      input_id: inputId
    });
  }
}

async function mountLivePage(page: Page): Promise<string> {
  const session = await apiLogin(page);
  await injectSession(page, session);
  const reqId = await ensureDemoRequirement(page.request, session);
  return reqId;
}

test.describe("S02 真栈 · 需求工作区", () => {
  test("L-01 登录链 + 列表加载真实需求（REQ-901）", async ({ page }) => {
    await mountLivePage(page);
    await page.goto(`/project/requirements?project_id=${PROJECT_ID}`);

    const list = page.getByLabel("需求列表");
    await expect(list).toBeVisible();
    await expect(list.getByText(DEMO_CODE)).toBeVisible();
    // 详情面板默认选中（真实 REST 读模型）
    await expect(page.getByText(DEMO_TITLE).first()).toBeVisible();
  });

  test("L-02 未登录访问业务页重定向登录页（守卫不因接真退化）", async ({
    page
  }) => {
    await page.goto("/project/requirements");
    await expect(page).toHaveURL(/login/);
  });
});

test.describe("S02 真栈 · 需求详情页", () => {
  test("L-03 详情页四卡 + 正文块 + 追溯链来源", async ({ page }) => {
    const reqId = await mountLivePage(page);
    await page.goto(`/project/requirements/${reqId}`);

    await expect(page.getByText(DEMO_CODE).first()).toBeVisible();
    await expect(page.getByText(DEMO_TITLE).first()).toBeVisible();
    // 正文块（当前正式版本 Markdown）
    await expect(page.getByText(DEMO_BODY)).toBeVisible();
    // 追溯链来源（S02-I3 真数据）
    await expect(page.getByText(DEMO_SOURCE).first()).toBeVisible();
    // 规格卡诚实占位（S02-I5 未实施）
    await expect(page.getByText("结构化规格暂无承载")).toBeVisible();
  });
});

test.describe("S02 真栈 · 修订历史页", () => {
  test("L-04 时间线版本 + 确认决策 + 比较版本 diff", async ({ page }) => {
    const reqId = await mountLivePage(page);
    await page.goto(`/project/requirements/${reqId}/history`);

    await expect(page.getByText("修订时间线")).toBeVisible();
    // 至少两版可比较（v1 / v2 徽标）
    await expect(page.getByText("v1", { exact: true }).first()).toBeVisible();
    await expect(page.getByText("v2", { exact: true }).first()).toBeVisible();
    // 确认决策映射自 Confirmation 记录（审批人类链路真实回显）
    await expect(page.getByText(/于\s.*确认定稿/).first()).toBeVisible();

    // 比较版本：行内 diff 出现新增行
    await page.getByRole("button", { name: "比较版本", exact: true }).click();
    await expect(page.locator(".rhc__row--added").first()).toBeVisible();
  });

  test("L-05 历史不可覆盖：页面明示且无恢复/覆盖动作（§7 红线）", async ({
    page
  }) => {
    const reqId = await mountLivePage(page);
    await page.goto(`/project/requirements/${reqId}/history`);

    await expect(page.getByText(NO_OVERWRITE_NOTE)).toBeVisible();
    const texts = await page.locator("button").allTextContents();
    expect(texts.some(t => t.includes("恢复") || t.includes("覆盖"))).toBe(
      false
    );
  });
});
