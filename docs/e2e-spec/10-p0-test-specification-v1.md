# AI-PM · P0 测试规格 V1.0（含首轮评审记录）

> **性质**：P0 发布验收的**总规格**。定义 7 个核心测试领域、24 条 P0 案例、执行阶段、结果判定与发布门禁。
> **上位文件**：`test-case/full-lifecycle-test-plan-v1.md`（全仓 L0–L4 策略与 G-merge/G-release 门禁）。
> **同级**：`docs/e2e-spec/00-e2e-testing-framework-v1.md`（体系总纲：8 套件 / 11 字段标准 / 判定口径）。
> **下位**：`docs/e2e-spec/11-p0-01-playwright-spec-v1.md`（P0-01 可执行 Playwright 规格）。
> **基线**：master @ `2b3f509e`；迁移基线 `migrations/0001_baseline.sql`。
> **纪律**：未执行不得标 PASS；NOT_IMPLEMENTED **不得计入 PASS**；UI 提示成功 **不得代替** 后端数据验证。

---

## 0. 首轮评审结论（TL;DR）

首轮把 24 条 P0 逐条对代码核验，得出 **9 条与实现不符**、**1 条编号冲突**、**3 条阻塞项**：

| 级别 | 数量 | 代表 |
|---|---|---|
| 🔴 阻塞（用例当前跑不起来） | 2 | P0-001 登录依赖 iam-service（栈里没有）；P0-011~014 知识权威性断言未核验 |
| 🟠 与实现不符（须改写断言） | 5 | InitItem 是 **7 态不是 6 态**；`core.project_initialization_operation` **不是**初始化权威源；`ai.project_cognition` **不存在**；Membership **无 status 列**；`ai.project_ai_config` 无 `auto_applied` 列 |
| 🟡 覆盖不足（断言可写但实现有洞） | 2 | AI 对 **task / milestone / 任务完成** 无人工闸；`requirement_analyst` 角色**无执行体** |
| ⚪ 编号冲突 | 1 | 规格引用的 DEF-01/DEF-02 与仓库既有 DEF 编号**不是同一件事** |

> 逐条证据见 §4。

---

## 1. P0 发布验收标准

| 项 | 值 |
|---|---|
| **Release Blocking** | ✅ 是 |
| **核心测试领域** | 7 |
| **首批测试案例** | 38（D22 扩编 P0-05；D27 原 P0-06 ACP 并入 P0-021、新 P0-06 = MCP 9 例；**D32**：原「权限隔离与安全」撤销独立套件（内容分散承接），P0-07 重定义 = 知识学习与采纳 8 例 P0-049~056） |
| **允许失败** | 0 |
| **允许 BLOCKED** | 0 |
| **允许 NOT_IMPLEMENTED** | 0 |

- 全部 P0 案例必须通过。
- 不允许存在未解释的测试失败。
- 不允许以 UI 显示成功代替后端数据验证。
- 不允许测试数据跨 Team 或 Project 污染。

---

## 2. P0 测试集划分

