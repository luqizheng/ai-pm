import { http } from "@/api/base";
import {
  listAuditLogs,
  listDecisions,
  listPrincipals,
  listProjects,
  listTasks
} from "./pm";
import type { AuditLog, ListResponse, Principal, Project } from "./types";
import type { DecisionRecord } from "./workspace-view";

/**
 * 个人主页（Personal Home）跨项目聚合读模型 —— 前端适配层。
 *
 * 现状（2026-11-04 代码事实）：pm-api **没有**「个人工作台」聚合端点，
 * 唯一提供 Human Actions / Recent Activity 语义的
 * `GET /v1/pm/projects/{id}/command-center` 强制带项目路径段。
 * 本模块因此在**前端**用既有端点做跨项目聚合，并按区块拆分，
 * 使首页的「Inbox 失败 / 项目失败」可以各自独立表达（失败 ≠ 空）。
 *
 *   - `GET /v1/pm/me`                       当前主体
 *   - `GET /v1/pm/projects`                 跨项目项目清单（后端已按 project_member 过滤）
 *   - `GET /v1/pm/tasks`                    跨项目任务（后端已按 project_member 过滤）
 *   - `GET /v1/pm/decisions`                待决策（审批请求 + 关联变更提案）
 *   - `GET /v1/pm/audit-logs`               近期活动（治理审计）
 *   - `GET /v1/pm/projects/{id}/members`    项目成员（取当前用户在项目中的角色）
 *
 * 边界（不可越界）：
 *   1. 只做**读取与筛选**，不推断任何业务状态迁移；
 *   2. 每条事项都保留来源项目与原始业务对象入口，不在本页复制状态机；
 *   3. 后端已按 `core.project_member` 做可见性过滤，本模块不再放宽；
 *   4. 截止时间 / 逾期字段后端**当前没有**（delivery.task 无 due 列），
 *      因此 `DueInfo.dueAt` 恒为 null，页面显示「无期限」而不是虚构期限；
 *      字段就位后本模块无需改动即可自动生效。
 *
 * 目标形态：后端提供专门的个人工作台读模型后，本文件整体替换为
 * `GET /v1/pm/personal-home` 的一次调用
 * （见 docs/ui/s01/s01-05-personal-home-high-fidelity-ui-v1.md §11.2 F-1）。
 */

/** GET /v1/pm/me —— 当前会话主体 */
export interface CurrentPrincipal {
  principal_id: string;
  display_name: string;
  is_active: boolean;
  is_admin: boolean;
  tenant_id: string;
  tenant_role: string;
  provider: string;
  claims_tenant_key: string | null;
  subject: string;
  auth_mode: string;
}

/** GET /v1/pm/projects/{id}/members —— 项目成员 */
export interface ProjectMember {
  principal_id: string;
  display_name: string;
  role: string;
  kind: string;
  is_active: boolean;
}

/** 人工介入事项的类别 */
export type HumanActionKind = "decision" | "approval" | "review" | "attention";

/** 活动操作者类别 */
export type ActivityActorKind = "human" | "ai" | "system";

/** 活动事件类别 —— 事实类=已成立，AI 类=待人工确认 */
export type ActivityEventKind =
  | "confirmed"
  | "proposal"
  | "aiCompleted"
  | "submitted"
  | "generic";

/** 紧急度档位 —— 颜色只承载这一维度，类型由中性 chip 表达 */
export type DueState = "overdue" | "soon" | "normal" | "none";

export interface DueInfo {
  /** 截止时间；后端当前无该字段 → null */
  dueAt: string | null;
  /** 距今天数：负数=已逾期，正数=还剩几天；无期限为 null */
  daysUntilDue: number | null;
  /** 紧急度档位 */
  state: DueState;
}

/** 需要人工介入的事项（首页 Inbox 上分区） */
export interface HumanAction {
  id: string;
  kind: HumanActionKind;
  /** 事项标题；来源无标题时为 null，由页面回落到类型文案 */
  title: string | null;
  /** 原始业务对象短标识 */
  subject: string;
  /** 原始业务对象类型 */
  objectType: string;
  projectId: string;
  projectName: string;
  /** 进入原始业务对象的路径 */
  objectPath: string;
  due: DueInfo;
  /** 任务优先级（high / normal 等）；无优先级为 null */
  priority: string | null;
  createdAt: string;
}

