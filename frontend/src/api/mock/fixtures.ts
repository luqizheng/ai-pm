/**
 * Mock 数据与请求解析 —— 仅供 build/mock-plugin.ts（node 侧）使用。
 *
 * 这里是后端契约的"活文档"：字段与 ai-pm/migrations 0001–0004 对齐（snake_case），
 * pm-api 实现对应端点后即可删除本目录与 mock 插件。
 * 不得 import 浏览器 API / Vue —— vite.config.ts 在 node 里加载本模块。
 */

export type MockRecord = {
  id: string;
  project_id?: string;
  created_at?: string;
} & Record<string, unknown>;

function daysAgo(days: number, hour = 10): string {
  const d = new Date();
  d.setDate(d.getDate() - days);
  d.setHours(hour, 30, 0, 0);
  return d.toISOString();
}

/** core.principal */
export const principals = [
  {
    id: "prc-zhang",
    kind: "human",
    display_name: "张伟",
    is_active: true,
    created_at: daysAgo(90)
  },
  {
    id: "prc-li",
    kind: "human",
    display_name: "李娜",
    is_active: true,
    created_at: daysAgo(85)
  },
  {
    id: "prc-agent-01",
    kind: "dev_agent",
    display_name: "dev-agent-01",
    is_active: true,
    created_at: daysAgo(60)
  },
  {
    id: "prc-aipm",
    kind: "ai_pm",
    display_name: "AI-PM",
    is_active: true,
    created_at: daysAgo(60)
  }
] satisfies MockRecord[];

/** core.project */
export const projects = [
  {
    id: "prj-shield",
    customer_id: "cus-xinghe",
    code: "SHIELD",
    name: "星河客服中台",
    description: "面向星河集团的多渠道客服与工单中台。",
    status: "active",
    revision: 12,
    created_at: daysAgo(80),
    updated_at: daysAgo(1)
  },
  {
    id: "prj-atlas",
    customer_id: "cus-yunfan",
    code: "ATLAS",
    name: "云帆数据门户",
    description: "云帆科技统一数据门户与自助报表。",
    status: "active",
    revision: 7,
    created_at: daysAgo(50),
    updated_at: daysAgo(2)
  },
  {
    id: "prj-horizon",
    customer_id: "cus-xinghe",
    code: "HORIZON",
    name: "星河移动端",
    description: "客服中台配套的移动端 App，规划阶段。",
    status: "planning",
    revision: 2,
    created_at: daysAgo(10),
    updated_at: daysAgo(3)
  }
] satisfies MockRecord[];

/** delivery.requirement */
export const requirements = [
  {
    id: "req-101",
    project_id: "prj-shield",
    code: "REQ-101",
    title: "工单聚合视图",
    status: "approved",
    current_revision_id: "rev-101-2",
    row_revision: 3,
    created_at: daysAgo(40)
  },
  {
    id: "req-102",
    project_id: "prj-shield",
    code: "REQ-102",
    title: "渠道接入标准化（微信/邮件）",
    status: "approved",
    current_revision_id: "rev-102-1",
    row_revision: 1,
    created_at: daysAgo(35)
  },
  {
    id: "req-103",
    project_id: "prj-shield",
    code: "REQ-103",
    title: "客服绩效报表",
    status: "draft",
    current_revision_id: null,
    row_revision: 2,
    created_at: daysAgo(12)
  },
  {
    id: "req-104",
    project_id: "prj-shield",
    code: "REQ-104",
    title: "SLA 倒计时告警",
    status: "draft",
    current_revision_id: null,
    row_revision: 1,
    created_at: daysAgo(6)
  },
  {
    id: "req-201",
    project_id: "prj-atlas",
    code: "REQ-201",
    title: "自助取数向导",
    status: "approved",
    current_revision_id: "rev-201-1",
    row_revision: 1,
    created_at: daysAgo(30)
  },
  {
    id: "req-202",
    project_id: "prj-atlas",
    code: "REQ-202",
    title: "报表订阅与分发",
    status: "draft",
    current_revision_id: null,
    row_revision: 1,
    created_at: daysAgo(9)
  },
  {
    id: "req-203",
    project_id: "prj-atlas",
    code: "REQ-203",
    title: "数据源健康巡检",
    status: "superseded",
    current_revision_id: "rev-203-1",
    row_revision: 2,
    created_at: daysAgo(28)
  }
] satisfies MockRecord[];

