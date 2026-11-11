# AI-PM · P0-01 用户登录与 Team Onboarding · Playwright E2E 规格 V1.0

> **性质**：P0-01 套件的**可执行**自动化规格。从「测试案例」下沉到「选择器 + 断言 + 数据准备 + 清理」。
> **上位文件**：`docs/e2e-spec/10-p0-test-specification-v1.md`（P0 总规格，24 案例 / 7 套件 / 门禁）。
> **复用基建**：`ai-frontend/e2e/team/stack.ts`（自签 JWKS + 真 pm-api + 一次性库）、`ai-frontend/e2e/team-onboarding.spec.ts`（Team QA B09-01~B09-20）。
> **执行配置**：`ai-frontend/playwright.team.config.ts`（vite :8850 + `PM_API_URL` + `VITE_USE_MOCK=0` + MS Edge）。
> **基线**：master @ `2b3f509e`；迁移基线 `migrations/0001_baseline.sql`。
> **判定纪律**：UI 状态 + 直连 pm-api 终态**双证据**；HTTP 200 不作唯一证据。

---

## 0. 评审结论速览（先读这节）

规格原稿有 6 处与代码/实测不符，必须在实现前裁决（R4/R5 为 2026-11-10 接 UIFLOW-01 **真浏览器实测**新增；R65 为 2026-11-10 P0 套件落地实测新增，编号续接全局序列避让 18 号文档 R57~R64。证据见 `test-case/e2e-flow-execution-report-v1.md` §1.0 / §1.3）：

| # | 规格原文 | 代码事实 | 影响 | 处置 |
|---|---|---|---|---|
| **R1** | P0-001「打开登录页面 → 输入账号密码 → 点击登录按钮」 | 前端登录调用的是 **lmms iam-service**（经 api-gateway `/v1/iam/auth/*`），**不是 pm-api**（`ai-frontend/src/api/auth.ts:14-15,112`）。现有 E2E 栈只起 pm-api + PG + JWKS，**无 iam-service**（`ai-frontend/e2e/team/global-setup.ts:13-21`） | 真实表单登录在现有栈上**不可执行** | 见 §9-裁决 1：P0-001 拆为 **P0-001a（会话建立，可自动化）** + **P0-001b（真实表单登录，需 iam 入栈或降级 L4）** |
| **R2** | P0-003「角色及 Membership **状态**正确」 | `core.tenant_member` **没有 status 列**，列只有 `tenant_id / principal_id / role / granted_by / created_at / updated_at`；`role` CHECK ∈ `owner/admin/member`（`migrations/0001_baseline.sql:1392-1400`） | 断言字段写错 | 「Membership 状态」改判为：**成员行存在** + `core.principal` 主体已激活 + `core.tenant.is_active` |
| **R3** | P0-001「页面展示正确的 Team 信息」 | `GET /v1/pm/me` 的团队扩展面在 `apps/pm-api/src/principals.rs`（`/v1/pm/me` 是未激活主体唯一自查入口），字段 `onboarding_required` 见 `ai-frontend/src/api/team.ts:45` | 断言锚点 | 「Team 信息」断言锚定 `GET /v1/pm/me` 的 `tenant_id` / `team` / `onboarding_required`，不是页面文本 |
| **R4** | P0-002「访问 `/projects` → 被重定向到 `/onboarding`（**深链不可绕过**）」 | **深链不被拦**（UIFLOW-01 真浏览器实测）。onboarding 门**只在登录中转页**（`AuthLoadingView.vue:51`）；`router.beforeEach`（`src/router/index.ts:617-641`）只处理 layout / 设计目录 / 无 token 三件事，无路由级 onboarding 守卫。与 team-onboarding.spec **B09-18 的 fixme** 同源（「待裁决：onboarding 门只在登录中转页」） | 深链断言执行必红 | 断言改为**记录现状**（深链停留 `/projects`，标注待裁决）；若产品裁决补路由级守卫，再改回「被拦」。见 §9-D6 |
| **R65** | §3.3 前置「User B 是另一有效账号（**独立 tid**）」 | `accept_invitation` 在 `team_onboarding.rs:759-760` **硬绑 tid**（「接受只可能发生在 token 所在租户」，V1 单团队现实）——跨 tid 接受被服务端拒绝 | 受邀者用独立 tid 时接受必 409/403，用例必红 | 受邀者与 owner **同 tid、不同 sub**（team/stack `joinAs` 的被证绿路径，B09-06 同款）；V1 单团队下「独立 tid 受邀者」不成立，随多团队裁决一并开放 |
| **R5** | P0-002 DB 断言「`core.tenant_member` 行数 = **0**」「`core.tenant` 无该 `tid` 对应新建行」；§4 示例 `tenant_id ?? null).toBeNull()` | 首次 API 调用即由 `resolve_or_provision` **自动物化**：`core.principal`（`is_active=false`）+ `core.principal_identity` + `core.tenant_member`（role=`member`，`auth.rs:616-624`）+ `core.tenant` 镜像行（`is_active` **默认 true**，`0001_baseline.sql:1305`）。`/me` 的 `team=null` 判据是 memberships **只统计 ACTIVE**（`principals.rs:41-49`，join `p.is_active`），且 `tenant_id` 恒为镜像租户 uuid（非 null） | 「行数=0」「tenant 无行」「tenant_id=null」三条全必红 | 改判：**ACTIVE 成员行数 = 0**（join `principal.is_active`）+ `/me.team=null` + `onboarding_required=true` + `core.tenant.slug IS NULL`（未认领）。见 §3.2 / §5 |

