/**
 * 执行规划演示数据（S04-UI-05 / S04-UI-06，dev mock 专用）。
 *
 * 后端缺口（见 docs/ui/s04/INDEX 与后端勘察）：项目级依赖图聚合、
 * 任务 planned dates、并行组、准备度聚合均无 REST 端点——本目录 fixtures
 * 演示交互模型；仅由 mock-preview 条件注册的路由动态引入，不进发布 dist。
 * fixtures 豁免域（i18n 硬编码门禁豁免）。
 */

// ─── S04-UI-05 依赖图 ────────────────────────────────────────────────

export interface PlanningTaskFixture {
  code: string;
  title: string;
  priority: string;
  status: string;
  repo: string;
  module: string;
  /** 关键路径（⚑ accent 描边） */
  critical: boolean;
  /** 可并行（◇ 虚线 halo） */
  parallel: boolean;
  /** 依赖原因（检查器展示） */
  reason: string;
}

export interface PlanningEdgeFixture {
  /** 前置（predecessor） */
  from: string;
  /** 后继（successor） */
  to: string;
}

export interface ParallelGroupFixture {
  id: string;
  name: string;
  taskCodes: string[];
  preconditions: string;
  repos: string[];
  conflicts: string;
  rationale: string;
}

export type PlanningRecKind =
  | "parallel"
  | "bottleneck"
  | "conflict"
  | "critical";

export interface PlanningRecFixture {
  id: string;
  kind: PlanningRecKind;
  text: string;
  impact: string;
  /** 涉及的任务编号（展示用） */
  taskCodes?: string[];
  /** parallel 类采纳后落地为一个并行组 */
  newGroup?: {
    name: string;
    taskCodes: string[];
    rationale: string;
    preconditions: string;
    repos: string[];
    conflicts: string;
  };
}

export const planningTasks: PlanningTaskFixture[] = [
  {
    code: "T-001",
    title: "实现认证 API",
    priority: "high",
    status: "ready",
    repo: "core-api",
    module: "services/auth",
    critical: true,
    parallel: false,
    reason: "认证域的入口任务，无前置。"
  },
  {
    code: "T-002",
    title: "实现令牌校验",
    priority: "urgent",
    status: "ready",
    repo: "core-api",
    module: "services/auth",
    critical: true,
    parallel: false,
    reason: "ADR-007：受保护端点统一走令牌校验，必须先于业务端点就绪。"
  },
  {
    code: "T-003",
    title: "实现登录 UI",
    priority: "high",
    status: "ready",
    repo: "web-app",
    module: "features/login",
    critical: false,
    parallel: true,
    reason: "前端登录流与令牌签发解耦，可并行推进。"
  },
  {
    code: "T-004",
    title: "登录表单校验",
    priority: "normal",
    status: "ready",
    repo: "web-app",
    module: "features/login",
    critical: false,
    parallel: true,
    reason: "纯前端校验，不依赖服务端行为。"
  },
  {
    code: "T-005",
    title: "实现用户档案 API",
    priority: "high",
    status: "draft",
    repo: "core-api",
    module: "services/profile",
    critical: true,
    parallel: false,
    reason: "档案接口的鉴权依赖 T-002 的令牌校验中间件。"
  },
  {
    code: "T-006",
    title: "实现档案 UI",
    priority: "normal",
    status: "draft",
    repo: "web-app",
    module: "features/profile",
    critical: false,
    parallel: true,
    reason: "界面骨架不依赖档案接口返回结构，可先行。"
  },
  {
    code: "T-007",
    title: "会话中间件加固",
    priority: "urgent",
    status: "blocked",
    repo: "core-api",
    module: "middleware/session",
    critical: false,
    parallel: false,
    reason: "密钥轮换方案（ADR-007 修订）未裁决前不能动工。"
  },
  {
    code: "T-008",
    title: "登录限流策略",
    priority: "high",
    status: "blocked",
    repo: "gateway",
    module: "gateway/ratelimit",
    critical: false,
    parallel: false,
    reason: "依赖网关侧的限流配置任务。"
  },
  {
    code: "T-009",
    title: "认证集成测试",
    priority: "high",
    status: "draft",
    repo: "core-api",
    module: "tests/auth",
    critical: true,
    parallel: false,
    reason: "集成测试必须在认证链路与档案 UI 稳定后执行。"
  },
  {
    code: "T-010",
    title: "审计埋点",
    priority: "low",
    status: "draft",
    repo: "core-api",
    module: "services/audit",
    critical: false,
    parallel: true,
    reason: "埋点为横切能力，可与认证主线并行。"
  },
  {
    code: "T-011",
    title: "密钥轮换脚本",
    priority: "high",
    status: "draft",
    repo: "core-api",
    module: "services/auth",
    critical: false,
    parallel: false,
    reason: "轮换脚本依赖会话中间件加固后的密钥约定。"
  },
  {
    code: "T-014",
    title: "配置迁移",
    priority: "normal",
    status: "draft",
    repo: "gateway",
    module: "gateway/config",
    critical: false,
    parallel: false,
    reason: "网关配置迁移需在路由变更前完成。"
  },
  {
    code: "T-015",
    title: "网关路由调整",
    priority: "normal",
    status: "draft",
    repo: "gateway",
    module: "gateway/config",
    critical: false,
    parallel: false,
    reason: "路由调整与配置迁移操作同一模块，需串行。"
  }
];

