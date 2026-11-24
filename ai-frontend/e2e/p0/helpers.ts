import { execFileSync } from "node:child_process";
import { createPrivateKey, sign } from "node:crypto";
import { existsSync, mkdtempSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { api, signToken } from "../team/stack";
import { P0_DB, P0_PG_PORT, psql, type PlanTaskSpec, type TestOwner } from "./fixtures";

/**
 * P0 阶段 B~D 共享助手（worker 异步链等待 / 身份注入 / mock provider /
 * 本地仓夹具 / 计划物化链 / 需求确认链 / 服务层交付提交）。
 */

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(HERE, "../../..");

/** 轮询直到 fn 返回真值或超时（worker 异步链的等待原语）。 */
export async function waitFor<T>(
  fn: () => Promise<T | null | undefined> | T | null | undefined,
  timeoutMs: number,
  intervalMs = 1000,
  label = "condition"
): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  let lastError: unknown = null;
  while (Date.now() < deadline) {
    try {
      const value = await fn();
      if (value !== null && value !== undefined) return value;
    } catch (error) {
      lastError = error;
    }
    await new Promise(r => setTimeout(r, intervalMs));
  }
  throw new Error(
    `waitFor(${label}) timeout after ${timeoutMs}ms${lastError ? `; last error: ${String(lastError)}` : ""}`
  );
}

/**
 * 自定义 claims 的自签令牌：MCP 权限面（PM_MCP_READ 等）与
 * api_key 通道（purpose=api_key → ensure_trusted_context 403）的钥匙。
 */
export function signTokenWith(
  sub: string,
  tid: number,
  claims: { permissions?: string[]; purpose?: string }
): string {
  const keys = resolve(HERE, "../../../apps/pm-api/tests/keys");
  const b64url = (input: Buffer | string): string => Buffer.from(input).toString("base64url");
  const now = Math.floor(Date.now() / 1000);
  const header = b64url(JSON.stringify({ alg: "RS256", typ: "JWT", kid: "default" }));
  const payload = b64url(
    JSON.stringify({
      sub,
      tid,
      username: sub,
      roles: [],
      permissions: claims.permissions ?? [],
      ...(claims.purpose ? { purpose: claims.purpose } : {}),
      aud: "coder-micro-services",
      iat: now,
      exp: now + 3600
    })
  );
  const signature = sign(
    "RSA-SHA256",
    Buffer.from(`${header}.${payload}`),
    createPrivateKey(readFileSync(resolve(keys, "private.pem"), "utf8"))
  );
  return `${header}.${payload}.${b64url(signature)}`;
}

/**
 * 把一个 provisioned 主体临时改成 kind='ai_pm' 并激活（规格 13 号 R24 负向：
 * `require_human_conn` 查库判定 kind，非 human → 403）。
 *
 * 三步顺序有讲究（都按实现事实源）：
 *  1. `GET /v1/pm/me` 触发 provision（identity + tenant_member 行随之产生，
 *     auth.rs:621）；
 *  2. **先按 human 授予项目成员资格**（`POST /v1/pm/projects/{id}/members`）——
 *     grant 端点只允许 human 主体（project_member.rs:265 `AGENT_PRINCIPAL`），
 *     且可见性检查（project_visible）先于人工门执行：非成员直调只会拿到 404，
 *     根本走不到 403 那一行；
 *  3. 再翻 kind='ai_pm' + is_active=true（require_active 先于 require_human，
 *     不激活会在权限层 403「principal is not activated」而非真人门 403）。
 *
 * 只对测试自建主体做注入，不触碰任何业务行。
 */
/**
 * P4-ORG Team-first：grant 的硬前置是目标主体的 `core.team_member` 行
 * （provision 只写 tenant_member，不再自动入队）。测试身份注入风格下，
 * 先给主体补 owner 所在 Team 的 'member' 行，grant 才可能成功、
 * require_human 的 403 人工门才在 REST 全链可达。
 */
function seedTeamMembership(owner: TestOwner, principalId: string): void {
  psql(
    `INSERT INTO core.team_member (team_id, principal_id, tenant_id, role, granted_by)
      SELECT tm.team_id, '${principalId}', tm.tenant_id, 'member', tm.principal_id
        FROM core.team_member tm
        JOIN core.principal_identity pi ON pi.principal_id = tm.principal_id
       WHERE pi.subject = '${owner.sub}'
         AND tm.role = 'owner'
       ON CONFLICT (team_id, principal_id) DO NOTHING`
  );
}

