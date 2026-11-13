import { createPrueServices, type PrueServices } from "prue-services";
import { getAccessToken } from "./token-storage";
import { sessionFetch } from "./session";

/**
 * lmms prue_backend SDK（动态路由 + 菜单配置版本，ETag 乐观锁），走 /gw → api-gateway。
 * 壳的动态菜单协议（get-async-routes）与菜单配置管理面都来自这里。
 */
export const prue: PrueServices = createPrueServices({
  baseUrl: import.meta.env.VITE_IAM_BASE_URL ?? "/gw",
  getToken: () => getAccessToken(),
  fetch: sessionFetch
});
