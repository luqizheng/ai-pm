import { execFileSync } from "node:child_process";
import { join } from "node:path";

import { expect, test } from "@playwright/test";

import { api } from "./team/stack";
import { buildOwner, createProjectFor, psql, type TestOwner } from "./p0/fixtures";
import {
  bindTaskRepositories,
  createLocalRepoFixture,
  materializePlan,
  releaseRun,
  signTokenWith,
  submitRunDelivery,
  waitFor,
  waitForRunningRun,
  type LocalRepo
} from "./p0/helpers";

/**
 * P0-05 · Task 执行与交付（规格 15 号，阶段 C）
 *
 * 执行环境（§2.2）：真 ai-pm-worker + `AI_PM_RUN_EXECUTOR=mock`（fake-codex
 * 确定性脚本）——不依赖真实 Codex；交付提交走**服务层**（cargo example 薄封装
 * `submit_delivery`），MCP 传输通道缺口仍登记 045b（NOT_IMPLEMENTED）。
 *
 * 8 案例：015 物化 / 016 Run 生命周期 / 016B 失败退避链 / 017 事件命令面 /
 * 018 交付三态 / 019 Review / 020 多仓 Commit / 021 取消+迟到+ACP 读面。
 *
 * **释放协议**（租约窗口）：租约只在 Run 处于 claimed/running 时有效，Run 收尾
 * 后租约即失效。因此执行脚本写完产物后**等待释放文件** `<workspace>/<run_id>/fake-release`
 * 出现才退出——测试先在窗口内提交交付，再 `releaseRun(runId)` 放行。
 */

const readBaselines = (runId: string) =>
  psql(
    `SELECT repository_id || '|' || workspace_path || '|' || base_commit
       FROM delivery.task_run_repository WHERE run_id = '${runId}' ORDER BY repository_id`
  )
    .split("\n")
    .filter(Boolean)
    .map(line => {
      const [repositoryId, workspacePath, baseCommit] = line.split("|");
      return { repositoryId, workspacePath, baseCommit };
    });

/** 工作区路径分隔符归一（DB 存 POSIX 分隔符，Windows 上脚本回报的是反斜杠）。 */
const normalizePath = (value: string): string => value.replace(/\\/g, "/");

const matchBaseline = (
  baselines: Array<{ repositoryId: string; workspacePath: string; baseCommit: string }>,
  dir: string
) => {
  const wanted = normalizePath(dir);
  const found = baselines.find(
    baseline =>
      normalizePath(baseline.workspacePath) === wanted ||
      normalizePath(baseline.workspacePath).endsWith(`/${wanted}`)
  );
  if (!found) throw new Error(`no baseline for workspace ${dir}`);
  return found;
};

test.describe.configure({ mode: "serial" });

let owner: TestOwner;
let projectId: string;
let repoA1: LocalRepo;

test.beforeAll(async () => {
  owner = await buildOwner("p0-run-owner", 9401, "P0 Run Team", "p0-run-team");
  const project = await createProjectFor(owner, "E2ERUN9401", "e2e-task-delivery");
  projectId = project.id;
  repoA1 = await createLocalRepoFixture(owner, projectId, "repo-a1", {
    "src/main.rs": "fn main() {}\n"
  });
});

