# ai-pm

每项目专属 AI-PM 平台（Rust + PostgreSQL）：Cargo Workspace，含 5 个核心库 crate（crates/）与 3 个服务（apps/）。

启动开发数据库：`docker compose -f docker-compose.dev.yml up -d`（postgres:16，`postgres://aipm:aipm@localhost:5433/aipm`）。

> 该 compose 已为本地开发/测试关闭 WAL 同步（`fsync=off` / `synchronous_commit=off` /
> `full_page_writes=off`，并调大 `max_connections` / `shared_buffers`）。`#[sqlx::test]`
> 每个用例都会重建库并重跑 `migrations/` 全部迁移，这些同步开销是测试墙钟的大头；
> 实测迁移应用从 ~1.1s/次 降到 ~0.35s/次（纯迁移段约 8x）。**仅适用于本地库，不得用于生产。**
> 修改 compose 后需 `docker compose -f docker-compose.dev.yml up -d` 重建容器才生效（数据保留）。

运行测试：`DATABASE_URL=postgres://aipm:aipm@localhost:5433/aipm cargo test`（推荐 `bash scripts/cargo-test.sh`，走 nextest 全局并行）。

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
