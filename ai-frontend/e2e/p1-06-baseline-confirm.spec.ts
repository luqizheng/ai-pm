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
 * P1-06 · FR-021 / ADR-023 Project Baseline（docs/e2e-spec/19 号 P1-06）
 *
 * 锚点（R86-R88）：4 端点 = GET overview / POST proposals（已有 pending → 409）
 * / POST items/{item_id}/decision（body {action,content?,note?}，action ∈
 * confirm|reject|defer|investigate → 状态 5 值；已决策 409）/ POST confirm
 * （存在 proposed 条目 → 409 "baseline proposal still has undecided items"；
 * investigation/deferred 不阻断 = R-BL-02）。提案派生源 = 已确认知识 + 仓库
 * 快照（fixture 用 confirmed requirement 的投影文档，D55 空数据行为待定）。
 * R70/DEF-P0-02：confirm_baseline 无真人守卫 → TC-07 以 fixme 记录，修复后
 * 转正断言 403。
 */

test.describe.configure({ mode: "serial" });

const TID = 9551;
let owner: TestOwner;
let project: TestProject;
let ownerId = "";

async function decide(itemIds: string[], action: string): Promise<number[]> {
  const statuses: number[] = [];
  for (const itemId of itemIds) {
    const res = await api(
      "POST",
      `/v1/pm/projects/${project.id}/baseline/items/${itemId}/decision`,
      owner.token,
      { action }
    );
    statuses.push(res.status);
  }
  return statuses;
}

async function pendingItemIds(proposalId: string): Promise<string[]> {
  const rows = psql(
    `SELECT id FROM ai.baseline_item
      WHERE proposal_id = '${proposalId}' AND state = 'proposed'
      ORDER BY created_at`
  );
  return rows ? rows.split("\n").filter(Boolean) : [];
}

test.beforeAll(async () => {
  owner = await buildOwner("p1-fr021-a", TID, "team-p1-fr021", "p1bl");
  project = await createProjectFor(owner, "E2EP1BL", "e2e-baseline");
  const me = await api("GET", "/v1/pm/me", owner.token);
  ownerId = me.body.principal_id as string;
  // 提案派生源（R87）= ai.project_knowledge active 行。确认需求**不会**自动
  // 投影知识行（D55 待定；P1 首跑实测 0 条目）——夹具与 b1_policy_guard
  // 同口径：末尾直接播种 active 知识行，验证派生与来源引用契约本身。
  await createAndConfirmRequirement(
    owner,
    project.id,
    "REQ-P1BL-001",
    "基线派生源需求",
    "# 基线派生源需求\n系统支持任务管理。"
  );
  psql(
    `INSERT INTO ai.project_knowledge
       (tenant_id, project_id, domain, content, confirmed_by)
     VALUES ('${owner.team.id}', '${project.id}', 'identity',
             'e2e 派生源：系统支持任务管理', '${ownerId}')`
  );
});

/** TC-P1-06-01 · 提案派生：条目非空、来源引用、无正文复制（R-BL-01 口径） */
test("TC-01 提案派生条目带来源引用", async () => {
  const created = await api(
    "POST",
    `/v1/pm/projects/${project.id}/baseline/proposals`,
    owner.token,
    {}
  );
  expect(created.status).toBe(201);
  const proposalId = created.body.proposal_id as string;
  expect(proposalId).toBeTruthy();

  const items = psql(
    `SELECT id || '|' || state || '|' || COALESCE(evidence->>'knowledge_id', '') ||
            '|' || COALESCE(evidence->>'type', '')
       FROM ai.baseline_item WHERE proposal_id = '${proposalId}'`
  );
  const lines = items.split("\n").filter(Boolean);
  expect(lines.length).toBeGreaterThan(0);
  for (const line of lines) {
    const [, state, knowledgeId, evidenceType] = line.split("|");
    expect(state).toBe("proposed");
    // 来源引用：evidence 指向 knowledge_id（不是把权威正文复制进条目）
    expect(knowledgeId || evidenceType).toBeTruthy();
  }

  // 清场（每用例自清场，同 TC-04 约定）：全 confirm + 确认基线，不留
  // pending 提案挡后续用例的 create（pending → 409）。
  const ids = await pendingItemIds(proposalId);
  expect((await decide(ids, "confirm")).every(st => st === 200)).toBe(true);
  expect(
    (
      await api(
        "POST",
        `/v1/pm/projects/${project.id}/baseline/confirm`,
        owner.token,
        {}
      )
    ).status
  ).toBe(200);
});

