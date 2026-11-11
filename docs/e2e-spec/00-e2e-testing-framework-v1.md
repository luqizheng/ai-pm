# AI-PM · E2E 测试体系总纲 V1.0

> **性质**：E2E 测试的体系级总纲。定义「测什么、怎么写案例、什么算通过、什么卡发布」四件事。
> **上位文件**：`test-case/full-lifecycle-test-plan-v1.md`（全仓 L0–L4 测试策略总纲）。
> **下位文件**：`test-case/e2e-test-plan-v1.md`（L2/L3 落地计划与用例矩阵）、`test-case/e2e-flow-execution-report-v1.md`（执行报告）。
> **边界**：本文件只管体系与口径，不重复落地拓扑与脚本细节，避免两套真源。
> **基线**：master @ `2b3f509e`；迁移基线 `migrations/0001_baseline.sql`。
> **纪律**（沿用 `test-case/README.md`）：未执行不得标 PASS；无法执行标 BLOCKED 并写明原因；HTTP 200 不作唯一证据，跨存储终态才算。

---

## 1. 体系总览

### 1.1 八套测试集

| Suite | 名称 | 关注点 | 执行分级 | 典型失败 |
|---|---|---|---|---|
| **01** | 基础功能测试 | 按钮显示、点击、触发及执行结果 | **P0** | 点了没反应 / 事件未触发 |
| **02** | 输入与数据验证 | 必填项、非法数据、边界值 | **P0** | 空提交入库 / 脏数据落库 |
| **03** | 状态与生命周期 | 执行中、成功、失败、取消、重试 | **P0** | 状态卡死 / 终态错误 |
| **04** | 权限与安全测试 | 角色权限、未授权、重复提交 | **P0** | 越权读写 / 审批绕过 |
| **05** | 异常与容错测试 | 网络中断、服务异常、超时 | P1 | 假成功 / 静默失败 |
| **06** | 并发与幂等测试 | 连续点击、多用户同时操作 | P1 | 重复建单 / 脏写覆盖 |
| **07** | 页面交互测试 | 刷新、导航、弹窗、反馈信息 | P1 | 刷新丢状态 / 弹窗误提交 |
| **08** | 完整业务流程测试 | 业务入口 → 结果落库 → 状态同步 | **P0** | 链路断点 / 前端假绿 |

> 01–04 + 08 为 **P0 必过**（发布卡口）；05–07 为 **P1**（发布前建议全过，例外需正式风险裁决）。

### 1.2 体系图

```
                     PM-AI E2E Testing Framework
┌────────────────────────────────────────────────────────────────────┐
│  E2E Test Runner        统一入口 / 执行 / 报告                     │
│  ├─ Functional Tests        (Suite 01–02)                          │
│  ├─ Business Flow Tests     (Suite 08 + 专属域)                     │
│  ├─ Permission Tests        (Suite 04)                             │
│  ├─ Exception Tests         (Suite 05)                             │
│  ├─ Concurrency Tests       (Suite 06)                             │
│  └─ Integration Tests       (Suite 03 + 07, 跨存储断言)            │
│                              ↓                                     │
│  Test Report          Pass / Fail / Blocked / Evidence / 失败分析  │
└────────────────────────────────────────────────────────────────────┘
        ↑ 断言三件套（缺一不可）                    ↓ 执行载体
┌────────────────────────┐          ┌──────────────────────────────────┐
│ 1. HTTP 状态码         │          │ Playwright（浏览器端 E2E）       │
│ 2. 业务 API 响应体     │          │ API 校验（真实后端业务结果）      │
│ 3. PostgreSQL 终态     │          │ PostgreSQL（持久化与数据一致性） │
└────────────────────────┘          │ Trace + Screenshot（失败现场）   │
                                    │ 独立测试数据（自动创建/清理）    │
                                    └──────────────────────────────────┘
```

### 1.3 套件与本仓载体的对应

