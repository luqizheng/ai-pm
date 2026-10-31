import { createIamServices, type IamServices } from "iam-services";
import { getAccessToken } from "./token-storage";

/**
 * lmms IAM SDK（iam-services）—— 上游 lmms 仓提供的官方客户端，
 * 从私有 registry（http://192.168.2.12:14873/，域名别名 npm.kudi.repos）安装，
 * 版本随上游 tag 升级（SDK 扩展协议：上游改 → 发布 → 下游升包）。
 *
 * baseUrl 走 `/gw` 前缀：vite dev 代理与生产边缘将其转发到 lmms api-gateway
 * （目标可用 LMMS_GATEWAY_URL 覆盖，默认 http://127.0.0.1:3000）。
 * SDK 不做自动刷新 —— 单飞刷新仍由 @digua/http 客户端承担（见 ./client.ts）。
 */
export const iam: IamServices = createIamServices({
  baseUrl: import.meta.env.VITE_IAM_BASE_URL ?? "/gw",
  getToken: () => getAccessToken(),
  // SDK 的 HttpCore 默认捕获**未 bind 的原生 fetch**，浏览器里以方法形式调用会抛
  // "Illegal invocation"（Node 无此限制，故上游测试未暴露）。显式注入绑定版 fetch，
  // 上游修复（HttpCore 内 fetch.bind）随 SDK 扩展协议反馈 lmms。
  fetch: fetch.bind(globalThis)
});
