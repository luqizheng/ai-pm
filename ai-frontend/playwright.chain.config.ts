import { defineConfig } from "@playwright/test";

/**
 * 阶段 D · P1 真实业务链 E2E（交付物 05 V1.1 §1 L3/L4 载体）
 *
 * 复用 P0 栈（e2e/p0/global-setup.ts）：一次性库 + 本地 JWKS + 真 pm-api(jwks)
 * + 真 ai-pm-worker + mock-llm（FakeModel v1）。本套件为 API 级真实链
 * （仓库分析 → KB → PC → 人工决策 → close → 幂等/恢复），不依赖浏览器 UI。
 *
 * 证据落 `test-case/evidence/chain/`；端口与 P0/Team QA 串行复用（不可并行）。
 * 环境变量（run-chain.sh 注入）：TEAM_QA_PM_API_BIN / P0_WORKER_BIN /
 * P0_DB=aipm_chain_e2e / P0_MOCK_LLM_PORT。
 */
const PM_API = process.env.TEAM_QA_PM_API ?? "http://127.0.0.1:18490";

export default defineConfig({
  testDir: "./e2e",
  testMatch: "chain/*.spec.ts",
  globalSetup: "./e2e/p0/global-setup.ts",
  timeout: 300_000,
  expect: { timeout: 20_000 },
  fullyParallel: false,
  workers: 1,
  reporter: [
    ["list"],
    ["json", { outputFile: "../test-case/evidence/chain/playwright-results.json" }],
    [
      "html",
      { outputFolder: "../test-case/evidence/chain/playwright-report", open: "never" }
    ]
  ],
  outputDir: "../test-case/evidence/chain/playwright-artifacts",
  use: {
    baseURL: PM_API
  }
});
