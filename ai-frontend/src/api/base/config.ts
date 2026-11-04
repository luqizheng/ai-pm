import { createConfigServices, type ConfigServices } from "config-services";
import { getAccessToken } from "./token-storage";

/** lmms config-service SDK（命名空间/配置项/导入导出），走 /gw → api-gateway。 */
export const config: ConfigServices = createConfigServices({
  baseUrl: import.meta.env.VITE_IAM_BASE_URL ?? "/gw",
  getToken: () => getAccessToken(),
  fetch: fetch.bind(globalThis)
});
