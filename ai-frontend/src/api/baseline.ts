import { http } from "@/api/base";

/** S01-05 · Project Baseline 客户端（契约 = pm-api pm_baseline.rs）。 */

export type BaselineCategory =
  | "identity" | "scope" | "technical" | "architecture"
  | "repository" | "standards" | "constraints";

export interface BaselineItem {
  id: string;
  category: BaselineCategory;
  content: string;
  evidence: Record<string, unknown>;
  confidence: "high" | "medium" | "low";
  state: "proposed" | "confirmed" | "rejected" | "investigation" | "deferred";
  decided_by: string | null;
  decided_at: string | null;
  decision_note: string | null;
}

export interface BaselineOverview {
  current_version: number | null;
  current_confirmed_at: string | null;
  history: { version: number; proposal_id: string; confirmed_at: string }[];
  proposal: { id: string; version: number; status: string; created_at: string } | null;
  items: BaselineItem[];
}

export function loadBaselineOverview(projectId: string) {
  return http.get<BaselineOverview>(`/v1/pm/projects/${projectId}/baseline`);
}

export function createBaselineProposal(projectId: string) {
  return http.post<{ proposal_id: string; version: number; item_count: number }>(
    `/v1/pm/projects/${projectId}/baseline/proposals`,
    {}
  );
}

export function decideBaselineItem(
  projectId: string,
  itemId: string,
  body: { action: "confirm" | "reject" | "defer" | "investigate"; content?: string; note?: string }
) {
  return http.post<{ id: string; state: BaselineItem["state"] }>(
    `/v1/pm/projects/${projectId}/baseline/items/${itemId}/decision`,
    body
  );
}

export function confirmBaseline(projectId: string) {
  return http.post<{ version: number; status: string }>(
    `/v1/pm/projects/${projectId}/baseline/confirm`,
    {}
  );
}
