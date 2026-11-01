import { http } from "@/shell/api";
import type { Paged, Project } from "./types";

/** `POST /v1/pm/projects`：建项目并自动落 5 人编制；传 customer_id 挂到已有客户上。 */
export function createProject(body: {
  code: string;
  name: string;
  description?: string;
  customer_id?: string;
}) {
  return http.post<Project>("/v1/pm/projects", body);
}

export function listProjects(
  params: { customer_id?: string; page?: number; size?: number } = {}
) {
  return http.get<Paged<Project>>("/v1/pm/projects", params);
}