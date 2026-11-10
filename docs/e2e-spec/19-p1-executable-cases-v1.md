# AI-PM · P1 可执行测试用例集 V1.0（滚动推进：P1-01 → P1-07）

> **性质**：P1 规格（`18-p1-test-specification-v1.md`）的**可执行下沉**。每组落到真实路由 / 字段 / 选择器 / 断言；未确认项显式标「待映射」，不假设已存在。
> **复用基建**：`ai-frontend/e2e/team/stack.ts`（自签 JWKS + 真 pm-api + 一次性库）、`playwright.team.config.ts`（:8850，`VITE_USE_MOCK=0`）。
> **推进状态**：P1-01 ✅ 本批；P1-02 ~ P1-07 待续（按序追加本文件）。

---

## P1-01 · DATA-020 初始化持久一致性（Should）

### 0. 落地评审（新增发现）

| # | 级别 | 发现 | 处置 |
|---|---|---|---|
| **R71** | 🟡 | `ai-frontend/src/api/initialization.ts` 为**混合模块**：V2 仓库函数（`listRepositoriesV2` 等 → `/v1/pm/projects/{id}/repositories` 真实路由）已接真被 `SettingsProjectRepositoriesView.vue:29` 使用；但 `getInitializationState` / `updateInitializationState`（`GET/PATCH /v1/pm/projects/{id}/initialization`）**后端无此路由** —— mock 时代遗留死代码 | 初始化读取**不得**走该端点；死函数按 AGENTS 第 1 条建议删除（D45）。无红线违规（后端从未实现直写） |
| **R72** | ✅ | **Goal 数据源实证**：`goal_present` = `ai.project_document(doc_key='project-goal')` 存在 published revision（`command_center.rs:735-741`）；`goal_pending` = 同 doc_key 的活跃 `ai.document_proposal`（`:584-591`）；`goal_status(present, human_pending, pending)`：present>0 → `completed`；human_pending（`redline_pending`）>0 → `waiting_for_human`；pending>0 → `in_progress`；否则 `not_started`（`:105-115`） | Fixture「Goal 已创建」= project-goal 文档 published；创建路径见 D43 |
| **R73** | ✅ | Key 建议端点真实存在：`GET /v1/pm/projects/suggest-code`（`initialization.ts:38-40` → 后端） | Fixture 建项目可复用 |

**读取路径定版（R71 + 12 号 R68）**

| 层 | 路径 | 结构 |
|---|---|---|
| API（快照主证据） | `GET /v1/pm/projects/{id}/command-center` | `blocks.initialization_progress.data.items[]`，每项 = `{key, title, status, required, source_state, detail}`（`command_center.rs:77` + `:325-333`；**无 expanded**） |
| UI（展示证据） | `/project/command-center?project_id=<uuid>` | 文本「项目初始化」「必需 {done} / {total}」（`s01/command-center.yaml:21-26`；无 testid，R18/12 号） |
| DB（权威源） | psql 直查 7 张领域表 | 见 §4 口径 |

### 1. 锚点映射（用户草案 → 实现）

| 草案元素 | 真实锚点 | 状态 |
|---|---|---|
| 「打开项目 Dashboard」 | `/project/command-center?project_id=`（无 `/projects/:id`，12 号 R11） | ✅ |
| 「初始化进度区域」 | CC API `initialization_progress` 块 + 页面「项目初始化」文本 | ✅ |
| InitItem 字段 | `key/title/status/required/source_state/detail`；**expanded 不存在**（D42 分歧） | ✅/⚠️ |
| Fixture「Goal 已创建」 | project-goal 文档 published（R72；创建路径 = 知识 V2 提案+确认，D43 待映射） | ⚠️ |
| 「登出后重新登录」 | 真实表单登录被 iam 阻塞（10 号 R1）→ 自动化等价物 = **新浏览器 context**（服务端无会话态，等价性成立；UI 登出流另测） | ⚠️ 映射 |
| 「清除缓存」 | fresh context + `clearCookies`（localStorage 本就不注入） | ✅ |
| 派生规则 | 7 key × 权威源表（见 §4） | ✅ |

### 2. 六个用例（可执行）

**TC-P1-01-01 · 部分初始化状态正确显示**

Fixture：Team+User（`stack.ts` 独立 tid）→ `POST /v1/pm/projects {code, name}` 201 →（D43）project-goal 文档 published → 无仓库 / 无 import / 无 discovery / 无 device / 无任务。

| 检查点 | 断言 | 锚点 |
|---|---|---|
| items 完整性 | 7 个固定 key 全在（`project_basics/goal/repository_access/code_analysis/project_cognition/dev_environment/human_actions`） | `command_center.rs:778-848` |
| project_basics | `completed` + `source_state='planning'` | `:778-783` |
| project_goal | `completed`（Goal 已创建） | R72 |
| repository_access / code_analysis / project_cognition / dev_environment / human_actions | 均 `not_started`（对应权威表 0 行） | `:119-146,798-848` |
| UI | 「项目初始化」「必需 2 / 7」类文本可见 | yaml :21-26 |
| 无虚假完成 | 上 5 项无一 `completed`/`blocked` 无依据 | 派生规则对照 |

