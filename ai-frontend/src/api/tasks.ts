import { http } from "@/api/base";

/**
 * S04 任务域端点（pm-api 契约，docs/scenarios/s04/ui）。
 *
 * 任务 CRUD 复用 pm.ts（listTasks / createTask / updateTask，revision 乐观锁），
 * 此处一并 re-export，让 S04 页面只依赖单一入口；本文件其余部分补
 * pm.ts 尚未覆盖的三组端点：
 *   - 依赖图（T-024/T-025）：环路由后端 `DEPENDENCY_CYCLE` 409 拒绝；
 *   - 任务↔仓库绑定（WP-C，只读呈现）；
 *   - 人工批量置 ready（E5：TaskReady 事件唯一入口，AI 调用 403）。
 */

export { createTask, listTasks, updateTask } from "./pm";

/** 依赖边（upstream / downstream 共用） */
export interface TaskDependencyEdge {
  id: string;
  title: string;
  status: string;
}

/** 单任务依赖图（后端逐任务聚合；项目级图接口待补） */
export interface TaskDependencyGraph {
  task_id: string;
  upstream: TaskDependencyEdge[];
  downstream: TaskDependencyEdge[];
}

export function listTaskDependencies(taskId: string) {
  return http.get<TaskDependencyGraph>(`/v1/pm/tasks/${taskId}/dependencies`);
}

export function addTaskDependency(taskId: string, dependsOnTaskId: string) {
  return http.post<TaskDependencyGraph>(`/v1/pm/tasks/${taskId}/dependencies`, {
    depends_on_task_id: dependsOnTaskId
  });
}

/** 204 幂等删除 */
export function removeTaskDependency(taskId: string, dependsOnTaskId: string) {
  return http.delete<unknown>(
    `/v1/pm/tasks/${taskId}/dependencies/${dependsOnTaskId}`
  );
}

/** 任务↔仓库绑定（WP-C）：仓库必须属于项目且已验证，role ∈ primary|secondary|knowledge */
export interface TaskRepositoryBinding {
  task_id: string;
  repository_id: string;
  repository_name: string;
  write_access: boolean;
  base_ref: string | null;
  role: string;
  path_scope: string | null;
  repository_verified: boolean;
}

export function listTaskRepositories(taskId: string) {
  return http.get<TaskRepositoryBinding[]>(
    `/v1/pm/tasks/${taskId}/repositories`
  );
}

/** mark-ready 结果：ready=本批迁移（各发一个 TaskReady）；skipped=非 draft/ready */
export interface MarkTasksReadyResult {
  ready: string[];
  already_ready: string[];
  skipped: string[];
}

/**
 * 人工批量 `draft → ready`（E2：AI 调用 Forbidden，无旁路）。
 * 整批确认必须走本端点而非 updateTask 直写——直写不产生 TaskReady 事件，
 * 任务永远不进执行调度。
 */
export function markTasksReady(projectId: string, taskIds: string[]) {
  return http.post<{ result: MarkTasksReadyResult }>(
    `/v1/pm/projects/${projectId}/tasks/mark-ready`,
    { task_ids: taskIds }
  );
}
