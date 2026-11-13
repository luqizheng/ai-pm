import { createFileServices, type FileServices } from "file-services";
import { getAccessToken } from "./token-storage";
import { sessionFetch } from "./session";

/**
 * lmms file-service SDK（multipart 上传/二进制下载/预签名）。
 *
 * baseUrl 走 pm 业务面（VITE_API_BASE）：用户面 `/v1/file/*` 由 pm-api 的文件
 * 代理承载（网关路由见 config/api-gateway/routes/file.router.toml）——
 * file-service 已收口为纯内部服务，只认 service-jwt，网关直连会被拒。
 */
export const file: FileServices = createFileServices({
  baseUrl: import.meta.env.VITE_API_BASE ?? "/api",
  getToken: () => getAccessToken(),
  fetch: sessionFetch
});
