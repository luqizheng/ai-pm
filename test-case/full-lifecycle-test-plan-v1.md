# AI-PM · 测试全流程测试计划 V1.0

> **性质**：全仓测试策略总纲。定义「测什么、在哪测、用什么载体测、什么算通过、什么卡发布」五件事，作为 S01–S05 各切片测试计划与 E2E 计划的上位文件。
> **范围**：本仓 `apps/`（pm-api / pm-worker / ai-pm-worker / ai-pm-runner）、`crates/`（pm-domain / pm-application / pm-infrastructure / pm-mcp / pm-ai / kb-pc-contracts）、`ai-frontend/`（Vue 3 + Vite + Playwright）。
> **判定纪律**（沿用 `test-case/README.md`）：用例文件即执行记录；未执行不得标 PASS；无法执行标 BLOCKED 并写明原因；HTTP 200 不作唯一证据，跨存储终态才算。
> **基线**：master @ `2b3f509e`（merge(s01-s05)），迁移基线 `migrations/0001_baseline.sql` + 0013/0016 系列。

---

## 1. 五层测试载体（Test Pyramid）

| 层 | 载体 | 入口 | 依赖 | 单次耗时量级 | 频率 |
|---|---|---|---|---|---|
| **L0 单元** | Rust `#[test]`（域内纯函数：状态机矩阵、哈希、加密信封、URL 硬化） | `cargo nextest run -p <crate>` | 无 | ms | 每次提交 |
| **L1 集成** | Rust `#[sqlx::test]`（真 PostgreSQL 临时库 + axum `oneshot` 真路由） | `cargo nextest run -p pm-api --test <name>` | 真实 PG（`127.0.0.1:16433`） | 0.1–3 s | 每次提交 |
| **L2 服务级 E2E** | 真 axum router + 真 PostgreSQL 临时库 + **真 Git 远端（SSH over 本机真实私钥）** | `bash test-case/evidence/e2e-flow/run-e2e-flow.sh` | PG 容器 + SSH 私钥 + 两个真实仓库 | 20 s | 每日 / 合并到主干前 |
| **L3 UI E2E** | Playwright（MS Edge headless，1440×900）+ 真 pm-api + 真 vite | `pnpm exec playwright test -c playwright.e2e.config.ts` | 同 L2 + 浏览器 | 5–20 min | 每日 / 发布前 |
| **L4 两机 / 跨进程** | 真实执行端（Codex / Z-Code）+ 真实 MCP 跨机暴露 | `test-case/t090-two-machine-test-cases-v1.md` | 两台内网机器（192.168.2.10 / .43） | 人工，小时级 | 里程碑节点 |

**分层裁决口径**

- L0/L1 判**逻辑与契约**（状态机、权限、幂等、迁移完整性、错误码）。
- L2 判**进程与网络**（真进程启停、worker 领取、真实 Git 协议与凭据、真实 DB 事务）。
- L3 判**用户可见状态**（页面渲染、路由跳转、交互反馈）。
- L4 判**跨机真实性**（Host 白名单、真实执行端行为、免鉴权入口）。

> **红线**：L2/L3 不得用 mock/fixture 冒充后端。`ai-frontend/build/release-guard.ts` 在 production/staging 构建期硬失败；fixtures/demo 仅允许本地 dev（`VITE_USE_MOCK=1`）与单测动态加载。

---

## 2. 业务全流程与测试覆盖映射

产品主链路（`docs/ai-pm-five-stage-business-architecture-v1.md`）与各层落点：

| 阶段 | 业务动作 | 后端落点 | 数据落点 | 前端落点 | 主要测试层 |
|---|---|---|---|---|---|
| **S0 身份** | 登录 / 建团 / 邀请 / 角色 | `pm-api/auth.rs`、`team_onboarding.rs`、`tenant_members.rs` | `core.principal`、`core.tenant`、`core.tenant_member` | `/auth/*`、`/onboarding/*`、`/settings/team/*` | L1 + L3 |
| **S1 项目初始化** | 建项目 / 接仓库 / 冷启动导入 / 认知基线 | `pm_lists.rs`、`pm_repositories.rs`、`pm_imports.rs`、`pm_discovery.rs` | `core.project`、`integration.project_repository`、`integration.repo_import` | `/project/create`、`/project/settings/repositories`、`/project/discovery`、`/project/cognition` | L1 + **L2（真实仓库）** + L3 |
| **S2 需求** | 需求草稿 → 修订 → 提交 → 真人确认 | `pm_requirements.rs`、`requirement.rs` | `delivery.requirement*` | `/project/requirements/*` | L1 + L3 |
| **S3 设计** | 设计 / ADR / 审批 / 实现就绪 | `pm_designs.rs`、`design.rs` | `design.*` | `/project/design/*` | L1 + L3 |
| **S4 计划** | 任务计划 → 审批 → 物化任务 | `pm_plans.rs`、`task_plan.rs` | `plan.task_plan*`、`delivery.task` | `/project/tasks/*` | L1 + L2 + L3 |
| **S5 执行** | 任务 ready → Run → 事件 → 干预 → 提交 | `pm_execution.rs`、`pm_runs.rs`、`run_dispatch/` | `delivery.task_run*`、`governance.*` | `/project/execution/*` | L1 + L2 + L3 + L4 |
| **S6 交付** | 交付清单 / 验收裁决 / 归档 | `pm_delivery.rs`、`pm_acceptance.rs`、`pm_reviews.rs`、`project_lifecycle.rs` | `delivery.*`、`governance.review_approval` | 交付/验收**前端无直连页**（见 §6 缺口 G1） | L1 + L2 |
| **贯穿** | AI 团队策略 / 会话 / 通知 / 审计 / 事件流 | `ai_roles.rs`、`ai_team_*`、`conversations.rs`、`audit_stream.rs` | `ai.*`、`governance.audit_log` | `/project/assistant`、`/settings/team/ai-providers`、`/notifications` | L1 + L3 |

