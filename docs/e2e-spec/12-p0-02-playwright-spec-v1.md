# AI-PM · P0-02 Team 与 Project 基础管理 · Playwright E2E 规格 V1.0

> **性质**：P0-02 套件的**可执行**自动化规格（4 条案例：P0-004 ~ P0-007）。
> **上位文件**：`docs/e2e-spec/10-p0-test-specification-v1.md`（P0 总规格）、`docs/e2e-spec/11-p0-01-playwright-spec-v1.md`（P0-01 规格，本套件沿用其栈与纪律）。
> **复用基建**：`ai-frontend/e2e/team/stack.ts`（自签 JWKS + 真 pm-api + 一次性库）。
> **执行配置**：`ai-frontend/playwright.team.config.ts`（vite :8850 + `PM_API_URL` + `VITE_USE_MOCK=0` + MS Edge）。
> **基线**：master @ `2b3f509e`；迁移基线 `migrations/0001_baseline.sql`。
> **判定纪律**：UI 状态 + 直连 pm-api 终态**双证据**；HTTP 200 不作唯一证据。

---

## 0. 评审结论速览（先读这节）

规格原稿的**业务意图全部成立**，但 13 处断言锚点与本仓实现不符，必须在落地前改写（R66~R68 为 2026-11-10 P0 套件落地实测新增，编号续接全局序列避让 18 号文档 R57~R64，套件 `ai-frontend/e2e/p0-02-team-project.spec.ts` 7/7 全绿；18 号 V1.1 新增发现让位编为 R69/R70）：

