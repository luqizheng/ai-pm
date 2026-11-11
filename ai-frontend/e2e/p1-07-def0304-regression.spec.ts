import { expect, test } from "@playwright/test";

import { api } from "./team/stack";
import {
  buildOwner,
  createProjectFor,
  psql,
  type TestOwner,
  type TestProject
} from "./p0/fixtures";
import { asAiPrincipal, createAndConfirmRequirement } from "./p0/helpers";

/**
 * P1-07 · DEF-03 / DEF-04 并发定稿与语义一致回归（docs/e2e-spec/19 号 P1-07）
 *
 * 锚点（R89-R91）：DEF-03 旧入口不可达（D38 已关闭）→ 回归打统一入口
 * （draft→submit→confirm，哈希门）；真并发 = Promise.all 双 confirm，
 * 恰一 200 一 409，无静默双 200。DEF-04 活跃分叉 = 任务域：
 * REST create_task require_human（pm_write/task.rs:180，ai_pm → 403）vs
 * 执行器非红线 AI 直写（R8）→ TC-05 为 DEF-P0-01 证据位（fixme）。
 * TC-06 = 旧定稿入口复活探测（404 探针）。
 */

test.describe.configure({ mode: "serial" });

const TID = 9561;
let owner: TestOwner;
let project: TestProject;

test.beforeAll(async () => {
  owner = await buildOwner("p1-def03-a", TID, "team-p1-def03", "p1d3");
  project = await createProjectFor(owner, "E2EP1D3", "e2e-def0304");
});

/** TC-P1-07-01 · 陈旧哈希 confirm → 409，真哈希 → 200，终态唯一 */
test("TC-01 陈旧哈希定稿 409，新哈希 200（统一入口）", async () => {
  const rev1 = await createAndConfirmRequirement(
    owner,
    project.id,
    "REQ-P1D3-001",
    "并发基线",
    "# 并发基线\n版本一正文。"
  );

  const draft = await api("POST", `/v1/pm/requirements/${rev1.id}/revisions`, owner.token, {
    markdown: "# 并发基线\n版本二正文。"
  });
  expect(draft.status).toBe(201);
  const rev2Id = draft.body.revision.id as string;
  const hash2 = draft.body.revision.content_hash as string;

  await api("POST", `/v1/pm/requirements/${rev1.id}/revisions/${rev2Id}/submit`, owner.token, {});

  const stale = await api(
    "POST",
    `/v1/pm/requirements/${rev1.id}/revisions/${rev2Id}/confirm`,
    owner.token,
    { expected_content_hash: rev1.contentHash } // 陈旧哈希（Rev1）
  );
  expect(stale.status).toBe(409);

  const fresh = await api(
    "POST",
    `/v1/pm/requirements/${rev1.id}/revisions/${rev2Id}/confirm`,
    owner.token,
    { expected_content_hash: hash2 }
  );
  expect(fresh.status).toBe(200);

  const head = psql(
    `SELECT current_revision_id FROM delivery.requirement WHERE id = '${rev1.id}'`
  );
  expect(head).toBe(rev2Id);
});

/** TC-P1-07-02 · 真并发双 confirm（同好哈希 / 新旧混合）：恰一 200 一 409 */
test("TC-02 真并发双 confirm 无静默双 200", async ({ }) => {
  const rev1 = await createAndConfirmRequirement(
    owner,
    project.id,
    "REQ-P1D3-002",
    "真并发基线",
    "# 真并发基线\n版本一。"
  );
  const draft = await api("POST", `/v1/pm/requirements/${rev1.id}/revisions`, owner.token, {
    markdown: "# 真并发基线\n版本二。"
  });
  const rev2Id = draft.body.revision.id as string;
  const hash2 = draft.body.revision.content_hash as string;
  await api("POST", `/v1/pm/requirements/${rev1.id}/revisions/${rev2Id}/submit`, owner.token, {});

  const confirmUrl = `/v1/pm/requirements/${rev1.id}/revisions/${rev2Id}/confirm`;
  const [first, second] = await Promise.all([
    api("POST", confirmUrl, owner.token, { expected_content_hash: hash2 }),
    api("POST", confirmUrl, owner.token, { expected_content_hash: hash2 })
  ]);
  expect([first.status, second.status].sort()).toEqual([200, 409]);

  const head = psql(
    `SELECT current_revision_id FROM delivery.requirement WHERE id = '${rev1.id}'`
  );
  expect(head).toBe(rev2Id);
});