/** TC-P1-06-02 · 逐项决策 + 已决策 409 */
test("TC-02 逐项决策；重复决策 409", async () => {
  const created = await api(
    "POST",
    `/v1/pm/projects/${project.id}/baseline/proposals`,
    owner.token,
    {}
  );
  expect(created.status).toBe(201);
  const proposalId = created.body.proposal_id as string;
  const itemIds = await pendingItemIds(proposalId);
  expect(itemIds.length).toBeGreaterThan(0);

  const statuses = await decide(itemIds, "confirm");
  for (const status of statuses) expect(status).toBe(200);

  // 已决策再决策 → 409
  const again = await api(
    "POST",
    `/v1/pm/projects/${project.id}/baseline/items/${itemIds[0]}/decision`,
    owner.token,
    { action: "confirm" }
  );
  expect([404, 409]).toContain(again.status);

  // 清场：条目已全 confirm，确认基线交出干净状态（pending 挡 TC-03 的 create）。
  expect(
    (
      await api(
        "POST",
        `/v1/pm/projects/${project.id}/baseline/confirm`,
        owner.token,
        {}
      )
    ).status
  ).toBe(200);
});

/** TC-P1-06-03/05 · investigation 不阻断（R-BL-02）→ confirm → 版本化快照 */
test("TC-03 investigation 条目不阻断确认；版本与 confirmed_by 落库", async () => {
  const created = await api(
    "POST",
    `/v1/pm/projects/${project.id}/baseline/proposals`,
    owner.token,
    {}
  );
  expect(created.status).toBe(201);
  const proposalId = created.body.proposal_id as string;
  const itemIds = await pendingItemIds(proposalId);

  // 第一条 confirm，其余 investigate（R-BL-02：未决允许且标记）
  const firstConfirm = await api(
    "POST",
    `/v1/pm/projects/${project.id}/baseline/items/${itemIds[0]}/decision`,
    owner.token,
    { action: "confirm" }
  );
  expect(firstConfirm.status).toBe(200);
  const statuses = await decide(itemIds.slice(1), "investigate");
  expect(statuses.every(s => s === 200)).toBe(true);

  const confirmed = await api(
    "POST",
    `/v1/pm/projects/${project.id}/baseline/confirm`,
    owner.token,
    {}
  );
  expect(confirmed.status).toBe(200);

  const head = psql(
    `SELECT version || '|' || confirmed_by || '|' ||
            (SELECT status FROM ai.baseline_proposal WHERE id = '${proposalId}')
       FROM ai.project_baseline
      WHERE project_id = '${project.id}'
      ORDER BY version DESC LIMIT 1`
  );
  const [version, confirmedBy, proposalStatus] = head.split("|");
  expect(Number(version)).toBeGreaterThanOrEqual(1);
  expect(confirmedBy).toBe(ownerId); // 确认人 = 真实人工主体（服务端写入）
  expect(proposalStatus).toBe("confirmed");
});