export const planningEdges: PlanningEdgeFixture[] = [
  { from: "T-001", to: "T-002" },
  { from: "T-002", to: "T-005" },
  { from: "T-002", to: "T-007" },
  { from: "T-003", to: "T-004" },
  { from: "T-002", to: "T-009" },
  { from: "T-006", to: "T-009" },
  { from: "T-007", to: "T-011" },
  { from: "T-009", to: "T-011" },
  { from: "T-008", to: "T-014" },
  { from: "T-014", to: "T-015" },
  // 校验演示：引用不存在的任务编号（Unknown reference）
  { from: "T-060", to: "T-008" }
];

export function planningGroupsFixture(): ParallelGroupFixture[] {
  return [
    {
      id: "G-A",
      name: "认证基础",
      taskCodes: ["T-001", "T-002"],
      preconditions: "core-api 仓库已验证；BASE-v1.2 已批准。",
      repos: ["core-api"],
      conflicts: "无。",
      rationale: "同域串行链路，组间无共享模块。"
    },
    {
      id: "G-B",
      name: "登录前端",
      taskCodes: ["T-003", "T-004"],
      preconditions: "web-app 仓库已验证。",
      repos: ["web-app"],
      conflicts: "无。",
      rationale: "纯前端改造，与服务端令牌签发解耦。"
    },
    {
      id: "G-C",
      name: "观测与配置",
      taskCodes: ["T-010", "T-014"],
      preconditions: "网关配置基线已快照。",
      repos: ["core-api", "gateway"],
      conflicts: "T-014 与 T-015 同模块，须与网关路由调整串行。",
      rationale: "埋点与配置迁移互不依赖。"
    }
  ];
}

export function planningRecsFixture(): PlanningRecFixture[] {
  return [
    {
      id: "R-1",
      kind: "parallel",
      text: "T-003（登录 UI）与 T-005（档案 API）可并行执行。",
      impact: "预计缩短关键路径 2 天。",
      newGroup: {
        name: "前后端并行组",
        taskCodes: ["T-003", "T-005"],
        preconditions: "T-002 令牌校验已就绪。",
        repos: ["web-app", "core-api"],
        conflicts: "无共享模块。",
        rationale: "两任务无依赖边且分属不同仓库。"
      }
    },
    {
      id: "R-2",
      kind: "bottleneck",
      text: "T-007 阻塞 T-011，且依赖 ADR-007 修订裁决，是当前瓶颈。",
      impact: "不解决将顺延密钥轮换与验收收尾。",
      taskCodes: ["T-007", "T-011"]
    },
    {
      id: "R-3",
      kind: "conflict",
      text: "T-014 与 T-015 修改同一模块（gateway/config），建议保持串行。",
      impact: "并行会产生配置回写冲突。",
      taskCodes: ["T-014", "T-015"]
    },
    {
      id: "R-4",
      kind: "critical",
      text: "关键路径为 T-001 → T-002 → T-005 → T-009 → T-011。",
      impact: "关键路径上任一延期直接顺延整体交付。",
      taskCodes: ["T-001", "T-002", "T-005", "T-009", "T-011"]
    }
  ];
}

// ─── S04-UI-06 时间线 ────────────────────────────────────────────────

export interface TimelineRowFixture {
  code: string;
  featureId: string;
  title: string;
  priority: string;
  status: string;
  repo: string;
  assignee: string;
  effortDays: number;
  /** 起始日（ISO yyyy-mm-dd）；null = 未排期 */
  start: string | null;
  /** 关键路径条（accent） */
  critical: boolean;
  /** 阻塞条（斜纹） */
  blocked: boolean;
}

export interface TimelineFeatureFixture {
  id: string;
  name: string;
}

export interface TimelineMilestoneFixture {
  id: string;
  name: string;
  date: string;
}

export interface TimelineDepFixture {
  from: string;
  to: string;
}

/** 时间线网格起点（周一）；窗口 6 周 */
export const TIMELINE_START = "2026-11-16";
export const TIMELINE_DAYS = 42;

export function timelineFeatures(): TimelineFeatureFixture[] {
  return [
    { id: "f-auth", name: "用户认证（Authentication）" },
    { id: "f-profile", name: "用户管理（User Management）" }
  ];
}