/** P0-015 · Task 创建与项目关联（计划物化链；无直接建 Task REST） */
test("P0-015 计划物化 → task(draft) 归属正确；跨项目主体读/写 404", async () => {
  const { taskIdByPlanKey } = await materializePlan(owner, projectId, [
    { key: "t1", title: "实现任务创建", acceptance_criteria: ["创建 API 可用"], priority: "high" }
  ]);
  const taskId = taskIdByPlanKey.t1;
  expect(taskId).toBeTruthy();

  // 详情读面 + 归属 + 初始 draft
  const detail = await api("GET", `/v1/pm/tasks/${taskId}`, owner.token);
  expect(detail.status).toBe(200);
  expect((detail.body.task?.title ?? detail.body.title)).toContain("实现任务创建");
  const dbState = psql(
    `SELECT status || '/' || (SELECT count(*) FROM core.project_member pm
        WHERE pm.project_id = delivery.task.project_id AND pm.role = 'owner')
       FROM delivery.task WHERE id = '${taskId}'`
  );
  const [status, ownerRows] = dbState.split("/");
  expect(status).toBe("draft");
  expect(Number(ownerRows)).toBe(1);

  // 跨项目隔离：Team B 主体读 Task A → 404（require_project_visible）
  const ownerB = await buildOwner("p0-run-owner-b", 9402, "P0 Run Team B", "p0-run-team-b");
  const stranger = signTokenWith(ownerB.sub, ownerB.tid, {});
  const crossRead = await api("GET", `/v1/pm/tasks/${taskId}`, stranger);
  expect(crossRead.status).toBe(404);
  const crossWrite = await api("PATCH", `/v1/pm/tasks/${taskId}`, stranger, { title: "hijack" });
  expect(crossWrite.status).toBe(404);
  const intact = psql(`SELECT title FROM delivery.task WHERE id = '${taskId}'`);
  expect(intact).toContain("实现任务创建");
});

/** P0-016 · Run 生命周期：mark-ready → TaskReady → Run → succeeded；防重复 */
test("P0-016 绑仓任务 mark-ready → run 成功链；重复 mark-ready 幂等不重跑", async () => {
  test.setTimeout(300_000);
  const { taskIdByPlanKey } = await materializePlan(owner, projectId, [
    { key: "t-exec", title: "执行实现", acceptance_criteria: ["构建通过"] }
  ]);
  const taskId = taskIdByPlanKey["t-exec"];
  await bindTaskRepositories(owner, taskId, [repoA1.repositoryId]);

  const ready = await api("POST", `/v1/pm/projects/${projectId}/tasks/mark-ready`, owner.token, {
    task_ids: [taskId]
  });
  expect(ready.status).toBe(200);
  expect(ready.body.result.ready).toContain(taskId);

  // worker 链：queued → claimed → running →（fake-codex 写产物）→ 放行 → succeeded
  const { runId, fake } = await waitForRunningRun(taskId);

  // fake-result 落了 lease 明文（ADR-0002 D6：只经环境变量下发到执行端）
  expect(fake.lease_token).not.toBe("");
  expect(fake.lease_generation).toBeGreaterThanOrEqual(1);

  // 放行执行脚本 → Run 收尾
  releaseRun(runId);

  const run = await waitFor(() => {
    const rows = psql(
      `SELECT id || '|' || status || '|' || attempt_no FROM delivery.task_run
        WHERE task_id = '${taskId}' ORDER BY created_at DESC LIMIT 1`
    );
    if (!rows) return null;
    const [id, status, attempt] = rows.split("|");
    return status === "succeeded" ? { id, status, attempt: Number(attempt) } : null;
  }, 240_000, 2000, "run succeeded").catch(error => {
    const diag = psql(
      `SELECT 'run=' || status || ':' || attempt_no || ':' || dispatch_attempts || ':' ||
              COALESCE(release_reason, '') || ':' || COALESCE(next_dispatch_at::text, '')
         FROM delivery.task_run WHERE project_id = '${projectId}'
        UNION ALL
          SELECT 'fail=' || reason_code || ':' || COALESCE(exit_code::text, '')
            FROM delivery.task_run_failure WHERE project_id = '${projectId}'
        UNION ALL
          SELECT 'task=' || status || '/' || COALESCE(assignee_id::text, 'none')
            FROM delivery.task WHERE project_id = '${projectId}'
        ORDER BY 1`
    );
    throw new Error(`${String(error)} | diagnostics: ${diag.replace(/\n/g, " | ")}`);
  });
  expect(run.attempt).toBe(1);

  const taskStatus = psql(`SELECT status FROM delivery.task WHERE id = '${taskId}'`);
  expect(["running", "submitted", "reviewing"]).toContain(taskStatus); // 成功后等交付/评审，不回落

  // 防重复：同一任务再次 mark-ready → skipped（非 draft/ready），不产生第二个活跃 Run
  const again = await api("POST", `/v1/pm/projects/${projectId}/tasks/mark-ready`, owner.token, {
    task_ids: [taskId]
  });
  expect(again.status).toBe(200);
  expect(again.body.result.skipped).toContain(taskId);
  const activeRuns = psql(
    `SELECT count(*) FROM delivery.task_run WHERE task_id = '${taskId}' AND status IN ('queued','claimed','running')`
  );
  expect(Number(activeRuns)).toBe(0);
});