/** 分配给当前用户的普通任务（首页 Inbox 下分区） */
export interface AssignedTask {
  id: string;
  title: string;
  subject: string;
  projectId: string;
  projectName: string;
  status: string;
  priority: string;
  objectPath: string;
  due: DueInfo;
  updatedAt: string;
}

/** 我参与的项目 */
export interface PersonalProject {
  id: string;
  name: string;
  code: string;
  description: string | null;
  status: string;
  /** 当前用户在项目中的角色；无成员行（未取到）为 null */
  role: string | null;
  /** 我的待办数量（人工事项 + 分配任务） */
  pendingCount: number;
  /** 最近活动时间 */
  lastActivityAt: string | null;
  /** 存在逾期人工事项 */
  attention: boolean;
  objectPath: string;
}

/** 近期活动条目 */
export interface ActivityEntry {
  id: string;
  actorKind: ActivityActorKind;
  actorName: string;
  eventKind: ActivityEventKind;
  /** 原始 action 字符串；页面按已知动作映射文案，未知则原样展示 */
  action: string;
  targetType: string;
  projectId: string;
  projectName: string;
  /** 关联业务对象标识 */
  objectRef: string | null;
  occurredAt: string;
}

/** 三个区块共享的上下文 */
export interface PersonalContext {
  me: CurrentPrincipal;
  projects: Project[];
  projectById: Map<string, Project>;
}

export interface InboxSection {
  humanActions: HumanAction[];
  assignedTasks: AssignedTask[];
}

export interface ProjectsSection {
  projects: PersonalProject[];
  archivedProjects: PersonalProject[];
}

/* -------------------------------------------------------------------------- */
/* 领域常量                                                                     */
/* -------------------------------------------------------------------------- */

/** 终态任务：不再出现在个人待办里 */
const TERMINAL_TASK_STATUS = new Set(["completed", "cancelled", "superseded"]);

/** 需要人工介入的任务状态 → 人工事项类别 */
const HUMAN_TASK_STATUS = new Map<string, HumanActionKind>([
  ["awaiting_decision", "decision"],
  ["submitted", "review"],
  ["reviewing", "review"],
  ["blocked", "attention"]
]);

/** 归档项目状态 */
const ARCHIVED_PROJECT_STATUS = "archived";

const DAY_MS = 86_400_000;

/** 取角色时的项目上限：避免成员查询放大到全量项目 */
const ROLE_LOOKUP_LIMIT = 12;

/* -------------------------------------------------------------------------- */
/* 端点                                                                         */
/* -------------------------------------------------------------------------- */

export function getCurrentPrincipal() {
  return http.get<CurrentPrincipal>("/v1/pm/me");
}

export function listProjectMembers(projectId: string) {
  return http.get<ListResponse<ProjectMember>>(
    `/v1/pm/projects/${projectId}/members`
  );
}

/* -------------------------------------------------------------------------- */
/* 派生                                                                         */
/* -------------------------------------------------------------------------- */

/** 截止信息 —— 无截止时间即「无期限」，不虚构期限 */
export function resolveDue(
  dueAt: string | null | undefined,
  now: number = Date.now()
): DueInfo {
  if (!dueAt) {
    return { dueAt: null, daysUntilDue: null, state: "none" };
  }
  const target = Date.parse(dueAt);
  if (Number.isNaN(target)) {
    return { dueAt: null, daysUntilDue: null, state: "none" };
  }
  const daysUntilDue = Math.ceil((target - now) / DAY_MS);
  const state: DueState =
    daysUntilDue < 0 ? "overdue" : daysUntilDue <= 3 ? "soon" : "normal";
  return { dueAt, daysUntilDue, state };
}

/**
 * 人工事项排序档位：
 *   1 已逾期 → 2 高紧急 → 3 临近截止 → 4 普通待处理 → 5 无明确期限
 * 同档内按创建时间倒序。
 */
