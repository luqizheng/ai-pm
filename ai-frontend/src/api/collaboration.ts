import { http } from "@/api/base";

/**
 * AI 协作工作区客户端（PA-06 UI · Figma 258:14081）。
 *
 * 决策中心（屏 C）的队列 = 三类「需要人类判断」的事项拼装，不另起后端
 * 聚合端点（读面盘点 PA06-IMP-001：三类端点均已存在）：
 *   - human_consultation（正式决策工作项，pending 待决）；
 *   - /v1/pm/decisions（审批请求 LEFT JOIN AI 变更提案的决策中心读模型，
 *     前端过滤 pending——后端无 status 过滤参数）；
 *   - waiting 队列（needs_decision 分组计数，仅作页签徽标来源）。
 *
 * 行为规范（画布 Screen feedback）：刷新≠批准、错误不扩权、空态不抹历史、
 * loading 不是执行状态。已决事项保留在列表可见（空态不抹历史）。
 */

/** governance.human_consultation 行（GET /v1/pm/human-consultations） */
export interface PendingConsultation {
  id: string;
  project_id: string;
  task_id: string | null;
  title: string;
  question: string;
  status: string;
  decision_content: string | null;
  decided_by: string | null;
  decided_at: string | null;
  product_scope: unknown;
  scope_note: string | null;
  cancelled_reason: string | null;
  supersedes_consultation_id: string | null;
  created_by: string;
  created_at: string;
}

/** /v1/pm/decisions 决策中心读模型（approval_request LEFT JOIN change_proposal） */
export interface PendingDecision {
  id: string;
  project_id: string;
  status: string;
  required_role: string;
  decided_by: string | null;
  decision_reason: string | null;
  decided_at: string | null;
  created_at: string;
  proposal_id: string | null;
  proposal_status: string | null;
  target_type: string | null;
  command_type: string | null;
  rationale: string | null;
  risk_level: string | null;
  evidence: unknown[] | null;
  proposed_data: Record<string, unknown> | null;
  analysis_run_id: string | null;
}

export type DecisionKind = "consultation" | "approval";

/** 决策队列统一行：两类来源归一，供队列/详情双屏渲染 */
export interface DecisionQueueItem {
  kind: DecisionKind;
  /** consultation id 或 approval_request id */
  id: string;
  /** 类目行（画布 D-018 · Formal requirement 的角色） */
  kindLabel: string;
  title: string;
  /** 副行：等待对象 / 风险级 · 提出时间 */
  meta: string;
  createdAt: string;
  /** 详情段：WHY HUMAN INVOLVEMENT IS REQUIRED */
  why: string;
  /** 详情段：AI-PM recommendation（consultation=question，proposal=rationale） */
  recommendation: string;
  /** 详情段：OPTIONS & IMPACT 的结构化呈现（无则省略段落） */
  options: Array<{ label: string; impact: string }>;
  /** 挂任务（受影响工作链接） */
  taskId: string | null;
  status: string;
  /** consultation 专属：已决内容 / 撤销理由（空态不抹历史） */
  decisionContent: string | null;
  cancelledReason: string | null;
}

export interface DecisionCenter {
  items: DecisionQueueItem[];
  /** 待决数（consultation pending + approval pending） */
  pendingCount: number;
  /** waiting 队列 needs_decision 计数（页签徽标口径，可能含队列外来源） */
  waitingDecisionCount: number;
}

function waitingMeta(row: PendingConsultation): string {
  const target = row.task_id ? `T-${row.task_id.slice(0, 8)}` : "—";
  const time = new Date(row.created_at).toLocaleTimeString();
  return `${target} waiting · Raised ${time}`;
}

export async function fetchConsultations(
  projectId: string,
  status?: string
): Promise<PendingConsultation[]> {
  const params = new URLSearchParams({ project_id: projectId });
  if (status) params.set("status", status);
  const res = await http.get<{ items: PendingConsultation[] }>(
    `/v1/pm/human-consultations?${params.toString()}`
  );
  return res.items ?? [];
}

export async function fetchPendingDecisions(
  projectId: string
): Promise<PendingDecision[]> {
  const res = await http.get<{ items: PendingDecision[]; total: number }>(
    `/v1/pm/decisions?project_id=${projectId}&page=1&size=100`
  );
  return (res.items ?? []).filter(row => row.status === "pending");
}

