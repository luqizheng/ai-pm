import { createAuditServices, type AuditServices } from "audit-services";
import { getAccessToken } from "./token-storage";
import { sessionFetch } from "./session";

/** lmms audit-service SDK（日志/合规导出/保留策略），走 /gw → api-gateway。 */
export const audit: AuditServices = createAuditServices({
  baseUrl: import.meta.env.VITE_IAM_BASE_URL ?? "/gw",
  getToken: () => getAccessToken(),
  fetch: sessionFetch
});