```ts
const cc = await api("GET", `/v1/pm/projects/${pid}/command-center`, token);
const items = cc.body.blocks.initialization_progress.data.items;
const byKey = Object.fromEntries(items.map(i => [i.key, i]));
expect(byKey.project_basics.status).toBe("completed");
expect(byKey.project_basics.source_state).toBe("planning");
expect(byKey.project_goal.status).toBe("completed");
for (const k of ["repository_access","code_analysis","project_cognition","dev_environment","human_actions"])
  expect(byKey[k].status).toBe("not_started");
```

**TC-P1-01-02 · 刷新一致**：快照（API items 全量）→ `page.reload()` → 重读 → `toEqual`。InitItem 无时间戳字段，可整对象比较（无需剔除逻辑；若后续加字段再引入 normalize）。

**TC-P1-01-03 · 重登一致**：记录快照 → **销毁 context 新建**（服务端无会话态，等价「登出重登」）→ 重新注入 → 重读 → 相等。UI 登出/登录真实流归 P0-001b（BLOCKED，不混入本组）。

**TC-P1-01-04 · 重开一致**：goto `/projects` 列表 → 再进 Command Center → 快照相等；**断言不混入他项目 items**（project_id query 隔离）。

**TC-P1-01-05 · 字段一致性**：

| 字段 | 断言 |
|---|---|
| `status` | 每个 key 按派生规则对照权威表重算后相等（§4 规则表） |
| `required` | 固定值不随操作漂移（哪些 key required=true **待映射 D44**：实现时从 `required_items`/`required` 字段落定） |
| `source_state` | `project_basics='planning'`；repo/import 类有来源时 = 领域原始状态（`repo_import.status` 透传） |
| `expanded` | **不存在** → 断言 `!('expanded' in item)` 并在验收记录登记 D42 分歧（矩阵 :70 vs 代码） |

**TC-P1-01-06 · 排除 Session/缓存**：fresh context + `clearCookies` → 重读 → API 快照与 **DB 重算预期**对照（不是与旧快照）：psql 读 7 表 → 按 §4 规则推演预期 items → 与 API 相等。证明后端纯派生、无客户端缓存参与。

**负向（18 号 §1）**：解绑仓库后 `repository_access` 回退（`disconnectRepository` 方法/路径 **待映射 D45**）；删除来源实体 → 重算；他会话完成步骤 → 本会话刷新后反映最新领域态。

### 3. 自动化模板（已映射真实栈）

```ts
// ai-frontend/e2e/p1-01-init-persistence.spec.ts
import { expect, test } from "@playwright/test";
import { api, browserSession, signToken } from "./team/stack";

const tid = 9501;
const sub = "p1-data020-user";
const token = signToken(sub, tid);

async function readItems(pid: string) {
  const cc = await api("GET", `/v1/pm/projects/${pid}/command-center`, token);
  expect(cc.status).toBe(200);
  return cc.body.blocks.initialization_progress.data.items;
}

test("TC-P1-01-02/03 刷新与重登后初始化状态一致", async ({ browser }) => {
  const pid = /* fixture 建项目所得 */ "";
  const ctxA = await browser.newContext();
  await ctxA.addInitScript(t => {
    for (const [k, v] of Object.entries(t))
      window.localStorage.setItem(k, v as string);
  }, browserSession(sub, tid));
  const pageA = await ctxA.newPage();
  await pageA.goto(`/project/command-center?project_id=${pid}`);
  await expect(pageA.getByText("项目初始化")).toBeVisible();

  const before = await readItems(pid);
  await pageA.reload();
  expect(await readItems(pid)).toEqual(before);        // TC-02

  await ctxA.close();                                   // TC-03：全新会话
  const ctxB = await browser.newContext();
  await ctxB.addInitScript(t => {
    for (const [k, v] of Object.entries(t))
      window.localStorage.setItem(k, v as string);
  }, browserSession(sub, tid));
  const pageB = await ctxB.newPage();
  await pageB.goto(`/project/command-center?project_id=${pid}`);
  await expect(pageB.getByText("项目初始化")).toBeVisible();
  expect(await readItems(pid)).toEqual(before);
});
```

### 4. 派生规则与 DB 口径（status 重算表）

| key | 权威查询 | 派生 |
|---|---|---|
| project_basics | `core.project` 存在 | 恒 `completed`（source_state `planning`） |
| project_goal | published `project-goal` 文档 / `redline_pending` 提案 / 活跃提案 | completed / waiting_for_human / in_progress / not_started（`:105-115`） |
| repository_access | `integration.project_repository` count + verified/unreachable/pending | 0→`not_started`；有→`in_progress`（规则 `:119-130` 细则实现时对照） |
| code_analysis | `integration.repo_import.status` | `queued/cloning/reading/analyzing→in_progress`；`draft_ready/proposed→waiting_for_human`；`applied→completed`；`failed/rejected/cancelled→blocked`；无→`not_started`（`:143-146`） |
| project_cognition | `discovery.discovery_session` + finding verification | 会话/发现状态映射（`:736-768` 区段，实现时对照） |
| dev_environment | `devenv.device.readiness` | 映射（`:834-848` 区段） |
| human_actions | `delivery.task` 待人工集合 | 无→`not_started` |