**另有一条重要事实（非缺陷，是复用红利，R4 修订后表述）**：P0-002 / P0-003 的主体已被 Team QA 覆盖 —— B09-01（首登无团队→分流，**可自动化**）、B09-02（建团→创建者=owner）、B09-06（被邀者接受→激活为 member）。B09-18（深链 `/dashboard` 被拦）是 **fixme**（R4 同源待裁决），**不是**已覆盖事实。本套件不重写可自动化用例，而是补齐缺口 + 加 P0 级 DB 断言。映射见 §2。

---

## 1. 测试环境与账号

### 1.1 环境矩阵

| 组件 | 载体 | 地址 | 来源 |
|---|---|---|---|
| 浏览器 | MS Edge headless，1440×900，`locale: zh-CN` | — | `ai-frontend/playwright.team.config.ts` |
| 前端 | vite 独立实例，`VITE_USE_MOCK=0` | `127.0.0.1:8850` | 同上（`baseURL` 现为 `localhost:8850`，落地时改 `127.0.0.1`） |
| 后端 | 真 pm-api 二进制，jwks 模式 | `127.0.0.1:18490` | `e2e/team/stack.ts:17` |
| JWKS | 本地自签 RS256 服务 | `127.0.0.1:18491` | `e2e/team/stack.ts:18` |
| PG | 容器 `aipm-teamqa-pg`，一次性空库 | `127.0.0.1:16499` | `e2e/team/global-setup.ts:21-22` |

> **零 mock 红线**：`VITE_USE_MOCK=0`（`playwright.team.config.ts` webServer env）；发布构建由 `ai-frontend/build/release-guard.ts` 硬失败。
> **端口纪律**：连 Docker 映射端口一律 `127.0.0.1`，禁 `localhost`（AGENTS.md 第 3 条）。

### 1.2 测试账号与数据

| 测试数据 | 用途 | 准备方式 |
|---|---|---|
| User A | 尚未加入任何 Team | 自签令牌 `signToken(sub, tid)`，`tid` 唯一。注意（R5）：**测试不手工插库**，但该主体第一次打任意 `/v1/pm/*` 请求时，`resolve_or_provision` 会自动落 inactive 的 principal + `tenant_member(member)` 行 + `core.tenant` 镜像行——这是产品行为，不是测试造数 |
| User B | 已有 Team 的普通成员（role=`member`） | 由 P0-003 的邀请链路**真实产生**，不手工插库 |
| User C | Team Owner | 由 `POST /v1/pm/teams` 建团者**真实产生**，role=`owner` |
| Team A | 测试团队 | `POST /v1/pm/teams` → `core.tenant` 行 |
| Invitation | 有效邀请 | `POST /v1/pm/tenants/{tenant_id}/invitations` 返回的 `token` |

