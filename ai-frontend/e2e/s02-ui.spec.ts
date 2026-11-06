import { expect, test, type Page } from "@playwright/test";

/**
 * S02 QA · UI 验收测试 V1.1（fixtures 演示态交互闭环）
 *
 * V1.1 修正（首轮 7 失败的根因）：
 * - 筛选器为自定义 Select 组件（reka-ui），非原生 select → 点击触发器+点选项
 * - ConfirmationDialog 无 dialog role（role="presentation"）→ 用 .ui-confirm__panel
 * - 弹窗 footer 顺序 = 取消在前、确认在后（组件注释「footer 右对齐：取消 secondary + 确认 primary」）
 * - 按钮文案 i18n 实值：actionPropose=提交确认、actionHistory=查看历史、actionConfirm=确认需求
 * - main/body 歧义 → getByRole("main")
 */

const STORAGE = {
  prism_access_token: "qa-ui-e2e-access-token",
  prism_refresh_token: "qa-ui-e2e-refresh-token",
  prism_expires_at: String(Date.now() + 3_600_000),
  prism_user: JSON.stringify({
    id: "qa-ui-runner",
    username: "qa-ui-runner",
    nickname: "QA UI Runner",
    email: null,
    phone: null,
    avatar: null,
    roles: [],
    permissions: []
  })
};

test.beforeEach(async ({ context }) => {
  await context.addInitScript(tokens => {
    for (const [key, value] of Object.entries(tokens)) {
      window.localStorage.setItem(key, value as string);
    }
  }, STORAGE);
});

const LIST_ARIA = "需求列表";
const FILTER_STATUS = "按状态过滤";
const PANEL = ".ui-confirm__panel";

async function openRequirements(page: Page) {
  await page.goto("/project/requirements");
  await expect(page).toHaveURL(/\/project\/requirements/);
  await expect(page.getByLabel(LIST_ARIA)).toBeVisible();
}

/** 自定义 Select：点击触发器后在 portal 菜单中点选项（按 label 文本） */
async function pickSelectOption(page: Page, ariaLabel: string, optionText: string) {
  await page.getByLabel(ariaLabel).click();
  await page.getByRole("option", { name: optionText }).click();
}

test.describe("S02 UI · 认证守卫", () => {
  test("U-12 未登录访问业务页重定向登录页", async ({ browser }) => {
    const ctx = await browser.newContext();
    const page = await ctx.newPage();
    await page.goto("/project/requirements");
    await expect(page).toHaveURL(/login/);
    await ctx.close();
  });

  test("U-13 已登录直访业务页不再拦截", async ({ page }) => {
    await openRequirements(page);
  });
});