| Suite | 测试领域 | 案例数 | 核验状态 |
|---|---|---|---|
| **P0-01** | 用户登录与 Team Onboarding | 3 | ✅ 已核验（1 条阻塞，见 §4-R1） |
| **P0-02** | Team 与 Project 基础管理 | 4 | ✅ 已核验（4 条需改写，见 §4-R4~R7） |
| **P0-03** | Project AI-PM 核心能力 | 3 | ✅ 已核验并落盘 → `13-p0-03-playwright-spec-v1.md`（R21–R26：008 拆 a/b、分析走 intake、确认三道闸 403/哈希/治理授权） |
| **P0-04** | Requirement 与 Knowledge 权威性 | 4 | ✅ 已核验并落盘 → `14-p0-04-playwright-spec-v1.md`（三层权威真实落地；绑定点=submit；PageIndex=`ai.document_section_index`/`index_state`，reindex 路由 ×2） |
| **P0-05** | Task 执行与交付 | **7**（P0-015~021，D22 扩编） | ✅ 已核验并落盘 → `15-p0-05-playwright-spec-v1.md`（R33–R40：**R33 已修订**——`POST /v1/pm/tasks` 直接建任务存在且 require_human；Run 幂等 0012、delivery 4 值、AlreadyDecided、C2 缺口 DEF-P0-01） |
| **P0-06** | 外部 Agent / MCP 协作及结果回传 | 9（**P0-040~048**，D27；原 ACP 3 例并入 P0-021 场景 C） | ✅ 已核验并落盘 → `16-p0-06-playwright-spec-v1.md`（R41–R49：MCP fallback+device_layer、5 固定工具、Skills 四件套实证、**回传通道=D4 未开通 → 045b NOT_IMPLEMENTED**、采纳管道待查证） |
| **P0-07** | 知识学习、外部 Commit 采纳与统一协调 | 8（**P0-049~056**，D32） | ✅ 已核验并落盘 → `17-p0-07-playwright-spec-v1.md`（R50–R56：门禁 8 规则后端强制不依赖模型、协调者幂等账本、kb.memory_ref 溯源、**051/052 采纳管道 NOT_IMPLEMENTED**） |
| | **合计** | **38**（原权限套件 4 例分散承接：跨Team读→P0-007/FLOW-08+047；跨Project写→P0-015；AI审批→P0-010/019C+DEF-P0-01；MCP绑定→P0-041） | |

---

## 3. 详细 P0 测试案例

> 「评审」列：✅ = 可直接执行；⚠️ = 断言须按 §4 改写；🔴 = 当前阻塞；⏳ = 锚点待核验。

### P0-01 · 用户登录与 Team Onboarding

| ID | 测试案例 | 测试步骤 | 预期结果 | 评审 |
|---|---|---|---|---|
| P0-001 | 用户正常登录 | 使用有效账号登录 | 成功进入系统 | 🔴 登录走 **iam-service**，栈内无 → 见 `11-p0-01-playwright-spec-v1.md` §3.1，拆 a/b |
| P0-002 | 首次登录无 Team | 使用没有 Team Membership 的账号登录 | 必须先创建或加入 Team，不能直接进入项目系统 | ⚠️ 「Membership 状态」列不存在（R2）；「不能直接进入」**仅对经中转页的登录流成立**——深链不被拦（R4，见 §P0-01 重点验证 / `11-p0-01` §9-D6 待裁决）；DB 断言用 R5 口径（ACTIVE 成员 = 0 + `slug IS NULL`，非行数 = 0） |
| P0-003 | Team 创建与邀请 | 创建 Team，再通过邀请链接或邮件邀请成员 | 创建者成为 Team Owner；受邀用户正确加入 Team，角色及 Membership 状态正确 | ⚠️ 同 R2；角色枚举已确认为 `owner/admin/member` |

**重点验证**
- Team Membership 是用户与 Team 的**多对多**关系 —— `core.tenant_member(tenant_id, principal_id)` 无唯一约束即多对多 ✅（`migrations/0001_baseline.sql:1392-1400`）。
- Team 角色是 **Team 作用域**，不是全局用户角色 —— 角色存于 `core.tenant_member.role`，与 `core.principal` 解耦 ✅。
- 没有 Team 的用户**不能绕过 Onboarding** —— 分流判据 `onboarding_required`（`ai-frontend/src/views/auth/AuthLoadingView.vue:51`）。⚠️ **深链不被拦（R4，2026-11-10 UIFLOW-01 实测修正）**：onboarding 门只在登录中转页，`router.beforeEach` 无路由级 onboarding 守卫——未入团深链 `/projects` 停留原页。与 B09-18 fixme 同源待裁决（`11-p0-01` §9-D6）；守卫补上前「不可绕过」仅指经中转页的登录流，不含深链。

### P0-02 · Team 与 Project 基础管理

