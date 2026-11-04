import { getPluginsList } from "./build/plugins.ts";
import { mockPlugin } from "./build/mock-plugin.ts";
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
  const useMock = env.VITE_USE_MOCK === "1";

  return {
    base: VITE_PUBLIC_PATH,
    root,
    resolve: {
      alias
    },
    server: {
      port: VITE_PORT,
      host: "0.0.0.0",
      // 本地跨域代理：/api 前缀转发到 pm-api（PM_API_URL 环境变量可覆盖目标）；
      // /gw 前缀转发到 lmms api-gateway（iam-services SDK 的 IAM 调用走这里）
      proxy: {
        "/api": {
          target: process.env.PM_API_URL ?? "http://127.0.0.1:8090",
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
    plugins: [...getPluginsList(), mockPlugin(useMock)],
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
