import { spawn, execFileSync, type ChildProcess } from "node:child_process";
import { createServer, type Server } from "node:http";
import { mkdtempSync, openSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { JWKS_PORT, PM_API, jwksDocument } from "./stack";

/**
 * Team QA · 独立后端栈（只为本套件存在，用毕即毁）：
 *  1. QA 专用 PG 容器里新建空库（绝不触碰 aipm 正式/演示库）；
 *  2. 本地 JWKS 服务发布测试公钥；
 *  3. 以 jwks 模式起 pm-api（独立端口，二进制由 TEAM_QA_PM_API_BIN 指定）。
 *
 * 环境变量（run-team-qa.sh 统一注入）：
 *  TEAM_QA_PM_API_BIN   pm-api 可执行文件（必填）
 *  TEAM_QA_PG_CONTAINER QA 专用 PG 容器名（默认 aipm-teamqa-pg）
 *  TEAM_QA_PG_PORT      该容器的宿主端口（默认 16499）
 *  TEAM_QA_LOG          pm-api 日志落点
 */
const CONTAINER = process.env.TEAM_QA_PG_CONTAINER ?? "aipm-teamqa-pg";
const PG_PORT = process.env.TEAM_QA_PG_PORT ?? "16499";
const DB = "aipm_teamqa_e2e";

function psql(sql: string): void {
  execFileSync(
    "docker",
    ["exec", CONTAINER, "psql", "-U", "aipm", "-d", "postgres", "-qc", sql],
    {
      stdio: "pipe"
    }
  );
}

async function waitHealthy(child: ChildProcess): Promise<void> {
  const deadline = Date.now() + 90_000;
  while (Date.now() < deadline) {
    if (child.exitCode !== null)
      throw new Error(`pm-api exited early (code ${child.exitCode})`);
    try {
      const res = await fetch(`${PM_API}/health`);
      if (res.ok) return;
    } catch {
      // 尚未监听
    }
    await new Promise(r => setTimeout(r, 500));
  }
  throw new Error("pm-api did not become healthy within 90s");
}

export default async function globalSetup(): Promise<() => Promise<void>> {
  const bin = process.env.TEAM_QA_PM_API_BIN;
  if (!bin)
    throw new Error(
      "TEAM_QA_PM_API_BIN is required (see test-case/evidence/team-qa/run-team-qa.sh)"
    );

  psql(`DROP DATABASE IF EXISTS ${DB} WITH (FORCE)`);
  psql(`CREATE DATABASE ${DB}`);

  const jwks = jwksDocument();
  const server: Server = createServer((req, res) => {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(jwks);
  });
  await new Promise<void>(r => server.listen(JWKS_PORT, "127.0.0.1", r));

  const knowledgeRoot = mkdtempSync(join(tmpdir(), "team-qa-knowledge-"));
  const port = new URL(PM_API).port;
  const logFd = openSync(
    process.env.TEAM_QA_LOG ?? join(tmpdir(), "team-qa-pm-api.log"),
    "w"
  );
  const child = spawn(bin, [], {
    env: {
      ...process.env,
      DATABASE_URL: `postgres://aipm:aipm@127.0.0.1:${PG_PORT}/${DB}`,
      PM_API_PORT: port,
      AI_PM_AUTH_MODE: "jwks",
      AI_PM_JWKS_URL: `http://127.0.0.1:${JWKS_PORT}/.well-known/jwks.json`,
      AI_PM_SECRET_KEY: "team-qa-e2e-secret",
      AI_PM_KNOWLEDGE_ROOT: knowledgeRoot,
      AI_PM_PUBLIC_BASE_URL: "http://localhost:8850",
      NO_PROXY: "127.0.0.1,localhost",
      RUST_LOG: process.env.RUST_LOG ?? "info"
    },
    stdio: ["ignore", logFd, logFd]
  });

  await waitHealthy(child);

  return async () => {
    child.kill();
    await new Promise(r => setTimeout(r, 500));
    server.close();
    rmSync(knowledgeRoot, { recursive: true, force: true });
    if (process.env.TEAM_QA_KEEP_DB !== "1")
      psql(`DROP DATABASE IF EXISTS ${DB} WITH (FORCE)`);
  };
}