| # | 级别 | 规格原文 | 本仓事实 | 处置 |
|---|---|---|---|---|
| **R11** | 🔴 | 创建后 `toHaveURL(/projects\/.+/)`；`goto('/projects/${id}')` | **无 `/projects/:id` 路由**。真实跳转 = `/project/command-center?project_id=<uuid>`（`ai-frontend/src/views/project/create/ProjectCreateView.vue:123-126`）；列表页是 `/projects` | 改断言 URL；详情页一律带 `project_id` query |
| **R12** | 🔴 | `project.team_id` 对应 Team A | **无 `team_id` 列**。Team ≡ `core.tenant`，归属列是 `core.project.tenant_id`（`migrations/0001_baseline.sql:1251-1262`）；项目成员在 `core.project_member`（role ∈ `owner/member/ai_pm/architect/developer/tester`，`:1287-1293`） | 后端断言改 `tenant_id` + `project_member` owner 行 |
| **R13** | 🔴 | `getByLabel(/project name/i)`、`/description/i` | 表单**无 id / html-for / data-testid**，标签是 zh-CN i18n（`ai-frontend/locales/zh-CN/s01/project-create.yaml`）：`项目名称` / `项目标识（Key）` / `项目描述` | 选择器改中文标签，或**先补 data-testid**（推荐） |
| **R14** | ⚠️ | 只填名称即可创建 | `code`（Key）**必填**，空则 400 `"project code and name are required"`（`apps/pm-api/src/pm_lists.rs:157-161`）；名称自动建议 Key = 首词大写字母数字截 10 位（`ProjectCreateView.vue:57-68`），`e2e-project-a1` ⇒ `E2EPROJECTA`；Key 重复 → 409 `project code \`X\` already exists`（`pm_lists.rs:234-236`） | 用例显式填 Key；数据命名带唯一后缀防撞号 |
| **R15** | ⚠️ | — | 创建响应 = **201 + ProjectRow**（`id/code/name/description/status/revision/customer_id/…`），SQL 带 `JOIN core.customer`；无 `customer_id` 时 bootstrap 为租户建**默认客户**（`crates/pm-application/src/bootstrap.rs:204-235`） | 默认客户是**租户级共享**，跨用例不隔离 → 只作创建前置，不作断言对象 |
| **R16** | ⚠️ | 「六种初始化状态」 | 实为 **7 态**：`not_started / in_progress / waiting_for_human / completed / blocked / not_applicable / undetermined`（`crates/pm-application/src/command_center.rs:74-83`） | `allowedStates` 补 `waiting_for_human` |
| **R17** | ✅ | 「InitItem 是派生 Read Model，不是独立实体」 | **成立且更强**：无 InitItem 表；`InitializationProgress { items, required_items }` 由 `init_judgement` 纯函数算出（`command_center.rs:98-146,314-317`） | 保留规则；读取路径写实为 `GET /v1/pm/projects/{id}/command-center` |
| **R18** | ⚠️ | `getByTestId("project-initialization")` | **无此 testid**；初始化块以文本呈现（`s01/command-center.yaml:21-26`：`项目初始化`、`必需 {done} / {total}`、`已完成/未完成/无法判定`） | 用文本选择器或补 testid |
| **R19** | ⚠️ | 越权期望 `[403, 404]` | 本仓口径：单体资源不可见一律 **404**（防存在性泄露，`apps/pm-api/src/support.rs:178-208`）；**集合型读是过滤而非报错** —— 200 + 空集（`pm_lists.rs:271-277` 按 `project_member` JOIN 过滤） | 分场景写死期望码，见 §6.3 口径表 |
| **R66** | 🔴 | P0-004 示例 `api("GET", \`/v1/pm/projects/${projectId}\`)` 读单体项目 | **无 `GET /v1/pm/projects/{id}` 路由**（`pm_lists.rs` 仅注册列表与子资源路由） | 示例调用 404 → 改用列表过滤（`GET /v1/pm/projects` 按 code 匹配）+ DB 直查 |
| **R67** | 🔴 | 需求创建响应取 `created.body.id`；跨项目写 body `{title}` | 创建响应为 **`{requirement: {...}}` 包裹**（`pm_requirements.rs`，与 L2 `e2e_flow.rs` 一致）；`DraftRevisionRequest` **必填 `markdown`**（`requirement.rs:144-153`，无 title 字段） | 示例取不到 id / 请求 400 → `reqId = created.body.requirement.id`；写 body 用 `{markdown}` |
| **R68** | 🟠 | R19/D9「集合型读跨项目 = 200 + 空集」；P0-006 示例 `cc.body.initialization.items` | ① 带 `project_id` 过滤的集合读**先过 `require_project_visible`**（`pm_lists.rs:261-269`）→ 非成员 **404**；「200+空集」只适用于**不带过滤**的全列表（membership JOIN）。② CC 响应块是 **`Block<T>` 包装**（`command_center.rs:231-239`）→ 数据在 `blocks.initialization_progress.data.items` | 隔离语义比规格声称的**更强**；CC 取值路径少一层 → 过滤读非成员写死 404 + 不带过滤全列表断言不含他项目数据；CC 断言走 `.data.items`（附 `status==='ok'`） |
| **R20** | ⚠️ | 目录 `tests/e2e`、`fixtures`、`helpers` | 现仓为 `ai-frontend/e2e/`（Playwright `testDir`）+ `ai-frontend/e2e/team/stack.ts` | 映射见 §9 |

**初始化事项的固定 key（7 个，可作断言锚点）**：`project_basics` / `project_goal` / `repository_access` / `code_analysis` / `project_cognition` / `dev_environment` / `human_actions`（`command_center.rs:778-848`，标题为中文「项目基本信息/项目目标/仓库接入/…」）。

---

## 1. 测试范围与验收目标

| 项 | 值 |
|---|---|
| **P0 Release Gate** | ✅ 是 |
| **测试案例** | 4（P0-004 ~ P0-007） |
| **Team** | 2（Team A / Team B） |
| **Project** | 3（A1 / A2 / B1） |

**六条业务边界 → 断言锚点**

| 业务边界 | 断言锚点 |
|---|---|
| Team 是项目的组织归属 | `core.project.tenant_id` = Team A.id（Team ≡ `core.tenant`） |
| Project 必须属于 Team | `core.project.tenant_id NOT NULL` + 创建时由调用者租户决定（`pm_lists.rs:171-173`） |
| 同 Team 可有多个 Project | Team A 下 `core.project` 2 行，`tenant_id` 相同、`id` 不同 |
| 创建后立即进入初始化流程 | 创建响应 201 → 跳转 `/project/command-center?project_id=` → `initialization.items` 非空 |
| 不依赖 Git / Runner / 外部 AI 工具 | `CreateProjectBody` 无相关字段；`bootstrap` 仅写 `ai.project_ai_config`（provider 默认 `'fake','fake'`，`bootstrap.rs:296-299`）；`ai.project_ai_role` 自动补齐（`bootstrap.rs:319`） |
| 不同 Project 数据隔离 | 列表按 `project_member` JOIN 过滤；单体资源不可见 404 |