/** delivery.task */
export const tasks = [
  {
    id: "tsk-001",
    project_id: "prj-shield",
    milestone_id: "ms-1",
    title: "工单聚合视图 API 设计",
    description: null,
    status: "done",
    priority: "high",
    assignee_id: "prc-agent-01",
    revision: 3,
    created_at: daysAgo(20),
    updated_at: daysAgo(4)
  },
  {
    id: "tsk-002",
    project_id: "prj-shield",
    milestone_id: "ms-1",
    title: "微信渠道适配器实现",
    description: null,
    status: "in_progress",
    priority: "high",
    assignee_id: "prc-zhang",
    revision: 2,
    created_at: daysAgo(10),
    updated_at: daysAgo(1)
  },
  {
    id: "tsk-003",
    project_id: "prj-shield",
    milestone_id: "ms-2",
    title: "SLA 倒计时前端组件",
    description: null,
    status: "backlog",
    priority: "normal",
    assignee_id: "prc-li",
    revision: 1,
    created_at: daysAgo(5),
    updated_at: daysAgo(5)
  },
  {
    id: "tsk-004",
    project_id: "prj-shield",
    milestone_id: null,
    title: "绩效报表指标口径确认",
    description: "与业务方确认统计口径。",
    status: "blocked",
    priority: "normal",
    assignee_id: "prc-li",
    revision: 4,
    created_at: daysAgo(8),
    updated_at: daysAgo(2)
  },
  {
    id: "tsk-005",
    project_id: "prj-shield",
    milestone_id: "ms-2",
    title: "渠道接入回归测试用例",
    description: null,
    status: "backlog",
    priority: "low",
    assignee_id: null,
    revision: 1,
    created_at: daysAgo(3),
    updated_at: daysAgo(3)
  },
  {
    id: "tsk-101",
    project_id: "prj-atlas",
    milestone_id: "ms-11",
    title: "取数向导 DSL 设计",
    description: null,
    status: "in_progress",
    priority: "urgent",
    assignee_id: "prc-agent-01",
    revision: 2,
    created_at: daysAgo(14),
    updated_at: daysAgo(1)
  },
  {
    id: "tsk-102",
    project_id: "prj-atlas",
    milestone_id: "ms-11",
    title: "报表订阅队列与投递",
    description: null,
    status: "backlog",
    priority: "normal",
    assignee_id: null,
    revision: 1,
    created_at: daysAgo(6),
    updated_at: daysAgo(6)
  },
  {
    id: "tsk-103",
    project_id: "prj-atlas",
    milestone_id: null,
    title: "数据源巡检任务拆分",
    description: null,
    status: "cancelled",
    priority: "low",
    assignee_id: "prc-zhang",
    revision: 3,
    created_at: daysAgo(25),
    updated_at: daysAgo(20)
  }
] satisfies MockRecord[];

/** governance.approval_request */
export const approvals = [
  {
    id: "apr-001",
    project_id: "prj-shield",
    proposal_id: "csp-001",
    status: "pending",
    required_role: "project_owner",
    decided_by: null,
    decision_reason: null,
    decided_at: null,
    created_at: daysAgo(0, 9)
  },
  {
    id: "apr-002",
    project_id: "prj-atlas",
    proposal_id: "csp-002",
    status: "pending",
    required_role: "project_owner",
    decided_by: null,
    decision_reason: null,
    decided_at: null,
    created_at: daysAgo(1, 15)
  },
  {
    id: "apr-003",
    project_id: "prj-shield",
    proposal_id: "csp-003",
    status: "approved",
    required_role: "project_owner",
    decided_by: "prc-zhang",
    decision_reason: "影响面可控，同意执行。",
    decided_at: daysAgo(2, 16),
    created_at: daysAgo(2, 10)
  },
  {
    id: "apr-004",
    project_id: "prj-atlas",
    proposal_id: "csp-004",
    status: "rejected",
    required_role: "project_owner",
    decided_by: "prc-li",
    decision_reason: "发布窗口与客户活动冲突，驳回。",
    decided_at: daysAgo(3, 11),
    created_at: daysAgo(4, 9)
  }
] satisfies MockRecord[];

