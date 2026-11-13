import { execFileSync } from "node:child_process";
import { createPrivateKey, createPublicKey, sign } from "node:crypto";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

/**
 * E2E 全流程 · UI 套件（UIFLOW-*）的测试侧工具。
 *
 * 与 `e2e/team/stack.ts` 同一套模式（自签 RS256 + JWKS + 直连 pm-api），
 * 但**端口与库独立**（8860 / 18495 / 18496 / aipm_e2e_flow_ui），两套件
 * 可并行存在互不干扰。不直接 import team/stack 的原因：其 PM_API 常量在
 * 模块加载时固化，ESM import 提升会让 config 里的 env 覆盖失效。
 *
 * 身份 = 自签 RS256 JWT（复用 pm-api 集成测试的 `apps/pm-api/tests/keys`），
 * 由 global-setup 起的本地 JWKS 服务发布公钥；pm-api 以 jwks 模式校验。
 */

const HERE = dirname(fileURLToPath(import.meta.url));
const KEYS = resolve(HERE, "../../../apps/pm-api/tests/keys");
/** 仓库根（ai-frontend/e2e/flow → 上三级）。 */
export const REPO_ROOT = resolve(HERE, "../../..");

export const PM_API = process.env.FLOW_E2E_PM_API ?? "http://127.0.0.1:18495";
export const JWKS_PORT = Number(process.env.FLOW_E2E_JWKS_PORT ?? 18496);

/** QA PG 容器（与 L2 的 run-e2e-flow.sh 共用，宿主 127.0.0.1:16433）。 */
export const PG_CONTAINER = process.env.FLOW_E2E_PG_CONTAINER ?? "aipm-pg-gate";
export const PG_PORT = process.env.FLOW_E2E_PG_PORT ?? "16433";
export const DB_NAME = process.env.FLOW_E2E_DB ?? "aipm_e2e_flow_ui";

/** 信封主密钥：必须与 global-setup 给 pm-api 的值一致。 */
export const SECRET_KEY = process.env.AI_PM_SECRET_KEY ?? "e2e-flow-ui-secret";

/** 本机真实私钥（GitLab + GitHub 都已注册为可访问）。 */
export function sshKeyPath(): string {
  return (
    process.env.AI_PM_E2E_SSH_KEY ??
    resolve(
      process.env.USERPROFILE ?? process.env.HOME ?? ".",
      ".ssh",
      "id_rsa"
    )
  );
}

/** 真实远端仓库（与本仓 L2 `apps/pm-api/tests/e2e_flow.rs` 一致）。 */
export const GITLAB_REPO =
  process.env.AI_PM_E2E_REPO_GITLAB ?? "git@192.168.2.16:tooling/ai-pm.git";
export const GITHUB_REPO =
  process.env.AI_PM_E2E_REPO_GITHUB ??
  "git@github.com:luqizheng/world-view-rs.git";

const privateKey = createPrivateKey(
  readFileSync(resolve(KEYS, "private.pem"), "utf8")
);

/** 由测试私钥导出的 JWKS 文档（kid=default，与 digua_common_jwt::sign_token 一致）。 */
export function jwksDocument(): string {
  const jwk = createPublicKey(
    readFileSync(resolve(KEYS, "public.pem"), "utf8")
  ).export({
    format: "jwk"
  });
  return JSON.stringify({
    keys: [{ ...jwk, kid: "default", alg: "RS256", use: "sig" }]
  });
}

const b64url = (input: Buffer | string): string =>
  Buffer.from(input).toString("base64url");

export function signToken(sub: string, tid: number): string {
  const now = Math.floor(Date.now() / 1000);
  const header = b64url(
    JSON.stringify({ alg: "RS256", typ: "JWT", kid: "default" })
  );
  const payload = b64url(
    JSON.stringify({
      sub,
      tid,
      username: sub,
      roles: [],
      permissions: [],
      aud: "coder-micro-services",
      iat: now,
      exp: now + 3600
    })
  );
  const signature = sign(
    "RSA-SHA256",
    Buffer.from(`${header}.${payload}`),
    privateKey
  );
  return `${header}.${payload}.${b64url(signature)}`;
}

export interface ApiResult<T = any> {
  status: number;
  body: T;
}

/** 直连 pm-api（不经 vite）：用于造数与终态断言。 */
export async function api<T = any>(
  method: string,
  path: string,
  token: string,
  body?: unknown
): Promise<ApiResult<T>> {
  const res = await fetch(`${PM_API}${path}`, {
    method,
    headers: {
      authorization: `Bearer ${token}`,
      ...(body === undefined ? {} : { "content-type": "application/json" })
    },
    body: body === undefined ? undefined : JSON.stringify(body)
  });
  const text = await res.text();
  let parsed: any = null;
  try {
    parsed = text ? JSON.parse(text) : null;
  } catch {
    parsed = text;
  }
  return { status: res.status, body: parsed };
}