export function timelineRows(): TimelineRowFixture[] {
  return [
    {
      code: "T-001",
      featureId: "f-auth",
      title: "实现认证 API",
      priority: "high",
      status: "ready",
      repo: "core-api",
      assignee: "amy",
      effortDays: 3,
      start: "2026-11-16",
      critical: true,
      blocked: false
    },
    {
      code: "T-002",
      featureId: "f-auth",
      title: "实现令牌校验",
      priority: "urgent",
      status: "ready",
      repo: "core-api",
      assignee: "amy",
      effortDays: 2,
      start: "2026-11-19",
      critical: true,
      blocked: false
    },
    {
      code: "T-003",
      featureId: "f-auth",
      title: "实现登录 UI",
      priority: "high",
      status: "ready",
      repo: "web-app",
      assignee: "rui",
      effortDays: 3,
      start: "2026-11-19",
      critical: false,
      blocked: false
    },
    {
      code: "T-004",
      featureId: "f-auth",
      title: "登录表单校验",
      priority: "normal",
      status: "ready",
      repo: "web-app",
      assignee: "rui",
      effortDays: 2,
      start: "2026-11-24",
      critical: false,
      blocked: false
    },
    {
      code: "T-009",
      featureId: "f-auth",
      title: "认证集成测试",
      priority: "high",
      status: "blocked",
      repo: "core-api",
      assignee: "leo",
      effortDays: 3,
      start: "2026-11-26",
      critical: false,
      blocked: true
    },
    {
      code: "T-005",
      featureId: "f-profile",
      title: "实现用户档案 API",
      priority: "high",
      status: "draft",
      repo: "core-api",
      assignee: "amy",
      effortDays: 4,
      start: "2026-11-23",
      critical: true,
      blocked: false
    },
    {
      code: "T-006",
      featureId: "f-profile",
      title: "实现档案 UI",
      priority: "normal",
      status: "draft",
      repo: "web-app",
      assignee: "rui",
      effortDays: 3,
      start: "2026-11-27",
      critical: false,
      blocked: false
    },
    {
      code: "T-007",
      featureId: "f-profile",
      title: "会话中间件加固",
      priority: "urgent",
      status: "blocked",
      repo: "core-api",
      assignee: "leo",
      effortDays: 2,
      start: "2026-11-30",
      critical: false,
      blocked: true
    },
    {
      code: "T-011",
      featureId: "f-profile",
      title: "密钥轮换脚本",
      priority: "high",
      status: "draft",
      repo: "core-api",
      assignee: "leo",
      effortDays: 2,
      start: "2026-12-04",
      critical: false,
      blocked: false
    },
    {
      code: "T-010",
      featureId: "f-auth",
      title: "审计埋点",
      priority: "low",
      status: "draft",
      repo: "core-api",
      assignee: "amy",
      effortDays: 2,
      start: null,
      critical: false,
      blocked: false
    },
    {
      code: "T-014",
      featureId: "f-profile",
      title: "配置迁移",
      priority: "normal",
      status: "draft",
      repo: "gateway",
      assignee: "rui",
      effortDays: 2,
      start: null,
      critical: false,
      blocked: false
    },
    {
      code: "T-015",
      featureId: "f-profile",
      title: "网关路由调整",
      priority: "normal",
      status: "draft",
      repo: "gateway",
      assignee: "rui",
      effortDays: 2,
      start: null,
      critical: false,
      blocked: false
    }
  ];
}

export function timelineDeps(): TimelineDepFixture[] {
  return [
    { from: "T-001", to: "T-002" },
    { from: "T-002", to: "T-005" },
    { from: "T-002", to: "T-009" },
    { from: "T-006", to: "T-009" },
    { from: "T-003", to: "T-006" },
    { from: "T-007", to: "T-011" }
  ];
}

export function timelineMilestones(): TimelineMilestoneFixture[] {
  return [
    { id: "M-1", name: "认证基础完成", date: "2026-11-21" },
    { id: "M-2", name: "API 集成完成", date: "2026-11-27" },
    { id: "M-3", name: "集成测试完成", date: "2026-12-08" },
    { id: "M-4", name: "实施基线交付", date: "2026-12-12" }
  ];
}

/** AI 排期建议：preview 由页面按 delta 计算幽灵条 */
export interface TimelineRecFixture {
  id: string;
  text: string;
  impact: string;
  /** 建议移动的任务与其天数增量（正 = 后移） */
  move?: { code: string; deltaDays: number };
}

export function timelineRecs(): TimelineRecFixture[] {
  return [
    {
      id: "TL-1",
      text: "把 T-006（档案 UI）提前 2 天，可与 T-005 联调窗口对齐。",
      impact: "前端联调不被档案 API 阻塞。",
      move: { code: "T-006", deltaDays: -2 }
    },
    {
      id: "TL-2",
      text: "T-004 与 T-009 在同一周集中落在 core-api，建议 T-009 后移 3 天。",
      impact: "缓解 core-api 同周负载峰值。",
      move: { code: "T-009", deltaDays: 3 }
    },
    {
      id: "TL-3",
      text: "当前排期在「集成测试完成」里程碑前形成瓶颈（T-011 依赖阻塞的 T-007）。",
      impact: "里程碑有顺延风险，建议先解阻 T-007。"
    }
  ];
}