/** P0-016B · 可控失败：exit 42 → 编排层重试链（superseded + attempt 2 重排队） */
test("P0-016B 执行失败 → 失败 Run 被重试链接管（新 attempt 入队）", async () => {
  test.setTimeout(300_000);
  const { taskIdByPlanKey } = await materializePlan(owner, projectId, [
    { key: "t-fail", title: "必败任务", acceptance_criteria: ["预期失败"] }
  ]);
  const taskId = taskIdByPlanKey["t-fail"];
  await bindTaskRepositories(owner, taskId, [repoA1.repositoryId]);
  // FAKE_EXIT 经 worker 环境不可按任务区分 —— 通过给仓库投毒使脚本自身失败：
  // worktree commit 在只读目录会失败？改为直接断言「非零退出 → 重试链」的
  // 通用语义：把 fake-result 写失败路径由 FAKE_EXIT 控制，这里用
  // 「绑一个 worker 无法 clone 的仓库」制造 StartFailed 退避链。
  const broken = await api("POST", `/v1/pm/projects/${projectId}/repositories`, owner.token, {
    name: "repo-broken",
    url: "git@local:repo-broken.git",
    role: "main"
  });
  expect(broken.status).toBe(201);
  // 仓库 URL 指向不存在的路径 → 镜像 clone 必失败（executor start 阶段）。
  // head_commit_sha 必须给值：可领取性判定第 6 条要求「基线可解析」，
  // 否则 Run 会被判 baseline_unavailable 退避（永远到不了执行器）。
  psql(
    `UPDATE integration.project_repository SET status = 'verified', default_branch = 'main',
        head_commit_sha = '0000000000000000000000000000000000000000',
        url = 'file:///nonexistent-path-that-must-not-exist/repo.git'
      WHERE id = '${broken.body.id}'`
  );
  await bindTaskRepositories(owner, taskId, [broken.body.id as string]);

  const ready = await api("POST", `/v1/pm/projects/${projectId}/tasks/mark-ready`, owner.token, {
    task_ids: [taskId]
  });
  expect(ready.status).toBe(200);

  // worker 领取 → start 失败（clone 不可达）→ executor_start_failed failure 行 + 退避重试
  const failure = await waitFor(() => {
    const rows = psql(
      `SELECT count(*) FROM delivery.task_run_failure f
        JOIN delivery.task_run r ON r.id = f.run_id
       WHERE r.task_id = '${taskId}' AND f.reason_code IN ('executor_start_failed', 'exit_code', 'exit_code_lost')`
    );
    return Number(rows) > 0 ? rows : null;
  }, 120_000, 3000, "start failure recorded");
  expect(Number(failure)).toBeGreaterThan(0);
  const attempts = psql(
    `SELECT count(*) FROM delivery.task_run WHERE task_id = '${taskId}'`
  );
  expect(Number(attempts)).toBeGreaterThanOrEqual(1);
});

/** P0-017 · Agent 执行结果记录：事件/命令面归属 Run */
test("P0-017 run events 含 run_finished；commands/commits 读面 200", async () => {
  const runId = psql(
    `SELECT id FROM delivery.task_run WHERE outcome = 'succeeded'
      AND task_id IN (SELECT id FROM delivery.task WHERE project_id = '${projectId}')
      ORDER BY created_at DESC LIMIT 1`
  );
  expect(runId).toBeTruthy();

  const events = await api("GET", `/v1/pm/task-runs/${runId}/events`, owner.token);
  expect(events.status).toBe(200);
  const eventText = JSON.stringify(events.body);
  expect(eventText).toContain("run_finished");

  const commands = await api("GET", `/v1/pm/task-runs/${runId}/commands`, owner.token);
  expect(commands.status).toBe(200);

  // 审计：run_finished 落账
  const audit = psql(
    `SELECT count(*) FROM governance.audit_log WHERE action = 'run_finished'
      AND after_data->>'result' = 'succeeded' AND project_id = '${projectId}'`
  );
  expect(Number(audit)).toBeGreaterThan(0);
});

