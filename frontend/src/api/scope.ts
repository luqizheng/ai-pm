import { computed, ref } from "vue";
import { defineStore } from "pinia";

import { listProjects } from "./pm";
import type { Project } from "./types";

/** ScopeSelect 里“全部项目”的哨兵值（reka-ui Select 不接受空串 value） */
export const SCOPE_ALL = "__all__";

/**
 * 全局项目范围：切换后各业务列表/统计自动按 project_id 过滤。
 * 空串/SCOPE_ALL 表示“全部项目”。
 */
export const useScopeStore = defineStore("pm-scope", () => {
  const projects = ref<Project[]>([]);
  const currentProjectId = ref<string>(SCOPE_ALL);
  const loaded = ref(false);

  /** ensureLoaded：单飞去重，失败不缓存（下次可重试） */
  let inflight: Promise<void> | null = null;

  async function ensureLoaded(): Promise<void> {
    if (loaded.value) return;
    if (!inflight) {
      inflight = listProjects({ page: 1, size: 200 })
        .then(result => {
          projects.value = result.items ?? [];
          loaded.value = true;
        })
        .finally(() => {
          inflight = null;
        });
    }
    await inflight;
  }

  /** 传给端点的 project_id 参数；全部项目时 undefined */
  const projectIdParam = computed(() =>
    currentProjectId.value === SCOPE_ALL || currentProjectId.value === ""
      ? undefined
      : currentProjectId.value
  );

  const currentProject = computed(
    () =>
      projects.value.find(project => project.id === currentProjectId.value) ??
      null
  );

  return {
    projects,
    currentProjectId,
    projectIdParam,
    currentProject,
    ensureLoaded
  };
});
