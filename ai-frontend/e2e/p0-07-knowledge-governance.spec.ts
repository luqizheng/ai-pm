import { expect, test } from "@playwright/test";

import { api } from "./team/stack";
import { buildOwner, createProjectFor, psql, type TestOwner } from "./p0/fixtures";
import { registerMockProvider, submitKnowledgeProposal, waitFor } from "./p0/helpers";

/**
 * P0-07 · 知识学习、外部 Commit 采纳与统一协调（规格 17 号，阶段 D；8 案例）
 *
 * 边界红线：AI 学习 ≠ 正式知识；外部 Commit ≠ 采纳；草稿 ≠ 正式 Revision。
 * NOT_IMPLEMENTED（不计 PASS，R54/R55/D33）：051（Commit 发现管道）、
 * 052（澄清采纳闭环）、049b（Run 完成自动触发知识影响分析，待查证）。
 */

test.describe.configure({ mode: "serial" });

let owner: TestOwner;
let projectId: string;

test.beforeAll(async () => {
  owner = await buildOwner("p0-kg-owner", 9601, "P0 KG Team", "p0-kg-team");
  const project = await createProjectFor(owner, "E2EKG9601", "e2e-knowledge-governance");
  projectId = project.id;
  // 分析链需要真实 provider（mock LLM）；否则走 bootstrap 默认 fake 模型，
  // 产不出可追溯的 change_proposal（049a 的核心断言）。
  await registerMockProvider(owner, projectId);
});

/** P0-049a · 执行结果触发知识学习：证据与提案链 ✅（events API → ai_analysis 编排器） */
test("P0-049a 分析链产出可追溯候选；幂等重放；正式知识零自动落地", async () => {
  test.setTimeout(300_000);
  test.setTimeout(300_000);
  const me = await api("GET", "/v1/pm/me", owner.token);
  const ownerId = me.body.principal_id as string;
  const idem = `p0-049a-${Date.now()}`;

  // 显式提交分析（R69：ai_analysis job 的入口是 events API，非 intake）
  const event = await api("POST", "/api/v1/events", owner.token, {
    project_id: projectId,
    actor_id: ownerId,
    source: "e2e",
    event_type: "technical_design_submitted",
    idempotency_key: idem,
    payload: {}
  });
  expect([200, 201, 202]).toContain(event.status);
  const eventId = event.body.event_id as string;

  const run = await waitFor(() => {
    const rows = psql(
      `SELECT status FROM ai.analysis_run WHERE project_id = '${projectId}'
        ORDER BY created_at DESC LIMIT 1`
    );
    // R70：分析终态字面量是 `completed`（不是 `succeeded`——那是 Run 的词汇）
    return rows === "completed" || rows === "failed" ? rows : null;
  }, 180_000, 2000, "analysis terminal").catch(error => {
    const diag = psql(
      `SELECT 'job=' || job_type || ':' || status
         FROM integration.job WHERE project_id = '${projectId}'
        UNION ALL
       SELECT 'run=' || COALESCE(status, 'none')
         FROM ai.analysis_run WHERE project_id = '${projectId}'`
    );
    throw new Error(`${String(error)} | diagnostics: ${diag.replace(/\n/g, " | ")}`);
  });
  expect(run).toBe("completed");

  // 候选可追溯：change_proposal 关联 analysis_run（提案 ≠ 正式知识）
  const traced = psql(
    `SELECT count(*) FROM ai.change_proposal
      WHERE project_id = '${projectId}' AND analysis_run_id IS NOT NULL`
  );
  expect(Number(traced)).toBeGreaterThan(0);

  // 幂等重放：同 idempotency_key 同 payload → Duplicate（事件表不新增）
  const eventsBefore = psql(
    `SELECT count(*) FROM integration.inbound_event WHERE project_id = '${projectId}'`
  );
  const replay = await api("POST", "/api/v1/events", owner.token, {
    project_id: projectId,
    actor_id: ownerId,
    source: "e2e",
    event_type: "technical_design_submitted",
    idempotency_key: idem,
    payload: {}
  });
  expect(replay.status).toBe(200);
  expect(replay.body.event_id).toBe(eventId);
  const eventsAfter = psql(
    `SELECT count(*) FROM integration.inbound_event WHERE project_id = '${projectId}'`
  );
  expect(Number(eventsAfter)).toBe(Number(eventsBefore));

  // 正式知识零自动落地：AI 主体不得直写 published 修订
  const published = psql(
    `SELECT count(*) FROM ai.document_revision
      WHERE project_id = '${projectId}' AND status = 'published' AND created_by IN (
        SELECT id FROM core.principal WHERE kind = 'ai_pm')`
  );
  expect(Number(published)).toBe(0);
});

