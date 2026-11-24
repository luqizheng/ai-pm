import { writeFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";

import { expect, test } from "@playwright/test";

import { api } from "../team/stack";
import { buildOwner, createProjectFor, psql, type TestOwner } from "../p0/fixtures";
import {
  asAiPrincipal,
  asApiKeyPrincipal,
  createLocalRepoFixture,
  waitFor,
  type LocalRepo
} from "../p0/helpers";

/**
 * 阶段 D · S1~S6 主链（交付物 05 V1.1 §4.1；2026-11-24 迁移 ProjC 管线）
 *
 * tenantA / projectA1 绑定本地 file:// 仓库夹具（固定 commit），真 pm-api +
 * 真 worker + FakeModel(derived；无 provider → analysis_mode=derived)：
 *   S1 analyze → report analyzing→analyzed（报告门 409 + commit 锚）
 *   S2 KB draft（确认前不可检索）
 *   S3 ProjC 草稿 + repo_analysis 提案 + submitted（draft lifecycle + source/evidence 钉 KB Revision）
 *   S4 人工 confirm_revision + ProjC P5 决策（AI/api_key 403；审阅流 v2：决定暂记，close 才发布）
 *   S5 close_report → ProjC 批量发布（lifecycle→active、version+snapshot_hash、锚点固化）
 *   S6 重复 close（409；版本数不增）
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

/** S3：ProjC 草稿 + repo_analysis 提案 + submitted；source/evidence 钉扎 KB Revision。 */
test("S3 ProjC：draft lifecycle + repo_analysis 提案 + submitted + 钉扎 KB Revision", async () => {
  const detail = await api(
    "GET",
    `/v1/pm/analysis-reports/${reportId}?project_id=${projectId}`,
    owner.token
  );
  // P3-11 双域分流：分析提案在 projc_proposals（pc.proposal 仅 identity 综合通道，
  // 分析链不再写 pc.proposal——2026-11-24 管线迁移）。
  const proposals = detail.body.projc_proposals ?? [];
  expect(proposals.length, "analysis must draft one ProjC proposal").toBeGreaterThan(0);
  proposalId = proposals[0].proposal_id as string;
  expect(proposals[0].status).toBe("submitted");

  const meta = psql(
    `SELECT origin_kind || '|' || status || '|' || expected_version_no
       FROM projc.proposal WHERE id = '${proposalId}'`,
    DB
  );
  const [originKind, proposalStatus, expectedVersionNo] = meta.split("|");
  // 分析链起草 = repo_analysis（旧 ai_agent 口径属 pc identity 综合通道）。
  expect(originKind).toBe("repo_analysis");
  expect(proposalStatus).toBe("submitted");
  expect(Number(expectedVersionNo)).toBeGreaterThanOrEqual(0);

  // lifecycle 恒 draft/identity（AI 无 active 写入）。
  const lifecycles = psql(
    `SELECT string_agg(DISTINCT lifecycle_status, ',')
       FROM projc.cognition_object
      WHERE project_id = '${projectId}'`,
    DB
  );
  // 分析起草对象为 draft；bootstrap 播种的 identity 单例并存；AI 不得写 active。
  expect(lifecycles.split(",")).toContain("draft");
  expect(lifecycles.split(",")).not.toContain("active");

  // source + evidence 钉扎本次 KB Revision（projc 面，G-1 硬门同源）。
  const sourcePinned = psql(
    `SELECT count(*) FROM projc.source
      WHERE project_id = '${projectId}' AND target_revision_id = '${kbRevisionId}'`,
    DB
  );
  expect(Number(sourcePinned)).toBeGreaterThan(0);
  const evidencePinned = psql(
    `SELECT count(*) FROM projc.evidence e
       JOIN projc.source s ON s.id = e.source_id
      WHERE e.project_id = '${projectId}' AND s.target_revision_id = '${kbRevisionId}'`,
    DB
  );
  expect(Number(evidencePinned)).toBeGreaterThan(0);

  // 通知状态（G-UI-4 衔接）：AI 起草提案 → projc_proposal_pending 留痕（outbox 消费）。
  await waitFor(
    () =>
      Number(
        psql(
          `SELECT count(*) FROM integration.notification
            WHERE project_id = '${projectId}' AND kind = 'projc_proposal_pending'`,
          DB
        )
      ) > 0
        ? true
        : null,
    60_000,
    1000,
    "projc_proposal_pending notification"
  );
});

/** S4：人工门（AI/api_key 403）→ confirm_revision + P5 决策 → 发布后可检索。 */
test("S4 人工：AI/api_key 决策 403；真人 confirm + approve；发布后索引/检索可见", async () => {
  // ProjC 提案无单体详情端点：expected_version_no 从报告详情的 projc_proposals 行取。
  const detail = await api(
    "GET",
    `/v1/pm/analysis-reports/${reportId}?project_id=${projectId}`,
    owner.token
  );
  const row = (detail.body.projc_proposals ?? []).find(
    (p: { proposal_id: string }) => p.proposal_id === proposalId
  );
  expect(row, "proposal row must persist in report detail").toBeTruthy();
  const expectedVersionNo = Number(row.expected_version_no ?? 0);
  expect(expectedVersionNo).toBeGreaterThanOrEqual(0);

  // P5 决策入口（审阅流 v2：close 前决定可反复改；close 即冻结）。
  const decide = (token: string, note: string) =>
    api(
      "POST",
      `/v1/pm/projc/proposals/${proposalId}/decide?project_id=${projectId}`,
      token,
      { action: "approve", expected_version_no: expectedVersionNo, note }
    );

  // 闸①：AI 主体（kind=ai_pm）决策 → 403（ensure_formal_authorized 治理授权真人面）。
  const ai = await asAiPrincipal(owner, "chain-ai", projectId);
  const aiAttempt = await decide(ai.token, "ai should be denied");
  expect(aiAttempt.status).toBe(403);

  // 闸②：api_key 通道 → 403（ensure_trusted_context；asApiKeyPrincipal 已入编，
  // 可见性闸放行后可信上下文门可达）。
  const apiKey = await asApiKeyPrincipal(owner, "chain-api-key", projectId);
  const keyAttempt = await decide(apiKey.token, "api key denied");
  expect(keyAttempt.status).toBe(403);

  // 终态未变：仍 submitted。
  expect(
    psql(`SELECT status FROM projc.proposal WHERE id = '${proposalId}'`, DB)
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

  // 真人 ProjC approve（审阅流 v2：决定暂记 approved；正式版本由 close 批量发布）。
  const approved = await decide(owner.token, "chain S4 human approve");
  expect(approved.status).toBe(200);
  expect(approved.body.status).toBe("approved");
  expect(approved.body.published_version_ref).toBeNull();

  const decided = psql(
    `SELECT p.status || '|' || a.status FROM projc.proposal p
       JOIN projc.approval a ON a.proposal_id = p.id
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

/** S5：close_report → ProjC 批量发布 + 版本快照 + 锚点固化。 */
test("S5 close：approved 提案批量发布、lifecycle→active、version+snapshot_hash、锚点固化", async () => {
  const close = await api(
    "POST",
    `/v1/pm/analysis-reports/${reportId}/close?project_id=${projectId}`,
    owner.token
  );
  expect(close.status).toBe(200);
  expect(close.body.status).toBe("closed");
  // pc 域零提案（管线已切 ProjC）；ProjC 域 1 批准提案首发。
  expect(close.body.applied_proposals).toBe(0);
  expect(close.body.projc_applied).toBe(1);
  expect(close.body.projc_replayed).toBe(0);
  expect(close.body.projc_publish_failed_proposals).toBe(0);
  expect(close.body.projc_withdrawn).toBe(0);
  expect(close.body.publish_failed_proposals).toBe(0);
  expect(close.body.withdrawn_proposals).toBe(0);
  expect(close.body.rejected_kb).toBe(false);
  expect(close.body.last_analyzed_commit).toBe(repo.head);

  // published_versions：projc 域首发记录可回读。
  const entries = (close.body.published_versions ?? []).filter(
    (e: { domain: string }) => e.domain === "projc"
  );
  expect(entries.length).toBe(1);
  expect(entries[0].proposal_id).toBe(proposalId);

  // 版本 + 成员快照 + snapshot_hash（projc 面）。
  const version = psql(
    `SELECT v.id || '|' || v.version_no || '|' || v.snapshot_hash || '|' ||
            (SELECT count(*) FROM projc.version_member vm WHERE vm.version_id = v.id)
       FROM projc.cognition_version v
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
      `SELECT string_agg(DISTINCT lifecycle_status, ',') FROM projc.cognition_object
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

  // ProjC 只读浏览面（网关路由 /v1/projc/* → pm-api）可回读已发布对象。
  const browse = await api(
    "GET",
    `/v1/projc/objects?project_id=${projectId}`,
    owner.token
  );
  expect(browse.status).toBe(200);

  // 通知状态（G-UI-4）：ProjC 版本发布留痕。
  await waitFor(
    () =>
      Number(
        psql(
          `SELECT count(*) FROM integration.notification
            WHERE project_id = '${projectId}' AND kind = 'projc_version_published'`,
          DB
        )
      ) > 0
        ? true
        : null,
    60_000,
    1000,
    "projc_version_published notification"
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

/** S6：重复 close —— 409；ProjC 版本数不增。 */
test("S6 幂等：重复 close 不重执行（409；版本数不增）", async () => {
  const versionsBefore = Number(
    psql(`SELECT count(*) FROM projc.cognition_version WHERE project_id = '${projectId}'`, DB)
  );

  const again = await api(
    "POST",
    `/v1/pm/analysis-reports/${reportId}/close?project_id=${projectId}`,
    owner.token
  );
  // 报告非 analyzed → 409（审阅流 v2：closed 即冻结，决策入口同样 409）。
  expect(again.status).toBe(409);

  const versionsAfter = Number(
    psql(`SELECT count(*) FROM projc.cognition_version WHERE project_id = '${projectId}'`, DB)
  );
  expect(versionsAfter).toBe(versionsBefore);
  expect(psql(`SELECT status FROM integration.repo_analysis_report WHERE id = '${reportId}'`, DB)).toBe(
    "closed"
  );

  // 关闭后决策入口冻结（409）。
  const frozen = await api(
    "POST",
    `/v1/pm/projc/proposals/${proposalId}/decide?project_id=${projectId}`,
    owner.token,
    { action: "reject", expected_version_no: 0, note: "frozen after close" }
  );
  expect(frozen.status).toBe(409);
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
        proposal: { proposal_id: proposalId, origin_kind: "repo_analysis" },
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