| ID | 测试案例 | 测试步骤 | 预期结果 | 评审 |
|---|---|---|---|---|
| P0-004 | 正常创建项目 | 在 Team 下创建项目 | 项目正确创建，归属当前 Team | ⚠️ 必填是 `code` + `name` 双字段（`apps/pm-api/src/pm_lists.rs:139-147`），非「只有名称」；端点在 `pm_lists.rs:150` |
| P0-005 | 创建后立即进入项目 | 完成创建操作 | 立即进入 Project Command Center，不要求先配置 Git 或 Runner | ✅ 成立：提交后 `router.push('/project/command-center')`（`ai-frontend/src/views/project/create/ProjectCreateView.vue:123-126`） |
| P0-006 | 项目初始化状态 | 查看新项目初始化进度 | 正确生成初始化事项及其派生状态 | ⚠️ InitItem 是 **7 态**（`crates/pm-application/src/command_center.rs:74-83`）且为**派生**；`core.project_initialization_operation` 与之无关（见 R6） |
| P0-007 | 多项目隔离 | 同一 Team 创建两个项目，分别写入数据 | 两个项目的数据及关联关系正确隔离 | ✅ 隔离层：`core.project.tenant_id` → `core.tenant` + `require_project_visible`（`apps/pm-api/src/support.rs:178-208`，不可见一律 404） |

**重点验证**
- Project 创建不能依赖外部 AI 工具 —— ✅ 成立：`bootstrap_inner` 只写 `ai.project_ai_config`（provider 默认 `'fake','fake'`，`crates/pm-application/src/bootstrap.rs:296-299`），无 Git/MCP/runner 依赖。
- Git Repository 和 Runner 都不是项目创建的前置条件 —— ✅ 成立（`CreateProjectBody` 无相关字段）。
- InitItem 的状态属于 Command Center 的**派生展示状态**，不应误当独立实体的数据库状态 —— ✅ 成立且**更强**：全部由纯函数 `init_judgement` 算出（`command_center.rs:98-217`），`migrations/` 中无对应列；权威源是 7 张领域表（`core.project.status`、`ai.document_revision.status`、`integration.project_repository.status`、`integration.repo_import.status`、`discovery.discovery_session.status`、`devenv.device.readiness`、`delivery.task.status`）。

### P0-03 · Project AI-PM 核心能力

| ID | 测试案例 | 测试步骤 | 预期结果 | 评审 |
|---|---|---|---|---|
| P0-008 | 无外部工具启动 AI-PM | 新建项目，不连接 MCP、Codex、Claude Code 或 Runner，打开 AI-PM | AI-PM 可以正常工作 | ⚠️ **按链路收窄**：分析链可退确定性 FakeModel 跑通（`crates/pm-ai/src/analysis_model.rs:110-150`）；**对话助手链**与**访谈编排链**必须有真实 LLM，且开关默认 off（`conversation_reply.rs:16-17,31-32`；`discovery_orchestrator.rs:30-32`） |
| P0-009 | 项目需求分析 | 向 AI-PM 提交一段项目需求，要求分析 | 正确生成分析结果，并保留项目关联 | ⚠️ `requirement_analyst` 角色**无执行体**（`crates/pm-application/src/ai_roles.rs:366`），需先补 `capability='manual'` 行；断言应指向 `ai.cognition_item` / `ai.cognition_run`（**无** `ai.project_cognition` 表） |
| P0-010 | AI 建议人工确认 | 要求 AI 提出项目认知或需求修改建议 | 产生待确认建议，不得未经授权直接修改正式认知 | 🟡 **部分不成立**：需求修订确认（`crates/pm-application/src/requirement.rs:787` 强制 `kind='human'`）、认知发布（`pc.cognition_version.confirmed_by NOT NULL`）已硬拦；但 **task / milestone / 任务完成无人工闸**（见 R8） |

