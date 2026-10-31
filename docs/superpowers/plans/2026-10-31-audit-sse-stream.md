# 审计实时流（SSE）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 让 `GET /v1/pm/events/stream` 真正可用：审计行落库后通过 SSE 推给前端，overview 的「实时动态」面板在真后端模式下自动显示，不再降级隐藏。

**Architecture:** **PG LISTEN/NOTIFY 单点广播**。`governance.audit_log` 新增 AFTER INSERT 触发器 `pg_notify('pm_audit', payload)`；pm-api 启动一条专用 `PgListener` 连接，解析 payload 后送进进程内 `tokio::sync::broadcast`；SSE handler 用 `BroadcastStream` 逐条下发 `event: audit`。业务写路径（pm_write / command_executor / MCP / 脚本 / 并发会话）**零改动**，全部自动覆盖；多实例时每个实例各自 LISTEN，天然一致。

**Tech Stack:** Rust（axum 0.8 `response::sse`、sqlx 0.8 `PgListener`、tokio broadcast / tokio-stream）+ Vue 3 EventSource（前端契约已存在，不改）。

**Spec/契约来源:** `frontend/src/api/events.ts`（`event: audit` + `AuditLog` JSON + `?access_token=`）、`frontend/src/api/types.ts::AuditLog`、`docs/runbook-login-chain.md §七.4`（当前明确标注未实现）。

## Global Constraints

- 一律绝对路径；每任务结束即 commit（并行会话对策），**只落本地，不 push**。
- 迁移 forward-only：新增 `migrations/0005_audit_notify.sql`，不改 0001-0004。
- 不新增 npm 依赖；Rust 新增 `tokio-stream`（SSE 流适配），workspace 统一版本。
- SSE 端点只允许 **query token**（`?access_token=`），其余路由仍只认 `Authorization`，避免 token 进日志的面扩大。
- 测试命令：`cargo test -p pm-api -p pm-application`（带 `CARGO_HTTP_PROXY="" NO_PROXY=192.168.2.12`）；前端 `pnpm test:unit`。
- 不推历史审计（前端已有 REST 列表），SSE 只推订阅之后的新增行。

## Review Focus

1. **业务层零侵入**：`append_audit` / `command_executor` 的 INSERT 都不改，触发器兜底 → 用「直接 INSERT 审计行」的集成测试证明也能收到事件。
2. **断线可恢复**：PgListener 断开（PG 重启 / 网络抖动）后按 1s→30s 退避重连，重连后重新 LISTEN；期间 SSE 连接保持，前端 EventSource 自动重连。
3. **慢客户端不拖垮服务**：broadcast 容量 256，`Lagged` 只跳过并记 warn，不 panic、不断流。
4. **payload 上限**：pg_notify 单条 ≤8000 字节；只带 7 个契约字段（不含 before/after JSONB），符合上限。
5. **认证边界**：无 token / 伪造 token / 缺 tid 一律 401 `IAM_AUTH_1004/1005`，与现有中间件一致；query token 仅对 `/v1/pm/events/stream` 生效。

## File Structure

```
ai-pm/migrations/0005_audit_notify.sql        # 触发器函数 + AFTER INSERT trigger（新增）
ai-pm/apps/pm-api/src/audit_stream.rs        # AuditLog 序列化、broadcast feed、PgListener 重连循环、SSE handler（新增）
ai-pm/apps/pm-api/src/auth.rs                # query token 提取（仅 SSE 路径）
ai-pm/apps/pm-api/src/routes.rs              # AppState 增 audit_tx；注册 GET /v1/pm/events/stream
ai-pm/apps/pm-api/src/main.rs                # 启动 listener 任务，注入 feed
ai-pm/apps/pm-api/Cargo.toml                 # + tokio-stream
ai-pm/Cargo.toml                             # workspace + tokio-stream
ai-pm/apps/pm-api/tests/audit_stream.rs      # SSE 集成测试（新增）
ai-pm/docs/runbook-login-chain.md            # §七.4 边界改为已实现 + 验收命令
```

## Tasks

### Task 1 — 迁移：审计行落库即 NOTIFY

- [x] 写 `migrations/0005_audit_notify.sql`：`governance.notify_audit_insert()` 用 `json_build_object` 拼 `{id, project_id, actor_id, action, target_type, target_id, created_at}`（`created_at` 统一 UTC ISO8601），`AFTER INSERT ... FOR EACH ROW` 执行 `pg_notify('pm_audit', ...)`。
- [x] `cargo run -p pm-api` 起一次让 `sqlx::migrate!` 应用；验证：`SELECT tgname FROM pg_trigger WHERE tgname='audit_log_notify';` 有 1 行。
- [x] 手工验证：`INSERT INTO governance.audit_log ...` 后另开 `LISTEN pm_audit;` 会话能收到 payload。
- [x] Commit：`feat(pm-api): notify pm_audit on audit insert`