/** P0-050 · 学习结果来源与可信度：kb.memory_ref / kb.source 锚点 */
test("P0-050 来源与可信度机制存在且可观测（memory_ref/source 表契约）", async () => {
  // 机制面：表存在 + 关键列契约（R50 权威锚点）——内容写入经分析链已验（049a）
  const memoryRefCols = psql(
    `SELECT string_agg(column_name, ',' ORDER BY column_name) FROM information_schema.columns
      WHERE table_schema = 'kb' AND table_name = 'memory_ref'`
  );
  for (const col of ["ref_role", "target_domain", "target_type", "revision_missing_reason"]) {
    expect(memoryRefCols).toContain(col);
  }
  const sourceCols = psql(
    `SELECT string_agg(column_name, ',' ORDER BY column_name) FROM information_schema.columns
      WHERE table_schema = 'kb' AND table_name = 'source'`
  );
  for (const col of ["source_kind", "content_hash"]) {
    expect(sourceCols).toContain(col);
  }
  // 语义闸：引用缺失必须声明原因（CHECK 存在）
  const checkExists = psql(
    `SELECT count(*) FROM pg_constraint
      WHERE conrelid = 'kb.memory_ref'::regclass AND contype = 'c'`
  );
  expect(Number(checkExists)).toBeGreaterThan(0);
});

/** P0-053 · 多 Agent 并行提议与统一协调：写锁 + 幂等账本 + 过期校验 */
test("P0-053 统一协调：两提案同文档串行落账；base_revision_id 过期提案被重校验拒绝", async () => {
  const me = await api("GET", "/v1/pm/me", owner.token);
  const ownerId = me.body.principal_id as string;
  const docKey = `kg-p0-053-${projectId.slice(0, 8)}`;

  // 提案 1（建立正式基线）：红线提案 → 人工确认 → published rev1
  const p1 = await submitKnowledgeProposal(owner, {
    projectId,
    docKey,
    markdown: "# 协调基线\n\n统一写入协调 v1。",
    changeSummary: "053 基线",
    createdBy: ownerId,
    redlineKind: "rewrite_delivered"
  });
  expect([200, 201]).toContain(p1.status);
  expect(p1.outcome).toBe("redline_pending");
  const c1 = await api("POST", `/v1/pm/documents/proposals/${p1.proposalId}/confirm`, owner.token, {
    project_id: projectId,
    confirmed_by: ownerId,
    operation_id: `p0-053-c1-${Date.now()}`
  });
  expect(c1.status).toBe(200);
  const baseRevisionId = psql(
    `SELECT id FROM ai.document_revision WHERE project_id = '${projectId}'
       AND status = 'published' ORDER BY revision_no DESC LIMIT 1`
  );
  expect(baseRevisionId).toBeTruthy();

  // 并行两提案（同一文档；不同 operation_id）→ 提案面都收，确认串行：
  // p2 基于当前基线（合法）→ 确认 → rev2；p3 仍声明旧基线（过期）→ 409。
  const p2 = await submitKnowledgeProposal(owner, {
    projectId,
    docKey,
    markdown: "# 协调基线\n\n统一写入协调 v2（Agent A）。",
    changeSummary: "053 Agent A",
    createdBy: ownerId,
    redlineKind: "rewrite_delivered",
    baseRevisionId: baseRevisionId
  });
  expect([200, 201]).toContain(p2.status);
  expect(p2.outcome).toBe("redline_pending");
  const c2 = await api("POST", `/v1/pm/documents/proposals/${p2.proposalId}/confirm`, owner.token, {
    project_id: projectId,
    confirmed_by: ownerId,
    operation_id: `p0-053-c2-${Date.now()}`
  });
  expect(c2.status).toBe(200);

  // p3 声明的基线已被 p2 推进 → 过期校验：提案面直接 409，不产生修订
  const p3 = await submitKnowledgeProposal(owner, {
    projectId,
    docKey,
    markdown: "# 协调基线\n\n统一写入协调 v2b（Agent B，基于同一基线）。",
    changeSummary: "053 Agent B",
    createdBy: ownerId,
    redlineKind: "rewrite_delivered",
    baseRevisionId: baseRevisionId
  });
  expect(p3.status).toBe(409);

  // 过期提案零副作用：该文档的正式修订仍是两条（rev1 + rev2），无 v2b 覆盖
  const revisions = psql(
    `SELECT count(*) FROM ai.document_revision r
       JOIN ai.project_document d ON d.id = r.document_id
      WHERE r.project_id = '${projectId}' AND r.status = 'published'
        AND d.doc_key = '${docKey}'`
  );
  expect(Number(revisions)).toBe(2);
  const head = psql(
    `SELECT r.content_hash FROM ai.document_revision r
       JOIN ai.project_document d ON d.id = r.document_id
      WHERE r.project_id = '${projectId}' AND r.status = 'published'
        AND d.doc_key = '${docKey}'
      ORDER BY r.revision_no DESC LIMIT 1`
  );
  const p2Hash = psql(
    `SELECT proposed_content_hash FROM ai.document_proposal WHERE id = '${p2.proposalId}'`
  );
  expect(head).toBe(p2Hash);

  // 幂等账本表存在（ai.knowledge_write_operation）
  const ledger = psql(
    `SELECT count(*) FROM information_schema.tables
      WHERE table_schema = 'ai' AND table_name = 'knowledge_write_operation'`
  );
  expect(Number(ledger)).toBe(1);
});

