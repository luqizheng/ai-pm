import { http } from "@/api/base";

/**
 * S02 需求域页面级读模型（pm-api `pm_requirement_read.rs`）。
 *
 * 领域语义（docs/ui/s02 UI-01 + S02 领域裁决）：
 * - **修订五态**：draft / pending_confirmation / approved / rejected /
 *   superseded；只有 approved 是正式依据。
 * - **需求 = 稳定身份；评审的是它的一版正文**（revision）。评审队列列的是
 *   `pending_confirmation` 修订，不是需求本身。
 * - **确认 ≠ 评审**：确认是人工动作（confirm/reject），已确认历史是另一面。
 * - **需求基线 = 全部已确认需求的当前正式修订集合**（S02 裁决 R3）——
 *   它是**派生视图**，不新建基线表，也不与 S03 的实施基线混用。
 * - **影响分析只列事实**（谁绑了哪一版、是否已过期），不下结论。
 */

/** 需求修订五态。 */
export type RequirementRevisionState =
  | "draft"
  | "pending_confirmation"
  | "approved"
  | "rejected"
  | "superseded";

/** 待评审修订行。 */
export interface ReviewProposal {
  revision_id: string;
  requirement_id: string;
  requirement_code: string | null;
  requirement_title: string | null;
  requirement_type: string | null;
  priority: string | null;
  version: number;
  state: RequirementRevisionState | string;
  content: Record<string, unknown>;
  content_hash: string;
  created_at: string;
  submitted_at: string | null;
  created_by: string;
  created_by_name: string | null;
  /** 前一正式版本号（供并排比较）。 */
  base_version: number | null;
  base_content_hash: string | null;
  /** 绑定到本需求的来源数（来源是需求级关联）。 */
  source_count: number;
}

/** 评审队列。 */
export interface RequirementReviewQueue {
  proposals: ReviewProposal[];
  by_type: Array<[string, number]>;
  by_priority: Array<[string, number]>;
}

/** 需求基线行。 */
export interface RequirementBaselineItem {
  requirement_id: string;
  code: string | null;
  title: string | null;
  requirement_type: string | null;
  priority: string | null;
  lifecycle_status: string | null;
  revision_id: string | null;
  version: number | null;
  state: string | null;
  content_hash: string | null;
  confirmed_at: string | null;
  confirmed_by: string | null;
  acceptance_criteria_count: number;
  source_count: number;
}

/** 需求基线（派生视图）。 */
export interface RequirementBaseline {
  items: RequirementBaselineItem[];
  confirmed_count: number;
  total_count: number;
  /** 有需求但未确认的数量（存在缺口时 > 0）。 */
  unconfirmed_count: number;
}

/** 影响面行：谁绑定了本需求。 */
export interface RequirementImpactRow {
  kind: "design" | "task" | string;
  target_id: string;
  code: string | null;
  title: string | null;
  /** 绑定的需求修订版本号。 */
  bound_version: number | null;
  /** 本需求当前正式版本号。 */
  current_version: number | null;
  /** 绑定的是否为当前正式版本；false = 已过期，需重新评估。 */
  is_current: boolean | null;
  /** 对端自身状态（设计修订状态 / 任务状态）。 */
  target_status: string | null;
}

/** 变更影响分析。 */
export interface RequirementImpact {
  requirement_id: string;
  code: string | null;
  title: string | null;
  current_version: number | null;
  designs: RequirementImpactRow[];
  tasks: RequirementImpactRow[];
  /** 过期绑定数（绑了已被取代的修订）。 */
  stale_binding_count: number;
}

/** `GET /v1/pm/projects/{project_id}/requirement-review-queue`：待裁决修订队列。 */
export function getRequirementReviewQueue(projectId: string) {
  return http.get<RequirementReviewQueue>(
    `/v1/pm/projects/${projectId}/requirement-review-queue`
  );
}

/** `GET /v1/pm/projects/{project_id}/requirement-baseline`：项目需求基线。 */
export function getRequirementBaseline(projectId: string) {
  return http.get<RequirementBaseline>(
    `/v1/pm/projects/${projectId}/requirement-baseline`
  );
}

/** `GET /v1/pm/requirements/{requirement_id}/impact`：变更影响分析。 */
export function getRequirementImpact(requirementId: string) {
  return http.get<RequirementImpact>(
    `/v1/pm/requirements/${requirementId}/impact`
  );
}

/** 修订正文里的 Markdown（content.markdown）。 */
export function revisionMarkdown(content: unknown): string {
  if (!content || typeof content !== "object") return "";
  const value = (content as Record<string, unknown>).markdown;
  return typeof value === "string" ? value : "";
}
