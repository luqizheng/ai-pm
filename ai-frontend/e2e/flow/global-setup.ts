import { spawn, execFileSync, type ChildProcess } from "node:child_process";
import { createServer, type Server } from "node:http";
import { mkdtempSync, openSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { DB_NAME, JWKS_PORT, PM_API, PG_CONTAINER, PG_PORT, jwksDocument } from "./stack";

/**
 * E2E 全流程 · UI 套件的独立后端栈（只为本套件存在，用毕即毁）：
 *  1. QA PG 容器里新建一次性空库（pm-api 启动时自动跑迁移；绝不触碰正式库）；
 *  2. 本地 JWKS 服务发布测试公钥；
 *  3. 以 jwks 模式起 pm-api（独立端口，二进制由 FLOW_E2E_PM_API_BIN 指定）。
 *
 * 与 team-qa 的栈完全隔离：vite 8860 / pm-api 18495 / JWKS 18496 / 库 aipm_e2e_flow_ui。
 *
 * 环境变量（run-e2e-flow-ui.sh 统一注入）：
 *  FLOW_E2E_PM_API_BIN  pm-api 可执行文件（必填）
 *  FLOW_E2E_PG_CONTAINER QA PG 容器名（默认 aipm-pg-gate）
 *  FLOW_E2E_PG_PORT     该容器宿主端口（默认 16433）
 *  FLOW_E2E_DB          一次性库名（默认 aipm_e2e_flow_ui）
 *  FLOW_E2E_LOG         pm-api 日志落点
 *  FLOW_E2E_KEEP_DB     =1 时保留库（调试用）
 */
function psql(sql: string): void {
  execFileSync(
    "docker",
    ["exec", PG_CONTAINER, "psql", "-U", "aipm", "-d", "postgres", "-qc", sql],
    { stdio: "pipe" }
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
  const bin = process.env.FLOW_E2E_PM_API_BIN;
  if (!bin)
    throw new Error(
      "FLOW_E2E_PM_API_BIN is required (see test-case/evidence/e2e-flow/run-e2e-flow-ui.sh)"
    );

  psql(`DROP DATABASE IF EXISTS ${DB_NAME} WITH (FORCE)`);
  psql(`CREATE DATABASE ${DB_NAME}`);

  const jwks = jwksDocument();
  const server: Server = createServer((req, res) => {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(jwks);
  });
  await new Promise<void>(r => server.listen(JWKS_PORT, "127.0.0.1", r));

  const knowledgeRoot = mkdtempSync(join(tmpdir(), "flow-e2e-knowledge-"));
  const repoCache = mkdtempSync(join(tmpdir(), "flow-e2e-repo-cache-"));
  const port = new URL(PM_API).port;
  const logFd = openSync(
    process.env.FLOW_E2E_LOG ?? join(tmpdir(), "flow-e2e-pm-api.log"),
    "w"
  );
  const child = spawn(bin, [], {
    env: {
      ...process.env,
      DATABASE_URL: `postgres://aipm:aipm@127.0.0.1:${PG_PORT}/${DB_NAME}`,
      PM_API_PORT: port,
      AI_PM_AUTH_MODE: "jwks",
      AI_PM_JWKS_URL: `http://127.0.0.1:${JWKS_PORT}/.well-known/jwks.json`,
      AI_PM_SECRET_KEY: process.env.AI_PM_SECRET_KEY ?? "e2e-flow-ui-secret",
      AI_PM_KNOWLEDGE_ROOT: knowledgeRoot,
      AI_PM_REPO_CACHE_ROOT: repoCache,
      AI_PM_PUBLIC_BASE_URL: "http://127.0.0.1:8860",
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
    rmSync(repoCache, { recursive: true, force: true });
    if (process.env.FLOW_E2E_KEEP_DB !== "1")
      psql(`DROP DATABASE IF EXISTS ${DB_NAME} WITH (FORCE)`);
  };
}