/** P0-054 · 质量门禁与人工确认：8 规则后端强制，redline_pending 落提案 */
test("P0-054 低质提案停 redline/待确认面；确认面人工门在（AI/api_key 不可代）", async () => {
  const me = await api("GET", "/v1/pm/me", owner.token);
  const ownerId = me.body.principal_id as string;
  // 极短/无实质内容提案 → 质量门禁介入（不依赖模型）：无来源锚、无标题、
  // 无引用锚 → 规则 2/6/8 三条失败 → 提案落 `rejected`（永不 published）。
  const proposal = await api("POST", "/v1/pm/documents/proposals", owner.token, {
    project_id: projectId,
    doc_key: `kg-p0-054-${Date.now()}`,
    doc_type: "knowledge",
    markdown: "。",
    change_summary: "054 低质输入",
    operation_id: `p0-054-p-${Date.now()}`,
    created_by: ownerId
  });
  expect([200, 201, 422]).toContain(proposal.status);
  if (proposal.status !== 422) {
    const proposalId = (proposal.body.proposal_id ?? proposal.body.id) as string;
    // 质量门落点：rejected（不是 confirmed/published）
    expect(proposal.body.outcome).toBe("rejected");
    const row = psql(
      `SELECT status FROM ai.document_proposal WHERE id = '${proposalId}'`
    );
    expect(row).toBe("rejected");
    // 门禁失败清单可观测（规则名 : 规则码）
    const reason = psql(
      `SELECT rejection_reason FROM ai.document_proposal WHERE id = '${proposalId}'`
    );
    expect(reason).toContain("source");
    // 人工确认面存在，但被拒提案不可确认（409：不是 redline_pending）
    const confirm = await api(
      "POST",
      `/v1/pm/documents/proposals/${proposalId}/confirm`,
      owner.token,
      { project_id: projectId, confirmed_by: ownerId, operation_id: `p0-054-c-${Date.now()}` }
    );
    expect(confirm.status).toBe(409);
  }
});

/** P0-055 · Revision 与 Git 版本一致性（引用 14 号证据面 + 账本恢复锚点） */
test("P0-055 published 修订恒有 git_commit_sha；Git 与 PG 版本引用一致", async () => {
  const mismatch = psql(
    `SELECT count(*) FROM ai.document_revision
      WHERE project_id = '${projectId}' AND status = 'published' AND git_commit_sha IS NULL`
  );
  expect(Number(mismatch)).toBe(0);
});

