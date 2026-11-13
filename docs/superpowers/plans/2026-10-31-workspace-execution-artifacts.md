# Workspace 剩余两页接真后端（AI 执行中心 + 产物验收）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** 把 workspace 五页中仅剩的两页——`ExecutionPage.vue`（AI 执行中心）与 `ArtifactsPage.vue`（产物验收）——从 `../fixtures` 切换到真实后端数据，与已接后端的 Goals/Decisions/Knowledge/Models 页保持一致。

**Architecture:** 复用既有 `pm-api` 列表端点风格（裸 payload + `{items,total}`）与前端 `useMock` 开关模式（见 `GoalsPage.vue`）。执行数据来自 `delivery.task_run`（0010，Slice C 已开始生产写入）；产物数据来自 `delivery.requirement_revision` + `/v1/pm/documents`。**首版不新建迁移**（无现成数据源的字段显示明确空态，不造假）。

**Tech Stack:** Rust axum + sqlx · PostgreSQL · Vue 3 + vitest · i18n 双语。

**Spec:** `docs/ui/ai-pm-ui-pages-v1-2.md`（§2 AI 执行中心、§5 产物验收）；执行语义见 `docs/tasks/ai-pm-task-state-machine-v1.md` §19.5。

## Global Constraints
- **不造假**：工具调用 / 日志 / 文件 Diff / 测试报告在后端无数据源时显示**空态文案**，绝不填充虚构数据。
- **导航不动**：导航「V1.2 五项 vs 现存 9 项」未裁决，本计划不改导航结构。
- **迁移**：首版不新增；若后续为「执行步骤 / 产物附件」建表，序号 ≥ **0011**。
- **依赖方向 / 事务**：只读查询，不涉及写入事务；不改 outbox/command_executor。
- 每任务 TDD（先红后绿），逐任务 commit；完成后跑本线测试 + 前端 `test:unit`/`typecheck`/`check:i18n`。

## 数据映射

| 页面 | 区域 | 数据源 | 首版状态 |
| --- | --- | --- | --- |
| 执行中心 | Run 列表 + 状态 + 关联任务 + 时间 | `delivery.task_run` ⋈ `delivery.task`（新增端点） | **真实** |
| 执行中心 | 失败分类/原因 | `delivery.task_run_failure` | 真实（有则显示） |
| 执行中心 | 执行步骤 / 工具调用 / 日志 / 代码变化 | **无**（待 Slice D 执行器） | **空态** |
| 产物验收 | 产物清单（需求修订） | `delivery.requirement_revision`（复用 `/v1/pm/requirements/{id}/revisions` + 新增聚合列表） | **真实** |
| 产物验收 | 版本 Diff（Markdown 行级） | 前端由相邻版本正文计算 | 真实 |
| 产物验收 | 测试报告 / 文件 Diff / 行级评论 | **无** | **空态** |
| 产物验收 | 验收结论 | 需求正文「验收条件」段 + 里程碑 `sign_acceptance` | 待裁（Q1） |

## Tasks

### Task 1: 后端 `GET /v1/pm/task-runs`（执行 Run 列表）
- Files: `apps/pm-api/src/pm_runs.rs`（新建）、`apps/pm-api/src/routes.rs`（merge）、`apps/pm-api/tests/task_runs.rs`（新建）
- API: `GET /v1/pm/task-runs?project_id=&page=&size=` → `{ items: [{ id, task_id, task_code, task_title, attempt_no, status, outcome, created_at, started_at, finished_at, last_error }], total }`
- 复用 `pm_lists` 的分页/错误风格；`project_id` 必填。
- 测试：建项目 + 任务 + 两条 Run（一条 `queued`、一条 `failed` 带 failure）→ 断言列表内容与计数；空项目 → `items: []`。

### Task 2: 后端 `GET /v1/pm/artifacts`（产物列表 = 需求修订）
- Files: `apps/pm-api/src/pm_artifacts.rs`（新建）、`routes.rs`（merge）、`apps/pm-api/tests/artifacts.rs`（新建）
- API: `GET /v1/pm/artifacts?project_id=&page=&size=` → `{ items: [{ id, requirement_id, requirement_code, requirement_title, version, state, content_hash, created_at, approved_by }], total }`
- 数据源：`delivery.requirement_revision` ⋈ `delivery.requirement`，`version` 倒序。
- 测试：两条修订（draft/approved）→ 断言排序与字段。

