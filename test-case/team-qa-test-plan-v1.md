# Team 功能 QA · 测试计划 V1.0

> **范围**：Team Onboarding / 团队管理全链（Team≡tenant 认领建团、邮箱邀请、邀请链接、名册与角色、解散重建、`/v1/pm/me` onboarding 门、团队域前端页面与 TeamSwitcher）。
> **性质**：独立验收。判定以代码、数据库终态、API 响应与执行日志为证，不采信开发结论。
> **记录约定**：沿用 `test-case/README.md`——用例文件（`team-qa-test-cases-v1.md`）即执行记录；未执行不得标 PASS；无法执行标 BLOCKED 并写明原因。

---

## 1. 被测对象

| 项 | 值 |
|---|---|
| 代码基线 | master @ `3680138`（2026-11-06，含 merge 813cc97 Team Onboarding 全栈 + 500cce2 AI 模型策略） |
| 后端 | `crates/pm-application/src/team_onboarding.rs`（服务层）、`apps/pm-api/src/team_onboarding.rs`（12 条路由）、`tenant_members.rs`（名册/角色/转让）、`principals.rs`（/me）、`auth.rs`（自助路径放行） |
| 数据 | 迁移 `0044_team_onboarding.sql`（tenant 扩列 + 邀请三表） |
| 前端 | `ai-frontend/`：`views/onboarding/*`、`views/team/TeamInvitationAcceptView.vue`、`views/settings/team/*`、`views/dashboard/TeamDashboardView.vue`、`components/global/team/TeamSwitcher.vue`、`api/team.ts`、`stores/team.ts` |
| 设计依据 | `docs/team-onboarding/team-domain-model-database-design-v1.md`（§6.1 权限矩阵、§6.3 隔离流程、§15 TDM-001~018）；`docs/team-onboarding/team-onboarding-team-management-spec-v1.md`（§9 验收 12 条）；产品裁决（记忆台账 2026-11-06：Team≡tenant / V1 单团队 / 无邮件投递 / owner 独占解散） |

## 2. 测试环境

| 项 | 值 |
|---|---|
| QA 专用 PostgreSQL | 容器 `aipm-teamqa-pg`（postgres:16，宿主 `:16499`），由 `run-team-qa.sh` 按需创建 |
| 集成测试库 | `sqlx::test` 每用例独立临时库（建在 QA 容器内，自毁） |
| E2E 后端 | 独立 pm-api 进程 `:18490`，`AI_PM_AUTH_MODE=jwks`；本地 JWKS 服务 `:18491` 发布 `apps/pm-api/tests/keys/public.pem`；一次性库 `aipm_teamqa_e2e`（跑完即删） |
| E2E 前端 | vite `:8850`（独占，`PM_API_URL` 指向独立 pm-api，`VITE_USE_MOCK=0`），MS Edge headless 1440×900 |
| 身份 | 自签 RS256 JWT（kid=default）；每个 UI 用例用独立 `tid` → 独立租户 |

**数据保护规则（硬约束）**
1. 不连接 `infrastructure-aipm-pg-1` 的 `aipm` 库（含演示种子），不对 192.168.2.10 执行任何操作。
2. 不使用、不修改共享 PG 的 `template1`（已被污染，见 §7 ENV-1）。
3. 解散（dissolve）只在一次性库内执行。
4. 不修改产品代码与迁移（缺陷只登记，修复另行裁决）。

## 3. 执行载体（两层证据）

| 层 | 载体 | 证据 |
|---|---|---|
| L1 代码级 | `cargo test`：`team_onboarding.rs`（既有 16）+ **`team_onboarding_qa.rs`（本批新增 33，含 3 个待裁决 `#[ignore]`）** + `tenant_members.rs` / `project_members.rs` 回归 | axum router oneshot + 真实 PG；断言响应 + DB 终态 + 审计行 |
| L2 UI E2E | **`ai-frontend/e2e/team-onboarding.spec.ts`（20 例，1 例待裁决 fixme）** + `playwright.team.config.ts` + `e2e/team/{stack,global-setup}.ts` | 浏览器 UI 状态 + 直连 pm-api 的终态双证据 |

