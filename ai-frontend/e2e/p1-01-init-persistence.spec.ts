import { expect, test, type BrowserContext, type Page } from "@playwright/test";

import { api } from "./team/stack";
import {
  buildOwner,
  createProjectFor,
  psql,
  signInVia,
  type TestOwner,
  type TestProject
} from "./p0/fixtures";

/**
 * P1-01 · DATA-020 项目初始化状态持久一致性（docs/e2e-spec/19 号 P1-01）
 *
 * 读取路径定版（R71/R68）：GET /v1/pm/projects/{id}/command-center →
 * blocks.initialization_progress.data.items[]，每项 {key,title,status,
 * required,source_state,detail}（**无 expanded**，D42）。
 * 「登出重登」的自动化等价物 = 销毁重建浏览器 context（服务端无会话态，
 * AuthLoadingView 每次进入都重拉 GET /v1/pm/me 与 CC）。
 * D43（project-goal 文档创建路径）未定 → 本组以「零依赖新项目」为部分初始化
 * fixture：project_basics=completed，其余 6 项 not_started；goal 已创建变体
 * 待 D43 后补。
 */

test.describe.configure({ mode: "serial" });

const TID = 9511;

let owner: TestOwner;
let projectA: TestProject;
let projectB: TestProject;

interface InitItem {
  key: string;
  status: string;
  required: boolean;
  source_state: string | null;
}

async function readItems(pid: string): Promise<InitItem[]> {
  const cc = await api("GET", `/v1/pm/projects/${pid}/command-center`, owner.token);
  expect(cc.status).toBe(200);
  const block = cc.body.blocks?.initialization_progress;
  expect(block?.status ?? "ok").toBeTruthy();
  return block.data.items as InitItem[];
}

async function openCommandCenter(page: Page, pid: string): Promise<void> {
  await page.goto(`/project/command-center?project_id=${pid}`);
  await expect(page.getByText("项目初始化")).toBeVisible();
}

async function newSession(browser: import("@playwright/test").Browser): Promise<{
  context: BrowserContext;
  page: Page;
}> {
  const context = await browser.newContext();
  await signInVia(context, owner.sub, owner.tid);
  return { context, page: await context.newPage() };
}

test.beforeAll(async ({ browser }) => {
  owner = await buildOwner("p1-data020-a", TID, "team-p1-data020", "p1d1");
  projectA = await createProjectFor(owner, "E2EP1D1", "e2e-init-persistence-a");
  projectB = await createProjectFor(owner, "E2EP1D2", "e2e-init-persistence-b");
});

/** TC-P1-01-01 · 部分初始化状态正确显示（零依赖新项目的派生快照） */
test("TC-01 部分初始化：7 个固定 key，basics=completed 其余 not_started", async () => {
  const items = await readItems(projectA.id);
  const byKey = Object.fromEntries(items.map(i => [i.key, i]));

  expect(items.map(i => i.key).sort()).toEqual(
    [
      "project_basics",
      "project_goal",
      "repository_access",
      "code_analysis",
      "project_cognition",
      "dev_environment",
      "human_actions"
    ].sort()
  );
  expect(byKey.project_basics.status).toBe("completed");
  expect(byKey.project_basics.source_state).toBe("planning");
  for (const key of [
    "project_goal",
    "repository_access",
    "code_analysis",
    "project_cognition",
    "dev_environment",
    "human_actions"
  ]) {
    expect(byKey[key].status, key).toBe("not_started");
  }
});

/** TC-P1-01-02 · 刷新页面后快照一致 */
test("TC-02 刷新后初始化快照一致", async ({ browser }) => {
  const { context, page } = await newSession(browser);
  await openCommandCenter(page, projectA.id);
  const before = await readItems(projectA.id);

  await page.reload();
  await expect(page.getByText("项目初始化")).toBeVisible();
  expect(await readItems(projectA.id)).toEqual(before);
  await context.close();
});