**重点验证**
- AI-PM 是系统内置的项目管理能力，而非外部 Coding Agent 的附属 —— ✅ 成立：建项目事务内自动补齐内置角色（`bootstrap.rs:319` → `crates/pm-application/src/ai_roles.rs:261-288`）。
- 外部工具缺失不能阻断需求分析、项目认知和项目管理 —— 🟡 分链成立：MCP/Runner 只影响**任务执行**（`AI_PM_RUN_EXECUTOR` 缺省 `NoopExecutor`，`apps/ai-pm-worker/src/main.rs:136-143`），需求分析走 `integration.job('ai_analysis')` + FakeModel 回退。

### P0-04 · Requirement 与 Knowledge 权威性

| ID | 测试案例 | 测试步骤 | 预期结果 | 评审 |
|---|---|---|---|---|
| P0-011 | 正式 Requirement 创建 | 人工创建一条正式需求 | Markdown 知识主体正确创建，PostgreSQL 保存 ID、状态及版本引用 | ⏳ 待核验 |
| P0-012 | Requirement Revision | 修改一条已存在的正式需求 | 产生新 Revision，原版本保留，可追溯 | ⏳ 待核验（表已确认存在：`delivery.requirement` / `delivery.requirement_revision`，`migrations/0001_baseline.sql`） |
| P0-013 | AI AutoApply 禁止 | AI 提出正式需求或认知修改并尝试自动应用 | 没有人工正式确认时，不得写入正式权威内容 | ⏳ 待核验（已知红线 12 条含 `create_requirements/create_milestones/create_tasks/write_documents`，`crates/pm-application/src/policy_engine.rs:45-69`） |
| P0-014 | Knowledge 索引重建 | 删除或重建 PageIndex 派生索引，再执行检索 | Markdown 权威内容不受影响，索引可以重建，检索恢复 | ⏳ 待核验（PageIndex 落点未定） |

**重点验证 —— 三层权威边界**

| 层 | 职责 | 首轮结论 |
|---|---|---|
| Markdown + Git | 内容权威（Requirement / Knowledge / Revision 正文） | ⏳ 待核验 |
| PostgreSQL | 状态权威（ID、状态、版本引用、审批、状态机、审计、事件） | ✅ 部分成立（`delivery.*` / `design.*` / `governance.audit_log` / `integration.outbox_event` 齐备） |
| PageIndex | 派生索引（可删除可重建，不拥有正式内容） | ⏳ 待核验 |

> **必须特别注意**：数据库中不能出现与 Markdown 权威内容相互冲突的**另一份可独立修改的正式内容** —— 该风险需在 §7 核验中逐列排查（`content` / `body` / `text` 类列）。

### P0-05 · Task 执行与交付

| ID | 测试案例 | 测试步骤 | 预期结果 | 评审 |
|---|---|---|---|---|
| P0-015 | Task 创建与项目关联 | 归属正确、隔离、审计 | Task 正确创建并关联 Project | ✅（R33 已修订）：直接 REST `POST /v1/pm/tasks`（require_human，`pm_write/task.rs:180`）与计划物化 `materialize_plan`（`pm_plans.rs:235`）双链均通；负向 ai_pm 建任务 → 403 |
| P0-016 | Task 执行与 Run 生命周期 | mark-ready → TaskReady → Run | 执行、交付、Review 状态正确转换 | ✅ 活跃 Run 唯一索引 `uq_task_run_active` + 0012 事件幂等；Run/Task `claimed` 双常量（R34/R35） |
| P0-017 | Agent 执行结果记录 | 命令账 / 事件 / Commit 归属 Run | 执行过程与结果可追溯 | ✅ 读面 `GET .../runs/{id}/commands`、`/events`、`/commits`（R38） |
| P0-018 | Delivery 交付记录 | 交付读面 4 值（含 partial） | 执行结果形成真实可追溯交付 | ✅ `GET /v1/pm/tasks/{id}/delivery`；`delivery_state ∈ delivered/partial/missing/no_contract`（R36） |
| P0-019 | Review 审核流程 | 人工裁决 + AI 不得代审 | 审核权限、结论、审计真实可靠 | 🟡 C1 可断言（ai_pm 令牌 → 拒）；**C2 缺口登记 DEF-P0-01**（auto_accept 默认放行，`policy_composition.rs:82-86`） |
| P0-020 | 多 Repository Commit 交付关联 | Commit 归属与双向追溯 | 多仓交付正确关联、部分完成可表达 | ✅ `delivery.task_repository_commit` + git 直验 + `partial` 表达（R38） |
| P0-021 | Run 异常终止与人工介入 | 取消/超时/迟到事件/ACP | 状态一致、终态不复活、Continuation 合规 | ✅ `/cancel`（`project_lifecycle.rs:33`）+ interactions 幂等 + S03 冻结基线（R39） |