> **铁律**：User B / User C / Team A **不得手工插库**，必须走真实业务链路产生 —— 否则测的就是「我造的数据能被读出来」，不是「流程能跑通」。
> **隔离**：每个用例独占 `tid`（= 独立租户），互不污染；`tid` 由用例序号派生，避免并行撞号。

### 1.3 Fixture 约定

复用 `ai-frontend/e2e/team/stack.ts` 现有能力，**不新建第二套**：

| helper | 签名 | 用途 |
|---|---|---|
| `signToken(sub, tid)` | → JWT | 自签 RS256，`tid` 即租户隔离键 |
| `browserSession(sub, tid, email?)` | → localStorage 四件套 | `prism_access_token` / `prism_refresh_token` / `prism_expires_at` / `prism_user`（`ai-frontend/src/api/base/token-storage.ts:8-11`） |
| `api(method, path, token, body?)` | → `{status, body}` | 直连 pm-api 做后端断言 |
| `claimTeam(sub, tid, name, slug?)` | → `{token, tenantId}` | 以 `sub/tid` 认领建团（`POST /v1/pm/teams`） |
| `joinAs(ownerToken, tenantId, sub, tid, role)` | → `{token}` | 经**真实邀请链路**把 `sub` 拉进团队并激活 |

P0-01 只需新增一个 **建团后返回 `tenant_id` 且能拿到邀请 token** 的薄封装，其余复用。

---

## 2. 与已有 Team QA 用例的映射（避免重复建设）

| P0 案例 | 已有用例 | 已有覆盖 | **缺口（本次要补）** | 处置 |
|---|---|---|---|---|
| **P0-001** 用户正常登录 | B09-01（首登分流） | 会话注入后身份正确 | **真实表单登录路径未覆盖**（无 iam-service）；「登录成功」目前只有 token 注入证据 | 拆 a/b，见 §3.1 + §9-裁决 1 |
| **P0-002** 首次登录无 Team | B09-01（首登分流） | 登录后分流 `/onboarding`（可自动化；深链拦截是 B09-18 fixme，见 R4） | ① 缺「不产生虚假默认 Team」的 **API + DB 双证据**（R5 口径：`/me.team=null` + **ACTIVE** 成员行数 = 0 + `core.tenant.slug IS NULL`）；② 深链 `/projects` 现状记录（不被拦，待裁决） | 扩 B09-01，加 R5 口径断言 |
| **P0-003** Team 创建与邀请 | B09-02、B09-04、B09-06、B09-15 | 建团→owner；邀请落库 PENDING；被邀者接受→member | ① 缺 **邀请链路全链**（Owner 发邀请→User B 接受→访问 Team A）的单条串联；② 缺 **`core.tenant_member` 行级断言**（role/status） | 新增串联用例 + DB 断言 |

> **不重写**：B09-01~B09-20 全部保留为回归基线；P0-01 只**新增** `p0-01-onboarding.spec.ts`，不改 Team QA 文件。

---

## 3. 详细测试用例

### 3.1 P0-001 · 用户正常登录

| 项 | 内容 |
|---|---|
| **Priority** | Must |
| **前置条件** | pm-api + PG + JWKS 已起（`global-setup`）；用例分配到唯一 `tid`；账号已存在（iam 侧或自签主体） |
| **步骤** | ① 打开 `/auth/login` ② 输入有效账号与密码 ③ 提交 ④ 等待跳转 ⑤ 检查当前身份 |
| **断言（UI）** | 登录后落在 `/auth/signing-in` 中转页 → 按 `onboarding_required` 分流到 `/onboarding` 或 `/dashboard`；无未授权页 |
| **断言（会话）** | `prism_access_token` / `prism_refresh_token` / `prism_expires_at` / `prism_user` 四件套写入 localStorage |
| **断言（API/DB）** | `GET /v1/pm/me` → 200；`body.principal_id`（或 `id`）= 预期主体；`body.tenant_id` = 预期租户；`body.onboarding_required` 与登录态一致 |
| **选择器** | 账号框 `#auth-login-account`；密码框 `#auth-login-password`；提交 `<button type="submit">`（`ai-frontend/src/views/auth/LoginView.vue:148,165,190`） |
| **现状** | **BLOCKED（R1）** —— 登录调用方是 iam-service，现有栈没有 |

