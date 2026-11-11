import { configDefaults, defineConfig, mergeConfig } from "vitest/config";
import viteConfig from "./vite.config";

/**
 * vitest 配置。
 *
 * 单独成文件的原因：vite.config.ts 里的 build.rolldownOptions 是 vite 8 / rolldown
 * 的选项，vitest 2.x 的类型定义还不认识它，放进同一个文件会类型不兼容。
 *
 * 超时放宽到 20s：部分 legacy 测试用真实定时器（防抖 / 延时），
 * 并行负载下 5s 默认超时不够。
 *
 * 排除 `e2e/**`：那是 Playwright 套件（真浏览器 + 真后端，见
 * `playwright.config.ts` / `playwright.team.config.ts`），不是 vitest 用例。
 * vitest 默认 glob 会把它们收进来，在 collect 阶段因缺 Playwright 运行时而报假红，
 * 掩盖真正的单元测试失败。
 */
const base =
  typeof viteConfig === "function"
    ? await viteConfig({ command: "serve", mode: "test" })
    : viteConfig;

export default mergeConfig(
  base,
  defineConfig({
    test: {
      testTimeout: 20000,
      hookTimeout: 20000,
      exclude: [...configDefaults.exclude, "e2e/**"]
    }
  })
);