export async function asAiPrincipal(
  owner: TestOwner,
  sub: string,
  projectId?: string
): Promise<{ token: string; principalId: string }> {
  const token = signToken(sub, owner.tid);
  // 先触发 provision（identity 行随首次 API 调用产生），再做身份注入
  const me = await api("GET", "/v1/pm/me", token);
  const principalId = me.body.principal_id as string;
  if (!principalId) throw new Error(`principal not provisioned for ${sub}`);
  if (projectId) {
    // 授予方必须是项目管理者（真人 owner）——用 owner 令牌，不是 AI 主体自己的
    seedTeamMembership(owner, principalId);
    const granted = await api("POST", `/v1/pm/projects/${projectId}/members`, owner.token, {
      principal_id: principalId
    });
    // 200/201 = 授予成功；409 = 已是成员（幂等语义）
    if (![200, 201, 409].includes(granted.status))
      throw new Error(
        `grant project membership failed: ${granted.status} ${JSON.stringify(granted.body)}`
      );
  }
  const updated = psql(
    `WITH me AS (
       SELECT pi.principal_id FROM core.principal_identity pi
        WHERE pi.subject = '${sub}' LIMIT 1)
     UPDATE core.principal SET kind = 'ai_pm', is_active = true
      WHERE id = (SELECT principal_id FROM me)
     RETURNING kind`
  );
  if (updated !== "ai_pm") throw new Error(`failed to flip principal kind: '${updated}'`);
  return { token, principalId };
}

/**
 * api_key 通道主体：provision → 补 team 资格 → owner 授予项目成员。
 * **不翻转 kind**（保持 human）：可信交互门（ensure_trusted_context）按
 * auth_mode='api_key' 拒绝——成员资格让可见性闸放行后，403 在 REST 可达。
 */
export async function asApiKeyPrincipal(
  owner: TestOwner,
  sub: string,
  projectId?: string
): Promise<{ token: string; principalId: string }> {
  const token = signTokenWith(sub, owner.tid, { purpose: "api_key" });
  const me = await api("GET", "/v1/pm/me", token);
  const principalId = me.body.principal_id as string;
  if (!principalId) throw new Error(`principal not provisioned for ${sub}`);
  if (projectId) {
    seedTeamMembership(owner, principalId);
    const granted = await api("POST", `/v1/pm/projects/${projectId}/members`, owner.token, {
      principal_id: principalId
    });
    if (![200, 201, 409].includes(granted.status))
      throw new Error(
        `grant project membership failed: ${granted.status} ${JSON.stringify(granted.body)}`
      );
  }
  return { token, principalId };
}

/**
 * P4-ORG Phase 2：Team 锚 = core.team.id（team_id，不再是 tenant_id）。
 * 建团后从 /v1/pm/me memberships[0].team_id 取（owner 恰好一条 active
 * membership；不读 owner.team.id——那是旧 tenant 锚）。
 */
async function teamIdOfOwner(owner: TestOwner): Promise<string> {
  const me = await api("GET", "/v1/pm/me", owner.token);
  const teamId = me.body?.memberships?.[0]?.team_id as string | undefined;
  if (!teamId) throw new Error(`no team membership for ${owner.sub}`);
  return teamId;
}

/** 注册 mock LLM provider 并绑定为项目策略。
 * provider_type 用 ollama：mock LLM 落在 127.0.0.1（回环），§17 SSRF 防护
 * （2026-11-11 D-3）只放行公网端点，ollama 类型按 2026-11-12 产品裁决豁免
 * （自托管推理服务合法落 localhost）。调用侧所有类型统一走 OpenAI 兼容
 * /chat/completions 面，类型取值不改变运行时行为。 */