**重点验证**：Task 与 Project 的关联关系；Run 的执行结果不能丢失；交付记录与实际 Commit 对应；**Review 不能只依赖前端显示成功**。

### P0-06 · 外部 Agent / MCP 协作及结果回传（D27 重定义）

> **原「ACP 人工介入与恢复」3 例撤销独立套件** —— 内容已由 P0-05 P0-021 场景 C 承接（S03 冻结基线不变）。
> 架构原则：**MCP 是通信接口，不是执行 Runtime**（定位基线 V1.1）。

| ID | 测试案例 | 核心验证 | 评审 |
|---|---|---|---|
| P0-040 | Remote MCP 连接与认证 | 连接、身份、撤销、审计 | ✅ rmcp streamable HTTP 无状态 JSON + `fallback_service(mcp_service).layer(device_layer)`（`routes.rs:205-209`）；`auth_mode` 服务端判定不可自报（R41/R42） |
| P0-041 | Device 与 Project Binding | 绑定授权、跨项目拒绝 | ✅ `devenv.device` + `devenv.device_project_binding`（R43） |
| P0-042 | Bootstrap 安装与初始化 | /install、Skills ZIP、版本、安全 | ✅ 四 SKILL.md 与规格清单一字不差 + SHA-256 manifest（R44，T-089） |
| P0-043 | 外部 Agent 获取 Task | pm_get_task_context / pm_list_tasks | ✅（领取语义按 D1/D3 写死，D28） |
| P0-044 | 外部 Agent 获取正式知识 | 权威内容 + 版本引用 | 🟡 知识读工具**不在固定 5 工具集**（R45），通道缺失则 BLOCKED |
| P0-045 | 执行结果回传 | 结果关联 Task/Run/Agent/设备 | 🔴 **拆 a/b**：a=内部编排链（条件 UPDATE + fencing `accept_executor_result`）✅；b=**MCP 回传通道 = D4 未开通，NOT_IMPLEMENTED 不计 PASS**（R46） |
| P0-046 | 外部 Commit 与交付同步 | Commit 归属验证、多仓、人工采纳 | ✅ + 🟡 场景 D（外部人工 Commit 采纳管道）初判 NOT_IMPLEMENTED（R47/D30） |
| P0-047 | 外部 Agent 权限边界 | 越权、伪造审批、提示注入 | ✅ `require_mcp_permission` + 服务端独立鉴权 |
| P0-048 | 通信异常与一致性 | 断线、重试、迟到、重启 | ✅ 终态条件 UPDATE 先到先得 + 0012 幂等（R48） |

**重点验证**：用户身份认证与设备项目授权分离；外部 Agent 结果是待验证证据；知识/审批权威不转移给外部 Agent。

### P0-07 · 知识学习、外部 Commit 采纳与统一知识协调（D32 重定义）

> **原「权限隔离与安全」4 例撤销独立套件**：跨 Team 读 → P0-007 + FLOW-08 + P0-047；跨 Project 写 → P0-015 步骤 6-7（守卫深验仍挂 §8）；AI 审批 → P0-010 三道闸 + P0-019C1 + **DEF-P0-01**；MCP 绑定 → P0-041。
> 边界红线：AI 学习 ≠ 正式知识；外部 Commit ≠ 采纳；草稿 ≠ 正式 Revision。

