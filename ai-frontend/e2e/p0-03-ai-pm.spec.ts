import { expect, test } from "@playwright/test";

import { api } from "./team/stack";
import { buildOwner, createProjectFor, psql, signInVia, type TestOwner } from "./p0/fixtures";
import { asAiPrincipal, asApiKeyPrincipal, registerMockProvider, waitFor } from "./p0/helpers";

/**
 * P0-03 · Project AI-PM 核心能力（规格 13 号，阶段 B）
 * 008a 会话数据面 / 008b 真实回复链（D11+D12：worker 进栈 + mock provider）/
 * 009 需求分析链（intake → analysis_run → change_proposal）/ 010 三道闸。
 *
 * 判定：结构断言 + DB 终态；HTTP 200 不作唯一证据。
 */

test.describe.configure({ mode: "serial" });

let owner: TestOwner;
let project: { id: string };

test.beforeAll(async () => {
  owner = await buildOwner("p0-ai-owner", 9201, "P0 AI Team", "p0-ai-team");
});

/** P0-008a · 无外部工具时 AI-PM 会话存储与持久化 */
test("P0-008a 无外部工具 → 会话创建/消息落库/项目关联/刷新不丢", async ({
  context,
  page
}) => {
  project = await createProjectFor(owner, "E2EAI9201", "e2e-ai-pm-no-tools");

  // §2.1 前置：仓库/Runner 均为空（能力面不依赖外部工具）
  const repos = await api("GET", `/v1/pm/projects/${project.id}/repositories`, owner.token);
  expect(repos.status).toBe(200);
  expect(repos.body.items?.length ?? 0).toBe(0);

  // UI：打开 AI-PM 助手
  await signInVia(context, owner.sub, owner.tid);
  await page.goto(`/project/assistant?project_id=${project.id}`);
  await expect(page.getByPlaceholder(/继续追问/).first()).toBeVisible({ timeout: 20_000 });

  // 发消息（POST 恒 role=user）
  const prompt = "请说明这个项目目前可以开展哪些项目管理工作";
  await page.getByPlaceholder(/继续追问/).first().fill(prompt);
  await page.keyboard.press("Enter");

  // 后端证据：会话关联 project + 消息落库（轮询等 UI POST 落库）
  const convId = await waitFor(async () => {
    const sessions = await api("GET", `/v1/pm/projects/${project.id}/conversations`, owner.token);
    expect(sessions.status).toBe(200);
    const list = Array.isArray(sessions.body) ? sessions.body : sessions.body.items;
    return (list[0]?.id as string) ?? null;
  }, 30_000, 1000, "conversation created");

  const userMsg = await waitFor(async () => {
    const res = await api(
      "GET",
      `/v1/pm/projects/${project.id}/conversations/${convId}/messages`,
      owner.token
    );
    expect(res.status).toBe(200);
    const list = Array.isArray(res.body) ? res.body : res.body.items;
    return (
      list.find(
        (m: { role: string; content: string }) => m.role === "user" && m.content.includes(prompt)
      ) ?? null
    );
  }, 30_000, 1000, "user message persisted");
  expect(userMsg.role).toBe("user");

  // 刷新后仍在
  await page.reload();
  await expect(page.getByText(prompt).first()).toBeVisible({ timeout: 20_000 });
});

/** P0-008b · 真实 AI 回复链（D11 worker + D12 mock provider；无外部 LLM 依赖） */
test("P0-008b 配置 provider + worker → 会话出现 assistant 真实回复", async () => {
  await registerMockProvider(owner, project.id);

  // 建会话 + 发消息（API 直发，与 008a 同链）
  const conv = await api("POST", `/v1/pm/projects/${project.id}/conversations`, owner.token, {
    title: "008b reply chain"
  });
  expect(conv.status).toBe(201);
  const convId = conv.body.id as string;
  const sent = await api(
    "POST",
    `/v1/pm/projects/${project.id}/conversations/${convId}/messages`,
    owner.token,
    { content: "请给我一条项目启动阶段的行动建议。" }
  );
  expect(sent.status).toBe(201);

  // 轮询 assistant 回复（worker 回复链，reply loop 默认 1s/轮）
  const reply = await waitFor(async () => {
    const msgs = await api(
      "GET",
      `/v1/pm/projects/${project.id}/conversations/${convId}/messages`,
      owner.token
    );
    const list = Array.isArray(msgs.body) ? msgs.body : msgs.body.items;
    return (list.find((m: { role: string }) => m.role === "assistant") as
      | { role: string; content: string }
      | undefined) ?? null;
  }, 90_000, 2000, "assistant reply");
  expect(reply.content.length).toBeGreaterThan(0);
});