export async function registerMockProvider(
  owner: TestOwner,
  projectId: string
): Promise<{ providerId: string }> {
  const teamId = await teamIdOfOwner(owner);
  const endpoint = process.env.P0_MOCK_LLM_URL;
  if (!endpoint) throw new Error("P0_MOCK_LLM_URL missing (global-setup must run)");
  // 真 LLM 模式（P0_REAL_LLM_URL）下模型名来自 P0_LLM_MODEL（如 qwen2.5:3b）
  const modelName = process.env.P0_LLM_MODEL ?? "mock-e2e";
  const created = await api("POST", `/v1/pm/teams/${teamId}/llm-providers`, owner.token, {
    name: "mock-e2e",
    provider_type: "ollama", // 回环端点豁免（见函数注释）
    endpoint,
    configuration: { model: modelName },
    api_key: "e2e-mock-key",
    timeout_secs: 30
  });
  if (created.status !== 201)
    throw new Error(`provider create failed: ${created.status} ${JSON.stringify(created.body)}`);
  const providerId = created.body.id as string;

  // 状态门：启用 provider（status API；乐观锁 expected_revision，409 重试一次）
  let enabled = false;
  for (let attempt = 0; attempt < 3 && !enabled; attempt += 1) {
    const list = await api("GET", `/v1/pm/teams/${teamId}/llm-providers`, owner.token);
    const row = (list.body.items ?? list.body ?? []).find(
      (item: { id: string }) => item.id === providerId
    );
    if (row?.status === "enabled") {
      enabled = true;
      break;
    }
    const status = await api(
      "POST",
      `/v1/pm/teams/${teamId}/llm-providers/${providerId}/status`,
      owner.token,
      { action: "enable", expected_revision: row?.revision ?? 0 }
    );
    if ([200, 201].includes(status.status)) enabled = true;
    else if (status.status !== 409)
      throw new Error(`provider enable failed: ${status.status} ${JSON.stringify(status.body)}`);
  }
  if (!enabled) throw new Error("provider enable failed after retries");

  const policyNow = await api("GET", `/v1/pm/projects/${projectId}/ai-model-policies`, owner.token);
  // Phase 3A 视图：revision 在 provider_constraint_revision；无策略行时为
  // undefined，与 set_project_ai_policy 的 CAS「无行 = 1」同口径取 1。
  const policyRevision = policyNow.body?.provider_constraint_revision ?? 1;
  const policy = await api("PUT", `/v1/pm/projects/${projectId}/ai-model-policies`, owner.token, {
    provider_id: providerId,
    expected_revision: policyRevision
  });
  if (![200, 201].includes(policy.status))
    throw new Error(`project policy bind failed: ${policy.status} ${JSON.stringify(policy.body)}`);

  // Phase 3A 模型解析四级链（team_llm_catalog::resolve_model_for_role）：
  // 项目覆盖 → Team Role 策略 → Team general 兜底 → general_policy_missing。
  // 仅绑 provider 不够，还须登记 Team 模型行并把 ai_pm_lead / general 两个
  // 角色指向它（会话回复解析走 general，分析链走 ai_pm_lead）。
  const modelCreated = await api(
    "POST",
    `/v1/pm/teams/${teamId}/llm-models`,
    owner.token,
    {
      provider_id: providerId,
      model_id: modelName,
      display_name: modelName
    }
  );
  if (![200, 201].includes(modelCreated.status))
    throw new Error(
      `team model create failed: ${modelCreated.status} ${JSON.stringify(modelCreated.body)}`
    );
  const modelRowId = modelCreated.body.id as string;

  // 模型行默认 disabled（created.enabled=false）——解析校验拒绝禁用模型，
  // 先 GET 列表取 revision 再 PATCH 启用（乐观锁，409 重试一次）。
  let modelEnabled = false;
  for (let attempt = 0; attempt < 3 && !modelEnabled; attempt += 1) {
    const models = await api(
      "GET",
      `/v1/pm/teams/${teamId}/llm-models`,
      owner.token
    );
    const row = (models.body.items ?? models.body ?? []).find(
      (item: { id: string }) => item.id === modelRowId
    );
    if (row?.enabled) {
      modelEnabled = true;
      break;
    }
    const patched = await api(
      "PATCH",
      `/v1/pm/teams/${teamId}/llm-models/${modelRowId}`,
      owner.token,
      { expected_revision: row?.revision ?? 0, enabled: true }
    );
    if ([200, 201].includes(patched.status)) modelEnabled = true;
    else if (patched.status !== 409)
      throw new Error(
        `model enable failed: ${patched.status} ${JSON.stringify(patched.body)}`
      );
  }
  if (!modelEnabled) throw new Error("model enable failed after retries");

  // 授权模型给项目使用（grant；未授权模型配策略被 400 拒绝）
  const granted = await api(
    "PUT",
    `/v1/pm/teams/${teamId}/llm-model-authorizations`,
    owner.token,
    { items: [{ model_id: modelRowId, action: "grant" }] }
  );
  if (![200, 201].includes(granted.status))
    throw new Error(
      `model authorization failed: ${granted.status} ${JSON.stringify(granted.body)}`
    );
  const policies = await api(
    "PUT",
    `/v1/pm/teams/${teamId}/llm-model-policies`,
    owner.token,
    {
      items: [
        { role_key: "ai_pm_lead", model_id: modelRowId },
        { role_key: "general", model_id: modelRowId }
      ]
    }
  );
  if (![200, 201].includes(policies.status))
    throw new Error(
      `team model policies set failed: ${policies.status} ${JSON.stringify(policies.body)}`
    );
  return { providerId };
}

