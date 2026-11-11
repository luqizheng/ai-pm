import { defineConfig } from "@playwright/test";

/**
 * P0 发布验收 · 可执行套件（docs/e2e-spec/11~17 号规格的落地载体）
 *
 * 复用 team-qa 真实后端栈（e2e/team/global-setup.ts：一次性库 + 本地 JWKS +
 * 真 pm-api jwks 模式），testMatch 只收 `p0-*.spec.ts`（D3/D4：P0 是发布门禁
 * 资产，与 Team QA 回归基线分文件、同端口）。证据落 `test-case/evidence/p0/`
 * ——不写 team-qa 目录，避免覆盖其 73 PASS / 7 FAIL 的既有报告。
 *
 * 与 team-qa 的端口（:8850 / :18490 / :18491）串行复用，不可并行跑两套。
 */
const PM_API = process.env.TEAM_QA_PM_API ?? "http://127.0.0.1:18490";

export default defineConfig({
  testDir: "./e2e",
  testMatch: "p0-*.spec.ts",
  globalSetup: "./e2e/p0/global-setup.ts",
  timeout: 90_000,
  expect: { timeout: 15_000 },
  fullyParallel: false,
  workers: 1,
  reporter: [
    ["list"],
    ["json", { outputFile: "../test-case/evidence/p0/playwright-results.json" }],
    [
      "html",
      { outputFolder: "../test-case/evidence/p0/playwright-report", open: "never" }
    ]
  ],
  outputDir: "../test-case/evidence/p0/playwright-artifacts",
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
      // DEF-E2E-04 教训：/gw 代理默认打 127.0.0.1:3000，会静默命中本机在跑的
      // full-dev 网关 → setup-status 误报 → 登录页被换成 /auth/setup。
      // 显式钉死死端口，让 fetchSetupStatus 走 fail-open=null。
      LMMS_GATEWAY_URL: "http://127.0.0.1:59999"
    }
  }
});
