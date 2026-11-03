# Runbook：内网测试服务器部署 ai-pm 栈（两机拓扑·服务器栈侧）

> 状态：**已按本 runbook 完整走通**（2026-11-03，机器 192.168.2.10，端到端实测全绿）。
> 机器清单与现状见 [test-environments.md](test-environments.md)；本文是**从零部署**的操作手册。
> 对应拓扑：Codex Bootstrap V1.3 裁清点 1 —— 本文档部署「服务器栈」（pm-api + PG + `/mcp`），
> Windows 执行机侧接入见 `/install/<tool>` 指引与 `ai-pm-executor-onboarding-runbook-v1.md`。

---

## 〇. 目标形态

| 项 | 值 |
|----|----|
| 部署机 | 内网 Linux（示例 192.168.2.10），Docker ≥ 29 + Compose v5.x，docker 免 sudo |
| 代码落位 | `~/ai-pm`（tar 传输；机器无 GitLab SSH key 时走这条路） |
| compose 项目 | `infrastructure`（与 lmms 栈同命名空间，网络 `superpowers` 服务名互访） |
| 服务集 | 12 个：postgres / consul / registrator / redis / rabbitmq / token-service / iam-service / api-gateway / aipm-pg / pm-api / pm-worker / ai-pm-worker（aipm-bff、aipm-web 需 ai-pm-web 仓镜像，暂不部署） |
| 对外端口 | `8090`（pm-api：MCP + REST + /install）、`3000`（api-gateway：登录/注册）、`8082`（token-service JWKS）、`5435`（aipm-pg 宿主映射） |
| 账号原则 | **空库起步；首个注册用户即全平台管理员**；口令/bootstrap token 由脚本自动生成 |

资源参考：4 核 / 3.8G 内存 / 87G 磁盘即可（实测栈起后内存占用 ~1.4G）。

---

## 一. 前置条件核查（部署机）

```bash
docker --version && docker compose version   # ≥ 29.x / v5.x（旧版见坑 3）
docker ps                                    # 免 sudo 可用
curl -sm 5 -o /dev/null -w '%{http_code}\n' http://192.168.2.12:16444/api/v1/crates/   # kellnr（构建期才需要，部署机可不通）
ping -c1 192.168.2.16                        # 内网 GitLab 可达（本次未用 clone，走 tar）
```

> ⚠️ 部署机大概率**拉不了 Docker Hub**（国内镜像源时效不稳定）——本 runbook 假设镜像全部从开发机传输。

---

## 二. 开发机：构建镜像

在 ai-pm 仓**主 checkout** 根目录（构建上下文 = 仓根，`sqlx::migrate!` 需要整体可见）：

```bash
cd <ai-pm 仓根>
# ⚠️ 构建前确认 HEAD 含全部预期功能（踩过：镜像早于 T-081 提交 → Host 白名单功能缺失）
git log --oneline -3

docker build --target runtime-pm-api       -t aipm/pm-api:0.1.0 .
docker build --target runtime-pm-worker    -t aipm/pm-worker:0.1.0 .
docker build --target runtime-ai-pm-worker -t aipm/ai-pm-worker:0.1.0 .
```

依赖说明（Dockerfile 已内置对策，勿动 `.dockerignore` / `COPY` 清单）：
- `.cargo/`（kellnr + rsproxy 配置）必须进构建上下文，否则容器内 cargo 报 `registry index was not found: kellnr`；
- `skills/` 必须进（`apps/pm-api/build.rs` 打包白名单 zip，缺目录直接 panic）；
- 容器内构建须可达 kellnr `192.168.2.12:16444`（匿名读）与 rsproxy；
- `rust:1-slim`（trixie/GLIBC 2.41）与 runtime `debian:trixie-slim` 同大版本——已配好，不要改回 bookworm。

## 三. 开发机：打包传输

### 3.1 镜像（11 个，gzip 后 ~400MB）

