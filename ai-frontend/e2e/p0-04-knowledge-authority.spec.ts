import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

import { expect, test } from "@playwright/test";

import { api } from "./team/stack";
import { buildOwner, createProjectFor, psql, signInVia, type TestOwner } from "./p0/fixtures";
import { asAiPrincipal, asApiKeyPrincipal, confirmDraftRevision, submitKnowledgeProposal } from "./p0/helpers";

/**
 * P0-04 · Requirement 与 Knowledge 三层权威（规格 14 号，阶段 B）
 * 011 三存储关联 / 012 修订与并发哈希门 / 013 知识域红线（Git HEAD 不变证据）/
 * 014 PageIndex 派生索引可删除重建。
 *
 * R27 修正已内化：create 只落 PG draft；Markdown 文件 + Git commit 在 submit
 * （投影绑定点）诞生 —— 所有 Git/文件断言排在 submit 之后。
 */

test.describe.configure({ mode: "serial" });

let owner: TestOwner;
let projectId: string;
let knowledgeRoot: string;

const git = (args: string[]) =>
  execFileSync("git", ["-C", knowledgeRoot, ...args], { encoding: "utf8" }).trim();

const sha256 = (text: string) => createHash("sha256").update(text, "utf8").digest("hex");

/** 找到 req-<uuid> 的投影文件（knowledge.rs：doc_key = `req-{uuid}`）。 */
function requirementDocPath(reqId: string): string {
  const all = execFileSync("git", ["-C", knowledgeRoot, "ls-files"], {
    encoding: "utf8"
  }).trim();
  const line = all
    .split(/\r?\n/)
    .filter(name => name.includes(`req-${reqId}`))[0];
  expect(line, `projection file for req-${reqId} must exist in knowledge git`).toBeTruthy();
  return line as string;
}

test.beforeAll(async () => {
  owner = await buildOwner("p0-kb-owner", 9301, "P0 KB Team", "p0-kb-team");
  const project = await createProjectFor(owner, "E2EKB9301", "e2e-knowledge-authority");
  projectId = project.id;
  knowledgeRoot = process.env.P0_KNOWLEDGE_ROOT ?? "";
  if (!knowledgeRoot || !existsSync(knowledgeRoot))
    throw new Error("P0_KNOWLEDGE_ROOT missing (global-setup)");
});

/** P0-011 · 正式 Requirement 创建：Markdown+Git+PG 三存储正确关联 */
test("P0-011 create→submit→confirm：投影文件/Git sha/PG 版本引用三面一致", async ({
  context,
  page
}) => {
  // create（仅 PG draft 行；R27：此时无文件无 commit）
  const created = await api("POST", `/v1/pm/projects/${projectId}/requirements`, owner.token, {
    code: "REQ-E2E-001",
    title: "用户任务管理",
    markdown: "# 用户任务管理\n\n系统支持团队成员创建任务。"
  });
  expect(created.status).toBe(201);
  const reqId = created.body.requirement.id as string;

  const beforeSubmit = psql(
    `SELECT COALESCE(markdown_version_ref, 'none') FROM delivery.requirement_revision
      WHERE requirement_id = '${reqId}' LIMIT 1`
  );
  expect(beforeSubmit).toBe("none");

  // submit（唯一投影绑定点）→ confirm（approved）
  const handle = await confirmDraftRevision(
    owner,
    reqId,
    "# 用户任务管理\n\n系统支持团队成员创建任务。",
    "P0-011 首次定稿"
  );

  // ① Markdown 权威：直读文件（绕过应用）
  const docPath = requirementDocPath(reqId);
  const docFile = join(knowledgeRoot, docPath);
  expect(existsSync(docFile)).toBe(true);
  const doc = readFileSync(docFile, "utf8");
  expect(doc).toContain("系统支持团队成员创建任务");

  // ② Git 权威：HEAD commit 含正文
  const commit = git(["log", "-1", "--format=%H", "--", docPath]);
  const atRef = git(["show", `${commit}:${docPath}`]);
  expect(atRef).toContain("系统支持团队成员创建任务");

  // ③ PG 状态权威：版本引用（markdown_version_ref）指向真实 Git 版本；
  //    投影绑定在 ai.document_revision（doc_key=req-<uuid>）与修订引用双落
  const meta = psql(
    `SELECT rv.markdown_version_ref || '/' ||
            (rv.content_hash = '${sha256(doc)}'::text) || '/' || rv.state || '/' ||
            (SELECT count(*) FROM ai.document_revision d
              WHERE d.project_id = '${projectId}' AND d.git_commit_sha = rv.markdown_version_ref)
       FROM delivery.requirement r
       JOIN delivery.requirement_revision rv ON rv.id = r.current_revision_id
      WHERE r.id = '${reqId}'`
  );
  const [sha, hashMatch, state, projectionRows] = meta.split("/");
  expect(sha).toBe(commit);
  expect(hashMatch).toBe("true");
  expect(state).toBe("approved");
  expect(Number(projectionRows)).toBeGreaterThan(0);
  expect(handle.contentHash).toBe(sha256(doc));

  // ④ UI 读真 + 刷新一致
  await signInVia(context, owner.sub, owner.tid);
  await page.goto(`/project/requirements?project_id=${projectId}`);
  await expect(page.locator(".req-row__title", { hasText: "用户任务管理" }).first()).toBeVisible({
    timeout: 20_000
  });
});

