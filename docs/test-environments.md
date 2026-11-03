# ai-pm 测试环境说明

> 信息来源：用户提供（2026-11-03）。内网测试环境，对外不可访问，凭据可直接入档。
> 两台机器对应 Codex Bootstrap V1.3 的「两机拓扑」（`docs/ai-pm-codex-bootstrap-v1.md` 裁清点 1）：服务器栈 + Windows PC。

## 机器清单

| # | 角色 | 地址 | 凭据 | 现状 | 对应拓扑侧 |
|---|------|------|------|------|-----------|
| 1 | Linux 测试服务器 | `192.168.2.10`（SSH 22） | `kudi` / `Ewq321#@!` | Docker 29.7.2 + Compose v5.5.0；PM-AI 栈已部署（见下） | 服务器栈：pm-api + PG + `/mcp` |
| 2 | Windows 测试服务器 | `192.168.2.43`，远程端口 `60209` | `kudi` / `123456` | 已装 Z-Code；**从 192.168.2.4 侧除 ICMP 外全部端口被过滤** | Windows PC：Codex / Z-Code 执行机 |

> 补充事实：本开发机（跑 Z-Code 的这台）地址是 **192.168.2.4**，不是 .43。

## 访问方式

- **Linux**：`ssh kudi@192.168.2.10`，密码 `Ewq321#@!`
- **Windows**：远程桌面连 `192.168.2.43:60209`（端口为用户给定，非默认 3389），账号 `kudi` / `123456`
  - ⚠️ 2026-11-03 实测：从 192.168.2.4 探测 192.168.2.43 的 22/3389/5985/5986/445/8090/3000 **全部被过滤**（ICMP 通）。
    需要在该机上放开入站防火墙（或放行本机来源 IP）后才能远程操作。

## 用途约定

- 两台机器均**可用于测试**。
- Linux 机跑 PM-AI 服务器栈（compose / `start-prod` 部署路径），承接 T-081 `/mcp` 跨机暴露、T-088 服务端验收。
- Windows 机作为真实 Windows 发起侧（T-081 七项身份链路门禁要求真实 Windows 发起；T-090 Skill + 自检闭环、T-091 两机实测在这台做）。

## 安全口径

用户明确：**不需要关心安全问题**——两台机器对外均无法访问（纯内网），凭据明文入档即可，无需走密钥/保险箱流程。

---

# Linux 机上的 PM-AI 服务器栈（2026-11-03 已部署）

## 部署形态

- 代码位置：`~/ai-pm`（由 Windows 开发机 tar 传输；该机无 GitLab SSH key，暂不能 `git pull`）
- compose 项目名：`infrastructure`（与 lmms 栈同命名空间，网络 `superpowers` 服务名互访）
- 叠加的文件：`docker-compose.yml`（include 基础设施/服务/可观测性）+ `docker-compose.prod.yml` + **`docker-compose.test-server.yml`**（测试机专用：把 `pm-api` 的 8090 与 `token-service` 的 8082 发布到宿主）
- 启动了 12 个服务：`postgres consul registrator redis rabbitmq token-service iam-service api-gateway aipm-pg pm-api pm-worker ai-pm-worker`
  （`aipm-bff` / `aipm-web` 未启动：镜像依赖已不在本地的 ai-pm-web 仓，无法构建）

## 端点

| 端点 | 地址 | 说明 |
|------|------|------|
| MCP | `http://192.168.2.10:8090/mcp` | Codex / Z-Code 的 Remote MCP 入口（Host 白名单见下） |
| pm-api REST | `http://192.168.2.10:8090/v1/pm/*` | 业务 API |
| 接入指引 | `http://192.168.2.10:8090/install`、`/install/codex` 等 | 免鉴权；按 `AI_PM_PUBLIC_BASE_URL` 渲染执行端接入指引（slug 见索引页） |
| 健康探针 | `http://192.168.2.10:8090/health` | 免鉴权 |
| 登录验证 | `http://192.168.2.10:3000/v1/iam/auth/login` | api-gateway（账号密码登录入口） |
| 首用户注册 | `http://192.168.2.10:3000/v1/iam/auth/register` | 仅当库里无用户时开放（首用户通道） |
| JWKS | `http://192.168.2.10:8082/.well-known/jwks.json` | token-service 公钥 |
| ai-pm 库 | `postgres://aipm:aipm@192.168.2.10:5435/aipm` | aipm-pg 容器宿主映射 5435 |

## 账号

| 账号 | 口令 | 取得方式 |
|------|------|----------|
| `admin` | `Admin@123456` | `POST /v1/iam/auth/register` 首用户通道创建（IAM 侧角色 `tenant_admin`） |

JWT claims 实测：`tid=1`、`roles=["tenant_admin"]`、`ct=false`、`permissions=["IAM_SYSTEM_READ","PM_MCP_READ","PM_MCP_DOCUMENT_WRITE"]`。

## 运维脚本

| 脚本 | 作用 |
|------|------|
| `infrastructure/scripts/setup-test-server.sh` | 首次初始化：生成 `.env.prod`（随机口令，幂等）+ 校验 `DATABASE_URL` 展开 |
| `infrastructure/scripts/seed-iam-platform-admin.sql` | IAM 侧：补 `PM_MCP_*` 权限码并绑给 `tenant_admin` |
| `scripts/seed-demo.sql` | ai-pm 侧演示数据（1 客户 / 1 项目 / 3 principal / 3 需求 / 6 任务 / 1 待审批 + 登录主体补为项目与租户 owner） |
| `infrastructure/docker-compose.test-server.yml` | 测试机 overlay：发布 `pm-api:8090`、`token-service:8082` |