```bash
IMGS="zhcoder-docker-registry.com:8000/official/postgres:16 \
zhcoder-docker-registry.com:8000/official/hashicorp/consul:2.0.1 \
zhcoder-docker-registry.com:8000/official/gliderlabs/registrator:master \
zhcoder-docker-registry.com:8000/official/redis:7-alpine \
zhcoder-docker-registry.com:8000/official/rabbitmq:3.13-management-alpine \
zhcoder-docker-registry.com:8000/micro-server/token-service:1.6.0-pg \
zhcoder-docker-registry.com:8000/micro-server/iam-service:1.6.0-pg \
zhcoder-docker-registry.com:8000/micro-server/api-gateway:1.6.0 \
aipm/pm-api:0.1.0 aipm/pm-worker:0.1.0 aipm/ai-pm-worker:0.1.0"

# 缺哪个先在开发机补齐（可观测性镜像按需另拉后重打 zhcoder tag）
for i in $IMGS; do docker image inspect "$i" >/dev/null 2>&1 || echo "MISS $i"; done

docker save $IMGS | gzip -1 > /tmp/aipm-images.tar.gz
scp /tmp/aipm-images.tar.gz kudi@192.168.2.10:/tmp/
ssh kudi@192.168.2.10 'gunzip -c /tmp/aipm-images.tar.gz | docker load'

# ⚠️ aipm-pg 的镜像名硬编码 postgres:16（不走 ${BASE_REGISTRY}），必须补这个 tag，
#    否则 compose up 会去 Docker Hub 拉取 → 部署机不通外网直接挂起
ssh kudi@192.168.2.10 'docker tag zhcoder-docker-registry.com:8000/official/postgres:16 postgres:16'
```

### 3.2 源码（~5.5MB）

```bash
cd <ai-pm 仓根>
tar czf /tmp/aipm-src.tar.gz \
  --exclude=./target --exclude=./.target-clippy --exclude=./frontend/node_modules \
  --exclude=./logs --exclude=./.git --exclude=./.arts .
scp /tmp/aipm-src.tar.gz kudi@192.168.2.10:/tmp/
ssh kudi@192.168.2.10 'mkdir -p ~/ai-pm && tar xzf /tmp/aipm-src.tar.gz -C ~/ai-pm'
```

### 3.3 SSH 自动化（Git Bash 无 sshpass 时的 askpass 配方）

```bash
mkdir -p /tmp/aipm-ssh
printf '#!/bin/sh\necho %s\n' "'<部署机密码>'" > /tmp/aipm-ssh/askpass.sh && chmod +x /tmp/aipm-ssh/askpass.sh
export SSH_ASKPASS=/tmp/aipm-ssh/askpass.sh SSH_ASKPASS_REQUIRE=force DISPLAY=:0
SSHOPT="-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null"
ssh $SSHOPT kudi@192.168.2.10 'echo CONNECTED'
# 之后 ssh/scp 都带 $SSHOPT
```

---

## 四. 部署机：初始化与首次起栈

> **顺序敏感**：`setup-test-server.sh` 必须在**首次起栈之前**跑 —— postgres 数据卷按首次
> 起栈时插值出的 `POSTGRES_PASSWORD` 初始化，**之后改口令不会再生效**（卷已存在）。
> 前提：`start-prod.sh` 含 `set -a` 导出修复（2026-11-03 起），否则 prod 覆盖对 compose
> 插值不可见，卷会按基础 `.env` 的值初始化（踩过，见坑 2）。

```bash
cd ~/ai-pm/infrastructure
chmod +x scripts/*.sh
bash scripts/setup-test-server.sh        # 生成 .env.prod：口令随机、幂等、校验 DATABASE_URL 展开
```

首次起栈：

```bash
cd ~/ai-pm/infrastructure
COMPOSE_OVERLAY=docker-compose.test-server.yml bash scripts/start-prod.sh \
  postgres consul registrator redis rabbitmq \
  token-service iam-service api-gateway \
  aipm-pg pm-api pm-worker ai-pm-worker
```

