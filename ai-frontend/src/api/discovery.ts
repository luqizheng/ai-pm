import { http } from "@/api/base";

export interface DiscoverySession {
  id: string;
  project_id: string;
  source: string;
  kind: string;
  status: string;
  created_by: string;
  created_at: string;
  updated_at: string;
  /** 最近一轮编排失败原因（成功一轮即清除；B03-01 失败可见性）。 */
  last_round_error: string | null;
  last_round_error_at: string | null;
}

export interface DiscoveryMessage {
  id: string;
  session_id: string;
  speaker: string;
  source: string;
  content: string;
  seq: number;
  created_at: string;
}

export interface DiscoveryEvidenceItem {
  repository?: string;
  file?: string;
  description?: string;
}

export interface DiscoveryFinding {
  id: string;
  session_id: string;
  kind: string;
  content: string;
  confidence: number | null;
  verification_status: string;
  evidence: DiscoveryEvidenceItem[];
  actor_role: string | null;
  repository_ids: string[];
  created_at: string;
}

export interface DiscoveryAnswer {
  id: string;
  question_id: string;
  answer: string;
  answered_by: string;
  created_at: string;
}

export interface DiscoveryQuestion {
  id: string;
  session_id: string;
  finding_id: string | null;
  question: string;
  why_blocking: string | null;
  status: string;
  created_at: string;
  answers: DiscoveryAnswer[];
}

export interface DiscoveryProposal {
  id: string;
  session_id: string;
  action: string;
  target_document: string;
  document_id: string | null;
  base_revision_id: string | null;
  proposed_content: string;
  evidence: DiscoveryEvidenceItem[];
  status: string;
  document_proposal_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface DiscoverySessionDetail {
  session: DiscoverySession;
  messages: DiscoveryMessage[];
  findings: DiscoveryFinding[];
  questions: DiscoveryQuestion[];
  proposals: DiscoveryProposal[];
}

export interface StartDiscoveryOutcome {
  session: DiscoverySession;
  questions_added: number;
  linked_import_id: string | null;
}

export interface FindingVerificationOutcome {
  finding: DiscoveryFinding;
  question: DiscoveryQuestion | null;
  answer: DiscoveryAnswer | null;
  confirmed_finding: DiscoveryFinding | null;
}

export interface ConfirmFindingPayload {
  status: string;
  answer: string;
  confirmed_finding?: string;
}

export function listDiscoverySessions(projectId: string) {
  return http.get<{ items: DiscoverySession[]; total: number }>(
    `/v1/pm/projects/${projectId}/discovery/sessions`
  );
}

/** Web 入口创建 source=WEB 的会话；带 import_id 时把草案 unknowns 映射为问题。 */
export function startDiscoverySession(projectId: string, importId?: string) {
  return http.post<StartDiscoveryOutcome>(
    `/v1/pm/projects/${projectId}/discovery/sessions`,
    importId ? { import_id: importId } : {}
  );
}

export function getDiscoverySession(projectId: string, sessionId: string) {
  return http.get<DiscoverySessionDetail>(
    `/v1/pm/projects/${projectId}/discovery/sessions/${sessionId}`
  );
}

export function transitionDiscoverySession(
  projectId: string,
  sessionId: string,
  status: string
) {
  return http.post<DiscoverySession>(
    `/v1/pm/projects/${projectId}/discovery/sessions/${sessionId}/status`,
    { status }
  );
}

export function confirmDiscoveryFinding(
  projectId: string,
  sessionId: string,
  findingId: string,
  payload: ConfirmFindingPayload
) {
  return http.post<FindingVerificationOutcome>(
    `/v1/pm/projects/${projectId}/discovery/sessions/${sessionId}/findings/${findingId}/verification`,
    payload
  );
}

export function answerDiscoveryQuestion(
  projectId: string,
  sessionId: string,
  questionId: string,
  answer: string
) {
  return http.post<DiscoveryAnswer>(
    `/v1/pm/projects/${projectId}/discovery/sessions/${sessionId}/questions/${questionId}/answers`,
    { answer }
  );
}

export function deferDiscoveryQuestion(
  projectId: string,
  sessionId: string,
  questionId: string
) {
  return http.post<DiscoveryQuestion>(
    `/v1/pm/projects/${projectId}/discovery/sessions/${sessionId}/questions/${questionId}/defer`,
    {}
  );
}
