/**
 * S03 设计与技术决策 · 视图契约（docs/ui/s03 页面结构 · 总体设计 V1.0）。
 *
 * 领域语义裁决（防状态混淆，落到每个页面）：
 * - **设计修订五态**（design.design_revision.state）：
 *   draft / pending_confirmation / approved / rejected / superseded。
 *   「已批准」= approved；design.current_revision_id 指向的才是**当前正式版本**。
 * - **ADR 六态**（design.adr.status）：
 *   proposed / under_review / accepted / rejected / superseded / deprecated。
 *   accepted = 人类正式接受；blockers_implementation 的 accepted/under_review
 *   决策会阻塞实施（Readiness Gate 条件 3）。
 * - **审批三 kind**：design_revision / adr_acceptance / baseline_freeze；
 *   状态 pending / approved / rejected。
 * - **完成 ≠ 批准**：设计评审通过（revision approved）≠ 实施基线冻结
 *   （Readiness Gate 五条件全过 + 真人冻结）≠ 已交接 S04（任务引用基线）。
 *   三个层级在 UI 上必须可区分。
 * - **需求覆盖绑定修订**：`is_current=false` = 需求已被新版本取代而设计仍
 *   引用旧版 → 影响分析重点，不可当已覆盖。
 *
 * 色调映射走全局语义 token（prism 门禁禁止裸色值）。
 */

/** 与 StatusIndicator / Badge 同口径的语义档。 */
export type DesignTone =
  | "neutral"
  | "info"
  | "success"
  | "warning"
  | "danger"
  | "accent";

/** 设计修订五态 → 语义档。 */
const REVISION_TONES: Record<string, DesignTone> = {
  draft: "neutral",
  pending_confirmation: "warning",
  approved: "success",
  rejected: "danger",
  superseded: "neutral"
};

/** ADR 六态 → 语义档。 */
const ADR_TONES: Record<string, DesignTone> = {
  proposed: "neutral",
  under_review: "warning",
  accepted: "success",
  rejected: "danger",
  superseded: "neutral",
  deprecated: "neutral"
};

/** 审批状态 → 语义档。 */
const APPROVAL_TONES: Record<string, DesignTone> = {
  pending: "warning",
  approved: "success",
  rejected: "danger"
};

export function revisionTone(state: string | null | undefined): DesignTone {
  if (!state) return "neutral";
  return REVISION_TONES[state] ?? "neutral";
}

export function adrTone(status: string | null | undefined): DesignTone {
  if (!status) return "neutral";
  return ADR_TONES[status] ?? "neutral";
}

export function approvalTone(status: string | null | undefined): DesignTone {
  if (!status) return "neutral";
  return APPROVAL_TONES[status] ?? "neutral";
}

/** 评审问题严重度 → 语义档。 */
export function severityTone(severity: string | null | undefined): DesignTone {
  switch (severity) {
    case "blocker":
      return "danger";
    case "warning":
      return "warning";
    case "info":
      return "info";
    default:
      return "neutral";
  }
}

/** 需求修订状态 → 语义档（用于覆盖表）。 */
export function requirementStateTone(
  state: string | null | undefined
): DesignTone {
  switch (state) {
    case "approved":
      return "success";
    case "pending_confirmation":
      return "warning";
    case "rejected":
      return "danger";
    case "draft":
      return "neutral";
    default:
      return "neutral";
  }
}

/** UUID 短形（列表展示用；完整 UUID 仅在表单与关联中出现）。 */
export function shortId(id: string | null | undefined): string {
  if (!id) return "";
  return id.length > 8 ? id.slice(0, 8) : id;
}

/** 时间格式化（列表与时间线展示用）。 */
export function formatDateTime(value: string | null | undefined): string {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(undefined, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit"
  });
}

/**
 * 设计修订正文（content jsonb）里的 Markdown 正文。
 *
 * content 形状：{ markdown, summary?, technical_analysis?, implementation_scope?,
 * acceptance_criteria? }——摘要/技术分析/实施范围/验收标准都是可选的，
 * 缺省即「该设计没写这一段」，UI 不造占位内容。
 */
export function revisionMarkdown(content: unknown): string {
  if (!content || typeof content !== "object") return "";
  const value = (content as Record<string, unknown>).markdown;
  return typeof value === "string" ? value : "";
}

/** 设计修订摘要（content.summary）。 */
export function revisionSummary(content: unknown): string {
  if (!content || typeof content !== "object") return "";
  const value = (content as Record<string, unknown>).summary;
  return typeof value === "string" ? value : "";
}
