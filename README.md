# ai-pm

每项目专属 AI-PM 平台（Rust + PostgreSQL）：Cargo Workspace，含 5 个核心库 crate（crates/）与 3 个服务（apps/）。

启动开发数据库：`docker compose -f docker-compose.dev.yml up -d`（postgres:16，`postgres://aipm:aipm@localhost:5433/aipm`）。

运行测试：`DATABASE_URL=postgres://aipm:aipm@localhost:5433/aipm cargo test`。

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

## 壳认证与前端业务端点（shell_routes）

同一服务还挂载前端壳与业务端点（契约见 `frontend/src/api/mock/fixtures.ts`，路由聚合在 `src/shell_routes.rs`，由 main.rs merge）：

- 壳认证：`POST /v1/iam/auth/{login,refresh,logout}`、`GET /v1/iam/auth/setup-status`——migration 0005 本地会话表实现，任意口令可登录（IAM 上游接管前的过渡）
- DB 菜单：`GET /v1/pure/get-async-routes`（静态树，菜单表落地后改查库）
- 用户目录：`GET /v1/iam/users/simple`
- 业务读端点：`GET /v1/pm/{principals,projects,requirements,tasks,approvals,audit-logs}`（`project_id` 过滤 + page/size 分页，需 Bearer 会话）
- 审批决策：`POST /v1/pm/approvals/{id}/decide`（`{decision: approved|rejected, reason}` → `{id, status, decided_at}`；事务落 approval_request + 审计，驳回另记 human_intervention(veto) 并置提案 vetoed；重复决策 409）

前端联调：`cd frontend && pnpm dev`（`.env.development` 置 `VITE_USE_MOCK = 0`，vite 代理 `/api → 127.0.0.1:8080`）。演示数据：`docker exec -i aipm-pg-dev psql -U aipm -d aipm < scripts/seed-demo.sql`（固定 UUID 幂等）。
