# ai-pm

每项目专属 AI-PM 平台（Rust + PostgreSQL）：Cargo Workspace，含 5 个核心库 crate（crates/）与 3 个服务（apps/）。

## 本地演示环境（账号密码）

**演示账号：`admin` / `Admin@123456`**

| 项 | 值 |
|---|---|
| 账号 | `admin` |
| 密码 | `Admin@123456` |
| 角色 | `tenant_admin`（首用户自动获得，含全部启用权限） |
| 前端入口 | http://localhost:8848 （`cd frontend && pnpm dev`） |
| 登录验证栈 | api-gateway `3000` → iam-service `9081` → token-service `8082` |
| 业务 API | pm-api `8090`（经前端 `/api` 代理；`/gw` 代理到 api-gateway） |
| 数据库 | aipm-pg `5435`（`postgres://aipm:aipm@127.0.0.1:5435/aipm`） |

> 账号由 `POST :3000/v1/iam/auth/register` 注册（首次部署，runbook 见
> `docs/runbook-login-chain.md` §三）。**仅限本地/内网演示环境**，生产必须换强口令并
> 走 secret manager。
>
> pm-api 侧该主体还需 `AI_PM_FIRST_ADMIN_TENANT=1` +
> `AI_PM_FIRST_ADMIN_SUBJECT=<注册返回的 IAM 用户 id>` 两个 env 才会落库为
> active + tenant owner（缺任一 → 403 `IAM_PERM_3002 principal is not activated`）。

演示数据（1 客户 / 1 项目 / 3 需求 / 6 任务 / 1 待审批 / 审计行，幂等）：

```bash
docker exec -i infrastructure-aipm-pg-1 psql -U aipm -d aipm < scripts/seed-demo.sql
```

启动开发数据库：`docker compose -f docker-compose.dev.yml up -d`（postgres:16，`postgres://aipm:aipm@localhost:5433/aipm`）。

> 该 compose 已为本地开发/测试关闭 WAL 同步（`fsync=off` / `synchronous_commit=off` /
> `full_page_writes=off`，并调大 `max_connections` / `shared_buffers`）。`#[sqlx::test]`
> 每个用例都会重建库并重跑 `migrations/` 全部迁移，这些同步开销是测试墙钟的大头；
> 实测迁移应用从 ~1.1s/次 降到 ~0.35s/次（纯迁移段约 8x）。**仅适用于本地库，不得用于生产。**
> 修改 compose 后需 `docker compose -f docker-compose.dev.yml up -d` 重建容器才生效（数据保留）。

运行测试：`DATABASE_URL=postgres://aipm:aipm@localhost:5433/aipm cargo test`（推荐 `bash scripts/cargo-test.sh`，走 nextest 全局并行）。

### 测试提速：模板库（`template1`）预建

`#[sqlx::test]` 每个用例都会从头跑一遍 `migrations/`（2026-11-09 起为基线单文件
`0001_baseline.sql` ≈3.6s/次；基线合并前旧链 64 文件 ≈4.06s）。sqlx 0.8 建库语句是裸
`create database "<name>"`，Postgres 默认以 `template1` 为模板，所以只要跑测试前
把迁移预跑进 `template1`（连 `_sqlx_migrations` 记账行一并写入），每个用例的建库
就退化成文件拷贝，sqlx 查到迁移已全部 applied（checksum 一致）直接跳过全部 DDL：

| 场景 | 单个 schema 用例 | 8 个 schema 用例（串行） |
|------|-----------------|-------------------------|
| 逐用例重跑迁移（旧） | 5.15s | 48.2s |
| 模板库克隆（新） | 0.33s | 3.55s |

`scripts/cargo-test.sh` 会自动执行
`cargo run -q -p pm-infrastructure --example seed_test_template`
（见 `crates/pm-infrastructure/examples/seed_test_template.rs`）。它按迁移文件
版本+checksum 记指纹：迁移没变则跳过（不付重建成本），新增/改动则自动重建，
因此不会出现 sqlx 的 "migration was previously applied but has been modified"。
直接 `cargo test` 不预建也能跑，只是回到慢路径。

> 该步骤会 DROP `template1` 里除系统 schema 外的一切对象，并改变该实例上**所有**
> 后续 `CREATE DATABASE` 的初始内容 —— **仅限本地 dev/test 库**；非 localhost 的
> `DATABASE_URL` 默认拒绝执行（`AIPM_TEST_TEMPLATE_FORCE=1` 可覆盖）。
> `AIPM_TEST_SKIP_TEMPLATE=1` 可跳过预建；`AIPM_TEST_TEMPLATE_DB=xxx` 可换模板库名。


## pm-api（REST）

启动：`DATABASE_URL=postgres://aipm:aipm@localhost:5433/aipm cargo run -p pm-api`（监听 `0.0.0.0:8090`，可用 `PM_API_PORT` 覆盖）。

当前路由 `POST /api/v1/events`（幂等入站事件，Task 9）：请求体为 `{project_id, actor_id, source, event_type, idempotency_key, payload}`，按 `(project_id, source, idempotency_key)` 幂等。

| 结果 | HTTP | body |
|------|------|------|
| 首次接收 | 202 | `{event_id, processing_status: "queued"}` |
| 同 key 同 payload 重放 | 200 | `{event_id, status, processing_status: "duplicate"}` |
| 同 key 不同 payload | 409 | `{error: "idempotency_conflict", event_id}` |
| 未知 project/actor | 404 | `{error: "unknown_project"}` |
| 数据库错误 | 500 | `{error: "internal"}` |

启动时自动执行 `migrations/`。服务层入口：`pm_application::ingestion::submit_event`（Task 16 的 MCP 工具将直接消费它）。