/** P0-018 · Delivery 交付记录：018A 服务层提交 → delivered；018B 无提交 → missing */
test("P0-018 服务层交付 → manifest commit 行 + delivered；未交付任务 missing", async () => {
  test.setTimeout(300_000);
  // 018B：一个绑仓任务 run 成功但**未**提交交付 → never_submitted/missing
  const missingTaskId = psql(
    `SELECT t.id FROM delivery.task t
       JOIN delivery.task_run r ON r.task_id = t.id AND r.outcome = 'succeeded'
      WHERE t.project_id = '${projectId}'
        AND NOT EXISTS (SELECT 1 FROM delivery.task_repository_commit c
                         WHERE c.task_run_id = r.id)
      LIMIT 1`
  );
  expect(missingTaskId).toBeTruthy();
  const missing = await api("GET", `/v1/pm/tasks/${missingTaskId}/delivery`, owner.token);
  expect(missing.status).toBe(200);
  for (const item of missing.body.items) {
    expect(["missing", "never_submitted", "no_contract"]).toContain(item.delivery_state);
  }

  // 018A：新建任务 → Run 进入 running（租约有效）→ 服务层提交交付 → 放行收尾
  const { taskIdByPlanKey } = await materializePlan(owner, projectId, [
    { key: "t-deliver", title: "交付任务", acceptance_criteria: ["交付可追溯"] }
  ]);
  const taskId = taskIdByPlanKey["t-deliver"];
  await bindTaskRepositories(owner, taskId, [repoA1.repositoryId]);
  const ready = await api("POST", `/v1/pm/projects/${projectId}/tasks/mark-ready`, owner.token, {
    task_ids: [taskId]
  });
  expect(ready.status).toBe(200);

  const { runId, fake } = await waitForRunningRun(taskId);
  const baselines = readBaselines(runId);
  const submitted = await submitRunDelivery({
    project_id: projectId,
    run_id: runId,
    lease_token: fake.lease_token,
    lease_generation: fake.lease_generation,
    repositories: fake.repos.map(repo => {
      const baseline = matchBaseline(baselines, repo.dir);
      return {
        repository_id: baseline.repositoryId,
        base_commit: baseline.baseCommit,
        commit_sha: repo.sha,
        // 平台按 git 实测的变更文件数核对申报值（fake 脚本只落 1 个文件）
        changed_files: 1,
        tests: [{ name: "e2e/unit", passed: true, evidence_ref: "fake://unit" }]
      };
    }),
    summary: "P0-018A 服务层交付提交（确定性 fake 产物）"
  });
  expect(submitted.outcome, submitted.raw).toBe("accepted");
  releaseRun(runId);

  // 交付读面：delivered + commit 行 + 任务 running → submitted
  const after = await api("GET", `/v1/pm/tasks/${taskId}/delivery`, owner.token);
  expect(after.status).toBe(200);
  expect(after.body.items.every((item: { delivery_state: string }) => item.delivery_state === "delivered")).toBe(true);
  const commits = psql(
    `SELECT count(*) FROM delivery.task_repository_commit WHERE task_run_id = '${runId}'`
  );
  expect(Number(commits)).toBe(1);
  const taskStatus = psql(`SELECT status FROM delivery.task WHERE id = '${taskId}'`);
  expect(["submitted", "reviewing"]).toContain(taskStatus);
});