| Suite | 主要落层 | 载体 | 本仓对应物 |
|---|---|---|---|
| 01 基础功能 | L3 UI | Playwright（MS Edge headless） | `ai-frontend/e2e/`、`ai-frontend/playwright.team.config.ts`（真后端栈；`playwright.config.ts` 是 S02 fixtures 演示态 :8848，发布态不跑） |
| 02 输入验证 | L1 + L3 | Rust 集成 + 浏览器 | `apps/pm-api/tests/` + 表单页 |
| 03 状态生命周期 | L1 + L2 + L3 | 状态机矩阵 + 真 router + 浏览器 | `crates/pm-domain/src/task.rs`、`task_transition.rs` |
| 04 权限安全 | L1 + L2 + L3 | 主体隔离断言 | `crates/pm-application/src/authz.rs`（`require_project_member`）、`apps/pm-api/src/support.rs`（`require_project_visible`）、`core.tenant_member` |
| 05 异常容错 | L2 + L4 | 故障注入 | `pm_execution.rs` 错误码面 |
| 06 并发幂等 | L2 + L3 | 多主体 / 多客户端 | **待建**（`apps/pm-api/tests/e2e_flow.rs` 现无并发/幂等用例，grep 计数 0；幂等先例 = `apps/pm-api/tests/imports.rs` 的 idempotency_key 用例） |
| 07 页面交互 | L3 | 浏览器 | `ai-frontend/src/router/` 导航测试 |
| 08 完整业务流程 | L2 + L3 | 端到端链路 | `test-case/e2e-flow-execution-report-v1.md`（FLOW-00~09） |

---

## 2. 被测对象模型（通用业务按钮）

所有套件的用例都可抽象为同一个模板对象，落地时**只替换按钮名与业务结果**：

```
① 触发面  按钮/入口   ──▶ ② 输入面  表单/参数   ──▶ ③ 结果面  状态机 + 落库
   （显示、可点、可达）      （必填、格式、边界）      （HTTP + API + DB）
```

### 2.1 PM-AI 的四个模板对象

| 模板对象 | 业务阶段 | 前端入口 | 接口族 | 权威状态/落点 |
|---|---|---|---|---|
| **创建项目** | S1 项目初始化 | `/project/create` | `POST /v1/pm/projects` | `core.project.status` |
| **提交需求** | S2 需求 | `/project/requirements` | `POST /v1/pm/projects/{id}/requirements`（修订链 `.../revisions` → `/submit` → `/confirm`） | `delivery.requirement` + `delivery.requirement_revision`（5 态：`draft/pending_confirmation/approved/rejected/superseded`） |
| **确认方案** | S3 设计 | `/project/design`、`/project/design/technical-designs` | `pm_designs` | `design.design` / `design.design_revision`（5 态，同需求）；`design.adr` 6 态；`plan.task_plan_revision` 5 态 |
| **启动任务** | S4/S5 计划执行 | `/project/tasks`、`/project/execution` | `POST /v1/pm/projects/{id}/tasks/mark-ready`、`/v1/pm/task-runs/{run_id}/commands` | `delivery.task`（13 态）、`delivery.task_run`（7 态：`queued/claimed/running/succeeded/failed/timed_out/cancelled`） |

### 2.2 三条断言纪律

1. **HTTP 200 不算通过** —— 必须同时校验响应体字段与 DB 终态。
2. **前端提示不算通过** —— 提示语是最弱的证据，DB 才是终态。
3. **状态机越权不算通过** —— 非法流转必须被拒（400/409）且 DB 终态不变。

---

## 3. 详细测试案例

> 以下为**通用模板**。落地时替换按钮名称、页面路径、接口与业务结果；每条落地用例的完整 11 字段见 §4.2。

### Suite 01 · 基础功能测试