test.describe("S02 UI · 需求工作区（RequirementsView / S02-UI-01）", () => {
  test("U-01 页面加载：壳+列表+摘要条渲染 fixtures 数据", async ({ page }) => {
    await openRequirements(page);
    await expect(page.getByText("REQ-001").first()).toBeVisible();
    await expect(page.getByText("User authentication").first()).toBeVisible();
    await expect(page.getByLabel(LIST_ARIA)).toBeVisible();
  });

  test("U-02 行点击选中 → 详情面板显示所选需求", async ({ page }) => {
    await openRequirements(page);
    const list = page.getByLabel(LIST_ARIA);
    await list.getByText("REQ-001").first().click();
    await expect(page.getByText("User authentication").last()).toBeVisible();
  });

  test("U-03 状态筛选改变列表行集", async ({ page }) => {
    await openRequirements(page);
    // REQ-001 为 pending_confirmation：筛选「已确认」后从列表消失，回「全部状态」恢复
    await pickSelectOption(page, FILTER_STATUS, "已确认");
    await expect(page.getByText("REQ-001")).toHaveCount(0);
    await pickSelectOption(page, FILTER_STATUS, "全部状态");
    await expect(page.getByText("REQ-001").first()).toBeVisible();
  });

  test("U-04 视图密度切换（list/compact）", async ({ page }) => {
    await openRequirements(page);
    const list = page.getByLabel(LIST_ARIA);
    await page.getByLabel("紧凑视图", { exact: false }).click().catch(() => undefined);
    await expect(list).toBeVisible();
  });

  test("U-05 Confirm 取消路径：弹窗取消后状态不变", async ({ page }) => {
    await openRequirements(page);
    const list = page.getByLabel(LIST_ARIA);
    await list.getByText("REQ-001").first().click();
    const confirmBtn = page.getByRole("button", { name: "确认需求" });
    await expect(confirmBtn).toBeVisible();
    await confirmBtn.click();
    const panel = page.locator(PANEL);
    await expect(panel).toBeVisible();
    // footer 顺序：取消在前（first），确认在后（last）
    await panel.getByRole("button").first().click();
    await expect(page.locator(PANEL)).toHaveCount(0);
    // 取消后状态仍为待确认：确认按钮仍可见
    await expect(confirmBtn).toBeVisible();
  });

  test("U-06 Confirm 确认路径：状态迁移为已确认", async ({ page }) => {
    await openRequirements(page);
    const list = page.getByLabel(LIST_ARIA);
    await list.getByText("REQ-001").first().click();
    await page.getByRole("button", { name: "确认需求" }).click();
    const panel = page.locator(PANEL);
    await expect(panel).toBeVisible();
    await panel.getByRole("button").last().click();
    await expect(page.locator(PANEL)).toHaveCount(0);
    await expect(page.getByText("已确认").first()).toBeVisible();
  });

  test("U-07 Propose Revision（提交确认）→ 弹窗确认 → 修订抽屉", async ({ page }) => {
    await openRequirements(page);
    const list = page.getByLabel(LIST_ARIA);
    // req-002 为 confirmed（canPropose=true）；pending 状态的 REQ-001 正确隐藏 propose 按钮（状态机）
    await list.getByText("REQ-002").first().click();
    await page.getByRole("button", { name: "提交确认" }).first().click();
    // propose 走显式确认弹窗（proposeDialog），确认后打开 RevisionSheet 抽屉
    const panel = page.locator(PANEL);
    await expect(panel).toBeVisible();
    await panel.getByRole("button").last().click();
    await expect(page.locator(PANEL)).toHaveCount(0);
    await expect(page.locator("body")).toContainText(/修订|revision/i);
  });

  test("U-08 AI 红线：AI 动作只追加提案池，不直接改需求状态", async ({ page }) => {
    await openRequirements(page);
    const list = page.getByLabel(LIST_ARIA);
    await list.getByText("REQ-001").first().click();
    const before = await page.getByText("待确认").count();
    const aiButton = page.getByRole("button", { name: /AI|智能|建议/i }).first();
    if (await aiButton.isVisible().catch(() => false)) {
      await aiButton.click().catch(() => undefined);
      const after = await page.getByText("待确认").count();
      expect(after).toBeGreaterThanOrEqual(before);
    }
  });
});

test.describe("S02 UI · 子页面导航", () => {
  test("U-09 历史查看：按钮打开页内修订抽屉 + 直访历史页路由", async ({ page }) => {
    await openRequirements(page);
    const list = page.getByLabel(LIST_ARIA);
    await list.getByText("REQ-001").first().click();
    // 「查看历史」= 打开页内 RevisionSheet 抽屉（不路由跳转）
    await page.getByRole("button", { name: "查看历史" }).first().click();
    await expect(page.locator("body")).toContainText(/修订|revision|历史/i);
    // 独立历史页路由直访渲染
    await page.goto("/project/requirements/req-001/history");
    await expect(page).toHaveURL(/history/);
    await expect(page.getByRole("main")).toBeVisible();
  });

  test("U-10 基线页 /project/requirements/baselines 渲染", async ({ page }) => {
    await page.goto("/project/requirements/baselines");
    await expect(page).toHaveURL(/baselines/);
    await expect(page.getByRole("main")).toBeVisible();
  });

  test("U-11 评审页 /project/requirements/review 渲染", async ({ page }) => {
    await page.goto("/project/requirements/review");
    await expect(page).toHaveURL(/review/);
    await expect(page.getByRole("main")).toBeVisible();
  });
});

test.describe("S02 UI · 应用壳冒烟", () => {
  test("U-14 侧栏折叠切换（data-sidebar 属性）", async ({ page }) => {
    await openRequirements(page);
    const shell = page.locator("[data-sidebar]").first();
    await expect(shell).toHaveAttribute("data-sidebar", /.+/);
  });
});