### 5. 通过 / 失败判定

**通过**：六 TC 全过；`required_items` 与 items `required` 自洽；API/UI/DB 无矛盾；fresh context 纯派生恢复。
**失败**：刷新回退；重登丢失；状态与权威表矛盾；前端缓存成为恢复必要条件；出现对 InitItem 的独立写入通道（后端无 —— R71 死代码不构成，但删除之，D45）。

### 6. 验收记录模板

| 用例 | 结果 | 缺陷 | 证据 |
|---|---|---|---|
| TC-01 部分初始化 | 待执行 | — | CC API 响应 + UI 截图 |
| TC-02 刷新一致 | 待执行 | — | 前后快照 diff |
| TC-03 重登一致 | 待执行 | — | 双 context 快照 |
| TC-04 重开一致 | 待执行 | — | 重开前后快照 |
| TC-05 字段一致 | 待执行 | — | 派生规则对照表 + D42 分歧记录 |
| TC-06 清缓存纯派生 | 待执行 | — | fresh context API vs DB 重算 |

### 7. 待映射项（不改变 DATA-020 验收目标）

| # | 事项 |
|---|---|
| **D43** | project-goal 文档创建路径确认（知识 V2 `POST /v1/pm/documents/proposals` + `confirm_redline`，或 `create_document` 是否有 REST 暴露）→ Fixture 定稿 |
| **D44** | `required` 固定集合落定（从 CC `required_items` 字段读取后写死断言） |
| **D45** | `initialization.ts` 死函数（get/updateInitializationState）删除 + `disconnectRepository` 真实方法/路径确认（负向用例用） |
| **D46** | `project_cognition` / `dev_environment` 派生细则（`:736-768,834-848` 区段逐分支对照后补全 §4 表） |

---

## P1-02 · UX-052 Personal Home 登录默认落地（Should）

### 0. 落地评审（新增发现）

| # | 级别 | 发现 | 处置 |
|---|---|---|---|
| **R74** | ✅ | 登录路由 = **`/auth/login`**（`AUTH_PATHS.login`，`router/auth.ts:12`），非草案的 `/login`；表单 zh 标签「邮箱或用户名 / 口令 / 登录」（`locales/zh-CN/auth.yaml:17-23`）；**无 testid**（R13 同族） | 选择器用 label 文本或补 testid（D6 族） |
| **R75** | ✅ | **落地优先级实证**（`AuthLoadingView.vue:47-63` `settleDestination`）：① onboarding 门（`onboarding_required → /onboarding`，**优先于一切**）→ ② `?redirect=` 回跳 → ③ 默认 `/personal`；redirect 由守卫写入（`beforeEach :625-629`） | TC-03/07 的裁决依据 |
| **R76** | 🟠 | **returnTo 链路断裂**：`LoginView.vue:114` `router.push(AUTH_PATHS.signingIn)` **不带 query** —— 守卫写在 login 页 URL 的 `?redirect=` 在中转时丢失，`settleDestination` 读到空 → **登录恒落 `/personal`** | 按草案口径（returnTo 是兼容性验证非产品要求）**实际行为可接受**；TC-03 改判为证据记录用例；修复 = 透传 query（**D49**，产品定） |
| **R77** | 🟠 | **TC-05 期望改判**：已登录访问登录页 → **`/dashboard`**（`beforeEach :613-616`），非草案的 `/personal` | 测试按实际写死；若产品要求 `/personal` 属需求变更（**D47**） |
| **R78** | ✅+⚠️ | TeamSwitcher 真实存在（`TopBar.vue:10` → `components/global/team/TeamSwitcher.vue`，「同账号团队切换 0044」）；但 JWT 单 `tid` —— **切换的令牌语义待查**（是否重签/换票） | TC-08 可测基线 = 多 tid 多会话；切换内部机制 **D48** |
| — | ℹ️ | 自动化口径：会话注入 = **真实自签 RS256 JWT + 服务端真验**（jwks 模式），非伪造前端状态；真实表单登录归 **P0-001b**（iam 阻塞，R1） | 全组用 `stack.ts` 注入 + `/auth/signing-in` 中转页触发**真实**落地逻辑（settleDestination 真调 `GET /v1/pm/me`） |

### 1. 锚点映射（用户草案 → 实现）

