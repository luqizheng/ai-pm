import { http } from "@/api/base";

/**
 * 知识域 API（KB 文档库 + PC 认知对象 + 统一检索）。
 *
 * 三面一一对应后端：
 * - KB 文档：`/v1/kb/documents*`（kb 域管理面；确认门 = WP-0 真人 interactive）
 * - PC 对象：`/v1/pc/objects`（P 域只读管理面）
 * - 统一检索：`/v1/retrieval/query`（检索架构 V1.0；LLM 产计划、确定性代码执行）
 */

/* ── KB 文档 ──────────────────────────────────────────────────────────── */

export type KbDocStatus = "draft" | "published" | "rejected" | "empty";

export interface KbDocumentRow {
  document_id: string;
  doc_key: string;
  doc_type: string;
  revision_no: number;
  status: KbDocStatus;
  content_hash: string;
  published_at: string | null;
  confirmed_by: string | null;
  updated_at: string;
}

export interface KbRevisionRow {
  revision_id: string;
  revision_no: number;
  status: KbDocStatus;
  content_hash: string;
  confirmed_by: string | null;
  published_at: string | null;
  created_at: string;
}

export interface KbDocumentDetail {
  document: KbDocumentRow;
  content: string | null;
  revisions: KbRevisionRow[];
}

/** 项目 KB 文档列表（最新修订投影）。 */
export function listKbDocuments(projectId: string) {
  return http.get<{ items: KbDocumentRow[] }>("/v1/kb/documents", {
    project_id: projectId
  });
}

/** KB 文档详情（正文 + 修订历史）。 */
export function getKbDocument(documentId: string, projectId: string) {
  return http.get<KbDocumentDetail>(`/v1/kb/documents/${documentId}`, {
    project_id: projectId
  });
}

/**
 * 确认发布（draft → published；幂等重确认 200 不改原确认人）。
 *
 * 权限：真人 interactive 会话——API Key / AI 主体 403（后端门，前端按钮仅对
 * 真人登录态展示）。
 */
export function confirmKbRevision(
  documentId: string,
  revisionId: string,
  projectId: string
) {
  return http.post(
    `/v1/kb/documents/${documentId}/revisions/${revisionId}/confirm`,
    {},
    { params: { project_id: projectId } }
  );
}

/* ── PC 认知对象 ──────────────────────────────────────────────────────── */

export interface PcObjectRow {
  object_id: string;
  object_type: string;
  object_key: string | null;
  display_name: string;
  lifecycle_status: string;
  revision_no: number | null;
  change_summary: string | null;
  excerpt: string | null;
  updated_at: string;
}

/** 项目 PC 认知对象列表。 */
export function listPcObjects(projectId: string) {
  return http.get<{ items: PcObjectRow[] }>("/v1/pc/objects", {
    project_id: projectId
  });
}

/** PC 对象最新修订（详情形状，与后端 P3 展开投影一致）。 */
export interface PcRevisionDetail {
  revision_no: number;
  summary: string | null;
  content: string | null;
  analysis_mode: string | null;
  versioned: boolean;
}

/** PC 对象详情（latest_revision 含全文——列表 excerpt 只有 240 字截断）。 */
export interface PcObjectDetail {
  object_id: string;
  object_type: string;
  object_key: string | null;
  name: string;
  lifecycle_status: string;
  latest_revision: PcRevisionDetail | null;
}

/** PC 对象详情（前端行内展开；无修订的 identity 对象 latest_revision=null）。 */
export function getPcObject(objectId: string, projectId: string) {
  return http.get<PcObjectDetail>(`/v1/pc/objects/${objectId}`, {
    project_id: projectId
  });
}

/* ── 统一检索 ─────────────────────────────────────────────────────────── */

export interface RetrievalEvidenceItem {
  evidence_id: string;
  source_domain: "PC" | "KB" | "REQUIREMENT";
  source_id: string;
  revision_id: string | null;
  authority_status: string;
  title: string | null;
  content_excerpt: string;
  locator: string | null;
  freshness: "recent" | "within_year" | "older" | "unknown" | null;
  truncated: boolean;
}

export interface TraversalEdgeView {
  relationship_id: string;
  relationship_type: string;
  source_id: string;
  target_id: string;
  traversal_direction: "forward" | "reverse";
}

export interface RetrievalQueryResponse {
  plan_source: "llm_planner" | "deterministic_fallback";
  degraded: boolean;
  degradation_reason: string | null;
  validation: { checks: { id: string; passed: boolean; reason?: string }[] };
  evidence: {
    evidence: RetrievalEvidenceItem[];
    traversal: {
      edges: TraversalEdgeView[];
      gaps: { kind: string; detail: string }[];
    };
    gaps: { kind: string; detail: string }[];
    degradations: string[];
    budget_report: Record<string, number | boolean>;
  };
}