### Task 3: 前端 API 客户端 + 类型
- Files: `frontend/src/api/execution.ts`（新建，**任务专属**，勿动共享 `types.ts`）、`frontend/src/api/artifacts.ts`（新建）
- `listTaskRuns(params)` / `listArtifacts(params)`；类型放专属文件（遵循本仓「本线类型不放共享 types.ts」约定）。
- 测试：纯函数/类型无需单测；接线由页面测试覆盖。

### Task 4: `ExecutionPage.vue` 接真后端
- Files: `frontend/src/workspace/pages/ExecutionPage.vue`（改）、`.../ExecutionPage.test.ts`（新建/更新）
- 模式同 `GoalsPage.vue`：`useMock` 开关 → mock 走 fixtures（单测基石）；真后端按 `projectId` 读 `listTaskRuns`。
- 结构：Run 列表（左）+ Run 详情（右：状态 / attempt / 时间 / 失败原因）+ 未选项目/加载失败/空态提示；「执行步骤/工具调用/日志」区改**空态**（文案：执行细节待执行器接入）。
- 测试：mock 模式渲染 fixtures 关键元素；真后端模式（mock http）渲染 Run；空态。

### Task 5: `ArtifactsPage.vue` 接真后端
- Files: `frontend/src/workspace/pages/ArtifactsPage.vue`（改）、`.../ArtifactsPage.test.ts`（新建/更新）
- 模式同上：产物清单 = 需求修订；版本 Diff 由相邻版本 Markdown 计算；测试报告/文件 Diff/评论区空态。
- 测试：mock 渲染 fixtures 核心元素；真后端渲染修订 + Diff；空态。

### Task 6: 门禁与提交
- 后端：`bash scripts/cargo-test.sh`（全量）；前端：`pnpm test:unit` / `typecheck` / `check:i18n` / 本线文件 `eslint`。
- 迁移序号确认未动（仍 0010）。按 `feat(workspace)` + `test` 议题提交。

## 待裁决
| # | 问题 | 建议 |
| --- | --- | --- |
| Q1 | 「产物」定义：以**需求修订**为准，还是纳入知识文档 / 里程碑验收？ | 首版以需求修订为主，知识文档作补充 |
| Q2 | 「验收结论」数据源：需求正文验收条件段 vs 里程碑 `sign_acceptance` | 首版只读需求正文验收段 |
| Q3 | 导航 5 项 vs 9 项 | 待用户裁决（本计划不动） |
---

## 完成证据（2026-10-31）

**后端（`pm-api`）**：
- Task 1 `581b0ff`：`GET /v1/pm/task-runs`（`delivery.task_run` ⋈ `delivery.task` + `task_run_failure`），测试 `apps/pm-api/tests/task_runs.rs`（2 条）。
- Task 2 `e59477d`：`GET /v1/pm/artifacts`（`delivery.requirement_revision` ⋈ `delivery.requirement`），测试 `apps/pm-api/tests/artifacts.rs`（2 条）。
- `pm-api` 全量：`73 tests run: 73 passed`。

**前端**：
- Task 3 `8a952ff`：`frontend/src/api/execution.ts` / `frontend/src/api/artifacts.ts`（任务专属类型，未动共享 `types.ts`）。
- Task 4 `1682abe`：`ExecutionPage.vue` 接真后端（项目选择 + Run 列表 + 失败摘要 +「执行步骤/工具调用/日志」空态），`useMock` 分支保留 fixtures。
- Task 5 `c675d0b`：`ArtifactsPage.vue` 接真后端（需求修订清单 + 元数据 + Diff/测试/评论空态），`useMock` 分支保留 fixtures。
- `style(workspace)` `d7adde6`：本线两文件 prettier 格式修正。

**门禁**：前端 `typecheck` 绿；`test:unit` `87 files / 528 passed`；`check:i18n` `722 key` 双语对齐、无新增硬编码；本线两文件 `eslint` 0 错误；后端 `pm-api` 全量 73 passed。

**偏差 / 未竟**：
- 「版本 Diff」未由相邻 Markdown 计算：`/v1/pm/artifacts` 只返回 `content_hash` 不含正文，故文件 Diff / 测试报告 / 评论显示**空态**（不造假），待产物管线接入。
- 未新增迁移（复用 `0010` 的 `task_run`、`0002` 的 `requirement_revision`）；迁移序号仍 ≥0011。
- 导航 5 项 vs 9 项仍未裁决（本计划未动导航）。
- Q1/Q2 待裁（产物语义、验收结论数据源）——首版按"产物=需求修订、只读需求正文"落地。