/** waiting 队列（needs_decision 分组计数；只读，答复走详情域） */
export async function fetchWaitingDecisionCount(
  projectId: string
): Promise<number> {
  const res = await http.get<{
    waiting: Array<{ reason: string }>;
    interactions: unknown[];
    approvals: unknown[];
  }>(`/v1/pm/projects/${projectId}/waiting`);
  return (res.waiting ?? []).filter(row => row.reason === "needs_decision")
    .length;
}

/** waiting 队列行（execution_read HumanAttentionQueue.waiting，屏 G 常规恢复段） */
export interface WaitingRow {
  task_id: string;
  task_title: string;
  reason: string;
  since?: string | null;
  deadline?: string | null;
}

export async function fetchWaitingQueue(
  projectId: string
): Promise<WaitingRow[]> {
  const res = await http.get<{ waiting: WaitingRow[] }>(
    `/v1/pm/projects/${projectId}/waiting`
  );
  return res.waiting ?? [];
}

function consultationToItem(row: PendingConsultation): DecisionQueueItem {
  return {
    kind: "consultation",
    id: row.id,
    kindLabel: row.task_id ? "Human consultation · Task" : "Human consultation",
    title: row.title,
    meta: waitingMeta(row),
    createdAt: row.created_at,
    why: row.question,
    recommendation: row.scope_note ?? row.question,
    options: [],
    taskId: row.task_id,
    status: row.status,
    decisionContent: row.decision_content,
    cancelledReason: row.cancelled_reason
  };
}

function decisionToItem(row: PendingDecision): DecisionQueueItem {
  const risk = row.risk_level ? row.risk_level : "review";
  return {
    kind: "approval",
    id: row.id,
    kindLabel: row.command_type
      ? `AI proposal · ${row.command_type}`
      : "AI proposal",
    title: row.rationale?.split("\n")[0] ?? row.command_type ?? "AI proposal",
    meta: `${risk} · Raised ${new Date(row.created_at).toLocaleTimeString()}`,
    createdAt: row.created_at,
    why:
      row.rationale ??
      "AI proposal awaiting approval within the red-line gate.",
    recommendation: row.rationale ?? "",
    options: (row.evidence ?? [])
      .map(item => String(item))
      .filter(text => text.trim().length > 0)
      .map(text => ({ label: text, impact: "" })),
    taskId: null,
    status: row.status,
    decisionContent: row.decision_reason,
    cancelledReason: null
  };
}

/** 决策中心聚合：三类来源拼装（consultation 在前=画布 D-018 置顶语义） */
export async function fetchDecisionCenter(
  projectId: string
): Promise<DecisionCenter> {
  const [consultations, decisions, waitingDecisionCount] = await Promise.all([
    fetchConsultations(projectId),
    fetchPendingDecisions(projectId),
    fetchWaitingDecisionCount(projectId).catch(() => 0)
  ]);
  const items = [
    ...consultations.map(consultationToItem),
    ...decisions.map(decisionToItem)
  ];
  const pendingCount = items.filter(item => item.status === "pending").length;
  return { items, pendingCount, waitingDecisionCount };
}

/** consultation 决策（决定人=当前调用者，服务端执法） */
export async function decideConsultation(
  id: string,
  decisionContent: string
): Promise<void> {
  await http.post(`/v1/pm/human-consultations/${id}/decide`, {
    decision_content: decisionContent
  });
}

/** consultation 撤销（必填理由） */
export async function cancelConsultation(
  id: string,
  reason: string
): Promise<void> {
  await http.post(`/v1/pm/human-consultations/${id}/cancel`, { reason });
}

/** delivery.issue/risk 域（PA-05 db7f59a）：屏 A 重要风险段数据源 */
export interface RiskRow {
  id: string;
  project_id: string;
  title: string;
  description: string | null;
  impact: string;
  probability: string;
  status: string;
  created_at: string;
}

export async function fetchProjectRisks(projectId: string): Promise<RiskRow[]> {
  const res = await http.get<{ items: RiskRow[] }>(
    `/v1/pm/projects/${projectId}/risks`
  );
  return res.items ?? [];
}