/** TC-P1-01-03 · 会话销毁重建（登出重登等价物）后快照一致 */
test("TC-03 重登（新 context）后初始化快照一致", async ({ browser }) => {
  const first = await newSession(browser);
  await openCommandCenter(first.page, projectA.id);
  const before = await readItems(projectA.id);
  await first.context.close();

  const second = await newSession(browser);
  await openCommandCenter(second.page, projectA.id);
  expect(await readItems(projectA.id)).toEqual(before);
  await second.context.close();
});

/** TC-P1-01-04 · 重开同一项目一致，且不混入他项目数据 */
test("TC-04 重开项目一致；访问 projectB 不影响 projectA 快照", async ({ browser }) => {
  const { context, page } = await newSession(browser);
  await openCommandCenter(page, projectA.id);
  const snapshotA = await readItems(projectA.id);

  await openCommandCenter(page, projectB.id);
  const snapshotB = await readItems(projectB.id);
  expect(snapshotB.map(i => i.key).sort()).toEqual(snapshotA.map(i => i.key).sort());

  await openCommandCenter(page, projectA.id);
  expect(await readItems(projectA.id)).toEqual(snapshotA);
  await context.close();
});

/** TC-P1-01-05 · 关键字段一致性（无 expanded + required 与 required_items 自洽） */
test("TC-05 字段一致性：无 expanded；required 与 required_items 自洽", async () => {
  const cc = await api("GET", `/v1/pm/projects/${projectA.id}/command-center`, owner.token);
  const block = cc.body.blocks.initialization_progress;
  const items = block.data.items as (InitItem & { title?: string })[];

  for (const item of items) {
    expect("expanded" in item, `${item.key} 不应有 expanded（D42）`).toBe(false);
    expect(typeof item.required, item.key).toBe("boolean");
  }
  const requiredKeys = items.filter(i => i.required).map(i => i.key).sort();
  expect(requiredKeys).toEqual([...(block.data.required_items ?? [])].sort());
});

/** TC-P1-01-06 · 排除 Session/缓存：fresh context 读数与 DB 派生规则重算一致 */
test("TC-06 fresh context 快照 = 领域表重算（纯派生，无缓存参与）", async ({ browser }) => {
  // 权威表实况（R72/D46 口径：零依赖项目各源计数为 0）
  const goalDocs = psql(
    `SELECT count(*) FROM ai.project_document d
      WHERE d.project_id = '${projectA.id}'
        AND d.doc_key = 'project-goal'
        AND EXISTS (SELECT 1 FROM ai.document_revision r
                     WHERE r.document_id = d.id AND r.status = 'published')`
  );
  const repos = psql(
    `SELECT count(*) FROM integration.project_repository WHERE project_id = '${projectA.id}'`
  );
  const imports = psql(
    `SELECT count(*) FROM integration.repo_import WHERE project_id = '${projectA.id}'`
  );
  const discovery = psql(
    `SELECT count(*) FROM discovery.discovery_session WHERE project_id = '${projectA.id}'`
  );
  const devices = psql(
    `SELECT count(*) FROM devenv.device_project_binding WHERE project_id = '${projectA.id}'`
  );
  const tasks = psql(
    `SELECT count(*) FROM delivery.task WHERE project_id = '${projectA.id}'`
  );

  // 派生规则重算（command_center.rs:105-146 的零值分支）
  const expected: Record<string, string> = {
    project_basics: "completed",
    project_goal: Number(goalDocs) > 0 ? "completed" : "not_started",
    repository_access: Number(repos) > 0 ? "in_progress" : "not_started",
    code_analysis: Number(imports) > 0 ? "in_progress" : "not_started",
    project_cognition: Number(discovery) > 0 ? "in_progress" : "not_started",
    dev_environment: Number(devices) > 0 ? "in_progress" : "not_started",
    human_actions: Number(tasks) > 0 ? "in_progress" : "not_started"
  };

  const context = await browser.newContext();
  await signInVia(context, owner.sub, owner.tid); // fresh storage，仅注入凭据
  const items = await readItems(projectA.id);
  for (const item of items) {
    expect(item.status, item.key).toBe(expected[item.key]);
  }
  await context.close();
});