| ID | 测试案例 | 核心验证 | 评审 |
|---|---|---|---|
| P0-049 | 执行结果触发知识学习 | 候选可溯源，不直写正式知识 | 🟡 拆 a（证据与提案链 ✅）/ b（自动触发待查证，R56/D33） |
| P0-050 | 学习结果来源与可信度 | 来源/证据/可信度持久化 | ✅ `kb.memory_ref`（evidence/subject + revision_missing_reason）+ `kb.source`（R50） |
| P0-051 | 外部 Commit 自动发现与分析 | 真实 SHA、归属、知识影响识别 | 🔴 **发现管道无实现 → NOT_IMPLEMENTED**（R54）；归属验证语义已在 P0-020 |
| P0-052 | 外部 Commit 澄清与采纳闭环 | 澄清/免澄清记录、人工采纳 | 🔴 **专用管道无实现 → NOT_IMPLEMENTED**（R55）；提案确认面可预演 |
| P0-053 | 多 Agent 并行提议与统一协调 | 并行分析、串行写入、过期校验 | ✅ `knowledge_write_coordinator` + `base_revision_id` + `pc.conflict`（R52/R53） |
| P0-054 | 质量门禁与人工确认 | 门禁后端强制、AI 不可绕过 | ✅ `knowledge_quality` **8 规则不依赖模型** + 3 红线 → `redline_pending`（R51） |
| P0-055 | Revision 与 Git 版本一致性 | 三面一致、账本恢复 | ✅ 14 号口径 + Git 先行 + 账本恢复路径 |
| P0-056 | 学习失败、冲突与恢复 | 异常矩阵、幂等、恢复 | ✅ 账本 + 0012 + `reindex_required` |

**重点验证**：「学习」与「正式知识写入」严格分开 —— 保护 Markdown+Git 权威模型与多 Agent 统一协调。

---

## 4. 评审记录（逐条证据）

| # | 结论 | 证据 |
|---|---|---|
| **R1** | 🔴 P0-001 真实表单登录当前**不可执行**：前端登录调用 lmms iam-service（`/v1/iam/auth/*`），非 pm-api；E2E 栈只起 pm-api + PG + JWKS | `ai-frontend/src/api/auth.ts:14-15,112`；`ai-frontend/e2e/team/global-setup.ts:13-21` |
| **R2** | ⚠️ `core.tenant_member` **无 status 列**；列 = `tenant_id / principal_id / role / granted_by / created_at / updated_at`，`role` CHECK ∈ `owner/admin/member` | `migrations/0001_baseline.sql:1392-1400` |
| **R3** | ⚠️ `ai.project_cognition` **不存在**；认知域实际为 `ai.cognition_item` / `ai.cognition_run` 与 `pc.cognition_object` / `pc.cognition_revision` / `pc.cognition_version` | `migrations/0001_baseline.sql` |
| **R4** | ⚠️ 创建项目必填是 **`code` + `name`**（空串 → 400 `"project code and name are required"`），端点在 `pm_lists.rs` | `apps/pm-api/src/pm_lists.rs:139-161` |
| **R5** | ⚠️ InitItem 是 **7 态**（`not_started / in_progress / waiting_for_human / completed / blocked / not_applicable / undetermined`），且为 `init_judgement` **派生**展示态，枚举在 `crates/pm-application/src/command_center.rs` 而非 `apps/pm-api/src/command_center.rs` | `crates/pm-application/src/command_center.rs:74-83,98-217` |
| **R6** | ⚠️ `core.project_initialization_operation` 与初始化展示**无关**：它是 repo import 占用锁（status CHECK ∈ `active/released/completed/cancelled`），仅导入路径写入 | `migrations/0001_baseline.sql:1269-1280`；`crates/pm-application/src/project_import/lifecycle.rs:146,203` |
| **R7** | ⚠️ 评审前写的框架文档中「创建项目 → `core.project_initialization_operation` 生成初始化任务」**不成立**（该行只在 `POST /imports` 产生）→ TC-803 断言对象须改为 `core.project` + `core.project_member` + `ai.project_ai_config` + `ai.project_ai_role` | `crates/pm-application/src/bootstrap.rs:270-319` |
| **R8** | 🟡 「AI 不得直接修改正式内容」**不全局成立**：红线只含 `create_milestones/create_requirements/create_tasks/write_documents`，不含 `create_task` / `update_task_status` / `adjust_milestone_plan`；策略引擎**黑名单默认 `auto_apply`**（`policy_engine.rs:32`），AI 以 `ActorKind::AiPm` 直写 `delivery.task` / `delivery.milestone`；且 `auto_completable()` 允许 `reviewing→completed` 无人工 | `crates/pm-application/src/policy_engine.rs:32,45-69`；`crates/pm-ai/src/orchestrator.rs:278-288,521`；`crates/pm-application/src/review/decide.rs:60-70`；`crates/pm-application/src/policy_composition.rs:82-86` |
| **R9** | ⚠️ **缺陷编号冲突**：仓库既有 `DEF-01`（Team QA，P0 迁移撞号）与 `DEF-02`（Team QA，P1 被移除成员残留访问），与规格所指「跨项目写入」「AI AutoApply 审批」不是同一件事 | `test-case/README.md`；`test-case/team-qa-execution-report-v1.md` |
| **R10** | ⚠️ `ai.project_baseline` / `delivery.requirement_revision` / `ai.project_ai_config` **均无 `auto_applied` 列**，不能作为审计断言锚点；自动应用语义由 `automation_mode ∈ {ai_lead, controlled}` + `enabled` 表达 | `migrations/0001_baseline.sql:1016-1032,1068-1076,1620-1640` |