/** governance.audit_log */
export const auditLogs = [
  {
    id: "aud-001",
    project_id: "prj-shield",
    actor_id: "prc-agent-01",
    action: "requirement.propose_revision",
    target_type: "requirement",
    target_id: "req-103",
    created_at: daysAgo(0, 9)
  },
  {
    id: "aud-002",
    project_id: "prj-shield",
    actor_id: "prc-zhang",
    action: "approval.decide",
    target_type: "approval_request",
    target_id: "apr-003",
    created_at: daysAgo(2, 16)
  },
  {
    id: "aud-003",
    project_id: "prj-shield",
    actor_id: "prc-aipm",
    action: "command.execute",
    target_type: "command",
    target_id: "cmd-010",
    created_at: daysAgo(2, 16)
  },
  {
    id: "aud-004",
    project_id: "prj-atlas",
    actor_id: "prc-li",
    action: "approval.decide",
    target_type: "approval_request",
    target_id: "apr-004",
    created_at: daysAgo(3, 11)
  },
  {
    id: "aud-005",
    project_id: "prj-atlas",
    actor_id: "prc-agent-01",
    action: "task.create",
    target_type: "task",
    target_id: "tsk-102",
    created_at: daysAgo(6, 14)
  },
  {
    id: "aud-006",
    project_id: "prj-shield",
    actor_id: "prc-aipm",
    action: "task.reorder",
    target_type: "task",
    target_id: "tsk-003",
    created_at: daysAgo(4, 10)
  },
  {
    id: "aud-007",
    project_id: "prj-shield",
    actor_id: "prc-zhang",
    action: "requirement.approve_revision",
    target_type: "requirement",
    target_id: "req-102",
    created_at: daysAgo(30, 11)
  },
  {
    id: "aud-008",
    project_id: "prj-atlas",
    actor_id: "prc-zhang",
    action: "human_intervention.freeze",
    target_type: "project",
    target_id: "prj-atlas",
    created_at: daysAgo(7, 17)
  }
] satisfies MockRecord[];

/** 登录返回的用户（shell store/auth 的 StoredUser 形状） */
export const mockUser = {
  id: "usr-admin",
  username: "admin",
  nickname: "管理员",
  email: null,
  phone: null,
  avatar: null,
  roles: ["project_owner"],
  permissions: []
};

/**
 * 需求修订（delivery.requirement_revision）契约。
 * content 目前是 `{markdown: string}`（V1；正文权威后续迁 Markdown+Git）。
 * 定稿：draft → approved（同需求其余 approved 转 superseded，requirement
 * .current_revision_id 指向新定稿、status 置 approved）。
 */
export interface MockRevision {
  id: string;
  requirement_id: string;
  project_id: string;
  version: number;
  content: { markdown?: string } & Record<string, unknown>;
  state: "draft" | "approved" | "superseded";
  content_hash: string;
  created_by: string;
  approved_by: string | null;
  created_at: string;
}

function hashOf(text: string): string {
  // mock 用简单确定性哈希（后端用 sha256 真值），仅保证形态一致
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return `mock${h.toString(16).padStart(8, "0")}`.padEnd(24, "0");
}

