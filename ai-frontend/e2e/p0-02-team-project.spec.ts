import { expect, test } from "@playwright/test";

import { api, signToken } from "./team/stack";
import {
  buildOwner,
  createProjectFor,
  psql,
  signInVia,
  type TestOwner,
  type TestProject
} from "./p0/fixtures";

/**
 * P0-02 · Team 与 Project 基础管理（docs/e2e-spec/12-p0-02 规格，4 案例）
 *
 * 评审修正已内化：
 *  - R11/R12: 无 /projects/:id 路由、无 team_id 列 → URL 带 project_id query、
 *    归属断言用 core.project.tenant_id。
 *  - R13: 表单无 id/html-for 关联 → 用**占位符 exact 匹配**（UIFLOW-03 被证绿
 *    的锚点；D6 的「补 testid」属产品改动，另立裁决，不在测试侧伪造）。
 *  - R58（本轮实证新增，回写规格 12 §0）: **无 GET /v1/pm/projects/{id} 路由**
 *    （pm_lists.rs 仅注册列表与子资源路由）——规格 P0-004 示例的单体读改用
 *    列表过滤 + DB 直查。
 *  - R59: 需求创建响应为 `{requirement: {...}}` 包裹（与 L2 e2e_flow.rs 一致）；
 *    DraftRevisionRequest 必填 `markdown`（规格示例的 title/body 均错）。
 *  - D8: 越权单体读/写写死 **404**；D9: 集合型读 = 200 + 空集（过滤语义）；
 *    D10: Key 显式唯一（E2EPA19104 等），不用名称自动建议。
 *
 * 判定纪律：UI 状态 + API/DB 终态双证据；HTTP 200 不作唯一证据。
 */

test.describe.configure({ mode: "serial" });

let ownerA: TestOwner;
let projectA1: TestProject;

/** P0-004 · 正常创建项目并归属 Team */
test("P0-004 UI 建项目 → 跳 CC 带 project_id；tenant_id 归属 + 5 人编制", async ({
  context,
  page
}) => {
  ownerA = await buildOwner("p0-owner-a", 9104, "E2E Team A", "e2e-team-a");

  await signInVia(context, ownerA.sub, ownerA.tid);

  // 走列表页入口（spec step 1–2）
  await page.goto("/projects");
  await page.getByRole("button", { name: "新建项目" }).first().click();
  await expect(page).toHaveURL(/\/project\/create$/, { timeout: 20_000 });

  // 表单（R13：占位符 exact 匹配——"例如 ATLAS" 是 "例如 Atlas Platform" 的忽略大小写子串）
  const form = page.locator("form.pc-page__form");
  await form.getByPlaceholder("例如 Atlas Platform", { exact: true }).fill("e2e-project-a1");
  await form.getByPlaceholder("例如 ATLAS", { exact: true }).fill("E2EPA19104");
  await form
    .getByPlaceholder("这个项目解决什么问题", { exact: false })
    .first()
    .fill("P0-004 creation test");
  await form.locator("button[type='submit']").click();

  // R11：创建后立即进入 Command Center（不是 /projects/:id）
  await expect(page).toHaveURL(/\/project\/command-center\?project_id=/, {
    timeout: 30_000
  });
  const projectId = new URL(page.url()).searchParams.get("project_id") ?? "";
  expect(projectId).not.toBe("");

  // 后端断言（R58 修正：无单体 GET 路由 → 列表过滤）
  const list = await api("GET", "/v1/pm/projects", ownerA.token);
  expect(list.status).toBe(200);
  const row = (list.body.items ?? list.body).find?.(
    (item: { code: string }) => item.code === "E2EPA19104"
  );
  expect(row, "created project must appear in tenant list").toBeTruthy();
  expect(row.status).toBe("planning");

  // DB：归属 Team A + 恰 5 人编制（owner/ai_pm/architect/developer/tester）
  const tenant = psql(
    `SELECT tenant_id FROM core.project WHERE id = '${projectId}'`
  );
  expect(tenant).toBe(ownerA.team.id);
  const roles = psql(
    `SELECT string_agg(role, ',' ORDER BY role) FROM core.project_member
      WHERE project_id = '${projectId}'`
  );
  expect(roles).toBe("ai_pm,architect,developer,owner,tester");

  projectA1 = { id: projectId, code: "E2EPA19104", name: "e2e-project-a1", tenantId: ownerA.team.id };
});