**P0-001a（可立即自动化，替代原案主体）**：会话建立与身份正确 —— 用 `browserSession()` 注入令牌 → 访问 `/auth/signing-in` → 断言分流落点 + `GET /v1/pm/me` 身份 + DB 主体行。

**P0-001b（真实表单登录）**：需 iam-service 进栈或降级 L4 手工项；在裁决前**不计入 PASS**（按 §4 规则标 BLOCKED / NOT_IMPLEMENTED）。

### 3.2 P0-002 · 首次登录无 Team

| 项 | 内容 |
|---|---|
| **Priority** | Must |
| **前置条件** | User A 使用全新 `tid`（自签 `signToken(A, tid)`）。**注意（R5）**：不需要也不可能保证 "`core.tenant_member` 无该 principal 行"——首次 API 调用即自动物化 inactive principal + member 行 + 租户镜像行（`auth.rs:616-624`）；测试只保证**不手工插库**，判据用 ACTIVE 口径 |
| **步骤** | ① User A 登录（经 `/auth/signing-in` 中转页）② 系统检查 Team Membership ③ 进入 Team Onboarding ④ 检查创建/加入双入口 ⑤ 直接访问 `/projects` 深链（**现状记录**，R4） |
| **断言（UI）** | ① 落到 `/onboarding`（`ai-frontend/src/views/auth/AuthLoadingView.vue:51`）② 创建入口可见可点（`data-testid="onboarding-choice-create"`，`OnboardingWelcomeView.vue:54`）③ 加入入口可用（`data-testid="onboarding-join-token"`，`OnboardingWelcomeView.vue:85`）④ 无 Skip / Later 按钮（`OnboardingWelcomeView.vue:16` 产品裁决 Q1） |
| **断言（深链现状，R4）** | 访问 `/projects` → **停留在 `/projects`，不被拦**（onboarding 门只在登录中转页，无路由级守卫——待裁决，见 §9-D6 / B09-18 fixme）。若产品裁决补路由级守卫，本断言改回「被重定向到 `/onboarding`」 |
| **断言（API/DB，R5 口径）** | `GET /v1/pm/me` → 200：`team=null`、`onboarding_required=true`、`tenant_id` = 镜像租户 uuid（**非 null**）。DB：该租户 **ACTIVE 成员行数 = 0**（`core.tenant_member` join `core.principal` ON `is_active`）；`core.tenant` 行存在（provision 镜像，`is_active` 默认 true）但 **`slug IS NULL`**（未认领，认领时才写 slug/团队名，`team_onboarding.rs:401-407`）——**不产生虚假默认 Team** 的可验证判据 |
| **选择器** | 用 `data-testid`，**禁止用 i18n 文案**（`t('team.onboarding.welcome.*')` 随 locale 变） |

### 3.3 P0-003 · Team 创建与邀请

| 项 | 内容 |
|---|---|
| **Priority** | Must |
| **前置条件** | User A 尚未加入 Team；User B 是另一有效账号（独立 `tid`） |
| **步骤** | ① User A 创建 Team A ② 检查 Team Owner ③ User A 创建成员邀请 ④ User B 接受邀请 ⑤ 检查 Team Membership ⑥ User B 访问 Team A |
| **断言（UI）** | ① 建团成功 → 跳 `/onboarding/invite` ② 邀请列表出现 PENDING 邀请 ③ User B 打开 `/invitation/accept?token=...` 预览通过 ④ 接受后进入团队首页 |
| **断言（API）** | `POST /v1/pm/teams` → **201**，body 含 `tenant_id`；`GET /v1/pm/me`（Owner token）→ `tenant_id` 一致、`onboarding_required=false` |
| **断言（DB）** | `core.tenant` 新增 1 行（`display_name`、`slug`、`is_active=true`）；`core.tenant_member` 新增 2 行：Owner `role='owner'`、User B `role='member'`；成员行**无 status 列**（R2）→ 状态判据 = 成员行存在 + `core.principal` 已激活 + `core.tenant.is_active=true` |
| **断言（隔离）** | User B 未加入前访问 Team A → 后端拒绝（不可见项目 404，`apps/pm-api/src/support.rs:178-208`）；加入后可访问 |
| **选择器/接口** | 建团 `POST /v1/pm/teams` body `{name, slug?, description?}`（`crates/pm-application/src/team_onboarding.rs:162-168`）；邀请 `POST /v1/pm/tenants/{tenant_id}/invitations` body `{email, role}`（`:181-184`）；接受 `POST /v1/pm/invitations/accept` body `{token, email?}`（`:187-192`） |