export const revisions: MockRevision[] = [
  {
    id: "rev-101-1",
    requirement_id: "req-101",
    project_id: "prj-shield",
    version: 1,
    content: { markdown: "# 工单聚合视图\n\n初始版本：聚合多渠道工单列表。" },
    state: "superseded",
    content_hash: hashOf("v1"),
    created_by: "prc-zhang",
    approved_by: "prc-zhang",
    created_at: daysAgo(38)
  },
  {
    id: "rev-101-2",
    requirement_id: "req-101",
    project_id: "prj-shield",
    version: 2,
    content: {
      markdown:
        "# 工单聚合视图\n\nv2：增加按渠道分组与 SLA 倒计时列，支持导出。"
    },
    state: "approved",
    content_hash: hashOf("v2"),
    created_by: "prc-agent-01",
    approved_by: "prc-zhang",
    created_at: daysAgo(20)
  },
  {
    id: "rev-201-1",
    requirement_id: "req-201",
    project_id: "prj-atlas",
    version: 1,
    content: { markdown: "# 自助取数向导\n\n初始定稿版本。" },
    state: "approved",
    content_hash: hashOf("v201"),
    created_by: "prc-li",
    approved_by: "prc-li",
    created_at: daysAgo(28)
  }
];

/** 新建修订（POST /revisions）：版本自增、state=draft、created_by=mockUser */
function createRevision(
  requirementId: string,
  markdown: string
): MockRevision | undefined {
  const requirement = requirements.find(item => item.id === requirementId);
  if (!requirement) return undefined;
  const existing = revisions.filter(
    item => item.requirement_id === requirementId
  );
  const version =
    existing.reduce((max, item) => Math.max(max, item.version), 0) + 1;
  const revision: MockRevision = {
    id: `rev-${requirementId}-${version}`,
    requirement_id: requirementId,
    project_id: requirement.project_id,
    version,
    content: { markdown },
    state: "draft",
    content_hash: hashOf(`${requirementId}:${version}:${markdown}`),
    created_by: mockUser.id,
    approved_by: null,
    created_at: new Date().toISOString()
  };
  revisions.push(revision);
  return revision;
}

/** 定稿（POST /approve）：draft→approved，其余 approved 转 superseded */
function approveRevision(
  requirementId: string,
  revisionId: string
): MockRevision | undefined {
  const revision = revisions.find(
    item => item.id === revisionId && item.requirement_id === requirementId
  );
  if (!revision || revision.state !== "draft") return undefined;
  for (const item of revisions) {
    if (item.requirement_id === requirementId && item.state === "approved") {
      item.state = "superseded";
    }
  }
  revision.state = "approved";
  revision.approved_by = mockUser.id;
  const requirement = requirements.find(item => item.id === requirementId);
  if (requirement) {
    requirement.current_revision_id = revision.id;
    requirement.status = "approved";
  }
  return revision;
}

function loginResult() {
  return {
    access_token: "mock-access-token",
    refresh_token: "mock-refresh-token",
    token_type: "Bearer",
    expires_in: 86400,
    user: mockUser
  };
}

/**
 * DB 菜单（GET /v1/pure/get-async-routes 的 data）。
 * component 字符串对应 src/shell/views/<component>.vue。
 */
export const mockAsyncRoutes = [
  {
    path: "/overview",
    name: "PmOverview",
    component: "pm/overview",
    meta: { title: "概览", icon: "layout-dashboard" }
  },
  {
    path: "/delivery",
    name: "DeliveryGroup",
    meta: { title: "交付" },
    children: [
      {
        path: "/projects",
        name: "PmProjects",
        component: "pm/projects",
        meta: { title: "项目", icon: "folder-tree" }
      },
      {
        path: "/requirements",
        name: "PmRequirements",
        component: "pm/requirements",
        meta: { title: "需求", icon: "file-text" }
      },
      {
        path: "/tasks",
        name: "PmTasks",
        component: "pm/tasks",
        meta: { title: "任务", icon: "list-tree" }
      }
    ]
  },
  {
    path: "/governance",
    name: "GovernanceGroup",
    meta: { title: "治理" },
    children: [
      {
        path: "/approvals",
        name: "PmApprovals",
        component: "pm/approvals",
        meta: { title: "审批确认", icon: "shield-check" }
      },
      {
        path: "/audit",
        name: "PmAudit",
        component: "pm/audit",
        meta: { title: "审计日志", icon: "scroll-text" }
      }
    ]
  },
  {
    path: "/pm-settings",
    name: "PmSettings",
    component: "pm/settings",
    meta: { title: "设置", icon: "sliders-horizontal" }
  }
];