**待裁决动作**

| # | 动作 | 建议 |
|---|---|---|
| D1 | P0-001 拆分 a/b | a = 会话级自动化（立即做）；b = 真实表单登录（iam 入栈，否则标 BLOCKED） |
| D2 | 缺陷编号空间 | P0 专属用 `DEF-P0-01…`，不与 Team QA 的 DEF-01/02 混用 |
| D3 | P0-010 / P0-023 断言收窄 | 断言「需求修订确认 + 认知发布」已拦；task/milestone/任务完成另立缺陷单，不写成 PASS 用例 |
| D4 | TC-803 断言对象 | 改为 `core.project` + `core.project_member`(5 行) + `ai.project_ai_config` + `ai.project_ai_role` |
| D5 | InitItem 状态数 | 全仓统一改口为 **7 态**，并注明为派生展示态 |

---

## 5. P0 测试执行顺序

不按编号串行执行，分四个阶段，每阶段环境与数据隔离；安全测试用独立账号与独立测试项目。

| 阶段 | 案例 | 验证目标 | 隔离要求 |
|---|---|---|---|
| **A · 系统准入** | P0-001 ~ P0-007 | 用户、Team、Project 基本可用 | 独立 `tid`；独立 Team/Project 命名前缀 |
| **B · 核心业务** | P0-008 ~ P0-014 | AI-PM、Requirement、Knowledge 的核心权威性 | 独立项目；独立认知/知识空间 |
| **C · 执行链路** | P0-015 ~ P0-021 + P0-040 ~ P0-048 | Task/Run/Delivery/Review 内部链 + 外部 Agent/MCP 协作 | 独立仓库关联；可控执行 adapter + 可控 MCP client |
| **D · 知识治理与恢复** | P0-049 ~ P0-056 | 知识学习、Commit 采纳、统一协调、门禁与恢复 | 独立知识空间（KnowledgeRoot）+ 可控 Agent |

> 阶段 D 必须**最后**执行且在**独立**环境中 —— 越权用例需要「合法主体」与「非法主体」并存，同环境复用会污染阶段 A~C 的数据。

---

## 6. 统一验收规则

### 6.1 结果定义

| 结果 | 判定标准 |
|---|---|
| **PASS** | 操作成功，业务结果及持久化数据均符合预期 |
| **FAIL** | 实际结果与预期不一致 |
| **BLOCKED** | 环境、依赖或权限问题导致无法执行 |
| **NOT_IMPLEMENTED** | 对应业务能力尚未开发，**不代表测试通过** |