/** P0-012 · 修订不可覆盖 + 并发陈旧哈希被拒（DEF-03 回归 · 统一入口） */
test("P0-012 Rev3 诞生旧版 superseded；陈旧哈希 confirm 409 且终态不变", async () => {
  const reqId = psql(
    `SELECT id FROM delivery.requirement WHERE code = 'REQ-E2E-001' AND project_id = '${projectId}'`
  );
  const headBefore = git(["rev-parse", "HEAD"]);

  // 定稿前的 approved 版本（011 已确认）及其 Git 版本
  const prevRevId = psql(
    `SELECT current_revision_id FROM delivery.requirement WHERE id = '${reqId}'`
  );
  const prevSha = psql(
    `SELECT markdown_version_ref FROM delivery.requirement_revision WHERE id = '${prevRevId}'`
  );
  expect(prevSha).toBeTruthy();

  // User A：正常定稿新修订（第三次投影绑定 → 新 commit）
  const v3 = await confirmDraftRevision(
    owner,
    reqId,
    "# 用户任务管理\n\n系统支持团队成员创建任务，并允许管理员分配任务。",
    "P0-012 修订"
  );

  // Git 历史推进；旧版本内容在旧 sha 处原样可追溯
  const commitCount = git(["rev-list", "--count", "HEAD"]);
  expect(Number(commitCount)).toBeGreaterThanOrEqual(2);
  const docPath = requirementDocPath(reqId);
  const headAfter = git(["rev-parse", "HEAD"]);
  expect(headAfter).not.toBe(headBefore);
  const prevContent = git(["show", `${prevSha}:${docPath}`]);
  expect(prevContent).toContain("系统支持团队成员创建任务。");
  const newContent = git(["show", `${v3.contentHash && prevSha && headAfter}:${docPath}`]).length > 0;

  // DB：current 指向新修订（approved）；旧修订 superseded
  const states = psql(
    `SELECT (SELECT state FROM delivery.requirement_revision WHERE id = '${v3.currentRevisionId}')
          || '/' ||
         (SELECT state FROM delivery.requirement_revision WHERE id = '${prevRevId}')
       FROM delivery.requirement WHERE id = '${reqId}'`
  );
  expect(states).toBe("approved/superseded");

  // User B：持旧哈希确认 → 409，终态不变（DEF-03 回归：统一入口哈希门）
  const stale = await api(
    "POST",
    `/v1/pm/requirements/${reqId}/revisions/${v3.currentRevisionId}/confirm`,
    owner.token,
    { expected_content_hash: prevSha }
  );
  expect(stale.status).toBe(409);
  const meta = psql(
    `SELECT current_revision_id FROM delivery.requirement WHERE id = '${reqId}'`
  );
  expect(meta).toBe(v3.currentRevisionId);
  expect(newContent).toBe(true);
});