| 草案元素 | 真实锚点 | 状态 |
|---|---|---|
| 「登录页面」 | `/auth/login`；`#auth-login-account` / `#auth-login-password` + zh 标签 | ✅ |
| 「点击登录」 | `<button type="submit">`（`LoginView.vue:190`）→ `signIn` → `push(signingIn)`（`:109-114`） | ✅ |
| 「跳转到 /personal」 | 注入后 `goto /auth/signing-in` → `settleDestination` → `/personal` | ✅ |
| 「personal-home testid」 | 无 testid → URL + 内容双证据（卡片 h2 / 空态欢迎卡 `PersonalHomeView.vue:289`）；建议补 testid（D6 族） | ⚠️ |
| 「路由守卫」 | `beforeEach :609-630`（已实证） | ✅ |
| 「无 Team 处理」 | 门 = 中转页 `:51`；**深链无路由级守卫**（B09-18 fixme / P0-01 D6 已知缺口） | ⚠️ |
| 「多 Team 切换」 | TeamSwitcher 组件在；令牌语义 D48 | ⚠️ |

### 2. 八个用例（可执行）

**TC-P1-02-01 · 正常登录默认落地**

| 检查点 | 断言 |
|---|---|
| 落地 | 注入（有 membership 的 tid）→ `goto /auth/signing-in` → `toHaveURL(/\/personal$/)` |
| 页面主体 | 非登录页、非项目页；空态用户见欢迎卡，有项目用户见列表卡 |
| 不跳过 | User-A（多项目）与 User-B（零项目）**均**落 `/personal` —— 单项目不跳项目 Dashboard |
| 与最近访问无关 | 先 visit 项目页再重走 signing-in → 仍 `/personal` |

**TC-P1-02-02 · 未登录访问受保护页面**

```ts
await page.context().clearCookies();
await page.goto("/personal");
await expect(page).toHaveURL(/\/auth\/login\?redirect=%2Fpersonal/);
await expect(page.getByLabel("邮箱或用户名")).toBeVisible();
```
无循环（URL 稳定一次跳转）；守卫写入 redirect 参数（R75 证据）。

**TC-P1-02-03 · 登录后返回原访问页面（R76 改判：证据记录用例）**

1. 无 token `goto /projects` → 断言 `/auth/login?redirect=%2Fprojects`（守卫写入 ✓）。
2. 完成登录链（注入 + `goto /auth/signing-in`）→ **实际落 `/personal`**（中转丢 query，R76）。
3. 记录两段证据；**D49 修复后转正**：断言落 `/projects` 且项目权限校验不被 returnTo 绕过（无权限项目仍 404）。

**TC-P1-02-04 · 刷新恢复**：`/personal` → `reload()` → 仍 `/personal`、数据重拉（5 端点）、无登录页闪烁/循环（有 token 时守卫直放）。

**TC-P1-02-05 · 已登录访问登录页（R77 改判）**

```ts
await signIn(context, sub, tid);          // 注入有效会话
await page.goto("/auth/login");
await expect(page).toHaveURL(/\/dashboard$/);   // 实际行为；D47 记录分歧
```
核心验收（不重复进入登录流程、无循环）成立；落点 `/dashboard` vs `/personal` 归 D47。

**TC-P1-02-06 · 页面导航**：`/personal` 列表 → 点有权限项目 → `/project/command-center?project_id=` → 返回 `/personal` 正常；**不替代 SEC-054**（过滤断言归 P1-03）。

**TC-P1-02-07 · 无 Team 用户**

| 检查点 | 断言 | 锚点 |
|---|---|---|
| 登录认证与 onboarding 区分 | 无 membership tid 注入 → `signing-in` → **`/onboarding`**（门优先，`settleDestination` 第一分支） | `:51-54` |
| 双入口可见 | `onboarding-choice-create` / `onboarding-join-token` testid | P0-002 锚点 |
| 建团后落地 | `claimTeam` → 重走 signing-in → `/personal` | R75 |
| **深链缺口（记录不改判）** | 无 Team 直接 `goto /personal` → **现状不拦**（无路由级守卫） | P0-01 D6 / B09-18 fixme |

**TC-P1-02-08 · 多 Team（R78：基线可测 + 机制待映射）**

- 可测基线：tid-A / tid-B 两个会话各自注入 → **各自落 `/personal`**；`GET /v1/pm/projects` 列表互斥（A 团队项目不出现在 B 会话）—— 即 membership 随 tid 正确切换。
- 切换机制（TeamSwitcher 内部是否重签 tid / 调用何端点）：**D48 待映射**，落地后补「切换后列表随新 tid、身份不变、无跨 Team 混淆」三条断言。

### 3. 自动化模板（已映射）

```ts
// ai-frontend/e2e/p1-02-login-landing.spec.ts
import { expect, test } from "@playwright/test";
import { browserSession } from "./team/stack";

async function inject(context, sub: string, tid: number) {
  await context.addInitScript(t => {
    for (const [k, v] of Object.entries(t))
      window.localStorage.setItem(k, v as string);
  }, browserSession(sub, tid));
}

test("TC-P1-02-01 登录默认落地 /personal", async ({ browser }) => {
  const ctx = await browser.newContext();
  await inject(ctx, "p1-ux052-a", 9502);
  const page = await ctx.newPage();
  await page.goto("/auth/signing-in");            // 真实落地逻辑（settleDestination）
  await expect(page).toHaveURL(/\/personal$/);
  await expect(page.locator("h2").first()).toBeVisible();
});
```

### 4. 后端与路由层断言

