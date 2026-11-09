# 对象存储（RustFS）落地与附件链路 — V1

> 2026-11-10 ｜ 分支 `feat/rustfs-s3-storage`
> 承接 E-4（object-storage 从未实现）/ D-IC-6（附件能力挂账）/ ADR-05（存储权威 = 对象存储 + 预签名 URL）。

## 一、结论

- 对象存储**已落地**：RustFS（S3 兼容，镜像 `zhcoder-docker-registry.com:8000/official/rustfs/rustfs:1.0.0-alpha.79`）作为附件与大产物的权威存储，由 `file-service` 的 **rustfs 后端**直连。
- 附件**读链路**已在业务面生效：AI 消息 / 产物 / 建议 / 信息卡渲染时，正文里的 `/v1/file/{id}` 现签现用 → 浏览器直取对象存储字节。
- 附件**写链路**在富文本编辑器可用（工具栏上传 / 粘贴上传）。
- **对话输入框附件仍未开放**：AI 请求契约里没有附件字段（`ai_interactions.rs` / `ai_conversations.rs` 均无 attachment/file 引用），需要另立切片改后端契约 + AI 上下文注入，故 CommandBar 与对话 Composer 的回形针**保持诚实禁用**。

## 二、链路

```
浏览器 ──multipart──► pm-api /v1/file/upload ──service-jwt──► file-service
                                                            │
                                                            │ rust-s3 (backend=rustfs)
                                                            ▼
                                                        RustFS :9000  ← 字节落这里
浏览器 ──GET 预签名 URL─────────────────────────────────────────┘（无 JWT 直取）
```

- **上传**走 multipart，不直连 S3：RustFS 兼容矩阵里 POST Object 表单上传未实现，且签名密钥不应下发到浏览器。
- **下载**走预签名 URL（SigV4 把 host 写进签名，浏览器侧无需 JWT）。

## 三、编排与配置

| 文件 | 变更 |
|---|---|
| `infrastructure/full-dev/docker-compose-services.yml` | 新增 `rustfs` 服务（`RUSTFS_VOLUMES=/data`，命名卷 `rustfs-data`）；`file-service` 切 `FILE_STORAGE_BACKEND=rustfs` + `S3_*` + `extra_hosts: rustfs.local:host-gateway`；本地卷 `file-storage` 退役 |
| `infrastructure/full-dev/.image.env` | 新增 `IMAGE_RUSTFS` |
| `infrastructure/full-dev/.env.example` | 新增 `RUSTFS_*` |
| `infrastructure/full-dev/docker-compose-services.{prod,remote}.yml` | rustfs 的端口与 restart 差异片 |
| `infrastructure/full-dev/config/api-gateway/routes/pm.router.toml` | **新建**：`/v1/pm/*` → pm-api（此前仓内无此文件，浏览器到不了 pm-api） |
| `infrastructure/full-dev/config/api-gateway/routes/file.router.toml` | 用户面 `/v1/file/*` 改指 pm-api（file-service 只认 service-jwt，网关铸不出来） |
| `infrastructure/full-dev/config/api-gateway/api-gateway.toml` | `egress_map` 补 `pm-api` |

### RUSTFS_PUBLIC_ENDPOINT 的硬约束

预签名 URL 的 host 写进 SigV4 签名 → **浏览器与容器必须解析到同一个 host**，否则 403 `SignatureDoesNotMatch`。当前用独立主机名 `rustfs.local`：

- 容器侧：compose `extra_hosts` 指到宿主网关 → 命中宿主发布的 9000 端口（已实测）。
- 浏览器侧：hosts/DNS 需把 `rustfs.local` 指到同一端口（本机 dev：`127.0.0.1 rustfs.local`；跨主机客户端：`<应用机 LAN IP> rustfs.local`）。已有可用域名时直接换成该域名。

## 四、实测结论（2026-11-10，RustFS 1.0.0-alpha.79）

| 能力 | 结果 |
|---|---|
| bucket 创建（`S3_AUTO_CREATE_BUCKET`） | ✅ file-service 自建 `ai-pm`（status=200） |
| 服务端 put/get/list/delete | ✅ |
| **预签名 PUT / GET** | ✅（浏览器式 PUT 200、GET 200） |
| CORS 预检 | ✅ `access-control-allow-origin: *` |
| 缺失对象预签名 GET | ✅ 404 |
| POST Object（表单直传） | ❌ 未实现 → 浏览器侧禁用直传，固定走 multipart |

**已知坑**：RustFS 以非 root（uid 10001）运行，卷必须挂到镜像自带的 `/data`（属主随镜像目录复制到命名卷）。挂到 `/data/<子目录>` 由 Docker 以 root 创建 → `Permission denied`；`RUSTFS_VOLUMES` 指向不存在/非挂载点 → `Volume not found`。

## 五、前端

| 文件 | 变更 |
|---|---|
| `src/api/base/file.ts` | baseUrl 从 `VITE_IAM_BASE_URL`（/gw）改到 `VITE_API_BASE`（/api）：用户面 `/v1/file/*` 由 pm-api 承载 |
| `src/api/base/attachments.ts` | 链路注释更新（此前是无人 import 的死代码） |
| `src/api/base/attachments.test.ts` | **新建**：上传参数映射 / 预签名 / 缓存 / 清缓存（6 例） |
| `RichTextEditorControl.vue` | 注入 `uploader` + `resolveUrl`（编辑器上传链路可达） |
| `AiConversationMessage.vue` / `ArtifactResponse.vue` / `InformationalResponse.vue` / `RecommendationResponse.vue` | 注入 `resolve-url`（正文里的附件引用现签现用） |

前端单测：1004 passed / 0 failed（`e2e/*.spec.ts` 的 3 个 Playwright 文件在 master 基线上同样失败，与本次无关）。

## 六、遗留

1. **对话输入框附件**（CommandBar / 对话 Composer）：需后端契约新增附件引用 + AI 上下文注入，本切片未做，按钮维持诚实禁用。
2. **浏览器侧 DNS/hosts**：部署前必须在客户端机器把 `rustfs.local`（或所选域名）解析到应用机的 9000 端口，否则预签名下载 403。
3. **产物级直传**：RustFS 兼容矩阵的 POST Object 未实现，浏览器直传只能走预签名 PUT；本仓暂以 multipart 代理上传，功能等价、代价是字节过一跳 pm-api。
