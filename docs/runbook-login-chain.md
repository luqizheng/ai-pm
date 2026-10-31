# 登录链路联调 Runbook（Slice B）

> 2026-10-31 首次全链路打通记录。链路：**浏览器(8848) → vite 代理 → lmms api-gateway(3000) → iam-service(9081→8081) → token-service(8082, RS256 签发) → pm-api(8090, JWKSCache 本地验签)**。

## 一、端口与通道

| 通道 | 前缀 | 目标 | 用途 |
|---|---|---|---|
| IAM | `/gw` | api-gateway `127.0.0.1:3000` | 登录/refresh/logout/setup-status、users/simple、菜单 get-async-routes |
| PM 业务 | `/api` | pm-api `127.0.0.1:8090` | `/v1/pm/*`（slice A 实现）、events/supervision、`/mcp` |

vite 代理见 `frontend/vite.config.ts`（`/api`→`PM_API_URL ?? 8090`；`/gw`→`LMMS_GATEWAY_URL ?? 3000`）。

## 二、起栈步骤

1. **lmms 鉴权栈**（在 `low-memory-micro-services/infrastructure/`，项目名同为 `infrastructure` → 与 ai-pm 栈共享 `infrastructure_superpowers` 网络）：
   ```bash
   docker compose -f docker-compose-infra.yml -f docker-compose-services.yml \
     up -d --no-deps consul redis mysql rabbitmq token-service iam-service api-gateway prue_backend
   ```
   基础设施四件套与 lmms 服务镜像本机已有，`--no-deps` 防止触碰 ai-pm 的 aipm-* 容器。

2. **pm-api 原生**（dev 循环推荐；容器版需重建 aipm/pm-api 镜像）：
   ```bash
   cd ai-pm
   DATABASE_URL="postgres://aipm:aipm@127.0.0.1:5433/aipm" \
   PM_API_PORT=8090 \
   AI_PM_AUTH_MODE=jwks \
   AI_PM_JWKS_URL=http://127.0.0.1:8082/.well-known/jwks.json \
   cargo run -p pm-api
   ```
   `AI_PM_AUTH_MODE=off` 为本地免鉴权（行为与 slice B 之前一致）。

3. **前端**：`cd frontend && pnpm dev`（`.env.development` 已 `VITE_USE_MOCK=0`）。

## 三、首次初始化（新 mysql 卷时）

- `GET :3000/v1/iam/auth/setup-status` → `{"setup_required":true,"user_count":0}` 时：
  ```bash
  curl -X POST :3000/v1/iam/auth/register -H "Content-Type: application/json" \
    -d '{"username":"admin","password":"Admin@123456","nickname":"Admin"}'
  ```
  首用户自动获得 `tenant_admin` + 全部启用权限。

## 四、验收矩阵（2026-10-31 全部通过）

| 用例 | 结果 |
|---|---|
| 无 token 打 pm-api `/api/v1/*` 与 `/mcp` | 401 `IAM_AUTH_1005` |
| 伪造 token | 401 `IAM_AUTH_1004` |
| 登录 token 带 Bearer | 200（notifications 空 feed） |
| gateway login → JWT（RS256, kid=default, aud=coder-micro-services, 含 tid/roles/permissions） | 200, expires_in=900 |
| refresh（不轮换） | 200 |
| logout → 204 → 原 token 再访问 | 401 `Token revoked`（gateway Redis jti 黑名单生效） |
| 浏览器真实登录 | 8848→守卫放行 `/dashboard`，localStorage 四键（prism_access_token 等），role=tenant_admin |
| 前端 vitest | 389/389 |
| ai-pm cargo test | 31 套件全绿（含 auth 5 用例） |

## 五、坑清单（务必读）

1. **cargo 代理**：git 全局 `http.proxy`（v2ray 127.0.0.1:10808）会被 cargo 回退采用。命令须带
   `CARGO_HTTP_PROXY="" NO_PROXY=192.168.2.12`（前者屏蔽死代理，外网走 rsproxy；后者内网 kellnr 直连）。
