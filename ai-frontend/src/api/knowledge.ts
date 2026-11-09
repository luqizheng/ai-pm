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
