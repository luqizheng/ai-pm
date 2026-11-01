import { http } from "@/shell/api";

/**
 * 知识空间（V1 只读）。
 *
 * 契约：pm-api `GET /v1/pm/documents?project_id=` 返回 `{items}`（裸 payload）；
 * `GET /v1/pm/documents/{id}?project_id=` 返回文档身份 + 正文 Markdown。
 * 正文权威在 Git 知识仓库，后端读取时按记录的 content_hash 校验漂移。
 */
export type DocumentType =
  | "requirement"
  | "decision"
  | "architecture"
  | "knowledge";

export interface KnowledgeDocumentSummary {
  id: string;
  doc_key: string;
  doc_type: DocumentType;
  canonical_path: string;
  revision_no: number;
  updated_at: string;
}

export interface KnowledgeDocument {
  document_id: string;
  doc_key: string;
  doc_type: DocumentType;
  canonical_path: string;
  revision_no: number;
  git_commit_sha: string;
  content_hash: string;
  markdown: string;
}

export function listDocuments(projectId: string) {
  return http.get<{ items: KnowledgeDocumentSummary[] }>("/v1/pm/documents", {
    project_id: projectId
  });
}

export function getDocument(documentId: string, projectId: string) {
  return http.get<KnowledgeDocument>(`/v1/pm/documents/${documentId}`, {
    project_id: projectId
  });
}
/* ------------------------------------------------------------------ V2 */

export interface GateFailure {
  gate: string;
  code: string;
  detail: string;
}

export interface QualityGateResult {
  passed: boolean;
  duplicate: boolean;
  redline_kind: string | null;
  reindex_required: boolean;
  failures: GateFailure[];
}

export type ProposalStatus =
  | "draft"
  | "quality_gates_passed"
  | "redline_pending"
  | "confirmed"
  | "commit_pending"
  | "committed"
  | "rejected"
  | "rolled_back";

export interface DocumentProposal {
  id: string;
  doc_key: string;
  doc_type: DocumentType;
  status: ProposalStatus;
  quality_gate_result: QualityGateResult;
  redline_kind: string | null;
  change_summary: string | null;
  resulting_revision_id: string | null;
  created_at: string;
}

export interface DocumentIndexState {
  document_revision_id: string;
  provider: string;
  status: string;
  indexed_content_hash: string | null;
  revision_content_hash: string | null;
  indexed_at: string | null;
  last_error: string | null;
}

export interface KnowledgeReference {
  id: string;
  document_id: string;
  document_revision_id: string;
  section_anchor: string | null;
  section_title: string | null;
  start_line: number | null;
  reference_kind: string;
  source_kind: string;
  source_id: string | null;
  created_at: string;
}

export interface SubmitProposalPayload {
  project_id: string;
  doc_key: string;
  doc_type: DocumentType;
  markdown: string;
  change_summary?: string;
  source_event_id?: string;
  analysis_run_id?: string;
  source_ref?: string;
  redline_kind?: string;
  base_content_hash?: string;
  created_by: string;
}

export function listProposals(projectId: string, documentId?: string) {
  return http.get<{ items: DocumentProposal[] }>("/v1/pm/documents/proposals", {
    project_id: projectId,
    ...(documentId ? { document_id: documentId } : {})
  });
}

export function submitProposal(payload: SubmitProposalPayload) {
  return http.post<Record<string, unknown>>(
    "/v1/pm/documents/proposals",
    payload
  );
}

export function confirmProposal(
  proposalId: string,
  projectId: string,
  confirmedBy: string
) {
  return http.post<Record<string, unknown>>(
    `/v1/pm/documents/proposals/${proposalId}/confirm`,
    { project_id: projectId, confirmed_by: confirmedBy }
  );
}

export function vetoRevision(
  revisionId: string,
  projectId: string,
  vetoedBy: string,
  reason: string
) {
  return http.post<Record<string, unknown>>(
    `/v1/pm/documents/revisions/${revisionId}/veto`,
    { project_id: projectId, vetoed_by: vetoedBy, reason }
  );
}

export function listReferences(projectId: string, documentId: string) {
  return http.get<{ items: KnowledgeReference[] }>(
    `/v1/pm/documents/${documentId}/references`,
    { project_id: projectId }
  );
}

export function getIndexState(projectId: string, documentId: string) {
  return http.get<{ items: DocumentIndexState[] }>(
    `/v1/pm/documents/${documentId}/index-state`,
    { project_id: projectId }
  );
}