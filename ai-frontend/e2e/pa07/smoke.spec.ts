import { expect, test, type BrowserContext, type Page } from "@playwright/test";

import { api, browserSession, psql, signToken } from "../flow/stack";

/**
 * PA-07 冒烟核对（Figma 179:3603「Task & Execution」，真后端零 mock）。
 *
 * 建真实数据链（UI 建团 → UI 建项目 → API 建任务 → DB 直插产品门禁/依赖/run）
 * 后逐页核对五个新页面的真实渲染并截图到 e2e/pa07/artifacts/：
 *   A 列表（调度条件四态） / C 焦点关系 / B 依赖页 / D Revision / E 产品 / 00 看板。
 * 产物为人工可视化核对证据；断言取「后端真实读模型可见的最小事实」。
 */

test.describe.configure({ mode: "serial" });

const SUB = "pa07-smoke-owner";
const TID = 990_307;
const TEAM_NAME = "PA07 冒烟团队";
const TEAM_SLUG = "pa07-smoke-team";
const PROJECT_CODE = "PA07SMK";

let token = "";
let projectId = "";
let taskMainId = "";
let taskPredId = "";
let productId = "";

/** ui/Input、ui/Textarea 的 testid 落在包裹 div 上：取其内（或自身）的原生控件。 */
const field = (loc: ReturnType<Page["getByTestId"]>) =>
  loc.locator("xpath=descendant-or-self::*[self::input or self::textarea]").first();

async function signIn(context: BrowserContext, sub = SUB, tid = TID) {
  await context.addInitScript(
    tokens => {
      for (const [key, value] of Object.entries(tokens)) {
        window.localStorage.setItem(key, value as string);
      }
    },
    browserSession(sub, tid)
  );
}

/** RETURNING 单值提取：psql -tAc 的 INSERT 标签行（INSERT 0 1）一并输出，取首行。 */
const scalar = (sql: string): string => psql(sql).split("\n")[0].trim();