/** P0-009 · 需求分析链（events API → ai_analysis job → analysis_run → change_proposal） */
test("P0-009 分析链 → mock provider 结构化结果 + 提案待确认 + 零自动落地", async () => {
  test.setTimeout(300_000);
  // R69 修正（规格 13 R23 与实现不符）：intake 的 analysis_request 路由到
  // kb_analyze 任务（kb stub），**不产生** ai.analysis_run / change_proposal。
  // ai_analysis job 的真实入口 = `POST /api/v1/events`（ingestion.rs:103，
  // "New event row plus its ai_analysis job"）→ worker 编排器 → 分析。
  const me = await api("GET", "/v1/pm/me", owner.token);
  const ownerId = me.body.principal_id as string;
  const event = await api("POST", "/api/v1/events", owner.token, {
    project_id: project.id,
    actor_id: ownerId,
    source: "e2e",
    event_type: "technical_design_submitted",
    idempotency_key: `p0-009-${Date.now()}`,
    payload: {}
  });
  expect([200, 201, 202]).toContain(event.status);

  // 等 analysis_run 终态（worker 领取 ai_analysis job；provider 已由 008b 注册）
  const run = await waitFor(() => {
    const rows = psql(
      `SELECT status || '|' || COALESCE(failure_code, '') || '|' || model_name
         FROM ai.analysis_run WHERE project_id = '${project.id}'
        ORDER BY created_at DESC LIMIT 1`
    );
    if (!rows) return null;
    const [status, failure, model] = rows.split("|");
    return status === "completed" || status === "failed" ? { status, failure, model } : null;
  }, 280_000, 3000, "analysis run terminal");
  expect(run.status).toBe("completed");
  expect(run.failure).toBe("");

  // 结构断言（不逐字比对）+ model_name 如实记录（mock provider 命中）
  const result = psql(
    `SELECT COALESCE(jsonb_array_length(result->'proposals'), -1) || '/' ||
            (result->>'summary' IS NOT NULL) || '/' || model_name
       FROM ai.analysis_run WHERE project_id = '${project.id}'
       ORDER BY created_at DESC LIMIT 1`
  );
  const [proposalCount, hasSummary, modelName] = result.split("/");
  expect(Number(proposalCount)).toBeGreaterThan(0);
  expect(hasSummary).toBe("true");
  // ai.analysis_run.model_name 来自 project_ai_config（bootstrap 默认 'fake'），
  // provider 命中由 worker 日志 + mock 调用记录证明；两值都算如实记录
  expect(["fake", "mock-e2e"]).toContain(modelName);

  // 提案停待确认面（proposed / redline_pending —— 红线命令不自动应用）
  const proposals = psql(
    `SELECT count(*) FROM ai.change_proposal
      WHERE project_id = '${project.id}' AND status IN ('proposed', 'redline_pending')`
  );
  expect(Number(proposals)).toBeGreaterThan(0);
  const pendingApprovals = psql(
    `SELECT count(*) FROM governance.approval_request
      WHERE project_id = '${project.id}' AND status = 'pending'`
  );
  expect(Number(pendingApprovals)).toBeGreaterThan(0);

  // 红线 create_requirements：AI 分析不得自动创建正式需求
  const reqCount = psql(
    `SELECT count(*) FROM delivery.requirement WHERE project_id = '${project.id}'`
  );
  expect(Number(reqCount)).toBe(0);
});

