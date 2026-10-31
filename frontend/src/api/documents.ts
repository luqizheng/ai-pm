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