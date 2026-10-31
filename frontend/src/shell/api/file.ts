import { createFileServices, type FileServices } from "file-services";
import { getAccessToken } from "./token-storage";

/** lmms file-service SDK（multipart 上传/二进制下载/预签名），走 /gw → api-gateway。 */
export const file: FileServices = createFileServices({
  baseUrl: import.meta.env.VITE_IAM_BASE_URL ?? "/gw",
  getToken: () => getAccessToken(),
  fetch: fetch.bind(globalThis)
});