---

## 4. Playwright 实现参考（已映射真实锚点）

> 下列代码为**落地参考**，选择器与路由已对齐本仓真实实现，不是通用脚本模板。

```ts
// ai-frontend/e2e/p0-01-onboarding.spec.ts
import { expect, test } from "@playwright/test";

import { api, browserSession, signToken } from "./team/stack";

/** 唯一 tid：用例隔离键（Team = core.tenant，tenant_id 即隔离维度）。 */
const tid = 9001;

async function signIn(context, sub: string, email: string | null = null) {
  await context.addInitScript(
    tokens => {
      for (const [key, value] of Object.entries(tokens)) {
        window.localStorage.setItem(key, value as string);
      }
    },
    browserSession(sub, tid, email)
  );
}

/**
 * P0-002 · 首次登录无 Team
 * 依据：AuthLoadingView.vue:51（onboarding_required → /onboarding）
 *       OnboardingWelcomeView.vue:54/85（data-testid 双入口）
 *       router/index.ts:482（/onboarding 路由）
 */
test("P0-002 首次登录无 Team → 强制 Onboarding（深链现状 = 不被拦，R4）", async ({
  context,
  page
}) => {
  const sub = "p0-user-a";
  await signIn(context, sub);

  // ① 登录中转页分流
  await page.goto("/auth/signing-in");
  await expect(page).toHaveURL(/\/onboarding/);

  // ② 创建 Team 入口（用 data-testid，不依赖 i18n 文案）
  await expect(page.getByTestId("onboarding-choice-create")).toBeVisible();

  // ③ 加入 Team 入口
  await expect(page.getByTestId("onboarding-join-token")).toBeVisible();

  // ④ 无 Skip / Later（产品裁决 Q1：首登只有两个入口）
  await expect(page.getByRole("button", { name: /skip|later/i })).toHaveCount(0);

  // ⑤ 深链现状（R4）：不被拦——onboarding 门只在登录中转页，无路由级守卫
  //    （与 B09-18 fixme 同源待裁决；若产品补守卫，此处改回 toHaveURL(/\/onboarding/)）
  await page.goto("/projects");
  await expect(page).toHaveURL(/\/projects/);

  // ⑥ 后端断言（R5 口径）：team=null + onboarding_required=true + tenant_id 为镜像租户 uuid
  const me = await api("GET", "/v1/pm/me", signToken(sub, tid));
  expect(me.status).toBe(200);
  expect(me.body.onboarding_required).toBe(true);
  expect(me.body.team).toBeNull();
  expect(typeof me.body.tenant_id).toBe("string"); // provision 镜像租户，非 null

  // ⑦ DB 断言（R5 口径，双证据）："不产生虚假默认 Team" = 无 ACTIVE 成员 + 未认领。
  //    行数 ≠ 0：provision 已自动落 inactive 的 member 行（auth.rs:616-624）。
  //    psql(`SELECT count(*) FROM core.tenant_member tm
  //           JOIN core.principal p ON p.id = tm.principal_id
  //          WHERE tm.tenant_id = '${me.body.tenant_id}' AND p.is_active`) === "0"
  //    psql(`SELECT slug FROM core.tenant WHERE id = '${me.body.tenant_id}'`) === ""
});
```

```ts
/**
 * P0-003 · Team 创建与邀请（全链）
 * 依据：team_onboarding.rs:35（POST /v1/pm/teams）
 *       team_onboarding.rs:37-45（邀请管理）
 *       team_onboarding.rs:64-71（preview / accept）
 *       migrations/0001_baseline.sql:1300-1310, 1392-1400
 */
