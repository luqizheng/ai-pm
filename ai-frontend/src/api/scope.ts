import { computed, ref } from "vue";
import { defineStore } from "pinia";

import { listProjects } from "./pm";
import type { Project } from "./types";
import { useShellStore } from "@/stores/shell";

/** ScopeSelect 里“全部项目”的哨兵值（reka-ui Select 不接受空串 value） */
export const SCOPE_ALL = "__all__";

/**
 * 全局项目范围：切换后各业务列表/统计自动按 project_id 过滤。
 * 空串/SCOPE_ALL 表示“全部项目”。
 *
 * **单一真源（整合计划 D-IC-3）**：范围跟随应用壳的当前项目（shell store
 * `currentProjectId`）——此前两者各自独立，壳顶栏切项目不会带动业务页的
 * project_id，导致「指令栏说 A 项目、页面查 B 项目」的双源分裂。现在
 * 壳为真源，本 store 只做派生。
 *
 * 路由 `?project_id=` 仍优先于本范围（各业务页自行判定），故本派生
 * 不覆盖显式路由上下文。
 */
export const useScopeStore = defineStore("pm-scope", () => {
  const projects = ref<Project[]>([]);
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

  /** 范围 = 壳当前项目；未选项目时为“全部项目”哨兵。 */
  const currentProjectId = computed<string>(() => {
    const shell = useShellStore();
    return shell.currentProjectId || SCOPE_ALL;
  });

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
