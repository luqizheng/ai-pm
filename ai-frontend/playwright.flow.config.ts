import { defineConfig } from "@playwright/test";

/**
 * E2E 全流程 · UI 套件（UIFLOW-01~08，真实后端：独立 pm-api + 自签 JWKS + 一次性库）
 *
 * 与 playwright.team.config.ts（:8850/18490/18491）完全隔离：本套件独占
 * vite :8860，代理 /api → 独立 pm-api :18495（PM_API_URL），不复用任何
 * 已在跑的 dev server。入口：test-case/evidence/e2e-flow/run-e2e-flow-ui.sh。
 */
const PM_API = process.env.FLOW_E2E_PM_API ?? "http://127.0.0.1:18495";

export default defineConfig({
  testDir: "./e2e",
  testMatch: "flow.spec.ts",
  globalSetup: "./e2e/flow/global-setup.ts",
  timeout: 90_000,
  expect: { timeout: 15_000 },
  fullyParallel: false,
  workers: 1,
  reporter: [
    ["list"],
    [
      "json",
      { outputFile: "../test-case/evidence/e2e-flow/playwright-ui-results.json" }
    ],
    [
      "html",
      {
        outputFolder: "../test-case/evidence/e2e-flow/playwright-ui-report",
        open: "never"
      }
    ]
  ],
  outputDir: "../test-case/evidence/e2e-flow/playwright-ui-artifacts",
  use: {
    baseURL: "http://127.0.0.1:8860",
    channel: "msedge",
    headless: true,
    viewport: { width: 1440, height: 900 },
    locale: "zh-CN",
    screenshot: "only-on-failure",
    trace: "retain-on-failure"
  },
  webServer: {
    command: "pnpm exec vite --port 8860 --strictPort",
    url: "http://127.0.0.1:8860",
    reuseExistingServer: false,
    timeout: 180_000,
    env: {
      PM_API_URL: PM_API,
      VITE_USE_MOCK: "0",
      VITE_PORT: "8860",
      // 本栈没有 iam 网关。若不显式指死端口，vite 的 /gw 代理会落到默认
      // 127.0.0.1:3000——恰好命中本机在跑的 full-dev 网关，setup-status
      // 误报「空系统须注册」→ 登录页被换成 /auth/setup（环境泄漏，2026-11-10
      // UIFLOW-07 首跑实测）。指死端口让 fetchSetupStatus 走 fail-open=null，
      // 登录页稳定停留，套件与外部环境解耦。
      LMMS_GATEWAY_URL: "http://127.0.0.1:59999"
    }
  }
});
