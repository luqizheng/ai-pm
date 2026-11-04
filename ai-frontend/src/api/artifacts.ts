import { http } from "@/api/base";
import type { Paged } from "./types";

/**
 * 产物验收读模型：`delivery.requirement_revision` ⋈ `delivery.requirement`。
 * 首版「产物」= 需求修订（AI 产出并经审批的需求正文版本）。
 */
export interface ArtifactRecord {
  id: string;
  requirement_id: string;
  requirement_code: string;
  requirement_title: string;
  version: number;
  state: string;
  content_hash: string;
  created_at: string;
  approved_by: string | null;
}

export function listArtifacts(
  params: { project_id?: string; page?: number; size?: number } = {}
) {
  return http.get<Paged<ArtifactRecord>>("/v1/pm/artifacts", params);
}