| ID | 测试场景 | 操作步骤 | 预期结果 |
|---|---|---|---|
| TC-001 | 按钮正常显示 | 打开目标页面 | 按钮正确显示 |
| TC-002 | 按钮文案 | 检查按钮文字 | 文案与设计一致 |
| TC-003 | 正常点击 | 点击按钮一次 | 正确触发业务操作 |
| TC-004 | 点击反馈 | 点击按钮 | 有明确的加载或反馈状态 |
| TC-005 | 操作成功 | 完成正常操作 | 显示成功提示 |
| TC-006 | 操作后页面状态 | 操作成功后检查页面 | 页面数据正确更新 |
| TC-007 | 按钮可用性 | 检查正常状态 | 按钮可点击 |
| TC-008 | 键盘操作 | Tab 定位并按 Enter | 能正常触发操作 |

**PM-AI 落点**：`/project/create` 的「创建项目」按钮、`/project/tasks` 的「Mark Ready」（`POST /v1/pm/projects/{project_id}/tasks/mark-ready`）、`/project/execution` 的「启动/取消运行」（`/v1/pm/task-runs/{run_id}/cancel`）。

### Suite 02 · 输入与数据验证

| ID | 测试场景 | 操作步骤 | 预期结果 |
|---|---|---|---|
| TC-101 | 正常输入 | 输入合法数据并提交 | 操作成功 |
| TC-102 | 空数据 | 不输入必填内容直接提交 | 阻止提交并提示 |
| TC-103 | 超长输入 | 输入超过限制的数据 | 按规则限制或提示 |
| TC-104 | 特殊字符 | 输入特殊字符 | 正确处理，不发生异常 |
| TC-105 | 非法格式 | 输入不符合格式的数据 | 校验失败并提示 |
| TC-106 | 边界值 | 输入最小、最大允许值 | 按规则正确处理 |
| TC-107 | 重复数据 | 提交已存在的数据 | 按业务规则处理重复记录 |

**PM-AI 落点**：项目名 / 需求标题 / 仓库 URL（SSH 硬化：拒绝 `https://`、`file://`、`ext::`）/ 提案条目数上限（执行器单批 **200**，见 FLOW-04）。

### Suite 03 · 状态与生命周期测试

> 本套尤其重要：E2E 不能只验证按钮能不能点，还要验证点击之后整个业务状态是否正确。

| ID | 测试场景 | 操作步骤 | 预期结果 |
|---|---|---|---|
| TC-201 | 初始状态 | 打开页面 | 显示正确初始状态 |
| TC-202 | 执行中状态 | 点击按钮启动操作 | 显示执行中 |
| TC-203 | 成功状态 | 等待操作完成 | 显示成功 |
| TC-204 | 失败状态 | 模拟服务端失败 | 显示失败及原因 |
| TC-205 | 取消操作 | 执行中取消 | 按业务规则进入终态 |
| TC-206 | 再次执行 | 失败后点击重试 | 正确重新执行 |
| TC-207 | 页面刷新 | 操作执行中刷新页面 | 能恢复真实任务状态 |
| TC-208 | 状态一致性 | 检查前端与后端数据 | 状态一致 |

**PM-AI 落点**：`delivery.task` 13 态流转（`draft→ready→claimed→running→submitted/reviewing→completed`）、`delivery.task_run` 7 态（`queued/claimed/running/succeeded/failed/timed_out/cancelled`）、`integration.repo_import` 9 态（`queued→cloning→reading→analyzing→draft_ready→proposed→applied`）。

### Suite 04 · 权限与安全测试

| ID | 测试场景 | 操作步骤 | 预期结果 |
|---|---|---|---|
| TC-301 | 有权限用户 | 授权用户点击按钮 | 操作成功 |
| TC-302 | 无权限用户 | 无权限用户尝试操作 | 操作被拒绝 |
| TC-303 | 未登录用户 | 清除登录状态后请求 | 跳转登录或返回 401 |
| TC-304 | 权限变化 | 操作前撤销用户权限 | 后端拒绝操作 |
| TC-305 | 跨项目操作 | 尝试操作其他项目的数据 | 后端拒绝越权 |
| TC-306 | 直接 API 调用 | 绕过前端直接请求接口 | 后端仍执行权限校验 |

