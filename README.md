# ai-pm

每项目专属 AI-PM 平台（Rust + PostgreSQL）：Cargo Workspace，含 5 个核心库 crate（crates/）与 3 个服务（apps/）。

启动开发数据库：`docker compose -f docker-compose.dev.yml up -d`（postgres:16，`postgres://aipm:aipm@localhost:5433/aipm`）。

运行测试：`DATABASE_URL=postgres://aipm:aipm@localhost:5433/aipm cargo test`。
