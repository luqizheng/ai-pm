import { createPrivateKey, createPublicKey, sign } from "node:crypto";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

/**
 * Team QA · 真实后端栈的测试侧工具（team-onboarding.spec.ts 专用）。
 *
 * 身份 = 自签 RS256 JWT（复用 pm-api 集成测试的 `apps/pm-api/tests/keys`），
 * 由 global-setup 起的本地 JWKS 服务发布公钥；pm-api 以 jwks 模式校验。
 * 每个用例用独立 tid → 独立租户（认领建团要求租户零 ACTIVE 成员）。
 */

const HERE = dirname(fileURLToPath(import.meta.url));
const KEYS = resolve(HERE, "../../../apps/pm-api/tests/keys");

export const PM_API = process.env.TEAM_QA_PM_API ?? "http://127.0.0.1:18490";
export const JWKS_PORT = Number(process.env.TEAM_QA_JWKS_PORT ?? 18491);

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

/** 浏览器端登录态（与 s02-ui.spec 同一套 localStorage 键）。 */
export function browserSession(
  sub: string,
  tid: number,
  email: string | null = null
) {
  return {
    prism_access_token: signToken(sub, tid),
    prism_refresh_token: `team-qa-refresh-${sub}`,
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

/** 以 sub/tid 认领建团，返回 owner token 与 tenant_id。 */
export async function claimTeam(
  sub: string,
  tid: number,
  name: string,
  slug?: string
) {
  const token = signToken(sub, tid);
  const created = await api("POST", "/v1/pm/teams", token, { name, slug });
  if (created.status !== 201) {
    throw new Error(
      `claim ${sub}@${tid} failed: ${created.status} ${JSON.stringify(created.body)}`
    );
  }
  const me = await api("GET", "/v1/pm/me", token);
  return { token, tenantId: me.body.tenant_id as string };
}

/** 以邀请把 sub 拉进团队并设为指定角色（经真实邀请链路）。 */
export async function joinAs(
  ownerToken: string,
  tenantId: string,
  sub: string,
  tid: number,
  role: "member" | "admin"
) {
  const inv = await api(
    "POST",
    `/v1/pm/tenants/${tenantId}/invitations`,
    ownerToken,
    {
      email: `${sub}@team-qa.test`,
      role
    }
  );
  const token = signToken(sub, tid);
  const accepted = await api("POST", "/v1/pm/invitations/accept", token, {
    token: inv.body.token
  });
  if (accepted.status !== 200) {
    throw new Error(
      `join ${sub} failed: ${accepted.status} ${JSON.stringify(accepted.body)}`
    );
  }
  return token;
}