---

## 2. 测试环境与 Fixture

### 2.1 环境矩阵（复用 P0-01 栈）

| 组件 | 地址 | 来源 |
|---|---|---|
| 浏览器 | MS Edge headless，1440×900，`locale: zh-CN` | `ai-frontend/playwright.team.config.ts` |
| 前端 | vite 独立实例，`VITE_USE_MOCK=0` | `127.0.0.1:8850` |
| 后端 | 真 pm-api（jwks 模式） | `127.0.0.1:18490` |
| JWKS | 自签 RS256 | `127.0.0.1:18491` |
| PG | 容器 `aipm-teamqa-pg`，一次性空库 | `127.0.0.1:16499` |

### 2.2 测试数据

| 数据 | 名称 | 用途 | 归属租户 |
|---|---|---|---|
| User A | `test-owner-a` | Team A Owner | tid A |
| User B | `test-owner-b` | Team B Owner（隔离对手方） | tid B |
| Team A | `e2e-team-a` | 主测试团队 | tid A |
| Team B | `e2e-team-b` | 隔离测试团队 | tid B |
| Project A1 | `e2e-project-a1`（Key `E2EPROJECTA`） | 主测试项目 | Team A |
| Project A2 | `e2e-project-a2`（Key `E2EPROJECTA2`） | 同 Team 第二项目 | Team A |
| Project B1 | `e2e-project-b1`（Key `E2EPROJECTA3`） | 跨 Team 隔离项目 | Team B |

> **Key 唯一性**：Key 在租户内唯一（409 冲突）。测试名必须带**唯一后缀**（用例序号/随机串），否则跨用例撞号。
> **默认客户**：`core.customer` 默认行是**租户级共享**，不作隔离断言对象（R15）。

### 2.3 Fixture 契约

```ts
// ai-frontend/e2e/p0/fixtures.ts
import { api, claimTeam, signToken } from "../team/stack";

export interface TestUser {
  sub: string;
  tid: number;
  token: string;
}

export interface TestTeam {
  id: string;      // = core.tenant.id
  name: string;
  slug: string;
}

export interface TestProject {
  id: string;
  code: string;
  name: string;
  tenantId: string;
}

export interface ProjectTestContext {
  userA: TestUser;
  userB: TestUser;
  teamA: TestTeam;
  teamB: TestTeam;
  projectA1?: TestProject;
  projectA2?: TestProject;
  projectB1?: TestProject;
}
```

**Fixture 职责与纪律**

| # | 职责 | 实现 | 纪律 |
|---|---|---|---|
| 1 | 创建测试用户 | `signToken(sub, tid)` 造主体（jwks 模式） | 不手工插 `core.principal` |
| 2 | 创建 Team | `claimTeam(sub, tid, name, slug)` → `POST /v1/pm/teams` | 走真实建团链路，owner 由服务端写 |
| 3 | 建立 Membership | 由建团事务自动写 `core.tenant_member`（role=`owner`） | **不手工插** |
| 4 | 创建 Project | `POST /v1/pm/projects { code, name, description? }` → 201 | 走真实链路，`code` 必填（R14） |
| 5 | 保存对象 ID | Fixture 返回全部 `id`（tenant_id / project_id） | 断言只用 ID，不用名字 |
| 6 | 清理 | 一次性库销毁（`TEAM_QA_KEEP_DB=1` 可保留排查） | 清理残留 `_sqlx_test_%` 库 |

> **禁止**：直接改生产业务表伪造用户操作（AGENTS.md 第 1 条：无存量数据、契约即真相）。

---

## 3. P0-004 · 正常创建项目

**测试目标**：用户能在当前 Team 下正常创建 Project，Team 归属正确。