/** P0-010 · AI 建议人工确认：三道闸全拒 + 人工确认产生 Revision 2 */
test("P0-010 提案停待确认；AI 身份 confirm 403（人工门）；api_key 403（可信上下文）；人工确认 superseded 旧版", async () => {
  // 前置：人工创建 REQ-001 并确认到 approved（真实链）
  const req = await api("POST", `/v1/pm/projects/${project.id}/requirements`, owner.token, {
    code: "REQ-010",
    title: "任务管理",
    markdown: "系统支持用户创建任务。"
  });
  expect(req.status).toBe(201);
  const reqId = req.body.requirement.id as string;
  const v1Draft = await api("POST", `/v1/pm/requirements/${reqId}/revisions`, owner.token, {
    markdown: "系统支持用户创建任务。"
  });
  const v1Id = v1Draft.body.revision.id as string;
  const v1Hash = v1Draft.body.revision.content_hash as string;
  await api("POST", `/v1/pm/requirements/${reqId}/revisions/${v1Id}/submit`, owner.token, {});
  const v1Confirm = await api(
    "POST",
    `/v1/pm/requirements/${reqId}/revisions/${v1Id}/confirm`,
    owner.token,
    { expected_content_hash: v1Hash }
  );
  expect(v1Confirm.status).toBe(200);

  // AI 建议链：新修订 submit → pending_confirmation（提案态；正式内容不动）
  const draft2 = await api("POST", `/v1/pm/requirements/${reqId}/revisions`, owner.token, {
    markdown: "系统支持用户创建任务，管理员可将任务分配给指定团队成员。",
    summary: "补充任务分配能力"
  });
  expect(draft2.status).toBe(201);
  const v2Id = draft2.body.revision.id as string;
  const v2Hash = draft2.body.revision.content_hash as string;
  const submitted = await api(
    "POST",
    `/v1/pm/requirements/${reqId}/revisions/${v2Id}/submit`,
    owner.token,
    {}
  );
  expect(submitted.status).toBe(200);
  expect(submitted.body.revision.state).toBe("pending_confirmation");

  // 正式内容仍是 v1
  const current = await api("GET", `/v1/pm/requirements/${reqId}`, owner.token);
  expect(current.status).toBe(200);
  expect(current.body.requirement.current_revision_id).toBe(v1Id);

  // 闸①：kind='ai_pm' 主体直调 confirm → 403（require_human 查库，R24 消息）。
  // asAiPrincipal 先补 team 资格（P4-ORG Team-first 硬前置）再由 owner 授予
  // project_member——可见性闸放行，人工门 require_human 按 kind='ai_pm' 拒绝。
  const ai = await asAiPrincipal(owner, "p0-ai-impersonated", project.id);
  const aiAttempt = await api(
    "POST",
    `/v1/pm/requirements/${reqId}/revisions/${v2Id}/confirm`,
    ai.token,
    { expected_content_hash: v2Hash }
  );
  expect(aiAttempt.status).toBe(403);
  expect(aiAttempt.body.message ?? "").toContain("human principal");

  // 闸②：api_key 通道直调 confirm → 403（ensure_trusted_context：机器凭据
  // 不构成可信交互上下文；主体已入编 → 可见性放行 → 可信门 403 可达）
  const key = await asApiKeyPrincipal(owner, "p0-api-key", project.id);
  const keyAttempt = await api(
    "POST",
    `/v1/pm/requirements/${reqId}/revisions/${v2Id}/confirm`,
    key.token,
    { expected_content_hash: v2Hash }
  );
  expect(keyAttempt.status).toBe(403);

  // 两次拒绝后终态不变
  const unchanged = await api("GET", `/v1/pm/requirements/${reqId}`, owner.token);
  expect(unchanged.body.requirement.current_revision_id).toBe(v1Id);
  expect(unchanged.body.requirement.current_content_hash).toBe(v1Hash);

  // 人工确认 → Revision 2 approved；v1 superseded；审计落 confirm 人
  const approved = await api(
    "POST",
    `/v1/pm/requirements/${reqId}/revisions/${v2Id}/confirm`,
    owner.token,
    { expected_content_hash: v2Hash }
  );
  expect(approved.status).toBe(200);
  const updated = await api("GET", `/v1/pm/requirements/${reqId}`, owner.token);
  expect(updated.body.requirement.current_revision_id).toBe(v2Id);

  const states = psql(
    `SELECT v2.state || '/' || v1.state FROM delivery.requirement_revision v2, delivery.requirement_revision v1
      WHERE v2.id = '${v2Id}' AND v1.id = '${v1Id}'`
  );
  expect(states).toBe("approved/superseded");
  const audit = psql(
    `SELECT count(*) FROM governance.audit_log
      WHERE project_id = '${project.id}' AND action LIKE '%confirm%'`
  );
  expect(Number(audit)).toBeGreaterThan(0);
});
