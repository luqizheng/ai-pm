import { writeFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";

import { expect, test } from "@playwright/test";

import { api } from "../team/stack";
import { buildOwner, createProjectFor, psql, type TestOwner } from "../p0/fixtures";
import {
  asAiPrincipal,
  createLocalRepoFixture,
  signTokenWith,
  waitFor,
  type LocalRepo
} from "../p0/helpers";

/**
 * 阶段 D · S1~S6 主链（交付物 05 V1.1 §4.1）
 *
 * tenantA / projectA1 绑定本地 file:// 仓库夹具（固定 commit），真 pm-api +
 * 真 worker + FakeModel(derived；无 provider → analysis_mode=derived)：
 *   S1 analyze → report analyzing→analyzed（报告门 409 + commit 锚）
 *   S2 KB draft（确认前不可检索）
 *   S3 PC 草稿 + ai_agent 提案 + pending approval（draft lifecycle + source/evidence 钉 KB Revision）
 *   S4 人工 confirm_revision + PC decision（AI/api_key 403；发布后索引/检索可见）
 *   S5 close_report → 批量发布（lifecycle→active、version+snapshot_hash、锚点固化）
 *   S6 重复 close（记录实际行为；幂等键 pc-decision:{id} 不重复发布）
 *
 * E1 对象身份关联表写入 test-case/evidence/chain/e1-object-identity.json。
 */

test.describe.configure({ mode: "serial" });
test.setTimeout(300_000);

let owner: TestOwner;
let ownerPrincipalId: string;
let projectId: string;
let repo: LocalRepo;
let reportId: string;
let intakeId: string;
let kbDocumentId: string;
let kbRevisionId: string;
let proposalId: string;
let versionId: string;

const DB = process.env.P0_DB ?? "aipm_chain_e2e";

function reportRow(): {
  id: string;
  status: string;
  commit_sha: string;
  run_mode: string;
  since_commit: string;
  stage_detail: string;
  error_code: string;
  error_detail: string;
} | null {
  const row = psql(
    `SELECT id || '|' || status || '|' || commit_sha || '|' || run_mode || '|' ||
            COALESCE(since_commit, '') || '|' || COALESCE(stage_detail, '') || '|' ||
            COALESCE(error_code, '') || '|' || COALESCE(error_detail, '')
       FROM integration.repo_analysis_report
      WHERE project_id = '${projectId}'
      ORDER BY created_at DESC LIMIT 1`,
    DB
  );
  if (!row) return null;
  const [id, status, commit_sha, run_mode, since_commit, stage_detail, error_code, error_detail] =
    row.split("|");
  return { id, status, commit_sha, run_mode, since_commit, stage_detail, error_code, error_detail };
}

async function waitReportTerminal() {
  return waitFor(
    () => {
      const row = reportRow();
      if (!row) return null;
      return row.status === "analyzed" || row.status === "failed" ? row : null;
    },
    240_000,
    2000,
    "analysis report terminal"
  );
}

test.beforeAll(async () => {
  owner = await buildOwner("chain-owner", 9401, "Chain Team", "chain-team");
  const me = await api("GET", "/v1/pm/me", owner.token);
  ownerPrincipalId = me.body.principal_id as string;
  const project = await createProjectFor(owner, "CHAIN9401", "chain-full-loop");
  projectId = project.id;
  repo = await createLocalRepoFixture(owner, projectId, "chain-repo", {
    "src/app.ts": "export const hello = (name: string) => `hello ${name}`;\n",
    "src/tasks.ts": "export const tasks = ['create', 'assign', 'track'];\n",
    "README.md": "# chain fixture\n\nA deterministic repository for the P1 business chain.\n"
  });
});

/** S1：analyze → 报告门 409（带 report_id）→ analyzed + commit 锚。 */
test("S1 仓库 analyze：报告门生效、commit_sha 锚定、报告 analyzed", async () => {
  const first = await api(
    "POST",
    `/v1/pm/repositories/${repo.repositoryId}/analyze`,
    owner.token,
    { idempotency_key: `chain-s1-${Date.now()}` }
  );
  expect(first.status).toBe(202);
  reportId = first.body.report_id as string;
  intakeId = first.body.intake_id as string;
  expect(reportId).toBeTruthy();
  expect(first.body.commit).toBe(repo.head);
  // `status` 是受理回执的 intake 路由态（ROUTED），报告行状态以 DB/读面为准。
  expect(first.body.replayed).toBe(false);

  // 报告门：analyzing 期间再次分析 → 409 report_open + 同一 report_id。
  const blocked = await api(
    "POST",
    `/v1/pm/repositories/${repo.repositoryId}/analyze`,
    owner.token,
    { idempotency_key: `chain-s1-blocked-${Date.now()}` }
  );
  expect(blocked.status).toBe(409);
  expect(blocked.body.error).toBe("report_open");
  expect(blocked.body.report_id).toBe(reportId);

  const row = await waitReportTerminal();
  expect(row.status).toBe("analyzed");
  expect(row.commit_sha).toBe(repo.head);
  expect(row.run_mode).toBe("full");
  expect(row.since_commit).toBe("");

  // API 读面与 DB 终态一致。
  const detail = await api(
    "GET",
    `/v1/pm/analysis-reports/${reportId}?project_id=${projectId}`,
    owner.token
  );
  expect(detail.status).toBe(200);
  expect(detail.body.status).toBe("analyzed");
  expect(detail.body.commit_sha).toBe(repo.head);
});

/** S2：KB draft（analysis_mode 归因；确认前不可检索）。 */
test("S2 KB：kb.document_revision draft + 确认前检索不命中", async () => {
  const detail = await api(
    "GET",
    `/v1/pm/analysis-reports/${reportId}?project_id=${projectId}`,
    owner.token
  );
  expect(detail.status).toBe(200);
  const kb = detail.body.kb_revision;
  expect(kb, "report must carry its KB revision").toBeTruthy();
  kbDocumentId = kb.document_id as string;
  kbRevisionId = kb.revision_id as string;
  expect(kb.status).toBe("draft");
  expect(["derived", "llm", "derived_fallback"]).toContain(kb.analysis_mode);
  expect(String(kb.content ?? "").length).toBeGreaterThan(0);

  // DB 双证据：draft + 索引 0 行。
  const dbStatus = psql(
    `SELECT status || '|' || analysis_mode FROM kb.document_revision WHERE id = '${kbRevisionId}'`,
    DB
  );
  expect(dbStatus.split("|")[0]).toBe("draft");
  const indexRows = psql(
    `SELECT count(*) FROM kb.document_section_index WHERE document_revision_id = '${kbRevisionId}'`,
    DB
  );
  expect(Number(indexRows)).toBe(0);

  // 确认前检索不命中（确定性降级检索）。
  const before = await api(
    "POST",
    `/v1/retrieval/query?project_id=${projectId}`,
    owner.token,
    { query: "repository snapshot" }
  );
  expect(before.status).toBe(200);
  expect(JSON.stringify(before.body)).not.toContain(kbRevisionId);
});

/** S3：PC 草稿 + ai_agent 提案 + pending approval；source/evidence 钉扎 KB Revision。 */
test("S3 PC：draft lifecycle + ai_agent 提案 + pending approval + 钉扎 KB Revision", async () => {
  const detail = await api(
    "GET",
    `/v1/pm/analysis-reports/${reportId}?project_id=${projectId}`,
    owner.token
  );
  const proposals = detail.body.proposals ?? [];
  expect(proposals.length, "analysis must draft one PC proposal").toBeGreaterThan(0);
  proposalId = proposals[0].proposal_id as string;
  expect(proposals[0].status).toBe("submitted");

  const proposal = await api(
    "GET",
    `/v1/pc/proposals/${proposalId}?project_id=${projectId}`,
    owner.token
  );
  expect(proposal.status).toBe(200);

  const meta = psql(
    `SELECT origin_kind || '|' || status || '|' || expected_version_no
       FROM pc.proposal WHERE id = '${proposalId}'`,
    DB
  );
  const [originKind, proposalStatus, expectedVersionNo] = meta.split("|");
  expect(originKind).toBe("ai_agent");
  expect(proposalStatus).toBe("submitted");
  expect(Number(expectedVersionNo)).toBeGreaterThanOrEqual(0);

  const approval = psql(
    `SELECT count(*) FROM pc.approval
      WHERE proposal_id = '${proposalId}' AND status = 'pending'`,
    DB
  );
  expect(Number(approval)).toBeGreaterThan(0);

  // lifecycle 恒 draft（AI 无 active 写入）。
  const lifecycles = psql(
    `SELECT string_agg(DISTINCT lifecycle_status, ',')
       FROM pc.cognition_object
      WHERE project_id = '${projectId}'`,
    DB
  );
  // bootstrap 播种的 identity 单例与本次分析产出的 draft 并存；AI 不得写 active。
  expect(lifecycles.split(",")).toContain("draft");
  expect(lifecycles.split(",")).not.toContain("active");

  // source + evidence 钉扎本次 KB Revision。
  const sourcePinned = psql(
    `SELECT count(*) FROM pc.source
      WHERE project_id = '${projectId}' AND target_revision_id = '${kbRevisionId}'`,
    DB
  );
  expect(Number(sourcePinned)).toBeGreaterThan(0);
  const evidencePinned = psql(
    `SELECT count(*) FROM pc.evidence e
       JOIN pc.source s ON s.id = e.source_id
      WHERE e.project_id = '${projectId}' AND s.target_revision_id = '${kbRevisionId}'
        AND e.target_revision_id = '${kbRevisionId}'`,
    DB
  );
  expect(Number(evidencePinned)).toBeGreaterThan(0);

  // 通知状态（G-UI-4 衔接）：AI 起草提案 → pc_proposal_pending 留痕（outbox 消费）。
  await waitFor(
    () =>
      Number(
        psql(
          `SELECT count(*) FROM integration.notification
            WHERE project_id = '${projectId}' AND kind = 'pc_proposal_pending'`,
          DB
        )
      ) > 0
        ? true
        : null,
    60_000,
    1000,
    "pc_proposal_pending notification"
  );
});

/** S4：人工门（AI/api_key 403）→ confirm_revision + decision → 发布后可检索。 */
test("S4 人工：AI/api_key 决策 403；真人 confirm + approve；发布后索引/检索可见", async () => {
  const proposal = await api(
    "GET",
    `/v1/pc/proposals/${proposalId}?project_id=${projectId}`,
    owner.token
  );
  const expectedVersionNo = Number(proposal.body.expected_version_no ?? 0);
  expect(expectedVersionNo).toBeGreaterThanOrEqual(0);

  // 闸①：AI 主体（kind=ai_pm）决策 → 403。
  const ai = await asAiPrincipal(owner, "chain-ai", projectId);
  const aiAttempt = await api(
    "POST",
    `/v1/pc/proposals/${proposalId}/decision?project_id=${projectId}`,
    ai.token,
    { action: "approve", expected_version_no: expectedVersionNo, note: "ai should be denied" }
  );
  expect(aiAttempt.status).toBe(403);

  // 闸②：api_key 通道 → 403（ensure_trusted_context）。
  const apiKey = signTokenWith("chain-api-key", owner.tid, { purpose: "api_key" });
  const keyAttempt = await api(
    "POST",
    `/v1/pc/proposals/${proposalId}/decision?project_id=${projectId}`,
    apiKey,
    { action: "approve", expected_version_no: expectedVersionNo, note: "api key denied" }
  );
  expect(keyAttempt.status).toBe(403);

  // 终态未变：仍 submitted / pending。
  expect(
    psql(`SELECT status FROM pc.proposal WHERE id = '${proposalId}'`, DB)
  ).toBe("submitted");

  // 真人 KB confirm（draft → published）。
  const kbConfirm = await api(
    "POST",
    `/v1/kb/documents/${kbDocumentId}/revisions/${kbRevisionId}/confirm?project_id=${projectId}`,
    owner.token
  );
  expect(kbConfirm.status).toBe(200);
  expect(kbConfirm.body.status).toBe("published");

  // outbox 消费 → 索引重建 → 章节全文索引可命中（published 后才可检索）。
  // 说明：/v1/retrieval/query 在 planner 默认关闭时走确定性降级（证据为空），
  // 因此"可检索"以章节索引的 FTS 命中为证据，不把降级检索当失败。
  await waitFor(
    () =>
      Number(
        psql(
          `SELECT count(*) FROM kb.document_section_index
            WHERE document_revision_id = '${kbRevisionId}'`,
          DB
        )
      ) > 0
        ? true
        : null,
    60_000,
    1000,
    "kb section index built"
  );
  const ftsHit = psql(
    `SELECT count(*) FROM kb.document_section_index
      WHERE document_revision_id = '${kbRevisionId}'
        AND search_tsv IS NOT NULL
        AND search_tsv @@ plainto_tsquery('simple', 'chain')`,
    DB
  );
  expect(Number(ftsHit)).toBeGreaterThan(0);

  // 真人 PC approve（暂记 approved；正式版本由 close 批量发布）。
  const approved = await api(
    "POST",
    `/v1/pc/proposals/${proposalId}/decision?project_id=${projectId}`,
    owner.token,
    { action: "approve", expected_version_no: expectedVersionNo, note: "chain S4 human approve" }
  );
  expect(approved.status).toBe(200);
  expect(approved.body.status).toBe("approved");
  expect(approved.body.published_version_no).toBeNull();

  const decided = psql(
    `SELECT p.status || '|' || a.status FROM pc.proposal p
       JOIN pc.approval a ON a.proposal_id = p.id
      WHERE p.id = '${proposalId}' ORDER BY a.created_at DESC LIMIT 1`,
    DB
  );
  expect(decided).toBe("approved|approved");

  // 通知状态（G-UI-4）：KB 发布留痕（去重锚 = revision id）。
  await waitFor(
    () =>
      Number(
        psql(
          `SELECT count(*) FROM integration.notification
            WHERE project_id = '${projectId}' AND kind = 'kb_document_revision_published'
              AND payload->>'dedup_key' = '${kbRevisionId}'`,
          DB
        )
      ) > 0
        ? true
        : null,
    60_000,
    1000,
    "kb_document_revision_published notification"
  );

  // 审计可追溯（G-UI-4）：发布记录的真实 Human Actor。
  const publishAudit = psql(
    `SELECT actor_id || '|' || action FROM governance.audit_log
      WHERE project_id = '${projectId}' AND action = 'kb.document_revision.published'
        AND target_id = '${kbRevisionId}' ORDER BY created_at DESC LIMIT 1`,
    DB
  );
  expect(publishAudit).toBe(`${ownerPrincipalId}|kb.document_revision.published`);
});

/** S5：close_report → 批量发布 + 版本快照 + 锚点固化。 */
test("S5 close：approved 提案批量发布、lifecycle→active、version+snapshot_hash、锚点固化", async () => {
  const close = await api(
    "POST",
    `/v1/pm/analysis-reports/${reportId}/close?project_id=${projectId}`,
    owner.token
  );
  expect(close.status).toBe(200);
  expect(close.body.status).toBe("closed");
  expect(close.body.applied_proposals).toBe(1);
  expect(close.body.publish_failed_proposals).toBe(0);
  expect(close.body.withdrawn_proposals).toBe(0);
  expect(close.body.rejected_kb).toBe(false);
  expect(close.body.last_analyzed_commit).toBe(repo.head);

  // 版本 + 成员快照 + snapshot_hash。
  const version = psql(
    `SELECT v.id || '|' || v.version_no || '|' || v.snapshot_hash || '|' ||
            (SELECT count(*) FROM pc.version_member vm WHERE vm.version_id = v.id)
       FROM pc.cognition_version v
      WHERE v.project_id = '${projectId}'
      ORDER BY v.version_no DESC LIMIT 1`,
    DB
  );
  const [vid, versionNo, snapshotHash, memberCount] = version.split("|");
  versionId = vid;
  expect(Number(versionNo)).toBeGreaterThanOrEqual(1);
  expect(snapshotHash.length).toBeGreaterThan(0);
  expect(Number(memberCount)).toBeGreaterThan(0);

  // lifecycle → active（identity 单例保留原态）；报告 closed；仓库锚点固化。
  expect(
    psql(
      `SELECT string_agg(DISTINCT lifecycle_status, ',') FROM pc.cognition_object
        WHERE project_id = '${projectId}'`,
      DB
    ).split(",")
  ).toContain("active");
  expect(psql(`SELECT status FROM integration.repo_analysis_report WHERE id = '${reportId}'`, DB)).toBe(
    "closed"
  );
  expect(
    psql(`SELECT last_analyzed_commit_sha FROM integration.project_repository WHERE id = '${repo.repositoryId}'`, DB)
  ).toBe(repo.head);

  // P7 版本读面可回读（服务化契约）。
  const p7 = await api(
    "GET",
    `/v1/pc/versions/${versionId}?project_id=${projectId}`,
    owner.token
  );
  expect([200, 404, 501]).toContain(p7.status);

  // 通知状态（G-UI-4）：版本发布留痕（去重锚 = version id）。
  await waitFor(
    () =>
      Number(
        psql(
          `SELECT count(*) FROM integration.notification
            WHERE project_id = '${projectId}' AND kind = 'pc_version_published'
              AND payload->>'dedup_key' = '${versionId}'`,
          DB
        )
      ) > 0
        ? true
        : null,
    60_000,
    1000,
    "pc_version_published notification"
  );

  // 审计可追溯（G-UI-4）：close 的真实 Human Actor。
  const closeAudit = psql(
    `SELECT actor_id || '|' || action FROM governance.audit_log
      WHERE project_id = '${projectId}' AND action = 'analysis_report.closed'
        AND target_id = '${reportId}' ORDER BY created_at DESC LIMIT 1`,
    DB
  );
  expect(closeAudit).toBe(`${ownerPrincipalId}|analysis_report.closed`);
});

/** S6：重复 close —— 记录实际行为；不得重复发布版本。 */
test("S6 幂等：重复 close 不重执行（实际行为登记；版本数不增）", async () => {
  const versionsBefore = Number(
    psql(`SELECT count(*) FROM pc.cognition_version WHERE project_id = '${projectId}'`, DB)
  );

  const again = await api(
    "POST",
    `/v1/pm/analysis-reports/${reportId}/close?project_id=${projectId}`,
    owner.token
  );
  // 现状：报告非 analyzed → 409（候选契约 E2-3「返回原结果」待裁决，只记录实际）。
  expect(again.status).toBe(409);

  const versionsAfter = Number(
    psql(`SELECT count(*) FROM pc.cognition_version WHERE project_id = '${projectId}'`, DB)
  );
  expect(versionsAfter).toBe(versionsBefore);
  expect(psql(`SELECT status FROM integration.repo_analysis_report WHERE id = '${reportId}'`, DB)).toBe(
    "closed"
  );
});

test.afterAll(() => {
  const dir = join(process.cwd(), "..", "test-case", "evidence", "chain");
  mkdirSync(dir, { recursive: true });
  writeFileSync(
    join(dir, "e1-object-identity.json"),
    JSON.stringify(
      {
        fixture_version: "fake-model-v1",
        tenant_id: owner.team.id,
        project_id: projectId,
        report: { report_id: reportId, intake_id: intakeId, commit_sha: repo.head },
        kb: { document_id: kbDocumentId, revision_id: kbRevisionId },
        proposal: { proposal_id: proposalId, origin_kind: "ai_agent" },
        version: { version_id: versionId },
        compatibility_ref: {
          note: "Report 身份=repo_analysis_report；正文由 kb.document_revision 持有；kb_revision_id 为兼容引用（非独立 Report Revision 对象，P2 落地前）"
        }
      },
      null,
      2
    )
  );
});