**PM-AI 落点**：跨租户隔离（FLOW-08）、项目归档越权（FLOW-09）、确认类命令的真人门 `ensure_trusted_context`。

### Suite 05 · 异常与容错测试

| ID | 测试场景 | 操作步骤 | 预期结果 |
|---|---|---|---|
| TC-401 | 网络断开 | 点击后断开网络 | 提示网络异常 |
| TC-402 | 请求超时 | 模拟 API 超时 | 正确提示超时 |
| TC-403 | 服务端 500 | 模拟服务器异常 | 提示服务异常 |
| TC-404 | 服务端 400 | 提交非法请求 | 显示业务错误 |
| TC-405 | 登录过期 | 操作时使 Token 失效 | 正确处理认证过期 |
| TC-406 | 服务恢复 | 故障恢复后重试 | 操作可以正常完成 |
| TC-407 | 响应丢失 | 服务端已成功但客户端未收到响应 | 不产生重复业务结果 |

**PM-AI 落点**：导入失败的 `error_code`（如 `ssh_key_missing`）、`ALREADY_ARCHIVED` 409、审批 409 幂等冲突。

### Suite 06 · 并发与幂等测试

| ID | 测试场景 | 操作步骤 | 预期结果 |
|---|---|---|---|
| TC-501 | 连续快速点击 | 100ms 内连续点击 | 不产生重复操作 |
| TC-502 | 双击 | 快速双击按钮 | 不重复创建业务对象 |
| TC-503 | 多用户同时操作 | 两个用户同时点击 | 按业务规则处理冲突 |
| TC-504 | 重复请求 | 使用相同请求标识重复调用 | 幂等结果一致 |
| TC-505 | 操作竞争 | 两个请求修改同一数据 | 不出现数据丢失 |
| TC-506 | 并发刷新 | 多个客户端查看执行状态 | 展示一致 |

**PM-AI 落点**：导入批次拆分上限（DEF-E2E-01）、任务认领租约、审批哈希门禁（FLOW-05）。

### Suite 07 · 页面交互测试

| ID | 测试场景 | 操作步骤 | 预期结果 |
|---|---|---|---|
| TC-601 | 弹窗触发 | 点击按钮 | 正确打开弹窗 |
| TC-602 | 弹窗取消 | 点击取消 | 不产生业务变更 |
| TC-603 | 弹窗确认 | 点击确认 | 正常提交 |
| TC-604 | 页面跳转 | 点击操作按钮 | 跳转正确页面 |
| TC-605 | 浏览器后退 | 操作后点击后退 | 页面状态正确 |
| TC-606 | 浏览器刷新 | 刷新当前页面 | 数据正确恢复 |
| TC-607 | 重复打开页面 | 多次进入页面 | 不出现重复数据或异常 |
| TC-608 | 加载状态 | 模拟慢响应 | 按钮状态及反馈合理 |

**PM-AI 落点**：路由守卫与未建团分流（`/auth/signing-in → /onboarding`）、`ai-frontend/src/router/navigation.ts` 导航行为。

### Suite 08 · 完整业务流程测试

> 本套最有价值：验证用户从业务入口开始，经按钮触发，**最终确实完成业务目标**。以 PM-AI 项目创建为例。

| ID | 测试场景 | 操作步骤 | 预期结果 |
|---|---|---|---|
| TC-801 | 创建项目 | 进入项目列表，点击创建项目 | 进入创建流程 |
| TC-802 | 提交项目 | 填写项目名称并提交 | 项目成功创建 |
| TC-803 | 项目初始化 | 检查初始化流程 | 初始化事项派生状态正确生成（Command Center 实时判定，**无独立初始化任务表**） |
| TC-804 | AI-PM 初始化 | 检查项目 AI 配置 | 内置 AI-PM 正常可用 |
| TC-805 | 项目详情 | 点击进入项目 | 正确进入 Project Command Center |
| TC-806 | 数据持久化 | 刷新页面 | 项目数据仍然存在 |
| TC-807 | 项目隔离 | 使用其他项目成员访问 | 遵循项目权限规则 |
| TC-808 | 完整流程 | 从创建到进入项目控制台 | 全流程成功，无异常 |