| 检查点 | 预期 | 锚点 |
|---|---|---|
| 登录认证 | 注入 JWT 经 `require_bearer → resolve_principal` 真验 | `routes.rs:479-490` |
| 默认导航 | `settleDestination` 第三分支 `/personal` | `:62` |
| 路由守卫 | 无 token → `/auth/login?redirect=<fullPath>` | `:625-629` |
| 已登录防重登 | → `/dashboard` | `:613-616` |
| onboarding 门 | 中转页优先分支 | `AuthLoadingView :51` |
| 项目访问 | 落地不绕过项目权限（列表 JOIN / 详情 404） | P1-03 口径 |

### 5. 缺陷判定（按草案 8 条 + 修正）

1-7 条沿用草案；**第 5 条修正**：已登录访问登录页的判定 = 「无循环且离开登录页」成立即可，落点按 R77 实际（`/dashboard`）；**新增第 8 条**：D49 修复后 returnTo 仍断链 → 回归 FAIL。

### 6. 验收清单

| 用例 | 结果 | 缺陷 | 证据 |
|---|---|---|---|
| TC-01 默认落地 | 待执行 | — | URL + 页面主体截图（多/零项目双 user） |
| TC-02 未登录拦截 | 待执行 | — | login?redirect URL + 表单可见 |
| TC-03 原目标路由（改判） | 待执行 | D49 | 守卫写入 + 中转丢失两段证据 |
| TC-04 刷新恢复 | 待执行 | — | reload 后 URL/内容 |
| TC-05 已登录防重登（改判） | 待执行 | D47 | /dashboard 落点证据 |
| TC-06 导航 | 待执行 | — | 进出项目往返 |
| TC-07 无 Team 引导 | 待执行 | （深链缺口引用 P0-01 D6） | onboarding 落点 + 建团后落地 |
| TC-08 多 Team（基线） | 待执行 | D48 | 双 tid 会话互斥证据 |

### 7. 待映射项

| # | 事项 |
|---|---|
| **D47** | 已登录访问登录页的落点产品裁决（现状 `/dashboard` vs 草案期望 `/personal`） |
| **D48** | TeamSwitcher 切换的令牌语义（组件 `components/global/team/TeamSwitcher.vue`；是否重签 tid / 调用端点） |
| **D49** | returnTo 修复：`LoginView.vue:114` 透传 `?redirect=` 至 signing-in（修复后 TC-03 转正） |
| （引用） | 登录/落地页补 testid（D6 族）；P0-001b iam 入栈后 TC-01 可升级为真实表单登录 |

---

## P1-03 · SEC-054 Personal Home 列表权限（**Must**，排 P1 批）

### 0. 落地评审

| # | 级别 | 发现 | 处置 |
|---|---|---|---|
| **R79** | ✅ | 列表接口 = `GET /v1/pm/projects`，响应为**分页包装** `{items, total, …}`（`pm_lists.rs:36` `page.to_response(items, total)`）；过滤 SQL = `core.project JOIN core.project_member AND principal_id`（`:111-127`） | 断言读 `body.items`，不虚构 `{projects}` 形状 |
| **R80** | ✅ | **撤销即时生效**：凭证（JWT）不含 membership，权限在每次查询时判库 → 名册移除成员（B09-16 已证链路）后**下一次请求**即不可见 | 撤销用例可写死「下一次请求」 |
| **R81** | ⚠️ | `GET /v1/pm/personal-home` 聚合端点**尚不存在**（目标形态，`personal-home.ts:37-39`）；`delivery.task` 无 due 列 → `DueInfo.dueAt` 恒 null（`:34-35`） | 断言用 `/v1/pm/projects`；期限断言 = 「无伪造期限」 |
| — | ℹ️ | UI 无 testid（R18 族）→ 列表可见性断言用 API 为主证据 + 页面卡片计数辅助 | 建议补 testid（D6 族） |

### 1. 用例（可执行）

**Fixture**：User-A（tidA，可见 PA）、User-B（tidB，可见 PB）、User-C（tidC，曾可见 PC 后被移除）；User-B 额外**不在** User-A 的租户 —— 跨租户 + 跨项目两层。

| TC | 步骤 | 断言 | 锚点 |
|---|---|---|---|
| TC-P1-03-01 列表过滤 | A 调 `GET /v1/pm/projects`；B 同 | A.items 含 PA 不含 PB；B.items 含 PB 不含 PA；`total` 与 items 长度一致 | `pm_lists.rs:111-127` |
| TC-P1-03-02 跨租户 | B 令牌查 A 租户项目（无此路由面 → 用需求列表/详情面） | 集合空集 / 详情 404 | P0-02 §6.3 口径 |
| TC-P1-03-03 搜索/分页不泄漏 | 列表接口带查询参数（名称子串） | 未授权项目名不出现在 items/total；无命中时 0 行而非他租户数据 | 同一过滤谓词 |
| TC-P1-03-04 **撤销时效** | C 先可见 PC → 名册移除 C → **下一次** `GET /v1/pm/projects` | PC 消失（不等 token 过期） | R80 |
| TC-P1-03-05 详情直访 | B 直调 A 项目 Command Center / 需求详情 | **404** | `support.rs:178-208` |
| TC-P1-03-06 期限不伪造 | A 的任务事项读面 | `dueAt === null`（后端无 due 列） | R81 |