| Step | 操作 | 预期结果 | 真实锚点 |
|---|---|---|---|
| 1 | 进入 Projects 页面 | 显示当前 Team 的项目 | 路由 `/projects`；标题「项目」 |
| 2 | 点击「新建项目」 | 进入创建页 | 按钮文案 `新建项目`（`pm.projects.new`）→ `router.push('/project/create')`（`ProjectListView.vue:179-186`） |
| 3 | 输入项目名称 | 接受输入 | 标签 `项目名称` |
| 4 | 填写项目标识（Key） | 接受输入（默认由名称建议） | 标签 `项目标识（Key）`，占位「例如 ATLAS」 |
| 5 | 填写项目描述（可选） | 接受输入 | 标签 `项目描述`，占位「这个项目解决什么问题？可选」 |
| 6 | 点击「创建项目」 | 提交成功 | 按钮 `创建项目`（`s01.project-create.createLabel`） |
| 7 | 检查跳转 | 进入 Command Center | `/project/command-center?project_id=<uuid>` |
| 8 | 查询后端项目记录 | Project 正确创建 | `POST /v1/pm/projects` → **201** + ProjectRow |
| 9 | 检查 Team 关联 | 项目归属 Team A | `core.project.tenant_id` = Team A.id；`core.project_member` 有 owner 行 |

```ts
test("P0-004 创建项目并正确归属 Team", async ({ page, context }) => {
  const a = await buildOwner("test-owner-a", 9101, "e2e-team-a");

  await signIn(context, a.sub, a.tid);
  await page.goto("/projects");

  await page.getByRole("button", { name: "新建项目" }).click();
  await expect(page).toHaveURL(/\/project\/create/);

  await page.getByLabel("项目名称").fill("e2e-project-a1");
  await page.getByLabel(/项目标识/).fill("E2EPROJECTA");
  await page.getByLabel("项目描述").fill("E2E project creation test");

  await page.getByRole("button", { name: "创建项目" }).click();

  // 创建后立即进入 Command Center（R11：不是 /projects/:id）
  await expect(page).toHaveURL(/\/project\/command-center\?project_id=/);

  const projectId = new URL(page.url()).searchParams.get("project_id")!;

  // 后端断言：201 + 归属 Team A
  const row = await api("GET", `/v1/pm/projects/${projectId}`, a.token);
  expect(row.status).toBe(200);
  expect(row.body.tenant_id).toBe(a.team.id);   // R12：无 team_id，归属列是 tenant_id
  expect(row.body.code).toBe("E2EPROJECTA");
});
```

**后端断言**

| 断言 | 口径 |
|---|---|
| 创建响应 | 201；body 含 `id` / `code` / `name` / `status`（默认 `'planning'`）/ `revision` |
| Team 归属 | `core.project.tenant_id` = Team A.id（≠ NULL） |
| 成员 | `core.project_member` 中该 project 有 `role='owner'` 行（创建者）+ `role='ai_pm'` 行（AI-PM 主体） |
| 错误分支 | 缺 `code` → 400 `project code and name are required`；重复 `code` → 409 |

---

## 4. P0-005 · 创建后立即进入 Project Command Center

**测试目标**：创建完成即进入 Command Center，**不要求**先配置 Repository / Runner / MCP / Codex / Claude Code。

> **产品基线**（实现已内置）：创建表单本身即声明「仓库 OAuth / AI-PM 认知 / Codex·MCP 引导 —— 不属于本表单」「它们都不会阻塞项目创建」（`s01/project-create.yaml` `deferredTitle/deferredBody/afterBody`）。

| Step | 操作 | 预期结果 | 真实锚点 |
|---|---|---|---|
| 1 | 创建新项目 | 创建成功 | 201 |
| 2 | 等待创建流程结束 | **不要求**配置 Git | 无仓库仍可进入 |
| 3 | 等待页面跳转 | 自动进入 Command Center | `/project/command-center?project_id=` |
| 4 | 检查项目标题 | 显示正确项目名称 | 页面标题 |
| 5 | 检查 Dashboard | 正常加载 | `GET /v1/pm/projects/{id}/command-center` → 200 |
| 6 | 检查 AI-PM 入口 | AI-PM 可用 | 路由 `/project/assistant`（`ai-frontend/src/router/index.ts`）；后端 `ai.project_ai_config` + `ai.project_ai_role` 存在 |
| 7 | 检查初始化进度 | 显示初始化事项 | 文本「项目初始化」「必需 {done} / {total}」 |
| 8 | 检查外部工具配置 | 未配置状态正常，不阻断 | 无仓库/Runner 时 `repository_access` 等项为合法未完成态 |