/** P0-013 · 知识域 AI AutoApply 禁止：提案待确认；Git HEAD 不变 = 权威未动 */
test("P0-013 知识提案停 redline/待确认；AI 身份 confirm 403（人工门）；api_key 403（可信上下文）；Git HEAD 不动", async () => {
  const docKey = `kb-p0-013-${projectId.slice(0, 8)}`;
  const me = await api("GET", "/v1/pm/me", owner.token);
  const ownerId = me.body.principal_id as string;

  // AI 生成修改建议（提案面：建议 ≠ 正式；提交式分析链同族）
  // 红线声明 → 提案落 redline_pending（等人工确认，**不写 Git**）
  const proposal = await submitKnowledgeProposal(owner, {
    projectId,
    docKey,
    markdown: "# 任务管理规则\n\n1. 任务必须有明确负责人。\n2. 任务完成前必须人工确认。",
    changeSummary: "P0-013 AI 建议（未确认）",
    createdBy: ownerId,
    redlineKind: "rewrite_delivered"
  });
  expect([200, 201]).toContain(proposal.status);
  expect(proposal.outcome).toBe("redline_pending");
  const proposalId = proposal.proposalId as string;
  expect(proposalId).toBeTruthy();

  // 未确认 → Git HEAD 不变（投影绑定只在确认路径；AI 无该入口）
  const headBefore = git(["rev-parse", "HEAD"]);

  // 闸①：kind='ai_pm' 主体直调红线确认 → 403
  // 人工门落在 confirmed_by 的 kind（knowledge_proposal/submit.rs:298
  // `require_human(pool, confirmed_by)`）——AI 主体把自己填成确认人即被拒。
  // asAiPrincipal 先补 team 资格（P4-ORG Team-first）再授予 project_member，
  // 可见性闸放行 → 403 可达。
  const ai = await asAiPrincipal(owner, "p0-kb-ai", projectId);
  const aiAttempt = await api(
    "POST",
    `/v1/pm/documents/proposals/${proposalId}/confirm`,
    ai.token,
    {
      project_id: projectId,
      confirmed_by: ai.principalId,
      operation_id: `p0-013-ai-${Date.now()}`
    }
  );
  expect(aiAttempt.status).toBe(403);

  // 闸②：api_key 通道 → 403（ensure_trusted_context：机器凭据不构成可信
  // 交互上下文；asApiKeyPrincipal 已入编 → 可见性放行 → 可信门 403 可达）
  const key = await asApiKeyPrincipal(owner, "p0-kb-key", projectId);
  const keyAttempt = await api(
    "POST",
    `/v1/pm/documents/proposals/${proposalId}/confirm`,
    key.token,
    { project_id: projectId, confirmed_by: ownerId, operation_id: `p0-013-key-${Date.now()}` }
  );
  expect(keyAttempt.status).toBe(403);

  // 权威未动三证据
  expect(git(["rev-parse", "HEAD"])).toBe(headBefore);
  // 本文档尚无已发布修订（提案未确认）→ 权威未动
  const publishedBefore = psql(
    `SELECT count(*) FROM ai.document_revision r
       JOIN ai.project_document d ON d.id = r.document_id
      WHERE r.project_id = '${projectId}' AND d.doc_key = '${docKey}'
        AND r.status = 'published'`
  );
  expect(Number(publishedBefore)).toBe(0);

  // 人工红线确认 → 正式 Revision 诞生 + Git commit 落地
  const confirmed = await api(
    "POST",
    `/v1/pm/documents/proposals/${proposalId}/confirm`,
    owner.token,
    { project_id: projectId, confirmed_by: ownerId, operation_id: `p0-013-human-${Date.now()}` }
  );
  expect(confirmed.status).toBe(200);
  expect(git(["rev-parse", "HEAD"])).not.toBe(headBefore);
  const published = psql(
    `SELECT count(*) FROM ai.document_revision r
       JOIN ai.project_document d ON d.id = r.document_id
      WHERE r.project_id = '${projectId}' AND d.doc_key = '${docKey}'
        AND r.status = 'published' AND r.git_commit_sha IS NOT NULL`
  );
  expect(Number(published)).toBe(1);
});

