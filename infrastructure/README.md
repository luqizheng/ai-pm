# ai-pm 编排（infrastructure/）

基础模板来自 lmms **coder-compose-template 1.6.0-pg**（PostgreSQL 方言，2026-11-03 发布；
`TEMPLATE_INFO.json` 溯源）。目录内容 = 模板 + ai-pm 适配；模板不带文档，本文档归 ai-pm 所有。
上游模板包的可追溯快照见本仓 `release/compose-template/coder-compose-template-1.6.0-pg/`。

## 编排结构（与上游同构：infra + services + observability）

```
docker-compose.yml              入口 include 三块
├─ docker-compose-infra.yml      基础设施：consul / registrator / redis / rabbitmq
│                                + postgres（默认库）/ mysql（profiles:["mysql"]）
├─ docker-compose-services.yml   业务服务（单文件，两段）：
│                                ① lmms 登录验证栈 + 其余业务服务（已发布镜像引用）
│                                   api-gateway:3000 / iam-service:8081→9081 / token-service:8082
│                                   config / file / notification / audit / prue + 2 outbox publisher
│                                ② ai-pm 自有 6 服务（aipm-pg / pm-api / pm-worker /
│                                   ai-pm-worker / aipm-bff / aipm-web）
└─ docker-compose-observability.yml  prometheus / grafana / jaeger / otel-collector
                                     / alertmanager / consul-template
```

**登录验证全在 `docker-compose-services.yml`**：浏览器 → `api-gateway:3000`（`/gw`）→
`iam-service` → `token-service:8082` 签发 RS256 JWT；`pm-api` 用
`AI_PM_JWKS_URL=http://token-service:8082/.well-known/jwks.json` 本地验签（同一 `superpowers`
网络，服务名即 DNS）。

## PG 方言语义

- `postgres` 是默认库（无 profile，直接起）；`mysql` 降为 `profiles: ["mysql"]`，需显式
  `--profile mysql`；`mysql-replica` 已随方言移除（模板不含）。
- 两套库互不干扰：模板共享 `postgres`（coder 库，lmms 服务用）+ ai-pm 自有 `aipm-pg`
  （aipm 库，pm-api / pm-worker / ai-pm-worker / aipm-bff 用）。

## 与上游模板包的有意差异（升级模板后需重放）

| # | 文件 | 差异 | 原因 |
|---|------|------|------|
| 1 | `docker-compose-services.yml` | lmms 段 `build:` 块 → 已发布镜像引用（`${kudi_repos:-…}/<svc>:${IMAGE_TAG:-1.6.0-pg}`；api-gateway 无 `-pg` 后缀） | ai-pm 不构建 lmms 服务，直接消费 registry 镜像（对齐 lmms `transform-compose.js --dialect pg`） |
| 2 | `docker-compose-services.yml` | 末尾追加 ai-pm 自有 6 服务 + `aipm_pg_data` 卷 | 本仓自有；**单文件服务定义**（原 `docker-compose-aipm.yml` 已并入并删除） |
| 3 | `docker-compose.prod.yml` | lmms 业务段收口 + ai-pm 段（pm-api / aipm-bff 收口，aipm-web:8091 保留） | prod 入口：前端 aipm-web:8091 + 登录验证 api-gateway:3000 |
| 4 | `config/nginx/nginx.conf.template` | `proxy_pass` 指向 `pm-api:8090`（原 api-gateway:3000）；canary 上游清单置空 | edge 后端是 pm-api；lmms 服务的 canary 分流属 lmms 侧 |
| 5 | `scripts/start-prod.sh` | ① 语义化 compose 版本门禁（上游正则误杀 v5.x、误放行 v2.10.3）② 必需变量 MYSQL_* → POSTGRES_* | 本机/WSL compose 是 v5.6.0；PG 形态默认库是 postgres |
| 6 | `.env.prod.example` | `POSTGRES_*` 块上移到 `DATABASE_URL` 之前（修 source 展开顺序：原文件会展开成 `postgres://://`）；删 MySQL 块 | PG 形态；`source` 是顺序求值 |
| 7 | `services/token-service/config/config.yaml`（仓根新建） | 上游 lmms 仓的 token-service 配置副本 | compose 的 `../services/token-service/config:/app/config:ro` 是 bind mount——ai-pm 仓无该目录时 Docker 会建**空目录**遮住镜像内 baked config，服务启动即 fail（`token-service config missing required fields`）。上游无此问题（它在 lmms 仓内） |
| 8 | `README.md` | 本文档 | 模板不带文档 |

另：`scripts/deploy-remote.sh` / `staging-*.sh` / `sync-staging-images.sh` 保持上游原样
（lmms 滚动重启流程，ai-pm 未用，仅参考）。