async function seedWorld(page: Page) {
  // UI 建团（onboarding choice-create）。choice 页在 /v1/pm/me 探测返回后会
  // 整体重渲染（按钮 detach），Playwright 重试永远输给重渲染——等渲染完成信号
  // 出现后用 evaluate 直接点 DOM（e2e 踩坑记忆同源）。
  await page.goto("/onboarding");
  await page.getByText("首次登录必须创建或加入", { exact: false }).waitFor({ timeout: 30_000 });
  await page.evaluate(() =>
    (document.querySelector("[data-testid='onboarding-choice-create']") as HTMLButtonElement)?.click()
  );
  await expect(page).toHaveURL(/\/onboarding\/create$/, { timeout: 30_000 });
  await field(page.getByTestId("onboarding-team-name")).fill(TEAM_NAME);
  await field(page.getByTestId("onboarding-team-slug")).fill(TEAM_SLUG);
  await page.getByTestId("onboarding-create-submit").click();
  await expect(page).toHaveURL(/\/onboarding\/invite/, { timeout: 30_000 });

  // UI 建项目（创建后进 Command Center，URL 携带 project_id）
  await page.goto("/project/create");
  const form = page.locator("form.pc-page__form");
  await form.getByPlaceholder("例如 Atlas Platform", { exact: true }).fill("pa07-smoke-project");
  await form.getByPlaceholder("例如 ATLAS", { exact: true }).fill(PROJECT_CODE);
  await form.locator("button[type='submit']").click();
  await expect(page).toHaveURL(/\/project\/command-center\?project_id=/, { timeout: 30_000 });
  projectId = new URL(page.url()).searchParams.get("project_id") ?? "";
  expect(projectId).not.toBe("");

  // 依赖边 created_by → principal.id（FK），先取物化 principal。
  const me = await api("GET", "/v1/pm/me", token);
  const principalId = me.body.principal_id as string;

  // API 建两个任务（require_human：owner token 是人工主体）
  const createMain = await api("POST", "/v1/pm/tasks", token, {
    project_id: projectId,
    title: "Implement authentication API",
    description: "PA-07 smoke main task",
    priority: "urgent"
  });
  expect(createMain.status).toBe(201);
  taskMainId = createMain.body.id;
  const createPred = await api("POST", "/v1/pm/tasks", token, {
    project_id: projectId,
    title: "Publish API schema",
    description: "PA-07 smoke predecessor",
    priority: "high"
  });
  expect(createPred.status).toBe(201);
  taskPredId = createPred.body.id;
  // 第三个任务：无依赖/无产品/无 run → Ready to Start（紫）。
  const createReady = await api("POST", "/v1/pm/tasks", token, {
    project_id: projectId,
    title: "Build token audit trail",
    description: "PA-07 smoke ready task",
    priority: "normal"
  });
  expect(createReady.status).toBe(201);

  // DB 直插产品门禁世界（归属 onboarding 建的 tenant；main 关联 normal 产品，
  // pred 关联 suspended 产品 → Blocked by Product Suspension 派生态）。
  const tenant = psql(`SELECT tenant_id FROM core.project WHERE id = '${projectId}'`);
  // core.product.team_id → core.team.id（0038 组织模型；tenant 是租户面，两者不同表）。
  const teamId = scalar(`SELECT id FROM core.team WHERE tenant_id = '${tenant}'`);
  productId = scalar(
    `INSERT INTO core.product (team_id, tenant_id, key, name, status) \
     VALUES ('${teamId}', '${tenant}', 'PA07-AUTH', 'Auth service', 'normal') RETURNING id`
  );
  const suspendedId = scalar(
    `INSERT INTO core.product (team_id, tenant_id, key, name, status) \
     VALUES ('${teamId}', '${tenant}', 'PA07-BILL', 'Billing API', 'suspended') RETURNING id`
  );
  psql(
    `INSERT INTO core.product_project_link (product_id, project_id, tenant_id) \
     VALUES ('${productId}', '${projectId}', '${tenant}'), ('${suspendedId}', '${projectId}', '${tenant}')`
  );
  // 把测试 principal 设为两个产品的 Owner（fetchProduct/product tasks 要求 Product Member）。
  psql(
    `INSERT INTO core.product_member (product_id, principal_id, tenant_id, role) \
     VALUES ('${productId}', '${principalId}', '${tenant}', 'owner'), \
             ('${suspendedId}', '${principalId}', '${tenant}', 'owner')`
  );
  psql(
    `INSERT INTO delivery.task_product (tenant_id, project_id, task_id, product_id) \
     SELECT tenant_id, project_id, '${taskMainId}', '${productId}' FROM delivery.task WHERE id = '${taskMainId}'`
  );
  psql(
    `INSERT INTO delivery.task_product (tenant_id, project_id, task_id, product_id) \
     SELECT tenant_id, project_id, '${taskPredId}', '${suspendedId}' FROM delivery.task WHERE id = '${taskPredId}'`
  );
  // 依赖边：main depends on pred（completed 条件；pred 未完成 → Waiting 1/1）。
  psql(
    `INSERT INTO delivery.task_dependency (tenant_id, project_id, task_id, depends_on_task_id, created_by, required_condition) \
     SELECT tenant_id, project_id, '${taskMainId}', '${taskPredId}', '${principalId}', 'completed' \
     FROM delivery.task WHERE id = '${taskMainId}'`
  );
  // main 一条 queued run（EXECUTION 列 + Dashboard 正在执行）。
  psql(
    `INSERT INTO delivery.task_run (project_id, task_id, attempt_no, status, tenant_id) \
     VALUES ('${projectId}', '${taskMainId}', 1, 'queued', '${tenant}')`
  );
}

test("PA07-00 建真实数据世界（团/项目/任务/产品门禁/依赖/run）", async ({ context, page }) => {
  token = signToken(SUB, TID);
  const me = await api("GET", "/v1/pm/me", token);
  expect(me.status).toBe(200);
  await signIn(context);
  await seedWorld(page);
});

test("PA07-01 任务列表：PM-code 列 / 产品 / 五态聚合 / 执行 / 调度条件 + 右栏", async ({
  context,
  page
}, testInfo) => {
  await signIn(context);
  await page.goto(`/project/tasks/list?project_id=${projectId}`);
  await expect(page.getByTestId("tasks-tab-list")).toBeVisible();

  // 任务身份锚：0050 触发器发号 PM-NNN。
  await expect(page.getByText("PM-001").first()).toBeVisible({ timeout: 20_000 });
  await expect(page.getByText("PM-002").first()).toBeVisible();

  // 调度条件派生：main 有 queued run → Running；pred 关联 suspended 产品 → 红。
  await expect(page.getByText("Ready to Start").first()).toBeVisible();
  await expect(page.getByText("Blocked by Product Suspension")).toBeVisible();
  await expect(page.getByText("Run #1").first()).toBeVisible();
  // Harness 裁决：无数据源不显示。
  expect(await page.content()).not.toContain("Harness");

  // 右栏 Execution context：项目关联产品 + 语义文案。
  await expect(page.getByTestId("execution-context-panel")).toBeVisible();
  await expect(page.getByTestId("panel-products")).toContainText("Auth service");
  await expect(page.getByTestId("panel-products")).toContainText("Billing API");
  await expect(page.getByText("上下文而非绕行")).toBeVisible();

  await page.screenshot({
    path: `e2e/pa07/artifacts/${testInfo.title.replace(/\s+/g, "-")}-A-list.png`,
    fullPage: false
  });
});