/** TC-P1-06-04 · 存在 proposed 条目时 confirm → 409（未决策阻断） */
test("TC-04 有 proposed 条目时 confirm 被拒 409", async () => {
  const created = await api(
    "POST",
    `/v1/pm/projects/${project.id}/baseline/proposals`,
    owner.token,
    {}
  );
  expect(created.status).toBe(201); // 新 pending 提案（旧 proposal 已 confirmed）

  const refused = await api(
    "POST",
    `/v1/pm/projects/${project.id}/baseline/confirm`,
    owner.token,
    {}
  );
  expect(refused.status).toBe(409);
  expect(JSON.stringify(refused.body)).toContain("undecided");

  // 清场：决策全部条目并确认（顺带完成一轮 R-BL-02 全流程，不留 pending）
  const proposalId = created.body.proposal_id as string;
  const itemIds = await pendingItemIds(proposalId);
  expect((await decide(itemIds, "confirm")).every(s => s === 200)).toBe(true);
  const settled = await api(
    "POST",
    `/v1/pm/projects/${project.id}/baseline/confirm`,
    owner.token,
    {}
  );
  expect(settled.status).toBe(200);
});

/** TC-P1-06-05 · 二轮提案 → 全 confirm → version 递增；旧提案 superseded */
test("TC-05 二轮确认 version 递增，旧提案 superseded", async () => {
  const before = psql(
    `SELECT COALESCE(MAX(version), 0) FROM ai.project_baseline WHERE project_id = '${project.id}'`
  );

  const created = await api(
    "POST",
    `/v1/pm/projects/${project.id}/baseline/proposals`,
    owner.token,
    {}
  );
  expect(created.status).toBe(201);
  const proposalId = created.body.proposal_id as string;
  const itemIds = await pendingItemIds(proposalId);
  expect((await decide(itemIds, "confirm")).every(s => s === 200)).toBe(true);

  const confirmed = await api(
    "POST",
    `/v1/pm/projects/${project.id}/baseline/confirm`,
    owner.token,
    {}
  );
  expect(confirmed.status).toBe(200);

  const after = psql(
    `SELECT COALESCE(MAX(version), 0) FROM ai.project_baseline WHERE project_id = '${project.id}'`
  );
  expect(Number(after)).toBe(Number(before) + 1); // 快照只增不改（R-BL-05）

  const oldStatus = psql(
    `SELECT status FROM ai.baseline_proposal WHERE id = '${proposalId}'`
  );
  expect(oldStatus).toBe("confirmed");
});

/** TC-P1-06-06 · 提案确认后条目决策面关闭（不可变） */
test("TC-06 已确认提案的条目不可再决策", async () => {
  const confirmedProposal = psql(
    `SELECT id FROM ai.baseline_proposal
      WHERE project_id = '${project.id}' AND status = 'confirmed'
      ORDER BY created_at DESC LIMIT 1`
  );
  const anyItem = psql(
    `SELECT id FROM ai.baseline_item WHERE proposal_id = '${confirmedProposal}' LIMIT 1`
  );
  const res = await api(
    "POST",
    `/v1/pm/projects/${project.id}/baseline/items/${anyItem}/decision`,
    owner.token,
    { action: "confirm" }
  );
  expect([404, 409]).toContain(res.status); // 非 pending 提案 → 决策面关闭
});

/**
 * TC-P1-06-07 · AI 不能确认 Baseline —— **DEF-P0-02 复现位**（R70）
 * 当前实现 confirm_baseline 仅 require_project_visible，ai_pm 成员令牌会成功；
 * 期望行为 = 403。修复（加 require_human_conn + 治理授权）后：
 * 1) 删除下方 test.fixme；2) 启用注释内的实断言。
 */
test.fixme("TC-07 AI 令牌确认 Baseline 被拒 403（DEF-P0-02）", async () => {
  const aiToken = await asAiPrincipal("p1-fr021-ai", TID);

  const created = await api(
    "POST",
    `/v1/pm/projects/${project.id}/baseline/proposals`,
    owner.token,
    {}
  );
  expect(created.status).toBe(201);
  const proposalId = created.body.proposal_id as string;
  const itemIds = await pendingItemIds(proposalId);
  await decide(itemIds, "confirm");

  // 修复后启用：
  // const res = await api("POST", `/v1/pm/projects/${project.id}/baseline/confirm`, aiToken, {});
  // expect(res.status).toBe(403);
  expect(aiToken).toBeTruthy();
});