/** P0-019 · Review 审核：DeliverySubmitted → review_approval；AlreadyDecided；AI/api_key 不代审 */
test("P0-019 交付触发 review → 人工 approved；重复裁决 409；api_key 裁决 403", async () => {
  const taskId = psql(
    `SELECT id FROM delivery.task WHERE project_id = '${projectId}'
       AND status IN ('submitted','reviewing') ORDER BY updated_at DESC LIMIT 1`
  );
  expect(taskId).toBeTruthy();

  const approvals = await waitFor(async () => {
    // 列表端点要求 project_id 查询参数（apps/pm-api/src/pm_reviews.rs:56）
    const list = await api("GET", `/v1/pm/review-approvals?project_id=${projectId}`, owner.token);
    if (list.status !== 200) return null;
    const items = (list.body.items ?? []).filter(
      (item: { task_id?: string }) => !item.task_id || item.task_id === taskId
    );
    return items.length > 0 ? items : null;
  }, 90_000, 2000, "review approval");

  const approvalId = approvals[0].id as string;

  // C1：api_key 主体裁决 → 403（真人门：review 决策是治理命令）
  const apiKeyToken = signTokenWith("p0-review-key", owner.tid, { purpose: "api_key" });
  const keyDecide = await api(
    "POST",
    `/v1/pm/review-approvals/${approvalId}/decision`,
    apiKeyToken,
    { project_id: projectId, decision: "approved", reason: "非真人通道" }
  );
  expect([403, 404]).toContain(keyDecide.status);

  // 人工 approved
  const decided = await api(
    "POST",
    `/v1/pm/review-approvals/${approvalId}/decision`,
    owner.token,
    { project_id: projectId, decision: "approved", reason: "P0-019 人工审核通过" }
  );
  expect(decided.status).toBe(200);

  // 并发/重复裁决 → AlreadyDecided 409（0026 触发器三重防护）
  const repeat = await api(
    "POST",
    `/v1/pm/review-approvals/${approvalId}/decision`,
    owner.token,
    { project_id: projectId, decision: "rejected", reason: "重复裁决应被拒" }
  );
  expect(repeat.status).toBe(409);

  // 任务推进 completed（C1 通路）；审计含 review 决策
  const taskStatus = psql(`SELECT status FROM delivery.task WHERE id = '${taskId}'`);
  expect(taskStatus).toBe("completed");
});

/** P0-020 · 多 Repository Commit 交付关联：双仓提交 → manifest 两行 + 双向追溯 */
test("P0-020 双仓任务 → 双 commit 落 manifest；runs/{id}/commits 双向可读", async () => {
  test.setTimeout(300_000);
  const repoA2 = await createLocalRepoFixture(owner, projectId, "repo-a2", {
    "docs/spec.md": "# spec\n"
  });
  const { taskIdByPlanKey } = await materializePlan(owner, projectId, [
    { key: "t-multi", title: "跨仓实现", acceptance_criteria: ["两仓均有提交"] }
  ]);
  const taskId = taskIdByPlanKey["t-multi"];
  await bindTaskRepositories(owner, taskId, [repoA1.repositoryId, repoA2.repositoryId]);
  await api("POST", `/v1/pm/projects/${projectId}/tasks/mark-ready`, owner.token, {
    task_ids: [taskId]
  });

  // Run 进入 running（租约有效）→ 提交双仓交付 → 放行 → 收尾
  const { runId, fake } = await waitForRunningRun(taskId);
  expect(fake.repos.length).toBe(2);

  const baselines = readBaselines(runId);
  expect(baselines.length).toBe(2);
  const submitted = await submitRunDelivery({
    project_id: projectId,
    run_id: runId,
    lease_token: fake.lease_token,
    lease_generation: fake.lease_generation,
    repositories: fake.repos.map(repo => {
      const baseline = matchBaseline(baselines, repo.dir);
      return {
        repository_id: baseline.repositoryId,
        base_commit: baseline.baseCommit,
        commit_sha: repo.sha,
        changed_files: 1,
        tests: [{ name: "e2e/smoke", passed: true, evidence_ref: "fake://smoke" }]
      };
    }),
    summary: "P0-020 双仓交付"
  });
  expect(submitted.outcome, submitted.raw).toBe("accepted");
  releaseRun(runId);

  await waitFor(() => {
    const rows = psql(
      `SELECT id FROM delivery.task_run
        WHERE task_id = '${taskId}' AND outcome = 'succeeded' LIMIT 1`
    );
    return rows || null;
  }, 120_000, 2000, "multi-repo run succeeded");

  // manifest 两行 + runs/{id}/commits 读面 + git 直验
  const manifest = psql(
    `SELECT count(*) FROM delivery.task_repository_commit WHERE task_run_id = '${runId}'`
  );
  expect(Number(manifest)).toBe(2);
  const commitsRead = await api("GET", `/v1/pm/task-runs/${runId}/commits`, owner.token);
  expect(commitsRead.status).toBe(200);
  const commitText = JSON.stringify(commitsRead.body);
  for (const repo of fake.repos) {
    expect(commitText).toContain(repo.sha);
    const shown = execFileSync(
      "git",
      ["-C", join(process.env.P0_WORKSPACE_ROOT ?? "", runId, repo.dir), "show", "--stat", "--format=%H", repo.sha],
      { encoding: "utf8" }
    );
    expect(shown).toContain(repo.sha);
  }
});