## 起服

```bash
# 0. 准备变量（复制 .env.prod.example 或从 lmms 的 infrastructure/.env 基线改）
cp infrastructure/.env.prod.example infrastructure/.env   # 按需改值

# 1. 一次起全栈（基础设施 + 登录验证栈 + ai-pm 服务 + 可观测性）
docker compose -f infrastructure/docker-compose.yml up -d

# 只起基础设施 + 登录验证栈 + aipm-pg（aipm 服务镜像需先构建）
docker compose -f infrastructure/docker-compose.yml up -d \
  consul registrator redis rabbitmq postgres token-service iam-service api-gateway aipm-pg

# 2. pm-api（开发态：宿主 cargo run；生产态：构建镜像 aipm/pm-api:0.1.0 后起容器）
DATABASE_URL="postgres://aipm:aipm@127.0.0.1:5435/aipm" PM_API_PORT=8090 \
AI_PM_AUTH_MODE=jwks AI_PM_JWKS_URL="http://127.0.0.1:8082/.well-known/jwks.json" \
AI_PM_FIRST_ADMIN_TENANT=1 AI_PM_FIRST_ADMIN_SUBJECT=<IAM 首用户 id> \
cargo run -p pm-api
#   首启自动跑 migrations（28 条）；AI_PM_FIRST_ADMIN_* 让首个登录主体
#   落库即 active + tenant owner（缺这两个 env → 403 IAM_PERM_3002 未激活）

# 3. 配置 IAM 首用户（首次部署）—— 演示账号 admin / Admin@123456
curl -s http://localhost:3000/v1/iam/auth/setup-status
curl -s -X POST http://localhost:3000/v1/iam/auth/register -H 'content-type: application/json' \
  -d '{"username":"admin","password":"Admin@123456","nickname":"Admin"}'
#   返回里的 id 即 JWT sub → pm-api 的 AI_PM_FIRST_ADMIN_SUBJECT 用它

# 4. demo 数据（tenant-aware，幂等）
docker exec -i infrastructure-aipm-pg-1 psql -U aipm -d aipm < scripts/seed-demo.sql

# 5. prod（base + prod overlay）
cp infrastructure/.env.prod.example infrastructure/.env.prod  # 填 POSTGRES_* + *_DEV_BT
./infrastructure/scripts/start-prod.sh
```

> 登录验证栈现在**在 ai-pm 栈内**（services.yml），不要再去 lmms 仓起第二份——同 compose 项目
> 同名服务会互抢。联调步骤见 `docs/runbook-login-chain.md` §二。

## 端口约定（避免与既有容器冲突）

| 端口 | 占用 | prod overlay |
|------|------|--------------|
| **3000** | **api-gateway（登录验证入口 /gw）** | **保留** |
| 3001 | grafana | 保留（ops 直连） |
| 4317 / 4318 / 8889 | otel-collector | 保留（ops） |
| 5433 | aipm-pg-dev（独立开发容器，历史） | 不在本编排 |
| 5434 | coder-test-pg（共享测试容器，常开） | 不在本编排 |
| **5435** | **aipm-pg（本编排）** | 保留（ops 连库） |
| 8081→9081 | iam-service | 收口 |
| 8082 | token-service | 收口 |
| 8083 / 8084 / 8085 / 8086 / 8087 | notification / audit / config / file / prue_backend | 收口 |
| 8080 | aipm-bff（ai-pm-web 仓的 Rust BFF，共享 aipm 库） | 收口 |
| 8090 | pm-api | 收口（走 aipm-web:8091） |
| **8091** | **aipm-web（静态站，/api 反代 aipm-bff）** | **保留（前端公共入口）** |
| 8500 / 8502 | consul | 收口 |
| 9090 / 9093 | prometheus / alertmanager | 保留（ops） |
| 16686 / 14250 / 14268 | jaeger | 保留（ops） |

## 重新同步上游模板

```bash
# 1. lmms 仓生成（版本取最新 tag；双方言各一包）
cd ../low-memory-micro-services
node packages/cli/bin/coder.js release compose-template --version <ver> --dialect pg

# 2. 快照收录到本仓 release/compose-template/coder-compose-template-<ver>-pg/，

# 3. 复制覆盖本目录，按上方「有意差异」清单重放 ai-pm 适配，

# 4. 校验编排合法：
docker compose -f infrastructure/docker-compose.yml config > /dev/null
docker compose -f infrastructure/docker-compose.yml -f infrastructure/docker-compose.prod.yml config > /dev/null
```

⚠️ 模板包内不含任何凭据文件（打包器排除 `login.json` / `*.pem` / `*.key` / `secrets*`）。
本仓凭据一律走 `.env` / `.env.prod`（均在 `.gitignore` 内）。