- `COMPOSE_OVERLAY` 是 `start-prod.sh` 的可选第三叠加层（base → prod → overlay）；
  test-server overlay 会把 `pm-api:8090`、`token-service:8082` 发布到宿主，并对
  `AI_PM_MCP_ALLOWED_HOSTS` / `AI_PM_PUBLIC_BASE_URL` 做必填门禁。
- ⚠️ **整栈同批重建原则**：改过任何中间件类 env（口令/token）后，必须把**整栈一起重建**，
  只重建单服务会造成新旧容器 bootstrap token 不一致 → 服务间握手失败 → 登录 401。

---

## 五. 账号引导：首个注册用户 = 全平台管理员

### 5.1 注册首用户（IAM 首用户通道，仅库中无用户时开放）

```bash
# 确认 setup_required
curl -s http://127.0.0.1:3000/v1/iam/auth/setup-status
# → {"setup_required":true,"user_count":0}

# 注册（响应里的 "id" 就是 sub，记下来）
curl -s -X POST http://127.0.0.1:3000/v1/iam/auth/register \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"Admin@123456","nickname":"管理员","email":"admin@example.com"}'
```

### 5.2 IAM 授权：权限码 + 角色修正

```bash
cd ~/ai-pm
docker exec -i infrastructure-postgres-1 psql -U postgres -d coder \
  < infrastructure/scripts/seed-iam-platform-admin.sql
```

脚本做三件事（幂等）：
1. 补 `PM_MCP_READ` / `PM_MCP_DOCUMENT_WRITE` 两个权限码进 `iam_permissions`；
2. 权限目录全部 enabled 权限 → 绑给 `tenant_admin`（注册通道默认角色）；
3. 解除 admin 的 `platform_admin` 绑定。

> ⚠️ **为什么不是 lmms 的 `platform_admin`**：lmms iam-service 把「持有 platform_admin」
> 直接翻译成 JWT `ct=true`（跨租户令牌），而 ai-pm V1 边界是「只收租户内令牌」→
> `ct=true` 一律 403 `IAM_PERM_3004`。ai-pm 侧的全平台管理员 =
> **`tenant_admin` + 全权限码 + ai-pm 库内 owner（下两步）**。

### 5.3 首管理员对 + Host 白名单写入 `.env.prod`，重建 pm-api

```bash
cd ~/ai-pm/infrastructure
SUB=<5.1 记下的 id>
cat >> .env.prod <<EOF

# 首管理员对：命中的登录主体首次接入即 active + 库内 owner
AI_PM_FIRST_ADMIN_TENANT=1
AI_PM_FIRST_ADMIN_SUBJECT=$SUB
# T-081：/mcp 跨机 Host 白名单（rmcp DNS-rebinding 防护，默认只放回环）
AI_PM_MCP_ALLOWED_HOSTS=<部署机 IP>
# /install 接入指引渲染的平台对外地址
AI_PM_PUBLIC_BASE_URL=http://<部署机 IP>:8090
EOF

# 若注册前已有请求触发过「自动开户」（inactive principal 行），清掉它
# （已存在 identity 的分支不会重新激活，必须删行让它走 first-admin 通道重新开户）
docker exec infrastructure-aipm-pg-1 psql -U aipm -d aipm -c "
  DELETE FROM core.project_member WHERE principal_id IN (SELECT principal_id FROM core.principal_identity);
  DELETE FROM core.tenant_member  WHERE principal_id IN (SELECT principal_id FROM core.principal_identity);
  DELETE FROM core.principal_identity;
  DELETE FROM core.principal p WHERE NOT EXISTS (SELECT 1 FROM core.principal_identity pi WHERE pi.principal_id=p.id) AND p.kind='human';"

# 只重建 pm-api（FIRST_ADMIN/HOSTS 只是 pm-api 的 env；没动中间件 env 就不用整栈）
COMPOSE_OVERLAY=docker-compose.test-server.yml bash scripts/start-prod.sh pm-api
```