**PM-AI 落点**：`/projects → /project/create → /project/command-center`，后端 `POST /v1/pm/projects` → `core.project` 新增 1 行 → `core.project_member`（含 `role='ai_pm'` 的 AI 主体）→ `ai.project_ai_config` + `ai.project_ai_role` 内置 AI-PM 就绪。（注：初始化展示态由 `crates/pm-application/src/command_center.rs` 的 `init_judgement` **派生**，非独立表状态；`core.project_initialization_operation` 是 import 占用锁，与初始化事项无关。）

---

## 4. E2E 测试的质量标准

### 4.1 案例必备字段

每条 E2E 案例**必须**包含以下 11 个字段，不接受「一句话测试描述」。

> **三套分级词汇表（严禁混用）** —— 同一份工单里三种「P0」长得一样是真实踩坑点：
>
> | 词汇 | 含义 | 用在哪 | 判定 |
> |---|---|---|---|
> | **Must / Should / Could** | **案例优先级**（本节 Priority 字段） | 每条用例 | Must = 发布必过；Should = 尽量过；Could = 择机 |
> | **P0 / P1 / P2** | **执行分级 / 发布批次**（§5） | 套件与跑批计划 | P0 批 = 发布执行全量；P1 批 = 相关功能子集 |
> | **S0–S3** | **缺陷严重度**（§5.1 脚注） | 缺陷单 | S0/S1 未清零禁止发布 |
>
> 映射关系：Must 案例构成 P0 执行批；Should 构成 P1 批；Could 归 P2 批。套件名前缀「P0-xx」（如 P0-04）指**发布门禁套件集**，不是案例优先级。


| 字段 | 说明 | 必填 |
|---|---|---|
| **Case ID** | 唯一测试编号 | ✅ |
| **Suite** | 所属测试集（01–08） | ✅ |
| **Priority** | 案例优先级：**Must / Should / Could**（MoSCoW，见下方词汇表；不用 P0/P1/P2，避免与执行分级混淆） | ✅ |
| **Preconditions** | 测试前置条件（登录态、数据、依赖服务） | ✅ |
| **Test Data** | 测试数据（账号、项目、参数值） | ✅ |
| **Steps** | 操作步骤 | ✅ |
| **Expected Result** | 预期结果（含 HTTP 码 / API 字段 / DB 终态） | ✅ |
| **Actual Result** | 实际结果（执行后回填） | ✅ |
| **Status** | Pass / Fail / Blocked | ✅ |
| **Evidence** | 截图、日志、Trace | ✅ |
| **Cleanup** | 测试后清理要求 | ✅ |

### 4.2 填写范例（创建项目）

| 字段 | 内容 |
|---|---|
| Case ID | TC-802 |
| Suite | 08 完整业务流程 |
| Priority | Must |
| Preconditions | PM-AI 已启动（`127.0.0.1:16433` PG + pm-api + vite `VITE_USE_MOCK=0`）；已登录且属于某租户；`tid` 唯一 |
| Test Data | 项目名 `e2e-proj-<随机后缀>`；**必填 `code` + `name`**（`apps/pm-api/src/pm_lists.rs:139-147`）；仓库 `git@192.168.2.16:tooling/ai-pm.git` 为**可选**（建项目不依赖 Git / Runner / 外部 AI 工具） |
| Steps | ① 打开 `/projects` ② 点「创建项目」③ 填名称 ④ 提交 ⑤ 进入 `/project/command-center` ⑥ 刷新 |
| Expected Result | HTTP 201；响应含 `id`；`core.project` 新增 1 行且 `status` 合法；页面渲染项目名；刷新后数据仍在；DB 行数 = 1（无重复） |
| Actual Result | （执行后回填） |
| Status | （执行后回填：Pass / Fail / Blocked） |
| Evidence | `test-case/evidence/<batch>/` 截图 + Trace + 服务端日志片段 |
| Cleanup | 归档测试项目（`POST /v1/pm/projects/{id}/archive`，**无删除端点**）；清理 `_sqlx_test_%` 残留库 |