---

## 3. 状态机（断言基准）

E2E 每条流转断言必须同时校验 **HTTP 码 + 响应字段 + DB 终态**。

| 实体 | 取值 | 允许流转 | 权威定义 |
|---|---|---|---|
| Task（13 态） | `draft ready claimed running awaiting_decision blocked submitted reviewing completed changes_requested failed cancelled superseded` | `draft→ready`；`ready→claimed`；`claimed→running`；`running→submitted/blocked/awaiting_decision`；`submitted→reviewing`；`reviewing→completed/changes_requested`；终态无出边 | `crates/pm-domain/src/task.rs:15`、`task_transition.rs:32` |
| Run（7 态） | `queued claimed running succeeded failed timed_out cancelled` | 活跃集合 `[queued, claimed, running]` | `crates/pm-application/src/run_dispatch/mod.rs:28` |
| 需求修订（5 态） | `draft pending_confirmation approved rejected superseded` | `draft→pending_confirmation→approved`；`approved→superseded` | `crates/pm-application/src/requirement_read.rs:15` |
| 需求生命周期（3 态） | `active withdrawn archived` | `withdraw/restore/archive` 单向 | `crates/pm-application/src/requirement.rs:39` |
| 设计修订（5 态） | 同需求修订 | 同上 | `crates/pm-domain/src/design.rs:17` |
| ADR（6 态） | `proposed under_review accepted rejected superseded deprecated` | `accepted→superseded/deprecated`；仅 `accepted` 可实施 | `crates/pm-domain/src/design.rs:80` |
| 计划（4 态）+ 修订（5 态） | `draft in_review approved superseded` / `draft submitted approved rejected superseded` | `draft→in_review→approved→superseded` | `crates/pm-domain/src/plan.rs:21` |
| Import | `queued cloning reading analyzing draft_ready proposed applied failed cancelled` | 活跃 = `queued/cloning/reading/analyzing` | `crates/pm-application/src/project_import/mod.rs:131` |

**非法流转门禁**：每次状态变更后必须有一条负向用例——非法流转应被拒绝（409/400），且 DB 终态不变。

---

## 4. 测试环境矩阵

| 环境 | 用途 | 组成 | 关键地址 |
|---|---|---|---|
| **DEV（本机）** | 开发自测 | vite :8848 / mock 模式 | 仅本地 |
| **QA（本机）** | L1–L3 自动化 | PG 容器 `aipm-pg-gate`（宿主 `127.0.0.1:16433`）+ 独立 pm-api/worker 进程 + 独立 vite | 见 `docs/test-environments.md` |
| **TEST-SERVER（内网）** | L4 部署验收 | `192.168.2.10` Docker 栈（pm-api 8090 / gateway 3000 / aipm-pg 5435） | 见 `docs/test-environments.md` |
| **WINDOWS-PC（内网）** | L4 真实执行端 | `192.168.2.43:60209` 远程桌面 | 同上 |

**硬约束**（AGENTS.md 第 3 条）
- 连 Docker 映射端口一律写 `127.0.0.1`，**禁止 `localhost`**（本机 IPv6 回环是黑洞，`localhost` 先耗尽 OS 超时才回落 `::1`）。
- `DATABASE_URL='postgres://aipm:aipm@127.0.0.1:16433/aipm'`。
- 测试进程被杀会泄漏 `_sqlx_test_%` 库 → 跑完必须清理。

**数据保护**
1. 绝不连接 `superpowers-full-postgres-1` 的正式 `aipm` 库做写入。
2. 团队解散 / 项目归档 / 审批裁决只在一次性库内执行。
3. 不修改 `template1`；不修改产品代码与迁移（缺陷只登记）。

