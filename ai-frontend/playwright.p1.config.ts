import { defineConfig } from "@playwright/test";

/**
 * P1 执行批 · 可执行套件（docs/e2e-spec/19 号规格的落地载体）
 *
 * 复用 P0 真实后端栈（e2e/p0/global-setup.ts：一次性库 + 本地 JWKS + 真
 * pm-api jwks 模式 + 真 ai-pm-worker + mock LLM——同一套进程，用毕即毁），
 * testMatch 只收 `p1-*.spec.ts`。证据落 `test-case/evidence/p1/`。
 *
 * 与 P0 / team-qa 的端口（:8850 / :18490 / :18491）串行复用，不可并行跑两套。
 */
const PM_API = process.env.TEAM_QA_PM_API ?? "http://127.0.0.1:18490";

export default defineConfig({
  testDir: "./e2e",
  testMatch: "p1-*.spec.ts",
  globalSetup: "./e2e/p0/global-setup.ts",
  timeout: 120_000,
  expect: { timeout: 15_000 },
  fullyParallel: false,
  workers: 1,
  reporter: [
    ["list"],
    ["json", { outputFile: "../test-case/evidence/p1/playwright-results.json" }],
    [
      "html",
      { outputFolder: "../test-case/evidence/p1/playwright-report", open: "never" }
    ]
  ],
  outputDir: "../test-case/evidence/p1/playwright-artifacts",
  use: {
    baseURL: "http://127.0.0.1:8850",
    channel: "msedge",
    headless: true,
    viewport: { width: 1440, height: 900 },
    locale: "zh-CN",
    screenshot: "only-on-failure",
    trace: "retain-on-failure"
  },
  webServer: {
    command: "pnpm exec vite --port 8850 --strictPort",
    url: "http://127.0.0.1:8850",
    reuseExistingServer: false,
    timeout: 180_000,
    env: {
      PM_API_URL: PM_API,
      VITE_USE_MOCK: "0",
      VITE_PORT: "8850",
      // DEF-E2E-04 教训（同 P0 配置）：钉死网关端口避免误命中本机 full-dev。
      LMMS_GATEWAY_URL: "http://127.0.0.1:59999"
    }
  }
});
