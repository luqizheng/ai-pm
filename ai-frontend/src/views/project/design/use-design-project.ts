import { computed } from "vue";
import { useRoute } from "vue-router";

import { SCOPE_ALL, useScopeStore } from "@/api/scope";

/**
 * S03 工作空间的**项目上下文**（四页共用）。
 *
 * 口径同 Project Setting 与任务管理：路由 `?project_id=` 优先，回退全局
 * scope store（壳当前项目）。无项目上下文时返回空串——页面据此显示
 * 「请先选择项目」空态，而不是退化成「全部项目」的错聚合。
 *
 * ⚠ 哨兵值必须用 `SCOPE_ALL` 常量比对（值是 `"__all__"`，不是 `"all"`）——
 * scope store 在未选项目时返回该哨兵；写成字面量 "all" 会把哨兵当成
 * 真实项目 id 发给后端。
 *
 * S03 的所有读端点都是**项目级**的（设计 / ADR / 基线 / 审批收件箱），
 * 没有项目上下文就没有可读的事实。
 */
export function useDesignProject() {
  const route = useRoute();
  const scope = useScopeStore();

  const projectId = computed(() => {
    const fromQuery = route.query.project_id;
    if (typeof fromQuery === "string" && fromQuery) return fromQuery;
    const id = scope.currentProjectId;
    return id && id !== SCOPE_ALL ? id : "";
  });

  const hasProject = computed(() => projectId.value.length > 0);

  return { projectId, hasProject };
}