一键入口：`bash test-case/evidence/team-qa/run-team-qa.sh [--shim] [--only-backend|--only-e2e] [--ignored]`
- 默认模式直接用仓库迁移（撞号未修时如实报红）；
- `--shim`：在仓库外临时副本把 `0044_s02_i3_source_traceability.sql` 顺移为 0045 后执行——**结果仅代表「以顺移为前提」的团队逻辑**；
- `--ignored`：额外执行待裁决探针（失败即记录现状）。

## 4. 批次

| 批次 | 内容 | 设计映射 |
|---|---|---|
| B00 | 前置：迁移完整性、环境健康 | — |
| B01 | 建团/认领（creator=owner、并发认领、名称/slug 边界、鉴权） | TDM-001/002/016 |
| B02 | 邮箱邀请生命周期（创建/预览/接受/撤销/过期/防重/邮箱比对/并发接受/token 存储） | TDM-006~009 |
| B03 | 邀请链接（配额、幂等、并发、过期、撤销、跨租户） | TDM-010/011 |
| B04 | 三角色权限矩阵（owner/admin/member/未激活） | §6.1 |
| B05 | 名册、改角色、owner 转让、最后 owner 保护、被移除成员 | TDM-012/013/014 |
| B06 | 解散与重建 | 产品裁决 2026-11-06 |
| B07 | 跨租户隔离 | TDM-015、§6.3 |
| B08 | `/me` 与 onboarding 门 | spec §9-1 |
| B09 | 前端 UI（onboarding 四屏、接受页、团队设置三页、Danger Zone、TeamSwitcher） | UIUX 稿 Screen 01-07 |
| B10 | 并发与数据完整性（并入 B01-07 / B02-17 / B03-06） | TDM-011、§8 |
| B11 | V1 不做项（多团队、切换、离开团队、历史迁移） | V1 裁决 |

## 5. 判定口径

- **PASS**：响应 + DB 终态（+ UI 状态）全部符合设计口径。
- **FAIL**：与设计口径或产品裁决不符 → 登记缺陷。
- **PENDING-DECISION**：设计稿与实现不一致、但属产品口径未定 → 不计缺陷，单列待裁，测试以设计口径书写并 `ignore/fixme`。
- **NOT IMPLEMENTED**：V1 裁决明确不做。
- **BLOCKED**：环境原因无法执行，写明原因。

## 6. 缺陷分级

- **P0** 安全/数据完整性/系统不可启动 —— 禁止发布
- **P1** 核心流程失败、权限越界、重要数据不一致 —— 禁止发布（除非风险裁决）
- **P2** 非核心功能异常或可控缺陷
- **P3** UI/文案/体验

## 7. 执行前已知事实（第 0 步实测）

| ID | 事实 | 证据 | 影响 |
|---|---|---|---|
| DEF-01 | master 上 `0044_s02_i3_source_traceability.sql`（5936105，17:19）与 `0044_team_onboarding.sql`（c2b6199，18:20）同版本号；台账只登记后者 | `git ls-files migrations`；`docs/reference/ai-pm-migration-ledger.md` L13-14 | 干净库迁移撞 `_sqlx_migrations_pkey`(44)；L1 默认模式全红，故采用 `--shim` 取团队逻辑证据 |
| ENV-1 | 共享 PG `infrastructure-aipm-pg-1` 的 `template1` 被预灌 42 条迁移，其中 0025 校验和与本仓文件不同（行尾 CRLF/LF 差） | `_sqlx_migrations` 校验和比对 | 任何指向 `:16433` 的 `sqlx::test` 报 `VersionMismatch(25)`——环境污染，非代码缺陷；本 QA 改用独立容器规避 |
| ENV-2 | `infrastructure-pm-api-1` 循环重启 `VersionMissing(40)` | `docker logs`；镜像构建于 2026-11-05，库已应用到 42 | 本地栈 pm-api 镜像过旧；与团队功能无关，本 QA 不使用该容器 |