export interface LocalRepo {
  repositoryId: string;
  head: string;
  hostDir: string;
}

/**
 * 本地 file:// 仓库夹具并绑定为项目 verified 仓库。
 * API 注册用合法占位 url（file:// 被 URL 硬化拒，规格 12 R-口径），psql 把
 * url 指向宿主 file:// + 置 verified（imports_e2e 同款偏差留痕；worker 与本
 * 进程同宿主机，file:// 对执行器/事实采集/交付验证均可达）。
 */
export async function createLocalRepoFixture(
  owner: TestOwner,
  projectId: string,
  name: string,
  files: Record<string, string> = {}
): Promise<LocalRepo> {
  const hostDir = mkdtempSync(join(tmpdir(), `p0-repo-${name}-`));
  const gitEnv = {
    ...process.env,
    GIT_AUTHOR_NAME: "Fixture",
    GIT_AUTHOR_EMAIL: "fixture@e2e.test",
    GIT_COMMITTER_NAME: "Fixture",
    GIT_COMMITTER_EMAIL: "fixture@e2e.test"
  };
  const git = (args: string[]) =>
    execFileSync("git", ["-C", hostDir, ...args], { encoding: "utf8", env: gitEnv }).trim();
  execFileSync("git", ["init", "-q", "-b", "main", hostDir]);
  writeFileSync(join(hostDir, "README.md"), `# ${name}\n\nP0 E2E fixture repo.\n`);
  for (const [path, content] of Object.entries(files)) {
    mkdirSync(dirname(join(hostDir, path)), { recursive: true });
    writeFileSync(join(hostDir, path), content);
  }
  execFileSync("git", ["-C", hostDir, "add", "-A"]);
  git(["-c", "commit.gpgsign=false", "commit", "-q", "-m", "init"]);
  const head = git(["rev-parse", "HEAD"]);

  const created = await api("POST", `/v1/pm/projects/${projectId}/repositories`, owner.token, {
    name,
    url: `git@local:${name}.git`,
    role: "main",
    default_branch: "main"
  });
  if (created.status !== 201)
    throw new Error(`repo create failed: ${created.status} ${JSON.stringify(created.body)}`);
  const repositoryId = created.body.id as string;
  const fileUrl = `file:///${hostDir.replace(/\\/g, "/").replace(/^\//, "")}`;
  psql(
    `UPDATE integration.project_repository
        SET url = '${fileUrl}', status = 'verified', default_branch = 'main',
            head_commit_sha = '${head}'
      WHERE id = '${repositoryId}'`
  );
  return { repositoryId, head, hostDir };
}

/**
 * 取项目的**执行主体**（dev_agent）。
 *
 * 事实源（crates/pm-application/src/bootstrap.rs:386 `ensure_tenant_agents`）：
 * 项目 bootstrap 时按租户创建 architect/developer/tester 三个 `dev_agent`
 * 主体，并写成 `core.project_member` 行（role 同名）。
 *
 * 为什么必须是它：可领取性判定第 4 条（run_dispatch/policy.rs:179）
 * `assignee_is_executor` 要求承接人 `core.principal.kind = 'dev_agent'`——
 * 派给真人（kind='human'）时 Run 会被判 `no_executor_assignee` 退避回队，
 * 永远不进入执行。真实链路上承接人由 Role 版本解析（ai_roles.rs:393），
 * E2E 直接取 bootstrap 播种好的 developer 主体，语义等价。
 */
export function resolveExecutorPrincipal(projectId: string): string {
  const principalId = psql(
    `SELECT am.principal_id FROM ai.project_agent_member am
       JOIN core.principal p ON p.id = am.principal_id
      WHERE am.project_id = '${projectId}' AND am.role_kind = 'developer'
        AND p.kind = 'dev_agent' AND p.is_active
      LIMIT 1`
  );
  if (!principalId)
    throw new Error(`no dev_agent executor principal in project ${projectId}`);
  return principalId;
}

