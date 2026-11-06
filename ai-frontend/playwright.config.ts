import { defineConfig } from "@playwright/test";

/**
 * S02 QA · UI 验收测试（Playwright）
 * 目标：ai-frontend dev server（vite :8848，fixtures 演示态页面）
 * 浏览器：系统 MS Edge（channel，免 Chromium 下载）
 * 视口：1440×900（设计基线桌面档位）
 */
export default defineConfig({
  testDir: "./e2e",
  timeout: 60_000,
  expect: { timeout: 10_000 },
  fullyParallel: false,
  reporter: [
    ["list"],
    [
      "html",
      {
        outputFolder: "../test-case/evidence/s02-ui/playwright-report",
        open: "never"
      }
    ]
  ],
  use: {
    baseURL: "http://localhost:8848",
    channel: "msedge",
    headless: true,
    viewport: { width: 1440, height: 900 },
    screenshot: "only-on-failure",
    trace: "retain-on-failure"
  },
  webServer: {
    command: "pnpm dev",
    url: "http://localhost:8848",
    reuseExistingServer: true,
    timeout: 180_000
  }
});