/** 浏览器端登录态（与 team-onboarding.spec 同一套 localStorage 键）。 */
export function browserSession(
  sub: string,
  tid: number,
  email: string | null = null
) {
  return {
    prism_access_token: signToken(sub, tid),
    prism_refresh_token: `flow-e2e-refresh-${sub}`,
    prism_expires_at: String(Date.now() + 3_600_000),
    prism_user: JSON.stringify({
      id: sub,
      username: sub,
      nickname: sub,
      email,
      phone: null,
      avatar: null,
      roles: [],
      permissions: []
    })
  };
}

/** 容器内 psql（`docker exec`）。返回 stdout；psql 失败会抛错。 */
export function psql(sql: string, db: string = DB_NAME): string {
  return execFileSync(
    "docker",
    ["exec", PG_CONTAINER, "psql", "-U", "aipm", "-d", db, "-tAc", sql],
    { encoding: "utf8", stdio: ["pipe", "pipe", "pipe"] }
  ).trim();
}

export interface SshKeyFacts {
  publicKey: string;
  fingerprint: string;
}

/** 读本机私钥的公钥与 SHA256 指纹（ssh-keygen）。 */
export function sshKeyFacts(keyPath: string = sshKeyPath()): SshKeyFacts {
  const pub = execFileSync("ssh-keygen", ["-y", "-f", keyPath], {
    encoding: "utf8"
  }).trim();
  const fpLine = execFileSync("ssh-keygen", ["-lf", keyPath], {
    encoding: "utf8"
  }).trim();
  const fingerprint = fpLine.split(/\s+/)[1] ?? "";
  if (!pub.startsWith("ssh-ed25519 ") || !fingerprint.startsWith("SHA256:")) {
    throw new Error(
      `unexpected ssh key material at ${keyPath}: fp=${fingerprint}`
    );
  }
  return { publicKey: `ssh-ed25519 ${pub.split(" ")[1]}`, fingerprint };
}

/**
 * 用**平台自己的加密路径**封装私钥（`cargo run --example seal_ssh_key`）。
 *
 * 红线：禁止在测试侧重实现 AES-256-GCM 信封（两套实现漂移 = 假绿/假红）。
 * 本函数只是调平台代码 + 解析 stdout。
 */
export function sealPrivateKey(
  projectId: string,
  keyPath: string = sshKeyPath()
): string {
  const out = execFileSync(
    "cargo",
    [
      "run",
      "-q",
      "-p",
      "pm-application",
      "--example",
      "seal_ssh_key",
      "--",
      projectId,
      keyPath
    ],
    {
      cwd: REPO_ROOT,
      encoding: "utf8",
      env: { ...process.env, AI_PM_SECRET_KEY: SECRET_KEY }
    }
  );
  const hex = out.trim().split(/\r?\n/).filter(Boolean).pop() ?? "";
  if (!/^[0-9a-f]{64,}$/i.test(hex)) {
    throw new Error(
      `seal_ssh_key produced no hex ciphertext: ${out.slice(-200)}`
    );
  }
  return hex;
}

/** 把本机私钥装进平台密钥库（`integration.project_ssh_key`，迁移 0022 多把模型）。 */
export function installSshKey(projectId: string): SshKeyFacts {
  const facts = sshKeyFacts();
  const hex = sealPrivateKey(projectId);
  psql(`
    INSERT INTO integration.project_ssh_key
      (project_id, label, key_type, public_key, private_key_ciphertext,
       fingerprint, cipher_suite, is_default, status, created_by, tenant_id)
    VALUES
      ('${projectId}', 'e2e-ui', 'ed25519', '${facts.publicKey}',
       decode('${hex}', 'hex'), '${facts.fingerprint}', 'aes256gcm-v1', true, 'active',
       (SELECT principal_id FROM core.project_member
         WHERE project_id = '${projectId}' AND role = 'owner' LIMIT 1),
       (SELECT tenant_id FROM core.project WHERE id = '${projectId}'))
    ON CONFLICT (project_id, label) DO UPDATE SET
      public_key = EXCLUDED.public_key,
      private_key_ciphertext = EXCLUDED.private_key_ciphertext,
      fingerprint = EXCLUDED.fingerprint,
      is_default = true,
      status = 'active',
      revoked_at = NULL
  `);
  return facts;
}

export interface ProjectRef {
  id: string;
  code: string;
  name: string;
}

/** API 造一个项目（真实链路），返回 id。 */
export async function createProject(
  token: string,
  code: string,
  name: string,
  description?: string
): Promise<ProjectRef> {
  const res = await api("POST", "/v1/pm/projects", token, {
    code,
    name,
    description: description ?? ""
  });
  if (res.status !== 201) {
    throw new Error(
      `create project ${code} failed: ${res.status} ${JSON.stringify(res.body)}`
    );
  }
  return {
    id: res.body.id as string,
    code: res.body.code as string,
    name: res.body.name as string
  };
}