test("P0-003 建团 → 邀请 → 受邀者接受 → 成员关系落库", async ({
  context,
  page
}) => {
  const owner = "p0-owner";
  const invitee = "p0-member";
  await signIn(context, owner);

  // ① 建团（走真实 UI 路径）
  await page.goto("/onboarding/create");
  await page.getByLabel(/name|名称/i).fill("Team A");
  await page.getByRole("button", { name: /create|创建/i }).click();
  await expect(page).toHaveURL(/\/onboarding\/invite/);

  // ② Owner 身份
  const ownerToken = signToken(owner, tid);
  const me = await api("GET", "/v1/pm/me", ownerToken);
  expect(me.status).toBe(200);
  expect(me.body.onboarding_required).toBe(false);
  const tenantId = me.body.tenant_id as string;

  // ③ 发邀请
  const inv = await api(
    "POST",
    `/v1/pm/tenants/${tenantId}/invitations`,
    ownerToken,
    { email: `${invitee}@p0.test`, role: "member" }
  );
  expect(inv.status).toBe(201);
  const invitationToken = inv.body.token as string;

  // ④ 受邀者接受
  await page.context().clearCookies();
  await signIn(context, invitee, `${invitee}@p0.test`);
  await page.goto(`/invitation/accept?token=${invitationToken}&type=invite`);
  await page.getByRole("button", { name: /accept|接受/i }).click();

  // ⑤ 终态断言：受邀者已入团
  const memberMe = await api("GET", "/v1/pm/me", signToken(invitee, tid));
  expect(memberMe.status).toBe(200);
  expect(memberMe.body.tenant_id).toBe(tenantId);
  expect(memberMe.body.onboarding_required).toBe(false);
});
```

---

## 5. 后端数据断言口径

| 断言目标 | 表 / 列 | 判据 |
|---|---|---|
| Team 已创建 | `core.tenant`（`migrations/0001_baseline.sql:1300-1310`） | 新增 1 行；`display_name` 匹配；`is_active=true` |
| Owner 角色 | `core.tenant_member.role` | `role='owner'`（CHECK ∈ `owner/admin/member`） |
| 成员角色 | `core.tenant_member.role` | `role='member'` |
| Membership 状态 | **无 status 列**（R2） | 行存在 + `core.principal` 主体已激活 + `core.tenant.is_active=true` |
| 无虚假默认 Team | `core.tenant_member` / `core.tenant`（R5 口径） | 该租户 **ACTIVE 成员行数 = 0**（join `principal.is_active`；provision 落的 inactive 行不算）；`core.tenant` 镜像行存在但 **`slug IS NULL`**（未认领）；`/me.team=null` |
| 邀请受理 | 邀请记录表 | 受理后状态由 PENDING 转为已接受；`core.tenant_member` 增 1 行 |
| 跨成员不可见 | `apps/pm-api/src/support.rs:178-208` | 未入团访问该租户项目 → **404**（不是 403） |

> **跨存储终态口径**：同一条断言必须同时给出 API 响应与 DB 行读数；两者不一致即 FAIL。

---

## 6. Fixture 与清理

| 阶段 | 动作 |
|---|---|
| 前置 | `global-setup` 起 PG 容器 `aipm-teamqa-pg`（宿主 `127.0.0.1:16499`）+ 一次性空库 `aipm` + pm-api（jwks 模式）+ JWKS 服务；库名含随机后缀 |
| 每例 | 分配唯一 `tid`；用 `claimTeam` / 邀请链路产生数据（**不手工插库**） |
| 后置 | 跑完销毁一次性库（`TEAM_QA_KEEP_DB=1` 时保留供排查）；清理残留 `_sqlx_test_%` 库 |
| 禁止 | 连正式库写入；改 `template1`；改产品代码或迁移（缺陷只登记） |

---

## 7. 失败证据落点

| 类型 | 位置 |
|---|---|
| Playwright 报告（HTML + JSON） | `test-case/evidence/team-qa/playwright-report/`、`playwright-results.json` |
| 截图 / Trace | `test-case/evidence/team-qa/playwright-artifacts/`（失败保留，`trace: retain-on-failure`） |
| pm-api 日志 | `TEAM_QA_LOG` 指定（默认系统 temp 目录），失败时随报告归档 |
| 缺陷登记 | `test-case/team-qa-execution-report-v1.md` + 缺陷单（含现象 / 复现 / 怀疑点 / 关联 P0 编号） |

> 失败**必须**保留：截图、Trace、请求/响应、服务端日志片段。

---

## 8. 完成标准

| # | 标准 | 判定 |
|---|---|---|
| 1 | P0-001a 自动化通过 | `GET /v1/pm/me` 身份 + 分流落点双证据 |
| 2 | P0-002 自动化通过 | UI 双入口 + 深链现状记录（R4）+ API/DB 无虚假 Team（R5 口径：ACTIVE=0 + slug NULL + team=null） |
| 3 | P0-003 自动化通过 | 建团→邀请→接受→成员行 role 落库 |
| 4 | 后端数据断言通过 | 每条用例均有 API + DB 双证据 |
| 5 | 测试数据清理通过 | 一次性库销毁；无 `_sqlx_test_%` 残留 |
| 6 | 失败保留 Trace 与 Screenshot | artifacts 目录落盘核对 |
| 7 | 零 mock | 跑批前确认 `VITE_USE_MOCK=0`，产物无 `api/mock` 引用 |
| 8 | P0-001b 状态明确 | 已实现（iam 入栈）**或**明确标 BLOCKED 并写明原因，**不得标 PASS** |

---

## 9. 待裁决项（实现前需拍板）

| # | 议题 | 选项 | 建议 |
|---|---|---|---|
| **D1** | P0-001 真实表单登录 | ① iam-service 进 E2E 栈 ② 降级 L4 手工 ③ 改写为会话级用例 | **① 优先**（契约已由 `api/auth.ts` 固定）；若 iam 不可得，先做 ③，b 标 BLOCKED |
| **D2** | 「Membership 状态」字段 | 改判为「成员行存在 + principal 激活 + tenant active」 | 直接采纳 R2 改判 |
| **D3** | 归属文件 | 新建 `ai-frontend/e2e/p0-01-onboarding.spec.ts` vs 并入 `team-onboarding.spec.ts` | **新建独立文件**（P0 是发布门禁资产，与 Team QA 回归基线分离）；共用 `team/stack.ts` |
| **D4** | 配置 | 复用 `playwright.team.config.ts`（`testMatch` 锁死 `team-onboarding.spec.ts`） | 新增 `playwright.p0.config.ts`，`testMatch: "p0-*.spec.ts"`，端口沿用 8850/18490/18491 |
| **D5** | `baseURL` 主机名 | 现为 `localhost:8850` | 改 `127.0.0.1:8850`（AGENTS.md 第 3 条） |
| **D6** | onboarding 深链路由级守卫（R4） | ① 产品裁决补路由级守卫（未入团深链一律回 `/onboarding`）② 维持现状（门只在登录中转页） | 产品裁决前**按现状写断言**（不被拦）；若裁决 ①，同步改回 B09-18 fixme 与本规格 P0-002 深链断言。证据：`test-case/e2e-flow-execution-report-v1.md` §3.1 OBS-03 |

---

## 10. 关联文档

| 文件 | 内容 |
|---|---|
| `docs/e2e-spec/10-p0-test-specification-v1.md` | P0 总规格（7 套件 / 24 案例 / 阶段 / 门禁）+ 评审记录 |
| `docs/e2e-spec/00-e2e-testing-framework-v1.md` | E2E 体系总纲（8 套件 / 11 字段标准 / 判定口径） |
| `ai-frontend/e2e/team/stack.ts` | 真实后端栈测试侧工具（signToken / browserSession / api / claimTeam / joinAs） |
| `ai-frontend/e2e/team-onboarding.spec.ts` | Team QA B09-01~B09-20（既有回归基线） |
| `ai-frontend/e2e/team/global-setup.ts` | PG 容器 + pm-api(jwks) + JWKS 起停 |
| `test-case/team-qa-test-plan-v1.md` / `team-qa-test-cases-v1.md` | Team 专项 QA 计划与 88 条用例 |
| `test-case/team-qa-execution-report-v1.md` | Team QA 执行报告（73 PASS / 7 FAIL，DEF-01/DEF-02） |