### Task 2 — audit_stream 模块（纯逻辑可测）

- [x] `AuditLog` serde 结构（snake_case，字段与前端 `types.ts` 一一对应；`target_id` Option、`created_at` 保留原始 ISO 字符串）。
- [x] `AuditFeed { tx: broadcast::Sender<AuditLog> }`：`new()`、`subscribe()`、`publish()`；容量 256。
- [x] `run_listener(pool, feed)`：`PgListener::connect_with(&pool)` → `listen("pm_audit")` → `recv()` 循环；单条解析失败只 warn 丢弃；连接错误按指数退避重连并重新 LISTEN。
- [x] 单测：payload→AuditLog 解析（含 target_id 为 null）、坏 payload 不中断、broadcast lagged 不 panic。
- [x] Commit：`feat(pm-api): audit feed with pg listener`

### Task 3 — SSE 端点与认证

- [x] `routes.rs`：`AppState` 增加 `audit_tx`；注册 `.route("/v1/pm/events/stream", get(audit_stream::stream_handler))`。
- [x] handler：`Sse::new(BroadcastStream)`，每条 `Event::default().event("audit").json_data(log)`；`KeepAlive::new().interval(15s).text("keepalive")`；`Lagged` 跳过。
- [x] `auth.rs`：`bearer_token` 先读 header，header 缺失且 `request.uri().path() == "/v1/pm/events/stream"` 时读 `access_token` query；其余情况保持 401。
- [x] `main.rs`：`router_with_auth` 前构造 `AuditFeed` 并 `tokio::spawn(run_listener(pool.clone(), feed.clone()))`，把 feed 注入 router（新增 `router_with_auth_and_feed`，旧 `router` / `router_with_auth` 内部用默认 feed，测试不受影响）。
- [x] Commit：`feat(pm-api): GET /v1/pm/events/stream (sse)`

### Task 4 — 集成测试

- [x] `tests/audit_stream.rs`：起测试 PG（沿用现有 tests 的池初始化方式）→ 真实 socket 起 router → `reqwest` 流式读取；用 `Authorization` 与 `?access_token=` 两种方式各断言 200 + `content-type: text/event-stream`。
- [x] 用例：无 token 401；伪造 token 401；有效 token 后直接 INSERT 一条审计 → 3s 内收到 `event: audit`，JSON 字段齐全；连续插 2 条按序到达。
- [x] 用例：同一 token 断开重连后仍能收到新事件（EventSource 语义模拟：关闭响应再开一条）。
- [x] `cargo test -p pm-api --test audit_stream` 全绿。
- [x] Commit：`test(pm-api): sse audit stream contract`

### Task 5 — 前端契约对齐（预计零改动，仅校验）

- [x] 校验 `events.ts` 的 `event: audit` 与后端 `Event::event("audit")` 一致、JSON 可被 `AuditLog` 直接消费。
- [x] 若后端首帧为 keepalive comment，确认 EventSource 不触发 `onerror`；若触发，改为 `KeepAlive` 空注释格式（不新增业务事件）。
- [x] `pnpm test:unit` 全绿（现有 419 例）。
- [x] Commit（若有改动）：`fix(frontend): align sse keepalive`

### Task 6 — 端到端实测

- [x] 起栈（runbook §二），浏览器真实登录 8848。
- [x] `curl -N "http://127.0.0.1:8090/v1/pm/events/stream?access_token=<jwt>"` 保持连接；另开终端调用任务创建或审批接口。
- [x] 断言：overview「实时动态」出现 `task.create` / `approval.decide`，面板不再隐藏；关闭一个事件后新事件仍继续。
- [x] Commit：`docs(runbook): audit sse verified`

### Task 7 — 文档收尾

- [x] 更新 `docs/runbook-login-chain.md §七.4`：删除「未实现」，补 SSE 验收命令与 curl 示例。
- [x] 记录边界：V1 全租户全推（audit_log 无 tenant 列，单租户前提）；多实例各自 LISTEN；慢客户端可丢事件。
- [x] Commit：`docs(runbook): audit stream usage`

## Done When

- 直接 INSERT 审计行也能被 SSE 推出（证明不依赖业务层改造）。
- `cargo test -p pm-api -p pm-application` 与 `pnpm test:unit` 全绿。
- 浏览器 overview 实时面板在真后端模式下出现新事件且持续接收。
- 全部 commit 落本地 master，未 push（push 由用户单独裁决）。