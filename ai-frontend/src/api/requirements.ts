/**
 * S02-I1 需求核心 REST 契约（pm-api `/v1/pm/*`，统一服务 pm-application::requirement）。
 *
 * 契约裁决（S02-04 V1.2，2026-11-06 后端已落地）：
 *  - 生命周期链：draft → submit → pending_confirmation → confirm/reject（均终态）；
 *    draft→approved 直跳已废除（S02D-04），旧 `approveRevision` 端点已退役；
 *  - confirm 必须携带 `expected_content_hash`（审批人核准哈希，跨版本重放防线 §7.1），
 *    可选 CAS 双令牌 `expected_row_revision` / `expected_current_revision_id`（I1 §7，
 *    令牌值来自列表/详情返回的 `row_revision`）；漂移一律 409；
 *  - AI 主体与 api_key 鉴权一律 403（S02D-02 #2 + §7.2 A*）；MCP 无确认工具（H-15）；
 *  - `content_hash` = sha256(markdown)（S02D-16 单点 helper）。
 *
 * `toRequirementEntry` / `toEntryFromListItem` 把后端读模型映射为
 * `views/project/requirements/requirement-entry.ts` 的 UI 读模型。
 * 诚实缺位（对应切片未实施，不伪造数据）：
 *  - specification / acceptance_criteria（S02-I5）、sources / related（S02-I3）、
 *    discussions / analysis / ai_suggestions（AI 编排切片）→ 空数组占位；
 *  - `origin` 后端无列 → undefined（UI 契约允许缺省=未记录）；
 *  - 需求级 lifecycle（withdrawn/archived，S02D-04 三态）UI 读模型暂无该轴，
 *    只保留后端原值于 entry 外层调用方需要时自行读取（适配器不吞掉）。
 */

import { http } from "@/api/base";
import type {
  AcceptanceCriterion,
  AiSuggestion,
  AnalysisFinding,
  DiscussionEntry,
  RelatedItem,
  RequirementEntry as UiRequirementEntry,
  RequirementPriority,
  RequirementUiState,
  RevisionRecord,
  SourceItem
} from "@/views/project/requirements/requirement-entry";

// ────────────────────────────────────────────────────── 后端契约类型

/** 需求条目（pm-application::requirement::RequirementEntry） */
export interface RequirementCoreEntry {
  id: string;
  project_id: string;
  code: string;
  title: string;
  requirement_type: "functional" | "non_functional";
  priority: "low" | "normal" | "high";
  lifecycle_status: "active" | "withdrawn" | "archived";
  created_at: string;
  updated_at: string;
  /** CAS 双令牌之一：写命令回传可保并发安全 */
  row_revision: number;
  current_revision_id: string | null;
  current_version: number | null;
  current_content_hash: string | null;
  latest_revision_id: string | null;
  latest_version: number | null;
  latest_state: string | null;
  latest_submitted_at: string | null;
}

/** 修订视图（content 投影提取后的 markdown/summary） */
export interface RequirementCoreRevision {
  id: string;
  requirement_id: string;
  version: number;
  state:
    | "draft"
    | "pending_confirmation"
    | "approved"
    | "rejected"
    | "superseded";
  markdown: string | null;
  summary: string | null;
  content_hash: string;
  markdown_version_ref: string | null;
  base_current_revision_id: string | null;
  base_row_revision: number | null;
  submitted_at: string | null;
  created_by: string;
  created_by_name: string | null;
  approved_by: string | null;
  approved_by_name: string | null;
  created_at: string;
}

/** 确认记录（S02D-21：一次确认/拒绝的不可变事实） */
export interface RequirementCoreConfirmation {
  id: string;
  revision_id: string;
  action: "confirm" | "reject";
  reason: string | null;
  human_principal_id: string;
  human_display_name: string | null;
  source: string;
  content_hash: string | null;
  markdown_version_ref: string | null;
  confirmed_at: string;
}

export interface RequirementCoreDetail {
  requirement: RequirementCoreEntry;
  revisions: RequirementCoreRevision[];
  confirmations: RequirementCoreConfirmation[];
}

// ────────────────────────────────────────────────────────── 请求构造

export interface CreateRequirementInput {
  code: string;
  title: string;
  description?: string;
  requirement_type?: "functional" | "non_functional";
  priority?: "low" | "normal" | "high";
  markdown?: string;
}

export interface DraftRevisionInput {
  markdown: string;
  summary?: string;
}

export interface ConfirmRevisionInput {
  expected_content_hash: string;
  expected_current_revision_id?: string;
  expected_row_revision?: number;
}

// ─────────────────────────────────────────────────────────── 端点方法

export function listProjectRequirements(projectId: string) {
  return http.get<{ items: RequirementCoreEntry[] }>(
    `/v1/pm/projects/${projectId}/requirements`
  );
}

export function createRequirement(
  projectId: string,
  body: CreateRequirementInput
) {
  return http.post<{ requirement: RequirementCoreEntry }>(
    `/v1/pm/projects/${projectId}/requirements`,
    body
  );
}

export function getRequirement(requirementId: string) {
  return http.get<RequirementCoreDetail>(
    `/v1/pm/requirements/${requirementId}`
  );
}

export function createDraftRevision(
  requirementId: string,
  body: DraftRevisionInput
) {
  return http.post<{ revision: RequirementCoreRevision }>(
    `/v1/pm/requirements/${requirementId}/revisions`,
    body
  );
}

export function updateDraftRevision(
  requirementId: string,
  revisionId: string,
  body: DraftRevisionInput
) {
  return http.patch<{ revision: RequirementCoreRevision }>(
    `/v1/pm/requirements/${requirementId}/revisions/${revisionId}`,
    body
  );
}