```ts
const listA = await api("GET", "/v1/pm/projects", a.token);
expect(listA.body.items.map(p => p.id)).toContain(pa.id);
expect(listA.body.items.map(p => p.id)).not.toContain(pb.id);
// 撤销后（removeMember 链路，B09-16 同款）
const after = await api("GET", "/v1/pm/projects", c.token);
expect(after.body.items.map(p => p.id)).not.toContain(pc.id);
```

### 2. 待映射项

| # | 事项 |
|---|---|
| **D50** | 名册移除成员端点路径定版（B09-16 已证存在；spec 引 `e2e/p0` 同款调用） |
| **D51** | `PersonalHomeView` 列表卡片选择器（补 testid 后收紧 UI 断言） |

---

## P1-04 · FR-085 Codex / MCP Bootstrap（Should）

### 0. 落地评审

| # | 级别 | 发现 | 处置 |
|---|---|---|---|
| **R82** | ✅ | `/install` 目录 + `/install/{tool}` 指引真实（`onboarding.rs:27-28`；未知 slug **不 404** 落通用条目 `:133`） | 三条端点断言可写死 |
| **R83** | ✅ | Skills ZIP = 构建期内嵌（`skills_package.rs:20`），manifest `{format, sha256, integrity_only:true, contents:[四 SKILL.md + verify-install.ps1/.sh], min_platform_api_version, protocol_version}`；下载端点 `GET /install/packages/pm-ai-skills-<version>.zip` + `release-manifest.json` | ZIP 字节级断言可执行 |
| — | ℹ️ | 「S27 临时代理」= 需求层术语（18 号 R59 已裁决），无运行时组件 → 原 6 项生命周期断言撤销 | 本组 = 安装 + 包完整性 + MCP 认证复用 |
| — | ℹ️ | 矩阵已登记自动化证据：`tests/pm_devices.rs` 用例族（设备层）；**`/install` 端点无自动化**（矩阵明示缺口）→ 本组补齐 | 新增 e2e 一条 |

### 1. 用例（可执行）

| TC | 步骤 | 断言 | 锚点 |
|---|---|---|---|
| TC-P1-04-01 工具目录 | `GET /install`（无凭据——引导面公开性按实现） | 200 + 工具清单含 codex 条目 | `onboarding.rs:105` |
| TC-P1-04-02 分工具指引 | `GET /install/codex`；`GET /install/unknown-tool` | 200；未知 slug 落通用条目**不 404** | `:133-134` |
| TC-P1-04-03 manifest | `GET /install/packages/release-manifest.json` | `contents` 含四 SKILL.md + 两脚本；`sha256` 64 位 hex；`integrity_only=true` | `skills_package.rs:40-53` |
| TC-P1-04-04 ZIP 完整性 | 下载 zip → 本地 sha256 | **等于 manifest.sha256**；解压后四 SKILL.md 存在 | R83 |
| TC-P1-04-05 无凭证泄漏 | 全部响应/zip 内容扫描 | 不含 `sk-`、`BEGIN PRIVATE KEY`、密钥样式串 | 规格红线 |
| TC-P1-04-06 MCP 认证撤销闭环 | 有效 Key 调 `pm_list_tasks` → 撤销 → 再调 | 前者 200，后者拒；设备层复用 `tests/pm_devices.rs` 证据 | 16 号 P0-040 |

```ts
const manifest = await api("GET", "/install/packages/release-manifest.json", token);
expect(manifest.body.contents).toContain("skills/pm-ai-bootstrap/SKILL.md");
expect(manifest.body.integrity_only).toBe(true);
const zip = await fetch(`${baseURL}/install/packages/${manifest.body.package ?? "pm-ai-skills.zip"}`);
const digest = createHash("sha256").update(Buffer.from(await zip.arrayBuffer())).digest("hex");
expect(digest).toBe(manifest.body.sha256);
```

### 2. 待映射项

| # | 事项 |
|---|---|
| **D52** | zip 实际下载文件名（manifest 是否含 `package`/`version` 字段 → 文件名拼接定版） |
| **D53** | `/install` 是否公开（`is_public_path` 放行清单实现时确认；公开面断言随之写死） |

---

## P1-05 · BR-011 / ADR-041 零外部依赖 AI-PM（增量 Should）

### 0. 落地评审

| # | 级别 | 发现 | 处置 |
|---|---|---|---|
| **R84** | ✅ | **P0 栈已具备真 provider 面**：global-setup 起真 ai-pm-worker（`AI_PM_RUN_DISPATCH=on` + `AI_PM_RUN_EXECUTOR=mock` + `AI_PM_CONVERSATION_REPLY=on`）+ **mock LLM**（OpenAI 兼容，确定性响应）→ 分析/回复链 provider 指向 mock | P1-05 的「真 LLM 面」在 E2E 内 = mock provider，`model_name` 断言 ≠ `fake` |
| **R85** | ✅ | 零依赖主体已被 P0-008a/b 覆盖（`13-p0-03`）→ 本组**只做增量**：① mock provider 下 `ai.analysis_run.model_name` 如实 ② 回复链 assistant 消息落库 ③ 提案进人工确认 | 复用 `p0/helpers.ts`（`registerMockProvider`） |
| — | ⚠️ | 「Provider 不可达明确失败」：mock 恒可达 → 需构造死地址 provider（**D54**） | 未落地前该条 test.fixme 标注 |