```ts
test("P0-005 无 Git/Runner 仍立即进入 Command Center", async ({ page, context }) => {
  const a = await buildOwner("test-owner-a", 9102, "e2e-team-a");
  await signIn(context, a.sub, a.tid);

  await page.goto("/project/create");
  await page.getByLabel("项目名称").fill("e2e-command-center");
  await page.getByLabel(/项目标识/).fill("E2E-CM-9102");
  await page.getByRole("button", { name: "创建项目" }).click();

  await expect(page).toHaveURL(/\/project\/command-center\?project_id=/);

  // 初始化进度可见（R18：无 testid，用文本）
  await expect(page.getByText("项目初始化")).toBeVisible();

  // 无强制配置仓库的阻断
  await expect(page.getByText(/必须.*仓库|required.*repository/i)).toHaveCount(0);

  // 后端：项目可访问 + AI 配置就绪 + 无仓库也无 Runner
  const cc = await api(
    "GET",
    `/v1/pm/projects/${projectId}/command-center`,
    a.token
  );
  expect(cc.status).toBe(200);
  expect(cc.body.initialization.items.length).toBeGreaterThan(0);
});
```

**后端断言**

| 断言 | 口径 |
|---|---|
| Project 创建 | 201；`core.project.status='planning'` |
| AI 配置就绪 | `ai.project_ai_config` 1 行（`enabled`、`automation_mode='ai_lead'`）、`ai.project_ai_role` 含内置角色行 |
| 无 Git 仍可用 | `integration.project_repository` 0 行时项目仍返回 200 |
| 无 Runner 仍可用 | `devenv.device` 无绑定；AI-PM 基础功能不依赖 Runner（`AI_PM_RUN_EXECUTOR` 缺省 `NoopExecutor`，只影响任务执行） |
| 无外部工具仍可用 | 未接 MCP / Codex 时需求分析链退确定性 FakeModel（`crates/pm-ai/src/analysis_model.rs:110-150`） |

> ⚠️ **不要把「没有 Git / Runner」判为项目初始化失败。** `repository_access` / `code_analysis` / `dev_environment` 处于 `not_started` / `not_applicable` / `undetermined` 是**合法初始状态**；`project_basics` 恒为 `completed`（`command_center.rs:778-783`）。

---

## 5. P0-006 · 项目初始化状态正确

**测试目标**：Command Center 的初始化进度正确反映真实业务状态。

> **架构规则（规格原文正确，保留）**：InitItem 是**派生的 Read Model**，不是独立业务实体；数据库**不存在** InitItem 表；不得把展示状态当作数据库业务实体状态。

**初始化状态定义（修正为 7 态，R16）**

| 状态 | 含义 |
|---|---|
| `not_started` | 尚未开始 |
| `in_progress` | 正在进行 |
| `waiting_for_human` | 系统已交出球，等待人类行动 |
| `completed` | 已完成 |
| `blocked` | 被阻塞 |
| `not_applicable` | 不适用 |
| `undetermined` | 尚无法确定 |

**7 个初始化事项与真实来源（权威源 = 领域表，不是 InitItem）**

| InitItem key | 标题 | 权威来源 |
|---|---|---|
| `project_basics` | 项目基本信息 | `core.project`（源态 `'planning'` → 恒 `completed`） |
| `project_goal` | 项目目标 | 目标相关计数（present / human_pending / pending） |
| `repository_access` | 仓库接入 | `integration.project_repository.status` |
| `code_analysis` | 代码分析 | `integration.repo_import.status`（`queued/cloning/reading/analyzing` → in_progress；`draft_ready/proposed` → waiting_for_human；`applied` → completed；`failed/rejected/cancelled` → blocked） |
| `project_cognition` | 项目认知 | `discovery.discovery_session.status` + `discovery.discovery_finding.verification_status` |
| `dev_environment` | 开发环境 | `devenv.device.readiness` |
| `human_actions` | 人工处理 | `delivery.task.status` |