test("PA07-02 依赖关系焦点视图三列", async ({ context, page }, testInfo) => {
  await signIn(context);
  await page.goto(`/project/tasks/relationships?project_id=${projectId}&focus=${taskMainId}`);
  await expect(page.getByTestId("rel-selected")).toBeVisible({ timeout: 20_000 });
  await expect(page.getByTestId("rel-depends-on")).toContainText("Publish API schema");
  await expect(page.getByText("AND 门禁")).toBeVisible();
  await page.screenshot({
    path: `e2e/pa07/artifacts/${testInfo.title.replace(/\s+/g, "-")}-C-relationships.png`
  });
});

test("PA07-03 任务依赖页：Blocked 面板 + 边满足态 + 三步流", async ({ context, page }, testInfo) => {
  await signIn(context);
  await page.goto(`/project/tasks/${taskMainId}/dependencies`);
  await expect(page.getByTestId("dep-gate")).toBeVisible({ timeout: 20_000 });
  await expect(page.getByTestId("dep-conditions-summary")).toContainText("0/1");
  await expect(page.getByTestId("dep-edge-satisfied").first()).toContainText("未满足");
  await expect(page.getByTestId("dep-task-code")).toContainText("PM-001");

  // 三步流打开（B2）。Vue 重渲染会让 locator click 输给 detach，evaluate 点 DOM。
  await page.evaluate(() =>
    (document.querySelector("[data-test='dep-add-dependency']") as HTMLButtonElement)?.click()
  );
  await expect(page.locator("[data-test='wizard-pick']")).toBeVisible({ timeout: 30_000 });
  await page.screenshot({
    path: `e2e/pa07/artifacts/${testInfo.title.replace(/\s+/g, "-")}-B-dependencies.png`
  });
});

test("PA07-04 Revision 历史：r1 冻结快照 + 时间线", async ({ context, page }, testInfo) => {
  await signIn(context);
  await page.goto(`/project/tasks/${taskMainId}/history`);
  await expect(page.getByTestId("rh-timeline")).toBeVisible({ timeout: 20_000 });
  // 创建即冻结 r1（PA07-IMP-002），Active 徽标在。
  await expect(page.getByTestId("rh-badge").first()).toBeVisible();
  await page.screenshot({
    path: `e2e/pa07/artifacts/${testInfo.title.replace(/\s+/g, "-")}-D-history.png`
  });
});

test("PA07-05 产品页：项目关联产品列表 + 四态徽标", async ({ context, page }, testInfo) => {
  await signIn(context);
  await page.goto(`/project/products?project_id=${projectId}`);
  await expect(page.getByTestId("products-list")).toBeVisible({ timeout: 20_000 });
  await expect(page.getByTestId("products-list")).toContainText("Auth service");
  await expect(page.getByTestId("products-list")).toContainText("Billing API");

  await page.evaluate(() =>
    (document.querySelector("[data-test='product-card']") as HTMLButtonElement)?.click()
  );
  await expect(page.getByTestId("product-overview")).toBeVisible({ timeout: 30_000 });
  await page.screenshot({
    path: `e2e/pa07/artifacts/${testInfo.title.replace(/\s+/g, "-")}-E-products.png`
  });
});

test("PA07-06 执行看板四块", async ({ context, page }, testInfo) => {
  await signIn(context);
  await page.goto(`/project/dashboard?project_id=${projectId}`);
  await expect(page.getByTestId("dash-overview")).toBeVisible({ timeout: 20_000 });
  await expect(page.getByTestId("dash-executing")).toContainText("Run #1");
  await expect(page.getByTestId("dash-stat-active")).toBeVisible();
  await page.screenshot({
    path: `e2e/pa07/artifacts/${testInfo.title.replace(/\s+/g, "-")}-00-dashboard.png`
  });
});