2. **reqwest 系统代理**：Windows 注册表残留 `ProxyServer=127.0.0.1:10808`（即使 ProxyEnable=0）会被 reqwest
   的 system-proxy 探测采用 → pm-api 的 JWKS 拉取必须 `.no_proxy()`（已在 `main.rs` 固化）。
3. **gateway JWKS 起动竞态**（上游缺陷）：gateway 不 depends_on token-service healthy，同批启动可能缓存到空
   JWKS → 一律 `Invalid or expired token`。**修复：`docker restart infrastructure-api-gateway-1`**（重拉 JWKS）。
4. **token-service 密钥不持久**：compose 不挂载 keys 卷，重启即重新生成 → 所有旧 token 失效，全员重登。
5. **iam V003 种子 × 首用户注册冲突**（上游缺陷，待反馈）：V003 播种 `tenant_admin` 角色（tenant_id 默认 1），
   首用户注册又无条件 INSERT 同码角色 → 唯一键冲突 500。**绕过：删种子行**
   `DELETE FROM iam_role_permissions WHERE role_id='role-tenant-admin-seed'; DELETE FROM iam_roles WHERE id='role-tenant-admin-seed';`
6. **iam-services SDK fetch 未绑定**（上游缺陷，待反馈）：`HttpCore` 默认捕获未 bind 的原生 fetch，浏览器端
   `Illegal invocation`。下游已在 `frontend/src/shell/api/iam.ts` 注入 `fetch: fetch.bind(globalThis)`；
   上游修复（`options.fetch ?? fetch.bind(globalThis)`）随 SDK 扩展协议反馈 lmms。
7. **dev PG 迁移残留**：`_sqlx_migrations` 若记录了已删除的迁移版本 → `VersionMissing(N)`。清法：
   `DELETE FROM _sqlx_migrations WHERE version=N;`（本次清的是 version=5 "auth local"）。
8. **8848 端口占用**：并行会话可能有旧 vite 实例占 8848 → 新实例落到 8849。浏览器验证前先看 vite 启动日志的实际端口。
9. **菜单为空属预期**：`/v1/pure/get-async-routes` 返回 `{code:0,data:[]}`（prue 库无菜单数据，播种属 lmms
   demo-data 范畴）。静态路由（dashboard/profile/workspace）兜底可达。

## 六、上游缺陷反馈（SDK 扩展协议）——2026-10-31 已上游落地（lmms 本地 commit，沿例未 push）

- [x] iam-service：V003 种子与首用户注册的唯一键冲突 → `c59891c2`（查存在优先复用 + 权限绑定幂等补齐；镜像重建后生效）
- [x] iam-services SDK：HttpCore fetch 未 bind → `ec305975`（六 SDK 统一 `fetch.bind(globalThis)`；**npm 发新版后 ai-pm 可摘除 iam.ts 的本地 bind 注入**）
- [x] api-gateway compose：gateway 缺 token-service healthy 依赖（JWKS 竞态）→ `2df3d618`
- [ ] （非缺陷）token-service 密钥持久化挂载——上游注释表明不挂载是有意设计（下游经 JWKS 动态取钥），维持现状；重启全员重登属已知代价

## 七、Slice A 联调（/v1/pm/* 业务端点，2026-10-31）

后端已实现 `/v1/pm/*`（六列表 + revisions 列表/创建/定稿 + tasks 创建/编辑 + 审批决策），契约源
`frontend/src/api/pm.ts` + `mock/fixtures.ts`；测试 19 例（tests/pm_lists.rs + pm_writes.rs）。

1. **演示种子**（幂等，可重复执行）：
   ```bash
   docker exec -i aipm-pg-dev psql -U aipm -d aipm < scripts/seed-demo.sql
   ```
   内容：AIDEMO 项目 + 3 principals + 3 需求（含定稿链）+ 6 任务 + 1 条 **pending 审批**（完整链
   event→run→proposal→approval，同意会真实执行 `update_task_status` 提案命令）+ 审计行。