```ts
/**
 * P0-006 初始化状态来自真实业务数据，页面与 Read Model 一致
 * 真实来源：GET /v1/pm/projects/{id}/command-center → initialization.items[]
 */
const INIT_KEYS = [
  "project_basics",
  "project_goal",
  "repository_access",
  "code_analysis",
  "project_cognition",
  "dev_environment",
  "human_actions"
];
const INIT_STATUSES = [
  "not_started",
  "in_progress",
  "waiting_for_human",
  "completed",
  "blocked",
  "not_applicable",
  "undetermined"
];

test("P0-006 初始化事项派生状态正确且与 Read Model 一致", async ({
  page,
  context
}) => {
  const a = await buildOwner("test-owner-a", 9103, "e2e-team-a");
  const project = await createProject(a, "e2e-init-state", "E2EINIT9103");

  await signIn(context, a.sub, a.tid);
  await page.goto(`/project/command-center?project_id=${project.id}`);

  // 页面展示与后端 Read Model 一致
  await expect(page.getByText("项目初始化")).toBeVisible();
  await expect(page.getByText("必需")).toBeVisible();

  const cc = await api(
    "GET",
    `/v1/pm/projects/${project.id}/command-center`,
    a.token
  );
  expect(cc.status).toBe(200);

  const items = cc.body.initialization.items as {
    key: string;
    status: string;
    required: boolean;
    source_state: string | null;
  }[];

  expect(items.length).toBeGreaterThan(0);

  for (const item of items) {
    expect(INIT_KEYS).toContain(item.key);      // key 必须是已知集合
    expect(INIT_STATUSES).toContain(item.status); // status 必须是 7 态之一
  }

  // 基础信息恒已完成，且源态来自 core.project
  const basics = items.find(i => i.key === "project_basics")!;
  expect(basics.status).toBe("completed");
  expect(basics.source_state).toBe("planning");

  // 刷新后状态不丢失
  await page.reload();
  await expect(page.getByText("项目初始化")).toBeVisible();
});
```

> **关于规格 step 4「完成一个初始化事项」**：纯 UI 操作无法可靠翻转任意 item。建议改为**驱动一个可控领域动作**（如设置项目目标 → `project_goal` 从 `not_started` 变为 `in_progress`/`completed`），再断言对应 item 状态变化与页面一致；避免为凑流程而伪造状态。

**通过标准**：初始化状态来自真实业务数据；页面与后端 Read Model 一致；刷新不丢失。

---

## 6. P0-007 · 多项目数据隔离

**测试目标**：同 Team 下多项目之间、跨 Team 之间的数据隔离 —— 不止页面显示，还要验后端读写权限。

### 6.1 数据准备

| 项目 | Team | 归属主体 |
|---|---|---|
| Project A1 | Team A | User A |
| Project A2 | Team A | User A |
| Project B1 | Team B | User B |

### 6.2 步骤与断言

| Step | 操作 | 预期结果 |
|---|---|---|
| 1 | 在 A1 创建 Requirement | A1 正常创建（`POST /v1/pm/projects/{project_id}/requirements`，`pm_requirements.rs:535-537`；body `CreateRequirementRequest`：`code`+`title` 必填，`markdown?` 缺省 `# {title}`） |
| 2 | 查看 A2 的 Requirement 列表 | **不显示** A1 的数据（200 + 空集，非报错） |
| 3 | 查看 B1 的 Requirement 列表 | **不显示** A1 的数据（跨 Team 同上） |
| 4 | 用 A1 身份读 A2 的单体资源 | 拒绝（**404**） |
| 5 | 尝试修改 A2 的 Requirement | 拒绝跨项目写入（**404**，原始数据不变） |
| 6 | 检查数据库 | 不存在错误的 Project 关联 |
| 7 | 分别刷新三个项目 | 各自数据保持正确 |