/** TC-P1-07-03 · 历史保留：Rev1 superseded / Rev2 approved，均可读 */
test("TC-03 历史 Revision 保留且状态正确", async () => {
  const rows = psql(
    `SELECT rv.version || '|' || rv.state
       FROM delivery.requirement r
       JOIN delivery.requirement_revision rv
         ON rv.id = ANY (ARRAY[r.current_revision_id]::uuid[]
                           || ARRAY(SELECT id FROM delivery.requirement_revision
                                     WHERE requirement_id = r.id))
      WHERE r.code = 'REQ-P1D3-001'`
  );
  const states = rows.split("\n").map(r => r.split("|")[1]);
  expect(states).toContain("superseded");
  expect(states).toContain("approved");
});

/** TC-P1-07-04 · REST 身份闸：ai_pm 令牌建任务 → 403（require_human） */
test("TC-04 REST 建任务拒绝 AI 身份（403）", async () => {
  const aiToken = await asAiPrincipal("p1-def03-ai", TID);
  // ai_pm 主体需为项目成员才走得到 require_human（bootstrap 的 ai_pm 是另一
  // 主体；此主体由 asAiPrincipal 新建 → fixture 补成员行，仅作身份面构造）
  const me = await api("GET", "/v1/pm/me", aiToken);
  const aiPrincipalId = me.body.principal_id as string;
  psql(
    `INSERT INTO core.project_member (project_id, principal_id, role, tenant_id)
     VALUES ('${project.id}', '${aiPrincipalId}', 'ai_pm', '${owner.team.id}')
     ON CONFLICT DO NOTHING`
  );

  const res = await api("POST", "/v1/pm/tasks", aiToken, {
    project_id: project.id,
    title: "AI 越权建任务"
  });
  expect(res.status).toBe(403); // pm_write/task.rs:180 require_human_conn
});

/**
 * TC-P1-07-05 · 执行器分叉（DEF-P0-01 证据位，R91）
 * REST 拒 AI（TC-04 403），而执行器路径非红线命令（update_task_status 等）
 * 当前可被 AI 直写（policy_engine 黑名单默认 auto_apply）→ 与 REST 语义分叉。
 * 修复（策略收紧/执行器闸对齐）后：转正为「双入口一致 403」断言。
 */
test.fixme("TC-05 执行器与 REST 语义一致（DEF-P0-01）", async () => {
  // 修复后：同一业务动作经 REST 与执行器产生一致的身份闸结果 + 审计。
});

/** TC-P1-07-06 · 旧定稿入口复活探测（D38 关闭后的守护）
 *  路由 fallback 是 MCP 服务（routes.rs:205-209），未匹配路径的状态码不保证
 *  axum 默认 404 → 断言「旧式路径不得产生 2xx 定稿效果」而非精确 404。 */
test("TC-06 旧式定稿/审批路径无定稿效果（非 2xx 探针）", async () => {
  const probes = [
    "/v1/pm/requirements/00000000-0000-0000-0000-000000000000/approve",
    "/v1/pm/requirements/00000000-0000-0000-0000-000000000000/finalize",
    "/v1/pm/requirements/00000000-0000-0000-0000-000000000000/decide"
  ];
  for (const path of probes) {
    const res = await api("POST", path, owner.token, {});
    expect([200, 201, 204], path).not.toContain(res.status);
  }
});