/** P0-014 · PageIndex 派生索引可删可重建；权威三面不动；检索恢复 */
test("P0-014 删索引 → Markdown/Git/PG 不动 → reindex → 检索命中", async () => {
  const docKey = `kb-p0-014-${projectId.slice(0, 8)}`;
  const me = await api("GET", "/v1/pm/me", owner.token);
  const ownerId = me.body.principal_id as string;

  // 造一篇知识文档（红线提案 → 人工确认 → published + 投影 + 索引）
  const proposal = await submitKnowledgeProposal(owner, {
    projectId,
    docKey,
    markdown:
      "# 任务管理规则\n\n1. 任务必须有明确负责人。\n2. 任务完成前必须人工确认。\n3. 阻塞任务必须登记原因。\n4. 交付必须带测试证据。",
    changeSummary: "P0-014 索引重建夹具",
    createdBy: ownerId,
    redlineKind: "rewrite_delivered"
  });
  expect([200, 201]).toContain(proposal.status);
  expect(proposal.outcome).toBe("redline_pending");
  const proposalId = proposal.proposalId as string;
  const confirmed = await api(
    "POST",
    `/v1/pm/documents/proposals/${proposalId}/confirm`,
    owner.token,
    { project_id: projectId, confirmed_by: ownerId, operation_id: `p0-014-confirm-${Date.now()}` }
  );
  expect(confirmed.status).toBe(200);

  const documentId = psql(
    `SELECT id FROM ai.project_document WHERE project_id = '${projectId}' AND doc_key = '${docKey}'`
  );
  expect(documentId).toBeTruthy();

  // 权威快照
  const headBefore = git(["rev-parse", "HEAD"]);
  const docPath = execFileSync("git", ["-C", knowledgeRoot, "ls-files"], {
    encoding: "utf8"
  })
    .trim()
    .split(/\r?\n/)
    .filter(name => name.includes(docKey))[0];
  const fileBefore = readFileSync(join(knowledgeRoot, docPath), "utf8");

  // 检索命中（前置索引存在）
  const search1 = await api(
    "GET",
    `/v1/pm/documents/search?project_id=${projectId}&q=${encodeURIComponent("任务必须有明确负责人")}`,
    owner.token
  );
  expect(search1.status).toBe(200);

  // 只删派生索引（ai.document_section_index + ai.document_index_state；
  // 两者按 document_revision_id 关联，不存 document_id）
  const deleted = psql(
    `WITH rev AS (SELECT id FROM ai.document_revision WHERE document_id = '${documentId}')
     DELETE FROM ai.document_section_index
      WHERE document_revision_id IN (SELECT id FROM rev) RETURNING 1`
  );
  expect(deleted).not.toBe("");
  psql(
    `WITH rev AS (SELECT id FROM ai.document_revision WHERE document_id = '${documentId}')
     DELETE FROM ai.document_index_state
      WHERE document_revision_id IN (SELECT id FROM rev)`
  );

  // 权威三面不动
  expect(git(["rev-parse", "HEAD"])).toBe(headBefore);
  expect(readFileSync(join(knowledgeRoot, docPath), "utf8")).toBe(fileBefore);
  const metaAfter = psql(
    `SELECT count(*) FROM ai.document_revision WHERE document_id = '${documentId}'`
  );
  const metaBeforeCount = metaAfter; // revision 行数不变由下方 reindex 后复核
  // 派生索引确已清空（这才是「可删」的证据）
  const indexRowsAfterDelete = psql(
    `SELECT count(*) FROM ai.document_section_index s
       JOIN ai.document_revision r ON r.id = s.document_revision_id
      WHERE r.document_id = '${documentId}'`
  );
  expect(Number(indexRowsAfterDelete)).toBe(0);

  // 重建（单文档 reindex）
  const revisionId = psql(
    `SELECT id FROM ai.document_revision WHERE document_id = '${documentId}'
      ORDER BY revision_no DESC LIMIT 1`
  );
  const reindexed = await api(
    "POST",
    `/v1/pm/documents/revisions/${revisionId}/reindex`,
    owner.token,
    { project_id: projectId }
  );
  expect(reindexed.status).toBe(200);

  // 重建后：不重复建文档、不修改正式内容、索引恢复、检索命中
  const docCount = psql(
    `SELECT count(*) FROM ai.project_document WHERE doc_key = '${docKey}'`
  );
  expect(Number(docCount)).toBe(1);
  expect(git(["rev-parse", "HEAD"])).toBe(headBefore);
  expect(readFileSync(join(knowledgeRoot, docPath), "utf8")).toBe(fileBefore);
  expect(
    Number(psql(`SELECT count(*) FROM ai.document_revision WHERE document_id = '${documentId}'`))
  ).toBe(Number(metaBeforeCount));
  // 派生索引已重建（可重建的证据）
  const indexRowsAfterRebuild = psql(
    `SELECT count(*) FROM ai.document_section_index s
       JOIN ai.document_revision r ON r.id = s.document_revision_id
      WHERE r.document_id = '${documentId}'`
  );
  expect(Number(indexRowsAfterRebuild)).toBeGreaterThan(0);

  const search2 = await api(
    "GET",
    `/v1/pm/documents/search?project_id=${projectId}&q=${encodeURIComponent("任务必须有明确负责人")}`,
    owner.token
  );
  expect(search2.status).toBe(200);
  expect(JSON.stringify(search2.body)).toContain(documentId);
});
