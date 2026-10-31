import type { Principal } from "@/api/types";
import type { DecisionRecord } from "@/api/workspace-view";
import type { Decision } from "../fixtures";

/** 真模式：审批记录映射成的决策卡，附带原记录用于批准/驳回。 */
export interface PageDecision extends Decision {
  record?: DecisionRecord;
}

export interface DecisionTexts {
  approvalTitle: string;
  noRationale: string;
  requiredRole: (role: string) => string;
  riskLabel: (level: string) => string;
  formatDecidedAt: (value: string) => string;
}

export function evidenceText(item: unknown): string {
  if (typeof item === "string") return item;
  if (item && typeof item === "object") {
    const record = item as Record<string, unknown>;
    return String(record.title ?? record.summary ?? record.kind ?? "");
  }
  return String(item ?? "");
}

function displayName(
  id: string | null,
  principalNames: Record<string, string>
): string {
  if (!id) return "—";
  return principalNames[id] ?? id.slice(0, 8);
}

/**
 * 真模式决策卡：只展示审批记录 + 变更提案里真实存在的字段。
 * 候选方案、比较维度、AI 推荐理由在 P0 数据模型里没有，保持空数组，由页面隐藏。
 */
export function toDecision(
  record: DecisionRecord,
  principalNames: Record<string, string>,
  texts: DecisionTexts
): PageDecision {
  const titleParts = [record.command_type, record.target_type].filter(
    (part): part is string => Boolean(part)
  );
  return {
    id: record.id,
    title: titleParts.length ? titleParts.join(" · ") : texts.approvalTitle,
    question: record.rationale ?? texts.noRationale,
    status: record.status === "pending" ? "pending" : "decided",
    background: record.rationale ?? "",
    options: [],
    dimensions: [],
    evidence: (record.evidence ?? [])
      .map(evidenceText)
      .filter(item => item !== ""),
    recommendation: "",
    risks: record.risk_level ? [texts.riskLabel(record.risk_level)] : [],
    impact: record.required_role ? texts.requiredRole(record.required_role) : "",
    conclusion: record.decision_reason ?? undefined,
    decidedBy: record.decided_by
      ? displayName(record.decided_by, principalNames)
      : undefined,
    approvedAt: record.decided_at
      ? texts.formatDecidedAt(record.decided_at)
      : undefined,
    record
  };
}

export function principalNameMap(
  principals: Principal[]
): Record<string, string> {
  return Object.fromEntries(
    principals.map(principal => [principal.id, principal.display_name] as const)
  );
}