> **NOT_IMPLEMENTED 不能计入 PASS。**

### 6.2 每次执行必须保存

- 测试执行时间、版本号、**Commit SHA**
- 测试账号、**Team ID**、**Project ID**
- Playwright Screenshot
- Playwright Trace
- API Request / Response
- 关键业务记录及状态变化
- 失败原因及关联缺陷编号

### 6.3 用例字段

沿用体系总纲的 **11 字段标准**（Case ID / Suite / Priority / Preconditions / Test Data / Steps / Expected Result / Actual Result / Status / Evidence / Cleanup），见 `docs/e2e-spec/00-e2e-testing-framework-v1.md` §4。

---

## 7. P0 发布门禁

| 项 | 值 |
|---|---|
| P0 总案例 | 38（D22 + D27 + D32） |
| 允许失败 | 0 |
| 允许 BLOCKED | 0 |
| 允许 NOT_IMPLEMENTED | 0 |
| **判定** | **38 / 38 PASS** 才允许通过 P0 发布验收 |

> 与全仓门禁的挂接：`G-release`（发布候选）要求 L1 + L2 全量 + L3 全量 + L4 关键路径全绿且**零 S0/S1 缺陷**（S0–S3 = 缺陷严重度，见总纲 §5.1，与用例优先级 P0/P1/P2 分离）；本规格是 **G-release 的 P0 子集判据**。

---

## 8. 待核验清单（下一轮）

| # | 套件 | 需核验内容 | 关键锚点（已定位） |
|---|---|---|---|
| 1 | ~~P0-04~~ | ✅ 已核验（2026-11-10）→ `14-p0-04-playwright-spec-v1.md` §0 R27–R32：权威 = `knowledge.rs`（Markdown+Git）+ `requirement_projection.rs`（绑定点 submit）；PageIndex = `ai.document_section_index` + `ai.document_index_state`；content jsonb 是读投影（`rebuild_projection` 可从 Git 重建），不构成第二权威 | 关闭 |
| 2 | ~~P0-05~~ | ✅ 已核验（2026-11-10；**R33 于 2026-11-11 修订**：直接建 Task REST 存在且 human 闸）→ `15-p0-05-playwright-spec-v1.md` §0 R33–R40；Run=TaskReady+`uq_task_run_active`+0012 幂等；delivery 4 值含 partial；Review `AlreadyDecided` 三重防护；**C2 缺口登记 DEF-P0-01** | 关闭 |
| 3 | ~~P0-06（原 ACP）~~ | ✅ 已裁决（D27）：撤销独立套件，内容由 P0-021 场景 C 承接；新 P0-06 = 外部 Agent/MCP（P0-040~048）已核验落盘 → `16-p0-06-playwright-spec-v1.md` | 关闭 |
| 4 | ~~P0-07 权限套件~~ | ✅ 已裁决（D32）：撤销独立套件、内容分散承接；**残留深验项**：跨 Project 写守卫（`command_executor/guards.rs`）归 P0-015 复验；MCP 绑定划界归 P0-041。新 P0-07（知识治理 049~056）已核验落盘 → `17-p0-07-playwright-spec-v1.md` | 关闭（残留移交） |

---

## 9. 关联文档

| 文件 | 内容 |
|---|---|
| `docs/e2e-spec/README.md` | 体系目录索引与边界 |
| `docs/e2e-spec/00-e2e-testing-framework-v1.md` | E2E 体系总纲（8 套件 / 11 字段 / 判定口径） |
| `docs/e2e-spec/11-p0-01-playwright-spec-v1.md` | P0-01 可执行 Playwright 规格（选择器 + 断言 + fixture） |
| `test-case/full-lifecycle-test-plan-v1.md` | 全仓 L0–L4 策略与发布门禁 |
| `test-case/e2e-test-plan-v1.md` | L2/L3 落地计划与用例矩阵 |
| `test-case/team-qa-test-plan-v1.md` | Team 专项 QA（DEF-01/DEF-02 原始出处） |