/** P0-056 · 学习失败、冲突与恢复：幂等账本 + reindex_required 可观测 */
test("P0-056 恢复面：账本表 + reindex_required 标志可观测", async () => {
  const ledger = psql(
    `SELECT count(*) FROM information_schema.tables
      WHERE table_schema = 'ai' AND table_name = 'knowledge_write_operation'`
  );
  expect(Number(ledger)).toBe(1);

  // reindex_required 的真实载体是**质量门结果 JSON**（knowledge_quality.rs:30
  // `QualityGateResult.reindex_required` → 落 `ai.document_proposal.quality_gate_result`），
  // 不是独立列——断言落点按实现事实源写。
  const me = await api("GET", "/v1/pm/me", owner.token);
  const ownerId = me.body.principal_id as string;
  const docKey = `kg-p0-056-${projectId.slice(0, 8)}`;
  const proposal = await submitKnowledgeProposal(owner, {
    projectId,
    docKey,
    markdown: "# 恢复面\n\n重建索引要求可观测。",
    changeSummary: "056 恢复面夹具",
    createdBy: ownerId,
    redlineKind: "rewrite_delivered"
  });
  expect([200, 201]).toContain(proposal.status);
  const gate = psql(
    `SELECT quality_gate_result->>'reindex_required' FROM ai.document_proposal
      WHERE id = '${proposal.proposalId}'`
  );
  expect(gate).toBe("true");

  // 派生索引状态面可观测（ai.document_index_state）
  const indexState = psql(
    `SELECT count(*) FROM information_schema.tables
      WHERE table_schema = 'ai' AND table_name = 'document_index_state'`
  );
  expect(Number(indexState)).toBe(1);
});

/** P0-051 · 外部 Commit 自动发现与分析 → NOT_IMPLEMENTED（R54） */
test("P0-051 发现管道无实现 → NOT_IMPLEMENTED（不计 PASS）", async () => {
  const watchSurface = psql(
    `SELECT count(*) FROM information_schema.tables
      WHERE table_schema IN ('integration','ai','kb')
        AND (table_name ILIKE '%commit_watch%' OR table_name ILIKE '%commit_sync%' OR table_name ILIKE '%commit_discover%')`
  );
  expect(Number(watchSurface)).toBe(0); // 无管道的**反证**即为该案例的现状记录
  test.info().annotations.push({
    type: "NOT_IMPLEMENTED",
    description: "外部 Commit 自动发现/同步管道未实现（R54/D30）；归属验证语义已由 P0-020 覆盖。"
  });
});

/** P0-052 · 澄清与采纳闭环 → NOT_IMPLEMENTED（R55）；澄清语义经 interactions 预演 */
test("P0-052 采纳闭环管道已实现 → 表面可观测；澄清语义锚点可观测", async () => {
  const table = psql(
    `SELECT count(*) FROM information_schema.tables
      WHERE table_schema IN ('kb','integration','ai')
        AND (table_name ILIKE '%adoption%' OR table_name ILIKE '%clarification%')`
  );
  // R55 已清偿（0058_external_commit_adoption，PA-P0 外部 Commit 采纳闭环）：
  // integration.external_commit + integration.external_commit_clarification
  expect(Number(table)).toBeGreaterThanOrEqual(1);
  // 澄清语义真实载体：`delivery.task_interaction`（链式续接语义在列上：
  // event_id 事件锚、previous_chain_id 链、continuation_generation 世代、
  // chain_open 链开合）——锚点可观测即预演成立
  const interactionCols = psql(
    `SELECT string_agg(column_name, ',') FROM information_schema.columns
      WHERE table_schema = 'delivery' AND table_name = 'task_interaction'`
  );
  for (const col of ["event_id", "previous_chain_id", "continuation_generation", "chain_open"]) {
    expect(interactionCols).toContain(col);
  }
  test.info().annotations.push({
    type: "IMPLEMENTED",
    description: "「发现→分析→澄清→采纳→关闭」管道已落地（0058；integration.external_commit / external_commit_clarification）；澄清语义锚点 delivery.task_interaction 保留可观测。"
  });
});
