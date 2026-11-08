import { http } from "@/api/base";

/**
 * 项目 AI Role 目录与启停（V2 D-1=A，迁移 0058）。
 *
 * 后端契约：pm-api `/v1/pm/projects/{project_id}/ai-roles*` ——
 * - GET  `/ai-roles`：目录合并视图（5 模板 LEFT JOIN 项目行）；项目行缺失 =
 *   `enabled: false`（懒生效语义，enable 时才建项目行 + 版本 1.0）。
 * - PUT  `/ai-roles/{role_id}/enabled`：body `{ enabled }`；写守卫 = 租户
 *   owner/admin 或项目 owner（否则 403）；禁用 project_manager → 409
 *   `AI_ROLE_NOT_DISABLEABLE`。
 * - GET  `/ai-roles/{role_id}/versions`：不可变版本历史（append-only，创建序）。
 * 错误体 `{message}`（@digua/http 收口为 HttpError；status 403/409 在页面判别）。
 */

/** Role 类目词表（pm-domain AiRoleCategory；蛇形小写回传） */
export type AiRoleCategory =
  | "management"
  | "product"
  | "knowledge"
  | "architecture"
  | "design"
  | "engineering"
  | "quality"
  | "governance";

/** 当前版本摘要（合并视图内嵌；无项目行时为 null） */
export interface AiRoleVersionSummary {
  id: string;
  version: string;
  created_at: string;
}

/** 目录合并视图行（ProjectAiRoleView 契约） */
export interface ProjectAiRole {
  role_id: string;
  role_key: string;
  name: string;
  category: AiRoleCategory;
  /** 项目行缺失时恒为 false（懒生效） */
  enabled: boolean;
  current_version: AiRoleVersionSummary | null;
}

/** 项目 Role 目录（五条模板固定返回，顺序 = 模板创建序） */
export function listProjectAiRoles(projectId: string) {
  return http.get<ProjectAiRole[]>(`/v1/pm/projects/${projectId}/ai-roles`);
}

/** 启停一个 Role；成功返回更新后的合并视图行 */
export function setProjectAiRoleEnabled(
  projectId: string,
  roleId: string,
  enabled: boolean
) {
  return http.put<ProjectAiRole>(
    `/v1/pm/projects/${projectId}/ai-roles/${roleId}/enabled`,
    { enabled }
  );
}

/** 不可变版本快照（全量 definition；历史只增不改） */
export interface AiRoleVersion {
  id: string;
  version: string;
  created_by: string | null;
  created_at: string;
  definition: unknown;
}

/** 某个 Role 的版本历史（append-only，按创建序） */
export function listProjectAiRoleVersions(projectId: string, roleId: string) {
  return http.get<AiRoleVersion[]>(
    `/v1/pm/projects/${projectId}/ai-roles/${roleId}/versions`
  );
}