```ts
test("P0-007 同 Team 与跨 Team 数据隔离", async ({ context }) => {
  const a = await buildOwner("test-owner-a", 9104, "e2e-team-a");
  const b = await buildOwner("test-owner-b", 9105, "e2e-team-b");

  const a1 = await createProject(a, "e2e-project-a1", "E2EPA19104");
  const a2 = await createProject(a, "e2e-project-a2", "E2EPA29104");
  const b1 = await createProject(b, "e2e-project-b1", "E2EPB19105");

  // A1 创建独有数据（真实契约：项目级路由 + code/title 必填）
  const created = await api(
    "POST",
    `/v1/pm/projects/${a1.id}/requirements`,
    a.token,
    { code: "REQ-A1-001", title: "A1-only-requirement" }
  );
  expect(created.status).toBe(201);
  const reqId = created.body.id as string;

  // A1 读得到
  const listA1 = await api(
    "GET",
    `/v1/pm/requirements?project_id=${a1.id}`,
    a.token
  );
  expect(listA1.status).toBe(200);
  expect(listA1.body.items.map((x: any) => x.id)).toContain(reqId);

  // A2 读不到 A1 的数据（集合型读：200 + 空集）
  const listA2 = await api(
    "GET",
    `/v1/pm/requirements?project_id=${a2.id}`,
    a.token
  );
  expect(listA2.status).toBe(200);
  expect(listA2.body.items.map((x: any) => x.id)).not.toContain(reqId);

  // 跨 Team 同理
  const listB1 = await api(
    "GET",
    `/v1/pm/requirements?project_id=${b1.id}`,
    b.token
  );
  expect(listB1.status).toBe(200);
  expect(listB1.body.items.map((x: any) => x.id)).not.toContain(reqId);

  // 单体读：不可见 → 404（R19）
  const crossRead = await api("GET", `/v1/pm/requirements/${reqId}`, b.token);
  expect(crossRead.status).toBe(404);

  // 跨项目写入被拒，且原始数据不变
  const crossWrite = await api(
    "POST",
    `/v1/pm/requirements/${reqId}/revisions`,
    b.token,
    { title: "unauthorized-update" }
  );
  expect([403, 404]).toContain(crossWrite.status);

  const original = await api("GET", `/v1/pm/requirements/${reqId}`, a.token);
  expect(original.status).toBe(200);
  expect(original.body.title).toBe("A1-only-requirement");
});
```

### 6.3 期望状态码口径表（务必区分）

| 场景 | 期望 | 依据 |
|---|---|---|
| 集合型读（列表）跨项目 | **200 + 空集**（过滤，不报错） | `pm_lists.rs:271-277` 按 `project_member` JOIN 过滤 |
| 单体读跨项目 | **404** | `support.rs:178-208` `require_project_visible` |
| Command Center 跨项目 | **404** | `apps/pm-api/src/command_center.rs:50` 显式调用同一守卫 |
| 跨项目写入 | **404**（或 403） | 写入路径同样要求可见性 |
| 跨租户主体读本项目需求列表 | 可见数 = 0 | 既有 FLOW-08 |

### 6.4 数据库断言

| # | 断言 | 口径 |
|---|---|---|
| 1 | Requirement 的 Project 关联正确 | `delivery.requirement.project_id` = A1.id |
| 2 | Project 的 Team 归属正确 | `core.project.tenant_id` = Team A.id |
| 3 | 无跨项目错误关联 | `delivery.requirement` 中不存在 `project_id` 指向 B1 却由 Team A 主体创建的错行 |
| 4 | 被拒写入未改数据 | `delivery.requirement.title` 仍为 `A1-only-requirement`，且未新增 revision 行 |
| 5 | 审计符合授权结果 | 越权尝试若落审计，必须是**拒绝**语义（`governance.audit_log`）；不得出现「成功」记录 |

---

## 7. 执行顺序与依赖

| 顺序 | 案例 | 作用 | 依赖 |
|---|---|---|---|
| 1 | **P0-004** | 创建 Project A1，验证创建与 Team 归属 | 依赖 Team A（由 P0-01 的 Team 链路或本套件 Fixture 建团） |
| 2 | **P0-005** | 验证新项目无需外部工具即可进入并使用 | 依赖 004 的建项链路 |
| 3 | **P0-006** | 验证 InitItem 派生状态与数据一致性 | 依赖 004/005 的项目 |
| 4 | **P0-007** | 创建 A2、B1，验证项目间与跨 Team 隔离 | 依赖 004；需 Team B 独立 tid |

> 隔离要求：同 Team 多项目用**同一 tid 不同 token**；跨 Team 用**不同 tid**（不同租户）。三条项目命名带唯一 Key 后缀防 409 撞号。

