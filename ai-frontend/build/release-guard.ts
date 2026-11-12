import type { Plugin } from "vite";

/**
 * 发布构建的 mock 门禁（2026-11-08 用户裁决：发布前端禁止 mock 数据）。
 *
 * 两道闸：
 *   1. `production` / `staging` 构建时 `VITE_USE_MOCK=1` 直接失败；
 *   2. 产物任一 chunk 引用了 fixtures / demo / 内置 mock 模块直接失败。
 *
 * 后者是硬证据：只要演示数据混进 dist，无论走哪个环境变量入口都会被拦下。
 * 开发（serve）与单测（.env.test）不受影响。
 */
const RELEASE_MODES = new Set(["production", "staging"]);

const MOCK_MODULE_PATTERN =
  /(?:^|[\\/])(?:fixtures|demo|src[\\/]api[\\/]mock)(?:[\\/]|[.])/;

export function releaseMockGuard(): Plugin {
  let mode = "";

  return {
    name: "ai-pm-release-mock-guard",
    apply: "build",
    configResolved(config) {
      mode = config.mode;
    },
    generateBundle(_options, bundle) {
      if (!RELEASE_MODES.has(mode)) return;

      const hits: string[] = [];
      for (const output of Object.values(bundle)) {
        if (output.type !== "chunk") continue;
        const moduleIds = output.moduleIds ?? Object.keys(output.modules ?? {});
        for (const id of moduleIds) {
          const normalized = id.replace(/\\/g, "/");
          if (MOCK_MODULE_PATTERN.test(normalized)) {
            hits.push(`${output.fileName} ← ${normalized}`);
          }
        }
      }

      if (hits.length > 0) {
        throw new Error(
          [
            "[release-guard] 发布构建产物包含 mock/fixtures/demo 模块，已中止：",
            ...hits.map(hit => `  - ${hit}`),
            "演示数据只能经 VITE_USE_MOCK=1 在本地 dev/单测中加载，发布构建必须走真实接口或空态。"
          ].join("\n")
        );
      }
    }
  };
}
