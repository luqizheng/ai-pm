// PA-07 冒烟栈 · 手动保活版（等价 e2e/flow/global-setup.ts，但不随 playwright 结束而 teardown）。
// 用法：node e2e/pa07/keep-alive-stack.mjs &（进程退出即整栈销毁；加 --keep-db 保留库）。
import { spawn, execFileSync } from "node:child_process";
import { createPrivateKey, createPublicKey, sign } from "node:crypto";
import { createServer } from "node:http";
import { readFileSync } from "node:fs";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const REPO = resolve(import.meta.dirname, "../../..");
const KEYS = join(REPO, "apps/pm-api/tests/keys");
const BIN = process.env.FLOW_E2E_PM_API_BIN ?? join(REPO, "target/debug/pm-api.exe");
const PG_CONTAINER = process.env.FLOW_E2E_PG_CONTAINER ?? "aipm-dev-postgres";
const PG_PORT = process.env.FLOW_E2E_PG_PORT ?? "16433";
const DB_NAME = process.env.FLOW_E2E_DB ?? "aipm_e2e_flow_ui";
const PM_API_PORT = "18495";
const JWKS_PORT = "18496";

function psql(sql) {
  return execFileSync(
    "docker",
    ["exec", PG_CONTAINER, "psql", "-U", "aipm", "-d", "postgres", "-qc", sql],
    { stdio: "pipe" }
  );
}

psql(`DROP DATABASE IF EXISTS ${DB_NAME} WITH (FORCE)`);
psql(`CREATE DATABASE ${DB_NAME}`);
console.log(`[stack] db ${DB_NAME} recreated`);

const jwk = createPublicKey(readFileSync(join(KEYS, "public.pem"), "utf8")).export({
  format: "jwk"
});
const jwks = JSON.stringify({
  keys: [{ ...jwk, kid: "default", alg: "RS256", use: "sig" }]
});
const server = createServer((req, res) => {
  res.writeHead(200, { "content-type": "application/json" });
  res.end(jwks);
});
await new Promise(r => server.listen(JWKS_PORT, "127.0.0.1", r));
console.log(`[stack] jwks on :${JWKS_PORT}`);

const knowledgeRoot = mkdtempSync(join(tmpdir(), "pa07-knowledge-"));
const repoCache = mkdtempSync(join(tmpdir(), "pa07-repo-cache-"));
const child = spawn(BIN, [], {
  env: {
    ...process.env,
    DATABASE_URL: `postgres://aipm:aipm@127.0.0.1:${PG_PORT}/${DB_NAME}`,
    PM_API_PORT: PM_API_PORT,
    AI_PM_AUTH_MODE: "jwks",
    AI_PM_JWKS_URL: `http://127.0.0.1:${JWKS_PORT}/.well-known/jwks.json`,
    AI_PM_SECRET_KEY: process.env.AI_PM_SECRET_KEY ?? "e2e-flow-ui-secret",
    AI_PM_KNOWLEDGE_ROOT: knowledgeRoot,
    AI_PM_REPO_CACHE_ROOT: repoCache,
    AI_PM_PUBLIC_BASE_URL: "http://127.0.0.1:8860",
    NO_PROXY: "127.0.0.1,localhost",
    RUST_LOG: process.env.RUST_LOG ?? "info"
  },
  stdio: ["ignore", "inherit", "inherit"]
});

const deadline = Date.now() + 90_000;
while (Date.now() < deadline) {
  if (child.exitCode !== null) throw new Error(`pm-api exited early (${child.exitCode})`);
  try {
    const res = await fetch(`http://127.0.0.1:${PM_API_PORT}/health`);
    if (res.ok) break;
  } catch {}
  await new Promise(r => setTimeout(r, 500));
}
console.log(`[stack] pm-api healthy on :${PM_API_PORT}`);
console.log("[stack] Ctrl+C（或杀进程）销毁整栈");

process.on("SIGINT", () => {
  child.kill();
  server.close();
  if (process.env.FLOW_E2E_KEEP_DB !== "1")
    try { psql(`DROP DATABASE IF EXISTS ${DB_NAME} WITH (FORCE)`); } catch {}
  process.exit(0);
});