export function humanActionRank(action: HumanAction): number {
  if (action.due.state === "overdue") return 0;
  if (action.priority === "high") return 1;
  if (action.due.state === "soon") return 2;
  if (action.due.state === "normal") return 3;
  return 4;
}

/** 活动事件分类 —— 未知动作一律降级为 generic，不得凭颜色伪造事实 */
export function classifyEvent(
  action: string,
  actorKind: ActivityActorKind
): ActivityEventKind {
  const value = action.toLowerCase();
  if (actorKind === "ai") {
    if (/complete|finish|done|succeed/.test(value)) return "aiCompleted";
    if (/propose|suggest|draft|analys/.test(value)) return "proposal";
    return "generic";
  }
  if (actorKind === "human") {
    if (/approve|confirm|decide|accept|sign|review/.test(value)) {
      return "confirmed";
    }
    return "generic";
  }
  if (/submit|commit|push|merge|publish/.test(value)) return "submitted";
  return "generic";
}

/** 操作者类别 —— 依据 core.principal.kind */
export function actorKindOf(
  principal: Principal | undefined
): ActivityActorKind {
  if (!principal) return "system";
  if (principal.kind === "human") return "human";
  if (principal.kind === "dev_agent" || principal.kind === "ai_pm") return "ai";
  return "system";
}

/** 短标识：长 id 取前 8 位 */
function shortRef(value: string | null | undefined): string {
  if (!value) return "";
  return value.length > 12 ? value.slice(0, 8) : value;
}

/** 项目在页面内的路径 */
function projectPath(projectId: string): string {
  return `/project/command-center?project_id=${projectId}`;
}

/* -------------------------------------------------------------------------- */
/* 区块加载                                                                     */
/* -------------------------------------------------------------------------- */

/** 共享上下文：当前主体 + 项目清单（三个区块共用） */
export async function loadPersonalContext(): Promise<PersonalContext> {
  const [me, projectsRes] = await Promise.all([
    getCurrentPrincipal(),
    listProjects({ page: 1, size: 200 })
  ]);
  const projects = projectsRes.items ?? [];
  const projectById = new Map<string, Project>();
  for (const project of projects) projectById.set(project.id, project);
  return { me, projects, projectById };
}

/** Inbox 区块：人工介入事项 + 分配给我的任务 */
export async function loadPersonalInbox(
  ctx: PersonalContext
): Promise<InboxSection> {
  const [tasksRes, decisionsRes] = await Promise.all([
    listTasks({ page: 1, size: 200 }),
    listDecisions({ page: 1, size: 200 })
  ]);
  const tasks = tasksRes.items ?? [];
  const decisions = decisionsRes.items ?? [];

  const projectName = (id: string): string =>
    ctx.projectById.get(id)?.name ?? "";

  const humanActions: HumanAction[] = [];

  // 1) 待决策：pending 审批请求 —— 需要人做出批准或否决
  for (const decision of decisions) {
    if (decision.status !== "pending") continue;
    humanActions.push({
      id: decision.id,
      kind: "approval",
      title: decision.rationale?.trim() || null,
      subject: shortRef(decision.proposal_id ?? decision.id),
      objectType: decision.target_type ?? "Proposal",
      projectId: decision.project_id,
      projectName: projectName(decision.project_id),
      objectPath: `/project/requirements?project_id=${decision.project_id}`,
      due: resolveDue(null),
      priority: null,
      createdAt: decision.created_at
    });
  }

  // 2) 分配给我的、需要人工介入的任务（待决策 / 待审查 / 阻塞）
  const myTasks = tasks.filter(
    task => task.assignee_id === ctx.me.principal_id
  );
  for (const task of myTasks) {
    const kind = HUMAN_TASK_STATUS.get(task.status);
    if (!kind) continue;
    humanActions.push({
      id: task.id,
      kind,
      title: task.title,
      subject: shortRef(task.id),
      objectType: "Task",
      projectId: task.project_id,
      projectName: projectName(task.project_id),
      objectPath: `/project/execution?project_id=${task.project_id}`,
      due: resolveDue(null),
      priority: task.priority ?? null,
      createdAt: task.created_at
    });
  }

  humanActions.sort((a, b) => {
    const rank = humanActionRank(a) - humanActionRank(b);
    if (rank !== 0) return rank;
    return b.createdAt.localeCompare(a.createdAt);
  });

  const assignedTasks: AssignedTask[] = myTasks
    .filter(task => !TERMINAL_TASK_STATUS.has(task.status))
    .filter(task => !HUMAN_TASK_STATUS.has(task.status))
    .map(task => ({
      id: task.id,
      title: task.title,
      subject: shortRef(task.id),
      projectId: task.project_id,
      projectName: projectName(task.project_id),
      status: task.status,
      priority: task.priority ?? "normal",
      objectPath: `/project/execution?project_id=${task.project_id}`,
      due: resolveDue(null),
      updatedAt: task.updated_at
    }));

  return { humanActions, assignedTasks };
}