常用命令（在 `~/ai-pm/infrastructure`）：

```bash
# 起栈（带测试机 overlay）
COMPOSE_OVERLAY=docker-compose.test-server.yml bash scripts/start-prod.sh \
  postgres consul registrator redis rabbitmq token-service iam-service api-gateway \
  aipm-pg pm-api pm-worker ai-pm-worker

# 灌种子
docker exec -i infrastructure-aipm-pg-1 psql -U aipm -d aipm < ~/ai-pm/scripts/seed-demo.sql
docker exec -i infrastructure-postgres-1 psql -U postgres -d coder \
  < ~/ai-pm/infrastructure/scripts/seed-iam-platform-admin.sql
```

> ⚠️ **裸跑 `docker compose ...` 会报 required variable 错**：test-server overlay 用
> `:?` 强制要求 `AI_PM_MCP_ALLOWED_HOSTS` / `AI_PM_PUBLIC_BASE_URL`，而 compose CLI
> 只自动读项目目录 `.env`（不含 prod 覆盖）。绕过方式二选一：
> ① 一律走 `start-prod.sh`（内部 `set -a` 导出两个 env 文件）；
> ② 手动导出后再跑：`set -a && source .env && source .env.prod && set +a && docker compose ...`

## `.env.prod` 关键项（该机实测值）

```bash
POSTGRES_PASSWORD=coder              # 与已初始化的 postgres 数据目录一致（改它会口令失配）
RABBITMQ_DEFAULT_PASS=guest          # 与基础 .env / rabbitmq.conf 一致
AI_PM_FIRST_ADMIN_TENANT=1
AI_PM_FIRST_ADMIN_SUBJECT=xOSuhTRzs-3yVuT7Gx2r0   # 注册出的 admin 的 sub
AI_PM_MCP_ALLOWED_HOSTS=192.168.2.10  # T-081：/mcp 的 Host 白名单（rmcp DNS-rebinding 防护）
AI_PM_PUBLIC_BASE_URL=http://192.168.2.10:8090   # /install 接入指引渲染的平台对外地址
```

## 已验证的完整链路（2026-11-03 实测）

1. `POST /v1/iam/auth/login` → 拿到 JWT（claims 含 `PM_MCP_READ`）
2. `POST /mcp`（JSON-RPC `initialize`）→ HTTP 200，`serverInfo = ai-pm-mcp v0.1.0`
3. `POST /mcp`（`tools/list`）→ **18 个工具**（`pm_list_tasks` / `pm_submit_document` / `pm_propose_document` / …）
4. `POST /mcp`（`tools/call pm_list_tasks`）→ 正常返回 `{"tasks":[],"total":0}`（种子任务状态非 ready）
5. `GET /v1/pm/me` → `is_active=true, is_admin=true, tenant_role=owner, auth_mode=interactive`
6. `GET /v1/pm/tasks?project_id=…` → 返回种子 6 条任务
7. `GET /install`、`/install/codex` → 接入指引正确渲染 `http://192.168.2.10:8090` 地址与 Codex TOML 配置片段

## 踩过的坑（复现要点）

1. **`ct=true` 与 `platform_admin` 冲突**：lmms 把「持有 `platform_admin` 角色」直接翻译成 JWT 的 `ct=true`（跨租户），而 ai-pm V1 边界是「只收租户内令牌」→ `ct=true` 一律 403 `IAM_PERM_3004`。
   **ai-pm 侧的全平台管理员 = `tenant_admin` + 权限目录全码 + 库内 `tenant_member.role='owner'`**，不要绑 lmms 的 `platform_admin`。
2. **`.env.prod` 不导出 = prod 覆盖失效**：`start-prod.sh` 用 `source` 加载 env，shell 变量未导出，compose 插值只看得到基础 `.env` → `${DATABASE_URL}` 静默回落。已改为 `set -a` 导出。
   **副作用**：任何服务重建都必须整栈一起重建，否则新旧容器拿到不同的 bootstrap token → 服务间握手失败（表现为登录 401）。
3. **compose 版本门禁**旧正则只认 `v2.2x~v2.99`，会误杀该机的 Compose v5.5.0 —— 已改为语义化比较（`compose_version_ok`）。
4. **`aipm-pg` 的镜像名是硬编码 `postgres:16`**，而基础设施侧走 `${BASE_REGISTRY}postgres:16`。只传后者会在该机上触发去 Docker Hub 拉取（该机不通外网）→ 需额外 `docker tag` 一份 `postgres:16`。
5. **rmcp 的 Host 白名单**（T-081）：默认只放回环，跨机访问需 `AI_PM_MCP_ALLOWED_HOSTS`；该 env 由 `apps/pm-api/src/mcp_hosts.rs` 读取，但**要进 compose 才生效**（已在 `docker-compose-services.yml` 透传）。
6. **token-service 重启会换签名密钥**（密钥存容器内文件，无卷）→ 旧 JWT 立即失效；pm-api 的 JWKS 缓存会自动刷新，但客户端要重新登录。
7. **该机拉不了 Docker Hub**（USTC 镜像源已失效，直连超时）；但内网 zhcoder registry 可达（401=需鉴权）。
   本次采用「Windows 本地 build/save → scp → Linux `docker load`」绕开 registry 凭据问题。
