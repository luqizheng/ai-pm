import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

import { expect, test } from "@playwright/test";

import { api } from "../team/stack";
import { buildOwner, createProjectFor, psql, type TestOwner } from "../p0/fixtures";
import { createLocalRepoFixture, waitFor, type LocalRepo } from "../p0/helpers";

/**
 * 阶段 D · 分支场景（交付物 05 §3.3 / §4.3；E2-1~E2-5 仅记录实际行为，不作强制断言）
 *
 * B1 close 残留：未审批提案 → withdrawn；KB draft → rejected；不产生版本。
 * B2 增量链：新 commit + mode=incremental → run_mode=incremental + 锚点=上一份报告 commit。
 * B3 无锚点增量 → 退 full（since_commit 空）。
 * B4 失败恢复：仓库不可达 → 报告 failed + error_code/stage_detail；intake 任务 FAILED。
 *
 * 说明：仓库分析没有 cancel 端点（取消仅存在于 imports/docproc），"两段式取消"
 * 不在本链可测面，登记为 NOT_APPLICABLE；job 耗尽语义由 B4 的失败终态观测。
 */

test.describe.configure({ mode: "serial" });
test.setTimeout(300_000);

const DB = process.env.P0_DB ?? "aipm_chain_e2e";

let owner: TestOwner;
let projectId: string;

function reportRow(projectId: string, repositoryId?: string) {
  const filter = repositoryId ? `AND repository_id = '${repositoryId}'` : "";
  const row = psql(
    `SELECT id || '|' || status || '|' || commit_sha || '|' || run_mode || '|' ||
            COALESCE(since_commit, '') || '|' || COALESCE(error_code, '') || '|' ||
            COALESCE(stage_detail, '') || '|' || COALESCE(kb_revision_id::text, '') || '|' ||
            COALESCE(error_detail, '')
       FROM integration.repo_analysis_report
      WHERE project_id = '${projectId}' ${filter}
      ORDER BY created_at DESC LIMIT 1`,
    DB
  );
  if (!row) return null;
  const [
    id,
    status,
    commit_sha,
    run_mode,
    since_commit,
    error_code,
    stage_detail,
    kb_revision_id,
    error_detail
  ] = row.split("|");
  return {
    id,
    status,
    commit_sha,
    run_mode,
    since_commit,
    error_code,
    stage_detail,
    kb_revision_id,
    error_detail
  };
}

async function waitTerminal(projectId: string, repositoryId?: string) {
  return waitFor(
    () => {
      const row = reportRow(projectId, repositoryId);
      return row && ["analyzed", "failed"].includes(row.status) ? row : null;
    },
    240_000,
    2000,
    "branch report terminal"
  );
}

async function analyze(token: string, repositoryId: string, mode?: string) {
  return api("POST", `/v1/pm/repositories/${repositoryId}/analyze`, token, {
    idempotency_key: `chain-b-${repositoryId}-${Date.now()}`,
    ...(mode ? { mode } : {})
  });
}

function commitFile(repo: LocalRepo, file: string, content: string): string {
  const env = {
    ...process.env,
    GIT_AUTHOR_NAME: "Fixture",
    GIT_AUTHOR_EMAIL: "fixture@e2e.test",
    GIT_COMMITTER_NAME: "Fixture",
    GIT_COMMITTER_EMAIL: "fixture@e2e.test"
  };
  mkdirSync(dirname(join(repo.hostDir, file)), { recursive: true });
  writeFileSync(join(repo.hostDir, file), content);
  execFileSync("git", ["-C", repo.hostDir, "add", "-A"], { env });
  execFileSync(
    "git",
    ["-C", repo.hostDir, "-c", "commit.gpgsign=false", "commit", "-q", "-m", "increment"],
    { env }
  );
  const head = execFileSync("git", ["-C", repo.hostDir, "rev-parse", "HEAD"], {
    encoding: "utf8"
  }).trim();
  psql(
    `UPDATE integration.project_repository
        SET head_commit_sha = '${head}', status = 'verified'
      WHERE id = '${repo.repositoryId}'`,
    DB
  );
  return head;
}

test.beforeAll(async () => {
  owner = await buildOwner("chain-branch-owner", 9402, "Chain Branch Team", "chain-branch-team");
  const project = await createProjectFor(owner, "CHAIN9402", "chain-branches");
  projectId = project.id;
});

/** B1：close 残留——未审批提案 withdrawn、KB draft rejected、零版本。 */
test("B1 close 残留：withdrawn + rejected_kb=true，不产生正式版本", async () => {
  const repo = await createLocalRepoFixture(owner, projectId, "chain-branch-b1", {
    "README.md": "# branch b1\n"
  });
  const started = await analyze(owner.token, repo.repositoryId);
  expect(started.status).toBe(202);
  const analyzed = await waitTerminal(projectId, repo.repositoryId);
  expect(analyzed.status).toBe("analyzed");

  const versionsBefore = Number(
    psql(`SELECT count(*) FROM pc.cognition_version WHERE project_id = '${projectId}'`, DB)
  );
  const close = await api(
    "POST",
    `/v1/pm/analysis-reports/${analyzed.id}/close?project_id=${projectId}`,
    owner.token
  );
  expect(close.status).toBe(200);
  expect(close.body.applied_proposals).toBe(0);
  expect(close.body.withdrawn_proposals).toBeGreaterThanOrEqual(1);
  expect(close.body.rejected_kb).toBe(true);

  expect(
    psql(
      `SELECT status FROM kb.document_revision WHERE id = '${analyzed.kb_revision_id}'`,
      DB
    )
  ).toBe("rejected");
  expect(
    Number(
      psql(
        `SELECT count(*) FROM pc.proposal
          WHERE project_id = '${projectId}' AND status = 'withdrawn'`,
        DB
      )
    )
  ).toBeGreaterThanOrEqual(1);
  const versionsAfter = Number(
    psql(`SELECT count(*) FROM pc.cognition_version WHERE project_id = '${projectId}'`, DB)
  );
  expect(versionsAfter).toBe(versionsBefore);

  writeFileSync(
    join(process.cwd(), "..", "test-case", "evidence", "chain", "branch-b1-close-residual.json"),
    JSON.stringify({ report: analyzed, close: close.body }, null, 2)
  );
});

