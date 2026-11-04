import type { Connect, Plugin, ViteDevServer } from "vite";

import { resolveMock } from "../src/api/mock/fixtures.ts";
import { resolveInitializationMock } from "../src/api/mock/initialization.ts";

/**
 * pm-api 未就绪期间的 dev mock 层：在 vite 代理之前拦截 /v1/* 请求并返回 fixtures。
 *
 * 由 VITE_USE_MOCK=1 启用（.env.development）。壳与业务代码零改动 ——
 * 接真实后端时置 0，本插件整体下线，pm-api 按 fixtures 里的契约实现端点即可。
 */
export function mockPlugin(enabled: boolean): Plugin {
  if (!enabled) return { name: "ai-pm-mock-disabled" };

  return {
    name: "ai-pm-mock",
    apply: "serve",
    configureServer(server: ViteDevServer) {
      const middleware: Connect.NextHandleFunction = (req, res, next) => {
        const url = new URL(req.url ?? "/", "http://localhost");
        // 归一化代理前缀：直连 /v1/* 与走 /api（pm-api/BFF）、/gw（lmms 网关）
        // 的 /v1/* 都拦截
        const pathname = url.pathname.replace(/^\/(api|gw)(?=\/)/, "");
        if (!pathname.startsWith("/v1/")) return next();

        // 请求体（审批 decide 等端点需要）；登录体不参与匹配，解析失败按 undefined 处理
        const chunks: Buffer[] = [];
        req.on("data", chunk => chunks.push(chunk as Buffer));
        req.on("end", () => {
          const raw = Buffer.concat(chunks).toString("utf8");
          let body: unknown;
          try {
            body = raw === "" ? undefined : JSON.parse(raw);
          } catch {
            body = undefined;
          }
          // S01 初始化域端点优先匹配（与 pm-api 契约一致，见
          // docs/s01-backend-api-requirements-v1.md）；未命中再走通用 fixtures。
          const result =
            resolveInitializationMock(
              req.method ?? "GET",
              pathname,
              url.searchParams,
              body
            ) ??
            resolveMock(req.method ?? "GET", pathname, url.searchParams, body);
          if (!result) return next();
          res.statusCode = result.status;
          res.setHeader("Content-Type", "application/json; charset=utf-8");
          res.end(JSON.stringify(result.body));
        });
      };
      server.middlewares.use(middleware);
      server.config.logger.info(
        "  ai-pm-mock: VITE_USE_MOCK=1，/v1/* 由内置 fixtures 响应"
      );
    }
  };
}