### 1. 用例（可执行）

| TC | 步骤 | 断言 | 锚点 |
|---|---|---|---|
| TC-P1-05-01 零依赖创建 | `POST /v1/pm/projects`（无仓库/设备/agent）→ CC | 200；`ai.project_ai_config` 就绪 | P0-008a 复用 |
| TC-P1-05-02 mock provider 分析 | `registerMockProvider` → intake 提交 → 等 `ai.analysis_run` | `status='succeeded'`；**`model_name` 含 mock 标识（≠ fake）**；`result` 非空 | `analysis_model.rs` 路由 |
| TC-P1-05-03 回复链 | 会话 + 消息 → 轮询 messages | `role='assistant'` 回复落库（`AI_PM_CONVERSATION_REPLY=on`） | `conversation_reply.rs` |
| TC-P1-05-04 建议人工确认 | 提案 → `submit_revision` | `pending_confirmation`；confirm 需真人哈希 | P0-03 三道闸 |
| TC-P1-05-05 provider 不可达 | 注册指向死端口的 provider → 分析 | 明确失败（`failure_code`/WARN），**不回退 fake 冒充成功** | D54 待构造 |

### 2. 待映射项

| # | 事项 |
|---|---|
| **D54** | 不可达 provider 构造方式（`registerMockProvider` 变体：URL 指死端口 + 配置面 `ai.team_llm_provider` 写入端点确认） |

---

## P1-06 · FR-021 / ADR-023 Project Baseline（Should + DEF-P0-02 复现）

### 0. 落地评审

| # | 级别 | 发现 | 处置 |
|---|---|---|---|
| **R86** | ✅ | **4 端点定版**：`GET /v1/pm/projects/{id}/baseline`（overview）；`POST .../baseline/proposals`（生成提案，已有 pending → **409**，`pm_baseline.rs:109`）；`POST .../baseline/items/{item_id}/decision`（body `{action, content?, note?}`，已决策 409）；`POST .../baseline/confirm` | 全部可执行 |
| **R87** | ✅ | 提案派生源 =「已确认知识 + 仓库快照」（`:108` 注释）→ fixture 需先有 confirmed 知识（复用 `createAndConfirmRequirement`） | 空知识下提案行为待映射 **D55** |
| **R88** | ✅ | 确认门 = 存在 `proposed` 态条目 → **409 `"baseline proposal still has undecided items"`**（`:329-331`）；`investigation/deferred` 不阻断（R-BL-02） | 决策动作集：`confirm/rejected/investigation/deferred`（按条目枚举） |

### 1. 用例（可执行）

| TC | 步骤 | 断言 | 裁决 |
|---|---|---|---|
| TC-P1-06-01 提案派生 | 建 confirmed 需求 → `POST proposals` | 201；条目非空、`state='proposed'`；**Requirement 只引用 Revision**（R-BL-01：条目引用字段含 revision 引用，无正文复制） | ✅ |
| TC-P1-06-02 逐项决策 | 对每条目 `POST decision`（confirm/investigation 混合） | 200；已决策再决策 → 409 | ✅ |
| TC-P1-06-03 未决不阻断 | 含 `investigation` 条目时 confirm | 200；版本头落库；标记保留（R-BL-02） | ✅ |
| TC-P1-06-04 未决策阻断 | 有 `proposed` 条目时 confirm | **409** `undecided items` | ✅ |
| TC-P1-06-05 版本化快照 | 二次提案 → 决策 → confirm | `version` 递增；旧提案 `superseded`（R-BL-05）；重复 pending 提案 → 409 | ✅ |
| TC-P1-06-06 不可变 | 确认后条目再改 | 拒绝（条目不可变语义，实现面确认） | ✅ |
| **TC-P1-06-07 AI 不能确认（DEF-P0-02 复现）** | human 提案+决策 → **ai_pm 令牌调 confirm** | 期望 **403**；**当前实现会成功**（仅 `require_project_visible`，R70） | `test.fixme` 标注 DEF-P0-02，修复后转正实断言 |
| TC-P1-06-08 确认身份落库 | human confirm 后查 `ai.project_baseline` | `confirmed_by` = human 主体（服务端写入，非参数） | R61 |

### 2. 待映射项

| # | 事项 |
|---|---|
| **D55** | 空知识/空仓库下 `create_proposal` 行为（派生 0 条目？提案仍建？）→ 决定 fixture 最低数据 |
| **D56** | 条目 `action` 合法值集合（`confirm/rejected/investigation/deferred` 对应 DecisionBody.action 取值实现时枚举） |

