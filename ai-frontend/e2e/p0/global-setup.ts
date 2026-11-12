import { spawn, execFileSync, type ChildProcess } from "node:child_process";
import { createServer, type Server } from "node:http";
import { mkdtempSync, openSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { JWKS_PORT, PM_API, jwksDocument } from "../team/stack";

/**
 * P0 发布验收 · 独立后端栈（规格 13~17 号：D11 worker 进栈）。
 *
 * 组件（用毕即毁）：
 *  1. QA PG 一次性空库（P0_DB，pm-api 启动时自动跑迁移）；
 *  2. 本地 JWKS 服务发布测试公钥；
 *  3. 真 pm-api（jwks 模式，AI_PM_ANALYSIS_MODEL=auto）；
 *  4. **真 ai-pm-worker**（D11：AI_PM_RUN_DISPATCH=on + AI_PM_RUN_EXECUTOR=mock
 *     + AI_PM_CONVERSATION_REPLY=on + AI_PM_ANALYSIS_MODEL 默认 auto）；
 *  5. **mock LLM**（OpenAI 兼容 /chat/completions，确定性响应——P0-008b 回复链
 *     与 P0-009 分析链的 provider 均指向它，E2E 全程无外部 LLM 依赖）。
 *  6. fake-codex 脚本 = AI_PM_CODEX_BIN（确定性执行器：默认 commit+exit 0，
 *     FAKE_EXIT/FAKE_SLEEP_MS 可注入失败/挂起）。
 *
 * 环境变量（run-p0.sh 注入）：TEAM_QA_PM_API_BIN（pm-api）、P0_WORKER_BIN
 * （ai-pm-worker）、P0_MOCK_LLM_PORT（默认 18497）、TEAM_QA_PG_CONTAINER/PORT。
 */
const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(HERE, "../../..");

const PG_CONTAINER = process.env.TEAM_QA_PG_CONTAINER ?? "aipm-teamqa-pg";
const PG_PORT = process.env.TEAM_QA_PG_PORT ?? "16499";
const MOCK_LLM_PORT = Number(process.env.P0_MOCK_LLM_PORT ?? "18497");
const SECRET_KEY = process.env.AI_PM_SECRET_KEY ?? "e2e-p0-secret";
const DB_NAME = process.env.P0_DB ?? "aipm_p0_e2e";

function psql(sql: string): void {
  execFileSync(
    "docker",
    ["exec", PG_CONTAINER, "psql", "-U", "aipm", "-d", "postgres", "-qc", sql],
    { stdio: "pipe" }
  );
}

async function waitHealthy(child: ChildProcess, url: string, label: string): Promise<void> {
  const deadline = Date.now() + 90_000;
  while (Date.now() < deadline) {
    if (child.exitCode !== null)
      throw new Error(`${label} exited early (code ${child.exitCode})`);
    try {
      const res = await fetch(url);
      if (res.ok) return;
    } catch {
      // 尚未监听
    }
    await new Promise(r => setTimeout(r, 500));
  }
  throw new Error(`${label} did not become healthy within 90s`);
}

export default async function globalSetup(): Promise<() => Promise<void>> {
  const apiBin = process.env.TEAM_QA_PM_API_BIN;
  const workerBin = process.env.P0_WORKER_BIN;
  if (!apiBin || !workerBin)
    throw new Error(
      "TEAM_QA_PM_API_BIN / P0_WORKER_BIN are required (see test-case/evidence/p0/run-p0.sh)"
    );

  psql(`DROP DATABASE IF EXISTS ${DB_NAME} WITH (FORCE)`);
  psql(`CREATE DATABASE ${DB_NAME}`);

  // ── JWKS ────────────────────────────────────────────────────────────────
  const jwks = jwksDocument();
  const jwksServer: Server = createServer((req, res) => {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(jwks);
  });
  await new Promise<void>(r => jwksServer.listen(JWKS_PORT, "127.0.0.1", r));

  // ── mock LLM / 真 LLM（P0_REAL_LLM_URL 指向外部 OpenAI 兼容端点，如本机
  //    Ollama http://127.0.0.1:11434/v1；设置后不 spawn mock）─────────────────
  const realLlmUrl = process.env.P0_REAL_LLM_URL;
  const mockLlm = realLlmUrl
    ? null
    : spawn(process.execPath, [join(HERE, "mock-llm.mjs"), String(MOCK_LLM_PORT)], {
        stdio: ["ignore", "ignore", "inherit"]
      });
  const mockUrl = realLlmUrl ?? `http://127.0.0.1:${MOCK_LLM_PORT}`;
  let mockReady = false;
  for (let attempt = 0; attempt < (realLlmUrl ? 10 : 40) && !mockReady; attempt += 1) {
    mockReady = await fetch(`${mockUrl}/chat/completions`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      // Ollama 兼容面要求 model 字段；mock server 忽略它
      body: JSON.stringify({
        model: process.env.P0_LLM_MODEL ?? "mock-e2e",
        messages: [{ role: "user", content: "ping" }]
      })
    })
      .then(res => res.ok)
      .catch(() => false);
    if (!mockReady) await new Promise(r => setTimeout(r, 500));
  }
  if (!mockReady) throw new Error("mock LLM did not become healthy");

  // ── 共享目录 ─────────────────────────────────────────────────────────────
  const knowledgeRoot = mkdtempSync(join(tmpdir(), "p0-knowledge-"));
  const repoCache = mkdtempSync(join(tmpdir(), "p0-repo-cache-"));
  const workspaceRoot = mkdtempSync(join(tmpdir(), "p0-workspaces-"));
  const logDir = process.env.P0_LOG_DIR ?? tmpdir();

  // 供各 spec 读取（knowledge root 直读 + git 断言；provider 注册）。
  process.env.P0_KNOWLEDGE_ROOT = knowledgeRoot;
  process.env.P0_MOCK_LLM_URL = `${mockUrl}`;
  process.env.P0_WORKSPACE_ROOT = workspaceRoot;

  const databaseUrl = `postgres://aipm:aipm@127.0.0.1:${PG_PORT}/${DB_NAME}`;
  const childEnv = {
    ...process.env,
    DATABASE_URL: databaseUrl,
    AI_PM_SECRET_KEY: SECRET_KEY,
    AI_PM_ANALYSIS_MODEL: "auto",
    AI_PM_REPO_ALLOWED_HOSTS: "",
    AI_PM_MCP_ALLOWED_HOSTS: "",
    NO_PROXY: "127.0.0.1,localhost",
    RUST_LOG: process.env.RUST_LOG ?? "info"
  };

  // ── pm-api ──────────────────────────────────────────────────────────────
  const port = new URL(PM_API).port;
  const apiLog = openSync(join(logDir, "p0-pm-api.log"), "w");
  const api = spawn(apiBin, [], {
    env: {
      ...childEnv,
      PM_API_PORT: port,
      AI_PM_AUTH_MODE: "jwks",
      AI_PM_JWKS_URL: `http://127.0.0.1:${JWKS_PORT}/.well-known/jwks.json`,
      AI_PM_KNOWLEDGE_ROOT: knowledgeRoot,
      AI_PM_REPO_CACHE_ROOT: repoCache,
      AI_PM_PUBLIC_BASE_URL: "http://127.0.0.1:8850"
    },
    stdio: ["ignore", apiLog, apiLog]
  });
  await waitHealthy(api, `${PM_API}/health`, "pm-api");

  // ── ai-pm-worker（D11）───────────────────────────────────────────────────
  const workerLog = openSync(join(logDir, "p0-worker.log"), "w");
  const worker = spawn(workerBin, [], {
    env: {
      ...childEnv,
      AI_PM_POLICY_FILE: join(REPO, "policies", "ai-pm-ai-lead-v2.yaml"),
      AI_PM_RUN_DISPATCH: "on",
      AI_PM_RUN_EXECUTOR: "mock",
      // Windows spawn 不能直接执行 .cmd/.mjs —— bin=node、ARGS=脚本路径
      AI_PM_CODEX_BIN: process.execPath,
      AI_PM_CODEX_ARGS: join(HERE, "fake-codex.mjs"),
      AI_PM_WORKSPACE_ROOT: workspaceRoot,
      AI_PM_CONVERSATION_REPLY: "on",
      AI_PM_KNOWLEDGE_ROOT: knowledgeRoot,
      AI_PM_REPO_CACHE_ROOT: repoCache
    },
    cwd: REPO, // 策略文件按相对路径可读；worker panic 时 cwd 也要有意义
    stdio: ["ignore", workerLog, workerLog]
  });
  // worker 无 HTTP 探针：存活观察 3s
  await new Promise(r => setTimeout(r, 3000));
  if (worker.exitCode !== null)
    throw new Error(`ai-pm-worker exited early (code ${worker.exitCode})`);

  return async () => {
    worker.kill();
    api.kill();
    mockLlm?.kill();
    await new Promise(r => setTimeout(r, 800));
    jwksServer.close();
    rmSync(knowledgeRoot, { recursive: true, force: true });
    rmSync(repoCache, { recursive: true, force: true });
    rmSync(workspaceRoot, { recursive: true, force: true });
    if (process.env.TEAM_QA_KEEP_DB !== "1")
      psql(`DROP DATABASE IF EXISTS ${DB_NAME} WITH (FORCE)`);
  };
}