### 5.4 ai-pm 业务种子（演示数据）

```bash
cd ~/ai-pm
docker exec -i infrastructure-aipm-pg-1 psql -U aipm -d aipm < scripts/seed-demo.sql
```

种子是幂等的：1 客户 + 1 DEMO 项目 + 3 个演示 principal + 3 需求（含 approved 定稿链）
+ 6 任务 + 1 pending 审批 + 当前登录主体自动补为项目 owner 与租户 owner（2c/2d 段）。

> 注意：种子任务大多不在 ready 状态，`pm_list_tasks` 返回 `{"tasks":[],"total":0}` 是
> 正常现象（T-082 只列 ready 任务）；REST `/v1/pm/tasks` 能看到全部 6 条。

---

## 六. 验证清单（全部应通过）

```bash
BASE=http://127.0.0.1   # 远程验证时换成部署机 IP
TOKEN=$(curl -s -X POST $BASE:3000/v1/iam/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"Admin@123456"}' \
  | sed -n 's/.*"access_token":"\([^"]*\)".*/\1/p')
echo "1 登录: $([ -n "$TOKEN" ] && echo OK || echo FAIL)"

# 1b JWT claims 抽查：应含 PM_MCP_READ / PM_MCP_DOCUMENT_WRITE、roles=[tenant_admin]、ct=false
echo "$TOKEN" | cut -d. -f2 | tr '_-' '/+' | base64 -d 2>/dev/null | head -c 400; echo

echo "2 health: $(curl -s -o /dev/null -w '%{http_code}' $BASE:8090/health)"   # 200
echo "3 initialize: $(curl -s -X POST $BASE:8090/mcp \
  -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"smoke","version":"0.1"}}}' \
  | grep -o '"serverInfo":{[^}]*}')"                                           # ai-pm-mcp v0.1.0
echo "4 tools/list: $(curl -s -X POST $BASE:8090/mcp -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  | grep -o '"name":"pm_[a-z_]*"' | wc -l)"                                    # 18
echo "5 /v1/pm/me: $(curl -s $BASE:8090/v1/pm/me -H "Authorization: Bearer $TOKEN")"
# → is_active=true, is_admin=true, tenant_role=owner, auth_mode=interactive
echo "6 /v1/pm/tasks: $(curl -s "$BASE:8090/v1/pm/tasks?project_id=22222222-2222-2222-2222-222222222222" \
  -H "Authorization: Bearer $TOKEN" | head -c 80)"                             # items[...]
echo "7 /install: $(curl -s $BASE:8090/install | head -1)"                     # AI-PM 执行端接入工具清单
```

跨机验证：从另一台内网机器把 `BASE` 换成 `http://<部署机 IP>` 重复上述步骤
（这一步就是 T-081「跨机暴露」的服务端侧验收；执行端侧七项门禁见 T-081 实施记录）。

---

## 七. 日常运维

```bash
cd ~/ai-pm/infrastructure

# 状态（两种姿势；裸 compose 会撞 :? 必填门禁，需先导出 env）
COMPOSE_OVERLAY=docker-compose.test-server.yml bash scripts/start-prod.sh --help
set -a && source .env && source .env.prod && set +a
docker compose -f docker-compose.yml -f docker-compose.prod.yml -f docker-compose.test-server.yml ps

# 日志
docker logs infrastructure-pm-api-1 --tail 100
docker logs infrastructure-api-gateway-1 --tail 50

# 停栈 / 起栈（stop-prod 保留数据卷）
bash scripts/stop-prod.sh                                  # down，卷保留
COMPOSE_OVERLAY=docker-compose.test-server.yml bash scripts/start-prod.sh \
  postgres consul registrator redis rabbitmq token-service iam-service api-gateway \
  aipm-pg pm-api pm-worker ai-pm-worker

# 代码更新（开发机改完后重传源码 + 重建三镜像 + load + 整栈重建）
# ⚠️ token-service 容器重建会重新生成 JWT 密钥（密钥存容器内文件，无卷）→ 所有旧 JWT 失效，需重新登录
```

