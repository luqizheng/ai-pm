#!/usr/bin/env node
/**
 * P0 E2E · 确定性假 Codex 执行脚本（AI_PM_CODEX_BIN 指向本文件 = mock 执行器）。
 *
 * 契约（crates/pm-infrastructure/src/codex_executor.rs）：
 *  - 环境注入：AI_PM_RUN_ID / AI_PM_WORKSPACE_ROOT / AI_PM_LEASE_TOKEN /
 *    AI_PM_LEASE_GENERATION；工作区 = WORKSPACE_ROOT/<run_id>/<workspace_path>。
 *  - 退出码 0 → ExitOutcome::Success；非 0 → NonZeroExit（编排层重试链）。
 *
 * 行为（环境开关）：
 *  - FAKE_SLEEP_MS>0：只挂起后退出（取消/超时场景用）。
 *  - FAKE_EXIT：最终退出码（默认 0）。
 *  - FAKE_RELEASE_TIMEOUT_MS：等待「释放文件」的最长时长（默认 180s）。
 *  - 默认：对每个含 .git 的工作区做一次 git commit，把
 *    {lease_token, lease_generation, repos:[{dir, sha}]} 写到
 *    WORKSPACE_ROOT/<run_id>/fake-result.json（交付提交助手 submit_run_result
 *    的 lease_token 来源 —— 真实 claim 链下发的明文），然后**等释放文件**
 *    WORKSPACE_ROOT/<run_id>/fake-release 出现再退出。
 *
 * 为什么等释放：租约只在 Run 处于 claimed/running 时有效（run_context.rs 的
 * lease 校验要求 status ∈ {claimed, running} 且 lease_token_hash 未清）。
 * 真实链路里执行端在进程退出**之前**通过 MCP 提交交付；脚本立即退出会让
 * 交付提交落在租约失效之后（lease_invalid）。等释放 = 把「执行端仍在跑」
 * 这段时间交给测试，由测试先提交交付再放行退出。
 */
import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const runId = process.env.AI_PM_RUN_ID ?? "";
const root = process.env.AI_PM_WORKSPACE_ROOT ?? "";
if (!runId || !root) {
  console.error("fake-codex: AI_PM_RUN_ID / AI_PM_WORKSPACE_ROOT required");
  process.exit(97);
}
const runRoot = join(root, runId);
const sleepMs = Number(process.env.FAKE_SLEEP_MS ?? "0");
if (sleepMs > 0) {
  await new Promise(resolve => setTimeout(resolve, sleepMs));
  process.exit(Number(process.env.FAKE_EXIT ?? "0"));
}

const git = (dir, args) =>
  execFileSync("git", ["-C", dir, ...args], {
    encoding: "utf8",
    env: {
      ...process.env,
      GIT_AUTHOR_NAME: "Fake Codex",
      GIT_AUTHOR_EMAIL: "fake-codex@e2e.test",
      GIT_COMMITTER_NAME: "Fake Codex",
      GIT_COMMITTER_EMAIL: "fake-codex@e2e.test"
    }
  }).trim();

const repos = [];
if (existsSync(runRoot)) {
  // 工作区路径 = `repos/<safe-name>-<repository_id>`（run_dispatch/port.rs
  // `workspace_path_for`）——run 目录下的**嵌套**层级；worktree 的 `.git` 是文件。
  // 这里做有界递归（深度 ≤ 3）找 worktree 根，报告相对 run 目录的路径。
  const candidates = [];
  const walk = (dir, rel, depth) => {
    if (depth > 3) return;
    let entries = [];
    try {
      entries = readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      if (!entry.isDirectory()) continue;
      const child = join(dir, entry.name);
      const childRel = rel ? join(rel, entry.name) : entry.name;
      if (existsSync(join(child, ".git"))) {
        candidates.push({ rel: childRel, dir: child });
        continue;
      }
      walk(child, childRel, depth + 1);
    }
  };
  walk(runRoot, "", 0);
  for (const candidate of candidates) {
    const dir = candidate.dir;
    // 造一处真实变更（否则 worktree 干净 → git commit 失败 → 脚本非零退出，
    // 编排层判 exit_code 失败，与「确定性成功产物」的意图相反）。
    writeFileSync(
      join(dir, "fake-delivery.md"),
      `# fake delivery ${runId}\n\ntask=${process.env.AI_PM_TASK_ID ?? ""}\n`
    );
    execFileSync("git", ["-C", dir, "add", "-A"]);
    try {
      git(dir, ["-c", "commit.gpgsign=false", "commit", "-q", "-m", `feat: fake delivery ${runId}`]);
    } catch {
      // 无变更可提交（幂等重放）：保留工作区现状，仍按当前 HEAD 上报。
    }
    repos.push({ dir: candidate.rel, sha: git(dir, ["rev-parse", "HEAD"]) });
  }
}

writeFileSync(
  join(runRoot, "fake-result.json"),
  JSON.stringify({
    lease_token: process.env.AI_PM_LEASE_TOKEN ?? "",
    lease_generation: Number(process.env.AI_PM_LEASE_GENERATION ?? "1"),
    repos
  })
);

// 等释放（交付提交要在租约有效期内完成）。
const releasePath = join(runRoot, "fake-release");
const releaseTimeoutMs = Number(process.env.FAKE_RELEASE_TIMEOUT_MS ?? "180000");
const deadline = Date.now() + releaseTimeoutMs;
while (!existsSync(releasePath) && Date.now() < deadline) {
  await new Promise(resolve => setTimeout(resolve, 200));
}
process.exit(Number(process.env.FAKE_EXIT ?? "0"));
