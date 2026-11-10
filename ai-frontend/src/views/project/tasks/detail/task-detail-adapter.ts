import type { TaskDetailView, TaskTrace } from "@/api/task-plans";

/**
 * S04-UI-03 Task Detail · 视图适配（真实读模型 → 页面模型）。
 *
 * 纪律：**只映射真有的字段**。后端无来源的段（执行说明 / 上下文 / AI 建议）
 * 一律返回空串 / 空数组，由页面渲染「暂无」——不用演示文案填充。
 *
 * 字段来源：
 * - 目标 ← task.goal；范围 ← task.description；
 * - 依据追溯 ← 需求直连 + 实施基线内的设计/ADR/基线；
 * - 依赖 ← 上下游双向（direction 由后端给定）；
 * - 仓库/模块 ← task ↔ repository 绑定（path_scope 拼模块路径）；
 * - 约束 ← task.constraints（jsonb）；交付 ← task.deliverables；
 * - 验收标准 ← task_acceptance_criterion；
 * - 历史审计 ← governance.audit_log（按任务）。
 */

export interface DetailTraceView {
  kind: string;
  label: string;
  version: string;
  status: string;
}

export interface DetailDependencyView {
  code: string;
  title: string;
  status: string;
  direction: "upstream" | "downstream";
}

export interface DetailCriterionView {
  id: string;
  description: string;
  method: string;
  evidence: string;
  mandatory: boolean;
}

export interface DetailHistoryView {
  id: string;
  actor: "ai" | "human";
  action: string;
  detail: string;
  at: string;
}

export interface DetailViewModel {
  id: string;
  title: string;
  status: string;
  priority: string;
  /** 人工批准状态：`approved` / `pending` / `none`。 */
  approval: "human_confirmed" | "pending" | "none";
  objective: string;
  background: string;
  scope: string;
  outOfScope: string;
  expected: string;
  traces: DetailTraceView[];
  dependencies: DetailDependencyView[];
  repo: string;
  module: string;
  instructions: string;
  constraints: string;
  context: string;
  criteria: DetailCriterionView[];
  requiredDeliverables: string[];
  optionalDeliverables: string[];
  history: DetailHistoryView[];
}

function text(value: unknown): string {
  if (typeof value === "string") return value;
  if (value === null || value === undefined) return "";
  if (typeof value === "number" || typeof value === "boolean")
    return String(value);
  return "";
}

function stringList(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value
    .map(item => {
      if (typeof item === "string") return item;
      if (item && typeof item === "object") {
        const record = item as Record<string, unknown>;
        const label =
          record.title ?? record.text ?? record.name ?? record.statement;
        return typeof label === "string" ? label : "";
      }
      return "";
    })
    .filter(item => item.length > 0);
}

function traceLabel(trace: TaskTrace): string {
  const code = trace.code ?? "";
  const title = trace.title ?? "";
  if (code && title) return `${code} · ${title}`;
  return code || title;
}

/** 真实读模型 → 页面模型。 */
export function toDetailView(detail: TaskDetailView): DetailViewModel {
  const task = detail.task;
  const repository = detail.repositories[0];
  const baseline = detail.baseline;
  const scope = baseline?.content?.scope;
  const excluded =
    scope && typeof scope === "object"
      ? (scope as Record<string, unknown>).excluded
      : undefined;

  // 人工批准：取任务级审批（governance.review_approval）；无审批行 = 未批准。
  const approvalRow = detail.approvals[0];
  const approval: DetailViewModel["approval"] = !approvalRow
    ? "none"
    : approvalRow.status === "approved"
      ? "human_confirmed"
      : "pending";

  return {
    id: task.id,
    title: task.title,
    status: task.status,
    priority: task.priority,
    approval,
    objective: text(task.goal),
    // 背景无独立字段（后端任务模型不含背景段）——不造文案。
    background: "",
    scope: text(task.description),
    outOfScope: stringList(excluded).join("；"),
    // 预期结果无独立字段——交付清单承载「产出什么」。
    expected: "",
    traces: detail.traces.map(
      (trace): DetailTraceView => ({
        kind: trace.kind,
        label: traceLabel(trace),
        version: trace.version ?? "",
        status: trace.state ?? ""
      })
    ),
    dependencies: detail.dependencies.map(
      (dep): DetailDependencyView => ({
        code: dep.id.slice(0, 8),
        title: dep.title,
        status: dep.status,
        direction: dep.direction === "downstream" ? "downstream" : "upstream"
      })
    ),
    repo: repository?.repository_name ?? "",
    module: (repository?.path_scope ?? []).join(" / "),
    // 执行说明无后端字段（任务模型只有 goal/constraints/deliverables）。
    instructions: "",
    constraints:
      text(task.constraints) === "{}" ? "" : JSON.stringify(task.constraints),
    context: "",
    criteria: detail.criteria.map(
      (criterion): DetailCriterionView => ({
        id: criterion.id.slice(0, 8),
        description: criterion.text,
        method: criterion.verification_kind,
        // 证据要求是人工在验收标准里写的，后端只存验证方式。
        evidence: "",
        mandatory: true
      })
    ),
    requiredDeliverables: stringList(task.deliverables),
    optionalDeliverables: [],
    history: detail.history.map(
      (entry): DetailHistoryView => ({
        id: entry.id,
        actor: "human",
        action: entry.action,
        detail: entry.actor_name ?? "",
        at: entry.created_at
      })
    )
  };
}