---

## P1-07 · DEF-03 / DEF-04 并发与语义一致回归（Should）

### 0. 落地评审

| # | 级别 | 发现 | 处置 |
|---|---|---|---|
| **R89** | ✅ | DEF-03 旧入口**不可达已实证**（D38 关闭：`pm_writes.rs` 无 revision handler 挂载）→ 回归只打统一入口；防复活 = 断言 revision 面仅 `pm_requirements.rs` 路由存在 | 路由清单快照守卫 |
| **R90** | ✅ | 统一入口链 helper 已有：`confirmDraftRevision`（draft→submit→confirm 哈希门，`p0/helpers.ts:312-346`）；陈旧哈希 409 已被 P0-012 用例验证 → 本组做**真并发**变体 | `Promise.all` 双 confirm |
| **R91** | 🟡 | DEF-04 活跃分叉 = 任务域：REST `create_task` `require_human`（`pm_write/task.rs:180`）vs 执行器非红线 AI 直写（R8）→ **DEF-P0-01 实证面** | 分叉证据用例（当前行为），修复后转一致断言 |

### 1. 用例（可执行）

| TC | 步骤 | 断言 | 裁决 |
|---|---|---|---|
| TC-P1-07-01 陈旧哈希 409 | 建 confirmed 需求 → A 起草新修订 → B 持**旧哈希** confirm；A 持真哈希 confirm | B **409** 且终态不变；A 200 → Rev 翻转 | FLOW-05/P0-012 复用 |
| TC-P1-07-02 **真并发**双 confirm | 同一 pending 修订，`Promise.all` 两个 confirm：真哈希 + 旧哈希 | 恰一个 200；另一个 409；终态唯一；审计含冲突（不静默双 200） | `decide` 竞态同族 |
| TC-P1-07-03 历史保留 | Rev1/Rev2 读回 | 旧行 `superseded` 保留；`read_at_ref` 可读旧版 | 14 号口径 |
| TC-P1-07-04 REST 身份闸 | ai_pm 令牌 `POST /v1/pm/tasks` | **403**（`require_human`，R69） | ✅ 可断言 |
| **TC-P1-07-05 执行器分叉（DEF-P0-01 证据）** | AI 命令链 `update_task_status`（非红线） | 当前**成功**（与 REST 403 分叉）→ `test.fixme` 标 DEF-P0-01，修复后断言一致 403 | R91 |
| TC-P1-07-06 路由清单快照 | 断言 revision 面路由只存在于 `pm_requirements.rs` 族（静态清单断言或 404 探测旧面） | 无旧定稿入口复活 | R89 |

```ts
// TC-P1-07-02 真并发：同一修订，两身份两哈希
const [mine, stale] = await Promise.all([
  api("POST", confirmUrl, a.token, { expected_content_hash: goodHash }),
  api("POST", confirmUrl, bToken, { expected_content_hash: staleHash })
]);
const codes = [mine.status, stale.status].sort();
expect(codes).toEqual([200, 409]);            // 恰一成功一拒，无静默双 200
```

### 2. 待映射项

| # | 事项 |
|---|---|
| **D57** | 并发窗口真实性：两 confirm 同事务竞争的时序构造（串行化下退化为顺序 200→409 也接受，但注明非真并发） |
| **D58** | AI 命令链触发方式复用（P0-05 spec 的执行器驱动入口 → `update_task_status` 命令提交面） |

---

## 推进状态

| 组 | 用例文档 | E2E 实现 | 待映射 |
|---|---|---|---|
| P1-01 | ✅ | `ai-frontend/e2e/p1-01-init-persistence.spec.ts`（6 例） | D43~D46 |
| P1-02 | ✅ | `e2e/p1-02-login-landing.spec.ts`（8 例） | D47~D49 |
| P1-03（Must） | ✅ | `e2e/p1-03-personal-list-permissions.spec.ts`（5 例） | D50~D51 |
| P1-04 | ✅ | `e2e/p1-04-bootstrap-install.spec.ts`（4 例） | D52~D53 |
| P1-05 | ✅ | `e2e/p1-05-zero-deps.spec.ts`（5 例，TC-05 fixme） | D54 |
| P1-06 | ✅ | `e2e/p1-06-baseline-confirm.spec.ts`（7 例，TC-07 fixme=DEF-P0-02） | D55~D56 |
| P1-07 | ✅ | `e2e/p1-07-def0304-regression.spec.ts`（6 例，TC-05 fixme=DEF-P0-01） | D57~D58 |

**实现状态（2026-11-11）**：`playwright.p1.config.ts`（复用 p0 global-setup：一次性库 + JWKS + 真 pm-api + 真 worker + mock LLM；证据落 `test-case/evidence/p1/`）+ 7 spec **41 用例**，`playwright test --list` 解析全通过。fixme ×3 = 缺陷/待构造位（DEF-P0-02 / DEF-P0-01 / D54）。运行：`pnpm exec playwright test --config=playwright.p1.config.ts`（需 Docker + `TEAM_QA_PM_API_BIN`，与 P0/team-qa 串行）。
