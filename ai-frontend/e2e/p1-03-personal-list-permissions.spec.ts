import { expect, test } from "@playwright/test";

import { api } from "./team/stack";
import { buildOwner, createProjectFor, psql, type TestOwner, type TestProject } from "./p0/fixtures";

/**
 * P1-03 · SEC-054 Personal Home 列表权限（docs/e2e-spec/19 号 P1-03，**Must**）
 *
 * 锚点（R79-R81）：列表 = GET /v1/pm/projects 分页包装 {items,total,…}，
 * 过滤 = core.project JOIN core.project_member AND principal_id（pm_lists.rs
 * :111-127）；撤销即时生效（JWT 不含 membership，逐请求判库）；聚合端点
 * GET /v1/pm/personal-home 尚不存在（目标形态）；delivery.task 无 due 列。
 * 撤销用 psql 删 project_member 行直测**过滤谓词本身**（fixture 操作，一次性
 * 库内；端点级移除归 D50）。
 */

test.describe.configure({ mode: "serial" });

const TID_A = 9521;
const TID_B = 9522;

let ownerA: TestOwner;
let ownerB: TestOwner;
let projectA: TestProject;
let projectB: TestProject;

async function projectIds(token: string): Promise<string[]> {
  const res = await api("GET", "/v1/pm/projects", token);
  expect(res.status).toBe(200);
  expect(typeof res.body.total).toBe("number");
  expect(res.body.total).toBe(res.body.items.length); // total 与过滤后集合一致
  return res.body.items.map((p: { id: string }) => p.id);
}

test.beforeAll(async () => {
  ownerA = await buildOwner("p1-sec054-a", TID_A, "team-p1-sec054-a", "p1s1");
  projectA = await createProjectFor(ownerA, "E2EP1S1", "e2e-sec054-a");
  ownerB = await buildOwner("p1-sec054-b", TID_B, "team-p1-sec054-b", "p1s2");
  projectB = await createProjectFor(ownerB, "E2EP1S2", "e2e-sec054-b");
});

/** TC-P1-03-01 · 列表只返回授权项目（双向） */
test("TC-01 列表过滤：各自只见自己的项目，total 一致", async () => {
  const idsA = await projectIds(ownerA.token);
  const idsB = await projectIds(ownerB.token);
  expect(idsA).toContain(projectA.id);
  expect(idsA).not.toContain(projectB.id);
  expect(idsB).toContain(projectB.id);
  expect(idsB).not.toContain(projectA.id);
});

/** TC-P1-03-02 · 跨租户/跨项目：集合过滤 + 单体 404（防存在性泄露） */
test("TC-02 跨租户读：列表互斥 + 对方项目详情 404", async () => {
  const cc = await api(
    "GET",
    `/v1/pm/projects/${projectA.id}/command-center`,
    ownerB.token
  );
  expect(cc.status).toBe(404); // 单体资源不可见一律 404（support.rs 口径）

  const reqs = await api(
    "GET",
    `/v1/pm/projects/${projectA.id}/requirements`,
    ownerB.token
  );
  expect(reqs.status).toBe(404); // 带 project_id 过滤的集合读先过可见性（12 号 R68）
});

/** TC-P1-03-03 · 过滤完整性：API 集合 == 调用者租户的 DB 项目集合 */
test("TC-03 列表集合与 DB 租户项目集合一致（无泄漏/无缺失）", async () => {
  for (const owner of [ownerA, ownerB]) {
    const res = await api("GET", "/v1/pm/projects", owner.token);
    const apiIds = (res.body.items as { id: string }[]).map(p => p.id).sort();
    const dbIds = psql(
      `SELECT id FROM core.project WHERE tenant_id = '${owner.team.id}' ORDER BY id`
    )
      .split("\n")
      .filter(Boolean)
      .sort();
    expect(apiIds).toEqual(dbIds); // 过滤在 SQL 层：恰好等于授权集合
  }
});

/** TC-P1-03-04 · 撤销访问后下一次请求即生效（判库制，不等 token 过期） */
test("TC-04 撤销 project_member 后下一次列表请求立即收窄", async () => {
  const before = await projectIds(ownerB.token);
  expect(before).toContain(projectB.id);

  // fixture：一次性库内直接移除成员行（过滤谓词的直接操作；端点级撤销归 D50）
  psql(`DELETE FROM core.project_member WHERE project_id = '${projectB.id}'`);

  const after = await projectIds(ownerB.token);
  expect(after).not.toContain(projectB.id); // 下一请求即生效（R80）
});

/** TC-P1-03-05 · 无伪造期限（delivery.task 无 due 列 → 期限恒空，不虚构） */
test("TC-05 列表行不携带伪造期限字段", async () => {
  const res = await api("GET", "/v1/pm/projects", ownerA.token);
  for (const row of res.body.items) {
    // R81：personal-home 聚合端点未存在；项目行不应出现 due/截止类字段
    expect(row.due_at ?? row.dueAt ?? null).toBeNull();
  }
});