/** 计划全链：create → submit → approvals → decide(approve) → materialize。 */
export async function materializePlan(
  owner: TestOwner,
  projectId: string,
  tasks: PlanTaskSpec[]
): Promise<{ taskIdByPlanKey: Record<string, string> }> {
  const plan = await api("POST", `/v1/pm/projects/${projectId}/task-plans`, owner.token, {
    title: `P0 计划 ${projectId.slice(0, 8)}`,
    tasks
  });
  if (plan.status !== 201)
    throw new Error(`plan create failed: ${plan.status} ${JSON.stringify(plan.body)}`);
  const planId = plan.body.plan.id as string;
  const revisionId = plan.body.revisions[0].id as string;

  const submitted = await api(
    "POST",
    `/v1/pm/task-plans/${planId}/revisions/${revisionId}/submit`,
    owner.token,
    {}
  );
  if (submitted.status !== 200)
    throw new Error(`plan submit failed: ${submitted.status} ${JSON.stringify(submitted.body)}`);

  const approval = await api("POST", `/v1/pm/task-plans/${planId}/approvals`, owner.token, {});
  if (approval.status !== 201)
    throw new Error(
      `plan approval request failed: ${approval.status} ${JSON.stringify(approval.body)}`
    );

  const decided = await api(
    "POST",
    `/v1/pm/design-approvals/${approval.body.approval.id}/decide`,
    owner.token,
    { decision: "approve", reason: "P0 E2E 批准" }
  );
  if (decided.status !== 200)
    throw new Error(`plan decide failed: ${decided.status} ${JSON.stringify(decided.body)}`);

  const materialized = await api("POST", `/v1/pm/task-plans/${planId}/materialize`, owner.token, {
    plan_revision_id: revisionId,
    task_keys: tasks.map(task => task.key)
  });
  if (materialized.status !== 200)
    throw new Error(
      `materialize failed: ${materialized.status} ${JSON.stringify(materialized.body)}`
    );

  const taskIdByPlanKey: Record<string, string> = {};
  const executorId = resolveExecutorPrincipal(projectId);
  for (const item of materialized.body.created) {
    // 调度前置：承接人必须是 dev_agent（可领取性判定第 4 条，见
    // resolveExecutorPrincipal 注释）；真人承接会被判 no_executor_assignee。
    const detail = await api("GET", `/v1/pm/tasks/${item.task_id}`, owner.token);
    if (detail.status !== 200)
      throw new Error(`task fetch failed: ${detail.status} ${JSON.stringify(detail.body)}`);
    const task = detail.body.task ?? detail.body;
    const patched = await api("PATCH", `/v1/pm/tasks/${item.task_id}`, owner.token, {
      title: task.title,
      description: task.description ?? undefined,
      assignee_id: executorId,
      status: "draft",
      priority: task.priority ?? "normal",
      revision: task.revision
    });
    if (patched.status !== 200)
      throw new Error(`assignee patch failed: ${patched.status} ${JSON.stringify(patched.body)}`);
    taskIdByPlanKey[item.key] = item.task_id;
  }
  return { taskIdByPlanKey };
}

export interface RequirementHandle {
  id: string;
  currentRevisionId: string;
  contentHash: string;
}

/** 需求全链：create → revision(draft) → submit（投影绑定）→ confirm（哈希门）。 */
export async function createAndConfirmRequirement(
  owner: TestOwner,
  projectId: string,
  code: string,
  title: string,
  markdown: string
): Promise<RequirementHandle> {
  const created = await api("POST", `/v1/pm/projects/${projectId}/requirements`, owner.token, {
    code,
    title,
    markdown
  });
  if (created.status !== 201)
    throw new Error(
      `requirement create failed: ${created.status} ${JSON.stringify(created.body)}`
    );
  const reqId = created.body.requirement.id as string;
  const handle = await confirmDraftRevision(owner, reqId, markdown);
  return { ...handle, id: reqId };
}

