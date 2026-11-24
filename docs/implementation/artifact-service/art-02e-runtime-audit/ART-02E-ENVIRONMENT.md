# ART-02E 环境核验记录（ENVIRONMENT）

- 任务：ART-02E — Artifact Service 目标环境运行态核验
- 性质：**只读审计**（仅执行 SELECT / SHOW / `docker inspect` / 文件读取；无任何数据库写操作，无部署/配置变更，无服务重启）
- 执行时间：2026-11-24 06:57–07:04 UTC（本地 +08:00 14:57–15:04）
- 基线：文档分支 `docs/artifact-service-baseline-v1` @ `fca8e279`（PR #6）

## 1. 环境类型判定

| 项 | 结果 | 判定 |
|---|---|---|
| 环境类型 | **development（本机 full-dev 全栈部署）** | VERIFIED |
| 生产实例 | 本机 Docker 内未发现任何生产 PostgreSQL 实例 | VERIFIED |
| 结论适用范围 | **仅适用于本 full-dev 环境，不得外推为生产状态** | 强制 |

## 2. 部署目录与 Compose 项目

| 项 | 值 |
|---|---|
| 部署目录 | `C:\projects\ai-pm\infrastructure\full-dev` |
| Compose 项目名 | `superpowers-full`（容器 label 实证） |
| 生效配置文件 | `docker-compose.yml` + `docker-compose-services.prod.yml`（label `com.docker.compose.project.config_files`） |
| 运行态 | 核验时 24 个容器全部 Up；postgres healthy |

关键容器（与本次审计相关）：

| 容器 | 镜像 | 端口 | 状态 |
|---|---|---|---|
| `superpowers-full-pm-api-1` | `superpowers-full-pm-api`（compose 本地构建） | 8090 | Up（2026-11-24T05:33:40Z 起） |
| `superpowers-full-ai-pm-worker-1` | `superpowers-full-ai-pm-worker`（本地构建） | 无 | Up |
| `superpowers-full-ai-frontend-web-1` | `superpowers-full-ai-frontend-web`（本地构建） | 8092 | Up（2026-11-24T06:20:12Z 起） |
| `superpowers-full-postgres-1` | postgres:16（自建 registry 镜像） | 无宿主映射 | Up (healthy) |

其余：micro-services 1.8.1-pg 系列（gateway/token/iam/audit/notification/file/prue-admin）、consul/redis/rabbitmq/rustfs、observability 栈（prometheus/grafana/jaeger/otel/alertmanager）。均为运行态，未做任何变更。

## 3. 本机 PostgreSQL 实例全量识别（任务 §三：多实例先分别识别）

| 容器 | 宿主端口 | 状态 | 归属判定 | 本次是否连接 |
|---|---|---|---|---|
| `superpowers-full-postgres-1` | 无 | Up (healthy) | **full-dev 部署库 = 审计目标** | 是（只读 docker exec） |
| `aipm-pg-gate` | 16433 | Up | 开发/测试库（sqlx 测试专用） | 否 |
| `aipm-teamqa-pg` | 16499 | Up | Team QA 库 | 否 |
| `aipm-dev-postgres` | — | Exited (22h) | 开发库（旧实例） | 否 |
| `ci-repair-postgres` | 5432 | Exited (44h) | CI 修复用 | 否 |

选择依据：任务目标是「当前部署」，即 full-dev Compose 项目内的 `superpowers-full-postgres-1`，与 ART-02 报告 §13 记录的 2026-11-15 历史快照容器同名同标识（同一环境的后继状态）。其余实例均为开发/测试用途，一律未连接。本机不存在生产实例，无「误选生产库」风险。

## 4. 数据库身份（实时查询，VERIFIED）

| 项 | 值 |
|---|---|
| current_database | `postgres` |
| current_user | `postgres` |
| PostgreSQL 版本 | PostgreSQL 16.16 (Debian 16.16-1.pgdg13+1), x86_64 |
| 数据库时区 | Etc/UTC |
| viz schema owner | postgres |
| `gen_random_uuid()` 依赖 | pgcrypto 1.3 已安装 |

库名/用户名为非敏感标识（与 ART-02 报告已公开的历史快照一致）；本文件不含数据库口令、JWT、API Key 或任何连接字符串凭据。

## 5. AI-PM 部署版本

| 项 | 值 | 证据 |
|---|---|---|
| pm-api / worker 镜像 | `superpowers-full-pm-api:latest`、`superpowers-full-ai-pm-worker:latest`（compose 本地构建，无 registry tag、无 OCI revision label） | `docker image inspect` |
| 镜像构建时间 | 2026-11-24T02:43:56Z（两镜像同秒构建） | image inspect |
| pm-api 容器启动 | 2026-11-24T05:33:40Z | container inspect |
| **源码状态（推断）** | **≈ `remediation/integration` @ `9a1adb3d`（2026-11-24T02:38:42Z）** | 时间线：构建时间落于 `9a1adb3d`（02:38:42Z）之后、`30888bc7`（03:53:28Z）之前 |
| 断言口径 | **BEST-EFFORT**：镜像不含 git 元数据，以构建时间线 + 迁移文件存在性交叉推断 | 见下 |

支撑证据：

1. `9a1adb3d` 已包含 `migrations/0059_discovery_round_error.sql`（`git cat-file -e` 验证），与库内 max migration version = 59 吻合；
2. `git diff 9a1adb3d..a8d00cf7 -- migrations/` 为空 —— 构建之后的所有提交未改任何迁移；
3. 部署源工作区（`C:\projects\ai-pm`，remediation/integration @ `a8d00cf7`）的 4 个未提交修改全部位于 `ai-frontend/`，不影响后端与迁移。

## 6. 与 PR #6 所依据 master 基线的一致性

| 项 | 值 |
|---|---|
| 文档分支与部署仓库 HEAD 的 merge-base | `3a50c7cd`（v1.0 业务验收报告提交，= 当前 master HEAD） |
| 部署提交 `9a1adb3d` 相对基线 | 领先 2 个提交：`ab0e84e5`（merge: R3 catalog UI + readiness probe）、`9a1adb3d`（merge: R4 design/ADR UI）——均为 UI 合并，**不触碰 migrations/** |
| ART-02E 核验范围内一致性 | **一致**：`migrations/0020_visualization_artifact.sql` 在 `fca8e279` / `3a50c7cd` / `9a1adb3d` / `a8d00cf7` 四处逐字节相同（diff 验证） |

结论：当前部署与 PR #6 所依据的 master 基线在 ART-02E 核验范围（viz schema 与迁移链）内**完全一致**；整体代码部署略领先基线 2 个 UI 提交。