---

## 8. 最终验收清单

**执行验收（11 项）**

- [ ] P0-004 项目创建成功
- [ ] P0-004 Team 归属关系正确（`tenant_id` + `project_member` owner 行）
- [ ] P0-005 创建后进入 Command Center（`/project/command-center?project_id=`）
- [ ] P0-005 无 Git/Runner 仍可使用 AI-PM
- [ ] P0-006 初始化事项正确生成（7 个 key）
- [ ] P0-006 **七种**派生状态正确（含 `waiting_for_human`）
- [ ] P0-006 页面与 Read Model 一致
- [ ] P0-007 同 Team 多项目数据隔离
- [ ] P0-007 跨 Team 数据隔离
- [ ] P0-007 越权写入被拒绝（404）
- [ ] P0-007 数据库关联及审计正确

**重置检查**

- [ ] 一次性库销毁；无 `_sqlx_test_%` 残留
- [ ] `VITE_USE_MOCK=0`；产物无 `api/mock` 引用
- [ ] 失败保留 Trace / Screenshot / API Request-Response / 服务端日志

---

## 9. 交付物（映射到本仓真实路径，R20）

```
ai-frontend/
├── e2e/
│   ├── p0-01-team-onboarding.spec.ts      # P0-01（11 号文档）
│   ├── p0-02-team-project.spec.ts         # ← 本套件
│   ├── p0/
│   │   ├── fixtures.ts                    # ProjectTestContext + buildOwner/createProject
│   │   └── project-api.ts                 # 项目/初始化读取封装（含 Command Center Read Model）
│   └── team/
│       └── stack.ts                       # 复用：signToken / browserSession / api / claimTeam
└── ../test-case/evidence/team-qa/         # 报告与产物落点（trace / screenshot / results.json）
```

> **不新建** `tests/e2e|fixtures|helpers` 平行目录 —— 本仓 Playwright `testDir` 是 `ai-frontend/e2e`，`stack.ts` 已提供栈能力，避免第二套真源。

---

## 10. 待裁决项

| # | 议题 | 选项 | 建议 |
|---|---|---|---|
| **D6** | 表单选择器 | 改用中文 i18n 标签 / 先补 `data-testid` | **补 testid**（`s01.project-create` 的 name/key/description + submit），文案随 locale 变，i18n 标签不可靠 |
| **D7** | P0-006 step 4「完成一个初始化事项」 | 用领域动作驱动 / 只断言派生一致性 | 用**领域动作**驱动一个 item 变化，不伪造状态 |
| **D8** | 越权期望码 | `[403,404]` / 写死 404 | 写死 **404**（本仓口径，R19） |
| **D9** | 集合型读期望 | 断言 200 + 空集 / 断言 403 | **200 + 空集**（过滤语义），并在文档写明「空集即隔离生效」 |
| **D10** | Key 命名策略 | 手工填 / 自动建议 | 显式填**唯一** Key（如 `E2EPA19104`），不用名称自动建议（避免多用例撞号） |

---

## 11. 关联文档

| 文件 | 内容 |
|---|---|
| `docs/e2e-spec/10-p0-test-specification-v1.md` | P0 总规格（7 领域 / 24 案例 / 门禁 / 评审 R1–R10） |
| `docs/e2e-spec/11-p0-01-playwright-spec-v1.md` | P0-01 可执行规格（栈、纪律、裁决 D1–D5） |
| `docs/e2e-spec/00-e2e-testing-framework-v1.md` | E2E 体系总纲（11 字段标准 / 判定口径） |
| `crates/pm-application/src/command_center.rs` | Command Center Read Model 与 `init_judgement`（权威实现） |
| `apps/pm-api/src/pm_lists.rs` | 项目创建/列表、Requirement 列表（隔离 SQL） |
| `apps/pm-api/src/support.rs:178-208` | `require_project_visible` → 404 隔离口径 |
| `ai-frontend/src/views/project/create/ProjectCreateView.vue` | 创建页真实表单与跳转 |
| `ai-frontend/locales/zh-CN/s01/project-create.yaml`、`s01/command-center.yaml` | 真实文案锚点 |