### 4.3 三条原则

1. **不能只检查页面有没有提示成功** —— 必须检查数据库或真实业务 API 的结果。
2. **测试数据必须隔离** —— 避免测试之间相互污染（独立 `tid` / 独立项目名后缀 / 一次性库）。
3. **测试失败必须保留现场** —— 截图、浏览器 Trace、网络请求、服务端日志，方便定位。

---

## 5. 测试执行分级

| 级别 | 定位 | 内容 | 执行时机 |
|---|---|---|---|
| **P0** | 核心回归 · **必须通过** | 登录、权限、核心业务创建、状态变更、数据持久化、完整业务链路 | 每次发布执行 P0 全量 |
| **P1** | 功能回归 · 建议通过 | 异常恢复、并发、幂等、页面刷新、重复提交 | 日常开发跑相关功能 P0/P1 子集 |
| **P2** | 体验回归 · 持续优化 | 键盘操作、交互反馈、特殊输入、显示兼容性 | 迭代期择机 |

### 5.1 与本仓发布门禁的挂接

| 门 | 触发 | E2E 相关要求 |
|---|---|---|
| G-commit | 每次 push | 不含 E2E；L0+L1 + 前端 vitest + release-guard 全绿 |
| G-merge | 合并到 master 前 | L2 冒烟（接一个真实仓库）全绿 |
| G-release | 发布候选 | **L2 全量 + L3 全量 + L4 关键路径全绿，零 S0/S1 缺陷** |
| G-hotfix | 线上 P0 修复 | 该 P0 的专项回归用例全绿 |

> **缺陷分级改记 S0–S3**（与用例优先级 P0/P1/P2、执行分级 P0/P1/P2 是三个不同概念，严禁混用）：S0 安全 / 数据完整性 / 审批边界失效 / 系统不可启动 —— 禁止发布；S1 核心流程失败 / 权限越界 / 重要数据不一致 —— 禁止发布（除非正式风险裁决）；S2 非核心功能异常；S3 UI / 文案 / 体验。S0–S3 与上位文件 `full-lifecycle-test-plan-v1.md` §5 的缺陷 P0–P3 同义（对照：旧P0→S0、旧P1→S1、旧P2→S2、旧P3→S3）；历史记录（如 DEF-E2E-01 标 P1）沿用旧记法，不回改。

---

## 6. PM-AI 专属测试集

PM-AI 是多项目、多仓库、多 Agent 的系统，通用套件之外必须增加专属套件，且需结合**真实业务状态机**而非模拟前端点击。

| 专属套件 | 重点测试内容 | 主要落层 |
|---|---|---|
| **Team** | 创建团队、邀请成员、角色权限 | L1 + L3 |
| **Project** | 项目创建、初始化、项目隔离 | L1 + L2 + L3 |
| **Repository** | 多仓库关联、Git 授权、仓库同步 | L2（真 Git over SSH） |
| **AI-PM** | AI 分析、知识建议、人工确认 | L1 + L3 |
| **Task** | 任务创建、分配、执行、交付、Review | L1 + L2 + L3 |
| **Agent** | 多 Agent 并行、结果汇总、失败恢复 | L2 + L4 |
| **Knowledge** | Markdown 写入、版本管理、索引重建 | L1 + L2 |
| **ACP** | 人工介入、暂停、恢复、继续执行 | L2 + L4 |
| **MCP** | 外部工具连接、任务拉取、结果回传 | L2 + L4 |