function sortDesc<T extends MockRecord>(items: T[]): T[] {
  return [...items].sort((a, b) =>
    (b.created_at ?? "").localeCompare(a.created_at ?? "")
  );
}

function filterByProject<T extends MockRecord>(
  items: T[],
  projectId: string | null
): T[] {
  return projectId
    ? items.filter(item => item.project_id === projectId)
    : items;
}

function paginate<T>(
  items: T[],
  query: URLSearchParams
): {
  items: T[];
  total: number;
  page: number;
  size: number;
} {
  const page = Math.max(1, Number(query.get("page") ?? 1) || 1);
  const size = Math.min(
    200,
    Math.max(1, Number(query.get("size") ?? 10) || 10)
  );
  return {
    items: items.slice((page - 1) * size, page * size),
    total: items.length,
    page,
    size
  };
}

export interface MockResponse {
  status: number;
  body: unknown;
}

/**
 * 审批决策契约（POST /v1/pm/approvals/{id}/decide）。
 * decision ∈ approved | rejected；reason 必填。
 * 成功 200 `{id, status, decided_at}`；404 未知 id；409 已决策；400 非法 decision。
 */
export interface DecideBody {
  decision?: string;
  reason?: string;
}

/** 解析一个 mock 请求；未注册的端点返回 undefined（走代理/404） */
export function resolveMock(
  method: string,
  pathname: string,
  query: URLSearchParams,
  body?: unknown
): MockResponse | undefined {
  const projectId = query.get("project_id");
  const list = (items: MockRecord[]) =>
    paginate(sortDesc(filterByProject(items, projectId)), query);

  if (method === "POST") {
    // 需求修订：新建（version 自增 draft）/ 定稿（draft→approved）
    const newRevision = pathname.match(
      /^\/v1\/pm\/requirements\/([^/]+)\/revisions$/
    );
    if (newRevision) {
      const payload = (body ?? {}) as { content?: { markdown?: string } };
      const markdown = payload.content?.markdown ?? "";
      if (markdown.trim() === "") {
        return {
          status: 400,
          body: { message: "content.markdown is required" }
        };
      }
      const revision = createRevision(newRevision[1], markdown);
      if (!revision) return { status: 404, body: { message: "not found" } };
      return { status: 201, body: revision };
    }
    const approve = pathname.match(
      /^\/v1\/pm\/requirements\/([^/]+)\/revisions\/([^/]+)\/approve$/
    );
    if (approve) {
      const revision = approveRevision(approve[1], approve[2]);
      if (!revision)
        return { status: 409, body: { message: "not a draft revision" } };
      return { status: 200, body: revision };
    }

    // 任务：创建 / 编辑（PATCH 语义在 mock 中等同 post 到 /{id}/update，
    // 真后端用 PATCH /v1/pm/tasks/{id}；两者都落在下方 PATCH 分支）
    const newTask = pathname === "/v1/pm/tasks";
    if (newTask) {
      const payload = (body ?? {}) as {
        project_id?: string;
        milestone_id?: string | null;
        title?: string;
        description?: string | null;
        priority?: string;
        assignee_id?: string | null;
      };
      const project = projects.find(item => item.id === payload.project_id);
      if (!project)
        return { status: 404, body: { message: "unknown project" } };
      if (!payload.title || payload.title.trim() === "") {
        return { status: 400, body: { message: "title is required" } };
      }
      const now = new Date().toISOString();
      const task = {
        id: `tsk-${Date.now()}`,
        project_id: project.id,
        milestone_id: payload.milestone_id ?? null,
        title: payload.title.trim(),
        description: payload.description ?? null,
        status: "backlog",
        priority: payload.priority ?? "normal",
        assignee_id: payload.assignee_id ?? null,
        revision: 1,
        created_at: now,
        updated_at: now
      };
      tasks.push(task as never);
      return { status: 201, body: task };
    }

    const decide = pathname.match(/^\/v1\/pm\/approvals\/([^/]+)\/decide$/);
    if (decide) {
      const payload = (body ?? {}) as DecideBody;
      const approval = approvals.find(item => item.id === decide[1]);
      if (!approval) return { status: 404, body: { message: "not found" } };
      if (approval.status !== "pending") {
        return { status: 409, body: { message: "already decided" } };
      }
      if (payload.decision !== "approved" && payload.decision !== "rejected") {
        return { status: 400, body: { message: "invalid decision" } };
      }
      if (!payload.reason || payload.reason.trim() === "") {
        return { status: 400, body: { message: "reason is required" } };
      }
      approval.status = payload.decision;
      approval.decided_by = mockUser.id;
      approval.decision_reason = payload.reason;
      approval.decided_at = new Date().toISOString();
      return {
        status: 200,
        body: {
          id: approval.id,
          status: approval.status,
          decided_at: approval.decided_at
        }
      };
    }

    switch (pathname) {
      case "/v1/iam/auth/login":
      case "/v1/iam/auth/refresh":
        return { status: 200, body: loginResult() };
      case "/v1/iam/auth/logout":
        return { status: 200, body: {} };
    }
    return undefined;
  }

  if (method === "PATCH") {
    // 任务编辑：全字段 + revision 乐观锁（不匹配 409）
    const patchTask = pathname.match(/^\/v1\/pm\/tasks\/([^/]+)$/);
    if (patchTask) {
      const task = tasks.find(item => item.id === patchTask[1]) as
        | (typeof tasks)[number]
        | undefined;
      if (!task) return { status: 404, body: { message: "not found" } };
      const payload = (body ?? {}) as {
        revision?: number;
        title?: string;
        description?: string | null;
        status?: string;
        priority?: string;
        assignee_id?: string | null;
      };
      if (payload.revision !== task.revision) {
        return { status: 409, body: { message: "revision conflict" } };
      }
      if (!payload.title || payload.title.trim() === "") {
        return { status: 400, body: { message: "title is required" } };
      }
      if (
        ![
          "backlog",
          "in_progress",
          "in_review",
          "done",
          "blocked",
          "cancelled"
        ].includes(payload.status ?? "")
      ) {
        return { status: 400, body: { message: "invalid status" } };
      }
      if (
        !["low", "normal", "high", "urgent"].includes(payload.priority ?? "")
      ) {
        return { status: 400, body: { message: "invalid priority" } };
      }
      task.title = payload.title.trim();
      task.description = payload.description ?? null;
      task.status = payload.status as typeof task.status;
      task.priority = payload.priority as typeof task.priority;
      task.assignee_id = payload.assignee_id ?? null;
      task.revision += 1;
      task.updated_at = new Date().toISOString();
      return { status: 200, body: task };
    }
    return undefined;
  }

  if (method !== "GET") return undefined;

  switch (pathname) {
    case "/v1/iam/auth/setup-status":
      return { status: 200, body: { setup_required: false } };
    case "/v1/pure/get-async-routes":
      return {
        status: 200,
        body: { code: 0, message: "ok", data: mockAsyncRoutes }
      };
    case "/v1/iam/users/simple": {
      const items = principals.map(p => ({
        id: p.id,
        nickname: p.display_name
      }));
      return {
        status: 200,
        body: { items, total: items.length, page: 1, size: items.length }
      };
    }
    case "/v1/pm/principals":
      return { status: 200, body: list(principals) };
    case "/v1/pm/projects":
      return { status: 200, body: list(projects) };
    case "/v1/pm/requirements":
      return { status: 200, body: list(requirements) };
    case "/v1/pm/tasks":
      return { status: 200, body: list(tasks) };
    case "/v1/pm/approvals":
      return { status: 200, body: list(approvals) };
    case "/v1/pm/audit-logs":
      return { status: 200, body: list(auditLogs) };
  }

  // 修订历史：GET /v1/pm/requirements/{id}/revisions
  const history = pathname.match(
    /^\/v1\/pm\/requirements\/([^/]+)\/revisions$/
  );
  if (history) {
    const items = revisions
      .filter(item => item.requirement_id === history[1])
      .sort((a, b) => b.version - a.version);
    return { status: 200, body: { items, total: items.length } };
  }
  return undefined;
}