---

## 八. 故障排查（实测踩坑索引）

| # | 症状 | 根因 | 处置 |
|---|------|------|------|
| 1 | MCP/REST 403 `IAM_PERM_3004 cross-tenant` | admin 绑了 lmms `platform_admin` ⇒ JWT `ct=true`，ai-pm 拒收跨租户令牌 | 跑 `seed-iam-platform-admin.sql`（含解除绑定）；管理员=tenant_admin+全权限码+库内 owner |
| 2 | 改了 `.env.prod` 口令但容器行为不变 / 新旧容器互踢 | `start-prod.sh` 老版本 `source` 不导出，compose 插值看不到 prod 覆盖 | 确认脚本含 `set -a` 修复；任何中间件 env 变更**整栈同批重建** |
| 3 | `start-prod.sh` 报 compose 版本不达标（实际是 v5.x） | 旧正则只认 v2.2x~v2.99 | 确认脚本含 `compose_version_ok` 语义化比较 |
| 4 | `compose up` 卡在拉 `postgres:16`（连 Docker Hub） | `aipm-pg` 镜像名硬编码，没打本地 tag | `docker tag zhcoder…/official/postgres:16 postgres:16`（§3.1 末尾） |
| 5 | MCP 403 `Forbidden: Host header is not allowed` | rmcp DNS-rebinding 白名单只放回环 | `.env.prod` 设 `AI_PM_MCP_ALLOWED_HOSTS=<部署机IP>` + 重建 pm-api；日志 `docker logs pm-api | grep rebind` 可见实际 Host |
| 6 | 登录突然 401（之前好的） | token-service 容器重建 ⇒ JWT 密钥重新生成，旧 token 全失效 | 重新登录即恢复；服务间握手失败则整栈同批重建 |
| 7 | MCP 403 `IAM_PERM_3002 principal is not activated` | 身份已按「非首管理员」自动开户为 inactive；已存在行不会重新激活 | §5.3 的清行 SQL + 确认 `AI_PM_FIRST_ADMIN_*` 已进容器 env（`docker exec pm-api env | grep FIRST`）后重建 |
| 8 | MCP 403 `IAM_PERM_3001 permission denied: PM_MCP_READ` | JWT permissions 缺权限码 | 确认 `seed-iam-platform-admin.sql` 已跑；重新登录拿新 token（权限在签发时进 claims） |
| 9 | MCP 401 `invalid token`（pm-api 侧） | pm-api JWKS 缓存未刷新（token-service 刚换过密钥） | pm-api JWKS 自动刷新（`digua_common_jwt::cache` 日志有 initial refresh）；稍候重试或重建 pm-api |
| 10 | 裸跑 `docker compose …` 报 required variable | test-server overlay 用 `:?` 门禁两个变量，compose CLI 只读项目目录 `.env` | 用 start-prod.sh，或 `set -a && source .env && source .env.prod && set +a` 后再跑 |

---

## 九. 相关文档

- [test-environments.md](test-environments.md) —— 机器清单、凭据、当前部署状态、端点速查
- [runbook-login-chain.md](runbook-login-chain.md) —— 登录链路（api-gateway → iam → token → JWKS）原理与排障
- [ai-pm-executor-onboarding-runbook-v1.md](ai-pm-executor-onboarding-runbook-v1.md) —— 执行端（Codex/Z-Code）接入
- [ai-pm-codex-bootstrap-v1.md](ai-pm-codex-bootstrap-v1.md) —— 两机拓扑设计基线（T-088/T-081/T-090/T-091）
- [ai-pm-principal-identity-v1.md](ai-pm-principal-identity-v1.md) —— 身份模型与鉴权矩阵（IAM_PERM_300x 语义）
