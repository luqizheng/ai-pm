# S02 QA · 发布验收测试计划 V1.0

> **依据**：《S02 Release Acceptance Test — QA Master Prompt V1.0》（用户 2026-11-06 下发）。
> **性质**：独立验收测试，不采信开发团队口头结论；全部判定以实际代码、数据库、API、Git 文件与执行日志为证。
> **记录约定**：用例文件即执行记录（`test-case/README.md`），每条用例执行后回填「实际结果/判定」；未执行的不得标 PASS，无法执行的标 BLOCKED 并写明原因。

---

## 1. 被测对象与版本

| 项 | 值 | 来源 |
|---|---|---|
| 代码基线 | master @ `75dc14d`（git log 确认） | 本仓 |
| S02 修复线 | S02-I1（需求核心统一服务）/ I2（Markdown+Git 投影）/ I6-A（Approver 显式授权，迁移 0040）/ I6-B（执行器 `ApproveRequirementRevision` 退役）——实现代码在 HEAD 工作区已核验存在 | `crates/pm-application/src/requirement.rs`、`apps/pm-api/src/pm_requirements.rs`、`crates/pm-application/src/command_executor/executor.rs:805` |
| 数据库迁移 | 0001–0042 全部应用（本地栈实测 `_sqlx_migrations` 顶部 42） | docker exec psql |
| 本地运行栈 | `aipm/pm-api:0.1.0`（镜像创建 2026-11-06T05:29Z），`AI_PM_AUTH_MODE=jwks`，DB=容器内 `aipm-pg:5432/aipm` | docker inspect |
| 历史核验基线 | S02-02 运行时核验报告 V1.0（2026-11-04）：DEF-01 FAIL / DEF-02 P0 FAIL / DEF-03 FAIL——本次全部强制重测 | `docs/scenarios/s02/s02-02-requirement-runtime-verification-v1.md` |

## 2. 测试环境

| 项 | 值 |
|---|---|
| 本机 Docker 栈 | `infrastructure-pm-api-1`（8090 仅容器网络）、`infrastructure-aipm-pg-1`（宿主 16433）、`infrastructure-api-gateway-1`（宿主 3000，IAM 登录） |
| 数据库直查 | `docker exec infrastructure-aipm-pg-1 psql -U aipm -d aipm`（宿主等价 `postgres://aipm:aipm@127.0.0.1:16433/aipm`） |
| 集成测试库 | `cargo test` + `sqlx::test`：每用例自建独立临时库，自毁，不触碰正式数据 |
| 知识库根 | 集成测试每请求注入独立临时目录（`AI_PM_KNOWLEDGE_ROOT`，git init 自动化）；本地栈知识库根在执行时实测定位 |
| 正式数据保护 | 本地库现存 `delivery.requirement` 5 行 / `requirement_revision` 9 行 / `requirement_confirmation` 5 行为演示种子数据（`scripts/seed-demo.sql`），**测试一律新建独立测试项目，禁止修改正式种子数据** |

## 3. 执行载体（三层证据）

1. **代码级（axum router oneshot + 独立临时库）**：既有回归面 `pm_requirement.rs`（524 行）、`requirement_projection.rs`、`pm_approvers.rs`、`documents.rs`、`knowledge_index_v2.rs`、`tenant_isolation.rs`、`visibility.rs`、`discovery.rs`、`command_executor.rs`——作为批次主证据之一，全部实跑。
2. **REST 网络面（真实 HTTP + jwks 认证链）**：本地栈 pm-api 容器内 curl，IAM `api-gateway:3000` 登录取 JWT；覆盖认证/错误码/HTTP 语义与网络层行为。
3. **存储直查**：psql 直查 PostgreSQL 终态 + git 直查知识库 commit/文件——验证跨存储一致性，不以 HTTP 200 为唯一证据。

## 4. 批次与用例映射

| 批次 | 用户任务书 | 用例文件章节 | 附加范围 |
|---|---|---|---|
| Batch 01 | S02-TEST-01 Requirement Core Functional Test | §B01 | — |
| Batch 02 | S02-TEST-02 Revision Lifecycle Test | §B02 | — |
| Batch 03 | S02-TEST-03 Markdown Git Authority Test | §B03 | — |
| Batch 04 | S02-TEST-04 Authorization and Security Test | §B04 | DEF-01、DEF-02 专项（§DF1/§DF2） |
| Batch 05 | S02-TEST-05 Concurrency and Data Integrity Test | §B05 | DEF-03 专项（§DF3） |
| Batch 06 | S02-TEST-06 Knowledge and PageIndex Test | §B06 | — |
| Batch 07 | （Master Prompt §4：Finding 创建/来源/确认/生命周期） | §B07 | 用户未单发任务书，按 Master Prompt 执行 |
| Batch 08 | S02-TEST-08 Cross Module Integration Test | §B08 | S01→S02 / S02→Task / S02→S03 / 端到端 |

## 5. 正式架构约束（判定基准，Master Prompt §3 摘录）

1. 正文权威 = Markdown + Git；PostgreSQL 只存 ID/状态/版本引用/审批/状态机/审计/事件，**不得成为正文第二权威**；PageIndex 为可重建派生索引，索引状态不得决定业务状态。
2. 人工审批边界：AI 可分析/建议/起草，**不得自行批准**；Revision 确认必须真人 + 正式确认记录；indexed ≠ approved；不允许经其他 API/内部服务/后台任务绕过审批。
3. 生命周期 `draft → pending_confirmation → approved → superseded`：每次正式修改创建新 Revision；历史保留；approved 不得静默修改；替代关系完整；确认记录实际审批者与时间。
4. 不建独立 Project Baseline 实体；Task 默认绑定单一执行依据版本；Finding 属 S02 管理范围。

## 6. 缺陷分级（Master Prompt §7，不因开发完成而降级）

- **P0** 严重安全 / 数据完整性 / 正式审批边界失效 —— 禁止发布
- **P1** 核心流程失败 / 重要数据不一致 / 关键功能不可用 —— 禁止发布（除非正式风险裁决）
- **P2** 非核心功能异常或可控缺陷
- **P3** UI / 文案 / 体验 / 轻微

## 7. 已知范围事实（执行前静态盘点，执行中验证）

| 事实 | 证据 | 影响 |
|---|---|---|
| S02-I3（source_input / requirement_finding 关联）未实施 | 迁移止于 0042，无该表 | B07/B08 相关项记 Not Implemented |
| S02-I4（task_requirement 改造）未实施（0039 注明归 I4） | 0039 头注 | B08 第二部分按现状测 |
| S02-I5（requirement_acceptance_criterion）/ I7 未实施 | 同上 | B08 范围注记 |
| `requirement_confirmation` 表已存在且种子库有 5 行 | 0039 + psql 实测 | B02 主验证面 |
| 执行器 `ApproveRequirementRevision` = 无条件拒绝（`refuse_requirement_confirmation`） | executor.rs:805-809 | DF2 执行器面 |
| MCP 无确认类工具（H-15） | routes.rs `.nest("/mcp", mcp_router)` + 设计稿裁决 | DF2 MCP 面 |
| 幂等键机制按裁决**不在 V1 实现**（重放一律 409，I1-D12 归 I6-B/O-5） | requirement.rs:21 | B05 幂等用例预期 = 显式 409，不视为缺陷 |