/** 对现有需求起草 → 提交 → 人工确认（哈希门）。 */
export async function confirmDraftRevision(
  owner: TestOwner,
  reqId: string,
  markdown: string,
  summary?: string
): Promise<RequirementHandle> {
  const draft = await api("POST", `/v1/pm/requirements/${reqId}/revisions`, owner.token, {
    markdown,
    summary
  });
  if (draft.status !== 201)
    throw new Error(`revision create failed: ${draft.status} ${JSON.stringify(draft.body)}`);
  const revId = draft.body.revision.id as string;
  const hash = draft.body.revision.content_hash as string;
  const submitted = await api(
    "POST",
    `/v1/pm/requirements/${reqId}/revisions/${revId}/submit`,
    owner.token,
    {}
  );
  if (submitted.status !== 200)
    throw new Error(`revision submit failed: ${submitted.status} ${JSON.stringify(submitted.body)}`);
  const confirmed = await api(
    "POST",
    `/v1/pm/requirements/${reqId}/revisions/${revId}/confirm`,
    owner.token,
    { expected_content_hash: hash }
  );
  if (confirmed.status !== 200)
    throw new Error(
      `revision confirm failed: ${confirmed.status} ${JSON.stringify(confirmed.body)}`
    );
  return { id: reqId, currentRevisionId: revId, contentHash: hash };
}

/** 任务绑仓（P0-020 多仓交付的关联面）。 */
export async function bindTaskRepositories(
  owner: TestOwner,
  taskId: string,
  repositoryIds: string[]
): Promise<void> {
  const bound = await api("POST", `/v1/pm/tasks/${taskId}/repositories`, owner.token, {
    entries: repositoryIds.map(repositoryId => ({ repository_id: repositoryId, write_access: true }))
  });
  if (![200, 201].includes(bound.status))
    throw new Error(`task repo bind failed: ${bound.status} ${JSON.stringify(bound.body)}`);
}

/**
 * 提交知识提案（**必过质量门**）。
 *
 * 质量门 8 规则（crates/pm-application/src/knowledge_quality.rs）里与 E2E
 * 相关的三条硬约束：
 *  · 规则 2「来源」：`source_ref` / `source_event_id` / `analysis_run_id` 至少
 *    有一个，否则门禁直接 rejected；
 *  · 规则 6「引用」：正文必须含 `source_events:` 或 `projects/` 锚点；
 *  · 规则 8「索引」：正文至少一个 Markdown 标题。
 *
 * 状态语义（submit.rs:144）：门禁不过 → `rejected`；声明红线 → `redline_pending`
 * （等人工确认，**不写 Git**）；否则 → 直接 `committed`（自动发布）。
 * 要断言「提案 ≠ 正式知识、确认后才发布」，必须声明红线。
 */
export async function submitKnowledgeProposal(
  owner: TestOwner,
  input: {
    projectId: string;
    docKey: string;
    markdown: string;
    changeSummary: string;
    createdBy: string;
    redlineKind?: string;
    baseRevisionId?: string;
    operationId?: string;
  }
): Promise<{ status: number; outcome: string; proposalId?: string; body: Record<string, unknown> }> {
  const sourceRef = `projects/${input.projectId}/kb/${input.docKey}`;
  const markdown = input.markdown.includes("source_events:") || input.markdown.includes("projects/")
    ? input.markdown
    : `${input.markdown}\n\n来源：${sourceRef}\n`;
  const res = await api("POST", "/v1/pm/documents/proposals", owner.token, {
    project_id: input.projectId,
    doc_key: input.docKey,
    doc_type: "knowledge",
    markdown,
    change_summary: input.changeSummary,
    source_ref: sourceRef,
    ...(input.redlineKind ? { redline_kind: input.redlineKind } : {}),
    ...(input.baseRevisionId ? { base_revision_id: input.baseRevisionId } : {}),
    operation_id: input.operationId ?? `p0-kb-${input.docKey}-${Date.now()}`,
    created_by: input.createdBy
  });
  const body = (res.body ?? {}) as Record<string, unknown>;
  const outcome = String(body.outcome ?? "");
  const proposalId = (body.proposal_id ?? body.id) as string | undefined;
  return { status: res.status, outcome, proposalId, body };
}

export interface SubmitOutcomeView {
  outcome: "accepted" | "rejected" | "error";
  raw: string;
}

export interface FakeResult {
  lease_token: string;
  lease_generation: number;
  repos: Array<{ dir: string; sha: string }>;
}

/** Run 目录（workspace root 下的 run_id 目录）。 */
const runDir = (runId: string): string => join(process.env.P0_WORKSPACE_ROOT ?? "", runId);

