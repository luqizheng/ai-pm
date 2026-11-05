import { http } from "@/api/base";

/**
 * S01-04 · AI-PM Cognition 客户端（V1 后端契约 = pm-api pm_cognition.rs）。
 *
 * 语义对齐设计稿：Run 由人类启动（Q2-C）；条目逐项独立决策（Q3-B，AI 无此
 * 入口——路由挂成员层）；确认即落项目知识（decided_by/at 为确认记录，§6.3）。
 */

export type CognitionDomain =
  | "identity"
  | "repository"
  | "domain"
  | "architecture"
  | "convention"
  | "evolution";

export type CognitionItemState =
  | "proposed"
  | "confirmed"
  | "rejected"
  | "investigation"
  | "deferred"
  | "superseded";

export interface CognitionRun {
  id: string;
  scope: Record<string, unknown>;
  item_count: number;
  confirmed_count: number;
  status: string;
  created_at: string;
}

export interface CognitionItem {
  id: string;
  domain: CognitionDomain;
  content: string;
  evidence: Record<string, unknown>;
  confidence: "high" | "medium" | "low";
  has_conflict: boolean;
  state: CognitionItemState;
  decided_by: string | null;
  decided_at: string | null;
  decision_note: string | null;
}

export interface CognitionOverview {
  runs: CognitionRun[];
  latest_run_id: string | null;
  items: CognitionItem[];
  knowledge_active: number;
  proposed: number;
}

export interface ProjectKnowledge {
  id: string;
  domain: CognitionDomain;
  content: string;
  version: number;
  source_item_id: string | null;
  confirmed_by: string;
  confirmed_at: string;
}

export type CognitionDecision = "confirm" | "reject" | "defer" | "investigate";

/** GET /v1/pm/projects/{id}/cognition —— 概览（runs + 最新 items + 统计） */
export function loadCognitionOverview(projectId: string) {
  return http.get<CognitionOverview>(`/v1/pm/projects/${projectId}/cognition`);
}

/** POST …/cognition/runs —— 人工启动认知 Run（Q2-C；V1 = 结构化事实派生） */
export function startCognitionRun(projectId: string) {
  return http.post<{ run_id: string; status: string; item_count: number }>(
    `/v1/pm/projects/${projectId}/cognition/runs`,
    {}
  );
}

/** POST …/items/{id}/decision —— 逐项决策（confirm 可带修订正文 = Edit & Confirm） */
export function decideCognitionItem(
  projectId: string,
  itemId: string,
  body: { action: CognitionDecision; content?: string; note?: string }
) {
  return http.post<{
    id: string;
    state: CognitionItemState;
    knowledge_id: string | null;
  }>(`/v1/pm/projects/${projectId}/cognition/items/${itemId}/decision`, body);
}

/** GET …/cognition/knowledge —— 已确认项目知识（Active） */
export function listProjectKnowledge(projectId: string) {
  return http.get<{ items: ProjectKnowledge[] }>(
    `/v1/pm/projects/${projectId}/cognition/knowledge`
  );
}