export function submitRevision(requirementId: string, revisionId: string) {
  return http.post<{ revision: RequirementCoreRevision }>(
    `/v1/pm/requirements/${requirementId}/revisions/${revisionId}/submit`,
    {}
  );
}

export function confirmRevision(
  requirementId: string,
  revisionId: string,
  body: ConfirmRevisionInput
) {
  return http.post<{ requirement: RequirementCoreEntry }>(
    `/v1/pm/requirements/${requirementId}/revisions/${revisionId}/confirm`,
    body
  );
}

export function rejectRevision(
  requirementId: string,
  revisionId: string,
  reason?: string
) {
  return http.post<{ revision: RequirementCoreRevision }>(
    `/v1/pm/requirements/${requirementId}/revisions/${revisionId}/reject`,
    { reason }
  );
}

export function withdrawRequirement(requirementId: string) {
  return http.post<{ requirement: RequirementCoreEntry }>(
    `/v1/pm/requirements/${requirementId}/withdraw`,
    {}
  );
}

export function restoreRequirement(requirementId: string) {
  return http.post<{ requirement: RequirementCoreEntry }>(
    `/v1/pm/requirements/${requirementId}/restore`,
    {}
  );
}

export function archiveRequirement(requirementId: string) {
  return http.post<{ requirement: RequirementCoreEntry }>(
    `/v1/pm/requirements/${requirementId}/archive`,
    {}
  );
}

// ───────────────────────────────────────────────────── 后端 → UI 读模型适配

/** S02D-05：后端行级优先级为 low/normal/high；UI 词表用 medium 表达 normal。 */
function toUiPriority(
  priority: RequirementCoreEntry["priority"]
): RequirementPriority {
  return priority === "normal" ? "medium" : priority;
}

/** UI 状态派生：confirmed=最新修订已获正式确认；conflict 为 UI 派生态（后端无此源）。 */
function toUiState(entry: RequirementCoreEntry): RequirementUiState {
  switch (entry.latest_state) {
    case "approved":
      return "confirmed";
    case "pending_confirmation":
      return "pending_confirmation";
    default:
      return "draft";
  }
}

function toRevisionRecord(
  revision: RequirementCoreRevision,
  confirmations: RequirementCoreConfirmation[]
): RevisionRecord {
  const decision = confirmations.find(c => c.revision_id === revision.id);
  return {
    id: revision.id,
    version: revision.version,
    state: revision.state,
    summary: revision.summary ?? "",
    author: revision.created_by_name ?? "",
    created_at: revision.created_at,
    markdown: revision.markdown ?? "",
    // origin：后端无列（I3 来源切片时补）；UI 契约允许缺省=未记录
    decision:
      decision === undefined
        ? undefined
        : {
            kind: decision.action === "confirm" ? "confirmed" : "rejected",
            actor: decision.human_display_name ?? decision.human_principal_id,
            acted_at: decision.confirmed_at
          },
    audit: {
      initiated_by: revision.created_by_name ?? "",
      reason: revision.summary ?? ""
    }
  };
}

const EMPTY_CRITERIA: AcceptanceCriterion[] = [];
const EMPTY_SOURCES: SourceItem[] = [];
const EMPTY_DISCUSSIONS: DiscussionEntry[] = [];
const EMPTY_ANALYSIS: AnalysisFinding[] = [];
const EMPTY_SUGGESTIONS: AiSuggestion[] = [];
const EMPTY_RELATED: RelatedItem[] = [];

/** 详情适配：完整修订链 + 确认记录映射。 */
export function toRequirementEntry(
  detail: RequirementCoreDetail
): UiRequirementEntry {
  const entry = detail.requirement;
  const revisions = detail.revisions
    .slice()
    .sort((a, b) => a.version - b.version)
    .map(r => toRevisionRecord(r, detail.confirmations));
  // 描述 = 最早修订（v1）的 summary（创建时由 description 落入 content 投影）
  const description = revisions.length > 0 ? (revisions[0].summary ?? "") : "";
  return {
    id: entry.id,
    code: entry.code,
    title: entry.title,
    description,
    type: entry.requirement_type,
    priority: toUiPriority(entry.priority),
    status: toUiState(entry),
    source_label: "",
    created_at: entry.created_at,
    updated_at: entry.updated_at,
    specification: {
      business_rationale: "",
      expected_outcome: "",
      scope: "",
      explicit_exclusions: ""
    },
    acceptance_criteria: EMPTY_CRITERIA,
    revisions: revisions.slice().reverse(), // UI 契约：version 倒序
    sources: EMPTY_SOURCES,
    discussions: EMPTY_DISCUSSIONS,
    analysis: EMPTY_ANALYSIS,
    ai_suggestions: EMPTY_SUGGESTIONS,
    related: EMPTY_RELATED
  };
}

/** 列表行适配（修订链不在列表载荷内，打开详情时补齐）。 */
export function toEntryFromListItem(
  entry: RequirementCoreEntry
): UiRequirementEntry {
  return {
    id: entry.id,
    code: entry.code,
    title: entry.title,
    description: "",
    type: entry.requirement_type,
    priority: toUiPriority(entry.priority),
    status: toUiState(entry),
    source_label: "",
    created_at: entry.created_at,
    updated_at: entry.updated_at,
    specification: {
      business_rationale: "",
      expected_outcome: "",
      scope: "",
      explicit_exclusions: ""
    },
    acceptance_criteria: EMPTY_CRITERIA,
    revisions: [],
    sources: EMPTY_SOURCES,
    discussions: EMPTY_DISCUSSIONS,
    analysis: EMPTY_ANALYSIS,
    ai_suggestions: EMPTY_SUGGESTIONS,
    related: EMPTY_RELATED
  };
}