/** P0-005 · 无 Git/Runner/外部工具仍立即进入 Command Center */
test("P0-005 零外部依赖 → CC 可用；AI 配置就绪；无阻断提示", async ({
  context,
  page
}) => {
  await signInVia(context, ownerA.sub, ownerA.tid);

  await page.goto("/project/create");
  const form = page.locator("form.pc-page__form");
  await form.getByPlaceholder("例如 Atlas Platform", { exact: true }).fill("e2e-command-center");
  await form.getByPlaceholder("例如 ATLAS", { exact: true }).fill("E2ECM19104");
  await form.locator("button[type='submit']").click();
  await expect(page).toHaveURL(/\/project\/command-center\?project_id=/, {
    timeout: 30_000
  });
  const projectId = new URL(page.url()).searchParams.get("project_id") ?? "";

  // 初始化进度可见（R18：无 testid，用文本）
  await expect(page.getByText("项目初始化")).toBeVisible({ timeout: 20_000 });
  // 无强制配置仓库的阻断
  await expect(page.getByText(/必须.*仓库|required.*repository/i)).toHaveCount(0);

  // CC Read Model 可读（未接任何仓库/Runner 仍 200）
  const cc = await api(
    "GET",
    `/v1/pm/projects/${projectId}/command-center`,
    ownerA.token
  );
  expect(cc.status).toBe(200);
  expect(
    (cc.body.blocks?.initialization_progress?.data?.items ?? []).length,
    "initialization items must be non-empty"
  ).toBeGreaterThan(0);

  // DB：AI 配置就绪 + 无仓库无 Runner（合法初始态）
  const aiCfg = psql(
    `SELECT enabled || '/' || automation_mode FROM ai.project_ai_config
      WHERE project_id = '${projectId}'`
  );
  expect(aiCfg).toBe("true/ai_lead");
  const repos = psql(
    `SELECT count(*) FROM integration.project_repository WHERE project_id = '${projectId}'`
  );
  expect(Number(repos)).toBe(0);
});

/** P0-006 · 初始化事项派生状态正确且与 Read Model 一致（7 态 / 7 key） */
test("P0-006 初始化 7 事项 7 态派生一致；project_basics 恒 completed；刷新不丢", async ({
  context,
  page
}) => {
  await signInVia(context, ownerA.sub, ownerA.tid);

  const project = await createProjectFor(
    ownerA,
    "E2EINIT19104",
    "e2e-init-state",
    "P0-006 init states"
  );

  await page.goto(`/project/command-center?project_id=${project.id}`);
  await expect(page.getByText("项目初始化")).toBeVisible({ timeout: 20_000 });

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

  const cc = await api(
    "GET",
    `/v1/pm/projects/${project.id}/command-center`,
    ownerA.token
  );
  expect(cc.status).toBe(200);
  const items: Array<{
    key: string;
    status: string;
    required: boolean;
    source_state: string | null;
  }> = cc.body.blocks.initialization_progress.data.items;
  expect(items.length).toBeGreaterThanOrEqual(INIT_KEYS.length);

  for (const item of items) {
    expect(INIT_KEYS, `unknown init key: ${item.key}`).toContain(item.key);
    expect(
      INIT_STATUSES,
      `invalid init status for ${item.key}: ${item.status}`
    ).toContain(item.status);
  }

  // project_basics 恒 completed，源态来自 core.project（'planning'）
  const basics = items.find(item => item.key === "project_basics");
  expect(basics?.status).toBe("completed");
  expect(basics?.source_state).toBe("planning");

  // 刷新后派生状态不丢失（Read Model 是实时计算，无独立实体可腐化）
  await page.reload();
  await expect(page.getByText("项目初始化")).toBeVisible({ timeout: 20_000 });
});

