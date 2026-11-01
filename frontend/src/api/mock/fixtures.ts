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
    customer_name: "星河集团",
    customer_status: "active",
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
    customer_name: "云帆科技",
    customer_status: "active",
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
    customer_name: "星河集团",
    customer_status: "active",
    code: "HORIZON",
    name: "星河移动端",
    description: "客服中台配套的移动端 App，规划阶段。",
    status: "planning",
    revision: 2,
    created_at: daysAgo(10),
    updated_at: daysAgo(3)
  }
] satisfies MockRecord[];

/** core.customer（0017 客户主数据）：一个客户对应多个项目 */
export const customers = [
  {
    id: "cus-xinghe",
    code: "XINGHE",
    name: "星河集团股份有限公司",
    short_name: "星河集团",
    industry: "零售",
    notes: null,
    parent_id: null,
    status: "active",
    project_count: 2,
    created_at: daysAgo(90),
    updated_at: daysAgo(2)
  },
  {
    id: "cus-yunfan",
    code: "YUNFAN",
    name: "云帆科技有限公司",
    short_name: "云帆科技",
    industry: "软件",
    notes: null,
    parent_id: null,
    status: "active",
    project_count: 1,
    created_at: daysAgo(70),
    updated_at: daysAgo(5)
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

// ---------- lmms 基础服务（iam/audit/config/file/notification/prue）mock 数据 ----------
// 字段对齐各 *-services SDK 的 DTO（即后端 serde 序列化）。

/** GET /v1/iam/users（全量用户，UserResponse 形状） */
export const iamUsers = [
  {
    id: "usr-admin",
    username: "admin",
    email: "admin@digua.dev",
    phone: null,
    nickname: "管理员",
    avatar: null,
    status: "active",
    email_verified: true,
    phone_verified: false,
    last_login_at: daysAgo(0, 9),
    last_login_ip: "10.0.0.1",
    created_at: daysAgo(120),
    updated_at: daysAgo(1)
  },
  {
    id: "usr-zhang",
    username: "zhangwei",
    email: "zhangwei@digua.dev",
    phone: null,
    nickname: "张伟",
    avatar: null,
    status: "active",
    email_verified: true,
    phone_verified: false,
    last_login_at: daysAgo(1, 14),
    last_login_ip: "10.0.0.2",
    created_at: daysAgo(90),
    updated_at: daysAgo(2)
  },
  {
    id: "usr-li",
    username: "lina",
    email: "lina@digua.dev",
    phone: null,
    nickname: "李娜",
    avatar: null,
    status: "disabled",
    email_verified: false,
    phone_verified: false,
    last_login_at: daysAgo(30),
    last_login_ip: "10.0.0.3",
    created_at: daysAgo(85),
    updated_at: daysAgo(30)
  }
] satisfies MockRecord[];

/** GET /v1/iam/roles（RoleResponse 形状；data_scope 出参为字符串） */
export const iamRoles = [
  {
    id: "role-admin",
    name: "管理员",
    code: "admin",
    description: "平台管理员",
    data_scope: "1",
    status: "active",
    created_at: daysAgo(120),
    updated_at: daysAgo(10)
  },
  {
    id: "role-owner",
    name: "项目负责人",
    code: "project_owner",
    description: "项目内全权",
    data_scope: "2",
    status: "active",
    created_at: daysAgo(100),
    updated_at: daysAgo(9)
  },
  {
    id: "role-guest",
    name: "访客",
    code: "guest",
    description: "只读",
    data_scope: "5",
    status: "disabled",
    created_at: daysAgo(60),
    updated_at: daysAgo(60)
  }
] satisfies MockRecord[];

/** GET /v1/pure/menu-configs（MenuConfigSummary 形状） */
export const menuConfigs = [
  {
    id: "mc-1",
    version: 1,
    name: "v1 基线菜单",
    description: "首次灌入的基线菜单树",
    status: "published",
    is_active: true,
    created_by: "usr-admin",
    created_at: daysAgo(30),
    updated_at: daysAgo(29),
    published_at: daysAgo(29),
    routes_etag: "mock-etag-v1"
  },
  {
    id: "mc-2",
    version: 2,
    name: "v2 系统页扩展",
    description: "追加系统管理与审计入口",
    status: "draft",
    is_active: false,
    created_by: "usr-admin",
    created_at: daysAgo(3),
    updated_at: daysAgo(2),
    published_at: null,
    routes_etag: "mock-etag-v2"
  }
];

/** GET /v1/notification/announcements（裸数组，Announcement 形状） */
export const announcements = [
  {
    id: 1,
    title: "平台维护窗口公告",
    body: "本周六 22:00-24:00 平台升级，期间服务短暂不可用。",
    publish_at: daysAgo(5),
    pinned: 1,
    popover: 0,
    status: "published",
    created_by: "usr-admin",
    created_at: daysAgo(6),
    expire_at: null
  },
  {
    id: 2,
    title: "新权限模型征求意见",
    body: "拟引入数据范围（data_scope）到角色维度，欢迎反馈。",
    publish_at: daysAgo(1),
    pinned: 0,
    popover: 1,
    status: "draft",
    created_by: "usr-admin",
    created_at: daysAgo(2),
    expire_at: null
  }
];

/** GET /v1/audit/login（LoginLog 形状） */
export const auditLoginLogs = [
  {
    id: 1,
    user_id: "usr-admin",
    username: "admin",
    success: true,
    ip_address: "10.0.0.1",
    request_id: "req-l-001",
    created_at: daysAgo(0, 9),
    fail_reason: null,
    location: null,
    user_agent: "Chrome/126"
  },
  {
    id: 2,
    user_id: "usr-zhang",
    username: "zhangwei",
    success: false,
    ip_address: "10.0.0.2",
    request_id: "req-l-002",
    created_at: daysAgo(1, 22),
    fail_reason: "invalid credentials",
    location: null,
    user_agent: "Firefox/127"
  },
  {
    id: 3,
    user_id: "usr-li",
    username: "lina",
    success: true,
    ip_address: "10.0.0.3",
    request_id: "req-l-003",
    created_at: daysAgo(3, 11),
    fail_reason: null,
    location: null,
    user_agent: "Chrome/125"
  }
];

/** GET /v1/audit/operation（OperationLog 形状） */
export const auditOperationLogs = [
  {
    id: 1,
    user_id: "usr-admin",
    action: "user.create",
    resource: "iam_user",
    request_id: "req-o-001",
    created_at: daysAgo(1, 10),
    resource_id: "usr-li",
    before_value: null,
    after_value: '{"username":"lina"}',
    ip_address: "10.0.0.1",
    user_agent: "Chrome/126"
  },
  {
    id: 2,
    user_id: "usr-zhang",
    action: "role.assign",
    resource: "iam_user",
    request_id: "req-o-002",
    created_at: daysAgo(2, 15),
    resource_id: "usr-zhang",
    before_value: "[]",
    after_value: '["project_owner"]',
    ip_address: "10.0.0.2",
    user_agent: "Firefox/127"
  }
];

/** GET /v1/config/item（ConfigItem 形状） */
export const configItems = [
  {
    id: "cfg-1",
    namespace_id: "ns-aipm-prod",
    key: "db.pool.max",
    value: "20",
    value_type: "number",
    is_secret: false,
    created_at: daysAgo(40),
    updated_at: daysAgo(5),
    description: "连接池上限",
    value_masked: false
  },
  {
    id: "cfg-2",
    namespace_id: "ns-aipm-prod",
    key: "llm.api_key",
    value: "********",
    value_type: "string",
    is_secret: true,
    created_at: daysAgo(40),
    updated_at: daysAgo(5),
    description: "LLM 网关密钥",
    value_masked: true
  }
] satisfies MockRecord[];

/** GET /v1/file/list（FileInfo 形状） */
export const files = [
  {
    id: "file-001",
    filename: "需求规格-v2.pdf",
    content_type: "application/pdf",
    size: 483_211,
    storage_path: "/data/files/file-001",
    tenant_id: 1,
    is_temporary: false,
    created_at: daysAgo(12),
    updated_at: daysAgo(12),
    hash: "mock-hash-1",
    user_id: "usr-admin",
    expires_at: null,
    url: "/v1/file/file-001"
  },
  {
    id: "file-002",
    filename: "架构评审纪要.docx",
    content_type:
      "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    size: 92_400,
    storage_path: "/data/files/file-002",
    tenant_id: 1,
    is_temporary: false,
    created_at: daysAgo(4),
    updated_at: daysAgo(4),
    hash: "mock-hash-2",
    user_id: "usr-zhang",
    expires_at: null,
    url: "/v1/file/file-002"
  }
] satisfies MockRecord[];

/** GET /v1/iam/permissions（PermissionResponse 形状；category 出参为字符串） */
export const iamPermissions = [
  {
    id: "perm-1",
    code: "IAM_USER_READ",
    name: "读取用户",
    description: "查看用户列表与详情",
    category: "1",
    resource: "user",
    action: "read",
    sort_order: 1,
    status: "active",
    created_at: daysAgo(120),
    updated_at: daysAgo(10)
  },
  {
    id: "perm-2",
    code: "CFG_CONFIG_READ",
    name: "读取配置",
    description: "查看配置中心条目",
    category: "2",
    resource: "config",
    action: "read",
    sort_order: 2,
    status: "active",
    created_at: daysAgo(120),
    updated_at: daysAgo(10)
  }
] satisfies MockRecord[];

/** GET /v1/iam/org-units（部门树，OrgUnitResponse 形状） */
export const orgUnits = [
  {
    id: "org-1",
    parent_id: null,
    name: "总公司",
    code: "HQ",
    level: 1,
    sort_order: 1,
    status: "active",
    created_at: daysAgo(100),
    updated_at: daysAgo(8),
    children: [
      {
        id: "org-2",
        parent_id: "org-1",
        name: "研发部",
        code: "RD",
        level: 2,
        sort_order: 1,
        status: "active",
        created_at: daysAgo(90),
        updated_at: daysAgo(8),
        children: []
      },
      {
        id: "org-3",
        parent_id: "org-1",
        name: "交付部",
        code: "DELIVERY",
        level: 2,
        sort_order: 2,
        status: "active",
        created_at: daysAgo(90),
        updated_at: daysAgo(8),
        children: []
      }
    ]
  }
];

/** GET /v1/api-keys（ApiKeySummary 形状） */
export const apiKeys = [
  {
    id: "ak-001",
    name: "CI 流水线密钥",
    permissions: ["IAM_USER_READ", "FILE_READ"],
    status: "active",
    created_at: daysAgo(20)
  },
  {
    id: "ak-002",
    name: "pm-worker 服务密钥",
    permissions: ["PM_TASK_UPDATE"],
    status: "disabled",
    created_at: daysAgo(45)
  }
] satisfies MockRecord[];

/** GET /v1/iam/auth/sessions（裸数组，SessionInfo 形状） */
export const sessions = [
  {
    id: "ses-001",
    user_id: "usr-admin",
    client_ip: "10.0.0.1",
    user_agent: "Chrome/126",
    created_at: daysAgo(0, 9),
    expires_at: daysAgo(-30),
    is_current: true
  },
  {
    id: "ses-002",
    user_id: "usr-admin",
    client_ip: "10.0.0.9",
    user_agent: "Edge/126",
    created_at: daysAgo(2, 20),
    expires_at: daysAgo(-28),
    is_current: false
  }
];

/**
 * 全局 LLM 供应商配置（ai.llm_provider）。
 * API Key 明文不回传，只有 has_api_key 与末 4 位提示。
 */
export interface MockLlmProvider {
  provider: string;
  base_url: string;
  model: string;
  has_api_key: boolean;
  api_key_hint: string | null;
  enabled: boolean;
  timeout_secs: number;
  updated_at: string;
}

export const llmProviders: MockLlmProvider[] = [
  {
    provider: "deepseek",
    base_url: "https://api.deepseek.com/v1",
    model: "deepseek-chat",
    has_api_key: false,
    api_key_hint: null,
    enabled: false,
    timeout_secs: 120,
    updated_at: daysAgo(0, 9)
  },
  {
    provider: "openai",
    base_url: "https://api.openai.com/v1",
    model: "gpt-4o-mini",
    has_api_key: false,
    api_key_hint: null,
    enabled: false,
    timeout_secs: 120,
    updated_at: daysAgo(0, 9)
  },
  {
    provider: "glm",
    base_url: "https://open.bigmodel.cn/api/paas/v4",
    model: "glm-4-plus",
    has_api_key: false,
    api_key_hint: null,
    enabled: false,
    timeout_secs: 120,
    updated_at: daysAgo(0, 9)
  },
  {
    provider: "minimax",
    base_url: "https://api.minimax.chat/v1",
    model: "MiniMax-Text-01",
    has_api_key: false,
    api_key_hint: null,
    enabled: false,
    timeout_secs: 120,
    updated_at: daysAgo(0, 9)
  },
  {
    provider: "ollama",
    base_url: "http://127.0.0.1:11434/v1",
    model: "qwen2.5:7b",
    has_api_key: false,
    api_key_hint: null,
    enabled: false,
    timeout_secs: 120,
    updated_at: daysAgo(0, 9)
  }
];

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
  },
  {
    path: "/system",
    name: "SystemGroup",
    meta: { title: "系统管理" },
    children: [
      {
        path: "/system/users",
        name: "SystemUsers",
        component: "system/user",
        meta: { title: "用户管理", icon: "users", auths: ["IAM_USER_READ"] }
      },
      {
        path: "/system/roles",
        name: "SystemRoles",
        component: "system/role",
        meta: { title: "角色管理", icon: "key-round", auths: ["IAM_ROLE_READ"] }
      },
      {
        path: "/system/menus",
        name: "SystemMenus",
        component: "system/menu",
        meta: { title: "菜单配置", icon: "list-tree", auths: ["IAM_MENU_READ"] }
      },
      {
        path: "/system/sessions",
        name: "SystemSessions",
        component: "system/session",
        meta: { title: "在线会话", icon: "monitor-dot" }
      }
    ]
  },
  {
    path: "/audit-center",
    name: "AuditGroup",
    meta: { title: "审计" },
    children: [
      {
        path: "/audit/login-logs",
        name: "AuditLoginLogs",
        component: "audit/login-log",
        meta: { title: "登录日志", icon: "log-in", auths: ["AUD_LOG_READ"] }
      },
      {
        path: "/audit/operation-logs",
        name: "AuditOperationLogs",
        component: "audit/operation-log",
        meta: {
          title: "操作日志",
          icon: "scroll-text",
          auths: ["AUD_LOG_READ"]
        }
      }
    ]
  },
  {
    path: "/notifications",
    name: "SystemNotification",
    component: "notification/announcement",
    meta: { title: "公告管理", icon: "megaphone" }
  },
  {
    path: "/inbox",
    name: "NotificationInbox",
    component: "notification/inbox",
    meta: { title: "公告中心", icon: "bell" }
  },
  {
    path: "/config-items",
    name: "SystemConfig",
    component: "config/item",
    meta: { title: "系统配置", icon: "settings-2", auths: ["CFG_CONFIG_READ"] }
  },
  {
    path: "/files",
    name: "SystemFiles",
    component: "file",
    meta: { title: "文件管理", icon: "folder-open", auths: ["FILE_READ"] }
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

    // LLM 供应商连通性测试
    const llmTest = pathname.match(/^\/v1\/pm\/llm\/providers\/([^/]+)\/test$/);
    if (llmTest) {
      const row = llmProviders.find(item => item.provider === llmTest[1]);
      if (!row) return { status: 404, body: { message: "unknown provider" } };
      return {
        status: 200,
        body: { ok: true, latency_ms: 42, message: "HTTP 200" }
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

  if (method === "PUT") {
    // LLM 供应商配置：api_key 缺省/空串 = 保持原值；clear_api_key = 删除
    const updateProvider = pathname.match(
      /^\/v1\/pm\/llm\/providers\/([^/]+)$/
    );
    if (updateProvider) {
      const row = llmProviders.find(
        item => item.provider === updateProvider[1]
      );
      if (!row) return { status: 404, body: { message: "unknown provider" } };
      const payload = (body ?? {}) as {
        base_url?: string;
        model?: string;
        api_key?: string;
        clear_api_key?: boolean;
        enabled?: boolean;
        timeout_secs?: number;
      };
      const baseUrl = (payload.base_url ?? "").trim().replace(/\/+$/, "");
      if (!baseUrl.startsWith("http://") && !baseUrl.startsWith("https://")) {
        return {
          status: 400,
          body: { message: "base_url must start with http:// or https://" }
        };
      }
      if (!(payload.model ?? "").trim()) {
        return { status: 400, body: { message: "model is required" } };
      }
      if (
        payload.timeout_secs === undefined ||
        payload.timeout_secs < 1 ||
        payload.timeout_secs > 600
      ) {
        return {
          status: 400,
          body: { message: "timeout_secs must be between 1 and 600" }
        };
      }
      row.base_url = baseUrl;
      row.model = (payload.model ?? "").trim();
      row.enabled = Boolean(payload.enabled);
      row.timeout_secs = payload.timeout_secs;
      if (payload.clear_api_key) {
        row.has_api_key = false;
        row.api_key_hint = null;
      } else if (payload.api_key && payload.api_key.trim() !== "") {
        const key = payload.api_key.trim();
        row.has_api_key = true;
        row.api_key_hint = `****${key.slice(-4)}`;
      }
      row.updated_at = new Date().toISOString();
      return { status: 200, body: row };
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
    // ---- lmms 基础服务端点（系统页） ----
    case "/v1/iam/users":
      return { status: 200, body: paginate(iamUsers, query) };
    case "/v1/iam/roles":
      return { status: 200, body: paginate(iamRoles, query) };
    case "/v1/iam/permissions":
      return { status: 200, body: paginate(iamPermissions, query) };
    case "/v1/iam/org-units":
      return { status: 200, body: orgUnits };
    case "/v1/api-keys":
      return { status: 200, body: paginate(apiKeys, query) };
    case "/v1/iam/auth/sessions":
      return { status: 200, body: sessions };
    case "/v1/pure/menu-configs":
      return {
        status: 200,
        body: { items: menuConfigs, total: menuConfigs.length }
      };
    case "/v1/notification/announcements":
      return { status: 200, body: announcements };
    case "/v1/audit/login":
      return { status: 200, body: paginate(auditLoginLogs, query) };
    case "/v1/audit/operation":
      return { status: 200, body: paginate(auditOperationLogs, query) };
    case "/v1/config/item":
      return { status: 200, body: paginate(configItems, query) };
    case "/v1/file/list":
      return { status: 200, body: paginate(files, query) };
    case "/v1/pm/principals":
      return { status: 200, body: list(principals) };
    case "/v1/pm/projects":
      return { status: 200, body: list(projects) };
    case "/v1/pm/customers":
      return { status: 200, body: paginate(customers, query) };
    case "/v1/pm/requirements":
      return { status: 200, body: list(requirements) };
    case "/v1/pm/tasks":
      return { status: 200, body: list(tasks) };
    case "/v1/pm/approvals":
      return { status: 200, body: list(approvals) };
    case "/v1/pm/audit-logs":
      return { status: 200, body: list(auditLogs) };
    case "/v1/pm/llm/providers":
      return {
        status: 200,
        body: { items: llmProviders, total: llmProviders.length }
      };
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
