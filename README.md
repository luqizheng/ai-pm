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
