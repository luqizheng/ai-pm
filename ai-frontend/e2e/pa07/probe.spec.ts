import { expect, test } from "@playwright/test";
import { browserSession, signToken } from "../flow/stack";

const SUB = "pa07-smoke-owner";
const TID = 990_307;

test("probe: /onboarding 401 请求序列", async ({ page }) => {
  await page.addInitScript(
    tokens => {
      for (const [key, value] of Object.entries(tokens)) {
        window.localStorage.setItem(key, value as string);
      }
    },
    browserSession(SUB, TID)
  );
  page.on("response", res => {
    if (res.status() >= 400) {
      const headers = res.request().headers();
      console.log(`[probe] ${res.status()} ${res.request().method()} ${res.url()} auth=${headers.authorization ? "yes" : "NO"}`);
    }
  });
  await page.goto("/onboarding");
  await page.getByText("首次登录必须创建或加入", { exact: false }).waitFor({ timeout: 30_000 });
  await page.evaluate(() =>
    (document.querySelector("[data-testid='onboarding-choice-create']") as HTMLButtonElement)?.click()
  );
  await expect(page).toHaveURL(/\/onboarding\/create$/, { timeout: 30_000 });
  await page.getByTestId("onboarding-team-name").locator("input").fill("probe team");
  await page.getByTestId("onboarding-create-submit").click();
  await page.waitForTimeout(5000);
});