/** B2：增量链 —— 新 commit + incremental → 锚点=上一份关闭报告的 commit。 */
test("B2 增量链：run_mode=incremental，since_commit=上一关闭报告锚点", async () => {
  const repo = await createLocalRepoFixture(owner, projectId, "chain-branch-b2", {
    "README.md": "# branch b2\n"
  });

  const first = await analyze(owner.token, repo.repositoryId);
  expect(first.status).toBe(202);
  const analyzed1 = await waitTerminal(projectId, repo.repositoryId);
  expect(analyzed1.status).toBe("analyzed");
  const closed1 = await api(
    "POST",
    `/v1/pm/analysis-reports/${analyzed1.id}/close?project_id=${projectId}`,
    owner.token
  );
  expect(closed1.status).toBe(200);
  const anchor = closed1.body.last_analyzed_commit as string;
  expect(anchor).toBe(analyzed1.commit_sha);

  const newHead = commitFile(repo, "src/extra.ts", "export const extra = 1;\n");
  expect(newHead).not.toBe(repo.head);

  const second = await analyze(owner.token, repo.repositoryId, "incremental");
  expect(second.status).toBe(202);
  const analyzed2 = await waitTerminal(projectId, repo.repositoryId);
  expect(analyzed2.status).toBe("analyzed");
  expect(analyzed2.run_mode).toBe("incremental");
  expect(analyzed2.since_commit).toBe(anchor);
  expect(analyzed2.commit_sha).toBe(newHead);

  const closed2 = await api(
    "POST",
    `/v1/pm/analysis-reports/${analyzed2.id}/close?project_id=${projectId}`,
    owner.token
  );
  expect(closed2.status).toBe(200);

  writeFileSync(
    join(process.cwd(), "..", "test-case", "evidence", "chain", "branch-b2-incremental.json"),
    JSON.stringify({ first: analyzed1, anchor, second: analyzed2 }, null, 2)
  );
});

/** B3：无锚点 incremental → 退 full（since_commit 空）。 */
test("B3 无锚点增量：降级为 full（since_commit 为空）", async () => {
  const repo = await createLocalRepoFixture(owner, projectId, "chain-branch-b3", {
    "README.md": "# branch b3 (no anchor)\n"
  });
  const started = await analyze(owner.token, repo.repositoryId, "incremental");
  expect(started.status).toBe(202);
  const analyzed = await waitTerminal(projectId, repo.repositoryId);
  expect(analyzed.status).toBe("analyzed");
  expect(analyzed.run_mode).toBe("full");
  expect(analyzed.since_commit).toBe("");

  const closed = await api(
    "POST",
    `/v1/pm/analysis-reports/${analyzed.id}/close?project_id=${projectId}`,
    owner.token
  );
  expect(closed.status).toBe(200);
});

/** B4：失败恢复——仓库不可达 → 报告 failed（error_code/stage_detail 可见）。 */
test("B4 失败恢复：报告 failed + error_code/stage_detail；intake 任务 FAILED", async () => {
  const repo = await createLocalRepoFixture(owner, projectId, "chain-branch-b4", {
    "README.md": "# branch b4\n"
  });
  psql(
    `UPDATE integration.project_repository
        SET url = 'file:///definitely-missing-chain-repo-9402'
      WHERE id = '${repo.repositoryId}'`,
    DB
  );

  const started = await analyze(owner.token, repo.repositoryId);
  expect(started.status).toBe(202);
  const failed = await waitTerminal(projectId, repo.repositoryId);
  expect(failed.status).toBe("failed");
  expect(failed.error_code.length).toBeGreaterThan(0);
  expect(failed.error_detail.length).toBeGreaterThan(0);

  const taskStates = psql(
    `SELECT string_agg(DISTINCT task_status, ',') FROM integration.intake_task
      WHERE intake_id = (SELECT intake_id FROM integration.repo_analysis_report WHERE id = '${failed.id}')`,
    DB
  );
  expect(taskStates).toContain("FAILED");

  // 失败报告不占门：修复 url 后可再次分析。
  psql(
    `UPDATE integration.project_repository
        SET url = 'file:///${repo.hostDir.replace(/\\/g, "/").replace(/^\//, "")}',
            status = 'verified', head_commit_sha = '${repo.head}'
      WHERE id = '${repo.repositoryId}'`,
    DB
  );
  const retry = await analyze(owner.token, repo.repositoryId);
  expect(retry.status).toBe(202);

  writeFileSync(
    join(process.cwd(), "..", "test-case", "evidence", "chain", "branch-b4-failure.json"),
    JSON.stringify({ failed, intake_task_states: taskStates, retry_status: retry.status }, null, 2)
  );
});
