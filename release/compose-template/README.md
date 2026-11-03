# compose 模板包（上游发布产物）

本目录存放 **lmms 上游发布的 compose 基座模板包**，按版本 + 方言两个维度并存：

| 目录 | 方言 | 说明 |
|---|---|---|
| `coder-compose-template-1.6.0/` | MySQL（默认） | `postgres` 不存在的形态：mysql 为默认库，含 `mysql-replica` |
| `coder-compose-template-1.6.0-pg/` | PostgreSQL | `postgres` 提为默认库、`mysql` 降为 `profiles: ["mysql"]`、移除 `mysql-replica` |

来源：`low-memory-micro-services` 的 `release/compose-template/`（由
`coder release compose-template [--dialect pg]` 生成，见上游
`docs/superpowers/plans/2026-11-02-postgresql-service-layer-support.md`）。

## 与本仓 `infrastructure/` 的关系

`infrastructure/` 是**本仓已适配落地的版本**（从模板包复制后按 ai-pm 实际
服务裁剪：`docker-compose-services.yml` 末尾并入本仓自有的 6 个服务
（aipm-pg / pm-api / pm-worker / ai-pm-worker / aipm-bff / aipm-web）+ `aipm_pg_data` 卷，
并把 lmms 段的 `build:` 块替换为已发布镜像引用；另保留本仓的 `nginx/`、`README.md`）。
模板包在此作为**可追溯的
上游快照**，用于：

- 升级时比对上游变更（模板包 → 重新适配到 `infrastructure/`）；
- 确认镜像 tag / 方言语义与上游发布产物一致。

## 重新同步

```bash
# 上游生成（上游仓内）
node packages/cli/bin/coder.js release compose-template --version <ver>
node packages/cli/bin/coder.js release compose-template --version <ver> --dialect pg

# 本仓更新：复制模板包 → 按本仓裁剪 → 校验 compose 合法
docker compose --env-file .env -f infrastructure/docker-compose.yml config > /dev/null
```

⚠️ 模板包内**不含**任何凭据文件（打包器排除 `login.json` / `*.pem` / `*.key` /
`secrets*` 等，见上游 `packages/cli/lib/compose-template.js`）。本仓的凭据一律
走 `.env` / `.env.prod`（均在 `.gitignore` 内）。