2. **验收路径**：8849 登录 → 经典页六页签看真数据 → 任务页新建/编辑（乐观锁）→ 需求页修订/定稿 →
   审批页对 pending 单同意/驳回 → 审计页出现 `task.create` / `approval.decide` / `requirement.approve_revision`。
3. **昵称显示**：user-directory 已切 `/v1/pm/principals`（iam 用户与 principals 是两个 ID 空间，
   V1 约定写操作 actor = 项目 owner human principal；JWT sub↔principal 映射表留后续里程碑）。
4. **审计实时流（SSE，2026-10-31 已实现）**：`GET /v1/pm/events/stream?access_token=<jwt>`，帧名 `event: audit`，
   数据为 `AuditLog` JSON。链路：`governance.audit_log INSERT` → 触发器 `pg_notify('pm_audit')`（迁移 0005）
   → pm-api 专用 `PgListener` → 进程内 `broadcast` → SSE。业务写路径零改动，直接 INSERT 也会推送。
   EventSource 带不了 Authorization，query token **仅对本路径**放行。
   边界：V1 全租户全推（audit_log 无 tenant 列，单租户前提）；多实例各自 LISTEN；慢客户端可能丢帧
   （broadcast 容量 256，`Lagged` 跳过，流不断）。
   ```bash
   # 终端 A：订阅（token 从 gateway 登录获取）
   curl -N "http://127.0.0.1:8090/v1/pm/events/stream?access_token=<jwt>"
   # 终端 B：触发一次写操作，A 侧应出现 event: audit（action=task.create）
   curl -X POST http://127.0.0.1:8090/v1/pm/tasks \
     -H "Authorization: Bearer <jwt>" -H "Content-Type: application/json" \
     -d '{"project_id":"<uuid>","title":"sse smoke"}'
   ```
   decide 的 approve 执行期冲突（目标 revision 漂移）返回 409 `command execution failed`。

## 八、真后端模式的动态菜单（prue_menu_configs）

`VITE_USE_MOCK=0` 时左导航来自 prue_backend 的激活菜单版本（`prue_menu_configs`，mysql superpowers 库）。
无激活行时所有动态页（pm 经典页 + 系统页）不可达（静态 dashboard/profile/workspace 除外）。

播种方式（对齐 `frontend/src/api/mock/fixtures.ts` 的 `mockAsyncRoutes`，含系统页全量树）：

```bash
# 1) 从 dev server 抽取权威树（浏览器 console 或 evaluate）：
#    (await import("/src/api/mock/fixtures.ts")).mockAsyncRoutes
# 2) 写入激活版本（utf8mb4 必须，否则中文乱码）：
docker exec -i infrastructure-mysql-1 sh -c 'mysql -uroot -p"$MYSQL_ROOT_PASSWORD" \
  --default-character-set=utf8mb4 superpowers' <<'SQL'
INSERT INTO prue_menu_configs (id, version, name, routes_json, status, is_active, created_by, published_at)
VALUES ('menu-seed-0001', 1, '联调初始菜单', '<mockAsyncRoutes 的 JSON>', 'published', 1, 'seed', now())
ON DUPLICATE KEY UPDATE routes_json=VALUES(routes_json), is_active=1, status='published', published_at=now();
SQL
```

正式维护走批三交付的「菜单配置」页（版本创建/草稿编辑/发布）。重新登录后生效（guard 拉取一次）。
## 九、MCP 端点权限（2026-10-31）

`/mcp` 在 bearer 层内再加一道权限门：claims 的 `permissions` 必须含 `PM_MCP_READ`（lmms
`contracts/permissions.yaml` 定义），否则 **403 `IAM_PERM_3001`**；无 token / 伪造 token 仍是
401 `IAM_AUTH_1004/1005`。守卫只作用于 `/mcp`，其余 `/v1/pm/*` 路由不受影响。

