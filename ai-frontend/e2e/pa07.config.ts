import { defineConfig } from "@playwright/test";

/**
 * PA-07 冒烟核对（Figma 179:3603，2026-11-21）专用配置。
 *
 * 复用 flow 全流程栈的独立后端（e2e/flow/global-setup.ts：一次性库
 * aipm_e2e_flow_ui + 本地 JWKS 18496 + pm-api jwks 模式 18495），vite 8860
 * 零 mock——与主 playwright.config（8848 fixtures）完全隔离，两套可并行。
 * 用法见 e2e/pa07/README（runner 命令）。
 */
export default defineConfig({
  testDir: "./pa07",
  timeout: 90_000,
  expect: { timeout: 15_000 },
  fullyParallel: false,
  reporter: [["list"]],
  use: {
    baseURL: "http://localhost:8860",
    channel: "msedge",
    headless: true,
    viewport: { width: 1600, height: 1000 },
    screenshot: "only-on-failure",
    trace: "retain-on-failure"
  },
  webServer: {
    command: "pnpm exec vite --port 8860 --strictPort",
    url: "http://localhost:8860",
    reuseExistingServer: true,
    timeout: 180_000,
    env: {
      VITE_USE_MOCK: "0",
      PM_API_URL: "http://127.0.0.1:18495"
    }
  }
});
