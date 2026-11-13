import {
  createNotificationServices,
  type NotificationServices
} from "notification-services";
import { getAccessToken } from "./token-storage";
import { sessionFetch } from "./session";

/** lmms notification-service SDK（公告/未读数，不含 WS），走 /gw → api-gateway。 */
export const notification: NotificationServices = createNotificationServices({
  baseUrl: import.meta.env.VITE_IAM_BASE_URL ?? "/gw",
  getToken: () => getAccessToken(),
  fetch: sessionFetch
});