```bash
# 浏览器登录 token 只有 IAM_SYSTEM_READ → 403
curl -i -X POST http://127.0.0.1:8090/mcp \
  -H "Authorization: Bearer <jwt>" -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"gate","version":"1"}}}'
```

按 tool 细分（JSON-RPC body 解析）：
- `tools/call` → `pm_submit_technical_design` 额外要求 `PM_MCP_DOCUMENT_WRITE`，缺权限 403 且 message 指名该权限；
- `pm_get_task_context` / `pm_get_submission_status` 只需 `PM_MCP_READ`；
- `PM_MCP_TASK_UPDATE` 暂无对应工具，等任务更新工具落地后接入映射。
- 只解析 POST（GET 是 SSE 传输）；请求体上限 256 KiB，超限 413 `IAM_REQUEST_TOO_LARGE`。
MCP 客户端请求必须带 `Host` 头（rmcp `allowed_hosts` 校验），否则 400 `missing Host header`。
## 十、知识空间 V1（Markdown + Git）

正文权威在**独立 Git 知识仓库**，PostgreSQL 只存身份/路径/版本/Commit/哈希（`ai.project_document`
+ `ai.document_revision`，迁移 0006）。写入只经应用服务（质量门与红线落地前不开放 REST 写）。

- 根目录：`AI_PM_KNOWLEDGE_ROOT`；未设置时默认 `<cwd>/../ai-pm-knowledge`（与 ai-pm 平级）。
  仓库必须**独立**（不能是 ai-pm 的子目录），首次使用自动 `git init` + 铺 `templates/`，
  并设置 `core.autocrlf=false`（保证 Windows 下哈希与文件一致）。
- 目录约定：`projects/<project-code>/<doc_type>/<doc_key>.md`；`doc_type` ∈
  `requirement | decision | architecture | knowledge`；`doc_key` 仅 `[A-Za-z0-9._-]`。
- 读取：
  ```bash
  curl "http://127.0.0.1:8090/v1/pm/documents?project_id=<uuid>" -H "Authorization: Bearer <jwt>"
  curl "http://127.0.0.1:8090/v1/pm/documents/<document-id>?project_id=<uuid>" -H "Authorization: Bearer <jwt>"
  ```
  详情返回 `markdown` 正文 + `git_commit_sha` + `content_hash`；文件被手改导致哈希不符时返回
  409 `KNOWLEDGE_DOCUMENT_STALE`（不会静默返回旧内容）。
- 前端：`/workspace/:projectId/knowledge`；真后端模式下页面内选择项目后展示文档树与正文，
  mock 模式继续使用 fixtures。
- 写入（MCP 工具 `pm_submit_document`）：参数 `project_id / doc_key / doc_type / markdown /
  change_summary?`；服务层写文件 → `git commit` → 两表落库，返回 `document_id / revision_no /
  git_commit_sha / content_hash / canonical_path`。调用者需带 `PM_MCP_DOCUMENT_WRITE`（jwks 模式
  由 `/mcp` 中间件强制；**off 模式无权限门，仅限本地开发**）。实际 `created_by` 记为项目的
  `ai_pm` principal；同 `doc_key` 重复创建返回 conflict 且不产生新 commit。
  ```bash
  # 本地 off 模式实例（8091）验证写入；文档随后由 8090 正常读取
  curl -X POST :8091/mcp -H "Host: 127.0.0.1:8091" -H "Content-Type: application/json" \
    -H "Accept: application/json, text/event-stream" --data-binary @mcp-doc.json
  # mcp-doc.json = {"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"pm_submit_document",
  #   "arguments":{"project_id":"<uuid>","doc_key":"ARCH-002","doc_type":"knowledge","markdown":"# ..."}}}
  git -C "$AI_PM_KNOWLEDGE_ROOT" log --oneline -1
  ```
  注意：含中文的 body 必须走 UTF-8 文件（`--data-binary @file`），直接 `-d` 会因终端编码报
  `invalid unicode code point`；项目必须已有 `ai_pm` principal（`bootstrap_project` 会建）。
