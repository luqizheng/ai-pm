# ai-pm 编排（infrastructure/）

基础模板来自 lmms（`pnpm run compose:template`，`TEMPLATE_INFO.json` 溯源；模板不带文档——本文档归 ai-pm 项目所有）。目录内容 = 模板 + ai-pm 自有服务定义。

## 来源与升级

```bash
# lmms 仓发布模板（版本取最新 tag）
cd ../low-memory-micro-services && pnpm run compose:template
# 产物在 release/compose-template/coder-compose-template-<ver>/，
# 复制覆盖本目录后，按本文件重做两处 ai-pm 适配（include 列表 + aipm 服务文件）。
```

## 对模板做的适配（升级模板后需重放）

1. `docker-compose.yml` 入口：include 列表把 `docker-compose-services.yml`（lmms 自有服务，
   build 上下文指向 lmms 仓）换成 `docker-compose-aipm.yml`；lmms 服务文件保留在本目录作参考。
2. `docker-compose-aipm.yml`：ai-pm 四服务（aipm-pg / pm-api / pm-worker / ai-pm-worker），
   挂模板的 `superpowers` 网络。

## 与 lmms 栈的关系

- 目录名同为 `infrastructure` → compose 项目同名 → 同一 `infrastructure_superpowers` 网络、
  同一卷命名空间。ai-pm 服务与 lmms 服务互访直接用服务名（如 `aipm-pg:5432`、`mysql:3306`）。
- 先在 lmms/project-manager 侧起好基础设施（mysql/redis/consul/rabbitmq），再起 ai-pm：

```bash
docker compose -f infrastructure/docker-compose.yml up -d aipm-pg pm-api pm-worker ai-pm-worker aipm-bff aipm-web
curl -s -X POST http://localhost:8090/api/v1/events -H 'content-type: application/json' -d '{}' -w '\n%{http_code}\n'
```

## 端口约定（避免与既有容器冲突）

| 端口 | 占用 |
|------|------|
| 5433 | aipm-pg-dev（独立开发容器，历史） |
| 5434 | coder-test-pg（共享测试容器，常开） |
| **5435** | **aipm-pg（本编排）** |
| 8090 | pm-api |
| 8080 | aipm-bff（ai-pm-web 前端仓的 Rust BFF，共享 aipm 库） |
| 8091 | aipm-web（前端静态站，/api 反代 aipm-bff） |