---

## 5. 批次与门禁（Release Gates）

| 门 | 触发 | 必须全绿的层 | 判定 |
|---|---|---|---|
| **G-commit** | 每次 push | L0 + L1（`cargo nextest run --workspace`）+ 前端 `vitest` + `release-guard` | 全绿才可合并 |
| **G-merge** | 合并到 master 前 | G-commit + L2 冒烟（接一个真实仓库） | 全绿 |
| **G-release** | 发布候选 | L1 + **L2 全量** + **L3 全量** + L4 关键路径 | 全绿 + 零 P0/P1 |
| **G-hotfix** | 线上 P0 修复 | G-commit + 该 P0 的专项回归用例 | 全绿 |

**缺陷分级**
- **P0** 安全 / 数据完整性 / 审批边界失效 / 系统不可启动 —— 禁止发布。
- **P1** 核心流程失败 / 权限越界 / 重要数据不一致 —— 禁止发布（除非正式风险裁决）。
- **P2** 非核心功能异常或可控缺陷。
- **P3** UI / 文案 / 体验。

**明确不做**：不为「旧数据 / 旧格式 / 旧行为」写迁移、双写、兼容分支。需要改契约直接改契约（AGENTS.md 第 1 条）。

---

## 6. 已知覆盖缺口（登记，不静默）

| # | 缺口 | 影响 | 处置 |
|---|---|---|---|
| **G1** | 交付清单 / 验收裁决 / 归档**前端无直连页面**（`api/acceptance.ts`、`api/review.ts` 未被任何 view 引用；归档按钮未接线） | L3 无法覆盖 S6 用户路径 | S6 目前只做 L1 + L2；前端接线另立项 |
| **G2** | 项目导入前端无页面（`api/imports.ts` 未接线） | S1 导入只能 L2 覆盖 | 同上 |
| **G3** | 客户主数据前端无页面（`api/customers.ts` 未接线） | 客户域只 L1 | 同上 |
| **G4** | 文档知识域 `/v1/pm/documents` 系未接线（前端只接 `/v1/kb` + `/v1/pc`） | 文档域两套契约并存 | 设计稿裁决前只测已接线面 |
| **G5** | `scripts/e2e/acp09_e2e.py`、`run_storyline.py` 为早期 Python 剧本，与本仓 Playwright 基建未统一 | 存在两套 E2E 口径 | 以 `ai-frontend/e2e/` + `test-case/evidence/` 为唯一真源；旧脚本标记 deprecated |

---

## 7. 文档索引

| 文件 | 内容 |
|---|---|
| `docs/e2e-spec/00-e2e-testing-framework-v1.md` | **E2E 测试体系总纲**（8 套测试集、案例 11 字段标准、P0/P1/P2 分级、PM-AI 专属套件） |
| `test-case/e2e-test-plan-v1.md` | **E2E 测试计划**（L2/L3 载体、用例矩阵、执行方式、SSH 真实仓库接入） |
| `test-case/e2e-flow-execution-report-v1.md` | E2E 执行报告（7/7 PASS + DEF-E2E-01/02/03 缺陷记录） |
| `apps/pm-api/tests/e2e_flow.rs` | E2E 用例实现（真 router + 真 PG + 真 Git 远端） |
| `test-case/team-qa-test-plan-v1.md` | Team Onboarding 专项 QA 计划 + 88 条用例 + 执行报告 |
| `test-case/s02-qa-test-plan-v1.md` | S02 发布验收测试计划 |
| `test-case/t090-two-machine-test-cases-v1.md` | L4 两机实测用例（T-090 / T-091） |
| `docs/test-environments.md` | 环境清单与端点 |
| `test-case/README.md` | 用例目录约定 |

---

## 8. 证据落盘规范

| 类型 | 位置 |
|---|---|
| 用例执行记录 | 对应 `test-case/*.md` 的「实际结果 / 判定」行 |
| Playwright 报告 | `test-case/evidence/<batch>/playwright-report/`（HTML）+ `playwright-results.json` |
| Playwright 截图/trace | `test-case/evidence/<batch>/playwright-artifacts/`（仅失败时保留） |
| 后端日志 | `test-case/evidence/<batch>/pm-api-e2e-<stamp>.log`、`ai-pm-worker-<stamp>.log` |
| DB 终态快照 | `test-case/evidence/<batch>/*.json`（关键表行数 + 状态分布） |
| 一键复跑 | `test-case/evidence/<batch>/run-<batch>.sh` |

**证据要求**：任何标 PASS 的条目必须能指到「用例编号 → 证据文件 → 关键断言值」。HTTP 状态码本身不是证据。