/**
 * 统一检索（唯一入口）。Planner 未开启时后端走确定性双域直查（degraded=true、
 * reason=planner_disabled）——端点始终可用。
 */
export function retrievalQuery(query: string, projectId: string) {
  return http.post<RetrievalQueryResponse>(
    "/v1/retrieval/query",
    { query },
    { params: { project_id: projectId } }
  );
}

/* ── PC 提案审批（P4 详查 / P5 决策；列表=管理面）──────────────────────── */

export interface PcProposalRow {
  proposal_id: string;
  title: string;
  status:
    | "draft"
    | "submitted"
    | "approved"
    | "rejected"
    | "withdrawn"
    | "expired"
    | "applied"
    | "failed";
  risk_level: string;
  confidence: string;
  origin_kind: string;
  expected_version_no: number;
  member_count: number;
  submitted_at: string | null;
  created_at: string;
}

/** 项目 PC 提案列表（status 可选过滤）。 */
export function listPcProposals(projectId: string, status?: string) {
  return http.get<{ items: PcProposalRow[] }>("/v1/pc/proposals", {
    project_id: projectId,
    ...(status ? { status } : {})
  });
}

/* ── PC 提案详情（既有 P4 端点；审批页签行内展开消费）──────────────────── */

/** 提案成员钉扎（对象/关系变更展示投影）。 */
export interface PcProposalMemberView {
  change_kind: string;
  object_id: string | null;
  object_revision_id: string | null;
  object_type: string | null;
  object_key: string | null;
  display_name: string | null;
  object_revision_no: number | null;
  change_summary: string | null;
  relationship_id: string | null;
  relationship_revision_id: string | null;
  relationship_type: string | null;
  relationship_revision_no: number | null;
}

/** 提案审批历史项。 */
export interface PcApprovalView {
  approval_id: string;
  status: string;
  proposal_content_hash: string;
  expected_version_no: number;
  decided_by: string | null;
  decided_by_name: string | null;
  decision_reason: string | null;
  decided_at: string | null;
  created_at: string;
}

/** 提案详情（P4；成员 + 审批历史 + 已发布版本引用）。 */
export interface PcProposalDetail {
  proposal_id: string;
  title: string;
  rationale: string;
  status: PcProposalRow["status"];
  origin_kind: string;
  governance_level: string;
  risk_level: string;
  confidence: string;
  expected_version_no: number;
  submitted_at: string | null;
  created_at: string;
  updated_at: string;
  members: PcProposalMemberView[];
  approvals: PcApprovalView[];
  content_hash: string;
  published_version_no: number | null;
}

/** 提案详情（P4 读面；项目作用域 404）。 */
export function getPcProposal(proposalId: string, projectId: string) {
  return http.get<PcProposalDetail>(`/v1/pc/proposals/${proposalId}`, {
    project_id: projectId
  });
}

/**
 * 提案决策（approve=批准并发布 / reject=驳回）。
 *
 * CAS：`expected_version_no` = 调用方看到的提案基线，失配 409。权限门在后端
 * （human + 治理授权；AI/Key 403）。
 */
export function decidePcProposal(
  proposalId: string,
  projectId: string,
  decision: {
    action: "approve" | "reject";
    note?: string;
    expected_version_no: number;
  }
) {
  return http.post(
    `/v1/pc/proposals/${proposalId}/decision`,
    {
      action: decision.action,
      ...(decision.note ? { note: decision.note } : {}),
      expected_version_no: decision.expected_version_no
    },
    { params: { project_id: projectId } }
  );
}

/* ── 摄入（解析→draft；等待真人确认）─────────────────────────────────── */

export type IngestKind = "markdown" | "text" | "docx";

export interface IngestResult {
  document_id: string;
  revision_id: string;
  revision_no: number;
  status: "draft";
}

/**
 * 摄入：解析（纯函数，后端）→ 家族追加 draft 修订。
 *
 * 红线③：解析只能产生 Draft——本端点不发 published 事件、不进检索；
 * 确认走 confirmKbRevision（真人门）。
 */
export function ingestKbDocument(
  projectId: string,
  input: {
    doc_key: string;
    doc_type: string;
    kind: IngestKind;
    content_b64: string;
  }
) {
  return http.post<IngestResult>("/v1/kb/documents/ingest", input, {
    params: { project_id: projectId }
  });
}
