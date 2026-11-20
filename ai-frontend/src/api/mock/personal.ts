import { approvals, projects } from "./fixtures.ts";
import type { MockResponse } from "./fixtures.ts";

/**
 * 个人主页（Personal Home）所需、但通用 fixtures 尚未覆盖的端点。
 *
 * 与 fixtures.ts 分开维护的原因：fixtures.ts 是 pm 业务通用夹具，
 * 本文件只服务「跨项目个人聚合」这一条链路，避免两处夹具互相污染。
 *
 * 覆盖端点：
 *   - `GET /v1/pm/me`                       当前主体
 *   - `GET /v1/pm/decisions`                待决策（审批请求 + 关联变更提案）
 *   - `GET /v1/pm/projects/{id}/members`    项目成员（取当前用户在该项目中的角色）
 */

/** 演示主体：与 fixtures 中 prc-zhang（张伟）对应 */
const DEMO_PRINCIPAL_ID = "prc-zhang";

const DEMO_ME = {
  principal_id: DEMO_PRINCIPAL_ID,
  display_name: "张伟",
  is_active: true,
  is_admin: false,
  tenant_id: "cus-xinghe",
  team_role: "member",
  provider: "lmms",
  claims_tenant_key: "1",
  subject: "usr-zhang",
  auth_mode: "interactive"
};

/** 项目成员演示数据：核心角色字段用 core.project_member.role 的合法取值 */
const PROJECT_MEMBERS: Record<
  string,
  Array<{
    principal_id: string;
    display_name: string;
    role: string;
    kind: string;
    is_active: boolean;
  }>
> = {
  "prj-shield": [
    {
      principal_id: "prc-zhang",
      display_name: "张伟",
      role: "owner",
      kind: "human",
      is_active: true
    },
    {
      principal_id: "prc-li",
      display_name: "李娜",
      role: "member",
      kind: "human",
      is_active: true
    },
    {
      principal_id: "prc-agent-01",
      display_name: "dev-agent-01",
      role: "developer",
      kind: "dev_agent",
      is_active: true
    }
  ],
  "prj-atlas": [
    {
      principal_id: "prc-zhang",
      display_name: "张伟",
      role: "member",
      kind: "human",
      is_active: true
    },
    {
      principal_id: "prc-agent-01",
      display_name: "dev-agent-01",
      role: "architect",
      kind: "dev_agent",
      is_active: true
    }
  ],
  "prj-horizon": [
    {
      principal_id: "prc-zhang",
      display_name: "张伟",
      role: "owner",
      kind: "human",
      is_active: true
    }
  ]
};

/** 待决策读模型：把审批请求补上提案侧的展示字段 */
const RATIONALES: Record<string, string> = {
  "apr-001": "将工单聚合视图的验收标准写入正式需求基线",
  "apr-002": "接入微信与邮件两个渠道，统一为标准渠道协议"
};

function buildDecisions() {
  return approvals.map(approval => ({
    id: approval.id,
    project_id: approval.project_id,
    status: approval.status,
    required_role: approval.required_role,
    decided_by: approval.decided_by,
    decision_reason: approval.decision_reason,
    decided_at: approval.decided_at,
    created_at: approval.created_at,
    proposal_id: approval.proposal_id,
    proposal_status: approval.status,
    target_type: "Requirement",
    command_type: "requirement.approve",
    rationale: RATIONALES[approval.id] ?? null,
    risk_level: "low",
    evidence: [],
    proposed_data: {},
    analysis_run_id: null
  }));
}

export function resolvePersonalMock(
  method: string,
  pathname: string,
  query: URLSearchParams,
  _body?: unknown
): MockResponse | undefined {
  if (method !== "GET") return undefined;

  if (pathname === "/v1/pm/me") {
    return { status: 200, body: DEMO_ME };
  }

  if (pathname === "/v1/pm/decisions") {
    const items = buildDecisions().filter(decision => {
      const projectId = query.get("project_id");
      return projectId ? decision.project_id === projectId : true;
    });
    return { status: 200, body: { items, total: items.length } };
  }

  const members = pathname.match(/^\/v1\/pm\/projects\/([^/]+)\/members$/);
  if (members) {
    const projectId = members[1];
    const project = projects.find(item => item.id === projectId);
    if (!project) {
      return { status: 404, body: { message: "project not found" } };
    }
    const items = PROJECT_MEMBERS[projectId] ?? [];
    return { status: 200, body: { items, total: items.length } };
  }

  return undefined;
}