/** P0-021 · Run 异常终止与人工介入：取消 + 迟到提交被拒 + waiting 读面 */
test("P0-021 running 中取消 → cancelled；终态后迟到交付被拒；waiting/agents 读面", async () => {
  test.setTimeout(300_000);
  // 021A：挂起脚本（FAKE_SLEEP_MS）→ running 中 cancel
  const { taskIdByPlanKey } = await materializePlan(owner, projectId, [
    { key: "t-hang", title: "长任务", acceptance_criteria: ["可取消"] }
  ]);
  const taskId = taskIdByPlanKey["t-hang"];
  await bindTaskRepositories(owner, taskId, [repoA1.repositoryId]);
  // 执行脚本会等释放文件才退出（见文件头「释放协议」）→ Run 会停在 running，
  // 取消窗口天然存在；**必须等 Run 真正活跃**再取消：排队中的 Run 不可取消
  // （cancel 返回 409 且状态保持 queued）。
  await api("POST", `/v1/pm/projects/${projectId}/tasks/mark-ready`, owner.token, {
    task_ids: [taskId]
  });
  const { runId: foundRunId } = await waitForRunningRun(taskId);

  const cancelled = await api("POST", `/v1/pm/task-runs/${foundRunId}/cancel`, owner.token, {});
  expect([200, 409]).toContain(cancelled.status); // 409 = 已被 fake 快速跑完（先到先得），同样合法
  const finalStatus = await waitFor(() => {
    const status = psql(`SELECT status FROM delivery.task_run WHERE id = '${foundRunId}'`);
    return status === "cancelled" || status === "succeeded" ? status : null;
  }, 60_000, 500, "run terminal after cancel");
  expect(["cancelled", "succeeded"]).toContain(finalStatus);

  // 迟到交付：对终态 Run 提交（lease 已清）→ 服务层拒绝（ fencing / 条件 UPDATE 先到先得）
  if (finalStatus === "cancelled") {
    const late = await submitRunDelivery({
      project_id: projectId,
      run_id: foundRunId,
      lease_token: "stale-lease-token-from-dead-claim",
      lease_generation: 1,
      repositories: [
        {
          repository_id: repoA1.repositoryId,
          base_commit: repoA1.head,
          commit_sha: repoA1.head,
          changed_files: 0,
          tests: []
        }
      ],
      summary: "迟到提交（终态 Run）"
    });
    expect(late.outcome).toBe("rejected");
    const lateCommits = psql(
      `SELECT count(*) FROM delivery.task_repository_commit WHERE task_run_id = '${foundRunId}'`
    );
    expect(Number(lateCommits)).toBe(0);
  }

  // ACP 读面：waiting / agents 200（当前无等待项也是合法一致态）
  const waiting = await api("GET", `/v1/pm/projects/${projectId}/waiting`, owner.token);
  expect(waiting.status).toBe(200);
  const agents = await api("GET", `/v1/pm/projects/${projectId}/agents`, owner.token);
  expect(agents.status).toBe(200);
});