> **Agent 执行与人工介入（ACP）必须验证整条执行链路**，不能只验前端点击：`claimed → running → awaiting_decision → submitted → reviewing → completed` 全程留有事件、提交与裁决证据。

---

## 7. 技术实现

| 关注点 | 方案 |
|---|---|
| 浏览器端 E2E 自动化 | **Playwright**（MS Edge headless，1440×900，`locale: zh-CN`） |
| API 验证 | 检查真实后端业务结果（HTTP 码 + 响应体 + 业务字段） |
| 持久化验证 | **PostgreSQL** —— 校验落库与数据一致性（`core` / `delivery` / `design` / `plan` / `integration` / `governance` / `ai`） |
| 失败现场 | **Trace + Screenshot** + 网络请求 + 服务端日志 |
| 独立测试数据 | 自动创建、清理测试项目与测试账号（独立 `tid` / 独立后缀命名） |

### 7.1 本仓已有实现（不自研第二套）

| 载体 | 路径 | 说明 |
|---|---|---|
| L2 服务级 E2E | `apps/pm-api/tests/e2e_flow.rs` | 真 axum router + 真 PostgreSQL 临时库 + 真 Git 远端（over SSH） |
| L3 UI E2E | `ai-frontend/e2e/`、`ai-frontend/playwright.team.config.ts` | Playwright + 真 pm-api + 真 vite（`VITE_USE_MOCK=0`；`playwright.config.ts` 是 S02 fixtures 演示态，不作为 L3 载体） |
| 一键复跑 | `test-case/evidence/e2e-flow/run-e2e-flow.sh` | 环境前置检查 → 用例执行 → 证据落盘 |

### 7.2 红线

- **L2/L3 不得用 mock / fixture 冒充后端。** `ai-frontend/build/release-guard.ts` 在 production/staging 构建期硬失败；fixtures/demo 仅允许本地 dev（`VITE_USE_MOCK=1`）与单测动态加载。
- **禁止在浏览器侧重新实现后端核心逻辑**（如 AES-256-GCM 密钥信封）：两套实现漂移 = 假绿或假红。需要复用平台逻辑时，走平台自己的接口。
- **连 Docker 映射端口一律写 `127.0.0.1`，禁止 `localhost`**（本机 IPv6 回环是黑洞）。
- **不写为「旧数据 / 旧格式 / 旧行为」服务的迁移、双写、兼容分支**（AGENTS.md 第 1 条）。
- 测试进程被杀会泄漏 `_sqlx_test_%` 库 —— 跑完必须清理。

---

## 8. 判定口径速查

| 问题 | 判定 |
|---|---|
| 页面提示成功，但 DB 无记录 | **FAIL**（假绿） |
| HTTP 200，但响应体字段不符合契约 | **FAIL** |
| HTTP 4xx/5xx 且错误码符合契约 | **PASS**（负向用例） |
| 状态机非法流转未被拒 | **FAIL**（P0） |
| 用例无法执行 | **BLOCKED** + 写明原因，禁止标 PASS |
| S0/S1 缺陷未清零 | **禁止发布** |

---

## 9. 关联文档

| 文件 | 内容 |
|---|---|
| `docs/e2e-spec/README.md` | 本目录索引、边界与待展开清单 |
| `test-case/full-lifecycle-test-plan-v1.md` | 全仓 L0–L4 载体、业务流覆盖矩阵、状态机基准、发布门禁 |
| `test-case/e2e-test-plan-v1.md` | L2/L3 落地计划、SSH 三层通路、用例矩阵、执行方式 |
| `test-case/e2e-flow-execution-report-v1.md` | E2E 执行报告（7/7 PASS + DEF-E2E-01/02） |
| `test-case/team-qa-test-plan-v1.md` | Team Onboarding 专项（专属套件范例） |
| `docs/test-environments.md` | 环境清单与端点 |
| `test-case/README.md` | 用例目录约定 |
