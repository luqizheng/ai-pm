import { expect, test } from "@playwright/test";

import { api } from "./team/stack";
import {
  buildOwner,
  createProjectFor,
  psql,
  type TestOwner,
  type TestProject
} from "./p0/fixtures";
import { registerMockProvider, waitFor } from "./p0/helpers";

/**
 * P1-05 · BR-011 / ADR-041 零外部依赖 AI-PM（docs/e2e-spec/19 号 P1-05，增量）
 *
 * 锚点（R84/R85）：P0 栈已起真 ai-pm-worker（AI_PM_RUN_DISPATCH=on +
 * AI_PM_RUN_EXECUTOR=mock + AI_PM_CONVERSATION_REPLY=on）+ mock LLM。
 * 零依赖主体已被 P0-008a/b 覆盖 → 本组增量：mock provider 下 model_name
 * 如实记录（≠ fake）、回复链 assistant 落库、建议进人工确认、
 * provider 不可达语义（D54 → fixme）。
 */

test.describe.configure({ mode: "serial" });

const TID = 9541;
let owner: TestOwner;
let project: TestProject;

test.beforeAll(async () => {
  owner = await buildOwner("p1-br011-a", TID, "team-p1-br011", "p1z1");
  project = await createProjectFor(owner, "E2EP1Z1", "e2e-zero-deps");
  await registerMockProvider(owner, project.id); // 真 provider 面（mock 端点）
});

/** TC-P1-05-01 · 零外部工具创建项目 → CC 就绪 + AI 配置落库 */
test("TC-01 零依赖建项目：CC 可读 + ai.project_ai_config 就绪", async () => {
  const cc = await api(
    "GET",
    `/v1/pm/projects/${project.id}/command-center`,
    owner.token
  );
  expect(cc.status).toBe(200);

  const config = psql(
    `SELECT enabled::text, automation_mode
       FROM ai.project_ai_config WHERE project_id = '${project.id}'`
  );
  const [enabled, mode] = config.split("|");
  expect(enabled).toBe("true");
  expect(mode).toBe("ai_lead");
});

/** TC-P1-05-02 · mock provider 分析链：model_name 如实（≠ fake），结构化结果 */
test("TC-02 分析链走已配置 provider（model_name ≠ fake）", async () => {
  const me = await api("GET", "/v1/pm/me", owner.token);
  const event = await api("POST", "/api/v1/events", owner.token, {
    project_id: project.id,
    actor_id: me.body.principal_id,
    source: "e2e",
    event_type: "technical_design_submitted",
    idempotency_key: `p1-05-${Date.now()}`,
    payload: {}
  });
  expect([200, 201, 202]).toContain(event.status);

  const run = await waitFor(
    () => {
      const rows = psql(
        `SELECT status || '|' || COALESCE(failure_code, '') || '|' || model_name
         FROM ai.analysis_run WHERE project_id = '${project.id}'
        ORDER BY created_at DESC LIMIT 1`
      );
      if (!rows) return null;
      const [status, failure, model] = rows.split("|");
      return status === "completed" || status === "failed"
        ? { status, failure, model }
        : null;
    },
    280_000,
    3000,
    "analysis run terminal"
  );
  expect(run.status).toBe("completed");
  expect(run.failure).toBe("");
  expect(run.model.toLowerCase()).not.toContain("fake"); // mock provider 命中，如实记录
});

/** TC-P1-05-03 · 回复链（provider 可达）：assistant 消息落库 */
test("TC-03 会话回复链落库（assistant 消息）", async () => {
  const conv = await api(
    "POST",
    `/v1/pm/projects/${project.id}/conversations`,
    owner.token,
    {}
  );
  expect([200, 201]).toContain(conv.status);
  const convId = conv.body.id as string;

  await api(
    "POST",
    `/v1/pm/projects/${project.id}/conversations/${convId}/messages`,
    owner.token,
    {
      content: "请概述当前可以开展的项目管理工作"
    }
  );

  const reply = await waitFor(
    async () => {
      const msgs = await api(
        "GET",
        `/v1/pm/projects/${project.id}/conversations/${convId}/messages`,
        owner.token
      );
      const list = Array.isArray(msgs.body)
        ? msgs.body
        : (msgs.body.items ?? []);
      return list.find((m: { role: string }) => m.role === "assistant") ?? null;
    },
    180_000,
    3000,
    "assistant reply"
  );

  expect(reply).toBeTruthy();
});

/** TC-P1-05-04 · AI 建议进人工确认（submit → pending_confirmation） */
test("TC-04 需求建议停在人待确认态", async () => {
  const created = await api(
    "POST",
    `/v1/pm/projects/${project.id}/requirements`,
    owner.token,
    { code: "REQ-P1Z1", title: "零依赖分析产物", markdown: "# 零依赖分析产物" }
  );
  expect(created.status).toBe(201);
  const reqId = created.body.requirement.id as string;

  const draft = await api(
    "POST",
    `/v1/pm/requirements/${reqId}/revisions`,
    owner.token,
    {
      markdown: "# 零依赖分析产物（AI 建议版）"
    }
  );
  expect(draft.status).toBe(201);
  const revId = draft.body.revision.id as string;

  const submitted = await api(
    "POST",
    `/v1/pm/requirements/${reqId}/revisions/${revId}/submit`,
    owner.token,
    {}
  );
  expect(submitted.status).toBe(200);

  const state = psql(
    `SELECT state FROM delivery.requirement_revision WHERE id = '${revId}'`
  );
  expect(state).toBe("pending_confirmation"); // 不自动转正（三道闸）

  // 人工确认收尾（哈希门）
  const confirmed = await api(
    "POST",
    `/v1/pm/requirements/${reqId}/revisions/${revId}/confirm`,
    owner.token,
    { expected_content_hash: draft.body.revision.content_hash }
  );
  expect(confirmed.status).toBe(200);
});

/** TC-P1-05-05 · provider 不可达 → 明确失败，不回退 fake 冒充（D54 待构造） */
test.fixme("TC-05 provider 不可达时明确失败（D54）", async () => {
  // 构造：注册指向死端口的 provider 并绑定为项目策略 → 触发分析
  // 期望：analysis_run failed + failure_code 记录；不产生 fake 冒充的成功
});