/** P0-007 · 同 Team 多项目 + 跨 Team 数据隔离（D8=404 / D9=200+空集） */
test("P0-007 集合读过滤、单体读 404、跨项目写 404、原始数据不变", async () => {
  const ownerB = await buildOwner("p0-owner-b", 9105, "E2E Team B", "e2e-team-b");

  // 同 Team 第二项目 + 跨 Team 项目
  const projectA2 = await createProjectFor(ownerA, "E2EPA29104", "e2e-project-a2");
  const projectB1 = await createProjectFor(ownerB, "E2EPB19105", "e2e-project-b1");

  // A1 创建独有需求（R59：响应为 {requirement} 包裹）
  const created = await api(
    "POST",
    `/v1/pm/projects/${projectA1.id}/requirements`,
    ownerA.token,
    { code: "REQ-A1-001", title: "A1-only-requirement" }
  );
  expect(created.status).toBe(201);
  const reqId = created.body.requirement.id as string;

  // A1 自己读得到
  const listA1 = await api(
    "GET",
    `/v1/pm/projects/${projectA1.id}/requirements`,
    ownerA.token
  );
  expect(listA1.status).toBe(200);
  expect(
    (listA1.body.items ?? []).map((item: { id: string }) => item.id)
  ).toContain(reqId);

  // 同 Team 另一项目：不显示 A1 的数据（D9：过滤语义，不报错）
  const listA2 = await api(
    "GET",
    `/v1/pm/projects/${projectA2.id}/requirements`,
    ownerA.token
  );
  expect(listA2.status).toBe(200);
  expect(
    (listA2.body.items ?? []).map((item: { id: string }) => item.id)
  ).not.toContain(reqId);

  // 跨 Team 非成员读（实测口径，pm_lists.rs:261-269——比规格 R19 声称的更强）：
  // ① 带 project_id 过滤的集合读（全局或项目域）→ **404**（先过 require_project_visible，
  //    非过滤场景的"200+空集"只适用于不带 project_id 的全列表）
  const listA1ByB = await api(
    "GET",
    `/v1/pm/requirements?project_id=${projectA1.id}`,
    ownerB.token
  );
  expect(listA1ByB.status).toBe(404);
  const scopedByB = await api(
    "GET",
    `/v1/pm/projects/${projectA1.id}/requirements`,
    ownerB.token
  );
  expect(scopedByB.status).toBe(404);
  // ② 不带过滤的全列表 → 200 + membership JOIN 过滤（B1 的列表里不出现 A1 数据）
  const globalByB = await api("GET", "/v1/pm/requirements", ownerB.token);
  expect(globalByB.status).toBe(200);
  expect(
    (globalByB.body.items ?? []).map((item: { id: string }) => item.id)
  ).not.toContain(reqId);

  // 单体读跨 Team：404（D8：本仓口径，防存在性泄露）
  const crossRead = await api("GET", `/v1/pm/requirements/${reqId}`, ownerB.token);
  expect(crossRead.status).toBe(404);

  // 跨项目写：404 且原始数据不变（D8；R59 修正 body：DraftRevisionRequest 必填 markdown）
  const revisionsBefore = psql(
    `SELECT count(*) FROM delivery.requirement_revision WHERE requirement_id = '${reqId}'`
  );
  const crossWrite = await api(
    "POST",
    `/v1/pm/requirements/${reqId}/revisions`,
    ownerB.token,
    { markdown: "# unauthorized-write" }
  );
  expect(crossWrite.status).toBe(404);
  const revisionsAfter = psql(
    `SELECT count(*) FROM delivery.requirement_revision WHERE requirement_id = '${reqId}'`
  );
  expect(Number(revisionsAfter)).toBe(Number(revisionsBefore));

  // DB：关联正确 + 无错行 + 原始数据未动
  const dbProject = psql(
    `SELECT project_id || '/' || title FROM delivery.requirement WHERE id = '${reqId}'`
  );
  expect(dbProject).toBe(`${projectA1.id}/A1-only-requirement`);
  const wrongRows = psql(
    `SELECT count(*) FROM delivery.requirement
      WHERE project_id = '${projectB1.id}' AND title = 'A1-only-requirement'`
  );
  expect(Number(wrongRows)).toBe(0);
});
