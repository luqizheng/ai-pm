import { defineConfig } from "@playwright/test";

/**
 * Team QA · UI E2E（真实后端：独立 pm-api + 自签 JWKS，见 e2e/team/global-setup.ts）
 *
 * 与 playwright.config.ts（S02，fixtures 演示态 :8848）分开：本套件独占 vite :8850，
 * 代理 /api → 独立 pm-api（PM_API_URL），不复用任何已在跑的 dev server。
 * 入口：test-case/evidence/team-qa/run-team-qa.sh。
 */
const PM_API = process.env.TEAM_QA_PM_API ?? "http://127.0.0.1:18490";

export default defineConfig({
  testDir: "./e2e",
  testMatch: "team-onboarding.spec.ts",
  globalSetup: "./e2e/team/global-setup.ts",
  timeout: 60_000,
  expect: { timeout: 15_000 },
  fullyParallel: false,
  workers: 1,
  reporter: [
    ["list"],
    [
      "json",
      { outputFile: "../test-case/evidence/team-qa/playwright-results.json" }
    ],
    [
      "html",
      {
        outputFolder: "../test-case/evidence/team-qa/playwright-report",
        open: "never"
      }
    ]
  ],
  outputDir: "../test-case/evidence/team-qa/playwright-artifacts",
  use: {
    baseURL: "http://localhost:8850",
    channel: "msedge",
    headless: true,
    viewport: { width: 1440, height: 900 },
    locale: "zh-CN",
    screenshot: "only-on-failure",
    trace: "retain-on-failure"
  },
  webServer: {
    command: "pnpm exec vite --port 8850 --strictPort",
    url: "http://localhost:8850",
    reuseExistingServer: false,
    timeout: 180_000,
    env: { PM_API_URL: PM_API, VITE_USE_MOCK: "0", VITE_PORT: "8850" }
  }
});
