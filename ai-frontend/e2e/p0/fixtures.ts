import { execFileSync } from "node:child_process";

import { api, claimTeam, signToken } from "../team/stack";

/**
 * P0 可执行套件共享 fixture（规格 12 §2.3 契约）。
 *
 * 纪律：全部走真实业务链路（建团 = POST /v1/pm/teams，主体 = 自签令牌按需
 * provision），**不手工插业务表**；唯一允许的直查是 psql 只读断言（终态双证据）。
 */

/** Team QA 栈的 PG 容器（与 playwright.team/p0 配置共用，见 team/global-setup.ts）。 */
export const P0_PG_CONTAINER =
  process.env.TEAM_QA_PG_CONTAINER ?? "aipm-teamqa-pg";
export const P0_PG_PORT = process.env.TEAM_QA_PG_PORT ?? "16499";
export const P0_DB = "aipm_teamqa_e2e";

/** 容器内 psql 只读断言（`docker exec`）。 */
export function psql(sql: string, db: string = P0_DB): string {
  return execFileSync(
    "docker",
    ["exec", P0_PG_CONTAINER, "psql", "-U", "aipm", "-d", db, "-tAc", sql],
    { encoding: "utf8", stdio: ["pipe", "pipe", "pipe"] }
  ).trim();
}

export interface TestUser {
  sub: string;
  tid: number;
  token: string;
}

export interface TestTeam {
  /** = core.tenant.id（Team ≡ tenant，R12：无 team_id 列） */
  id: string;
  name: string;
  slug: string;
}

export interface TestOwner extends TestUser {
  team: TestTeam;
}

export interface TestProject {
  id: string;
  code: string;
  name: string;
  tenantId: string;
}

/**
 * 建一个「已认领团队」的 owner（走真实建团链路，创建者由服务端写为 owner）。
 * 同一 tid 只能认领一次（建团要求租户零 ACTIVE 成员）——每个 owner 独占 tid。
 */
export async function buildOwner(
  sub: string,
  tid: number,
  teamName: string,
  slug: string
): Promise<TestOwner> {
  const claimed = await claimTeam(sub, tid, teamName, slug);
  return {
    sub,
    tid,
    token: claimed.token,
    team: { id: claimed.tenantId, name: teamName, slug }
  };
}

/** API 建项目（真实链路；`code` 必填，租户内唯一 → 409）。 */
export async function createProjectFor(
  owner: TestOwner,
  code: string,
  name: string,
  description?: string
): Promise<TestProject> {
  const res = await api("POST", "/v1/pm/projects", owner.token, {
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
    name: res.body.name as string,
    tenantId: owner.team.id
  };
}

/** 浏览器登录态注入（与 team/flow 套件同一套 localStorage 键）。 */
export async function signInVia(
  context: import("@playwright/test").BrowserContext,
  sub: string,
  tid: number
): Promise<void> {
  const { browserSession } = await import("../team/stack");
  await context.addInitScript(
    tokens => {
      for (const [key, value] of Object.entries(tokens)) {
        window.localStorage.setItem(key, value as string);
      }
    },
    browserSession(sub, tid)
  );
}
