import { getPluginsList } from "./build/plugins.ts";
import { mockPlugin } from "./build/mock-plugin.ts";
import { releaseMockGuard } from "./build/release-guard.ts";
import { include, exclude } from "./build/optimize.ts";
import { type UserConfigExport, type ConfigEnv, loadEnv } from "vite";
import { root, alias, wrapperEnv, pathResolve } from "./build/utils.ts";

/**
 * ai-frontend 基础工程的构建配置。
 *
 * 这是**独立的前端工程** —— 不依赖 pure-admin 的构建链：
 * 没有 Tailwind、没有 unplugin-icons、没有 CDN 替换、没有 element-plus。
 */
export default async ({ mode }: ConfigEnv): Promise<UserConfigExport> => {
  const { VITE_PORT, VITE_PUBLIC_PATH } = wrapperEnv(loadEnv(mode, root));
  const env = loadEnv(mode, root);
  // mock 开关由 .env.development 的 VITE_USE_MOCK 驱动（2026-11-06 起默认 0 = 真实模式，
  // /api 与 /gw 都走网关；pm-api 未实现的端点在 build/mock-plugin.ts 保留兜底能力）
  const useMock = env.VITE_USE_MOCK === "1";
  const isReleaseMode = mode === "production" || mode === "staging";
  if (isReleaseMode && useMock) {
    throw new Error(
      "[release-guard] 发布构建禁止 mock：production/staging 不得设置 VITE_USE_MOCK=1"
    );
  }

  return {
    base: VITE_PUBLIC_PATH,
    root,
    resolve: {
      alias
    },
    server: {
      port: VITE_PORT,
      host: "0.0.0.0",
      // 本地跨域代理：/api 与 /gw 都指向 lmms api-gateway（2026-11-05 架构口径：
      // pm 业务与 iam 一致挂网关对外，pm-api:8090 不发布宿主；网关侧路由见
      // ai-pm infrastructure/config/api-gateway/routes/pm.router.toml）。
      // PM_API_URL 环境变量可覆盖（直连 pm-api 调试时用，如 http://127.0.0.1:8090）
      proxy: {
        "/api": {
          target: process.env.PM_API_URL ?? "http://127.0.0.1:3000",
          changeOrigin: true,
          rewrite: (p: string) => p.replace(/^\/api/, "")
        },
        "/gw": {
          target: process.env.LMMS_GATEWAY_URL ?? "http://127.0.0.1:3000",
          changeOrigin: true,
          rewrite: (p: string) => p.replace(/^\/gw/, "")
        }
      },
      // 预热文件以提前转换和缓存结果，降低启动期间的初始页面加载时长并防止转换瀑布
      warmup: {
        clientFiles: ["./index.html", "./src/entry.ts", "./src/App.vue"]
      }
    },
    plugins: [
      ...getPluginsList(),
      mockPlugin(useMock),
      releaseMockGuard()
    ],
    // https://cn.vitejs.dev/config/dep-optimization-options.html#dep-optimization-options
    optimizeDeps: {
      include,
      exclude
    },
    build: {
      // https://cn.vitejs.dev/guide/build.html#browser-compatibility
      target: "es2020",
      sourcemap: false,
      chunkSizeWarningLimit: 4000,
      rolldownOptions: {
        input: {
          index: pathResolve("index.html")
        },
        // 静态资源分类打包
        output: {
          chunkFileNames: "static/js/[name]-[hash].js",
          entryFileNames: "static/js/[name]-[hash].js",
          assetFileNames: "static/[ext]/[name]-[hash].[ext]"
        },
        checks: {
          pluginTimings: false,
          toleratedTransform: false
        }
      }
    },
    define: {
      __INTLIFY_PROD_DEVTOOLS__: false
    }
  };
};