/**
 * 等 Run 进入活跃态（claimed/running）且执行脚本已写出 fake-result.json。
 *
 * fake-result.json 里有 claim 链下发的**租约明文**（ADR-0002 L1/L2：只经环境
 * 变量下发，不落 pid 文件/日志）——交付提交必须用它。
 */
export async function waitForRunningRun(
  taskId: string,
  timeoutMs = 120_000
): Promise<{ runId: string; fake: FakeResult }> {
  return waitFor(() => {
    const rows = psql(
      `SELECT id || '|' || status FROM delivery.task_run
        WHERE task_id = '${taskId}' ORDER BY created_at DESC LIMIT 1`
    );
    if (!rows) return null;
    const [runId, status] = rows.split("|");
    if (status !== "running" && status !== "claimed") return null;
    const resultPath = join(runDir(runId), "fake-result.json");
    if (!existsSync(resultPath)) return null;
    return {
      runId,
      fake: JSON.parse(readFileSync(resultPath, "utf8")) as FakeResult
    };
  }, timeoutMs, 1000, "run running with fake result");
}

/**
 * 放行执行脚本（它退出 → Run 收尾 → outcome=succeeded）。
 *
 * 交付提交必须在放行**之前**完成：租约只在 Run 处于 claimed/running 时有效
 * （run_context.rs 的 lease 校验：status ∈ {claimed, running} 且 token_hash 未清）。
 */
export function releaseRun(runId: string): void {
  writeFileSync(join(runDir(runId), "fake-release"), "release");
}

/**
 * 服务层交付提交（平台自己的 submit_delivery —— cargo example 薄封装）。
 *
 * 三种结果：
 *  - `accepted`：交付被接受（commit 行落库 + 任务推进）；
 *  - `rejected`：业务拒绝（example exit 0，outcome=rejected）或服务层拒绝
 *    （example exit 2，stderr 带 `delivery error: <code>`——如终态 Run 的
 *    `lease_invalid`）；
 *  - `error`：非业务性失败（编译/连接问题）。
 */
export async function submitRunDelivery(job: {
  project_id: string;
  run_id: string;
  lease_token: string;
  lease_generation: number;
  repositories: Array<{
    repository_id: string;
    base_commit: string;
    commit_sha: string;
    changed_files: number;
    tests: Array<{ name: string; passed: boolean; evidence_ref?: string }>;
  }>;
  summary: string;
}): Promise<SubmitOutcomeView> {
  const jobPath = join(mkdtempSync(join(tmpdir(), "p0-submit-")), "job.json");
  writeFileSync(jobPath, JSON.stringify(job));
  let stdout = "";
  let stderr = "";
  try {
    stdout = execFileSync(
      "cargo",
      ["run", "-q", "-p", "pm-application", "--example", "submit_run_result", "--", jobPath],
      {
        cwd: REPO_ROOT,
        encoding: "utf8",
        env: {
          ...process.env,
          DATABASE_URL: `postgres://aipm:aipm@127.0.0.1:${P0_PG_PORT}/${P0_DB}`,
          AI_PM_SECRET_KEY: process.env.AI_PM_SECRET_KEY ?? "e2e-p0-secret",
          AI_PM_WORKSPACE_ROOT: process.env.P0_WORKSPACE_ROOT ?? "",
          NO_PROXY: "127.0.0.1,localhost"
        }
      }
    );
  } catch (error) {
    const failure = error as { stdout?: string; stderr?: string };
    stdout = failure.stdout ?? "";
    stderr = failure.stderr ?? "";
  }
  const lines = stdout.trim().split(/\r?\n/).filter(Boolean);
  const jsonLine = lines[lines.length - 1] ?? "";
  let outcome = "";
  try {
    outcome = (JSON.parse(jsonLine) as { outcome?: string }).outcome ?? "";
  } catch {
    outcome = "";
  }
  if (!outcome) {
    // 服务层错误（exit 2）：按业务拒绝计——凭据/终态/基线类拒绝码。
    const code = /delivery error: ([a-z_]+)/.exec(stderr)?.[1] ?? "";
    outcome = code ? "rejected" : "error";
  }
  const raw = `${stdout}\n${stderr}`.trim();
  if (outcome !== "accepted" && outcome !== "rejected" && outcome !== "error")
    throw new Error(`unexpected submit outcome: ${raw}`);
  return { outcome, raw };
}