/** 项目区块：活跃项目 + 归档项目（含当前用户在该项目中的角色） */
export async function loadPersonalProjects(
  ctx: PersonalContext
): Promise<ProjectsSection> {
  const visible = ctx.projects
    .filter(project => project.status !== ARCHIVED_PROJECT_STATUS)
    .slice(0, ROLE_LOOKUP_LIMIT);

  const roleByProject = new Map<string, string | null>();
  await Promise.all(
    visible.map(async project => {
      try {
        const members = await listProjectMembers(project.id);
        const mine = (members.items ?? []).find(
          member => member.principal_id === ctx.me.principal_id
        );
        roleByProject.set(project.id, mine?.role ?? null);
      } catch {
        // 成员查询失败不阻断页面：角色留空，其余信息照常展示
        roleByProject.set(project.id, null);
      }
    })
  );

  const toPersonal = (project: Project): PersonalProject => ({
    id: project.id,
    name: project.name,
    code: project.code,
    description: project.description ?? null,
    status: project.status,
    role: roleByProject.get(project.id) ?? null,
    pendingCount: 0,
    lastActivityAt: project.updated_at ?? null,
    attention: false,
    objectPath: projectPath(project.id)
  });

  return {
    projects: visible.map(toPersonal),
    archivedProjects: ctx.projects
      .filter(project => project.status === ARCHIVED_PROJECT_STATUS)
      .map(toPersonal)
  };
}

/** 活动区块：近期审计事件 */
export async function loadPersonalActivity(
  ctx: PersonalContext
): Promise<ActivityEntry[]> {
  const [auditRes, principalsRes] = await Promise.all([
    listAuditLogs({ page: 1, size: 50 }),
    listPrincipals({ page: 1, size: 200 })
  ]);
  const audits = auditRes.items ?? [];
  const principals = principalsRes.items ?? [];

  const principalById = new Map<string, Principal>();
  for (const principal of principals)
    principalById.set(principal.id, principal);

  return audits.slice(0, 12).map((log: AuditLog) => {
    const actor = principalById.get(log.actor_id);
    const actorKind = actorKindOf(actor);
    return {
      id: log.id,
      actorKind,
      actorName: actor?.display_name ?? "",
      eventKind: classifyEvent(log.action, actorKind),
      action: log.action,
      targetType: log.target_type,
      projectId: log.project_id,
      projectName: ctx.projectById.get(log.project_id)?.name ?? "",
      objectRef: shortRef(log.target_id),
      occurredAt: log.created_at
    };
  });
}

/** 聚合入口 —— 一次拿到全部区块（测试与整体刷新用） */
export async function getPersonalHome(): Promise<{
  principal: {
    id: string;
    displayName: string;
    tenantRole: string;
    isAdmin: boolean;
  };
  inbox: InboxSection;
  projects: ProjectsSection;
  activity: ActivityEntry[];
}> {
  const ctx = await loadPersonalContext();
  const [inbox, projects, activity] = await Promise.all([
    loadPersonalInbox(ctx),
    loadPersonalProjects(ctx),
    loadPersonalActivity(ctx)
  ]);
  return {
    principal: {
      id: ctx.me.principal_id,
      displayName: ctx.me.display_name,
      tenantRole: ctx.me.tenant_role,
      isAdmin: ctx.me.is_admin
    },
    inbox,
    projects,
    activity
  };
}

export type { DecisionRecord };
